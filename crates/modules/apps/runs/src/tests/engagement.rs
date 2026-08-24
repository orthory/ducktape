use super::*;

#[test]
fn pages_comment_mention_dispatches_without_a_chat_watch() {
    let mut registry = registry(&[("bot", &[ACTION_PAGES_COMMENT])]);
    registry.get_mut("bot").unwrap().caps.pages_write = vec!["p1".into()];
    let mut m = module()
        .with_files_module("files")
        .with_pages_module("pages");
    let thread = pages::ThreadView {
        thread: pages::Thread {
            id: "thread-1".into(),
            target: "b-p".into(),
            opener: pages::AuthorRef::User(vec![4; 32]),
            created_at: 1,
            anchor: None,
            resolved: false,
            resolved_by: None,
            comment_ids: vec!["comment-1".into()],
        },
        comments: vec![pages::Comment {
                id: "comment-1".into(),
                thread_id: "thread-1".into(),
                author: pages::AuthorRef::User(vec![4; 32]),
                text: "@bot review this page".into(),
                created_at: 1,
                edited_at: None,
            deleted: false,
        }],
    };
    let mut ctx = CaptureCtx::new()
        .at(3)
        .with_tagging_origin()
        .with_registry(&registry)
        .with_page("p1", page_blocks("p1", "Spec"))
        .with_page_thread(thread);
    let event = Msg {
        target: "runs".into(),
        payload: tagging_encode_event(&EngagementEvent {
            source: "pages".into(),
            container: "thread-1".into(),
            content_seq: 1,
            author: Author::User(vec![4; 32]),
            tags: vec![agent_tag("bot")],
        }),
    };
    exec(&mut m, &mut ctx, &event).unwrap();
    commit(&mut m);

    let run_id = page_run_id_for("thread-1", 1, "bot");
    let pending = get_pending(&m, &run_id).expect("page mention engaged bot");
    assert_eq!(pending.channel_id, "runs:pages:thread-1");
    let DispatchMsg::Dispatch { payload, .. } = &ctx.dispatch_msgs()[0] else {
        panic!("expected page dispatch")
    };
    let envelope: serde_json::Value = serde_json::from_slice(payload).unwrap();
    assert!(
        envelope["conversation"]
            .as_str()
            .unwrap()
            .contains("review this page")
    );
    assert!(envelope["context"].as_str().unwrap().contains("Spec"));
}

// ---- the engagement intake: turn policies ----------------------------------

#[test]
fn mention_policy_engages_only_this_modules_tagged_active_agents() {
    let registry = registry(&[("bot1", &[ACTION_CHAT_POST]), ("bot2", &[ACTION_CHAT_POST])]);
    let mut m = watched(TurnPolicy::Mention, &registry);

    // the post tags bot1, an entity of a FOREIGN module, and an
    // unregistered agent — only bot1 engages.
    let mut ctx = CaptureCtx::new()
        .at(3)
        .with_tagging_origin()
        .with_registry(&registry)
        .with_transcript("general", transcript(3));
    exec(
        &mut m,
        &mut ctx,
        &engagement(
            "general",
            3,
            vec![
                agent_tag("bot1"),
                EntityRef {
                    module: "other-module".into(),
                    entity: "bot2".into(),
                },
                agent_tag("ghost"),
            ],
        ),
    )
    .unwrap();
    commit(&mut m);

    let run_id = run_id_for("general", 3, "bot1");
    let entry = get_pending(&m, &run_id).expect("bot1 engaged");
    assert_eq!(entry.dispatch_id, dispatch_id_for(&run_id));
    assert_eq!(entry.requester, SagaOrigin::Module("tagging".into()));
    assert_eq!(get_pending(&m, &run_id_for("general", 3, "bot2")), None);

    // exactly one dispatch, under the agent's own recipe, carrying the
    // fully composed envelope — thread key, contract, transcript, skills.
    let dispatches = ctx.dispatch_msgs();
    assert_eq!(dispatches.len(), 1);
    let DispatchMsg::Dispatch {
        dispatch_id,
        recipe_id,
        payload,
        ..
    } = &dispatches[0]
    else {
        panic!("expected a dispatch");
    };
    assert_eq!(*dispatch_id, dispatch_id_for(&run_id));
    assert_eq!(*recipe_id, recipe_id_for("bot1"));
    let envelope: serde_json::Value =
        serde_json::from_slice(payload).expect("the payload is a JSON envelope");
    assert_eq!(envelope["ducktape_run"], crate::envelope::RUN_ENVELOPE_MARKER);
    assert_eq!(envelope["agent_id"], "bot1");
    assert!(
        envelope.get("prompt_hash").is_none(),
        "the prompt pin retired: an agent is its curated skills"
    );
    assert!(
        envelope["contract"]
            .as_str()
            .unwrap()
            .contains("Return ONLY a JSON object"),
        "the strict output contract rides the payload"
    );
    assert!(
        envelope["conversation"].as_str().unwrap().contains("msg 3"),
        "the pinned transcript rides the payload verbatim"
    );
    assert!(
        envelope["instructions"]
            .as_str()
            .unwrap()
            .starts_with("You are a Ducktape agent."),
        "the generic fallback instructions ride the envelope"
    );
}

