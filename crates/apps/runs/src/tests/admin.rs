use super::*;

// ---- watches -----------------------------------------------------------------

#[test]
fn watch_and_unwatch_stage_the_policy_and_emit_the_plane_subscription_atomically() {
    let registry = registry(&[]);
    let mut m = module();
    let mut ctx = CaptureCtx::new()
        .with_origin(user(9))
        .with_registry(&registry);
    exec(
        &mut m,
        &mut ctx,
        &admin(&RunsMsg::WatchChannel {
            channel_id: "general".into(),
            policy: TurnPolicy::Mention,
        }),
    )
    .unwrap();
    // the watch and the plane Subscribe follow-up are one atomic unit (P2).
    assert_eq!(
        ctx.tagging_msgs(),
        vec![TaggingMsg::Subscribe {
            source: "chat".into(),
            container: "general".into(),
        }]
    );
    commit(&mut m);

    // an Assigned policy must name a registered agent.
    let mut ctx = CaptureCtx::new()
        .with_origin(user(9))
        .with_registry(&registry);
    let err = exec(
        &mut m,
        &mut ctx,
        &admin(&RunsMsg::WatchChannel {
            channel_id: "other".into(),
            policy: TurnPolicy::Assigned("ghost".into()),
        }),
    )
    .unwrap_err();
    assert!(matches!(err, Error::Module(_)));
    abort(&mut m);

    // unwatch removes the watch and drops the plane subscription.
    let mut ctx = CaptureCtx::new().with_origin(user(9));
    exec(
        &mut m,
        &mut ctx,
        &admin(&RunsMsg::UnwatchChannel {
            channel_id: "general".into(),
        }),
    )
    .unwrap();
    assert_eq!(
        ctx.tagging_msgs(),
        vec![TaggingMsg::Unsubscribe {
            source: "chat".into(),
            container: "general".into(),
        }]
    );
    commit(&mut m);

    // unwatching an unwatched channel stages and emits NOTHING.
    let before = m.root();
    let mut ctx = CaptureCtx::new().with_origin(user(9));
    exec(
        &mut m,
        &mut ctx,
        &admin(&RunsMsg::UnwatchChannel {
            channel_id: "general".into(),
        }),
    )
    .unwrap();
    assert!(ctx.msgs.is_empty(), "an idempotent unwatch emits nothing");
    commit(&mut m);
    assert_eq!(m.root(), before);
}

#[test]
fn enable_job_worker_is_admin_gated_and_emits_self_registration() {
    let mut m = module();

    let mut intruder = CaptureCtx::new().with_origin(Origin::System);
    let err = exec(
        &mut m,
        &mut intruder,
        &admin(&RunsMsg::EnableJobWorker { enabled: true }),
    )
    .unwrap_err();
    assert!(matches!(err, Error::Module(_)));
    abort(&mut m);

    let mut ctx = CaptureCtx::new().with_origin(user(9));
    exec(
        &mut m,
        &mut ctx,
        &admin(&RunsMsg::EnableJobWorker { enabled: true }),
    )
    .unwrap();
    assert_eq!(ctx.job_msgs(), vec![JobsMsg::RegisterWorker {}]);
    commit(&mut m);

    let mut ctx = CaptureCtx::new().with_origin(user(9));
    exec(
        &mut m,
        &mut ctx,
        &admin(&RunsMsg::EnableJobWorker { enabled: false }),
    )
    .unwrap();
    assert_eq!(ctx.job_msgs(), vec![JobsMsg::UnregisterWorker {}]);
    commit(&mut m);

    let mut without_jobs = RunsModule::new(
        "runs",
        "chat",
        "saga",
        "tagging",
        "dispatch",
        "agent",
        Some("tasks".into()),
        None,
    );
    let mut ctx = CaptureCtx::new().with_origin(user(9));
    let err = exec(
        &mut without_jobs,
        &mut ctx,
        &admin(&RunsMsg::EnableJobWorker { enabled: true }),
    )
    .unwrap_err();
    assert!(matches!(err, Error::Module(m) if m.contains("jobs module")));
}

// ---- explicit runs + cancellation ------------------------------------------------

#[test]
fn request_run_validates_agent_origin_and_anchor() {
    let mut registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
    let mut m = watched(TurnPolicy::Mention, &registry);
    let request = |agent: &str, seq: u64| {
        admin(&RunsMsg::RequestRun {
            agent_id: agent.into(),
            channel_id: "general".into(),
            anchor_seq: seq,
        })
    };

    // unknown agent, empty origin, missing anchor, anchor 0: all errors.
    let mut ctx = CaptureCtx::new()
        .with_origin(user(1))
        .with_registry(&registry)
        .with_transcript("general", transcript(3));
    assert!(exec(&mut m, &mut ctx, &request("ghost", 3)).is_err());
    abort(&mut m);
    let mut ctx = CaptureCtx::new()
        .with_origin(Origin::External(Vec::new()))
        .with_registry(&registry)
        .with_transcript("general", transcript(3));
    assert!(exec(&mut m, &mut ctx, &request("bot", 3)).is_err());
    abort(&mut m);
    let mut ctx = CaptureCtx::new()
        .with_origin(user(1))
        .with_registry(&registry)
        .with_transcript("general", transcript(3));
    assert!(
        exec(&mut m, &mut ctx, &request("bot", 9)).is_err(),
        "an anchor past the head does not exist"
    );
    abort(&mut m);
    assert!(exec(&mut m, &mut ctx, &request("bot", 0)).is_err());
    abort(&mut m);

    // a paused agent cannot be explicitly run either.
    pause(&mut registry, "bot");
    let mut ctx = CaptureCtx::new()
        .with_origin(user(1))
        .with_registry(&registry)
        .with_transcript("general", transcript(3));
    assert!(exec(&mut m, &mut ctx, &request("bot", 3)).is_err());
    abort(&mut m);

    // resumed, the request lands: entry staged + dispatch emitted,
    // requester recorded as the submitting user.
    registry.get_mut("bot").unwrap().status = AgentStatus::Active;
    let mut ctx = CaptureCtx::new()
        .at(6)
        .with_origin(user(1))
        .with_registry(&registry)
        .with_transcript("general", transcript(3));
    exec(&mut m, &mut ctx, &request("bot", 3)).unwrap();
    assert_eq!(ctx.dispatch_msgs().len(), 1);
    commit(&mut m);
    let entry = get_pending(&m, &run_id_for("general", 3, "bot")).unwrap();
    assert_eq!(entry.requester, SagaOrigin::External(vec![1; 32]));
    assert_eq!(entry.created_at, 6);
}

