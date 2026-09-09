//! admission, dedup, caps, deadlines, the delivery state machine, attempt
//! fencing and retention — the message plane end to end.

mod common;

use collaboration::{
    CollaborationMsg, CollaborationReply, DeliveryState, EventBody, EventPage, MessageKind,
    ProtectedRead, Reference, Role, SendState, TaskRef, MAX_BODY_BYTES, MAX_REFERENCES,
    MAX_SEQUENCE, MAX_UNDELIVERED_PER_SENDER, QUEUE_FULL, RECEIPT_PRUNED,
};
use common::*;
use futures::executor::block_on;
use sdk::Origin;

/// alice sends bob one notice; returns the module, alice's owner key and the
/// sequence the admission took.
async fn one_message() -> (collaboration::Collaboration, Vec<u8>, Vec<u8>, u64) {
    let scene = scene("c1").await;
    let mut module = scene.module;
    let mut alice = at(5, Origin::External(scene.owner_a.clone()));
    ok(
        &mut module,
        &mut alice,
        CollaborationMsg::Send(note("c1", "alice", "bob", 1, 1, 100)),
    )
    .await;
    let CollaborationReply::SendState(SendState::Admitted { seq, .. }) = read(
        &module,
        &alice,
        "alice",
        None,
        ProtectedRead::SendState {
            generation: 1,
            sequence: 1,
        },
    )
    .await
    else {
        panic!("the send was admitted");
    };
    (module, scene.owner_a, scene.owner_b, seq)
}

#[test]
fn an_admitted_message_stores_a_stored_receipt_and_charges_the_mailbox() {
    block_on(async {
        let (module, _alice_key, bob_key, seq) = one_message().await;
        let bob = at(6, Origin::External(bob_key));

        let CollaborationReply::Receipt(Some(receipt)) = read(
            &module,
            &bob,
            "bob",
            None,
            ProtectedRead::Receipt {
                conversation_id: "c1".into(),
                seq,
            },
        )
        .await
        else {
            panic!("the recipient reads its receipt");
        };
        assert_eq!(receipt.state, DeliveryState::Stored);
        assert_eq!(receipt.advanced_by, 0, "nothing has advanced it yet");

        let CollaborationReply::Mailbox(usage) =
            read(&module, &bob, "bob", None, ProtectedRead::Mailbox).await
        else {
            panic!("mailbox")
        };
        assert_eq!(usage.undelivered, 1);
        assert!(usage.queued_bytes > 0);
    });
}

#[test]
fn a_sender_cannot_name_a_credential_it_does_not_hold() {
    block_on(async {
        let scene = scene("c1").await;
        let mut module = scene.module;
        let mut alice = at(5, Origin::External(scene.owner_a.clone()));
        // alice's owner credential is 1; naming 2 borrows a binding's number.
        let refusal = apply(
            &mut module,
            &mut alice,
            CollaborationMsg::Send(note("c1", "alice", "bob", 2, 1, 100)),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("authenticates as 1"),
            "{refusal:?}"
        );
    });
}

#[test]
fn nobody_can_send_as_a_participant_they_do_not_control() {
    block_on(async {
        let scene = scene("c1").await;
        let mut module = scene.module;
        // bob's key naming alice as the sender.
        let mut bob = at(5, Origin::External(scene.owner_b.clone()));
        let refusal = apply(
            &mut module,
            &mut bob,
            CollaborationMsg::Send(note("c1", "alice", "bob", 1, 1, 100)),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("not authorized to send as"),
            "the payload's sender field is a claim, not authority: {refusal:?}"
        );
    });
}

#[test]
fn an_observer_may_not_send() {
    block_on(async {
        let scene = scene("c1").await;
        let mut module = scene.module;
        let mut owner = at(2, Origin::External(scene.owner_a.clone()));
        ok(
            &mut module,
            &mut owner,
            seat("c1", "alice", Some(Role::Observer)),
        )
        .await;

        let mut alice = at(5, Origin::External(scene.owner_a.clone()));
        let refusal = apply(
            &mut module,
            &mut alice,
            CollaborationMsg::Send(note("c1", "alice", "bob", 1, 1, 100)),
        )
        .await
        .unwrap_err();
        assert!(format!("{refusal:?}").contains("may not send"), "{refusal:?}");
    });
}

