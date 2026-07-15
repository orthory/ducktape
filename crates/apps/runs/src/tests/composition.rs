use super::*;

// ---- the composer always emits the portable v3 wire --------------------------

#[test]
fn a_run_composes_v3_with_or_without_files_wired() {
    let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
    let agent = record("bot", &[ACTION_CHAT_POST]);
    let head = "aa".repeat(32);

    // no files module (dev tools/tests): still the v3 wire, with a null pin.
    let m0 = module();
    let ctx0 = CaptureCtx::new()
        .with_registry(&registry)
        .with_transcript("general", transcript(2));
    let prepared = block_on(m0.prepare_dispatch(
        &ctx0,
        &agent,
        &crate::run_id_for("general", 2, "bot"),
        "general",
        2,
        &[],
    ))
    .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&prepared.payload).unwrap();
    assert_eq!(v["ducktape_run"], 3, "every composer emits v3 (flag day)");
    assert!(
        v["workspace"]["source_snapshot"].is_null(),
        "an unwired files module composes an explicit null pin"
    );

    // files wired: the v3 payload pins the committed head.
    let m4 = module().with_files_module("files");
    let ctx4 = CaptureCtx::new()
        .with_registry(&registry)
        .with_transcript("general", transcript(2))
        .with_files_head(&head);
    let prepared = block_on(m4.prepare_dispatch(
        &ctx4,
        &agent,
        &crate::run_id_for("general", 2, "bot"),
        "general",
        2,
        &[],
    ))
    .unwrap();
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
            load: agent::LoadMode::Always,
        },
        agent::SkillRef {
            name: "tracking".into(),
            source_prefix: "/shared/skills/tracking".into(),
            source_snapshot: None,
            load: agent::LoadMode::OnDemand,
        },
    ];

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

    // files wired + a committed head: head pinned, skills resolved.
    let ctx4 = CaptureCtx::new().with_files_head(&head);
    let inputs = block_on(m.portable_inputs(&ctx4, &agent, &[])).unwrap();
    assert_eq!(duckfs_pin(&inputs).as_deref(), Some(head.as_str()));
    assert!(inputs.sink.is_chain(), "the duckfs lane requests no sink");
    assert!(
        inputs.context.is_none(),
        "the duckfs lane injects no context"
    );
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
    // the curated load mode rides through pin resolution untouched — it is what
    // tells the host which bodies to inline into the agent's context document.
    assert!(inputs.skills[0].always, "the persona loads always");
    assert!(!inputs.skills[1].always, "the reference skill is on demand");

    // files wired + an unresolved head: a null pin (fresh network).
    let ctx_empty = CaptureCtx::new();
    let inputs = block_on(m.portable_inputs(&ctx_empty, &agent, &[])).unwrap();
    assert!(
        duckfs_pin(&inputs).is_none(),
        "an unresolved head is a legitimate null pin"
    );

    // no files module (dev tools/tests): no query is issued, the pin is null,
    // skills still resolve (a tracking skill then has no head to pin to).
    let unwired = module();
    let ctx0 = CaptureCtx::new().with_files_head(&head);
    let inputs = block_on(unwired.portable_inputs(&ctx0, &agent, &[])).unwrap();
    assert!(
        duckfs_pin(&inputs).is_none(),
        "an unwired files module composes a null pin, never a files query"
    );
    assert_eq!(
        inputs.skills[1].source_snapshot, None,
        "a tracking skill stays unpinned without a files head"
    );
}

// ---- runner-result decode (facet-free + faceted) ----------------------------

