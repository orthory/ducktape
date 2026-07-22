//! invite-after-genesis over real sockets: an UNINVITED key becomes a live
//! validator with zero config edits and zero member restarts.
//!
//! the flow under test (two humans, three commands):
//!   friend starts an out-of-mesh node        -> it parks, refused by the mesh
//!   a member runs `promote <pubkey>` (direct) -> governance passes, valset Join
//!   the epoch cutover re-tracks the mesh     -> the parked node syncs at the
//!                                                boundary, fabricates its
//!                                                recovery checkpoint, reboots,
//!                                                and votes in the new epoch
//!
//! two scenarios, two distinct promotion paths:
//! - `solo_founder_invites_a_friend`: n=1 -> 2. epoch 1 cannot finalize until
//!   the friend arrives (quorum 2 of 2), so the boundary freezes AT the epoch
//!   base — the joiner spawns on the epoch's genesis floor.
//! - `live_quorum_admits_a_fourth_validator`: n=3 -> 4. the three incumbents
//!   keep finalizing past the cutover (quorum 3 of 4), so the joiner syncs a
//!   mid-epoch boundary and needs the served finalization floor certificate.

mod common;

use std::time::Duration;

use common::{Cluster, poll_until, serial};
use directory::{DirMsg, DirQuery, DirReply, decode_reply, encode_msg, encode_query};

/// convergence budget: mesh formation + leader rotation are real-time on a
/// possibly-loaded CI core; polls exit early, so generosity is free.
const CONVERGE: Duration = Duration::from_secs(180);
/// budget for one submitted op to finalize and become readable elsewhere.
const FINALIZE: Duration = Duration::from_secs(60);

fn dir_set(key: &str, value: &str) -> Vec<u8> {
    encode_msg(&DirMsg::Set {
        key: key.into(),
        value: value.into(),
    })
}

fn dir_value(cluster: &Cluster, idx: usize, key: &str) -> Option<String> {
    let reply = cluster.query(
        idx,
        "directory",
        &encode_query(&DirQuery::Get { key: key.into() }),
    )?;
    match decode_reply(&reply) {
        Ok(DirReply::Value(v)) => v,
        Err(_) => None,
    }
}

fn hex(bytes: &[u8]) -> String {
    common::hex(bytes)
}

#[test]
fn solo_founder_invites_a_friend() {
    let _serial = serial();
    // a network of ONE: the founder is mesh and quorum all by itself.
    let mut cluster = Cluster::new(&[0], &[0]);
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));
    cluster.wait_marker(0, "converged app_hash=", CONVERGE);

    // the friend starts an out-of-mesh node: it must PARK (the founder's
    // tracked set does not contain this key), not sync and not crash.
    let joiner = cluster.spawn_joiner(1);
    cluster.wait_marker(joiner, "joiner mode:", Duration::from_secs(60));
    cluster.wait_marker(joiner, "joining:", Duration::from_secs(60));

    // one command on the founder's node: propose + the deciding solo ballot
    // + execute, all through the running node's rpc.
    let friend_hex = hex(&Cluster::identity(1));
    let cfg = cluster.config_file(0);
    let (ok, out) = cluster.run_verb(&[
        "node",
        "member",
        "promote",
        &friend_hex,
        "--config",
        cfg.to_str().expect("utf-8 config path"),
    ]);
    assert!(ok, "promote failed:\n{out}");
    assert!(out.contains("admitted"), "unexpected verb output:\n{out}");

    // the cutover (the nop pusher advances the views)
    // seats the friend directly — epoch 1 then STALLS at its base (quorum
    // 2-of-2), which is exactly what hands the joiner a frozen boundary at
    // the epoch's genesis floor.
    cluster.wait_marker(0, "cutover complete: epoch 1", CONVERGE);

    // the parked node notices its admission, syncs the boundary, fabricates
    // its recovery checkpoint, and reboots into the restore path.
    cluster.wait_marker(joiner, "admitted at epoch 1", CONVERGE);
    cluster.wait_marker(joiner, "synced app_hash=", CONVERGE);
    cluster.wait_marker(joiner, "promoted: validator at epoch 1", CONVERGE);
    cluster.wait_marker(joiner, "recovered app_hash=", CONVERGE);

    // THE property: consensus is live again, and only because the friend
    // votes — a 2-validator simplex finalizes nothing without both. an op
    // submitted via the FOUNDER must become readable via the FRIEND...
    cluster.submit(0, "directory", &dir_set("from-founder", "hello"));
    let value = poll_until("founder's op to read on the friend", FINALIZE, || {
        dir_value(&cluster, joiner, "from-founder")
    });
    assert_eq!(value, "hello");

    // ...and an op submitted via the FRIEND (whose bytes only the friend
    // holds until the relay lane gossips them) must read on the founder.
    cluster.submit(joiner, "directory", &dir_set("from-friend", "hi back"));
    let value = poll_until("friend's op to read on the founder", FINALIZE, || {
        dir_value(&cluster, 0, "from-friend")
    });
    assert_eq!(value, "hi back");

    // no fork: identical status app-hashes once both sides quiesce.
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(
        cluster.status(0)["app_hash"],
        cluster.status(joiner)["app_hash"],
        "founder and promoted friend disagree on state"
    );
}

