//! the automatic half of onboarding: a joiner holding a TOKENED invite parks
//! and DELIVERS its pubkey to the member over the lobby channel — no
//! copy/paste — and the member sees it as a pending join request. approval
//! stays a member decision: `promote` (direct admission) casts the ballot, and only then
//! does the joiner promote.

mod common;

use std::time::Duration;

use common::{NetworkShapeCluster, serial};

const CONVERGE: Duration = Duration::from_secs(180);

#[test]
fn a_tokened_join_delivers_the_pubkey_and_manual_approval_promotes() {
    let _serial = serial();
    let mut cluster = NetworkShapeCluster::new();

    cluster.init_founder("join-request");
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));

    // the default invite carries the token — the whole point.
    let invite = cluster.invite();
    let friend_key = cluster.join_friend(&invite);
    assert_eq!(friend_key.len(), 64, "join prints the friend's pubkey hex");

    cluster.spawn(1);
    cluster.wait_marker(1, "joiner mode: parking", Duration::from_secs(60));

    // the joiner's announce reaches the founder without any human in the
    // middle: the founder logs the request and its queue lists the key.
    cluster.wait_marker(0, "join request:", Duration::from_secs(90));
    cluster.wait_marker(1, "join request sent to member", Duration::from_secs(90));
    let requests = cluster.join_requests();
    let requests = requests.as_array().expect("a json array");
    assert_eq!(requests.len(), 1, "exactly one pending request: {requests:?}");
    assert_eq!(requests[0]["joiner"], friend_key.as_str());
    let issuer = requests[0]["issuer"].as_str().expect("issuer is hex");
    assert_eq!(issuer.len(), 64, "the queue names the inviting member");

    // nothing is admitted until a member approves — that is the manual gate.
    let (ok, out) = cluster.run_promote(&friend_key);
    assert!(ok, "promote failed:\n{out}");
    assert!(out.contains("admitted"), "unexpected verb output:\n{out}");

    cluster.wait_marker(0, "cutover complete: epoch 1", CONVERGE);
    cluster.wait_marker(1, "admitted at epoch 1", CONVERGE);
    cluster.wait_marker(1, "synced app_hash=", CONVERGE);
    cluster.wait_marker(1, "promoted: validator at epoch 1", CONVERGE);

    // settled: the approved key is a member now, so the queue drains.
    let after = cluster.join_requests();
    assert_eq!(
        after.as_array().map(Vec::len),
        Some(0),
        "an approved request leaves the queue: {after:?}"
    );
}
