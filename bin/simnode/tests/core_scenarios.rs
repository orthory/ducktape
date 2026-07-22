//! core-module scenarios against the sim: consensus-ordering races, the
//! logical clock as a lease clock, cross-module cascades, and origin-derived
//! authority — module semantics the app never surfaces (jobs, inbox,
//! automations, and the identity→duckdns chain have no console actions), so
//! neither the TS scenario lane nor fleet live-QA can reach them. every
//! rejection asserted here is the REAL module refusing over noded's exact
//! wire; the sim adds only WHEN blocks commit. the job board lives in the
//! merged `tasks` module; job ops ride the WorkMsg envelope's `job` arm.

mod harness;

use commonware_cryptography::Signer as _;
use harness::{Sim, create_channel, ed_bind_auth, post_message};
use identity::bind_preimage;

type Ed = commonware_cryptography::ed25519::PrivateKey;

// ── jobs: the board under consensus ordering ────────────

#[test]
fn a_lost_claim_race_fails_deterministically() {
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &["--auto"]);
    sim.submit_ok(
        "tasks",
        serde_json::json!({ "job": { "submit": { "job_id": "j1", "kind": "build", "spec": "{}" } } }),
        Some("poster"),
    );

    // script the race: our claim parks…
    sim.set_auto(false);
    let pending = sim.submit_in_background(
        "tasks",
        serde_json::json!({ "job": { "claim": { "job_id": "j1", "lease_views": 10 } } }),
        Some("worker-a"),
    );
    sim.await_sim_state("held", 1);

    // …and the rival's claim commits first — the consensus order already
    // picked the winner.
    sim.peer_block(
        "tasks",
        serde_json::json!({ "job": { "claim": { "job_id": "j1", "lease_views": 10 } } }),
        "worker-b",
    );

    // releasing ours mints NO block; the loser fails identically on every node.
    let report = sim.step();
    assert!(
        report["committed"].is_null(),
        "a lost claim must not commit: {report}"
    );
    let (code, reply) = pending.join().expect("claim thread");
    assert_eq!(code, 400, "lost claim must be rejected: {reply}");
    let error = reply["error"].as_str().unwrap_or_default();
    assert!(
        error.contains("job not claimable (status Processing)"),
        "rejection names the committed status: {error}"
    );

    // the board holds the winner's lease.
    let job = sim.query("tasks", serde_json::json!({ "job": { "get": { "job_id": "j1" } } }));
    assert_eq!(job["job"]["job"]["status"], "processing", "board: {job}");
    assert_eq!(job["job"]["job"]["attempt"], 1);
}

#[test]
fn an_expired_lease_reclaims_exactly_past_its_deadline() {
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &["--auto"]);
    sim.submit_ok(
        "tasks",
        serde_json::json!({ "job": { "submit": { "job_id": "j1", "kind": "build", "spec": "{}" } } }),
        Some("poster"),
    ); // height 1
    sim.submit_ok(
        "tasks",
        serde_json::json!({ "job": { "claim": { "job_id": "j1", "lease_views": 10 } } }),
        Some("worker-a"),
    ); // height 2 → deadline = 2 + 10

    let reclaim = serde_json::json!({ "job": { "reclaim": { "job_id": "j1" } } });

    // an early reclaim is refused with the deadline math (the rejected op
    // still seals a block, so it advances the clock to height 3).
    let error = sim.submit_rejected("tasks", reclaim.clone(), Some("scavenger"));
    assert!(
        error.contains("lease not expired"),
        "early reclaim: {error}"
    );

    // the LOGICAL clock is the lease clock: walk it with filler blocks to the
    // last in-lease height. a reclaim executing AT the deadline still fails…
    for i in 0..8 {
        sim.submit_ok(
            "inbox",
            serde_json::json!({ "deliver": { "member": "filler", "kind": "tick", "body": format!("{i}") } }),
            Some("filler"),
        );
    } // heights 4..=11 — the next op executes at 12 == deadline
    let error = sim.submit_rejected("tasks", reclaim.clone(), Some("scavenger"));
    assert!(
        error.contains("lease not expired (height 12 <= deadline 12)"),
        "boundary reclaim: {error}"
    );

    // …and one block later the permissionless reclaim requeues the job.
    sim.submit_ok(
        "inbox",
        serde_json::json!({ "deliver": { "member": "filler", "kind": "tick", "body": "last" } }),
        Some("filler"),
    ); // height 13
    sim.submit_ok("tasks", reclaim, Some("scavenger")); // height 14 > deadline
    let job = sim.query("tasks", serde_json::json!({ "job": { "get": { "job_id": "j1" } } }));
    assert_eq!(job["job"]["job"]["status"], "pending", "reclaimed board: {job}");
}

