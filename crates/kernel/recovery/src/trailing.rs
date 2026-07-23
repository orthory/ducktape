//! trailing-seal bound-and-verify — the per-commit height-cursor consumer.
//!
//! ## the gap this closes (the un-fsync'd trailing seal)
//!
//! a [`crate::Record::Seal`] is a PLAIN journal append: the NEXT block's
//! pre-apply sync makes it durable. a POWER CUT (not a SIGKILL — the page
//! cache preserves an un-fsync'd append across a process death) after a disk
//! module's per-block commit but before that next sync loses the tip seal.
//! boot then sees the disk module at a live root that matches NO recorded
//! post-root — the record that would have vouched for it is exactly the one
//! that was lost — and the forward pre-scan (which seeds durable floors ONLY
//! from exact live-root == sealed-post-root matches) leaves it floorless, so
//! the sequential classifier fail-stops the first sealed block that touched
//! the module: a FALSE [`crate::Error::Torn`] that bricks a solo node.
//!
//! ## the bound-and-verify
//!
//! the fix does NOT loosen the exact-match rule. it widens the set of
//! VERIFIABLE states by consuming one extra durable record: a disk module's
//! per-commit height cursor ([`sdk::Module::durable_commit_height`]) —
//! the height of its last durable commit, persisted ATOMICALLY with that
//! commit (the duckfs refs-file envelope stamps it inside the same
//! checksummed, atomically-renamed file as the refs image). a floorless disk
//! module is accepted iff:
//!
//! 1. BOUND — the journal tip holds its single unsealed [`crate::Record::
//!    Block`] (the WAL invariant: each pre-apply record is fsync'd before the
//!    apply, and at most one apply is ever in flight, so the trailing height
//!    is unambiguous), and
//! 2. VERIFY — the module's cursor claims EXACTLY that height. the cursor
//!    rides the module's own atomic commit, so it binds the live root to one
//!    specific finalized frame — the frame recovery still holds, durably, in
//!    the WAL. determinism does the rest: the module's committed state for
//!    finalized frame H is a pure function of (its seal-verified state at
//!    H-1, the frame), the module commits only through the host's block
//!    path, and its commit is a single atomic unit — so the live root IS the
//!    post-root the lost seal would have recorded.
//!
//! anything else stays fail-closed: no cursor, a cursor at any other height, a claimed height with no
//! trailing WAL record, more than one claimant (the multi-store atomicity
//! limit), or a moved module a claim cannot explain — all still
//! [`crate::Error::Torn`]. this is the "per-commit height cursor" fix the
//! crate docblock prescribed for trailing states.
//!
//! ## residual (shared with the pre-existing torn heal)
//!
//! selective replay of a mixed trailing block re-executes the frame with
//! the claimant already at its post state; an op that READS the claimant's
//! committed state could in principle observe post- instead of pre-state and
//! diverge. The sealed torn-block heal has the same exposure, with a
//! per-module post-root backstop; a trailing block has no recorded roots.
//! Re-derivation is required because sealing observed mixed roots is wrong
//! whenever the trailing block fanned out to the in-memory cohort.

use std::collections::{BTreeMap, BTreeSet};

use host::Host;
use sdk::{ModuleId, StateRoot};

use crate::{Error, Record};

/// the height of the journal tip's single unsealed [`Record::Block`], if any —
/// the only height a trailing durable commit can possibly belong to. records
/// at or below the checkpoint are pre-checkpoint remnants (not yet pruned) and
/// are skipped, mirroring the replay loop's own window filter.
pub(crate) fn trailing_wal_height(records: &[Record], checkpoint: Option<u64>) -> Option<u64> {
    let mut pending: Option<u64> = None;
    for record in records {
        match record {
            Record::Block { height, .. } => {
                if checkpoint.is_some_and(|h| *height <= h) {
                    continue;
                }
                // the replay loop enforces the at-most-one-unsealed invariant
                // (and fail-stops Corrupt on violation); here the LAST unsealed
                // block is the tip either way.
                pending = Some(*height);
            }
            Record::Seal { height, .. } => {
                if checkpoint.is_some_and(|h| *height <= h) {
                    continue;
                }
                if pending == Some(*height) {
                    pending = None;
                }
            }
            Record::Pinned { .. } | Record::Cutover { .. } => {}
        }
    }
    pending
}

