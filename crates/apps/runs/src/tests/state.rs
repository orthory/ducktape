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
