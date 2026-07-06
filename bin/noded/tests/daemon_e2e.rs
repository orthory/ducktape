//! daemon e2e: a REAL spawned `ducktape-noded` process driven over its actual
//! http/ws surface — the seam every app build (web, desktop sidecar) dials.
//! `tests/router.rs` covers the axum wiring against a FAKE actor; this suite
//! is the other half: real genesis, real `Host::submit_at` blocks, real
//! broadcast fan-out, real storage persistence across a restart.
//!
//! transport is deliberately raw std-TCP http/1.1 (plus a minimal ws client):
//! the daemon's whole point is that ANY plain http client is a full citizen —
//! if this file needs a feature a hand-rolled client can't express, the wire
//! has drifted from that promise.

use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// a running daemon, killed on drop so failures never leak an orphan (the
/// REAL orphan lifecycle — outliving a client — is the desktop shell's
/// contract with a detached spawn; this harness owns its child instead).
struct Daemon {
    child: Child,
    port: u16,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Daemon {
    /// every spawn gets an EXPLICIT storage dir: the daemon's default is
    /// temp_dir()/ducktape-noded-{pid}, which the process never cleans up —
    /// a leaked dir plus a recycled pid would reopen stale qmdb state and
    /// fail this suite spuriously.
    fn spawn(storage: &Path) -> Self {
        Self::spawn_inner(storage, false, &[])
    }

    fn spawn_with_echo_oracle(storage: &Path) -> Self {
        Self::spawn_inner(storage, true, &[])
    }

    fn spawn_inner(storage: &Path, echo_oracle: bool, env: &[(String, String)]) -> Self {
        let port = free_port();
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_ducktape-noded"));
        cmd.arg("--listen")
            .arg(format!("127.0.0.1:{port}"))
            .arg("--storage")
            .arg(storage)
            .stdout(Stdio::null())
            // startup failures (port stolen in the free_port window, bad
            // storage) land on stderr — keep it visible or they read as an
            // opaque readiness timeout.
            .stderr(Stdio::inherit());
        if echo_oracle {
            cmd.env("DUCKTAPE_NODED_ECHO_ORACLE", "1");
        }
        for (key, value) in env {
            cmd.env(key, value);
        }
        let child = cmd.spawn().expect("spawn ducktape-noded");
        let mut daemon = Self { child, port };
        // readiness = a status answer, never the listen println: the daemon
        // prints before binding, and status only answers once genesis is done.
        daemon.await_status();
        daemon
    }

    fn await_status(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Ok((200, _)) = self.try_request("GET", "/v1/status", None) {
                return;
            }
            if let Some(status) = self.child.try_wait().expect("poll daemon") {
                panic!("daemon exited during startup ({status}) — see stderr above");
            }
            assert!(
                Instant::now() < deadline,
                "daemon on port {} never answered /v1/status",
                self.port
            );
            std::thread::sleep(Duration::from_millis(150));
        }
    }

