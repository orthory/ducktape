//! reactor-seam scenarios: the block-COMPOSITION seams the real host and the
//! sim driver own, not any one module's semantics. every block here is
//! composed by the REAL `host::Host` (route → drain FIFO under
//! `MAX_DISPATCHES` → commit-or-abort → recompose root-hash), driven over
//! noded's exact /v1 wire plus the /sim control lane. what these pin:
//!
//! - the self-retriggering rule: an automation that posts into its own hooked
//!   channel is the natural infinite loop — WHAT stops it (the module's own
//!   user-author guard, ahead of the host's dispatch budget).
//! - oracle-drain discipline: worker follow-ups queue behind their triggering
//!   block and drain ONE per `/sim/step`, never coalescing, undisturbed by a
//!   peer block wedged between two drains.
//! - saga's callback-poison wedge cap: a callback that would wedge a saga at
//!   Pending forever is rejected at trigger time, minting no saga and no block.
//! - whole-registry determinism: one script over (almost) every registered
//!   module walks byte-identical root-hashes on two fresh dirs, and through the
//!   auto and stepped commit paths alike — the standing guard against
//!   HashMap-iteration / wall-clock nondeterminism creeping into any module.
//! - restart/resume: the sim's own height-resume-above-the-watermark path
//!   (sim-only code, otherwise untested) survives a child kill + respawn on
//!   the same storage dir with state and height intact.

mod harness;

use commonware_cryptography::Signer as _;
use harness::{Sim, create_channel, ed_bind_auth, post_message};
use identity::bind_preimage;
use serde_json::{Value, json};

type Ed = commonware_cryptography::ed25519::PrivateKey;

// ── shared wire builders ────────────────────────────────

/// saga's id space is namespaced per trigger origin, and the sim stamps the
/// caller-named origin verbatim as `Origin::External(name.as_bytes())` — so a
/// caller can only ever trigger inside its own namespace.
fn sid(origin: &str, id: &str) -> String {
    saga::namespaced_id(&sdk::Origin::External(origin.as_bytes().to_vec()), id)
}

/// a fire-and-forget saga trigger carrying `spec` as its opaque work spec. the
/// echo worker (behind `--echo-oracle`) claims the emitted `WorkerRequest`
/// only when the spec decodes as a dispatch `WorkSpec`, so A3 hands it a real
/// one; A4/A5 reach the saga before the spec is ever inspected.
fn saga_trigger(saga_id: &str, spec: &[u8]) -> Value {
    json!({ "trigger": {
        "saga_id": saga_id,
        "spec": spec,
        "reply_to": null,
        "reply_payload": [],
        "deadline": null,
        "max_attempts": 1,
        "lease_views": null,
        "capability": null,
        "demands": {},
        "pinned_assignee": null,
    }})
}

/// a dispatch `WorkSpec` the echo worker recognizes (`kind == WORK_SPEC_KIND`).
fn echo_work_spec() -> Vec<u8> {
    dispatch::encode_work_spec(&dispatch::WorkSpec {
        kind: dispatch::WORK_SPEC_KIND.into(),
        dispatch_id: "d1".into(),
        capability: "echo".into(),
        payload: b"hi".to_vec(),
        demands: Default::default(),
        // `Queue` — the wait-for-capacity behavior this seam exercises.
        admission: Default::default(),
    })
}

// ── A1: the self-retriggering rule ──────────────────────

