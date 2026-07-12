use super::*;

#[test]
fn responses_beyond_the_agents_grants_fail_the_run() {
    // an agent granted ONLY chat.post must not create tasks...
    let (mut m, registry, run_id) = awaiting_run(&[ACTION_CHAT_POST]);
    let mut ctx = CaptureCtx::new()
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", transcript(2));
    exec(
        &mut m,
        &mut ctx,
        &result_event(
            &run_id,
            Ok(response(
                &["look what i did"],
                vec![AgentAction::CreateTask {
                    task_id: "t1".into(),
                    title: "sneaky".into(),
                }],
            )),
        ),
    )
    .unwrap();
    assert!(
        ctx.task_msgs().is_empty(),
        "a disallowed action emits no task writes"
    );
    // the agent holds chat.post, so the failure surfaces as the ⚠ reply.
    let posts = ctx.chat_msgs();
    assert_eq!(posts.len(), 1);
    let ChatMsg::PostMessage { blocks, .. } = &posts[0] else {
        panic!("expected a post");
    };
    assert_eq!(
        *blocks,
        vec![Block::paragraph(format!(
            "⚠ BOT failed: agent bot is not allowed to {ACTION_TASKS_CREATE}"
        ))]
    );
    commit(&mut m);
    assert_eq!(get_pending(&m, &run_id), None);

    // ...and an agent granted only tasks.create must not post replies —
    // and without chat.post the failure CANNOT surface in chat either:
    // the old breadcrumb-only silence holds.
    let (mut m, registry, run_id) = awaiting_run(&[ACTION_TASKS_CREATE]);
    let mut ctx = CaptureCtx::new()
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", transcript(2));
    exec(
        &mut m,
        &mut ctx,
        &result_event(&run_id, Ok(response(&["hello"], vec![]))),
    )
    .unwrap();
    assert!(ctx.msgs.is_empty());
    let breadcrumbs: Vec<String> = ctx
        .events
        .iter()
        .map(|e| String::from_utf8_lossy(&e.payload).into_owned())
        .collect();
    assert!(
        breadcrumbs.iter().any(|b| b.contains(ACTION_CHAT_POST)),
        "the failure names the missing grant: {breadcrumbs:?}"
    );
    commit(&mut m);
    assert_eq!(get_pending(&m, &run_id), None);
}

#[test]
fn task_actions_without_a_configured_tasks_module_fail_the_run() {
    let registry = registry(&[("bot", &[ACTION_CHAT_POST, ACTION_TASKS_CREATE])]);
    let mut m = RunsModule::new(
        "runs", "chat", "saga", "tagging", "dispatch", "agent", None, None,
    );
    let mut ctx = CaptureCtx::new()
        .with_origin(user(9))
        .with_registry(&registry);
    exec(
        &mut m,
        &mut ctx,
        &admin(&RunsMsg::WatchChannel {
            channel_id: "general".into(),
            policy: TurnPolicy::All,
        }),
    )
    .unwrap();
    commit(&mut m);
    engage_post(&mut m, &registry, 2, &[]);
    commit(&mut m);
    let run_id = run_id_for("general", 2, "bot");

    let mut ctx = CaptureCtx::new()
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", transcript(2));
    exec(
        &mut m,
        &mut ctx,
        &result_event(
            &run_id,
            Ok(response(
                &["ok"],
                vec![AgentAction::CreateTask {
                    task_id: "t1".into(),
                    title: "x".into(),
                }],
            )),
        ),
    )
    .unwrap();
    assert!(ctx.task_msgs().is_empty(), "no task write may escape");
    // the failure still surfaces in chat — the agent holds chat.post.
    assert_eq!(ctx.chat_msgs().len(), 1);
    let breadcrumbs: Vec<String> = ctx
        .events
        .iter()
        .map(|e| String::from_utf8_lossy(&e.payload).into_owned())
        .collect();
    assert!(breadcrumbs.iter().any(|b| b.contains("no tasks module")));
    commit(&mut m);
    assert_eq!(get_pending(&m, &run_id), None);
}

#[test]
fn a_squatted_reply_message_id_fails_the_run_instead_of_the_block() {
    let (mut m, registry, run_id) = awaiting_run(&[ACTION_CHAT_POST]);
    // someone posted a message whose id IS the run's reply id.
    let mut squatted = transcript(2);
    squatted[1].head.message_id = reply_message_id(&run_id);
    let mut ctx = CaptureCtx::new()
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", squatted);
    exec(
        &mut m,
        &mut ctx,
        &result_event(&run_id, Ok(response(&["hi"], vec![]))),
    )
    .unwrap();
    assert!(ctx.msgs.is_empty(), "the squatted id emits NOTHING");
    let breadcrumbs: Vec<String> = ctx
        .events
        .iter()
        .map(|e| String::from_utf8_lossy(&e.payload).into_owned())
        .collect();
    assert!(breadcrumbs.iter().any(|b| b.contains("already taken")));
    commit(&mut m);
    assert_eq!(get_pending(&m, &run_id), None);
}

