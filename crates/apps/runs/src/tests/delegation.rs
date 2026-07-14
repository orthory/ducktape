use super::*;

fn request(agent_id: &str, instruction: impl Into<String>) -> DelegationRequest {
    DelegationRequest {
        agent_id: agent_id.into(),
        instruction: instruction.into(),
    }
}

fn delegated_response(requests: Vec<DelegationRequest>) -> Vec<u8> {
    let prose = agent::encode_response(&AgentResponse {
        reply_blocks: vec![ReplyBlock {
            kind: "paragraph".into(),
            text: "Delegated the bounded work wave.".into(),
            lang: None,
        }],
        actions: Vec::new(),
        delegations: requests,
        commit_message: None,
    });
    runner_wrapper(&String::from_utf8(prose).unwrap(), serde_json::json!({}))
}

fn delegation_registry(budget: u32) -> Registry {
    let mut registry = registry(&[
        ("bot", &[ACTION_CHAT_POST]),
        ("child-a", &[ACTION_CHAT_POST]),
        ("child-b", &[ACTION_CHAT_POST]),
    ]);
    let parent = registry.get_mut("bot").unwrap();
    parent.caps.subagent_budget = budget;
    parent.caps.duckfs_read.push("/shared/skills".into());
    registry.get_mut("child-a").unwrap().skills = vec![agent::SkillRef {
        name: "specialist".into(),
        source_prefix: "/shared/skills/specialist".into(),
        source_snapshot: None,
        load: agent::LoadMode::Always,
    }];
    registry
}

fn start_parent(registry: &Registry) -> (RunsModule, String) {
    let mut module = watched(TurnPolicy::Mention, registry);
    engage_post(&mut module, registry, 2, &["bot"]);
    commit(&mut module);
    (module, run_id_for("general", 2, "bot"))
}

#[test]
fn final_response_stages_one_bounded_child_wave_with_parent_context() {
    let registry = delegation_registry(2);
    assert!(registry["bot"].skills.is_empty());
    let mut module = watched(TurnPolicy::Mention, &registry);
    let transcript = vec![
        message(1, "root"),
        message_in(
            "general",
            2,
            AuthorRef::User(vec![1; 32]),
            "split this",
            Some(1),
        ),
    ];
    let mut engage = CaptureCtx::new()
        .at(2)
        .with_tagging_origin()
        .with_registry(&registry)
        .with_transcript("general", transcript.clone());
    exec(
        &mut module,
        &mut engage,
        &engagement("general", 2, vec![agent_tag("bot")]),
    )
    .unwrap();
    commit(&mut module);
    let parent_run = run_id_for("general", 2, "bot");

    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", transcript);
    exec(
        &mut module,
        &mut ctx,
        &result_event(
            &parent_run,
            Ok(delegated_response(vec![
                request("child-a", "Implement the parser."),
                request("child-b", "Verify the boundary."),
            ])),
        ),
    )
    .unwrap();

    let dispatches = ctx.dispatch_msgs();
    assert_eq!(dispatches.len(), 2);
    for (agent_id, instruction) in [
        ("child-a", "Implement the parser."),
        ("child-b", "Verify the boundary."),
    ] {
        let child_run = run_id_for("general", 2, agent_id);
        let dispatch = dispatches
            .iter()
            .find(|dispatch| {
                matches!(dispatch, DispatchMsg::Dispatch { recipe_id, .. } if recipe_id == &recipe_id_for(agent_id))
            })
            .expect("one dispatch per delegated child");
        let DispatchMsg::Dispatch {
            dispatch_id,
            payload,
            demands,
            admission,
            ..
        } = dispatch
        else {
            unreachable!()
        };
        assert_eq!(*dispatch_id, dispatch_id_for(&child_run));
        assert_eq!(
            *demands,
            BTreeMap::from([
                ("cores".into(), DELEGATED_CHILD_CORES),
                ("mem_gb".into(), DELEGATED_CHILD_MEM_GB),
            ])
        );
        assert_eq!(*admission, dispatch::AdmissionPolicy::Queue);
        let envelope: serde_json::Value = serde_json::from_slice(payload).unwrap();
        assert_eq!(envelope["run_id"], child_run);
        assert_eq!(envelope["agent_id"], agent_id);
        assert_eq!(envelope["thread_key"], "general#1");
        assert_eq!(
            envelope["workspace"]["source_prefix"],
            "/shared/agent-workspaces/bot"
        );
        if agent_id == "child-a" {
            assert_eq!(
                envelope["skills"][0]["source_prefix"],
                "/shared/skills/specialist"
            );
        }
        let context = envelope["context"].as_str().unwrap();
        assert!(
            context.contains(&format!("Parent run: {parent_run}")),
            "{context}"
        );
        assert!(context.contains("Parent agent: bot"), "{context}");
        assert!(context.contains(instruction), "{context}");
    }

    commit(&mut module);
    assert!(get_pending(&module, &parent_run).is_none());
    for child in ["child-a", "child-b"] {
        let pending = get_pending(&module, &run_id_for("general", 2, child)).unwrap();
        assert_eq!(pending.thread_root, Some(1));
        assert_eq!(pending.requester, SagaOrigin::Module("tagging".into()));
    }
    let ChatMsg::PostMessage { thread, .. } = &ctx.chat_msgs()[0] else {
        panic!("expected the parent reply")
    };
    assert_eq!(*thread, Some(1));
}

