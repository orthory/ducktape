//! `ducktape module …` — the operator verbs for a live code swap.
mod common;

use std::process::Command;
use std::time::Duration;

use common::Cluster;
use common::module_verbs::{
    AFTER, active_hash, assert_ceremony_scheduled, fixture, outputs, run_on_each, sha256_hex,
    spawn_founders,
};

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

fn three_validators() -> Cluster {
    spawn_founders(Cluster::new(&[1, 2, 3], &[1, 2, 3]))
}

#[test]
fn register_then_update_activate_across_three_validators() {
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