#[test]
fn an_identical_retry_replays_the_same_admission_and_a_conflicting_one_is_refused() {
    block_on(async {
        let scene = scene("c1").await;
        let mut module = scene.module;
        let mut alice = at(5, Origin::External(scene.owner_a.clone()));
        let request = note("c1", "alice", "bob", 1, 1, 100);

        ok(
            &mut module,
            &mut alice,
            CollaborationMsg::Send(request.clone()),
        )
        .await;
        let CollaborationReply::Mailbox(after_first) = read(
            &module,
            &at(6, Origin::External(scene.owner_b.clone())),
            "bob",
            None,
            ProtectedRead::Mailbox,
        )
        .await
        else {
            panic!("mailbox")
        };

        // identical bytes: the same receipt, no second admission.
        ok(
            &mut module,
            &mut alice,
            CollaborationMsg::Send(request.clone()),
        )
        .await;
        let CollaborationReply::Mailbox(after_retry) = read(
            &module,
            &at(6, Origin::External(scene.owner_b.clone())),
            "bob",
            None,
            ProtectedRead::Mailbox,
        )
        .await
        else {
            panic!("mailbox")
        };
        assert_eq!(
            after_first, after_retry,
            "a retry of identical bytes admits nothing new"
        );

        // different bytes under the same id: refused, never a second message.
        let mut conflicting = request;
        conflicting.body = "different".into();
        let refusal = apply(
            &mut module,
            &mut alice,
            CollaborationMsg::Send(conflicting),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("different bytes"),
            "{refusal:?}"
        );
    });
}

#[test]
fn a_deadline_must_be_in_the_future_and_within_the_ceiling() {
    block_on(async {
        let scene = scene("c1").await;
        let mut module = scene.module;
        let mut alice = at(50, Origin::External(scene.owner_a.clone()));

        let refusal = apply(
            &mut module,
            &mut alice,
            CollaborationMsg::Send(note("c1", "alice", "bob", 1, 1, 50)),
        )
        .await
        .unwrap_err();
        assert!(format!("{refusal:?}").contains("not after"), "{refusal:?}");

        let refusal = apply(
            &mut module,
            &mut alice,
            CollaborationMsg::Send(note("c1", "alice", "bob", 1, 2, 50 + MAX_TTL + 1)),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("more than"),
            "the composer's ceiling bounds the request: {refusal:?}"
        );
    });
}

#[test]
fn oversized_bodies_and_references_are_refused_not_truncated() {
    block_on(async {
        let scene = scene("c1").await;
        let mut module = scene.module;
        let mut alice = at(5, Origin::External(scene.owner_a.clone()));

        let mut big = note("c1", "alice", "bob", 1, 1, 100);
        big.body = "x".repeat(MAX_BODY_BYTES + 1);
        let refusal = apply(&mut module, &mut alice, CollaborationMsg::Send(big))
            .await
            .unwrap_err();
        assert!(format!("{refusal:?}").contains("over the"), "{refusal:?}");

        let mut many = note("c1", "alice", "bob", 1, 2, 100);
        many.references = (0..=MAX_REFERENCES)
            .map(|n| Reference::Commit {
                repo: "ducktape".into(),
                commit: format!("{n:040x}"),
            })
            .collect();
        let refusal = apply(&mut module, &mut alice, CollaborationMsg::Send(many))
            .await
            .unwrap_err();
        assert!(format!("{refusal:?}").contains("references"), "{refusal:?}");
    });
}

#[test]
fn only_immutable_commit_blob_and_duck_references_are_admitted() {
    block_on(async {
        let scene = scene("c1").await;
        let mut module = scene.module;
        let mut alice = at(5, Origin::External(scene.owner_a.clone()));

        let good = vec![
            Reference::Commit {
                repo: "ducktape".into(),
                commit: "a".repeat(40),
            },
            Reference::Blob {
                hash: "b".repeat(64),
            },
            Reference::Duck {
                url: "duck://forge.duck/pr/7".into(),
            },
        ];
        let mut request = note("c1", "alice", "bob", 1, 1, 100);
        request.references = good;
        ok(&mut module, &mut alice, CollaborationMsg::Send(request)).await;

        // a mutable branch alias is not a commit, and a local path cannot even
        // be spelled: there is no arm for a file scheme.
        let bad = vec![
            Reference::Commit {
                repo: "ducktape".into(),
                commit: "main".into(),
            },
            Reference::Blob {
                hash: "not-a-digest".into(),
            },
            Reference::Duck {
                url: "file:///tmp/private".into(),
            },
            Reference::Duck {
                url: "duck://forge.duck/pr 7".into(),
            },
        ];
        for (n, reference) in bad.into_iter().enumerate() {
            let mut request = note("c1", "alice", "bob", 1, 10 + n as u64, 100);
            request.references = vec![reference.clone()];
            let refusal = apply(&mut module, &mut alice, CollaborationMsg::Send(request))
                .await
                .unwrap_err();
            assert!(
                format!("{refusal:?}").contains("reference"),
                "{reference:?} should be refused: {refusal:?}"
            );
        }
    });
}

