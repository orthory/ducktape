use super::*;

// ---- faceted delivery -------------------------------------------------------

/// build a faceted RunnerResult wrapper: the three core fields plus whatever
/// facet keys `facets` carries (data / effects / sink / status, and a
/// `workspace_receipt` override when present).
fn runner_wrapper(response_text: &str, facets: serde_json::Value) -> Vec<u8> {
    let mut obj = serde_json::json!({
        "ducktape_runner_result": 1,
        "response_text": response_text,
        "workspace_receipt": {
            "source_prefix": "/shared/agent-workspaces/bot",
            "source_snapshot": null,
            "output_snapshot": null,
            "commit_height": null,
            "rebased": false,
            "no_changes": true
        }
    });
    if let serde_json::Value::Object(extra) = facets {
        let base = obj.as_object_mut().expect("object");
        for (k, v) in extra {
            base.insert(k, v);
        }
    }
    serde_json::to_vec(&obj).expect("wrapper serializes")
}

/// a module wired with the forge sink, one watch on "general", one engaged
/// run for agent "bot" at seq 2.
fn awaiting_run_with_forge(registry: &Registry) -> (RunsModule, String) {
    let mut m = module().with_sink_forge("forge");
    let mut ctx = CaptureCtx::new()
        .with_origin(user(9))
        .with_registry(registry);
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
    engage_post(&mut m, registry, 2, &[]);
    commit(&mut m);
    (m, run_id_for("general", 2, "bot"))
}

#[test]
fn a_plain_result_delivers_its_prose_and_parsed_actions() {
    // a bare response_text (no runner marker, no facets) flows through the
    // single delivery path: the message is delivered and the prose-parsed
    // action is applied — exactly as today's message-only delivery did.
    let response_text = String::from_utf8(response(
        &["on it"],
        vec![AgentAction::CreateTask {
            task_id: "from_prose".into(),
            title: "prose".into(),
        }],
    ))
    .unwrap();
    let (mut m, registry, run_id) = awaiting_run(&[ACTION_CHAT_POST, ACTION_TASKS_CREATE]);
    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", transcript(2));
    exec(
        &mut m,
        &mut ctx,
        &result_event(&run_id, Ok(response_text.into_bytes())),
    )
    .unwrap();
    assert_eq!(ctx.chat_msgs().len(), 1, "the run delivers its message");
    assert_eq!(
        ctx.task_msgs(),
        vec![TaskMsg::CreateTask {
            task_id: "from_prose".into(),
            title: "prose".into(),
        }],
        "the prose-parsed action is applied"
    );
    assert!(
        ctx.msgs.iter().all(|msg| msg.target != "forge"),
        "a message-only result opens no sink"
    );
    commit(&mut m);
    assert_eq!(get_pending(&m, &run_id), None);
}

#[test]
fn effects_facet_applies_cap_checked() {
    // response_text is plain prose with NO action; the task write comes from
    // the host-assembled effects facet (R1).
    let (mut m, registry, run_id) = awaiting_run(&[ACTION_CHAT_POST, ACTION_TASKS_CREATE]);
    let facets = serde_json::json!({
        "effects": [{"kind":"tasks.create","task_id":"t1","title":"from effect"}]
    });
    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", transcript(2));
    exec(
        &mut m,
        &mut ctx,
        &result_event(&run_id, Ok(runner_wrapper("done", facets))),
    )
    .unwrap();
    assert_eq!(
        ctx.task_msgs(),
        vec![TaskMsg::CreateTask {
            task_id: "t1".into(),
            title: "from effect".into(),
        }]
    );
    commit(&mut m);
    assert_eq!(get_pending(&m, &run_id), None);
}

#[test]
fn unknown_effect_kind_fails_the_run() {
    let (mut m, registry, run_id) = awaiting_run(&[ACTION_CHAT_POST, ACTION_TASKS_CREATE]);
    let facets = serde_json::json!({
        "effects": [{"kind":"forge.delete_universe","task_id":"t1"}]
    });
    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", transcript(2));
    exec(
        &mut m,
        &mut ctx,
        &result_event(&run_id, Ok(runner_wrapper("done", facets))),
    )
    .unwrap();
    assert!(
        ctx.task_msgs().is_empty(),
        "no task write escapes a failed run"
    );
    assert!(
        ctx.events
            .iter()
            .any(|e| String::from_utf8_lossy(&e.payload).contains("unknown effect kind")),
        "the failure names the unknown effect kind"
    );
    commit(&mut m);
    assert_eq!(get_pending(&m, &run_id), None);
}

#[test]
fn empty_effects_falls_back_to_response_parsed_actions() {
    // critic #4 fallback: with an EMPTY effects facet, a model that emitted
    // the action only in prose still gets it applied — never a silent drop.
    let (mut m, registry, run_id) = awaiting_run(&[ACTION_CHAT_POST, ACTION_TASKS_CREATE]);
    let response_text = String::from_utf8(response(
        &["on it"],
        vec![AgentAction::CreateTask {
            task_id: "t1".into(),
            title: "from prose".into(),
        }],
    ))
    .unwrap();
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
            Ok(runner_wrapper(&response_text, serde_json::json!({}))),
        ),
    )
    .unwrap();
    assert_eq!(
        ctx.task_msgs(),
        vec![TaskMsg::CreateTask {
            task_id: "t1".into(),
            title: "from prose".into(),
        }]
    );
}

