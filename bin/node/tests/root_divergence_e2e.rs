//! the validator root-divergence watch, on real processes.
//!
//! Simplex orders `sha256(frame)` digests, so two validators whose fold
//! produced different state finalize together forever: the certificate binds
//! ordered bytes, never a root. `sync::divergence` closes the OBSERVATION half
//! — every validator polls a co-peer's `(height, root_hash)` on the detection
//! lane it already answers for others, and names a disagreement.
//!
//! The unit test in `sync::divergence` pins the compare itself (aligned
//! heights only, latched per peer). What no unit test can prove, and what the
//! build-stamp column that shipped before it never proved at all, is REACH: a
//! validator polls nobody by default, so the whole lane can be wired and dead
//! and silence would look identical to agreement. This drives three real
//! validators and asserts the poll actually completed a round trip — a
//! co-peer answered and this node read its own tip back — and that a healthy
//! cluster names no divergence doing it.

mod common;

use std::time::Duration;

use common::Cluster;

/// the poll ticks every 12s (`sync::divergence::ROOT_POLL_TICK`) and the first
/// one lands after boot + mesh formation; this is generous slack over that on
/// a loaded box, and the wait exits the moment the line appears.
const POLLED: Duration = Duration::from_secs(120);

#[test]
fn every_validator_compares_its_root_with_a_co_peer_and_a_healthy_cluster_diverges_never() {
    let mut cluster = Cluster::new(&[0, 1, 2], &[0, 1, 2]);
    // the completed round trip is a per-op fact, so it logs at debug — RUST_LOG
    // appends to the daemon's info floor, leaving the divergence warn (which
    // this test must also be able to see) exactly where it already was.
    for idx in 0..3 {
        cluster.env[idx] = vec![(
            "RUST_LOG".to_string(),
            "info,ducktape::statesync=debug".to_string(),
        )];
    }

    cluster.spawn(0);
    cluster.wait_marker(0, "rpc listening on", Duration::from_secs(60));
    cluster.spawn(1);
    cluster.spawn(2);
    for idx in 0..3 {
        cluster.wait_marker(idx, "converged root_hash=", Duration::from_secs(180));
    }

    // REACH: the watch reached a co-peer, the peer answered its tip
    // coordinates, and this node read its own back through the same seam.
    // asserted on every validator — the gap this closes is precisely that a
    // validator was the role nothing polled.
    for idx in 0..3 {
        cluster.wait_marker(idx, "root divergence watch compared tips", POLLED);
    }

    // and having compared, a healthy cluster said nothing: these three folded
    // the same blocks with the same module set, so no aligned height may carry
    // two roots.
    for idx in 0..3 {
        assert_eq!(
            cluster.marker_count(idx, "root_divergence"),
            0,
            "validator {idx} named a root divergence on a healthy cluster"
        );
    }
}
