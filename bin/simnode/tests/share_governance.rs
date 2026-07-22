//! the account-share governance lane plus two more `--with-valset` follow-ups
//! the four-PR campaign left queued, all driven deterministically over noded's
//! exact /v1 wire (no live roll). three things pinned here:
//!
//! - **account-share governance** (the last governance surface unexercised):
//!   validators adopt shares over two bound Identity accounts, which ALSO flips
//!   the electorate to account mode; a share-mode Signal is then decided by
//!   ParticipatingMajority over ACCOUNT-keyed ballots, honours its deadline, and
//!   — the load-bearing invariant — every proposal FREEZES its electorate, so
//!   one opened in share mode stays account-decided even after the mode flips
//!   back to validator ballots.
//! - **kv under the preset**: kv registers only with `--with-valset`; a
//!   set/get/overwrite round-trip, the no-delete reality, and a cheap
//!   same-script-same-hash determinism pass.
//!
//! every ballot is authored as a real principal: validators through the `hex:`
//! origin escape (governance keys their ballots on `Origin::External`), account
//! voters through the 32-byte node key bound to the account — governance
//! resolves that node to its Identity account (`account_of_node`), so the
//! account, never the node, is the frozen ballot principal.

mod harness;

use commonware_cryptography::Signer as _;
use commonware_cryptography::ed25519::PrivateKey;
use harness::{Sim, ed_bind_auth};
use identity::bind_preimage;
use serde_json::{Value, json};
use std::path::Path;

type Ed = PrivateKey;

// the two account nodes: 32-byte ASCII node keys (bind origins AND account-mode
// ballot origins — printable, so no `hex:` escape is needed for them).
const NODE_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const NODE_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

// ── ceremony helpers (mirrors governance_scenarios.rs) ──

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

/// a governed sim with two share-holding Identity accounts adopted (A:2, B:1)
/// — which ALSO enables account-share mode. returns the account founding keys.
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

// ── account-share governance: adopt, decide, honour the deadline ─

/// adopting shares configures the registry AND enables share mode in one action;
/// a share-mode Signal is then decided by ParticipatingMajority over ACCOUNT
/// ballots — the majority holder passes it early, and a lone minority ballot
/// cannot decide before the deadline, settling rejected at it.
#[test]
fn account_shares_adopt_enable_and_decide_by_participating_majority() {
    let storage = tempfile::tempdir().expect("storage dir");
    let (sim, key_a, _key_b) = share_governed(storage.path());
    let acct_a = key_a.public_key().as_ref().to_vec();

    // adoption already switched the electorate on — the registry is active with
    // total power 3.
    let shares = shares_view(&sim);
    assert_eq!(
        shares["active"], true,
        "adoption enabled share mode: {shares}"
    );
    assert_eq!(shares["total"], 3, "total power A:2 + B:1: {shares}");

    // …so an explicit SetShareMode(true) is now a no-op switch, refused at the
    // door. it must be PROPOSED by an account holder — in share mode even a
    // validator can no longer open a ballot (the electorate is now accounts).
    // (AdoptShares enabling share mode is the reality to pin — the doc reads as
    // if the switch were separate.)
    let error = sim.submit_rejected(
        "governance",
        json!({ "propose": { "proposal_id": "redundant", "voting_period": 1000, "action": {
            "set_share_mode": { "enabled": true }
        }}}),
        Some(NODE_A),
    );
    assert!(
        error.contains("governance is already using the requested voting mode"),
        "adopt already enabled share mode: {error}"
    );

    // a share-mode Signal, proposed by account A's node. the proposal FREEZES an
    // account electorate under the ParticipatingMajority rule (quorum ceil(3/2)=2).
    propose(
        &sim,
        NODE_A,
        "ship",
        json!({ "signal": { "text": "ship it" } }),
        1_000_000,
    );
    let view = proposal(&sim, "ship");
    assert_eq!(
        view["voter_kind"], "account",
        "frozen to account ballots: {view}"
    );
    assert_eq!(
        view["voting_rule"]["participating_majority"]["quorum"], 2,
        "share-mode Signal uses ParticipatingMajority quorum ceil(n/2): {view}"
    );

    // account A holds 2 of 3 shares: its lone yes is an irreversible majority, so
    // execute settles EARLY, well before the deadline — and the ballot is keyed
    // by the ACCOUNT id, not A's node key (the node→account resolution).
    vote(&sim, NODE_A, "ship", true);
    let view = proposal(&sim, "ship");
    assert_eq!(
        view["votes"][0][0],
        json!(acct_a),
        "the ballot principal is the account, not the node: {view}"
    );
    execute_ok(&sim, NODE_A, "ship");
    assert_eq!(
        proposal(&sim, "ship")["status"],
        "passed",
        "early passage by shares"
    );

    // the deadline clock: a Signal carried only by account B (1 of 3) cannot
    // reach the quorum, so it cannot decide early…
    let base = height(&sim);
    propose(
        &sim,
        NODE_A,
        "minority",
        json!({ "signal": { "text": "nope" } }),
        5000,
    );
    vote(&sim, NODE_B, "minority", true);
    let error = execute_rejected(&sim, NODE_A, "minority");
    assert!(
        error.contains("not decidable yet"),
        "a lone minority ballot cannot settle early: {error}"
    );
    // …and once the clock passes the deadline it settles rejected (participation
    // never reached the quorum).
    walk_to(&sim, base + 10);
    execute_ok(&sim, NODE_A, "minority");
    assert_eq!(
        proposal(&sim, "minority")["status"],
        "rejected",
        "settled at the deadline without a quorum"
    );
}

