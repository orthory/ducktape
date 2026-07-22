//! module-gap scenarios against the sim: the second wave of module semantics
//! the app never surfaces — identity's shared replay nonce, automations'
//! cross-module abort atomicity (P2), the jobs authorization matrix and its
//! attempt ceiling, forge's per-branch compare-and-swap, and tagging's
//! module-origin gate. like `core_scenarios`, every rejection asserted here is
//! the REAL module refusing over noded's exact wire; the sim only decides WHEN
//! a block commits. these paths have no console action (no identity key
//! ceremony, no jobs board, no raw forge push, no tagging surface), so neither
//! the TS scenario lane nor fleet live-QA can reach them.

mod harness;

use commonware_cryptography::Signer as _;
use harness::{Sim, create_channel, post_message};
use identity::{
    IDENTITY_ADD_MEMBER_NS, IDENTITY_BIND_NS, IDENTITY_REMOVE_MEMBER_NS, IDENTITY_UNBIND_NS,
    KeyKind, add_member_preimage, bind_preimage, remove_member_preimage, unbind_preimage,
};

type Ed = commonware_cryptography::ed25519::PrivateKey;

/// a MemberAuth whose ed25519 `key` consents to `preimage` under `ns` — the
/// identity module's own member-consent shape (the shared `identity::testkit`
/// builder, wrapped back to the untyped JSON the sim's `/v1/submit` takes), over
/// ANY signing namespace — the general ns variant this suite needs (bind, unbind,
/// add/remove-member).
fn ed_auth(key: &Ed, ns: &[u8], preimage: &[u8]) -> serde_json::Value {
    serde_json::to_value(identity::testkit::ed_auth(key, ns, preimage))
        .expect("MemberAuth serializes")
}

/// 40-char sha1 hex → its 20 raw bytes (a forge oid on the RefUpdate wire).
fn hex_to_bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex oid"))
        .collect()
}

// ── C2 — identity: the shared replay nonce ──────────────

/// AddMemberKey advances the account's shared nonce, so a certificate minted at
/// the old nonce can never replay; the account always keeps one live key.
#[test]
fn a_member_add_bumps_the_nonce_and_invalidates_a_pre_bump_cert() {
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &["--auto"]);

    // found account A: key_a's consent binds node_a at nonce 0, and acceptance
    // advances the account's shared nonce to 1. (the sim's chain_id is empty.)
    let node_a = "a".repeat(32);
    let key_a = Ed::from_seed(1);
    let acct_a = key_a.public_key().as_ref().to_vec();
    sim.submit_ok(
        "identity",
        serde_json::json!({ "bind_node": {
            "authorizer": ed_auth(&key_a, IDENTITY_BIND_NS, &bind_preimage("", node_a.as_bytes(), 0)),
        }}),
        Some(&node_a),
    );
    let acct = sim.query(
        "identity",
        serde_json::json!({ "get": { "account_id": acct_a } }),
    );
    assert_eq!(
        acct["account"]["nonce"], 1,
        "founding bind advances the nonce: {acct}"
    );

    // pre-build (but do NOT yet submit) an unbind cert for node_a at the CURRENT
    // nonce, 1. an AddMemberKey is about to move the nonce out from under it.
    let stale_unbind = ed_auth(
        &key_a,
        IDENTITY_UNBIND_NS,
        &unbind_preimage("", node_a.as_bytes(), 1),
    );

    // admit a second member key_c: an existing member (key_a) consents AND the
    // new key proves possession, both over the add-preimage at nonce 1. this
    // advances the nonce to 2.
    let key_c = Ed::from_seed(3);
    let add_at_1 = add_member_preimage(
        "",
        &acct_a,
        key_c.public_key().as_ref(),
        KeyKind::Ed25519,
        1,
    );
    sim.submit_ok(
        "identity",
        serde_json::json!({ "add_member_key": {
            "new_key": key_c.public_key().as_ref().to_vec(),
            "new_kind": "ed25519",
            "new_label": null,
            "possession": { "signature": { "sig": key_c.sign(IDENTITY_ADD_MEMBER_NS, &add_at_1).as_ref().to_vec() } },
            "authorizer": ed_auth(&key_a, IDENTITY_ADD_MEMBER_NS, &add_at_1),
        }}),
        Some(&node_a),
    );
    let acct = sim.query(
        "identity",
        serde_json::json!({ "get": { "account_id": acct_a } }),
    );
    assert_eq!(
        acct["account"]["nonce"], 2,
        "the add advances the nonce: {acct}"
    );

    // THE REPLAY: the stale unbind cert (signed at nonce 1) no longer verifies
    // against the account's advanced nonce (2) — the module recomputes the
    // preimage at the current nonce, so a one-nonce-behind cert is a forgery.
    let error = sim.submit_rejected(
        "identity",
        serde_json::json!({ "unbind_node": {
            "node_key": node_a.as_bytes().to_vec(),
            "authorizer": stale_unbind,
        }}),
        Some(&node_a),
    );
    assert!(
        error.contains("authorizer certificate does not verify"),
        "the pre-bump cert must not replay: {error}"
    );

    // a FRESH-nonce unbind cert (at nonce 2) evicts node_a — nonce → 3 — and
    // node_a re-binds cleanly with another fresh cert (at nonce 3).
    sim.submit_ok(
        "identity",
        serde_json::json!({ "unbind_node": {
            "node_key": node_a.as_bytes().to_vec(),
            "authorizer": ed_auth(&key_a, IDENTITY_UNBIND_NS, &unbind_preimage("", node_a.as_bytes(), 2)),
        }}),
        Some(&node_a),
    );
    sim.submit_ok(
        "identity",
        serde_json::json!({ "bind_node": {
            "authorizer": ed_auth(&key_a, IDENTITY_BIND_NS, &bind_preimage("", node_a.as_bytes(), 3)),
        }}),
        Some(&node_a),
    );
    let acct = sim.query(
        "identity",
        serde_json::json!({ "get": { "account_id": acct_a } }),
    );
    assert_eq!(
        acct["account"]["nonce"], 4,
        "unbind then re-bind each advance the nonce: {acct}"
    );
}

