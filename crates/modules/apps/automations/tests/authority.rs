use automations::{
    Action, Automations, AutomationsMsg, AutomationsQuery, AutomationsReply, Trigger,
};
use futures::executor::block_on;
use host::{BlockContext, Host};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot};
use sdk_testkit::MemStore;

struct RelayChat;
#[async_trait::async_trait(?Send)]
impl Module for RelayChat {
    fn id(&self) -> ModuleId {
        "chat".into()
    }
    fn root(&self) -> StateRoot {
        StateRoot::ZERO
    }
    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        ctx.emit_msg(Msg {
            target: "automations".into(),
            payload: msg.payload.clone(),
        });
        Ok(())
    }
    async fn query(&self, _: &[u8]) -> Result<Vec<u8>, Error> {
        Ok(chat::encode_reply(&chat::ChatReply::Access(
            chat::ChannelAccess {
                may_read: true,
                may_post: true,
            },
        )))
    }
}
struct Executor;
#[async_trait::async_trait(?Send)]
impl Module for Executor {
    fn id(&self) -> ModuleId {
        "executor".into()
    }
    fn root(&self) -> StateRoot {
        StateRoot::ZERO
    }
    async fn execute(&mut self, _: &mut dyn Ctx, _: &Msg) -> Result<(), Error> {
        Ok(())
    }
}
fn ctx(origin: Origin) -> BlockContext {
    BlockContext {
        height: 1,
        consensus_time: 1,
        origin,
    }
}
async fn identity_op(host: &mut Host, origin: Origin, msg: identity::IdentityMsg) {
    host.submit_at(
        ctx(origin),
        Msg {
            target: "identity".into(),
            payload: identity::encode_msg(&msg),
        },
    )
    .await
    .unwrap();
}
async fn arena() -> Host {
    let mut host = Host::genesis(vec![
        Box::new(Automations::new(
            "automations",
            Box::new(MemStore::new()),
            "chat",
            "tasks",
            "identity",
            "attribution",
        )),
        Box::new(identity::Identity::new(
            "identity",
            Box::new(MemStore::new()),
            "test".into(),
        )),
        Box::new(attribution::AttributionModule::new(
            "attribution",
            Box::new(MemStore::new()),
        )),
        Box::new(RelayChat),
        Box::new(Executor),
    ])
    .unwrap();
    for byte in [1, 2] {
        identity_op(
            &mut host,
            Origin::External(vec![byte; 32]),
            identity::IdentityMsg::Create {
                name: format!("User {byte}"),
                scheme: identity::KeyScheme::Ed25519,
            },
        )
        .await;
    }
    identity_op(
        &mut host,
        Origin::Module("executor".into()),
        identity::IdentityMsg::CreateProgram {
            name: "Reporter".into(),
            controller: 1,
            request: 0,
        },
    )
    .await;
    create_rule(&mut host).await;
    host
}
async fn create_rule(host: &mut Host) {
    host.submit_at(
        ctx(Origin::Program(3)),
        Msg {
            target: "automations".into(),
            payload: automations::encode_msg(&AutomationsMsg::CreateRule {
                rule_id: "report".into(),
                trigger: Trigger {
                    channel_id: None,
                    mention: None,
                    text_contains: None,
                },
                action: Action::Report {
                    recipient: 3,
                    kind: "observed".into(),
                    body_template: "{author}".into(),
                },
            }),
        },
    )
    .await
    .unwrap();
}
async fn fire(host: &mut Host, author: chat::Party) {
    host.submit_at(
        ctx(Origin::System),
        Msg {
            target: "chat".into(),
            payload: chat::encode_event(&chat::ChatEvent::MessagePosted {
                channel_id: "general".into(),
                seq: 1,
                thread_root: None,
                author,
                mentions: Vec::new(),
            }),
        },
    )
    .await
    .unwrap();
}
async fn rule(host: &Host) -> automations::Rule {
    let bytes = host
        .query(
            "automations",
            &automations::encode_query(&AutomationsQuery::GetRule {
                rule_id: "report".into(),
            }),
        )
        .await
        .unwrap();
    let AutomationsReply::Rule(Some(rule)) = automations::decode_reply(&bytes).unwrap() else {
        panic!("rule");
    };
    rule
}
async fn changes(host: &Host) -> Vec<attribution::ChangeEntry> {
    let query = attribution::AttributionQuery::Changes {
        after: 0,
        limit: 100,
    };
    let bytes = host
        .query("attribution", &attribution::encode_query(&query))
        .await
        .unwrap();
    let attribution::AttributionReply::Changes(changes) =
        attribution::decode_reply(&bytes).unwrap()
    else {
        panic!("changes");
    };
    changes
}

#[test]
fn current_identity_authority_controls_existing_standing_rules() {
    block_on(async {
        for (origin, change) in [
            (
                Origin::External(vec![1; 32]),
                identity::IdentityMsg::RevokeProgram { account: 3 },
            ),
            (
                Origin::External(vec![1; 32]),
                identity::IdentityMsg::TransferControl { account: 3, to: 2 },
            ),
            (
                Origin::Module("executor".into()),
                identity::IdentityMsg::SetProgramStanding {
                    account: 3,
                    standing: identity::ProgramStanding::Suspended,
                },
            ),
        ] {
            let mut host = arena().await;
            fire(&mut host, chat::Party::Account(3)).await;
            assert_eq!(rule(&host).await.fire_count, 1);
            identity_op(&mut host, origin, change).await;
            let before = host.root_hash();
            fire(&mut host, chat::Party::Module("worker".into())).await;
            assert_eq!(host.root_hash(), before);
            assert_eq!(rule(&host).await.fire_count, 1);
            assert_eq!(changes(&host).await.len(), 1);
        }
    });
}

#[test]
fn each_report_has_a_fresh_source_even_after_rule_recreation() {
    block_on(async {
        let mut host = arena().await;
        fire(&mut host, chat::Party::System).await;
        host.submit_at(
            ctx(Origin::Program(3)),
            Msg {
                target: "automations".into(),
                payload: automations::encode_msg(&AutomationsMsg::DeleteRule {
                    rule_id: "report".into(),
                }),
            },
        )
        .await
        .unwrap();
        create_rule(&mut host).await;
        fire(&mut host, chat::Party::Module("worker".into())).await;
        let records = changes(&host).await;
        assert_eq!(records.len(), 2);
        assert_ne!(
            records[0].change.source.object,
            records[1].change.source.object
        );
        assert!(
            records
                .iter()
                .all(|entry| entry.change.source.module == "automations"
                    && entry.change.actor == attribution::Actor::Account(3)
                    && entry.change.recipient == 3)
        );
    });
}
