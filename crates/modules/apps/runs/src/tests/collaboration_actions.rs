//! the two `collaboration.*` operations: what runs composes, and — more to the
//! point — what it deliberately does NOT decide.
//!
//! runs holds ONE half of the authority: the model's committed grant. The other
//! half lives in `collaboration`, which judges the binding against the
//! `Origin::Program(account)` the effect actually arrives under. That is why
//! both operations are live-lane only: the settle path emits as
//! `Origin::Module("runs")`, and no module speaks for a participant.

use super::*;
use crate::{ACTION_COLLABORATION_ACKNOWLEDGE, ACTION_COLLABORATION_SEND};
use collaboration::{CollaborationMsg, DeliveryState, MessageKind};

/// the network this test module is composed on — every emitted request is
/// bound to it by name.
const NETWORK: &str = "test-net";
const ASSIGNEE: [u8; 32] = [0xab; 32];
const SESSION_KEY: [u8; 32] = [0xcd; 32];

fn send(sequence: u64) -> ActionEnvelope {
    envelope(
        ACTION_COLLABORATION_SEND,
        Some(serde_json::json!({
            "conversation_id": "review",
            "participant_id": "alice",
        })),
        serde_json::json!({
            "credential": 7,
            "sequence": sequence,
            "recipient_participant_id": "bob",
            "kind": "question",
            "body": "does this hold?",
            "expires_at": 900,
        }),
    )
}

fn acknowledge(state: &str) -> ActionEnvelope {
    envelope(
        ACTION_COLLABORATION_ACKNOWLEDGE,
        Some(serde_json::json!({"conversation_id": "review"})),
        serde_json::json!({"credential": 7, "seq": 4, "state": state}),
    )
}

/// a live run for a model granted `actions`, on a module wired to the
/// collaboration plane and bound to [`NETWORK`].
fn live_run(actions: &[&str]) -> (RunsModule, Registry, String) {
    let registry = registry(&[("bot", actions)]);
    let mut m = configured(&registry)
        .with_collaboration_module("collaboration")
        .with_chain_id(NETWORK);
    request_post(&mut m, &registry, 2, &[]);
    commit(&mut m);
    let run_id = run_id_for("general", 2, "bot");
    let mut ctx = session_ctx(&registry, &run_id, Origin::External(ASSIGNEE.to_vec()));
    exec(
        &mut m,
        &mut ctx,
        &admin(&RunsMsg::OpenAgentSession {
            attempt: 0,
            run_id: run_id.clone(),
            session_key: SESSION_KEY.to_vec(),
        }),
    )
    .unwrap();
    commit(&mut m);
    (m, registry, run_id)
}

fn session_ctx(registry: &Registry, run_id: &str, origin: Origin) -> CaptureCtx {
    CaptureCtx::new()
        .at(5)
        .with_origin(origin)
        .with_registry(registry)
        .with_transcript("general", transcript(2))
        .with_lease_holder(run_id, &ASSIGNEE)
}

fn act(run_id: &str, action: ActionEnvelope) -> Msg {
    admin(&RunsMsg::AgentAction {
        run_id: run_id.into(),
        request_id: "req-1".into(),
        action,
    })
}

#[test]
fn a_granted_send_prepares_one_network_bound_message_naming_no_actor() {
    let (mut m, registry, run_id) = live_run(&[ACTION_COLLABORATION_SEND]);
    let mut ctx = session_ctx(&registry, &run_id, Origin::External(SESSION_KEY.to_vec()));
    exec(&mut m, &mut ctx, &act(&run_id, send(3))).unwrap();

    let msgs = ctx.collaboration_msgs();
    assert_eq!(msgs.len(), 1, "exactly one collaboration follow-up");
    assert_eq!(
        msgs[0].network, NETWORK,
        "the op is bound to this network, so it cannot be replayed onto another"
    );
    let CollaborationMsg::Send(request) = &msgs[0].op else {
        panic!("expected a Send, got {:?}", msgs[0].op);
    };
    assert_eq!(request.conversation_id, "review");
    assert_eq!(request.sender_participant_id, "alice");
    assert_eq!(request.recipient_participant_id, "bob");
    assert_eq!(request.kind, MessageKind::Question);
    assert_eq!(request.body, "does this hold?");
    assert_eq!(request.expires_at, 900);
    // the credential is the message id's generation half: the binding the
    // sender claims, which collaboration checks against the one it holds.
    assert_eq!(request.message_id.generation, 7);
    assert_eq!(request.message_id.sequence, 3);
    // WHO is sending is nowhere in these bytes. It is the program origin the
    // account's own call mints, and only collaboration ever sees it.
    assert_eq!(request.task, None);
    assert!(request.references.is_empty());
}

