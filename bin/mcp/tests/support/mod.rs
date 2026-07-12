//! an in-process node for the MCP e2e: a REAL `host::Host::genesis` with the
//! agent registry, saga (the registry's declared collaborator) and tasks, on a
//! dedicated actor thread, fronted by `noded::router` on a local listener —
//! the same shape as `bin/fs`'s harness, minus the files module and plus the
//! two the tool plane's gate is built on.
//!
//! chat and pages are deliberately ABSENT. both need a commonware runtime
//! context to `init`, which would drag a deterministic runner into a test whose
//! subject is a subprocess talking http. what they would prove — that a
//! `ChatMsg` / `PageMsg` encodes to the module's wire shape — is proven in the
//! unit tests against the modules' own types. what only an e2e can prove is
//! that the CAP GATE and the SUBMIT path are real, and `tasks` proves both: a
//! granted write reaches consensus, a denied one never leaves the process.
#![allow(dead_code)]

use std::io::{BufRead, BufReader, Read as _, Write as _};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use futures::StreamExt as _;
use futures::channel::mpsc;
use host::{BlockContext, Host};
use noded::{BlockSummary, ModuleCategory, ModuleStatus, NodeCommand, NodeHandle, NodeStatus};
use sdk::{Msg, Origin};
use serde_json::{Value, json};

/// the agent every test registers, and the owner it is registered under. the
/// owner is the origin the tool plane's writes must land with — the assertion
/// that a run's writes are attributed to the person who owns the agent, not to
/// the daemon that happened to execute it.
pub const AGENT_ID: &str = "quackbot";
pub const OWNER: &str = "eddy";

pub struct Harness {
    port: u16,
    dir: Option<tempfile::TempDir>,
    server: Option<JoinHandle<()>>,
    actor: Option<JoinHandle<()>>,
}

impl Harness {
    /// stand the node up and register `AGENT_ID` with exactly `allowed_actions`
    /// — the grant every assertion in the calling test is written against.
    pub fn start(allowed_actions: &[&str]) -> Self {
        let dir = tempfile::Builder::new()
            .prefix("ducktape-mcp-e2e")
            .tempdir()
            .expect("harness tempdir");
        let port = free_port();

        let (handle, cmd_rx, _events) = NodeHandle::channel();
        let actor = std::thread::Builder::new()
            .name("mcp-e2e-actor".into())
            .spawn(move || run_actor(cmd_rx))
            .expect("spawn actor");

        let server = std::thread::Builder::new()
            .name("mcp-e2e-server".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("server runtime");
                rt.block_on(async move {
                    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
                        .await
                        .expect("bind harness listener");
                    let _ = noded::serve(listener, handle).await;
                });
            })
            .expect("spawn server");

