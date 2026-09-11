//! Source histories and delivery receipts are identical through native and
//! compiled guest execution, including source-operation rollback.

use attribution::{
    Actor, AttributionModule, AttributionMsg, AttributionQuery, AttributionReply,
    AttributionUpdate, ChangeKind, ObjectRef, Reason, Relation, Source, Transfer,
};
use futures::executor::block_on;
use sdk::{Ack, Cause, DeliveryOutcome, Env, Module, Msg, Origin};
use sdk_testkit::{MemStore, TestCtx};
use wasm_host::WasmModule;

const COMPONENT: &[u8] = include_bytes!("fixtures/attribution.component.wasm");

struct Pair {
    native: AttributionModule,
    wasm: WasmModule,
}

fn context(height: u64, origin: Origin) -> TestCtx {
    TestCtx::with_env(Env {
        height,
        consensus_time: height,
        me: "attribution".into(),
        origin,
        cause: Cause::Direct,
    })
}

impl Pair {
    fn new() -> Self {
        Self {
            native: AttributionModule::new("attribution", Box::new(MemStore::new()))
                .with_subscribers(["agent", "inbox"]),
            wasm: WasmModule::with_store("attribution", COMPONENT, Box::new(MemStore::new()))
                .unwrap(),
        }
    }

    async fn apply(
        &mut self,
        height: u64,
        origin: Origin,
        operation: AttributionMsg,
    ) -> Result<(), sdk::Error> {
        let msg = Msg {
            target: "attribution".into(),
            payload: attribution::encode_msg(&operation),
        };
        let mut native = context(height, origin.clone());
        let mut wasm = context(height, origin);
        let a = self.native.execute(&mut native, &msg).await;
        let b = self.wasm.execute(&mut wasm, &msg).await;
        assert_eq!(a, b);
        assert_eq!(native.assigned(), wasm.assigned());
        assert_eq!(native.output(), wasm.output());
        assert_eq!(native.msgs(), wasm.msgs());
        assert_eq!(native.events(), wasm.events());
        a
    }

    async fn commit(&mut self) {
        self.native.commit_block().await.unwrap();
        self.wasm.commit_block().await.unwrap();
        assert_eq!(self.native.root(), self.wasm.root());
        assert_eq!(
            self.native.pending_items().await.unwrap(),
            self.wasm.pending_items().await.unwrap()
        );
    }

    async fn query(&self, query: AttributionQuery) -> AttributionReply {
        let request = attribution::encode_query(&query);
        let native = self.native.query(&request).await.unwrap();
        let wasm = self.wasm.query(&request).await.unwrap();
        assert_eq!(native, wasm);
        attribution::decode_reply(&native).unwrap()
    }
}

fn update(object: &str, revision: u64, recipient: u64) -> AttributionUpdate {
    AttributionUpdate {
        object: ObjectRef {
            kind: "message".into(),
            object: object.into(),
        },
        revision,
        actor: Actor::Account(1),
        relations: vec![Relation {
            recipient,
            reason: Reason::Ownership,
            detail: Vec::new(),
        }],
        transfers: Vec::new(),
    }
}

fn batch(updates: Vec<AttributionUpdate>) -> AttributionMsg {
    AttributionMsg::AttributeBatch { updates }
}

