//! the automatic half of onboarding, end to end: minting the invite IS the
//! admission decision. a joiner holding a TOKENED invite delivers its pubkey
//! over the lobby channel, the receiving member submits the governance
//! `Redeem` op on its behalf — no approval verb, no human in the middle —
//! and the joiner comes up as a FULL NODE (observer standing: mesh +
//! statesync + a serving read surface). seating it in the QUORUM stays a
//! separate, deliberate act (`promote`), exercised at the end.

mod common;

use std::time::Duration;

use common::{NetworkShapeCluster, poll_until, serial};
use valset::{ValsetQuery, ValsetReply};

const CONVERGE: Duration = Duration::from_secs(180);

#[test]
fn a_tokened_join_redeems_itself_into_a_full_node() {
    let _serial = serial();
    let mut cluster = NetworkShapeCluster::new();

    cluster.init_founder("join-request");
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));

    // the default invite carries the token — the admission capability.
    let invite = cluster.invite();
    let friend_key = cluster.join_friend(&invite);
    assert_eq!(friend_key.len(), 64, "join prints the friend's pubkey hex");

    cluster.spawn(1);
    cluster.wait_marker(1, "joiner mode:", Duration::from_secs(60));

    // the joiner's announce reaches the founder, which redeems it — NO verb
    // runs anywhere in this window.
    cluster.wait_marker(1, "invite announce sent to member", Duration::from_secs(90));
    cluster.wait_marker(0, "invite redemption submitted:", Duration::from_secs(90));

    // the redemption lands in consensus state: the friend holds RESIDENT
    // standing (a full node), while the quorum still seats only the founder.
    let expected = vec![common::unhex(&friend_key)];
    poll_until("the redemption to grant resident standing", CONVERGE, || {
        cluster
            .query(0, "valset", &valset::encode_query(&ValsetQuery::Residents))
            .and_then(|raw| valset::decode_reply(&raw).ok())
            .and_then(|r| match r {
                ValsetReply::Residents(v) if v == expected => Some(()),
                _ => None,
            })
    });
    let validators = cluster
        .query(0, "valset", &valset::encode_query(&ValsetQuery::Validators))
        .and_then(|raw| valset::decode_reply(&raw).ok())
        .map(|r| match r {
            ValsetReply::Validators(v) => v,
            other => panic!("expected Validators, got {other:?}"),
        })
        .expect("valset validators readable");
    assert_eq!(validators.len(), 1, "the quorum still seats ONLY the founder");

    // the full node pre-syncs and serves — the whole point of the flow.
    cluster.wait_marker(1, "resident: pre-synced boundary", CONVERGE);

    // a second announce cannot double-admit: the nonce is spent, standing
    // already exists, and the founder's tracker drains once settled.
    let requests = cluster.join_requests();
    assert_eq!(
        requests.as_array().map(Vec::len),
        Some(0),
        "a settled redemption leaves the queue: {requests:?}"
    );

    // seating it in the quorum is a separate, deliberate act — the existing
    // promote verb over the standing the redemption granted. the redemption's
    // own grant cutover was epoch 1, so the promotion cuts over to epoch 2.
    let (ok, out) = cluster.run_promote(&friend_key);
    assert!(ok, "promote failed:\n{out}");
    cluster.wait_marker(0, "cutover complete: epoch 2", CONVERGE);
    cluster.wait_marker(1, "promoted: validator at epoch 2", CONVERGE);
}

#[test]
fn a_spent_invite_is_refused_loudly_on_both_ends() {
    let _serial = serial();
    let mut cluster = NetworkShapeCluster::new();

    cluster.init_founder("spent-invite");
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));

    // first redeemer: the normal flow, driven to a COMMITTED redemption so
    // the nonce is durably spent before anyone reuses the blob.
    let invite = cluster.invite();
    let friend_key = cluster.join_friend(&invite);
    cluster.spawn(1);
    cluster.wait_marker(0, "invite redemption submitted:", Duration::from_secs(90));
    let expected = vec![common::unhex(&friend_key)];
    poll_until("the redemption to grant resident standing", CONVERGE, || {
        cluster
            .query(0, "valset", &valset::encode_query(&ValsetQuery::Residents))
            .and_then(|raw| valset::decode_reply(&raw).ok())
            .and_then(|r| match r {
                ValsetReply::Residents(v) if v == expected => Some(()),
                _ => None,
            })
    });

    // second redeemer: the SAME blob under a FRESH identity — the shared-blob
    // mistake. every invite is now TARGETED, so `join` refuses the mismatched
    // local key at the CLI, loudly, BEFORE any node spawns. (the spent-nonce
    // lobby path this test used to drive is now unreachable — a non-target key
    // can never announce; the nonce single-use invariant is covered by the
    // governance redemption rig.)
    cluster.kill(1);
    std::fs::remove_dir_all(&cluster.friend_dir).expect("wipe first redeemer");
    let out = cluster.try_join_friend(&invite);
    assert!(
        !out.status.success(),
        "a mismatched-target join must fail loudly at the CLI, not park a node"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("locked to a different key"),
        "the join names the real problem: {stderr}"
    );
    assert!(
        stderr.contains("fresh invite"),
        "the refusal carries actionable guidance (ask for a fresh invite): {stderr}"
    );
    let _ = friend_key;
}
