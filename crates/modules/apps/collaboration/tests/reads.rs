//! the read lane is authenticated from the query CONTEXT, never from a
//! participant named in the payload.

mod common;

use collaboration::{
    BoundPrincipal, CollaborationMsg, CollaborationReply, DenyReason, Party, ProtectedRead,
};
use common::*;
use futures::executor::block_on;
use sdk::{Cause, Env, Module, Origin};
use sdk_testkit::TestCtx;

fn events(channel_id: &str) -> ProtectedRead {
    ProtectedRead::Events {
        channel_id: channel_id.into(),
        from_seq: 0,
        limit: 16,
    }
}

#[test]
fn the_unauthenticated_public_lane_reads_nothing() {
    block_on(async {
        let Scene {
            module,
            chat,
            alice,
            ..
        } = scene("c1");
        for origin in [Origin::System, Origin::Module("runs".into())] {
            let ctx = at(&chat, 1, origin);
            assert_eq!(
                read(&module, &ctx, &alice, None, events("c1")).await,
                CollaborationReply::Denied(DenyReason::Unauthenticated)
            );
            assert_eq!(
                read(&module, &ctx, &alice, None, ProtectedRead::Mailbox).await,
                CollaborationReply::Denied(DenyReason::Unauthenticated)
            );
        }
        // an empty external key is not a principal either.
        let ctx = at(&chat, 1, Origin::External(Vec::new()));
        assert_eq!(
            read(&module, &ctx, &alice, None, events("c1")).await,
            CollaborationReply::Denied(DenyReason::Unauthenticated)
        );
    });
}

#[test]
fn the_bare_query_lane_has_no_answer_at_all() {
    block_on(async {
        let Scene { module, .. } = scene("c1");
        let refusal = module.query(b"anything").await.unwrap_err();
        assert!(matches!(refusal, sdk::Error::QueryUnsupported));
    });
}

#[test]
fn the_context_origin_decides_not_the_payloads_participant() {
    block_on(async {
        let Scene {
            module,
            chat,
            alice,
            ..
        } = scene("c1");
        // bob's key asking to read as alice.
        let ctx = at(&chat, 1, Origin::External(key(2)));
        assert_eq!(
            read(&module, &ctx, &alice, None, events("c1")).await,
            CollaborationReply::Denied(DenyReason::NotReader)
        );
        // alice's own key reads.
        let ctx = at(&chat, 1, Origin::External(key(1)));
        assert!(matches!(
            read(&module, &ctx, &alice, None, events("c1")).await,
            CollaborationReply::Events(_)
        ));
        // a channel she may not read answers the roster token.
        assert_eq!(
            read(&module, &ctx, &alice, None, events("c2")).await,
            CollaborationReply::Denied(DenyReason::NotPermitted)
        );
    });
}

#[test]
fn a_program_account_reads_as_the_participant_it_is() {
    block_on(async {
        let Scene {
            mut module, chat, ..
        } = scene("c1");
        let agent = Party::Account(42);
        chat.borrow_mut().channel("c1", std::slice::from_ref(&agent));
        let ctx = as_program(&chat, 1, 42);
        assert!(matches!(
            read(&module, &ctx, &agent, None, events("c1")).await,
            CollaborationReply::Events(_)
        ));
        // and a different program account is not it.
        let ctx = as_program(&chat, 1, 43);
        assert_eq!(
            read(&module, &ctx, &agent, None, events("c1")).await,
            CollaborationReply::Denied(DenyReason::NotReader)
        );
        // a program principal bound to a KEY participant reads that channel.
        let holder = party(1);
        chat.borrow_mut().channel("c1", std::slice::from_ref(&holder));
        let mut ctx = at(&chat, 1, Origin::External(key(1)));
        ok(
            &mut module,
            &mut ctx,
            bind_to("c1", &holder, BoundPrincipal::Program(43), 0),
        )
        .await;
        let ctx = as_program(&chat, 2, 43);
        assert!(matches!(
            read(&module, &ctx, &holder, Some("c1"), events("c1")).await,
            CollaborationReply::Events(_)
        ));
    });
}