/// an account never gives up its last key: RemoveMemberKey of the sole member
/// is refused before any signature is even considered.
#[test]
fn removing_the_last_member_key_is_refused() {
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &["--auto"]);

    // found account B with a single member, key_b (binding node_b).
    let node_b = "b".repeat(32);
    let key_b = Ed::from_seed(2);
    let acct_b = key_b.public_key().as_ref().to_vec();
    sim.submit_ok(
        "identity",
        serde_json::json!({ "bind_node": {
            "authorizer": ed_auth(&key_b, IDENTITY_BIND_NS, &bind_preimage("", node_b.as_bytes(), 0)),
        }}),
        Some(&node_b),
    );

    // removing the only member is refused by the keep-one-key guard, which fires
    // before the authorizer cert is verified (a valid cert would still fail).
    let error = sim.submit_rejected(
        "identity",
        serde_json::json!({ "remove_member_key": {
            "target_key": acct_b,
            "authorizer": ed_auth(&key_b, IDENTITY_REMOVE_MEMBER_NS, &remove_member_preimage("", &acct_b, &acct_b, 1)),
        }}),
        Some(&node_b),
    );
    assert!(
        error.contains("cannot remove the last member of an account"),
        "the last key must survive: {error}"
    );
}

// ── C3 — automations: id-squatting + P2 abort atomicity ──

/// a rival that pre-posts a rule's deterministic message id is DEFENDED by the
/// probe, not an abort: the triggering post commits, the rule records a
/// no-fire, and the user's block is protected. (this is the designed id-squat
/// defense — see the abort test below for the path that is NOT catchable.)
#[test]
fn a_squatted_post_id_downgrades_the_rule_without_aborting_the_post() {
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &["--auto"]);
    sim.submit_ok("chat", create_channel("general", "General"), None);

    // the rival squats the id the rule WILL compose for the next post. the rule
    // fires on message seq 2 (this squat is seq 1), so the composed id is
    // `auto-{rule}-{channel}-2`. the squat is posted BEFORE the hook, so it
    // reaches no rule itself.
    sim.submit_ok(
        "chat",
        post_message("general", "auto-r1-general-2", "i got here first"),
        Some("rival"),
    );

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
            "action": { "post_message": { "channel_id": "general", "template": "deploy acknowledged" } },
        }}),
        Some("operator"),
    );

    // the triggering post COMMITS — the probe caught the squatted id and
    // downgraded the rule's action to a run-history breadcrumb, protecting the
    // posting user's block rather than aborting it.
    sim.submit_ok(
        "chat",
        post_message("general", "trigger", "please deploy now"),
        None,
    );

    let history = sim.query(
        "automations",
        serde_json::json!({ "run_history": { "rule_id": "r1", "limit": 10 } }),
    );
    assert_eq!(
        history["history"].as_array().map(Vec::len),
        Some(1),
        "the squat is recorded as one no-fire: {history}"
    );
    assert_eq!(
        history["history"][0]["action_ok"], false,
        "the action did not fire: {history}"
    );
    assert!(
        history["history"][0]["detail"]
            .as_str()
            .unwrap_or_default()
            .contains("message id already taken"),
        "the recorded reason is the squat: {history}"
    );
    let rule = sim.query(
        "automations",
        serde_json::json!({ "get_rule": { "rule_id": "r1" } }),
    );
    assert_eq!(
        rule["rule"]["fire_count"], 0,
        "a downgraded rule never bumps fire_count: {rule}"
    );
}

