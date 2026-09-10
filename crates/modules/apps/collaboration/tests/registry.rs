//! participants, conversations, rosters and bindings: who exists, who may
//! edit them, and the credential allocation that keeps two attached devices
//! from writing each other's dedup keys.

mod common;

use collaboration::{
    BoundPrincipal, CollaborationMsg, CollaborationReply, DenyReason, ProtectedRead, Role,
    SendState,
};
use common::*;
use futures::executor::block_on;
use sdk::Origin;

#[test]
fn a_participant_is_owned_by_the_key_that_registered_it() {
    block_on(async {
        let mut module = module();
        let mut ctx = at(1, Origin::External(key(1)));
        ok(&mut module, &mut ctx, register("alice")).await;

        let CollaborationReply::Participant(participant) =
            read(&module, &ctx, "alice", None, ProtectedRead::Participant).await
        else {
            panic!("the owner reads its own record");
        };
        assert_eq!(participant.owner, collaboration::Party::Key(key(1)));
        // credential 1 is the owner's; 0 stays reserved for "holds none".
        assert_eq!(participant.owner_credential, 1);
        assert_eq!(participant.next_credential, 2);
        assert!(!participant.revoked);
    });
}

#[test]
fn system_origin_cannot_register_a_participant() {
    block_on(async {
        let mut module = module();
        let mut ctx = at(1, Origin::System);
        let refusal = apply(&mut module, &mut ctx, register("nobody"))
            .await
            .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("system origin"),
            "system origin authorizes nobody: {refusal:?}"
        );
    });
}

#[test]
fn a_second_registration_of_one_id_is_refused() {
    block_on(async {
        let mut module = module();
        let mut ctx = at(1, Origin::External(key(1)));
        ok(&mut module, &mut ctx, register("alice")).await;

        let mut other = at(2, Origin::External(key(9)));
        let refusal = apply(&mut module, &mut other, register("alice"))
            .await
            .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("already exists"),
            "{refusal:?}"
        );
    });
}

#[test]
fn any_member_edits_a_conversations_roster() {
    block_on(async {
        let mut module = module();
        let mut owner = at(1, Origin::External(key(1)));
        ok(&mut module, &mut owner, register("alice")).await;
        ok(&mut module, &mut owner, conversation("c1")).await;

        // a member who neither created the conversation nor registered the
        // participant seats it, and unseats it again.
        let mut stranger = at(2, Origin::External(key(7)));
        ok(
            &mut module,
            &mut stranger,
            seat("c1", "alice", Some(Role::Member)),
        )
        .await;
        ok(&mut module, &mut stranger, seat("c1", "alice", None)).await;
    });
}

#[test]
fn an_unregistered_participant_cannot_be_seated() {
    block_on(async {
        let mut module = module();
        let mut owner = at(1, Origin::External(key(1)));
        ok(&mut module, &mut owner, conversation("c1")).await;
        let refusal = apply(
            &mut module,
            &mut owner,
            seat("c1", "ghost", Some(Role::Member)),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("no participant"),
            "{refusal:?}"
        );
    });
}

#[test]
fn binding_replacement_requires_the_expected_credential() {
    block_on(async {
        let scene = scene("c1").await;
        let mut module = scene.module;
        let mut owner = at(2, Origin::External(scene.owner_a.clone()));

        ok(&mut module, &mut owner, bind("c1", "alice", key(10), 0)).await;

        // a second device racing with the same expectation loses.
        let refusal = apply(&mut module, &mut owner, bind("c1", "alice", key(11), 0))
            .await
            .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("not the expected"),
            "two devices cannot both claim the binding: {refusal:?}"
        );

        // naming the credential it is replacing succeeds.
        ok(&mut module, &mut owner, bind("c1", "alice", key(11), 2)).await;
        let CollaborationReply::Binding(Some(view)) = read(
            &module,
            &owner,
            "alice",
            None,
            ProtectedRead::Binding {
                conversation_id: "c1".into(),
            },
        )
        .await
        else {
            panic!("the owner reads its own binding");
        };
        assert_eq!(view.credential, 3, "a replacement draws a FRESH credential");
        assert!(!view.detached);
    });
}