// ── automations: the chat-hook cascade ──────────────────

#[test]
fn a_matching_post_fires_its_rule_atomically_in_the_same_block() {
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &["--auto"]);
    sim.submit_ok("chat", create_channel("general", "General"), None);
    sim.submit_ok(
        "chat",
        serde_json::json!({ "register_hook": { "channel_id": "general", "module_id": "automations" } }),
        Some("operator"),
    );
    sim.submit_ok(
        "automations",
        serde_json::json!({ "create_rule": {
            "rule_id": "r1",
            "trigger": { "channel_id": "general", "mention": null, "text_contains": "deploy" },
            "action": { "create_task": { "task_id_prefix": "auto", "title_template": "deploy requested" } },
        }}),
        Some("operator"),
    );

    // a non-matching post fires nothing.
    sim.submit_ok("chat", post_message("general", "m-1", "hello world"), None);
    let tasks = sim.query("tasks", serde_json::json!({ "task": "list" }));
    assert_eq!(
        tasks["task"]["tasks"].as_array().map(Vec::len),
        Some(0),
        "non-matching post must not fire: {tasks}"
    );

    // the matching post and its task commit as ONE atomic block: the receipt
    // height IS the chain tip afterwards — no follow-up block exists.
    let receipt = sim.submit_ok(
        "chat",
        post_message("general", "m-2", "please deploy now"),
        None,
    );
    let fired_at = receipt["height"].as_u64().expect("receipt height");
    assert_eq!(
        sim.status()["height"].as_u64(),
        Some(fired_at),
        "the rule's effect rides the triggering block"
    );

    // the task id is deterministic per (prefix, channel, seq) — m-2 is seq 2.
    let tasks = sim.query("tasks", serde_json::json!({ "task": "list" }));
    assert_eq!(tasks["task"]["tasks"][0]["id"], "auto-general-2", "tasks: {tasks}");
    assert_eq!(tasks["task"]["tasks"][0]["title"], "deploy requested");

    // the run history recorded exactly one fire.
    let history = sim.query(
        "automations",
        serde_json::json!({ "run_history": { "rule_id": "r1", "limit": 10 } }),
    );
    assert_eq!(
        history["history"].as_array().map(Vec::len),
        Some(1),
        "history: {history}"
    );
}

#[test]
fn a_hook_event_cannot_be_spoofed_from_outside_chat() {
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &["--auto"]);

    // routing is by the HOST-ASSIGNED origin: an external submitter claiming
    // the chat-hook payload never reaches the hook arm.
    let error = sim.submit_rejected(
        "automations",
        serde_json::json!({ "hook_event": [1, 2, 3] }),
        Some("mallory"),
    );
    assert!(
        error.contains("hook events must originate from the chat module"),
        "spoofed hook: {error}"
    );
}

// ── inbox: the seq discipline ───────────────────────────

