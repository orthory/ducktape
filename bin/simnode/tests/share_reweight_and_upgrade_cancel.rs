//! two more `--with-valset` governance follow-ups the campaign left queued, both
//! driven deterministically over noded's exact /v1 wire (no live roll):
//!
//! - **SetShares mid-mode** (the last account-share surface unexercised): while
//!   share mode is ON, a passing proposal RE-WEIGHTS an account, and a passing
//!   `shares = 0` REMOVES one. the load-bearing invariant is the electorate
//!   freeze under change: a proposal opened AFTER the changes snapshots the NEW
//!   weights, while an in-flight proposal opened BEFORE keeps its OLD frozen
//!   electorate — a removed account still carries its frozen ballot there, yet
//!   cannot touch a later proposal.
//! - **CancelUpgrade** (the upgrade slot's third door): a governance
//!   `CancelUpgrade` clears a scheduled-but-not-armed upgrade BEFORE its boundary
//!   and frees the at-most-one slot, which immediately accepts (and arms) a fresh
//!   schedule. this is the operator's explicit clear — distinct from the already-
//!   pinned abort path (governance_scenarios.rs), where the system-injected
//!   boundary `Advance` clears an incomplete-readiness upgrade AT its activation.
//!   the boundary block itself is too late on BOTH block paths, with different
//!   refusals: the single-op drain queues the `Advance` ahead of the root op's
//!   follow-ups (the cancel finds an empty slot; the block aborts, rolling the
//!   `Advance` back), while the batch engine drains a member's follow-ups
//!   before its step-4 injections (the cancel finds the slot occupied — the
//!   too-late arm — and the block's own `Advance` then settles it).
//!
//! ceremony mirrors share_governance.rs / governance_scenarios.rs: validators
//! author through the `hex:` origin escape (governance keys ballots on
//! `Origin::External`), account voters through the 32-byte node key their Identity
//! bind seated (governance resolves node→account, so the account is the principal).

mod harness;

use commonware_cryptography::Signer as _;
use commonware_cryptography::ed25519::PrivateKey;
use harness::{Sim, ed_bind_auth};
use identity::bind_preimage;
use serde_json::{Value, json};
use std::path::Path;

type Ed = PrivateKey;

// the two account nodes: 32-byte ASCII node keys (printable — no `hex:` escape).
const NODE_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const NODE_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

// ── ceremony helpers (mirror share_governance.rs) ───────

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// a validator's submit origin: the `hex:` escape resolving to its raw pubkey.
fn origin(k: &Ed) -> String {
    format!("hex:{}", hex(k.public_key().as_ref()))
}

/// spawn an `--auto --with-valset` sim seeded with three genesis validators.
fn governed(storage: &Path) -> (Sim, Vec<Ed>) {
    let validators: Vec<Ed> = (1..=3u64).map(Ed::from_seed).collect();
    let hexes = validators
        .iter()
        .map(|k| hex(k.public_key().as_ref()))
        .collect::<Vec<_>>()
        .join(",");
    let sim = Sim::spawn(storage, &["--auto", "--with-valset", &hexes]);
    (sim, validators)
}

/// found an Identity account: `key` consents to binding `node` at nonce 0 (the
/// sim's identity has empty chain_id). the account_id IS `key`'s pubkey.
fn bind_account(sim: &Sim, key: &Ed, node: &str) {
    let preimage = bind_preimage("", node.as_bytes(), 0);
    sim.submit_ok(
        "identity",
        json!({ "bind_node": { "authorizer": ed_bind_auth(key, &preimage) } }),
        Some(node),
    );
}

fn propose(sim: &Sim, origin: &str, id: &str, action: Value, period: u64) {
    sim.submit_ok(
        "governance",
        json!({ "propose": { "proposal_id": id, "action": action, "voting_period": period } }),
        Some(origin),
    );
}

fn vote(sim: &Sim, origin: &str, id: &str, approve: bool) {
    sim.submit_ok(
        "governance",
        json!({ "vote": { "proposal_id": id, "approve": approve } }),
        Some(origin),
    );
}

