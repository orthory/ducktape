//! The restarted resident's boot ORDER, guarded at the source.
//!
//! A restarted resident's reachability plane is dead until its boot
//! `Retarget` reaches it: no target, no restore, no tunnels — and every
//! transport it has left (p2p mesh, state sync, index backfill) rides those
//! tunnels. So the retarget must be the FIRST thing the recovery path does
//! with the replayed state, ahead of `heal_and_backfill_index`, whose
//! per-module fetches go over the very mesh the retarget resurrects: with
//! the tunnels down each one burns its full timeout, and ordering the plane
//! behind them held a live node in `joining` for nearly four minutes.
//!
//! The rule is an ORDER, which no type can express — hence this lint.

use std::path::Path;

/// The 1-based lines carrying `needle` in `src`, in order. An empty hit list
/// is a defect, not a pass: the lint would otherwise guard nothing.
fn lines_of(src: &str, needle: &str) -> Vec<usize> {
    let hits: Vec<usize> = src
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains(needle))
        .map(|(n, _)| n + 1)
        .collect();
    assert!(
        !hits.is_empty(),
        "`{needle}` is gone from the replica park loop — this lint no longer guards anything"
    );
    hits
}

#[test]
fn the_recovery_path_targets_the_plane_before_any_index_heal() {
    let park = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/replica/park.rs");
    let src = std::fs::read_to_string(&park).expect("read the replica park loop");

    let retarget = lines_of(&src, "retarget_reach_plane(cmd, rec.epoch");
    assert_eq!(
        retarget.len(),
        1,
        "the recovery path retargets the plane exactly once: {retarget:?}"
    );
    // the ascension path heals the index too; the boot retarget precedes
    // every heal, so the earliest one is the bound that matters.
    let first_heal = lines_of(
        &src,
        "heal_and_backfill_index(&index, &client, tip, &label).await",
    )
    .into_iter()
    .min()
    .expect("a non-empty hit list has a minimum");

    assert!(
        retarget[0] < first_heal,
        "the boot Retarget (line {}) must precede heal_and_backfill_index (line {first_heal}): \
         the heal awaits fetches over the mesh the retarget brings up, so behind it the \
         restarted resident sits tunnel-less for the whole doomed sweep",
        retarget[0]
    );
}
