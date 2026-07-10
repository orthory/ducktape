use super::*;

// ---- the composer's v2-vs-v3 selection (files presence) ---------------------

#[test]
fn a_run_composes_v2_without_files_and_v3_with_files_wired() {
    let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
    let agent = record("bot", &[ACTION_CHAT_POST]);
    let head = "aa".repeat(32);

    // no files module: the byte-identical v2 payload, no portable fields.
    let m0 = module();
    let ctx0 = CaptureCtx::new()
        .with_registry(&registry)
        .with_transcript("general", transcript(2));
    let prepared = block_on(m0.prepare_dispatch(&ctx0, &agent, "general", 2)).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&prepared.payload).unwrap();
    assert_eq!(v["ducktape_run"], 2, "no files module composes v2");
    assert!(
        v.get("workspace").is_none(),
        "no v3 workspace without files"
    );
    assert!(v.get("skills").is_none());

    // files wired: the v3 payload pins the committed head.
    let m4 = module().with_files_module("files");
    let ctx4 = CaptureCtx::new()
        .with_registry(&registry)
        .with_transcript("general", transcript(2))
        .with_files_head(&head);
    let prepared = block_on(m4.prepare_dispatch(&ctx4, &agent, "general", 2)).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&prepared.payload).unwrap();
    assert_eq!(v["ducktape_run"], 3, "a wired files module composes v3");
    assert_eq!(
        v["workspace"]["source_prefix"],
        "/shared/agent-workspaces/bot"
    );
    assert_eq!(
        v["workspace"]["source_snapshot"], head,
        "source_snapshot pins the committed duckfs head (W2)"
    );
    assert!(
        v["workspace"].get("mount_path").is_none(),
        "the composed v3 workspace carries NO mount_path (D7)"
    );
}

#[test]
fn portable_inputs_gate_pin_and_skill_resolution() {
    let head = "aa".repeat(32);
    let mut agent = record("bot", &[ACTION_CHAT_POST]);
    agent.skills = vec![
        agent::SkillRef {
            name: "pinned".into(),
            source_prefix: "/shared/skills/pinned".into(),
            source_snapshot: Some("bb".repeat(32)),
        },
        agent::SkillRef {
            name: "tracking".into(),
            source_prefix: "/shared/skills/tracking".into(),
            source_snapshot: None,
        },
    ];

    // no files module: None (the composer takes its v2 path).
    let unwired = module();
    let ctx0 = CaptureCtx::new().with_files_head(&head);
    assert!(
        block_on(unwired.portable_inputs(&ctx0, &agent))
            .unwrap()
            .is_none(),
        "no portable inputs without a wired files module"
    );

    let m = module().with_files_module("files");

    // the duckfs snapshot pin, from the tagged workspace source.
    fn duckfs_pin(inputs: &envelope::PortableInputs) -> Option<String> {
        match &inputs.workspace {
            envelope::WorkspaceSource::Duckfs {
                source_snapshot, ..
            } => source_snapshot.clone(),
            other => panic!("expected a duckfs workspace source, got {other:?}"),
        }
    }

    // files wired + a committed head: Some, head pinned, skills resolved.
    let ctx4 = CaptureCtx::new().with_files_head(&head);
    let inputs = block_on(m.portable_inputs(&ctx4, &agent)).unwrap().unwrap();
    assert_eq!(duckfs_pin(&inputs).as_deref(), Some(head.as_str()));
    assert!(inputs.sink.is_chain(), "the duckfs lane requests no sink");
    assert!(inputs.context.is_none(), "the duckfs lane injects no context");
    // pinned skill passes its snapshot through; tracking resolves to the head.
    assert_eq!(
        inputs.skills[0].source_snapshot.as_deref(),
        Some("bb".repeat(32).as_str())
    );
    assert_eq!(
        inputs.skills[1].source_snapshot.as_deref(),
        Some(head.as_str()),
        "a tracking skill pins the same committed head (W2)"
    );

    // files wired + an unresolved head: Some with a null pin (fresh network).
    let ctx_empty = CaptureCtx::new();
    let inputs = block_on(m.portable_inputs(&ctx_empty, &agent))
        .unwrap()
        .unwrap();
    assert!(
        duckfs_pin(&inputs).is_none(),
        "an unresolved head is a legitimate null pin, still Some"
    );
}

// ---- runner-result decode (facet-free + faceted) ----------------------------