/// an automations rule whose action posts back into its own hooked channel is
/// the textbook infinite loop. WHAT stops it, from the wire, is the module's
/// OWN loop-prevention guard (a rule fires only on `AuthorRef::User` posts, and
/// its follow-up post is module-authored), not the host's `MAX_DISPATCHES`
/// budget — the raw `Error::BudgetExceeded` path stays unreachable through this
/// route. the guard swallows the re-entry silently (before any run record), so
/// the whole cascade commits as ONE atomic block.
#[test]
fn a_rule_posting_into_its_own_hooked_channel_fires_once_not_forever() {
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &["--auto"]);
    // the operator owns the channel — hook registration is the owner's call.
    sim.submit_ok("chat", create_channel("loop", "Loop"), Some("operator"));
    sim.submit_ok(
        "chat",
        json!({ "register_hook": { "channel_id": "loop", "module_id": "automations" } }),
        Some("operator"),
    );
    // the loop, wired deliberately: the trigger matches "ping", and the action
    // posts "ping again" — which contains "ping" — straight back into "loop".
    sim.submit_ok(
        "automations",
        json!({ "create_rule": {
            "rule_id": "echo",
            "trigger": { "channel_id": "loop", "mention": null, "text_contains": "ping" },
            "action": { "post_message": { "channel_id": "loop", "template": "ping again" } },
        }}),
        Some("operator"),
    );

    let before = sim.status();
    let before_hash = before["root_hash"].as_str().expect("root hash").to_string();
    let before_height = before["height"].as_u64().expect("height");

    // a USER post fires the rule; the rule's own reply is MODULE-authored, so
    // it never re-triggers. the fire rides the triggering block — no follow-up.
    let receipt = sim.submit_ok("chat", post_message("loop", "u1", "ping"), Some("user"));
    let fired_at = receipt["height"].as_u64().expect("receipt height");
    assert_eq!(fired_at, before_height + 1, "exactly one block committed");
    assert_eq!(
        sim.status()["height"].as_u64(),
        Some(fired_at),
        "the rule fire rides the triggering block — no runaway follow-up blocks"
    );
    // the block COMMITTED — the guard prevented the loop, so there was never a
    // BudgetExceeded abort (which would have rolled the root-hash back unchanged).
    assert_ne!(
        sim.status()["root_hash"].as_str().map(str::to_string),
        Some(before_hash),
        "the triggering block committed atomically, not aborted"
    );

    // the rule fired EXACTLY once. the module-authored re-post produced NO run
    // record — the guard returns before any record is staged.
    let history = sim.query(
        "automations",
        json!({ "run_history": { "rule_id": "echo", "limit": 10 } }),
    );
    let records = history["history"].as_array().expect("history array");
    assert_eq!(
        records.len(),
        1,
        "the guard stops the loop at one fire: {history}"
    );
    assert_eq!(
        records[0]["action_ok"], true,
        "the one fire emitted its action"
    );

    // the channel holds exactly two messages: the user's ping and the SINGLE
    // module-authored reply — the guard, not the budget, capped the cascade.
    let msgs = sim.query(
        "chat",
        json!({ "messages_range": { "channel_id": "loop", "from_seq": 0, "limit": 100 } }),
    );
    assert_eq!(
        msgs["messages"].as_array().map(Vec::len),
        Some(2),
        "one fire, one reply, no cascade: {msgs}"
    );
}

// ── A3: oracle-drain discipline ─────────────────────────

/// with the echo worker installed in HOLD mode, each committed saga trigger's
/// `WorkerRequest` effect is claimed by the worker, whose `OracleResult`
/// follow-up parks in the oracle queue. two follow-ups queue without coalescing;
/// each `/sim/step` commits EXACTLY ONE as its own `oracle`-kind block; and a
/// peer block wedged between two drains commits fine without disturbing queue
/// order (`oracle_queued` tracks the count throughout).
#[test]
fn each_step_drains_exactly_one_queued_oracle_follow_up_in_order() {
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &["--echo-oracle"]); // hold mode + echo worker
    let spec = echo_work_spec();

    // two peer blocks commit two triggers; each enqueues one worker follow-up.
    let b1 = sim.peer_block("saga", saga_trigger(&sid("peer", "s1"), &spec), "peer");
    assert_eq!(b1["height"], 1, "first trigger committed: {b1}");
    assert_eq!(sim.sim_state()["oracle_queued"], 1, "one follow-up queued");
    let b2 = sim.peer_block("saga", saga_trigger(&sid("peer", "s2"), &spec), "peer");
    assert_eq!(b2["height"], 2, "second trigger committed: {b2}");
    assert_eq!(
        sim.sim_state()["oracle_queued"],
        2,
        "follow-ups queue behind each other — they never coalesce"
    );

    // a step drains EXACTLY ONE follow-up as its own oracle block.
    let r = sim.step();
    assert_eq!(
        r["committed"]["kind"], "oracle",
        "step drained an oracle follow-up: {r}"
    );
    assert_eq!(r["committed"]["height"], 3);
    assert_eq!(
        sim.sim_state()["oracle_queued"],
        1,
        "one drained, one still queued"
    );

    // a peer block wedged between two drains commits fine and leaves the queue
    // — and its order — untouched.
    let wedge = sim.peer_block("chat", create_channel("aside", "Aside"), "rival");
    assert_eq!(
        wedge["height"], 4,
        "the wedge committed ahead of the queue: {wedge}"
    );
    assert_eq!(
        sim.sim_state()["oracle_queued"],
        1,
        "the peer wedge did not disturb the parked follow-up"
    );

    // the second follow-up drains next, still one-per-step, still its own block.
    let r = sim.step();
    assert_eq!(r["committed"]["kind"], "oracle");
    assert_eq!(r["committed"]["height"], 5);
    assert_eq!(sim.sim_state()["oracle_queued"], 0, "queue drained");

    // nothing left: a further step commits nothing (both queues empty).
    let r = sim.step();
    assert!(
        r["committed"].is_null(),
        "an empty drain mints no block: {r}"
    );

    // both echo results actually landed — the sagas are Done, in submit order.
    for id in [sid("peer", "s1"), sid("peer", "s2")] {
        let saga = sim.query("saga", json!({ "get": { "saga_id": id } }));
        assert_eq!(
            saga["saga"]["status"], "done",
            "saga {id} completed: {saga}"
        );
    }
}

