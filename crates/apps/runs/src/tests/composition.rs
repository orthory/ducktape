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

    // files wired + a committed head: Some, head pinned, skills resolved.
    let ctx4 = CaptureCtx::new().with_files_head(&head);
    let inputs = block_on(m.portable_inputs(&ctx4, &agent)).unwrap().unwrap();
    assert_eq!(inputs.source_snapshot.as_deref(), Some(head.as_str()));
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
        inputs.source_snapshot.is_none(),
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