#[test]
fn legacy_raw_text_results_decode_as_message_only() {
    // a raw-text result (or the AgentResponse JSON the model emits) carries
    // no runner marker, so it decodes to a facet-free message-only result:
    // response_text = the lossy-decoded bytes, no effects, Chain sink, Ok.
    for raw in [
        "just a prose answer",
        "",
        r#"{"reply_blocks":[{"id":"x","kind":"paragraph","text":"hi"}],"actions":[]}"#,
        // a JSON object WITHOUT the marker is not a runner wrapper.
        r#"{"response_text":"nope"}"#,
    ] {
        let result = decode_run_result_v1(raw.as_bytes()).unwrap();
        assert_eq!(result.response_text, raw);
        assert!(result.effects.is_empty());
        assert!(matches!(result.sink, WireSink::Chain));
        assert_eq!(result.status, WireStatus::Ok);
    }
    // invalid utf-8 still degrades lossily rather than erroring.
    assert_eq!(
        decode_run_result_v1(&[0xff, 0xfe]).unwrap().response_text,
        "\u{fffd}\u{fffd}"
    );
}

#[test]
fn a_well_formed_runner_result_yields_its_response_text() {
    let wrapper = serde_json::json!({
        "ducktape_runner_result": 1,
        "response_text": "the deliverable prose",
        "workspace_receipt": {
            "source_prefix": "/shared/agent-workspaces/bot",
            "source_snapshot": null,
            "output_snapshot": null,
            "commit_height": null,
            "rebased": false,
            "no_changes": true
        }
    })
    .to_string();
    assert_eq!(
        decode_run_result_v1(wrapper.as_bytes())
            .unwrap()
            .response_text,
        "the deliverable prose"
    );
}

#[test]
fn a_broken_runner_wrapper_is_a_loud_error_not_raw_delivery() {
    // claims the marker but the version is unknown → fail the run.
    let bad_version = serde_json::json!({
        "ducktape_runner_result": 99,
        "response_text": "x",
        "workspace_receipt": {
            "source_prefix": "p", "source_snapshot": null, "output_snapshot": null,
            "commit_height": null, "rebased": false, "no_changes": false
        }
    })
    .to_string();
    let err = decode_run_result_v1(bad_version.as_bytes()).unwrap_err();
    assert!(err.contains("version 99"), "got {err:?}");

    // claims the marker but the shape is malformed → fail, never deliver
    // the raw JSON as if it were the model's prose.
    let malformed = r#"{"ducktape_runner_result":1,"response_text":42}"#;
    let err = decode_run_result_v1(malformed.as_bytes()).unwrap_err();
    assert!(err.contains("malformed"), "got {err:?}");
}
// ---- the forge compose lane (M1) --------------------------------------------

fn compose_forge(
    m: &RunsModule,
    ctx: &CaptureCtx,
    registry: &Registry,
    channel: &str,
) -> Result<serde_json::Value, String> {
    let agent = registry.get("bot").expect("bot registered");
    let prepared = block_on(m.prepare_dispatch(ctx, agent, channel, 2))?;
    Ok(serde_json::from_slice(&prepared.payload).expect("payload is JSON"))
}

#[test]
fn an_issue_run_forks_main_with_an_unborn_item_branch_and_requests_a_pr() {
    let registry = forge_read_registry();
    let main_tip = "cd".repeat(20);
    let m = forge_module();
    let ctx = CaptureCtx::new()
        .with_registry(&registry)
        .with_transcript("forge:app:7", transcript(2))
        .with_forge_item("app", forge_issue(7, "Fix the gate", "repro inside"))
        .with_forge_tip("app", "main", &main_tip)
        .with_files_head(&"aa".repeat(32));
    let v = compose_forge(&m, &ctx, &registry, "forge:app:7").unwrap();

    assert_eq!(v["ducktape_run"], 3);
    assert_eq!(v["workspace"]["kind"], "forge");
    assert_eq!(v["workspace"]["repo"], "app");
    assert_eq!(
        v["workspace"]["commit"], main_tip,
        "an issue with an unborn work branch forks the committed main tip"
    );
    assert_eq!(v["workspace"]["branch"], "agent/item-7");
    assert_eq!(v["workspace"]["branch_born"], false);
    // the requested sink: a PR of the work branch onto main, no title/body.
    assert_eq!(v["result_contract"]["sink"]["mode"], "pr");
    assert_eq!(v["result_contract"]["sink"]["repo"], "app");
    assert_eq!(v["result_contract"]["sink"]["source_branch"], "agent/item-7");
    assert_eq!(v["result_contract"]["sink"]["target_branch"], "main");
    assert!(v["result_contract"]["sink"].get("title").is_none());
    assert!(v["result_contract"]["sink"].get("body").is_none());
    // the deterministic item context rides the envelope.
    let context = v["context"].as_str().expect("a forge run carries context");
    assert!(context.contains("repo: app"), "{context}");
    assert!(context.contains("issue #7"), "{context}");
    assert!(context.contains("title: Fix the gate"), "{context}");
    assert!(context.contains("repro inside"), "{context}");
    assert!(context.contains("work branch: agent/item-7"), "{context}");
    // thread continuity: unchanged — replies land in the item discussion.
    assert_eq!(v["thread_key"], "forge:app:7#2");
    // skills machinery is the duckfs lane's: the duckfs head still pins.
    assert_eq!(v["skills"].as_array().unwrap().len(), 0);
}

