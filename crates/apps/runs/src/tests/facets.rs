use super::*;

// ---- faceted delivery -------------------------------------------------------

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
    // a facet-free wrapper (prose only, no effects/sink/data) flows through
    // the single delivery path: the message is delivered and the PROSE-parsed
    // action is applied (the effects-facet fallback).
    let response_text = String::from_utf8(response_json(
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
        &result_event(&run_id, Ok(runner_wrapper(&response_text, serde_json::json!({})))),
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
    let response_text = String::from_utf8(response_json(
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
    // the wire sink echoes title/body — delivery IGNORES them and derives
    // both from the message facet (asserted exactly below).
    let sink = serde_json::json!({
        "sink": {"mode":"pr","repo":"app","source_branch":"agent/x","target_branch":"main","title":"My PR","body":"details"}
    });

    // (1) GRANTED forge_push (D3 cap) + both branches born → OpenPr emitted.
    let mut granted = registry(&[("bot", &[ACTION_CHAT_POST])]);
    granted.get_mut("bot").unwrap().caps.forge_push = vec!["app".into()];
    let (mut m, run_id) = awaiting_run_with_forge(&granted);
    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&granted)
        .with_transcript("general", transcript(2))
        .with_forge_ref("app", "agent/x")
        .with_forge_ref("app", "main");
    exec(
        &mut m,
        &mut ctx,
        &result_event(&run_id, Ok(runner_wrapper("done", sink.clone()))),
    )
    .unwrap();
    let forge_ops: Vec<_> = ctx.msgs.iter().filter(|m| m.target == "forge").collect();
    assert_eq!(forge_ops.len(), 1, "one OpenPr emitted");
    // title = first line of the message facet; body = the full message +
    // the receipt breadcrumb block (run id, output_ref, executing node).
    // the default wrapper receipt is a no-changes duckfs receipt and no
    // saga record is seeded, so output/node degrade honestly.
    assert_eq!(
        forge::decode_msg(&forge_ops[0].payload).unwrap(),
        forge::ForgeMsg::OpenPr {
            repo: "app".into(),
            title: "done".into(),
            body: format!(
                "done\n\n---\nrun: {run_id}\noutput: none (no changes this run)\nnode: unknown"
            ),
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
fn pr_sink_with_an_unborn_target_branch_degrades_without_aborting() {
    // forge rejects an OpenPr whose TARGET branch is unborn in committed
    // refs, and a rejected follow-up aborts the whole delivery block — the
    // sink must skip with a breadcrumb instead (R4). repro: the target (e.g.
    // "dev") was deleted after earlier work, then the item is re-mentioned.
    let (mut m, granted, run_id) = forge_push_run();
    // source born, target "main" NOT born.
    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&granted)
        .with_transcript("general", transcript(2))
        .with_forge_ref("app", "agent/x");
    exec(
        &mut m,
        &mut ctx,
        &result_event(&run_id, Ok(forge_wrapper("done", None, None, true, None))),
    )
    .unwrap();
    assert!(
        ctx.msgs.iter().all(|m| m.target != "forge"),
        "an unborn target branch must never emit an OpenPr (no-fail rule)"
    );
    assert!(
        breadcrumbs(&ctx)
            .contains(&format!("run {run_id} pr sink skipped: target branch main not born")),
        "the breadcrumb names the unborn target: {:?}",
        breadcrumbs(&ctx)
    );
    assert_eq!(ctx.chat_msgs().len(), 1, "the run still delivers its message");
    // the delivered-runs ring still records the run — just with no PR.
    commit(&mut m);
    let rec = &recent_runs(&m)[0];
    assert_eq!(rec.run_id, run_id);
    assert_eq!(rec.pr_number, None);
}

#[test]
fn pr_sink_with_source_equal_to_target_degrades_without_aborting() {
    // forge rejects an OpenPr whose source and target are the same branch —
    // degrade with a breadcrumb, never emit the aborting op.
    let (mut m, granted, run_id) = forge_push_run();
    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&granted)
        .with_transcript("general", transcript(2))
        .with_forge_ref("app", "agent/x");
    exec(
        &mut m,
        &mut ctx,
        &result_event(
            &run_id,
            Ok(runner_wrapper(
                "done",
                serde_json::json!({"sink":{"mode":"pr","repo":"app","source_branch":"agent/x","target_branch":"agent/x","title":""}}),
            )),
        ),
    )
    .unwrap();
    assert!(
        ctx.msgs.iter().all(|m| m.target != "forge"),
        "source==target must never emit an OpenPr (no-fail rule)"
    );
    assert!(
        breadcrumbs(&ctx).contains(&format!(
            "run {run_id} pr sink skipped: source and target are the same branch"
        )),
        "the breadcrumb names the malformed pair: {:?}",
        breadcrumbs(&ctx)
    );
    assert_eq!(ctx.chat_msgs().len(), 1, "the run still delivers its message");
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
    // a duckfs output_ref carries NO forge keys — the additive fields
    // skip-serialize, so pre-forge consumers see byte-identical shapes.
    let output_ref = v["output_ref"].as_object().unwrap();
    assert!(!output_ref.contains_key("branch"));
    assert!(!output_ref.contains_key("output_commit"));
}

#[test]
fn job_finalize_output_ref_carries_forge_coordinates() {
    // §5 distillation: a forge receipt (no snapshot, branch + output
    // commit) distills into an output_ref stating branch@oid, so the
    // app/jobs surface can render the forge coordinates.
    let registry = job_registry(); // agent "duck" with tasks.create
    let mut m = module();
    let mut ctx = CaptureCtx::new()
        .at(3)
        .with_jobs_origin()
        .with_registry(&registry);
    exec(&mut m, &mut ctx, &jobs_event("job-1", "agent/duck", "spec")).unwrap();
    commit(&mut m);
    let run_id = job_run_id_for("job-1", "duck", 3);

    let oid = "1a".repeat(20);
    let facets = serde_json::json!({
        "workspace_receipt": {
            "source_prefix": "forge:app",
            "source_snapshot": "2b".repeat(20),
            "output_snapshot": null,
            "commit_height": null,
            "rebased": false,
            "no_changes": false,
            "branch": "agent/item-7",
            "output_commit": oid,
        },
        "effects": [{"kind":"tasks.create","task_id":"t1","title":"todo"}],
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
    let finalize = ctx.job_msgs();
    assert_eq!(finalize.len(), 1);
    let JobsMsg::Finalize { payload, .. } = &finalize[0] else {
        panic!("expected a finalize");
    };
    let v: serde_json::Value = serde_json::from_str(payload).unwrap();
    assert_eq!(v["output_ref"]["source_prefix"], "forge:app");
    assert!(v["output_ref"]["output_snapshot"].is_null());
    assert_eq!(v["output_ref"]["branch"], "agent/item-7");
    assert_eq!(v["output_ref"]["output_commit"], serde_json::json!(oid));
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

/// a registry whose one agent "bot" may chat and push to "app", plus an
/// awaiting run — the PR-sink happy-path scaffold.
fn forge_push_run() -> (RunsModule, Registry, String) {
    let mut granted = registry(&[("bot", &[ACTION_CHAT_POST])]);
    granted.get_mut("bot").unwrap().caps.forge_push = vec!["app".into()];
    let (m, run_id) = awaiting_run_with_forge(&granted);
    (m, granted, run_id)
}

/// a faceted wrapper whose receipt is a forge receipt (§5 shape).
fn forge_wrapper(
    response_text: &str,
    branch: Option<&str>,
    output_commit: Option<&str>,
    no_changes: bool,
    commit_error: Option<&str>,
) -> Vec<u8> {
    let mut receipt = serde_json::json!({
        "source_prefix": "forge:app",
        "source_snapshot": "2b".repeat(20),
        "output_snapshot": null,
        "commit_height": null,
        "rebased": false,
        "no_changes": no_changes,
    });
    let map = receipt.as_object_mut().unwrap();
    if let Some(b) = branch {
        map.insert("branch".into(), b.into());
    }
    if let Some(oid) = output_commit {
        map.insert("output_commit".into(), oid.into());
    }
    if let Some(e) = commit_error {
        map.insert("commit_error".into(), e.into());
    }
    runner_wrapper(
        response_text,
        serde_json::json!({
            "workspace_receipt": receipt,
            "sink": {"mode":"pr","repo":"app","source_branch":"agent/x","target_branch":"main","title":"","body":""},
            "status": if commit_error.is_some() { "degraded" } else { "ok" },
        }),
    )
}

fn breadcrumbs(ctx: &CaptureCtx) -> Vec<String> {
    ctx.events
        .iter()
        .map(|e| String::from_utf8_lossy(&e.payload).into_owned())
        .collect()
}

#[test]
fn pr_sink_derives_title_and_body_from_the_message_facet_and_forge_receipt() {
    // the full derivation: title = first line of the message facet, body =
    // the whole facet + the receipt breadcrumb (run id, branch@oid, the
    // executing node from the run's Done saga record).
    let (mut m, granted, run_id) = forge_push_run();
    let oid = "1a".repeat(20);
    let saga_id = sink::saga_id_for_dispatch("runs", &dispatch_id_for(&run_id));
    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&granted)
        .with_transcript("general", transcript(2))
        .with_forge_ref("app", "agent/x")
        .with_forge_ref("app", "main")
        .with_saga_assignee(&saga_id, &[0xab; 32]);
    let message = "Fix the flaky gate: retry twice\n\nDetails in the diff.";
    exec(
        &mut m,
        &mut ctx,
        &result_event(
            &run_id,
            Ok(forge_wrapper(message, Some("agent/x"), Some(&oid), false, None)),
        ),
    )
    .unwrap();
    let forge_ops: Vec<_> = ctx.msgs.iter().filter(|m| m.target == "forge").collect();
    assert_eq!(forge_ops.len(), 1, "one OpenPr emitted");
    assert_eq!(
        forge::decode_msg(&forge_ops[0].payload).unwrap(),
        forge::ForgeMsg::OpenPr {
            repo: "app".into(),
            title: "Fix the flaky gate: retry twice".into(),
            body: format!(
                "{message}\n\n---\nrun: {run_id}\noutput: agent/x@{oid}\nnode: {}",
                "ab".repeat(32)
            ),
            source_branch: "agent/x".into(),
            target_branch: "main".into(),
        }
    );
    // the delivered-runs ring observes the same delivery: the forge
    // output ref and the number the fresh OpenPr gets (empty tracker → 1).
    commit(&mut m);
    let rec = &recent_runs(&m)[0];
    assert_eq!(rec.output_ref, Some(format!("agent/x@{oid}")));
    assert_eq!(rec.pr_number, Some(1));
    assert_eq!(rec.executing_node, "ab".repeat(32));
}

#[test]
fn pr_sink_skips_an_open_pr_with_the_same_source_and_notes_the_update() {
    // the duplicate-PR guard: an OPEN PR whose source branch matches the
    // sink's ⇒ no OpenPr — the branch update WAS the feedback.
    let (mut m, granted, run_id) = forge_push_run();
    let oid = "1a".repeat(20);
    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&granted)
        .with_transcript("general", transcript(2))
        .with_forge_ref("app", "agent/x")
        .with_forge_item("app", forge_pr(4, "existing", "", "agent/x", "main"));
    exec(
        &mut m,
        &mut ctx,
        &result_event(
            &run_id,
            Ok(forge_wrapper("done", Some("agent/x"), Some(&oid), false, None)),
        ),
    )
    .unwrap();
    assert!(
        ctx.msgs.iter().all(|m| m.target != "forge"),
        "an open PR with the same source must not be re-opened"
    );
    assert!(
        breadcrumbs(&ctx).contains(&format!("run {run_id} pr sink: updated PR #4")),
        "the breadcrumb names the updated PR: {:?}",
        breadcrumbs(&ctx)
    );
    // the ring records the guard-found PR as this run's pr_number.
    commit(&mut m);
    assert_eq!(recent_runs(&m)[0].pr_number, Some(4));
}

#[test]
fn pr_sink_guard_wording_is_honest_when_nothing_was_pushed() {
    // guard hit + a no_changes receipt: nothing was pushed — say so.
    let (mut m, granted, run_id) = forge_push_run();
    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&granted)
        .with_transcript("general", transcript(2))
        .with_forge_ref("app", "agent/x")
        .with_forge_item("app", forge_pr(4, "existing", "", "agent/x", "main"));
    exec(
        &mut m,
        &mut ctx,
        &result_event(&run_id, Ok(forge_wrapper("done", None, None, true, None))),
    )
    .unwrap();
    assert!(ctx.msgs.iter().all(|m| m.target != "forge"));
    assert!(
        breadcrumbs(&ctx)
            .contains(&format!("run {run_id} pr sink: PR #4 already open, no changes pushed")),
        "honest no-changes wording: {:?}",
        breadcrumbs(&ctx)
    );

    // guard hit + a commit_error receipt: the workspace commit failed.
    let (mut m2, granted2, run_id2) = forge_push_run();
    let mut ctx2 = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&granted2)
        .with_transcript("general", transcript(2))
        .with_forge_ref("app", "agent/x")
        .with_forge_item("app", forge_pr(4, "existing", "", "agent/x", "main"));
    exec(
        &mut m2,
        &mut ctx2,
        &result_event(
            &run_id2,
            Ok(forge_wrapper("done", Some("agent/x"), None, false, Some("push CAS reject"))),
        ),
    )
    .unwrap();
    assert!(ctx2.msgs.iter().all(|m| m.target != "forge"));
    assert!(
        breadcrumbs(&ctx2).contains(&format!(
            "run {run_id2} pr sink: PR #4 already open, nothing pushed (workspace commit failed)"
        )),
        "honest commit-error wording: {:?}",
        breadcrumbs(&ctx2)
    );
}

#[test]
fn pr_sink_guard_ignores_closed_prs_issues_and_other_sources() {
    // a CLOSED PR on the same source, an open PR on another source, and an
    // open issue are all non-hits: OpenPr fires (re-proposing existing work
    // after a PR was closed is intended).
    let (mut m, granted, run_id) = forge_push_run();
    let mut closed = forge_pr(3, "closed", "", "agent/x", "main");
    closed.summary.state = forge::ItemState::Closed;
    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&granted)
        .with_transcript("general", transcript(2))
        .with_forge_ref("app", "agent/x")
        .with_forge_ref("app", "main")
        .with_forge_item("app", closed)
        .with_forge_item("app", forge_pr(5, "other", "", "other/y", "main"))
        .with_forge_item("app", forge_issue(6, "an issue", ""));
    let oid = "1a".repeat(20);
    exec(
        &mut m,
        &mut ctx,
        &result_event(
            &run_id,
            Ok(forge_wrapper("done", Some("agent/x"), Some(&oid), false, None)),
        ),
    )
    .unwrap();
    assert_eq!(
        ctx.msgs.iter().filter(|m| m.target == "forge").count(),
        1,
        "no open PR matches the source — OpenPr fires"
    );
    // the ring records the number the fresh OpenPr gets: committed max
    // item number (issue 6) + 1.
    commit(&mut m);
    assert_eq!(recent_runs(&m)[0].pr_number, Some(7));
}

#[test]
fn a_no_changes_run_with_a_born_branch_and_no_open_pr_still_opens_the_pr() {
    // plan-literal: the branch is born (earlier session work) and no PR is
    // open for it — OpenPr fires even though THIS run pushed nothing.
    let (mut m, granted, run_id) = forge_push_run();
    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&granted)
        .with_transcript("general", transcript(2))
        .with_forge_ref("app", "agent/x")
        .with_forge_ref("app", "main");
    exec(
        &mut m,
        &mut ctx,
        &result_event(&run_id, Ok(forge_wrapper("re-proposing", None, None, true, None))),
    )
    .unwrap();
    let forge_ops: Vec<_> = ctx.msgs.iter().filter(|m| m.target == "forge").collect();
    assert_eq!(forge_ops.len(), 1, "OpenPr fires for a born branch with no open PR");
    let forge::ForgeMsg::OpenPr { body, .. } = forge::decode_msg(&forge_ops[0].payload).unwrap()
    else {
        panic!("expected an OpenPr");
    };
    assert!(
        body.contains("output: none (no changes this run)"),
        "the body is honest about the empty push: {body:?}"
    );
}

