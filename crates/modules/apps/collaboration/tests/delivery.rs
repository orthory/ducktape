//! delivery requests, receipts, expiry and the mailbox caps.

mod common;

use collaboration::{
    CollaborationMsg, CollaborationReply, DeliverRequest, Delivery, DeliveryEligibility,
    DeliveryState, EventBody, MAX_UNDELIVERED_PER_SENDER, MessageKind, ProtectedRead, Reference,
    TaskRef,
};
use common::*;
use futures::executor::block_on;
use sdk::{Module as _, Origin};

/// the setup every test here starts from: alice bound on `c1` under service
/// key 0x5e, bob bound under 0x5f, and alice's message `m1` posted in chat.
struct Bound {
    scene: Scene,
    /// the chat sequence of alice's message `m1`.
    seq: u64,
}

async fn bound() -> Bound {
    let mut scene = scene("c1");
    let mut ctx = scene.as_alice(1);
    let alice = scene.alice.clone();
    let bob = scene.bob.clone();
    ok(&mut scene.module, &mut ctx, bind("c1", &alice, key(0x5e), 0)).await;
    let mut ctx = scene.as_bob(1);
    ok(&mut scene.module, &mut ctx, bind("c1", &bob, key(0x5f), 0)).await;
    let seq = scene.alice_posts("c1", "m1");
    Bound { scene, seq }
}

async fn requested(bound: &mut Bound, expires_at: u64) {
    let bob = bound.scene.bob.clone();
    let mut ctx = bound.scene.as_alice(1);
    ok(
        &mut bound.scene.module,
        &mut ctx,
        CollaborationMsg::Deliver(deliver("c1", "m1", &bob, expires_at)),
    )
    .await;
}

async fn bobs_delivery(bound: &Bound, now: u64) -> Option<Delivery> {
    let ctx = bound.scene.as_bob(now);
    let CollaborationReply::Delivery(delivery) = read(
        &bound.scene.module,
        &ctx,
        &bound.scene.bob,
        None,
        ProtectedRead::Delivery {
            channel_id: "c1".into(),
            seq: bound.seq,
        },
    )
    .await
    else {
        panic!("a delivery read answers");
    };
    delivery
}

async fn eligibility(bound: &Bound, now: u64) -> DeliveryEligibility {
    let ctx = bound.scene.as_bob(now);
    let CollaborationReply::Eligibility(verdict) = read(
        &bound.scene.module,
        &ctx,
        &bound.scene.bob,
        None,
        ProtectedRead::DeliveryEligibility {
            channel_id: "c1".into(),
            seq: bound.seq,
        },
    )
    .await
    else {
        panic!("an eligibility read answers");
    };
    verdict
}

fn ack(seq: u64, recipient: &collaboration::Party, credential: u64, state: DeliveryState) -> CollaborationMsg {
    CollaborationMsg::Acknowledge {
        channel_id: "c1".into(),
        seq,
        recipient: recipient.clone(),
        binding_credential: credential,
        state,
        reason: None,
    }
}

#[test]
fn a_requested_delivery_stores_a_stored_record_and_charges_the_mailbox() {
    block_on(async {
        let mut bound = bound().await;
        requested(&mut bound, 100).await;
        let delivery = bobs_delivery(&bound, 2).await.expect("bob's record");
        assert_eq!(delivery.state, DeliveryState::Stored);
        assert_eq!(delivery.sender, bound.scene.alice);
        assert_eq!(delivery.message_id, "m1");
        assert_eq!(delivery.seq, bound.seq);
        assert_eq!(delivery.advanced_by, 0);

        let ctx = bound.scene.as_bob(2);
        let CollaborationReply::Mailbox(usage) = read(
            &bound.scene.module,
            &ctx,
            &bound.scene.bob,
            None,
            ProtectedRead::Mailbox,
        )
        .await
        else {
            panic!("bob's mailbox reads");
        };
        assert_eq!(usage.undelivered, 1);
        assert!(usage.queued_bytes > 0);
        assert!(matches!(
            eligibility(&bound, 2).await,
            DeliveryEligibility::Eligible {
                state: DeliveryState::Stored,
                expires_at: 100,
                asked_at: 2
            }
        ));
    });
}

