mod support;
use futures::executor::block_on;
use sdk::Msg;
use support::*;

#[test]
fn mention_and_interactive_action_use_the_actual_program_account() {
    block_on(async {
        let mut network = Network::new().await;
        let run_id = network.provision().await;
        network
            .submit(
                session(),
                msg(
                    "runs",
                    &runs::RunsMsg::AgentAction {
                        run_id: run_id.clone(),
                        action: runs::AgentAction::CreateTask {
                            task_id: "made-by-program".into(),
                            title: "Actual committed tool write".into(),
                        },
                    },
                ),
            )
            .await;
        assert!(
            network.task("made-by-program").await.is_none(),
            "session admission does not impersonate the user"
        );
        let id = runs::action_request_id(&run_id, 0);
        assert!(matches!(
            network.action(&id).await.status,
            runs::ActionStatus::AwaitingProgram
        ));
        network.drain().await;
        let request = network.action(&id).await;
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
        let invocations = String::from_utf8(invocations).unwrap();
        assert!(
            matches!(
                request.status,
                runs::ActionStatus::Completed {
                    outcome: dispatch::CallOutcomeSummary::Applied { .. },
                    ..
                }
            ),
            "{request:?}\n{invocations}"
        );
        assert_eq!(
            network.task("made-by-program").await.unwrap().owner,
            tasks::Party::Account(2)
        );
        let bytes = network
            .host
            .query(
                "identity",
                &identity::encode_query(&identity::IdentityQuery::Get { number: 2 }),
            )
            .await
            .unwrap();
        let identity::IdentityReply::Account(Some(account)) =
            identity::decode_reply(&bytes).unwrap()
        else {
            panic!("account reply");
        };
        assert!(
            account.keys.is_empty(),
            "the external run credential never becomes a user key"
        );
    });
}

#[test]
fn revoking_a_program_rejects_its_waiting_tool_request_without_a_write() {
    block_on(async {
        let mut network = Network::new().await;
        let run_id = network.provision().await;
        network
            .submit(
                session(),
                msg(
                    "runs",
                    &runs::RunsMsg::AgentAction {
                        run_id: run_id.clone(),
                        action: runs::AgentAction::CreateTask {
                            task_id: "revoked".into(),
                            title: "Must not land".into(),
                        },
                    },
                ),
            )
            .await;
        network
            .submit(
                member(),
                msg(
                    "identity",
                    &identity::IdentityMsg::RevokeProgram { account: 2 },
                ),
            )
            .await;
        network.drain().await;
        let request = network.action(&runs::action_request_id(&run_id, 0)).await;
        assert!(
            matches!(request.status, runs::ActionStatus::Rejected { .. }),
            "{request:?}"
        );
        assert!(network.task("revoked").await.is_none());
    });
}

async fn propose_task(network: &mut Network, run_id: &str, task_id: &str) -> String {
    network
        .submit(
            session(),
            msg(
                "runs",
                &runs::RunsMsg::AgentAction {
                    run_id: run_id.into(),
                    action: runs::AgentAction::CreateTask {
                        task_id: task_id.into(),
                        title: "A tool write".into(),
                    },
                },
            ),
        )
        .await;
    runs::action_request_id(run_id, 0)
}

#[test]
fn an_actual_target_rejection_reaches_the_tool_receipt() {
    block_on(async {
        let mut network = Network::new().await;
        let run = network.provision().await;
        let request = propose_task(&mut network, &run, "contended").await;
        network
            .submit(
                member(),
                Msg {
                    target: "tasks".into(),
                    payload: tasks::encode_task_msg(&tasks::TaskMsg::CreateTask {
                        task_id: "contended".into(),
                        title: "Member won".into(),
                        owner: None,
                    }),
                },
            )
            .await;
        network.drain().await;
        assert!(matches!(
            network.action(&request).await.status,
            runs::ActionStatus::Completed {
                outcome: dispatch::CallOutcomeSummary::Rejected { .. },
                ..
            }
        ));
        assert_eq!(
            network.task("contended").await.unwrap().owner,
            tasks::Party::Account(1)
        );
    });
}

#[test]
fn a_program_that_ignores_a_tool_request_returns_a_terminal_receipt() {
    block_on(async {
        let mut program = runs::model_program("builder");
        let finish = program.steps.len() as u64 - 1;
        let agent::Step::Branch { then, .. } = &mut program.steps[0] else {
            panic!("default router");
        };
        *then = finish;
        let mut network = Network::new().await;
        let run = network.provision_program(program).await;
        let request = propose_task(&mut network, &run, "ignored").await;
        network.drain().await;
        assert!(matches!(
            network.action(&request).await.status,
            runs::ActionStatus::Rejected { .. }
        ));
        assert!(network.task("ignored").await.is_none());
    });
}

#[test]
fn pausing_the_model_rejects_unclaimed_session_work() {
    block_on(async {
        let mut network = Network::new().await;
        let run = network.provision().await;
        let request = propose_task(&mut network, &run, "paused").await;
        network
            .submit(
                member(),
                msg(
                    "runs",
                    &runs::RunsMsg::ConfigureModel {
                        operation: runs::ModelMsg::PauseModel {
                            agent_id: "builder".into(),
                        },
                    },
                ),
            )
            .await;
        network.drain().await;
        assert!(matches!(
            network.action(&request).await.status,
            runs::ActionStatus::Rejected { .. }
        ));
        assert!(network.task("paused").await.is_none());
    });
}