        let harness = Harness {
            port,
            dir: Some(dir),
            server: Some(server),
            actor: Some(actor),
        };
        harness.await_ready();
        harness.register_agent(allowed_actions);
        harness
    }

    pub fn node_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// register the agent under `OWNER` with the given grant. submitted with the
    /// owner as the external origin, which is what makes `AgentRecord.owner`
    /// `SagaOrigin::External(b"eddy")` — the value the tool plane reads back and
    /// submits its writes as.
    fn register_agent(&self, allowed_actions: &[&str]) {
        let mut actions: Vec<&str> = allowed_actions.to_vec();
        // the registry canonicalizes to a sorted, deduped set; submit it that
        // way so the record's bytes are the ones the test expects.
        actions.sort_unstable();
        actions.dedup();
        let payload = json!({
            "register_agent": {
                "agent_id": AGENT_ID,
                "display_name": "Quackbot",
                "capability": "codex",
                // no prompt pin: an agent IS its curated skills now, and this
                // one curates none — the tool plane is what it is here to use.
                "allowed_actions": actions,
            }
        });
        let reply = self.submit("agent", payload, OWNER);
        assert!(
            reply.get("height").is_some(),
            "registering the agent must commit a block, got {reply}"
        );
    }

    /// a `ducktape-mcp` subprocess wired exactly as the node's provisioner wires
    /// one: the node url and the agent id in the environment, and NOTHING else.
    /// if the binary needed more than this to work, the provisioner would have
    /// to supply more than this — so the test wires only what production does.
    pub fn mcp(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_ducktape-mcp"));
        cmd.env("DUCKTAPE_NODE", self.node_url())
            .env("DUCKTAPE_RUN_AGENT", AGENT_ID)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        cmd
    }

    /// a `ducktape-mcp` subprocess carrying a SESSION — the write-capable shape
    /// the provisioner produces once it has bound a key to a live run.
    ///
    /// the seed is hexed exactly as `agent_provision::session` hexes the real
    /// one, so the server rebuilds the same signer from it. no run is bound in
    /// this harness, which is the point: consensus must refuse the write, and it
    /// must be consensus that does it.
    pub fn mcp_with_session(&self, seed: [u8; 32], run_id: &str) -> Command {
        let mut cmd = self.mcp();
        cmd.env("DUCKTAPE_RUN_SESSION_KEY", hex(&seed))
            .env("DUCKTAPE_RUN_ID", run_id);
        cmd
    }

    /// a `ducktape-mcp` subprocess with NO agent bound — the "started outside a
    /// provisioned run" shape.
    pub fn mcp_agentless(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_ducktape-mcp"));
        cmd.env("DUCKTAPE_NODE", self.node_url())
            .env_remove("DUCKTAPE_RUN_AGENT")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        cmd
    }

    /// drive one `tools/call` through a real subprocess over stdio and return
    /// its `result`. the whole point of doing it this way rather than calling
    /// the handler in-process: it proves the FRAMING works — that a runner
    /// speaking MCP at this binary gets an answer back.
    pub fn call(&self, cmd: Command, tool: &str, arguments: Value) -> Value {
        self.session(cmd, &[json!({"name": tool, "arguments": arguments})])
            .pop()
            .expect("one call, one result")
    }

    /// a full MCP session: initialize, then one `tools/call` per entry, in
    /// order, down one stdin — the shape a real runner uses.
    pub fn session(&self, mut cmd: Command, calls: &[Value]) -> Vec<Value> {
        let mut child: Child = cmd.spawn().expect("spawn ducktape-mcp");
        let mut stdin = child.stdin.take().expect("stdin");
        let stdout = BufReader::new(child.stdout.take().expect("stdout"));

        let mut frames = vec![
            json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {}}).to_string(),
            // the notification a real client always sends — and which the server
            // must NOT answer. if it does, every id below reads off by one and
            // the test fails loudly.
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string(),
        ];
        for (i, params) in calls.iter().enumerate() {
            frames.push(
                json!({
                    "jsonrpc": "2.0",
                    "id": i + 1,
                    "method": "tools/call",
                    "params": params,
                })
                .to_string(),
            );
        }
        for frame in &frames {
            writeln!(stdin, "{frame}").expect("write frame");
        }
        stdin.flush().expect("flush");
        drop(stdin);

        let mut responses: Vec<Value> = Vec::new();
        for line in stdout.lines() {
            let line = line.expect("read a response line");
            if line.trim().is_empty() {
                continue;
            }
            responses.push(serde_json::from_str(&line).expect("a response is json"));
        }
        let _ = child.wait();

        assert_eq!(
            responses.len(),
            calls.len() + 1,
            "expected one initialize response plus one per call — a notification that got \
             answered would show up right here: {responses:#?}"
        );
        assert_eq!(responses[0]["id"], 0, "the first response is initialize's");
        responses
            .into_iter()
            .skip(1)
            .map(|r| r["result"].clone())
            .collect()
    }

    /// the `initialize` result, for the guide/protocol assertions.
    pub fn initialize(&self) -> Value {
        let mut child = self.mcp().spawn().expect("spawn ducktape-mcp");
        let mut stdin = child.stdin.take().expect("stdin");
        let mut stdout = BufReader::new(child.stdout.take().expect("stdout"));
        writeln!(
            stdin,
            "{}",
            json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {}})
        )
        .expect("write initialize");
        stdin.flush().expect("flush");
        let mut line = String::new();
        stdout.read_line(&mut line).expect("read initialize reply");
        drop(stdin);
        let _ = child.wait();
        let value: Value = serde_json::from_str(&line).expect("json");
        value["result"].clone()
    }

    /// submit an op straight at the node — the test's own seeding lane, and the
    /// oracle every write assertion is checked against.
    pub fn submit(&self, target: &str, payload: Value, origin: &str) -> Value {
        self.post(
            "/v1/submit",
            &json!({"target": target, "payload": payload, "origin": origin}),
        )
    }

    /// query the node directly — how a test reads back what the MCP server
    /// wrote, WITHOUT going through the MCP server that claims to have written
    /// it.
    pub fn query(&self, target: &str, query: Value) -> Value {
        self.post("/v1/query", &json!({"target": target, "query": query}))
    }

    fn post(&self, path: &str, body: &Value) -> Value {
        let body = body.to_string();
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .expect("timeout");
        let req = format!(
            "POST {path} HTTP/1.1\r\nhost: 127.0.0.1\r\ncontent-type: application/json\r\n\
             content-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(req.as_bytes()).expect("write request");
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).expect("read response");
        let text = String::from_utf8_lossy(&raw);
        let payload = text
            .split_once("\r\n\r\n")
            .map(|(_, b)| b)
            .unwrap_or_default();
        serde_json::from_str(payload).unwrap_or(Value::Null)
    }

    fn await_ready(&self) {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if http_status(self.port, "GET", "/v1/status") == Some(200) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "harness node never answered /v1/status on port {}",
                self.port
            );
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = http_status(self.port, "POST", "/v1/shutdown");
        if let Some(s) = self.server.take() {
            let _ = s.join();
        }
        if let Some(a) = self.actor.take() {
            let _ = a.join();
        }
        drop(self.dir.take());
    }
}

