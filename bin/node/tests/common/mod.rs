//! process-level harness for the real-socket node e2e: spawns REAL
//! `ducktape` binaries (via `CARGO_BIN_EXE_ducktape`) with generated
//! toml configs, captures their stdout to files, polls for the node's
//! greppable markers, and speaks the json-lines rpc — the rust replacement for
//! what `demo-2node.sh` used to orchestrate in bash.
//!
//! shared by several test binaries, each using a different subset of the
//! helpers — the per-binary dead-code lint would otherwise flag whichever
//! helpers this particular binary skips.
#![allow(dead_code)]
//!
//! the constraints this harness encodes (they are invariants of the node, not
//! choices of the tests):
//! - every process needs a DISTINCT storage root (qmdb + the simplex journal
//!   persist; shared roots corrupt each other) — one tempdir per cluster.
//! - nothing supports port 0, so ports are pre-allocated by binding the whole
//!   batch simultaneously and then releasing it.
//! - the namespace is unique per cluster so a stale process from an earlier
//!   run can never handshake its way into this mesh.
//! - `converged root_hash=` latches after ANY `validator_seeds.len()` frames
//!   apply — it is a liveness marker, not proof of specific ops; state
//!   assertions go through rpc queries instead.

use std::io::{BufRead as _, BufReader, Write as _};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use commonware_cryptography::{Signer as _, ed25519};

/// the socket suite is heavyweight (4 OS processes each): serialize the tests
/// in this binary so two clusters never compete for one CI core budget.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

static CLUSTER_SEQ: AtomicU64 = AtomicU64::new(0);

/// a running node process. killed (and reaped) on drop so an assertion
/// failure never leaks a validator into the host system.
pub struct NodeProc {
    pub id: u64,
    child: Child,
    pub log: PathBuf,
}

impl Drop for NodeProc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// one e2e cluster: the shared config (mesh membership, ports, namespace,
/// storage tempdir) plus whichever node processes are currently running.
pub struct Cluster {
    pub namespace: String,
    pub peer_ids: Vec<u64>,
    pub validator_ids: Vec<u64>,
    /// p2p listen port per `peer_ids` position.
    pub p2p_ports: Vec<u16>,
    /// rpc port per `peer_ids` position (every config gets one; `--sync-only`
    /// simply never binds it).
    pub rpc_ports: Vec<u16>,
    /// http/ws app-surface port per `peer_ids` position (the noded wire
    /// contract served by the validator itself; off under `--sync-only`).
    pub http_ports: Vec<u16>,
    /// per-node `advertised` override (test-only), index-aligned with
    /// `peer_ids`. `Some(addr)` emits an `advertised = "<addr>"` line right
    /// after `listen` in the generated config (e.g. a sentry/forwarder in
    /// front of the node); `None` emits nothing — byte-for-byte the plain node.
    pub advertised: Vec<Option<String>>,
    /// override for the bootstrapper address every non-founder node dials.
    /// `None` -> `127.0.0.1:p2p_ports[0]` (identical to today); `Some(addr)`
    /// points bootstrap at a forwarder in front of node 0.
    pub bootstrap_addr_override: Option<String>,
    /// When true every config gets `wireguard_listen` on the node's distinct
    /// UDP port — the reachability plane runs the real, unprivileged
    /// userspace transport (the node's only backend).
    pub wireguard: bool,
    /// extra `node.toml` lines appended verbatim to EVERY node's generated
    /// config (`spawn` regenerates the file, so a hand-edit after the fact
    /// would not survive a respawn). set before the first spawn; empty by
    /// default so existing tests are byte-for-byte unchanged.
    pub extra_toml: Vec<String>,
    /// extra environment variables for node `idx`'s process, index-aligned
    /// with `peer_ids` (what gives each node its own capability-provider
    /// surface: `DUCKTAPE_CAPABILITY_DIR`, spec `detect.env` overrides).
    /// empty per node by default; set before spawn — a respawn re-applies.
    pub env: Vec<Vec<(String, String)>>,
    /// declared BEFORE `dir` so drop order kills + reaps every child first —
    /// removing the tempdir under live processes races their qmdb/journal
    /// writes and silently leaks the subtree.
    nodes: Vec<Option<NodeProc>>,
    dir: tempfile::TempDir,
}

/// a two-person network-shape ceremony: real `init`/`invite`/`join` verbs,
/// key files, network.toml descriptors, and node.toml configs.
pub struct NetworkShapeCluster {
    pub p2p_ports: Vec<u16>,
    pub rpc_ports: Vec<u16>,
    pub http_ports: Vec<u16>,
    pub founder_dir: PathBuf,
    pub friend_dir: PathBuf,
    /// extra process env per node (set before `spawn`) — the same knob
    /// [`Cluster::env`] exposes, e.g. capability spec-dir overrides.
    pub env: Vec<Vec<(String, String)>>,
    nodes: Vec<Option<NodeProc>>,
    dir: tempfile::TempDir,
}