#[test]
fn only_the_origin_that_posted_the_message_may_request_its_delivery() {
    block_on(async {
        let mut bound = bound().await;
        let alice = bound.scene.alice.clone();
        // bob asks to deliver alice's message to alice: not his words.
        let mut ctx = bound.scene.as_bob(1);
        let refusal = apply(
            &mut bound.scene.module,
            &mut ctx,
            CollaborationMsg::Deliver(deliver("c1", "m1", &alice, 100)),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("not posted by this origin"),
            "{refusal:?}"
        );
        // alice to alice: a message is not delivered to its sender.
        let mut ctx = bound.scene.as_alice(1);
        let refusal = apply(
            &mut bound.scene.module,
            &mut ctx,
            CollaborationMsg::Deliver(deliver("c1", "m1", &alice, 100)),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("not delivered to its sender"),
            "{refusal:?}"
        );
        // an unknown message, and a message on another channel.
        let bob = bound.scene.bob.clone();
        let refusal = apply(
            &mut bound.scene.module,
            &mut ctx,
            CollaborationMsg::Deliver(deliver("c1", "m9", &bob, 100)),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("no chat message"),
            "{refusal:?}"
        );
        bound.scene.chat.borrow_mut().channel("c2", &[alice.clone(), bob.clone()]);
        bound.scene.alice_posts("c2", "m2");
        let refusal = apply(
            &mut bound.scene.module,
            &mut ctx,
            CollaborationMsg::Deliver(deliver("c1", "m2", &bob, 100)),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("is on channel c2"),
            "{refusal:?}"
        );
    });
}

#[test]
fn the_recipient_must_read_the_channel() {
    block_on(async {
        let mut bound = bound().await;
        let stranger = party(9);
        let mut ctx = bound.scene.as_alice(1);
        let refusal = apply(
            &mut bound.scene.module,
            &mut ctx,
            CollaborationMsg::Deliver(deliver("c1", "m1", &stranger, 100)),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("recipient may not read"),
            "{refusal:?}"
        );
    });
}

#[test]
fn an_identical_repeat_answers_the_record_and_a_conflicting_one_is_refused() {
    block_on(async {
        let mut bound = bound().await;
        requested(&mut bound, 100).await;
        let root = bound.scene.module.root();
        let bob = bound.scene.bob.clone();

        let mut ctx = bound.scene.as_alice(2);
        ok(
            &mut bound.scene.module,
            &mut ctx,
            CollaborationMsg::Deliver(deliver("c1", "m1", &bob, 100)),
        )
        .await;
        assert_eq!(bound.scene.module.root(), root, "a replay stages nothing");
        let output = ctx.output().expect("the replay answers the record");
        let CollaborationReply::Delivery(Some(existing)) =
            collaboration::decode_reply(output).unwrap()
        else {
            panic!("the existing record is the answer");
        };
        assert_eq!(existing.seq, bound.seq);

        let mut ctx = bound.scene.as_alice(2);
        let refusal = apply(
            &mut bound.scene.module,
            &mut ctx,
            CollaborationMsg::Deliver(deliver("c1", "m1", &bob, 101)),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("different metadata"),
            "{refusal:?}"
        );
    });
}

#[test]
fn a_deadline_must_be_in_the_future_and_within_the_ceiling() {
    block_on(async {
        let mut bound = bound().await;
        let bob = bound.scene.bob.clone();
        let mut ctx = bound.scene.as_alice(10);
        for (expires_at, expected) in [
            (10, "not after"),
            (5, "not after"),
            (10 + MAX_TTL + 1, "more than"),
        ] {
            let refusal = apply(
                &mut bound.scene.module,
                &mut ctx,
                CollaborationMsg::Deliver(deliver("c1", "m1", &bob, expires_at)),
            )
            .await
            .unwrap_err();
            assert!(
                format!("{refusal:?}").contains(expected),
                "{expires_at}: {refusal:?}"
            );
        }
        ok(
            &mut bound.scene.module,
            &mut ctx,
            CollaborationMsg::Deliver(deliver("c1", "m1", &bob, 10 + MAX_TTL)),
        )
        .await;
    });
}