// ── A4: the saga callback-poison wedge cap ──────────────

/// a saga trigger whose `reply_to` would abort every future terminal block —
/// aimed at the saga module itself (which cannot decode its own callback) or at
/// an unregistered module — is rejected AT TRIGGER TIME, so it can never wedge
/// a saga at Pending forever. a rejected trigger mints no saga and no block.
/// nobody has exercised this cap over the wire before.
#[test]
fn a_callback_that_would_wedge_a_saga_is_rejected_at_trigger_time() {
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &["--auto"]);

    let with_reply = |saga_id: &str, reply_to: &str| {
        json!({ "trigger": {
            "saga_id": saga_id,
            "spec": [1, 2, 3],
            "reply_to": reply_to,
            "reply_payload": [],
            "deadline": null,
            "max_attempts": 1,
            "lease_views": null,
            "capability": null,
            "demands": {},
            "pinned_assignee": null,
        }})
    };

    // a callback aimed at the saga module itself: rejected.
    let error = sim.submit_rejected(
        "saga",
        with_reply(&sid("owner", "s1"), "saga"),
        Some("owner"),
    );
    assert!(
        error.contains("reply_to must not target the saga module itself"),
        "self-targeting callback: {error}"
    );
    // a callback aimed at a module that was never registered: rejected.
    let error = sim.submit_rejected(
        "saga",
        with_reply(&sid("owner", "s2"), "ghost"),
        Some("owner"),
    );
    assert!(
        error.contains("reply_to targets unknown module ghost"),
        "unknown-module callback: {error}"
    );

    // each rejected trigger JOURNALS a block now (validator parity — the op
    // rides the drain and seals its height with a `rejected` row), so two
    // rejects advanced the height by two. what stays true is the SEMANTIC guard:
    // no saga was minted, so nothing is wedged at Pending.
    assert_eq!(
        sim.status()["height"],
        2,
        "each rejected trigger seals its own block (validator parity)"
    );
    // …and no wedged saga exists to be stuck at Pending.
    for id in [sid("owner", "s1"), sid("owner", "s2")] {
        let saga = sim.query("saga", json!({ "get": { "saga_id": id } }));
        assert!(saga["saga"].is_null(), "no wedged saga {id} exists: {saga}");
    }
}

// ── A5: whole-registry determinism sweep ────────────────

