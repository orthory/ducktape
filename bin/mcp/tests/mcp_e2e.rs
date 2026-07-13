//! e2e for `ducktape-mcp` against an in-process node (see `support/mod.rs`).
//!
//! the binary is driven as a REAL subprocess over stdio, wired with exactly the
//! two environment variables the node's provisioner sets — so what these tests
//! exercise is the production path end to end: MCP framing in, node http out,
//! consensus at the far end.
//!
//! the assertions that matter, and why:
//!
//! - a GRANTED write reaches the chain. read back by querying the node
//!   DIRECTLY, never through the server that claims to have written it.
//! - a DENIED write never leaves the process. proven not by the refusal text
//!   but by the chain: the tasks module still holds nothing.
//! - the grant that gates it is the COMMITTED one. narrowed on-chain mid-run,
//!   the very next call refuses — no cached permission outlives its revocation.
//! - a refusal is a tool RESULT, not a protocol error, so the model can read it.

mod support;

use serde_json::json;
use support::{AGENT_ID, Harness, OWNER, content, payload};

/// every action the registry knows — the "fully trusted agent" grant.
const ALL_ACTIONS: &[&str] = &[
    "chat.post",
    "chat.post_message",
    "tasks.create",
    "tasks.update_status",
    "pages.comment",
    "pages.set_checked",
];

#[test]
fn whoami_reports_the_committed_grant() {
    let h = Harness::start(&["tasks.create"]);

    let who = payload(&h.call(h.mcp(), "ducktape_whoami", json!({})));

    assert_eq!(who["agent_id"], AGENT_ID);
    assert_eq!(who["display_name"], "Quackbot");
    // the grant the agent sees is the one consensus holds — not one copied into
    // the environment, which could disagree with the chain.
    assert_eq!(who["allowed_actions"], json!(["tasks.create"]));
    assert_eq!(who["owner"], json!({"external": OWNER.as_bytes()}));
}

/// a session key bound to nothing — no run in this harness was ever dispatched.
/// consensus must therefore refuse every action it signs, and the refusal must
/// come from `runs`, not from the binary under test.
const SEED: [u8; 32] = [77u8; 32];
const UNBOUND_RUN: &str = "no-such-saga:0";

#[test]
fn whoami_reports_the_run_id_without_exposing_the_session_key() {
    let h = Harness::start(&[]);

    let sessionless = payload(&h.call(h.mcp(), "ducktape_whoami", json!({})));
    assert!(sessionless["run_id"].is_null());

    let bound = payload(&h.call(
        h.mcp_with_session(SEED, UNBOUND_RUN),
        "ducktape_whoami",
        json!({}),
    ));
    assert_eq!(bound["run_id"], UNBOUND_RUN);
    assert!(bound.get("session_key").is_none());
    assert!(!bound.to_string().contains(&"4d".repeat(32)));
}

#[test]
fn a_write_is_refused_by_consensus_not_by_the_tool_server() {
    // the agent holds tasks.create — so if anything refuses this, it is NOT the
    // grant. it is `runs`, in consensus, observing that the session key signing
    // the op is bound to no live run. that is the whole architecture in one
    // assertion: the tool server no longer decides.
    let h = Harness::start(&["tasks.create"]);

    let refused = h.call(
        h.mcp_with_session(SEED, UNBOUND_RUN),
        "ducktape_task_create",
        json!({"title": "should never land"}),
    );
    let (is_error, text) = content(&refused);
    assert!(is_error, "an action for an unbound run must refuse: {text}");
    // `runs`'s own words, reaching the model verbatim through the frame lane.
    assert!(
        text.contains("agent session") || text.contains("not in flight"),
        "the refusal must be the runs module's, about the session: {text}"
    );

    // and the chain is unmoved. a gate that refused in words but wrote anyway
    // would pass the check above.
    let reply = h.query("tasks", json!("list"));
    assert!(
        reply["tasks"].as_array().is_none_or(|t| t.is_empty()),
        "nothing may have been written: {reply}"
    );
}

#[test]
fn a_write_without_a_session_never_reaches_the_wire_at_all() {
    // no session key: the server has no credential to prove the write came from
    // this agent, so it refuses locally rather than falling back to a lane that
    // would file the write under the executing node's identity. that fallback IS
    // the defect this whole design removes, so its absence is asserted.
    let h = Harness::start(&["tasks.create"]);

    let refused = h.call(h.mcp(), "ducktape_task_create", json!({"title": "nope"}));
    let (is_error, text) = content(&refused);
    assert!(is_error, "a session-less write must refuse: {text}");
    assert!(
        text.contains("session"),
        "the refusal must say the run holds no session: {text}"
    );
}

#[test]
fn the_reported_grant_is_the_committed_one_even_as_it_narrows() {
    // whoami reads the registry per call, so an owner narrowing the agent
    // mid-run is visible immediately. (the ENFORCEMENT of that grant now lives
    // in consensus — see runs' own tests; what the tool server still owes the
    // model is an honest answer about what it currently holds.)
    let h = Harness::start(ALL_ACTIONS);
    let before = payload(&h.call(h.mcp(), "ducktape_whoami", json!({})));
    assert!(
        before["allowed_actions"]
            .as_array()
            .unwrap()
            .contains(&json!("tasks.create"))
    );

    h.submit(
        "agent",
        json!({"update_agent": {"agent_id": AGENT_ID, "allowed_actions": ["chat.post"]}}),
        OWNER,
    );

    let after = payload(&h.call(h.mcp(), "ducktape_whoami", json!({})));
    assert_eq!(
        after["allowed_actions"],
        json!(["chat.post"]),
        "a cached grant would still be reporting the revoked one"
    );
}

