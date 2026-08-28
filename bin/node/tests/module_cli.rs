//! `ducktape module …` — the operator verbs for a live code swap.
mod common;

use std::process::Command;
use std::time::Duration;

use common::{Cluster, FIXTURES};

fn ducktape(args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_ducktape"))
        .args(args)
        .output()
        .expect("run ducktape");
    (
        out.status.success(),
        format!(
            "{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

#[test]
fn status_against_no_node_says_the_node_is_not_running() {
    let ws = tempfile::tempdir().expect("tempdir");
    // a dev-shape node.toml with rpc_listen and no node behind it
    let cfg = ws.path().join("node.toml");
    std::fs::write(&cfg, common::minimal_dev_shape_toml(ws.path())).expect("write");
    let (ok, out) = ducktape(&["module", "status", "--config", cfg.to_str().unwrap()]);
    assert!(!ok, "{out}");
    assert!(out.contains("not running"), "{out}");
}

fn fixture(id: &str) -> String {
    format!("{FIXTURES}/{id}.component.wasm")
}

/// `module status --json` is the registry read; the e2e keys on the same
/// projection the operator sees. `None` while a swap is still pending.
fn active_hash(cluster: &Cluster, idx: usize, id: &str) -> Option<String> {
    let cfg = cluster.config_file(idx);
    let (ok, out) = cluster.run_verb(&[
        "module",
        "status",
        "--json",
        "--config",
        cfg.to_str().unwrap(),
    ]);
    assert!(ok, "{out}");
    // stdout is a pretty-printed array; stderr (empty on success) trails it.
    let stdout = &out[..=out.rfind(']')?];
    let modules: Vec<serde_json::Value> = serde_json::from_str(stdout).ok()?;
    let m = modules.iter().find(|m| m["module_id"] == id)?;
    let pending = !m["pending"].is_null();
    if pending {
        return None;
    }
    m["active_code_hash"].as_array().map(|bytes| {
        bytes
            .iter()
            .map(|b| format!("{:02x}", b.as_u64().unwrap()))
            .collect::<String>()
    })
}

fn sha256_hex(path: &str) -> String {
    use sha2::Digest as _;
    let bytes = std::fs::read(path).expect("fixture");
    format!("{:x}", sha2::Sha256::digest(&bytes))
}

/// the house shape: three founders, all validators (3-of-3), each serving
/// with its module-code plane bound and at least one tunnel carrying traffic.
///
/// the code bytes travel over the OVERLAY and nothing else: without
/// `wireguard_listen` the plane binds an OS socket on a `/128` no host owns,
/// retries forever, and the stage route waits on a fan-out that never starts.
fn three_validators() -> Cluster {
    let mut cluster = Cluster::new(&[1, 2, 3], &[1, 2, 3]);
    cluster.wireguard = true;
    // hermetic: without this every node dials the LIVE public coordinator.
    cluster
        .extra_toml
        .push("primary_coordinator = \"none\"".into());
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));
    cluster.spawn(1);
    cluster.spawn(2);
    for idx in 0..3 {
        cluster.wait_marker(idx, "converged root_hash=", Duration::from_secs(120));
        cluster.wait_marker(
            idx,
            "module-code plane: overlay stream bound",
            Duration::from_secs(120),
        );
        cluster.wait_marker(idx, "peer handshake COMPLETE", Duration::from_secs(120));
    }
    cluster
}

/// run one module verb on every validator in turn — the ceremony's own shape:
/// each member runs the same verb, the run landing the deciding ballot
/// executes. one (ok, output) per node, in node order.
fn run_on_each(cluster: &Cluster, verb: &[&str]) -> Vec<(bool, String)> {
    (0..3)
        .map(|idx| {
            let cfg = cluster.config_file(idx);
            let mut args = verb.to_vec();
            args.extend(["--config", cfg.to_str().unwrap()]);
            cluster.run_verb(&args)
        })
        .collect()
}

/// every run exits 0; the runs before the deciding ballot join the first
/// run's proposal (the matcher: each computed its own height) and wait, the
/// deciding run reports the schedule, and any run after finds the registry
/// already holding it. validator-node governance decides at a majority, so
/// at 3-of-3 the second ballot decides.
fn assert_ceremony_scheduled(runs: &[(bool, String)], id: &str) {
    // the ceremony as the operators saw it, for a `--nocapture` reader
    println!("{}", outputs(runs));
    for (ok, out) in runs {
        assert!(*ok, "{out}");
    }
    let scheduled = format!("scheduled {id}");
    let deciding = runs
        .iter()
        .position(|(_, out)| out.contains(&scheduled))
        .unwrap_or_else(|| panic!("no run scheduled {id}:\n{}", outputs(runs)));
    assert_eq!(
        deciding,
        1,
        "the second ballot is the majority at 3-of-3:\n{}",
        outputs(runs)
    );
    assert!(
        runs[0].1.contains("waiting on other voters"),
        "{}",
        runs[0].1
    );
    assert!(runs[1].1.contains("joining open proposal"), "{}", runs[1].1);
    assert!(runs[2].1.contains("already scheduled"), "{}", runs[2].1);
}

fn outputs(runs: &[(bool, String)]) -> String {
    runs.iter()
        .enumerate()
        .map(|(idx, (ok, out))| format!("--- node {idx} (ok={ok}) ---\n{out}"))
        .collect()
}

/// blocks a ceremony's activation is placed out: three sequential runs plus
/// the readiness signals must all land under the lifecycle floor
/// (`height + MIN_SWAP_LEAD`) at execute time, and the chain ticks one block
/// per second while idle. it also bounds the MATCHER: run 1 joins run 0's
/// proposal only while that activation still clears run 1's own floor, so a
/// lead this far above the runs' wall-clock spacing is what keeps
/// `deciding == 1` exact on a loaded box.
const AFTER: &str = "60";

#[test]
fn register_then_update_activate_across_three_validators() {
    let _guard = common::serial();
    let cluster = three_validators();

    // register: hello is not in PRODUCTION, so it is a free id
    let runs = run_on_each(
        &cluster,
        &[
            "module",
            "register",
            "hello",
            &fixture("hello"),
            "--after",
            AFTER,
        ],
    );
    assert_ceremony_scheduled(&runs, "hello");
    let first = sha256_hex(&fixture("hello"));
    let seen = cluster.await_committed(
        0,
        "hello registered and active",
        Duration::from_secs(180),
        || active_hash(&cluster, 0, "hello").filter(|h| *h == first),
    );
    assert_eq!(seen, first);

    // update: the replacement steps the counter by 100
    let runs = run_on_each(
        &cluster,
        &[
            "module",
            "update",
            "hello",
            &fixture("hello-replacement"),
            "--after",
            AFTER,
        ],
    );
    assert_ceremony_scheduled(&runs, "hello");
    let second = sha256_hex(&fixture("hello-replacement"));
    let seen = cluster.await_committed(0, "hello swapped", Duration::from_secs(180), || {
        active_hash(&cluster, 0, "hello").filter(|h| *h == second)
    });
    assert_eq!(seen, second);

    // the table view names the same state on every member
    for idx in 0..3 {
        let cfg = cluster.config_file(idx);
        let (ok, out) = cluster.run_verb(&["module", "status", "--config", cfg.to_str().unwrap()]);
        assert!(ok, "{out}");
        let row = out
            .lines()
            .find(|line| line.starts_with("hello "))
            .unwrap_or_else(|| panic!("no hello row on node {idx}:\n{out}"));
        assert!(row.contains(&second[..12]), "{row}");
    }
}

/// a dead peer refuses BEFORE the proposal (spec decision 2-B): the
/// custodian's push dials it over the userspace stack, which has no SYN
/// timeout, so only the code plane's `OPEN_TIMEOUT` (15 s) turns that peer
/// into a receipt the operator can read — well inside the CLI's 60 s.
#[test]
fn a_dead_peer_refuses_the_proposal_before_it_is_made() {
    let _guard = common::serial();
    let mut cluster = three_validators();
    cluster.kill(2);
    let cfg = cluster.config_file(0);
    let cfg = cfg.to_str().unwrap();
    let started = std::time::Instant::now();
    let (ok, out) = cluster.run_verb(&[
        "module",
        "register",
        "hello",
        &fixture("hello"),
        "--config",
        cfg,
    ]);
    println!("dead-peer run took {:?}:\n{out}", started.elapsed());
    assert!(!ok, "{out}");
    assert!(out.contains("peer  status"), "{out}");
    let dead = common::hex(&Cluster::identity(3));
    assert!(out.contains(&format!("{dead}  open timed out")), "{out}");
    assert!(out.contains("not proposed"), "{out}");
    // nothing reached governance: no pending swap, no proposal at all
    let (_, status) = cluster.run_verb(&["module", "status", "--config", cfg]);
    assert!(!status.contains("hello"), "{status}");
    assert_no_proposals(&cluster, 0);
}

/// "before any governance" is only proven by the proposal list itself: the
/// registry writes nothing until execute, so an empty `module status` row
/// would still pass with a minted proposal sitting open.
fn assert_no_proposals(cluster: &Cluster, idx: usize) {
    use governance::{GovQuery, GovReply, decode_reply, encode_query};
    let raw = cluster
        .query(idx, "governance", &encode_query(&GovQuery::Proposals))
        .expect("governance answers");
    let GovReply::Proposals(views) = decode_reply(&raw).expect("a proposals reply") else {
        panic!("expected Proposals");
    };
    assert!(views.is_empty(), "a proposal was minted: {views:?}");
}

#[test]
fn an_activation_inside_the_min_lead_is_refused_with_the_registry_reason() {
    let _guard = common::serial();
    let cluster = three_validators();
    let runs = run_on_each(
        &cluster,
        &[
            "module",
            "register",
            "hello",
            &fixture("hello"),
            "--after",
            "2",
        ],
    );
    // the lead rule's static half is refused before anything is staged or
    // proposed, on every member alike. (its dynamic half — the ceremony's own
    // blocks overtaking a lead that was long enough at propose time — rejects
    // governance's execute op in-kernel and leaves the proposal open; the CLI
    // names the same rules on that path, but which run trips it depends on
    // block timing, so it is not pinned here.)
    for (ok, out) in &runs {
        assert!(!ok, "{}", outputs(&runs));
        assert!(
            out.contains(
                "--after 2 cannot schedule anything: activation must exceed height+MIN_SWAP_LEAD (3)"
            ),
            "{out}"
        );
        assert!(!out.contains("staged"), "nothing is staged first: {out}");
    }
    let (_, status) = cluster.run_verb(&[
        "module",
        "status",
        "--config",
        cluster.config_file(0).to_str().unwrap(),
    ]);
    assert!(!status.contains("hello"), "{status}");
    assert_no_proposals(&cluster, 0);
}
