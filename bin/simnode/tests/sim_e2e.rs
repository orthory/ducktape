//! sim e2e: a REAL spawned `ducktape-simnode` driven over its /v1 + /sim
//! wires. what this suite pins is the DRIVER's contract — held submits commit
//! only on step, the logical clock makes identical scripts reproduce identical
//! app-hashes, personas shape the receipt/ring exactly like the two real
//! nodes, and peer blocks commit past a parked submit queue. the /v1 routes
//! themselves are noded's (covered by noded's own router/daemon suites).
//!
//! transport is the same deliberately raw std-TCP http/1.1 as noded's
//! daemon_e2e: any plain http client must be a full citizen of this wire.

use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct Sim {
    child: Child,
    port: u16,
}

impl Drop for Sim {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Sim {
    /// spawn with an explicit fresh storage dir: the sim has no height-resume
    /// watermark, so reusing a dir would restart heights over persisted state.
    fn spawn(storage: &Path, extra_args: &[&str]) -> Self {
        let port = free_port();
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_ducktape-simnode"));
        cmd.arg("--listen")
            .arg(format!("127.0.0.1:{port}"))
            .arg("--storage")
            .arg(storage)
            .args(extra_args)
            .stdout(Stdio::null())
            // startup failures land on stderr — keep it visible or they read
            // as an opaque readiness timeout.
            .stderr(Stdio::inherit());
        let child = cmd.spawn().expect("spawn ducktape-simnode");
        let mut sim = Self { child, port };
        sim.await_status();
        sim
    }

    fn await_status(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Ok((200, _)) = try_request(self.port, "GET", "/v1/status", None) {
                return;
            }
            if let Some(status) = self.child.try_wait().expect("poll sim") {
                panic!("sim exited during startup ({status}) — see stderr above");
            }
            assert!(
                Instant::now() < deadline,
                "sim on port {} never answered /v1/status",
                self.port
            );
            std::thread::sleep(Duration::from_millis(150));
        }
    }

    fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> (u16, serde_json::Value) {
        try_request(self.port, method, path, body).expect("sim reachable")
    }

    fn status(&self) -> serde_json::Value {
        let (status, reply) = self.request("GET", "/v1/status", None);
        assert_eq!(status, 200, "status failed: {reply}");
        reply
    }

    fn sim_state(&self) -> serde_json::Value {
        let (status, reply) = self.request("GET", "/sim/state", None);
        assert_eq!(status, 200, "sim state failed: {reply}");
        reply
    }