/// one op per registered module the sim can reach over /v1/submit, in a fixed
/// order. `chat` cascades a `TagEvent` into `tagging`; `runs` subscribes through
/// `tagging` too; `identity`'s bind founds the account `duckdns` then claims a
/// handle on, and `gateway` publishes a member-signed route from that same
/// just-bound publisher node. no op here produces a CROSS-block follow-up (the
/// fire-and-forget saga trigger's effect is dropped — no worker — and no
/// dispatch result lands), so auto and stepped commit the identical block per op.
///
/// TWO modules stay absent (see the report): `files` (duckfs takes a BINARY
/// op-frame, not a JSON submit) and `forge` (a libgit2-on-disk module whose
/// determinism is repo-internal — its own e2e owns it, and it stays out of this
/// cross-dir root-hash-equality assertion). `gateway` was the third until its
/// SetRoute MemberAuthorization ceremony joined the sweep here — a route the
/// account's founding Ed25519 member signs, keyed on the just-bound node.
fn sweep_script() -> Vec<(&'static str, Value, Option<String>)> {
    let node = "n".repeat(32);
    let key = Ed::from_seed(7);
    let preimage = bind_preimage("", node.as_bytes(), 0);
    let spec = echo_work_spec();
    vec![
        // chat — and, via the post's TagEvent, tagging.
        (
            "chat",
            create_channel("general", "General"),
            Some("owner".into()),
        ),
        (
            "chat",
            post_message("general", "m1", "hello sweep"),
            Some("owner".into()),
        ),
        // tasks
        (
            "tasks",
            json!({ "task": { "create_task": { "task_id": "t1", "title": "sweep" } } }),
            Some("owner".into()),
        ),
        // inbox
        (
            "inbox",
            json!({ "deliver": { "member": "alice", "kind": "note", "body": "hi" } }),
            Some("courier".into()),
        ),
        // the job board (the merged tasks module's `job` arm)
        (
            "tasks",
            json!({ "job": { "submit": { "job_id": "j1", "kind": "build", "spec": "{}" } } }),
            Some("poster".into()),
        ),
        // automations — a rule whose trigger no later post matches (and no hook
        // is registered), so it never fires and never cascades.
        (
            "automations",
            json!({ "create_rule": {
                "rule_id": "r1",
                "trigger": { "channel_id": "general", "mention": null, "text_contains": "zznomatch" },
                "action": { "create_task": { "task_id_prefix": "auto", "title_template": "x" } },
            }}),
            Some("owner".into()),
        ),
        // saga — fire-and-forget: stages a pending saga, emits a WorkerRequest
        // effect no worker claims (dropped). no follow-up in either mode.
        (
            "saga",
            saga_trigger(&sid("owner", "s1"), &spec),
            Some("owner".into()),
        ),
        // dispatch — an external RegisterRecipe (module-origin is only needed
        // for Dispatch itself).
        (
            "dispatch",
            json!({ "register_recipe": {
                "recipe_id": "sum",
                "description": "d",
                "capability": "echo",
                "routing": "rendezvous",
                "output_contract": "text",
                "max_attempts": 1,
                "deadline_views": null,
                "lease_views": null,
            }}),
            Some("owner".into()),
        ),
        // pages
        (
            "pages",
            json!({ "create_page": { "page_id": "p1", "title": "Sweep", "parent": null } }),
            Some("owner".into()),
        ),
        // agent
        (
            "agent",
            json!({ "register_agent": {
                "agent_id": "quackbot",
                "display_name": "Quackbot",
                "capability": "echo",
                "allowed_actions": ["chat.post"],
            }}),
            Some("owner".into()),
        ),
        // runs — watching the channel subscribes through the tagging plane.
        (
            "runs",
            json!({ "watch_channel": { "channel_id": "general", "policy": "mention" } }),
            Some("owner".into()),
        ),
        // identity — bind_node founds the account (deterministic ed25519 consent).
        (
            "identity",
            json!({ "bind_node": { "authorizer": ed_bind_auth(&key, &preimage) } }),
            Some(node.clone()),
        ),
        // duckdns — claim a handle on the just-bound account (32-byte origin).
        (
            "gateway",
            json!({ "set_handle": { "handle": "alice" } }),
            Some(node.clone()),
        ),
        // gateway — the account's founding key signs a route from the just-bound
        // publisher node. the sim wires `Gateway::new(.., None, "local")` (no
        // valset), so the only ceremony is: the publisher node is bound to the
        // account (the identity op above), and a current Ed25519 member signs.
        (
            "gateway",
            gateway_set_route(&key, &node),
            Some(node.clone()),
        ),
    ]
}

/// a gateway `SetRoute` op, member-signed. the route names the account founded
/// by `key`'s bind (its `account_id` is that key's pubkey) and the publisher
/// `node` the identity op bound to it; the `MemberAuthorization` is `key`'s
/// ed25519 signature over the route-signing preimage under `GATEWAY_ROUTE_NS` —
/// the same member-consent shape `ed_bind_auth` builds, keyed on the gateway
/// namespace. deterministic: same seed, same preimage, same signature.
fn gateway_set_route(key: &Ed, node: &str) -> Value {
    let statement = gateway::RouteStatement {
        version: 1,
        chain_id: "local".into(),
        account_id: key.public_key().as_ref().to_vec(),
        name: gateway::RouteName::named("api"),
        publisher_node: node.as_bytes().to_vec(),
        revision: 1,
        route: Some(gateway::RouteDefinition {
            target: gateway::RouteTarget::LoopbackHttp,
            policy: gateway::RoutePolicy {
                audience: gateway::RouteAudience::Owner,
                methods: vec![gateway::RouteMethod::Get],
                max_request_bytes: 0,
                max_response_bytes: 1024,
                allow_authorization: false,
                allow_upgrade: false,
            },
        }),
    };
    let preimage = gateway::route_signing_preimage(&statement).expect("route preimage");
    let signature = key.sign(gateway::GATEWAY_ROUTE_NS, &preimage);
    let authorization = gateway::MemberAuthorization {
        signer: key.public_key().as_ref().to_vec(),
        signature: signature.as_ref().to_vec(),
    };
    serde_json::to_value(gateway::GatewayMsg::SetRoute {
        statement,
        authorization,
    })
    .expect("gateway op serializes")
}