#[test]
fn invalid_delegation_batches_fail_without_staging_any_child() {
    for case in [
        "inactive",
        "cross-owner",
        "different-capability",
        "unreadable-skill",
        "authority-escalation",
        "nested",
        "duplicate",
        "over-budget",
        "over-hard-cap",
        "self",
        "empty-instruction",
        "oversized-instruction",
        "oversized-batch",
        "taken-turn",
    ] {
        let mut registry = delegation_registry(2);
        let requests = match case {
            "inactive" => {
                pause(&mut registry, "child-b");
                vec![
                    request("child-a", "valid first"),
                    request("child-b", "paused"),
                ]
            }
            "cross-owner" => {
                registry.get_mut("child-a").unwrap().owner = SagaOrigin::External(vec![8; 32]);
                vec![request("child-a", "cross owner")]
            }
            "different-capability" => {
                registry.get_mut("child-a").unwrap().capability = "model-2".into();
                vec![request("child-a", "other runtime")]
            }
            "unreadable-skill" => {
                registry.get_mut("child-a").unwrap().skills[0].source_prefix =
                    "/private/specialist".into();
                vec![request("child-a", "unreadable specialist")]
            }
            "authority-escalation" => {
                registry
                    .get_mut("child-a")
                    .unwrap()
                    .caps
                    .forge_push
                    .push("app".into());
                vec![request("child-a", "push")]
            }
            "nested" => {
                registry.get_mut("child-a").unwrap().caps.subagent_budget = 1;
                vec![request("child-a", "nest")]
            }
            "duplicate" => vec![request("child-a", "one"), request("child-a", "two")],
            "over-budget" => {
                registry.get_mut("bot").unwrap().caps.subagent_budget = 1;
                vec![request("child-a", "one"), request("child-b", "two")]
            }
            "over-hard-cap" => {
                registry.get_mut("bot").unwrap().caps.subagent_budget = 9;
                (0..=MAX_DELEGATIONS_PER_RUN)
                    .map(|index| request(&format!("child-{index}"), "work"))
                    .collect()
            }
            "self" => vec![request("bot", "recurse")],
            "empty-instruction" => vec![request("child-a", " \n ")],
            "oversized-instruction" => vec![request(
                "child-a",
                "x".repeat(MAX_DELEGATION_INSTRUCTION_BYTES + 1),
            )],
            "oversized-batch" => vec![
                request("child-a", "x".repeat(MAX_DELEGATION_INSTRUCTION_BYTES)),
                request("child-b", "y".repeat(MAX_DELEGATION_INSTRUCTION_BYTES)),
            ],
            "taken-turn" => vec![request("child-a", "already ran")],
            _ => unreachable!(),
        };
        let (mut module, parent_run) = start_parent(&registry);
        let mut ctx = CaptureCtx::new()
            .at(8)
            .with_dispatch_origin()
            .with_registry(&registry)
            .with_transcript("general", transcript(2));
        if case == "taken-turn" {
            ctx = ctx.with_taken_dispatch(&dispatch_id_for(&run_id_for("general", 2, "child-a")));
        }
        exec(
            &mut module,
            &mut ctx,
            &result_event(&parent_run, Ok(delegated_response(requests))),
        )
        .unwrap();
        assert!(
            ctx.dispatch_msgs().is_empty(),
            "{case} must not stage a partial child wave"
        );
        assert!(
            ctx.notes().iter().any(|note| note.contains("failed")),
            "{case} should leave a failure breadcrumb: {:?}",
            ctx.notes()
        );
    }
}