#[test]
fn the_envelope_tracks_the_registrys_live_skill_set() {
    // runs never mirrors the agent's soul: composition queries the registry at
    // dispatch time (staged same-block registrations included), so an
    // UpdateAgent that re-curates the skills — including flipping one to
    // `always`, which is what the persona IS — is picked up by the very next
    // run without any hook payload carrying it, and a capability retune never
    // disturbs it.
    let mut registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
    registry.get_mut("bot").unwrap().skills = vec![agent::SkillRef {
        name: "persona".into(),
        source_prefix: "/shared/skills/persona".into(),
        source_snapshot: Some("bb".repeat(32)),
        load: agent::LoadMode::OnDemand,
    }];
    let mut m = watched(TurnPolicy::All, &registry);

    // the registration hook fires as it would in the record's block.
    let mut hook_ctx = CaptureCtx::new()
        .with_agent_origin()
        .with_registry(&registry);
    exec(
        &mut m,
        &mut hook_ctx,
        &agent_event(&AgentEvent::Registered {
            agent_id: "bot".into(),
            capability: "model-1".into(),
        }),
    )
    .unwrap();
    commit(&mut m);

    let ctx = engage_post(&mut m, &registry, 2, &[]);
    commit(&mut m);
    let DispatchMsg::Dispatch { payload, .. } = &ctx.dispatch_msgs()[0] else {
        panic!("expected a dispatch");
    };
    let envelope: serde_json::Value = serde_json::from_slice(payload).unwrap();
    assert_eq!(
        envelope["skills"][0]["always"], false,
        "the skill starts on-demand: no persona yet"
    );

    // the owner promotes the skill to the agent's persona; the registry hook
    // only ever carries capability retunes — process one to show it is
    // orthogonal.
    registry.get_mut("bot").unwrap().skills[0].load = agent::LoadMode::Always;
    let mut hook_ctx = CaptureCtx::new()
        .with_agent_origin()
        .with_registry(&registry);
    exec(
        &mut m,
        &mut hook_ctx,
        &agent_event(&AgentEvent::CapabilityChanged {
            agent_id: "bot".into(),
            capability: "model-2".into(),
        }),
    )
    .unwrap();
    commit(&mut m);

    let ctx = engage_post(&mut m, &registry, 3, &[]);
    commit(&mut m);
    let DispatchMsg::Dispatch { payload, .. } = &ctx.dispatch_msgs()[0] else {
        panic!("expected a dispatch");
    };
    let envelope: serde_json::Value = serde_json::from_slice(payload).unwrap();
    assert_eq!(
        envelope["skills"][0]["always"], true,
        "the next run composes from the updated record — the host now inlines it"
    );
    assert_eq!(envelope["agent_id"], "bot");
}

#[test]
fn an_agent_with_no_always_skill_dispatches_the_generic_instructions() {
    // no curated skills at all: nothing to assemble a soul from, so the generic
    // instructions are the floor. no prompt pin exists to hide them any more.
    let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
    let mut m = watched(TurnPolicy::All, &registry);
    let ctx = engage_post(&mut m, &registry, 2, &[]);
    commit(&mut m);
    let DispatchMsg::Dispatch { payload, .. } = &ctx.dispatch_msgs()[0] else {
        panic!("expected a dispatch");
    };
    let envelope: serde_json::Value = serde_json::from_slice(payload).unwrap();
    assert_eq!(
        envelope["skills"].as_array().unwrap().len(),
        0,
        "no skills is []"
    );
    assert!(
        envelope["instructions"]
            .as_str()
            .unwrap()
            .starts_with("You are a Ducktape agent.")
    );
}