/// the actor loop: one block per submit, queries answered inline.
fn run_actor(mut cmd_rx: mpsc::Receiver<NodeCommand>) {
    // `runs` is here so a WRITE from the tool server reaches the module that
    // actually gates it. no run is ever dispatched in this harness — driving the
    // full engagement loop needs chat + a deterministic runtime context, and
    // `runs`'s own collaboration_loop e2e already does exactly that against a
    // REAL lease. what this harness proves is the other half: that a signed
    // AgentAction leaves the tool server, crosses the router, reaches `runs`, and
    // is refused BY CONSENSUS rather than by anything in the binary under test.
    let mut host = Host::genesis(vec![
        Box::new(agent::AgentModule::new("agent", "saga", None)),
        Box::new(saga::SagaModule::new("saga")),
        Box::new(tasks::Tasks::new("tasks")),
        Box::new(dispatch::DispatchModule::new("dispatch", "saga")),
        Box::new(tagging::TaggingModule::new("tagging")),
        Box::new(runs::RunsModule::new(
            "runs",
            "chat",
            "saga",
            "tagging",
            "dispatch",
            "agent",
            Some("tasks".into()),
            None,
        )),
    ])
    .expect("genesis");
    let mut height: u64 = 0;
    futures::executor::block_on(async move {
        while let Some(cmd) = cmd_rx.next().await {
            match cmd {
                NodeCommand::Submit {
                    target,
                    payload,
                    origin,
                    reply,
                } => {
                    let next = height + 1;
                    let ctx = BlockContext {
                        protocol_version: 0,
                        height: next,
                        consensus_time: next,
                        origin: Origin::External(origin),
                    };
                    let result = match host.submit_at(ctx, Msg { target, payload }).await {
                        Ok(out) => {
                            height = next;
                            Ok(BlockSummary {
                                height,
                                app_hash: noded::hex_root(&out.app_hash),
                            })
                        }
                        Err(err) => Err(err.to_string()),
                    };
                    let _ = reply.send(result);
                }
                // the signed-frame lane, FAITHFUL to both real binaries: the
                // origin is the frame's verified signer, never a caller string.
                // a stub that stamped a claimed origin here would reproduce the
                // exact defect this lane closes — an e2e passing on attribution
                // production cannot produce.
                NodeCommand::SubmitFrame { frame, reply } => {
                    let result = match node::decode_frame(&frame) {
                        Ok((origin, msg)) => {
                            let next = height + 1;
                            let ctx = BlockContext {
                                protocol_version: 0,
                                height: next,
                                consensus_time: next,
                                origin,
                            };
                            match host.submit_at(ctx, msg).await {
                                Ok(out) => {
                                    height = next;
                                    Ok(BlockSummary {
                                        height,
                                        app_hash: noded::hex_root(&out.app_hash),
                                    })
                                }
                                Err(err) => Err(err.to_string()),
                            }
                        }
                        // a forged/tampered frame is a rejection, not a block.
                        Err(err) => Err(err.to_string()),
                    };
                    let _ = reply.send(result);
                }
                NodeCommand::Query { target, req, reply } => {
                    let result = host.query(&target, &req).await.map_err(|e| e.to_string());
                    let _ = reply.send(result);
                }
                NodeCommand::Status { reply } => {
                    let _ = reply.send(NodeStatus {
                        version: env!("CARGO_PKG_VERSION").into(),
                        app_hash: noded::hex_root(&host.app_hash()),
                        height,
                        modules: vec![ModuleStatus {
                            id: "agent".into(),
                            root: host
                                .module_root("agent")
                                .map(|r| noded::hex_root(&r))
                                .unwrap_or_default(),
                            category: ModuleCategory::of("agent"),
                        }],
                        public_key: String::new(),
                    });
                }
                NodeCommand::Metrics { reply } => {
                    let _ = reply.send(String::new());
                }
            }
        }
    });
}

/// the text a `tools/call` result carries, and whether it was a refusal.
pub fn content(result: &Value) -> (bool, String) {
    let is_error = result["isError"].as_bool().unwrap_or(false);
    let text = result["content"][0]["text"].as_str().unwrap_or("").to_string();
    (is_error, text)
}

/// a successful tool result's json payload, parsed back out of the content text.
pub fn payload(result: &Value) -> Value {
    let (is_error, text) = content(result);
    assert!(!is_error, "expected a success, got a refusal: {text}");
    serde_json::from_str(&text).expect("a successful tool result carries json")
}

/// the session key's wire form: lowercase hex of the 32-byte seed, exactly as
/// `agent_provision::session` writes it into the run's environment.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind port probe")
        .local_addr()
        .expect("probe addr")
        .port()
}

fn http_status(port: u16, method: &str, path: &str) -> Option<u16> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let req = format!(
        "{method} {path} HTTP/1.1\r\nhost: 127.0.0.1\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).ok()?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).ok()?;
    let text = String::from_utf8_lossy(&raw);
    text.split_whitespace().nth(1).and_then(|s| s.parse().ok())
}
