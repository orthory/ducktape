use super::*;

// ---- the result intake ----------------------------------------------------------

#[test]
fn a_valid_response_emits_the_reply_and_actions_and_prunes_the_entry() {
    let (mut m, registry, run_id) = awaiting_run(&[
        ACTION_CHAT_POST,
        ACTION_TASKS_CREATE,
        ACTION_TASKS_UPDATE_STATUS,
    ]);
    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", transcript(2));
    exec(
        &mut m,
        &mut ctx,
        &result_event(
            &run_id,
            Ok(response(
                &["on it"],
                vec![
                    AgentAction::CreateTask {
                        task_id: "t1".into(),
                        title: "ship it".into(),
                    },
                    // updating a task created earlier in this SAME response
                    // is valid — tasks applies the follow-ups in order.
                    AgentAction::UpdateTaskStatus {
                        task_id: "t1".into(),
                        status: "in_progress".into(),
                    },
                ],
            )),
        ),
    )
    .unwrap();
    commit(&mut m);

    assert_eq!(
        get_pending(&m, &run_id),
        None,
        "the delivered entry pruned — the dispatch module holds the history"
    );
    assert_eq!(
        ctx.chat_msgs(),
        vec![ChatMsg::PostMessage {
            channel_id: "general".into(),
            message_id: reply_message_id(&run_id),
            blocks: vec![Block::paragraph("on it")],
            thread: None,
            as_agent: Some("bot".into()),
        }],
        "the reply posts as the AGENT, under the run's message id"
    );
    assert_eq!(
        ctx.task_msgs(),
        vec![
            TaskMsg::CreateTask {
                task_id: "t1".into(),
                title: "ship it".into(),
            },
            TaskMsg::UpdateStatus {
                task_id: "t1".into(),
                status: TaskStatus::InProgress,
            },
        ]
    );
}

#[test]
fn a_threaded_anchor_threads_the_reply() {
    let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
    let mut m = watched(TurnPolicy::All, &registry);
    // seq 3 is a reply to root 1; the pin records thread_root = 1.
    let mut thread_transcript = transcript(2);
    thread_transcript.push(message_in(
        "general",
        3,
        AuthorRef::User(vec![1; 32]),
        "in thread",
        Some(1),
    ));
    let mut ctx = CaptureCtx::new()
        .at(3)
        .with_tagging_origin()
        .with_registry(&registry)
        .with_transcript("general", thread_transcript.clone());
    exec(&mut m, &mut ctx, &engagement("general", 3, vec![])).unwrap();
    commit(&mut m);
    let run_id = run_id_for("general", 3, "bot");
    assert_eq!(get_pending(&m, &run_id).unwrap().thread_root, Some(1));
    // the envelope keys thread continuity by the ROOT, not the anchor.
    let dispatches = ctx.dispatch_msgs();
    let DispatchMsg::Dispatch { payload, .. } = &dispatches[0] else {
        panic!("expected a dispatch");
    };
    let envelope: serde_json::Value = serde_json::from_slice(payload).unwrap();
    assert_eq!(envelope["thread_key"], "general#1");

    let mut ctx = CaptureCtx::new()
        .at(9)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", thread_transcript);
    exec(
        &mut m,
        &mut ctx,
        &result_event(&run_id, Ok(response(&["answered"], vec![]))),
    )
    .unwrap();
    commit(&mut m);
    let posts = ctx.chat_msgs();
    assert_eq!(posts.len(), 1);
    let ChatMsg::PostMessage { thread, .. } = &posts[0] else {
        panic!("expected a post");
    };
    assert_eq!(*thread, Some(1), "the reply joins the anchor's thread");
}