#[test]
fn live_quorum_admits_a_fourth_validator() {
    let _serial = serial();
    // three validators — enough that the mesh KEEPS finalizing through the
    // whole admission (quorum(4) = 3), forcing the mid-epoch joiner path.
    let mut cluster = Cluster::new(&[0, 1, 2], &[0, 1, 2]);
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));
    cluster.spawn(1);
    cluster.spawn(2);
    for i in 0..3 {
        cluster.wait_marker(i, "converged app_hash=", CONVERGE);
    }

    let joiner = cluster.spawn_joiner(3);
    cluster.wait_marker(joiner, "joiner mode:", Duration::from_secs(60));

    // strict majority of 3 is 2: members 0 and 1 each run the SAME command;
    // the second one's ballot decides and executes.
    let friend_hex = hex(&Cluster::identity(3));
    for member in [0usize, 1] {
        let cfg = cluster.config_file(member);
        let (ok, out) = cluster.run_verb(&[
            "node",
        "member",
        "promote",
            &friend_hex,
            "--config",
            cfg.to_str().expect("utf-8 config path"),
        ]);
        assert!(ok, "promote via member {member} failed:\n{out}");
    }

    // direct admission: ONE cutover seats the joiner on every incumbent.
    for i in 0..3 {
        cluster.wait_marker(i, "cutover complete: epoch 1", CONVERGE);
    }

    // advance the boundary PAST the epoch base while the joiner is still
    // syncing-or-parked: quorum(4) = 3, so the incumbents finalize without
    // it and the joiner must adopt a mid-epoch boundary plus its
    // finalization floor.
    for n in 0..5 {
        cluster.submit(0, "directory", &dir_set(&format!("epoch2-op-{n}"), "x"));
        let _ = poll_until("epoch-2 filler to finalize", FINALIZE, || {
            dir_value(&cluster, 1, &format!("epoch2-op-{n}"))
        });
    }

    cluster.wait_marker(joiner, "admitted at epoch 1", CONVERGE);
    cluster.wait_marker(joiner, "synced app_hash=", CONVERGE);
    cluster.wait_marker(joiner, "promoted: validator at epoch 1", CONVERGE);
    cluster.wait_marker(joiner, "recovered app_hash=", CONVERGE);

    // the promoted validator's own op finalizes and reads on an incumbent —
    // its frame bytes start out ONLY in its store, so this proves the joiner
    // is wired into the payload relay and the vote lanes of the live epoch.
    cluster.submit(joiner, "directory", &dir_set("from-the-fourth", "present"));
    let value = poll_until(
        "the fourth validator's op to read on node 2",
        FINALIZE,
        || dir_value(&cluster, 2, "from-the-fourth"),
    );
    assert_eq!(value, "present");

    // and it holds the full replicated state (no fork after quiesce).
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(
        cluster.status(0)["app_hash"],
        cluster.status(joiner)["app_hash"],
        "incumbent and promoted validator disagree on state"
    );
}
