//! bindings: who carries a participant's deliveries on one channel, and the
//! credential fence that keeps two devices from both claiming it.

mod common;

use collaboration::{
    BoundPrincipal, CollaborationMsg, CollaborationReply, DeliveryState, Party, PrincipalView,
    ProtectedRead,
};
use common::*;
use futures::executor::block_on;
use sdk::Origin;

/// the chat membership follow-ups one dispatch emitted, as (party, member).
fn seats(ctx: &sdk_testkit::TestCtx) -> Vec<(Party, bool)> {
    ctx.msgs()
        .iter()
        .filter(|m| m.target == CHAT)
        .map(|m| match chat::decode_msg(&m.payload).expect("a chat op") {
            chat::ChatMsg::SetMembership { party, member, .. } => (party, member),
            other => panic!("collaboration emitted {other:?}"),
        })
        .collect()
}

#[test]
fn a_binding_needs_a_participant_who_may_read_the_channel() {
    block_on(async {
        let Scene {
            mut module,
            chat,
            alice,
            ..
        } = scene("c1");
        let stranger = party(9);
        let mut ctx = at(&chat, 1, Origin::External(key(1)));
        let refusal = apply(&mut module, &mut ctx, bind("c1", &stranger, key(0x5e), 0))
            .await
            .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("may not read"),
            "{refusal:?}"
        );
        // an unknown channel fails closed the same way.
        let refusal = apply(&mut module, &mut ctx, bind("nope", &alice, key(0x5e), 0))
            .await
            .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("may not read"),
            "{refusal:?}"
        );
        // a module or the system is not a participant at all.
        let refusal = apply(
            &mut module,
            &mut ctx,
            bind("c1", &Party::Module("runs".into()), key(0x5e), 0),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("account or a non-empty key"),
            "{refusal:?}"
        );
    });
}

#[test]
fn a_service_key_binding_seats_the_key_and_replacement_requires_the_expected_credential() {
    block_on(async {
        let Scene {
            mut module,
            chat,
            alice,
            ..
        } = scene("c1");
        let mut ctx = at(&chat, 1, Origin::External(key(1)));
        ok(&mut module, &mut ctx, bind("c1", &alice, key(0x5e), 0)).await;
        assert_eq!(
            seats(&ctx),
            vec![(Party::Key(key(0x5e)), true)],
            "the bound service key is seated on the channel so it can post"
        );
        assert_eq!(credential_of(&module, &ctx, &alice, "c1").await, 1);

        // a second device naming 0 is refused: the fence.
        let mut ctx = at(&chat, 2, Origin::External(key(1)));
        let refusal = apply(&mut module, &mut ctx, bind("c1", &alice, key(0x5f), 0))
            .await
            .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("not the expected"),
            "{refusal:?}"
        );

        // naming the current credential replaces it: the old key loses its
        // seat, the new one takes one, and the credential moves to 2.
        let mut ctx = at(&chat, 3, Origin::External(key(1)));
        ok(&mut module, &mut ctx, bind("c1", &alice, key(0x5f), 1)).await;
        assert_eq!(
            seats(&ctx),
            vec![
                (Party::Key(key(0x5e)), false),
                (Party::Key(key(0x5f)), true)
            ]
        );
        assert_eq!(credential_of(&module, &ctx, &alice, "c1").await, 2);
        let CollaborationReply::Binding(Some(view)) = read(
            &module,
            &ctx,
            &alice,
            None,
            ProtectedRead::Binding {
                channel_id: "c1".into(),
            },
        )
        .await
        else {
            panic!("alice's binding reads");
        };
        assert_eq!(view.principal, PrincipalView::ServiceKey);
        assert!(!view.detached);
    });
}