    fn try_request(
        &self,
        method: &str,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> std::io::Result<(u16, serde_json::Value)> {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port))?;
        stream.set_read_timeout(Some(Duration::from_secs(15)))?;
        let body_bytes = body
            .map(|b| serde_json::to_vec(b).expect("request body serializes"))
            .unwrap_or_default();
        let req = format!(
            "{method} {path} HTTP/1.1\r\nhost: 127.0.0.1\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            body_bytes.len()
        );
        stream.write_all(req.as_bytes())?;
        stream.write_all(&body_bytes)?;
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw)?;
        let text = String::from_utf8_lossy(&raw);
        let status: u16 = text
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let payload = text
            .split("\r\n\r\n")
            .nth(1)
            .map(parse_http_body)
            .unwrap_or(serde_json::Value::Null);
        Ok((status, payload))
    }

    fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> (u16, serde_json::Value) {
        self.try_request(method, path, body)
            .expect("daemon reachable")
    }

    fn submit(
        &self,
        target: &str,
        payload: serde_json::Value,
        origin: Option<&str>,
    ) -> (u16, serde_json::Value) {
        self.request(
            "POST",
            "/v1/submit",
            Some(&serde_json::json!({
                "target": target,
                "payload": payload,
                "origin": origin,
            })),
        )
    }

    fn query(&self, target: &str, query: serde_json::Value) -> serde_json::Value {
        let (status, reply) = self.request(
            "POST",
            "/v1/query",
            Some(&serde_json::json!({ "target": target, "query": query })),
        );
        assert_eq!(status, 200, "query {target} failed: {reply}");
        reply
    }

    fn status(&self) -> serde_json::Value {
        let (status, reply) = self.request("GET", "/v1/status", None);
        assert_eq!(status, 200, "status failed: {reply}");
        reply
    }

    /// GET /metrics as raw OpenMetrics text (not json — the scrape body is a
    /// text exposition, so reuse the byte lane and utf-8 decode it).
    fn metrics(&self) -> String {
        let (status, body) = self.request_bytes("GET", "/metrics", &[]);
        assert_eq!(status, 200, "metrics failed");
        String::from_utf8(body).expect("metrics body is utf-8")
    }

    /// raw-byte request for the blob lane: returns status + the response body
    /// BYTES exactly as received. the json helpers above lossy-decode the
    /// whole response as utf-8, which would corrupt binary chunk bodies.
    fn request_bytes(&self, method: &str, path: &str, body: &[u8]) -> (u16, Vec<u8>) {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("daemon reachable");
        stream
            .set_read_timeout(Some(Duration::from_secs(15)))
            .expect("read timeout");
        let head = format!(
            "{method} {path} HTTP/1.1\r\nhost: 127.0.0.1\r\ncontent-type: application/octet-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(head.as_bytes()).expect("write head");
        // best-effort body write: the daemon may legally answer 413 and stop
        // reading mid-body, which can surface here as a broken pipe.
        let _ = stream.write_all(body);
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).expect("read response");
        // split head/body at the byte level — chunk bytes must round-trip
        // untouched, so no utf-8 decoding of the body.
        let split = raw
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .expect("http header terminator");
        let status_line = String::from_utf8_lossy(&raw[..split]);
        let status: u16 = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        (status, raw[split + 4..].to_vec())
    }

    /// open /v1/ws with a minimal rfc6455 client handshake and return the
    /// stream positioned after the 101 response.
    fn ws_connect(&self) -> BufReader<TcpStream> {
        let stream = TcpStream::connect(("127.0.0.1", self.port)).expect("ws connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .expect("ws read timeout");
        let mut stream = stream;
        let req = format!(
            "GET /v1/ws HTTP/1.1\r\nhost: 127.0.0.1\r\nupgrade: websocket\r\nconnection: upgrade\r\nsec-websocket-key: ZHVja3RhcGUtZTJlLXdzLWtleQ==\r\nsec-websocket-version: 13\r\n\r\n"
        );
        stream
            .write_all(req.as_bytes())
            .expect("ws handshake write");
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).expect("ws handshake status");
        assert!(line.contains("101"), "ws upgrade rejected: {line}");
        // drain the rest of the handshake headers.
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).expect("ws handshake header");
            if header == "\r\n" {
                break;
            }
        }
        reader
    }

    /// read one server->client text frame (unfragmented, unmasked — what the
    /// daemon sends for block events).
    fn ws_read_text(reader: &mut BufReader<TcpStream>) -> String {
        let mut head = [0u8; 2];
        reader.read_exact(&mut head).expect("ws frame head");
        assert_eq!(head[0] & 0x0f, 0x1, "expected a text frame");
        let mut len = (head[1] & 0x7f) as u64;
        if len == 126 {
            let mut ext = [0u8; 2];
            reader.read_exact(&mut ext).expect("ws extended len");
            len = u16::from_be_bytes(ext) as u64;
        } else if len == 127 {
            let mut ext = [0u8; 8];
            reader.read_exact(&mut ext).expect("ws extended len");
            len = u64::from_be_bytes(ext);
        }
        let mut payload = vec![0u8; len as usize];
        reader.read_exact(&mut payload).expect("ws frame payload");
        String::from_utf8(payload).expect("ws text frame is utf-8")
    }

    /// a websocket-upgrade GET that expects an http REFUSAL, not a 101: sends
    /// the full rfc6455 handshake so the request reaches the handler body
    /// (axum's extractor stops a plain GET before the handler can say why),
    /// then returns whatever status + raw response text the daemon answers.
    /// a refusal leaves the connection open (keep-alive), so the read is
    /// timeout-bounded instead of read-to-close.
    fn ws_upgrade_refusal(&self, path: &str) -> (u16, String) {
        let mut stream =
            TcpStream::connect(("127.0.0.1", self.port)).expect("daemon reachable");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("read timeout");
        let req = format!(
            "GET {path} HTTP/1.1\r\nhost: 127.0.0.1\r\nupgrade: websocket\r\nconnection: upgrade\r\nsec-websocket-key: ZHVja3RhcGUtZTJlLXdzLWtleQ==\r\nsec-websocket-version: 13\r\n\r\n"
        );
        stream.write_all(req.as_bytes()).expect("write request");
        let mut raw = Vec::new();
        // read_to_end keeps what arrived before the timeout error — exactly
        // the refusal head + body on a connection the server holds open.
        let _ = stream.read_to_end(&mut raw);
        let text = String::from_utf8_lossy(&raw).into_owned();
        let status = text
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        (status, text)
    }
}

fn parse_http_body(body: &str) -> serde_json::Value {
    // axum replies with content-length (no chunking) for these routes; the
    // split above already isolated the body.
    serde_json::from_str(body.trim()).unwrap_or(serde_json::Value::Null)
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind port probe")
        .local_addr()
        .expect("probe addr")
        .port()
}

fn post_message(channel: &str, message_id: &str, text: &str) -> serde_json::Value {
    serde_json::json!({
        "post_message": {
            "channel_id": channel,
            "message_id": message_id,
            "blocks": [{ "paragraph": [{ "text": text, "marks": [] }] }],
            "thread": null,
            "as_agent": null,
        }
    })
}

fn post_mention(channel: &str, message_id: &str, agent_id: &str) -> serde_json::Value {
    serde_json::json!({
        "post_message": {
            "channel_id": channel,
            "message_id": message_id,
            "blocks": [{
                "paragraph": [
                    { "text": "hey ", "marks": [] },
                    {
                        "text": format!("@{agent_id}"),
                        "marks": [{
                            "mention": {
                                "agent": { "module": "runs", "agent_id": agent_id }
                            }
                        }]
                    },
                    { "text": " can you handle this?", "marks": [] }
                ]
            }],
            "thread": null,
            "as_agent": null,
        }
    })
}