#[test]
fn only_immutable_commit_blob_and_duck_references_are_admitted() {
    block_on(async {
        let mut bound = bound().await;
        let bob = bound.scene.bob.clone();
        let mut ctx = bound.scene.as_alice(1);
        let with = |references: Vec<Reference>| {
            let mut request = deliver("c1", "m1", &bob, 100);
            request.references = references;
            CollaborationMsg::Deliver(request)
        };
        for (references, expected) in [
            (
                vec![Reference::Commit {
                    repo: "r".into(),
                    commit: "main".into(),
                }],
                "40 lowercase hex",
            ),
            (vec![Reference::Blob { hash: "AB".into() }], "64 lowercase hex"),
            (
                vec![Reference::Duck {
                    url: "file:///tmp/x".into(),
                }],
                "duck://",
            ),
            (
                vec![Reference::Duck {
                    url: "duck://a b".into(),
                }],
                "no whitespace",
            ),
            (
                vec![
                    Reference::Blob {
                        hash: "a".repeat(64)
                    };
                    17
                ],
                "over the 16 cap",
            ),
        ] {
            let refusal = apply(&mut bound.scene.module, &mut ctx, with(references))
                .await
                .unwrap_err();
            assert!(format!("{refusal:?}").contains(expected), "{refusal:?}");
        }
        ok(
            &mut bound.scene.module,
            &mut ctx,
            with(vec![
                Reference::Commit {
                    repo: "r".into(),
                    commit: "a".repeat(40),
                },
                Reference::Blob {
                    hash: "b".repeat(64),
                },
                Reference::Duck {
                    url: "duck://forge/r".into(),
                },
            ]),
        )
        .await;
        let delivery = bobs_delivery(&bound, 2).await.expect("bob's record");
        assert_eq!(delivery.references.len(), 3);
    });
}

#[test]
fn a_task_update_must_name_its_task_and_a_result_its_cause() {
    block_on(async {
        let mut bound = bound().await;
        let bob = bound.scene.bob.clone();
        let mut ctx = bound.scene.as_alice(1);
        for kind in [MessageKind::TaskUpdate, MessageKind::Result] {
            let mut request = deliver("c1", "m1", &bob, 100);
            request.kind = kind;
            let refusal = apply(
                &mut bound.scene.module,
                &mut ctx,
                CollaborationMsg::Deliver(request),
            )
            .await
            .unwrap_err();
            assert!(
                format!("{refusal:?}").contains("must name"),
                "{kind:?}: {refusal:?}"
            );
        }
        // a result that answers a thread needs no task.
        bound
            .scene
            .chat
            .borrow_mut()
            .post_in_thread("c1", "m-reply", Origin::External(key(1)), Some(bound.seq));
        let mut request = deliver("c1", "m-reply", &bob, 100);
        request.kind = MessageKind::Result;
        ok(
            &mut bound.scene.module,
            &mut ctx,
            CollaborationMsg::Deliver(request),
        )
        .await;
    });
}

#[test]
fn a_delivery_naming_a_superseded_attempt_is_refused_as_stale() {
    block_on(async {
        let mut bound = bound().await;
        let bob = bound.scene.bob.clone();
        let task = Some(TaskRef {
            id: "job-1".into(),
            expected_attempt: 3,
        });
        let with_task = || {
            let mut request = deliver("c1", "m1", &bob, 100);
            request.kind = MessageKind::TaskUpdate;
            request.task = task.clone();
            CollaborationMsg::Deliver(request)
        };
        let mut ctx = with_job(
            &bound.scene.chat,
            1,
            Origin::External(key(1)),
            Some(job("job-1", 4)),
        );
        let refusal = apply(&mut bound.scene.module, &mut ctx, with_task())
            .await
            .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("stale task target"),
            "{refusal:?}"
        );
        let mut ctx = with_job(
            &bound.scene.chat,
            1,
            Origin::External(key(1)),
            Some(job("job-1", 3)),
        );
        ok(&mut bound.scene.module, &mut ctx, with_task()).await;

        // the attempt is rechecked at acknowledgement: a returning previous
        // attempt cannot publish as the current one.
        let mut ctx = with_job(
            &bound.scene.chat,
            2,
            Origin::External(key(0x5f)),
            Some(job("job-1", 4)),
        );
        let refusal = apply(
            &mut bound.scene.module,
            &mut ctx,
            ack(bound.seq, &bob, 1, DeliveryState::Queued),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("stale attempt acknowledgement"),
            "{refusal:?}"
        );
    });
}

