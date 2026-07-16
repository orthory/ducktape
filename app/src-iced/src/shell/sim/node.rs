//! Spawn/control a real `ducktape-simnode` child for the sim lane.
//!
//! Copied-down spawn bits of `bin/simnode/tests/harness/mod.rs`; consolidation
//! into a shared crate is deferred until this branch converges with `dev`.

use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Once;
use std::time::{Duration, Instant};

pub(super) struct SimNode {
    child: Child,
    port: u16,
    _storage: tempfile::TempDir,
}

impl Drop for SimNode {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// `port`/`query` are the harness read API the later sim-lane tasks (shell
// harness + committed-state assertions) consume; this task lands them ahead of
// their first caller.
#[allow(dead_code)]
impl SimNode {
    /// Spawn a fresh `--auto` simnode on a free port with fresh storage.
    /// Returns None (after reporting loudly) when the child binaries are not
    /// built; panics instead under `DUCKTAPE_SIM_REQUIRE=1` so the Make gate
    /// can never pass vacuously.
    pub(super) fn spawn() -> Option<Self> {
        let Some(simnode) = resolve_bin("DUCKTAPE_SIMNODE_BIN", "ducktape-simnode") else {
            return skip("ducktape-simnode is not built");
        };
        let Some(node) = resolve_bin("DUCKTAPE_NODE_BIN", "ducktape-node") else {
            return skip("ducktape-node is not built (signing verbs need it)");
        };
        export_node_bin(&node);

        let storage = tempfile::tempdir().expect("sim storage dir");
        let port = free_port();
        let mut cmd = Command::new(simnode);
        cmd.arg("--listen")
            .arg(format!("127.0.0.1:{port}"))
            .arg("--storage")
            .arg(storage.path())
            .arg("--auto")
            .stdout(Stdio::null())
            // Startup failures land on stderr — keep it visible or they read
            // as an opaque readiness timeout.
            .stderr(Stdio::inherit());
        let child = cmd.spawn().expect("spawn ducktape-simnode");
        let mut sim = Self { child, port, _storage: storage };
        sim.await_status();
        Some(sim)
    }

    pub(super) fn port(&self) -> u16 {
        self.port
    }

    /// Chain-side read for committed-state assertions.
    pub(super) fn query(&self, target: &str, query: serde_json::Value) -> serde_json::Value {
        let (status, reply) = self.request(
            "POST",
            "/v1/query",
            Some(&serde_json::json!({ "target": target, "query": query })),
        );
        assert_eq!(status, 200, "query {target} failed: {reply}");
        reply
    }

    fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> (u16, serde_json::Value) {
        try_request(self.port, method, path, body).expect("sim reachable")
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
}

/// `DUCKTAPE_NODE_BIN` must be set process-wide: `Backend`'s verb runner
/// falls back to a sibling of the current exe, which under `cargo test` is
/// `target/debug/deps/` — not where cargo puts `ducktape-node`.
fn export_node_bin(node: &std::path::Path) {
    static EXPORT: Once = Once::new();
    let node = node.to_path_buf();
    EXPORT.call_once(move || {
        if std::env::var_os("DUCKTAPE_NODE_BIN").is_none() {
            // SAFETY: first harness boot; no backend verb thread exists yet
            // to race this write (edition 2024 makes set_var unsafe).
            unsafe { std::env::set_var("DUCKTAPE_NODE_BIN", &node) };
        }
    });
}

fn skip(reason: &str) -> Option<SimNode> {
    if std::env::var_os("DUCKTAPE_SIM_REQUIRE").is_some() {
        panic!("sim lane required (DUCKTAPE_SIM_REQUIRE) but {reason}");
    }
    eprintln!(
        "SKIP shell::sim — {reason}; run `CARGO_INCREMENTAL=0 cargo build -p simnode -p node-bin`"
    );
    None
}

fn resolve_bin(env_key: &str, file_name: &str) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(env_key) {
        let path = PathBuf::from(path);
        return path.exists().then_some(path);
    }
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target"));
    ["debug", "release"]
        .iter()
        .map(|profile| target.join(profile).join(file_name))
        .find(|path| path.exists())
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind a free port")
        .local_addr()
        .expect("local addr")
        .port()
}

fn try_request(
    port: u16,
    method: &str,
    path: &str,
    body: Option<&serde_json::Value>,
) -> std::io::Result<(u16, serde_json::Value)> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    stream.set_read_timeout(Some(Duration::from_secs(15)))?;
    let payload = body.map(|value| value.to_string()).unwrap_or_default();
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    stream.write_all(request.as_bytes())?;
    // Read raw bytes and lossy-decode, matching the wire-proven upstream
    // harness: `read_to_string` would turn any non-UTF8 reply byte into an IO
    // error, which the caller reports as an opaque "sim reachable" panic.
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw)?;
    let text = String::from_utf8_lossy(&raw);
    let status: u16 = text
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or(0);
    let json_body = text
        .split("\r\n\r\n")
        .nth(1)
        .map(str::trim)
        .filter(|body| !body.is_empty())
        .and_then(|body| serde_json::from_str(body).ok())
        .unwrap_or(serde_json::Value::Null);
    Ok((status, json_body))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The child spawns, answers /v1/status, and dies with the handle.
    #[test]
    fn spawn_answers_status() {
        let Some(sim) = SimNode::spawn() else { return };
        let (status, reply) = sim.request("GET", "/v1/status", None);
        assert_eq!(status, 200, "status: {reply}");
    }
}