#[test]
fn a_service_key_issues_its_own_binding() {
    block_on(async {
        let scene = scene("c1").await;
        let mut module = scene.module;

        // the service key binds itself, then sends under the credential the
        // binding drew: what the binding authorizes is the principal it names.
        let mut service = at(2, Origin::External(key(10)));
        ok(&mut module, &mut service, bind("c1", "alice", key(10), 0)).await;
        let CollaborationReply::Binding(Some(view)) = read(
            &module,
            &service,
            "alice",
            Some("c1"),
            ProtectedRead::Binding {
                conversation_id: "c1".into(),
            },
        )
        .await
        else {
            panic!("the service reads its own binding");
        };
        assert_eq!(view.credential, 2);
        ok(
            &mut module,
            &mut service,
            CollaborationMsg::Send(note("c1", "alice", "bob", 2, 1, 100)),
        )
        .await;
    });
}

/// Root's multi-device edge: ONE participant attached to TWO conversations on
/// TWO devices. Each service starts its own sequence at 1, and the two
/// admissions must not collide — which is exactly what per-credential
/// namespaces buy.
#[test]
fn two_devices_of_one_participant_hold_disjoint_sequence_spaces() {
    block_on(async {
        let mut module = module();
        let owner_a = key(1);
        let owner_b = key(2);
        let device_1 = key(10);
        let device_2 = key(11);

        let mut a = at(1, Origin::External(owner_a.clone()));
        ok(&mut module, &mut a, register("alice")).await;
        ok(&mut module, &mut a, conversation("c1")).await;
        ok(&mut module, &mut a, conversation("c2")).await;

        let mut b = at(1, Origin::External(owner_b.clone()));
        ok(&mut module, &mut b, register("bob")).await;

        for room in ["c1", "c2"] {
            ok(&mut module, &mut a, seat(room, "alice", Some(Role::Member))).await;
            ok(&mut module, &mut a, seat(room, "bob", Some(Role::Member))).await;
        }
        ok(
            &mut module,
            &mut a,
            bind("c1", "alice", device_1.clone(), 0),
        )
        .await;
        ok(
            &mut module,
            &mut a,
            bind("c2", "alice", device_2.clone(), 0),
        )
        .await;

        let CollaborationReply::Binding(Some(one)) = read(
            &module,
            &a,
            "alice",
            None,
            ProtectedRead::Binding {
                conversation_id: "c1".into(),
            },
        )
        .await
        else {
            panic!("binding on c1")
        };
        let CollaborationReply::Binding(Some(two)) = read(
            &module,
            &a,
            "alice",
            None,
            ProtectedRead::Binding {
                conversation_id: "c2".into(),
            },
        )
        .await
        else {
            panic!("binding on c2")
        };
        assert_ne!(
            one.credential, two.credential,
            "each attachment gets its own credential"
        );

        // both services send THEIR sequence 1. neither may borrow the other's
        // credential number, and both admissions land.
        let mut service_1 = at(5, Origin::External(device_1));
        ok(
            &mut module,
            &mut service_1,
            CollaborationMsg::Send(note("c1", "alice", "bob", one.credential, 1, 100)),
        )
        .await;

        let mut service_2 = at(5, Origin::External(device_2));
        ok(
            &mut module,
            &mut service_2,
            CollaborationMsg::Send(note("c2", "alice", "bob", two.credential, 1, 100)),
        )
        .await;

        // each dedup record is its own: probing one credential's sequence 1
        // does not see the other's.
        let CollaborationReply::SendState(first) = read(
            &module,
            &service_1,
            "alice",
            Some("c1"),
            ProtectedRead::SendState {
                generation: one.credential,
                sequence: 1,
            },
        )
        .await
        else {
            panic!("send state")
        };
        let CollaborationReply::SendState(second) = read(
            &module,
            &service_2,
            "alice",
            Some("c2"),
            ProtectedRead::SendState {
                generation: two.credential,
                sequence: 1,
            },
        )
        .await
        else {
            panic!("send state")
        };
        let (SendState::Admitted { seq: first, .. }, SendState::Admitted { seq: second, .. }) =
            (first, second)
        else {
            panic!("both sequence-1 sends were admitted");
        };
        assert!(first > 0 && second > 0);
    });
}

#[test]
fn replacing_one_binding_leaves_a_sibling_conversation_attached() {
    block_on(async {
        let mut module = module();
        let owner = key(1);
        let mut a = at(1, Origin::External(owner.clone()));
        ok(&mut module, &mut a, register("alice")).await;
        for room in ["c1", "c2"] {
            ok(&mut module, &mut a, conversation(room)).await;
            ok(&mut module, &mut a, seat(room, "alice", Some(Role::Member))).await;
            ok(&mut module, &mut a, bind(room, "alice", key(10), 0)).await;
        }
        let CollaborationReply::Binding(Some(before)) = read(
            &module,
            &a,
            "alice",
            None,
            ProtectedRead::Binding {
                conversation_id: "c2".into(),
            },
        )
        .await
        else {
            panic!("binding on c2")
        };

        // replace the c1 attachment only.
        ok(
            &mut module,
            &mut a,
            bind("c1", "alice", key(12), before.credential - 1),
        )
        .await;

        let CollaborationReply::Binding(Some(after)) = read(
            &module,
            &a,
            "alice",
            None,
            ProtectedRead::Binding {
                conversation_id: "c2".into(),
            },
        )
        .await
        else {
            panic!("binding on c2")
        };
        assert_eq!(
            after, before,
            "replacing one conversation's binding must not disturb another's"
        );
    });
}