#[test]
fn an_applied_target_stays_applied_when_authority_is_revoked_before_completion() {
    block_on(async {
        let mut network = Network::new().await;
        let run = network.provision().await;
        let request = propose_task(&mut network, &run, "already-applied").await;
        loop {
            assert!(
                network.host.has_pending_work().await.unwrap(),
                "the action must reach a terminal state"
            );
            network.step().await;
            if network.task("already-applied").await.is_some() {
                break;
            }
        }
        network
            .submit(
                member(),
                msg(
                    "identity",
                    &identity::IdentityMsg::RevokeProgram { account: 2 },
                ),
            )
            .await;
        network.drain().await;
        assert!(matches!(
            network.action(&request).await.status,
            runs::ActionStatus::Completed {
                outcome: dispatch::CallOutcomeSummary::Applied { .. },
                ..
            }
        ));
    });
}

#[test]
fn manual_requests_preserve_the_authenticated_humans_control_and_turn_deduplication() {
    block_on(async {
        let mut network = Network::new().await;
        network.provision().await;
        let caller = sdk::Origin::External(vec![3; 32]);
        network
            .submit(
                member(),
                msg(
                    "chat",
                    &chat::ChatMsg::PostMessage {
                        channel_id: "general".into(),
                        message_id: "manual-anchor".into(),
                        blocks: vec![chat::Block::paragraph("Please work here")],
                        thread: None,
                    },
                ),
            )
            .await;
        let request = runs::RunsMsg::RequestRun {
            agent_id: "builder".into(),
            channel_id: "general".into(),
            anchor_seq: 2,
            demands: Default::default(),
            skills: Vec::new(),
        };
        network.submit(caller.clone(), msg("runs", &request)).await;
        network.submit(caller.clone(), msg("runs", &request)).await;
        network.drain().await;
        let manual_id = runs::run_id_for("general", 2, "builder");
        let manual: Vec<_> = network
            .runs()
            .await
            .into_iter()
            .filter(|run| run.run_id == manual_id)
            .collect();
        assert_eq!(
            manual.len(),
            1,
            "duplicate requests share one model/anchor turn"
        );
        assert_eq!(manual[0].requester, caller);
        assert_eq!(
            network.runs().await.len(),
            2,
            "one mention run and one requested run"
        );
        let denied = network
            .host
            .submit_at(
                host::BlockContext {
                    height: network.height + 1,
                    consensus_time: network.height + 1,
                    origin: sdk::Origin::External(vec![4; 32]),
                },
                msg(
                    "runs",
                    &runs::RunsMsg::CancelRun {
                        run_id: manual_id.clone(),
                    },
                ),
            )
            .await;
        assert!(
            denied.is_err(),
            "a different human cannot claim requester authority"
        );
        network.height += 1;
        network
            .submit(
                sdk::Origin::External(vec![7; 32]),
                msg(
                    "capability",
                    &capability::CapabilityMsg::Announce {
                        capabilities: vec!["model-1".into()],
                        resources: Default::default(),
                    },
                ),
            )
            .await;

        network
            .submit(
                caller.clone(),
                msg(
                    "runs",
                    &runs::RunsMsg::ReassignRun {
                        run_id: manual_id.clone(),
                        attempt: 0,
                    },
                ),
            )
            .await;
        network
            .submit(
                caller,
                msg(
                    "runs",
                    &runs::RunsMsg::CancelRun {
                        run_id: manual_id.clone(),
                    },
                ),
            )
            .await;
        network.drain().await;
        assert!(
            !network
                .runs()
                .await
                .iter()
                .any(|run| run.run_id == manual_id)
        );
    });
}

#[test]
fn another_sources_run_request_detail_cannot_assign_manual_requester_authority() {
    block_on(async {
        use agent::{Continuation, Decode, Program, Step, Value};
        use std::collections::BTreeMap;
        let mut network = Network::new().await;
        network.provision().await;
        let program = Program {
            steps: vec![
                Step::Call {
                    module: "runs".into(),
                    msg: Value::Map(BTreeMap::from([(
                        "request_attributed_run".into(),
                        Value::Map(BTreeMap::from([
                            ("agent_id".into(), Value::Text("builder".into())),
                            (
                                "change_seq".into(),
                                Value::Ref(vec!["change".into(), "seq".into()]),
                            ),
                        ])),
                    )])),
                    bind: "outcome".into(),
                    decode: Decode::Json,
                    on_failure: Continuation::Unhandled,
                },
                Step::Finish,
            ],
        };
        network
            .submit(
                member(),
                msg(
                    "agent",
                    &agent::AgentMsg::Replace {
                        account: 2,
                        program,
                    },
                ),
            )
            .await;
        // This module origin is the host test seam for an authenticated content
        // source. Attribution stamps "chat", so a copied runs detail has no
        // authority even when the account's program tries to consume it.
        network.submit(sdk::Origin::Module("chat".into()), msg("attribution", &attribution::AttributionMsg::Attribute {
            object: attribution::ObjectRef { kind: "run_request".into(), object: "forged".into() }, revision: 1,
            actor: attribution::Actor::Module("chat".into()),
            relations: vec![attribution::Relation { recipient: 2, reason: attribution::Reason::Defined("model_run".into()),
                detail: sdk::wire::encode(&serde_json::json!({"manual":{"requester":{"external":vec![3;32]},"agent_id":"builder","channel_id":"general","anchor_seq":1,"demands":{},"skills":[]}})),
            }], transfers: Vec::new(),
        })).await;
        network.drain().await;
        assert_eq!(
            network.runs().await.len(),
            1,
            "the existing mention run is the only run"
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
        assert!(
            matches!(last.status, agent::Status::Failed { .. }),
            "{last:?}"
        );
        assert!(
            serde_json::to_string(last)
                .unwrap()
                .contains("no composer for the attribution source")
        );
    });
}