#[test]
fn a_full_thread_fails_the_run_instead_of_the_block() {
    let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
    let mut m = watched(TurnPolicy::All, &registry);
    // the anchor replies to a root that has hit the reply cap.
    let mut root = message(1, "root");
    root.head.reply_count = MAX_THREAD_REPLIES as u64;
    let anchor = message_in("general", 2, AuthorRef::User(vec![1; 32]), "reply", Some(1));
    let full = vec![root, anchor];
    let mut ctx = CaptureCtx::new()
        .at(2)
        .with_tagging_origin()
        .with_registry(&registry)
        .with_transcript("general", full.clone());
    exec(&mut m, &mut ctx, &engagement("general", 2, vec![])).unwrap();
    commit(&mut m);
    let run_id = run_id_for("general", 2, "bot");

    let mut ctx = CaptureCtx::new()
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", full);
    exec(
        &mut m,
        &mut ctx,
        &result_event(&run_id, Ok(response(&["hi"], vec![]))),
    )
    .unwrap();
    assert!(ctx.msgs.is_empty());
    let breadcrumbs: Vec<String> = ctx
        .events
        .iter()
        .map(|e| String::from_utf8_lossy(&e.payload).into_owned())
        .collect();
    assert!(breadcrumbs.iter().any(|b| b.contains("thread reply cap")));
    commit(&mut m);
    assert_eq!(get_pending(&m, &run_id), None);
}

#[test]
fn a_reply_and_a_post_message_into_one_near_full_thread_refuse_the_overflow() {
    // THE SAME-BLOCK ACCOUNTING. the reply and a `chat.post_message` are two
    // posts into ONE thread inside ONE delivery block, and every probe reads
    // COMMITTED state — so both see the same 4095 replies and both pass. chat
    // applies the first (4096) and REJECTS the second, which aborts the delivery
    // block; the mailbox re-injects it and it aborts again, forever. the
    // overflowing post must be refused at validation instead.
    let registry = registry(&[("bot", &[ACTION_CHAT_POST, ACTION_CHAT_POST_MESSAGE])]);
    let mut m = watched(TurnPolicy::All, &registry);
    // one reply short of the cap: room for EXACTLY one more post.
    let mut root = message(1, "root");
    root.head.reply_count = MAX_THREAD_REPLIES as u64 - 1;
    let anchor = message_in("general", 2, AuthorRef::User(vec![1; 32]), "reply", Some(1));
    let nearly_full = vec![root, anchor];
    let mut ctx = CaptureCtx::new()
        .at(2)
        .with_tagging_origin()
        .with_registry(&registry)
        .with_transcript("general", nearly_full.clone());
    exec(&mut m, &mut ctx, &engagement("general", 2, vec![])).unwrap();
    commit(&mut m);
    let run_id = run_id_for("general", 2, "bot");

    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", nearly_full);
    exec(
        &mut m,
        &mut ctx,
        &result_event(
            &run_id,
            Ok(response(
                &["done"],
                vec![AgentAction::PostMessage {
                    channel_id: "general".into(),
                    text: "and one more".into(),
                    thread: Some(1),
                }],
            )),
        ),
    )
    .unwrap();

    assert!(
        ctx.notes().iter().any(|n| n.contains("thread reply cap")),
        "the overflow is refused at validation: {:?}",
        ctx.notes()
    );
    // the strict lane's policy: the RUN fails, never the block — and the ⚠
    // reply still fits the one free slot the thread had.
    let posts = ctx.chat_msgs();
    assert_eq!(posts.len(), 1, "only the failure reply: {posts:?}");
    assert!(
        matches!(
            &posts[0],
            ChatMsg::PostMessage { message_id, thread: Some(1), .. }
                if *message_id == reply_message_id(&run_id)
        ),
        "the agent's own post never existed: {:?}",
        posts[0]
    );
    commit(&mut m);
    assert_eq!(recent_runs(&m)[0].outcome, RunOutcome::Failed);
}

#[test]
fn two_post_messages_into_one_near_full_thread_refuse_the_second() {
    // the counter must see the FIRST staged post when it probes the second.
    // the anchor is unthreaded here, so the run's own reply consumes nothing —
    // the two actions alone race for the thread's last free slot.
    let (mut m, registry, run_id) = awaiting_run(&[ACTION_CHAT_POST, ACTION_CHAT_POST_MESSAGE]);
    let mut nearly_full = transcript(2);
    nearly_full[0].head.reply_count = MAX_THREAD_REPLIES as u64 - 1;
    let post = |text: &str| AgentAction::PostMessage {
        channel_id: "general".into(),
        text: text.into(),
        thread: Some(1),
    };
    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", nearly_full);
    exec(
        &mut m,
        &mut ctx,
        &result_event(
            &run_id,
            Ok(response(&["done"], vec![post("fits"), post("overflows")])),
        ),
    )
    .unwrap();

    assert!(
        ctx.notes().iter().any(|n| n.contains("thread reply cap")),
        "the second post is refused at validation: {:?}",
        ctx.notes()
    );
    // all-or-nothing: the strict lane fails the whole run, so even the post
    // that WOULD have fit never lands — only the unthreaded ⚠ reply.
    let posts = ctx.chat_msgs();
    assert_eq!(posts.len(), 1, "only the failure reply: {posts:?}");
    commit(&mut m);
    assert_eq!(recent_runs(&m)[0].outcome, RunOutcome::Failed);
}

