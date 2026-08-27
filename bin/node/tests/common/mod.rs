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

/// A cluster's storage root, named so an ABANDONED one can be found and swept.
///
/// `tempfile::TempDir` removes itself on Drop, which covers a normal finish AND
/// a panicking test — the unwind still runs Drop, and a clean `cluster_e2e` run
/// measurably leaks nothing. What it does not cover is the process being KILLED:
/// a CI timeout, a Ctrl-C, an OOM. Then nothing unwinds and the whole storage
/// subtree survives.
///
/// That is not a nuisance where `TMPDIR` is tmpfs — it is RAM, and this box has
/// no swap. One session left **22 GB across 11 dirs**, two of them 7.2 GB, and
/// the machine only got slower until the compiler started dying mid-build.
///
/// Worse, `tempfile`'s names are random (`.tmpXXXXXX`), so a ducktape leak was
/// indistinguishable from any other program's temp dir — reclaiming it meant
/// `rm -rf /tmp/.tmp*`, a blunt instrument pointed at everyone's data.
///
/// Naming them `ducktape-e2e-<pid>-…` fixes both halves: the leak is
/// identifiable, and each new cluster first sweeps the ones whose owning
/// process is gone. A LIVE pid is never touched (sibling test binaries run
/// concurrently), and pid reuse only makes the sweep skip a directory — it can
/// never make it delete a live one.
fn e2e_tempdir(tag: &str) -> tempfile::TempDir {
    sweep_abandoned_e2e_dirs();
    tempfile::Builder::new()
        .prefix(&format!("ducktape-e2e-{}-{tag}-", std::process::id()))
        .tempdir()
        .expect("e2e tempdir")
}

/// Remove `ducktape-e2e-<pid>-*` roots whose owning process is gone.
fn sweep_abandoned_e2e_dirs() {
    let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(rest) = name.to_str().and_then(|n| n.strip_prefix("ducktape-e2e-")) else {
            continue;
        };
        let Some(pid) = rest.split('-').next().and_then(|p| p.parse::<i32>().ok()) else {
            continue;
        };
        if pid_is_alive(pid) {
            continue;
        }
        let _ = std::fs::remove_dir_all(entry.path());
    }
}

/// Signal 0 asks "may I signal this pid" WITHOUT delivering anything — the
/// portable liveness probe. Anything other than "no such process" is treated as
/// ALIVE, so an unreadable pid is skipped rather than swept.
fn pid_is_alive(pid: i32) -> bool {
    // SAFETY: `kill` with signal 0 delivers nothing and touches no memory.
    if unsafe { libc::kill(pid, 0) } == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
}

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
    /// direct invite intro port per `peer_ids` position. Kept explicit because
    /// the product default (`wireguard_listen + 1`) can be another node's
    /// WireGuard port when a test cluster asks the OS for a batch of ports.
    pub(crate) invite_ports: Vec<u16>,
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
    /// grant every node the `compute` service, the harness twin of `ducktape
    /// service enable compute`. The compute plane needs BOTH a `[sandbox]`
    /// table (HOW runs are isolated — set via `extra_toml`) and this grant
    /// (WHETHER this node runs any), so a test that expects provider
    /// discovery, an oracle pool or a capability announce must set it.
    ///
    /// The tags are what the grant CONSENTED to announce; the node announces
    /// those intersected with what it actually discovers. `Some(vec![])` is
    /// therefore the accept-lane-only provider (runs work, advertises
    /// nothing) — the only way to express that state, since the retired
    /// `announce_capabilities` key is gone. `None` = no grant at all.
    pub compute_grant: Option<Vec<String>>,
    /// extra environment variables for node `idx`'s process, index-aligned
    /// with `peer_ids` (what gives each node its own capability-provider
    /// surface: `DUCKTAPE_CAPABILITY_DIR`, spec `detect.env` overrides).
    /// empty per node by default; set before spawn — a respawn re-applies.
    pub env: Vec<Vec<(String, String)>>,
    /// declared BEFORE `dir` so drop order kills + reaps every child first —
    /// removing the tempdir under live processes races their qmdb/journal
    /// writes and silently leaks the subtree.
    nodes: Vec<Option<NodeProc>>,
    /// each node's COMPUTE DAEMON, when `compute_grant` gave it one.
    ///
    /// The node process runs no provider work any more — `ducktape service run
    /// compute` is the compute plane — so a cluster that expects a run to
    /// EXECUTE must run the daemon beside its node, exactly as an operator
    /// does. Declared beside `nodes` so drop order reaps them before the
    /// tempdir goes.
    daemons: Vec<Option<NodeProc>>,
    /// the kinds [`Cluster::spawn_service`] granted per node, so a rewrite of
    /// `services.toml` (every `spawn`) keeps them.
    service_kinds: Vec<Vec<String>>,
    /// each node's EXPLICITLY started service daemons
    /// ([`Cluster::spawn_service`]), one per kind. Same drop-order reasoning as
    /// `daemons`.
    services: Vec<Vec<ServiceProc>>,
    dir: tempfile::TempDir,
}