#[test]
fn unbind_spends_the_credential_and_unseats_the_key() {
    block_on(async {
        let Scene {
            mut module,
            chat,
            alice,
            ..
        } = scene("c1");
        let mut ctx = at(&chat, 1, Origin::External(key(1)));
        ok(&mut module, &mut ctx, bind("c1", &alice, key(0x5e), 0)).await;

        // any authenticated member releases it, naming the credential.
        let mut ctx = at(&chat, 2, Origin::External(key(2)));
        let refusal = apply(
            &mut module,
            &mut ctx,
            CollaborationMsg::Unbind {
                channel_id: "c1".into(),
                participant: alice.clone(),
                expected_credential: 7,
            },
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("not the expected"),
            "{refusal:?}"
        );
        ok(
            &mut module,
            &mut ctx,
            CollaborationMsg::Unbind {
                channel_id: "c1".into(),
                participant: alice.clone(),
                expected_credential: 1,
            },
        )
        .await;
        assert_eq!(seats(&ctx), vec![(Party::Key(key(0x5e)), false)]);

        // detached: a second unbind is refused, and the next bind must name
        // the SPENT credential, never 0 again.
        let mut ctx = at(&chat, 3, Origin::External(key(1)));
        let refusal = apply(
            &mut module,
            &mut ctx,
            CollaborationMsg::Unbind {
                channel_id: "c1".into(),
                participant: alice.clone(),
                expected_credential: 1,
            },
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("already detached"),
            "{refusal:?}"
        );
        let refusal = apply(&mut module, &mut ctx, bind("c1", &alice, key(0x5e), 0))
            .await
            .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("is 1, not the expected 0"),
            "{refusal:?}"
        );
        ok(&mut module, &mut ctx, bind("c1", &alice, key(0x5e), 1)).await;
        assert_eq!(credential_of(&module, &ctx, &alice, "c1").await, 2);
    });
}

#[test]
fn a_program_principal_is_seated_by_nobody_here() {
    block_on(async {
        let Scene {
            mut module,
            chat,
            alice,
            ..
        } = scene("c1");
        let mut ctx = at(&chat, 1, Origin::External(key(1)));
        ok(
            &mut module,
            &mut ctx,
            bind_to("c1", &alice, BoundPrincipal::Program(42), 0),
        )
        .await;
        assert!(
            seats(&ctx).is_empty(),
            "a program account is seated by whoever grants it a seat, never by its binding"
        );
    });
}

#[test]
fn an_empty_service_key_is_refused_at_bind() {
    block_on(async {
        let Scene {
            mut module,
            chat,
            alice,
            ..
        } = scene("c1");
        let mut ctx = at(&chat, 1, Origin::External(key(1)));
        let refusal = apply(&mut module, &mut ctx, bind("c1", &alice, Vec::new(), 0))
            .await
            .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("bound service key must be"),
            "{refusal:?}"
        );
    });
}

#[test]
fn the_event_stream_carries_binding_transitions() {
    block_on(async {
        let Scene {
            mut module,
            chat,
            alice,
            ..
        } = scene("c1");
        let mut ctx = at(&chat, 1, Origin::External(key(1)));
        ok(&mut module, &mut ctx, bind("c1", &alice, key(0x5e), 0)).await;
        ok(
            &mut module,
            &mut ctx,
            CollaborationMsg::Unbind {
                channel_id: "c1".into(),
                participant: alice.clone(),
                expected_credential: 1,
            },
        )
        .await;
        let CollaborationReply::Events(page) = read(
            &module,
            &ctx,
            &alice,
            None,
            ProtectedRead::Events {
                channel_id: "c1".into(),
                from_seq: 0,
                limit: 16,
            },
        )
        .await
        else {
            panic!("events read");
        };
        let bodies: Vec<_> = page.events.iter().map(|e| e.body.clone()).collect();
        assert_eq!(
            bodies,
            vec![
                collaboration::EventBody::BindingChanged {
                    participant: alice.clone(),
                    credential: 1,
                    detached: false
                },
                collaboration::EventBody::BindingChanged {
                    participant: alice.clone(),
                    credential: 1,
                    detached: true
                },
            ]
        );
        assert_eq!(page.next_seq, 3);
        assert!(page.deliveries.is_empty());
        // and nothing about a delivery state rides a binding event.
        assert!(
            !bodies.iter().any(|body| matches!(
                body,
                collaboration::EventBody::DeliveryAdvanced { state: DeliveryState::Stored, .. }
            ))
        );
    });
}