/// Root's roster case: a participant removed from the roster while a message
/// is queued cannot keep writing receipts with its old credential.
#[test]
fn a_removed_participant_cannot_acknowledge_late() {
    block_on(async {
        let scene = scene("c1").await;
        let mut module = scene.module;
        let mut owner_a = at(2, Origin::External(scene.owner_a.clone()));
        let mut owner_b = at(2, Origin::External(scene.owner_b.clone()));
        ok(&mut module, &mut owner_b, bind("c1", "bob", key(20), 0)).await;

        let mut alice = at(5, Origin::External(scene.owner_a.clone()));
        ok(
            &mut module,
            &mut alice,
            CollaborationMsg::Send(note("c1", "alice", "bob", 1, 1, 100)),
        )
        .await;

        let seq = admitted_seq(&module, &alice, "alice", 1, 1).await;
        let credential = credential_of(&module, &owner_b, "bob", "c1").await;

        // the conversation owner revokes bob's seat while the message sits
        // queued.
        ok(&mut module, &mut owner_a, seat("c1", "bob", None)).await;

        let mut service = at(6, Origin::External(key(20)));
        let refusal = apply(
            &mut module,
            &mut service,
            CollaborationMsg::Acknowledge {
                conversation_id: "c1".into(),
                seq,
                binding_credential: credential,
                state: DeliveryState::Queued,
                reason: None,
            },
        )
        .await
        .unwrap_err();
        let refusal = format!("{refusal:?}");
        assert!(
            refusal.contains("no longer on conversation") || refusal.contains("is stale"),
            "a removed participant's old credential writes nothing: {refusal}"
        );
    });
}

/// Expiry must not manufacture a false `Expired` over evidence that the
/// provider really did accept the input. A late but authentic acceptance wins;
/// once a record is settled, expiry refuses it.
#[test]
fn a_late_authentic_acceptance_beats_the_sweeper_and_is_not_relabelled() {
    block_on(async {
        let scene = scene("c1").await;
        let mut module = scene.module;
        let mut owner_b = at(2, Origin::External(scene.owner_b.clone()));
        ok(&mut module, &mut owner_b, bind("c1", "bob", key(20), 0)).await;
        let mut alice = at(5, Origin::External(scene.owner_a.clone()));
        ok(
            &mut module,
            &mut alice,
            CollaborationMsg::Send(note("c1", "alice", "bob", 1, 1, 100)),
        )
        .await;

        let seq = admitted_seq(&module, &alice, "alice", 1, 1).await;
        let credential = credential_of(&module, &owner_b, "bob", "c1").await;

        // past the deadline, but nobody has swept it yet: the service reports
        // what actually happened.
        let mut service = at(150, Origin::External(key(20)));
        ok(
            &mut module,
            &mut service,
            CollaborationMsg::Acknowledge {
                conversation_id: "c1".into(),
                seq,
                binding_credential: credential,
                state: DeliveryState::Queued,
                reason: None,
            },
        )
        .await;
        ok(
            &mut module,
            &mut service,
            CollaborationMsg::Acknowledge {
                conversation_id: "c1".into(),
                seq,
                binding_credential: credential,
                state: DeliveryState::AdapterAccepted,
                reason: None,
            },
        )
        .await;

        // the sweeper cannot now overwrite that evidence with Expired.
        let mut sweeper = at(151, Origin::External(key(77)));
        let refusal = apply(
            &mut module,
            &mut sweeper,
            CollaborationMsg::ExpireMessage {
                conversation_id: "c1".into(),
                seq,
            },
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("already settled"),
            "expiry must not fabricate a state over real acceptance: {refusal:?}"
        );

        let CollaborationReply::Receipt(Some(receipt)) = read(
            &module,
            &owner_b,
            "bob",
            None,
            ProtectedRead::Receipt {
                conversation_id: "c1".into(),
                seq,
            },
        )
        .await
        else {
            panic!("receipt")
        };
        assert_eq!(receipt.state, DeliveryState::AdapterAccepted);
    });
}