#[test]
fn marker_less_results_are_loud_errors_not_message_only_delivery() {
    // FLAG DAY: the flat-string passthrough is gone. bytes without the
    // ducktape_runner_result marker — raw prose, bare AgentResponse JSON,
    // invalid utf-8 — fail the decode deterministically (the run fails).
    for raw in [
        "just a prose answer".as_bytes(),
        "".as_bytes(),
        br#"{"reply_blocks":[{"id":"x","kind":"paragraph","text":"hi"}],"actions":[]}"#.as_slice(),
        // a JSON object WITHOUT the marker is not a runner wrapper.
        br#"{"response_text":"nope"}"#.as_slice(),
        &[0xff, 0xfe],
    ] {
        let err = decode_run_result_v1(raw).unwrap_err();
        assert!(
            err.contains("malformed"),
            "{raw:?} must be a loud error, got {err:?}"
        );
    }
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
    let prepared = block_on(m.prepare_dispatch(
        ctx,
        agent,
        &crate::run_id_for(channel, 2, "bot"),
        channel,
        2,
        &[],
    ))?;
    Ok(serde_json::from_slice(&prepared.payload).expect("payload is JSON"))
}

#[test]
fn an_issue_run_forks_dev_with_an_unborn_item_branch_and_requests_a_pr() {
    let registry = forge_read_registry();
    let dev_tip = "cd".repeat(20);
    let m = forge_module();
    let ctx = CaptureCtx::new()
        .with_registry(&registry)
        .with_transcript("forge:app:7", transcript(2))
        .with_forge_item("app", forge_issue(7, "Fix the gate", "repro inside"))
        .with_forge_tip("app", "dev", &dev_tip)
        .with_files_head(&"aa".repeat(32));
    let v = compose_forge(&m, &ctx, &registry, "forge:app:7").unwrap();

    assert_eq!(v["ducktape_run"], 3);
    assert_eq!(v["workspace"]["kind"], "forge");
    assert_eq!(v["workspace"]["repo"], "app");
    assert_eq!(v["workspace"]["item_title"], "Fix the gate");
    assert_eq!(
        v["workspace"]["commit"], dev_tip,
        "an issue with an unborn work branch forks the committed dev tip"
    );
    assert_eq!(v["workspace"]["branch"], "agent/item-7");
    assert_eq!(v["workspace"]["branch_born"], false);
    // the requested sink: a PR of the work branch onto dev, no title/body.
    assert_eq!(v["result_contract"]["sink"]["mode"], "pr");
    assert_eq!(v["result_contract"]["sink"]["repo"], "app");
    assert_eq!(
        v["result_contract"]["sink"]["source_branch"],
        "agent/item-7"
    );
    assert_eq!(v["result_contract"]["sink"]["target_branch"], "dev");
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
    let dev_tip = "cd".repeat(20);
    let item_tip = "ef".repeat(20);
    let m = forge_module();
    let ctx = CaptureCtx::new()
        .with_registry(&registry)
        .with_transcript("forge:app:7", transcript(2))
        .with_forge_item("app", forge_issue(7, "Fix the gate", "body"))
        .with_forge_tip("app", "dev", &dev_tip)
        .with_forge_tip("app", "agent/item-7", &item_tip);
    let v = compose_forge(&m, &ctx, &registry, "forge:app:7").unwrap();
    assert_eq!(
        v["workspace"]["commit"], item_tip,
        "a born work branch is the session: later runs fork ITS tip, not dev"
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
        .with_forge_tip("app", "dev", &"cd".repeat(20))
        .with_forge_tip("app", "feature/x", &src_tip);
    let v = compose_forge(&m, &ctx, &registry, "forge:app:8").unwrap();
    // THE pr-item rule: the session pushes the PR's own branch, so the
    // open PR updates in place.
    assert_eq!(v["workspace"]["item_title"], "Wire it");
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

// ---- `[[page:<id>]]` page-spec injection (M2) ---------------------------------

#[test]
fn a_page_ref_in_the_trigger_message_injects_the_page_section() {
    // a PLAIN channel (the duckfs lane): the trigger message's ref alone
    // composes a context section carrying the page subtree.
    let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
    let agent = record("bot", &[ACTION_CHAT_POST]);
    let m = module()
        .with_files_module("files")
        .with_pages_module("pages");
    let ctx = CaptureCtx::new()
        .with_registry(&registry)
        .with_transcript(
            "general",
            vec![
                message(1, "msg 1"),
                message(2, "please work from [Plan](duck://page/plan)"),
            ],
        )
        .with_page("plan", page_blocks("plan", "Project Plan"));
    let prepared = block_on(m.prepare_dispatch(
        &ctx,
        &agent,
        &crate::run_id_for("general", 2, "bot"),
        "general",
        2,
        &[],
    ))
    .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&prepared.payload).unwrap();
    let context = v["context"].as_str().expect("a page ref composes context");
    assert!(context.starts_with("Referenced pages:"), "{context}");
    assert!(
        context.contains("[Project Plan](duck://page/plan)"),
        "{context}"
    );
    assert!(context.contains("spec paragraph"), "{context}");
    assert!(
        context.contains("- [ ] do the thing [blk:b-t]"),
        "{context}"
    );
}

#[test]
fn a_file_ref_in_the_trigger_message_injects_the_attachment_text() {
    // a duck://files ref pulls the referenced attachment's committed TEXT into
    // the same context section — the agent-integration payoff of the unified
    // grammar. an IMAGE (non-utf8) in the same message is named, not inlined.
    let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
    let agent = record("bot", &[ACTION_CHAT_POST]);
    let m = module().with_files_module("files");
    let ctx = CaptureCtx::new()
        .with_registry(&registry)
        .with_transcript(
            "general",
            vec![message(
                1,
                "notes [notes.md](duck://files/shared/attachments/u1/notes.md) \
                 and ![shot](duck://files/shared/attachments/u2/shot.png)",
            )],
        )
        .with_file(
            "/shared/attachments/u1/notes.md",
            b"# Handoff\nrun the flaky gate twice",
        )
        // non-utf8 bytes = an image; named, never inlined.
        .with_file("/shared/attachments/u2/shot.png", &[0x89, 0x50, 0x4e, 0xff, 0xfe]);
    let prepared = block_on(m.prepare_dispatch(
        &ctx,
        &agent,
        &crate::run_id_for("general", 1, "bot"),
        "general",
        1,
        &[],
    ))
    .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&prepared.payload).unwrap();
    let context = v["context"].as_str().expect("a file ref composes context");
    assert!(context.starts_with("Referenced attachments:"), "{context}");
    assert!(
        context.contains("[attachment: notes.md]\n# Handoff\nrun the flaky gate twice"),
        "{context}"
    );
    assert!(
        context.contains("[attachment: shot.png — binary content, not shown]"),
        "{context}"
    );
}