#[test]
fn hard_fanout_cap_accepts_eight_children() {
    let mut registry = registry(&[("bot", &[ACTION_CHAT_POST])]);
    registry.get_mut("bot").unwrap().caps.subagent_budget = 9;
    let mut requests = Vec::new();
    for index in 0..MAX_DELEGATIONS_PER_RUN {
        let agent_id = format!("child-{index}");
        registry.insert(agent_id.clone(), record(&agent_id, &[ACTION_CHAT_POST]));
        requests.push(request(&agent_id, "bounded work"));
    }
    let (mut module, parent_run) = start_parent(&registry);
    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", transcript(2));
    exec(
        &mut module,
        &mut ctx,
        &result_event(&parent_run, Ok(delegated_response(requests))),
    )
    .unwrap();
    assert_eq!(ctx.dispatch_msgs().len(), MAX_DELEGATIONS_PER_RUN);
}

#[test]
fn delegated_child_preserves_requester_and_owner_lifecycle_control() {
    let registry = delegation_registry(1);
    let mut module = watched(TurnPolicy::Mention, &registry);
    let mut request_ctx = CaptureCtx::new()
        .with_origin(user(1))
        .with_registry(&registry)
        .with_transcript("general", transcript(2));
    exec(
        &mut module,
        &mut request_ctx,
        &admin(&RunsMsg::RequestRun {
            agent_id: "bot".into(),
            channel_id: "general".into(),
            anchor_seq: 2,
            demands: Default::default(),
        }),
    )
    .unwrap();
    commit(&mut module);
    let parent_run = run_id_for("general", 2, "bot");
    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", transcript(2));
    exec(
        &mut module,
        &mut ctx,
        &result_event(
            &parent_run,
            Ok(delegated_response(vec![request("child-a", "bounded work")])),
        ),
    )
    .unwrap();
    commit(&mut module);
    let child_run = run_id_for("general", 2, "child-a");
    assert!(get_pending(&module, &child_run).is_some());

    let mut ctx = CaptureCtx::new()
        .with_origin(user(1))
        .with_registry(&registry);
    exec(
        &mut module,
        &mut ctx,
        &admin(&RunsMsg::CancelRun {
            run_id: child_run.clone(),
        }),
    )
    .unwrap();
    assert_eq!(
        ctx.dispatch_msgs(),
        vec![DispatchMsg::CancelDispatch {
            dispatch_id: dispatch_id_for(&child_run),
        }]
    );
    abort(&mut module);

    let mut ctx = CaptureCtx::new()
        .with_origin(user(9))
        .with_registry(&registry);
    exec(
        &mut module,
        &mut ctx,
        &admin(&RunsMsg::ReassignRun {
            run_id: child_run.clone(),
            attempt: 0,
        }),
    )
    .unwrap();
    assert_eq!(
        ctx.dispatch_msgs(),
        vec![DispatchMsg::ReassignDispatch {
            dispatch_id: dispatch_id_for(&child_run),
            attempt: 0,
        }]
    );
}