/// account voting is gated at the node→account resolution: a node bound to no
/// Identity account cannot open or carry a share-mode ballot.
#[test]
fn a_node_bound_to_no_account_cannot_vote_in_share_mode() {
    let storage = tempfile::tempdir().expect("storage dir");
    let (sim, _a, _b) = share_governed(storage.path());

    // an unbound 32-byte node proposing in share mode is refused when governance
    // resolves it to an account and finds none.
    let stranger = "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz";
    let error = sim.submit_rejected(
        "governance",
        json!({ "propose": { "proposal_id": "x", "voting_period": 1000, "action": {
            "signal": { "text": "hi" }
        }}}),
        Some(stranger),
    );
    assert!(
        error.contains("submitter node is not bound to an Identity account"),
        "share-mode authorship resolves through Identity: {error}"
    );
}

// ── the electorate freezes across a mode flip ───────────

/// a proposal opened in share mode stays ACCOUNT-decided even after governance
/// flips back to validator ballots: its frozen electorate keeps resolving
/// ballots by account (a validator cannot vote on it) and it tallies by the
/// account rule, while a proposal opened AFTER the flip is validator-keyed.
#[test]
fn a_share_mode_proposal_stays_account_decided_after_the_flip_back() {
    let storage = tempfile::tempdir().expect("storage dir");
    let (sim, key_a, _key_b) = share_governed(storage.path());
    let acct_a = key_a.public_key().as_ref().to_vec();

    // open a Signal in share mode (frozen account electorate), do NOT settle it.
    propose(
        &sim,
        NODE_A,
        "frozen",
        json!({ "signal": { "text": "pre-flip" } }),
        1_000_000,
    );
    assert_eq!(proposal(&sim, "frozen")["voter_kind"], "account");

    // flip the mode back to validator. SetShareMode(false) is itself an
    // account-keyed proposal (opened while share mode is active) — a structural
    // action, so ceil(2n/3)=2, which account A's 2 shares meet alone.
    propose(
        &sim,
        NODE_A,
        "flip",
        json!({ "set_share_mode": { "enabled": false } }),
        1_000_000,
    );
    let flip = proposal(&sim, "flip");
    assert_eq!(
        flip["voter_kind"], "account",
        "the flip itself is account-decided: {flip}"
    );
    assert_eq!(
        flip["voting_rule"]["threshold"]["required_yes"], 2,
        "ceil(2n/3): {flip}"
    );
    vote(&sim, NODE_A, "flip", true);
    execute_ok(&sim, NODE_A, "flip");
    assert_eq!(
        shares_view(&sim)["active"],
        false,
        "share mode is off (registry retained)"
    );

    // a proposal opened AFTER the flip is validator-keyed again.
    let v0 = origin(&Ed::from_seed(1));
    propose(
        &sim,
        v0.as_str(),
        "post",
        json!({ "signal": { "text": "post-flip" } }),
        1_000_000,
    );
    assert_eq!(
        proposal(&sim, "post")["voter_kind"],
        "validator_node",
        "new proposals follow the current (validator) mode"
    );

    // THE FREEZING INVARIANT: the "frozen" proposal STILL resolves its ballots by
    // account — a validator's vote is refused at the node→account resolution,
    // exactly as in share mode…
    let error = sim.submit_rejected(
        "governance",
        json!({ "vote": { "proposal_id": "frozen", "approve": true } }),
        Some(v0.as_str()),
    );
    assert!(
        error.contains("submitter node is not bound to an Identity account"),
        "the frozen proposal keeps its account resolution: {error}"
    );

    // …and it tallies by the FROZEN account electorate: account A's 2 of 3 shares
    // pass it under ParticipatingMajority, though the network is now in validator
    // mode. the winning ballot is keyed by the account.
    vote(&sim, NODE_A, "frozen", true);
    execute_ok(&sim, NODE_A, "frozen");
    let settled = proposal(&sim, "frozen");
    assert_eq!(
        settled["status"], "passed",
        "settled by the frozen account rule: {settled}"
    );
    assert_eq!(
        settled["voter_kind"], "account",
        "electorate never changed: {settled}"
    );
    assert_eq!(
        settled["votes"][0][0],
        json!(acct_a),
        "keyed by the account id: {settled}"
    );
}