#[test]
fn invalid_responses_fail_the_run_and_surface_a_threaded_failure_reply() {
    // normalization already absorbed shape problems (prose, fences,
    // oversize); what remains failable is POLICY: task validity and
    // grants. every case emits NO follow-up except the ⚠ failure reply
    // (the agent here holds chat.post), leaves a breadcrumb, and prunes
    // the entry — never the block.
    let cases: Vec<(&str, Vec<u8>)> = vec![
        (
            "task already exists: t0",
            response(
                &["ok"],
                vec![AgentAction::CreateTask {
                    task_id: "t0".into(),
                    title: "dup of a committed task".into(),
                }],
            ),
        ),
        (
            "task already exists: fresh",
            response(
                &["ok"],
                vec![
                    AgentAction::CreateTask {
                        task_id: "fresh".into(),
                        title: "one".into(),
                    },
                    AgentAction::CreateTask {
                        task_id: "fresh".into(),
                        title: "two".into(),
                    },
                ],
            ),
        ),
        (
            "unknown task: ghost",
            response(
                &["ok"],
                vec![AgentAction::UpdateTaskStatus {
                    task_id: "ghost".into(),
                    status: "done".into(),
                }],
            ),
        ),
        (
            "unknown task status",
            response(
                &["ok"],
                vec![AgentAction::UpdateTaskStatus {
                    task_id: "t0".into(),
                    status: "shipped".into(),
                }],
            ),
        ),
        (
            "non-empty task_id",
            response(
                &["ok"],
                vec![AgentAction::CreateTask {
                    task_id: String::new(),
                    title: "x".into(),
                }],
            ),
        ),
    ];
    for (fragment, bytes) in cases {
        let (mut m, registry, run_id) = awaiting_run(&[
            ACTION_CHAT_POST,
            ACTION_TASKS_CREATE,
            ACTION_TASKS_UPDATE_STATUS,
        ]);
        let mut ctx = CaptureCtx::new()
            .at(8)
            .with_dispatch_origin()
            .with_registry(&registry)
            .with_transcript("general", transcript(2))
            .with_task("t0");
        exec(&mut m, &mut ctx, &result_event(&run_id, Ok(bytes))).unwrap();
        assert!(
            ctx.task_msgs().is_empty(),
            "an invalid response must emit no task writes ({fragment})"
        );
        let posts = ctx.chat_msgs();
        assert_eq!(posts.len(), 1, "exactly one failure reply ({fragment})");
        let ChatMsg::PostMessage {
            message_id,
            blocks,
            as_agent,
            ..
        } = &posts[0]
        else {
            panic!("expected a post");
        };
        assert_eq!(
            *message_id,
            reply_message_id(&run_id),
            "the failure reply holds the run's one reply id ({fragment})"
        );
        assert_eq!(*as_agent, Some("bot".into()));
        assert_eq!(blocks.len(), 1, "one ⚠ paragraph ({fragment})");
        let Block::Paragraph(spans) = &blocks[0] else {
            panic!("expected a paragraph");
        };
        let text: String = spans.iter().map(|s| s.text.as_str()).collect();
        assert!(
            text.starts_with("⚠ BOT failed: "),
            "the reply names the agent's display name: {text}"
        );
        assert!(
            text.contains(fragment),
            "the reply carries the reason excerpt: {text}"
        );
        let breadcrumbs: Vec<String> = ctx
            .events
            .iter()
            .map(|e| String::from_utf8_lossy(&e.payload).into_owned())
            .collect();
        assert!(
            breadcrumbs.iter().any(|b| b.contains(fragment)),
            "breadcrumbs {breadcrumbs:?} must mention {fragment:?}"
        );
        commit(&mut m);
        assert_eq!(
            get_pending(&m, &run_id),
            None,
            "the failed run's entry pruned ({fragment})"
        );
    }
}

#[test]
fn raw_model_text_normalizes_into_a_postable_reply() {
    // the oracle wraps the model's RAW text in the runner result; the
    // intake's deterministic normalization turns prose, empty JSON, and
    // oversized answers into valid replies instead of failed runs.
    let cases: Vec<Vec<u8>> = vec![
        runner_wrapper("just prose, no JSON anywhere", serde_json::json!({})),
        response(&[], vec![]),
        runner_wrapper(
            &"x".repeat(MAX_REPLY_BLOCKS_BYTES + 1),
            serde_json::json!({}),
        ),
    ];
    for bytes in cases {
        let (mut m, registry, run_id) = awaiting_run(&[ACTION_CHAT_POST]);
        let mut ctx = CaptureCtx::new()
            .at(8)
            .with_dispatch_origin()
            .with_registry(&registry)
            .with_transcript("general", transcript(2));
        exec(&mut m, &mut ctx, &result_event(&run_id, Ok(bytes))).unwrap();
        commit(&mut m);
        assert_eq!(get_pending(&m, &run_id), None, "the entry pruned");
        let posts = ctx.chat_msgs();
        assert_eq!(posts.len(), 1, "exactly one normalized reply posts");
    }
}