#[test]
fn a_task_update_must_name_its_task_and_a_result_its_cause() {
    block_on(async {
        let scene = scene("c1").await;
        let mut module = scene.module;
        let mut alice = at(5, Origin::External(scene.owner_a.clone()));

        let mut update = note("c1", "alice", "bob", 1, 1, 100);
        update.kind = MessageKind::TaskUpdate;
        let refusal = apply(&mut module, &mut alice, CollaborationMsg::Send(update))
            .await
            .unwrap_err();
        assert!(format!("{refusal:?}").contains("must name its task"), "{refusal:?}");

        let mut result = note("c1", "alice", "bob", 1, 2, 100);
        result.kind = MessageKind::Result;
        let refusal = apply(&mut module, &mut alice, CollaborationMsg::Send(result))
            .await
            .unwrap_err();
        assert!(format!("{refusal:?}").contains("must name its task"), "{refusal:?}");
    });
}

#[test]
fn a_message_naming_a_superseded_attempt_is_refused_as_stale() {
    block_on(async {
        let scene = scene("c1").await;
        let mut module = scene.module;

        let mut request = note("c1", "alice", "bob", 1, 1, 100);
        request.kind = MessageKind::TaskUpdate;
        request.task = Some(TaskRef {
            id: "j1".into(),
            expected_attempt: 1,
        });

        // the job has moved to attempt 2 — a reassignment while this sender
        // was away.
        let mut alice = with_job(5, Origin::External(scene.owner_a.clone()), Some(job("j1", 2)));
        let refusal = apply(
            &mut module,
            &mut alice,
            CollaborationMsg::Send(request.clone()),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("stale task target"),
            "the instruction must not be delivered into the new attempt: {refusal:?}"
        );

        // naming the current attempt lands.
        let mut current =
            with_job(5, Origin::External(scene.owner_a.clone()), Some(job("j1", 1)));
        ok(&mut module, &mut current, CollaborationMsg::Send(request)).await;
    });
}

#[test]
fn delivery_advances_only_through_the_diagram_and_only_from_the_live_binding() {
    block_on(async {
        let scene = scene("c1").await;
        let mut module = scene.module;
        let mut owner_b = at(2, Origin::External(scene.owner_b.clone()));
        ok(&mut module, &mut owner_b, bind("c1", "bob", key(20), 0)).await;

        let mut alice = at(5, Origin::External(scene.owner_a.clone()));
        ok(
            &mut module,
            &mut alice,
            CollaborationMsg::Send(note("c1", "alice", "bob", 1, 1, 100)),
        )
        .await;
        let seq = admitted_seq(&module, &alice, "alice", 1, 1).await;
        let credential = credential_of(&module, &owner_b, "bob", "c1").await;
        let stale = credential - 1;
        let mut service = at(6, Origin::External(key(20)));

        // Stored -> AdapterAccepted skips Queued: not in the diagram.
        let refusal = apply(
            &mut module,
            &mut service,
            CollaborationMsg::Acknowledge {
                conversation_id: "c1".into(),
                seq,
                binding_credential: credential,
                state: DeliveryState::AdapterAccepted,
                reason: None,
            },
        )
        .await
        .unwrap_err();
        assert!(format!("{refusal:?}").contains("cannot move from"), "{refusal:?}");

        ok(
            &mut module,
            &mut service,
            CollaborationMsg::Acknowledge {
                conversation_id: "c1".into(),
                seq,
                binding_credential: credential,
                state: DeliveryState::Queued,
                reason: None,
            },
        )
        .await;
        ok(
            &mut module,
            &mut service,
            CollaborationMsg::Acknowledge {
                conversation_id: "c1".into(),
                seq,
                binding_credential: credential,
                state: DeliveryState::Held,
                reason: Some("provider_approval".into()),
            },
        )
        .await;

        // a stale credential cannot overwrite the current state.
        let refusal = apply(
            &mut module,
            &mut service,
            CollaborationMsg::Acknowledge {
                conversation_id: "c1".into(),
                seq,
                binding_credential: stale,
                state: DeliveryState::AdapterAccepted,
                reason: None,
            },
        )
        .await
        .unwrap_err();
        assert!(format!("{refusal:?}").contains("is stale"), "{refusal:?}");

        // and a key that holds no binding cannot either.
        let mut stranger = at(7, Origin::External(key(99)));
        let refusal = apply(
            &mut module,
            &mut stranger,
            CollaborationMsg::Acknowledge {
                conversation_id: "c1".into(),
                seq,
                binding_credential: credential,
                state: DeliveryState::AdapterAccepted,
                reason: None,
            },
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("only the bound service key"),
            "{refusal:?}"
        );

        ok(
            &mut module,
            &mut service,
            CollaborationMsg::Acknowledge {
                conversation_id: "c1".into(),
                seq,
                binding_credential: credential,
                state: DeliveryState::AdapterAccepted,
                reason: None,
            },
        )
        .await;

        // terminal is terminal.
        let refusal = apply(
            &mut module,
            &mut service,
            CollaborationMsg::Acknowledge {
                conversation_id: "c1".into(),
                seq,
                binding_credential: credential,
                state: DeliveryState::Queued,
                reason: None,
            },
        )
        .await
        .unwrap_err();
        assert!(format!("{refusal:?}").contains("cannot move from"), "{refusal:?}");

        // reaching a terminal state released the queue slot.
        let CollaborationReply::Mailbox(usage) =
            read(&module, &owner_b, "bob", None, ProtectedRead::Mailbox).await
        else {
            panic!("mailbox")
        };
        assert_eq!(usage.undelivered, 0);
        assert_eq!(usage.queued_bytes, 0);
    });
}