fn execute_ok(sim: &Sim, origin: &str, id: &str) -> Value {
    sim.submit_ok(
        "governance",
        json!({ "execute": { "proposal_id": id } }),
        Some(origin),
    )
}

fn execute_rejected(sim: &Sim, origin: &str, id: &str) -> String {
    sim.submit_rejected(
        "governance",
        json!({ "execute": { "proposal_id": id } }),
        Some(origin),
    )
}

/// propose (origins[0]) → every origin votes yes → execute (origins[0]).
fn pass(sim: &Sim, origins: &[&str], id: &str, action: Value, period: u64) -> Value {
    propose(sim, origins[0], id, action, period);
    for o in origins {
        vote(sim, o, id, true);
    }
    execute_ok(sim, origins[0], id)
}

fn proposal(sim: &Sim, id: &str) -> Value {
    sim.query("governance", json!({ "proposal": { "proposal_id": id } }))["proposal"].clone()
}

fn shares_view(sim: &Sim) -> Value {
    sim.query("governance", json!("shares"))["shares"].clone()
}

fn upgrade_status(sim: &Sim) -> Value {
    sim.query("lifecycle", json!("upgrade_status"))["upgrade_status"].clone()
}

fn signal(name: &str, to_version: u64) -> Value {
    json!({ "upgrade_ready": { "name": name, "to_version": to_version, "commitment": null } })
}

fn schedule(name: &str, activation_height: u64, to_version: u64) -> Value {
    json!({ "schedule_upgrade": { "name": name, "activation_height": activation_height, "to_version": to_version } })
}

fn cancel(name: &str) -> Value {
    json!({ "cancel_upgrade": { "name": name } })
}

/// one filler block — advances the logical clock without touching membership.
fn filler(sim: &Sim) {
    sim.submit_ok(
        "inbox",
        json!({ "deliver": { "member": "filler", "kind": "tick", "body": "walk" } }),
        Some("filler"),
    );
}

fn height(sim: &Sim) -> u64 {
    sim.status()["height"].as_u64().expect("height")
}

fn walk_to(sim: &Sim, target: u64) {
    while height(sim) < target {
        filler(sim);
    }
}

/// a governed sim with two share-holding Identity accounts adopted (A:2, B:1) —
/// which ALSO enables account-share mode. returns the account founding keys.
fn share_governed(storage: &Path) -> (Sim, Ed, Ed) {
    let (sim, validators) = governed(storage);
    let key_a = Ed::from_seed(10);
    let key_b = Ed::from_seed(11);
    bind_account(&sim, &key_a, NODE_A);
    bind_account(&sim, &key_b, NODE_B);
    let v0 = origin(&validators[0]);
    let v1 = origin(&validators[1]);
    pass(
        &sim,
        &[&v0, &v1],
        "adopt",
        json!({ "adopt_shares": { "allocations": [
            { "account_id": key_a.public_key().as_ref().to_vec(), "shares": 2 },
            { "account_id": key_b.public_key().as_ref().to_vec(), "shares": 1 },
        ]}}),
        1_000_000,
    );
    (sim, key_a, key_b)
}

/// the account's frozen power on a proposal view, if present in its electorate.
fn power_of(proposal: &Value, account: &[u8]) -> Option<u64> {
    proposal["electorate"].as_array()?.iter().find_map(|pair| {
        let pair = pair.as_array()?;
        (pair[0] == json!(account.to_vec())).then(|| pair[1].as_u64())?
    })
}

fn set_shares(account: &[u8], shares: u64) -> Value {
    json!({ "set_shares": { "account_id": account.to_vec(), "shares": shares } })
}

// ── SetShares mid-mode: re-weight, remove, and the freeze ─