#[test]
fn a_forge_session_continues_from_the_born_work_branch_tip() {
    let registry = forge_read_registry();
    let main_tip = "cd".repeat(20);
    let item_tip = "ef".repeat(20);
    let m = forge_module();
    let ctx = CaptureCtx::new()
        .with_registry(&registry)
        .with_transcript("forge:app:7", transcript(2))
        .with_forge_item("app", forge_issue(7, "Fix the gate", "body"))
        .with_forge_tip("app", "main", &main_tip)
        .with_forge_tip("app", "agent/item-7", &item_tip);
    let v = compose_forge(&m, &ctx, &registry, "forge:app:7").unwrap();
    assert_eq!(
        v["workspace"]["commit"], item_tip,
        "a born work branch is the session: later runs fork ITS tip, not main"
    );
    assert_eq!(v["workspace"]["branch"], "agent/item-7");
    assert_eq!(v["workspace"]["branch_born"], true);
}

#[test]
fn a_pr_item_run_works_the_prs_own_source_branch() {
    let registry = forge_read_registry();
    let src_tip = "12".repeat(20);
    let m = forge_module();
    let ctx = CaptureCtx::new()
        .with_registry(&registry)
        .with_transcript("forge:app:8", transcript(2))
        .with_forge_item("app", forge_pr(8, "Wire it", "please", "feature/x", "dev"))
        .with_forge_tip("app", "main", &"cd".repeat(20))
        .with_forge_tip("app", "feature/x", &src_tip);
    let v = compose_forge(&m, &ctx, &registry, "forge:app:8").unwrap();
    // THE pr-item rule: the session pushes the PR's own branch, so the
    // open PR updates in place.
    assert_eq!(v["workspace"]["branch"], "feature/x");
    assert_eq!(v["workspace"]["commit"], src_tip);
    assert_eq!(v["workspace"]["branch_born"], true);
    assert_eq!(v["result_contract"]["sink"]["source_branch"], "feature/x");
    assert_eq!(
        v["result_contract"]["sink"]["target_branch"], "dev",
        "a PR item's requested sink targets the PR's own target branch"
    );
    let context = v["context"].as_str().unwrap();
    assert!(context.contains("pr #8"), "{context}");
    assert!(context.contains("pr source branch: feature/x"), "{context}");
    assert!(context.contains("pr target branch: dev"), "{context}");
}

#[test]
fn a_forge_run_without_the_forge_read_cap_fails_compose_deterministically() {
    // no forge_read grant → compose Err naming the cap gate, BEFORE any
    // tracker lookup (the fixtures deliberately hold no item).
    let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
    let m = forge_module();
    let ctx = CaptureCtx::new()
        .with_registry(&registry)
        .with_transcript("forge:app:7", transcript(2));
    let reason = compose_forge(&m, &ctx, &registry, "forge:app:7").unwrap_err();
    assert!(
        reason.contains("forge_read"),
        "the reason names the missing cap: {reason}"
    );
}

#[test]
fn forge_compose_failures_have_deterministic_reasons() {
    let registry = forge_read_registry();
    let m = forge_module();

    // item missing.
    let ctx = CaptureCtx::new()
        .with_registry(&registry)
        .with_transcript("forge:app:7", transcript(2))
        .with_forge_tip("app", "main", &"cd".repeat(20));
    let reason = compose_forge(&m, &ctx, &registry, "forge:app:7").unwrap_err();
    assert!(reason.contains("no forge item"), "{reason}");

    // issue with no main branch to fork.
    let ctx = CaptureCtx::new()
        .with_registry(&registry)
        .with_transcript("forge:app:7", transcript(2))
        .with_forge_item("app", forge_issue(7, "t", "b"));
    let reason = compose_forge(&m, &ctx, &registry, "forge:app:7").unwrap_err();
    assert!(reason.contains("main"), "{reason}");

    // PR whose source branch is not born.
    let ctx = CaptureCtx::new()
        .with_registry(&registry)
        .with_transcript("forge:app:8", transcript(2))
        .with_forge_item("app", forge_pr(8, "t", "b", "feature/x", "main"))
        .with_forge_tip("app", "main", &"cd".repeat(20));
    let reason = compose_forge(&m, &ctx, &registry, "forge:app:8").unwrap_err();
    assert!(reason.contains("feature/x"), "{reason}");

    // forge channel, but no forge module wired.
    let unwired = module().with_files_module("files");
    let ctx = CaptureCtx::new()
        .with_registry(&registry)
        .with_transcript("forge:app:7", transcript(2))
        .with_forge_item("app", forge_issue(7, "t", "b"))
        .with_forge_tip("app", "main", &"cd".repeat(20));
    let reason = compose_forge(&unwired, &ctx, &registry, "forge:app:7").unwrap_err();
    assert!(reason.contains("forge module"), "{reason}");
}