#[test]
fn a_reason_must_be_a_snake_case_token() {
    block_on(async {
        let scene = scene("c1").await;
        let mut module = scene.module;
        let mut owner_b = at(2, Origin::External(scene.owner_b.clone()));
        ok(&mut module, &mut owner_b, bind("c1", "bob", key(20), 0)).await;
        let mut alice = at(5, Origin::External(scene.owner_a.clone()));
        ok(
            &mut module,
            &mut alice,
            CollaborationMsg::Send(note("c1", "alice", "bob", 1, 1, 100)),
        )
        .await;

        let seq = admitted_seq(&module, &alice, "alice", 1, 1).await;
        let credential = credential_of(&module, &owner_b, "bob", "c1").await;
        let mut service = at(6, Origin::External(key(20)));
        let refusal = apply(
            &mut module,
            &mut service,
            CollaborationMsg::Acknowledge {
                conversation_id: "c1".into(),
                seq,
                binding_credential: credential,
                state: DeliveryState::Queued,
                reason: Some("the provider said /home/eddy/token".into()),
            },
        )
        .await
        .unwrap_err();
        assert!(format!("{refusal:?}").contains("snake_case"), "{refusal:?}");
    });
}

#[test]
fn expiry_is_permissionless_agreed_time_and_frees_the_queue() {
    block_on(async {
        let (mut module, _alice, bob_key, seq) = one_message().await;

        // before the deadline nobody can expire it.
        let mut early = at(50, Origin::External(key(77)));
        let refusal = apply(
            &mut module,
            &mut early,
            CollaborationMsg::ExpireMessage {
                conversation_id: "c1".into(),
                seq,
            },
        )
        .await
        .unwrap_err();
        assert!(format!("{refusal:?}").contains("not yet at"), "{refusal:?}");

        // past it, ANY caller gets the same answer — the deadline is agreed
        // network time, not a laptop's clock.
        let mut late = at(101, Origin::External(key(77)));
        ok(
            &mut module,
            &mut late,
            CollaborationMsg::ExpireMessage {
                conversation_id: "c1".into(),
                seq,
            },
        )
        .await;

        let bob = at(102, Origin::External(bob_key));
        let CollaborationReply::Receipt(Some(receipt)) = read(
            &module,
            &bob,
            "bob",
            None,
            ProtectedRead::Receipt {
                conversation_id: "c1".into(),
                seq,
            },
        )
        .await
        else {
            panic!("receipt")
        };
        assert_eq!(receipt.state, DeliveryState::Expired);
        assert_eq!(receipt.reason.as_deref(), Some("deadline_passed"));

        let CollaborationReply::Mailbox(usage) =
            read(&module, &bob, "bob", None, ProtectedRead::Mailbox).await
        else {
            panic!("mailbox")
        };
        assert_eq!(usage.undelivered, 0);
    });
}

