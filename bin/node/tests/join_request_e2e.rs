//! the automatic half of onboarding, end to end: minting the invite IS the
//! admission decision. a joiner holding a TOKENED invite delivers its pubkey
//! in its sealed first-contact intro, the receiving member submits the governance
//! `Redeem` op on its behalf — no approval verb, no human in the middle —
//! and the joiner comes up as a FULL NODE (observer standing: mesh +
//! statesync + a serving read surface). seating it in the QUORUM stays a
//! separate, deliberate act (`promote`), exercised at the end.

mod common;

use std::time::Duration;

use common::NetworkShapeCluster;
use valset::{ValsetQuery, ValsetReply};

const CONVERGE: Duration = Duration::from_secs(180);

#[test]
fn a_tokened_join_redeems_itself_into_a_full_node() {
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

    // the joiner reaches the founder on its own (the join protocol first contact or the
    // announce fallback), and the founder redeems it — NO verb runs anywhere
    // in this window.
    cluster.wait_admitted(1, Duration::from_secs(90));
    cluster.wait_marker(0, "gate: redemption submitted for", Duration::from_secs(90));

    // the redemption lands in consensus state: the friend holds RESIDENT
    // standing (a full node), while the quorum still seats only the founder.
    let expected = vec![common::unhex(&friend_key)];
    cluster.await_committed(
        0,
        "the redemption to grant resident standing",
        CONVERGE,
        || {
            cluster
                .query(0, "valset", &valset::encode_query(&ValsetQuery::Residents))
                .and_then(|raw| valset::decode_reply(&raw).ok())
                .and_then(|r| match r {
                    ValsetReply::Residents(v) if v == expected => Some(()),
                    _ => None,
                })
        },
    );
    let validators = cluster
        .query(0, "valset", &valset::encode_query(&ValsetQuery::Validators))
        .and_then(|raw| valset::decode_reply(&raw).ok())
        .map(|r| match r {
            ValsetReply::Validators(v) => v,
            other => panic!("expected Validators, got {other:?}"),
        })
        .expect("valset validators readable");
    assert_eq!(
        validators.len(),
        1,
        "the quorum still seats ONLY the founder"
    );

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
    let mut cluster = NetworkShapeCluster::new();

    cluster.init_founder("spent-invite");
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));

    // first redeemer: the normal flow, driven to a COMMITTED redemption so
    // the nonce is durably spent before anyone reuses the blob.
    let invite = cluster.invite();
    let friend_key = cluster.join_friend(&invite);
    cluster.spawn(1);
    cluster.wait_marker(0, "gate: redemption submitted for", Duration::from_secs(90));
    let expected = vec![common::unhex(&friend_key)];
    cluster.await_committed(
        0,
        "the redemption to grant resident standing",
        CONVERGE,
        || {
            cluster
                .query(0, "valset", &valset::encode_query(&ValsetQuery::Residents))
                .and_then(|raw| valset::decode_reply(&raw).ok())
                .and_then(|r| match r {
                    ValsetReply::Residents(v) if v == expected => Some(()),
                    _ => None,
                })
        },
    );

    // second redeemer: the SAME blob under a FRESH identity — the shared-blob
    // mistake. a bearer invite mints the workspace locally without
    // complaint — there is no targeted key for the CLI to check — and the
    // single-use invariant lands TERMINALLY at first contact: the founder's
    // gate sees the spent nonce and refuses permanently, and the joiner stops
    // loudly instead of parking forever.
    cluster.kill(1);
    std::fs::remove_dir_all(&cluster.friend_dir).expect("wipe first redeemer");
    let out = cluster.try_join_friend(&invite);
    assert!(
        out.status.success(),
        "a bearer join mints the workspace locally; the refusal is at first contact:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    cluster.spawn(1);
    cluster.wait_marker(
        0,
        "ALREADY-REDEEMED invite",
        std::time::Duration::from_secs(90),
    );
    cluster.wait_marker(1, "join gate refused", std::time::Duration::from_secs(90));
    // the refusal is terminal: the joiner exits instead of spinning.
    cluster.wait_exit(1, std::time::Duration::from_secs(60));
    let _ = friend_key;
}