/// run the script through the HOLD path (submit parks, a step commits it),
/// collecting the root-hash committed at every height.
fn run_stepped(script: &[(&'static str, Value, Option<String>)]) -> Vec<String> {
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &[]);
    let mut hashes = Vec::new();
    for (target, payload, origin) in script {
        let pending = sim.submit_in_background(target, payload.clone(), origin.as_deref());
        sim.await_sim_state("held", 1);
        let report = sim.step();
        hashes.push(
            report["committed"]["root_hash"]
                .as_str()
                .unwrap_or_else(|| panic!("{target} did not commit: {report}"))
                .to_string(),
        );
        let (code, reply) = pending.join().expect("submit thread");
        assert_eq!(code, 200, "{target} rejected in the stepped run: {reply}");
    }
    hashes
}

/// run the script through the AUTO path (each submit commits inline); the
/// receipt IS the commit, carrying the same per-height root-hash.
fn run_auto(script: &[(&'static str, Value, Option<String>)]) -> Vec<String> {
    let storage = tempfile::tempdir().expect("storage dir");
    let sim = Sim::spawn(storage.path(), &["--auto"]);
    script
        .iter()
        .map(|(target, payload, origin)| {
            let receipt = sim.submit_ok(target, payload.clone(), origin.as_deref());
            receipt["root_hash"]
                .as_str()
                .unwrap_or_else(|| panic!("{target} receipt has no root hash: {receipt}"))
                .to_string()
        })
        .collect()
}

#[test]
fn the_whole_registry_walks_identical_root_hashes() {
    let script = sweep_script();
    // same script, two fresh storage dirs → byte-identical root-hash at EVERY
    // height. any HashMap-iteration or wall-clock read in any touched module
    // would fork one of these.
    assert_eq!(
        run_stepped(&script),
        run_stepped(&script),
        "the same script on two fresh dirs diverged"
    );
    // the two commit paths (auto's inline drain vs. hold's stepped commit) must
    // walk the identical root-hashes for the identical script — per height, and
    // so also at the final tip.
    assert_eq!(
        run_stepped(&script),
        run_auto(&script),
        "the auto and stepped commit paths diverged"
    );
}

// ── A6: restart / height-resume ─────────────────────────

/// the sim resumes height ABOVE the index watermark (`index.resume_height()`) —
/// sim-only code otherwise untested. kill the child, respawn on the SAME storage
/// dir, and the chain continues: height does not restart at 0, the qmdb-backed
/// module state (and hence the root-hash) survives the boot, a query serves the
/// persisted data, and a new commit lands at watermark+1 with a moved root-hash.
#[test]
fn a_restart_on_the_same_storage_resumes_height_and_state() {
    let storage = tempfile::tempdir().expect("storage dir");

    let (pre_height, pre_hash) = {
        let sim = Sim::spawn(storage.path(), &["--auto"]);
        sim.submit_ok("chat", create_channel("room", "Room"), Some("owner"));
        sim.submit_ok("chat", create_channel("den", "Den"), Some("owner"));
        let status = sim.status();
        (
            status["height"].as_u64().expect("height"),
            status["root_hash"].as_str().expect("root hash").to_string(),
        )
        // the sim (and its child) drops here: Drop kills + waits, releasing the
        // storage before the respawn opens it.
    };
    assert_eq!(pre_height, 2, "two channels committed → height 2");

    // respawn on the SAME dir — a restart, not a fresh genesis.
    let sim = Sim::spawn(storage.path(), &["--auto"]);
    let status = sim.status();
    assert_eq!(
        status["height"].as_u64(),
        Some(pre_height),
        "height resumed above the watermark, not restarted at 0: {status}"
    );
    assert_eq!(
        status["root_hash"].as_str(),
        Some(pre_hash.as_str()),
        "committed module state survived the restart (root-hash byte-identical)"
    );

    // the persisted channels are served from the reloaded qmdb — both of them,
    // by name, so a half-reload cannot pass as a count.
    for (id, name) in [("room", "Room"), ("den", "Den")] {
        let channel = sim
            .channel(id)
            .unwrap_or_else(|| panic!("{id} reloaded from the qmdb-backed store"));
        assert_eq!(
            channel["name"], name,
            "{id} reloaded with its record intact"
        );
    }

    // a NEW commit continues the chain at watermark+1 with a changed root-hash.
    let receipt = sim.submit_ok("chat", create_channel("hall", "Hall"), Some("owner"));
    assert_eq!(
        receipt["height"],
        pre_height + 1,
        "the new block continues the height: {receipt}"
    );
    assert_ne!(
        sim.status()["root_hash"].as_str(),
        Some(pre_hash.as_str()),
        "the new commit moved the root-hash off the resumed root"
    );
}
