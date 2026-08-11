# Client ACL Plane Implementation Plan (PR8 — thin-client foundation)

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use `- [ ]`.

**Goal:** A `role=Client` invite, when redeemed, grants **client standing** (a consensus-committed ACL entry) instead of being rejected — and a client's own-signed submit is authorized at the validator door by that standing. This is the foundational plane the thin client (Design 4) builds on. NO tunnel, NO client-mode noded, NO app UX in this PR.

**Spec:** `docs/superpowers/specs/2026-07-13-coordinator-invites-thin-client-design.md` Design 4. The `role` byte already ships (targeted-invite cutover); `handle_redeem` currently rejects Client with "client invites are not redeemable yet — the thin-client plane lands separately". This PR is that plane's consensus half.

**The decisive design call:** client standing lives in a **NEW `clients` module**, NOT a valset tier. PR6 made statesync fail-closed keyed off `members ∪ residents`; a separate module makes it structurally impossible for a client to ever obtain statesync standing (the sync door reads valset, never clients). Clients are explicitly NOT consensus participants — keep them out of valset's "consensus membership" contract.

## Global Constraints

- Branch `client-acl`, worktree `.worktree/client-acl` (forked post-campaign dev). PR against `dev`. Commit after every task (0-commit worktrees get swept).
- Build via `ops/build-with.sh cargo ...`; `ulimit -s unlimited` before cargo fixes the rustc SIGSEGVs. Packages: the new `clients` crate, `governance`, `node-bin`, `simnode`.
- Gates per touched crate: `cargo clippy -p <crate> --tests --no-deps`. **`-p simnode` compile+test is a standing gate** (this touches the redeem/valset-adjacent plane — #545 broke simnode by skipping it). files wasm gate must stay green.
- NO backward compatibility. This adds a genesis module → the genesis root-hash changes → **flag day, all nodes rebuild genesis together**.
- Add `"clients"` to the `MODULE_IDS` genesis registry. The registry parity
  test catches omissions. There is no prior genesis format to accept: re-seed
  local networks after the root-hash changes.

## Key anchors (verified, may drift ±20 lines)

- Reject block: `crates/system/governance/src/lib.rs:1122-1127`. Resident-grant tail to mirror: `lib.rs:1148-1165` (dedup nonce → `emit_msg(ValsetMsg::Grant)`). Governance ctor `Governance::new("governance","valset","upgrade","identity")` at `bin/node/src/host_state.rs:242` (+ :375, :743) — it holds module ids as strings.
- Valset single-set discipline to CLONE: `crates/system/valset/src/lib.rs` — `residents`/`pending_residents` (`:55-69`), `ValsetMsg::Grant` handler (`:325-336`), `overlay`/`effective` (`:136-146`), snapshot/root (`:160-243`), query (`:345-352`), the `members()`/`residents()` cross-module read helpers (`:250-260`). Grant is module-origin-gated (only governance emits it).
- Genesis registration: `bin/node/src/host_state.rs:234-265` `ProductionModules` (+ the 2 sibling builders at :369, :714). Add the `clients` module beside `valset`.
- Submit door: `bin/node/src/relay.rs:191-205` `verify_relay_submit` — today admits only committed-resident origins ("origin holds no committed resident standing"). Extend to ALSO admit committed-client origins.
- Standing read helpers: `bin/node/src/host_reads.rs:9-38` (`read_valset_residents`) — add a `read_clients` sibling.
- Precedent for the ACL shape: `crates/apps/runs/src/sessions.rs:162-170` (origin == committed bound key).
- Test rigs: `crates/system/governance/tests/invite_redemption.rs` (the `mint_for`/`redeem`/`submit_as` rig with `--with-valset`-style host), `bin/simnode/tests/governance_scenarios.rs` (the sim redeem lane).

---

### Task 1: the `clients` consensus module

**Files:** create `crates/system/clients/` (Cargo.toml + src/lib.rs). Add to the workspace members list (root `Cargo.toml`).

**Design:** a single-set ACL module — clone valset's resident-set discipline for ONE set. `ClientsMsg::{ Grant { key }, Revoke { key } }` (module-origin-gated: only governance may emit Grant; Revoke may be governance too — keep it symmetric). `ClientsQuery::{ Clients, IsClient { key } }`. Committed `BTreeSet<Vec<u8>>` + `pending` overlay + `commit_block` fold + `snapshot`/`root`/`install` mirroring valset. A cross-module read helper `clients::is_client(host, key)` / `clients()` like `valset::members()`.

**Interfaces produced:**
```rust
pub enum ClientsMsg { Grant { key: Vec<u8> }, Revoke { key: Vec<u8> } }
pub enum ClientsQuery { Clients, IsClient { key: Vec<u8> } }
pub enum ClientsReply { Clients(Vec<Vec<u8>>), IsClient(bool) }
pub fn encode_msg/decode_msg/encode_query/decode_query/encode_reply/decode_reply(...);
pub async fn clients(host) -> Vec<Vec<u8>>;   // cross-module read, mirrors valset::members
pub struct Clients; impl Module for Clients { /* id, root, execute, query, commit_block, snapshot, install */ }
```

**Steps:** TDD. (1) unit tests: Grant from a MODULE origin inserts; Grant from an External origin is refused (module-gated, mirror valset's origin check); Revoke removes; the query reflects pending-over-committed; root changes on grant, stable after; snapshot round-trips. (2) implement by cloning the valset resident-set code paths for one set. (3) `cargo test -p clients` green. (4) commit.

### Task 2: register `clients` at genesis + wire governance to it

**Files:** `bin/node/src/host_state.rs` (all 3 `ProductionModules`-style builders — add `clients: Clients::new("clients")` beside valset), `Governance::new(...)` gains a `clients` module id arg (or a `.with_clients("clients")` builder like `.with_invite_binding`), the module registry vec each builder returns, and `MODULE_IDS`.

**Steps:** (1) add the module to genesis + governance's known ids and `MODULE_IDS`. (2) `cargo build -p node-bin` + the genesis/root-hash test if one exists (`grep -rn "root_hash\|genesis" bin/node/tests | grep -i hash`) — expect the genesis hash to CHANGE; update any golden and re-seed. (3) commit.

### Task 3: `handle_redeem` grants client standing

**Files:** `crates/system/governance/src/lib.rs:1122-1127` (the reject block) + the handler's tail.

**Design:** replace the early `Err` for `InviteRole::Client` with the client-grant path: keep the join-proof check (`lib.rs:1128` — proves the redeemer holds the target key); for a Client, the dedup gate is "already a client?" (query the clients module) rather than "already a resident/validator?"; record the nonce in the SAME `redeemed`/`pending_redeemed` single-use set (a client invite is single-use too); emit `ClientsMsg::Grant { key: joiner }` to the clients module instead of `ValsetMsg::Grant`. Resident redeems are UNCHANGED.

**Steps:** TDD in `crates/system/governance/tests/invite_redemption.rs` — extend the rig to register a clients module (mirror how it registers valset), then: (1) a `role=Client` targeted redeem by the target now SUCCEEDS and the joiner is in the clients set (was: rejected). (2) single-use: the same Client nonce can't redeem twice. (3) target-lock + expiry + join-proof still enforced for Client. (4) a Resident redeem still grants residency (unchanged), and a Client redeem grants CLIENT not resident (the joiner is NOT in residents — the tiers are distinct). Implement. `cargo test -p governance` green. Commit.

### Task 4: the submit door admits client standing

**Files:** `bin/node/src/relay.rs:191-205` (`verify_relay_submit`), `bin/node/src/host_reads.rs` (add `read_clients`).

**Design:** today the door admits an `Origin::External` key iff it holds committed resident standing. Extend: admit iff the key holds resident standing **OR** client standing (read the clients module). The error message updates ("origin holds no committed resident or client standing"). A client's own-signed frame (via `/v1/submit/frame`, no re-sign) is then authorized. NOTE: this only authorizes the SUBMIT; it does NOT give the client statesync or a quorum seat.

**Steps:** unit/integration test: a frame whose external origin is a committed client is admitted by `verify_relay_submit`; a random non-standing key is still refused. (Use the relay test harness if one exists; else a governance-rig-level test that a client-signed op settles.) Implement. Commit.

### Task 5: simnode scenario + gates + PR

**Files:** `bin/simnode/tests/governance_scenarios.rs` (add a client-standing scenario, mirroring the B-series), `bin/simnode/src/main.rs` if the sim's `--with-valset` genesis needs the clients module registered too (it will, to redeem Client).

**Steps:** (1) simnode: a `role=Client` redeem grants client standing (queryable), is single-use, and the joiner is NOT a resident/validator. (2) full gates: clippy (clients, governance, node-bin, simnode), `cargo test -p clients -p governance -p simnode`, the invite/redemption e2es, files wasm gate. (3) Push, open PR. Body: what client standing IS (submit authorization, NOT statesync/quorum — the separate-module boundary and WHY, given PR6), the genesis-module flag day + the preflight-const bump + re-seed instruction, and that this is the consensus half — the client-mode noded/tunnel/proxy/app land in follow-up PRs. Cite Design 4.

## Scope boundary (do NOT build here)
- client-mode noded (no-consensus proxy node), the tunnel bring-up reuse, `/v1/*` proxying, the app "remote workspace" UX. Those are PR9+. This PR ends at: a Client invite grants a committed client-ACL entry, and a client-signed submit is authorized at the door. Everything testable via the governance rig + simnode, no tunnel/app.

## Risks
- **Genesis cutover**: the new module changes the root-hash — `MODULE_IDS` must include `clients`; verify a fresh-seed boot.
- **Single-use set sharing**: Client and Resident redeems share the `redeemed` nonce set — confirm a nonce is single-use across BOTH (a Client invite's nonce can't later be reused as a Resident invite; they're different tokens with different nonces anyway, but the set is shared and that's correct).
- **Door correctness**: extending `verify_relay_submit` must not accidentally admit a client to anything beyond submit — confirm the door is ONLY the submit-authorization gate, not reused for statesync/quorum (PR6's statesync door reads valset, not this — verify no shared helper conflates them).
