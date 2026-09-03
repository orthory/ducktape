//! Regression e2e for parked-joiner mesh stability.
//!
//! Historically (under `authenticated::discovery`) a resident that tracked a
//! DIFFERENT set composition than the founder at a shared index was killed on
//! a bit-vector length mismatch every gossip round — churning the mesh and
//! dropping in-flight statesync. The transport is `authenticated::lookup`
//! now: there is no shared wire artifact left to disagree over, and the
//! tracked window is derived from replicated state on every node. What can
//! still go wrong — and what this test pins — is the tracking discipline
//! itself: a node whose window-sync re-tracked an existing generation index
//! (or regressed one) is silently warn-dropped by commonware, leaving its
//! mesh view stale exactly like the old bug did.
//!
//! This drives the REAL product join flow with `commonware_p2p=debug` on the
//! resident and asserts that after several quiet rounds at the post-grant
//! generation it logged ZERO tracker rejections — the direct health signal
//! of the generation-window discipline on a long-lived link.
//!
//! run alone (cluster e2es flake under parallel load):
//!   cargo test -p node-bin --test resident_peerset_stability_e2e -- --nocapture --test-threads=1

mod common;

use std::time::Duration;

use common::NetworkShapeCluster;

/// standing + follow-arm pre-sync is several blocks of slack.
const CONVERGE: Duration = Duration::from_secs(180);
/// the lookup dialer/tracker act within seconds; this many rounds is plenty
/// to surface a permanent tracking disagreement as repeated rejections.
const SETTLE: Duration = Duration::from_secs(20);

#[test]
fn a_parked_resident_tracks_the_window_and_never_churns_the_mesh() {
    let mut cluster = NetworkShapeCluster::new();
    // capture the resident's mesh layer: tracker rejections are
    // `commonware_p2p ... warn` lines.
    cluster.env[1] = vec![(
        "RUST_LOG".to_string(),
        "commonware_p2p=debug".to_string(),
    )];

    let chain_id = cluster.init_founder("peerset-stability");
    assert!(!chain_id.is_empty(), "init should print the founded chain id");
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));

    // the product join flow: the parked joiner announces the invite, a member
    // redeems it automatically, resident standing lands — advancing the
    // membership generation that puts the resident into every tracked window.
    let invite = cluster.invite();
    let friend_key_hex = cluster.join_friend(&invite);
    assert_eq!(friend_key_hex.len(), 64, "join prints the friend's pubkey hex");
    cluster.spawn(1);
    cluster.wait_marker(1, "joining:", Duration::from_secs(60));
    cluster.wait_admitted(1, CONVERGE);
    cluster.wait_marker(1, "resident: pre-synced boundary", CONVERGE);

    // let the parked resident run several quiet rounds at the post-grant
    // generation. a window-sync that re-tracked or regressed an index would
    // be warn-dropped here, once per sync attempt.
    std::thread::sleep(SETTLE);

    // the resident's own log (the friend node, idx 1): dir/friend.log.
    let log_path = cluster
        .friend_dir
        .parent()
        .expect("friend dir has a parent")
        .join("friend.log");
    let log = std::fs::read_to_string(&log_path).unwrap_or_default();
    let duplicates = log.matches("peer set already exists").count();
    let regressions = log.matches("index must monotonically increase").count();

    assert_eq!(
        duplicates + regressions, 0,
        "resident logged {duplicates} duplicate-index and {regressions} \
         regressed-index tracker rejections — its window sync is fighting \
         the monotonic tracking discipline and its mesh view is stale"
    );
}