/// the P2 contract: a post-probe follow-up collision — two rules composing the
/// SAME task id in one event — aborts the whole triggering block, and NOTHING
/// from it survives in ANY module root.
#[test]
fn a_task_id_collision_aborts_the_entire_triggering_block() {
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &["--auto"]);
    sim.submit_ok("chat", create_channel("general", "General"), None);
    sim.submit_ok(
        "chat",
        serde_json::json!({ "register_hook": { "channel_id": "general", "module_id": "automations" } }),
        Some("operator"),
    );

    // two rules, SAME task_id_prefix, same trigger: their composed task ids both
    // resolve to `auto-general-{seq}`. each probes tasks and sees no collision
    // (the sibling's CreateTask is a queued follow-up, invisible to the probe),
    // so both emit — and the second follow-up collides at execute.
    for rule_id in ["r1", "r2"] {
        sim.submit_ok(
            "automations",
            serde_json::json!({ "create_rule": {
                "rule_id": rule_id,
                "trigger": { "channel_id": "general", "mention": null, "text_contains": "deploy" },
                "action": { "create_task": { "task_id_prefix": "auto", "title_template": "deploy requested" } },
            }}),
            Some("operator"),
        );
    }

    // snapshot the chain tip BEFORE the doomed submit.
    let before = sim.status();
    let before_height = before["height"].as_u64().expect("height");
    let before_hash = before["appHash"].as_str().expect("app hash").to_string();

    // the triggering post fires both rules; the second CreateTask collides with
    // the first's staged id, and the WHOLE block aborts (P2) — the op is rejected
    // and moves no state (the atomic block rolled back).
    let (code, reply) = sim.submit(
        "chat",
        post_message("general", "trigger", "please deploy now"),
        None,
    );
    assert_eq!(
        code, 400,
        "the collision must abort the triggering block: {reply}"
    );
    assert!(
        reply["error"]
            .as_str()
            .unwrap_or_default()
            .contains("task already exists"),
        "the abort names the duplicate id: {reply}"
    );

    // NO STATE survives: the rejected op journals a block (validator parity — so
    // the HEIGHT advances by one) but the atomic abort rolled back every write,
    // so the app-hash is byte-identical, the triggering message never entered
    // chat, no task landed, and neither rule recorded a fire.
    let after = sim.status();
    assert_eq!(
        after["height"].as_u64(),
        Some(before_height + 1),
        "the rejected op sealed its own block (validator parity): {after}"
    );
    assert_eq!(
        after["appHash"].as_str(),
        Some(before_hash.as_str()),
        "app-hash unmoved (the rejected op rolled back): {after}"
    );
    let message = sim.query(
        "chat",
        serde_json::json!({ "message": { "message_id": "trigger" } }),
    );
    assert!(
        message["message"].is_null(),
        "the aborted post left no message: {message}"
    );
    let tasks = sim.query("tasks", serde_json::json!({ "task": "list" }));
    assert_eq!(
        tasks["task"]["tasks"].as_array().map(Vec::len),
        Some(0),
        "no task survived the abort: {tasks}"
    );
    for rule_id in ["r1", "r2"] {
        let rule = sim.query(
            "automations",
            serde_json::json!({ "get_rule": { "rule_id": rule_id } }),
        );
        assert_eq!(
            rule["rule"]["fire_count"], 0,
            "the aborted rule kept fire_count 0: {rule}"
        );
    }
}

// ── C5 — jobs: the authorization matrix + attempt ceiling ─