#[test]
fn one_sender_cannot_fill_another_mailbox_past_its_quota() {
    block_on(async {
        let scene = scene("c1").await;
        let mut module = scene.module;
        let mut alice = at(5, Origin::External(scene.owner_a.clone()));

        for sequence in 1..=MAX_UNDELIVERED_PER_SENDER {
            ok(
                &mut module,
                &mut alice,
                CollaborationMsg::Send(note("c1", "alice", "bob", 1, sequence, 100)),
            )
            .await;
        }
        let refusal = apply(
            &mut module,
            &mut alice,
            CollaborationMsg::Send(note(
                "c1",
                "alice",
                "bob",
                1,
                MAX_UNDELIVERED_PER_SENDER + 1,
                100,
            )),
        )
        .await
        .unwrap_err();
        let refusal = format!("{refusal:?}");
        assert!(
            refusal.contains(QUEUE_FULL),
            "a full queue is a retryable capacity refusal, not an acceptance: {refusal}"
        );
    });
}

#[test]
fn the_event_stream_carries_delivery_and_binding_transitions_not_only_mail() {
    block_on(async {
        let scene = scene("c1").await;
        let mut module = scene.module;
        let mut owner_b = at(2, Origin::External(scene.owner_b.clone()));
        ok(&mut module, &mut owner_b, bind("c1", "bob", key(20), 0)).await;
        let mut alice = at(5, Origin::External(scene.owner_a.clone()));
        ok(
            &mut module,
            &mut alice,
            CollaborationMsg::Send(note("c1", "alice", "bob", 1, 1, 100)),
        )
        .await;
        let seq = admitted_seq(&module, &alice, "alice", 1, 1).await;
        let credential = credential_of(&module, &owner_b, "bob", "c1").await;
        let mut service = at(6, Origin::External(key(20)));
        ok(
            &mut module,
            &mut service,
            CollaborationMsg::Acknowledge {
                conversation_id: "c1".into(),
                seq,
                binding_credential: credential,
                state: DeliveryState::Queued,
                reason: None,
            },
        )
        .await;

        let CollaborationReply::Events(EventPage::Page { events, messages, .. }) = read(
            &module,
            &owner_b,
            "bob",
            None,
            ProtectedRead::Events {
                conversation_id: "c1".into(),
                from_seq: 1,
                limit: 64,
            },
        )
        .await
        else {
            panic!("an event page")
        };
        assert_eq!(messages.len(), 1, "the page joins the admitted message");

        let kinds: Vec<&str> = events
            .iter()
            .map(|event| match event.body {
                EventBody::RosterChanged { .. } => "roster",
                EventBody::BindingChanged { .. } => "binding",
                EventBody::MessageAdmitted { .. } => "message",
                EventBody::DeliveryAdvanced { .. } => "delivery",
            })
            .collect();
        assert_eq!(
            kinds,
            vec!["roster", "roster", "binding", "message", "delivery"],
            "a cursor consumer sees Queued/Held moves, not just new mail"
        );
    });
}

#[test]
fn pruning_refuses_while_a_message_is_still_undelivered() {
    block_on(async {
        let (mut module, alice_key, _bob, seq) = one_message().await;
        let mut owner = at(50, Origin::External(alice_key));
        let refusal = apply(
            &mut module,
            &mut owner,
            CollaborationMsg::Prune {
                conversation_id: "c1".into(),
                through_seq: seq + 1,
            },
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("stays until delivery"),
            "undelivered records survive retention: {refusal:?}"
        );
    });
}

