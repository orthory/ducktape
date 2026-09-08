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

// ---- the every-action grant and the every-id caps ------------------------------

/// `*` in an action list is the whole vocabulary, including the actions the
/// vocabulary gains later: it normalizes to exactly `["*"]`, it grants every
/// known action, and a call scopes it to the other side's list.
#[test]
fn the_every_action_grant_names_every_known_action_and_scopes_to_the_peer() {
    let stored = RunsModule::validate_actions(vec![ACTION_CHAT_POST.into(), EVERY.into()]).unwrap();
    assert_eq!(stored, vec![EVERY.to_string()]);
    let refused = RunsModule::validate_actions(vec!["chat.everything".into()]).unwrap_err();
    assert!(matches!(refused, Error::Module(reason) if reason.contains("unknown action")));

    let everything = record("bot", &[EVERY]);
    for action in crate::KNOWN_ACTIONS {
        assert!(everything.allows(action), "{action}");
    }
    assert!(!record("bot", &[ACTION_CHAT_POST]).allows(crate::ACTION_PAGES_POST));

    let narrow = record("peer", &[ACTION_CHAT_POST]);
    assert_eq!(
        everything.scoped_for_call(&narrow).allowed_actions,
        vec![ACTION_CHAT_POST.to_string()],
        "an every-action caller lends the callee exactly the callee's grant"
    );
    assert_eq!(
        narrow.scoped_for_call(&everything).allowed_actions,
        vec![ACTION_CHAT_POST.to_string()],
        "an every-action callee runs under the caller's grant"
    );
    assert_eq!(
        everything.scoped_for_call(&everything).allowed_actions,
        vec![EVERY.to_string()]
    );
}

/// `*` in a forge or pages cap list grants every id, push implies read, and a
/// call narrows `*` to the other side's list.
#[test]
fn the_every_id_cap_grants_every_repo_and_page_and_narrows_to_the_peer() {
    let mut pusher = record("bot", &[]);
    pusher.caps.forge_push = vec![EVERY.into()];
    assert!(pusher.permits(&CapRequest::ForgePush("anything")));
    assert!(pusher.permits(&CapRequest::ForgeRead("anything")));
    let mut reader = record("bot", &[]);
    reader.caps.forge_read = vec![EVERY.into()];
    assert!(reader.permits(&CapRequest::ForgeRead("anything")));
    assert!(!reader.permits(&CapRequest::ForgePush("anything")));
    let mut pages = record("bot", &[]);
    pages.caps.pages_write = vec![EVERY.into()];
    assert!(pages.permits(&CapRequest::PagesWrite("agent/abc/page/0")));

    let listed = crate::ResourceCaps {
        forge_read: vec!["docs".into()],
        forge_push: vec!["app".into(), "lib".into()],
        pages_write: vec!["p1".into()],
        ..Default::default()
    };
    let narrowed = pusher.caps.intersection(&listed);
    assert_eq!(narrowed.forge_push, vec!["app".to_string(), "lib".to_string()]);
    assert_eq!(
        narrowed.forge_read,
        vec!["app".to_string(), "docs".to_string(), "lib".to_string()],
        "push implies read on both sides of the narrowing"
    );
    let both = pusher.caps.intersection(&pusher.caps);
    assert_eq!(both.forge_push, vec![EVERY.to_string()]);
}
