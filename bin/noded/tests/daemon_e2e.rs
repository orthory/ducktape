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
        "PostMessage": {
            "channel_id": channel,
            "message_id": message_id,
            "blocks": [{ "Paragraph": [{ "text": text, "marks": [] }] }],
            "thread": null,
            "as_agent": null,
        }
    })
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
    assert_eq!(modules, ["chat", "tasks", "inbox", "document", "forge"]);
    let genesis_hash = status["appHash"].as_str().expect("appHash").to_string();

    // subscribe BEFORE submitting: every committed block must fan out.
    let mut ws = daemon.ws_connect();

    // one msg = one block; the summary echoes the new height + app-hash.
    let (code, block) = daemon.submit(
        "chat",
        serde_json::json!({
            "CreateChannel": { "channel_id": "general", "name": "General", "post_policy": "Open" }
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

    // the ws stream carries both blocks, tagged and in order.
    for expected_height in [1u64, 2] {
        let frame: serde_json::Value =
            serde_json::from_str(&Daemon::ws_read_text(&mut ws)).expect("ws frame json");
        assert_eq!(frame["type"], "block");
        assert_eq!(frame["height"], expected_height);
    }

    // committed state reads back; authorship derived from the submit origin.
    let reply = daemon.query(
        "chat",
        serde_json::json!({ "MessagesLatest": { "channel_id": "general", "limit": 16 } }),
    );
    let messages = reply["Messages"].as_array().expect("Messages reply");
    assert_eq!(messages.len(), 1);
    let head = &messages[0]["head"];
    assert_eq!(head["message_id"], "m1");
    assert_eq!(head["blocks"][0]["Paragraph"][0]["text"], "hello from e2e");
    let author_bytes: Vec<u8> = head["author"]["User"]
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
fn state_persists_across_restart() {
    let storage = tempfile::TempDir::new().expect("storage dir");

    {
        let daemon = Daemon::spawn(storage.path());
        let (code, _) = daemon.submit(
            "chat",
            serde_json::json!({
                "CreateChannel": { "channel_id": "durable", "name": "Durable", "post_policy": "Open" }
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

    // a fresh daemon over the SAME storage root: qmdb state must survive; the
    // height counter is a local block counter and restarts at 0 by design.
    let daemon = Daemon::spawn(storage.path());
    assert_eq!(daemon.status()["height"], 0);
    let reply = daemon.query(
        "chat",
        serde_json::json!({ "MessagesLatest": { "channel_id": "durable", "limit": 16 } }),
    );
    let messages = reply["Messages"].as_array().expect("Messages reply");
    assert_eq!(messages.len(), 1, "chat state must survive a restart");
    assert_eq!(
        messages[0]["head"]["blocks"][0]["Paragraph"][0]["text"],
        "written before restart"
    );
}
