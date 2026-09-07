mod support;

use futures::executor::block_on;
use support::*;

async fn page(network: &Network, query: pages::PageQuery) -> pages::PageReply {
    let bytes = network
        .host
        .query("pages", &pages::encode_query(&query))
        .await
        .unwrap();
    pages::decode_reply(&bytes).unwrap()
}

async fn configure_pages(network: &mut Network) {
    network
        .submit(
            member(),
            msg(
                "runs",
                &runs::RunsMsg::ConfigureModel {
                    operation: runs::ModelMsg::UpdateModel {
                        agent_id: "builder".into(),
                        display_name: None,
                        capability: None,
                        allowed_actions: Some(vec![
                            runs::ACTION_CHAT_POST.into(),
                            runs::ACTION_PAGES_COMMENT.into(),
                            runs::ACTION_PAGES_SET_CHECKED.into(),
                        ]),
                        recipe_hash: None,
                        caps: Some(runs::ResourceCaps {
                            pages_write: vec!["spec".into()],
                            ..Default::default()
                        }),
                        skills: None,
                    },
                },
            ),
        )
        .await;
    network
        .submit(
            member(),
            msg(
                "pages",
                &pages::PageMsg::CreatePage {
                    page_id: "spec".into(),
                    title: "Builder review this spec".into(),
                },
            ),
        )
        .await;
    network
        .submit(
            member(),
            msg(
                "pages",
                &pages::PageMsg::InsertBlock {
                    parent: "spec".into(),
                    after: None,
                    block: pages::NewBlock {
                        id: "todo".into(),
                        kind: pages::BlockKind::Todo,
                        text: "Builder review this task".into(),
                        marks: Vec::new(),
                    },
                },
            ),
        )
        .await;
    network.drain().await;
}

async fn saga_for(network: &Network, run: &runs::PendingRun) -> (String, saga::SagaView) {
    let bytes = network
        .host
        .query(
            "dispatch",
            &dispatch::encode_query(&dispatch::DispatchQuery::Dispatch {
                receiver: "runs".into(),
                dispatch_id: run.dispatch_id.clone(),
            }),
        )
        .await
        .unwrap();
    let dispatch::DispatchReply::Dispatch(Some(dispatched)) =
        dispatch::decode_reply(&bytes).unwrap()
    else {
        panic!("dispatch reply");
    };
    let dispatch::DispatchStatus::AwaitingResult { saga_id } = dispatched.status else {
        panic!("awaiting model work");
    };
    let bytes = network
        .host
        .query(
            "saga",
            &saga::encode_query(&saga::SagaQuery::Get {
                saga_id: saga_id.clone(),
            }),
        )
        .await
        .unwrap();
    let saga::SagaReply::Saga(Some(work)) = saga::decode_reply(&bytes).unwrap() else {
        panic!("saga reply");
    };
    (saga_id, work)
}