#[test]
fn an_acknowledge_reports_a_state_under_the_binding_credential() {
    let (mut m, registry, run_id) = live_run(&[ACTION_COLLABORATION_ACKNOWLEDGE]);
    let mut ctx = session_ctx(&registry, &run_id, Origin::External(SESSION_KEY.to_vec()));
    exec(&mut m, &mut ctx, &act(&run_id, acknowledge("adapter_accepted"))).unwrap();

    let msgs = ctx.collaboration_msgs();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].network, NETWORK);
    let CollaborationMsg::Acknowledge {
        conversation_id,
        seq,
        binding_credential,
        state,
        reason,
    } = &msgs[0].op
    else {
        panic!("expected an Acknowledge, got {:?}", msgs[0].op);
    };
    assert_eq!(conversation_id, "review");
    assert_eq!(*seq, 4);
    assert_eq!(*binding_credential, 7);
    assert_eq!(*state, DeliveryState::AdapterAccepted);
    assert_eq!(*reason, None);
}

#[test]
fn an_ungranted_model_sends_nothing() {
    // runs' half of the intersection. The binding could be perfect and this
    // still emits nothing, because the model was never granted the action.
    let (mut m, registry, run_id) = live_run(&[ACTION_CHAT_POST]);
    let mut ctx = session_ctx(&registry, &run_id, Origin::External(SESSION_KEY.to_vec()));
    let err = exec(&mut m, &mut ctx, &act(&run_id, send(3))).unwrap_err();
    assert!(
        matches!(&err, Error::Module(reason) if reason.contains("is not allowed to collaboration.send")),
        "{err:?}"
    );
    assert!(ctx.collaboration_msgs().is_empty());
}

#[test]
fn the_states_a_service_may_report_exclude_the_networks_own_facts() {
    // `stored` is the admission fact and `expired` the deadline's: a bound
    // service claiming either would be reporting on the network's behalf.
    let (mut m, registry, run_id) = live_run(&[ACTION_COLLABORATION_ACKNOWLEDGE]);
    for state in ["stored", "expired", "delivered"] {
        let mut ctx = session_ctx(&registry, &run_id, Origin::External(SESSION_KEY.to_vec()));
        let err = exec(&mut m, &mut ctx, &act(&run_id, acknowledge(state))).unwrap_err();
        assert!(
            matches!(&err, Error::Module(reason) if reason.contains("unknown delivery state")),
            "{state} must not be reportable: {err:?}"
        );
        abort(&mut m);
    }
}

#[test]
fn an_unwired_collaboration_plane_refuses_rather_than_degrades() {
    // an unsent message must never look sent: with no collaboration module
    // there is nowhere for the effect to go, so the action fails loudly.
    let registry = registry(&[("bot", &[ACTION_COLLABORATION_SEND])]);
    let mut m = configured(&registry).with_chain_id(NETWORK);
    request_post(&mut m, &registry, 2, &[]);
    commit(&mut m);
    let run_id = run_id_for("general", 2, "bot");
    let mut ctx = session_ctx(&registry, &run_id, Origin::External(ASSIGNEE.to_vec()));
    exec(
        &mut m,
        &mut ctx,
        &admin(&RunsMsg::OpenAgentSession {
            attempt: 0,
            run_id: run_id.clone(),
            session_key: SESSION_KEY.to_vec(),
        }),
    )
    .unwrap();
    commit(&mut m);

    let mut ctx = session_ctx(&registry, &run_id, Origin::External(SESSION_KEY.to_vec()));
    let err = exec(&mut m, &mut ctx, &act(&run_id, send(3))).unwrap_err();
    assert!(
        matches!(&err, Error::Module(reason) if reason.contains("wires none")),
        "{err:?}"
    );
    assert!(ctx.collaboration_msgs().is_empty());
}

#[test]
fn the_settle_lane_admits_neither_operation() {
    // THE LANE TEST. A settle-path follow-up leaves as `Origin::Module("runs")`,
    // which collaboration refuses by design — so the operation must never reach
    // that lane in the first place, and the run fails by name instead of
    // emitting bytes nobody will accept.
    for action in [send(3), acknowledge("queued")] {
        let name = action.operation.clone();
        let registry = registry(&[(
            "bot",
            &[
                ACTION_CHAT_POST,
                ACTION_COLLABORATION_SEND,
                ACTION_COLLABORATION_ACKNOWLEDGE,
            ],
        )]);
        let mut m = configured(&registry)
            .with_collaboration_module("collaboration")
            .with_chain_id(NETWORK);
        request_post(&mut m, &registry, 2, &[]);
        commit(&mut m);
        let run_id = run_id_for("general", 2, "bot");

        let mut ctx = CaptureCtx::new()
            .with_dispatch_origin()
            .with_registry(&registry)
            .with_transcript("general", transcript(2));
        exec(
            &mut m,
            &mut ctx,
            &result_event(&run_id, Ok(response(&["ok"], vec![action]))),
        )
        .unwrap();
        assert!(
            ctx.collaboration_msgs().is_empty(),
            "{name} must emit nothing from the settle lane"
        );
        commit(&mut m);
    }
}
