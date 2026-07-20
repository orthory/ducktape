//! e2e for `ducktape mcp` against an in-process node (see `support/mod.rs`).
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

#[path = "mcp_support/mod.rs"]
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

#[test]
fn agents_and_runs_are_read_from_the_real_modules() {
    let h = Harness::start(&["tasks.create"]);
    h.submit(
        "agent",
        json!({"register_agent": {
            "agent_id": "tailbot",
            "display_name": "Tailbot",
            "capability": "codex",
            "allowed_actions": [],
            "caps": {},
        }}),
        OWNER,
    );

    let results = h.session(
        h.mcp(),
        &[
            json!({"name": "ducktape_agents", "arguments": {"other": 1}}),
            json!({"name": "ducktape_agents", "arguments": {"limit": 1}}),
            json!({"name": "ducktape_runs", "arguments": {}}),
        ],
    );
    let (is_error, text) = content(&results[0]);
    assert!(is_error, "an unknown argument must be refused: {text}");

    let agents = payload(&results[1]);
    assert_eq!(agents["agents"][0]["agent_id"], AGENT_ID);
    assert_eq!(agents["agents"][0]["display_name"], "Quackbot");
    assert_eq!(agents["agents"][0]["status"], "active");
    assert_eq!(
        agents["agents"][0]["allowed_actions"],
        json!(["tasks.create"])
    );
    assert_eq!(agents["agents"].as_array().unwrap().len(), 1);
    assert_eq!(agents["total"], 2);
    assert_eq!(agents["truncated"], true);

    let runs = payload(&results[2]);
    assert_eq!(runs["pending_runs"], json!([]));
    assert_eq!(runs["pending_total"], 0);
    assert_eq!(runs["pending_truncated"], false);
    assert_eq!(runs["recent_runs"], json!([]));
    assert_eq!(runs["recent_total"], 0);
    assert_eq!(runs["recent_truncated"], false);
    assert!(runs.get("agent_sessions").is_none());
}

/// No run is dispatched by this read/query harness, so its scoped action URL is
/// intentionally unavailable. The real provisioner boundary is covered in
/// noded's session tests.
const UNBOUND_RUN: &str = "no-such-saga:0";

#[test]
fn whoami_reports_the_run_id_without_exposing_the_session_key() {
    let h = Harness::start(&[]);

    let sessionless = payload(&h.call(h.mcp(), "ducktape_whoami", json!({})));
    assert!(sessionless["run_id"].is_null());

    let bound = payload(&h.call(h.mcp_with_action(UNBOUND_RUN), "ducktape_whoami", json!({})));
    assert_eq!(bound["run_id"], UNBOUND_RUN);
    assert!(bound.get("session_key").is_none());
    assert!(!bound.to_string().contains(&"4d".repeat(32)));
}

#[test]
fn an_unavailable_scoped_endpoint_never_falls_back_to_an_ambient_write_lane() {
    let h = Harness::start(&["tasks.create"]);

    let refused = h.call(
        h.mcp_with_action(UNBOUND_RUN),
        "ducktape_task_create",
        json!({"title": "should never land"}),
    );
    let (is_error, text) = content(&refused);
    assert!(is_error, "an action for an unbound run must refuse: {text}");
    assert!(
        text.contains("could not reach") || text.contains("scoped action"),
        "the refusal must identify the scoped action lane: {text}"
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
fn a_write_without_a_scoped_endpoint_never_reaches_the_wire_at_all() {
    // No scoped endpoint: the server has no credential to prove the write came from
    // this agent, so it refuses locally rather than falling back to a lane that
    // would file the write under the executing node's identity. that fallback IS
    // the defect this whole design removes, so its absence is asserted.
    let h = Harness::start(&["tasks.create"]);

    let refused = h.call(h.mcp(), "ducktape_task_create", json!({"title": "nope"}));
    let (is_error, text) = content(&refused);
    assert!(is_error, "an endpoint-less write must refuse: {text}");
    assert!(
        text.contains("scoped action endpoint"),
        "the refusal must name the missing scoped endpoint: {text}"
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

    let refused = h.call(
        h.mcp(),
        "ducktape_forge_pr_diff",
        json!({"repo": "app", "number": 1}),
    );
    let (is_error, text) = content(&refused);
    assert!(is_error, "an uncapped forge read must refuse: {text}");
    assert!(
        text.contains("forge_read"),
        "the refusal must name the cap field the owner would widen: {text}"
    );
}

#[test]
fn a_forge_scoped_read_only_agent_can_review_a_real_pr_diff() {
    let h = Harness::start_with_forge_read(&[], &["app"]);
    let commit = |path: &str, content: &str, message: &str| {
        h.submit(
            "forge",
            json!({"commit": {
                "repo": "app", "path": path, "content": content, "message": message
            }}),
            OWNER,
        );
        h.query("forge", json!({"head_of": {"repo": "app"}}))["head"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let oid_bytes = |hex: &str| {
        hex.as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect::<Vec<_>>()
    };
    let target = commit("base.txt", "base\n", "base");
    let source = commit("review.txt", "reviewable\n", "feature");
    h.submit(
        "forge",
        json!({"push_refs": {
            "repo": "app",
            "updates": [
                {"ref_name": "dev", "prev_oid": null, "new_oid": oid_bytes(&target)},
                {"ref_name": "feature", "prev_oid": null, "new_oid": oid_bytes(&source)}
            ],
            "pack_digest": vec![7u8; 32]
        }}),
        OWNER,
    );
    h.submit(
        "forge",
        json!({"open_pr": {
            "repo": "app", "title": "review me", "body": "",
            "source_branch": "feature", "target_branch": "dev"
        }}),
        OWNER,
    );

    let reply = payload(&h.call(
        h.mcp(),
        "ducktape_forge_pr_diff",
        json!({"repo": "app", "number": 1}),
    ));
    let diff = &reply["pr_diff"];
    assert_eq!(diff["source_oid"], source);
    assert_eq!(diff["target_oid"], target);
    assert_eq!(diff["truncated"], false);
    assert!(
        diff["patch"].as_str().unwrap().contains("+reviewable"),
        "{diff}"
    );
    assert!(
        diff["patch"].as_str().unwrap().len() <= forge::MAX_PR_DIFF_BYTES,
        "the typed MCP response must preserve Forge's context cap"
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

    // Whatever refuses — the tool server for a missing endpoint, or `runs` for
    // an invalid live action — its own words must reach the model rather than a
    // reworded guess at them. an agent can only correct a mistake it can read.
    let refused = h.call(
        h.mcp_with_action(UNBOUND_RUN),
        "ducktape_task_status",
        json!({"task_id": "no-such-task", "status": "done"}),
    );
    let (is_error, text) = content(&refused);
    assert!(is_error, "the refusal must surface as one");
    assert!(
        text.contains("could not reach") || text.contains("Ducktape refused the request"),
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
        h.mcp_with_action(UNBOUND_RUN),
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
