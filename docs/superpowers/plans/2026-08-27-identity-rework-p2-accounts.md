# Identity Rework Phase 2+3 — Accounts by Number, No Node Binding, Host Planes

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the key-keyed, node-bound identity module with number-keyed accounts + key associations, and move every consumer (consensus modules, host planes, CLI, tests, ops, app compile surface) off "whose node is this" onto user-signed origins.

**Architecture:** The identity module stores `acct\0{n}` records, a `key\0{pubkey}→n` index, a `gen\0{pubkey}` admission counter and `next`. The frame ORIGIN is the acting key for every op; only `AddKey` carries an explicit authorizer proof (an existing member signs `add_key_preimage(chain, scheme, new_key, gen)`). Consumers resolve accounts with `OfKey` only. Governance/forge principals for accounts are `identity::account_principal(number)` (8-byte LE, length-disjoint from any key). Gateway account ids are `u64` end to end. Host planes attribute callers from the request's user PoP (server side here; app stamping is phase 4) or carry no account.

**Tech Stack:** Rust workspace; borsh state records; serde snake_case wire; `keyscheme` (phase 1); wasm guests via `make wasm-modules`; simnode + bin/node e2e.

**Spec:** `docs/superpowers/specs/2026-08-27-identity-rework-design.md` (phases 2 and 3; phase 3's PoP *stamping* is phase 4).

## Why phases 2 and 3 are one PR

Deleting `OfNode`/`BindNode` breaks `bin/node` (work admission, airlock lanes, term plane, gateway plane, CLI), `noded` admin, simnode and every e2e at compile time; and `OfKey(saga origin)` only works once `agent run/sched` submit user-signed frames. Stubbing those for one PR would be the dual-path code CLAUDE.md forbids. So this PR carries the spec's phase-3 table too. What stays out: app UI redesign (phase 4 — here the app only compiles and its tests pass), WebAuthn flows (5), signed push (6).

## Global Constraints

- Zero live networks: flag-day replacement, no compat arms, no versioned enums.
- `MAX_ACCOUNTS = 65_536`; account numbers start at 1; `0` is never an account.
- `IDENTITY_ADD_KEY_NS = b"ducktape-identity-add-key-v1"`; `add_key_preimage(chain_id, scheme, new_key, gen)` = `push_bytes(chain) ‖ scheme.tag() ‖ push_bytes(new_key) ‖ gen LE8`.
- `Origin::External(pubkey)` stays raw bytes. `Create`/`AddKey` declare the origin key's `scheme` in the payload; the module requires `scheme.pubkey_wellformed(origin)`.
- `identity::account_principal(n: u64) -> Vec<u8>` = `n.to_le_bytes().to_vec()`.
- Gateway: `account_id: u64` in every wire type, state key, preimage and header; `x-duck-caller-account` / `x-duck-route-account` carry the DECIMAL number; a caller without a PoP has no account.
- Airlock lanes: (1) deleted; (3) `saga.pinned_assignee == caller_node`; (4) `OfKey(saga origin) ∈ owner ∪ grants`. `WorkAdmission::Owner` deleted.
- Node-key origins never resolve to an account anywhere. `grep -rn 'OfNode\|BindNode\|account_of_node\|node_is_current' crates bin app ops` must be empty at the end.
- House rules: named predicates, one match per discriminant, no sleeps in tests, `cargo clippy -p <crate> --tests --no-deps` clean on every touched crate, format only touched files.

---

## File map

**identity module** (rewrite): `crates/modules/system/identity/src/{lib.rs, interface.rs, tests.rs, testkit.rs, guest.rs}`, `tests/sync_round_trip.rs`, `Cargo.toml` (drop `valset` dep).

**consensus consumers:** `crates/kernel/host/src/lib.rs:1490-1511`; `crates/modules/system/governance/src/{lib.rs, interface.rs}` + `tests/governance_shares.rs`; `crates/modules/system/gateway/src/{interface.rs, module.rs, proxy.rs, guest.rs}` + `tests/{module.rs, sync_round_trip.rs, contract.rs}`; `crates/duckdns/src/wire.rs`; `crates/modules/apps/forge/src/{state.rs, module.rs(tests), lib.rs(doc)}`; `crates/modules/system/acl/{src/interface.rs(doc), tests/dispatch_gate.rs}`; `crates/kernel/host/tests/{wasm_identity_parity.rs, wasm_gateway_parity.rs, wasm_governance_parity.rs}`.

**host planes + CLI:** `bin/node/src/{work_admission.rs, work_admission/tests.rs, airlock.rs, gateway_plane.rs, compute/cred.rs, term_plane.rs, cli.rs, cli_args.rs, agent_cli.rs, cred_cli.rs, userkey_cli.rs, main.rs, boot/surfaces.rs, host_state.rs}`, new `bin/node/src/account_cli.rs`; `crates/noded/src/admin.rs` + `tests/router.rs`; `crates/airlock/src/{server.rs, testkit.rs}` + `tests/e2e.rs`; `crates/workspace-config/src/identity.rs`; `bin/noded/src/main.rs`; `bin/simnode/src/lib.rs` + `tests/{harness/mod.rs, module_gaps.rs, core_scenarios.rs, governance_frames.rs, share_governance.rs, reactor_seams.rs, gateway_registry.rs, frame_and_batch.rs}`; `bin/node/tests/{identity_e2e.rs, gateway_naming_e2e.rs, gateway_e2e.rs, airlock_gateway_e2e.rs, cred_lending.rs, remote_session.rs, sched_pinned_run.rs}`.

**app (compile + tests only):** `app/src/backend/{node.rs, live.rs, agent.rs}`, `app/src/ui/{extern/backend.ice, state/node.ice, handlers/roster.ice, screens/settings.ice, view.ice, tests/app.ice}`, `app/src/tests/shell.rs`, `app/src/backend/tests/messages.rs`.

**ops/docs:** `ops/demo-gateway.mjs`, `ops/completions/ducktape.{bash,zsh}`, `skills/qa/SKILL.md`.

---

## Task 1: identity module — wire surface

**Files:** `crates/modules/system/identity/src/interface.rs` (rewrite), `Cargo.toml` (remove `valset`).

- [ ] **Step 1: Write `interface.rs`**

```rust
//! the identity module's public wire surface -- types only.
//!
//! an ACCOUNT is an abstract principal identified by a NUMBER (monotonic from
//! 1), owning an ASSOCIATION of keys of mixed [`KeyScheme`]s. the frame ORIGIN
//! is the acting key for every op: `Create` founds an account for the origin,
//! `AddKey` admits the origin into an existing member's account (that member
//! proves consent over [`add_key_preimage`] at the origin's CURRENT generation,
//! so the proof is single-use), `RemoveKey`/`SetName`/`SetProfile` need only the
//! origin's membership. no account is ever keyed by a key, and no node is ever
//! bound to an account.

use serde::{Deserialize, Serialize};
pub use keyscheme::KeyScheme;

pub type AccountNumber = u64;

/// signing domain for add-key consents.
pub const IDENTITY_ADD_KEY_NS: &[u8] = b"ducktape-identity-add-key-v1";
pub const MAX_NAME_LEN: usize = 64;
pub const MAX_LABEL_LEN: usize = 64;
pub const MAX_BIO_LEN: usize = 280;
pub const MAX_AVATAR_REF_LEN: usize = 512;
pub const MAX_QUERY_LIMIT: u64 = 256;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct KeyView { pub scheme: KeyScheme, pub pubkey: Vec<u8>, pub label: Option<String>, pub added_at: u64 }

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AccountView {
    pub number: AccountNumber,
    pub name: String,
    pub keys: Vec<KeyView>,   // ascending by pubkey; never empty
    pub avatar: Option<String>,
    pub bio: Option<String>,
    pub updated_at: u64,
}

/// an existing member's consent to admit the frame origin: which key, and its
/// scheme-owned proof over [`add_key_preimage`].
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Authorizer { pub key: Vec<u8>, pub proof: Vec<u8> }

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum IdentityMsg {
    /// found an account for the ORIGIN key (of `scheme`); origin must belong to no account.
    Create { name: String, scheme: KeyScheme },
    /// admit the ORIGIN key (of `scheme`) into `authorizer.key`'s account. on success gen[origin] += 1.
    AddKey { scheme: KeyScheme, label: Option<String>, authorizer: Authorizer },
    /// origin ∈ account; any member removes any key except the last.
    RemoveKey { key: Vec<u8> },
    SetName { name: String },
    SetProfile { avatar: Option<String>, bio: Option<String> },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum IdentityQuery {
    /// accounts numbered `from..`, at most `limit` (clamped to MAX_QUERY_LIMIT). `from: 0` reads from 1.
    All { from: u64, limit: u64 },
    Get { number: AccountNumber },
    OfKey { key: Vec<u8> },
    /// how many times `key` has been admitted anywhere (absent = 0) — the gen an AddKey consent must sign.
    KeyGen { key: Vec<u8> },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum IdentityReply { Accounts(Vec<AccountView>), Account(Option<AccountView>), Gen(u64) }

/// chain ‖ scheme tag ‖ new key ‖ gen — no account number and no account nonce ON PURPOSE (see spec).
pub fn add_key_preimage(chain_id: &str, scheme: KeyScheme, new_key: &[u8], gen: u64) -> Vec<u8> {
    let mut out = Vec::new();
    sdk::codec::push_bytes(&mut out, chain_id.as_bytes());
    out.push(scheme.tag());
    sdk::codec::push_bytes(&mut out, new_key);
    out.extend_from_slice(&gen.to_le_bytes());
    out
}

/// the byte principal other modules (governance ballots, forge owners) use for
/// an ACCOUNT: 8 bytes LE, length-disjoint from every key scheme's pubkey.
pub fn account_principal(number: AccountNumber) -> Vec<u8> { number.to_le_bytes().to_vec() }
/// the inverse, for renderers: `Some(n)` iff `bytes` is an 8-byte principal.
pub fn principal_account(bytes: &[u8]) -> Option<AccountNumber> {
    <[u8; 8]>::try_from(bytes).ok().map(u64::from_le_bytes)
}

// encode_msg/decode_msg/encode_query/decode_query/encode_reply/decode_reply unchanged (sdk::wire).
```

Tests in `interface.rs`: preimage deterministic + each of chain/scheme/key/gen moves it; codec round-trips for every message/query/reply variant; `principal_account(account_principal(7)) == Some(7)` and `principal_account(&[0u8;32]) == None`.

- [ ] **Step 2: Cargo.toml** — remove `valset = { workspace = true }` (no member gate). Keep `keyscheme`, `commonware-cryptography` (testkit), `borsh`, `sdk`, `serde*`, `async-trait`.

---

## Task 2: identity module — state and execution

**Files:** `src/lib.rs` (rewrite), `src/guest.rs` (drop `VALSET_ID`; `Identity::new(MODULE_ID, Box::new(WitStore), chain_id)`).

**State keys:** `acct\0{n LE8}` → `AccountRecord`; `key\0{pubkey}` → `u64`; `gen\0{pubkey}` → `u64`; `next` → `u64` (absent = 1). No roster.

```rust
#[derive(BorshSerialize, BorshDeserialize, ...)]
struct KeyMeta { scheme: KeyScheme, label: Option<String>, added_at: u64 }
struct AccountRecord { name: String, keys: BTreeMap<Vec<u8>, KeyMeta>, avatar: Option<String>, bio: Option<String>, updated_at: u64 }
pub const MAX_ACCOUNTS: u64 = 65_536;
pub const MAX_ACCOUNT_RECORD_BYTES: usize = 512 * 1024;

pub struct Identity { id: ModuleId, chain_id: String, staged: StagedStore }
impl Identity { pub fn new(id: impl Into<ModuleId>, store: Box<dyn MerkleStore>, chain_id: String) -> Self }
```

Reads: `account(n)`, `stored_account(n)` (loud on missing), `owner_of_key(key) -> Option<u64>`, `key_gen(key) -> u64`, `next_number() -> u64`. Writers: `store_account(n, &rec)` (byte-capped), `store(key, &v)` for the index/gen/next, `staged.delete`.

**execute** — one match, one delegation per arm:
- `Create { name, scheme }`: `origin = origin_key(ctx)?`; `scheme.pubkey_wellformed(origin)` else "founding key is malformed for its scheme"; `owner_of_key(origin).is_none()` else "key already belongs to an account"; `name = clean_name(name)?` (trim; empty → "account name is empty"; > MAX_NAME_LEN → reject); `n = next_number()`; `n > MAX_ACCOUNTS` → "account cap reached (65536)"; write record `{name, keys: {origin: KeyMeta{scheme, label: None, added_at: now}}, avatar: None, bio: None, updated_at: now}`, `key\0{origin} = n`, `next = n + 1`.
- `AddKey { scheme, label, authorizer }`: origin wellformed; origin unowned; `n = owner_of_key(authorizer.key)` else "authorizer belongs to no account"; `rec = stored_account(n)`; `meta = rec.keys[authorizer.key]` (must exist — index and record agree; loud otherwise); `gen = key_gen(origin)`; `preimage = add_key_preimage(chain, scheme, origin, gen)`; `meta.scheme.verify(authorizer.key, IDENTITY_ADD_KEY_NS, preimage, authorizer.proof)` else "authorizer consent does not verify"; `label = clean_label(label)?`; insert; `updated_at = now`; store record; `key\0{origin} = n`; `gen\0{origin} = gen + 1`.
- `RemoveKey { key }`: `n = owner_of_key(origin)` else "origin key belongs to no account"; `rec.keys.contains_key(key)` else "target key is not a member of this account"; `rec.keys.len() > 1` else "cannot remove the last key of an account"; remove; store; `delete key\0{key}`. (`gen` untouched — re-admission signs at the next generation.)
- `SetName { name }`: origin ∈ account; `clean_name`; store.
- `SetProfile`: origin ∈ account; `clean_field` as today.

**query:**
- `All { from, limit }`: `first = from.max(1)`; `end = next_number()`; walk `first..end` taking `limit.min(MAX_QUERY_LIMIT)` records (`stored_account`).
- `Get { number }` → `account(number).map(view)`.
- `OfKey { key }` → `owner_of_key(key)` → `Account(Some(view))`/`None`.
- `KeyGen { key }` → `Gen(key_gen(key))`.

`account_view(n, &rec)` maps keys ascending. `origin_key` unchanged. Doc header rewritten (no nodes, no nonce, no roster).

- [ ] **Step 1: write lib.rs + guest.rs as above.** `cargo check -p identity`.

---

## Task 3: identity module — tests + testkit + sync round trip

**Files:** `src/testkit.rs`, `src/tests.rs` (rewrite), `tests/sync_round_trip.rs` (rewrite).

`testkit.rs`:
```rust
/// an existing ed25519 member's consent to admit `new_key` (of `scheme`) at `gen`.
pub fn ed_authorizer(member: &ed25519::PrivateKey, chain_id: &str, scheme: KeyScheme, new_key: &[u8], gen: u64) -> Authorizer {
    let preimage = add_key_preimage(chain_id, scheme, new_key, gen);
    Authorizer { key: member.public_key().as_ref().to_vec(), proof: keyscheme::testkit::ed25519_proof(member, IDENTITY_ADD_KEY_NS, &preimage) }
}
pub fn create(name: &str) -> IdentityMsg { IdentityMsg::Create { name: name.into(), scheme: KeyScheme::Ed25519 } }
```

`tests.rs` (MemStore harness as today, `apply(id, origin, msg)`), tests:
1. `create_founds_account_one_then_two` — numbers 1, 2; `OfKey`, `Get`, `All{0,16}` ordering; `KeyGen` of a fresh key is 0.
2. `a_key_founds_at_most_one_account` — second `Create` from the same key refused "already belongs".
3. `add_key_admits_with_member_consent_and_bumps_gen` — founder authorizes joiner (gen 0); `keys.len()==2`; `KeyGen(joiner)==1`; replaying the same AddKey (same proof) refused "does not verify".
4. `a_removed_key_relinks_at_its_next_generation` — remove joiner; `OfKey(joiner)==None`; `KeyGen==1`; re-add with a gen-1 consent to ANOTHER account succeeds; a gen-0 consent is refused.
5. `a_removed_authorizer_cannot_consent` — B's consent minted, B removed, AddKey refused "authorizer belongs to no account".
6. `last_key_is_never_removed`.
7. `wallet_and_passkey_found_and_authorize` — Secp256k1 founder (`keyscheme::testkit::eth_*`), Secp256r1 authorizer over the passkey envelope (proof = `passkey_proof(sk, "ducktape", IDENTITY_ADD_KEY_NS, preimage, true)`), an ed25519 joiner.
8. `create_rejects_malformed_key_wrong_scheme` — 32-byte origin declared Secp256k1 refused.
9. `set_name_and_profile_are_member_gated` — a stranger origin refused; empty name refused; caps enforced.
10. `account_cap` — set `next` past `MAX_ACCOUNTS` via a test-only store poke (stage `next = MAX_ACCOUNTS+1`) and assert Create refuses.
11. `abort_block_drops_staged_accounts`.
12. `all_pages_by_number` — 3 accounts; `All{from:2,limit:1}` == [2]; `All{from:0,limit:0}` empty.

`sync_round_trip.rs`: seed `__config`, `Create` (h1), `AddKey` passkey (h2), `RemoveKey` (h3), `SetProfile` (h4); replies compared for `All`, `Get{1}`, `OfKey{founder}`, `OfKey{removed}`, `KeyGen{removed}`; root equality.

- [ ] Gate: `cargo test -p identity --features testkit`; `cargo clippy -p identity --tests --no-deps --features testkit`; commit `feat(identity): accounts by number, key associations, no node binding`.

---

## Task 4: host ACL + forge + acl test

- `crates/kernel/host/src/lib.rs:1490-1511`: `identity_account_holds` = one `IdentityQuery::OfKey { key: submitter.to_vec() }`, `matches!(Account(Some(_)))`.
- `crates/modules/system/acl/src/interface.rs:34-47` doc: "a key on some Identity account".
- `crates/modules/apps/forge/src/state.rs:454-514`: `identity_account` returns `Option<u64>` (`a.number`); `principal_of_origin` = `OfKey(key)` → `identity::account_principal(n)` else `key`. `lib.rs:80-84` doc: "the app's user key and any other key of the same association collapse onto one account principal; a key on no account is its own principal". `module.rs:1654-1669` stub: `identity_of(number)` answering `OfKey` with `AccountView{number, name: "o".into(), keys: vec![], avatar: None, bio: None, updated_at: 0}`; test `:2404-2440` becomes "two member keys of one account share the owner": both queries answer `Some(view)`.
- `crates/modules/system/acl/tests/dispatch_gate.rs:255-320`: found the account with `IdentityMsg::Create { name: "alice", scheme: Ed25519 }` from `Origin::External(user_key)` (a valset seat is no longer needed — keep the valset host); probe = `SetName` from the SAME user key passes; an unknown key is refused "requires user standing". `gate_host` → `Identity::new("identity", MemStore, CHAIN)`.
- [ ] Gate: `cargo test -p host --lib`, `cargo test -p forge`, `cargo test -p acl`; commit.

---

## Task 5: governance

**`interface.rs`:** `ShareAllocation { account_id: u64, shares }`; `GovAction::SetShares { account_id: u64, shares }`; docs on `ProposalView.{proposer, votes, electorate}`: "a node key in validator mode; `identity::account_principal(n)` (8 bytes LE) in share mode".

**`lib.rs`:**
- `Actor::{ Account { number: u64 }, Node(Vec<u8>) }`; delete `nodes()`; `principal()` → `account_principal(number)` / node bytes.
- `resolve_actor`: `OfKey { key: origin }` → `Account { number }` else `Node`.
- delete `account_of_node`; `account_principal(ctx, submitter)` = `resolve_actor` → `Account{number}` → principal bytes, `Node` → `Err("submitter key belongs to no Identity account")`.
- `require_account(number)` via `Get { number }`; `identity_account` decoder gets a `Gen(_)` arm → error "unexpected identity reply".
- `frozen_electorate` validator mode: submitter must itself be a member node; the "account fans out to bound nodes" branch and its refusal string die: refusal `"submitter is not a validator-set member node"`.
- `node_ballots` deleted; `handle_vote` ValidatorNode arm: `eligible(submitter)` → `[submitter]` else no ballot (refuse "voter is not in the frozen electorate").
- shares map `BTreeMap<u64, u64>`; `Electorate.powers` in share mode keyed by `account_principal(n)`; `AdoptShares`/`SetShares` use `u64`.
- `guest.rs` unchanged strings.

**`tests/governance_shares.rs`:** `IdentityStub { accounts: BTreeMap<u64, AccountView>, by_key: BTreeMap<Vec<u8>, u64> }`; `new(entries: Vec<(u64, Vec<Vec<u8>>)>)` = number → member keys; `query` answers `All/Get/OfKey/KeyGen`. `share_host()` → `(host, [key;4], [1,2,3])`: account 1 has keys[0] and keys[1] ("one human, two devices"); the "two nodes share one ballot" assertions become "two keys of one account cast ONE account ballot". Validator-mode tests (`governance_gates_valset.rs`, `invite_redemption.rs`) unchanged except `Identity::new` arity.

- [ ] Gate: `cargo test -p governance`; commit.

---

## Task 6: gateway + duckdns

**`crates/duckdns/src/wire.rs`:** `ResolvedAccount { account_id: u64 }`, `HandleRegistration { handle, account_id: u64 }`. (`app/src/domain/duckdns-client.ts` mirror: update the type to `number` if the file exists.)

**`gateway/src/interface.rs`:** delete `MAX_ACCOUNT_ID_BYTES` and `validate_account_id`; add `pub fn validate_account_number(n: u64) -> Result<(), String>` (0 → "account number must be non-zero"); `RouteAudience::Accounts { account_ids: Vec<u64> }` (sorted-unique check on u64); `RouteStatement.account_id: u64` (keep `publisher_node`); `CredentialRecord { owner_account: u64, grants: BTreeSet<u64> }`; `RemoveCredentialStatement.owner_account: u64`; `CredentialGrantStatement { owner_account: u64, account: u64 }`; `credential_use_allowed(record, account: u64)`; `GatewayQuery::{Get{account_id: u64, name}, List{account_id: u64}}`; preimages push `account_id.to_le_bytes()` (route `:632`, credential `:333/:350/:372/:374`, `encode_policy :656-661` each audience id LE8); `ProxyRequestHead.account_id: u64` (proxy.rs:94).

**`gateway/src/module.rs`:**
- keys: `owner_key(n)` = `owner\0` + LE8; `route_key(n, name)` = `route\0` + LE8 + tag; `route_roster_key(n)` = `routes\0` + LE8; `handle_key` value = borsh `u64`. Delete `MAX_HANDLE_ACCOUNT_ID_LEN` + the length check.
- `origin_node` → `origin_key(ctx) -> Result<Vec<u8>>`: non-empty external origin (no 32-byte rule; error "gateway: origin must be an external key").
- `account_of_node` → `account_of_origin(ctx, origin) -> AccountView` via `OfKey`, error "gateway: origin key belongs to no Identity account".
- `set_route`: drop the `publisher_node != origin` check (the account vouches for the node it names); `statement.account_id == account.number` else "gateway: route account is not the origin's account"; `signer = account.keys.iter().find(|k| k.pubkey == authorization.signer)` else "gateway: signer is not a current account member"; `signer.scheme.verify(&authorization.signer, GATEWAY_ROUTE_NS, &preimage, &authorization.signature)`.
- `verify_credential_owner(ctx, origin, chain_id, owner_account: u64, authorization, preimage)`: same three gates with `keys` + stored scheme.
- `owned_credential(name, owner: u64)`, `set_credential`/`grant`/`revoke`/`remove`: `validate_account_number` where `validate_account_id` was.
- query: `Resolve`/`Registrations` produce u64; `Get`/`List` validate the number.
- `guest.rs` doc: "identity `OfKey` account derivation".

**`gateway/src/proxy.rs`:** `audience_allows(audience, owner: u64, caller: Option<u64>)`: `Owner` → `caller == Some(owner)`; `Network` → `true` (any mesh peer; a PoP is not required) — NOTE: today `Network` requires a resolved caller; per spec "Network = any mesh peer, PoP or not"; `Accounts` → `caller.is_some_and(|c| ids.binary_search(&c).is_ok())`. `request_matches_record` compares u64.

**tests:** `tests/module.rs` — `TestCtx.accounts: BTreeMap<Vec<u8>, AccountView>` keyed by ORIGIN KEY answering `OfKey`; `account(signer) -> AccountView { number, name: "Alice", keys: [KeyView{Ed25519, signer}], .. }`; `statement(account_id: u64, node, ..)`; `fixture(seed)` returns `(u64, PrivateKey, AccountView, TestCtx, Gateway)`; the origin of every op = the signer's pubkey. `tests/sync_round_trip.rs` same shape (`on_query("identity")` answers `OfKey`). `tests/contract.rs:14` `account_id: 7`. `crates/noded/tests/router.rs:965-1085` gateway fixtures: `account_id: 1`, JSON `"account_id": 1`.

- [ ] Gate: `cargo test -p gateway -p duckdns`; commit.

---

## Task 7: bin/node host planes

**`work_admission.rs`:**
- `WorkAdmission::{Accounts(BTreeSet<u64>), Anyone}`; default `Accounts(∅)`; `entries()` decimal strings; `work-admit.toml` `admit = ["12"]` / `["anyone"]`; parse decimal (error "admit entry is not an account number"); `AdmitTarget::Account(u64)`.
- `WorkCaller::{ThisNode, Account(u64), KeyWithoutAccount, PeerNode, NotAnAccountOrigin, Unresolved}`.
- `resolve_caller`: `Saga(SagaOrigin::External(key))`: `key == me` → `ThisNode`; else `OfKey(key)` → `Account(n)` / `KeyWithoutAccount`. `Peer(node)`: `node == me` → `ThisNode`; else `PeerNode`.
- `admit`: no owner read. `admits(policy, caller)`: `ThisNode` → yes; `Anyone` → yes; `Accounts(set)` + `Account(n)` → `set.contains(n)`; `PeerNode`/`KeyWithoutAccount` under `Accounts` → `NotAdmitted` / `CallerUnbound` (detail: "the submitting key is on no Identity account — `ducktape account create` there"). Delete `owner_account`, `account_of_node`, `admit_account_fixture` → `admit_account_fixture(workspace, number)`.
- Module doc + refusal strings rewritten (no BindNode, no account-init). Keep the source-parsing lints; `the_submit_lane_still_resigns_with_the_node_key` stays true (only node-authored ops use `/v1/submit`).
- `tests.rs`: consts `OWNER/FRIEND/STRANGER: u64 = 1/2/3`; `OwnerReadFails` → `KeyReadFails` decoding `OfKey`; replace `an_ownerless_node_admits_no_account` with `an_empty_policy_admits_only_this_node`; file tests use decimal.

**`airlock.rs`:** `GrantQuestion.caller_node: Vec<u8>` (was `caller`); lane (1) `caller_is_granted` deleted; `caller_is_the_pinned_executor` = `saga.pinned_assignee == caller_node` (pure compare, no read); `submitter_is_granted` = `OfKey(saga origin External key)` → `credential_use_allowed(record, n)`; the 4-byte prefix logging stays (node keys are still bytes). Test fixtures: identity fake answers `OfKey` with `AccountView{number, ..}`; `OWNER_ACCOUNT: u64 = 1` etc.

**`crates/airlock/src/server.rs`:** `CALLER_NODE_HEADER = "x-duck-caller-node"` replaces `CALLER_ACCOUNT_HEADER`; `vouched_caller` hex-decodes the node key; `GrantQuestion { credential, caller_node, work }`; `testkit::behind_gateway_proxy(app, node: &[u8])`; `tests/e2e.rs` stub_grant_check matches `caller_node`; refusal token `"caller_node_unverified"`.

**`gateway_plane.rs`:**
- `ProxyRequestHead` gains `pub user_pop: Option<UserPop { key: Vec<u8>, ts: u64, sig: Vec<u8> }>` (in `gateway/src/proxy.rs`). The FIRST hop (the local node serving its app) reads `x-duck-user-key/-ts/-sig` from the inbound request into the head and strips them; the inbound `x-duck-*` refusal exempts exactly these three names.
- `caller_account(commands, head, statement) -> Result<Option<u64>>`: `None` without a PoP; with one: `OfKey(key)` → account, `keys` entry → scheme, `ts` within 30 s of now, `scheme.verify(key, GATEWAY_CALLER_NS, caller_pop_preimage(publisher_node, account_id, route name, method, path, ts), sig)` → `Some(number)`; a present-but-invalid PoP is `Forbidden("gateway caller proof does not verify")`. `GATEWAY_CALLER_NS = b"ducktape-gateway-caller-v1"` and `caller_pop_preimage` live in `gateway/src/interface.rs`.
- `serve_current`: `audience_allows(&audience, record.statement.account_id, caller)`; `proxy_loopback(caller: Option<u64>, ..)` stamps `x-duck-caller-account: <decimal>` only when `Some`; `x-duck-route-account: <decimal>`.
- `revalidate_route_authority`: `Get { number: statement.account_id }`; signer ∈ `keys` with its scheme; drop `node_is_current`.
- in-file tests: `account(number, member) -> AccountView`; header assertions: caller-account header ABSENT without a PoP; one new test `a_valid_user_pop_stamps_the_caller_account` (sign with `keyscheme::testkit::ed25519_proof`).

**`compute/cred.rs`:** delete `account_of_node`; `build_airlock` uses `record.owner_account` → `handle_of_account(owner: u64)`.

**`term_plane.rs`:** `AdmitOk.owner_account: u64`; `owner_airlock_authority(u64)`; peer creates are `PeerNode` callers — admitted only by `Anyone`; delete the fake identity in the test (`:1320-1352`); test `a mesh peer this node does not admit is refused` keeps its assertion with policy `Accounts(∅)`.

**`crates/noded/src/admin.rs`:** `AdminConfig { node_key: Option<Vec<u8>> /* PoP salt */, owner_key: Option<Vec<u8>> /* the local user key whose account owns admin */, identity_module }`; `resolve_owner`: `owner_key` None → `NoOwner`; `OfKey(owner_key)` → `Owned(view.keys[].pubkey)` / `NoOwner`; `admit_owner` unchanged (`members.contains(presented)`). `bin/node/src/boot/surfaces.rs:184` sets `owner_key: <active keystore pubkey>` (from `keystore` — same source `userkey_cli` unlocks; read-only pubkey needs no password: use the keystore's `status` pubkey reader). `tests/router.rs` fake actor decodes `OfKey` and answers the owner's account.

**`cli.rs`:** delete `account_line` + the `account=` status line; `proposal_principal(VoterKind::Account)` → `OfKey(active user pubkey)` → `account_principal(n)`, and governance vote/propose submit as a USER-signed frame when the frozen kind is `Account` (follow the existing `sign-frame` path: `node::encode_frame(&user, seq, &msg)` → `/v1/submit/frame`); `cmd_work_*` print decimal.

**`agent_cli.rs:594-643`:** `--host-node` accepts a 64-hex node key only; the display-name form is deleted (doc `cli_args.rs:182-184`). `agent run`/`agent sched` submit the saga trigger as a USER-signed frame (the saga origin becomes the user key so `OfKey(saga origin)` attributes it) — locate the submit call and route it through the same frame helper as `sign-frame`, unlocking the keystore like `cred_cli` does.

**`cred_cli.rs`:** `query_owner_account_view` via `OfKey` (error "this user key belongs to no account — `ducktape account create` first"); `resolve_account(input)`: decimal number, else `All` by `name`; grant/revoke/remove/add carry `u64`; `cmd_list` prints the number; `ensure_airlock_route(.., account: u64)`; docs "an account number or a name". `cred add/grant/revoke` already sign with the user key — verify they submit via `/v1/submit/frame` (if they use `/v1/submit`, switch).

**`userkey_cli.rs` → `account_cli.rs`:** delete verbs `account-init`, `sign-bind`, `sign-unbind`, `sign-possession`, `sign-add-member`, `sign-remove-member`, `webauthn-challenge` and `NodeBindArgs/PossessionArgs/AddMemberArgs/RemoveMemberArgs/EnrollArgs`; keep `key`, `sign-gateway-route`, `sign-frame`, `sign-admin`, `cred`. New `Family::Account(account_cli::AccountCmd)` in `main.rs`/`cli_args.rs`:
```
ducktape account create --name <name>                 # Create{name, scheme: Ed25519} as a user-signed frame
ducktape account show [--number N | --key <hex>]      # Get / OfKey (default OfKey(active key)); prints number, name, keys
ducktape account key list
ducktape account key approve                          # prints this device's pubkey hex (+ scheme) for an existing member to admit
ducktape account key add --pubkey <hex> --scheme <s> [--label <s>]   # existing member: KeyGen(pubkey) → add_key_preimage → prints the AddKey ticket JSON
ducktape account key join --ticket <json>             # the new device: submits AddKey as a frame signed by ITS key
ducktape account key remove --pubkey <hex>            # RemoveKey frame
ducktape account set-name --name <s> / set-profile [--avatar] [--bio]
```
All submits go through `/v1/submit/frame` with the keystore's active key (`load_user_signer` + the `sign-frame` path); `--node`/`-n` resolution as `cred_cli`. Unit tests: `create` decodes to `Create`, `key add` ticket decodes to `AddKey` and its authorizer proof verifies with `KeyScheme::Ed25519.verify`, `key join` wraps the ticket verbatim, verb list test (`account`, `create`, `show`, `key`, `set-name`, `set-profile`).

**`workspace-config/src/identity.rs`:** `ed25519_member_auth` → `ed25519_authorizer(user, chain_id, scheme, new_key, gen) -> identity::Authorizer`; delete `ed25519_possession`; tests: consent verifies against `add_key_preimage`, is chain-scoped, gen-scoped.

**`host_state.rs`:** update the doc strings (`:526-535`, `:795-812`) and re-pin `GENESIS_ROOT_HASH` after `make wasm-modules` (Task 10). **`bin/noded/src/main.rs:288-297`, `bin/simnode/src/lib.rs:834-844`:** `Identity::new("identity", store, String::new())`.

**`ops/completions`:** drop the deleted verbs; add `account create show key set-name set-profile` (+ `list approve add join remove`, flags `--name --number --pubkey --scheme --label --ticket --avatar --bio`). **`main.rs` GETTING_STARTED:** `ducktape account create --name <you>`.

- [ ] Gate: `cargo check -p node-bin --all-targets`, `cargo test -p node-bin --bin ducktape`, `cargo test -p noded`, `cargo test -p airlock`, `cargo test -p workspace-config --lib` (pre-existing join.rs breakage: fix the refutable pattern with `let ... else { unreachable!() }` → NO: leave it if it is dev's; report); commit `feat(node): host planes and the account CLI speak accounts by number`.

---

## Task 8: e2e + simnode

**simnode harness:** `ed_bind_auth` → delete; add `pub fn create(name: &str) -> serde_json::Value = json!({"create": {"name": name, "scheme": "ed25519"}})` and `ed_authorizer(key, chain_id, new_key_hex_or_bytes, gen)` → `serde_json::to_value(identity::testkit::ed_authorizer(...))`. In the sim the ORIGIN is a fabricated 32-byte string, so `Create` from origin `node_a` with `scheme: ed25519` founds account 1 for that "key".
- `module_gaps.rs`: rewrite C2 as `an_add_key_consent_is_single_use_and_a_removed_key_relinks_at_its_next_gen` (Create from `"a"*32`; AddKey origin `"c"*32` with `ed_authorizer(key_a…)` — NOTE the sim's origin bytes are not real keys, so the authorizer must be a REAL ed25519 key that founded the account: found with `Create` from origin = `key_a.public_key()` bytes (`Some(&hex)`? the sim's `submit_ok(.., Some(&origin))` takes a 32-byte string — check the harness: if it requires ASCII, use `ed25519 seed keys` only where the harness lets an arbitrary 32-byte origin through; otherwise keep the KeyGen/replay test in the identity crate and make the sim test assert only `create` + `remove last key` refusal). `removing_the_last_member_key_is_refused` → `{"remove_key":{"key": origin}}`.
- `core_scenarios.rs:222-305`: `create` founds; gateway refusals: "origin key belongs to no Identity account"; `resolved["resolved"]["account_id"] == 1` then `== 2`.
- `governance_frames.rs`: `bind_node_to_account` → the validator node key ITSELF is the origin of `Create` (its own account, since node keys are ordinary ed25519 keys) — then `proposal["proposer"]` == the node key in validator mode (assert the proposer is the NODE key; the "authored by the account" claim is now share-mode only).
- `share_governance.rs`: `bind_account` → `create` from `NODE_A`/`NODE_B` origins (accounts 1, 2); `adopt_shares` allocations `{"account_id": 1}`/`2`; ballots `view["votes"][0][0] == account_principal(1)` bytes; the no-account refusal string.
- `reactor_seams.rs`, `gateway_registry.rs`, `frame_and_batch.rs`: `create` instead of `bind_node`; `RouteStatement.account_id: 1`; `get_route(sim, 1, ..)`.

**bin/node e2e** (`common` helpers: add `create_account(cluster, idx, user: &PrivateKey, name) -> u64` that submits `Create` as a user-signed frame via `/v1/submit/frame` and polls `OfKey`; `add_key(cluster, idx, member, new_key)`):
- `identity_e2e.rs`: `Create` on node A → `Get{1}`/`OfKey`; `AddKey` (second key, consent from the founder at gen 0) → `keys.len()==2`; roots converge; `RemoveKey` from the second key removing the founder → `OfKey(founder)==None`, `KeyGen(founder)==1`; roots converge.
- `gateway_naming_e2e.rs`: `create_account` then `SetHandle` as a USER-signed frame (origin = user key); `resolved.account_id == number`.
- `gateway_e2e.rs`, `airlock_gateway_e2e.rs`, `cred_lending.rs`, `remote_session.rs`, `sched_pinned_run.rs`: accounts via `create_account`; routes/credentials signed + submitted by the user key; `work-admit.toml` `admit = ["<number>"]`; `x-duck-caller-account` assertions: ABSENT (no PoP from a CLI/peer) — `gateway_e2e:170` asserts the header is missing; airlock lanes: the pinned-executor lane compares node keys, the grant lane resolves the saga's USER origin (the `agent sched` submit is user-signed now). Keep every existing assertion that is about roots, refusal tokens, or session behavior.

- [ ] Gate: `cargo test -p simnode`; the bin/node e2e suites listed (run each `--test`); commit.

---

## Task 9: app compile surface

- `app/src/backend/node.rs`: `AccountData { generation, exists: bool, number: i64, name, bio, keys: i64 }` (drop `bound`/`account_id`/`display_name`/`nodes`); `load_account`: `of_key { key: local user key bytes }` → fields; `set_account_name` → `IdentityMsg::SetName { name }`.
- `agent.rs`: delete `node_account_names`; `host_node_row` shows the short node key.
- `live.rs` `load_dm_peers`: `account["number"]` → `key = number.to_string()`, `name = account["name"]`, self-filter over `account["keys"][].pubkey`; `open_dm(peer)`: `Get { number }` → seat every `keys[].pubkey` of the peer (plus `me`) in `SetMembership`; `dm_channel_id(me, number_string)`.
- Ice: `backend.ice` `AccountData(generation, exists, number, name, bio, keys)`; `node.ice` state `account_exists`, `account_number:str`, `account_name`, `account_bio`, `account_keys:i64` (drop `account_id`, `account_bound`, `account_members`, `account_nodes`); `roster.ice`, `view.ice`, `settings.ice` (the card: number line replaces the key line; "keys" count; "Copy number"), `tests/app.ice` mount; `app/src/tests/shell.rs:751-802` restate for the new field names; `backend/tests/messages.rs` DM fixture key `"42"`.
- [ ] Gate: `cargo test -p ducktape-app`; commit `feat(app): the account card and DM directory read accounts by number`.

---

## Task 10: wasm regen, parity proofs, genesis pin, ops

- `make wasm-modules`; rewrite `wasm_identity_parity.rs` (ops: Create ×2, AddKey ed25519 + passkey, RemoveKey, SetName; reads: `All{0,MAX}`, `All{2,1}`, `All{0,0}`, `Get{absent 99}`, `OfKey{absent}`, `KeyGen{absent}`, per-account `Get`, per-key `OfKey`/`KeyGen`; rejections: replayed consent, wrong scheme, stranger SetName, last-key removal, malformed founding key, second Create from a member); `wasm_gateway_parity.rs` + `wasm_governance_parity.rs`: `bind()` → `create()` from the FOUNDER key as origin; `ShareAllocation { account_id: 1/2 }`; gateway statements `account_id: u64`.
- `GENESIS_ROOT_HASH` in `host_state.rs:1440-1455`: run the pin test, copy the new hash.
- `ops/demo-gateway.mjs`: `ducktape account create --name demo` (user-signed) then `of_key` → `accountId = account.number`; routes carry the number. `skills/qa/SKILL.md:205-237` prose.
- [ ] Gates: `make wasm-modules-check`, `cargo test -p host` (minus the pre-broken forge parity target), `cargo test -p wasm-host`; commit artifacts.

---

## Task 11: full gates + PR

`cargo check --workspace --all-targets` (only the 3 pre-existing dev breakages may remain), crate tests for every touched crate, clippy per touched crate, `cargo check -p files --no-default-features`, `cargo test -p ducktape-app`, the leftover grep from Global Constraints. `gh pr create --base dev --title "identity rework phase 2+3: accounts by number, no node binding, host planes"`.

## Self-review notes

- Spec coverage: every row of the spec's two consumer tables maps to Task 4–7 items; `SetHandle` u64 is Task 6; `Standing::User` Task 4; PoP verification Task 7 (stamping → phase 4); CLI verbs Task 7; ops Task 10.
- Deviations from the spec text, stated: `scheme` in `Create`/`AddKey` payloads (origin carries no tag); `account key join` verb added (someone must submit the frame from the new key); noded admin owner = the local user key's account (not "any account"); `WorkCaller::PeerNode` (spec's "ThisNode or PeerNode").