#[test]
fn an_unresolvable_file_ref_composes_its_marker_never_a_failure() {
    let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
    let agent = record("bot", &[ACTION_CHAT_POST]);
    let m = module().with_files_module("files");
    let ctx = CaptureCtx::new().with_registry(&registry).with_transcript(
        "general",
        vec![message(
            1,
            "see [gone.txt](duck://files/shared/attachments/u/gone.txt)",
        )],
    );
    let prepared = block_on(m.prepare_dispatch(
        &ctx,
        &agent,
        &crate::run_id_for("general", 1, "bot"),
        "general",
        1,
        &[],
    ))
    .expect("an unresolvable attachment never fails compose");
    let v: serde_json::Value = serde_json::from_slice(&prepared.payload).unwrap();
    let context = v["context"].as_str().expect("a file ref composes context");
    assert!(
        context.contains("[attachment: gone.txt — not found]"),
        "{context}"
    );
}

#[test]
fn a_page_ref_in_the_forge_item_body_appends_after_the_item_context() {
    let registry = forge_read_registry();
    let m = forge_module();
    let ctx = CaptureCtx::new()
        .with_registry(&registry)
        .with_transcript("forge:app:7", transcript(2))
        .with_forge_item(
            "app",
            forge_issue(7, "Fix the gate", "spec at [Plan](duck://page/plan)"),
        )
        .with_forge_tip("app", "dev", &"cd".repeat(20))
        .with_page("plan", page_blocks("plan", "Project Plan"));
    let v = compose_forge(&m, &ctx, &registry, "forge:app:7").unwrap();
    let context = v["context"].as_str().unwrap();
    // the M1 item context is untouched and leads; the page section follows.
    assert!(context.starts_with("Forge item context"), "{context}");
    assert!(context.contains("spec at [Plan](duck://page/plan)"), "{context}");
    let item_body = context.find("spec at").unwrap();
    let pages_at = context
        .find("Referenced pages:")
        .expect("page section rides");
    assert!(
        item_body < pages_at,
        "the page section follows the item context: {context}"
    );
    assert!(
        context.contains("[Project Plan](duck://page/plan)"),
        "{context}"
    );
    assert!(
        context.contains("- [ ] do the thing [blk:b-t]"),
        "{context}"
    );
}