#[test]
fn a_failed_dispatch_outcome_posts_a_threaded_failure_reply_and_prunes_the_entry() {
    // the anchor is a thread reply, so the failure reply must join the
    // same thread a success reply would have.
    let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
    let mut m = watched(TurnPolicy::All, &registry);
    let mut thread_transcript = transcript(1);
    thread_transcript.push(message_in(
        "general",
        2,
        AuthorRef::User(vec![1; 32]),
        "in thread",
        Some(1),
    ));
    let mut ctx = CaptureCtx::new()
        .at(2)
        .with_tagging_origin()
        .with_registry(&registry)
        .with_transcript("general", thread_transcript.clone());
    exec(&mut m, &mut ctx, &engagement("general", 2, vec![])).unwrap();
    commit(&mut m);
    let run_id = run_id_for("general", 2, "bot");

    // the dispatch plane already folded saga failures, timeouts, and
    // contract violations into the Err lane — one shape lands here. the
    // reason's newlines collapse into the single-paragraph excerpt.
    let mut ctx = CaptureCtx::new()
        .at(20)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", thread_transcript.clone());
    exec(
        &mut m,
        &mut ctx,
        &result_event(&run_id, Err("worker exploded\nstack line two".into())),
    )
    .unwrap();
    assert_eq!(
        ctx.chat_msgs(),
        vec![ChatMsg::PostMessage {
            channel_id: "general".into(),
            message_id: reply_message_id(&run_id),
            blocks: vec![Block::paragraph(
                "⚠ BOT failed: worker exploded stack line two"
            )],
            thread: Some(1),
            as_agent: Some("bot".into()),
        }],
        "one threaded ⚠ reply, authored as the agent"
    );
    commit(&mut m);
    assert_eq!(get_pending(&m, &run_id), None, "the entry pruned");

    // a redelivered result finds no entry: no second post, breadcrumb
    // only — the one-reply-per-run dedup holds.
    let mut ctx = CaptureCtx::new()
        .at(21)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", thread_transcript);
    exec(
        &mut m,
        &mut ctx,
        &result_event(&run_id, Err("worker exploded\nstack line two".into())),
    )
    .unwrap();
    assert!(ctx.msgs.is_empty(), "a redelivery must never double-post");
    let breadcrumbs: Vec<String> = ctx
        .events
        .iter()
        .map(|e| String::from_utf8_lossy(&e.payload).into_owned())
        .collect();
    assert!(breadcrumbs.iter().any(|b| b.contains("unknown dispatch")));
}

#[test]
fn a_failure_reply_requires_the_chat_post_grant() {
    // without chat.post the pre-existing silence holds: breadcrumbs only,
    // never a post the validator could not have proven postable.
    let (mut m, registry, run_id) = awaiting_run(&[]);
    let mut ctx = CaptureCtx::new()
        .at(20)
        .with_dispatch_origin()
        .with_registry(&registry);
    exec(
        &mut m,
        &mut ctx,
        &result_event(&run_id, Err("timed out".into())),
    )
    .unwrap();
    assert!(ctx.msgs.is_empty(), "no grant, no failure post");
    let breadcrumbs: Vec<String> = ctx
        .events
        .iter()
        .map(|e| String::from_utf8_lossy(&e.payload).into_owned())
        .collect();
    assert!(
        breadcrumbs
            .iter()
            .any(|b| b.contains("failure not surfaced") && b.contains(ACTION_CHAT_POST)),
        "the silence leaves its reason as a breadcrumb: {breadcrumbs:?}"
    );
    commit(&mut m);
    assert_eq!(get_pending(&m, &run_id), None, "the entry still pruned");
}

#[test]
fn failure_excerpts_are_single_line_and_bounded() {
    assert_eq!(
        failure_excerpt("line one\n\n  line two\tend"),
        "line one line two end"
    );
    assert_eq!(failure_excerpt("  \n \t "), "no error detail");
    let long = "x".repeat(FAILURE_EXCERPT_BYTES * 2);
    let bounded = failure_excerpt(&long);
    assert!(bounded.len() <= FAILURE_EXCERPT_BYTES + '…'.len_utf8());
    assert!(bounded.ends_with('…'));
}
