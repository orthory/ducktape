//! the read path's authorization: who the query CONTEXT proves the caller to
//! be, and how far that credential reaches.
//!
//! The public `/v1/query` lane reaches a module as `Origin::System`, so these
//! tests drive that origin directly to prove the module fails closed on it —
//! and drive `Origin::External(key)` to prove an authenticated seam works.

mod common;

use collaboration::{
    CollaborationMsg, CollaborationReply, DenyReason, ProtectedRead, Role,
};
use common::*;
use futures::executor::block_on;
use sdk::{Module, Origin};

const EVERY_READ: &[fn() -> ProtectedRead] = &[
    || ProtectedRead::Participant,
    || ProtectedRead::Mailbox,
    || ProtectedRead::SendState {
        generation: 1,
        sequence: 1,
    },
    || ProtectedRead::Conversation {
        conversation_id: "c1".into(),
    },
    || ProtectedRead::Access {
        conversation_id: "c1".into(),
    },
    || ProtectedRead::Binding {
        conversation_id: "c1".into(),
    },
    || ProtectedRead::Events {
        conversation_id: "c1".into(),
        from_seq: 1,
        limit: 8,
    },
    || ProtectedRead::Receipt {
        conversation_id: "c1".into(),
        seq: 1,
    },
];

#[test]
fn the_unauthenticated_public_lane_reads_nothing() {
    block_on(async {
        let scene = scene("c1").await;
        let system = at(9, Origin::System);
        for build in EVERY_READ {
            let reply = read(&scene.module, &system, "alice", None, build()).await;
            assert_eq!(
                reply,
                CollaborationReply::Denied(DenyReason::Unauthenticated),
                "the public query lane must fail closed on every read"
            );
        }
    });
}

#[test]
fn a_module_or_program_origin_is_not_a_reader_either() {
    block_on(async {
        let scene = scene("c1").await;
        for origin in [Origin::Module("chat".into()), Origin::Program(7)] {
            let ctx = at(9, origin.clone());
            let reply = read(&scene.module, &ctx, "alice", None, ProtectedRead::Participant).await;
            assert_eq!(
                reply,
                CollaborationReply::Denied(DenyReason::Unauthenticated),
                "{origin:?} holds no key and reads as nobody"
            );
        }
    });
}

#[test]
fn the_context_origin_decides_not_the_payloads_participant_id() {
    block_on(async {
        let scene = scene("c1").await;
        // bob's key asking to read as alice.
        let bob = at(9, Origin::External(scene.owner_b.clone()));
        let reply = read(&scene.module, &bob, "alice", None, ProtectedRead::Participant).await;
        assert_eq!(reply, CollaborationReply::Denied(DenyReason::NotReader));
    });
}

#[test]
fn an_unknown_participant_and_a_forbidden_one_answer_the_same_token() {
    block_on(async {
        let scene = scene("c1").await;
        let bob = at(9, Origin::External(scene.owner_b.clone()));
        let forbidden = read(&scene.module, &bob, "alice", None, ProtectedRead::Participant).await;
        let unknown = read(&scene.module, &bob, "nobody", None, ProtectedRead::Participant).await;
        assert_eq!(
            forbidden, unknown,
            "probing must not distinguish 'not yours' from 'does not exist'"
        );
    });
}

#[test]
fn the_bare_query_lane_has_no_answer_at_all() {
    block_on(async {
        let scene = scene("c1").await;
        // `Module::query` carries no context, so it cannot authenticate
        // anybody — it must not serve a projection instead.
        let refused = scene.module.query(&[]).await;
        assert!(
            matches!(refused, Err(sdk::Error::QueryUnsupported)),
            "the context-free read path answers nothing: {refused:?}"
        );
    });
}

#[test]
fn a_participant_off_the_roster_is_refused_the_conversation() {
    block_on(async {
        let mut module = module();
        let owner = key(1);
        let outsider = key(3);
        let mut a = at(1, Origin::External(owner.clone()));
        ok(&mut module, &mut a, register("alice")).await;
        ok(&mut module, &mut a, conversation("c1")).await;
        ok(&mut module, &mut a, seat("c1", "alice", Some(Role::Member))).await;

        let mut b = at(1, Origin::External(outsider.clone()));
        ok(&mut module, &mut b, register("carol")).await;

        let carol = at(2, Origin::External(outsider));
        let reply = read(
            &module,
            &carol,
            "carol",
            None,
            ProtectedRead::Events {
                conversation_id: "c1".into(),
                from_seq: 1,
                limit: 8,
            },
        )
        .await;
        assert_eq!(reply, CollaborationReply::Denied(DenyReason::NotPermitted));
    });
}