#[test]
fn all_policy_engages_every_active_agent_and_paused_agents_never_engage() {
    let mut registry = registry(&[("a", &[]), ("b", &[]), ("c", &[])]);
    let mut m = watched(TurnPolicy::All, &registry);
    pause(&mut registry, "b");

    let ctx = engage_post(&mut m, &registry, 2, &[]);
    commit(&mut m);
    assert_eq!(
        ctx.dispatch_msgs().len(),
        2,
        "two active agents, two dispatches"
    );
    assert!(get_pending(&m, &run_id_for("general", 2, "a")).is_some());
    assert_eq!(
        get_pending(&m, &run_id_for("general", 2, "b")),
        None,
        "a paused agent never engages"
    );
    assert!(get_pending(&m, &run_id_for("general", 2, "c")).is_some());
}

#[test]
fn all_policy_shares_one_sibling_budget_across_multiple_composes() {
    let mut registry = Registry::new();
    for index in 0..20 {
        let id = format!("bot-{index:02}");
        registry.insert(id.clone(), record(&id, &[ACTION_CHAT_POST]));
    }
    let mut m = watched(TurnPolicy::All, &registry)
        .with_files_module("files")
        .with_pages_module("pages");
    let page_limit = usize::from(pages::MAX_PAGE_QUERY_LIMIT);
    let mut ctx = CaptureCtx::new()
        .at(2)
        .with_tagging_origin()
        .with_registry(&registry)
        .with_transcript(
            "general",
            vec![
                message(1, "start"),
                message(
                    2,
                    "[Plan](duck://page/plan) [notes](duck://files/shared/attachments/u/notes.md)",
                ),
            ],
        )
        .with_page("plan", page_with_block_count(page_limit * 50, ""))
        .with_file("/shared/attachments/u/notes.md", b"shared context");

    exec(&mut m, &mut ctx, &engagement("general", 2, Vec::new())).unwrap();

    assert_eq!(ctx.distinct_query_count(), MAX_SIBLING_QUERY_READS);
    assert_eq!(
        ctx.dispatch_msgs().len(),
        5,
        "five agents fit; the sixth distinct turn lookup exhausts the ledger"
    );
    for dispatch in ctx.dispatch_msgs() {
        let DispatchMsg::Dispatch { payload, .. } = dispatch else {
            panic!("expected dispatch");
        };
        let payload: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        let context = payload["context"].as_str().unwrap();
        assert!(context.contains("[Project Plan](duck://page/plan)"));
        assert!(context.contains("shared context"));
    }
}

#[test]
fn round_robin_picks_by_anchor_seq_over_the_sorted_active_agents() {
    let mut registry = registry(&[("a", &[]), ("b", &[]), ("c", &[])]);
    let mut m = watched(TurnPolicy::RoundRobin, &registry);

    // seq 4 over [a, b, c]: 4 % 3 = 1 -> "b".
    engage_post(&mut m, &registry, 4, &[]);
    commit(&mut m);
    assert!(get_pending(&m, &run_id_for("general", 4, "b")).is_some());
    assert_eq!(get_pending(&m, &run_id_for("general", 4, "a")), None);
    assert_eq!(get_pending(&m, &run_id_for("general", 4, "c")), None);

    // pause "b": the domain shrinks to [a, c]; seq 5 % 2 = 1 -> "c".
    pause(&mut registry, "b");
    engage_post(&mut m, &registry, 5, &[]);
    commit(&mut m);
    assert!(get_pending(&m, &run_id_for("general", 5, "c")).is_some());
    assert_eq!(get_pending(&m, &run_id_for("general", 5, "b")), None);
}