#[test]
fn one_session_carries_many_calls_and_never_answers_the_notification() {
    let h = Harness::start(&["tasks.create"]);
    h.submit(
        "tasks",
        json!({"create_task": {"task_id": "seeded", "title": "from the test"}}),
        OWNER,
    );

    // the framing test: a real runner opens ONE stdio session, sends the
    // `initialized` notification, then makes call after call down the same pipe.
    // an answered notification would shift every id, and `session` asserts the
    // count.
    let results = h.session(
        h.mcp(),
        &[
            json!({"name": "ducktape_whoami", "arguments": {}}),
            json!({"name": "ducktape_tasks", "arguments": {}}),
            json!({"name": "ducktape_chat_channels", "arguments": {}}),
        ],
    );
    assert_eq!(results.len(), 3);
    assert_eq!(payload(&results[0])["agent_id"], AGENT_ID);
    assert_eq!(payload(&results[1])["tasks"][0]["id"], "seeded");
}

#[test]
fn a_cap_gated_read_refuses_when_the_caps_do_not_cover_it() {
    // an agent with every ACTION but no resource CAPS: the two halves of the
    // grant are independent, and forge reads are gated on caps.forge_read,
    // which this agent's (default, empty) caps do not carry.
    let h = Harness::start(ALL_ACTIONS);

    let refused = h.call(h.mcp(), "ducktape_forge_items", json!({"repo": "app"}));
    let (is_error, text) = content(&refused);
    assert!(is_error, "an uncapped forge read must refuse: {text}");
    assert!(
        text.contains("forge_read"),
        "the refusal must name the cap field the owner would widen: {text}"
    );
}

#[test]
fn an_ungated_read_reaches_the_module() {
    let h = Harness::start(&["tasks.create"]);
    h.submit(
        "tasks",
        json!({"create_task": {"task_id": "seeded", "title": "from the test"}}),
        OWNER,
    );

    // chat/tasks/pages carry no read cap in the caps vocabulary, so reads of
    // them are ungated — inventing a gate the registry cannot express would be
    // a permission nobody could grant.
    let listed = payload(&h.call(h.mcp(), "ducktape_tasks", json!({})));
    assert_eq!(listed["tasks"][0]["id"], "seeded");
    assert_eq!(listed["tasks"][0]["title"], "from the test");
}

#[test]
fn a_run_with_no_agent_can_read_but_never_write() {
    let h = Harness::start(ALL_ACTIONS);
    h.submit(
        "tasks",
        json!({"create_task": {"task_id": "seeded", "title": "visible"}}),
        OWNER,
    );

    // no DUCKTAPE_RUN_AGENT: the server is bound to a node but acting for
    // nobody. ungated reads still work...
    let listed = payload(&h.call(h.mcp_agentless(), "ducktape_tasks", json!({})));
    assert_eq!(listed["tasks"][0]["id"], "seeded");

    // ...and every write refuses, because there is no grant to check against and
    // no owner to attribute it to. it must NOT fall back to the node's own
    // identity, which would file the write under the operator's name.
    let refused = h.call(
        h.mcp_agentless(),
        "ducktape_task_create",
        json!({"title": "should never exist"}),
    );
    let (is_error, text) = content(&refused);
    assert!(is_error, "an agentless write must refuse: {text}");

    let reply = h.query("tasks", json!("list"));
    assert_eq!(
        reply["tasks"].as_array().unwrap().len(),
        1,
        "the agentless write must not have landed: {reply}"
    );
}

#[test]
fn a_refusal_reaches_the_model_verbatim() {
    let h = Harness::start(&["tasks.update_status"]);

    // whatever refuses — the tool server for a missing credential, or `runs` for
    // an unbound session — its OWN words must reach the model rather than a
    // reworded guess at them. an agent can only correct a mistake it can read.
    let refused = h.call(
        h.mcp_with_session(SEED, UNBOUND_RUN),
        "ducktape_task_status",
        json!({"task_id": "no-such-task", "status": "done"}),
    );
    let (is_error, text) = content(&refused);
    assert!(is_error, "the refusal must surface as one");
    assert!(
        text.contains("Ducktape refused the request"),
        "the refusal must reach the model verbatim: {text}"
    );
}

#[test]
fn a_bad_status_name_is_refused_before_it_reaches_the_node() {
    let h = Harness::start(&["tasks.update_status"]);

    // the ONE thing still checked locally: a status that is not one of the three
    // wire names. not a permission decision — a spelling one — so answering it
    // here costs the agent a consensus round-trip it would only lose anyway.
    let refused = h.call(
        h.mcp_with_session(SEED, UNBOUND_RUN),
        "ducktape_task_status",
        json!({"task_id": "t1", "status": "Done"}),
    );
    let (is_error, text) = content(&refused);
    assert!(is_error);
    assert!(
        text.contains("open, in_progress, or done"),
        "a near-miss status must be told the three real names: {text}"
    );
}

#[test]
fn initialize_hands_the_model_the_guide() {
    let h = Harness::start(&[]);
    let result = h.initialize();

    assert_eq!(result["serverInfo"]["name"], "ducktape");
    let guide = result["instructions"].as_str().expect("the guide");
    // the two things an agent gets wrong without being told: that its workspace
    // is not duckfs, and that a refusal is information rather than an obstacle.
    assert!(guide.contains("ducktape_whoami"), "{guide}");
    assert!(guide.contains("duckfs"), "{guide}");
}