impl Drop for Cluster {
    /// Kill every daemon this cluster started.
    ///
    /// There is nothing to reap after them any more. A compute daemon used to
    /// leave a `podman system service` child behind — started `--time=0` so it
    /// never idle-exited, surviving the SIGKILL in [`NodeProc::drop`], holding
    /// ~45 MB rooted in a tempdir about to vanish, and reaped only when a
    /// SUCCESSOR booted on the same root, which a torn-down cluster never gets
    /// (102 of them, ~4.5 GB, were once swept by hand). A run's VMM is a child
    /// of its daemon spawned `kill_on_drop`, so the SIGKILL that ends the
    /// daemon ends its guests too.
    fn drop(&mut self) {
        for daemon in &mut self.daemons {
            *daemon = None; // NodeProc::drop kills + waits
        }
        for procs in &mut self.services {
            procs.clear(); // same: kill + wait
        }
    }
}

/// One `ducktape service run <kind>` daemon attached to a node.
struct ServiceProc {
    kind: String,
    proc: NodeProc,
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
        let dir = e2e_tempdir("shape");
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
        let out = Command::new(env!("CARGO_BIN_EXE_ducktape"))
            .arg("node")
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
        let out = Command::new(env!("CARGO_BIN_EXE_ducktape"))
            .arg("node")
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
        let out = Command::new(env!("CARGO_BIN_EXE_ducktape"))
            .arg("node")
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
        let out = Command::new(env!("CARGO_BIN_EXE_ducktape"))
            .arg("node")
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
        Command::new(env!("CARGO_BIN_EXE_ducktape"))
            .arg("node")
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
        let child = Command::new(env!("CARGO_BIN_EXE_ducktape"))
            .arg("node")
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