#[test]
fn assigned_policy_engages_exactly_its_agent_and_respects_pause() {
    let mut registry = registry(&[("a", &[]), ("b", &[])]);
    let mut m = watched(TurnPolicy::Assigned("b".into()), &registry);
    engage_post(&mut m, &registry, 2, &[]);
    commit(&mut m);
    assert!(get_pending(&m, &run_id_for("general", 2, "b")).is_some());
    assert_eq!(get_pending(&m, &run_id_for("general", 2, "a")), None);

    // paused assignee: nothing engages, the block still commits.
    pause(&mut registry, "b");
    let ctx = engage_post(&mut m, &registry, 3, &[]);
    commit(&mut m);
    assert!(ctx.dispatch_msgs().is_empty());
    assert_eq!(get_pending(&m, &run_id_for("general", 3, "b")), None);
}

#[test]
fn foreign_sources_and_direct_chat_or_saga_follow_ups_are_dead_letters() {
    // the LOOP RULE itself lives in the tagging plane (only user posts
    // fire) and is tested there; this module's job is to survive the
    // events it should not act on.
    let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
    let mut m = watched(TurnPolicy::All, &registry);
    let before = m.root();

    // an engagement whose source is not chat: dropped with a breadcrumb.
    let mut ctx = CaptureCtx::new()
        .at(2)
        .with_tagging_origin()
        .with_registry(&registry);
    exec(
        &mut m,
        &mut ctx,
        &Msg {
            target: "runs".into(),
            payload: tagging_encode_event(&EngagementEvent {
                source: "pages".into(),
                container: "general".into(),
                content_seq: 2,
                author: Author::User(vec![1; 32]),
                tags: vec![],
            }),
        },
    )
    .unwrap();
    assert!(ctx.msgs.is_empty());
    assert!(!ctx.events.is_empty(), "the drop leaves a breadcrumb");

    // a direct chat-origin follow-up (no hook is ever registered now):
    // dead-lettered, never an abort of the posting block.
    let mut ctx = CaptureCtx::new().with_origin(Origin::Module("chat".into()));
    exec(
        &mut m,
        &mut ctx,
        &Msg {
            target: "runs".into(),
            payload: b"anything at all".to_vec(),
        },
    )
    .unwrap();
    assert!(ctx.msgs.is_empty());

    // a saga-origin callback (a foreign trigger's reply_to pointed here):
    // dead-lettered — an Err would abort the saga's terminal block.
    let mut ctx = CaptureCtx::new().with_origin(Origin::Module("saga".into()));
    exec(
        &mut m,
        &mut ctx,
        &Msg {
            target: "runs".into(),
            payload: b"a saga callback of any shape".to_vec(),
        },
    )
    .unwrap();
    assert!(ctx.msgs.is_empty());
    assert!(!ctx.events.is_empty(), "the drop leaves a breadcrumb");
    commit(&mut m);
    assert_eq!(m.root(), before, "nothing was staged");
}

#[test]
fn unwatched_channels_and_failed_pins_are_staged_no_ops_on_the_engagement_arm() {
    let registry = registry(&[("bot", &[])]);
    let mut m = watched(TurnPolicy::All, &registry);
    let before = m.root();

    // an engagement for a channel we do not watch (subscription drift
    // within a block): no-op, never an error.
    let mut ctx = CaptureCtx::new()
        .at(2)
        .with_tagging_origin()
        .with_registry(&registry)
        .with_transcript("random", transcript(2));
    exec(&mut m, &mut ctx, &engagement("random", 2, vec![])).unwrap();
    assert!(ctx.msgs.is_empty());

    // a failing context pin (the ctx serves NO transcript at all — the
    // chat query errors) must not poison the posting block: Ok, no run.
    let mut ctx = CaptureCtx::new()
        .at(2)
        .with_tagging_origin()
        .with_registry(&registry);
    exec(&mut m, &mut ctx, &engagement("general", 2, vec![])).unwrap();
    assert!(
        ctx.dispatch_msgs().is_empty(),
        "no dispatch on a failed pin"
    );
    assert!(!ctx.events.is_empty(), "the skip leaves a breadcrumb event");
    commit(&mut m);
    assert_eq!(m.root(), before, "nothing was staged");
}

// ---- the turn claim ----------------------------------------------------------