#[test]
fn delivery_advances_only_through_the_diagram_and_only_from_the_live_binding() {
    block_on(async {
        let mut bound = bound().await;
        requested(&mut bound, 100).await;
        let bob = bound.scene.bob.clone();
        let seq = bound.seq;

        // Stored -> AdapterAccepted skips Queued: refused.
        let mut ctx = bound.scene.as_key(2, 0x5f);
        let refusal = apply(
            &mut bound.scene.module,
            &mut ctx,
            ack(seq, &bob, 1, DeliveryState::AdapterAccepted),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("cannot move from stored to adapter_accepted"),
            "{refusal:?}"
        );
        // a stale credential is refused.
        let refusal = apply(
            &mut bound.scene.module,
            &mut ctx,
            ack(seq, &bob, 9, DeliveryState::Queued),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("is stale"),
            "{refusal:?}"
        );
        // a reason must be a token.
        let refusal = apply(
            &mut bound.scene.module,
            &mut ctx,
            CollaborationMsg::Acknowledge {
                channel_id: "c1".into(),
                seq,
                recipient: bob.clone(),
                binding_credential: 1,
                state: DeliveryState::Queued,
                reason: Some("Not A Token".into()),
            },
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("snake_case token"),
            "{refusal:?}"
        );

        ok(
            &mut bound.scene.module,
            &mut ctx,
            ack(seq, &bob, 1, DeliveryState::Queued),
        )
        .await;
        assert!(matches!(
            eligibility(&bound, 3).await,
            DeliveryEligibility::Eligible {
                state: DeliveryState::Queued,
                ..
            }
        ));
        ok(
            &mut bound.scene.module,
            &mut ctx,
            ack(seq, &bob, 1, DeliveryState::AdapterAccepted),
        )
        .await;
        let delivery = bobs_delivery(&bound, 3).await.expect("bob's record");
        assert_eq!(delivery.state, DeliveryState::AdapterAccepted);
        assert_eq!(delivery.advanced_by, 1);
        assert!(matches!(
            eligibility(&bound, 3).await,
            DeliveryEligibility::Settled {
                state: DeliveryState::AdapterAccepted
            }
        ));

        // terminal frees the queue slot and never moves again.
        let ctx_b = bound.scene.as_bob(3);
        let CollaborationReply::Mailbox(usage) = read(
            &bound.scene.module,
            &ctx_b,
            &bob,
            None,
            ProtectedRead::Mailbox,
        )
        .await
        else {
            panic!("mailbox reads");
        };
        assert_eq!(usage.undelivered, 0);
        assert_eq!(usage.queued_bytes, 0);
        let refusal = apply(
            &mut bound.scene.module,
            &mut ctx,
            ack(seq, &bob, 1, DeliveryState::Refused),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("cannot move from adapter_accepted"),
            "{refusal:?}"
        );
    });
}

#[test]
fn a_removed_recipient_cannot_acknowledge_late() {
    block_on(async {
        let mut bound = bound().await;
        requested(&mut bound, 100).await;
        let bob = bound.scene.bob.clone();
        bound.scene.chat.borrow_mut().channel("c1", &[bound.scene.alice.clone()]);
        let mut ctx = bound.scene.as_key(2, 0x5f);
        let refusal = apply(
            &mut bound.scene.module,
            &mut ctx,
            ack(bound.seq, &bob, 1, DeliveryState::Queued),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("may no longer read"),
            "{refusal:?}"
        );
    });
}

#[test]
fn expiry_is_permissionless_agreed_time_and_never_reported_by_a_service() {
    block_on(async {
        let mut bound = bound().await;
        requested(&mut bound, 100).await;
        let bob = bound.scene.bob.clone();
        let seq = bound.seq;
        let expire = CollaborationMsg::ExpireMessage {
            channel_id: "c1".into(),
            seq,
            recipient: bob.clone(),
        };

        // a service may not declare a live message expired.
        let mut ctx = bound.scene.as_key(2, 0x5f);
        let refusal = apply(
            &mut bound.scene.module,
            &mut ctx,
            ack(seq, &bob, 1, DeliveryState::Expired),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("expiry is not reported"),
            "{refusal:?}"
        );
        // too early for anyone.
        let mut ctx = bound.scene.as_key(99, 7);
        let refusal = apply(&mut bound.scene.module, &mut ctx, expire.clone())
            .await
            .unwrap_err();
        assert!(format!("{refusal:?}").contains("not yet"), "{refusal:?}");

        // at the deadline eligibility ends, and a stranger may sweep it.
        assert!(matches!(
            eligibility(&bound, 100).await,
            DeliveryEligibility::Expired {
                expires_at: 100,
                asked_at: 100
            }
        ));
        let mut ctx = bound.scene.as_key(100, 7);
        ok(&mut bound.scene.module, &mut ctx, expire.clone()).await;
        let delivery = bobs_delivery(&bound, 101).await.expect("bob's record");
        assert_eq!(delivery.state, DeliveryState::Expired);
        assert_eq!(delivery.reason.as_deref(), Some("deadline_passed"));
        let ctx_b = bound.scene.as_bob(101);
        let CollaborationReply::Mailbox(usage) = read(
            &bound.scene.module,
            &ctx_b,
            &bob,
            None,
            ProtectedRead::Mailbox,
        )
        .await
        else {
            panic!("mailbox reads");
        };
        assert_eq!(usage.undelivered, 0, "expiry frees the queue slot");
        // settled: a second sweep is refused.
        let refusal = apply(&mut bound.scene.module, &mut ctx, expire)
            .await
            .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("already settled"),
            "{refusal:?}"
        );
    });
}