#[test]
fn page_and_block_mentions_start_model_work_and_reply_under_program_authority() {
    block_on(async {
        for target in ["spec", "todo"] {
            let mut network = Network::new().await;
            network.provision().await;
            configure_pages(&mut network).await;
            network
                .submit(
                    member(),
                    msg(
                        "pages",
                        &pages::PageMsg::SetSpanMark {
                            block_id: target.into(),
                            start: 0,
                            end: 7,
                            kind: pages::InlineMark::Mention(2),
                            active: true,
                        },
                    ),
                )
                .await;
            assert_eq!(network.runs().await.len(), 1, "the source commits first");
            network.drain().await;
            let pending = network.runs().await;
            assert_eq!(pending.len(), 2, "both source forms start model work");
            let run = pending
                .iter()
                .find(|run| run.channel_id == format!("runs:page-block:{target}"))
                .unwrap();
            assert_eq!(run.requester, sdk::Origin::Program(2));
            let (saga_id, work) = saga_for(&network, run).await;
            let spec: dispatch::WorkSpec = sdk::wire::decode(&work.spec).unwrap();
            let payload = String::from_utf8(spec.payload).unwrap();
            assert!(payload.contains(&format!(
                "Pages inline mention on block {target}, in page spec"
            )));
            assert!(payload.contains("account:1: Builder review"));
            assert!(payload.contains("Reply in a new comment thread on this block"));
            assert!(payload.contains("Builder review this task"));
            network
                .submit(
                    provider(),
                    msg(
                        "saga",
                        &saga::SagaMsg::Accept {
                            saga_id: saga_id.clone(),
                            attempt: 0,
                        },
                    ),
                )
                .await;
            network
                .submit(
                    provider(),
                    msg(
                        "runs",
                        &runs::RunsMsg::OpenAgentSession {
                            run_id: run.run_id.clone(),
                            session_key: vec![10; 32],
                        },
                    ),
                )
                .await;
            network
                .submit(
                    sdk::Origin::External(vec![10; 32]),
                    msg(
                        "runs",
                        &runs::RunsMsg::AgentAction {
                            run_id: run.run_id.clone(),
                            action: runs::AgentAction::SetPageChecked {
                                block: "todo".into(),
                                checked: true,
                            },
                        },
                    ),
                )
                .await;
            network.drain().await;
            let receipt = network
                .action(&runs::action_request_id(&run.run_id, 0))
                .await;
            assert!(
                matches!(
                    receipt.status,
                    runs::ActionStatus::Completed {
                        outcome: dispatch::CallOutcomeSummary::Rejected { .. },
                        ..
                    }
                ),
                "page grants cannot impersonate the author: {receipt:?}"
            );
            let pages::PageReply::Block(Some(todo)) = page(
                &network,
                pages::PageQuery::GetBlock {
                    block_id: "todo".into(),
                },
            )
            .await
            else {
                panic!("todo");
            };
            assert_eq!(todo.author, pages::Party::Account(1));
            assert!(!todo.checked);
            let result = sdk::wire::encode(&serde_json::json!({
                "ducktape_runner_result": 1,
                "response_text": "Reviewed the tagged source.",
                "workspace_receipt": { "source_prefix": "/home/acct:2", "output_snapshot": null, "commit_height": null, "rebased": false, "no_changes": true }
            }));
            network
                .submit(
                    provider(),
                    msg(
                        "saga",
                        &saga::SagaMsg::OracleResult {
                            saga_id,
                            attempt: 0,
                            outcome: Ok(result),
                            usage: None,
                        },
                    ),
                )
                .await;
            network.drain().await;
            let thread_id = format!("agent/{}/thread/reply", run.dispatch_id);
            let pages::PageReply::CommentThread(Some(thread)) =
                page(&network, pages::PageQuery::CommentThread { thread_id }).await
            else {
                panic!("the actual program reply must commit");
            };
            assert_eq!(thread.thread.target, target);
            assert_eq!(thread.thread.opener, pages::Party::Account(2));
            assert_eq!(thread.comments[0].author, pages::Party::Account(2));
            assert_eq!(thread.comments[0].text, "Reviewed the tagged source.");
            assert_eq!(network.runs().await.len(), 1, "the page run closed");
        }
    });
}

#[test]
fn another_module_cannot_present_its_attribution_as_a_page_block_source() {
    block_on(async {
        let mut network = Network::new().await;
        network.provision().await;
        configure_pages(&mut network).await;
        // The host authenticates the publisher. Copying a real page's object
        // id into another module's relation does not make that source Pages.
        network
            .submit(
                sdk::Origin::Module("chat".into()),
                msg(
                    "attribution",
                    &attribution::AttributionMsg::Attribute {
                        object: attribution::ObjectRef {
                            kind: "block".into(),
                            object: "spec".into(),
                        },
                        revision: 1,
                        actor: attribution::Actor::Module("chat".into()),
                        relations: vec![attribution::Relation {
                            recipient: 2,
                            reason: attribution::Reason::Mention,
                            detail: Vec::new(),
                        }],
                        transfers: Vec::new(),
                    },
                ),
            )
            .await;
        network.drain().await;
        assert_eq!(
            network.runs().await.len(),
            1,
            "a forged source starts no page run"
        );
        let bytes = network
            .host
            .query(
                "agent",
                &agent::encode_query(&agent::AgentQuery::Invocations {
                    account: 2,
                    after: 0,
                    limit: 100,
                }),
            )
            .await
            .unwrap();
        let agent::AgentReply::Invocations(invocations) = agent::decode_reply(&bytes).unwrap()
        else {
            panic!("invocations");
        };
        let last = &invocations.last().unwrap().invocation;
        assert!(last.bindings["run"].get("rejected").is_some(), "{last:?}");
        assert!(
            serde_json::to_string(last)
                .unwrap()
                .contains("no composer for the attribution source")
        );
    });
}
