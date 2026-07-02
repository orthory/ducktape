use consensus::{ObservationOutcome, ObservedValset, ValsetOrchestrator, ValsetRoot};

fn root(byte: u8) -> ValsetRoot {
    ValsetRoot([byte; 32])
}

fn observed(byte: u8, members: &[&'static str]) -> ObservedValset<&'static str> {
    ObservedValset::from_validator_set(root(byte), members.iter().copied())
}

#[test]
fn respawn_waits_until_cutover_view() {
    let mut orchestrator = ValsetOrchestrator::new(3, observed(1, &["a", "b", "c"]));

    let outcome = orchestrator.observe_finalized_valset(10, observed(2, &["a", "b", "c", "d"]));
    let cutover = match outcome {
        ObservationOutcome::Scheduled(cutover) => cutover,
        other => panic!("expected scheduled cutover, got {other:?}"),
    };
    assert_eq!(cutover.cutover_view(), 13);

    assert!(orchestrator.respawn_if_due(12).is_none());
    assert_eq!(orchestrator.epoch(), 0);

    let respawn = orchestrator
        .respawn_if_due(13)
        .expect("cutover view should trigger the epoch respawn");
    assert_eq!(respawn.epoch(), 1);
    assert_eq!(respawn.epoch_base(), 13);
    assert_eq!(orchestrator.epoch(), 1);
}

#[test]
fn membership_change_schedules_one_cutover() {
    let mut orchestrator = ValsetOrchestrator::new(2, observed(1, &["a", "b", "c"]));
    let next = observed(2, &["a", "b", "c", "d"]);

    let first = orchestrator.observe_finalized_valset(7, next.clone());
    let cutover = match first {
        ObservationOutcome::Scheduled(cutover) => cutover,
        other => panic!("expected scheduled cutover, got {other:?}"),
    };
    assert_eq!(cutover.observed_view(), 7);
    assert_eq!(cutover.cutover_view(), 9);
    assert_eq!(cutover.next_valset(), &next);
    assert_eq!(
        cutover.next_valset().membership().transport_members(),
        cutover.next_valset().membership().consensus_members(),
        "epoch transport membership follows the validator set"
    );

    let second = orchestrator.observe_finalized_valset(8, next);
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

#[test]
fn app_height_continues_across_respawn() {
    let mut orchestrator = ValsetOrchestrator::new(2, observed(1, &["a", "b", "c"]));

    assert_eq!(orchestrator.app_height(8), 8);
    orchestrator.observe_finalized_valset(8, observed(2, &["a", "b", "c", "d"]));
    assert_eq!(orchestrator.app_height(9), 9);

    let respawn = orchestrator
        .respawn_if_due(10)
        .expect("cutover view should trigger the epoch respawn");
    assert_eq!(respawn.cutover_app_height(), 10);
    assert_eq!(orchestrator.epoch_base(), 10);
    assert_eq!(orchestrator.app_height(1), 11);
}

#[test]
fn unchanged_valset_does_not_churn_epochs() {
    let current = observed(1, &["a", "b", "c"]);
    let mut orchestrator = ValsetOrchestrator::new(2, current.clone());

    for view in 1..=5 {
        let observed = if view % 2 == 0 {
            observed(1, &["c", "b", "a"])
        } else {
            current.clone()
        };
        let outcome = orchestrator.observe_finalized_valset(view, observed);
        assert_eq!(outcome, ObservationOutcome::Unchanged);
        assert!(orchestrator.respawn_if_due(view).is_none());
    }

    assert_eq!(orchestrator.epoch(), 0);
    assert!(orchestrator.pending_cutover().is_none());
}
