use super::*;

// ---- faceted delivery -------------------------------------------------------

/// a module wired with the forge sink, one watch on "general", one engaged
/// run for agent "bot" at seq 2.
fn awaiting_run_with_forge(registry: &Registry) -> (RunsModule, String) {
    let mut m = module().with_sink_forge("forge");
    m.models = registry.clone();
    commit(&mut m);
    request_post(&mut m, registry, 2, &[]);
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
        &result_event(
            &run_id,
            Ok(runner_wrapper(&response_text, serde_json::json!({}))),
        ),
    )
    .unwrap();
    assert_eq!(ctx.chat_msgs().len(), 1, "the run delivers its message");
    assert_eq!(
        ctx.task_msgs(),
        vec![TaskMsg::CreateTask {
            task_id: "from_prose".into(),
            title: "prose".into(),
            owner: None,
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
    let oid = "1a".repeat(20);
    let granted_facets = serde_json::json!({
        "workspace_receipt": {
            "source_prefix": "forge:app",
            "source_snapshot": "2b".repeat(20),
            "output_snapshot": null,
            "commit_height": null,
            "rebased": false,
            "no_changes": false,
            "branch": "agent/x",
            "output_commit": oid,
        },
        "sink": sink["sink"].clone(),
    });
    exec(
        &mut m,
        &mut ctx,
        &result_event(&run_id, Ok(runner_wrapper("done", granted_facets))),
    )
    .unwrap();
    let forge_ops: Vec<_> = ctx.msgs.iter().filter(|m| m.target == "forge").collect();
    assert_eq!(forge_ops.len(), 1, "one OpenPr emitted");
    // title = first line of the message facet; body = the full message +
    // the receipt breadcrumb block (run id, output_ref, executing node).
    // the forge receipt proves this run pushed the source branch; no saga
    // record is seeded, so only the executing node degrades honestly.
    assert_eq!(
        forge::decode_msg(&forge_ops[0].payload).unwrap(),
        forge::ForgeMsg::OpenPr {
            repo: "app".into(),
            title: "done".into(),
            body: format!("done\n\n---\nrun: {run_id}\noutput: agent/x@{oid}\nnode: unknown"),
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
fn pr_sink_with_a_deleted_source_branch_degrades_without_aborting() {
    let mut granted = registry(&[("bot", &[ACTION_CHAT_POST])]);
    granted.get_mut("bot").unwrap().caps.forge_push = vec!["app".into()];
    let (mut m, run_id) = awaiting_run_with_forge(&granted);
    // The host observed a push, but the source branch was deleted before the
    // result settled, so committed Forge no longer exposes the ref.
    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&granted)
        .with_transcript("general", transcript(2));
    let oid = "1a".repeat(20);
    exec(
        &mut m,
        &mut ctx,
        &result_event(
            &run_id,
            Ok(forge_wrapper(
                "done",
                Some("agent/x"),
                Some(&oid),
                false,
                None,
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
    let oid = "1a".repeat(20);
    exec(
        &mut m,
        &mut ctx,
        &result_event(
            &run_id,
            Ok(forge_wrapper(
                "done",
                Some("agent/x"),
                Some(&oid),
                false,
                None,
            )),
        ),
    )
    .unwrap();
    assert!(
        ctx.msgs.iter().all(|m| m.target != "forge"),
        "an unborn target branch must never emit an OpenPr (no-fail rule)"
    );
    assert!(
        breadcrumbs(&ctx).contains(&format!(
            "run {run_id} pr sink skipped: target branch main not born"
        )),
        "the breadcrumb names the unborn target: {:?}",
        breadcrumbs(&ctx)
    );
    assert_eq!(
        ctx.chat_msgs().len(),
        1,
        "the run still delivers its message"
    );
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
    assert_eq!(
        ctx.chat_msgs().len(),
        1,
        "the run still delivers its message"
    );
}

#[test]
fn malformed_facet_fails_the_run_without_aborting() {
    // sink is not an object → decode_run_result fails → the run fails
    // deterministically (R4), never a delivery-block abort.
    let (mut m, registry, run_id) = awaiting_run(&[ACTION_CHAT_POST]);
    let bad = serde_json::json!({
        "ducktape_runner_result": 1,
        "response_text": "hi",
        "workspace_receipt": {"source_prefix":"p","source_snapshot":null,"output_snapshot":null,"commit_height":null,"rebased":false,"no_changes":false},
        "sink": "not-an-object"
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
fn job_finalize_is_a_delivery_receipt_with_output_ref() {
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
        "status": "ok"
    });
    // the prose carries the task write (the production path — the oracle never
    // lifts effects; a job run with no action would fail validation).
    let prose = String::from_utf8(response_json(
        &[],
        vec![AgentAction::CreateTask {
            task_id: "t1".into(),
            title: "todo".into(),
        }],
    ))
    .unwrap();
    let mut ctx = CaptureCtx::new()
        .at(10)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_claimed_job("job-1", 3);
    exec(
        &mut m,
        &mut ctx,
        &result_event(&run_id, Ok(runner_wrapper(&prose, facets))),
    )
    .unwrap();

    // the prose-parsed action applied a task write.
    assert_eq!(
        ctx.task_msgs(),
        vec![TaskMsg::CreateTask {
            task_id: "t1".into(),
            title: "todo".into(),
            owner: None,
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
fn raw_commit_message_does_not_inflate_the_job_finalize_receipt() {
    let registry = job_registry();
    let mut m = module();
    let mut ctx = CaptureCtx::new()
        .at(3)
        .with_jobs_origin()
        .with_registry(&registry);
    exec(&mut m, &mut ctx, &jobs_event("job-1", "agent/duck", "spec")).unwrap();
    commit(&mut m);
    let run_id = job_run_id_for("job-1", "duck", 3);
    let response = crate::encode_response(&AgentResponse {
        reply_blocks: Vec::new(),
        actions: vec![AgentAction::CreateTask {
            task_id: "t1".into(),
            title: "todo".into(),
        }],
        commit_message: Some("x".repeat(JOB_FINALIZE_PAYLOAD_BYTES * 2)),
    });
    let response = String::from_utf8(response).unwrap();
    let mut ctx = CaptureCtx::new()
        .at(10)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_claimed_job("job-1", 3);
    exec(
        &mut m,
        &mut ctx,
        &result_event(
            &run_id,
            Ok(runner_wrapper(&response, serde_json::json!({}))),
        ),
    )
    .unwrap();

    let finalize = ctx.job_msgs();
    assert_eq!(finalize.len(), 1);
    let JobsMsg::Finalize { payload, .. } = &finalize[0] else {
        panic!("expected a finalize");
    };
    assert!(payload.len() <= JOB_FINALIZE_PAYLOAD_BYTES);
    let value: serde_json::Value = serde_json::from_str(payload).unwrap();
    assert_eq!(value["status"], "ok");
    assert_eq!(value["response"]["actions"].as_array().unwrap().len(), 1);
    assert!(value["response"].get("commit_message").is_none());
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
    });
    // the prose carries the action (job runs with no action fail validation).
    let prose = String::from_utf8(response_json(
        &[],
        vec![AgentAction::CreateTask {
            task_id: "t1".into(),
            title: "todo".into(),
        }],
    ))
    .unwrap();
    let mut ctx = CaptureCtx::new()
        .at(10)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_claimed_job("job-1", 3);
    exec(
        &mut m,
        &mut ctx,
        &result_event(&run_id, Ok(runner_wrapper(&prose, facets))),
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
        decode_run_result(&no_sink).unwrap().sink,
        WireSink::Chain
    ));
    // a present {"mode":"pr",...} → Pr.
    let pr = runner_wrapper(
        "hi",
        serde_json::json!({"sink":{"mode":"pr","repo":"a","source_branch":"s","title":"t"}}),
    );
    assert!(matches!(
        decode_run_result(&pr).unwrap().sink,
        WireSink::Pr { .. }
    ));
    // an unsupported wrapper version fails to decode (R4).
    let badv = serde_json::json!({
        "ducktape_runner_result": 99,
        "response_text": "x",
        "workspace_receipt": {"source_prefix":"p","source_snapshot":null,"output_snapshot":null,"commit_height":null,"rebased":false,"no_changes":false}
    });
    assert!(decode_run_result(&serde_json::to_vec(&badv).unwrap()).is_err());
}

/// a registry whose one agent "bot" may chat and push to "app", plus an
/// awaiting run — the PR-sink happy-path scaffold.
fn forge_push_run() -> (RunsModule, Registry, String) {
    let mut granted = registry(&[("bot", &[ACTION_CHAT_POST])]);
    granted.get_mut("bot").unwrap().caps.forge_push = vec!["app".into()];
    let (m, run_id) = awaiting_run_with_forge(&granted);
    (m, granted, run_id)
}

/// Re-key the lightweight sink fixture as a real Forge issue run. Composition
/// itself is covered separately; this keeps the publication-boundary test
/// focused on the committed tracker lookup that verifies the title.
fn bind_run_to_forge_issue(m: &mut RunsModule, run_id: &str, number: u64) -> String {
    let old_dispatch = dispatch_id_for(run_id);
    let mut entry = m
        .pending
        .remove(&old_dispatch)
        .expect("pending fixture run");
    entry.channel_id = format!("forge:app:{number}");
    let bound_run_id = entry.run_id();
    m.pending.insert(dispatch_id_for(&bound_run_id), entry);
    bound_run_id
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
fn pr_sink_uses_verified_issue_title_and_keeps_response_prose_in_the_body() {
    // The full derivation: title = the committed bound Forge issue title;
    // body = the whole response facet + receipt breadcrumb (run id,
    // branch@oid, executing node). The Pages-style receipt must not become
    // publication metadata.
    let (mut m, granted, fixture_run_id) = forge_push_run();
    let run_id = bind_run_to_forge_issue(&mut m, &fixture_run_id, 7);
    let oid = "1a".repeat(20);
    let saga_id = sink::saga_id_for_dispatch("runs", &dispatch_id_for(&run_id));
    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&granted)
        .with_transcript("general", transcript(2))
        .with_forge_ref("app", "agent/x")
        .with_forge_ref("app", "main")
        .with_forge_item(
            "app",
            forge_issue(7, "Fix the flaky gate", "Issue reproduction"),
        )
        .with_saga_assignee(&saga_id, &[0xab; 32]);
    let message = "Implemented and recorded verification on the referenced Pages block.\n\nDetails in the diff.";
    exec(
        &mut m,
        &mut ctx,
        &result_event(
            &run_id,
            Ok(forge_wrapper(
                message,
                Some("agent/x"),
                Some(&oid),
                false,
                None,
            )),
        ),
    )
    .unwrap();
    let forge_ops: Vec<_> = ctx.msgs.iter().filter(|m| m.target == "forge").collect();
    assert_eq!(forge_ops.len(), 1, "one OpenPr emitted");
    assert_eq!(
        forge::decode_msg(&forge_ops[0].payload).unwrap(),
        forge::ForgeMsg::OpenPr {
            repo: "app".into(),
            title: "Fix the flaky gate".into(),
            body: format!(
                "{message}\n\n---\nrun: {run_id}\noutput: agent/x@{oid}\nnode: {}",
                "ab".repeat(32)
            ),
            source_branch: "agent/x".into(),
            target_branch: "main".into(),
        }
    );
    // the delivered-runs ring observes the same delivery: the forge
    // output ref and the number the fresh OpenPr gets (issue #7 → PR #8).
    commit(&mut m);
    let rec = &recent_runs(&m)[0];
    assert_eq!(rec.output_ref, Some(format!("agent/x@{oid}")));
    assert_eq!(rec.pr_number, Some(8));
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
            Ok(forge_wrapper(
                "done",
                Some("agent/x"),
                Some(&oid),
                false,
                None,
            )),
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
fn pr_sink_rejects_a_commit_pushed_to_another_repository() {
    let (mut m, granted, run_id) = forge_push_run();
    let oid = "1a".repeat(20);
    let receipt = serde_json::json!({
        "source_prefix": "forge:other",
        "source_snapshot": "2b".repeat(20),
        "output_snapshot": null,
        "commit_height": null,
        "rebased": false,
        "no_changes": false,
        "branch": "agent/x",
        "output_commit": oid,
    });
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
        &result_event(
            &run_id,
            Ok(runner_wrapper(
                "done",
                serde_json::json!({
                    "workspace_receipt": receipt,
                    "sink": {"mode":"pr","repo":"app","source_branch":"agent/x","target_branch":"main","title":"","body":""},
                    "status": "ok",
                }),
            )),
        ),
    )
    .unwrap();
    assert!(
        ctx.msgs.iter().all(|m| m.target != "forge"),
        "a commit pushed to another repository cannot authorize this sink"
    );
    assert!(
        breadcrumbs(&ctx).contains(&format!(
            "run {run_id} pr sink skipped: no publishable workspace commit for source branch"
        )),
        "the mismatched repository is recorded as an honest publication skip: {:?}",
        breadcrumbs(&ctx)
    );
}

#[test]
fn pr_sink_never_updates_an_existing_pr_when_nothing_was_pushed() {
    // A stale source ref and open PR are not publication evidence. output:none
    // remains chat/history-only and must not be reported as a PR update.
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
        breadcrumbs(&ctx).contains(&format!(
            "run {run_id} pr sink skipped: no publishable workspace commit for source branch"
        )),
        "the no-publish gate runs before the duplicate-PR probe: {:?}",
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
            Ok(forge_wrapper(
                "done",
                Some("agent/x"),
                None,
                false,
                Some("push CAS reject"),
            )),
        ),
    )
    .unwrap();
    assert!(ctx2.msgs.iter().all(|m| m.target != "forge"));
    assert!(
        breadcrumbs(&ctx2).contains(&format!(
            "run {run_id2} pr sink skipped: no publishable workspace commit for source branch"
        )),
        "commit failure is not a PR update: {:?}",
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
            Ok(forge_wrapper(
                "done",
                Some("agent/x"),
                Some(&oid),
                false,
                None,
            )),
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
fn output_none_with_a_born_stale_branch_never_opens_a_pr_from_response_prose() {
    // Regression for #102: a review-only output:none result can carry general
    // response prose and point at an old born branch, but neither is a commit.
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
        &result_event(
            &run_id,
            Ok(forge_wrapper("re-proposing", None, None, true, None)),
        ),
    )
    .unwrap();
    let forge_ops: Vec<_> = ctx.msgs.iter().filter(|m| m.target == "forge").collect();
    assert!(
        forge_ops.is_empty(),
        "response prose must not become a PR without a commit"
    );
    assert!(
        breadcrumbs(&ctx).contains(&format!(
            "run {run_id} pr sink skipped: no publishable workspace commit for source branch"
        )),
        "the audit breadcrumb records the no-change skip: {:?}",
        breadcrumbs(&ctx)
    );
    commit(&mut m);
    let rec = &recent_runs(&m)[0];
    assert_eq!(rec.output_ref, None);
    assert_eq!(rec.pr_number, None);
}

#[test]
fn a_late_commit_for_a_merged_anchor_stays_chat_and_history_only() {
    let (mut m, granted, fixture_run_id) = forge_push_run();
    let run_id = bind_run_to_forge_issue(&mut m, &fixture_run_id, 7);
    let mut merged = forge_pr(7, "Already merged", "", "agent/x", "main");
    merged.summary.state = forge::ItemState::Merged;
    merged.merge_oid = Some("2b".repeat(20));
    let oid = "1a".repeat(20);
    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&granted)
        .with_transcript("forge:app:7", transcript(2))
        .with_forge_ref("app", "agent/x")
        .with_forge_ref("app", "main")
        .with_forge_item("app", merged);
    exec(
        &mut m,
        &mut ctx,
        &result_event(
            &run_id,
            Ok(forge_wrapper(
                "Post-merge review found no changes needed.",
                Some("agent/x"),
                Some(&oid),
                false,
                None,
            )),
        ),
    )
    .unwrap();
    assert!(ctx.msgs.iter().all(|m| m.target != "forge"));
    assert!(breadcrumbs(&ctx).contains(&format!(
        "run {run_id} pr sink skipped: bound Forge item is merged"
    )));
    commit(&mut m);
    assert_eq!(recent_runs(&m)[0].pr_number, None);
}

#[test]
fn a_success_result_arriving_after_cancellation_cannot_publish() {
    let (mut m, granted, run_id) = forge_push_run();
    let mut cancelled = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&granted)
        .with_transcript("general", transcript(2));
    exec(
        &mut m,
        &mut cancelled,
        &result_event(&run_id, Err("cancelled".into())),
    )
    .unwrap();
    commit(&mut m);

    let oid = "1a".repeat(20);
    let mut late = CaptureCtx::new()
        .at(9)
        .with_dispatch_origin()
        .with_registry(&granted)
        .with_forge_ref("app", "agent/x")
        .with_forge_ref("app", "main");
    exec(
        &mut m,
        &mut late,
        &result_event(
            &run_id,
            Ok(forge_wrapper(
                "late",
                Some("agent/x"),
                Some(&oid),
                false,
                None,
            )),
        ),
    )
    .unwrap();
    assert!(late.msgs.iter().all(|m| m.target != "forge"));
    assert!(
        breadcrumbs(&late)
            .iter()
            .any(|note| note.contains("dropped result for unknown dispatch"))
    );
    assert_eq!(
        recent_runs(&m).len(),
        1,
        "the late replay adds no history entry"
    );
}

#[test]
fn saga_id_mirror_matches_the_dispatch_modules_derivation() {
    // pin the executing-node lookup's saga-id mirror against the REAL
    // dispatch module: register a recipe, dispatch, and read the saga id
    // off the emitted trigger — the mirror must derive the same id.
    let mut d =
        dispatch::DispatchModule::new("dispatch", "saga", "identity", Box::new(sdk_testkit::MemStore::new()));
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
                admission: dispatch::AdmissionPolicy::Queue,
            }),
        },
    ))
    .unwrap();
    let trigger = ctx
        .msgs
        .iter()
        .find(|m| m.target == "saga")
        .expect("the dispatch emits a saga trigger");
    let saga::SagaMsg::Trigger { saga_id, .. } = saga::decode_msg(&trigger.payload).unwrap() else {
        panic!("expected a saga trigger");
    };
    assert_eq!(saga_id, sink::saga_id_for_dispatch("runs", "d1"));
}

#[test]
fn workspace_receipt_mirror_decodes_the_forge_fields() {
    // present: the §5 additive fields land on the mirror.
    let wrapper = forge_wrapper(
        "done",
        Some("agent/item-7"),
        Some(&"1a".repeat(20)),
        false,
        None,
    );
    let receipt = decode_run_result(&wrapper).unwrap().workspace_receipt;
    assert_eq!(receipt.branch.as_deref(), Some("agent/item-7"));
    assert_eq!(receipt.output_commit.as_deref(), Some(&*"1a".repeat(20)));

    // absent (every pre-forge receipt): serde defaults, not an error.
    let receipt = decode_run_result(&runner_wrapper("done", serde_json::json!({})))
        .unwrap()
        .workspace_receipt;
    assert_eq!(receipt.branch, None);
    assert_eq!(receipt.output_commit, None);
}