#[test]
fn a_service_key_reads_its_one_channel_through_via_and_nothing_else() {
    block_on(async {
        let Scene {
            mut module,
            chat,
            alice,
            bob,
        } = scene("c1");
        chat.borrow_mut().channel("c2", &[alice.clone(), bob.clone()]);
        let mut ctx = at(&chat, 1, Origin::External(key(1)));
        ok(&mut module, &mut ctx, bind("c1", &alice, key(0x5e), 0)).await;

        let service = at(&chat, 2, Origin::External(key(0x5e)));
        // without via the key names no binding.
        assert_eq!(
            read(&module, &service, &alice, None, events("c1")).await,
            CollaborationReply::Denied(DenyReason::NotReader)
        );
        // with via it reads c1 …
        assert!(matches!(
            read(&module, &service, &alice, Some("c1"), events("c1")).await,
            CollaborationReply::Events(_)
        ));
        assert!(matches!(
            read(
                &module,
                &service,
                &alice,
                Some("c1"),
                ProtectedRead::Binding {
                    channel_id: "c1".into()
                }
            )
            .await,
            CollaborationReply::Binding(Some(_))
        ));
        // … but not c2, which alice may also read, and not her mailbox.
        assert_eq!(
            read(&module, &service, &alice, Some("c1"), events("c2")).await,
            CollaborationReply::Denied(DenyReason::NotPermitted)
        );
        assert_eq!(
            read(&module, &service, &alice, Some("c1"), ProtectedRead::Mailbox).await,
            CollaborationReply::Denied(DenyReason::NotReader)
        );
        // naming c2 as via with a key bound on c1 is not a reader.
        assert_eq!(
            read(&module, &service, &alice, Some("c2"), events("c2")).await,
            CollaborationReply::Denied(DenyReason::NotReader)
        );

        // a detached key stops authenticating.
        let mut ctx = at(&chat, 3, Origin::External(key(1)));
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
        assert_eq!(
            read(&module, &service, &alice, Some("c1"), events("c1")).await,
            CollaborationReply::Denied(DenyReason::NotReader)
        );
    });
}

#[test]
fn a_delivery_read_is_the_recipients_and_a_sender_learns_nothing() {
    block_on(async {
        let mut scene = scene("c1");
        let (alice, bob) = (scene.alice.clone(), scene.bob.clone());
        let mut ctx = scene.as_bob(1);
        ok(&mut scene.module, &mut ctx, bind("c1", &bob, key(0x5f), 0)).await;
        let seq = scene.alice_posts("c1", "m1");
        let mut ctx = scene.as_alice(1);
        ok(
            &mut scene.module,
            &mut ctx,
            CollaborationMsg::Deliver(deliver("c1", "m1", &bob, 100)),
        )
        .await;
        let probe = ProtectedRead::Delivery {
            channel_id: "c1".into(),
            seq,
        };
        let eligibility = ProtectedRead::DeliveryEligibility {
            channel_id: "c1".into(),
            seq,
        };
        // alice, the sender: no record under HER key.
        assert_eq!(
            read(&scene.module, &ctx, &alice, None, probe.clone()).await,
            CollaborationReply::Delivery(None)
        );
        assert_eq!(
            read(&scene.module, &ctx, &alice, None, eligibility.clone()).await,
            CollaborationReply::Eligibility(collaboration::DeliveryEligibility::Unknown)
        );
        // bob: his record, and his eligibility.
        let ctx = scene.as_bob(2);
        assert!(matches!(
            read(&scene.module, &ctx, &bob, None, probe).await,
            CollaborationReply::Delivery(Some(_))
        ));
        assert!(matches!(
            read(&scene.module, &ctx, &bob, None, eligibility).await,
            CollaborationReply::Eligibility(collaboration::DeliveryEligibility::Eligible { .. })
        ));
    });
}

#[test]
fn an_unbound_recipient_is_told_so() {
    block_on(async {
        let mut scene = scene("c1");
        let bob = scene.bob.clone();
        let seq = scene.alice_posts("c1", "m1");
        let mut ctx = scene.as_alice(1);
        ok(
            &mut scene.module,
            &mut ctx,
            CollaborationMsg::Deliver(deliver("c1", "m1", &bob, 100)),
        )
        .await;
        let ctx = scene.as_bob(2);
        assert_eq!(
            read(
                &scene.module,
                &ctx,
                &bob,
                None,
                ProtectedRead::DeliveryEligibility {
                    channel_id: "c1".into(),
                    seq
                }
            )
            .await,
            CollaborationReply::Eligibility(collaboration::DeliveryEligibility::Unbound)
        );
    });
}

/// a read ctx whose identity sibling ERRORS: an unresolvable origin denies,
/// it does not fail the query.
#[test]
fn an_unresolvable_origin_denies_rather_than_erroring() {
    block_on(async {
        let Scene {
            module,
            chat,
            alice,
            ..
        } = scene("c1");
        let chat_for_ctx = chat.clone();
        let ctx = TestCtx::with_env(Env {
            height: 1,
            consensus_time: 1,
            origin: Origin::Program(7),
            me: MODULE.into(),
            cause: Cause::Direct,
        })
        .on_query(IDENTITY, |_| {
            Ok(identity::encode_reply(&identity::IdentityReply::Account(
                None,
            )))
        })
        .on_query(CHAT, move |req| {
            let _ = req;
            let _ = &chat_for_ctx;
            Err(sdk::Error::Module("unreachable".into()))
        });
        assert_eq!(
            read(&module, &ctx, &alice, None, ProtectedRead::Mailbox).await,
            CollaborationReply::Denied(DenyReason::NotReader)
        );
    });
}