    fn step(&self) -> serde_json::Value {
        let (status, reply) = self.request("POST", "/sim/step", None);
        assert_eq!(status, 200, "step failed: {reply}");
        reply
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

    /// spawn a submit on its own thread — in hold mode the http reply hangs
    /// until a step releases it, so the caller must not block on it inline.
    fn submit_in_background(
        &self,
        target: &str,
        payload: serde_json::Value,
    ) -> std::thread::JoinHandle<(u16, serde_json::Value)> {
        let port = self.port;
        let target = target.to_string();
        std::thread::spawn(move || {
            try_request(
                port,
                "POST",
                "/v1/submit",
                Some(&serde_json::json!({ "target": target, "payload": payload })),
            )
            .expect("held submit reachable")
        })
    }

    /// poll /sim/state until `field` reaches `want` — the held queue is fed by
    /// another thread's in-flight request, so arrival is asynchronous.
    fn await_sim_state(&self, field: &str, want: u64) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if self.sim_state()[field] == want {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "sim state {field} never reached {want}"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}

fn try_request(
    port: u16,
    method: &str,
    path: &str,
    body: Option<&serde_json::Value>,
) -> std::io::Result<(u16, serde_json::Value)> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
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
        .map(|b| serde_json::from_str(b.trim()).unwrap_or(serde_json::Value::Null))
        .unwrap_or(serde_json::Value::Null);
    Ok((status, payload))
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind port probe")
        .local_addr()
        .expect("probe addr")
        .port()
}

fn create_channel(channel: &str, name: &str) -> serde_json::Value {
    serde_json::json!({
        "create_channel": { "channel_id": channel, "name": name, "post_policy": "open" }
    })
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

// ── The driver contract ─────────────────────────────────

#[test]
fn held_submit_commits_only_on_step() {
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &[]);

    let pending = sim.submit_in_background("chat", create_channel("general", "General"));
    sim.await_sim_state("held", 1);

    // parked: nothing committed, and reads serve pre-op state.
    assert_eq!(sim.status()["height"], 0, "held submit must not commit");
    let channels = sim.query("chat", serde_json::json!("channels"));
    assert_eq!(
        channels["channels"].as_array().map(Vec::len),
        Some(0),
        "held op must be invisible to queries: {channels}"
    );

    let report = sim.step();
    assert_eq!(report["committed"]["kind"], "held", "step report: {report}");
    assert_eq!(report["committed"]["height"], 1);

    // the step released the parked http reply — a receipt for block 1, with
    // the local persona's opHash content address.
    let (code, receipt) = pending.join().expect("submit thread");
    assert_eq!(code, 200, "released submit failed: {receipt}");
    assert_eq!(receipt["height"], 1, "receipt is the inclusion block");
    assert_eq!(
        receipt["opHash"].as_str().map(str::len),
        Some(64),
        "local persona receipts carry the content address: {receipt}"
    );

    assert_eq!(sim.status()["height"], 1);
    let channels = sim.query("chat", serde_json::json!("channels"));
    assert_eq!(channels["channels"].as_array().map(Vec::len), Some(1));
}

#[test]
fn same_script_same_app_hash() {
    // the whole point of the logical clock: two fresh sims fed an identical
    // op script must walk identical app-hashes, block by block.
    let run = || -> Vec<String> {
        let storage = tempfile::tempdir().expect("storage dir");
        let sim = Sim::spawn(storage.path(), &[]);
        let mut hashes = Vec::new();
        for (target, payload) in [
            ("chat", create_channel("general", "General")),
            ("chat", post_message("general", "m-1", "hello determinism")),
            ("tasks", serde_json::json!({ "create_task": { "task_id": "t-1", "title": "repeatable" } })),
        ] {
            let pending = sim.submit_in_background(target, payload);
            sim.await_sim_state("held", 1);
            let report = sim.step();
            hashes.push(
                report["committed"]["appHash"]
                    .as_str()
                    .expect("step committed")
                    .to_string(),
            );
            let (code, _) = pending.join().expect("submit thread");
            assert_eq!(code, 200);
        }
        hashes.push(sim.status()["appHash"].as_str().unwrap_or_default().to_string());
        hashes
    };

    assert_eq!(run(), run(), "identical scripts diverged");
}

#[test]
fn personas_shape_receipts_and_ring() {
    // networked: height-only receipts, populated ring, addressable op bytes.
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &["--persona", "networked"]);

    let pending = sim.submit_in_background("chat", create_channel("general", "General"));
    sim.await_sim_state("held", 1);
    sim.step();
    let (code, receipt) = pending.join().expect("submit thread");
    assert_eq!(code, 200, "released submit failed: {receipt}");
    assert_eq!(receipt["height"], 1);
    assert!(
        receipt.get("opHash").is_none(),
        "networked receipts are height-only: {receipt}"
    );

    let (code, body) = sim.request("GET", "/v1/blocks", None);
    assert_eq!(code, 200);
    let records = body["blocks"].as_array().expect("blocks is an array");
    assert_eq!(records.len(), 1, "networked persona fills the ring: {body}");
    // a block carries its member ops under `ops[]`; the sim is one op per block.
    assert_eq!(records[0]["ops"][0]["target"], "chat");
    let op_hash = records[0]["ops"][0]["opHash"].as_str().unwrap_or_default();
    assert_eq!(op_hash.len(), 64, "ring records carry the content address");
    let (code, blob) = sim.request("GET", &format!("/v1/files/blob/{op_hash}"), None);
    assert_eq!(code, 200, "op hash must dereference on the blob lane");
    assert_eq!(
        blob,
        create_channel("general", "General"),
        "blob lane serves the committed payload back"
    );