/// every guarded transition rejects the wrong actor with its own precise
/// message: finalize/release are claimant-only, cancel is submitter-only, and
/// prune only touches terminal jobs.
#[test]
fn the_jobs_authorization_matrix_gates_every_transition() {
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &["--auto"]);
    // the job board now lives in the merged `tasks` module; ops ride the
    // WorkMsg envelope's `job` arm.
    let job = |op: serde_json::Value| serde_json::json!({ "job": op });
    sim.submit_ok(
        "tasks",
        job(serde_json::json!({ "submit": { "job_id": "j1", "kind": "build", "spec": "{}" } })),
        Some("poster"),
    );
    sim.submit_ok(
        "tasks",
        job(serde_json::json!({ "claim": { "job_id": "j1", "lease_views": 10 } })),
        Some("worker-a"),
    );

    // finalize and release are the claimant's alone: a stranger is refused by
    // identity, not by state — the job IS processing.
    let error = sim.submit_rejected(
        "tasks",
        job(serde_json::json!({ "finalize": { "job_id": "j1", "ok": true, "payload": "" } })),
        Some("intruder"),
    );
    assert!(
        error.contains("only the current claimant may finalize"),
        "finalize gate: {error}"
    );
    let error = sim.submit_rejected(
        "tasks",
        job(serde_json::json!({ "release": { "job_id": "j1" } })),
        Some("intruder"),
    );
    assert!(
        error.contains("only the current claimant may release"),
        "release gate: {error}"
    );

    // release it back to pending so the CANCEL test hits the submitter gate, not
    // the pending-only status guard.
    sim.submit_ok(
        "tasks",
        job(serde_json::json!({ "release": { "job_id": "j1" } })),
        Some("worker-a"),
    );
    let error = sim.submit_rejected(
        "tasks",
        job(serde_json::json!({ "cancel": { "job_id": "j1" } })),
        Some("worker-a"),
    );
    assert!(
        error.contains("only the submitter may cancel"),
        "cancel gate: {error}"
    );

    // prune only applies to terminal jobs: j1 is pending, so even its own
    // submitter is refused by the status guard.
    let error = sim.submit_rejected(
        "tasks",
        job(serde_json::json!({ "prune": { "job_id": "j1" } })),
        Some("poster"),
    );
    assert!(
        error.contains("prune only applies to terminal jobs"),
        "prune gate: {error}"
    );
}

/// the attempt ceiling is exact: an expired reclaim requeues on every claim
/// below MAX_ATTEMPTS and FAILS the job on the claim that reaches it.
#[test]
fn an_expired_reclaim_fails_the_job_exactly_at_the_attempt_ceiling() {
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &["--auto"]);
    // the job board now lives in the merged `tasks` module; ops and the `get`
    // query ride the WorkMsg/WorkQuery envelope's `job` arm, and the reply is a
    // WorkReply::Job wrapping a JobsReply::Job (hence the doubled `["job"]`).
    let job = |op: serde_json::Value| serde_json::json!({ "job": op });
    sim.submit_ok(
        "tasks",
        job(serde_json::json!({ "submit": { "job_id": "j1", "kind": "build", "spec": "{}" } })),
        Some("poster"),
    );

    let claim = job(serde_json::json!({ "claim": { "job_id": "j1", "lease_views": 10 } }));
    let reclaim = job(serde_json::json!({ "reclaim": { "job_id": "j1" } }));
    let mut fill: u64 = 0;

    // walk claim/expiry cycles. each claim bumps `attempt`; the LOGICAL clock is
    // the lease clock, so inbox filler blocks age the lease past its deadline.
    // claims 1..MAX requeue on expiry; the MAX-th claim's expiry fails the job.
    for attempt in 1..=tasks::MAX_ATTEMPTS {
        let claimed_at = sim.submit_ok("tasks", claim.clone(), Some("worker"))["height"]
            .as_u64()
            .expect("claim height");
        // lease_views clamps to MIN_LEASE_VIEWS (10), so deadline = claim + 10;
        // the reclaim must execute at a height strictly past it.
        let deadline = claimed_at + tasks::MIN_LEASE_VIEWS;
        while sim.status()["height"].as_u64().expect("height") < deadline {
            sim.submit_ok(
                "inbox",
                serde_json::json!({ "deliver": { "member": "filler", "kind": "tick", "body": fill.to_string() } }),
                Some("filler"),
            );
            fill += 1;
        }
        sim.submit_ok("tasks", reclaim.clone(), Some("scavenger"));

        let reply = sim.query("tasks", job(serde_json::json!({ "get": { "job_id": "j1" } })));
        let job_view = &reply["job"];
        assert_eq!(
            job_view["job"]["attempt"].as_u64(),
            Some(attempt),
            "attempt tracks the claim count: {reply}"
        );
        if attempt < tasks::MAX_ATTEMPTS {
            assert_eq!(
                job_view["job"]["status"], "pending",
                "an expired reclaim below the ceiling requeues: {reply}"
            );
        } else {
            assert_eq!(
                job_view["job"]["status"], "failed",
                "the ceiling-th expired reclaim fails the job: {reply}"
            );
            assert_eq!(
                job_view["job"]["result"]["payload"], "attempts exhausted",
                "the failure carries the ceiling reason: {reply}"
            );
        }
    }
}

