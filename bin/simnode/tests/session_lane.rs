//! agent-session-lane scenarios against the sim: the mid-run write door whose
//! ACL lives in consensus (#423/#429). module_gaps.rs's C4 proved the lever —
//! over /v1/submit the sim stamps a caller-named origin verbatim, so the
//! signed-frame lane's two forgeries become legitimate control: claim a run's
//! saga lease with a chosen "executing node" (`SagaMsg::Accept`), `OpenAgentSession`
//! as that lease-holder, and act as the bound 32-byte session key. what these pin,
//! all with EXACT rejection strings from `runs/src/sessions.rs`:
//!
//! - the per-session action budget (`MAX_ACTIONS_PER_SESSION`) is exact: the
//!   grant is spent to the boundary, and the next action refuses.
//! - one session per run: a second `OpenAgentSession` never replaces the live one.
//! - only the bound key may act: a different 32-byte origin, session open, is
//!   refused at the ACL rung (not the unknown-session rung).
//! - only the lease-holder may open: a non-assignee node, run in flight, is
//!   refused at the lease rung (an assignee already holds the lease).
//!
//! no `--echo-oracle`: with no worker the run never settles, so it stays in
//! flight (its lease claimable, its session bindable) for the whole test.

mod harness;

use harness::{Sim, create_channel, post_message};
use serde_json::{Value, json};

/// the node we make the run's execution lease-holder (its Accept'd assignee).
const NODE: &str = "nnnnnnnnnnnnnnnnnnnnnnnnnnnnnnnn";
/// a different 32-byte node — never the lease-holder.
const OTHER: &str = "mmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmm";
/// the 32-byte session key: bytes we can name as BOTH the open payload (a byte
/// array) and the acting origin (a UTF-8 string), since the module checks only
/// its length on open and byte-equality on act.
const SESSION: &str = "ssssssssssssssssssssssssssssssss";
/// a 32-byte origin that is neither the lease-holder nor the session key.
const WRONG: &str = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";

/// the per-session action cap, from the source constant — the boundary the
/// budget rung enforces, and the number its exact refusal string carries.
const BUDGET: u32 = runs::MAX_ACTIONS_PER_SESSION;

/// a wasm tenant's refusal crosses the boundary Debug-formatted
/// (`Error::Rejected("…")`), so a run id's 0x1f separators reach the wire as
/// `\u{1f}`. render an expected id the same way before matching on it.
fn as_refused(run_id: &str) -> String {
    run_id.escape_debug().to_string()
}

/// register agent `scribe` (granted `allowed`), open channel `room` with an
/// anchor message, request an explicit run against it, and read back its
/// (run_id, saga_id). the sim announces no provider pool, so the run's saga
/// attempt stays UNASSIGNED — claimable by the first `Accept`.
fn stage_run(sim: &Sim, allowed: Value) -> (String, String) {
    sim.submit_ok(
        "agent",
        json!({ "register_agent": {
            "agent_id": "scribe",
            "display_name": "Scribe",
            "capability": "text",
            "allowed_actions": allowed,
        }}),
        Some("owner"),
    );
    sim.submit_ok("chat", create_channel("room", "Room"), None);
    sim.submit_ok("chat", post_message("room", "m-1", "please help"), None);
    sim.submit_ok(
        "runs",
        json!({ "request_run": { "agent_id": "scribe", "channel_id": "room", "anchor_seq": 1 } }),
        Some("requester"),
    );
    let pending = sim.query("runs", json!("pending_runs"));
    let entry = &pending["pending_runs"][0];
    let run_id = entry["run_id"]
        .as_str()
        .expect("pending run id")
        .to_string();
    let dispatch_id = entry["dispatch_id"]
        .as_str()
        .expect("dispatch id")
        .to_string();
    // dispatch's own id scheme: `dispatch\x1f{receiver}\x1f{dispatch_id}`.
    let saga_id = format!("dispatch\u{1f}runs\u{1f}{dispatch_id}");
    (run_id, saga_id)
}

/// claim the run's execution lease as `NODE` (the first Accept in consensus
/// order wins the assignee), then bind the session key from that lease-holder.
fn claim_and_open(sim: &Sim, saga_id: &str, run_id: &str) {
    sim.submit_ok(
        "saga",
        json!({ "accept": { "saga_id": saga_id, "attempt": 0 } }),
        Some(NODE),
    );
    sim.submit_ok(
        "runs",
        json!({ "open_agent_session": { "run_id": run_id, "session_key": SESSION.as_bytes().to_vec() } }),
        Some(NODE),
    );
}

/// one mid-run chat post, authored by `origin`.
fn post_action(run_id: &str) -> Value {
    json!({ "agent_action": {
        "run_id": run_id,
        "action": { "post_message": { "channel_id": "room", "text": "progress", "thread": null } },
    }})
}

// ── (a) the budget is spent to its exact boundary ───────

