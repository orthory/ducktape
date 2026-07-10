use super::*;

// ---- determinism + queries + state-sync -------------------------------------------

#[test]
fn two_instances_replaying_the_same_ops_produce_identical_roots() {
    let registry = registry(&[("bot", &[ACTION_CHAT_POST]), ("z", &[])]);
    let run_id = run_id_for("general", 2, "bot");
    let build = || {
        let mut m = module();
        let mut roots = Vec::new();
        // block 1: watch.
        let mut ctx = CaptureCtx::new()
            .at(1)
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
        commit(&mut m);
        roots.push(m.root());
        // block 2: an engagement engages bot.
        let mut ctx = CaptureCtx::new()
            .at(2)
            .with_tagging_origin()
            .with_registry(&registry)
            .with_transcript("general", transcript(2));
        exec(
            &mut m,
            &mut ctx,
            &engagement("general", 2, vec![agent_tag("bot")]),
        )
        .unwrap();
        commit(&mut m);
        roots.push(m.root());
        // block 3: the dispatch result lands and prunes.
        let mut ctx = CaptureCtx::new()
            .at(3)
            .with_dispatch_origin()
            .with_registry(&registry)
            .with_transcript("general", transcript(2));
        exec(
            &mut m,
            &mut ctx,
            &result_event(&run_id, Ok(response(&["done"], vec![]))),
        )
        .unwrap();
        commit(&mut m);
        roots.push(m.root());
        // block 4: a second watch.
        let mut ctx = CaptureCtx::new()
            .at(4)
            .with_origin(user(9))
            .with_registry(&registry);
        exec(
            &mut m,
            &mut ctx,
            &admin(&RunsMsg::WatchChannel {
                channel_id: "dev".into(),
                policy: TurnPolicy::All,
            }),
        )
        .unwrap();
        commit(&mut m);
        roots.push(m.root());
        roots
    };

    let left = build();
    let right = build();
    assert_eq!(left, right, "same ops, same blocks -> identical roots");
    assert_ne!(*left.last().unwrap(), StateRoot::ZERO);
}

#[test]
fn queries_list_pending_and_watches() {
    let registry = registry(&[("a", &[]), ("b", &[])]);
    let mut m = watched(TurnPolicy::All, &registry);
    // watch a second channel and create runs in both.
    let mut ctx = CaptureCtx::new()
        .with_origin(user(9))
        .with_registry(&registry);
    exec(
        &mut m,
        &mut ctx,
        &admin(&RunsMsg::WatchChannel {
            channel_id: "dev".into(),
            policy: TurnPolicy::All,
        }),
    )
    .unwrap();
    commit(&mut m);
    engage_post(&mut m, &registry, 2, &[]);
    let mut ctx = CaptureCtx::new()
        .at(3)
        .with_tagging_origin()
        .with_registry(&registry)
        .with_transcript(
            "dev",
            vec![message_in(
                "dev",
                1,
                AuthorRef::User(vec![1; 32]),
                "hello dev",
                None,
            )],
        );
    exec(&mut m, &mut ctx, &engagement("dev", 1, vec![])).unwrap();
    commit(&mut m);

    let runs = pending_runs(&m);
    assert_eq!(runs.len(), 4, "2 agents x 2 channels, all in flight");
    assert!(
        runs.iter()
            .all(|r| r.dispatch_id == dispatch_id_for(&r.run_id)),
        "every view carries its own dispatch id"
    );

    let reply = block_on(m.query(&encode_query(&RunsQuery::Watches))).unwrap();
    let RunsReply::Watches(watches) = runs_decode_reply(&reply).unwrap() else {
        panic!("watches reply expected");
    };
    assert_eq!(
        watches,
        vec![
            WatchView {
                channel_id: "dev".into(),
                policy: TurnPolicy::All,
            },
            WatchView {
                channel_id: "general".into(),
                policy: TurnPolicy::All,
            },
        ]
    );
}

// ---- the delivered-runs ring -------------------------------------------------