#[test]
fn oversized_actions_fail_the_run_deterministically() {
    // the byte peer of the count cap: one action carrying a huge payload
    // (a pasted-file title) must be a deterministic run failure — never an
    // oversized finalize payload the jobs board would byte-truncate into
    // invalid JSON.
    let (mut m, registry, run_id) = awaiting_run(&[ACTION_TASKS_CREATE]);
    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", transcript(2));
    let huge = response(
        &[],
        vec![AgentAction::CreateTask {
            task_id: "t1".into(),
            title: "x".repeat(MAX_ACTIONS_BYTES),
        }],
    );
    exec(&mut m, &mut ctx, &result_event(&run_id, Ok(huge))).unwrap();
    commit(&mut m);
    assert_eq!(
        get_pending(&m, &run_id),
        None,
        "the failed run's entry pruned"
    );
    let breadcrumbs: Vec<String> = ctx
        .events
        .iter()
        .map(|e| String::from_utf8_lossy(&e.payload).into_owned())
        .collect();
    assert!(
        breadcrumbs
            .iter()
            .any(|b| b.contains(&format!("the cap is {MAX_ACTIONS_BYTES}"))),
        "the failure names the byte cap: {breadcrumbs:?}"
    );
}

#[test]
fn code_blocks_survive_normalization_into_chat_blocks() {
    let (mut m, registry, run_id) = awaiting_run(&[ACTION_CHAT_POST]);
    let raw = r#"{"reply_blocks":[{"id":"b1","kind":"paragraph","text":"hello"},{"kind":"code","lang":"rust","text":"fn main() {}"},{"kind":"Alien","text":"dropped"},{"kind":"paragraph","text":"  "}],"actions":[]}"#;
    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", transcript(2));
    exec(
        &mut m,
        &mut ctx,
        &result_event(&run_id, Ok(runner_wrapper(raw, serde_json::json!({})))),
    )
    .unwrap();
    let posts = ctx.chat_msgs();
    let ChatMsg::PostMessage { blocks, .. } = &posts[0] else {
        panic!("expected a post");
    };
    assert_eq!(
        *blocks,
        vec![
            Block::paragraph("hello"),
            Block::Code {
                lang: Some("rust".into()),
                text: "fn main() {}".into(),
            },
        ],
        "known kinds map to chat blocks; unknown kinds and blank texts drop"
    );
}

#[test]
fn a_fenced_json_reply_is_parsed_into_prose_not_dumped_as_a_code_block() {
    // the observed failure: an agentic CLI wraps its AgentResponse in a
    // ```json fence despite the contract, the bare parse fails, and the
    // whole fenced string lands in chat as a raw code block. the tolerant
    // parser must recover the real prose.
    let (mut m, registry, run_id) = awaiting_run(&[ACTION_CHAT_POST]);
    let raw = "```json\n{\"reply_blocks\":[{\"kind\":\"paragraph\",\"text\":\"QUACKTEST! Hello there.\"}],\"actions\":[]}\n```";
    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", transcript(2));
    exec(
        &mut m,
        &mut ctx,
        &result_event(&run_id, Ok(runner_wrapper(raw, serde_json::json!({})))),
    )
    .unwrap();
    let posts = ctx.chat_msgs();
    let ChatMsg::PostMessage { blocks, .. } = &posts[0] else {
        panic!("expected a post");
    };
    assert_eq!(
        *blocks,
        vec![Block::paragraph("QUACKTEST! Hello there.")],
        "a fenced AgentResponse is decoded to its prose, never posted as raw JSON"
    );
}

#[test]
fn parse_strict_response_tolerates_the_shapes_llms_actually_emit() {
    let bare = r#"{"reply_blocks":[{"kind":"paragraph","text":"hi"}],"actions":[]}"#;
    assert_eq!(
        parse_strict_response(bare).unwrap().reply_blocks[0].text,
        "hi"
    );

    // a fence with an info string (```json), the reproduced case.
    let fenced = format!("```json\n{bare}\n```");
    assert_eq!(
        parse_strict_response(&fenced).unwrap().reply_blocks[0].text,
        "hi"
    );

    // a bare fence (```), no info string.
    let bare_fence = format!("```\n{bare}\n```");
    assert_eq!(
        parse_strict_response(&bare_fence).unwrap().reply_blocks[0].text,
        "hi"
    );

    // JSON with a trailing line of prose the model tacked on.
    let trailing = format!("{fenced}\nHope that helps!");
    assert_eq!(
        parse_strict_response(&trailing).unwrap().reply_blocks[0].text,
        "hi"
    );

    // genuine prose (no JSON object) does NOT parse — it must fall back to
    // the raw-text paragraph, not be swallowed.
    assert!(parse_strict_response("just a plain hello, no json here").is_none());
    assert!(parse_strict_response("   ").is_none());

    // a `}` before the first `{` must not panic the outermost-object span.
    assert!(parse_strict_response("close } then open { please").is_none());
}