#[test]
fn pr_sink_emits_open_pr_only_with_the_forge_push_cap() {
    let sink = serde_json::json!({
        "sink": {"mode":"pr","repo":"app","source_branch":"agent/x","target_branch":"main","title":"My PR","body":"details"}
    });

    // (1) GRANTED forge_push (D3 cap) + branch born → OpenPr emitted.
    let mut granted = registry(&[("bot", &[ACTION_CHAT_POST])]);
    granted.get_mut("bot").unwrap().caps.forge_push = vec!["app".into()];
    let (mut m, run_id) = awaiting_run_with_forge(&granted);
    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&granted)
        .with_transcript("general", transcript(2))
        .with_forge_ref("app", "agent/x");
    exec(
        &mut m,
        &mut ctx,
        &result_event(&run_id, Ok(runner_wrapper("done", sink.clone()))),
    )
    .unwrap();
    let forge_ops: Vec<_> = ctx.msgs.iter().filter(|m| m.target == "forge").collect();
    assert_eq!(forge_ops.len(), 1, "one OpenPr emitted");
    assert_eq!(
        forge::decode_msg(&forge_ops[0].payload).unwrap(),
        forge::ForgeMsg::OpenPr {
            repo: "app".into(),
            title: "My PR".into(),
            body: "details".into(),
            source_branch: "agent/x".into(),
            target_branch: "main".into(),
        }
    );

    // (2) NO forge_push cap → degrade to a breadcrumb, no forge op, no abort.
    let ungranted = registry(&[("bot", &[ACTION_CHAT_POST])]);
    let (mut m2, run_id2) = awaiting_run_with_forge(&ungranted);
    let mut ctx2 = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&ungranted)
        .with_transcript("general", transcript(2))
        .with_forge_ref("app", "agent/x");
    exec(
        &mut m2,
        &mut ctx2,
        &result_event(&run_id2, Ok(runner_wrapper("done", sink))),
    )
    .unwrap();
    assert!(
        ctx2.msgs.iter().all(|m| m.target != "forge"),
        "no cap → no forge op"
    );
    assert!(
        ctx2.events
            .iter()
            .any(|e| String::from_utf8_lossy(&e.payload).contains("lacks forge_push")),
        "the breadcrumb names the missing cap"
    );
    assert_eq!(
        ctx2.chat_msgs().len(),
        1,
        "the run still delivers its message"
    );
}

#[test]
fn pr_sink_with_empty_required_fields_degrades_without_emitting_forge_op() {
    let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
    let (mut m, run_id) = awaiting_run_with_forge(&registry);
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
                    "sink": {
                        "mode": "pr",
                        "repo": "",
                        "source_branch": "agent/x",
                        "target_branch": "",
                        "title": "t",
                        "body": ""
                    }
                }),
            )),
        ),
    )
    .unwrap();
    assert!(
        ctx.msgs.iter().all(|m| m.target != "forge"),
        "incomplete pr sink must not emit an OpenPr"
    );
    assert!(
        ctx.events
            .iter()
            .any(|e| String::from_utf8_lossy(&e.payload).contains("incomplete pr sink")),
        "the breadcrumb names the incomplete pr sink"
    );
    assert_eq!(
        ctx.chat_msgs().len(),
        1,
        "the run still delivers its message"
    );
}

#[test]
fn pr_sink_with_an_unborn_branch_degrades_without_aborting() {
    let mut granted = registry(&[("bot", &[ACTION_CHAT_POST])]);
    granted.get_mut("bot").unwrap().caps.forge_push = vec!["app".into()];
    let (mut m, run_id) = awaiting_run_with_forge(&granted);
    // no with_forge_ref → the source branch is NOT born in committed forge.
    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&granted)
        .with_transcript("general", transcript(2));
    exec(
            &mut m,
            &mut ctx,
            &result_event(
                &run_id,
                Ok(runner_wrapper(
                    "done",
                    serde_json::json!({"sink":{"mode":"pr","repo":"app","source_branch":"agent/x","target_branch":"main","title":"PR"}}),
                )),
            ),
        )
        .unwrap();
    assert!(
        ctx.msgs.iter().all(|m| m.target != "forge"),
        "an unborn source branch must never emit an OpenPr (no-fail rule)"
    );
    assert!(
        ctx.events
            .iter()
            .any(|e| String::from_utf8_lossy(&e.payload).contains("source branch not present"))
    );
}

