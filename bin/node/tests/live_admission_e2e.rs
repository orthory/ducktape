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

    // the MANUAL flavor (token-less v2 blob): the pubkey travels out-of-band
    // and no lobby announce happens — the tokened flavor has its own e2e
    // (join_request_e2e).
    let invite = cluster.invite_manual();
    let friend_key = cluster.join_friend(&invite);
    assert_eq!(
        friend_key.len(),
        64,
        "join should print the friend's public key hex"
    );

    // opt the friend into the shipped-index warm start (indexable spec §7
    // lane 2) the way an operator would: a hand-edited node-local policy
    // line. the whole lane then rides this admission for real — the founder
    // cuts and serves its index checkpoints over the mesh, the friend
    // fetches and stages them, and the promoted reboot adopts the set.
    let cfg = cluster.config_file(1);
    let toml = std::fs::read_to_string(&cfg).expect("read friend node.toml");
    std::fs::write(&cfg, format!("{toml}sync_index = true\n")).expect("write friend node.toml");

    cluster.spawn(1);
    cluster.wait_marker(1, "joiner mode: parking", Duration::from_secs(60));
    cluster.wait_marker(1, "parked:", Duration::from_secs(60));

    let (ok, out) = cluster.run_invite_accept(&friend_key);
    assert!(ok, "invite-accept failed:\n{out}");
    assert!(out.contains("admitted"), "unexpected verb output:\n{out}");

    cluster.wait_marker(0, "cutover complete: epoch 1", CONVERGE);
    cluster.wait_marker(1, "admitted at epoch 1", CONVERGE);
    cluster.wait_marker(1, "synced app_hash=", CONVERGE);
    cluster.wait_marker(1, "shipped index staged", CONVERGE);
    cluster.wait_marker(1, "promoted: validator at epoch 1", CONVERGE);
}
