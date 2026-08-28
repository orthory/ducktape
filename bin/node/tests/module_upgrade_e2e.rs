//! spec §5: a module registered after genesis, live-swapped, then carried
//! across a crash restart and to a statesynced joiner — the acceptance test
//! for §1's "admitted modules across restart and statesync".
mod common;

use std::time::Duration;

use common::Cluster;
use common::module_verbs::{
    AFTER, active_hash, assert_ceremony_scheduled, fixture, run_on_each, sha256_hex, spawn_founders,
};

/// a query or status read that should already be true lands within a block
/// or two; this is the budget for a ws-block-fed wait on it.
const FINALIZE: Duration = Duration::from_secs(60);
/// a swap activates `AFTER` idle blocks after the deciding ballot.
const ACTIVATE: Duration = Duration::from_secs(180);

/// three founders (3-of-3) plus a DECLARED fourth peer (idx 3 = id 4) that
/// is not spawned: statesync is fail-closed for a peer with no committed
/// standing, and the harness's joiner helpers key on the index, so the
/// joiner must exist in the cluster layout from the start.
fn founders_and_declared_joiner() -> Cluster {
    let mut cluster = Cluster::new(&[1, 2, 3, 4], &[1, 2, 3]);
    // every sealed block stays in the journal-replay window: the restart
    // must REPLAY the register, the swap and both incs, not restore hello
    // from a checkpoint that happened to land after them.
    cluster.extra_toml.push("checkpoint_blocks = 100000".into());
    spawn_founders(cluster)
}

/// hello's query is "any bytes → the counter as LE u64"; `None` while the
/// node cannot answer for it yet (not serving, or the module not active).
fn count(cluster: &Cluster, idx: usize) -> Option<u64> {
    let reply = cluster.query(idx, "hello", b"")?;
    let bytes: [u8; 8] = reply.get(..8)?.try_into().ok()?;
    Some(u64::from_le_bytes(bytes))
}

/// the composite root every listed node reports, when they all agree.
fn root_hashes_agree(cluster: &Cluster, idxs: &[usize]) -> Option<String> {
    let hashes: Vec<String> = idxs
        .iter()
        .map(|&idx| {
            cluster.status(idx)["root_hash"]
                .as_str()
                .map(str::to_string)
        })
        .collect::<Option<_>>()?;
    let all_same = hashes.iter().all(|h| *h == hashes[0]);
    all_same.then(|| hashes[0].clone())
}

/// `inc` on one node, the new count readable on every founder.
fn inc_and_confirm(cluster: &Cluster, submit_on: usize, expect: u64) {
    cluster.submit(submit_on, "hello", b"inc");
    for idx in 0..3 {
        let seen = cluster.await_committed(
            idx,
            &format!("hello count == {expect} on node {idx}"),
            FINALIZE,
            || count(cluster, idx).filter(|c| *c == expect),
        );
        assert_eq!(seen, expect);
    }
}

fn register_and_activate(cluster: &Cluster) {
    let runs = run_on_each(
        cluster,
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
    for idx in 0..3 {
        let seen = cluster.await_committed(idx, "hello active", ACTIVATE, || {
            active_hash(cluster, idx, "hello").filter(|h| *h == first)
        });
        assert_eq!(seen, first);
    }
}

fn update_and_activate(cluster: &Cluster) {
    let runs = run_on_each(
        cluster,
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
    for idx in 0..3 {
        let seen = cluster.await_committed(idx, "hello swapped", ACTIVATE, || {
            active_hash(cluster, idx, "hello").filter(|h| *h == second)
        });
        assert_eq!(seen, second);
    }
}

#[test]
fn a_registered_module_survives_a_live_swap_a_restart_and_statesync() {
    let _guard = common::serial();
    let mut cluster = founders_and_declared_joiner();

    // 2. register hello on all three; 3. inc → 1 everywhere
    register_and_activate(&cluster);
    inc_and_confirm(&cluster, 1, 1);

    // 4. swap in the replacement (steps by 100); 5. inc → 101 everywhere,
    // and the composite root agrees across the founders
    update_and_activate(&cluster);
    inc_and_confirm(&cluster, 1, 101);
    let root_after_swap = cluster.await_committed(
        0,
        "founders' root-hashes to agree after the swap",
        FINALIZE,
        || root_hashes_agree(&cluster, &[0, 1, 2]),
    );

    // Task 3 continues here (step 6), then Task 4 (step 7)
    let _ = (&mut cluster, root_after_swap);
}