impl NetworkShapeCluster {
    /// freeze (`SIGSTOP`) or thaw (`SIGCONT`) a running node — the closest a
    /// test gets to a laptop sleeping mid-run: the process vanishes from the
    /// scheduler while its kernel keeps its sockets ESTABLISHED, exactly the
    /// silent half-open shape a slept machine leaves its peers holding.
    pub fn signal(&self, idx: usize, signal: &str) {
        let node = self.nodes[idx].as_ref().expect("node not running");
        let pid = node.child.id();
        let status = Command::new("kill")
            .arg(format!("-{signal}"))
            .arg(pid.to_string())
            .status()
            .expect("run kill");
        assert!(status.success(), "kill -{signal} {pid} failed");
    }

    pub fn new() -> Self {
        let dir = tempfile::TempDir::new().expect("network-shape tempdir");
        let ports = alloc_ports(6);
        let (p2p_ports, rest) = ports.split_at(2);
        let (rpc_ports, http_ports) = rest.split_at(2);
        Self {
            p2p_ports: p2p_ports.to_vec(),
            rpc_ports: rpc_ports.to_vec(),
            http_ports: http_ports.to_vec(),
            founder_dir: dir.path().join("founder"),
            friend_dir: dir.path().join("friend"),
            env: vec![Vec::new(), Vec::new()],
            nodes: vec![None, None],
            dir,
        }
    }

