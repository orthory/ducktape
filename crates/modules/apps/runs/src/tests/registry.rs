use super::*;

// ---- model configuration and recipe updates --------------------------------

#[test]
fn a_model_registration_registers_the_dispatch_recipe() {
    let mut m = module();
    let mut ctx = CaptureCtx::new().with_agent_origin();
    m.apply_model_change(
        &mut ctx,
        ModelChange::Registered {
            agent_id: "bot".into(),
            capability: "model-1".into(),
        },
    )
    .unwrap();

    let recipes = ctx.dispatch_msgs();
    assert_eq!(recipes.len(), 1);
    let DispatchMsg::RegisterRecipe {
        recipe_id,
        capability,
        routing,
        output_contract,
        max_attempts,
        deadline_views,
        lease_views,
        ..
    } = &recipes[0]
    else {
        panic!("expected a recipe registration");
    };
    assert_eq!(*recipe_id, recipe_id_for("bot"));
    assert_eq!(*capability, "model-1");
    assert_eq!(*routing, Routing::Rendezvous);
    assert_eq!(
        *output_contract,
        OutputContract::Text,
        "raw model text back; THIS module normalizes"
    );
    assert_eq!(*max_attempts, RUN_MAX_ATTEMPTS);
    assert_eq!(*deadline_views, Some(RUN_DEADLINE_VIEWS));
    assert_eq!(*lease_views, Some(RUN_LEASE_VIEWS));
}

#[test]
fn a_capability_change_event_retunes_the_dispatch_recipe() {
    let mut m = module();
    let mut ctx = CaptureCtx::new().with_agent_origin();
    m.apply_model_change(
        &mut ctx,
        ModelChange::CapabilityChanged {
            agent_id: "bot".into(),
            capability: "model-2".into(),
        },
    )
    .unwrap();
    assert_eq!(
        ctx.dispatch_msgs(),
        vec![DispatchMsg::UpdateRecipe {
            recipe_id: recipe_id_for("bot"),
            description: None,
            capability: Some("model-2".into()),
            routing: None,
            output_contract: None,
            max_attempts: None,
        }]
    );
}

#[test]
fn a_model_removal_removes_the_dispatch_recipe() {
    let mut m = module();
    let mut ctx = CaptureCtx::new().with_agent_origin();
    m.apply_model_change(
        &mut ctx,
        ModelChange::Deregistered {
            agent_id: "bot".into(),
        },
    )
    .unwrap();
    assert_eq!(
        ctx.dispatch_msgs(),
        vec![DispatchMsg::RemoveRecipe {
            recipe_id: recipe_id_for("bot"),
        }]
    );
}

#[test]
fn the_model_recipe_update_may_error_to_abort_the_registration_block() {
    let mut m = module();

    // an agent id whose recipe id would blow the dispatch id cap: the
    // hook ERRORS, aborting the registration block — the atomic recipe
    // seam (the registry record must never land without its recipe).
    let oversized = "x".repeat(dispatch::MAX_ID_BYTES);
    let mut ctx = CaptureCtx::new().with_agent_origin();
    let err = m
        .apply_model_change(
            &mut ctx,
            ModelChange::Registered {
                agent_id: oversized,
                capability: "model-1".into(),
            },
        )
        .unwrap_err();
    assert!(matches!(err, Error::Module(reason) if reason.contains("recipe id")));

    // malformed bytes from the registry origin error the same way — the
    // registry is genesis-trusted code, so this is a bug, not traffic.
    let mut ctx = CaptureCtx::new().with_agent_origin();
    let err = exec(
        &mut m,
        &mut ctx,
        &Msg {
            target: "runs".into(),
            payload: b"not an agent event".to_vec(),
        },
    )
    .unwrap_err();
    assert!(matches!(err, Error::Module(_)));
}