#[test]
fn inbox_seqs_never_rewind_and_maintenance_is_idempotent() {
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &["--auto"]);
    let deliver = |body: &str| serde_json::json!({ "deliver": { "member": "eddy", "kind": "note", "body": body } });
    let list = serde_json::json!({ "list": { "member": "eddy", "from_seq": 0, "limit": 10 } });
    let unread = serde_json::json!({ "unread": { "member": "eddy" } });

    sim.submit_ok("inbox", deliver("one"), Some("courier"));
    sim.submit_ok("inbox", deliver("two"), Some("courier"));
    let items = sim.query("inbox", list.clone());
    let seqs: Vec<u64> = items["items"]
        .as_array()
        .expect("items array")
        .iter()
        .map(|n| n["seq"].as_u64().expect("seq"))
        .collect();
    assert_eq!(seqs, vec![1, 2], "items: {items}");
    assert_eq!(sim.query("inbox", unread.clone())["unread_count"], 2);

    // mark-read is a watermark, and over-marking is a deterministic no-op —
    // never an error (an unknown member too).
    sim.submit_ok(
        "inbox",
        serde_json::json!({ "mark_read": { "member": "eddy", "up_to_seq": 1 } }),
        Some("eddy"),
    );
    assert_eq!(sim.query("inbox", unread.clone())["unread_count"], 1);
    sim.submit_ok(
        "inbox",
        serde_json::json!({ "mark_read": { "member": "eddy", "up_to_seq": 999 } }),
        Some("eddy"),
    );
    assert_eq!(sim.query("inbox", unread.clone())["unread_count"], 0);
    sim.submit_ok(
        "inbox",
        serde_json::json!({ "mark_read": { "member": "nobody", "up_to_seq": 7 } }),
        Some("eddy"),
    );

    // clearing deletes the items but NEVER rewinds next_seq: the next
    // delivery continues the sequence, so a consumer's watermark stays valid.
    sim.submit_ok(
        "inbox",
        serde_json::json!({ "clear": { "member": "eddy", "up_to_seq": 2 } }),
        Some("eddy"),
    );
    assert_eq!(
        sim.query("inbox", list.clone())["items"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    sim.submit_ok("inbox", deliver("three"), Some("courier"));
    let items = sim.query("inbox", list);
    assert_eq!(items["items"][0]["seq"], 3, "seq rewound: {items}");
}

// ── identity → duckdns: origin-derived account authority ─

#[test]
fn identity_binding_gates_the_duck_handle_and_labels_are_exclusive() {
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &["--auto"]);
    // duckdns derives the account from the ORIGIN, which must be a 32-byte
    // node key — the sim's origin strings become those bytes verbatim.
    let node_a = "a".repeat(32);
    let node_b = "b".repeat(32);
    let set_handle =
        |handle: serde_json::Value| serde_json::json!({ "set_handle": { "handle": handle } });

    // a short origin is not a node key, and an unbound node has no account.
    let error = sim.submit_rejected("gateway", set_handle("eddy".into()), Some("noded"));
    assert!(
        error.contains("origin must be a 32-byte node key"),
        "{error}"
    );
    let error = sim.submit_rejected("gateway", set_handle("eddy".into()), Some(&node_a));
    assert!(
        error.contains("not bound to an Identity account"),
        "{error}"
    );

    // found account A: the founding key consents to binding node_a (nonce 0,
    // the sim's chain_id is empty). a forged consent must not bind.
    let key_a = Ed::from_seed(1);
    let preimage = bind_preimage("", node_a.as_bytes(), 0);
    let mut forged = ed_bind_auth(&key_a, &preimage);
    forged["proof"]["signature"]["sig"] = serde_json::json!(vec![9u8; 64]);
    let error = sim.submit_rejected(
        "identity",
        serde_json::json!({ "bind_node": { "authorizer": forged } }),
        Some(&node_a),
    );
    assert!(error.contains("does not verify"), "forged bind: {error}");
    sim.submit_ok(
        "identity",
        serde_json::json!({ "bind_node": { "authorizer": ed_bind_auth(&key_a, &preimage) } }),
        Some(&node_a),
    );

    // the bound node registers the handle, and resolution serves account A.
    sim.submit_ok("gateway", set_handle("eddy".into()), Some(&node_a));
    let resolved = sim.query(
        "gateway",
        serde_json::json!({ "resolve": { "name": { "handle": "eddy" } } }),
    );
    assert_eq!(
        resolved["resolved"]["account_id"],
        serde_json::json!(key_a.public_key().as_ref().to_vec()),
        "resolution: {resolved}"
    );

    // account B cannot take the claimed label…
    let key_b = Ed::from_seed(2);
    let preimage_b = bind_preimage("", node_b.as_bytes(), 0);
    sim.submit_ok(
        "identity",
        serde_json::json!({ "bind_node": { "authorizer": ed_bind_auth(&key_b, &preimage_b) } }),
        Some(&node_b),
    );
    let error = sim.submit_rejected("gateway", set_handle("eddy".into()), Some(&node_b));
    assert!(
        error.contains("already claimed by another account"),
        "label exclusivity: {error}"
    );

    // …until the holder releases it; then the label re-registers cleanly.
    sim.submit_ok(
        "gateway",
        set_handle(serde_json::Value::Null),
        Some(&node_a),
    );
    sim.submit_ok("gateway", set_handle("eddy".into()), Some(&node_b));
    let resolved = sim.query(
        "gateway",
        serde_json::json!({ "resolve": { "name": { "handle": "eddy" } } }),
    );
    assert_eq!(
        resolved["resolved"]["account_id"],
        serde_json::json!(key_b.public_key().as_ref().to_vec()),
        "re-registration: {resolved}"
    );
}