/// while share mode is on, `SetShares` re-weights an account and `shares = 0`
/// removes one — and the change lands in a LATER-frozen electorate but never in
/// an in-flight one. the account removed from the registry keeps its frozen
/// ballot on the proposal opened before the removal, yet is a stranger to the one
/// opened after.
#[test]
fn set_shares_reweights_and_removes_and_only_a_later_frozen_electorate_sees_it() {
    let storage = tempfile::tempdir().expect("storage dir");
    let (sim, key_a, key_b) = share_governed(storage.path());
    let acct_a = key_a.public_key().as_ref().to_vec();
    let acct_b = key_b.public_key().as_ref().to_vec();

    // an IN-FLIGHT proposal opened while the registry is {A:2, B:1}: it freezes
    // that account electorate (total 3) and is left open.
    propose(
        &sim,
        NODE_A,
        "in-flight",
        json!({ "signal": { "text": "before the reweight" } }),
        1_000_000,
    );
    let before = proposal(&sim, "in-flight");
    assert_eq!(before["voter_kind"], "account", "frozen to account ballots");
    assert_eq!(
        power_of(&before, &acct_a),
        Some(2),
        "A frozen at 2: {before}"
    );
    assert_eq!(
        power_of(&before, &acct_b),
        Some(1),
        "B frozen at 1: {before}"
    );

    // RE-WEIGHT A from 2 to 5. structural share actions need ceil(2n/3); at total
    // 3 that is 2, which A's 2 shares alone meet — A passes it. the account
    // authors, and the mode does NOT change (SetShares only touches the registry).
    pass(
        &sim,
        &[NODE_A],
        "reweight-a",
        set_shares(&acct_a, 5),
        1_000_000,
    );
    let view = shares_view(&sim);
    assert_eq!(view["active"], true, "still share mode: {view}");
    assert_eq!(view["total"], 6, "A:5 + B:1 = 6: {view}");

    // REMOVE B via shares = 0. its own frozen electorate is {A:5, B:1} total 6,
    // ceil(2*6/3)=4 — A's 5 shares carry it alone.
    pass(
        &sim,
        &[NODE_A],
        "remove-b",
        set_shares(&acct_b, 0),
        1_000_000,
    );
    let view = shares_view(&sim);
    assert_eq!(view["total"], 5, "only A:5 remains: {view}");
    let allocations = view["allocations"].as_array().expect("allocations");
    assert_eq!(allocations.len(), 1, "B left the registry: {view}");
    assert_eq!(allocations[0]["account_id"], json!(acct_a));
    assert_eq!(allocations[0]["shares"], 5);

    // a LATER-frozen proposal snapshots the NEW weights: {A:5}, B absent, and the
    // Signal quorum is ceil(5/2)=3.
    propose(
        &sim,
        NODE_A,
        "after",
        json!({ "signal": { "text": "after the changes" } }),
        1_000_000,
    );
    let after = proposal(&sim, "after");
    assert_eq!(power_of(&after, &acct_a), Some(5), "A now 5: {after}");
    assert_eq!(
        power_of(&after, &acct_b),
        None,
        "B gone from the electorate: {after}"
    );
    assert_eq!(
        after["voting_rule"]["participating_majority"]["quorum"], 3,
        "ceil(5/2) over the new total: {after}"
    );

    // THE FREEZE INVARIANT: the in-flight proposal STILL carries {A:2, B:1}. the
    // removed account B can still cast its FROZEN ballot there…
    let still = proposal(&sim, "in-flight");
    assert_eq!(
        power_of(&still, &acct_a),
        Some(2),
        "in-flight A unchanged: {still}"
    );
    assert_eq!(
        power_of(&still, &acct_b),
        Some(1),
        "in-flight B unchanged: {still}"
    );
    vote(&sim, NODE_B, "in-flight", true); // accepted: B is in the frozen electorate

    // …but B is a stranger to the later proposal it never belonged to.
    let error = sim.submit_rejected(
        "governance",
        json!({ "vote": { "proposal_id": "after", "approve": true } }),
        Some(NODE_B),
    );
    assert!(
        error.contains("submitter is not a member of this proposal's frozen electorate"),
        "B cannot vote on a proposal frozen after its removal: {error}"
    );

    // and the in-flight proposal settles by its FROZEN rule: A(2) + B(1) yes meets
    // quorum 2, so it passes under the old electorate the changes never touched.
    vote(&sim, NODE_A, "in-flight", true);
    execute_ok(&sim, NODE_A, "in-flight");
    assert_eq!(
        proposal(&sim, "in-flight")["status"],
        "passed",
        "the in-flight proposal settled by its frozen {{A:2,B:1}} electorate"
    );
}