#[test]
fn pruning_raises_the_replay_floor_and_below_it_a_retry_is_receipt_pruned() {
    block_on(async {
        let (mut module, alice_key, _bob, seq) = one_message().await;

        // settle it, then retire it.
        let mut sweeper = at(101, Origin::External(key(77)));
        ok(
            &mut module,
            &mut sweeper,
            CollaborationMsg::ExpireMessage {
                conversation_id: "c1".into(),
                seq,
            },
        )
        .await;
        let mut owner = at(102, Origin::External(alice_key.clone()));
        ok(
            &mut module,
            &mut owner,
            CollaborationMsg::Prune {
                conversation_id: "c1".into(),
                through_seq: seq + 1,
            },
        )
        .await;

        // the dedup record is gone and the floor rose past it.
        let CollaborationReply::SendState(state) = read(
            &module,
            &owner,
            "alice",
            None,
            ProtectedRead::SendState {
                generation: 1,
                sequence: 1,
            },
        )
        .await
        else {
            panic!("send state")
        };
        assert_eq!(state, SendState::ReceiptPruned);

        // re-sending those exact bytes cannot become a NEW admission.
        let mut retry = at(103, Origin::External(alice_key));
        let refusal = apply(
            &mut module,
            &mut retry,
            CollaborationMsg::Send(note("c1", "alice", "bob", 1, 1, 200)),
        )
        .await
        .unwrap_err();
        assert!(format!("{refusal:?}").contains(RECEIPT_PRUNED), "{refusal:?}");
    });
}

/// Retention advances a credential's replay floor to `sequence + 1`, so the
/// top of the sequence space is not admissible: admitting it would leave no
/// floor above it and the prune that retired it would have to wrap.
#[test]
fn the_top_of_a_credentials_sequence_space_is_not_admissible() {
    block_on(async {
        let scene = scene("c1").await;
        let mut module = scene.module;
        let mut alice = at(5, Origin::External(scene.owner_a.clone()));

        let refusal = apply(
            &mut module,
            &mut alice,
            CollaborationMsg::Send(note("c1", "alice", "bob", 1, u64::MAX, 100)),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("exhausted its sequence space"),
            "an exhausted credential rotates, it does not wrap: {refusal:?}"
        );

        // one below the top is admissible, and its floor still advances past
        // it when the message is retired.
        ok(
            &mut module,
            &mut alice,
            CollaborationMsg::Send(note("c1", "alice", "bob", 1, MAX_SEQUENCE, 100)),
        )
        .await;
        let event_seq = admitted_seq(&module, &alice, "alice", 1, MAX_SEQUENCE).await;
        let mut sweeper = at(101, Origin::External(key(77)));
        ok(
            &mut module,
            &mut sweeper,
            CollaborationMsg::ExpireMessage {
                conversation_id: "c1".into(),
                seq: event_seq,
            },
        )
        .await;
        let mut owner = at(102, Origin::External(scene.owner_a.clone()));
        ok(
            &mut module,
            &mut owner,
            CollaborationMsg::Prune {
                conversation_id: "c1".into(),
                through_seq: event_seq + 1,
            },
        )
        .await;
        let CollaborationReply::SendState(state) = read(
            &module,
            &owner,
            "alice",
            None,
            ProtectedRead::SendState {
                generation: 1,
                sequence: MAX_SEQUENCE,
            },
        )
        .await
        else {
            panic!("send state")
        };
        assert_eq!(
            state,
            SendState::ReceiptPruned,
            "the floor cleared the highest admissible sequence without wrapping"
        );
    });
}

#[test]
fn a_cursor_below_the_retained_floor_gets_an_explicit_history_gap() {
    block_on(async {
        let (mut module, alice_key, bob_key, seq) = one_message().await;
        let mut sweeper = at(101, Origin::External(key(77)));
        ok(
            &mut module,
            &mut sweeper,
            CollaborationMsg::ExpireMessage {
                conversation_id: "c1".into(),
                seq,
            },
        )
        .await;
        let mut owner = at(102, Origin::External(alice_key));
        ok(
            &mut module,
            &mut owner,
            CollaborationMsg::Prune {
                conversation_id: "c1".into(),
                through_seq: seq + 1,
            },
        )
        .await;

        let bob = at(103, Origin::External(bob_key));
        let CollaborationReply::Events(page) = read(
            &module,
            &bob,
            "bob",
            None,
            ProtectedRead::Events {
                conversation_id: "c1".into(),
                from_seq: 1,
                limit: 64,
            },
        )
        .await
        else {
            panic!("an event page")
        };
        assert_eq!(
            page,
            EventPage::HistoryGap {
                floor_seq: seq + 1
            },
            "a consumer behind the floor resyncs, it does not advance blind"
        );
    });
}

