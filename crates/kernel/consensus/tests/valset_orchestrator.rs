use std::collections::BTreeSet;

use consensus::{
    BoundaryUpgrade, ObservationOutcome, PendingUpgrade, UpgradeVerdict, ValsetOrchestrator,
};

fn members(list: &[&'static str]) -> Vec<&'static str> {
    list.to_vec()
}

/// the boundary-upgrade a membership-only respawn (or a node without the upgrade
/// module) passes: no pending upgrade, current version `1`.
fn no_pending() -> BoundaryUpgrade<&'static str> {
    BoundaryUpgrade::baseline(1)
}

fn ready_set(list: &[&'static str]) -> BTreeSet<&'static str> {
    list.iter().copied().collect()
}

fn pending_upgrade(
    current: u32,
    name: &str,
    activation_height: u64,
    to_version: u32,
    ready: &[&'static str],
) -> BoundaryUpgrade<&'static str> {
    BoundaryUpgrade {
        current_version: current,
        pending: Some(PendingUpgrade {
            name: name.to_string(),
            activation_height,
            to_version,
            ready: ready_set(ready),
        }),
    }
}

#[test]
fn respawn_waits_until_cutover_view() {
    let mut orchestrator = ValsetOrchestrator::new(3, members(&["a", "b", "c"]));

    let outcome = orchestrator.observe_members(10, members(&["a", "b", "c", "d"]));
    let cutover = match outcome {
        ObservationOutcome::Scheduled(cutover) => cutover,
        other => panic!("expected scheduled cutover, got {other:?}"),
    };
    assert_eq!(cutover.cutover_view(), 13);

    assert!(
        orchestrator
            .respawn_if_due(12, members(&["a", "b", "c", "d"]), no_pending())
            .is_none()
    );
    assert_eq!(orchestrator.epoch(), 0);

    let respawn = orchestrator
        .respawn_if_due(13, members(&["a", "b", "c", "d"]), no_pending())
        .expect("cutover view should trigger the epoch respawn");
    assert_eq!(respawn.epoch(), 1);
    assert_eq!(respawn.epoch_base(), 13);
    assert_eq!(orchestrator.epoch(), 1);
    assert_eq!(orchestrator.current_members().len(), 4);
    // pure-membership boundary: the version path is byte-unchanged.
    assert_eq!(respawn.boundary_version(), 1);
    assert_eq!(respawn.upgrade_verdict(), &UpgradeVerdict::None);
}

#[test]
fn membership_change_schedules_one_cutover() {
    let mut orchestrator = ValsetOrchestrator::new(2, members(&["a", "b", "c"]));

    let first = orchestrator.observe_members(7, members(&["a", "b", "c", "d"]));
    let cutover = match first {
        ObservationOutcome::Scheduled(cutover) => cutover,
        other => panic!("expected scheduled cutover, got {other:?}"),
    };
    assert_eq!(cutover.observed_view(), 7);
    assert_eq!(cutover.cutover_view(), 9);

    let second = orchestrator.observe_members(8, members(&["a", "b", "c", "d"]));
    let pending = match second {
        ObservationOutcome::Pending(cutover) => cutover,
        other => panic!("expected existing pending cutover, got {other:?}"),
    };
    assert_eq!(pending.observed_view(), 7);
    assert_eq!(pending.cutover_view(), 9);
    assert_eq!(
        orchestrator
            .pending_cutover()
            .expect("pending")
            .observed_view(),
        7
    );
}

/// the boundary read decides the next set: a SECOND change landing inside
/// the cutover window neither moves the armed boundary nor needs its own
/// epoch — the set read at the boundary already includes it, identically on
/// every node (the discard ceiling froze state there).
#[test]
fn boundary_read_absorbs_a_second_change_inside_the_window() {
    let mut orchestrator = ValsetOrchestrator::new(3, members(&["a", "b", "c"]));

    let armed = orchestrator.observe_members(10, members(&["a", "b", "c", "d"]));
    assert!(matches!(armed, ObservationOutcome::Scheduled(_)));

    // view 11: "e" joins too. the boundary stays 13.
    let second = orchestrator.observe_members(11, members(&["a", "b", "c", "d", "e"]));
    let pending = match second {
        ObservationOutcome::Pending(cutover) => cutover,
        other => panic!("expected pending, got {other:?}"),
    };
    assert_eq!(pending.cutover_view(), 13);

    let respawn = orchestrator
        .respawn_if_due(13, members(&["a", "b", "c", "d", "e"]), no_pending())
        .expect("due at the armed boundary");
    assert_eq!(respawn.epoch(), 1);
    assert_eq!(
        respawn.valset().consensus_members().len(),
        5,
        "boundary set includes both joins"
    );
    assert_eq!(orchestrator.current_members().len(), 5);
    assert_eq!(respawn.boundary_version(), 1);
    assert_eq!(respawn.upgrade_verdict(), &UpgradeVerdict::None);

    // nothing further pending: the next identical observation is Unchanged.
    assert_eq!(
        orchestrator.observe_members(14, members(&["a", "b", "c", "d", "e"])),
        ObservationOutcome::Unchanged
    );
}

#[test]
fn app_height_continues_across_respawn() {
    let mut orchestrator = ValsetOrchestrator::new(2, members(&["a", "b", "c"]));

    assert_eq!(orchestrator.app_height(8), 8);
    orchestrator.observe_members(8, members(&["a", "b", "c", "d"]));
    assert_eq!(orchestrator.app_height(9), 9);

    let respawn = orchestrator
        .respawn_if_due(10, members(&["a", "b", "c", "d"]), no_pending())
        .expect("cutover view should trigger the epoch respawn");
    assert_eq!(respawn.cutover_app_height(), 10);
    assert_eq!(orchestrator.epoch_base(), 10);
    assert_eq!(orchestrator.app_height(1), 11);
}

#[test]
fn unchanged_valset_does_not_churn_epochs() {
    let mut orchestrator = ValsetOrchestrator::new(2, members(&["a", "b", "c"]));

    for view in 1..=5 {
        // order never matters: membership is a set.
        let observed = if view % 2 == 0 {
            members(&["c", "b", "a"])
        } else {
            members(&["a", "b", "c"])
        };
        let outcome = orchestrator.observe_members(view, observed);
        assert_eq!(outcome, ObservationOutcome::Unchanged);
        assert!(
            orchestrator
                .respawn_if_due(view, members(&["a", "b", "c"]), no_pending())
                .is_none()
        );
    }

    assert_eq!(orchestrator.epoch(), 0);
    assert!(orchestrator.pending_cutover().is_none());
}

/// a node that crashed mid-window resumes with the recorded pending boundary
/// and crosses it exactly like its uninterrupted peers.
#[test]
fn resume_rearms_a_pending_cutover() {
    // pre-crash: epoch 2 based at 100, spawn set {a,b,c}, a join observed at
    // view 10 armed a cutover at view 13 — all recorded, then the crash.
    let mut orchestrator =
        ValsetOrchestrator::resume(3, members(&["a", "b", "c"]), 2, 100, Some(13));

    assert_eq!(orchestrator.epoch(), 2);
    assert_eq!(orchestrator.epoch_base(), 100);
    let pending = orchestrator.pending_cutover().expect("re-armed");
    assert_eq!(pending.cutover_view(), 13);
    assert_eq!(pending.observed_view(), 10);
    assert_eq!(pending.next_epoch(), 3);

    // a further observation of the changed set stays Pending — the armed
    // boundary never moves.
    let outcome = orchestrator.observe_members(12, members(&["a", "b", "c", "d"]));
    assert!(matches!(outcome, ObservationOutcome::Pending(_)));

    let respawn = orchestrator
        .respawn_if_due(13, members(&["a", "b", "c", "d"]), no_pending())
        .expect("resumed boundary triggers");
    assert_eq!(respawn.epoch(), 3);
    assert_eq!(respawn.epoch_base(), 113, "base 100 + cutover view 13");
    assert_eq!(respawn.valset().consensus_members().len(), 4);
    assert_eq!(
        respawn.valset().transport_members(),
        respawn.valset().consensus_members(),
        "epoch transport membership follows the validator set"
    );
}

// ============================================================================
// Phase 6 — deterministic activation over the existing boundary.
// ============================================================================

/// `observe_upgrade` arms the single shared slot at `H - epoch_base`; a second
/// observation returns `Pending` (the armed boundary never moves).
#[test]
fn version_flip_arms_cutover_at_activation_height() {
    let mut orchestrator = ValsetOrchestrator::new(3, members(&["a", "b", "c"]));

    // activation app-height 10, epoch_base 0 -> cutover_view 10, strictly future.
    let outcome = orchestrator.observe_upgrade(6, 10);
    let cutover = match outcome {
        ObservationOutcome::Scheduled(c) => c,
        other => panic!("expected scheduled version cutover, got {other:?}"),
    };
    assert_eq!(cutover.cutover_view(), 10);
    assert_eq!(cutover.next_epoch(), 1);

    // a second observation of the same pending returns Pending — one slot only.
    let again = orchestrator.observe_upgrade(7, 10);
    assert!(matches!(again, ObservationOutcome::Pending(_)));
}

/// at the boundary, `R == n` (every boundary member ready) and `H` reached ⇒ the
/// plan carries `boundary_version == to_version` with verdict `Armed`.
#[test]
fn boundary_read_flips_when_r_equals_n() {
    let mut orchestrator = ValsetOrchestrator::new(3, members(&["a", "b", "c"]));
    orchestrator.observe_upgrade(6, 10);

    let boundary = pending_upgrade(1, "forge-multi-repo", 10, 2, &["a", "b", "c"]);
    let respawn = orchestrator
        .respawn_if_due(10, members(&["a", "b", "c"]), boundary)
        .expect("version boundary crosses");
    assert_eq!(respawn.boundary_version(), 2, "R==n flips to to_version");
    assert_eq!(
        respawn.upgrade_verdict(),
        &UpgradeVerdict::Armed {
            name: "forge-multi-repo".into(),
            to_version: 2
        }
    );
    // the epoch still rotates (the boundary is a real teardown-respawn).
    assert_eq!(respawn.epoch(), 1);
}

/// one boundary member never signaled ⇒ `R < n` ⇒ the version stays current and
/// the verdict is a clean `Abort`.
#[test]
fn straggler_aborts_upgrade_cleanly() {
    let mut orchestrator = ValsetOrchestrator::new(3, members(&["a", "b", "c"]));
    orchestrator.observe_upgrade(6, 10);

    // only a, b signaled — c is a straggler.
    let boundary = pending_upgrade(1, "forge-multi-repo", 10, 2, &["a", "b"]);
    let respawn = orchestrator
        .respawn_if_due(10, members(&["a", "b", "c"]), boundary)
        .expect("boundary still crosses (the membership/epoch boundary is real)");
    assert_eq!(
        respawn.boundary_version(),
        1,
        "abort leaves the version unchanged"
    );
    assert_eq!(
        respawn.upgrade_verdict(),
        &UpgradeVerdict::Abort {
            name: "forge-multi-repo".into()
        }
    );
}

/// readiness is measured against the BOUNDARY valset: a non-member ready signal
/// is dead weight (a real member missing still aborts); and dead keys alongside a
/// full boundary set still arm.
#[test]
fn non_member_ready_signals_are_dead_weight() {
    // extra non-member "z" ready but real member "c" missing ⇒ Abort.
    let mut o1 = ValsetOrchestrator::new(3, members(&["a", "b", "c"]));
    o1.observe_upgrade(6, 10);
    let boundary = pending_upgrade(1, "n", 10, 2, &["a", "b", "z"]);
    let respawn = o1
        .respawn_if_due(10, members(&["a", "b", "c"]), boundary)
        .expect("crosses");
    assert_eq!(respawn.boundary_version(), 1);
    assert_eq!(
        respawn.upgrade_verdict(),
        &UpgradeVerdict::Abort { name: "n".into() }
    );

    // every boundary member ready + extra dead keys ⇒ Armed.
    let mut o2 = ValsetOrchestrator::new(3, members(&["a", "b", "c"]));
    o2.observe_upgrade(6, 10);
    let boundary = pending_upgrade(1, "n", 10, 2, &["a", "b", "c", "z"]);
    let respawn = o2
        .respawn_if_due(10, members(&["a", "b", "c"]), boundary)
        .expect("crosses");
    assert_eq!(respawn.boundary_version(), 2, "dead keys don't block an armed set");
    assert_eq!(
        respawn.upgrade_verdict(),
        &UpgradeVerdict::Armed {
            name: "n".into(),
            to_version: 2
        }
    );
}

/// a membership change and a pending-upgrade-at-`H` in the same window are carried
/// by ONE respawn: the plan has the new valset AND `boundary_version == to_version`.
#[test]
fn coincident_membership_and_version_share_one_respawn() {
    let mut orchestrator = ValsetOrchestrator::new(3, members(&["a", "b", "c"]));

    // a membership change arms the single slot at view 13 (delay 3).
    let armed = orchestrator.observe_members(10, members(&["a", "b", "c", "d"]));
    assert!(matches!(armed, ObservationOutcome::Scheduled(_)));

    // a pending upgrade lands in the same window — it does NOT arm a competing
    // cutover; it rides the membership boundary via the boundary read.
    assert!(matches!(
        orchestrator.observe_upgrade(11, 13),
        ObservationOutcome::Pending(_)
    ));

    // at the boundary the set is {a,b,c,d} and all four signaled ⇒ one plan
    // carries the new valset AND the version flip.
    let boundary = pending_upgrade(1, "forge-multi-repo", 13, 2, &["a", "b", "c", "d"]);
    let respawn = orchestrator
        .respawn_if_due(13, members(&["a", "b", "c", "d"]), boundary)
        .expect("coincident boundary crosses");
    assert_eq!(respawn.valset().consensus_members().len(), 4, "new valset");
    assert_eq!(respawn.boundary_version(), 2, "and the version flip");
    assert_eq!(
        respawn.upgrade_verdict(),
        &UpgradeVerdict::Armed {
            name: "forge-multi-repo".into(),
            to_version: 2
        }
    );
}

/// a version cutover armed first; a membership change inside the window returns
/// `Pending`; `respawn_if_due` then reads the NEW members AND flips in one plan.
#[test]
fn version_cutover_absorbs_membership_change_inside_window() {
    let mut orchestrator = ValsetOrchestrator::new(3, members(&["a", "b", "c"]));

    // a version cutover arms the slot at view 13.
    let scheduled = orchestrator.observe_upgrade(10, 13);
    assert!(matches!(scheduled, ObservationOutcome::Scheduled(_)));

    // a membership change inside the window rides the same boundary.
    let absorbed = orchestrator.observe_members(11, members(&["a", "b", "c", "d"]));
    assert!(matches!(absorbed, ObservationOutcome::Pending(_)));

    // boundary read: new members {a,b,c,d}, all ready ⇒ new valset + version flip.
    let boundary = pending_upgrade(1, "n", 13, 2, &["a", "b", "c", "d"]);
    let respawn = orchestrator
        .respawn_if_due(13, members(&["a", "b", "c", "d"]), boundary)
        .expect("crosses");
    assert_eq!(respawn.valset().consensus_members().len(), 4);
    assert_eq!(respawn.boundary_version(), 2);
    assert_eq!(
        respawn.upgrade_verdict(),
        &UpgradeVerdict::Armed {
            name: "n".into(),
            to_version: 2
        }
    );
}

/// a membership boundary that fires at an app-height BELOW the pending upgrade's
/// activation height yields verdict `None` — the version does not flip early
/// (`effective_version` is pure below `H`).
#[test]
fn effective_version_pure_below_h() {
    let mut orchestrator = ValsetOrchestrator::new(3, members(&["a", "b", "c"]));
    orchestrator.observe_members(10, members(&["a", "b", "c", "d"])); // cutover at 13

    // pending activation is 20 — the membership boundary at app-height 13 is below it.
    let boundary = pending_upgrade(1, "n", 20, 2, &["a", "b", "c", "d"]);
    let respawn = orchestrator
        .respawn_if_due(13, members(&["a", "b", "c", "d"]), boundary)
        .expect("membership boundary crosses");
    assert_eq!(respawn.cutover_app_height(), 13);
    assert_eq!(
        respawn.boundary_version(),
        1,
        "version does not flip below H"
    );
    assert_eq!(respawn.upgrade_verdict(), &UpgradeVerdict::None);
}

/// the single slot is consumed EXACTLY once: a second `respawn_if_due` at the same
/// boundary returns `None`.
#[test]
fn abort_verdict_evaluated_exactly_once() {
    let mut orchestrator = ValsetOrchestrator::new(3, members(&["a", "b", "c"]));
    orchestrator.observe_upgrade(6, 10);

    let boundary = pending_upgrade(1, "n", 10, 2, &["a", "b"]); // straggler -> Abort
    let first = orchestrator.respawn_if_due(10, members(&["a", "b", "c"]), boundary.clone());
    assert!(first.is_some());
    // the slot is consumed — a second call yields nothing (no double-evaluation).
    let second = orchestrator.respawn_if_due(10, members(&["a", "b", "c"]), boundary);
    assert!(second.is_none());
}

/// `resume` re-arms a version-scheduled cutover from recovered coordinates (the
/// shared single slot), and crossing it flips exactly like an uninterrupted peer.
#[test]
fn resume_rearms_pending_upgrade() {
    // recovered: epoch 2 based at 100, a version schedule at activation app-height
    // 113 -> shared-slot cutover_view 13 (113 - 100).
    let mut orchestrator =
        ValsetOrchestrator::resume(3, members(&["a", "b", "c"]), 2, 100, Some(13));
    assert_eq!(orchestrator.pending_cutover().expect("re-armed").cutover_view(), 13);

    let boundary = pending_upgrade(1, "n", 113, 2, &["a", "b", "c"]);
    let respawn = orchestrator
        .respawn_if_due(13, members(&["a", "b", "c"]), boundary)
        .expect("resumed version boundary triggers");
    assert_eq!(respawn.cutover_app_height(), 113, "base 100 + view 13");
    assert_eq!(respawn.boundary_version(), 2, "re-armed upgrade flips at the same H");
    assert_eq!(
        respawn.upgrade_verdict(),
        &UpgradeVerdict::Armed {
            name: "n".into(),
            to_version: 2
        }
    );
}

/// determinism: two orchestrators fed byte-identical inputs produce byte-identical
/// respawn plans (the boundary decision is a pure function of agreed state).
#[test]
fn boundary_decision_is_deterministic() {
    let build = || {
        let mut o = ValsetOrchestrator::new(3, members(&["a", "b", "c"]));
        o.observe_members(10, members(&["a", "b", "c", "d"]));
        o.respawn_if_due(
            13,
            members(&["a", "b", "c", "d"]),
            pending_upgrade(1, "forge-multi-repo", 13, 2, &["a", "b", "c", "d"]),
        )
        .expect("crosses")
    };
    let plan_a = build();
    let plan_b = build();
    assert_eq!(plan_a, plan_b, "same inputs -> same boundary plan");
    assert_eq!(plan_a.boundary_version(), 2);
    assert_eq!(
        plan_a.upgrade_verdict(),
        &UpgradeVerdict::Armed {
            name: "forge-multi-repo".into(),
            to_version: 2
        }
    );
}