// ── CancelUpgrade: clear before the boundary, free the slot ─

/// a governance `CancelUpgrade` clears a scheduled-but-not-armed upgrade before
/// its activation and frees the at-most-one slot immediately — a fresh, higher
/// schedule then takes the slot and arms in turn. (contrast: the abort path in
/// governance_scenarios.rs clears an incomplete upgrade via the system `Advance`
/// AT the boundary; this clears it EARLY via an explicit governance action.) a
/// cancel that names no live pending is refused by the module when its follow-up
/// drains, aborting the execute block with the verbatim reason.
#[test]
fn a_governance_cancel_clears_a_pending_upgrade_early_and_frees_the_slot() {
    let storage = tempfile::tempdir().expect("storage dir");
    let (sim, validators) = governed(storage.path());
    let v0 = origin(&validators[0]);
    let v1 = origin(&validators[1]);

    // schedule v2 with a comfortably-future activation, then land ONE readiness
    // signal (so cancel is proven to clear readiness too, not just the slot).
    let activation = height(&sim) + 30;
    pass(
        &sim,
        &[&v0, &v1],
        "sched-v2",
        schedule("v2", activation, 2),
        1_000_000,
    );
    sim.submit_ok("lifecycle", signal("v2", 2), Some(v0.as_str()));
    let st = upgrade_status(&sim);
    assert!(st["pending"].is_object(), "v2 is pending: {st}");
    assert_eq!(st["ready_count"], 1, "one validator signaled: {st}");

    // CANCEL v2 through governance, WELL before the boundary. the emitted
    // LifecycleMsg::CancelUpgrade drains under Origin::Module(governance) and clears both
    // the pending slot and the residual readiness.
    assert!(height(&sim) < activation, "still before the boundary");
    pass(&sim, &[&v0, &v1], "cancel-v2", cancel("v2"), 1_000_000);
    let st = upgrade_status(&sim);
    assert!(st["pending"].is_null(), "the pending slot cleared: {st}");
    assert_eq!(
        st["current_version"], 0,
        "no arm — the version is untouched: {st}"
    );
    assert_eq!(st["ready_count"], 0, "the residual readiness cleared: {st}");

    // the FREED slot immediately takes a fresh, higher schedule and arms it —
    // proving the cancel genuinely released the at-most-one slot for reuse.
    let activation2 = height(&sim) + 30;
    pass(
        &sim,
        &[&v0, &v1],
        "sched-v3",
        schedule("v3", activation2, 3),
        1_000_000,
    );
    assert!(
        upgrade_status(&sim)["pending"].is_object(),
        "the freed slot took v3"
    );
    for v in &validators {
        sim.submit_ok("lifecycle", signal("v3", 3), Some(origin(v).as_str()));
    }
    walk_to(&sim, activation2);
    let st = upgrade_status(&sim);
    assert_eq!(
        st["current_version"], 3,
        "the rescheduled upgrade armed: {st}"
    );
    assert!(st["pending"].is_null(), "and freed the slot again: {st}");

    // a cancel that matches NO live pending (nothing is scheduled now) is
    // authorized by governance but refused by the upgrade module when the emitted
    // Cancel drains — aborting the execute block with the module's exact string.
    propose(&sim, v0.as_str(), "cancel-ghost", cancel("v3"), 1_000_000);
    vote(&sim, v0.as_str(), "cancel-ghost", true);
    vote(&sim, v1.as_str(), "cancel-ghost", true);
    let error = execute_rejected(&sim, v0.as_str(), "cancel-ghost");
    assert!(
        error.contains("no matching pending upgrade to cancel"),
        "cancelling nothing is refused by the module: {error}"
    );
}