#[test]
fn revoking_a_roster_seat_detaches_that_binding() {
    block_on(async {
        let scene = scene("c1").await;
        let mut module = scene.module;
        let mut owner = at(2, Origin::External(scene.owner_a.clone()));
        ok(&mut module, &mut owner, bind("c1", "alice", key(10), 0)).await;

        ok(&mut module, &mut owner, seat("c1", "alice", None)).await;

        // the service key can no longer read as the participant: its binding
        // is spent, so `via` no longer authenticates it.
        let service = at(3, Origin::External(key(10)));
        let reply = read(
            &module,
            &service,
            "alice",
            Some("c1"),
            ProtectedRead::Participant,
        )
        .await;
        assert_eq!(
            reply,
            CollaborationReply::Denied(DenyReason::NotReader),
            "a revoked seat fences its binding"
        );
    });
}

#[test]
fn a_revoked_participant_stops_admitting_on_every_credential() {
    block_on(async {
        let scene = scene("c1").await;
        let mut module = scene.module;
        let mut owner = at(2, Origin::External(scene.owner_a.clone()));
        ok(&mut module, &mut owner, bind("c1", "alice", key(10), 0)).await;

        ok(
            &mut module,
            &mut owner,
            CollaborationMsg::RevokeParticipant {
                participant_id: "alice".into(),
            },
        )
        .await;

        // the owner's own credential is retired with the rest.
        let mut send_as_owner = at(3, Origin::External(scene.owner_a.clone()));
        let refusal = apply(
            &mut module,
            &mut send_as_owner,
            CollaborationMsg::Send(note("c1", "alice", "bob", 1, 1, 100)),
        )
        .await
        .unwrap_err();
        assert!(format!("{refusal:?}").contains("revoked"), "{refusal:?}");

        // and so is the scoped service credential.
        let mut send_as_service = at(3, Origin::External(key(10)));
        let refusal = apply(
            &mut module,
            &mut send_as_service,
            CollaborationMsg::Send(note("c1", "alice", "bob", 2, 1, 100)),
        )
        .await
        .unwrap_err();
        assert!(format!("{refusal:?}").contains("revoked"), "{refusal:?}");
    });
}

#[test]
fn a_detached_service_key_cannot_send_and_any_member_unbinds() {
    block_on(async {
        let scene = scene("c1").await;
        let mut module = scene.module;
        let mut owner = at(2, Origin::External(scene.owner_a.clone()));
        ok(&mut module, &mut owner, bind("c1", "alice", key(10), 0)).await;
        ok(&mut module, &mut owner, bind("c1", "alice", key(11), 2)).await;

        let mut stale = at(3, Origin::External(key(10)));
        let refusal = apply(
            &mut module,
            &mut stale,
            CollaborationMsg::Send(note("c1", "alice", "bob", 2, 1, 100)),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("not authorized"),
            "a replaced device is fenced: {refusal:?}"
        );

        // the replaced credential no longer names the binding, so an unbind
        // naming it is refused whoever sends it.
        let refusal = apply(
            &mut module,
            &mut stale,
            CollaborationMsg::Unbind {
                conversation_id: "c1".into(),
                participant_id: "alice".into(),
                expected_credential: 2,
            },
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("not the expected"),
            "{refusal:?}"
        );

        // any member releases the live binding by naming its credential.
        let mut stranger = at(4, Origin::External(key(99)));
        ok(
            &mut module,
            &mut stranger,
            CollaborationMsg::Unbind {
                conversation_id: "c1".into(),
                participant_id: "alice".into(),
                expected_credential: 3,
            },
        )
        .await;
        let CollaborationReply::Binding(Some(view)) = read(
            &module,
            &owner,
            "alice",
            None,
            ProtectedRead::Binding {
                conversation_id: "c1".into(),
            },
        )
        .await
        else {
            panic!("the owner reads the released binding");
        };
        assert!(view.detached, "the stranger's unbind landed");
    });
}

