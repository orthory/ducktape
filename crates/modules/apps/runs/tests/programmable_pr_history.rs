mod support;

use futures::executor::block_on;
use support::*;

struct Directory(std::path::PathBuf);
impl Directory {
    fn new(label: &str) -> Self {
        Self(std::env::temp_dir().join(format!("runs-pr-history-{label}-{}", std::process::id())))
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn push(branch: &str, previous: Option<u8>, next: Option<u8>) -> forge::ForgeMsg {
    forge::ForgeMsg::PushRefs {
        repo: "demo".into(),
        updates: vec![forge::RefUpdate {
            ref_name: branch.into(),
            prev_oid: previous.map(|byte| vec![byte; 20]),
            new_oid: next.map(|byte| vec![byte; 20]),
        }],
        pack_digest: next.map(|_| vec![9; 32]),
        cert: None,
    }
}

async fn issue(network: &mut Network, title: &str) {
    network
        .submit(
            member(),
            msg(
                "forge",
                &forge::ForgeMsg::OpenIssue {
                    repo: "demo".into(),
                    title: title.into(),
                    body: String::new(),
                },
            ),
        )
        .await;
}

async fn history(network: &Network, run: &str) -> Option<runs::RunRecord> {
    let bytes = network
        .host
        .query("runs", &runs::encode_query(&runs::RunsQuery::RecentRuns))
        .await
        .unwrap();
    let runs::RunsReply::RecentRuns(records) = runs::decode_reply(&bytes).unwrap() else {
        panic!("history");
    };
    records.into_iter().find(|record| record.run_id == run)
}

async fn item(network: &Network, number: u64) -> Option<Box<forge::ItemDetail>> {
    let bytes = network
        .host
        .query(
            "forge",
            &forge::encode_query(&forge::ForgeQuery::GetItem {
                repo: "demo".into(),
                number,
            }),
        )
        .await
        .unwrap();
    let forge::ForgeReply::Item(item) = forge::decode_reply(&bytes).unwrap() else {
        panic!("item");
    };
    item
}

async fn awaiting_pr(
    directory: &Directory,
    program: agent::Program,
) -> (Network, runs::PendingRun) {
    let mut network = Network::new().await;
    network.host.register(Box::new(
        forge::Forge::init("forge", &directory.0)
            .unwrap()
            .with_chat("chat")
            .with_attribution("attribution")
            .with_chain_id("runs-test"),
    ));
    network.provision_program(program).await;
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
                            runs::ACTION_TASKS_CREATE.into(),
                            runs::ACTION_CHAT_POST.into(),
                            runs::ACTION_CHAT_POST_MESSAGE.into(),
                        ]),
                        recipe_hash: None,
                        skills: None,
                        caps: Some(runs::ResourceCaps {
                            forge_read: vec!["demo".into()],
                            forge_push: vec!["demo".into()],
                            ..Default::default()
                        }),
                    },
                },
            ),
        )
        .await;
    network
        .submit(member(), msg("forge", &push("dev", None, Some(1))))
        .await;
    network
        .submit(
            member(),
            msg("forge", &push("agent/item-1", None, Some(0x1a))),
        )
        .await;
    issue(&mut network, "Implement the change").await;
    network
        .submit(
            member(),
            msg(
                "chat",
                &chat::ChatMsg::PostMessage {
                    channel_id: "forge:demo:1".into(),
                    message_id: "forge-anchor".into(),
                    thread: None,
                    blocks: vec![chat::Block::paragraph("Implement this issue")],
                },
            ),
        )
        .await;
    network
        .submit(
            member(),
            msg(
                "runs",
                &runs::RunsMsg::RequestRun {
                    agent_id: "builder".into(),
                    channel_id: "forge:demo:1".into(),
                    anchor_seq: 1,
                    demands: Default::default(),
                    skills: Vec::new(),
                },
            ),
        )
        .await;
    network.drain().await;
    let pending = network.runs().await;
    let Some(run) = pending
        .into_iter()
        .find(|run| run.channel_id == "forge:demo:1")
    else {
        let invocations = network
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
        panic!(
            "forge run missing: {}",
            String::from_utf8(invocations).unwrap()
        );
    };
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
    let dispatch::DispatchReply::Dispatch(Some(dispatch::DispatchView {
        status: dispatch::DispatchStatus::AwaitingResult { saga_id },
        ..
    })) = dispatch::decode_reply(&bytes).unwrap()
    else {
        panic!("dispatch");
    };
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
    assert_eq!(run.run_id, runs::run_id_for("forge:demo:1", 1, "builder"));
    assert!(run.run_id.contains('\u{1f}'));
    network
        .submit(
            provider(),
            msg(
                "runs",
                &runs::RunsMsg::OpenAgentSession {
                    run_id: run.run_id.clone(),
                    session_key: vec![9; 32],
                },
            ),
        )
        .await;
    network
        .submit(
            session(),
            msg(
                "runs",
                &runs::RunsMsg::AgentAction {
                    run_id: run.run_id.clone(),
                    action: runs::AgentAction::PostMessage {
                        channel_id: "forge:demo:1".into(),
                        text: "Working on this issue".into(),
                        thread: None,
                    },
                },
            ),
        )
        .await;
    network.drain().await;
    assert!(matches!(
        network
            .action(&runs::action_request_id(&run.run_id, 0))
            .await
            .status,
        runs::ActionStatus::Completed {
            outcome: dispatch::CallOutcomeSummary::Applied { .. },
            ..
        }
    ));
    let result = sdk::wire::encode(&serde_json::json!({
        "ducktape_runner_result": 1,
        "response_text": "Implemented the requested change.",
        "sink": {"mode":"pr", "repo":"demo", "source_branch":"agent/item-1", "target_branch":"dev"},
        "workspace_receipt": {"source_prefix":"forge:demo", "source_snapshot":null, "output_snapshot":null, "commit_height":null, "rebased":false, "no_changes":false, "branch":"agent/item-1", "output_commit":"1a".repeat(20)}
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
    while history(&network, &run.run_id).await.is_none() {
        assert!(
            network.host.has_pending_work().await.unwrap(),
            "result delivery remains queued"
        );
        network.step().await;
    }
    assert_eq!(
        history(&network, &run.run_id).await.unwrap().pr_number,
        None
    );
    assert!(
        item(&network, 2).await.is_none(),
        "a queued proposal has not opened the predicted PR"
    );
    (network, run)
}

#[test]
fn history_links_the_actual_program_allocation_after_another_item_wins_the_next_number() {
    block_on(async {
        let directory = Directory::new("allocated");
        let (mut network, run) = awaiting_pr(&directory, runs::model_program("builder")).await;
        issue(&mut network, "Another transaction allocates item two").await;
        network.drain().await;
        assert_eq!(
            history(&network, &run.run_id).await.unwrap().outcome,
            runs::RunOutcome::ResultAccepted
        );
        let bytes = network
            .host
            .query(
                "chat",
                &chat::encode_query(&chat::ChatQuery::Message {
                    message_id: runs::reply_message_id(&run.run_id),
                }),
            )
            .await
            .unwrap();
        let chat::ChatReply::Message(Some(reply)) = chat::decode_reply(&bytes).unwrap() else {
            panic!("explicit RequestRun reply must actually commit");
        };
        assert_eq!(reply.head.author, chat::Party::Account(2));
        assert_eq!(reply.head.origin, sdk::Origin::Program(2));
        assert_eq!(
            history(&network, &run.run_id).await.unwrap().pr_number,
            Some(3)
        );
        let opened = item(&network, 3).await.unwrap();
        assert_eq!(opened.summary.author, chat::Party::Account(2));
        assert_eq!(opened.source_branch.as_deref(), Some("agent/item-1"));
        assert_eq!(opened.target_branch.as_deref(), Some("dev"));
    });
}

#[test]
fn a_rejected_program_target_never_links_a_predicted_pr() {
    block_on(async {
        let directory = Directory::new("rejected");
        let (mut network, run) = awaiting_pr(&directory, runs::model_program("builder")).await;
        network
            .submit(
                member(),
                msg("forge", &push("agent/item-1", Some(0x1a), None)),
            )
            .await;
        network.drain().await;
        assert_eq!(
            history(&network, &run.run_id).await.unwrap().pr_number,
            None
        );
        assert!(item(&network, 2).await.is_none());
        assert_eq!(
            history(&network, &run.run_id).await.unwrap().outcome,
            runs::RunOutcome::ActionRejected
        );
        let receipt = network
            .action(&format!("result/{}/1", run.dispatch_id))
            .await;
        assert!(
            matches!(
                receipt.status,
                runs::ActionStatus::Completed {
                    outcome: dispatch::CallOutcomeSummary::Rejected { .. },
                    ..
                }
            ),
            "{receipt:?}"
        );
    });
}

#[test]
fn a_program_that_omits_the_target_leaves_the_pr_link_empty() {
    block_on(async {
        let directory = Directory::new("omitted");
        let mut program = runs::model_program("builder");
        for step in &mut program.steps {
            if matches!(step, agent::Step::Call { module, .. } if module == "forge") {
                *step = agent::Step::Finish;
            }
        }
        let (mut network, run) = awaiting_pr(&directory, program).await;
        network.drain().await;
        assert_eq!(
            history(&network, &run.run_id).await.unwrap().pr_number,
            None
        );
        assert!(item(&network, 2).await.is_none());
    });
}

#[test]
fn forged_program_output_cannot_redirect_the_link_of_a_successful_call() {
    block_on(async {
        let directory = Directory::new("forged");
        let mut program = runs::model_program("builder");
        for step in &mut program.steps {
            let agent::Step::Call {
                msg: agent::Value::Map(message),
                ..
            } = step
            else {
                continue;
            };
            let Some(agent::Value::Map(completion)) = message.get_mut("complete_action_request")
            else {
                continue;
            };
            completion.insert(
                "result".into(),
                agent::Value::Map(std::collections::BTreeMap::from([(
                    "applied".into(),
                    agent::Value::Map(std::collections::BTreeMap::from([
                        (
                            "output".into(),
                            agent::Value::Map(std::collections::BTreeMap::from([
                                ("repo".into(), agent::Value::Text("demo".into())),
                                ("number".into(), agent::Value::Number(999)),
                            ])),
                        ),
                        ("assigned".into(), agent::Value::Null),
                    ])),
                )])),
            );
        }
        let (mut network, run) = awaiting_pr(&directory, program).await;
        network.drain().await;
        assert_eq!(
            history(&network, &run.run_id).await.unwrap().pr_number,
            None
        );
        assert_eq!(
            item(&network, 2).await.unwrap().summary.author,
            chat::Party::Account(2)
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
        assert!(
            String::from_utf8(bytes)
                .unwrap()
                .contains("reported PR output does not match the committed action")
        );
    });
}
