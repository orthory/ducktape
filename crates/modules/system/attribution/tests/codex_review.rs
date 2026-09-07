use attribution::{
    Actor, AttributionModule, AttributionMsg, AttributionQuery, AttributionReply, ObjectRef,
    Reason, Relation, Source, decode_reply, encode_msg, encode_query,
};
use futures::executor::block_on;
use sdk::{Env, Module, Msg, Origin};
use sdk_testkit::{MemStore, TestCtx};

fn module() -> AttributionModule {
    AttributionModule::new("attribution", Box::new(MemStore::new()))
}

fn source_context() -> TestCtx {
    TestCtx::with_env(Env {
        height: 1,
        consensus_time: 1,
        origin: Origin::Module("chat".into()),
        me: "attribution".into(),
        cause: sdk::Cause::Direct,
    })
}

fn report(actor: Actor, relations: Vec<Relation>) -> Msg {
    Msg {
        target: "attribution".into(),
        payload: encode_msg(&AttributionMsg::Attribute {
            object: ObjectRef {
                kind: "message".into(),
                object: "review-message".into(),
            },
            revision: 1,
            actor,
            relations,
            transfers: Vec::new(),
        }),
    }
}

#[test]
fn maximum_history_cursors_are_past_the_end() {
    block_on(async {
        let module = module();
        let queries = [
            AttributionQuery::Changes {
                after: u64::MAX,
                limit: 1,
            },
            AttributionQuery::ChangesFor {
                recipient: 1,
                after: u64::MAX,
                limit: 1,
            },
            AttributionQuery::ChangesOf {
                source: Source {
                    module: "chat".into(),
                    kind: "message".into(),
                    object: "review-message".into(),
                },
                after: u64::MAX,
                limit: 1,
            },
        ];
        for query in queries {
            let bytes = module.query(&encode_query(&query)).await.unwrap();
            assert_eq!(
                decode_reply(&bytes).unwrap(),
                AttributionReply::Changes(Vec::new()),
                "a past-the-end cursor returns an empty page"
            );
        }
    });
}

#[test]
fn zero_is_not_an_actor_or_recipient_account() {
    block_on(async {
        for (actor, recipient) in [(Actor::Account(0), 1), (Actor::Account(1), 0)] {
            let mut module = module();
            let before = module.root();
            let mut context = source_context();
            let msg = report(
                actor,
                vec![Relation {
                    recipient,
                    reason: Reason::Mention,
                    detail: Vec::new(),
                }],
            );
            assert!(
                module.execute(&mut context, &msg).await.is_err(),
                "identity reserves account zero; attribution must not record it"
            );
            module.commit_block().await.unwrap();
            assert_eq!(module.root(), before, "rejection leaves no staged writes");
        }
    });
}

#[test]
fn records_exceeding_the_backing_codec_are_rejected_before_staging() {
    block_on(async {
        let mut module = module();
        let before = module.root();
        let mut context = source_context();
        let msg = report(
            Actor::Account(1),
            vec![
                Relation {
                    recipient: 1,
                    reason: Reason::Authorship,
                    detail: Vec::new(),
                },
                Relation {
                    recipient: 2,
                    reason: Reason::Report,
                    // The qmdb decoder permits at most 1 << 20 value bytes.
                    // Record fields make this encoding exceed that existing bound.
                    detail: vec![0; 1 << 20],
                },
            ],
        );
        assert!(
            module.execute(&mut context, &msg).await.is_err(),
            "every stored record must be readable by the backing codec"
        );
        module.commit_block().await.unwrap();
        assert_eq!(module.root(), before, "no earlier change may remain staged");
    });
}