/// Root's escalation case: ONE participant, TWO conversations, TWO scoped
/// service keys. The A-scoped key must not reach B — not its events, not its
/// receipts, not its binding, not its access answer — even though the
/// participant is seated on both. The owner reaches both.
#[test]
fn a_conversation_scoped_service_key_cannot_read_a_sibling_conversation() {
    block_on(async {
        let mut module = module();
        let owner = key(1);
        let peer = key(2);
        let key_a = key(10);
        let key_b = key(11);

        let mut o = at(1, Origin::External(owner.clone()));
        ok(&mut module, &mut o, register("alice")).await;
        let mut p = at(1, Origin::External(peer.clone()));
        ok(&mut module, &mut p, register("bob")).await;

        for room in ["a", "b"] {
            ok(&mut module, &mut o, conversation(room)).await;
            ok(&mut module, &mut o, seat(room, "alice", Some(Role::Member))).await;
            ok(&mut module, &mut o, seat(room, "bob", Some(Role::Member))).await;
        }
        ok(&mut module, &mut o, bind("a", "alice", key_a.clone(), 0)).await;
        ok(&mut module, &mut o, bind("b", "alice", key_b.clone(), 0)).await;

        // alice sends into both rooms so there is content to leak.
        let mut send_a = at(5, Origin::External(owner.clone()));
        ok(
            &mut module,
            &mut send_a,
            CollaborationMsg::Send(note("a", "alice", "bob", 1, 1, 100)),
        )
        .await;
        ok(
            &mut module,
            &mut send_a,
            CollaborationMsg::Send(note("b", "alice", "bob", 1, 2, 100)),
        )
        .await;

        let service_a = at(6, Origin::External(key_a));

        // its own conversation: served.
        let own = read(
            &module,
            &service_a,
            "alice",
            Some("a"),
            ProtectedRead::Events {
                conversation_id: "a".into(),
                from_seq: 1,
                limit: 8,
            },
        )
        .await;
        assert!(
            matches!(own, CollaborationReply::Events(_)),
            "the A-scoped key reads A: {own:?}"
        );

        // the sibling conversation: refused on every conversation-scoped read.
        for read_of_b in [
            ProtectedRead::Events {
                conversation_id: "b".into(),
                from_seq: 1,
                limit: 8,
            },
            ProtectedRead::Receipt {
                conversation_id: "b".into(),
                seq: 4,
            },
            ProtectedRead::Binding {
                conversation_id: "b".into(),
            },
            ProtectedRead::Conversation {
                conversation_id: "b".into(),
            },
        ] {
            let reply = read(&module, &service_a, "alice", Some("a"), read_of_b.clone()).await;
            assert_eq!(
                reply,
                CollaborationReply::Denied(DenyReason::NotPermitted),
                "the A-scoped key must not reach B through {read_of_b:?}"
            );
        }

        // access answers all-false rather than describing B.
        let CollaborationReply::Access(access) = read(
            &module,
            &service_a,
            "alice",
            Some("a"),
            ProtectedRead::Access {
                conversation_id: "b".into(),
            },
        )
        .await
        else {
            panic!("access answers")
        };
        assert!(!access.may_read && !access.may_send && access.binding_credential == 0);

        // participant-wide projections are the owner's, not a scoped key's.
        for wide in [ProtectedRead::Participant, ProtectedRead::Mailbox] {
            let reply = read(&module, &service_a, "alice", Some("a"), wide.clone()).await;
            assert_eq!(
                reply,
                CollaborationReply::Denied(DenyReason::NotReader),
                "{wide:?} aggregates every conversation and stays with the owner"
            );
        }

        // and it cannot read the sibling credential's dedup state.
        let reply = read(
            &module,
            &service_a,
            "alice",
            Some("a"),
            ProtectedRead::SendState {
                generation: 3,
                sequence: 1,
            },
        )
        .await;
        assert_eq!(reply, CollaborationReply::Denied(DenyReason::NotReader));

        // the OWNER reaches both rooms.
        for room in ["a", "b"] {
            let reply = read(
                &module,
                &o,
                "alice",
                None,
                ProtectedRead::Events {
                    conversation_id: room.into(),
                    from_seq: 1,
                    limit: 8,
                },
            )
            .await;
            assert!(
                matches!(reply, CollaborationReply::Events(_)),
                "the owner reads {room}: {reply:?}"
            );
        }
    });
}

#[test]
fn a_revoked_participants_service_key_stops_authenticating() {
    block_on(async {
        let scene = scene("c1").await;
        let mut module = scene.module;
        let mut owner = at(2, Origin::External(scene.owner_a.clone()));
        ok(&mut module, &mut owner, bind("c1", "alice", key(10), 0)).await;

        let service = at(3, Origin::External(key(10)));
        assert!(
            matches!(
                read(
                    &module,
                    &service,
                    "alice",
                    Some("c1"),
                    ProtectedRead::Access {
                        conversation_id: "c1".into()
                    }
                )
                .await,
                CollaborationReply::Access(_)
            ),
            "the scoped key reads before revocation"
        );

        ok(
            &mut module,
            &mut owner,
            CollaborationMsg::RevokeParticipant {
                participant_id: "alice".into(),
            },
        )
        .await;

        let reply = read(
            &module,
            &service,
            "alice",
            Some("c1"),
            ProtectedRead::Access {
                conversation_id: "c1".into(),
            },
        )
        .await;
        assert_eq!(
            reply,
            CollaborationReply::Denied(DenyReason::NotReader),
            "revocation retires every scoped credential at authentication"
        );

        // the owner still reads the history revocation did not rewrite.
        assert!(
            matches!(
                read(&module, &owner, "alice", None, ProtectedRead::Participant).await,
                CollaborationReply::Participant(_)
            ),
            "revocation fences the future, not the past"
        );
    });
}

#[test]
fn a_service_key_without_via_cannot_authenticate() {
    block_on(async {
        let scene = scene("c1").await;
        let mut module = scene.module;
        let mut owner = at(2, Origin::External(scene.owner_a.clone()));
        ok(&mut module, &mut owner, bind("c1", "alice", key(10), 0)).await;

        let service = at(3, Origin::External(key(10)));
        let reply = read(
            &module,
            &service,
            "alice",
            None,
            ProtectedRead::Access {
                conversation_id: "c1".into(),
            },
        )
        .await;
        assert_eq!(
            reply,
            CollaborationReply::Denied(DenyReason::NotReader),
            "a scoped key names the binding it holds; it is not searched for"
        );
    });
}
