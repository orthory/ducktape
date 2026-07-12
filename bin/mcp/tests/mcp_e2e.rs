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

#[test]
fn a_granted_write_lands_on_chain_attributed_to_the_owner() {
    let h = Harness::start(&["tasks.create"]);

    let created = payload(&h.call(
        h.mcp(),
        "ducktape_task_create",
        json!({"title": "ship the tool plane"}),
    ));
    let task_id = created["task_id"].as_str().expect("the minted id comes back");

    // the oracle: ask the NODE what it holds, not the server that told us it
    // wrote. a tool that lied about writing would pass every assertion that
    // only read its own reply.
    let reply = h.query("tasks", json!("list"));
    let tasks = reply["tasks"].as_array().expect("a task list");
    assert_eq!(tasks.len(), 1, "exactly the one task we created: {reply}");
    assert_eq!(tasks[0]["id"], task_id);
    assert_eq!(tasks[0]["title"], "ship the tool plane");
    assert_eq!(tasks[0]["status"], "open");
}

#[test]
fn a_denied_write_never_reaches_the_chain() {
    // the agent may create tasks but NOT move them.
    let h = Harness::start(&["tasks.create"]);

    let created = payload(&h.call(
        h.mcp(),
        "ducktape_task_create",
        json!({"title": "stays open"}),
    ));
    let task_id = created["task_id"].as_str().unwrap().to_string();

    let refused = h.call(
        h.mcp(),
        "ducktape_task_status",
        json!({"task_id": task_id, "status": "done"}),
    );
    let (is_error, text) = content(&refused);
    assert!(is_error, "the denied write must refuse: {text}");
    // the refusal names the exact grant the owner would have to widen — an
    // agent that cannot say WHICH permission it lacks is useless to its owner.
    assert!(
        text.contains("tasks.update_status"),
        "the refusal must name the missing action: {text}"
    );

    // the assertion that actually proves the gate: the chain is unmoved. a gate
    // that refused in words but submitted anyway would pass the check above.
    let reply = h.query("tasks", json!("list"));
    assert_eq!(
        reply["tasks"][0]["status"], "open",
        "a denied write must not have touched consensus: {reply}"
    );
}

#[test]
fn the_gate_follows_the_committed_grant_when_it_narrows_mid_run() {
    let h = Harness::start(ALL_ACTIONS);

    // with the full grant, the write lands.
    let created = payload(&h.call(h.mcp(), "ducktape_task_create", json!({"title": "first"})));
    assert!(created["task_id"].is_string());

    // the owner narrows the agent on-chain, mid-run — revoking tasks.create.
    h.submit(
        "agent",
        json!({
            "update_agent": {
                "agent_id": AGENT_ID,
                "allowed_actions": ["chat.post"],
            }
        }),
        OWNER,
    );

    // the very next call must refuse. a grant cached at startup would happily
    // keep honouring a permission consensus has already taken away — which is
    // exactly why `Run::record` re-reads the registry per call.
    let refused = h.call(h.mcp(), "ducktape_task_create", json!({"title": "second"}));
    let (is_error, text) = content(&refused);
    assert!(is_error, "the revoked action must now refuse: {text}");
    assert!(text.contains("tasks.create"), "{text}");

    let reply = h.query("tasks", json!("list"));
    assert_eq!(
        reply["tasks"].as_array().unwrap().len(),
        1,
        "the second task must not exist: {reply}"
    );
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
fn a_module_rejection_reaches_the_model_verbatim() {
    let h = Harness::start(&["tasks.update_status"]);

    // moving a task that does not exist: the module refuses, and its own words
    // must reach the model rather than a reworded guess at them — an agent can
    // only correct a mistake it can read.
    let refused = h.call(
        h.mcp(),
        "ducktape_task_status",
        json!({"task_id": "no-such-task", "status": "done"}),
    );
    let (is_error, text) = content(&refused);
    assert!(is_error, "a module rejection must surface as a refusal");
    assert!(
        text.contains("Ducktape refused the request"),
        "the module's rejection must reach the model: {text}"
    );
}

#[test]
fn a_bad_status_name_is_refused_before_it_reaches_the_node() {
    let h = Harness::start(&["tasks.update_status"]);
    h.submit(
        "tasks",
        json!({"create_task": {"task_id": "t1", "title": "t"}}),
        OWNER,
    );

    let refused = h.call(
        h.mcp(),
        "ducktape_task_status",
        json!({"task_id": "t1", "status": "Done"}),
    );
    let (is_error, text) = content(&refused);
    assert!(is_error);
    assert!(
        text.contains("open, in_progress, or done"),
        "a near-miss status must be told the three real names: {text}"
    );

    let reply = h.query("tasks", json!("list"));
    assert_eq!(reply["tasks"][0]["status"], "open", "nothing moved");
}

#[test]
fn one_session_carries_many_calls_and_never_answers_the_notification() {
    let h = Harness::start(&["tasks.create"]);

    // the framing test: a real runner opens ONE stdio session, sends the
    // `initialized` notification, and then makes call after call down the same
    // pipe. an answered notification would shift every id and `session` asserts
    // the count.
    let results = h.session(
        h.mcp(),
        &[
            json!({"name": "ducktape_whoami", "arguments": {}}),
            json!({"name": "ducktape_task_create", "arguments": {"title": "one"}}),
            json!({"name": "ducktape_task_create", "arguments": {"title": "two"}}),
            json!({"name": "ducktape_tasks", "arguments": {}}),
        ],
    );
    assert_eq!(results.len(), 4);
    assert_eq!(payload(&results[0])["agent_id"], AGENT_ID);

    let listed = payload(&results[3]);
    let titles: Vec<&str> = listed["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["title"].as_str().unwrap())
        .collect();
    assert!(titles.contains(&"one") && titles.contains(&"two"), "{titles:?}");

    // both writes landed, with distinct minted ids — the id minter must not
    // collide within one process.
    let reply = h.query("tasks", json!("list"));
    assert_eq!(reply["tasks"].as_array().unwrap().len(), 2);
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
