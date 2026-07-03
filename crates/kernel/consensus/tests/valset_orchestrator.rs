use consensus::{ObservationOutcome, ValsetOrchestrator};

fn members(list: &[&'static str]) -> Vec<&'static str> {
    list.to_vec()
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
            .respawn_if_due(12, members(&["a", "b", "c", "d"]))
            .is_none()
    );
    assert_eq!(orchestrator.epoch(), 0);

    let respawn = orchestrator
        .respawn_if_due(13, members(&["a", "b", "c", "d"]))
        .expect("cutover view should trigger the epoch respawn");
    assert_eq!(respawn.epoch(), 1);
    assert_eq!(respawn.epoch_base(), 13);
    assert_eq!(orchestrator.epoch(), 1);
    assert_eq!(orchestrator.current_members().len(), 4);
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
        .respawn_if_due(13, members(&["a", "b", "c", "d", "e"]))
        .expect("due at the armed boundary");
    assert_eq!(respawn.epoch(), 1);
    assert_eq!(
        respawn.valset().consensus_members().len(),
        5,
        "boundary set includes both joins"
    );
    assert_eq!(orchestrator.current_members().len(), 5);

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
        .respawn_if_due(10, members(&["a", "b", "c", "d"]))
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
                .respawn_if_due(view, members(&["a", "b", "c"]))
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
        .respawn_if_due(13, members(&["a", "b", "c", "d"]))
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