#[test]
fn saga_id_mirror_matches_the_dispatch_modules_derivation() {
    // pin the executing-node lookup's saga-id mirror against the REAL
    // dispatch module: register a recipe, dispatch, and read the saga id
    // off the emitted trigger — the mirror must derive the same id.
    let mut d = dispatch::DispatchModule::new("dispatch", "saga");
    let mut ctx = CaptureCtx::new().with_origin(Origin::Module("runs".into()));
    block_on(d.execute(
        &mut ctx,
        &Msg {
            target: "dispatch".into(),
            payload: dispatch_encode_msg(&DispatchMsg::RegisterRecipe {
                recipe_id: "agent/bot".into(),
                description: "runs for agent bot".into(),
                capability: "model-1".into(),
                routing: Routing::Rendezvous,
                output_contract: OutputContract::Text,
                max_attempts: 1,
                deadline_views: None,
                lease_views: None,
            }),
        },
    ))
    .unwrap();
    block_on(d.execute(
        &mut ctx,
        &Msg {
            target: "dispatch".into(),
            payload: dispatch_encode_msg(&DispatchMsg::Dispatch {
                dispatch_id: "d1".into(),
                recipe_id: "agent/bot".into(),
                payload: b"in".to_vec(),
                demands: Default::default(),
            }),
        },
    ))
    .unwrap();
    let trigger = ctx
        .msgs
        .iter()
        .find(|m| m.target == "saga")
        .expect("the dispatch emits a saga trigger");
    let saga::SagaMsg::Trigger { saga_id, .. } = saga::decode_msg(&trigger.payload).unwrap()
    else {
        panic!("expected a saga trigger");
    };
    assert_eq!(saga_id, sink::saga_id_for_dispatch("runs", "d1"));
}

#[test]
fn workspace_receipt_mirror_decodes_the_forge_fields() {
    // present: the §5 additive fields land on the mirror.
    let wrapper = forge_wrapper("done", Some("agent/item-7"), Some(&"1a".repeat(20)), false, None);
    let receipt = decode_run_result_v1(&wrapper).unwrap().workspace_receipt;
    assert_eq!(receipt.branch.as_deref(), Some("agent/item-7"));
    assert_eq!(receipt.output_commit.as_deref(), Some(&*"1a".repeat(20)));

    // absent (every pre-forge receipt): serde defaults, not an error.
    let receipt = decode_run_result_v1(&runner_wrapper("done", serde_json::json!({})))
        .unwrap()
        .workspace_receipt;
    assert_eq!(receipt.branch, None);
    assert_eq!(receipt.output_commit, None);
}