#[test]
fn the_delivered_runs_ring_evicts_past_the_cap_and_serves_newest_first() {
    let rec = |i: u64| RunRecord {
        run_id: format!("run-{i}"),
        agent_id: "bot".into(),
        channel_id: "general".into(),
        anchor_seq: i,
        outcome: RunOutcome::Delivered,
        degraded: false,
        created_at: i,
        delivered_at: i + 1,
        executing_node: "unknown".into(),
        output_ref: None,
        pr_number: None,
    };
    let mut m = module();
    m.pending_history.extend((1..=101).map(rec));
    commit(&mut m);
    let runs = recent_runs(&m);
    assert_eq!(runs.len(), RUN_HISTORY_CAP, "the ring caps at 100");
    assert_eq!(runs.first().unwrap().run_id, "run-101", "newest first");
    assert_eq!(runs.last().unwrap().run_id, "run-2", "run-1 evicted");
    // an aborted block leaves no ghost record.
    m.pending_history.push(rec(999));
    abort(&mut m);
    assert_eq!(recent_runs(&m), runs, "aborted staging records nothing");
}

#[test]
fn replaying_the_same_ops_rebuilds_the_identical_delivered_runs_ring() {
    let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
    let run_id = run_id_for("general", 2, "bot");
    let saga_id = sink::saga_id_for_dispatch("runs", &dispatch_id_for(&run_id));
    let build = || {
        let mut m = watched(TurnPolicy::Mention, &registry);
        engage_post(&mut m, &registry, 2, &["bot"]);
        commit(&mut m);
        let mut ctx = CaptureCtx::new()
            .at(9)
            .with_dispatch_origin()
            .with_registry(&registry)
            .with_transcript("general", transcript(2))
            .with_saga_assignee(&saga_id, &[0xcd; 32]);
        exec(
            &mut m,
            &mut ctx,
            &result_event(&run_id, Ok(response(&["done"], vec![]))),
        )
        .unwrap();
        commit(&mut m);
        recent_runs(&m)
    };
    let (left, right) = (build(), build());
    assert_eq!(left, right, "same ops => same ring");
    assert_eq!(left.len(), 1);
    let rec = &left[0];
    assert_eq!(rec.run_id, run_id);
    assert_eq!(rec.agent_id, "bot");
    assert_eq!(rec.channel_id, "general");
    assert_eq!(rec.anchor_seq, 2);
    assert_eq!(rec.outcome, RunOutcome::Delivered);
    assert!(!rec.degraded);
    assert_eq!(
        (rec.created_at, rec.delivered_at),
        (2, 9),
        "the staging and delivery blocks' consensus counters"
    );
    assert_eq!(rec.executing_node, "cd".repeat(32), "the saga assignee");
    assert_eq!(rec.output_ref, None, "a plain reply moves nothing");
    assert_eq!(rec.pr_number, None);

    // the failure path records too — outcome failed, delivery pruned.
    let mut m = watched(TurnPolicy::Mention, &registry);
    engage_post(&mut m, &registry, 2, &["bot"]);
    commit(&mut m);
    let mut ctx = CaptureCtx::new()
        .at(11)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", transcript(2));
    exec(&mut m, &mut ctx, &result_event(&run_id, Err("boom".into()))).unwrap();
    commit(&mut m);
    let failed = recent_runs(&m);
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].outcome, RunOutcome::Failed);
    assert_eq!(failed[0].executing_node, "unknown", "no saga record served");
    assert_eq!(get_pending(&m, &run_id), None, "the entry still prunes");
}

#[test]
fn recent_runs_query_decodes_the_bare_string_and_replies_with_the_ring() {
    // the TS client sends the serde unit variant — the bare string — and
    // reads the snake_case-keyed reply. pin both wire shapes.
    assert_eq!(
        decode_query(br#""recent_runs""#).unwrap(),
        RunsQuery::RecentRuns
    );
    let m = module();
    let reply = block_on(m.query(&encode_query(&RunsQuery::RecentRuns))).unwrap();
    assert_eq!(String::from_utf8(reply).unwrap(), r#"{"recent_runs":[]}"#);
}

#[test]
fn state_sync_handle_exposes_the_snapshot_bytes() {
    let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
    let mut m = watched(TurnPolicy::All, &registry);
    engage_post(&mut m, &registry, 2, &[]);
    commit(&mut m);
    assert_eq!(
        m.state_sync_handle().unwrap(),
        StateSyncHandle::SnapshotBytes(m.snapshot()),
        "the handle IS the canonical snapshot"
    );
}