// ── kv under the valset preset ──────────────────────────

/// kv registers only under `--with-valset`. a set/get round-trip, then the
/// no-delete reality: the wire carries only Set, so the nearest "delete" is a
/// set to an empty value — which reads back as an EMPTY value, never as absence;
/// only a never-written key reads null.
#[test]
fn kv_round_trips_and_a_set_empty_is_not_a_delete() {
    let storage = tempfile::tempdir().expect("storage dir");
    let (sim, _validators) = governed(storage.path());
    let key = b"greeting".to_vec();
    let get = |k: &[u8]| sim.query("kv", json!({ "get": { "key": k.to_vec() } }))["value"].clone();

    // an unset key reads null (kv takes any origin — no origin gate).
    assert_eq!(get(&key), Value::Null, "an unset key reads null");

    // set → get; overwrite → get the new value.
    sim.submit_ok(
        "kv",
        json!({ "set": { "key": key, "value": b"hi".to_vec() } }),
        Some("writer"),
    );
    assert_eq!(get(&key), json!(b"hi".to_vec()), "set then get");
    sim.submit_ok(
        "kv",
        json!({ "set": { "key": key, "value": b"yo".to_vec() } }),
        Some("writer"),
    );
    assert_eq!(get(&key), json!(b"yo".to_vec()), "overwrite wins");

    // "delete" = set to empty. the value becomes Some([]), NOT None — kv has no
    // tombstone op over the wire.
    sim.submit_ok(
        "kv",
        json!({ "set": { "key": key, "value": Vec::<u8>::new() } }),
        Some("writer"),
    );
    assert_eq!(
        get(&key),
        json!(Vec::<u8>::new()),
        "an emptied key is Some([]), not absent"
    );
    assert_eq!(
        get(b"never"),
        Value::Null,
        "only a never-written key is null"
    );
}

/// the whole app-hash is deterministic under the valset preset: the same
/// kv-touching script on two fresh dirs walks to a byte-identical tip hash
/// (the standing guard against nondeterminism in the preset modules).
#[test]
fn a_kv_script_is_deterministic_across_fresh_dirs() {
    let run = |storage: &Path| {
        let (sim, _validators) = governed(storage);
        for (k, v) in [("a", "1"), ("b", "2"), ("a", "3")] {
            sim.submit_ok(
                "kv",
                json!({ "set": { "key": k.as_bytes().to_vec(), "value": v.as_bytes().to_vec() } }),
                Some("writer"),
            );
        }
        sim.status()["appHash"]
            .as_str()
            .expect("app hash")
            .to_string()
    };
    let d1 = tempfile::tempdir().expect("dir");
    let d2 = tempfile::tempdir().expect("dir");
    assert_eq!(
        run(d1.path()),
        run(d2.path()),
        "the kv script diverged across two fresh dirs"
    );
}
