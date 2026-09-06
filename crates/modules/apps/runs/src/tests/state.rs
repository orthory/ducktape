use super::*;

#[test]
fn model_run_session_and_request_state_round_trip() {
    let (mut module, registry, run_id) = awaiting_run(&[ACTION_TASKS_CREATE]);
    let mut open = CaptureCtx::new()
        .with_origin(user(8))
        .with_registry(&registry)
        .with_lease_holder(&run_id, &[8; 32]);
    exec(
        &mut module,
        &mut open,
        &admin(&RunsMsg::OpenAgentSession {
            run_id: run_id.clone(),
            session_key: vec![7; 32],
        }),
    )
    .unwrap();
    commit(&mut module);
    let mut act = CaptureCtx::new()
        .with_origin(user(7))
        .with_registry(&registry)
        .with_lease_holder(&run_id, &[8; 32]);
    exec(
        &mut module,
        &mut act,
        &admin(&RunsMsg::AgentAction {
            run_id,
            action: AgentAction::CreateTask {
                task_id: "persisted".into(),
                title: "a task".into(),
            },
        }),
    )
    .unwrap();
    commit(&mut module);
    let bytes = module.snapshot();
    let expected = module.root();
    let mut restored = super::module();
    restored.install(&bytes, expected).unwrap();
    assert_eq!(restored.snapshot(), bytes);
    assert_eq!(restored.root(), expected);
    assert_eq!(
        block_on(restored.action_deliveries()).unwrap(),
        block_on(module.action_deliveries()).unwrap()
    );
    let before = restored.snapshot();
    assert!(
        restored
            .install(&bytes[..bytes.len() - 1], expected)
            .is_err()
    );
    assert_eq!(restored.snapshot(), before);
}