#[test]
fn a_fenced_job_response_still_yields_actions_only() {
    // job runs drop reply_blocks; the fenced-parse path must still recover
    // the actions inside the fence.
    let raw = "```json\n{\"reply_blocks\":[{\"kind\":\"paragraph\",\"text\":\"noise\"}],\"actions\":[{\"create_task\":{\"task_id\":\"t1\",\"title\":\"did it\"}}]}\n```";
    let parsed = agent_response_from_text(raw, true);
    assert!(
        parsed.reply_blocks.is_empty(),
        "job runs post no chat reply"
    );
    assert_eq!(parsed.actions.len(), 1, "the fenced action is recovered");
}

// ---- chat.post_message on the settle path ------------------------------------

#[test]
fn a_post_message_action_lands_agent_authored_under_a_deterministic_id() {
    // the agent SPEAKING (its own channel, its own message) rather than
    // ANSWERING where it was engaged — one more action in the strict lane, and
    // one more chat post carrying `as_agent`.
    let (mut m, registry, run_id) = awaiting_run(&[ACTION_CHAT_POST, ACTION_CHAT_POST_MESSAGE]);
    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", transcript(2));
    exec(
        &mut m,
        &mut ctx,
        &result_event(
            &run_id,
            Ok(response(
                &["done"],
                vec![AgentAction::PostMessage {
                    channel_id: "general".into(),
                    text: "progress: halfway".into(),
                    thread: None,
                }],
            )),
        ),
    )
    .unwrap();
    commit(&mut m);

    let msgs = ctx.chat_msgs();
    assert_eq!(msgs.len(), 2, "the run's reply AND the agent's own post");
    assert_eq!(
        msgs[1],
        ChatMsg::PostMessage {
            channel_id: "general".into(),
            // the settle path numbers by the action's index in the response —
            // disjoint from the session lane's `s`-prefixed slots.
            message_id: post_message_id(&run_id, "0"),
            blocks: vec![Block::paragraph("progress: halfway")],
            thread: None,
            as_agent: Some("bot".into()),
        }
    );
    assert_ne!(
        msgs[1], msgs[0],
        "the post never squats the run's ONE reply id"
    );
}

#[test]
fn post_message_without_its_own_grant_fails_the_run() {
    // THE ESCALATION GUARD, on the settle path: `chat.post` authorizes the
    // reply and nothing more. an agent registered before this action existed
    // must not have been silently handed the wider power.
    let (mut m, registry, run_id) = awaiting_run(&[ACTION_CHAT_POST]);
    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", transcript(2));
    exec(
        &mut m,
        &mut ctx,
        &result_event(
            &run_id,
            Ok(response(
                &["done"],
                vec![AgentAction::PostMessage {
                    channel_id: "general".into(),
                    text: "sneaking in".into(),
                    thread: None,
                }],
            )),
        ),
    )
    .unwrap();

    // the strict lane: an ungranted action fails the RUN (never the block).
    assert!(
        ctx.notes()
            .iter()
            .any(|n| n.contains("not allowed to chat.post_message")),
        "{:?}",
        ctx.notes()
    );
    let posts = ctx.chat_msgs();
    assert_eq!(posts.len(), 1, "only the failure reply: {posts:?}");
    assert!(
        matches!(
            &posts[0],
            ChatMsg::PostMessage { message_id, .. } if *message_id == reply_message_id(&run_id)
        ),
        "the agent's own post never existed — only the run's failure reply"
    );
    commit(&mut m);
    assert_eq!(recent_runs(&m)[0].outcome, RunOutcome::Failed);
}

#[test]
fn a_post_message_effect_from_the_host_lane_decodes_and_threads() {
    // the host-assembled effects facet is authoritative when non-empty (R1), so
    // the new kind must decode there too — thread included.
    let (mut m, registry, run_id) = awaiting_run(&[ACTION_CHAT_POST, ACTION_CHAT_POST_MESSAGE]);
    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", transcript(2));
    exec(
        &mut m,
        &mut ctx,
        &result_event(
            &run_id,
            Ok(runner_wrapper(
                "done",
                serde_json::json!({
                    "effects": [{
                        "kind": ACTION_CHAT_POST_MESSAGE,
                        "channel_id": "general",
                        "text": "threaded update",
                        "thread": 1,
                    }]
                }),
            )),
        ),
    )
    .unwrap();

    let msgs = ctx.chat_msgs();
    assert_eq!(msgs.len(), 2);
    assert!(
        matches!(
            &msgs[1],
            ChatMsg::PostMessage { thread: Some(1), channel_id, .. } if channel_id == "general"
        ),
        "the effect's thread rides through: {:?}",
        msgs[1]
    );
}