/// `reply_to` names a MESSAGE. Every committed change takes an event sequence —
/// a roster change, a delivery advance — so a range check would let a
/// `SetRoster` event be the parent of a reply, and a reader threading by
/// `reply_to` would find no message there at all.
#[test]
fn a_reply_answers_a_message_not_any_event_sequence() {
    block_on(async {
        let (mut module, alice_key, _bob_key, seq) = one_message().await;
        let mut alice = at(6, Origin::External(alice_key));

        // the sequence just below the first message is the roster event that
        // seated bob — a real, retained, in-range event that is not a message.
        let roster_event = seq - 1;
        let refusal = apply(
            &mut module,
            &mut alice,
            CollaborationMsg::Send(collaboration::SendRequest {
                reply_to: Some(roster_event),
                ..note("c1", "alice", "bob", 1, 2, 100)
            }),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("not a retained message"),
            "{refusal:?}"
        );

        // the message itself is a parent.
        ok(
            &mut module,
            &mut alice,
            CollaborationMsg::Send(collaboration::SendRequest {
                reply_to: Some(seq),
                ..note("c1", "alice", "bob", 1, 2, 100)
            }),
        )
        .await;
    });
}

/// `ExpireMessage` is permissionless BECAUSE it checks the clock: every caller
/// asking gets the same answer. Reaching the same terminal state through an
/// acknowledgement would route around that check — a bound service, or the
/// participant's own owner, could settle a still-live message and free its
/// queue slot early.
///
/// A LATE report is a different thing and stays admissible (see
/// [`a_late_authentic_acceptance_beats_the_sweeper_and_is_not_relabelled`]):
/// the deadline bounds the resource, not the truth.
#[test]
fn expiry_is_never_reported_by_a_service() {
    block_on(async {
        let scene = scene("c1").await;
        let mut module = scene.module;
        let mut owner_b = at(2, Origin::External(scene.owner_b.clone()));
        ok(&mut module, &mut owner_b, bind("c1", "bob", key(20), 0)).await;
        let mut alice = at(5, Origin::External(scene.owner_a.clone()));
        ok(
            &mut module,
            &mut alice,
            CollaborationMsg::Send(note("c1", "alice", "bob", 1, 1, 100)),
        )
        .await;
        let seq = admitted_seq(&module, &alice, "alice", 1, 1).await;
        let credential = credential_of(&module, &owner_b, "bob", "c1").await;
        let claim_expired = |seq, credential| CollaborationMsg::Acknowledge {
            conversation_id: "c1".into(),
            seq,
            binding_credential: credential,
            state: DeliveryState::Expired,
            reason: None,
        };

        // before, exactly at, and after the deadline: the reporter never owns
        // this state, so the answer does not depend on the clock at all.
        for (now, when) in [(6, "before"), (100, "at"), (150, "after")] {
            let mut service = at(now, Origin::External(key(20)));
            let refusal = apply(&mut module, &mut service, claim_expired(seq, credential))
                .await
                .unwrap_err();
            assert!(
                format!("{refusal:?}").contains("expiry is not reported"),
                "a service {when} the deadline must not claim expiry: {refusal:?}"
            );
        }
        // the owner is no shortcut either: it authorizes the report, it does
        // not authorize skipping the clock.
        let mut owner = at(6, Origin::External(scene.owner_b.clone()));
        let refusal = apply(&mut module, &mut owner, claim_expired(seq, credential))
            .await
            .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("expiry is not reported"),
            "{refusal:?}"
        );

        // a timely report still advances, and the record is still open.
        let mut service = at(7, Origin::External(key(20)));
        ok(
            &mut module,
            &mut service,
            CollaborationMsg::Acknowledge {
                conversation_id: "c1".into(),
                seq,
                binding_credential: credential,
                state: DeliveryState::Queued,
                reason: None,
            },
        )
        .await;

        // and the ONE lawful path to Expired opens at exactly the deadline.
        let mut sweeper = at(100, Origin::External(key(77)));
        ok(
            &mut module,
            &mut sweeper,
            CollaborationMsg::ExpireMessage {
                conversation_id: "c1".into(),
                seq,
            },
        )
        .await;
        let bob = at(101, Origin::External(scene.owner_b));
        let CollaborationReply::Receipt(Some(receipt)) = read(
            &module,
            &bob,
            "bob",
            None,
            ProtectedRead::Receipt {
                conversation_id: "c1".into(),
                seq,
            },
        )
        .await
        else {
            panic!("receipt")
        };
        assert_eq!(receipt.state, DeliveryState::Expired);
    });
}