#[test]
fn cancel_run_is_gated_to_the_requester_or_the_owner() {
    let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
    let mut m = watched(TurnPolicy::Mention, &registry);
    let mut ctx = CaptureCtx::new()
        .with_origin(user(1))
        .with_registry(&registry)
        .with_transcript("general", transcript(3));
    exec(
        &mut m,
        &mut ctx,
        &admin(&RunsMsg::RequestRun {
            agent_id: "bot".into(),
            channel_id: "general".into(),
            anchor_seq: 3,
        }),
    )
    .unwrap();
    commit(&mut m);
    let run_id = run_id_for("general", 3, "bot");
    let cancel = admin(&RunsMsg::CancelRun {
        run_id: run_id.clone(),
    });

    // a foreign origin (neither requester user(1) nor owner user(9)).
    let mut ctx = CaptureCtx::new()
        .with_origin(user(2))
        .with_registry(&registry);
    assert!(exec(&mut m, &mut ctx, &cancel).is_err());
    abort(&mut m);
    // an unknown run is an error too.
    let mut ctx = CaptureCtx::new()
        .with_origin(user(1))
        .with_registry(&registry);
    assert!(
        exec(
            &mut m,
            &mut ctx,
            &admin(&RunsMsg::CancelRun {
                run_id: "nope".into(),
            }),
        )
        .is_err()
    );
    abort(&mut m);

    let mut ctx = CaptureCtx::new()
        .with_origin(user(1))
        .with_registry(&registry);
    exec(
        &mut m,
        &mut ctx,
        &admin(&RunsMsg::ReassignRun {
            run_id: run_id.clone(),
            attempt: 0,
        }),
    )
    .unwrap();
    assert_eq!(
        ctx.dispatch_msgs(),
        vec![DispatchMsg::ReassignDispatch {
            dispatch_id: dispatch_id_for(&run_id),
            attempt: 0,
        }]
    );
    commit(&mut m);

    // the REQUESTER cancels: the dispatch plane is told; the entry STAYS
    // pending — the plane's Err("cancelled") delivery is the one result
    // path that prunes it.
    let mut ctx = CaptureCtx::new()
        .at(7)
        .with_origin(user(1))
        .with_registry(&registry);
    exec(&mut m, &mut ctx, &cancel).unwrap();
    assert_eq!(
        ctx.dispatch_msgs(),
        vec![DispatchMsg::CancelDispatch {
            dispatch_id: dispatch_id_for(&run_id),
        }]
    );
    commit(&mut m);
    assert!(get_pending(&m, &run_id).is_some(), "still pending delivery");

    // the plane's Err("cancelled") delivery prunes the entry. it rides
    // the ONE result path, so it surfaces like any failed run — a
    // threaded ⚠ reply, never silence.
    let mut ctx = CaptureCtx::new()
        .with_dispatch_origin()
        .with_registry(&registry);
    exec(
        &mut m,
        &mut ctx,
        &result_event(&run_id, Err("cancelled".into())),
    )
    .unwrap();
    assert_eq!(
        ctx.chat_msgs(),
        vec![ChatMsg::PostMessage {
            channel_id: "general".into(),
            message_id: reply_message_id(&run_id),
            blocks: vec![Block::paragraph("⚠ BOT failed: cancelled")],
            thread: None,
            as_agent: Some("bot".into()),
        }]
    );
    commit(&mut m);
    assert_eq!(get_pending(&m, &run_id), None);

    // cancelling the now-delivered run is an idempotent no-op (the
    // dispatch record proves it existed); a truly unknown one errors.
    let mut ctx = CaptureCtx::new()
        .with_origin(user(1))
        .with_registry(&registry)
        .with_taken_dispatch(&dispatch_id_for(&run_id));
    exec(&mut m, &mut ctx, &cancel).unwrap();
    assert!(ctx.msgs.is_empty());

    // the OWNER may cancel an engagement-created run (requester = the
    // tagging plane).
    engage_post(&mut m, &registry, 2, &["bot"]);
    commit(&mut m);
    let engaged_run = run_id_for("general", 2, "bot");
    let mut ctx = CaptureCtx::new()
        .with_origin(user(9))
        .with_registry(&registry);
    exec(
        &mut m,
        &mut ctx,
        &admin(&RunsMsg::CancelRun {
            run_id: engaged_run.clone(),
        }),
    )
    .unwrap();
    assert_eq!(ctx.dispatch_msgs().len(), 1);
}