#[test]
fn ownership_transfer_withdrawal_and_delivery_receipts_match() {
    block_on(async {
        let mut pair = Pair::new();
        let source = Origin::Module("pages".into());
        pair.apply(1, source.clone(), batch(vec![update("one", 1, 1)]))
            .await
            .unwrap();
        pair.commit().await;
        let mut transfer = update("one", 2, 2);
        transfer.transfers.push(Transfer {
            reason: Reason::Ownership,
            from: 1,
            to: 2,
        });
        pair.apply(2, source.clone(), batch(vec![transfer]))
            .await
            .unwrap();
        pair.commit().await;
        let mut withdrawn = update("one", 3, 2);
        withdrawn.relations.clear();
        pair.apply(3, source, batch(vec![withdrawn])).await.unwrap();
        pair.commit().await;
        let AttributionReply::Changes(changes) = pair
            .query(AttributionQuery::Changes {
                after: 0,
                limit: 10,
            })
            .await
        else {
            panic!("history")
        };
        assert_eq!(changes.len(), 4);
        assert_eq!(changes[0].change.kind, ChangeKind::Added);
        assert_eq!(changes[1].change.kind, ChangeKind::TransferredOut { to: 2 });
        assert_eq!(
            changes[2].change.kind,
            ChangeKind::TransferredIn { from: 1 }
        );
        assert_eq!(changes[3].change.kind, ChangeKind::Withdrawn);
        let queued = pair.native.pending_items().await.unwrap();
        assert_eq!(queued.len(), 8);
        for item in queued {
            let outcome = match item.item % 2 {
                0 => DeliveryOutcome::Applied,
                _ => DeliveryOutcome::Failed {
                    reason: "subscriber refused".into(),
                },
            };
            let ack = Ack {
                item: item.item,
                target: item.target,
                outcome,
            };
            pair.native
                .acknowledge(&mut context(4, Origin::System), &ack)
                .await
                .unwrap();
            pair.wasm
                .acknowledge(&mut context(4, Origin::System), &ack)
                .await
                .unwrap();
        }
        pair.commit().await;
        assert!(pair.native.pending_items().await.unwrap().is_empty());
        let AttributionReply::Deliveries(receipts) = pair
            .query(AttributionQuery::DeliveriesOf {
                subscriber: "agent".into(),
                after: 0,
                limit: 10,
            })
            .await
        else {
            panic!("receipts")
        };
        assert_eq!(receipts.len(), 4);
    });
}

#[test]
fn a_later_bad_object_rolls_back_its_batch_and_preserves_prior_operation() {
    block_on(async {
        let mut pair = Pair::new();
        let source = Origin::Module("pages".into());
        pair.apply(1, source.clone(), batch(vec![update("prior", 1, 1)]))
            .await
            .unwrap();
        assert!(
            pair.apply(
                1,
                source.clone(),
                batch(vec![update("discard", 1, 2), update("invalid", 1, 0)])
            )
            .await
            .is_err()
        );
        assert!(
            pair.apply(1, Origin::Program(1), batch(Vec::new()))
                .await
                .is_err()
        );
        pair.commit().await;
        let AttributionReply::Changes(changes) = pair
            .query(AttributionQuery::Changes {
                after: 0,
                limit: 10,
            })
            .await
        else {
            panic!("history")
        };
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].change.source.object, "prior");
        assert_eq!(
            pair.query(AttributionQuery::Relations {
                source: Source {
                    module: "pages".into(),
                    kind: "message".into(),
                    object: "discard".into()
                }
            })
            .await,
            AttributionReply::Relations(None)
        );
        assert!(
            pair.apply(2, source, batch(vec![update("prior", 1, 1)]))
                .await
                .is_err()
        );
        pair.commit().await;
    });
}

#[test]
fn many_distinct_sources_and_recipients_can_be_withdrawn_in_bounded_batches() {
    block_on(async {
        let mut pair = Pair::new();
        let source = Origin::Module("pages".into());
        let objects: Vec<_> = (0..1100)
            .map(|index| {
                let mut report = update(&format!("block-{index}"), 1, index * 2 + 1);
                report.relations.push(Relation {
                    recipient: index * 2 + 2,
                    reason: Reason::Mention,
                    detail: Vec::new(),
                });
                report
            })
            .collect();
        // Each source contributes its own object and two recipient reads.
        // The host's source unit may emit several bounded publication batches.
        for group in objects.chunks(42) {
            pair.apply(1, source.clone(), batch(group.to_vec()))
                .await
                .unwrap();
        }
        pair.commit().await;
        for group in objects.chunks(42) {
            let removed = group
                .iter()
                .cloned()
                .map(|mut report| {
                    report.revision = 2;
                    report.relations.clear();
                    report
                })
                .collect();
            pair.apply(2, source.clone(), batch(removed)).await.unwrap();
        }
        pair.commit().await;
        for object in ["block-0", "block-1099"] {
            let AttributionReply::Changes(changes) = pair
                .query(AttributionQuery::ChangesOf {
                    source: Source {
                        module: "pages".into(),
                        kind: "message".into(),
                        object: object.into(),
                    },
                    after: 0,
                    limit: 10,
                })
                .await
            else {
                panic!("object history")
            };
            assert_eq!(changes.len(), 4);
            assert!(
                changes[2..]
                    .iter()
                    .all(|entry| entry.change.kind == ChangeKind::Withdrawn)
            );
        }
    });
}