    /// keep node `idx`'s `<kind>` hello alive — a service daemon's ENTIRE
    /// contribution to the capability-announce lane, without the daemon.
    ///
    /// The daemon PROCESS is deliberately not spawned, and that is a SCOPING
    /// choice, not a limitation of the host: a daemon's entire contribution to
    /// the announce lane is this POST, so booting a sandbox to prove capability
    /// announcement buys no extra signal and couples this lane to `/dev/kvm`
    /// and the guest artifacts. What a real daemon would additionally prove —
    /// that a REAL hello carries the shape this lane expects — is a
    /// daemon-fixture concern, and it belongs in the dispatch e2e (#826), which
    /// owns the `[sandbox]` fixture that lane actually needs.
    ///
    /// The FIRST hello is synchronous and asserted — that IS the readiness
    /// event; the refresh then rides a heartbeat thread like the daemon's.
    ///
    /// ponytail: two known divergences from the real daemon, both latent while
    /// one serialized test uses this.
    /// 1. The real daemon treats a failed FIRST hello as FATAL; this retries
    ///    for 60s, so a permanent refusal (a build-identity skew) burns the
    ///    whole budget and surfaces as a timeout instead of failing instantly.
    ///    Fix by splitting connect-refused (retry, the node is still binding)
    ///    from an answered non-200 (fail now) when a second caller appears.
    /// 2. The heartbeat thread has no exit condition and no liveness check, and
    ///    `common` is shared across test binaries over recycled port ranges —
    ///    so a future SECOND caller could inject a stale `compute` hello into
    ///    an unrelated cluster that inherited the port. Give it a stop flag
    ///    cleared by `kill` at that point.
    pub fn signal_service(&mut self, idx: usize, kind: &str, capabilities: &[&str]) {
        let hello = noded::services::Hello {
            kind: kind.into(),
            version: env!("CARGO_PKG_VERSION").into(),
            build: noded::services::build_identity()
                .expect("tests run from a git checkout")
                .into(),
            capabilities: capabilities.iter().map(|tag| tag.to_string()).collect(),
            scopes: Vec::new(),
            needs: Vec::new(),
        };
        let body = serde_json::to_value(&hello).expect("a hello serializes");
        let port = self.http_ports[idx];
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            match nettest::try_http_json(port, "POST", "/v1/services/hello", Some(&body)) {
                Ok((200, _)) => break,
                other => assert!(
                    Instant::now() < deadline,
                    "node idx {idx} never accepted a {kind:?} hello ({other:?});\n{}",
                    self.all_log_tails(60),
                ),
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        // detached like the daemon's own heartbeat thread: it outlives this
        // call, ignores a post to a node that has gone away, and dies with the
        // test process.
        std::thread::Builder::new()
            .name(format!("hello-{kind}-{idx}"))
            .spawn(move || {
                loop {
                    std::thread::sleep(noded::services::HELLO_TTL / 3);
                    let _ = nettest::try_http_json(port, "POST", "/v1/services/hello", Some(&body));
                }
            })
            .expect("spawn the hello heartbeat");
    }

    /// kill node `idx`'s process (reaped by NodeProc's drop).
    pub fn kill(&mut self, idx: usize) {
        self.nodes[idx] = None;
    }

    /// node `idx`'s captured stdout+stderr — for a failing test to preserve
    /// evidence before the cluster tempdir (and the logs in it) is dropped.
    pub fn log_path(&self, idx: usize) -> PathBuf {
        self.nodes[idx]
            .as_ref()
            .expect("node not running")
            .log
            .clone()
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

    /// drive a membership ceremony verb (`member promote`, `member remove`,
    /// `resident accept`, `resident remove`) against node `idx`'s running rpc,
    /// from that node's config.
    /// `verb` is the space-separated two-token spelling; it is split into argv.
    pub fn run_membership_verb_as(
        &self,
        idx: usize,
        verb: &str,
        pubkey_hex: &str,
    ) -> (bool, String) {
        let cfg = self.config_file(idx);
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

    /// drive a membership ceremony from the founder's running node.
    pub fn run_membership_verb(&self, verb: &str, pubkey_hex: &str) -> (bool, String) {
        self.run_membership_verb_as(0, verb, pubkey_hex)
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
        let dir = e2e_tempdir("cluster");
        let ports = alloc_ports(peer_ids.len() * 4);
        let (p2p_ports, rest) = ports.split_at(peer_ids.len());
        let (rpc_ports, rest) = rest.split_at(peer_ids.len());
        let (http_ports, invite_ports) = rest.split_at(peer_ids.len());
        Self {
            namespace,
            peer_ids: peer_ids.to_vec(),
            validator_ids: validator_ids.to_vec(),
            p2p_ports: p2p_ports.to_vec(),
            invite_ports: invite_ports.to_vec(),
            rpc_ports: rpc_ports.to_vec(),
            http_ports: http_ports.to_vec(),
            advertised: peer_ids.iter().map(|_| None).collect(),
            bootstrap_addr_override: None,
            wireguard: false,
            extra_toml: Vec::new(),
            compute_grant: None,
            daemons: peer_ids.iter().map(|_| None).collect(),
            service_kinds: peer_ids.iter().map(|_| Vec::new()).collect(),
            services: peer_ids.iter().map(|_| Vec::new()).collect(),
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

    pub(crate) fn config_path(&self, idx: usize) -> PathBuf {
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
        cfg.push_str(&self.peer_addrs_toml());
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
            cfg.push_str(&format!(
                "invite_listen = \"127.0.0.1:{}\"\n",
                self.invite_ports[idx]
            ));
        }
        for line in &self.extra_toml {
            cfg.push_str(line);
            cfg.push('\n');
        }
        std::fs::write(&path, cfg).expect("write node config");
        self.write_service_grants(idx);
        path
    }

    /// Write (or remove) the workspace `services.toml` the node reads at boot:
    /// one `[[service]]` per granted kind, unique and kind-SORTED — the file's
    /// own validation rule, which an append would break the moment a second
    /// kind sorted before the first.
    ///
    /// Regenerated alongside node.toml on every spawn, so a respawn after
    /// `wipe_storage` still finds its grants. The dev shape's `storage_dir` IS
    /// its workspace, so the file lands beside the node's state.
    fn write_service_grants(&self, idx: usize) {
        let workspace = self.workspace(idx);
        std::fs::create_dir_all(&workspace).expect("create workspace dir");
        let path = workspace.join("services.toml");
        // A MAP, so `services.toml`'s own rule — kinds unique and sorted — holds
        // by construction rather than by everyone remembering it: two grants of
        // one kind cannot be expressed, and iteration is already sorted. A
        // duplicate `[[service]]` would make the node under test refuse the file
        // at load and fail to boot, with the reason nowhere near the cause.
        //
        // the compute grant announces tags; every explicitly spawned kind
        // announces none (airlock, the lender, discovers no capability at all).
        let mut granted: std::collections::BTreeMap<&str, &[String]> =
            std::collections::BTreeMap::new();
        if let Some(tags) = &self.compute_grant {
            granted.insert("compute", tags);
        }
        for kind in &self.service_kinds[idx] {
            granted.entry(kind.as_str()).or_insert(&[]);
        }
        if granted.is_empty() {
            let _ = std::fs::remove_file(&path);
            return;
        }
        let mut file = String::new();
        for (position, (kind, tags)) in granted.iter().enumerate() {
            let announced = tags
                .iter()
                .map(|tag| format!("{tag:?}"))
                .collect::<Vec<_>>()
                .join(", ");
            // any well-formed id/nonce pair does: the node asks WHETHER a grant
            // of that kind exists and WHAT it announces, never re-deriving the
            // id. The bytes come from the grant's POSITION, which is unique by
            // construction — a first-byte-of-kind derivation collided outright
            // ("agent" and "airlock" are both 0x61).
            let byte = format!("{:02x}", position + 1);
            file.push_str(&format!(
                "\n[[service]]\nkind = \"{kind}\"\ninstance = \"{}\"\n\
                 nonce = \"{}\"\ngranted_unix = 1700000000\ncapabilities = [{announced}]\n\
                 scopes = []\n",
                byte.repeat(32),
                byte.repeat(16),
            ));
        }
        std::fs::write(&path, file).expect("write services.toml");
    }

    /// spawn the node at `idx` as a validator/mesh member, stdout+stderr to a
    /// log file. does not wait for readiness — pair with [`Self::wait_marker`].
    pub fn spawn(&mut self, idx: usize) {
        let id = self.peer_ids[idx];
        let cfg = self.config_path(idx);
        let log = self.dir.path().join(format!("node{id}.log"));
        let out = std::fs::File::create(&log).expect("create node log");
        let err = out.try_clone().expect("clone node log handle");
        let child = Command::new(env!("CARGO_BIN_EXE_ducktape"))
            .arg("node")
            .arg("run")
            .arg("--config")
            .arg(&cfg)
            .envs(self.env[idx].iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(err))
            .spawn()
            .expect("spawn ducktape");
        self.nodes[idx] = Some(NodeProc { id, child, log });
        self.spawn_compute(idx);
    }

    /// spawn node `idx`'s compute daemon, when it has a grant to act under.
    ///
    /// Waits for the node to PUBLISH ITS MESH IDENTITY first, and that is not
    /// politeness: the daemon's opening move is a `/v1/services/hello` POST
    /// whose failure is deliberately fatal (`run_service`: "the FIRST hello
    /// must land ... a build mismatch or a down node is a loud exit, not a
    /// silent spin"). Only the query lane retries. Launching both processes in
    /// the same breath therefore killed the daemon outright whenever it won the
    /// race to the socket — observed as `FATAL: POST http://…/v1/services/hello`
    /// — which is a coin-flip, not a test. An operator starts a service against
    /// a node that is already up, and so does this.
    ///
    /// "app surface listening" is the WRONG marker for it and used to be the one
    /// waited on: the node binds its HTTP listener well before the boundary
    /// status carries a `public_key`, so the daemon could reach a node that was
    /// up and still read `public_key: ""` — exiting with `FATAL: this node has
    /// not published a mesh identity yet`. `validator::run` logs the publish as
    /// its own once-per-boot fact precisely so this wait has a seam to hold.
    ///
    /// `--config` points it at the SAME dev-shape config the node reads (a dev
    /// workspace is its `storage_dir`, which does not contain the config file,
    /// so `--workspace` cannot name it).
    fn spawn_compute(&mut self, idx: usize) {
        if self.compute_grant.is_none() {
            return;
        }
        self.wait_marker(idx, "mesh identity published", Duration::from_secs(90));
        let id = self.peer_ids[idx];
        let cfg = self.config_path(idx);
        let log = self.dir.path().join(format!("compute{id}.log"));
        let out = std::fs::File::create(&log).expect("create compute log");
        let err = out.try_clone().expect("clone compute log handle");
        let child = Command::new(env!("CARGO_BIN_EXE_ducktape"))
            .arg("service")
            .arg("run")
            .arg("compute")
            .arg("--config")
            .arg(&cfg)
            // the grant is already on disk (`write_service_grants`), so the
            // daemon must never try to mint one — there is no tty here.
            .arg("--no-enable")
            .envs(self.env[idx].iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(err))
            .spawn()
            .expect("spawn compute daemon");
        self.daemons[idx] = Some(NodeProc { id, child, log });
    }

    /// Grant node `idx` a service of `kind` and run its daemon beside the node
    /// — the harness twin of `service enable <kind>` + `service run <kind>`.
    ///
    /// Called EXPLICITLY rather than folded into [`Self::spawn`] (where the
    /// compute daemon rides along) because a daemon's FIRST hello must LAND:
    /// the node has to have PUBLISHED ITS MESH IDENTITY before the daemon
    /// starts — a listening http surface is not enough, see [`Self::spawn_compute`]
    /// — and only the test knows when it has waited for that. Any caller that
    /// waited on committed state has already cleared it: the identity goes out
    /// in the startup snapshot, before the first block.
    pub fn spawn_service(&mut self, idx: usize, kind: &str) {
        let id = self.peer_ids[idx];
        // One grant per kind is the file's rule and one daemon per kind is the
        // operator's: a second `service run <kind>` beside the same node would
        // be two processes sharing one grant, which is a test bug, not a
        // scenario. Refuse it here, where the cause is visible.
        let already_running = self.services[idx]
            .iter()
            .any(|service| service.kind == kind);
        let compute_rides_along = kind == "compute" && self.compute_grant.is_some();
        assert!(
            !already_running && !compute_rides_along,
            "a {kind} daemon already runs beside node idx {idx}"
        );
        // `config_file`, not `config_path`: the latter REWRITES node.toml and
        // services.toml, which would drop the grant appended just below.
        let cfg = self.config_file(idx);
        self.service_kinds[idx].push(kind.to_string());
        self.write_service_grants(idx);
        let log = self.dir.path().join(format!("{kind}{id}.log"));
        let out = std::fs::File::create(&log).expect("create service log");
        let err = out.try_clone().expect("clone service log handle");
        let child = Command::new(env!("CARGO_BIN_EXE_ducktape"))
            .arg("service")
            .arg("run")
            .arg(kind)
            .arg("--config")
            .arg(&cfg)
            // the grant is already on disk, so the daemon must never try to
            // mint one — there is no tty here.
            .arg("--no-enable")
            .envs(self.env[idx].iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(err))
            .spawn()
            .expect("spawn service daemon");
        self.services[idx].push(ServiceProc {
            kind: kind.to_string(),
            proc: NodeProc { id, child, log },
        });
    }

    /// poll the log of node `idx`'s `kind` service daemon until a line contains
    /// `marker`. Fails fast if the daemon exits without printing it — the twin
    /// of [`Self::wait_marker`], for the process on the other side of the link.
    pub fn wait_service_marker(
        &mut self,
        idx: usize,
        kind: &str,
        marker: &str,
        timeout: Duration,
    ) -> String {
        let deadline = Instant::now() + timeout;
        loop {
            let service = self.services[idx]
                .iter_mut()
                .find(|service| service.kind == kind)
                .unwrap_or_else(|| panic!("no {kind} daemon runs beside node idx {idx}"));
            let text = std::fs::read_to_string(&service.proc.log).unwrap_or_default();
            if let Some(rest) = find_marker(&text, marker) {
                return rest;
            }
            let exited = service
                .proc
                .child
                .try_wait()
                .expect("poll service")
                .is_some();
            if exited || Instant::now() >= deadline {
                let verb = if exited { "exited" } else { "timed out" };
                panic!(
                    "{kind} daemon beside node idx {idx} {verb} without printing {marker:?};\n{}\n{text}",
                    self.all_log_tails(60),
                );
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    }

    /// kill the node at `idx` (crash-fault injection) and reap it.
    ///
    /// Its service daemons go too: a daemon is that node's plane, and leaving
    /// one attached to a dead node would keep retrying against a port the next
    /// spawn reuses.
    pub fn kill(&mut self, idx: usize) {
        self.services[idx].clear(); // NodeProc::drop kills + waits
        self.daemons[idx] = None;
        self.nodes[idx] = None;
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

    /// the full `peer_addrs` line, index-aligned with `peer_seeds`: node 0
    /// rides `bootstrap_addr()` (honoring a sentry/forwarder override), the
    /// rest their real p2p ports. the mesh has no address gossip, so every
    /// config carries the whole list; self-entries are filtered at resolve.
    fn peer_addrs_toml(&self) -> String {
        let addrs: Vec<String> = self
            .p2p_ports
            .iter()
            .enumerate()
            .map(|(i, port)| match i {
                0 => self.bootstrap_addr(),
                _ => format!("127.0.0.1:{port}"),
            })
            .collect();
        format!("peer_addrs = {addrs:?}\n")
    }

    /// spawn an UNINVITED joiner: identity seed `id`, deliberately absent
    /// from every existing member's `peer_seeds` — the mesh refuses it until
    /// governance admits it and the epoch cutover re-tracks. its own config
    /// lists the CURRENT members as mesh + validators (the invite descriptor
    /// a real joiner receives). transport/rpc/http/invite ports are allocated
    /// so the node can be driven after it promotes itself. call this AFTER
    /// every member spawn — it appends to the cluster index space.
    ///
    /// returns the joiner's cluster index.
    pub fn spawn_joiner(&mut self, id: u64) -> usize {
        let ports = alloc_ports(4);
        let path = self.dir.path().join(format!("node{id}.toml"));
        let mut cfg = String::new();
        cfg.push_str(&format!("id = {id}\n"));
        cfg.push_str(&format!("listen = \"127.0.0.1:{}\"\n", ports[0]));
        cfg.push_str(&format!("namespace = {:?}\n", self.namespace));
        cfg.push_str(&format!("peer_seeds = {:?}\n", self.peer_ids));
        cfg.push_str(&format!("validator_seeds = {:?}\n", self.validator_ids));
        cfg.push_str(&self.peer_addrs_toml());
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
        if self.wireguard {
            cfg.push_str(&format!("wireguard_listen = \"127.0.0.1:{}\"\n", ports[0]));
            cfg.push_str(&format!("invite_listen = \"127.0.0.1:{}\"\n", ports[3]));
        }
        // the joiner runs the same node.toml a member does — including whatever
        // the test appended. Skipping it made a joiner silently diverge from the
        // cluster it was joining (a hermetic `primary_coordinator = "none"` on
        // the members, and a joiner still dialing the live public coordinator).
        for line in &self.extra_toml {
            cfg.push_str(line);
            cfg.push('\n');
        }
        std::fs::write(&path, cfg).expect("write joiner config");

        let log = self.dir.path().join(format!("node{id}.log"));
        let out = std::fs::File::create(&log).expect("create joiner log");
        let err = out.try_clone().expect("clone joiner log handle");
        let child = Command::new(env!("CARGO_BIN_EXE_ducktape"))
            .arg("node")
            .arg("run")
            .arg("--config")
            .arg(&path)
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(err))
            .spawn()
            .expect("spawn joiner node");

        self.peer_ids.push(id);
        self.p2p_ports.push(ports[0]);
        self.invite_ports.push(ports[3]);
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
        let mut child = Command::new(env!("CARGO_BIN_EXE_ducktape"))
            .arg("node")
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
        // (in-seam refactor: the loop body is shared with the compute-daemon
        // twin below, so it moved to `await_marker` verbatim.)
        match await_marker(
            self.nodes[idx].as_mut().expect("node is running"),
            marker,
            timeout,
        ) {
            Ok(rest) => rest,
            Err(why) => panic!(
                "node {why} without printing {marker:?};\n{}",
                self.all_log_tails(60)
            ),
        }
    }

    /// poll node `idx`'s COMPUTE DAEMON log until a line contains `marker`.
    ///
    /// The compute plane is a SEPARATE PROCESS with its own failure domain, and
    /// without this the suite has no eye on it at all: a daemon that exits at
    /// boot — an unconfigured `[sandbox]`, a `/dev/kvm` it cannot open, a hello that
    /// raced its node — leaves a cluster that looks perfectly healthy, because
    /// the node is. The suite then waits out a three-minute convergence budget
    /// and fails on whatever unrelated predicate it happened to be holding,
    /// while the daemon's one-line FATAL sits unread in a log nothing prints.
    /// That is exactly how a compute plane that could not claim ANY work reached
    /// review, so gating on the daemon's own lifecycle marker is the fix.
    pub fn wait_compute_marker(&mut self, idx: usize, marker: &str, timeout: Duration) -> String {
        let daemon = self.daemons[idx]
            .as_mut()
            .expect("node has a compute daemon (set `compute_grant` before spawn)");
        let log = daemon.log.clone();
        match await_marker(daemon, marker, timeout) {
            Ok(rest) => rest,
            Err(why) => panic!(
                "compute daemon {why} without printing {marker:?};\n\
                 --- compute daemon log ---\n{}",
                log_tail(&std::fs::read_to_string(&log).unwrap_or_default(), 60),
            ),
        }
    }

    /// Wait until `probe` answers, re-evaluating it on node `idx`'s heartbeat.
    ///
    /// Replaces a 300ms client-side sleep-and-retry with a thread that blocks on
    /// the node's own ws stream: committed state cannot change without a block,
    /// so re-reading on the node's schedule instead of the test's is both
    /// cheaper and impossible to make fire early. It is a ≤3s wake rather than a
    /// pure event — see [`BlockFeed`] for exactly why.
    ///
    /// Derived read models (`/v1/index/...`) apply BEHIND finalized state, so a
    /// predicate over one simply satisfies a block or two later; the loop is
    /// already per-block, so that costs nothing but patience.
    pub fn await_committed<T>(
        &self,
        idx: usize,
        what: &str,
        timeout: Duration,
        mut probe: impl FnMut() -> Option<T>,
    ) -> T {
        // committed state may already satisfy it — never wait on a block that
        // has nothing left to deliver (and the chain may be idle-quiet).
        if let Some(value) = probe() {
            return value;
        }
        let mut blocks = self.block_feed(idx, timeout);
        loop {
            if let Err(why) = blocks.next_block() {
                panic!(
                    "timed out after {timeout:?} waiting for {what} \
                     (via node idx {idx}): {why};\n{}",
                    self.all_log_tails(60)
                );
            }
            if let Some(value) = probe() {
                return value;
            }
        }
    }

    /// Attach to node `idx`'s block-wake feed (see [`BlockFeed`]).
    pub fn block_feed(&self, idx: usize, timeout: Duration) -> BlockFeed {
        let url = format!("ws://127.0.0.1:{}/v1/ws", self.http_ports[idx]);
        let (socket, _response) = tokio_tungstenite::tungstenite::connect(&url)
            .unwrap_or_else(|e| panic!("attach to node idx {idx} block feed: {e}"));
        // the failure path's bound, not a poll interval — see [`BlockFeed`].
        if let tokio_tungstenite::tungstenite::stream::MaybeTlsStream::Plain(tcp) = socket.get_ref()
        {
            tcp.set_read_timeout(Some(timeout))
                .expect("bound the block feed's failure path");
        }
        BlockFeed {
            socket,
            deadline: Instant::now() + timeout,
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

// ---- sandboxed compute: the `[sandbox]` table and the host gate -------------
//
// One home for both halves, because they are one decision. A suite that boots a
// compute daemon needs a `[sandbox]` table (without one the daemon exits at
// boot) AND needs to know whether this host can honour it — and the two answers
// have to agree. They were separately copied into two test binaries, which is
// how one of them ended up gating on a runtime's version string while the other
// gated on the product's own predicate.

/// the `[sandbox]` table a cluster node boots with. Appended LAST to
/// [`Cluster::extra_toml`] — nothing may follow a toml table header.
///
/// It says only HOW a run is isolated. WHETHER this node runs any is
/// [`Cluster::compute_grant`]; the daemon needs both, and refuses to boot
/// without the table.
///
/// Every node in a cluster names the SAME two images, and that is now free
/// rather than expensive: the guest kernel and rootfs are read-only and shared,
/// so N nodes attach one copy. The container backend gave each daemon its own
/// graph root, which meant a three-node cluster pulled its image three times
/// into three empty stores on every run — the reason that helper took an image
/// argument and defaulted to the smallest one that could work.
///
/// The runtime is the platform's own hypervisor flavor — the same choice
/// [`guest_backend`] probes with — so the daemon this table boots is the one
/// the capability gate just proved can run.
pub fn sandbox_toml() -> Vec<String> {
    let dir = std::env::var("DUCKTAPE_GUEST_DIR")
        .unwrap_or_else(|_| guest_dir().display().to_string());
    let runtime = provider_host::Vmm::platform_default().config_token();
    vec![
        "[sandbox]".into(),
        format!("runtime = {runtime:?}"),
        format!("kernel = {:?}", format!("{dir}/vmlinux")),
        format!("rootfs = {:?}", format!("{dir}/rootfs.ext4")),
        "cores = 0".into(),
        "mem_gb = 0".into(),
    ]
}

/// Can this host isolate a run the way the compute daemon demands? `Some(why)`
/// = no.
///
/// Asks the PRODUCT'S OWN predicate rather than a weaker proxy, and the
/// difference is not academic. `firecracker` on `PATH` is one of several things
/// the backend needs — `/dev/kvm` must OPEN read-write for this process (a host
/// can list the kvm group and still get EACCES until the next login), `mke2fs`
/// and `debugfs` must exist, and the guest images must be built. A suite gated
/// on the weaker question runs anyway and FAILS instead of skipping. Gating on
/// `probe()` means a suite skips when, and only when, a real node would refuse
/// to serve compute.
pub fn unsandboxable_host() -> Option<String> {
    guest_backend().probe().err()
}

/// the backend an e2e node is configured with: the guest artifacts
/// `ops/build-guest-rootfs.sh` produces, overridable for a box that keeps them
/// somewhere else.
pub fn guest_backend() -> provider_host::SandboxBackend {
    let vmm = provider_host::Vmm::platform_default();
    let dir = std::env::var("DUCKTAPE_GUEST_DIR")
        .map_or_else(|_| guest_dir(), PathBuf::from);
    provider_host::SandboxBackend::MicroVm {
        vmm,
        kernel: dir.join("vmlinux"),
        rootfs: dir.join("rootfs.ext4"),
        // the operator's own installed CLIs, exactly as a real node resolves
        // them: an e2e run execs what this box actually has.
        executors: workspace_config::executor_dir().expect("executor dir"),
    }
}

/// where the guest artifacts live by default — the same answer
/// `workspace-config::default_guest_dir` gives `node init`.
pub fn guest_dir() -> std::path::PathBuf {
    workspace_config::default_guest_dir().expect("guest dir")
}

/// `Some(())` = this test cannot run here and the caller must return; `None` =
/// run it.
///
/// A host that cannot sandbox FAILS by default; skipping is the opt-in
/// ([`nettest::ALLOW_MISSING_TOOLS_ENV`]), because libtest captures stderr and a
/// "skipped" line from a passing test reaches nobody. One switch for every
/// capability gate in the tree — a second, sandbox-only one would just be a
/// second thing to remember.
pub fn skip_unless_sandboxed(test: &str) -> Option<()> {
    nettest::skip_without(test, unsandboxable_host())
}

#[allow(unused_imports)] // a shared prelude: not every e2e binary polls
pub use nettest::poll_until;

// `n` distinct free localhost ports, collision-safe (holds every listener at
// once — sequential bind-drop could hand the same port back twice).
use nettest::alloc_ports;

/// Watch one child process's log for `marker`, returning the rest of the line.
///
/// `Err` carries only the reason phrase (`"#3 exited"`, `"timed out"`) so each
/// caller can attach the log tails that make ITS failure diagnosable.
fn await_marker(proc: &mut NodeProc, marker: &str, timeout: Duration) -> Result<String, String> {
    let deadline = Instant::now() + timeout;
    loop {
        let text = std::fs::read_to_string(&proc.log).unwrap_or_default();
        if let Some(rest) = find_marker(&text, marker) {
            return Ok(rest);
        }
        let exited = proc.child.try_wait().expect("poll child").is_some();
        if exited {
            return Err(format!("#{} exited", proc.id));
        }
        if Instant::now() >= deadline {
            return Err(format!("#{} timed out", proc.id));
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

/// A live feed of one node's heartbeat frames — the harness's wake seam for
/// "committed state may have changed".
///
/// The node sends a `heartbeat` frame to every ws client on every block wake,
/// nop fillers included, so an unsubscribed connection is already the changed
/// feed (the compute daemon's own intake rides the same frame). Blocking on that
/// socket means the thread wakes on the chain's own event and re-reads only when
/// there is something new to read.
///
/// **It is not purely event-driven, and the difference matters.**
/// `crates/noded/src/stream.rs` also emits a byte-identical heartbeat on a 3s
/// `tokio::time::interval`, and nothing in the frame distinguishes the two — so
/// this is a ≤3s poll that additionally wakes per block. What it buys over the
/// 300ms client-side spin it replaces is real but bounded: it cannot fire early
/// on a slow box, it re-reads only on the node's own schedule, and an idle chain
/// costs nothing. Calling it "waits on events, never on time" would be false.
///
/// The socket read timeout is the FAILURE path only — a node that has stopped
/// sending anything must fail with a diagnosis rather than hang CI forever. No
/// successful wait ever waits it out.
pub struct BlockFeed {
    socket: tokio_tungstenite::tungstenite::WebSocket<
        tokio_tungstenite::tungstenite::stream::MaybeTlsStream<TcpStream>,
    >,
    deadline: Instant,
}

impl BlockFeed {
    /// Block until the node reports its next block wake.
    fn next_block(&mut self) -> Result<(), String> {
        use tokio_tungstenite::tungstenite::Message;
        loop {
            if Instant::now() >= self.deadline {
                // NOT "no block wake" — the chain is almost always producing
                // them fine and the predicate is simply still false. Saying
                // otherwise sends the reader hunting a stalled consensus that
                // is not stalled; the caller names the predicate instead.
                return Err("never became true".into());
            }
            let frame = self
                .socket
                .read()
                .map_err(|error| format!("block feed read failed: {error}"))?;
            // an unsubscribed connection carries heartbeats and nothing else,
            // but a control frame still has to be stepped over.
            let Message::Text(text) = frame else { continue };
            let woke = serde_json::from_str::<serde_json::Value>(&text)
                .is_ok_and(|value| value["type"] == "heartbeat");
            if woke {
                return Ok(());
            }
        }
    }
}

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