#[test]
fn delegated_child_inherits_the_pinned_forge_item_workspace() {
    let channel = "forge:ducktape:7";
    let mut registry = delegation_registry(1);
    for agent_id in ["bot", "child-a"] {
        registry
            .get_mut(agent_id)
            .unwrap()
            .caps
            .forge_read
            .push("ducktape".into());
    }
    let mut item = forge_issue(7, "Fix delegation", "keep the item workspace pinned");
    item.channel_id = channel.into();
    let branch_tip = "ef".repeat(20);
    let transcript = transcript(2);
    let mut module = forge_module();
    let mut request_ctx = CaptureCtx::new()
        .with_origin(user(1))
        .with_registry(&registry)
        .with_transcript(channel, transcript.clone())
        .with_forge_item("ducktape", item.clone())
        .with_forge_tip("ducktape", "dev", &"cd".repeat(20))
        .with_forge_tip("ducktape", "agent/item-7", &branch_tip);
    exec(
        &mut module,
        &mut request_ctx,
        &admin(&RunsMsg::RequestRun {
            agent_id: "bot".into(),
            channel_id: channel.into(),
            anchor_seq: 2,
            demands: Default::default(),
        }),
    )
    .unwrap();
    let DispatchMsg::Dispatch { payload, .. } = &request_ctx.dispatch_msgs()[0] else {
        panic!("expected parent dispatch")
    };
    let parent: serde_json::Value = serde_json::from_slice(payload).unwrap();
    commit(&mut module);

    let parent_run = run_id_for(channel, 2, "bot");
    let mut result_ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript(channel, transcript)
        .with_forge_item("ducktape", item)
        .with_forge_tip("ducktape", "dev", &"cd".repeat(20))
        .with_forge_tip("ducktape", "agent/item-7", &branch_tip);
    exec(
        &mut module,
        &mut result_ctx,
        &result_event(
            &parent_run,
            Ok(delegated_response(vec![request("child-a", "verify it")])),
        ),
    )
    .unwrap();
    let DispatchMsg::Dispatch { payload, .. } = &result_ctx.dispatch_msgs()[0] else {
        panic!("expected child dispatch")
    };
    let child: serde_json::Value = serde_json::from_slice(payload).unwrap();

    assert_eq!(child["workspace"], parent["workspace"]);
    assert_eq!(child["workspace"]["kind"], "forge");
    assert_eq!(child["workspace"]["branch"], "agent/item-7");
    assert_eq!(child["workspace"]["commit"], branch_tip);
    assert_eq!(
        child["result_contract"]["sink"],
        parent["result_contract"]["sink"]
    );
    let sink = &child["result_contract"]["sink"];
    assert_eq!(sink["mode"], "pr");
    assert_eq!(sink["repo"], "ducktape");
    assert_eq!(sink["source_branch"], "agent/item-7");
    assert_eq!(sink["target_branch"], "dev");
    let parent_context = parent["context"].as_str().unwrap();
    let child_context = child["context"].as_str().unwrap();
    assert!(child_context.starts_with(parent_context), "{child_context}");
    assert!(child_context.contains("issue #7"), "{child_context}");
    assert!(child_context.contains("title: Fix delegation"), "{child_context}");
    assert!(
        child_context.contains("work branch: agent/item-7"),
        "{child_context}"
    );
    assert!(child_context.contains(&format!("Parent run: {parent_run}")));
    assert!(child_context.contains("Instruction:\nverify it"));
}

#[test]
fn delegation_is_final_only_and_limited_to_chat_or_forge() {
    let registry = delegation_registry(1);
    let module = module();
    let ctx = CaptureCtx::new()
        .with_registry(&registry)
        .with_transcript("general", transcript(2));
    let response = || AgentResponse {
        delegations: vec![request("child-a", "work")],
        ..AgentResponse::default()
    };
    let entry = |channel_id: String, job_id: Option<String>| PendingState {
        agent_id: "bot".into(),
        channel_id,
        anchor_seq: 2,
        thread_root: None,
        job_id,
        job_claim_height: 0,
        requester: SagaOrigin::External(vec![9; 32]),
        created_at: 0,
    };

    let session = block_on(module.validate_response(
        &ctx,
        "parent",
        &entry("general".into(), None),
        Lane::Session(0),
        response(),
    ))
    .unwrap_err();
    assert!(session.contains("final-only"), "{session}");

    let forge = block_on(module.validate_response(
        &ctx,
        "parent",
        &entry("forge:app:7".into(), None),
        Lane::Settle,
        response(),
    ))
    .unwrap();
    assert_eq!(forge.delegations.len(), 1);

    for source in [
        entry(page_channel_id("thread-1"), None),
        entry(String::new(), Some("job-1".into())),
    ] {
        let reason =
            block_on(module.validate_response(&ctx, "parent", &source, Lane::Settle, response()))
                .unwrap_err();
        assert!(reason.contains("chat or Forge"), "{reason}");
    }
}