    // local: receipts carry opHash — and the blocks lane is served either
    // way (both real daemons feed the durable block index; the persona only
    // shapes the receipt).
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &["--persona", "local"]);
    let pending = sim.submit_in_background("chat", create_channel("general", "General"));
    sim.await_sim_state("held", 1);
    sim.step();
    let (code, receipt) = pending.join().expect("submit thread");
    assert_eq!(code, 200);
    assert_eq!(receipt["opHash"].as_str().map(str::len), Some(64));
    let (code, body) = sim.request("GET", "/v1/blocks", None);
    assert_eq!(code, 200);
    let records = body["blocks"].as_array().expect("blocks is an array");
    assert_eq!(records.len(), 1, "the durable block index serves both personas: {body}");
    assert_eq!(
        records[0]["hash"], "",
        "nothing is framed on this lane — the frame hash stays empty: {body}"
    );
}

#[test]
fn auto_and_step_commit_paths_walk_identical_app_hashes() {
    // hold mode commits through step_once, auto mode through handle_submit's
    // inline drain — two code paths, one logical clock. the same script must
    // walk the same app-hashes through either, or a refactor of one path has
    // quietly forked the sim's determinism contract.
    let script = || {
        [
            ("chat", create_channel("general", "General")),
            ("chat", post_message("general", "m-1", "hello determinism")),
            ("tasks", serde_json::json!({ "create_task": { "task_id": "t-1", "title": "repeatable" } })),
        ]
    };

    let stepped: Vec<String> = {
        let storage = tempfile::tempdir().expect("storage dir");
        let sim = Sim::spawn(storage.path(), &[]);
        script()
            .into_iter()
            .map(|(target, payload)| {
                let pending = sim.submit_in_background(target, payload);
                sim.await_sim_state("held", 1);
                let report = sim.step();
                let (code, _) = pending.join().expect("submit thread");
                assert_eq!(code, 200);
                report["committed"]["appHash"]
                    .as_str()
                    .expect("step committed")
                    .to_string()
            })
            .collect()
    };

    let auto: Vec<String> = {
        let storage = tempfile::tempdir().expect("storage dir");
        let sim = Sim::spawn(storage.path(), &["--auto"]);
        script()
            .into_iter()
            .map(|(target, payload)| {
                // auto mode: the submit reply IS the commit receipt.
                let (code, receipt) = sim.request(
                    "POST",
                    "/v1/submit",
                    Some(&serde_json::json!({ "target": target, "payload": payload })),
                );
                assert_eq!(code, 200, "auto submit failed: {receipt}");
                receipt["appHash"]
                    .as_str()
                    .expect("receipt app hash")
                    .to_string()
            })
            .collect()
    };

    assert_eq!(stepped, auto, "the two commit paths diverged");
}

#[test]
fn peer_block_commits_past_a_parked_queue() {
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &[]);

    // park a submit, then let a "concurrent writer" land first.
    let pending = sim.submit_in_background("chat", create_channel("mine", "Mine"));
    sim.await_sim_state("held", 1);

    let (code, peer) = sim.request(
        "POST",
        "/sim/peer-block",
        Some(&serde_json::json!({
            "target": "chat",
            "payload": create_channel("theirs", "Theirs"),
            "origin": "rival",
        })),
    );
    assert_eq!(code, 200, "peer block failed: {peer}");
    assert_eq!(peer["height"], 1, "peer commits ahead of the parked op");
    assert_eq!(sim.sim_state()["held"], 1, "the parked submit stays parked");

    let report = sim.step();
    assert_eq!(report["committed"]["height"], 2, "held op lands after: {report}");
    let (code, receipt) = pending.join().expect("submit thread");
    assert_eq!(code, 200);
    assert_eq!(receipt["height"], 2);

    // both writers' state committed, in that order.
    let channels = sim.query("chat", serde_json::json!("channels"));
    assert_eq!(channels["channels"].as_array().map(Vec::len), Some(2));
}