/// the boundary block is TOO LATE to cancel — and WHICH refusal fires exposes
/// a real reactor-ordering seam between the two block paths:
///
/// - single-op drain: the queue is `[root, Advance, …]`, and the root's
///   follow-ups push BEHIND the injections — so the governance cancel's
///   emitted `LifecycleMsg::CancelUpgrade` drains AFTER the `Advance` already cleared
///   the slot, and dies with "no matching pending upgrade to cancel". the
///   whole block aborts, rolling the `Advance` back with it: state is
///   untouched, the boundary has not passed.
/// - batch engine (`submit_block`): each member drains root+follow-ups to
///   completion FIRST; the injections run once, after every member (step 4).
///   so the same cancel reaches the module while the slot is still occupied
///   at `height == activation` — past the `height < activation_height`
///   window — and dies with "cannot cancel: activation height already
///   reached". the member is rejected and isolated, the block still commits,
///   and its step-4 `Advance` settles the incomplete upgrade in the same
///   breath: slot cleared, version untouched.
#[test]
fn a_cancel_landing_at_the_activation_boundary_is_too_late_on_both_block_paths() {
    let storage = tempfile::tempdir().expect("storage dir");
    let (sim, validators) = governed(storage.path());
    let v0 = origin(&validators[0]);
    let v1 = origin(&validators[1]);

    // schedule v2 with enough lead to fit the cancel ballot before the boundary.
    let activation = height(&sim) + 20;
    pass(
        &sim,
        &[&v0, &v1],
        "sched-v2",
        schedule("v2", activation, 2),
        1_000_000,
    );

    // pass the cancel ballot but hold its EXECUTE: it must land exactly at the
    // boundary, so walk the chain to activation - 1 first.
    propose(&sim, v0.as_str(), "cancel-late", cancel("v2"), 1_000_000);
    vote(&sim, v0.as_str(), "cancel-late", true);
    vote(&sim, v1.as_str(), "cancel-late", true);
    walk_to(&sim, activation - 1);

    // SINGLE-OP LANE: the boundary block's queue runs root → Advance →
    // follow-ups, so the cancel drains after the Advance emptied the slot. the
    // abort rolls the whole block back — Advance included — so nothing moved.
    let error = execute_rejected(&sim, v0.as_str(), "cancel-late");
    assert!(
        error.contains("no matching pending upgrade to cancel"),
        "on the single-op lane the Advance outruns the follow-up: {error}"
    );
    assert_eq!(
        height(&sim),
        activation - 1,
        "the aborted execute minted no block"
    );
    let st = upgrade_status(&sim);
    assert!(
        st["pending"].is_object(),
        "the rolled-back Advance left v2 pending: {st}"
    );

    // BATCH LANE: a member's follow-ups drain BEFORE the step-4 injections, so
    // the same cancel now finds the slot occupied AT the boundary — the
    // module's too-late arm. the member is rejected and isolated; the block
    // commits; its own step-4 Advance settles the incomplete upgrade.
    let (code, reply) = sim.peer_batch(json!([{
        "target": "governance",
        "payload": { "execute": { "proposal_id": "cancel-late" } },
        "origin": v0,
    }]));
    assert_eq!(code, 200, "the batch block commits: {reply}");
    assert_eq!(
        reply["members"][0]["disposition"], "rejected",
        "the boundary cancel member is rejected: {reply}"
    );
    assert!(
        reply["members"][0]["rejection"]
            .as_str()
            .unwrap_or_default()
            .contains("cannot cancel: activation height already reached"),
        "the batch lane reaches the too-late arm: {reply}"
    );
    assert_eq!(
        reply["height"],
        json!(activation),
        "the batch block IS the boundary block: {reply}"
    );
    let st = upgrade_status(&sim);
    assert!(
        st["pending"].is_null(),
        "the same block's step-4 Advance settled the slot: {st}"
    );
    assert_eq!(
        st["current_version"], 0,
        "incomplete readiness aborts, never arms: {st}"
    );
}