#[test]
fn a_missing_page_ref_composes_its_marker_never_a_failure() {
    let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
    let agent = record("bot", &[ACTION_CHAT_POST]);
    let m = module()
        .with_files_module("files")
        .with_pages_module("pages");
    let ctx = CaptureCtx::new()
        .with_registry(&registry)
        .with_transcript("general", vec![message(1, "see [Gone](duck://page/gone)")]);
    let prepared = block_on(m.prepare_dispatch(
        &ctx,
        &agent,
        &crate::run_id_for("general", 1, "bot"),
        "general",
        1,
        &[],
    ))
    .expect("an unresolvable ref never fails compose");
    let v: serde_json::Value = serde_json::from_slice(&prepared.payload).unwrap();
    let context = v["context"].as_str().unwrap();
    assert!(context.contains("[page gone — not found]"), "{context}");
}

#[test]
fn page_refs_without_a_wired_pages_module_compose_no_page_section() {
    let registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
    let agent = record("bot", &[ACTION_CHAT_POST]);
    let m = module().with_files_module("files");
    let ctx = CaptureCtx::new()
        .with_registry(&registry)
        .with_transcript("general", vec![message(1, "see [Plan](duck://page/plan)")]);
    let prepared = block_on(m.prepare_dispatch(
        &ctx,
        &agent,
        &crate::run_id_for("general", 1, "bot"),
        "general",
        1,
        &[],
    ))
    .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&prepared.payload).unwrap();
    assert!(
        v.get("context").is_none(),
        "no pages module wired composes no context key"
    );
}

#[test]
fn page_injection_composes_byte_deterministically() {
    let registry = forge_read_registry();
    let m = forge_module();
    let ctx = || {
        CaptureCtx::new()
            .with_registry(&registry)
            .with_transcript("forge:app:7", transcript(2))
            .with_forge_item("app", forge_issue(7, "Fix", "see [Plan](duck://page/plan)"))
            .with_forge_tip("app", "dev", &"cd".repeat(20))
            .with_page("plan", page_blocks("plan", "Project Plan"))
    };
    let agent = registry.get("bot").unwrap();
    let run = crate::run_id_for("forge:app:7", 2, "bot");
    let a = block_on(m.prepare_dispatch(&ctx(), agent, &run, "forge:app:7", 2, &[])).unwrap();
    let b = block_on(m.prepare_dispatch(&ctx(), agent, &run, "forge:app:7", 2, &[])).unwrap();
    assert_eq!(a.payload, b.payload, "same committed state, same bytes");
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
        .with_forge_tip("app", "dev", &"cd".repeat(20));
    let reason = compose_forge(&m, &ctx, &registry, "forge:app:7").unwrap_err();
    assert!(reason.contains("no forge item"), "{reason}");

    // issue with no dev branch to fork.
    let ctx = CaptureCtx::new()
        .with_registry(&registry)
        .with_transcript("forge:app:7", transcript(2))
        .with_forge_item("app", forge_issue(7, "t", "b"));
    let reason = compose_forge(&m, &ctx, &registry, "forge:app:7").unwrap_err();
    assert!(reason.contains("dev"), "{reason}");

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
        .with_forge_tip("app", "dev", &"cd".repeat(20));
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
    let mut watch_ctx = CaptureCtx::new()
        .with_origin(user(9))
        .with_registry(&registry);
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
        .with_forge_tip("app", "dev", &"cd".repeat(20));
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
            demands: Default::default(),
            skills: Vec::new(),
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
    let mut watch_ctx = CaptureCtx::new()
        .with_origin(user(9))
        .with_registry(&registry);
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
        .with_forge_tip("app", "dev", &"cd".repeat(20));
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
