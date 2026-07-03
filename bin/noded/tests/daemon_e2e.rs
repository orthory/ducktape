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
        Self::spawn_inner(storage, false)
    }

    fn spawn_with_echo_oracle(storage: &Path) -> Self {
        Self::spawn_inner(storage, true)
    }

    fn spawn_inner(storage: &Path, echo_oracle: bool) -> Self {
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

fn post_mention(channel: &str, message_id: &str, agent_id: &str) -> serde_json::Value {
    serde_json::json!({
        "PostMessage": {
            "channel_id": channel,
            "message_id": message_id,
            "blocks": [{
                "Paragraph": [
                    { "text": "hey ", "marks": [] },
                    {
                        "text": format!("@{agent_id}"),
                        "marks": [{
                            "Mention": {
                                "Agent": { "module": "agent", "agent_id": agent_id }
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
            "tasks",
            "inbox",
            "automations",
            "jobs",
            "agent",
            "document",
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
fn agent_run_drains_oracle_effect_and_posts_reply() {
    let storage = tempfile::TempDir::new().expect("storage dir");
    let daemon = Daemon::spawn_with_echo_oracle(storage.path());

    let (code, block) = daemon.submit(
        "chat",
        serde_json::json!({
            "CreateChannel": { "channel_id": "general", "name": "General", "post_policy": "Open" }
        }),
        Some("owner"),
    );
    assert_eq!(code, 200, "create channel failed: {block}");

    let prompt_hash = vec![7u8; 32];
    let (code, block) = daemon.submit(
        "agent",
        serde_json::json!({
            "RegisterAgent": {
                "agent_id": "quackbot",
                "display_name": "Quackbot",
                "model_ref": "echo-model",
                "prompt_hash": prompt_hash,
                "allowed_actions": ["chat.post"]
            }
        }),
        Some("owner"),
    );
    assert_eq!(code, 200, "register agent failed: {block}");

    let (code, block) = daemon.submit(
        "agent",
        serde_json::json!({
            "WatchChannel": {
                "channel_id": "general",
                "policy": "Mention"
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
    assert_eq!(
        block["height"], 5,
        "the post block plus oracle follow-up block should both drain"
    );

    let run_id = "chat\u{1f}general\u{1f}1\u{1f}quackbot";
    let run = daemon.query("agent", serde_json::json!({ "Run": { "run_id": run_id } }));
    assert_eq!(
        run["Run"]["status"], "Done",
        "run should settle Done: {run}"
    );

    let reply = daemon.query(
        "chat",
        serde_json::json!({ "MessagesLatest": { "channel_id": "general", "limit": 16 } }),
    );
    let messages = reply["Messages"].as_array().expect("Messages reply");
    assert_eq!(messages.len(), 2, "user post plus agent reply should exist");
    let agent_reply = &messages[1]["head"];
    assert_eq!(agent_reply["message_id"], format!("agent/{run_id}"));
    assert_eq!(
        agent_reply["author"],
        serde_json::json!({ "Agent": { "module": "agent", "agent_id": "quackbot" } })
    );
    assert_eq!(
        agent_reply["blocks"][0]["Paragraph"][0]["text"],
        format!("echo: handling {run_id}")
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
        files_interface::digest_hex(&chunk),
        "the returned digest is sha256 of the exact uploaded bytes"
    );

    // fetch round-trips byte-identical.
    let (code, fetched) = daemon.request_bytes("GET", &format!("/v1/files/blob/{digest}"), &[]);
    assert_eq!(code, 200);
    assert_eq!(fetched, chunk, "fetched bytes must be byte-identical");

    // a well-formed digest nobody uploaded is a 404; a malformed digest
    // (uppercase hex included) is a 400, not a miss.
    let absent = files_interface::digest_hex(b"never uploaded");
    let (code, _) = daemon.request_bytes("GET", &format!("/v1/files/blob/{absent}"), &[]);
    assert_eq!(code, 404, "absent chunk must be a 404");
    let upper = digest.to_uppercase();
    let (code, _) = daemon.request_bytes("GET", &format!("/v1/files/blob/{upper}"), &[]);
    assert_eq!(code, 400, "digest must be lowercase hex");

    // the cap is MAX_CHUNK_SIZE inclusive: exactly 4 MiB lands...
    let max = vec![0xABu8; files_interface::MAX_CHUNK_SIZE as usize];
    let (code, _) = daemon.request_bytes("POST", "/v1/files/blob", &max);
    assert_eq!(code, 200, "a chunk of exactly MAX_CHUNK_SIZE must land");
    // ...and one byte more is a 413 in the daemon's error envelope.
    let over = vec![0xCDu8; files_interface::MAX_CHUNK_SIZE as usize + 1];
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
            "AddManifest": {
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

    let reply = daemon.query("files", serde_json::json!({ "Stat": { "file_id": "f1" } }));
    let manifest: files_interface::Manifest =
        serde_json::from_value(reply["Stat"].clone()).expect("Stat carries the manifest");
    files_interface::verify_chunk(&manifest, 0, &fetched)
        .expect("fetched bytes verify against the committed manifest");
}
