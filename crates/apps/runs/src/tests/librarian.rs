use super::*;

fn valid_payload() -> LibrarianAnswerPayload {
    LibrarianAnswerPayload {
        answer: "bounded answer".into(),
        evidence_refs: vec!["forge:ducktape#106".into()],
        uncertainties: vec!["activation is intentionally absent".into()],
        degraded: false,
    }
}

fn valid_result() -> LibrarianCallResult {
    LibrarianCallResult {
        answer: valid_payload(),
        child_run_id: "librarian-child-1".into(),
        provenance: format!("{}@nodes.duck", "ab".repeat(32)),
    }
}

#[test]
fn request_bounds_are_exact_and_unknown_fields_are_rejected() {
    for (call_id, question, ok) in [
        ("c".repeat(CALL_ID_MAX_BYTES), "q".into(), true),
        ("é".repeat(CALL_ID_MAX_BYTES / 2), "q".into(), true),
        ("c".repeat(CALL_ID_MAX_BYTES + 1), "q".into(), false),
        ("c".into(), "q".repeat(QUESTION_MAX_BYTES), true),
        ("c".into(), "q".repeat(QUESTION_MAX_BYTES + 1), false),
        (String::new(), "q".into(), false),
        ("c".into(), String::new(), false),
    ] {
        let bytes = serde_json::to_vec(&serde_json::json!({
            "call_id": call_id,
            "question": question,
        }))
        .unwrap();
        assert_eq!(decode_librarian_call_request(&bytes).is_ok(), ok);
    }
    assert!(
        decode_librarian_call_request(
            br#"{"call_id":"c","question":"q","owner":"forged"}"#
        )
        .is_err()
    );
    assert!(decode_librarian_call_request(br#"{"call_id":"c"}"#).is_err());
}

#[test]
fn answer_parser_rejects_each_bound_without_truncation() {
    let mut payload = valid_payload();
    payload.answer = "a".repeat(ANSWER_MAX_BYTES);
    assert_eq!(decode_librarian_answer(&serde_json::to_vec(&payload).unwrap()).unwrap(), payload);

    payload.answer.push('a');
    assert!(decode_librarian_answer(&serde_json::to_vec(&payload).unwrap()).is_err());

    for (evidence, uncertainties) in [
        (MAX_EVIDENCE_REFS + 1, 0),
        (0, MAX_UNCERTAINTIES + 1),
    ] {
        let mut payload = valid_payload();
        payload.evidence_refs = vec!["e".into(); evidence];
        payload.uncertainties = vec!["u".into(); uncertainties];
        assert!(decode_librarian_answer(&serde_json::to_vec(&payload).unwrap()).is_err());
    }

    for field in ["evidence_refs", "uncertainties"] {
        let mut value = serde_json::to_value(valid_payload()).unwrap();
        value[field] = serde_json::json!(["x".repeat(MAX_ENTRY_BYTES + 1)]);
        assert!(decode_librarian_answer(&serde_json::to_vec(&value).unwrap()).is_err());
        value[field] = serde_json::json!([""]);
        assert!(decode_librarian_answer(&serde_json::to_vec(&value).unwrap()).is_err());
    }

    assert!(decode_librarian_answer(&vec![b' '; MAX_ENCODED_ANSWER_BYTES + 1]).is_err());
    let mut aggregate = valid_payload();
    aggregate.answer = "\n".repeat(ANSWER_MAX_BYTES);
    assert!(validate_librarian_answer_payload(&aggregate).is_err());
    assert!(
        decode_librarian_answer(
            br#"{"answer":"a","evidence_refs":[],"uncertainties":[],"degraded":false,"extra":1}"#,
        )
        .is_err()
    );
    assert!(decode_librarian_answer(b"not-json").is_err());
}

#[test]
fn complete_results_require_valid_child_run_and_lowercase_node_provenance() {
    let valid = valid_result();
    assert_eq!(
        librarian_provenance(&"ab".repeat(32)).unwrap(),
        valid.provenance
    );
    assert_eq!(
        decode_librarian_call_result(&serde_json::to_vec(&valid).unwrap()).unwrap(),
        valid
    );
    for provenance in [
        format!("{}@nodes.duck", "AB".repeat(32)),
        format!("{}@nodes.duck", "ab".repeat(31)),
        "unknown@nodes.duck".into(),
        "ab".repeat(32),
    ] {
        let mut result = valid_result();
        result.provenance = provenance;
        assert!(validate_librarian_call_result(&result).is_err());
    }
    let mut result = valid_result();
    result.child_run_id.clear();
    assert!(validate_librarian_call_result(&result).is_err());
    let mut unknown = serde_json::to_value(valid_result()).unwrap();
    unknown["owner"] = serde_json::json!("forged");
    assert!(decode_librarian_call_result(&serde_json::to_vec(&unknown).unwrap()).is_err());
}

#[test]
fn dormant_mutations_fail_identically_without_effects_or_state_movement() {
    let origins = [
        Origin::External(vec![1; 32]),
        Origin::External(vec![2; 32]),
        Origin::External(vec![3; 32]),
        Origin::External(Vec::new()),
        Origin::System,
    ];
    for origin in origins {
        for op in [
            RunsMsg::BeginLibrarianCall {
                run_id: "parent".into(),
                call_id: "call".into(),
                question: "question".into(),
            },
            RunsMsg::CancelLibrarianCall {
                run_id: "parent".into(),
                call_id: "call".into(),
            },
        ] {
            let mut module = module();
            assert_eq!(module.state_schema_revision(), 2);
            let before_root = module.root();
            let before_snapshot = module.snapshot();
            let mut ctx = CaptureCtx::new().with_origin(origin.clone());
            let err = exec(&mut module, &mut ctx, &admin(&op)).unwrap_err();
            assert_eq!(err, Error::Module(LIBRARIAN_REGENESIS_REQUIRED.into()));
            assert!(ctx.msgs.is_empty());
            assert!(ctx.events.is_empty());
            assert_eq!(module.root(), before_root);
            assert_eq!(module.snapshot(), before_snapshot);
        }
    }
}

#[test]
fn dormant_queries_are_deterministic() {
    let module = module();
    assert_eq!(
        block_on(module.query(&encode_query(&RunsQuery::LibrarianAvailability {
            run_id: "any".into(),
        })))
        .unwrap(),
        encode_reply(&RunsReply::LibrarianAvailability(LibrarianAvailability {
            feature_active: false,
            permitted: false,
            remaining_child_budget: 0,
        }))
    );
    assert_eq!(
        block_on(module.query(&encode_query(&RunsQuery::LibrarianCall {
            parent_run_id: "any".into(),
            call_id: "any".into(),
        })))
        .unwrap(),
        encode_reply(&RunsReply::LibrarianCall(None))
    );
}

#[test]
fn every_preexisting_wire_variant_keeps_its_golden_bytes() {
    let mut demands = BTreeMap::new();
    demands.insert("cores".into(), 2);
    let messages = [
        (
            RunsMsg::WatchChannel {
                channel_id: "c".into(),
                policy: TurnPolicy::Mention,
            },
            r#"{"watch_channel":{"channel_id":"c","policy":"mention"}}"#,
        ),
        (
            RunsMsg::UnwatchChannel {
                channel_id: "c".into(),
            },
            r#"{"unwatch_channel":{"channel_id":"c"}}"#,
        ),
        (
            RunsMsg::EnableJobWorker { enabled: true },
            r#"{"enable_job_worker":{"enabled":true}}"#,
        ),
        (
            RunsMsg::RequestRun {
                agent_id: "a".into(),
                channel_id: "c".into(),
                anchor_seq: 7,
                demands,
            },
            concat!(
                r#"{"request_run":{"agent_id":"a","channel_id":"c","anchor_seq":7,"#,
                r#""demands":{"cores":2}}}"#,
            ),
        ),
        (
            RunsMsg::CancelRun { run_id: "r".into() },
            r#"{"cancel_run":{"run_id":"r"}}"#,
        ),
        (
            RunsMsg::ReassignRun {
                run_id: "r".into(),
                attempt: 3,
            },
            r#"{"reassign_run":{"run_id":"r","attempt":3}}"#,
        ),
        (
            RunsMsg::OpenAgentSession {
                run_id: "r".into(),
                session_key: vec![1, 2],
            },
            r#"{"open_agent_session":{"run_id":"r","session_key":[1,2]}}"#,
        ),
        (
            RunsMsg::AgentAction {
                run_id: "r".into(),
                action: AgentAction::CreateTask {
                    task_id: "t".into(),
                    title: "T".into(),
                },
            },
            concat!(
                r#"{"agent_action":{"run_id":"r","action":{"create_task":{"task_id":"t","#,
                r#""title":"T"}}}}"#,
            ),
        ),
    ];
    for (message, golden) in messages {
        assert_eq!(encode_msg(&message), golden.as_bytes());
        assert_eq!(decode_msg(golden.as_bytes()).unwrap(), message);
    }

    let queries = [
        (RunsQuery::PendingRuns, r#""pending_runs""#),
        (RunsQuery::Watches, r#""watches""#),
        (RunsQuery::RecentRuns, r#""recent_runs""#),
        (RunsQuery::AgentSessions, r#""agent_sessions""#),
    ];
    for (query, golden) in queries {
        assert_eq!(encode_query(&query), golden.as_bytes());
        assert_eq!(decode_query(golden.as_bytes()).unwrap(), query);
    }

    let replies = [
        (RunsReply::PendingRuns(Vec::new()), r#"{"pending_runs":[]}"#),
        (RunsReply::Watches(Vec::new()), r#"{"watches":[]}"#),
        (RunsReply::RecentRuns(Vec::new()), r#"{"recent_runs":[]}"#),
        (
            RunsReply::AgentSessions(Vec::new()),
            r#"{"agent_sessions":[]}"#,
        ),
    ];
    for (reply, golden) in replies {
        assert_eq!(encode_reply(&reply), golden.as_bytes());
        assert_eq!(decode_reply(golden.as_bytes()).unwrap(), reply);
    }
}