// ---- the program-account principal -----------------------------------------
//
// The lane an AGENT reaches this module through. It never signs: dispatch's
// call lane mints `Origin::Program(account)` after `identity` has proved the
// account is a program executed by the requesting module at an unmoved
// generation, and the host runs the unit at this module under that origin.
// This module's own check is the second, independent authorization — the
// participant's owner bound THAT account to THIS conversation.

/// the account number alice's owner delegates to. a number, not a key: nothing
/// here is signed.
const AGENT_ACCOUNT: sdk::AccountNumber = 42;

#[test]
fn a_bound_program_account_sends_under_its_credential() {
    block_on(async {
        let scene = scene("c1").await;
        let mut module = scene.module;
        let mut owner = at(1, Origin::External(scene.owner_a.clone()));
        ok(
            &mut module,
            &mut owner,
            bind_to("c1", "alice", BoundPrincipal::Program(AGENT_ACCOUNT), 0),
        )
        .await;
        let credential = credential_of(&module, &owner, "alice", "c1").await;

        // the CALL lane's origin — no signature anywhere in this dispatch.
        let mut agent = as_program(2, AGENT_ACCOUNT);
        ok(
            &mut module,
            &mut agent,
            CollaborationMsg::Send(note("c1", "alice", "bob", credential, 1, 400)),
        )
        .await;

        let CollaborationReply::SendState(state) = read(
            &module,
            &agent,
            "alice",
            Some("c1"),
            ProtectedRead::SendState {
                generation: credential,
                sequence: 1,
            },
        )
        .await
        else {
            panic!("the bound program reads its own send state");
        };
        assert!(
            matches!(state, SendState::Admitted { .. }),
            "the program-account lane admits: {state:?}"
        );
    });
}

#[test]
fn a_module_origin_never_authenticates_as_a_bound_principal() {
    block_on(async {
        let scene = scene("c1").await;
        let mut module = scene.module;
        let mut owner = at(1, Origin::External(scene.owner_a.clone()));
        ok(
            &mut module,
            &mut owner,
            bind_to("c1", "alice", BoundPrincipal::Program(AGENT_ACCOUNT), 0),
        )
        .await;
        let credential = credential_of(&module, &owner, "alice", "c1").await;

        // a follow-up from the module that would have queued the call. It is
        // the module in the MIDDLE, not a principal, and it carries no account
        // — so it can never pass for the bound one.
        let mut relay = at(2, Origin::Module("runs".into()));
        let refusal = apply(
            &mut module,
            &mut relay,
            CollaborationMsg::Send(note("c1", "alice", "bob", credential, 1, 400)),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("not authorized to send as"),
            "a module origin must not send as a participant: {refusal:?}"
        );

        // and it reads nothing either.
        assert_eq!(
            read(
                &module,
                &relay,
                "alice",
                Some("c1"),
                ProtectedRead::Events {
                    conversation_id: "c1".into(),
                    from_seq: 1,
                    limit: 8,
                },
            )
            .await,
            CollaborationReply::Denied(DenyReason::Unauthenticated),
            "a module origin is not a reader"
        );
    });
}

#[test]
fn a_different_program_account_is_not_the_bound_one() {
    block_on(async {
        let scene = scene("c1").await;
        let mut module = scene.module;
        let mut owner = at(1, Origin::External(scene.owner_a.clone()));
        ok(
            &mut module,
            &mut owner,
            bind_to("c1", "alice", BoundPrincipal::Program(AGENT_ACCOUNT), 0),
        )
        .await;
        let credential = credential_of(&module, &owner, "alice", "c1").await;

        // identity knows this account too; it is simply not the bound one.
        let mut other = as_program(2, AGENT_ACCOUNT + 1);
        let refusal = apply(
            &mut module,
            &mut other,
            CollaborationMsg::Send(note("c1", "alice", "bob", credential, 1, 400)),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("not authorized to send as"),
            "the binding names ONE account: {refusal:?}"
        );
    });
}

#[test]
fn a_binding_cannot_authorize_an_empty_service_key() {
    block_on(async {
        let scene = scene("c1").await;
        let mut module = scene.module;
        let mut owner = at(1, Origin::External(scene.owner_a.clone()));
        let refusal = apply(
            &mut module,
            &mut owner,
            bind_to("c1", "alice", BoundPrincipal::ServiceKey(Vec::new()), 0),
        )
        .await
        .unwrap_err();
        assert!(
            format!("{refusal:?}").contains("bound service key must be"),
            "an empty key would authenticate nobody and look live: {refusal:?}"
        );
    });
}