// ── C6 — forge: per-branch compare-and-swap ─────────────

/// PushRefs is a per-branch CAS against the committed head: a stale prev_oid is
/// refused, a matching one births a branch, and a review pins the exact commit
/// oid its author reviewed.
#[test]
fn forge_push_is_cas_guarded_and_a_review_pins_its_commit() {
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &["--auto"]);

    // born the default repo via `commit` (PR #530's compose-over-the-wire path):
    // `main` gets a real head oid.
    sim.submit_ok(
        "forge",
        serde_json::json!({ "commit": { "repo": "default", "path": "README.md", "content": "hi", "message": "init" } }),
        None,
    );
    let head = sim.query("forge", serde_json::json!("head"))["head"]
        .as_str()
        .expect("main is born")
        .to_string();
    let head_bytes = hex_to_bytes(&head);

    // a push whose prev_oid does NOT equal the committed head is a CAS reject —
    // the SOLE consensus gate of the push path, fired at execute (no block).
    let error = sim.submit_rejected(
        "forge",
        serde_json::json!({ "push_refs": {
            "repo": "default",
            "updates": [{ "ref_name": "main", "prev_oid": vec![0u8; 20], "new_oid": head_bytes }],
            "pack_digest": vec![0u8; 32],
        }}),
        None,
    );
    assert!(
        error.contains("non-fast-forward: forge HEAD moved"),
        "a stale prev_oid must fail the CAS: {error}"
    );

    // a correctly-CAS'd push births a new branch (prev_oid None = unborn) at the
    // main head. the pack is node-local catch-up; consensus gates only the CAS.
    sim.submit_ok(
        "forge",
        serde_json::json!({ "push_refs": {
            "repo": "default",
            "updates": [{ "ref_name": "feature/x", "prev_oid": null, "new_oid": head_bytes }],
            "pack_digest": vec![0u8; 32],
        }}),
        None,
    );
    let refs = sim.query(
        "forge",
        serde_json::json!({ "list_refs": { "repo": "default" } }),
    );
    let born: Vec<&str> = refs["refs"]
        .as_array()
        .expect("refs array")
        .iter()
        .map(|r| r["name"].as_str().unwrap_or_default())
        .collect();
    assert!(
        born.contains(&"main") && born.contains(&"feature/x"),
        "the push births the branch: {refs}"
    );

    // a review anchors at the exact source head the reviewer saw — open a PR
    // from the born branch, review it against `head`, and read the pin back.
    sim.submit_ok(
        "forge",
        serde_json::json!({ "open_pr": { "repo": "default", "title": "ship it", "source_branch": "feature/x", "target_branch": "main" } }),
        Some("author"),
    );
    sim.submit_ok(
        "forge",
        serde_json::json!({ "submit_review": { "repo": "default", "number": 1, "verdict": "comment", "body": "lgtm", "commit_oid": head, "comments": [] } }),
        Some("reviewer"),
    );
    let item = sim.query(
        "forge",
        serde_json::json!({ "get_item": { "repo": "default", "number": 1 } }),
    );
    assert_eq!(
        item["item"]["reviews"][0]["commit_oid"], head,
        "the review pins the reviewed commit: {item}"
    );
}

// ── C7 — tagging: the module-origin gate ────────────────

/// tagging admits NO external surface: a direct tag op over /v1/submit — even
/// one naming the genesis-configured direct owner — is refused by origin, the
/// same shape as a spoofed chat hook.
#[test]
fn a_direct_tagging_op_cannot_be_driven_from_outside_a_module() {
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &["--auto"]);

    // the sim wires `TaggingModule::new("tagging").with_direct_owner("runs")`,
    // but that grant is a MODULE-to-module routing capability. over /v1/submit
    // every origin is external, and the tag intake resolves its source from the
    // dispatch origin, never a payload field — so an external tag has no surface.
    let error = sim.submit_rejected(
        "tagging",
        serde_json::json!({ "tag": {
            "container": "thread-1",
            "content_seq": 1,
            "author": { "user": [1, 2, 3] },
            "tags": [{ "module": "runs", "entity": "qa-luna" }],
        }}),
        Some("mallory"),
    );
    assert!(
        error.contains("tagging ops are module-origin only"),
        "spoofed tag: {error}"
    );

    // the subscription arm is gated identically: no external submitter may
    // register a subscription on another module's behalf.
    let error = sim.submit_rejected(
        "tagging",
        serde_json::json!({ "subscribe": { "source": "chat", "container": "general" } }),
        Some("mallory"),
    );
    assert!(
        error.contains("tagging ops are module-origin only"),
        "spoofed subscribe: {error}"
    );
}

