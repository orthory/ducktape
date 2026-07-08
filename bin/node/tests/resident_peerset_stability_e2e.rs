//! Regression e2e for the parked-joiner discovery peer-set bug.
//!
//! A joined resident that stays un-promoted must track the SAME epoch mesh as
//! the founder — `descriptor_mesh ∪ members ∪ residents`. When the joiner
//! dropped the manifest's residents, commonware's `authenticated::discovery`
//! killed the link on a bit-vector length mismatch (`expected=2 actual=3`)
//! every gossip round — churning the mesh and dropping in-flight statesync.
//! (This is the churn observed live on the sentry + coordinator rig.)
//!
//! This drives the REAL product join flow with `commonware_p2p=debug` on the
//! resident and asserts it logs ZERO such mismatches once it is a committed
//! resident: the founder tracks `{founder, lobby, resident}` (3), so a resident
//! that under-counted to `{founder, lobby}` (2) would disagree at the shared
//! epoch index and PeerKill every round. With the fix both sides track 3.
//!
//! run alone (cluster e2es flake under parallel load):
//!   cargo test -p node-bin --test resident_peerset_stability_e2e -- --nocapture --test-threads=1

mod common;

use std::time::Duration;

use common::{NetworkShapeCluster, serial};

/// standing + follow-arm pre-sync is several blocks of slack.
const CONVERGE: Duration = Duration::from_secs(180);
/// discovery gossips every few seconds; this many rounds is plenty to surface
/// a permanent peer-set disagreement as repeated PeerKills.
const SETTLE: Duration = Duration::from_secs(20);

#[test]
fn a_parked_resident_tracks_residents_and_never_churns_discovery() {
    let _serial = serial();
    let mut cluster = NetworkShapeCluster::new();
    // capture the resident's discovery layer: the bit-vector mismatch that
    // drove the churn is a `commonware_p2p ... debug` line.
    cluster.env[1] = vec![(
        "RUST_LOG".to_string(),
        "commonware_p2p=debug".to_string(),
    )];

    let chain_id = cluster.init_founder("peerset-stability");
    assert!(!chain_id.is_empty(), "init should print the founded chain id");
    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));

    // the product join flow: the parked joiner announces the invite, a member
    // redeems it automatically, resident standing lands — arming the cutover
    // that puts the resident into the founder's tracked epoch mesh.
    let invite = cluster.invite();
    let friend_key_hex = cluster.join_friend(&invite);
    assert_eq!(friend_key_hex.len(), 64, "join prints the friend's pubkey hex");
    cluster.spawn(1);
    cluster.wait_marker(1, "joining:", Duration::from_secs(60));
    cluster.wait_marker(1, "resident: standing granted", CONVERGE);
    cluster.wait_marker(1, "resident: pre-synced boundary", CONVERGE);

    // let the parked resident run several discovery rounds at the post-grant
    // epoch. a resident that dropped its own membership from the tracked set
    // would PeerKill the founder link on every round here.
    std::thread::sleep(SETTLE);

    // the resident's own log (the friend node, idx 1): dir/friend.log.
    let log_path = cluster
        .friend_dir
        .parent()
        .expect("friend dir has a parent")
        .join("friend.log");
    let log = std::fs::read_to_string(&log_path).unwrap_or_default();
    let mismatches = log.matches("bit vector length mismatch").count();
    let killed = log.matches("PeerKilled").count();

    assert_eq!(
        mismatches, 0,
        "resident logged {mismatches} discovery bit-vector length mismatches \
         (and {killed} PeerKilled) — it is under-counting the epoch mesh \
         (dropping residents) and will churn the founder link every round"
    );
}