#[test]
fn a_malformed_forge_channel_composes_the_duckfs_lane_as_today() {
    // "forge:app" (no number) is NOT a forge channel — the duckfs lane
    // composes exactly as for any other channel id.
    let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
    let m = forge_module();
    let ctx = CaptureCtx::new()
        .with_registry(&registry)
        .with_transcript("forge:app", transcript(2))
        .with_files_head(&"aa".repeat(32));
    let v = compose_forge(&m, &ctx, &registry, "forge:app").unwrap();
    assert_eq!(v["workspace"]["kind"], "duckfs");
    assert!(v.get("context").is_none());
    assert!(v["result_contract"].get("sink").is_none());
}

#[test]
fn an_engagement_on_a_forge_channel_without_the_cap_skips_with_a_breadcrumb() {
    // the engagement arm is NO-FAIL: a compose failure is a skip + note,
    // never a dispatch and never a block abort.
    let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
    let mut m = forge_module();
    let mut watch_ctx = CaptureCtx::new().with_origin(user(9)).with_registry(&registry);
    exec(
        &mut m,
        &mut watch_ctx,
        &admin(&RunsMsg::WatchChannel {
            channel_id: "forge:app:7".into(),
            policy: TurnPolicy::All,
        }),
    )
    .unwrap();
    commit(&mut m);

    let mut ctx = CaptureCtx::new()
        .at(2)
        .with_tagging_origin()
        .with_registry(&registry)
        .with_transcript("forge:app:7", transcript(2))
        .with_forge_item("app", forge_issue(7, "t", "b"))
        .with_forge_tip("app", "main", &"cd".repeat(20));
    exec(&mut m, &mut ctx, &engagement("forge:app:7", 2, vec![])).unwrap();

    assert!(
        ctx.dispatch_msgs().is_empty(),
        "a compose failure stages no dispatch"
    );
    assert!(
        ctx.events.iter().any(|e| {
            let s = String::from_utf8_lossy(&e.payload);
            s.contains("run skipped") && s.contains("forge_read")
        }),
        "the skip breadcrumb names the compose reason"
    );
}

#[test]
fn a_request_run_on_a_forge_channel_without_the_cap_rejects_with_the_reason() {
    let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
    let mut m = forge_module();
    let mut ctx = CaptureCtx::new()
        .with_origin(user(9))
        .with_registry(&registry)
        .with_transcript("forge:app:7", transcript(2));
    let err = exec(
        &mut m,
        &mut ctx,
        &admin(&RunsMsg::RequestRun {
            agent_id: "bot".into(),
            channel_id: "forge:app:7".into(),
            anchor_seq: 2,
        }),
    )
    .unwrap_err();
    let Error::Module(reason) = err else {
        panic!("expected a module rejection");
    };
    assert!(reason.contains("forge_read"), "{reason}");
}

#[test]
fn a_forge_engagement_with_the_cap_stages_the_dispatch() {
    // the happy path END TO END through the engagement arm: watch the item
    // channel, mention-free All engagement, forge workspace composed.
    let registry = forge_read_registry();
    let mut m = forge_module();
    let mut watch_ctx = CaptureCtx::new().with_origin(user(9)).with_registry(&registry);
    exec(
        &mut m,
        &mut watch_ctx,
        &admin(&RunsMsg::WatchChannel {
            channel_id: "forge:app:7".into(),
            policy: TurnPolicy::All,
        }),
    )
    .unwrap();
    commit(&mut m);

    let mut ctx = CaptureCtx::new()
        .at(2)
        .with_tagging_origin()
        .with_registry(&registry)
        .with_transcript("forge:app:7", transcript(2))
        .with_forge_item("app", forge_issue(7, "Fix the gate", "body"))
        .with_forge_tip("app", "main", &"cd".repeat(20));
    exec(&mut m, &mut ctx, &engagement("forge:app:7", 2, vec![])).unwrap();

    let dispatches = ctx.dispatch_msgs();
    assert_eq!(dispatches.len(), 1, "one run staged");
    let DispatchMsg::Dispatch { payload, .. } = &dispatches[0] else {
        panic!("expected a dispatch");
    };
    let v: serde_json::Value = serde_json::from_slice(payload).unwrap();
    assert_eq!(v["workspace"]["kind"], "forge");
    assert_eq!(v["workspace"]["branch"], "agent/item-7");
    assert!(
        get_pending(&m, &run_id_for("forge:app:7", 2, "bot")).is_some(),
        "the pending entry is staged"
    );
}