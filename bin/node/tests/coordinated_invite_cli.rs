use std::net::{IpAddr, Ipv4Addr, TcpListener};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use commonware_cryptography::{Hasher as _, Sha256, Signer as _, ed25519::PrivateKey};
use reachability::PersistedMesh;
use wireguard::{
    AdmissionRoot, Endpoint, EndpointAdvertisement, EndpointRecord, MeshVersion, PortPolicy, Root,
    Transport, ValidatorIdentity, X25519PublicKey,
};

fn command_output(out: &std::process::Output) -> String {
    format!(
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// mint (or reuse) `<dir>/identity.key` via the `keygen` verb and return its
/// pubkey hex — the join code every targeted invite locks to. `join --dir <dir>`
/// reuses this identity, so the join-side target self-check passes.
fn keygen(dir: &Path) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
        .args(["keygen", "--dir"])
        .arg(dir)
        .output()
        .expect("run keygen");
    assert!(out.status.success(), "keygen failed:\n{}", command_output(&out));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// grab `n` distinct free localhost ports by holding every listener at once
/// (sequential bind-drop can hand the same port back twice).
fn alloc_ports(n: usize) -> Vec<u16> {
    let listeners: Vec<TcpListener> = (0..n)
        .map(|_| TcpListener::bind("127.0.0.1:0").expect("bind port-0 probe"))
        .collect();
    listeners
        .iter()
        .map(|l| l.local_addr().expect("probe addr").port())
        .collect()
}

/// recompute a founder's genesis namespace exactly as `config::genesis_namespace`
/// does — sha256 over the scheme + sorted validator hexes, chain-id prefixed —
/// so a seeded `mesh-state.json` rides the SAME chain id the running `invite`
/// verb keys its `reachability::store::load` on. `ducktape:genesis:v1:` is a
/// pinned domain tag; changing it is a genesis flag day the descriptor tests
/// guard, not this one.
fn genesis_namespace(chain_id: &str, validators_hex: &[String]) -> String {
    let mut sorted: Vec<String> = validators_hex
        .iter()
        .map(|v| v.trim().to_ascii_lowercase())
        .collect();
    sorted.sort();
    let mut hasher = Sha256::default();
    hasher.update(b"ducktape:genesis:v1:");
    hasher.update(b"ed25519");
    for v in &sorted {
        hasher.update(b"\n");
        hasher.update(v.as_bytes());
    }
    let digest = hasher.finalize();
    let suffix: String = digest.as_ref()[..16]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("{chain_id}@{suffix}")
}

/// a signed advert for a reachable member with a routable WireGuard underlay
/// endpoint — the mesh peer `cmd_invite` maps into a DIRECT front. Mirrors the
/// node's own persistence shape so `reachability::store::load` verifies it.
fn direct_member_advert(namespace: &str, seed: u64, octet: u8) -> EndpointAdvertisement {
    let policy = PortPolicy::production();
    let signer = PrivateKey::from_seed(seed);
    let ep = |port, transport| {
        Endpoint::new(
            IpAddr::V4(Ipv4Addr::new(8, 8, 8, octet)),
            port,
            transport,
            &policy,
        )
        .expect("endpoint within the production port policy")
    };
    let record = EndpointRecord {
        namespace: namespace.into(),
        epoch: 1,
        valset_root: Root([1; 32]),
        admission_root: AdmissionRoot([2; 32]),
        validator_identity: ValidatorIdentity::try_from(signer.public_key().as_ref()).unwrap(),
        wireguard_public_key: X25519PublicKey([octet; 32]),
        control_endpoint: ep(443, Transport::Tcp),
        wireguard_endpoint: Some(ep(51820, Transport::Udp)),
        expires_at_view: 1000,
        nonce: 1,
    };
    EndpointAdvertisement::sign(record, MeshVersion([7; 32]), &signer)
}

/// lowercase hex of a seed's ed25519 public key — the identity `cmd_invite`
/// writes into a front's `member_key`.
fn identity_hex(seed: u64) -> String {
    PrivateKey::from_seed(seed)
        .public_key()
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// run `ducktape-node --config <config> <extra…>` with combined output to
/// `log`, polling until it exits on its own or `timeout` elapses (then kill +
/// reap). Returns `(exit code, captured log)`; `None` code means it was killed
/// for timing out — a HANG, which for the honest-terminal path is a failure.
fn run_node_until_exit(
    config: &Path,
    extra: &[&str],
    log: &Path,
    timeout: Duration,
) -> (Option<i32>, String) {
    let out = std::fs::File::create(log).expect("create node log");
    let err = out.try_clone().expect("clone node log handle");
    let mut child = Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
        .arg("--config")
        .arg(config)
        .args(extra)
        .stdout(Stdio::from(out))
        .stderr(Stdio::from(err))
        .spawn()
        .expect("spawn ducktape-node");
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().expect("poll node") {
            let text = std::fs::read_to_string(log).unwrap_or_default();
            return (status.code(), text);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let text = std::fs::read_to_string(log).unwrap_or_default();
            return (None, text);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn coordinated_invite_persists_tunnel_bootstrap_without_direct_endpoint() {
    let dir = tempfile::tempdir().expect("tempdir");
    let founder = dir.path().join("founder");
    let friend = dir.path().join("friend");

    let init = Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
        .args([
            "init",
            "--name",
            "coordinated-default",
            "--dir",
            founder.to_str().expect("utf-8 founder dir"),
        ])
        .output()
        .expect("run init");
    assert!(
        init.status.success(),
        "init failed:\n{}",
        command_output(&init)
    );

    let target = keygen(&friend);
    let invite = Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
        .args(["invite", "--config"])
        .arg(founder.join("node.toml"))
        .args(["--target", &target])
        .output()
        .expect("run invite");
    assert!(
        invite.status.success(),
        "invite failed:\n{}",
        command_output(&invite)
    );
    let blob = String::from_utf8_lossy(&invite.stdout).trim().to_string();

    let join = Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
        .args([
            "join",
            &blob,
            "--dir",
            friend.to_str().expect("utf-8 friend dir"),
        ])
        .output()
        .expect("run join");
    assert!(
        join.status.success(),
        "join failed:\n{}",
        command_output(&join)
    );

    let bootstrap = friend.join("invite-wireguard.toml");
    assert!(
        bootstrap.exists(),
        "coordinated invite must persist the inviter's WireGuard bootstrap"
    );
    let text = std::fs::read_to_string(&bootstrap).expect("read invite-wireguard");
    assert!(
        text.contains("public_key"),
        "bootstrap must carry the inviter's WireGuard key:\n{text}"
    );
    assert!(
        text.contains("mesh_port"),
        "bootstrap must carry the inviter's overlay mesh port:\n{text}"
    );
    assert!(
        !text.contains("endpoint"),
        "coordinated bootstrap must not bake in a direct underlay endpoint:\n{text}"
    );
}

/// The unified all-paths invite bundles the inviter's reachable MEMBERS as
/// `fronts`: seed a founder's persisted `mesh-state.json` with one member that
/// has a routable WireGuard underlay endpoint, mint an invite, and prove the
/// decoded blob carries that member as a direct front. We read the fronts back
/// through `join`, which decodes the invite and persists them to
/// `invite-fronts.json` — the joiner's own record of the paths it will race.
#[test]
fn invite_bundles_reachable_member_fronts_from_seeded_mesh_state() {
    let dir = tempfile::tempdir().expect("tempdir");
    let founder = dir.path().join("founder");
    let friend = dir.path().join("friend");

    let init = Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
        .args([
            "init",
            "--name",
            "fronts-bundle",
            "--dir",
            founder.to_str().expect("utf-8 founder dir"),
        ])
        .output()
        .expect("run init");
    assert!(
        init.status.success(),
        "init failed:\n{}",
        command_output(&init)
    );
    // init prints the chain id on stdout and the founder identity on stderr.
    let chain_id = String::from_utf8_lossy(&init.stdout).trim().to_string();
    let founder_hex = String::from_utf8_lossy(&init.stderr)
        .lines()
        .find_map(|l| l.rsplit("identity ").next().filter(|h| h.len() == 64))
        .expect("init stderr names the founder identity")
        .to_string();

    // seed the founder's mesh with ONE reachable member (seed 7) that has a
    // routable underlay endpoint — a direct front. The persisted mesh's chain
    // id must equal the descriptor's genesis namespace or `invite` treats it as
    // a foreign file and carries no fronts.
    let namespace = genesis_namespace(&chain_id, &[founder_hex]);
    let advert = direct_member_advert(&namespace, 7, 20);
    let mesh = PersistedMesh::new(namespace, 1, vec![advert]);
    let storage = founder.join("storage");
    std::fs::create_dir_all(&storage).expect("create founder storage");
    reachability::store::save(&storage.join("mesh-state.json"), &mesh)
        .expect("seed mesh-state.json");

    let target = keygen(&friend);
    let invite = Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
        .args(["invite", "--config"])
        .arg(founder.join("node.toml"))
        .args(["--target", &target])
        .output()
        .expect("run invite");
    assert!(
        invite.status.success(),
        "invite failed:\n{}",
        command_output(&invite)
    );
    let blob = String::from_utf8_lossy(&invite.stdout).trim().to_string();

    let join = Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
        .args([
            "join",
            &blob,
            "--dir",
            friend.to_str().expect("utf-8 friend dir"),
        ])
        .output()
        .expect("run join");
    assert!(
        join.status.success(),
        "join failed:\n{}",
        command_output(&join)
    );

    // join decodes the invite and persists its fronts — proof the blob carried
    // them across.
    let fronts_file = friend.join("invite-fronts.json");
    assert!(
        fronts_file.exists(),
        "the decoded invite must carry the seeded member as a front (invite-fronts.json absent — \
         the mesh state was ignored, likely a chain-id mismatch):\n{}",
        command_output(&invite)
    );
    let fronts = std::fs::read_to_string(&fronts_file).expect("read invite-fronts.json");
    let member_hex = identity_hex(7);
    assert!(
        fronts.contains(&member_hex),
        "the front names the seeded member's identity:\n{fronts}"
    );
    assert!(
        fronts.contains("8.8.8.20:51820"),
        "the reachable member rides as a DIRECT front carrying its underlay endpoint:\n{fronts}"
    );
}

/// A coordinated-only invite (the inviter offers no direct underlay endpoint,
/// no fronts) on a node running the real kernel-TUN effect has NOTHING to race:
/// the userspace rendezvous the coordinated path needs is inactive under TUN,
/// so the by-identity candidate is dropped and the joiner hits the HONEST
/// terminal — a distinct non-zero exit with a mode-naming FATAL, never a hang
/// and never a silent success.
#[test]
fn coordinated_only_invite_on_a_tun_node_fails_honestly() {
    let dir = tempfile::tempdir().expect("tempdir");
    let founder = dir.path().join("founder");
    let friend = dir.path().join("friend");

    let init = Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
        .args([
            "init",
            "--name",
            "coordinated-honest-terminal",
            "--dir",
            founder.to_str().expect("utf-8 founder dir"),
        ])
        .output()
        .expect("run init");
    assert!(
        init.status.success(),
        "init failed:\n{}",
        command_output(&init)
    );

    // a default founder advertises the overlay ULA (not a routable host), so its
    // WireGuard bootstrap is coordinated-only — no underlay endpoint baked in.
    let target = keygen(&friend);
    let invite = Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
        .args(["invite", "--config"])
        .arg(founder.join("node.toml"))
        .args(["--target", &target])
        .output()
        .expect("run invite");
    assert!(
        invite.status.success(),
        "invite failed:\n{}",
        command_output(&invite)
    );
    let blob = String::from_utf8_lossy(&invite.stdout).trim().to_string();

    // join in TUN mode: the friend's node.toml carries `wireguard_effect = "tun"`.
    let ports = alloc_ports(4);
    let join = Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
        .args([
            "join",
            &blob,
            "--dir",
            friend.to_str().expect("utf-8 friend dir"),
            "--listen",
            &format!("127.0.0.1:{}", ports[0]),
            "--advertised",
            &format!("127.0.0.1:{}", ports[0]),
            "--http",
            &format!("127.0.0.1:{}", ports[1]),
            "--rpc",
            &format!("127.0.0.1:{}", ports[2]),
            "--wireguard-effect",
            "tun",
        ])
        .output()
        .expect("run join");
    assert!(
        join.status.success(),
        "join failed:\n{}",
        command_output(&join)
    );

    let (code, log) = run_node_until_exit(
        &friend.join("node.toml"),
        &[],
        &dir.path().join("friend-run.log"),
        Duration::from_secs(90),
    );
    assert!(
        log.contains("first contact failed across all"),
        "a coordinated-only invite under TUN must surface the HONEST first-contact terminal, not \
         hang or silently proceed:\n{log}"
    );
    assert_eq!(
        code,
        Some(3),
        "the honest terminal exits with a distinct non-zero code (never a silent success):\n{log}"
    );
}

/// The field failure this branch fixes, end to end through the REAL binary: a
/// node boots while its coordinator is DARK (machine woke before its network,
/// coordinator restarting) and the coordinator only comes up LATER. The plane
/// must self-heal — retry in the background and register the moment the
/// coordinator answers — instead of degrading to pass-through for the life of
/// the process (the old behavior: one missed 3s window at boot meant no
/// rendezvous, no punch, no tunnels until an operator restarted the node by
/// hand).
#[test]
fn a_dark_coordinator_at_boot_heals_once_it_comes_up() {
    let dir = tempfile::tempdir().expect("tempdir");
    let founder = dir.path().join("founder");

    // reserve a UDP port for the coordinator, then leave it dark for boot.
    let probe = std::net::UdpSocket::bind("127.0.0.1:0").expect("udp port probe");
    let coord_addr = probe.local_addr().expect("probe addr");
    drop(probe);

    // distinct listen/http/rpc ports AND a distinct wireguard UDP port: this
    // test binary runs its tests in parallel, and two spawned nodes on
    // init's defaults (p2p listen, wireguard 0.0.0.0:51820) collide.
    let ports = alloc_ports(3);
    let wg_probe = std::net::UdpSocket::bind("127.0.0.1:0").expect("wg port probe");
    let wg_port = wg_probe.local_addr().expect("wg probe addr").port();
    drop(wg_probe);
    let init = Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
        .args([
            "init",
            "--name",
            "coordinator-heal",
            "--dir",
            founder.to_str().expect("utf-8 founder dir"),
            "--wireguard-effect",
            "socket",
            "--wireguard-listen",
            &format!("127.0.0.1:{wg_port}"),
            "--primary-coordinator",
            &coord_addr.to_string(),
            "--listen",
            &format!("127.0.0.1:{}", ports[0]),
            "--advertised",
            &format!("127.0.0.1:{}", ports[0]),
            "--http",
            &format!("127.0.0.1:{}", ports[1]),
            "--rpc",
            &format!("127.0.0.1:{}", ports[2]),
        ])
        .output()
        .expect("run init");
    assert!(
        init.status.success(),
        "init failed:\n{}",
        command_output(&init)
    );

    let log_path = dir.path().join("founder-heal.log");
    let out = std::fs::File::create(&log_path).expect("create node log");
    let err = out.try_clone().expect("clone node log handle");
    let mut child = Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
        .arg("--config")
        .arg(founder.join("node.toml"))
        .stdout(Stdio::from(out))
        .stderr(Stdio::from(err))
        .spawn()
        .expect("spawn ducktape-node");

    let wait_for = |marker: &str, budget: Duration| -> (bool, String) {
        let deadline = Instant::now() + budget;
        loop {
            let log = std::fs::read_to_string(&log_path).unwrap_or_default();
            if log.contains(marker) {
                return (true, log);
            }
            if Instant::now() >= deadline {
                return (false, log);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    };

    // act 1 — dark: the plane reports the outage and keeps the node up.
    let (saw_unavailable, log) = wait_for("coordinator rendezvous unavailable", Duration::from_secs(25));
    let still_running = child.try_wait().expect("poll node").is_none();
    if !saw_unavailable || !still_running {
        let _ = child.kill();
        let _ = child.wait();
        panic!(
            "the node must stay up and report the dark coordinator (running: \
             {still_running}):\n{log}"
        );
    }
    assert!(
        !log.contains("coordinator-observed reflexive"),
        "nothing answered yet — no reflexive can exist:\n{log}"
    );

    // act 2 — the coordinator comes up on the same address...
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("coordinator runtime");
    let _coordinator = rt.spawn(async move {
        let sock = tokio::net::UdpSocket::bind(coord_addr)
            .await
            .expect("bind the late coordinator");
        nat_traversal::run_coordinator(
            sock,
            nat_traversal::AuthPolicy::Open { require_pop: false },
        )
        .await;
    });

    // ...and the running node registers on its own: establishment retries at
    // 3s doubling to 30s, so one full backoff cycle bounds the wait.
    let (healed, log) = wait_for("coordinator-observed reflexive", Duration::from_secs(45));
    let _ = child.kill();
    let _ = child.wait();
    assert!(
        healed,
        "the plane must establish rendezvous once the coordinator answers — \
         without a process restart:\n{log}"
    );
}

/// End to end through the REAL binary: `invite --short` publishes the full
/// signed blob to the coordinator by content id and prints a `🦆://<name>/<id>`
/// short URL; `join <url>` fetches it back and materializes the workspace. When
/// the joiner points at a coordinator that does NOT hold the link (a restart
/// dropped the in-memory shelf), the short join fails LOUDLY — and the full
/// blob (printed above the URL) still joins with no coordinator at all.
#[test]
fn short_invite_publishes_joins_and_falls_back_to_the_full_blob() {
    let dir = tempfile::tempdir().expect("tempdir");
    let founder = dir.path().join("founder");

    // coordinator A holds the shelf; B starts fresh (an empty, restarted shelf).
    let probe_a = std::net::UdpSocket::bind("127.0.0.1:0").expect("udp probe A");
    let coord_a = probe_a.local_addr().expect("probe A addr");
    drop(probe_a);
    let probe_b = std::net::UdpSocket::bind("127.0.0.1:0").expect("udp probe B");
    let coord_b = probe_b.local_addr().expect("probe B addr");
    drop(probe_b);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("coordinator runtime");
    let _a = rt.spawn(async move {
        let sock = tokio::net::UdpSocket::bind(coord_a).await.expect("bind coord A");
        nat_traversal::run_coordinator(sock, nat_traversal::AuthPolicy::Open { require_pop: true })
            .await;
    });
    let _b = rt.spawn(async move {
        let sock = tokio::net::UdpSocket::bind(coord_b).await.expect("bind coord B");
        nat_traversal::run_coordinator(sock, nat_traversal::AuthPolicy::Open { require_pop: true })
            .await;
    });
    // let both coordinators bind before the first publish (the client retries
    // anyway, but this keeps the test snappy and deterministic).
    std::thread::sleep(Duration::from_millis(300));

    // found a network pointed at coordinator A.
    let init = Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
        .args([
            "init",
            "--name",
            "short-e2e",
            "--dir",
            founder.to_str().expect("utf-8 founder dir"),
            "--primary-coordinator",
            &coord_a.to_string(),
        ])
        .output()
        .expect("run init");
    assert!(init.status.success(), "init failed:\n{}", command_output(&init));

    // mint a SHORT invite: publishes the blob to A, prints the full blob then
    // the short URL as the LAST line.
    let invite = Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
        .args(["invite", "--config"])
        .arg(founder.join("node.toml"))
        .arg("--short")
        .output()
        .expect("run invite --short");
    assert!(
        invite.status.success(),
        "invite --short failed:\n{}",
        command_output(&invite)
    );
    let stdout = String::from_utf8_lossy(&invite.stdout);
    let lines: Vec<&str> = stdout.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let full_blob = lines.first().expect("invite prints the full blob").to_string();
    let short_url = lines.last().expect("invite --short prints the URL").to_string();
    assert!(
        short_url.starts_with("🦆://short-e2e/"),
        "the last line is a 🦆://<name>/<id> short URL:\n{}",
        command_output(&invite)
    );
    assert!(
        full_blob.starts_with('🦆') && !full_blob.contains("://"),
        "the full blob is printed above the short URL:\n{}",
        command_output(&invite)
    );

    // join via the SHORT URL, pointed at A: fetch + materialize the workspace.
    let friend = dir.path().join("friend");
    let join = Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
        .args([
            "join",
            &short_url,
            "--dir",
            friend.to_str().expect("utf-8 friend dir"),
            "--primary-coordinator",
            &coord_a.to_string(),
        ])
        .output()
        .expect("run join <short-url>");
    assert!(
        join.status.success(),
        "join via short URL failed:\n{}",
        command_output(&join)
    );
    let network_toml = friend.join("network.toml");
    assert!(
        network_toml.exists(),
        "a short-URL join must materialize the workspace descriptor"
    );
    let descriptor = std::fs::read_to_string(&network_toml).expect("read network.toml");
    assert!(
        descriptor.contains("short-e2e#"),
        "the materialized descriptor is for the named network:\n{descriptor}"
    );

    // join the SAME URL pointed at the EMPTY coordinator B (a restart dropped
    // the shelf): the link is gone, and the join must fail LOUDLY.
    let friend2 = dir.path().join("friend2");
    let join_gone = Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
        .args([
            "join",
            &short_url,
            "--dir",
            friend2.to_str().expect("utf-8 friend2 dir"),
            "--primary-coordinator",
            &coord_b.to_string(),
        ])
        .output()
        .expect("run join against empty coordinator");
    assert!(
        !join_gone.status.success(),
        "a short invite absent from the coordinator must fail the join, not proceed:\n{}",
        command_output(&join_gone)
    );
    assert!(
        String::from_utf8_lossy(&join_gone.stderr).contains("not on the coordinator"),
        "the failure names the dead link and points at the full-blob fallback:\n{}",
        command_output(&join_gone)
    );

    // the FULL blob still joins with no coordinator at all — the universal
    // fallback the loud error tells the user about.
    let friend3 = dir.path().join("friend3");
    let join_full = Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
        .args([
            "join",
            &full_blob,
            "--dir",
            friend3.to_str().expect("utf-8 friend3 dir"),
        ])
        .output()
        .expect("run join <full-blob>");
    assert!(
        join_full.status.success(),
        "the full blob must still join without any coordinator:\n{}",
        command_output(&join_full)
    );
    assert!(
        friend3.join("network.toml").exists(),
        "the full-blob join materializes the workspace too"
    );
}

/// Regression: an UNREACHABLE ambient coordinator must NOT take down the whole
/// reachability plane. A socket-mode node whose only coordinator is a blackhole
/// used to hard-fail at plane bring-up ("plane not started"), which then made
/// every subsequent first-contact send fail with "reachability plane is gone" —
/// killing even DIRECT joins that never needed a coordinator (and silently
/// overriding a founder's `--primary-coordinator none`). The plane must instead
/// degrade to pass-through: rendezvous off, direct/front paths still live, node
/// stays up.
#[test]
fn unreachable_coordinator_degrades_the_plane_instead_of_killing_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let founder = dir.path().join("founder");

    // a socket-mode network whose only coordinator is an unroutable blackhole
    // (RFC5737 TEST-NET-3 — guaranteed never to answer).
    let init = Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
        .args([
            "init",
            "--name",
            "coordinator-degrade",
            "--dir",
            founder.to_str().expect("utf-8 founder dir"),
            "--wireguard-effect",
            "socket",
            "--primary-coordinator",
            "203.0.113.1:3478",
        ])
        .output()
        .expect("run init");
    assert!(
        init.status.success(),
        "init failed:\n{}",
        command_output(&init)
    );

    // the node never exits on its own here (a healthy solo founder runs
    // forever), so we drive our own deadline: poll the log until the plane
    // either DEGRADES (pass) or refuses to start (fail), then tear the node
    // down. Returning on the first decisive line keeps the test fast.
    let log_path = dir.path().join("founder-run.log");
    let out = std::fs::File::create(&log_path).expect("create node log");
    let err = out.try_clone().expect("clone node log handle");
    let mut child = Command::new(env!("CARGO_BIN_EXE_ducktape-node"))
        .arg("--config")
        .arg(founder.join("node.toml"))
        .stdout(Stdio::from(out))
        .stderr(Stdio::from(err))
        .spawn()
        .expect("spawn ducktape-node");

    let deadline = Instant::now() + Duration::from_secs(25);
    let (degraded, log) = loop {
        let log = std::fs::read_to_string(&log_path).unwrap_or_default();
        if log.contains("coordinator rendezvous unavailable") {
            break (true, log);
        }
        if log.contains("plane not started") {
            break (false, log);
        }
        if Instant::now() >= deadline || child.try_wait().expect("poll node").is_some() {
            break (false, log);
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let still_running = child.try_wait().expect("poll node").is_none();
    let _ = child.kill();
    let _ = child.wait();

    assert!(
        degraded,
        "an unreachable coordinator must DEGRADE the plane (\"coordinator rendezvous \
         unavailable\"), never refuse to start it:\n{log}"
    );
    assert!(
        !log.contains("plane not started"),
        "the reachability plane must START (degraded), never hard-fail, on a dead \
         coordinator — else direct/front joins break:\n{log}"
    );
    assert!(
        still_running,
        "the node must keep running to serve direct/front paths, not exit, on a dead \
         coordinator:\n{log}"
    );
}