#[test]
fn a_late_authentic_acceptance_beats_the_sweeper_and_is_not_relabelled() {
    block_on(async {
        let mut bound = bound().await;
        requested(&mut bound, 100).await;
        let bob = bound.scene.bob.clone();
        let seq = bound.seq;
        let mut ctx = bound.scene.as_key(2, 0x5f);
        ok(
            &mut bound.scene.module,
            &mut ctx,
            ack(seq, &bob, 1, DeliveryState::Queued),
        )
        .await;
        // past the deadline, the provider really did take it: the record
        // logs the truth, and eligibility (forward-looking) says expired.
        let mut ctx = bound.scene.as_key(150, 0x5f);
        ok(
            &mut bound.scene.module,
            &mut ctx,
            ack(seq, &bob, 1, DeliveryState::AdapterAccepted),
        )
        .await;
        let delivery = bobs_delivery(&bound, 151).await.expect("bob's record");
        assert_eq!(delivery.state, DeliveryState::AdapterAccepted);
        assert!(matches!(
            eligibility(&bound, 151).await,
            DeliveryEligibility::Settled {
                state: DeliveryState::AdapterAccepted
            }
        ));
    });
}

#[test]
fn one_sender_cannot_fill_another_mailbox_past_its_quota() {
    block_on(async {
        let mut bound = bound().await;
        let bob = bound.scene.bob.clone();
        for n in 0..MAX_UNDELIVERED_PER_SENDER {
            let id = format!("q{n}");
            bound.scene.alice_posts("c1", &id);
            let mut ctx = bound.scene.as_alice(1);
            ok(
                &mut bound.scene.module,
                &mut ctx,
                CollaborationMsg::Deliver(deliver("c1", &id, &bob, 100)),
            )
            .await;
        }
        let mut ctx = bound.scene.as_alice(1);
        let refusal = apply(
            &mut bound.scene.module,
            &mut ctx,
            CollaborationMsg::Deliver(deliver("c1", "m1", &bob, 100)),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains(collaboration::QUEUE_FULL),
            "{refusal:?}"
        );
    });
}

#[test]
fn the_event_stream_carries_requests_and_transitions_with_their_records() {
    block_on(async {
        let mut bound = bound().await;
        requested(&mut bound, 100).await;
        let bob = bound.scene.bob.clone();
        let mut ctx = bound.scene.as_key(2, 0x5f);
        ok(
            &mut bound.scene.module,
            &mut ctx,
            ack(bound.seq, &bob, 1, DeliveryState::Queued),
        )
        .await;
        let ctx = bound.scene.as_bob(3);
        let CollaborationReply::Events(page) = read(
            &bound.scene.module,
            &ctx,
            &bob,
            None,
            ProtectedRead::Events {
                channel_id: "c1".into(),
                from_seq: 3,
                limit: 16,
            },
        )
        .await
        else {
            panic!("events read");
        };
        // 1 and 2 were the two bindings; 3 is the request, 4 the transition.
        let bodies: Vec<_> = page.events.iter().map(|e| e.body.clone()).collect();
        assert_eq!(
            bodies,
            vec![
                EventBody::DeliveryRequested {
                    message_seq: bound.seq,
                    sender: bound.scene.alice.clone(),
                    recipient: bob.clone(),
                    kind: MessageKind::Notice
                },
                EventBody::DeliveryAdvanced {
                    message_seq: bound.seq,
                    recipient: bob.clone(),
                    state: DeliveryState::Queued,
                    reason: None
                },
            ]
        );
        assert_eq!(page.deliveries.len(), 1);
        assert_eq!(page.deliveries[0].state, DeliveryState::Queued);
        assert_eq!(page.next_seq, 5);
        // the request carries the metadata verbatim.
        let _: &DeliverRequest = &deliver("c1", "m1", &bob, 100);
    });
}
