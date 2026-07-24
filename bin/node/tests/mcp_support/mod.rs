//! an in-process node for the MCP e2e, over noded's shared in-proc daemon
//! testkit: a REAL `host::Host::genesis` with the agent registry, saga (the
//! registry's declared collaborator) and tasks, fronted by `noded::router` on a
//! local listener — the same shape as `bin/fs`'s harness, plus the modules the
//! tool plane's gate is built on.
//!
//! chat and pages are deliberately ABSENT. both need a commonware runtime
//! context to `init`, which would drag a deterministic runner into a test whose
//! subject is a subprocess talking http. what they would prove — that a
//! `ChatMsg` / `PageMsg` encodes to the module's wire shape — is proven in the
//! unit tests against the modules' own types. what only an e2e can prove is
//! that the CAP GATE and the SUBMIT path are real, and `tasks` proves both: a
//! granted write reaches consensus, a denied one never leaves the process.
//!
//! the `NodeCommand` actor this used to hand-mirror now lives ONCE in
//! `noded::testkit`; this harness just builds the host and hands it over.
#![allow(dead_code)]

use std::io::{BufRead, BufReader, Write as _};
use std::process::{Child, Command, Stdio};

use host::Host;
use noded::testkit::InProcDaemon;
use serde_json::{Value, json};

/// the agent every test registers, and the owner it is registered under. the
/// owner is the origin the tool plane's writes must land with — the assertion
/// that a run's writes are attributed to the person who owns the agent, not to
/// the daemon that happened to execute it.
pub const AGENT_ID: &str = "quackbot";
pub const OWNER: &str = "eddy";

pub struct Harness {
    // dropped BEFORE `dir` (fields drop in declaration order): the daemon's Drop
    // joins the actor, closing the host's forge/qmdb handles, so the tempdir is
    // removed only afterward.
    daemon: InProcDaemon,
    dir: tempfile::TempDir,
}

impl Harness {
    /// stand the node up and register `AGENT_ID` with exactly `allowed_actions`
    /// — the grant every assertion in the calling test is written against.
    pub fn start(allowed_actions: &[&str]) -> Self {
        Self::start_with_forge_read(allowed_actions, &[])
    }

    /// Stand the same real node up with an explicitly bounded Forge read cap.
    pub fn start_with_forge_read(allowed_actions: &[&str], forge_read: &[&str]) -> Self {
        let dir = tempfile::Builder::new()
            .prefix("ducktape mcp-e2e")
            .tempdir()
            .expect("harness tempdir");
        let forge_base = dir.path().join("forge");

        let daemon = InProcDaemon::start(
            move || {
                Host::genesis(vec![
                    // no commonware context in this sync closure, so the
                    // registry rides the in-memory store test double.
                    Box::new(agent::AgentModule::new(
                        "agent",
                        Box::new(sdk_testkit::MemStore::new()),
                        "saga",
                        None,
                    )),
                    Box::new(saga::SagaModule::new("saga")),
                    Box::new(tasks::Tasks::new("tasks")),
                    Box::new(dispatch::DispatchModule::new("dispatch", "saga")),
                    Box::new(tagging::TaggingModule::new(
            "tagging",
            Box::new(sdk_testkit::MemStore::new()),
        )),
                    Box::new(forge::Forge::init("forge", forge_base).expect("forge module")),
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
                .expect("genesis")
            },
            vec!["agent".into()],
        );

        let harness = Harness { daemon, dir };
        harness.register_agent(allowed_actions, forge_read);
        harness
    }

    pub fn node_url(&self) -> String {
        self.daemon.node_url()
    }

    /// register the agent under `OWNER` with the given grant. submitted with the
    /// owner as the external origin, which is what makes `AgentRecord.owner`
    /// `SagaOrigin::External(b"eddy")` — the value the tool plane reads back and
    /// submits its writes as.
    fn register_agent(&self, allowed_actions: &[&str], forge_read: &[&str]) {
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
                "caps": {"forge_read": forge_read},
            }
        });
        let reply = self.submit("agent", payload, OWNER);
        assert!(
            reply.get("height").is_some(),
            "registering the agent must commit a block, got {reply}"
        );
    }

    /// a `ducktape mcp` subprocess wired exactly as the node's provisioner wires
    /// one: the node url and the agent id in the environment, and NOTHING else.
    /// if the binary needed more than this to work, the provisioner would have
    /// to supply more than this — so the test wires only what production does.
    pub fn mcp(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_ducktape"));
        cmd.arg("mcp");
        cmd.env("DUCKTAPE_NODE", self.node_url())
            .env("DUCKTAPE_RUN_AGENT", AGENT_ID)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        cmd
    }

    /// A write-capable MCP shape with a scoped endpoint that is deliberately
    /// unavailable. Production creates this endpoint inside the provisioner;
    /// this read/query harness does not dispatch runs.
    pub fn mcp_with_action(&self, run_id: &str) -> Command {
        let mut cmd = self.mcp();
        cmd.env(
            "DUCKTAPE_RUN_ACTION_URL",
            "http://127.0.0.1:9/v1/run-action",
        )
        .env(
            "DUCKTAPE_RUN_ACTION_TOKEN",
            "abababababababababababababababababababababababababababababababab",
        )
        .env("DUCKTAPE_RUN_ID", run_id);
        cmd
    }

    /// a `ducktape mcp` subprocess with NO agent bound — the "started outside a
    /// provisioned run" shape.
    pub fn mcp_agentless(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_ducktape"));
        cmd.arg("mcp");
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
        let mut child: Child = cmd.spawn().expect("spawn ducktape mcp");
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
        let mut child = self.mcp().spawn().expect("spawn ducktape mcp");
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
        nettest::http_json(self.daemon.port(), "POST", path, Some(body)).1
    }
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
