//! network-shape live admission: a fresh identity produced by `join` can start
//! immediately, park as a read-only observer, and promote once a running member
//! admits it through governance.

mod common;

use std::time::Duration;

use common::{NetworkShapeCluster, serial};

const CONVERGE: Duration = Duration::from_secs(180);

#[test]
fn network_shape_joiner_parks_until_invite_accept() {
    let _serial = serial();
    let mut cluster = NetworkShapeCluster::new();

    let chain_id = cluster.init_founder("live-admission");
    assert!(
        !chain_id.is_empty(),
        "init should print the founded chain id"
    );
    cluster.spawn(0);
    // network-shape nodes never print the dev-demo `converged app_hash=`; the
    // founder is up and finalizing once its rpc surface is listening (genesis
    // is already crossed by then), which is all `invite`/`invite-accept` need.
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));

    let invite = cluster.invite();
    let friend_key = cluster.join_friend(&invite);
    assert_eq!(
        friend_key.len(),
        64,
        "join should print the friend's public key hex"
    );

    cluster.spawn(1);
    cluster.wait_marker(1, "joiner mode: parking", Duration::from_secs(60));
    cluster.wait_marker(1, "parked:", Duration::from_secs(60));

    let (ok, out) = cluster.run_invite_accept(&friend_key);
    assert!(ok, "invite-accept failed:\n{out}");
    assert!(out.contains("admitted"), "unexpected verb output:\n{out}");

    cluster.wait_marker(0, "cutover complete: epoch 1", CONVERGE);
    cluster.wait_marker(1, "admitted at epoch 1", CONVERGE);
    cluster.wait_marker(1, "synced app_hash=", CONVERGE);
    cluster.wait_marker(1, "promoted: validator at epoch 1", CONVERGE);
}
