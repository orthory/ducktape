//! the two `collaboration.*` operations: what runs composes, and — more to the
//! point — what it deliberately does NOT decide.
//!
//! runs composes the message and authorizes nothing: the authority lives in
//! `collaboration`, which judges the binding against the
//! `Origin::Program(account)` the effect actually arrives under. That is why
//! both operations are live-lane only: the settle path emits as
//! `Origin::Module("runs")`, and no module speaks for a participant.

use super::*;
use crate::{OP_COLLABORATION_ACKNOWLEDGE, OP_COLLABORATION_DELIVER};
use collaboration::{CollaborationMsg, DeliveryState, MessageKind, Party};

/// the network this test module is composed on — every emitted request is
/// bound to it by name.
const NETWORK: &str = "test-net";
const ASSIGNEE: [u8; 32] = [0xab; 32];
const SESSION_KEY: [u8; 32] = [0xcd; 32];

fn deliver(message_id: &str) -> ActionEnvelope {
    envelope(
        OP_COLLABORATION_DELIVER,
        Some(serde_json::json!({"channel_id": "review"})),
        serde_json::json!({
            "message_id": message_id,
            "recipient": "acct:42",
            "kind": "question",
            "expires_at": 900,
        }),
    )
}

fn acknowledge(state: &str) -> ActionEnvelope {
    envelope(
        OP_COLLABORATION_ACKNOWLEDGE,
        Some(serde_json::json!({"channel_id": "review", "participant": "acct:42"})),
        serde_json::json!({"credential": 7, "seq": 4, "state": state}),
    )
}

/// a live run for "bot", on a module wired to the collaboration plane and
/// bound to [`NETWORK`].
fn live_run() -> (RunsModule, Registry, String) {
    let registry = registry(&["bot"]);
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
fn a_deliver_prepares_one_network_bound_request_naming_no_actor() {
    let (mut m, registry, run_id) = live_run();
    let mut ctx = session_ctx(&registry, &run_id, Origin::External(SESSION_KEY.to_vec()));
    exec(&mut m, &mut ctx, &act(&run_id, deliver("m3"))).unwrap();

    let msgs = ctx.collaboration_msgs();
    assert_eq!(msgs.len(), 1, "exactly one collaboration follow-up");
    assert_eq!(
        msgs[0].network, NETWORK,
        "the op is bound to this network, so it cannot be replayed onto another"
    );
    let CollaborationMsg::Deliver(request) = &msgs[0].op else {
        panic!("expected a Deliver, got {:?}", msgs[0].op);
    };
    assert_eq!(request.channel_id, "review");
    assert_eq!(request.message_id, "m3");
    assert_eq!(request.recipient, Party::Account(42));
    assert_eq!(request.kind, MessageKind::Question);
    assert_eq!(request.expires_at, 900);
    // WHO is asking is nowhere in these bytes. It is the program origin the
    // account's own call mints, and collaboration checks the chat message
    // was posted by exactly that origin.
    assert_eq!(request.task, None);
    assert!(request.references.is_empty());
}

#[test]
fn a_task_update_preserves_the_typed_attempt_reference() {
    let (mut m, registry, run_id) = live_run();
    let mut ctx = session_ctx(&registry, &run_id, Origin::External(SESSION_KEY.to_vec()));
    let mut action = deliver("m3");
    action.input["kind"] = serde_json::json!("task_update");
    action.input["task"] = serde_json::json!({"id": "review-task", "expected_attempt": 7});
    exec(&mut m, &mut ctx, &act(&run_id, action)).unwrap();
    let msgs = ctx.collaboration_msgs();
    let CollaborationMsg::Deliver(request) = &msgs[0].op else {
        panic!("expected Deliver")
    };
    assert_eq!(request.kind, MessageKind::TaskUpdate);
    assert_eq!(
        request.task,
        Some(collaboration::TaskRef {
            id: "review-task".into(),
            expected_attempt: 7
        })
    );
}

#[test]
fn an_acknowledge_reports_a_state_under_the_binding_credential() {
    let (mut m, registry, run_id) = live_run();
    let mut ctx = session_ctx(&registry, &run_id, Origin::External(SESSION_KEY.to_vec()));
    exec(
        &mut m,
        &mut ctx,
        &act(&run_id, acknowledge("adapter_accepted")),
    )
    .unwrap();

    let msgs = ctx.collaboration_msgs();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].network, NETWORK);
    let CollaborationMsg::Acknowledge {
        channel_id,
        seq,
        recipient,
        binding_credential,
        state,
        reason,
    } = &msgs[0].op
    else {
        panic!("expected an Acknowledge, got {:?}", msgs[0].op);
    };
    assert_eq!(channel_id, "review");
    assert_eq!(*recipient, Party::Account(42));
    assert_eq!(*seq, 4);
    assert_eq!(*binding_credential, 7);
    assert_eq!(*state, DeliveryState::AdapterAccepted);
    assert_eq!(*reason, None);
}

#[test]
fn the_states_a_service_may_report_exclude_the_networks_own_facts() {
    // `stored` is the admission fact and `expired` the deadline's: a bound
    // service claiming either would be reporting on the network's behalf.
    let (mut m, registry, run_id) = live_run();
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
    let registry = registry(&["bot"]);
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
    let err = exec(&mut m, &mut ctx, &act(&run_id, deliver("m3"))).unwrap_err();
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
    for action in [deliver("m3"), acknowledge("queued")] {
        let name = action.operation.clone();
        let registry = registry(&["bot"]);
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