// ── C4 — agent session-key ACL (the mid-run write lane) ──

/// an agent's MID-RUN write is refused at the module layer when the action is
/// outside its committed grant. the session lane is the one door to that ACL
/// that returns the refusal LOUDLY (the settle path degrades it) — and it is
/// reachable here precisely because the sim honors a caller-named origin: the
/// signed-frame lane's two forgeries (the executing node, then the session key)
/// become legitimate control. so this claims the run's lease via
/// `SagaMsg::Accept`, opens the session as that assignee, and acts as the bound
/// key — exercising the exact `allowed_actions` gate #423/#429 put in consensus,
/// with no mesh, dispatch pool, or provisioner.
#[test]
fn an_out_of_acl_agent_action_is_refused_at_the_module_layer() {
    let storage = tempfile::tempdir().expect("storage dir");
    // NO --echo-oracle: with no worker the run never settles, so it stays in
    // flight while we bind a session and act through it.
    let sim = Sim::spawn(storage.path(), &["--auto"]);

    // an agent granted NOTHING: allowed_actions is empty.
    sim.submit_ok(
        "agent",
        serde_json::json!({ "register_agent": {
            "agent_id": "scribe",
            "display_name": "Scribe",
            "capability": "text",
            "allowed_actions": [],
        }}),
        Some("owner"),
    );
    // a channel with an anchor message the run pins its context to.
    sim.submit_ok("chat", create_channel("room", "Room"), None);
    sim.submit_ok("chat", post_message("room", "m-1", "please help"), None);

    // an explicit run of the agent against the anchor — creates the pending run
    // and its dispatch/saga. the sim announces no provider pool, so the saga's
    // attempt stays UNASSIGNED, claimable by the first Accept.
    sim.submit_ok(
        "runs",
        serde_json::json!({ "request_run": { "agent_id": "scribe", "channel_id": "room", "anchor_seq": 1 } }),
        Some("requester"),
    );
    let pending = sim.query("runs", serde_json::json!("pending_runs"));
    let entry = &pending["pending_runs"][0];
    let run_id = entry["run_id"]
        .as_str()
        .expect("pending run id")
        .to_string();
    let dispatch_id = entry["dispatch_id"]
        .as_str()
        .expect("dispatch id")
        .to_string();
    // the saga id the dispatch minted for this run (dispatch's own id scheme:
    // `dispatch\x1f{receiver}\x1f{dispatch_id}`).
    let saga_id = format!("dispatch\u{1f}runs\u{1f}{dispatch_id}");

    // claim the run's execution lease: the first Accept in consensus order wins
    // the assignee. over /v1/submit the sim stamps our named origin verbatim, so
    // the "executing node" is a key we choose.
    let node = "n".repeat(32);
    sim.submit_ok(
        "saga",
        serde_json::json!({ "accept": { "saga_id": saga_id, "attempt": 0 } }),
        Some(&node),
    );

    // the lease-holder binds an ephemeral session key to the run. the key never
    // has to be a real ed25519 point here — the module only checks its LENGTH on
    // open and byte-equality on act — so a 32-byte ASCII stand-in lets us name
    // the same bytes as a UTF-8 origin below.
    sim.submit_ok(
        "runs",
        serde_json::json!({ "open_agent_session": { "run_id": run_id, "session_key": vec![b's'; 32] } }),
        Some(&node),
    );

    // the bound key acts mid-run: a chat post the agent was never granted. the
    // origin IS the session key's bytes, so the module trusts the authorship and
    // reaches the shared validator — which refuses the ungranted action.
    let error = sim.submit_rejected(
        "runs",
        serde_json::json!({ "agent_action": {
            "run_id": run_id,
            "action": { "post_message": { "channel_id": "room", "text": "progress update" } },
        }}),
        Some(&"s".repeat(32)),
    );
    assert!(
        error.contains("scribe is not allowed to chat.post_message"),
        "the out-of-ACL action is refused at the module layer: {error}"
    );
}