#[test]
fn malformed_facet_fails_the_run_without_aborting() {
    // effects is not an array → decode_run_result_v1 fails → the run fails
    // deterministically (R4), never a delivery-block abort.
    let (mut m, registry, run_id) = awaiting_run(&[ACTION_CHAT_POST]);
    let bad = serde_json::json!({
        "ducktape_runner_result": 1,
        "response_text": "hi",
        "workspace_receipt": {"source_prefix":"p","source_snapshot":null,"output_snapshot":null,"commit_height":null,"rebased":false,"no_changes":false},
        "effects": "not-an-array"
    });
    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", transcript(2));
    // exec returns Ok — the block commits — but the run FAILED.
    exec(
        &mut m,
        &mut ctx,
        &result_event(&run_id, Ok(serde_json::to_vec(&bad).unwrap())),
    )
    .unwrap();
    assert!(
        ctx.events
            .iter()
            .any(|e| String::from_utf8_lossy(&e.payload).contains("malformed")),
        "a malformed facet fails the run loudly"
    );
    assert_eq!(
        ctx.chat_msgs().len(),
        1,
        "the failure surfaces as a threaded reply"
    );
    commit(&mut m);
    assert_eq!(get_pending(&m, &run_id), None);
}

#[test]
fn status_failed_overrides_a_present_message() {
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
            Ok(runner_wrapper(
                "a perfectly good message",
                serde_json::json!({"status":"failed"}),
            )),
        ),
    )
    .unwrap();
    assert!(
        ctx.events
            .iter()
            .any(|e| String::from_utf8_lossy(&e.payload).contains("failed status")),
        "a failed status fails the run despite the present message"
    );
}

#[test]
fn job_finalize_is_a_delivery_receipt_with_data_and_output_ref() {
    let registry = job_registry(); // agent "duck" with tasks.create
    let mut m = module();
    let mut ctx = CaptureCtx::new()
        .at(3)
        .with_jobs_origin()
        .with_registry(&registry);
    exec(&mut m, &mut ctx, &jobs_event("job-1", "agent/duck", "spec")).unwrap();
    commit(&mut m);
    let run_id = job_run_id_for("job-1", "duck", 3);

    let facets = serde_json::json!({
        "workspace_receipt": {"source_prefix":"/ws/duck","source_snapshot":null,"output_snapshot":"deadbeef","commit_height":7,"rebased":false,"no_changes":false},
        "data": "{\"summary\":\"ok\"}",
        "effects": [{"kind":"tasks.create","task_id":"t1","title":"todo"}],
        "status": "ok"
    });
    let mut ctx = CaptureCtx::new()
        .at(10)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_claimed_job("job-1", 3);
    exec(
        &mut m,
        &mut ctx,
        &result_event(&run_id, Ok(runner_wrapper("done", facets))),
    )
    .unwrap();

    // the effects facet applied a task write.
    assert_eq!(
        ctx.task_msgs(),
        vec![TaskMsg::CreateTask {
            task_id: "t1".into(),
            title: "todo".into(),
        }]
    );
    // the finalize payload is a faceted DeliveryReceipt (not a bare response).
    let finalize = ctx.job_msgs();
    assert_eq!(finalize.len(), 1);
    let JobsMsg::Finalize { ok, payload, .. } = &finalize[0] else {
        panic!("expected a finalize");
    };
    assert!(*ok);
    let v: serde_json::Value = serde_json::from_str(payload).unwrap();
    assert_eq!(v["ducktape_delivery"], 1);
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"], "{\"summary\":\"ok\"}");
    assert_eq!(v["output_ref"]["output_snapshot"], "deadbeef");
    assert_eq!(v["output_ref"]["commit_height"], 7);
    assert_eq!(v["output_ref"]["source_prefix"], "/ws/duck");
}

#[test]
fn wire_sink_defaults_to_chain_and_decodes_a_present_pr() {
    // a MISSING sink field → Chain (internal-tag + serde default interplay).
    let no_sink = runner_wrapper("hi", serde_json::json!({}));
    assert!(matches!(
        decode_run_result_v1(&no_sink).unwrap().sink,
        WireSink::Chain
    ));
    // a present {"mode":"pr",...} → Pr.
    let pr = runner_wrapper(
        "hi",
        serde_json::json!({"sink":{"mode":"pr","repo":"a","source_branch":"s","title":"t"}}),
    );
    assert!(matches!(
        decode_run_result_v1(&pr).unwrap().sink,
        WireSink::Pr { .. }
    ));
    // an unsupported wrapper version fails to decode (R4).
    let badv = serde_json::json!({
        "ducktape_runner_result": 99,
        "response_text": "x",
        "workspace_receipt": {"source_prefix":"p","source_snapshot":null,"output_snapshot":null,"commit_height":null,"rebased":false,"no_changes":false}
    });
    assert!(decode_run_result_v1(&serde_json::to_vec(&badv).unwrap()).is_err());
}

#[test]
fn forge_sink_mirror_matches_forge_decode_msg() {
    // pin the local ForgeSinkMsg mirror against the real forge decoder so the
    // wire cannot silently drift (the reason forge is a dev-dependency).
    let bytes = forge_open_pr_bytes("app", "T", "B", "agent/x", "main");
    assert_eq!(
        forge::decode_msg(&bytes).unwrap(),
        forge::ForgeMsg::OpenPr {
            repo: "app".into(),
            title: "T".into(),
            body: "B".into(),
            source_branch: "agent/x".into(),
            target_branch: "main".into(),
        }
    );
}