/// a session granted `chat.post_message` may act exactly `MAX_ACTIONS_PER_SESSION`
/// times; the action right after the boundary refuses with the budget string,
/// and the counter never advances past the cap.
#[test]
fn a_session_spends_its_action_budget_to_the_exact_boundary() {
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &["--auto"]);
    let (run_id, saga_id) = stage_run(&sim, json!(["chat.post_message"]));
    claim_and_open(&sim, &saga_id, &run_id);

    // spend the whole grant — each applied action mints its own chat post
    // (`agent/{run}/post/s{n}`, unique per counter) and advances the counter.
    for _ in 0..BUDGET {
        sim.submit_ok("runs", post_action(&run_id), Some(SESSION));
    }
    let sessions = sim.query("runs", json!("agent_sessions"));
    assert_eq!(
        sessions["agent_sessions"][0]["actions"], BUDGET,
        "the budget is fully spent: {sessions}"
    );

    // one past the boundary: refused, and the counter stays pinned at the cap.
    let error = sim.submit_rejected("runs", post_action(&run_id), Some(SESSION));
    assert!(
        error.contains(&format!("has spent its budget of {BUDGET} actions")),
        "the budget rung refuses with its exact string: {error}"
    );
    let sessions = sim.query("runs", json!("agent_sessions"));
    assert_eq!(
        sessions["agent_sessions"][0]["actions"], BUDGET,
        "a refused action spends nothing: {sessions}"
    );
}

// ── (b) one session per run, first binding wins ─────────

/// a second `OpenAgentSession` on a run that already has one is refused — the
/// live session's key is the authority the agent is currently acting under, and
/// a silent replace would let a squatter revoke it mid-run.
#[test]
fn a_second_open_agent_session_on_the_same_run_is_refused() {
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &["--auto"]);
    let (run_id, saga_id) = stage_run(&sim, json!(["chat.post_message"]));
    claim_and_open(&sim, &saga_id, &run_id);

    // the lease-holder re-opens with a fresh key: refused at the one-session rung
    // (which fires ahead of the lease-holder check), so even the rightful opener
    // cannot replace the live binding.
    let error = sim.submit_rejected(
        "runs",
        json!({ "open_agent_session": { "run_id": run_id, "session_key": OTHER.as_bytes().to_vec() } }),
        Some(NODE),
    );
    assert!(
        error.contains("already has an open agent session"),
        "the one-session-per-run rung refuses the re-open: {error}"
    );

    // the original key still stands and still acts.
    sim.submit_ok("runs", post_action(&run_id), Some(SESSION));
    let sessions = sim.query("runs", json!("agent_sessions"));
    assert_eq!(
        sessions["agent_sessions"][0]["session_key"],
        json!(SESSION.as_bytes().to_vec()),
        "the live key is unchanged: {sessions}"
    );
}

// ── (c) only the bound key may act ──────────────────────

/// with a session open, a DIFFERENT 32-byte origin acting on the run is refused
/// at the ACL rung — the wrong-key string, not the unknown-session one — proving
/// the bound key is the sole authority, not the assignee's own node key either.
#[test]
fn only_the_bound_session_key_may_act_on_the_run() {
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &["--auto"]);
    let (run_id, saga_id) = stage_run(&sim, json!(["chat.post_message"]));
    claim_and_open(&sim, &saga_id, &run_id);

    // a stranger's 32-byte key: the session IS open, so this passes the
    // has-a-session gate and fails at the byte-equality ACL.
    let error = sim.submit_rejected("runs", post_action(&run_id), Some(WRONG));
    assert!(
        error.contains(&format!(
            "only the bound session key may act for run {}",
            as_refused(&run_id)
        )),
        "a different origin fails at the wrong-key rung: {error}"
    );

    // even the lease-holder node — which opened the session — may not act as it.
    let error = sim.submit_rejected("runs", post_action(&run_id), Some(NODE));
    assert!(
        error.contains("only the bound session key may act"),
        "the assignee's own key does not pass the ACL: {error}"
    );

    // no budget was spent by either refusal.
    let sessions = sim.query("runs", json!("agent_sessions"));
    assert_eq!(
        sessions["agent_sessions"][0]["actions"], 0,
        "refusals spend no budget: {sessions}"
    );
}

// ── (d) only the lease-holder may open ──────────────────

/// on a run in flight with an assignee but NO session yet, `OpenAgentSession`
/// from a non-assignee node is refused at the lease rung — the run's committed
/// lease, not the payload, is the authorization.
#[test]
fn only_the_lease_holder_may_open_the_agent_session() {
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &["--auto"]);
    let (run_id, saga_id) = stage_run(&sim, json!(["chat.post_message"]));

    // NODE claims the lease; no session is opened yet.
    sim.submit_ok(
        "saga",
        json!({ "accept": { "saga_id": saga_id, "attempt": 0 } }),
        Some(NODE),
    );

    // a different node tries to open: refused at the lease-holder rung (the run
    // is in flight and has no session, so the earlier rungs pass).
    let error = sim.submit_rejected(
        "runs",
        json!({ "open_agent_session": { "run_id": run_id, "session_key": SESSION.as_bytes().to_vec() } }),
        Some(OTHER),
    );
    assert!(
        error.contains(&format!(
            "only the node holding the run's execution lease may open its agent session: {}",
            as_refused(&run_id)
        )),
        "a non-assignee is refused at the lease rung: {error}"
    );

    // no session was staged; the real lease-holder still can open one.
    assert!(
        sim.query("runs", json!("agent_sessions"))["agent_sessions"]
            .as_array()
            .is_some_and(Vec::is_empty),
        "the refused open staged nothing"
    );
    sim.submit_ok(
        "runs",
        json!({ "open_agent_session": { "run_id": run_id, "session_key": SESSION.as_bytes().to_vec() } }),
        Some(NODE),
    );
    let sessions = sim.query("runs", json!("agent_sessions"));
    assert_eq!(
        sessions["agent_sessions"][0]["run_id"], run_id,
        "the lease-holder binds the session: {sessions}"
    );
}