/// the embedded daemon runs no mesh, so it never wires a call hub — which
/// makes the real binary exactly the no-hub case /v1/call/ws must refuse
/// LOUDLY: 503 at upgrade with a body that says why (the #178 posture — every
/// refusal path explains itself), never a silent hang. the replaced
/// /v1/voice/ws route is gone outright (app and node ship lockstep): 404.
#[test]
fn call_ws_without_a_hub_refuses_with_a_reason() {
    let storage = tempfile::TempDir::new().expect("storage dir");
    let daemon = Daemon::spawn(storage.path());

    let (status, raw) = daemon.ws_upgrade_refusal("/v1/call/ws?channel=general");
    assert_eq!(status, 503, "no call hub → refused at upgrade: {raw}");
    assert!(
        raw.contains("no mesh call hub"),
        "refusal says WHY: {raw}"
    );

    let (status, _raw) = daemon.ws_upgrade_refusal("/v1/voice/ws?channel=general");
    assert_eq!(status, 404, "the old voice route is unrouted, not refused");
}

#[test]
fn full_surface_blocks_authorship_and_ws() {
    let storage = tempfile::TempDir::new().expect("storage dir");
    let daemon = Daemon::spawn(storage.path());

    // status at genesis: build version, height 0, every registered module root.
    let status = daemon.status();
    assert_eq!(status["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(status["height"], 0);
    let modules: Vec<&str> = status["modules"]
        .as_array()
        .expect("modules array")
        .iter()
        .map(|m| m["id"].as_str().expect("module id"))
        .collect();
    assert_eq!(
        modules,
        [
            "chat",
            "saga",
            "dispatch",
            "tagging",
            "tasks",
            "inbox",
            "automations",
            "jobs",
            "agent",
            "runs",
            "pages",
            "forge",
            "files",
            "memory",
            "profiles"
        ]
    );
    let genesis_hash = status["appHash"].as_str().expect("appHash").to_string();

    // subscribe BEFORE submitting: every committed block must fan out.
    let mut ws = daemon.ws_connect();

    // one msg = one block; the summary echoes the new height + app-hash.
    let (code, block) = daemon.submit(
        "chat",
        serde_json::json!({
            "create_channel": { "channel_id": "general", "name": "General", "post_policy": "open" }
        }),
        None,
    );
    assert_eq!(code, 200, "create channel failed: {block}");
    assert_eq!(block["height"], 1);
    assert_ne!(block["appHash"].as_str(), Some(genesis_hash.as_str()));

    let (code, block) = daemon.submit(
        "chat",
        post_message("general", "m1", "hello from e2e"),
        Some("eddy"),
    );
    assert_eq!(code, 200, "post failed: {block}");
    assert_eq!(block["height"], 2);

    // the ws stream carries one tagged Block frame per committed block, in
    // order. classify by `type` — unknown kinds are a wire regression here.
    let mut block_heights: Vec<u64> = Vec::new();
    while block_heights.len() < 2 {
        let frame: serde_json::Value =
            serde_json::from_str(&Daemon::ws_read_text(&mut ws)).expect("ws frame json");
        match frame["type"].as_str() {
            Some("block") => block_heights.push(frame["height"].as_u64().expect("block height")),
            other => panic!("unexpected ws frame type: {other:?}"),
        }
    }
    assert_eq!(block_heights, [1, 2], "both blocks fan out in order");

    // committed state reads back; authorship derived from the submit origin.
    let reply = daemon.query(
        "chat",
        serde_json::json!({ "messages_latest": { "channel_id": "general", "limit": 16 } }),
    );
    let messages = reply["messages"].as_array().expect("Messages reply");
    assert_eq!(messages.len(), 1);
    let head = &messages[0]["head"];
    assert_eq!(head["message_id"], "m1");
    assert_eq!(head["blocks"][0]["paragraph"][0]["text"], "hello from e2e");
    let author_bytes: Vec<u8> = head["author"]["user"]
        .as_array()
        .expect("User author")
        .iter()
        .map(|v| v.as_u64().expect("byte") as u8)
        .collect();
    assert_eq!(
        author_bytes, b"eddy",
        "authorship must come from the submit origin"
    );

    // a deterministic rejection is a clean 400, not a dead daemon.
    let (code, err) = daemon.submit("no-such-module", serde_json::json!({"Nope": {}}), None);
    assert_eq!(code, 400, "unknown target must reject: {err}");
    daemon.status(); // still alive, still answering.
}

#[test]
fn agent_run_drains_oracle_effect_and_posts_reply() {
    let storage = tempfile::TempDir::new().expect("storage dir");
    let daemon = Daemon::spawn_with_echo_oracle(storage.path());

    let (code, block) = daemon.submit(
        "chat",
        serde_json::json!({
            "create_channel": { "channel_id": "general", "name": "General", "post_policy": "open" }
        }),
        Some("owner"),
    );
    assert_eq!(code, 200, "create channel failed: {block}");

    // no prompt ref: a missing/null `prompt` keeps the runs module's generic
    // default prompt — the exact wire shape a minimal client submits.
    let (code, block) = daemon.submit(
        "agent",
        serde_json::json!({
            "register_agent": {
                "agent_id": "quackbot",
                "display_name": "Quackbot",
                "capability": "echo-model",
                "prompt": null,
                "allowed_actions": ["chat.post"]
            }
        }),
        Some("owner"),
    );
    assert_eq!(code, 200, "register agent failed: {block}");

    let (code, block) = daemon.submit(
        "runs",
        serde_json::json!({
            "watch_channel": {
                "channel_id": "general",
                "policy": "mention"
            }
        }),
        Some("owner"),
    );
    assert_eq!(code, 200, "watch channel failed: {block}");

    let (code, block) = daemon.submit(
        "chat",
        post_mention("general", "m1", "quackbot"),
        Some("eddy"),
    );
    assert_eq!(code, 200, "mention post failed: {block}");
    // the receipt reports the block that INCLUDED the post, not the drain tail…
    assert_eq!(
        block["height"], 4,
        "the receipt should carry the post's inclusion block"
    );
    // …while the drain tail runs behind it: the oracle follow-up block (5)
    // commits the result into the dispatch mailbox, and the nudge block (6)
    // carries the DeliverPending injection that posts the reply — the
    // never-pop-stack rule made visible in the block arithmetic.
    assert_eq!(
        daemon.status()["height"],
        6,
        "post + oracle follow-up + delivery nudge should all drain"
    );

    let run_id = "chat\u{1f}general\u{1f}1\u{1f}quackbot";
    // the run's lifecycle lives in the dispatch module; the runs module's
    // pending entry pruned when the delivery landed.
    let pending = daemon.query("runs", serde_json::json!("pending_runs"));
    assert_eq!(
        pending["pending_runs"].as_array().map(Vec::len),
        Some(0),
        "the delivered run must leave no pending entry: {pending}"
    );
    let dispatch = daemon.query(
        "dispatch",
        serde_json::json!({
            "dispatch": {
                "receiver": "runs",
                "dispatch_id": runs::dispatch_id_for(run_id),
            }
        }),
    );
    assert_eq!(
        dispatch["dispatch"]["status"], "delivered",
        "the dispatch record is the run's history: {dispatch}"
    );

    let reply = daemon.query(
        "chat",
        serde_json::json!({ "messages_latest": { "channel_id": "general", "limit": 16 } }),
    );
    let messages = reply["messages"].as_array().expect("Messages reply");
    assert_eq!(messages.len(), 2, "user post plus agent reply should exist");
    let agent_reply = &messages[1]["head"];
    assert_eq!(agent_reply["message_id"], format!("agent/{run_id}"));
    assert_eq!(
        agent_reply["author"],
        serde_json::json!({ "agent": { "module": "runs", "agent_id": "quackbot" } })
    );
    let text = agent_reply["blocks"][0]["paragraph"][0]["text"]
        .as_str()
        .expect("reply text");
    assert!(
        text.starts_with("echo: handling dispatch "),
        "the reply is the echo worker's dispatch-lane answer, normalized \
         into a paragraph by the runs module: {text}"
    );
}

#[test]
fn state_persists_across_restart() {
    let storage = tempfile::TempDir::new().expect("storage dir");

    {
        let daemon = Daemon::spawn(storage.path());
        let (code, _) = daemon.submit(
            "chat",
            serde_json::json!({
                "create_channel": { "channel_id": "durable", "name": "Durable", "post_policy": "open" }
            }),
            None,
        );
        assert_eq!(code, 200);
        let (code, _) = daemon.submit(
            "chat",
            post_message("durable", "m1", "written before restart"),
            Some("eddy"),
        );
        assert_eq!(code, 200);

        // graceful retirement THROUGH the wire — the port is the daemon's
        // identity; a client that spawned it has no pid to signal.
        let (code, _) = daemon.request("POST", "/v1/shutdown", None);
        assert_eq!(code, 200);
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut daemon = daemon;
        loop {
            match daemon.child.try_wait().expect("poll daemon") {
                Some(status) => {
                    assert!(status.success(), "shutdown must exit cleanly");
                    break;
                }
                None => {
                    assert!(Instant::now() < deadline, "daemon ignored /v1/shutdown");
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }
    }

    // a fresh daemon over the SAME storage root: qmdb state must survive, and
    // the local block counter resumes ABOVE the per-module index watermark
    // (two blocks were indexed) — a counter restarting at 0 would re-use
    // indexed heights and every new block would be silently skipped.
    let daemon = Daemon::spawn(storage.path());
    assert_eq!(daemon.status()["height"], 2);
    let reply = daemon.query(
        "chat",
        serde_json::json!({ "messages_latest": { "channel_id": "durable", "limit": 16 } }),
    );
    let messages = reply["messages"].as_array().expect("Messages reply");
    assert_eq!(messages.len(), 1, "chat state must survive a restart");
    assert_eq!(
        messages[0]["head"]["blocks"][0]["paragraph"][0]["text"],
        "written before restart"
    );

    // the explorer survives too: /v1/blocks reads the durable block index,
    // not an in-memory ring, so both pre-restart blocks are still served.
    let (code, blocks) = daemon.request("GET", "/v1/blocks", None);
    assert_eq!(code, 200, "blocks failed: {blocks}");
    let blocks = blocks["blocks"].as_array().expect("blocks array").clone();
    assert_eq!(blocks.len(), 2, "pre-restart blocks survive: {blocks:?}");
    assert_eq!(blocks[0]["height"], 1);
    let post = &blocks[1];
    assert_eq!(post["height"], 2);
    assert_eq!(post["target"], "chat");
    assert_eq!(post["disposition"], "applied");
    // this lane frames and signs nothing: the hash is honestly empty, and
    // the proposer is the SUBMITTER's origin bytes as hex ("eddy").
    assert_eq!(post["hash"], "");
    assert_eq!(post["proposer"], "65646479");
    assert!(
        post["operations"].as_array().is_some_and(|ops| !ops.is_empty()),
        "the dispatch trace rides the row: {post}"
    );
}

#[test]
fn per_module_index_serves_ops_and_views() {
    let storage = tempfile::TempDir::new().expect("storage dir");
    let hits_of = |reply: &serde_json::Value| -> Vec<serde_json::Value> {
        reply["hits"].as_array().expect("hits reply").clone()
    };

    let pre_restart_height;
    {
        let daemon = Daemon::spawn(storage.path());
        let (code, _) = daemon.submit(
            "chat",
            serde_json::json!({
                "create_channel": { "channel_id": "eng", "name": "Eng", "post_policy": "open" }
            }),
            None,
        );
        assert_eq!(code, 200);
        let (code, _) = daemon.submit("chat", post_message("eng", "m1", "fluent index demo"), Some("eddy"));
        assert_eq!(code, 200);
        let (code, _) = daemon.submit(
            "tasks",
            serde_json::json!({ "create_task": { "task_id": "t1", "title": "wire the indexer" } }),
            None,
        );
        assert_eq!(code, 200);

        // the raw op log: every applied chat op, oldest-first, json envelopes.
        let (code, ops) = daemon.request("GET", "/v1/index/chat/ops?limit=10", None);
        assert_eq!(code, 200, "ops failed: {ops}");
        let rows = ops["ops"].as_array().expect("ops array");
        assert_eq!(rows.len(), 2, "create-channel and post: {ops}");
        // the payload is the module op VERBATIM (chat's wire is snake_case);
        // the envelope itself (origin/height/seq) is the indexer's camelCase.
        assert_eq!(rows[1]["payload"]["post_message"]["message_id"], "m1");
        assert_eq!(rows[1]["origin"]["kind"], "external");
        assert_eq!(rows[1]["height"], 2);

        // chat's OWN endpoint: the materialized search view.
        let (code, reply) = daemon.request(
            "POST",
            "/v1/index/chat/view",
            Some(&serde_json::json!({ "search": { "text": "fluent" } })),
        );
        assert_eq!(code, 200, "chat view failed: {reply}");
        let hits = hits_of(&reply);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0]["messageId"], "m1");
        assert_eq!(hits[0]["author"], "user:eddy");

        // tasks' endpoint: the by-status partition.
        let (code, reply) = daemon.request(
            "POST",
            "/v1/index/tasks/view",
            Some(&serde_json::json!({ "byStatus": { "status": "open" } })),
        );
        assert_eq!(code, 200, "tasks view failed: {reply}");
        let tasks = reply["tasks"]["tasks"].as_array().expect("tasks array");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0]["title"], "wire the indexer");

        // a module with no materialized view answers 404 — forge's substrate
        // is already a queryable git repo; it never registers one.
        let (code, _) = daemon.request(
            "POST",
            "/v1/index/forge/view",
            Some(&serde_json::json!({ "anything": {} })),
        );
        assert_eq!(code, 404);

        // the watermark surface: all three blocks indexed, nothing poisoned.
        // EVERY module's watermark tracks the last applied block — chat reads
        // 3 even though its last op landed in block 2 — so a watermark below
        // the tip always means missing blocks, never a quiet module.
        let (code, status) = daemon.request("GET", "/v1/index/status", None);
        assert_eq!(code, 200);
        assert_eq!(status["poisoned"], false);
        assert_eq!(status["modules"]["chat"], 3);
        assert_eq!(status["modules"]["tasks"], 3);

        pre_restart_height = daemon.status()["height"].as_u64().expect("height");

        let (code, _) = daemon.request("POST", "/v1/shutdown", None);
        assert_eq!(code, 200);
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut daemon = daemon;
        while daemon.child.try_wait().expect("poll daemon").is_none() {
            assert!(Instant::now() < deadline, "daemon ignored /v1/shutdown");
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    // restart over the same storage: the index survives, the block counter
    // resumes above its watermark, and NEW blocks keep indexing.
    let daemon = Daemon::spawn(storage.path());
    assert_eq!(
        daemon.status()["height"].as_u64().expect("height"),
        pre_restart_height
    );
    let (code, reply) = daemon.request(
        "POST",
        "/v1/index/chat/view",
        Some(&serde_json::json!({ "search": { "text": "fluent" } })),
    );
    assert_eq!(code, 200);
    assert_eq!(hits_of(&reply).len(), 1, "index survives a restart");

    let (code, _) = daemon.submit("chat", post_message("eng", "m2", "fresh after restart"), Some("eddy"));
    assert_eq!(code, 200);
    let (code, reply) = daemon.request(
        "POST",
        "/v1/index/chat/view",
        Some(&serde_json::json!({ "search": { "text": "fresh" } })),
    );
    assert_eq!(code, 200);
    let hits = hits_of(&reply);
    assert_eq!(hits.len(), 1, "post-restart blocks keep indexing");
    assert_eq!(hits[0]["messageId"], "m2");
}

#[test]
fn files_blob_seam_round_trips_and_ties_into_consensus() {
    let storage = tempfile::TempDir::new().expect("storage dir");
    let daemon = Daemon::spawn(storage.path());
    let genesis_hash = daemon.status()["appHash"]
        .as_str()
        .expect("appHash")
        .to_string();

    // upload: binary, non-utf8, deliberately smaller than the chunk size so
    // the manifest's tail-length rule is exercised below.
    let chunk: Vec<u8> = (0..3000u32).map(|i| (i % 251) as u8).collect();
    let (code, body) = daemon.request_bytes("POST", "/v1/files/blob", &chunk);
    assert_eq!(
        code,
        200,
        "upload failed: {}",
        String::from_utf8_lossy(&body)
    );
    let reply: serde_json::Value = serde_json::from_slice(&body).expect("upload reply json");
    let digest = reply["digest"].as_str().expect("digest").to_string();
    assert_eq!(
        digest,
        files::digest_hex(&chunk),
        "the returned digest is sha256 of the exact uploaded bytes"
    );

    // fetch round-trips byte-identical.
    let (code, fetched) = daemon.request_bytes("GET", &format!("/v1/files/blob/{digest}"), &[]);
    assert_eq!(code, 200);
    assert_eq!(fetched, chunk, "fetched bytes must be byte-identical");

    // a well-formed digest nobody uploaded is a 404; a malformed digest
    // (uppercase hex included) is a 400, not a miss.
    let absent = files::digest_hex(b"never uploaded");
    let (code, _) = daemon.request_bytes("GET", &format!("/v1/files/blob/{absent}"), &[]);
    assert_eq!(code, 404, "absent chunk must be a 404");
    let upper = digest.to_uppercase();
    let (code, _) = daemon.request_bytes("GET", &format!("/v1/files/blob/{upper}"), &[]);
    assert_eq!(code, 400, "digest must be lowercase hex");

    // the cap is MAX_CHUNK_SIZE inclusive: exactly 4 MiB lands...
    let max = vec![0xABu8; files::MAX_CHUNK_SIZE as usize];
    let (code, _) = daemon.request_bytes("POST", "/v1/files/blob", &max);
    assert_eq!(code, 200, "a chunk of exactly MAX_CHUNK_SIZE must land");
    // ...and one byte more is a 413 in the daemon's error envelope.
    let over = vec![0xCDu8; files::MAX_CHUNK_SIZE as usize + 1];
    let (code, body) = daemon.request_bytes("POST", "/v1/files/blob", &over);
    assert_eq!(
        code,
        413,
        "oversized chunk must be rejected: {}",
        String::from_utf8_lossy(&body)
    );
    let err: serde_json::Value = serde_json::from_slice(&body).expect("413 body is json");
    assert!(
        err["error"].is_string(),
        "413 uses the error envelope: {err}"
    );

    // the whole blob lane is off-consensus: no blocks, no app-hash movement.
    let status = daemon.status();
    assert_eq!(status["height"], 0, "blob puts must not commit blocks");
    assert_eq!(
        status["appHash"].as_str(),
        Some(genesis_hash.as_str()),
        "blob puts must not move the app hash"
    );

    // the consensus tie-in: ONLY the digest crosses /v1/submit. the committed
    // manifest then verifies the fetched bytes end to end.
    let (code, block) = daemon.submit(
        "files",
        serde_json::json!({
            "add_manifest": {
                "file_id": "f1",
                "name": "blob.bin",
                "mime": "application/octet-stream",
                "size": 3000,
                "chunk_size": 4096,
                "chunks": [digest],
            }
        }),
        Some("eddy"),
    );
    assert_eq!(code, 200, "AddManifest failed: {block}");
    assert_eq!(block["height"], 1, "the manifest IS a block");

    let reply = daemon.query("files", serde_json::json!({ "stat": { "file_id": "f1" } }));
    let manifest: files::Manifest =
        serde_json::from_value(reply["stat"].clone()).expect("Stat carries the manifest");
    files::verify_chunk(&manifest, 0, &fetched)
        .expect("fetched bytes verify against the committed manifest");
}

#[test]
fn metrics_endpoint_exposes_ducktape_and_runtime_series() {
    let storage = tempfile::TempDir::new().expect("storage dir");
    let daemon = Daemon::spawn(storage.path());

    // at genesis the ducktape series are registered but a block-derived series
    // like the height gauge has not been observed yet — commit one block first.
    let (code, block) = daemon.submit(
        "chat",
        serde_json::json!({
            "create_channel": { "channel_id": "general", "name": "General", "post_policy": "open" }
        }),
        None,
    );
    assert_eq!(code, 200, "create channel failed: {block}");

    let text = daemon.metrics();
    // the daemon's own series, registered into commonware's registry.
    assert!(
        text.contains("ducktape_blocks_total"),
        "blocks counter present: {text}"
    );
    assert!(
        text.contains("ducktape_block_height"),
        "height gauge present"
    );
    assert!(
        text.contains("ducktape_block_apply_latency_seconds"),
        "latency histogram present",
    );
    // the per-dispatch counter carries the low-cardinality labels, and the
    // block above dispatched chat as an external submit.
    assert!(
        text.contains("ducktape_dispatch_total") && text.contains("module=\"chat\""),
        "labelled dispatch counter present: {text}",
    );
    // the same encode() also carries commonware's runtime metrics — proof the
    // series share one registry — and closes with the OpenMetrics EOF sentinel.
    assert!(
        text.contains("runtime_"),
        "commonware runtime metrics present too"
    );
    assert!(
        text.trim_end().ends_with("# EOF"),
        "OpenMetrics EOF terminator"
    );
}

// ============================================================================
// git smart-HTTP receive-pack: REAL `git push` against the daemon's /forge lane.
//
// this is the make-or-break gate for the git-http bridge: a stock `git` client
// pushes to http://127.0.0.1:<port>/forge/testrepo and the pushed commit must
// become forge's committed HEAD. exercises the whole path — info/refs ref
// advertisement, the pkt-line command + packfile POST, the node-local pack
// stash, and the consensus `Push` CAS.
// ============================================================================

/// whether a `git` binary is on PATH (the bridge test needs a real client).
fn have_git() -> bool {
    Command::new("git")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// a `git` invocation in `dir` with a hermetic config: no host global/system
/// config leaks in (gpg signing, aliases), the default branch is `main`, a fixed
/// identity, and no interactive credential/gpg prompts can hang the test.
fn git_cmd(dir: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new("git");
    cmd.current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .args([
            "-c",
            "init.defaultBranch=main",
            "-c",
            "user.name=Ducktape Test",
            "-c",
            "user.email=test@ducktape.local",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args);
    cmd
}

/// run a git command, capturing stdout+stderr (git prints push progress and
/// rejections to stderr), WITHOUT asserting success — the caller decides.
fn git_capture(dir: &Path, args: &[&str]) -> std::process::Output {
    git_cmd(dir, args).output().expect("spawn git")
}

/// run a git command that must succeed.
fn git_ok(dir: &Path, args: &[&str]) {
    let out = git_capture(dir, args);
    assert!(
        out.status.success(),
        "git {args:?} failed:\n{}",
        render(&out)
    );
}

/// a legible dump of a git subprocess result for assertion messages / logs.
fn render(out: &std::process::Output) -> String {
    format!(
        "status: {}\n--- stdout ---\n{}--- stderr ---\n{}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    )
}

/// stage a file with `content`, then commit it with `message`.
fn commit_file(dir: &Path, name: &str, content: &str, message: &str) {
    std::fs::write(dir.join(name), content).expect("write work file");
    git_ok(dir, &["add", name]);
    git_ok(dir, &["commit", "-m", message]);
}

/// this repo's current HEAD oid hex.
fn rev_parse_head(dir: &Path) -> String {
    let out = git_capture(dir, &["rev-parse", "HEAD"]);
    assert!(out.status.success(), "rev-parse failed:\n{}", render(&out));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// forge's committed HEAD oid hex for `repo` over /v1/query (`None` == unborn).
fn forge_head(daemon: &Daemon, repo: &str) -> Option<String> {
    let reply = daemon.query("forge", serde_json::json!({ "head_of": { "repo": repo } }));
    reply["head"].as_str().map(str::to_string)
}

#[test]
fn git_push_over_http_lands_in_forge_head() {
    if !have_git() {
        eprintln!("skipping git_push_over_http_lands_in_forge_head: no `git` on PATH");
        return;
    }
    let storage = tempfile::TempDir::new().expect("storage dir");
    let daemon = Daemon::spawn(storage.path());
    let url = format!("http://127.0.0.1:{}/forge/testrepo", daemon.port);

    // an unborn repo advertises no head.
    assert_eq!(forge_head(&daemon, "testrepo"), None, "repo starts unborn");

    // a scratch repo with one commit, wired to push at the daemon.
    let work = tempfile::TempDir::new().expect("git work dir");
    let wd = work.path();
    git_ok(wd, &["init"]);
    commit_file(wd, "hello.txt", "hi from git\n", "first commit");
    git_ok(wd, &["remote", "add", "ducktape", &url]);

    // THE gate: a real `git push` to the daemon exits 0 and updates the ref.
    let push1 = git_capture(wd, &["push", "ducktape", "main"]);
    eprintln!("=== git push #1 (create) ===\n{}", render(&push1));
    assert!(
        push1.status.success(),
        "git push failed:\n{}",
        render(&push1)
    );
    let head1 = rev_parse_head(wd);
    assert_eq!(
        forge_head(&daemon, "testrepo"),
        Some(head1.clone()),
        "forge HEAD must equal the pushed commit"
    );

    // a second commit fast-forwards: the CAS matches the prev head and advances.
    commit_file(wd, "hello.txt", "hi again\n", "second commit");
    let head2 = rev_parse_head(wd);
    assert_ne!(head2, head1, "second commit is a new oid");
    let push2 = git_capture(wd, &["push", "ducktape", "main"]);
    eprintln!("=== git push #2 (fast-forward) ===\n{}", render(&push2));
    assert!(
        push2.status.success(),
        "fast-forward push failed:\n{}",
        render(&push2)
    );
    assert_eq!(
        forge_head(&daemon, "testrepo"),
        Some(head2.clone()),
        "forge HEAD must fast-forward to the second commit"
    );

    // a non-fast-forward push is rejected: rewind one commit, commit a divergent
    // history, and push without force. git detects the non-ff against the
    // advertised head and refuses; forge's HEAD stays put.
    git_ok(wd, &["reset", "--hard", "HEAD~1"]);
    commit_file(wd, "hello.txt", "divergent line\n", "divergent commit");
    let push3 = git_capture(wd, &["push", "ducktape", "main"]);
    eprintln!(
        "=== git push #3 (non-fast-forward, expected reject) ===\n{}",
        render(&push3)
    );
    assert!(
        !push3.status.success(),
        "a non-fast-forward push must be rejected:\n{}",
        render(&push3)
    );
    assert_eq!(
        forge_head(&daemon, "testrepo"),
        Some(head2),
        "a rejected push must not move forge HEAD"
    );
}

// ============================================================================
// git smart-HTTP upload-pack: the FULL push -> clone round trip. this is the
// make-or-break gate for the fetch side: after a real `git push` lands two real
// commits, a stock `git clone` of the same URL must reconstruct the repo
// byte-for-byte — same HEAD oid, same file bytes, and the SAME two-commit
// history with the SAME oids (proving faithful object transfer over the wire,
// not a re-synthesized commit).
// ============================================================================

/// every commit oid on this repo's HEAD history, newest-first, one hex per line.
fn log_oids(dir: &Path) -> Vec<u8> {
    let out = git_capture(dir, &["log", "--format=%H"]);
    assert!(out.status.success(), "git log failed:\n{}", render(&out));
    out.stdout
}

#[test]
fn git_clone_over_http_round_trips_full_history() {
    if !have_git() {
        eprintln!("skipping git_clone_over_http_round_trips_full_history: no `git` on PATH");
        return;
    }
    let storage = tempfile::TempDir::new().expect("storage dir");
    let daemon = Daemon::spawn(storage.path());
    let url = format!("http://127.0.0.1:{}/forge/roundtrip", daemon.port);

    // a scratch repo with TWO real commits, pushed to the daemon over http.
    let work = tempfile::TempDir::new().expect("git work dir");
    let wd = work.path();
    git_ok(wd, &["init"]);
    commit_file(wd, "readme.md", "line one\n", "first commit");
    commit_file(wd, "readme.md", "line one\nline two\n", "second commit");
    git_ok(wd, &["remote", "add", "ducktape", &url]);
    let push = git_capture(wd, &["push", "ducktape", "main"]);
    eprintln!("=== git push (2 commits) ===\n{}", render(&push));
    assert!(push.status.success(), "push failed:\n{}", render(&push));

    let pushed_head = rev_parse_head(wd);
    assert_eq!(
        forge_head(&daemon, "roundtrip"),
        Some(pushed_head.clone()),
        "forge HEAD must equal the pushed commit before we clone it back"
    );
    let pushed_oids = log_oids(wd);

    // THE gate: a real `git clone` of the same URL into a fresh dir exits 0.
    let clone_root = tempfile::TempDir::new().expect("clone root dir");
    let dst = clone_root.path().join("clone");
    let clone = git_capture(
        clone_root.path(),
        &["clone", &url, dst.to_str().expect("utf-8 clone path")],
    );
    eprintln!("=== git clone ===\n{}", render(&clone));
    assert!(
        clone.status.success(),
        "git clone failed:\n{}",
        render(&clone)
    );

    // the cloned HEAD is the pushed HEAD, to the oid.
    let cloned_head = rev_parse_head(&dst);
    assert_eq!(
        cloned_head, pushed_head,
        "cloned HEAD must equal the pushed HEAD"
    );

    // the checked-out file bytes match the source byte-for-byte.
    let cloned_bytes = std::fs::read(dst.join("readme.md")).expect("read cloned file");
    assert_eq!(
        cloned_bytes, b"line one\nline two\n",
        "cloned file content must match the pushed content byte-for-byte"
    );

    // full history: `git log --oneline` shows BOTH commits...
    let log = git_capture(&dst, &["log", "--oneline"]);
    eprintln!("=== git log --oneline (clone) ===\n{}", render(&log));
    assert!(log.status.success(), "git log failed:\n{}", render(&log));
    let log_text = String::from_utf8_lossy(&log.stdout);
    assert_eq!(
        log_text.lines().count(),
        2,
        "the clone must carry both commits:\n{log_text}"
    );
    assert!(
        log_text.contains("first commit") && log_text.contains("second commit"),
        "both commit messages must survive the clone:\n{log_text}"
    );

    // ...with the SAME oids in the SAME order as the source repo — the proof of
    // faithful object transfer (real history, not a reconstructed commit).
    assert_eq!(
        log_oids(&dst),
        pushed_oids,
        "the cloned history oids must match the pushed repo exactly"
    );
}

/// Regression: a push whose data exceeds git's `http.postBuffer` is preceded by
/// a flush-only PROBE POST (zero commands) before the real chunked request. The
/// receive-pack handler must answer that probe 200, not 400 — otherwise every
/// push larger than the buffer (the common case for a real repo) fails. Forcing
/// `http.postBuffer=1` makes git take the probe path for even a one-commit push.
#[test]
fn git_push_larger_than_post_buffer_uses_the_probe_path() {
    if !have_git() {
        eprintln!("skipping git_push_larger_than_post_buffer_uses_the_probe_path: no `git` on PATH");
        return;
    }
    let storage = tempfile::TempDir::new().expect("storage dir");
    let daemon = Daemon::spawn(storage.path());
    let url = format!("http://127.0.0.1:{}/forge/probed", daemon.port);

    let work = tempfile::TempDir::new().expect("git work dir");
    let wd = work.path();
    git_ok(wd, &["init"]);
    commit_file(wd, "hello.txt", "hi from a probed push\n", "first commit");
    git_ok(wd, &["remote", "add", "ducktape", &url]);

    // `-c http.postBuffer=1` forces git through the large-request probe.
    let push = git_capture(wd, &["-c", "http.postBuffer=1", "push", "ducktape", "main"]);
    eprintln!("=== probed git push ===\n{}", render(&push));
    assert!(
        push.status.success(),
        "a push through the postBuffer probe path must succeed:\n{}",
        render(&push)
    );
    assert_eq!(
        forge_head(&daemon, "probed"),
        Some(rev_parse_head(wd)),
        "forge HEAD must equal the pushed commit after a probed push"
    );
}
