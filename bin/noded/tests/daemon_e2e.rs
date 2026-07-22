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
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use sha2::{Digest as _, Sha256};

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
        nettest::try_http_json(self.port, method, path, body)
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
        nettest::http_bytes(self.port, method, path, "application/octet-stream", body)
    }

    /// open /v1/ws with a minimal rfc6455 client handshake and return the
    /// stream positioned after the 101 response.
    fn ws_connect(&self) -> BufReader<TcpStream> {
        let stream = TcpStream::connect(("127.0.0.1", self.port)).expect("ws connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .expect("ws read timeout");
        let mut stream = stream;
        let req = "GET /v1/ws HTTP/1.1\r\nhost: 127.0.0.1\r\nupgrade: websocket\r\nconnection: upgrade\r\nsec-websocket-key: ZHVja3RhcGUtZTJlLXdzLWtleQ==\r\nsec-websocket-version: 13\r\n\r\n".to_string();
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
    /// daemon sends for stream frames).
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

    /// send one client->server text frame. RFC6455 requires client frames to
    /// be masked; the static key is fine for a deterministic test client.
    fn ws_send_text(reader: &mut BufReader<TcpStream>, text: &str) {
        let payload = text.as_bytes();
        let mut frame = Vec::new();
        frame.push(0x81);
        if payload.len() < 126 {
            frame.push(0x80 | payload.len() as u8);
        } else if payload.len() <= u16::MAX as usize {
            frame.push(0x80 | 126);
            frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        } else {
            frame.push(0x80 | 127);
            frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
        }
        let mask = [0x11, 0x22, 0x33, 0x44];
        frame.extend_from_slice(&mask);
        for (i, byte) in payload.iter().enumerate() {
            frame.push(byte ^ mask[i % mask.len()]);
        }
        reader.get_mut().write_all(&frame).expect("ws frame write");
    }

    fn ws_read_json(reader: &mut BufReader<TcpStream>) -> serde_json::Value {
        serde_json::from_str(&Self::ws_read_text(reader)).expect("ws frame json")
    }

    fn ws_read_type(reader: &mut BufReader<TcpStream>, want: &str) -> serde_json::Value {
        loop {
            let frame = Self::ws_read_json(reader);
            if frame["type"] == want {
                return frame;
            }
        }
    }

    /// a websocket-upgrade GET that expects an http REFUSAL, not a 101: sends
    /// the full rfc6455 handshake so the request reaches the handler body
    /// (axum's extractor stops a plain GET before the handler can say why),
    /// then returns whatever status + raw response text the daemon answers.
    /// a refusal leaves the connection open (keep-alive), so the read is
    /// timeout-bounded instead of read-to-close.
    fn ws_upgrade_refusal(&self, path: &str) -> (u16, String) {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("daemon reachable");
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

use nettest::free_port;

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
    assert!(raw.contains("no mesh call hub"), "refusal says WHY: {raw}");

    let (status, raw) = daemon.ws_upgrade_refusal("/v1/presence/ws?page=page-1");
    assert_eq!(status, 503, "no realtime hub → presence refused: {raw}");
    assert!(raw.contains("no mesh realtime hub"), "refusal says WHY: {raw}");

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
            "identity",
            "gateway"
        ]
    );
    let genesis_hash = status["appHash"].as_str().expect("appHash").to_string();

    // connect before submitting: the stream heartbeats without a subscription,
    // then module events catch up from the subscribed cursor.
    let mut ws = daemon.ws_connect();
    let heartbeat = Daemon::ws_read_type(&mut ws, "heartbeat");
    assert_eq!(heartbeat["height"], 0);
    assert_eq!(heartbeat["intervalMs"], 3_000);

    Daemon::ws_send_text(&mut ws, r#"{"op":"subscribe","topics":["module:chat"]}"#);
    let subscribed = Daemon::ws_read_type(&mut ws, "subscribed");
    assert_eq!(
        subscribed["topics"]["module:chat"],
        "op/0000000000000000/ffff"
    );

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

    // the ws stream carries index-backed op rows, not payload-free block
    // ticks. each event cursor is the same `after` token the HTTP op log uses.
    let event1 = Daemon::ws_read_type(&mut ws, "event");
    let event2 = Daemon::ws_read_type(&mut ws, "event");
    assert_eq!(event1["topic"], "module:chat");
    assert_eq!(event2["topic"], "module:chat");
    assert_eq!(event1["cursor"], "op/0000000000000001/0000");
    assert_eq!(event2["cursor"], "op/0000000000000002/0000");

    let (code, ops) = daemon.request("GET", "/v1/index/chat/ops?limit=10", None);
    assert_eq!(code, 200, "ops failed: {ops}");
    let rows = ops["ops"].as_array().expect("ops array");
    assert_eq!(rows.len(), 2, "create and post rows: {ops}");
    assert_eq!(event1["op"], rows[0]);
    assert_eq!(event2["op"], rows[1]);

    let cursor1 = event1["cursor"].as_str().expect("event cursor");
    let (code, paged) = daemon.request(
        "GET",
        &format!("/v1/index/chat/ops?after={cursor1}&limit=10"),
        None,
    );
    assert_eq!(code, 200, "paged ops failed: {paged}");
    let paged_rows = paged["ops"].as_array().expect("paged ops array");
    assert_eq!(paged_rows.as_slice(), &rows[1..], "cursor pages to row 2");

    drop(ws);
    let mut ws = daemon.ws_connect();
    let _heartbeat = Daemon::ws_read_type(&mut ws, "heartbeat");
    Daemon::ws_send_text(
        &mut ws,
        &format!(
            r#"{{"op":"subscribe","topics":["module:chat"],"resume":{{"module:chat":"{cursor1}"}}}}"#
        ),
    );
    let _subscribed = Daemon::ws_read_type(&mut ws, "subscribed");
    let replay = Daemon::ws_read_type(&mut ws, "event");
    assert_eq!(replay["cursor"], event2["cursor"]);
    assert_eq!(replay["op"], rows[1]);

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

/// read frames until block `height`'s module event and require a heartbeat
/// carrying that height to have arrived FIRST — the per-block tip push, not
/// the interval beat (which the loop tolerates at other heights).
fn assert_tip_precedes_event(ws: &mut BufReader<TcpStream>, height: u64) {
    let mut tip_seen = false;
    loop {
        let frame = Daemon::ws_read_json(ws);
        if frame["type"] == "heartbeat" && frame["height"] == height {
            tip_seen = true;
            continue;
        }
        if frame["type"] == "event" {
            assert_eq!(frame["op"]["height"], height, "event for block {height}");
            assert!(
                tip_seen,
                "no tip heartbeat at height {height} arrived before its event"
            );
            return;
        }
    }
}

/// the tip rides the block wake itself: every committed block pushes a
/// heartbeat frame with the new height BEFORE that block's module events, so
/// a console's height ticks per block instead of waiting out the 3s timer
/// beat. asserting the ordering on TWO consecutive blocks makes a
/// coincidental timer beat unable to false-pass the test — two timer beats
/// are 3s apart and cannot both land inside one test's submit window.
#[test]
fn block_commits_push_tip_heartbeats_before_their_events() {
    let storage = tempfile::TempDir::new().expect("storage dir");
    let daemon = Daemon::spawn(storage.path());

    let mut ws = daemon.ws_connect();
    let heartbeat = Daemon::ws_read_type(&mut ws, "heartbeat");
    assert_eq!(heartbeat["height"], 0);

    Daemon::ws_send_text(&mut ws, r#"{"op":"subscribe","topics":["module:chat"]}"#);
    let _subscribed = Daemon::ws_read_type(&mut ws, "subscribed");

    let (code, block) = daemon.submit(
        "chat",
        serde_json::json!({
            "create_channel": { "channel_id": "general", "name": "General", "post_policy": "open" }
        }),
        None,
    );
    assert_eq!(code, 200, "create channel failed: {block}");
    assert_eq!(block["height"], 1);
    assert_tip_precedes_event(&mut ws, 1);

    let (code, block) = daemon.submit("chat", post_message("general", "m1", "tick"), None);
    assert_eq!(code, 200, "post failed: {block}");
    assert_eq!(block["height"], 2);
    assert_tip_precedes_event(&mut ws, 2);
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

    let (code, block) = daemon.submit(
        "agent",
        serde_json::json!({
            "register_agent": {
                "agent_id": "quackbot",
                "display_name": "Quackbot",
                "capability": "echo-model",
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
        let (code, _) = daemon.request("POST", "/v1/admin/shutdown", None);
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
                    assert!(Instant::now() < deadline, "daemon ignored /v1/admin/shutdown");
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
    // a block now carries its member ops under `ops[]`; this lane is one op
    // per block.
    let op = &post["ops"][0];
    assert_eq!(op["target"], "chat");
    assert_eq!(op["disposition"], "applied");
    // this lane frames and signs nothing: the block hash is honestly empty, and
    // the op's proposer is the SUBMITTER's origin bytes as hex ("eddy").
    assert_eq!(post["hash"], "");
    assert_eq!(op["proposer"], "65646479");
    assert!(
        op["operations"]
            .as_array()
            .is_some_and(|ops| !ops.is_empty()),
        "the dispatch trace rides the op: {post}"
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
        let (code, _) = daemon.submit(
            "chat",
            post_message("eng", "m1", "fluent index demo"),
            Some("eddy"),
        );
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

        let (code, _) = daemon.request("POST", "/v1/admin/shutdown", None);
        assert_eq!(code, 200);
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut daemon = daemon;
        while daemon.child.try_wait().expect("poll daemon").is_none() {
            assert!(Instant::now() < deadline, "daemon ignored /v1/admin/shutdown");
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

    let (code, _) = daemon.submit(
        "chat",
        post_message("eng", "m2", "fresh after restart"),
        Some("eddy"),
    );
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
fn blob_receipt_lane_round_trips_and_stays_off_consensus() {
    let storage = tempfile::TempDir::new().expect("storage dir");
    let daemon = Daemon::spawn(storage.path());
    let genesis_hash = daemon.status()["appHash"]
        .as_str()
        .expect("appHash")
        .to_string();

    // sha256 as 64-char lowercase hex — the digest rendering the lane returns.
    let digest_hex = |bytes: &[u8]| -> String {
        Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    };

    // upload: binary, non-utf8 receipt bytes.
    let receipt: Vec<u8> = (0..3000u32).map(|i| (i % 251) as u8).collect();
    let (code, body) = daemon.request_bytes("POST", "/v1/files/blob", &receipt);
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
        digest_hex(&receipt),
        "the returned digest is sha256 of the exact uploaded bytes"
    );

    // fetch round-trips byte-identical.
    let (code, fetched) = daemon.request_bytes("GET", &format!("/v1/files/blob/{digest}"), &[]);
    assert_eq!(code, 200);
    assert_eq!(fetched, receipt, "fetched bytes must be byte-identical");

    // a well-formed digest nobody uploaded is a 404; a malformed digest
    // (uppercase hex included) is a 400, not a miss.
    let absent = digest_hex(b"never uploaded");
    let (code, _) = daemon.request_bytes("GET", &format!("/v1/files/blob/{absent}"), &[]);
    assert_eq!(code, 404, "absent receipt must be a 404");
    let upper = digest.to_uppercase();
    let (code, _) = daemon.request_bytes("GET", &format!("/v1/files/blob/{upper}"), &[]);
    assert_eq!(code, 400, "digest must be lowercase hex");

    // the receipt-lane body cap is 4 MiB inclusive: exactly 4 MiB lands...
    let max = vec![0xABu8; 4 * 1024 * 1024];
    let (code, _) = daemon.request_bytes("POST", "/v1/files/blob", &max);
    assert_eq!(code, 200, "a body of exactly the cap must land");
    // ...and one byte more is a 413 in the daemon's error envelope.
    let over = vec![0xCDu8; 4 * 1024 * 1024 + 1];
    let (code, body) = daemon.request_bytes("POST", "/v1/files/blob", &over);
    assert_eq!(
        code,
        413,
        "oversized body must be rejected: {}",
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
}

// ============================================================================
// duckfs product surface: the stage -> commit -> read round trip against a real
// daemon. two chunks staged over POST /v1/files/stage, a commit that references
// them (Chunks content) alongside an inline file, then ls/read/stat/history read
// it all back — read byte-exact. a rejected op (dangling chunk, oversized stage)
// is a clean 4xx, never a 500/panic. distinct from the op-receipt /v1/files/blob
// lane, which its own test above keeps green.
// ============================================================================

#[test]
fn duckfs_surface_stage_commit_and_reads_round_trip() {
    let storage = tempfile::TempDir::new().expect("storage dir");
    let daemon = Daemon::spawn(storage.path());
    let genesis_hash = daemon.status()["appHash"]
        .as_str()
        .expect("appHash")
        .to_string();

    // refs on a fresh module: no head (the empty filesystem) and an empty window,
    // the base state the checkout engine starts from.
    let (code, refs0) = daemon.request("GET", "/v1/files/refs", None);
    assert_eq!(code, 200, "empty refs failed: {refs0}");
    assert!(
        refs0["head"].is_null(),
        "no head before any commit: {refs0}"
    );
    assert_eq!(refs0["window_len"], 0, "empty window before any commit");

    // a duckfs chunk digest is the chunk object id: sha256 over the chunk kind
    // tag byte (0x00) followed by the bytes — what the module stages under and a
    // commit references. the stage endpoint returns it; we recompute it here to
    // prove the returned digest is exactly that.
    let chunk_digest = |bytes: &[u8]| -> String {
        let mut h = Sha256::new();
        h.update([0u8]);
        h.update(bytes);
        h.finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };

    // ---- stage two chunks -> digests ----
    let chunk_a: Vec<u8> = (0..64u32).map(|i| (i * 7 % 256) as u8).collect();
    let chunk_b: Vec<u8> = (0..48u32).map(|i| (200 - i) as u8).collect();

    let (code, body) = daemon.request_bytes("POST", "/v1/files/stage", &chunk_a);
    assert_eq!(
        code,
        200,
        "stage a failed: {}",
        String::from_utf8_lossy(&body)
    );
    let digest_a =
        serde_json::from_slice::<serde_json::Value>(&body).expect("stage a json")["digest"]
            .as_str()
            .expect("digest a")
            .to_string();
    assert_eq!(
        digest_a,
        chunk_digest(&chunk_a),
        "stage returns the chunk object id"
    );

    let (code, body) = daemon.request_bytes("POST", "/v1/files/stage", &chunk_b);
    assert_eq!(
        code,
        200,
        "stage b failed: {}",
        String::from_utf8_lossy(&body)
    );
    let digest_b =
        serde_json::from_slice::<serde_json::Value>(&body).expect("stage b json")["digest"]
            .as_str()
            .expect("digest b")
            .to_string();
    assert_eq!(digest_b, chunk_digest(&chunk_b));

    // a stage is a real block: staging IS consensus state, so two stages commit
    // two blocks and the module root moves off genesis.
    let after_stage = daemon.status();
    assert_eq!(after_stage["height"], 2, "two stages committed two blocks");
    assert_ne!(
        after_stage["appHash"].as_str(),
        Some(genesis_hash.as_str()),
        "staging moves the module root"
    );

    // ---- commit: two chunk-backed files referencing the digests + an inline
    // file, all under /shared (auto-created parent) ----
    let inline_bytes: &[u8] = b"hello duckfs";
    let commit_body = serde_json::json!({
        "base_snapshot": null,
        "message": "seed duckfs",
        "changes": [
            { "put": { "path": "/shared/a.bin", "exec": false,
                "content": { "chunks": { "size": chunk_a.len() as u64, "chunks": [digest_a] } } } },
            { "put": { "path": "/shared/b.bin", "exec": false,
                "content": { "chunks": { "size": chunk_b.len() as u64, "chunks": [digest_b] } } } },
            { "put": { "path": "/shared/hello.txt", "exec": false,
                "content": { "inline": { "b64": STANDARD.encode(inline_bytes) } } } },
        ],
    });
    let (code, block) = daemon.request("POST", "/v1/files/commit", Some(&commit_body));
    assert_eq!(code, 200, "commit failed: {block}");
    assert_eq!(block["height"], 3, "commit is the third block");

    // ---- ls shows all three, in name order ----
    let (code, ls) = daemon.request("GET", "/v1/files/ls?path=/shared", None);
    assert_eq!(code, 200, "ls failed: {ls}");
    let names: Vec<&str> = ls["entries"]
        .as_array()
        .expect("entries array")
        .iter()
        .map(|e| e["path"].as_str().expect("entry path"))
        .collect();
    assert_eq!(
        names,
        ["/shared/a.bin", "/shared/b.bin", "/shared/hello.txt"]
    );

    // ---- read returns the exact bytes (b64-decoded), eof set for a whole-file
    // read ----
    let read_bytes = |path: &str| -> Vec<u8> {
        let (code, r) = daemon.request("GET", &format!("/v1/files/read?path={path}"), None);
        assert_eq!(code, 200, "read {path} failed: {r}");
        assert_eq!(r["eof"], true, "a whole-file read reaches eof: {r}");
        STANDARD
            .decode(r["b64"].as_str().expect("read b64"))
            .expect("read b64 decodes")
    };
    assert_eq!(
        read_bytes("/shared/a.bin"),
        chunk_a,
        "chunk file a round-trips byte-exact"
    );
    assert_eq!(
        read_bytes("/shared/b.bin"),
        chunk_b,
        "chunk file b round-trips byte-exact"
    );
    assert_eq!(
        read_bytes("/shared/hello.txt"),
        inline_bytes,
        "inline file round-trips byte-exact"
    );

    // ---- stat shows the right kind + size ----
    let (code, st) = daemon.request("GET", "/v1/files/stat?path=/shared/a.bin", None);
    assert_eq!(code, 200, "stat failed: {st}");
    assert_eq!(st["kind"], "file");
    assert_eq!(st["size"].as_u64(), Some(chunk_a.len() as u64));
    assert_eq!(st["exec"], false);
    let (code, st) = daemon.request("GET", "/v1/files/stat?path=/shared", None);
    assert_eq!(code, 200);
    assert_eq!(st["kind"], "dir", "a directory stats as a dir");
    // an absent path is the natural 404.
    let (code, _) = daemon.request("GET", "/v1/files/stat?path=/shared/nope", None);
    assert_eq!(code, 404, "an absent path stats 404");

    // ---- history shows the commit ----
    let (code, hist) = daemon.request("GET", "/v1/files/history", None);
    assert_eq!(code, 200, "history failed: {hist}");
    let snaps = hist["snapshots"].as_array().expect("snapshots array");
    assert_eq!(snaps.len(), 1, "one commit lands in history: {hist}");
    assert_eq!(snaps[0]["message"], "seed duckfs");
    let seed_snapshot = snaps[0]["id"]
        .as_str()
        .expect("seed snapshot id")
        .to_string();

    // ---- refs: head advanced from None (checked empty above) to the seed
    // snapshot, and the window now holds one commit ----
    let (code, refs) = daemon.request("GET", "/v1/files/refs", None);
    assert_eq!(code, 200, "refs failed: {refs}");
    assert_eq!(
        refs["head"].as_str(),
        Some(seed_snapshot.as_str()),
        "refs head is the seed snapshot: {refs}"
    );
    assert_eq!(refs["window_len"], 1, "one commit in the window");

    // ---- has-chunks flips false -> true across a stage; order is preserved ----
    let chunk_c: Vec<u8> = (0..32u32).map(|i| (i * 3 + 1) as u8).collect();
    let digest_c = chunk_digest(&chunk_c);
    let (code, probe) =
        daemon.request("GET", &format!("/v1/files/has-chunks?ids={digest_c}"), None);
    assert_eq!(code, 200, "has-chunks failed: {probe}");
    assert_eq!(
        probe["present"],
        serde_json::json!([false]),
        "an unstaged chunk is absent: {probe}"
    );
    let (code, _) = daemon.request_bytes("POST", "/v1/files/stage", &chunk_c);
    assert_eq!(code, 200, "stage c failed");
    let absent = "22".repeat(32);
    let (code, probe) = daemon.request(
        "GET",
        &format!("/v1/files/has-chunks?ids={digest_c},{absent}"),
        None,
    );
    assert_eq!(code, 200, "has-chunks re-probe failed: {probe}");
    assert_eq!(
        probe["present"],
        serde_json::json!([true, false]),
        "the staged chunk flips present, request order intact: {probe}"
    );

    // ---- diff between the seed snapshot and a follow-up edit ----
    let commit2 = serde_json::json!({
        "base_snapshot": seed_snapshot,
        "message": "edit hello",
        "changes": [
            { "put": { "path": "/shared/hello.txt", "exec": false,
                "content": { "inline": { "b64": STANDARD.encode(b"HELLO AGAIN") } } } },
        ],
    });
    let (code, block2) = daemon.request("POST", "/v1/files/commit", Some(&commit2));
    assert_eq!(code, 200, "second commit failed: {block2}");
    let (code, refs2) = daemon.request("GET", "/v1/files/refs", None);
    assert_eq!(code, 200, "refs2 failed: {refs2}");
    let head2 = refs2["head"].as_str().expect("head2 set").to_string();
    let (code, diff) = daemon.request(
        "GET",
        &format!("/v1/files/diff?from={seed_snapshot}&to={head2}&prefix=/shared"),
        None,
    );
    assert_eq!(code, 200, "diff failed: {diff}");
    let entries = diff["entries"].as_array().expect("diff entries array");
    assert_eq!(entries.len(), 1, "exactly one path changed: {diff}");
    assert_eq!(entries[0]["path"], "/shared/hello.txt");
    assert_eq!(
        entries[0]["kind"], "modified",
        "the edited file is modified"
    );

    // ---- a rejected op is a clean 4xx carrying the error, not a 500/panic ----
    // a commit referencing a never-staged chunk digest: the module cannot
    // resolve the bytes, so it rejects with a 400.
    let bogus = "11".repeat(32); // 64 hex chars, valid shape, never staged
    let bad_commit = serde_json::json!({
        "base_snapshot": null,
        "message": "dangling chunk",
        "changes": [
            { "put": { "path": "/shared/dangling.bin", "exec": false,
                "content": { "chunks": { "size": 10, "chunks": [bogus] } } } },
        ],
    });
    let (code, err) = daemon.request("POST", "/v1/files/commit", Some(&bad_commit));
    assert_eq!(code, 400, "a dangling-chunk commit must reject: {err}");
    assert!(
        err["error"].is_string(),
        "the reject carries the module error: {err}"
    );

    // an oversized stage trips the single-chunk body cap: one byte past
    // CHUNK_SIZE is a 413 in the daemon's error envelope, not a panic.
    let over = vec![0u8; 1024 * 1024 + 1]; // CHUNK_SIZE + 1
    let (code, body) = daemon.request_bytes("POST", "/v1/files/stage", &over);
    assert_eq!(
        code,
        413,
        "an oversized stage is a 413: {}",
        String::from_utf8_lossy(&body)
    );
    let err: serde_json::Value = serde_json::from_slice(&body).expect("413 body is json");
    assert!(
        err["error"].is_string(),
        "413 uses the error envelope: {err}"
    );

    // the daemon is still alive and answering after the rejections.
    daemon.status();
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

#[test]
fn metrics_stream_topic_pushes_the_scrape_over_ws() {
    let storage = tempfile::TempDir::new().expect("storage dir");
    let daemon = Daemon::spawn(storage.path());

    // commit one block so the ducktape series carry observed values.
    let (code, block) = daemon.submit(
        "chat",
        serde_json::json!({
            "create_channel": { "channel_id": "general", "name": "General", "post_policy": "open" }
        }),
        None,
    );
    assert_eq!(code, 200, "create channel failed: {block}");

    let mut ws = daemon.ws_connect();
    Daemon::ws_send_text(&mut ws, r#"{"op":"subscribe","topics":["metrics"]}"#);
    let subscribed = Daemon::ws_read_type(&mut ws, "subscribed");
    assert_eq!(subscribed["topics"]["metrics"], "0", "fresh snapshot cursor");

    // the subscribe replay pushes the first sample immediately — no wait for
    // the next heartbeat tick — carrying the SAME exposition GET /metrics
    // serves, stamped with the server-side sample instant as its cursor.
    let tail = Daemon::ws_read_type(&mut ws, "tail");
    assert_eq!(tail["topic"], "metrics");
    let text = tail["item"]["text"].as_str().expect("scrape text");
    assert!(
        text.contains("ducktape_blocks_total"),
        "stream sample carries the block series: {text}"
    );
    assert!(text.trim_end().ends_with("# EOF"), "whole scrape body rides");
    let time_ms = tail["item"]["timeMs"].as_u64().expect("sample instant");
    assert_eq!(tail["cursor"], time_ms.to_string());

    // the next sample arrives on the heartbeat tick without any block moving.
    let tail2 = Daemon::ws_read_type(&mut ws, "tail");
    assert_eq!(tail2["topic"], "metrics");
    assert!(
        tail2["item"]["timeMs"].as_u64().expect("second instant") >= time_ms,
        "tick samples advance monotonically"
    );
}

// ============================================================================
// off-loop oracle execution: REAL script-backed providers through the full
// capability-host path, proving the daemon's command loop no longer awaits
// provider execution — the fix for the status heartbeat starving (and the
// desktop "reconnecting" banner) during long agent runs.
// ============================================================================

/// stage one script-backed capability provider for a spawned daemon: an
/// operator spec dir with a single `text`-format spec whose `detect.env`
/// points at `body`'s script. returns the daemon env that provides the tag —
/// including detect overrides that HIDE the embedded executor specs, so a dev
/// box with a real `claude`/`codex` on PATH runs these tests identically to CI.
fn stage_script_provider(root: &Path, tag: &str, body: &str) -> Vec<(String, String)> {
    use std::os::unix::fs::PermissionsExt as _;
    let spec_dir = root.join("specs");
    std::fs::create_dir_all(&spec_dir).expect("provider spec dir");
    let script = root.join(format!("{tag}.sh"));
    std::fs::write(&script, format!("#!/bin/sh\n{body}\n")).expect("write provider script");
    let mut perms = std::fs::metadata(&script)
        .expect("script metadata")
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).expect("chmod provider script");

    let env_var = format!(
        "DUCKTAPE_TEST_{}_BIN",
        tag.replace(['-', '.'], "_").to_uppercase()
    );
    std::fs::write(
        spec_dir.join(format!("{tag}.toml")),
        format!(
            "spec = 1\n\
             [capability]\n\
             tag = \"{tag}\"\n\
             description = \"daemon e2e script executor\"\n\
             [detect]\n\
             bin = \"{tag}-nonexistent-cli\"\n\
             env = \"{env_var}\"\n\
             [invoke]\n\
             args = []\n\
             prompt = \"stdin\"\n\
             timeout_secs = 60\n\
             [output]\n\
             format = \"text\"\n"
        ),
    )
    .expect("write provider spec");

    let missing = root.join("missing-executor");
    vec![
        (
            "DUCKTAPE_CAPABILITY_DIR".into(),
            spec_dir.display().to_string(),
        ),
        (env_var, script.display().to_string()),
        ("DUCKTAPE_CLAUDE_BIN".into(), missing.display().to_string()),
        ("DUCKTAPE_CODEX_BIN".into(), missing.display().to_string()),
    ]
}

/// channel + registered agent + mention watch: the client-side trigger stack
/// for one agent run in `channel`. no prompt blob — an agent is its curated
/// skills, and one that curates none still runs (it simply has no persona).
fn arm_agent(daemon: &Daemon, channel: &str, agent_id: &str, tag: &str) {
    let (code, block) = daemon.submit(
        "chat",
        serde_json::json!({
            "create_channel": { "channel_id": channel, "name": channel, "post_policy": "open" }
        }),
        Some("owner"),
    );
    assert_eq!(code, 200, "create channel failed: {block}");
    let (code, block) = daemon.submit(
        "agent",
        serde_json::json!({
            "register_agent": {
                "agent_id": agent_id,
                "display_name": agent_id,
                "capability": tag,
                "allowed_actions": ["chat.post"]
            }
        }),
        Some("owner"),
    );
    assert_eq!(code, 200, "register agent failed: {block}");
    let (code, block) = daemon.submit(
        "runs",
        serde_json::json!({
            "watch_channel": { "channel_id": channel, "policy": "mention" }
        }),
        Some("owner"),
    );
    assert_eq!(code, 200, "watch channel failed: {block}");
}

/// the plain texts of `channel`'s agent-authored replies.
fn agent_replies(daemon: &Daemon, channel: &str) -> Vec<String> {
    let reply = daemon.query(
        "chat",
        serde_json::json!({ "messages_latest": { "channel_id": channel, "limit": 32 } }),
    );
    reply["messages"]
        .as_array()
        .expect("Messages reply")
        .iter()
        .filter(|m| m["head"]["author"].get("agent").is_some())
        .map(|m| {
            m["head"]["blocks"][0]["paragraph"][0]["text"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        })
        .collect()
}

fn pending_run_count(daemon: &Daemon) -> usize {
    let pending = daemon.query("runs", serde_json::json!("pending_runs"));
    pending["pending_runs"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(0)
}

use nettest::poll_until;

/// THE HEADLINE FIX, asserted directly: submit a run whose provider sleeps,
/// then Status and Query answer BEFORE the run completes. on the pre-fix
/// inline path the mention's submit itself blocked through provider
/// execution AND delivery, so "submit returned, reply not posted yet" was
/// unreachable — this test is red there without any wall-clock assertion.
#[test]
fn status_answers_while_a_slow_run_is_in_flight() {
    let storage = tempfile::TempDir::new().expect("storage dir");
    let fixtures = tempfile::TempDir::new().expect("provider fixtures dir");
    let env = stage_script_provider(
        fixtures.path(),
        "slow-quack",
        "cat > /dev/null\nsleep 6\nprintf 'slow answer\\n'",
    );
    let daemon = Daemon::spawn_inner(storage.path(), false, &env);
    arm_agent(&daemon, "general", "sloth", "slow-quack");

    let (code, block) = daemon.submit("chat", post_mention("general", "m1", "sloth"), Some("eddy"));
    assert_eq!(code, 200, "mention post failed: {block}");

    // the provider is asleep for ~6s. the daemon answers NOW:
    let status = daemon.status();
    assert!(
        status["height"].as_u64().is_some(),
        "status is live: {status}"
    );
    assert_eq!(
        pending_run_count(&daemon),
        1,
        "the run is in flight while status answered"
    );
    assert!(
        agent_replies(&daemon, "general").is_empty(),
        "status/query answered BEFORE the run completed"
    );

    // ... and the run still lands: the result re-enters as a submit, the
    // mailbox delivers next block, the reply posts, the pending entry prunes.
    poll_until(
        "the slow run's reply to post",
        Duration::from_secs(30),
        || {
            let replies = agent_replies(&daemon, "general");
            (!replies.is_empty()).then_some(replies)
        },
    );
    assert_eq!(agent_replies(&daemon, "general"), ["slow answer"]);
    poll_until(
        "the pending entry to prune",
        Duration::from_secs(30),
        || (pending_run_count(&daemon) == 0).then_some(()),
    );
}

/// two slow runs execute CONCURRENTLY: the second child starts while the
/// first is still asleep — the in-flight log carries start,start before any
/// end. on the pre-fix path the second mention's submit queued behind the
/// first run, so the log serialized (start,end,start,end).
#[test]
fn two_slow_runs_overlap() {
    let storage = tempfile::TempDir::new().expect("storage dir");
    let fixtures = tempfile::TempDir::new().expect("provider fixtures dir");
    let log = fixtures.path().join("exec.log");
    let env = stage_script_provider(
        fixtures.path(),
        "slow-pair",
        &format!(
            "cat > /dev/null\n\
             echo start >> {log}\n\
             sleep 3\n\
             echo end >> {log}\n\
             printf 'done\\n'",
            log = log.display()
        ),
    );
    let daemon = Daemon::spawn_inner(storage.path(), false, &env);
    // two channels, one agent each: two independent runs.
    arm_agent(&daemon, "left", "pair-a", "slow-pair");
    arm_agent(&daemon, "right", "pair-b", "slow-pair");

    let (code, block) = daemon.submit("chat", post_mention("left", "m1", "pair-a"), Some("eddy"));
    assert_eq!(code, 200, "first mention failed: {block}");
    let (code, block) = daemon.submit("chat", post_mention("right", "m2", "pair-b"), Some("eddy"));
    assert_eq!(code, 200, "second mention failed: {block}");

    for channel in ["left", "right"] {
        poll_until("both slow runs to reply", Duration::from_secs(30), || {
            let replies = agent_replies(&daemon, channel);
            (!replies.is_empty()).then_some(())
        });
    }
    let lines: Vec<String> = std::fs::read_to_string(&log)
        .expect("exec log written")
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(
        lines[..2],
        ["start", "start"],
        "both children started before either finished — the runs overlapped: {lines:?}"
    );
    assert_eq!(lines.len(), 4, "two complete executions: {lines:?}");
}

/// the failure path is loud, not silent: a provider that exits non-zero
/// still produces the failure OracleResult, the saga burns its attempts,
/// and the terminal failure prunes the pending entry — but the agent posts
/// exactly one ⚠ failure reply into the channel (authored as the agent,
/// carrying the provider's stderr excerpt) instead of dying silently. and
/// the daemon answers throughout.
#[test]
fn a_failing_provider_still_fails_the_run_cleanly() {
    let storage = tempfile::TempDir::new().expect("storage dir");
    let fixtures = tempfile::TempDir::new().expect("provider fixtures dir");
    let log = fixtures.path().join("exec.log");
    let env = stage_script_provider(
        fixtures.path(),
        "kaboom",
        &format!(
            "cat > /dev/null\n\
             echo ran >> {log}\n\
             echo 'provider exploded' >&2\n\
             exit 3",
            log = log.display()
        ),
    );
    let daemon = Daemon::spawn_inner(storage.path(), false, &env);
    arm_agent(&daemon, "general", "boomer", "kaboom");

    let (code, block) = daemon.submit(
        "chat",
        post_mention("general", "m1", "boomer"),
        Some("eddy"),
    );
    assert_eq!(code, 200, "mention post failed: {block}");

    // the terminal failure delivers (that is what prunes the entry) — the
    // saga's retry cycle ran through the pool to completion.
    poll_until("the failed run to prune", Duration::from_secs(30), || {
        (pending_run_count(&daemon) == 0).then_some(())
    });

    // the deliberate failure surface: exactly one ⚠ reply, authored as the
    // agent, one-reply-per-run message id, provider stderr in the excerpt.
    // (the anchor is a top-level post here, so the reply joins the channel
    // with `thread: null` — the runs crate's unit tests cover the reply
    // joining a threaded anchor's thread.)
    let reply = daemon.query(
        "chat",
        serde_json::json!({ "messages_latest": { "channel_id": "general", "limit": 16 } }),
    );
    let agent_msgs: Vec<&serde_json::Value> = reply["messages"]
        .as_array()
        .expect("Messages reply")
        .iter()
        .filter(|m| m["head"]["author"].get("agent").is_some())
        .collect();
    assert_eq!(
        agent_msgs.len(),
        1,
        "a failed run posts exactly one ⚠ failure reply: {reply}"
    );
    let head = &agent_msgs[0]["head"];
    assert_eq!(
        head["author"],
        serde_json::json!({ "agent": { "module": "runs", "agent_id": "boomer" } }),
        "the failure reply is authored as the agent"
    );
    let run_id = "chat\u{1f}general\u{1f}1\u{1f}boomer";
    assert_eq!(
        head["message_id"],
        format!("agent/{run_id}"),
        "one reply per run, failure included"
    );
    assert!(
        head["thread"].is_null(),
        "a top-level anchor's failure reply is not threaded: {head}"
    );
    let text = head["blocks"][0]["paragraph"][0]["text"]
        .as_str()
        .expect("reply text");
    assert!(
        text.starts_with("⚠ boomer failed: "),
        "the ⚠ failure reply names the agent: {text}"
    );
    assert!(
        text.contains("provider exploded"),
        "the provider's stderr surfaces in the failure excerpt: {text}"
    );
    let executions = std::fs::read_to_string(&log)
        .map(|s| s.lines().count())
        .unwrap_or(0);
    assert!(
        executions >= 1,
        "the provider actually ran (got {executions} executions)"
    );
    daemon.status(); // still alive, still answering.
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

/// Regression for stateless upload-pack negotiation: once a checkout has
/// common objects with Forge, stock git sends one or more flush-ended `have`
/// rounds before `done`. The server must answer those rounds with NAK only;
/// PACK bytes are legal only in the final response.
#[test]
fn git_fetch_and_pull_into_nonempty_checkout_complete_negotiation() {
    if !have_git() {
        eprintln!(
            "skipping git_fetch_and_pull_into_nonempty_checkout_complete_negotiation: no `git` on PATH"
        );
        return;
    }
    let storage = tempfile::TempDir::new().expect("storage dir");
    let daemon = Daemon::spawn(storage.path());
    let url = format!("http://127.0.0.1:{}/forge/negotiated", daemon.port);

    let source = tempfile::TempDir::new().expect("source repo");
    let src = source.path();
    git_ok(src, &["init"]);
    // More than git's initial have window guarantees at least one have batch
    // ends in a flush before the client reaches `done`.
    for number in 1..=20 {
        let content = format!("base {number}\n");
        let message = format!("base commit {number}");
        commit_file(src, "history.txt", &content, &message);
    }
    git_ok(src, &["remote", "add", "ducktape", &url]);
    git_ok(src, &["push", "ducktape", "main"]);
    let first_head = rev_parse_head(src);

    let checkout_root = tempfile::TempDir::new().expect("checkout root");
    let checkout = checkout_root.path().join("checkout");
    git_ok(
        checkout_root.path(),
        &["clone", &url, checkout.to_str().expect("utf-8 checkout path")],
    );

    // A fetch from a non-empty repo has a common first commit. This exercises
    // the intermediate have/NAK round and leaves the worktree at its prior head.
    commit_file(src, "history.txt", "fetched\n", "fetched commit");
    git_ok(src, &["push", "ducktape", "main"]);
    let fetch = git_capture(&checkout, &["fetch", "origin"]);
    eprintln!("=== negotiated git fetch ===\n{}", render(&fetch));
    assert!(
        fetch.status.success(),
        "fetch into a non-empty checkout failed:\n{}",
        render(&fetch)
    );
    assert_eq!(
        rev_parse_head(&checkout),
        first_head,
        "fetch must not move the checked-out branch"
    );

    // Advance once more so pull performs its own negotiated fetch, then verify
    // both the ref update and checkout bytes through stock git.
    commit_file(src, "history.txt", "pulled\n", "pulled commit");
    git_ok(src, &["push", "ducktape", "main"]);
    let pull = git_capture(&checkout, &["pull", "--ff-only"]);
    eprintln!("=== negotiated git pull ===\n{}", render(&pull));
    assert!(
        pull.status.success(),
        "pull into a non-empty checkout failed:\n{}",
        render(&pull)
    );
    assert_eq!(rev_parse_head(&checkout), rev_parse_head(src));
    assert_eq!(
        std::fs::read(checkout.join("history.txt")).expect("read pulled file"),
        b"pulled\n"
    );
}

/// The desktop remote-forge mirror fetches with LIBGIT2, not stock git: a
/// fresh bare mirror pulls the full closure after a NAK, and a re-sync after
/// the origin advances completes against the ACKed incremental pack — the
/// exact client the app's `forge_sync_remote` runs, so this pins that interop.
#[test]
fn libgit2_mirror_fetch_completes_incremental_sync() {
    if !have_git() {
        eprintln!("skipping libgit2_mirror_fetch_completes_incremental_sync: no `git` on PATH");
        return;
    }
    let storage = tempfile::TempDir::new().expect("storage dir");
    let daemon = Daemon::spawn(storage.path());
    let url = format!("http://127.0.0.1:{}/forge/mirrored", daemon.port);

    let source = tempfile::TempDir::new().expect("source repo");
    let src = source.path();
    git_ok(src, &["init"]);
    commit_file(src, "history.txt", "one\n", "first commit");
    git_ok(src, &["remote", "add", "ducktape", &url]);
    git_ok(src, &["push", "ducktape", "main"]);
    let first_head = rev_parse_head(src);

    let mirror_dir = tempfile::TempDir::new().expect("mirror dir");
    let mirror = git2::Repository::init_bare(mirror_dir.path()).expect("init mirror");
    let refspec = ["+refs/heads/*:refs/heads/*"];
    let fetch = |mirror: &git2::Repository| {
        let mut remote = mirror.remote_anonymous(&url).expect("anonymous remote");
        remote
            .fetch(&refspec, None::<&mut git2::FetchOptions<'_>>, None)
            .expect("libgit2 fetch");
    };

    fetch(&mirror);
    let first_oid = git2::Oid::from_str(&first_head).expect("head oid");
    assert!(mirror.find_commit(first_oid).is_ok(), "fresh sync lands the head");

    // origin advances; the re-sync's haves earn an ACK + delta pack, and the
    // mirror must still complete the new head's closure from it.
    commit_file(src, "history.txt", "two\n", "second commit");
    git_ok(src, &["push", "ducktape", "main"]);
    let second_head = rev_parse_head(src);
    fetch(&mirror);
    let second_oid = git2::Oid::from_str(&second_head).expect("head oid");
    let landed = mirror.find_commit(second_oid).expect("incremental sync lands the head");
    assert_eq!(
        landed
            .tree()
            .expect("tree")
            .get_name("history.txt")
            .map(|entry| entry.id()),
        git2::Repository::open(src)
            .expect("open source")
            .find_commit(second_oid)
            .expect("source head")
            .tree()
            .expect("source tree")
            .get_name("history.txt")
            .map(|entry| entry.id()),
        "the delta pack must complete the changed blob"
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
        eprintln!(
            "skipping git_push_larger_than_post_buffer_uses_the_probe_path: no `git` on PATH"
        );
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

// ============================================================================
// the FULL-STACK proof of the client transport: the `duckfs-client` checkout/
// commit engine driven through `HttpNode` against a real spawned daemon —
// checkout an empty prefix, write a small file AND a >1 MiB file (the stage
// path), commit, checkout again byte-identically, then force a same-path
// conflict and assert it surfaces a structured `ConflictReport` (never a silent
// merge). the hand-rolled `HttpNode` contract lives in the crate's
// `http_contract.rs`; this is the wire against the actual noded routes.
// ============================================================================

#[test]
fn duckfs_engine_round_trips_and_reports_conflict_through_http_node() {
    use duckfs_client::checkout::{CheckoutOptions, checkout_with};
    use duckfs_client::commit::{CommitError, commit};
    use duckfs_client::http::HttpNode;

    let storage = tempfile::TempDir::new().expect("storage dir");
    let daemon = Daemon::spawn(storage.path());
    let base_url = format!("http://127.0.0.1:{}", daemon.port);
    let node = HttpNode::new(base_url.clone());
    let opts = CheckoutOptions {
        node_url: base_url.clone(),
        ..Default::default()
    };

    // checkout the empty prefix: base is None (nothing committed yet).
    let dir_a = tempfile::TempDir::new().expect("checkout a");
    let idx = checkout_with(&node, dir_a.path(), "/shared/e2e", None, &opts)
        .expect("checkout empty prefix");
    assert!(idx.base_snapshot.is_none(), "empty checkout has no base");

    // a small (inline) file and a >1 MiB file — the latter forces the stage
    // path through real consensus (POST /v1/files/stage per chunk).
    std::fs::write(dir_a.path().join("small"), b"hello duckfs engine").expect("write small");
    let big: Vec<u8> = (0..(2 * 1024 * 1024 + 7))
        .map(|i| (i % 251) as u8)
        .collect();
    std::fs::write(dir_a.path().join("big"), &big).expect("write big");

    let summary = commit(&node, dir_a.path(), "seed via engine").expect("commit seed");
    assert!(!summary.rebased, "a first commit never rebases");

    // a fresh checkout elsewhere reads back byte-identical (the big file is
    // reassembled from staged chunks and verified against its object id).
    let dir_b = tempfile::TempDir::new().expect("checkout b");
    checkout_with(&node, dir_b.path(), "/shared/e2e", None, &opts).expect("checkout again");
    assert_eq!(
        std::fs::read(dir_b.path().join("small")).unwrap(),
        b"hello duckfs engine",
        "small file round-trips"
    );
    assert_eq!(
        std::fs::read(dir_b.path().join("big")).unwrap(),
        big,
        ">1 MiB file round-trips byte-identical"
    );

    // both checkouts edit the SAME path off the same base: A lands, B must
    // surface a ConflictReport naming the clashing path — no silent merge.
    std::fs::write(dir_a.path().join("small"), b"edit from A").expect("edit a");
    std::fs::write(dir_b.path().join("small"), b"edit from B").expect("edit b");
    commit(&node, dir_a.path(), "A wins").expect("A commits clean");
    let err = commit(&node, dir_b.path(), "B loses").expect_err("B must conflict");
    match err {
        CommitError::Conflict(report) => {
            assert!(
                report.clashing.iter().any(|p| p == "/shared/e2e/small"),
                "the conflicting path is named in the report: {report:?}"
            );
        }
        other => panic!("expected a structured conflict, got {other:?}"),
    }
}

// ============================================================================
// workspace RPC (the jobs/sandbox seam): the daemon manages a checkout under an
// injected root, driven entirely over http — create -> files on disk -> commit
// -> read back over the files surface -> delete. state lives on disk under
// `<storage>/duckfs-workspaces/<id>`, so this test reads/writes that path
// directly (same machine). a conflicting workspace commit is a 409 carrying the
// serialized ConflictReport.
// ============================================================================

#[test]
fn duckfs_workspace_rpc_maps_workspace_prefix_into_managed_namespace() {
    let storage = tempfile::TempDir::new().expect("storage dir");
    let daemon = Daemon::spawn(storage.path());

    // `/workspace` is the caller's local sandbox vocabulary. The workspace RPC
    // owns the duckfs namespace choice; it must not persist `/workspace` into
    // the .duckfs index and let commit fail later with the module's authority
    // error ("files: path is outside /home and /shared").
    let (code, ws) = daemon.request(
        "POST",
        "/v1/fs/workspaces",
        Some(&serde_json::json!({ "prefix": "/workspace" })),
    );
    assert_eq!(code, 200, "create managed workspace failed: {ws}");
    let id = ws["id"].as_str().expect("workspace id").to_string();
    let path = ws["path"].as_str().expect("workspace path").to_string();

    std::fs::write(std::path::Path::new(&path).join("hello.txt"), b"inside").unwrap();
    let (code, done) = daemon.request(
        "POST",
        &format!("/v1/fs/workspaces/{id}/commit"),
        Some(&serde_json::json!({ "message": "commit managed workspace" })),
    );
    assert_eq!(
        code, 200,
        "workspace commit should use a managed duckfs prefix: {done}"
    );

    let index = duckfs_client::index::Index::load(std::path::Path::new(&path)).unwrap();
    assert!(
        index.prefix.starts_with("/shared/workspaces/"),
        "the managed checkout records an internal writable prefix, got {}",
        index.prefix
    );
    let read_path = format!("{}/hello.txt", index.prefix);
    let (code, read) = daemon.request("GET", &format!("/v1/files/read?path={read_path}"), None);
    assert_eq!(code, 200, "read committed managed workspace file: {read}");
    let bytes = STANDARD
        .decode(read["b64"].as_str().expect("b64").as_bytes())
        .expect("decode b64");
    assert_eq!(bytes, b"inside");
}

#[test]
fn duckfs_workspace_rpc_lifecycle_and_conflict() {
    let storage = tempfile::TempDir::new().expect("storage dir");
    let daemon = Daemon::spawn(storage.path());

    // ---- create: an empty checkout under /shared/job1 ----
    let (code, ws) = daemon.request(
        "POST",
        "/v1/fs/workspaces",
        Some(&serde_json::json!({ "prefix": "/shared/job1" })),
    );
    assert_eq!(code, 200, "create workspace failed: {ws}");
    let id = ws["id"].as_str().expect("workspace id").to_string();
    let path = ws["path"].as_str().expect("workspace path").to_string();
    assert!(ws["snapshot"].is_null(), "empty checkout has no base: {ws}");
    // the managed checkout wrote its .duckfs index to disk at `path`.
    let index_json = std::path::Path::new(&path).join(".duckfs/index.json");
    assert!(
        index_json.exists(),
        "the workspace index must exist at {}",
        index_json.display()
    );

    // ---- edit on disk, then commit over rpc ----
    std::fs::write(
        std::path::Path::new(&path).join("hello.txt"),
        b"workspace bytes",
    )
    .expect("write into the workspace");
    let (code, done) = daemon.request(
        "POST",
        &format!("/v1/fs/workspaces/{id}/commit"),
        Some(&serde_json::json!({ "message": "commit from the workspace rpc" })),
    );
    assert_eq!(code, 200, "workspace commit failed: {done}");
    assert!(
        done["snapshot"].is_string(),
        "commit returns a snapshot id: {done}"
    );
    assert_eq!(done["rebased"], false, "a first commit never rebases");

    // ---- read the committed file back over the files surface ----
    let (code, read) = daemon.request("GET", "/v1/files/read?path=/shared/job1/hello.txt", None);
    assert_eq!(code, 200, "read the committed file: {read}");
    let bytes = STANDARD
        .decode(read["b64"].as_str().expect("b64").as_bytes())
        .expect("decode b64");
    assert_eq!(bytes, b"workspace bytes", "the committed bytes round-trip");

    // ---- delete: the workspace directory is gone ----
    let (code, gone) = daemon.request("DELETE", &format!("/v1/fs/workspaces/{id}"), None);
    assert_eq!(code, 200, "delete workspace failed: {gone}");
    assert_eq!(gone["ok"], true);
    assert!(
        !std::path::Path::new(&path).exists(),
        "the workspace dir is removed on delete"
    );

    // ---- conflict: two workspaces off the same base, same-path edits ----
    let make_ws = || {
        let (c, v) = daemon.request(
            "POST",
            "/v1/fs/workspaces",
            Some(&serde_json::json!({ "prefix": "/shared/wsconflict" })),
        );
        assert_eq!(c, 200, "create conflict workspace: {v}");
        (
            v["id"].as_str().unwrap().to_string(),
            v["path"].as_str().unwrap().to_string(),
        )
    };
    let commit_ws = |id: &str, msg: &str| -> (u16, serde_json::Value) {
        daemon.request(
            "POST",
            &format!("/v1/fs/workspaces/{id}/commit"),
            Some(&serde_json::json!({ "message": msg })),
        )
    };

    let (id1, path1) = make_ws();
    std::fs::write(std::path::Path::new(&path1).join("f.txt"), b"v1").unwrap();
    let (c, _) = commit_ws(&id1, "seed");
    assert_eq!(c, 200, "seed commit lands");

    // ws2 checks out the seeded head (its base is snapshot1).
    let (id2, path2) = make_ws();

    // ws1 advances the shared path...
    std::fs::write(std::path::Path::new(&path1).join("f.txt"), b"from1").unwrap();
    let (c, _) = commit_ws(&id1, "advance");
    assert_eq!(c, 200, "ws1 advances the shared path");

    // ...so ws2's same-path commit conflicts: a 409 with the clashing path.
    std::fs::write(std::path::Path::new(&path2).join("f.txt"), b"from2").unwrap();
    let (c, report) = commit_ws(&id2, "loses");
    assert_eq!(c, 409, "an overlapping workspace commit is a 409: {report}");
    let clashing = report["clashing"].as_array().expect("clashing array");
    assert!(
        clashing.iter().any(|p| p == "/shared/wsconflict/f.txt"),
        "the conflict report names the clashing path: {report}"
    );
}
