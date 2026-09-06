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
            demands: Default::default(),
            skills: Vec::new(),
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
fn request_run_is_gated_on_the_submitters_chat_standing() {
    // #1630: an external submitter who is not a member of a members-only
    // channel must not be able to have an agent pin its transcript and post
    // into it under module authority.
    let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
    let mut m = watched(TurnPolicy::Mention, &registry);
    let request = admin(&RunsMsg::RequestRun {
        agent_id: "bot".into(),
        channel_id: "board".into(),
        anchor_seq: 3,
        demands: Default::default(),
        skills: Vec::new(),
    });

    // mallory (user 1) is not a member of "board".
    let mut ctx = CaptureCtx::new()
        .with_origin(user(1))
        .with_registry(&registry)
        .with_transcript("board", transcript(3))
        .with_members_only("board", vec![2; 32]);
    let err = exec(&mut m, &mut ctx, &request).unwrap_err();
    assert!(matches!(err, Error::Module(_)));
    assert!(ctx.dispatch_msgs().is_empty());
    abort(&mut m);

    // a member (user 2) is admitted and the run is staged normally.
    let mut ctx = CaptureCtx::new()
        .with_origin(user(2))
        .with_registry(&registry)
        .with_transcript("board", transcript(3))
        .with_members_only("board", vec![2; 32]);
    exec(&mut m, &mut ctx, &request).unwrap();
    assert_eq!(ctx.dispatch_msgs().len(), 1);
    commit(&mut m);
}

#[test]
fn request_run_threads_demands_into_the_dispatch_emit() {
    // an explicit RequestRun's demands ride verbatim onto the emitted
    // DispatchMsg::Dispatch — the only demand surface in this phase.
    let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
    let mut m = watched(TurnPolicy::Mention, &registry);
    let demands = BTreeMap::from([("cores".to_string(), 4u64)]);
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
            demands: demands.clone(),
            skills: Vec::new(),
        }),
    )
    .unwrap();
    let dispatches = ctx.dispatch_msgs();
    assert_eq!(dispatches.len(), 1);
    let DispatchMsg::Dispatch {
        demands: captured, ..
    } = &dispatches[0]
    else {
        panic!("expected a Dispatch");
    };
    assert_eq!(captured, &demands);
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
            demands: Default::default(),
            skills: Vec::new(),
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

// ---- per-run skill curation on the operator RequestRun path -------------------

/// the skill names the run's composed envelope carries, in order.
fn dispatched_skills(ctx: &CaptureCtx) -> Vec<String> {
    let DispatchMsg::Dispatch { payload, .. } = ctx
        .dispatch_msgs()
        .into_iter()
        .next()
        .expect("a run was dispatched")
    else {
        panic!("expected a Dispatch");
    };
    let v: serde_json::Value = serde_json::from_slice(&payload).expect("envelope is JSON");
    v["skills"]
        .as_array()
        .expect("skills array")
        .iter()
        .map(|s| s["name"].as_str().expect("skill name").to_string())
        .collect()
}

fn request_with_skills(agent: &str, seq: u64, skills: &[&str]) -> Msg {
    admin(&RunsMsg::RequestRun {
        agent_id: agent.into(),
        channel_id: "general".into(),
        anchor_seq: seq,
        demands: Default::default(),
        skills: skills.iter().map(|s| s.to_string()).collect(),
    })
}

#[test]
fn request_skills_are_curated_onto_the_agents_own() {
    let mut registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
    registry.get_mut("bot").unwrap().skills = vec![agent::SkillRef {
        name: "persona".into(),
        source_prefix: "/shared/skills/persona".into(),
        source_snapshot: None,
        load: agent::LoadMode::Always,
    }];
    let mut m = watched(TurnPolicy::Mention, &registry);
    let mut ctx = CaptureCtx::new()
        .with_origin(user(1))
        .with_registry(&registry)
        .with_transcript("general", transcript(3));
    exec(
        &mut m,
        &mut ctx,
        &request_with_skills("bot", 3, &["rust-gates"]),
    )
    .unwrap();
    assert_eq!(
        dispatched_skills(&ctx),
        ["persona", "rust-gates"],
        "the agent keeps its persona and gains what the request curated"
    );
}

#[test]
fn a_request_naming_a_non_library_skill_is_refused() {
    // the request carries NAMES, not paths — so a name that is not a single
    // library entry (a traversal, a slash, empty) cannot resolve to an
    // arbitrary duckfs subtree. this is the trust boundary: the ro-mount reads
    // on node authority with no cap gate, so a requester must never name a path.
    let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
    let mut m = watched(TurnPolicy::Mention, &registry);
    for bad in ["../agents/victim/persona", "a/b", ".."] {
        let mut ctx = CaptureCtx::new()
            .with_origin(user(1))
            .with_registry(&registry)
            .with_transcript("general", transcript(3));
        let err = exec(&mut m, &mut ctx, &request_with_skills("bot", 3, &[bad])).unwrap_err();
        assert!(
            matches!(&err, Error::Module(reason)
                if reason.contains("single shared-library entry") || reason.contains("dot segment")),
            "{bad:?} must be refused: {err:?}"
        );
        abort(&mut m);
    }
}

#[test]
fn request_skills_are_not_part_of_a_runs_identity() {
    // the turn is claimed on (channel, anchor, agent). a second request at the
    // same anchor with a DIFFERENT curation is the same turn — first wins, the
    // second no-ops. re-running with different skills means a new anchor.
    let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
    let mut m = watched(TurnPolicy::Mention, &registry);
    let mut ctx = CaptureCtx::new()
        .with_origin(user(1))
        .with_registry(&registry)
        .with_transcript("general", transcript(3));
    exec(&mut m, &mut ctx, &request_with_skills("bot", 3, &["first"])).unwrap();
    assert_eq!(dispatched_skills(&ctx), ["first"]);
    commit(&mut m);

    let mut ctx = CaptureCtx::new()
        .with_origin(user(1))
        .with_registry(&registry)
        .with_transcript("general", transcript(3));
    exec(
        &mut m,
        &mut ctx,
        &request_with_skills("bot", 3, &["second"]),
    )
    .unwrap();
    assert!(
        ctx.dispatch_msgs().is_empty(),
        "the turn was already claimed; a new curation does not mint a new one"
    );
}