    pub fn init_founder(&self, name: &str) -> String {
        // the join protocol refuses to mint an invite from a member with no reachability
        // plane, and this harness is deliberately coordinator-free — so every
        // founder carries a distinct-port WireGuard listen.
        let wg_listen = format!("127.0.0.1:{}", alloc_ports(1)[0]);
        let out = Command::new(env!("CARGO_BIN_EXE_ducktape")).arg("node")
            .args([
                "init",
                "--name",
                name,
                // hermetic: no ambient coordinator. the default would dial the
                // LIVE public coordinator from inside the test AND flip both
                // nodes into the overlay shape (wireguard on a shared port +
                // ULA-advertised mesh) — same-host nodes then fight over one
                // interface and the underlay never assembles. the coordinated
                // shape has its own e2e (coordinated_invite_cli).
                "--primary-coordinator",
                "none",
                "--dir",
                self.founder_dir.to_str().expect("utf-8 founder dir"),
                "--listen",
                &format!("127.0.0.1:{}", self.p2p_ports[0]),
                "--advertised",
                &format!("127.0.0.1:{}", self.p2p_ports[0]),
                "--http",
                &format!("127.0.0.1:{}", self.http_ports[0]),
                "--rpc",
                &format!("127.0.0.1:{}", self.rpc_ports[0]),
            ])
            .args(["--wireguard-listen", &wg_listen])
            .output()
            .expect("run init");
        assert!(
            out.status.success(),
            "init failed:\n{}",
            command_output(&out)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// mint (or reuse) the friend workspace's identity via the `node key` verb
    /// and return its pubkey hex — the JOIN CODE the invite locks to.
    /// `join_friend` reuses this pre-generated identity.
    pub fn keygen_friend(&self, _idx: usize) -> String {
        let out = Command::new(env!("CARGO_BIN_EXE_ducktape")).arg("node")
            .args(["key", "--dir"])
            .arg(&self.friend_dir)
            .output()
            .expect("run node key");
        assert!(
            out.status.success(),
            "keygen failed:\n{}",
            command_output(&out)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// mint a bearer invite (the join protocol: single-use, sealed to whoever redeems it
    /// at first contact). Still pre-generates the friend identity so
    /// `join_friend` reuses one stable key across the ceremony.
    pub fn invite(&self) -> String {
        self.keygen_friend(1);
        let cfg = self.config_file(0);
        let out = Command::new(env!("CARGO_BIN_EXE_ducktape")).arg("node")
            .args(["invite", "--config"])
            .arg(cfg)
            .output()
            .expect("run invite");
        assert!(
            out.status.success(),
            "invite failed:\n{}",
            command_output(&out)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// a MANUAL-flow join: every invite is tokened now (mint is the
    /// admission), so the manual path is made by joining normally and then
    /// dropping the stored credential — the node parks with no announce and
    /// admission stays a member verb, which is exactly what the staged
    /// admission tests exercise.
    pub fn join_friend_manual(&self, invite: &str) -> String {
        let key = self.join_friend(invite);
        for stale in ["invite.token", "invite-wireguard.toml"] {
            let _ = std::fs::remove_file(self.friend_dir.join(stale));
        }
        key
    }

    /// the founder's verified join-request queue, parsed from the
    /// `join requests` verb's JSON stdout.
    pub fn join_requests(&self) -> serde_json::Value {
        let cfg = self.config_file(0);
        let out = Command::new(env!("CARGO_BIN_EXE_ducktape")).arg("node")
            .args(["join", "requests", "--config"])
            .arg(cfg)
            .output()
            .expect("run node join requests");
        assert!(
            out.status.success(),
            "join requests failed:\n{}",
            command_output(&out)
        );
        serde_json::from_str(String::from_utf8_lossy(&out.stdout).trim())
            .expect("join requests prints json")
    }

    /// run the `join` verb against the friend workspace WITHOUT asserting
    /// success — the caller inspects the outcome (a targeted invite refuses a
    /// mismatched local identity at the CLI, before any node spawns).
    pub fn try_join_friend(&self, invite: &str) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_ducktape")).arg("node")
            .args([
                "join",
                invite,
                "--dir",
                self.friend_dir.to_str().expect("utf-8 friend dir"),
                "--listen",
                &format!("127.0.0.1:{}", self.p2p_ports[1]),
                "--advertised",
                &format!("127.0.0.1:{}", self.p2p_ports[1]),
                "--http",
                &format!("127.0.0.1:{}", self.http_ports[1]),
                "--rpc",
                &format!("127.0.0.1:{}", self.rpc_ports[1]),
                "--wireguard-listen",
                &format!("127.0.0.1:{}", alloc_ports(1)[0]),
                // hermetic: without this the joined node registers with the
                // LIVE public coordinator from inside the test.
                "--primary-coordinator",
                "none",
            ])
            .output()
            .expect("run join")
    }

    pub fn join_friend(&self, invite: &str) -> String {
        let out = self.try_join_friend(invite);
        assert!(
            out.status.success(),
            "join failed:\n{}",
            command_output(&out)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    pub fn config_file(&self, idx: usize) -> PathBuf {
        match idx {
            0 => self.founder_dir.join("node.toml"),
            1 => self.friend_dir.join("node.toml"),
            _ => panic!("unknown network-shape node idx {idx}"),
        }
    }

    pub fn spawn(&mut self, idx: usize) {
        let cfg = self.config_file(idx);
        let label = match idx {
            0 => "founder",
            1 => "friend",
            _ => panic!("unknown network-shape node idx {idx}"),
        };
        let log = self.dir.path().join(format!("{label}.log"));
        let out = std::fs::File::create(&log).expect("create node log");
        let err = out.try_clone().expect("clone node log handle");
        let child = Command::new(env!("CARGO_BIN_EXE_ducktape")).arg("node")
            .arg("run")
            .arg("--config")
            .arg(&cfg)
            .envs(self.env[idx].iter().map(|(k, v)| (k.clone(), v.clone())))
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(err))
            .spawn()
            .expect("spawn network-shape node");
        self.nodes[idx] = Some(NodeProc {
            id: idx as u64,
            child,
            log,
        });
    }

    /// kill node `idx`'s process (reaped by NodeProc's drop).
    pub fn kill(&mut self, idx: usize) {
        self.nodes[idx] = None;
    }

    /// node `idx`'s captured stdout+stderr — for a failing test to preserve
    /// evidence before the cluster tempdir (and the logs in it) is dropped.
    pub fn log_path(&self, idx: usize) -> PathBuf {
        self.nodes[idx].as_ref().expect("node not running").log.clone()
    }

    /// one json-lines rpc against node `idx` — the NetworkShapeCluster
    /// mirror of [`Cluster::rpc`] (same wire, same ports array).
    pub fn rpc(&self, idx: usize, req: serde_json::Value) -> serde_json::Value {
        let port = self.rpc_ports[idx];
        let mut line = serde_json::to_string(&req).expect("rpc request serializes");
        line.push('\n');
        let retryable = req["cmd"] == "query";
        let deadline = Instant::now() + Duration::from_secs(90);
        loop {
            // the listener is up before the pump — connecting retries for
            // every cmd (nothing has been sent yet, so retrying is safe).
            let connect_deadline = Instant::now() + Duration::from_secs(30);
            let stream = loop {
                match TcpStream::connect(("127.0.0.1", port)) {
                    Ok(s) => break s,
                    Err(e) => {
                        assert!(
                            Instant::now() < connect_deadline,
                            "rpc connect to node idx {idx} (port {port}) failed: {e};\n{}",
                            self.all_log_tails(40)
                        );
                        std::thread::sleep(Duration::from_millis(200));
                    }
                }
            };
            let mut stream = stream;
            stream
                .set_read_timeout(Some(Duration::from_secs(if retryable { 15 } else { 60 })))
                .expect("rpc read timeout");
            stream.write_all(line.as_bytes()).expect("rpc write");
            let mut reply = String::new();
            match BufReader::new(stream).read_line(&mut reply) {
                Ok(n) if n > 0 => {
                    return serde_json::from_str(reply.trim()).expect("rpc reply is json");
                }
                res => {
                    // a query is idempotent: reconnect and resend across the
                    // windows where the listener answers connects but the node
                    // cannot reply yet (a promotion reboot, an epoch-cutover
                    // engine restart). a submit is NEVER resent — a duplicate
                    // could double-apply — it gets one long read window above.
                    let why = match res {
                        Ok(_) => "connection closed before a reply line".to_string(),
                        Err(e) => e.to_string(),
                    };
                    if !retryable || Instant::now() >= deadline {
                        panic!(
                            "rpc to node idx {idx} (port {port}) failed: {why}; request: {req};\n{}",
                            self.all_log_tails(60)
                        );
                    }
                    std::thread::sleep(Duration::from_millis(300));
                }
            }
        }
    }

    pub fn query(&self, idx: usize, target: &str, req: &[u8]) -> Option<Vec<u8>> {
        let reply = self.rpc(
            idx,
            serde_json::json!({
                "cmd": "query",
                "target": target,
                "req_hex": hex(req),
            }),
        );
        if reply["ok"] != true {
            return None;
        }
        Some(unhex(
            reply["reply_hex"]
                .as_str()
                .expect("query reply carries hex"),
        ))
    }

    pub fn submit(&self, idx: usize, target: &str, payload: &[u8]) {
        let reply = self.rpc(
            idx,
            serde_json::json!({
                "cmd": "submit",
                "target": target,
                "payload_hex": hex(payload),
            }),
        );
        assert_eq!(
            reply["ok"], true,
            "submit to {target} via node idx {idx} rejected: {reply}"
        );
    }

    pub fn status(&self, idx: usize) -> serde_json::Value {
        let reply = self.rpc(idx, serde_json::json!({ "cmd": "status" }));
        assert_eq!(
            reply["ok"], true,
            "status via node idx {idx} failed: {reply}"
        );
        reply["status"].clone()
    }

    /// drive a membership ceremony verb (`member promote`, `resident accept`,
    /// `resident remove`) against node 0's running rpc, from node 0's config.
    /// `verb` is the space-separated two-token spelling; it is split into argv.
    pub fn run_membership_verb(&self, verb: &str, pubkey_hex: &str) -> (bool, String) {
        let cfg = self.config_file(0);
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_ducktape"));
        cmd.arg("node");
        for token in verb.split(' ') {
            cmd.arg(token);
        }
        let out = cmd
            .args([pubkey_hex, "--config"])
            .arg(cfg)
            .output()
            .unwrap_or_else(|e| panic!("run {verb}: {e}"));
        (out.status.success(), command_output(&out))
    }

    /// drive the DIRECT admission ceremony (`member promote` — the pre-staged
    /// `resident accept` semantics) from node 0's config.
    pub fn run_promote(&self, pubkey_hex: &str) -> (bool, String) {
        self.run_membership_verb("member promote", pubkey_hex)
    }

    pub fn wait_marker(&mut self, idx: usize, marker: &str, timeout: Duration) -> String {
        let deadline = Instant::now() + timeout;
        loop {
            let node = self.nodes[idx].as_mut().expect("node is running");
            let text = std::fs::read_to_string(&node.log).unwrap_or_default();
            if let Some(rest) = find_marker(&text, marker) {
                return rest;
            }
            let exited = node.child.try_wait().expect("poll node").is_some();
            if exited || Instant::now() >= deadline {
                let verb = if exited { "exited" } else { "timed out" };
                panic!(
                    "network-shape node idx {idx} {verb} without printing {marker:?};\n{}",
                    self.all_log_tails(60),
                );
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    }

    /// wait until node `idx` has COMMITTED STANDING as a member. the join protocol has
    /// two legitimate admission paths and which one lands first is a race:
    /// direct first contact prints "standing is committed" (replica/wiring),
    /// the announce-redeem park path prints "resident: standing granted".
    /// Waiting on either is the semantic event the resident tests gate on.
    pub fn wait_admitted(&mut self, idx: usize, timeout: Duration) {
        let markers = ["standing is committed", "resident: standing granted"];
        let deadline = Instant::now() + timeout;
        loop {
            let node = self.nodes[idx].as_mut().expect("node is running");
            let text = std::fs::read_to_string(&node.log).unwrap_or_default();
            if markers.iter().any(|m| find_marker(&text, m).is_some()) {
                return;
            }
            let exited = node.child.try_wait().expect("poll node").is_some();
            if exited || Instant::now() >= deadline {
                let verb = if exited { "exited" } else { "timed out" };
                panic!(
                    "network-shape node idx {idx} {verb} without printing any of \
                     {markers:?};\n{}",
                    self.all_log_tails(60),
                );
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    }

    /// wait for node `idx` to exit ON ITS OWN (e.g. the fail-loud FATAL path)
    /// and reap it — the [`Cluster::wait_exit`] mirror.
    pub fn wait_exit(&mut self, idx: usize, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        loop {
            let node = self.nodes[idx].as_mut().expect("node is running");
            if node.child.try_wait().expect("poll node").is_some() {
                self.nodes[idx] = None;
                return;
            }
            assert!(
                Instant::now() < deadline,
                "network-shape node idx {idx} did not exit within {timeout:?};\n{}",
                self.all_log_tails(40)
            );
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn all_log_tails(&self, lines: usize) -> String {
        self.nodes
            .iter()
            .enumerate()
            .filter_map(|(idx, n)| {
                n.as_ref().map(|n| {
                    let text = std::fs::read_to_string(&n.log).unwrap_or_default();
                    format!(
                        "--- network-shape node idx {idx} log tail ---\n{}",
                        log_tail(&text, lines)
                    )
                })
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl Cluster {
    /// lay out a cluster: `peer_ids` is the full authorized mesh (index 0 is
    /// the bootstrapper), `validator_ids` the consensus subset.
    pub fn new(peer_ids: &[u64], validator_ids: &[u64]) -> Self {
        let seq = CLUSTER_SEQ.fetch_add(1, Ordering::Relaxed);
        let namespace = format!("ducktape-e2e-{}-{seq}", std::process::id());
        let dir = tempfile::TempDir::new().expect("cluster tempdir");
        let ports = alloc_ports(peer_ids.len() * 3);
        let (p2p_ports, rest) = ports.split_at(peer_ids.len());
        let (rpc_ports, http_ports) = rest.split_at(peer_ids.len());
        Self {
            namespace,
            peer_ids: peer_ids.to_vec(),
            validator_ids: validator_ids.to_vec(),
            p2p_ports: p2p_ports.to_vec(),
            rpc_ports: rpc_ports.to_vec(),
            http_ports: http_ports.to_vec(),
            advertised: peer_ids.iter().map(|_| None).collect(),
            bootstrap_addr_override: None,
            wireguard: false,
            extra_toml: Vec::new(),
            env: peer_ids.iter().map(|_| Vec::new()).collect(),
            dir,
            nodes: peer_ids.iter().map(|_| None).collect(),
        }
    }

    /// the deterministic dev identity for a seed — what the node itself derives.
    pub fn identity(seed: u64) -> Vec<u8> {
        ed25519::PrivateKey::from_seed(seed)
            .public_key()
            .as_ref()
            .to_vec()
    }

    fn config_path(&self, idx: usize) -> PathBuf {
        let id = self.peer_ids[idx];
        let path = self.dir.path().join(format!("node{id}.toml"));
        let mut cfg = String::new();
        cfg.push_str(&format!("id = {id}\n"));
        cfg.push_str(&format!("listen = \"127.0.0.1:{}\"\n", self.p2p_ports[idx]));
        if let Some(addr) = &self.advertised[idx] {
            cfg.push_str(&format!("advertised = {addr:?}\n"));
        }
        cfg.push_str(&format!("namespace = {:?}\n", self.namespace));
        cfg.push_str(&format!("peer_seeds = {:?}\n", self.peer_ids));
        cfg.push_str(&format!("validator_seeds = {:?}\n", self.validator_ids));
        if idx != 0 {
            cfg.push_str(&format!("bootstrapper_addr = \"{}\"\n", self.bootstrap_addr()));
        }
        cfg.push_str(&format!(
            "storage_dir = {:?}\n",
            self.dir
                .path()
                .join(format!("storage-{id}"))
                .to_str()
                .unwrap()
        ));
        cfg.push_str(&format!(
            "rpc_listen = \"127.0.0.1:{}\"\n",
            self.rpc_ports[idx]
        ));
        cfg.push_str(&format!(
            "http_listen = \"127.0.0.1:{}\"\n",
            self.http_ports[idx]
        ));
        if self.wireguard {
            cfg.push_str(&format!(
                "wireguard_listen = \"127.0.0.1:{}\"\n",
                self.p2p_ports[idx]
            ));
        }
        for line in &self.extra_toml {
            cfg.push_str(line);
            cfg.push('\n');
        }
        std::fs::write(&path, cfg).expect("write node config");
        path
    }

    /// spawn the node at `idx` as a validator/mesh member, stdout+stderr to a
    /// log file. does not wait for readiness — pair with [`Self::wait_marker`].
    pub fn spawn(&mut self, idx: usize) {
        let id = self.peer_ids[idx];
        let cfg = self.config_path(idx);
        let log = self.dir.path().join(format!("node{id}.log"));
        let out = std::fs::File::create(&log).expect("create node log");
        let err = out.try_clone().expect("clone node log handle");
        let child = Command::new(env!("CARGO_BIN_EXE_ducktape")).arg("node")
            .arg("run")
            .arg("--config")
            .arg(&cfg)
            .envs(self.env[idx].iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(err))
            .spawn()
            .expect("spawn ducktape");
        self.nodes[idx] = Some(NodeProc { id, child, log });
    }

    /// kill the node at `idx` (crash-fault injection) and reap it.
    pub fn kill(&mut self, idx: usize) {
        self.nodes[idx] = None; // NodeProc::drop kills + waits
    }

    /// remove node `idx`'s storage directory — a killed slot reused as a
    /// FRESH resident (the sync-only rebuild) must not inherit the previous
    /// occupant's state or index locks.
    pub fn wipe_storage(&self, idx: usize) {
        let id = self.peer_ids[idx];
        let _ = std::fs::remove_dir_all(self.dir.path().join(format!("storage-{id}")));
    }

    /// send SIGTERM to node `idx` WITHOUT reaping it — the graceful-quit fault
    /// the desktop shell injects when it SIGTERMs the daemon on app quit. the
    /// node's own signal arm should run its final checkpoint and exit 0; the
    /// caller then reaps via [`Self::wait_exit`]. dependency-free: shells out to
    /// `kill(1)` on the child pid rather than pulling in libc/nix for one call.
    pub fn term(&self, idx: usize) {
        let node = self.nodes[idx].as_ref().expect("node is running");
        let pid = node.child.id();
        let status = Command::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .status()
            .expect("send SIGTERM");
        assert!(status.success(), "kill -TERM {pid} failed");
    }

    /// the deterministic path of node `idx`'s config file (written by a prior
    /// [`Self::spawn`] / [`Self::spawn_joiner`]) — for verbs that read it.
    pub fn config_file(&self, idx: usize) -> PathBuf {
        self.dir
            .path()
            .join(format!("node{}.toml", self.peer_ids[idx]))
    }

    /// Node-local workspace used by dev-shape operational commands. In this
    /// shape it is the same directory as `storage_dir`.
    pub fn workspace(&self, idx: usize) -> PathBuf {
        self.dir
            .path()
            .join(format!("storage-{}", self.peer_ids[idx]))
    }

    /// the bootstrapper address every non-founder / joiner dials: the override
    /// when set (e.g. a sentry/forwarder in front of node 0), else node 0's own
    /// p2p port — today's default behavior.
    fn bootstrap_addr(&self) -> String {
        self.bootstrap_addr_override
            .clone()
            .unwrap_or_else(|| format!("127.0.0.1:{}", self.p2p_ports[0]))
    }

    /// spawn an UNINVITED joiner: identity seed `id`, deliberately absent
    /// from every existing member's `peer_seeds` — the mesh refuses it until
    /// governance admits it and the epoch cutover re-tracks. its own config
    /// lists the CURRENT members as mesh + validators (the invite descriptor
    /// a real joiner receives). rpc/http ports are allocated so the node can
    /// be driven after it promotes itself. call this AFTER every member
    /// spawn — it appends to the cluster index space.
    ///
    /// returns the joiner's cluster index.
    pub fn spawn_joiner(&mut self, id: u64) -> usize {
        let ports = alloc_ports(3);
        let path = self.dir.path().join(format!("node{id}.toml"));
        let mut cfg = String::new();
        cfg.push_str(&format!("id = {id}\n"));
        cfg.push_str(&format!("listen = \"127.0.0.1:{}\"\n", ports[0]));
        cfg.push_str(&format!("namespace = {:?}\n", self.namespace));
        cfg.push_str(&format!("peer_seeds = {:?}\n", self.peer_ids));
        cfg.push_str(&format!("validator_seeds = {:?}\n", self.validator_ids));
        cfg.push_str(&format!("bootstrapper_addr = \"{}\"\n", self.bootstrap_addr()));
        cfg.push_str(&format!(
            "storage_dir = {:?}\n",
            self.dir
                .path()
                .join(format!("storage-{id}"))
                .to_str()
                .unwrap()
        ));
        cfg.push_str(&format!("rpc_listen = \"127.0.0.1:{}\"\n", ports[1]));
        cfg.push_str(&format!("http_listen = \"127.0.0.1:{}\"\n", ports[2]));
        std::fs::write(&path, cfg).expect("write joiner config");

        let log = self.dir.path().join(format!("node{id}.log"));
        let out = std::fs::File::create(&log).expect("create joiner log");
        let err = out.try_clone().expect("clone joiner log handle");
        let child = Command::new(env!("CARGO_BIN_EXE_ducktape")).arg("node")
            .arg("run")
            .arg("--config")
            .arg(&path)
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(err))
            .spawn()
            .expect("spawn joiner node");

        self.peer_ids.push(id);
        self.p2p_ports.push(ports[0]);
        self.rpc_ports.push(ports[1]);
        self.http_ports.push(ports[2]);
        // keep `advertised`/`env` index-aligned with the extended index space
        // so a later `config_path(joiner_idx)` / `spawn` never panics.
        self.advertised.push(None);
        self.env.push(Vec::new());
        self.nodes.push(Some(NodeProc { id, child, log }));
        self.peer_ids.len() - 1
    }

    /// run a ducktape VERB (resident accept, admit, ...) to completion and
    /// return (success, combined output).
    pub fn run_verb(&self, args: &[&str]) -> (bool, String) {
        let out = Command::new(env!("CARGO_BIN_EXE_ducktape"))
            .args(args)
            .output()
            .expect("run ducktape verb");
        (
            out.status.success(),
            format!(
                "{}\n{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ),
        )
    }

    /// wait for node `idx` to exit ON ITS OWN (a graceful shutdown path) and
    /// reap it — the counterpart of [`Self::kill`] for restart tests.
    pub fn wait_exit(&mut self, idx: usize, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        loop {
            let node = self.nodes[idx].as_mut().expect("node is running");
            if node.child.try_wait().expect("poll node").is_some() {
                self.nodes[idx] = None;
                return;
            }
            assert!(
                Instant::now() < deadline,
                "node idx {idx} did not exit within {timeout:?};\n{}",
                self.all_log_tails(40)
            );
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// run the node at `idx` with `--sync-only` to completion and return
    /// (success, full log). the sync path exits on its own — 0 with a
    /// `synced root_hash=` line on parity, 1 on any mismatch.
    pub fn run_sync_only(&mut self, idx: usize, timeout: Duration) -> (bool, String) {
        // this port was reserved minutes ago (cluster layout) and nothing held
        // it since — by joiner time another process may own it. re-allocate
        // fresh: the listen addr lives only in THIS node's config (peers know
        // the joiner by identity, not address).
        self.p2p_ports[idx] = alloc_ports(1)[0];
        let id = self.peer_ids[idx];
        let cfg = self.config_path(idx);
        let log = self.dir.path().join(format!("node{id}-sync.log"));
        let out = std::fs::File::create(&log).expect("create joiner log");
        let err = out.try_clone().expect("clone joiner log handle");
        let mut child = Command::new(env!("CARGO_BIN_EXE_ducktape")).arg("node")
            .arg("run")
            .arg("--config")
            .arg(&cfg)
            .arg("--sync-only")
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(err))
            .spawn()
            .expect("spawn sync-only joiner");
        let deadline = Instant::now() + timeout;
        let status = loop {
            match child.try_wait().expect("poll joiner") {
                Some(status) => break Some(status),
                None if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
                None => std::thread::sleep(Duration::from_millis(200)),
            }
        };
        let text = std::fs::read_to_string(&log).unwrap_or_default();
        match status {
            Some(s) => (s.success(), text),
            None => (false, format!("JOINER TIMED OUT after {timeout:?}\n{text}")),
        }
    }

    /// non-blocking probe: has node `idx` printed `marker` yet? returns the
    /// rest of the first matching line (trimmed — for `foo=` markers that is
    /// the value).
    pub fn marker(&self, idx: usize, marker: &str) -> Option<String> {
        let node = self.nodes[idx].as_ref().expect("node is running");
        let text = std::fs::read_to_string(&node.log).unwrap_or_default();
        find_marker(&text, marker)
    }

    /// poll node `idx`'s log until a line contains `marker`, returning the
    /// rest of that line. fails fast if the process exits without printing it.
    pub fn wait_marker(&mut self, idx: usize, marker: &str, timeout: Duration) -> String {
        let deadline = Instant::now() + timeout;
        loop {
            let node = self.nodes[idx].as_mut().expect("node is running");
            let text = std::fs::read_to_string(&node.log).unwrap_or_default();
            if let Some(rest) = find_marker(&text, marker) {
                return rest;
            }
            let exited = node.child.try_wait().expect("poll node").is_some();
            if exited || Instant::now() >= deadline {
                let id = node.id;
                let verb = if exited { "exited" } else { "timed out" };
                panic!(
                    "node #{id} {verb} without printing {marker:?};\n{}",
                    self.all_log_tails(60),
                );
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    }

    /// one json-lines rpc round-trip against node `idx`, with connect retries
    /// (the listener is up before the pump — early calls may still race the
    /// process start).
    pub fn rpc(&self, idx: usize, req: serde_json::Value) -> serde_json::Value {
        let port = self.rpc_ports[idx];
        let mut line = serde_json::to_string(&req).expect("rpc request serializes");
        line.push('\n');
        let retryable = req["cmd"] == "query";
        let deadline = Instant::now() + Duration::from_secs(90);
        loop {
            // the listener is up before the pump — connecting retries for
            // every cmd (nothing has been sent yet, so retrying is safe).
            let connect_deadline = Instant::now() + Duration::from_secs(30);
            let stream = loop {
                match TcpStream::connect(("127.0.0.1", port)) {
                    Ok(s) => break s,
                    Err(e) => {
                        assert!(
                            Instant::now() < connect_deadline,
                            "rpc connect to node idx {idx} (port {port}) failed: {e};\n{}",
                            self.all_log_tails(40)
                        );
                        std::thread::sleep(Duration::from_millis(200));
                    }
                }
            };
            let mut stream = stream;
            stream
                .set_read_timeout(Some(Duration::from_secs(if retryable { 15 } else { 60 })))
                .expect("rpc read timeout");
            stream.write_all(line.as_bytes()).expect("rpc write");
            let mut reply = String::new();
            match BufReader::new(stream).read_line(&mut reply) {
                Ok(n) if n > 0 => {
                    return serde_json::from_str(reply.trim()).expect("rpc reply is json");
                }
                res => {
                    // a query is idempotent: reconnect and resend across the
                    // windows where the listener answers connects but the node
                    // cannot reply yet (a promotion reboot, an epoch-cutover
                    // engine restart). a submit is NEVER resent — a duplicate
                    // could double-apply — it gets one long read window above.
                    let why = match res {
                        Ok(_) => "connection closed before a reply line".to_string(),
                        Err(e) => e.to_string(),
                    };
                    if !retryable || Instant::now() >= deadline {
                        panic!(
                            "rpc to node idx {idx} (port {port}) failed: {why}; request: {req};\n{}",
                            self.all_log_tails(60)
                        );
                    }
                    std::thread::sleep(Duration::from_millis(300));
                }
            }
        }
    }

    /// submit an op via node `idx`'s rpc and assert the lane accepted it
    /// (accepted != finalized — follow with a query poll).
    pub fn submit(&self, idx: usize, target: &str, payload: &[u8]) {
        let reply = self.try_submit(idx, target, payload);
        assert_eq!(
            reply["ok"], true,
            "submit to {target} via node idx {idx} rejected: {reply}"
        );
    }

    /// submit an op via node `idx`'s rpc and return the raw reply — for tests
    /// that assert a submit is REJECTED (cleanly, with the node still live).
    pub fn try_submit(&self, idx: usize, target: &str, payload: &[u8]) -> serde_json::Value {
        self.rpc(
            idx,
            serde_json::json!({
                "cmd": "submit",
                "target": target,
                "payload_hex": hex(payload),
            }),
        )
    }

    /// query a module through node `idx`'s rpc. `None` on a module error —
    /// polls legitimately race finalization (e.g. an unknown channel BEFORE
    /// the create op applies), so rejection is "not yet", not a harness bug.
    pub fn query(&self, idx: usize, target: &str, req: &[u8]) -> Option<Vec<u8>> {
        let reply = self.rpc(
            idx,
            serde_json::json!({
                "cmd": "query",
                "target": target,
                "req_hex": hex(req),
            }),
        );
        if reply["ok"] != true {
            return None;
        }
        Some(unhex(
            reply["reply_hex"]
                .as_str()
                .expect("query reply carries hex"),
        ))
    }

    /// one request against node `idx`'s http/ws APP SURFACE (the noded wire
    /// contract the validator now serves itself) — raw http/1.1 over std TCP,
    /// returning (status, json body). the surface trusts localhost callers,
    /// so a hand-rolled client is a full citizen by design.
    pub fn http(
        &self,
        idx: usize,
        method: &str,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> (u16, serde_json::Value) {
        http_request(self.http_ports[idx], method, path, body)
    }

    /// GET a raw TEXT body from node `idx`'s app surface — for non-json
    /// responses like the Prometheus `/metrics` exposition, which the
    /// json-parsing [`Self::http`] twin would flatten to `Null`.
    pub fn http_text(&self, idx: usize, path: &str) -> (u16, String) {
        http_text_request(self.http_ports[idx], path)
    }

    /// the http base url of node `idx`'s app surface — what a `duckfs-client`
    /// `HttpNode` (or any plain http client) dials.
    pub fn http_base(&self, idx: usize) -> String {
        format!("http://127.0.0.1:{}", self.http_ports[idx])
    }

    /// every running node's log tail — the panic payload that makes a stalled
    /// mesh diagnosable from a CI failure alone.
    pub fn all_log_tails(&self, lines: usize) -> String {
        self.nodes
            .iter()
            .flatten()
            .map(|n| {
                let text = std::fs::read_to_string(&n.log).unwrap_or_default();
                format!(
                    "--- node #{} log tail ---\n{}",
                    n.id,
                    log_tail(&text, lines)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// the node's status projection (height, root_hash, module roots).
    pub fn status(&self, idx: usize) -> serde_json::Value {
        let reply = self.rpc(idx, serde_json::json!({ "cmd": "status" }));
        assert_eq!(
            reply["ok"], true,
            "status via node idx {idx} failed: {reply}"
        );
        reply["status"].clone()
    }
}

fn command_output(out: &std::process::Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// one request against an http/ws APP SURFACE port (the noded wire contract)
/// — raw http/1.1 over std TCP, returning (status, json body). the port-keyed
/// twin of [`Cluster::http`], for harnesses that hold ports without a
/// `Cluster` (e.g. [`NetworkShapeCluster`]). the surface trusts localhost
/// callers, so a hand-rolled client is a full citizen by design.
pub fn http_request(
    port: u16,
    method: &str,
    path: &str,
    body: Option<&serde_json::Value>,
) -> (u16, serde_json::Value) {
    nettest::http_json(port, method, path, body)
}

/// GET a raw TEXT body from an app-surface port — the non-json twin of
/// [`http_request`], for bodies like the Prometheus `/metrics` exposition.
pub fn http_text_request(port: u16, path: &str) -> (u16, String) {
    nettest::http_text(port, "GET", path)
}

#[allow(unused_imports)] // a shared prelude: not every e2e binary polls
pub use nettest::poll_until;

// `n` distinct free localhost ports, collision-safe (holds every listener at
// once — sequential bind-drop could hand the same port back twice).
use nettest::alloc_ports;

fn find_marker(text: &str, marker: &str) -> Option<String> {
    text.lines().find_map(|line| {
        line.find(marker)
            .map(|at| line[at + marker.len()..].trim().to_string())
    })
}

fn log_tail(text: &str, lines: usize) -> String {
    let all: Vec<&str> = text.lines().collect();
    let from = all.len().saturating_sub(lines);
    all[from..].join("\n")
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("valid hex"))
        .collect()
}
