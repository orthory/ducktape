//! the shared spawn/drive harness behind the sim integration suites: a REAL
//! `ducktape-simnode` child process driven over its /v1 + /sim wires.
//! transport is the same deliberately raw std-TCP http/1.1 as noded's
//! daemon_e2e: any plain http client must be a full citizen of this wire.
//!
//! each tests/*.rs file is its own crate, so unused helpers per binary are
//! expected — hence the file-wide dead_code allow.
#![allow(dead_code)]

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub struct Sim {
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
    /// spawn a sim child against `storage`. a FRESH dir is the norm — the
    /// determinism scenarios need one, since reused module state defeats the
    /// same-script reproducibility the tool exists for — but reusing a dir is a
    /// SUPPORTED restart: the sim resumes height above the index watermark
    /// (`index.resume_height()`) and every qmdb-backed module reloads its
    /// committed state, so respawning on the same dir CONTINUES the chain rather
    /// than restarting at 0 (exercised by reactor_seams.rs's restart scenario;
    /// `Drop` kills+waits the prior child, so a plain drop-then-respawn is safe).
    pub fn spawn(storage: &Path, extra_args: &[&str]) -> Self {
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

    /// Block until this sim answers `/v1/status`.
    ///
    /// Liveness FIRST, and the order is the whole point: a child that lost its
    /// listen port exits, and something else is then answering on that number.
    /// Probing first would read the WINNER's 200 as this child's readiness, and
    /// the test would drive a stranger's sim for its entire run — visible only
    /// as an unrelated flake. Asking "is my child alive?" before "did someone
    /// answer?" turns that into a named startup failure. (The same reorder
    /// landed in `bin/noded/tests/daemon_e2e.rs`; this harness was the copy left
    /// behind.)
    fn await_status(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(status) = self.child.try_wait().expect("poll sim") {
                panic!("sim exited during startup ({status}) — see stderr above");
            }
            if let Ok((200, _)) = try_request(self.port, "GET", "/v1/status", None) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "sim on port {} never answered /v1/status",
                self.port
            );
            std::thread::sleep(Duration::from_millis(150));
        }
    }

    /// this sim's http port — for the routes the helpers below do not wrap,
    /// namely `/v1/admin/*`, which needs a credential header no json helper
    /// carries.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// block until the child exits ON ITS OWN — the event a graceful
    /// `/v1/admin/shutdown` produces. `Drop`'s kill is the backstop for a test
    /// that never reaches here; this is what a shutdown assertion waits on, and
    /// it waits on the process's own exit rather than on a duration.
    pub fn wait_for_exit(&mut self) -> std::process::ExitStatus {
        self.child.wait().expect("wait for sim exit")
    }

    pub fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> (u16, serde_json::Value) {
        try_request(self.port, method, path, body).expect("sim reachable")
    }

    pub fn status(&self) -> serde_json::Value {
        let (status, reply) = self.request("GET", "/v1/status", None);
        assert_eq!(status, 200, "status failed: {reply}");
        reply
    }

    pub fn sim_state(&self) -> serde_json::Value {
        let (status, reply) = self.request("GET", "/sim/state", None);
        assert_eq!(status, 200, "sim state failed: {reply}");
        reply
    }

    pub fn step(&self) -> serde_json::Value {
        let (status, reply) = self.request("POST", "/sim/step", None);
        assert_eq!(status, 200, "step failed: {reply}");
        reply
    }

    pub fn set_auto(&self, enabled: bool) {
        let (status, reply) = self.request(
            "POST",
            "/sim/auto",
            Some(&serde_json::json!({ "enabled": enabled })),
        );
        assert_eq!(status, 200, "set auto failed: {reply}");
    }

    pub fn query(&self, target: &str, query: serde_json::Value) -> serde_json::Value {
        let (status, reply) = self.request(
            "POST",
            "/v1/query",
            Some(&serde_json::json!({ "target": target, "query": query })),
        );
        assert_eq!(status, 200, "query {target} failed: {reply}");
        reply
    }

    /// the canonical chat point read: the channel's committed record, or `None`
    /// when nothing is committed under that id.
    ///
    /// there is no list-all read to count any more, and that is the product's
    /// position, not a gap: the read-model cutover moved every UI-shaped
    /// iteration into the index guests (`POST /v1/index/chat/view`) and left
    /// canonical state answering point reads ALONE. so a "did this commit?"
    /// assertion has to name the channel it means — which is also the stronger
    /// claim, since a count of 2 never said WHICH two.
    pub fn channel(&self, channel_id: &str) -> Option<serde_json::Value> {
        let reply = self.query(
            "chat",
            serde_json::json!({ "channel": { "channel_id": channel_id } }),
        );
        match reply["channel"] {
            serde_json::Value::Null => None,
            _ => Some(reply["channel"].clone()),
        }
    }

    /// an inline submit — only sound in auto mode (or for an op the module
    /// rejects at once), where the reply does not wait on a step.
    pub fn submit(
        &self,
        target: &str,
        payload: serde_json::Value,
        origin: Option<&str>,
    ) -> (u16, serde_json::Value) {
        let mut body = serde_json::json!({ "target": target, "payload": payload });
        if let Some(origin) = origin {
            body["origin"] = serde_json::json!(origin);
        }
        try_request(self.port, "POST", "/v1/submit", Some(&body)).expect("submit reachable")
    }

    /// inline submit that must COMMIT — returns the receipt.
    pub fn submit_ok(
        &self,
        target: &str,
        payload: serde_json::Value,
        origin: Option<&str>,
    ) -> serde_json::Value {
        let (code, reply) = self.submit(target, payload, origin);
        assert_eq!(code, 200, "submit to {target} failed: {reply}");
        reply
    }

    /// inline submit that the module must REJECT — returns the error text.
    pub fn submit_rejected(
        &self,
        target: &str,
        payload: serde_json::Value,
        origin: Option<&str>,
    ) -> String {
        let (code, reply) = self.submit(target, payload, origin);
        assert_eq!(code, 400, "submit to {target} was not rejected: {reply}");
        reply["error"]
            .as_str()
            .unwrap_or_else(|| panic!("rejection carries no error text: {reply}"))
            .to_string()
    }

    /// spawn a submit on its own thread — in hold mode the http reply hangs
    /// until a step releases it, so the caller must not block on it inline.
    pub fn submit_in_background(
        &self,
        target: &str,
        payload: serde_json::Value,
        origin: Option<&str>,
    ) -> std::thread::JoinHandle<(u16, serde_json::Value)> {
        let port = self.port;
        let target = target.to_string();
        let mut body = serde_json::json!({ "target": target, "payload": payload });
        if let Some(origin) = origin {
            body["origin"] = serde_json::json!(origin);
        }
        std::thread::spawn(move || {
            try_request(port, "POST", "/v1/submit", Some(&body)).expect("held submit reachable")
        })
    }

    /// POST raw signed-frame bytes to `/v1/submit/frame` — the authenticated
    /// lane. the body is `application/octet-stream` (the exact bytes
    /// `node::encode_frame` produced), not json, so it needs its own sender.
    /// only sound in auto mode (the reply commits without a step).
    pub fn submit_frame(&self, frame: &[u8]) -> (u16, serde_json::Value) {
        post_raw(
            self.port,
            "/v1/submit/frame",
            "application/octet-stream",
            frame,
        )
        .expect("frame submit reachable")
    }

    /// a `/sim/peer-block` batch: N ops committed as ONE block. returns the
    /// (status, reply) so a test can assert per-member verdicts (the reply
    /// carries `members: [{applied|rejected}]`) and the single committed height.
    pub fn peer_batch(&self, ops: serde_json::Value) -> (u16, serde_json::Value) {
        self.request(
            "POST",
            "/sim/peer-block",
            Some(&serde_json::json!({ "ops": ops })),
        )
    }

    /// commit a concurrent writer's block, independent of the held queue.
    pub fn peer_block(
        &self,
        target: &str,
        payload: serde_json::Value,
        origin: &str,
    ) -> serde_json::Value {
        let (status, reply) = self.request(
            "POST",
            "/sim/peer-block",
            Some(&serde_json::json!({
                "target": target,
                "payload": payload,
                "origin": origin,
            })),
        );
        assert_eq!(status, 200, "peer block failed: {reply}");
        reply
    }

    /// poll /sim/state until `field` reaches `want` — the held queue is fed by
    /// another thread's in-flight request, so arrival is asynchronous.
    pub fn await_sim_state(&self, field: &str, want: u64) {
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

/// json request/response against the sim's /v1 + /sim wires — the shared raw
/// http/1.1 client, `io::Result` so `await_status` can poll a not-yet-up sim.
/// `embed.rs` drives the embedded server through this directly.
pub use nettest::try_http_json as try_request;

use nettest::free_port;

/// POST arbitrary body bytes with an explicit content-type — the raw-bytes
/// twin of [`try_request`] (which is json-only), for the octet-stream frame
/// lane.
fn post_raw(
    port: u16,
    path: &str,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<(u16, serde_json::Value)> {
    let (status, raw) = nettest::try_http_bytes(port, "POST", path, content_type, body)?;
    Ok((status, serde_json::from_slice(&raw).unwrap_or(serde_json::Value::Null)))
}

pub fn create_channel(channel: &str, name: &str) -> serde_json::Value {
    serde_json::json!({
        "create_channel": { "channel_id": channel, "name": name, "post_policy": "open" }
    })
}

pub fn post_message(channel: &str, message_id: &str, text: &str) -> serde_json::Value {
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

/// a `MemberAuth` JSON whose ed25519 `key` consents to `preimage` under the
/// identity bind namespace — the shared member-auth builder the bound and
/// governed scenarios reuse (identity binds, gateway routes, share adoption).
/// the ed25519 signing + `MemberAuth` shape now live ONCE in `identity::testkit`;
/// this wraps it back to the untyped JSON the sim's `/v1/submit` lane takes (the
/// serde shape is byte-identical to the hand-rolled json).
pub fn ed_bind_auth(
    key: &commonware_cryptography::ed25519::PrivateKey,
    preimage: &[u8],
) -> serde_json::Value {
    serde_json::to_value(identity::testkit::ed_bind_auth(key, preimage))
        .expect("MemberAuth serializes")
}