/// seed a durable floor AT the trailing WAL height for every disk-cohort
/// module whose live root matched no recorded root but whose per-commit
/// height cursor claims exactly that height (the bound-and-verify above).
/// returns the CLAIMED set — the trailing roll-forward needs it to tell a
/// verified-ahead module from an unexplained moved one.
///
/// consulted only for modules the exact-match pre-scan left floorless AND
/// whose live root moved off the checkpoint root: a module at any recorded
/// root needs no claim, and a cursor may never override a recorded root.
/// every rejection here leaves the module floorless, which the sequential
/// classifier fail-stops as [`Error::Torn`] — fail-closed is preserved, the
/// cursor can only WIDEN the verifiable set, never bypass a record.
pub(crate) fn seed_trailing_claims(
    host: &Host,
    disk_cohort: &BTreeSet<ModuleId>,
    checkpoint_roots: &BTreeMap<ModuleId, StateRoot>,
    trailing: Option<u64>,
    disk_floor: &mut BTreeMap<ModuleId, u64>,
) -> BTreeSet<ModuleId> {
    let mut claims = BTreeSet::new();
    let Some(trailing) = trailing else {
        // no unsealed WAL record: there is nothing durable a trailing commit
        // could be verified against — every floorless moved root stays torn.
        return claims;
    };
    for id in disk_cohort {
        if disk_floor.contains_key(id) {
            continue; // seal-verified at its own floor; the cursor is moot.
        }
        let live = host.module_root(id);
        if live.is_none() || live == checkpoint_roots.get(id).copied() {
            continue; // still at the checkpoint pre-root: normal replay.
        }
        let Some(cursor) = host.durable_commit_height(id) else {
            continue; // no cursor: nothing to verify a trailing state by.
        };
        if cursor == trailing {
            disk_floor.insert(id.clone(), trailing);
            claims.insert(id.clone());
        }
        // cursor != trailing: the module claims a height for which no durable
        // record (seal OR WAL frame) can vouch — leave it floorless (Torn).
    }
    claims
}

/// classify the trailing block's moved set against the verified claims.
/// fail-closed on every ambiguity:
/// - a moved module WITHOUT a claim alongside a claimant is unexplained
///   state — a claim proves the trailing block committed durably, so an
///   unverifiable second mover is damage (or a >1-substrate commit), not a
///   healthy roll-forward;
/// - two or more claimants is a changed set spanning >1 per-block-durable
///   substrate — the multi-store atomicity limit the sealed-block path
///   refuses for the same reason (the single-frame re-execution could read a
///   partially-committed world, and a trailing block has no sealed roots to
///   verify the heal against).
pub(crate) fn classify_trailing(
    height: u64,
    moved: &BTreeSet<ModuleId>,
    claims: &BTreeSet<ModuleId>,
) -> Result<(), Error> {
    let verified: BTreeSet<&ModuleId> = moved.intersection(claims).collect();
    if verified.is_empty() {
        return Ok(());
    }
    let unexplained: Vec<&ModuleId> = moved.iter().filter(|id| !verified.contains(*id)).collect();
    if !unexplained.is_empty() {
        return Err(Error::Torn(format!(
            "trailing block {height}: module(s) {unexplained:?} moved off their pre-block \
             roots with no seal and no verified height-cursor claim, alongside verified \
             claimant(s) {verified:?} — unverifiable trailing state. wipe app state and \
             re-sync (keep the consensus journal)"
        )));
    }
    if verified.len() >= 2 {
        return Err(Error::Torn(format!(
            "trailing block {height}: {} per-block-durable disk substrates all claim its \
             commit — the multi-store atomicity limit; an unsealed block's selective replay \
             cannot be verified across >1 substrate. wipe app state and re-sync (keep the \
             consensus journal)",
            verified.len()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use node::Disposition;

    fn ids(names: &[&str]) -> BTreeSet<ModuleId> {
        names.iter().map(|s| s.to_string()).collect()
    }

    fn seal(height: u64) -> Record {
        Record::Seal {
            height,
            disposition: Disposition::Applied,
            roots: vec![],
            app_hash: StateRoot([0; 32]),
        }
    }

    fn block(height: u64) -> Record {
        Record::Block {
            height,
            frame: vec![],
        }
    }

    #[test]
    fn trailing_height_is_the_tip_unsealed_block_only() {
        // fully sealed journal: no trailing height.
        let records = vec![block(0), seal(0), block(1), seal(1)];
        assert_eq!(trailing_wal_height(&records, None), None);
        // an unsealed tip block IS the trailing height.
        let records = vec![block(0), seal(0), block(1)];
        assert_eq!(trailing_wal_height(&records, None), Some(1));
        // pre-checkpoint remnants are outside the window.
        let records = vec![block(0), seal(0), block(1)];
        assert_eq!(trailing_wal_height(&records, Some(1)), None);
        // an empty journal has no trailing height.
        assert_eq!(trailing_wal_height(&[], None), None);
    }

    #[test]
    fn classify_no_claims_allows_rederivation() {
        classify_trailing(7, &ids(&["kv"]), &BTreeSet::new()).expect("rederive");
    }

    #[test]
    fn classify_single_verified_claim_selectively_replays() {
            classify_trailing(7, &ids(&["files"]), &ids(&["files"])).expect("verified");
    }

    #[test]
    fn classify_unexplained_mover_alongside_a_claim_fail_stops() {
        // "files" verifiably committed the trailing block, but "kv" ALSO moved
        // with no claim: unverifiable — must stay fail-closed.
        let err = classify_trailing(7, &ids(&["files", "kv"]), &ids(&["files"]))
            .expect_err("unexplained mover");
        assert!(matches!(err, Error::Torn(_)), "got {err:?}");
    }

    #[test]
    fn classify_two_claimants_fail_stops_at_the_multi_store_limit() {
        let err =
            classify_trailing(7, &ids(&["a", "b"]), &ids(&["a", "b"])).expect_err("multi-store");
        match err {
            Error::Torn(msg) => assert!(msg.contains("multi-store"), "{msg}"),
            other => panic!("expected Torn, got {other:?}"),
        }
    }
}