#[test]
fn duplicate_turn_claims_are_deterministic_no_ops() {
    let registry = registry(&[("bot", &[])]);
    let mut m = watched(TurnPolicy::All, &registry);

    // the engagement claims the turn in the posting block...
    let ctx = engage_post(&mut m, &registry, 2, &[]);
    assert_eq!(ctx.dispatch_msgs().len(), 1);
    let run_id = run_id_for("general", 2, "bot");
    let created = get_pending(&m, &run_id).unwrap();

    // ...an explicit RequestRun for the SAME turn in the same block is a
    // staged no-op (first in consensus order won)...
    let mut ctx = CaptureCtx::new()
        .at(2)
        .with_origin(user(5))
        .with_registry(&registry)
        .with_transcript("general", transcript(2));
    exec(
        &mut m,
        &mut ctx,
        &admin(&RunsMsg::RequestRun {
            agent_id: "bot".into(),
            channel_id: "general".into(),
            anchor_seq: 2,
            demands: Default::default(),
            skills: Vec::new(),
        }),
    )
    .unwrap();
    assert!(ctx.msgs.is_empty(), "the losing claim re-fires nothing");
    commit(&mut m);
    assert_eq!(
        get_pending(&m, &run_id).unwrap(),
        created,
        "the first claim's entry survives untouched"
    );

    // ...and a COMMITTED duplicate (the same engagement replayed later)
    // is equally a no-op.
    let root = m.root();
    let ctx = engage_post(&mut m, &registry, 2, &[]);
    assert!(ctx.msgs.is_empty());
    commit(&mut m);
    assert_eq!(m.root(), root, "a duplicate claim moves nothing");
}

#[test]
fn a_delivered_turn_stays_claimed_via_the_dispatch_record() {
    // after delivery the pending entry is pruned — the dispatch module's
    // permanent record is what keeps the turn claimed. re-staging an
    // entry here would orphan it forever (the dispatch module no-ops the
    // duplicate dispatch and no ResultEvent would ever prune it).
    let registry = registry(&[("bot", &[])]);
    let mut m = watched(TurnPolicy::All, &registry);
    let run_id = run_id_for("general", 2, "bot");
    let taken = dispatch_id_for(&run_id);

    let mut ctx = CaptureCtx::new()
        .at(9)
        .with_tagging_origin()
        .with_registry(&registry)
        .with_transcript("general", transcript(2))
        .with_taken_dispatch(&taken);
    exec(&mut m, &mut ctx, &engagement("general", 2, vec![])).unwrap();
    assert!(ctx.msgs.is_empty(), "a taken turn re-fires nothing");

    let mut ctx = CaptureCtx::new()
        .at(9)
        .with_origin(user(5))
        .with_registry(&registry)
        .with_transcript("general", transcript(2))
        .with_taken_dispatch(&taken);
    exec(
        &mut m,
        &mut ctx,
        &admin(&RunsMsg::RequestRun {
            agent_id: "bot".into(),
            channel_id: "general".into(),
            anchor_seq: 2,
            demands: Default::default(),
            skills: Vec::new(),
        }),
    )
    .unwrap();
    assert!(ctx.msgs.is_empty());
    commit(&mut m);
    assert_eq!(get_pending(&m, &run_id), None, "nothing was re-staged");
}

#[test]
fn chat_and_job_run_keys_are_structurally_disjoint_and_reject_separator_inputs() {
    assert_ne!(
        run_id_for("job", 7, "duck"),
        job_run_id_for("7", "duck", 3),
        "a channel literally named job must not collide with job runs"
    );

    let registry = registry(&[("bot", &[])]);
    let mut m = watched(TurnPolicy::All, &registry);
    let root = m.root();

    let mut ctx = CaptureCtx::new()
        .with_origin(user(9))
        .with_registry(&registry);
    let err = exec(
        &mut m,
        &mut ctx,
        &admin(&RunsMsg::WatchChannel {
            channel_id: "bad\u{1f}channel".into(),
            policy: TurnPolicy::All,
        }),
    )
    .unwrap_err();
    assert!(matches!(err, Error::Module(message) if message.contains("unit separator")));
    abort(&mut m);

    let mut ctx = CaptureCtx::new()
        .with_origin(user(1))
        .with_registry(&registry)
        .with_transcript("bad\u{1f}channel", transcript(1));
    let err = exec(
        &mut m,
        &mut ctx,
        &admin(&RunsMsg::RequestRun {
            agent_id: "bot".into(),
            channel_id: "bad\u{1f}channel".into(),
            anchor_seq: 1,
            demands: Default::default(),
            skills: Vec::new(),
        }),
    )
    .unwrap_err();
    assert!(matches!(err, Error::Module(message) if message.contains("unit separator")));
    abort(&mut m);

    let mut ctx = CaptureCtx::new()
        .with_jobs_origin()
        .with_registry(&registry);
    exec(
        &mut m,
        &mut ctx,
        &jobs_event("bad\u{1f}job", "agent/bot", "spec"),
    )
    .expect("separator in a no-fail jobs event is a no-op");
    assert!(ctx.msgs.is_empty(), "no claim emitted for a bad job id");

    // a spec that does not hash to spec_hash is dropped the same way.
    let mut ctx = CaptureCtx::new()
        .with_jobs_origin()
        .with_registry(&registry);
    exec(
        &mut m,
        &mut ctx,
        &Msg {
            target: "runs".into(),
            payload: jobs_encode_event(&JobsEvent::Submitted {
                job_id: "job-x".into(),
                kind: "agent/bot".into(),
                submitter: "ext:01".into(),
                spec: "actual".into(),
                spec_hash: vec![9u8; 32],
            }),
        },
    )
    .expect("a mismatched spec hash is a no-op");
    assert!(ctx.msgs.is_empty());
    commit(&mut m);
    assert_eq!(m.root(), root, "bad jobs events staged nothing");
}

// ---- the no-fail arms ----------------------------------------------------------

#[test]
fn malformed_intake_payloads_are_staged_no_ops() {
    let registry = registry(&[("bot", &[])]);
    let mut m = watched(TurnPolicy::All, &registry);
    let before = m.root();

    // garbage from the tagging origin: the posting block must survive.
    let mut ctx = CaptureCtx::new().with_tagging_origin();
    exec(
        &mut m,
        &mut ctx,
        &Msg {
            target: "runs".into(),
            payload: b"not an engagement".to_vec(),
        },
    )
    .unwrap();
    assert!(ctx.msgs.is_empty());
    assert!(!ctx.events.is_empty(), "the drop leaves a breadcrumb");

    // garbage from the dispatch origin: the delivery block must survive.
    let mut ctx = CaptureCtx::new().with_dispatch_origin();
    exec(
        &mut m,
        &mut ctx,
        &Msg {
            target: "runs".into(),
            payload: b"not a result event".to_vec(),
        },
    )
    .unwrap();
    assert!(ctx.msgs.is_empty());

    // garbage from the jobs origin: the submit block must survive.
    let mut ctx = CaptureCtx::new().with_jobs_origin();
    exec(
        &mut m,
        &mut ctx,
        &Msg {
            target: "runs".into(),
            payload: b"not a jobs event".to_vec(),
        },
    )
    .unwrap();
    assert!(ctx.msgs.is_empty());

    // a well-formed result event for an UNKNOWN dispatch: staged no-op.
    let mut ctx = CaptureCtx::new().with_dispatch_origin();
    exec(&mut m, &mut ctx, &result_event("ghost-run", Ok(Vec::new()))).unwrap();

    commit(&mut m);
    assert_eq!(m.root(), before, "none of the drops staged anything");
}

#[test]
fn external_submitters_cannot_fake_the_engagement_or_result_intakes() {
    let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
    let mut m = watched(TurnPolicy::All, &registry);

    // engagement-shaped bytes from an EXTERNAL origin route to the
    // RunsMsg decoder and fail there — no run is ever created.
    let mut ctx = CaptureCtx::new()
        .at(2)
        .with_origin(user(1))
        .with_registry(&registry)
        .with_transcript("general", transcript(2));
    let err = exec(&mut m, &mut ctx, &engagement("general", 2, vec![])).unwrap_err();
    assert!(matches!(err, Error::Module(_)));
    abort(&mut m);
    assert_eq!(get_pending(&m, &run_id_for("general", 2, "bot")), None);

    // result-shaped bytes from an EXTERNAL origin: same story.
    engage_post(&mut m, &registry, 2, &[]);
    commit(&mut m);
    let run_id = run_id_for("general", 2, "bot");
    let mut ctx = CaptureCtx::new().with_origin(user(1));
    let err = exec(&mut m, &mut ctx, &result_event(&run_id, Ok(Vec::new()))).unwrap_err();
    assert!(matches!(err, Error::Module(_)));
    abort(&mut m);
    assert!(
        get_pending(&m, &run_id).is_some(),
        "the forged delivery pruned nothing"
    );
}
