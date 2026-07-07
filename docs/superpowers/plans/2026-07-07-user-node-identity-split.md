# User-Key / Node-Key Split Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One user (an app-held ed25519 keypair) can own multiple nodes: a new replicated `identity` module binds node keys to user keys, and the app resolves every node-key origin to the owning user's display identity.

**Architecture:** A profiles-shaped snapshot module (`crates/system/identity`) stores `user → {display_name, nonce, nodes}` and verifies user-signed bind/unbind certificates deterministically in `execute`; the node's frame format, valset, mesh, and NAT layers are untouched. The desktop shell keeps the user key at `~/.ducktape/user.key` (via new `ducktape-node` CLI verbs, same shell-out pattern as `keygen`) and auto-binds each workspace's node on connect. The React console overlays user display names onto the existing `authorNames` map so every view inherits user-level identity with no per-view rewrites.

**Tech Stack:** Rust (commonware-cryptography ed25519, sdk Module contract, serde_json wire), Tauri commands, TypeScript/React console, vitest.

**Spec:** `docs/superpowers/specs/2026-07-07-user-node-identity-split-design.md`

## Global Constraints

- Signing namespaces exactly: `IDENTITY_BIND_NS = b"ducktape-identity-bind-v1"`, `IDENTITY_UNBIND_NS = b"ducktape-identity-unbind-v1"`.
- Display-name limit 64 bytes (profiles' `MAX_NAME_LEN`); query page cap 256 (`MAX_QUERY_LIMIT`).
- The module id is `"identity"`; it joins every host composition (`bin/node` genesis/restore/joiner, `bin/noded`, `bin/simnode`, `bin/demo`) or app-hash parity breaks.
- Adding the module is a module-set change: existing networks need a genesis rebuild; this is accepted (dev-stage networks, same class as the video-calls engine-bank change).
- `Frame`, valset, governance, mesh, NAT, WireGuard code paths must NOT change.
- Determinism: no wall clock, no randomness inside the module; ed25519 verify in `execute` is allowed (pure).
- Rust style: match the repo's lowercase doc-comment voice; JSON wire via serde `snake_case` enums like profiles.
- Run Rust tests with `cargo test -p <crate>`; app tests with `cd app && bun run test -- --run <file>` (vitest); clippy gate only on touched crates (`cargo clippy -p identity` must be clean; node-bin baseline-diff only).
- Commit after every task with the shown message; all work in worktree `/home/eddy/dev/ducktape/.claude/worktrees/feat+identity-split` on branch `feat/identity-split`.

---

### Task 1: `identity` crate — wire surface (`interface.rs`)

**Files:**
- Create: `crates/system/identity/Cargo.toml`
- Create: `crates/system/identity/src/interface.rs`
- Create: `crates/system/identity/src/lib.rs` (stub: `mod interface; pub use interface::*;`)
- Modify: root `Cargo.toml` (workspace members + `identity = { path = "crates/system/identity" }` in `[workspace.dependencies]`, alphabetical)

**Interfaces (Produces):**
- `IdentityMsg::{BindNode{user_key: Vec<u8>, user_sig: Vec<u8>}, UnbindNode{node_key: Vec<u8>, user_sig: Vec<u8>}, SetUserName{display_name: String}}`
- `IdentityQuery::{All{from: u64, limit: u64}, Get{user_key: Vec<u8>}, UserOf{node_key: Vec<u8>}}`
- `IdentityReply::{Users(Vec<UserView>), User(Option<UserView>)}`
- `UserView { user_key: Vec<u8>, display_name: Option<String>, nonce: u64, nodes: Vec<Vec<u8>>, updated_at: u64 }`
- `bind_preimage(chain_id: &str, node_key: &[u8], nonce: u64) -> Vec<u8>`, `unbind_preimage(...)` same shape
- `IDENTITY_BIND_NS`, `IDENTITY_UNBIND_NS`, `MAX_NAME_LEN=64`, `MAX_QUERY_LIMIT=256`
- codecs `encode_msg/decode_msg/encode_query/decode_query/encode_reply/decode_reply` (serde_json, mirroring profiles)

- [ ] **Step 1: Crate scaffolding + workspace registration**

`crates/system/identity/Cargo.toml`:

```toml
[package]
name = "identity"
edition.workspace = true
version.workspace = true

# the user->nodes binding registry as replicated state: one USER (an ed25519
# keypair held by the person, app-side) owns many NODES (each a mesh/valset
# identity). depends only on sdk + valset wire types (to member-gate binds) +
# commonware-cryptography (to verify user certificates deterministically).
[dependencies]
sdk = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
valset = { workspace = true }
commonware-cryptography = { workspace = true }
sha2 = { workspace = true }
async-trait = { workspace = true }

[dev-dependencies]
futures = "0.3"
```

Add to root `Cargo.toml`: `"crates/system/identity"` in `[workspace] members` (alphabetical near `crates/system/governance`), and `identity = { path = "crates/system/identity" }` in `[workspace.dependencies]`.

- [ ] **Step 2: Write `interface.rs` (types, namespaces, preimages, codecs) with unit tests**

```rust
//! the identity module's public wire surface -- types only.
//!
//! a USER is an ed25519 keypair held by the person (in the app), a NODE is a
//! workspace's mesh/valset identity. this module binds nodes to users so any
//! verified submit origin (a node key) resolves to the human who owns it.
//! writes go via [`IdentityMsg`]; reads via [`IdentityQuery`] ->
//! [`IdentityReply`]. bind/unbind carry USER-KEY SIGNATURES over
//! chain-and-nonce-scoped preimages so a certificate can never replay across
//! networks or after an unbind.

use serde::{Deserialize, Serialize};

/// signing domain for bind certificates -- namespace-separated from every
/// other signed artifact (frames, invites, coord caps, endpoint records).
pub const IDENTITY_BIND_NS: &[u8] = b"ducktape-identity-bind-v1";
/// signing domain for unbind certificates.
pub const IDENTITY_UNBIND_NS: &[u8] = b"ducktape-identity-unbind-v1";

/// max user display-name length, in bytes (profiles' exact limit).
pub const MAX_NAME_LEN: usize = 64;
/// query pagination ceiling -- [`IdentityQuery::All`] clamps `limit` to this.
pub const MAX_QUERY_LIMIT: u64 = 256;

/// one user record as queries expose it.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct UserView {
    pub user_key: Vec<u8>,
    pub display_name: Option<String>,
    /// replay guard: every accepted user-signed op must sign the CURRENT
    /// nonce, and acceptance bumps it.
    pub nonce: u64,
    pub nodes: Vec<Vec<u8>>,
    pub updated_at: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IdentityMsg {
    /// bind the SUBMITTING NODE (the verified origin -- never a payload field)
    /// to `user_key`. `user_sig` is the user key's signature over
    /// [`bind_preimage`] with the user's current nonce (0 when the user record
    /// does not exist yet). both consents ride one op: the node consents by
    /// being the origin, the user by the signature.
    BindNode { user_key: Vec<u8>, user_sig: Vec<u8> },
    /// remove `node_key` from its user's set. user-signed over
    /// [`unbind_preimage`]; accepted from ANY external origin so a surviving
    /// device can evict a lost one. bumps the nonce, killing stale bind certs.
    UnbindNode { node_key: Vec<u8>, user_sig: Vec<u8> },
    /// set the display name of the user the SUBMITTING NODE is bound to.
    /// origin-gated (a bound node is user-trusted hardware); empty trim
    /// clears, over [`MAX_NAME_LEN`] bytes rejects.
    SetUserName { display_name: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IdentityQuery {
    /// every user, ascending by user key, offset+limit paginated.
    All { from: u64, limit: u64 },
    /// one user by user key.
    Get { user_key: Vec<u8> },
    /// the user owning `node_key`, if bound -- the resolver other modules and
    /// the app read through.
    UserOf { node_key: Vec<u8> },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IdentityReply {
    Users(Vec<UserView>),
    User(Option<UserView>),
}

/// the signed preimage of a bind certificate: length-prefixed chain id +
/// node key + nonce, so no field boundary ambiguity exists and a cert minted
/// for one network can never bind a node on another.
pub fn bind_preimage(chain_id: &str, node_key: &[u8], nonce: u64) -> Vec<u8> {
    preimage(chain_id, node_key, nonce)
}

/// the signed preimage of an unbind certificate (same shape, different
/// namespace at signing time).
pub fn unbind_preimage(chain_id: &str, node_key: &[u8], nonce: u64) -> Vec<u8> {
    preimage(chain_id, node_key, nonce)
}

fn preimage(chain_id: &str, node_key: &[u8], nonce: u64) -> Vec<u8> {
    let chain = chain_id.as_bytes();
    let mut out = Vec::with_capacity(16 + chain.len() + node_key.len() + 8);
    out.extend_from_slice(&(chain.len() as u64).to_le_bytes());
    out.extend_from_slice(chain);
    out.extend_from_slice(&(node_key.len() as u64).to_le_bytes());
    out.extend_from_slice(node_key);
    out.extend_from_slice(&nonce.to_le_bytes());
    out
}

pub fn encode_msg(m: &IdentityMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}
pub fn decode_msg(b: &[u8]) -> Result<IdentityMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_query(q: &IdentityQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}
pub fn decode_query(b: &[u8]) -> Result<IdentityQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_reply(r: &IdentityReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}
pub fn decode_reply(b: &[u8]) -> Result<IdentityReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preimage_is_length_prefixed_and_deterministic() {
        let a = bind_preimage("net-a", &[1u8; 32], 0);
        let b = bind_preimage("net-a", &[1u8; 32], 0);
        assert_eq!(a, b);
        // chain id and node key cannot bleed into each other
        assert_ne!(bind_preimage("ab", &[1, 2, 3], 0), bind_preimage("a", &[98, 1, 2, 3], 0));
        // nonce moves the preimage
        assert_ne!(bind_preimage("n", &[1u8; 32], 0), bind_preimage("n", &[1u8; 32], 1));
    }

    #[test]
    fn msg_codec_roundtrips() {
        for m in [
            IdentityMsg::BindNode { user_key: vec![7; 32], user_sig: vec![9; 64] },
            IdentityMsg::UnbindNode { node_key: vec![1; 32], user_sig: vec![2; 64] },
            IdentityMsg::SetUserName { display_name: "eddy".into() },
        ] {
            assert_eq!(decode_msg(&encode_msg(&m)).unwrap(), m);
        }
        let q = IdentityQuery::UserOf { node_key: vec![3; 32] };
        assert_eq!(decode_query(&encode_query(&q)).unwrap(), q);
        let r = IdentityReply::User(None);
        assert_eq!(decode_reply(&encode_reply(&r)).unwrap(), r);
    }
}
```

`lib.rs` stub for now:

```rust
//! deterministic user->nodes binding registry. module impl lands next task.
mod interface;
pub use interface::*;
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p identity`
Expected: PASS (2 tests).

- [ ] **Step 4: Commit**

```bash
git add crates/system/identity Cargo.toml Cargo.lock
git commit -m "feat(identity): wire surface for the user->nodes binding registry"
```

---

### Task 2: `identity` module — state machine (`lib.rs` execute/query)

**Files:**
- Modify: `crates/system/identity/src/lib.rs` (replace stub)

**Interfaces:**
- Consumes: Task 1 types; `sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle}`; `commonware_cryptography::{ed25519::{PublicKey, Signature}, Verifier as _}` (use the same decode/verify calls as `crates/kernel/node/src/lib.rs::decode_frame`); valset wire types for the member gate (copy `crates/system/capability/src/lib.rs::members`, but query BOTH `ValsetQuery::Validators` and `ValsetQuery::Observers` and union them).
- Produces: `pub struct Identity` with `Identity::new(id, valset_id: Option<ModuleId>, chain_id: String)`; `pub fn snapshot(&self) -> Vec<u8>` / `pub fn install(&mut self, bytes, expected) -> Result<(), Error>` (implemented Task 3 — stub them returning canonical bytes of committed state / unimplemented error for now is NOT allowed; Task 3 provides them, so in this task implement `root()` over canonical bytes and leave snapshot/install OUT until Task 3 — `state_sync_handle` lands in Task 3 too).

Internal state (committed + staged overlay, mirroring profiles):

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
struct UserRecord {
    display_name: Option<String>,
    nonce: u64,
    nodes: BTreeSet<Vec<u8>>,
    updated_at: u64,
}

pub struct Identity {
    id: ModuleId,
    valset_id: Option<ModuleId>,
    chain_id: String,
    users: BTreeMap<Vec<u8>, UserRecord>,          // committed
    node_index: BTreeMap<Vec<u8>, Vec<u8>>,        // committed, derived: node -> user
    pending: BTreeMap<Vec<u8>, Option<UserRecord>>, // staged per-user upserts/clears
}
```

Execution rules (each rejection is a deterministic `Error::Module` with a specific message):

- `origin_key(ctx)`: exactly profiles' — `Origin::External(non-empty)` else reject.
- `BindNode`:
  1. `user_key` must decode as a valid ed25519 point (`PublicKey::decode`), else "bind user_key is not a valid ed25519 key".
  2. member gate: when `valset_id` is `Some`, the origin must be in validators ∪ observers, else "bind origin is not a network member or observer". (When `None` — daemon shape — ungated.)
  3. resolve current binding of origin node via merged view: if bound to `user_key` already → idempotent no-op `Ok(())` (do NOT bump nonce). If bound to a different user → "node is already bound to another user; unbind first".
  4. fetch merged user record for `user_key` (or fresh `{display_name: None, nonce: 0, nodes: {}, updated_at: 0}`), verify `Signature::decode(user_sig)` + `pubkey.verify(IDENTITY_BIND_NS, &bind_preimage(&self.chain_id, &origin, record.nonce), &sig)`, else "bind certificate does not verify".
  5. stage: `record.nodes.insert(origin)`, `record.nonce += 1`, `record.updated_at = ctx.env().consensus_time`; write to `pending` under `user_key`.
- `UnbindNode { node_key, user_sig }`:
  1. merged `node_index` must map `node_key` to some `user_key`, else "node is not bound".
  2. verify against that user's merged record nonce with `IDENTITY_UNBIND_NS` and `unbind_preimage`, else "unbind certificate does not verify".
  3. stage: remove node from set, bump nonce, stamp `updated_at`. The record persists even with an empty node set (name + nonce survive).
  4. NO member gate and NO origin restriction beyond external (recovery path).
- `SetUserName { display_name }`:
  1. merged `node_index` must map the ORIGIN to a user, else "origin node is not bound to a user".
  2. trim; empty → stage `display_name = None`; `> MAX_NAME_LEN` bytes → reject "display name exceeds the 64-byte limit"; else stage `Some(trimmed)`, stamp `updated_at`. Nonce is NOT bumped (no user signature consumed).
- Queries run over the merged (pending-over-committed) view; `All` sorted ascending by user key, `from`/`limit` semantics identical to profiles (`limit.min(MAX_QUERY_LIMIT)`); `UserOf` walks `merged_index()`.
- `commit_block`: fold `pending` into `users` AND rebuild affected `node_index` entries (remove all index entries pointing at the touched user, reinsert from the new set; a `None` clears the user and its index entries). `abort_block`: clear pending.
- `root()`: Task 2 gives the canonical-bytes + sha256 implementation over COMMITTED `users` only (index is derived; excluded):
  canonical bytes = `u64-le user count`, then per sorted user: `len+user_key`, `u8 name-present flag + (len+name bytes if present)`, `u64-le nonce`, `u64-le node count` then per sorted node `len+node bytes`, `u64-le updated_at`. Empty map → root `StateRoot::ZERO` (match capability: snapshot of empty is the lone zero count, root ZERO unhashed).

The merged view helpers (write these exactly once):

```rust
fn merged_users(&self) -> BTreeMap<Vec<u8>, UserRecord> { /* profiles::merged pattern */ }
fn merged_record(&self, user_key: &[u8]) -> Option<UserRecord> { /* pending-over-committed */ }
fn merged_index(&self) -> BTreeMap<Vec<u8>, Vec<u8>> { /* derive from merged_users */ }
```

- [ ] **Step 1: Write the failing test matrix first** (same-file `#[cfg(test)] mod tests`). Copy capability's `TestCtx` (`crates/system/capability/src/lib.rs:385-430`) but make `query` decode the valset request and answer BOTH variants:

```rust
async fn query(&self, t: &str, r: &[u8]) -> Result<Vec<u8>, Error> {
    if t != "valset" { return Err(Error::QueryUnsupported); }
    let q = valset::decode_query(r).map_err(Error::Module)?;
    match (q, &self.members, &self.observers) {
        (valset::ValsetQuery::Validators, Some(m), _) =>
            Ok(valset::encode_reply(&valset::ValsetReply::Validators(m.clone()))),
        (valset::ValsetQuery::Observers, _, Some(o)) =>
            Ok(valset::encode_reply(&valset::ValsetReply::Observers(o.clone()))),
        _ => Err(Error::QueryUnsupported),
    }
}
```

Test helpers: `fn user() -> ed25519::PrivateKey { ed25519::PrivateKey::from_seed(7) }` (commonware), `fn bind_msg(user: &PrivateKey, chain: &str, node: &[u8], nonce: u64) -> Msg` building the signed `IdentityMsg::BindNode`. Cover, with `futures::executor::block_on` like capability's tests:

1. `bind_happy_path_binds_and_bumps_nonce` — gated ctx with node in validators; after execute+commit, `UserOf(node)` → user with `nonce == 1`, `nodes == [node]`.
2. `bind_rejects_wrong_signature` — sig from a different user key.
3. `bind_rejects_wrong_chain` — cert signed over another chain id.
4. `bind_rejects_stale_nonce` — bind, commit, then replay the SAME cert (nonce 0) from a second node → rejected; fresh cert with nonce 1 for node2 → accepted, user has 2 nodes.
5. `bind_same_user_is_idempotent` — second identical bind after commit → `Ok`, nonce unchanged, still 1 node.
6. `bind_rejects_second_user_takeover` — node bound to user A; bind cert from user B → "already bound".
7. `bind_rejects_non_member_when_gated` / `bind_ungated_without_valset` — observer-only origin passes the gate too (observers count).
8. `unbind_from_any_origin_with_valid_cert` — bind node1+node2 to user; submit `UnbindNode{node_key: node1}` from origin node2 (or a third key) with nonce-2 cert → node1 gone; replayed node1 BIND cert (old nonce) now rejects.
9. `unbind_rejects_unbound_node` and `unbind_bad_cert`.
10. `set_user_name_origin_gated` — bound origin sets name; unbound origin rejected; 65-byte name rejected; empty trim clears name but record survives.
11. `queries_read_staged_overlay` — read-your-writes before commit; `abort_block` drops staged bind.
12. `root_changes_on_commit_only` — root stable across staged-but-uncommitted writes; changes after commit; empty registry roots to `StateRoot::ZERO`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p identity`
Expected: FAIL — `Identity` not defined / tests don't compile. (Compile failure counts; the matrix defines the contract.)

- [ ] **Step 3: Implement the module** per the rules above. Module trait wiring mirrors profiles (`crates/apps/profiles/src/lib.rs:138-219`): `id()`, `root()`, `execute` (decode → dispatch), `query`, `commit_block`, `abort_block`. Leave `state_sync_handle` unimplemented until Task 3 (default trait impl if one exists; otherwise implement it in Task 3's step).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p identity`
Expected: PASS (all matrix tests + Task 1 tests).

- [ ] **Step 5: Clippy + commit**

Run: `cargo clippy -p identity -- -D warnings` → clean.

```bash
git add crates/system/identity
git commit -m "feat(identity): user->nodes binding state machine with cert verification"
```

---

### Task 3: `identity` snapshot/install (state-sync surface)

**Files:**
- Modify: `crates/system/identity/src/lib.rs`

**Interfaces:**
- Produces: `pub fn snapshot(&self) -> Vec<u8>` (canonical bytes of COMMITTED users — the exact `root()` preimage), `pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error>` (strict decode → recompute root → adopt + rebuild `node_index` + clear pending), `fn state_sync_handle(&self) -> Result<StateSyncHandle, Error>` returning `StateSyncHandle::SnapshotBytes(self.snapshot())`.

- [ ] **Step 1: Write failing tests** (append to the module test mod):

1. `snapshot_is_root_preimage` — non-empty registry: `sha256(snapshot()) == root()` bytes; empty registry snapshots to lone zero count and roots to ZERO.
2. `install_roundtrip_rebuilds_index` — build a 2-user/3-node registry, snapshot, install into a fresh `Identity`, assert `root()` equal and `UserOf` resolves every node.
3. `install_rejects_root_mismatch` — flip a byte in expected root → error, state untouched.
4. `install_rejects_malformed` — truncated bytes; trailing bytes; non-increasing user keys; non-increasing node keys within a user; name flag 1 with over-long name; forged count exceeding buffer (`count > buf.len()/16` guard, capability's pattern) — each rejects and leaves prior state byte-identical.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p identity`
Expected: FAIL (snapshot/install undefined).

- [ ] **Step 3: Implement** — strict decode copies capability's discipline (`crates/system/capability/src/lib.rs:160-260`): every length checked against remaining buffer before allocation, strictly-increasing keys, exactly one valid encoding per state (name-present flag must be 0 or 1; `display_name` non-empty when present). Decode into a temporary; mutate `self` only after root matches; success clears `pending` and rebuilds `node_index` from the adopted map. Duplicate node claimed by two users must reject ("node bound twice in snapshot") — the execute path can never commit that state.

- [ ] **Step 4: Run tests**

Run: `cargo test -p identity`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/system/identity
git commit -m "feat(identity): trust-free snapshot/install state-sync surface"
```

---

### Task 4: register `identity` in every host composition

**Files:**
- Modify: `bin/node/src/main.rs` — `MODULE_IDS` (22→23, add `"identity"` after `"profiles"`), `genesis_host` (new param `chain_id: &str`; register `Box::new(Identity::new("identity", Some("valset".into()), chain_id.to_string()))` next to profiles), `restore_host` (same param; construct + `snapshot_of("identity")` + `install`, exactly the capability restore block at ~`main.rs:763-772`), `sync_all_modules` joiner compose (~`main.rs:894-1139`: sync the snapshot lane + include in the compose vec), and the call sites (`genesis_host(...)` at ~`:5026`, `restore_host(...)` at ~`:5073` — both inside `run_node` where `config.chain_id` is in scope; the joiner path threads the same).
- Modify: `bin/node/Cargo.toml` — add `identity = { workspace = true }`.
- Modify: `bin/noded/src/main.rs` — module id list (~`:74`) + registry (~`:208/:224`): `Identity::new("identity", None, String::new())` (daemon has no valset and no chain; ungated, chain-unscoped certs are a dev-only surface). Cargo.toml dep.
- Modify: `bin/noded/src/lib.rs` — `ModuleCategory::of` mapping: `"identity"` → `System` (see the test at `lib.rs:2069-2094` enumerating categories — extend it).
- Modify: `bin/simnode/src/main.rs` (~`:107/:396/:412`) — same as noded (`None` valset, empty chain unless simnode carries a chain id — if it does, pass it). Cargo.toml dep.
- Modify: `bin/demo/src/main.rs` (~`:85`) — register `Identity::new("identity", None, "demo".into())` beside Profiles. Cargo.toml dep.

**Interfaces:**
- Consumes: `Identity::new(id, valset_id: Option<ModuleId>, chain_id: String)`, `snapshot()`, `install()` from Tasks 2–3.
- Produces: `"identity"` present in every `MODULE_IDS`-style list and every `Host::genesis` vec; `genesis_host`/`restore_host` signatures gain `chain_id: &str`.

- [ ] **Step 1: Wire everything above.** Grep to enumerate every composition before editing:

Run: `grep -rn "Profiles::new(\"profiles\")" --include=*.rs bin/ | grep -v target`
Expected hits: `bin/node/src/main.rs` ×3 (genesis/restore/joiner), `bin/noded/src/main.rs`, `bin/simnode/src/main.rs`, `bin/demo/src/main.rs`. Add an `Identity` line adjacent to each `Profiles` line, plus the restore/joiner install lanes where the in-memory cohort installs snapshots (follow `capability`'s blocks in the same functions).

- [ ] **Step 2: Build + run the workspace module/id tests**

Run: `cargo build -p node-bin -p noded -p simnode -p demo 2>&1 | tail -5` (package names per each `Cargo.toml`; verify with `grep '^name' bin/*/Cargo.toml`)
Expected: clean build.
Run: `cargo test -p noded module_category` (the category enumeration test) and `cargo test -p demo`
Expected: PASS — `joiner_rebuilds_global_app_hash` in demo proves the new module composes into app-hash parity.

- [ ] **Step 3: Run the node's own test suite (slow, but the joiner/cluster proofs live here)**

Run: `cargo test -p node-bin 2>&1 | tail -20`
Expected: PASS (pre-existing suites — `network_joiner_full`, `cluster_e2e`, `invite_e2e` — still green with the wider module set).

- [ ] **Step 4: Commit**

```bash
git add bin/ Cargo.lock
git commit -m "feat(identity): register the binding registry in every host composition"
```

---

### Task 5: `ducktape-node` user-key CLI verbs

**Files:**
- Modify: `bin/node/src/config.rs` — nothing new needed for load/generate (`load_or_generate_identity(path)` at `config.rs:75` is already path-based and public); add two pure helpers + tests:
  - `pub fn mint_bind_cert(user: &ed25519::PrivateKey, chain_id: &str, node_pub: &[u8], nonce: u64) -> Vec<u8>` → `user.sign(identity::IDENTITY_BIND_NS, &identity::bind_preimage(chain_id, node_pub, nonce)).as_ref().to_vec()`
  - `pub fn mint_unbind_cert(...)` (UNBIND ns/preimage).
- Modify: `bin/node/src/main.rs` — three CLI verbs beside `keygen` (~`:2410`):
  - `user-key --out <path>` → `load_or_generate_identity`, print pubkey hex (identical contract to `keygen`).
  - `user-sign-bind --key <path> --chain-id <id> --node-pub <hex> --nonce <n>` → print the ready-to-submit `IdentityMsg::BindNode` JSON (`{"bind_node":{"user_key":[...],"user_sig":[...]}}` — emit via `identity::encode_msg`).
  - `user-sign-unbind --key <path> --chain-id <id> --node-pub <hex> --nonce <n>` → the `UnbindNode` JSON.
- Modify: `bin/node/Cargo.toml` if `identity` dep wasn't added in Task 4.

**Interfaces:**
- Consumes: `identity::{IDENTITY_BIND_NS, bind_preimage, encode_msg, IdentityMsg}`; `config::load_or_generate_identity`.
- Produces: the three verbs; stdout contract = last line is the value (pubkey hex / msg JSON), matching how `workspaces.rs::run_verb(...).map(|out| last_line(&out))` consumes verbs.

- [ ] **Step 1: Write failing config tests** (in `config.rs`'s test mod): `bind_cert_verifies_against_module_preimage` — mint with `from_seed(1)` user, verify with `PublicKey.verify(IDENTITY_BIND_NS, &bind_preimage(...), &Signature::decode(...))`; a cert minted for chain A fails verification for chain B; unbind cert cross-checks the UNBIND namespace (bind cert must NOT verify under unbind ns).

- [ ] **Step 2: Run to verify failure** — `cargo test -p node-bin mint_bind` → FAIL (undefined).

- [ ] **Step 3: Implement helpers + verbs.** Follow the existing verb parsing style around `keygen`/`init` (~`main.rs:2400-2470`). Verify by hand:

Run: `cargo run -p node-bin -- user-key --out /tmp/claude-user.key` twice
Expected: same pubkey hex both times (second run reuses).
Run: `cargo run -p node-bin -- user-sign-bind --key /tmp/claude-user.key --chain-id test@abc --node-pub $(printf 'aa%.0s' {1..32}) --nonce 0`
Expected: one JSON line decodable by `identity::decode_msg`.

- [ ] **Step 4: Run tests** — `cargo test -p node-bin mint_ 2>&1 | tail -5` → PASS.

- [ ] **Step 5: Commit**

```bash
git add bin/node
git commit -m "feat(node): user-key custody + bind/unbind certificate CLI verbs"
```

---

### Task 6: Tauri user-identity commands

**Files:**
- Create: `app/src-tauri/src/user_identity.rs`
- Modify: `app/src-tauri/src/main.rs` — module decl + `invoke_handler` registration (find the `workspace_*` command list).
- Modify: `app/src-tauri/src/workspaces.rs` — make `run_verb`/`last_line` and the `~/.ducktape` base-dir helper `pub(crate)` if private.

**Interfaces:**
- Consumes: `crate::daemon::resolve_node_bin()`, `run_verb`, `last_line` (workspaces.rs patterns at `:331-374`).
- Produces (Tauri commands, camelCase over the wire like workspaces.rs):
  - `user_identity_status() -> Result<UserIdentity, String>` where `UserIdentity { pubkey: String }` — ensures `~/.ducktape/user.key` via `user-key --out`.
  - `user_sign_bind(chain_id: String, node_pub: String, nonce: u64) -> Result<String, String>` — returns the IdentityMsg JSON string from `user-sign-bind`.
  - `user_sign_unbind(chain_id: String, node_pub: String, nonce: u64) -> Result<String, String>`.

```rust
//! user-key custody for the desktop shell. the USER key (`~/.ducktape/user.key`)
//! is machine-per-user, shared by every workspace -- unlike `identity.key`,
//! which is per workspace. the shell never parses or holds the secret: it
//! shells out to `ducktape-node` verbs exactly like workspace keygen, and only
//! signatures/pubkeys cross this boundary.
```

- [ ] **Step 1: Implement** (no direct unit test lane exists for tauri commands in this repo — the workspaces module is tested through the TS store tests; keep functions thin over `run_verb`). Build check:

Run: `cargo check -p ducktape-desktop 2>&1 | tail -3`
Expected: clean.

- [ ] **Step 2: Commit**

```bash
git add app/src-tauri
git commit -m "feat(desktop): user-key custody commands (status, sign-bind, sign-unbind)"
```

---

### Task 7: TS identity client + name resolution overlay

**Files:**
- Create: `app/src/domain/identity-client.ts`
- Create: `app/src/domain/identity-client.test.ts`
- Modify: `app/src/console/store/state.ts` — add `nodeUsers: Record<string, { userKey: string; name: string | null }>` (hex node key → owner), default `{}` in the initial state (~`:362`).
- Modify: `app/src/console/store/DucktapeProvider.tsx` — fetch `allUsers` beside `allProfiles` (~`:143`), build `nodeUsers`, and OVERLAY user names into `authorNames` (~`:173`): profiles map first, then for each user with a `display_name`, for each of its nodes `authorNames[hex(node)] = display_name`. Store both in state (~`:247`).

**Interfaces:**
- Consumes: `NodeTransport` (`transport.submit(target, payload, origin?)`, `transport.query(target, req)`), `replyVariant` from `./wire` — mirror `profiles-client.ts` exactly.
- Produces:

```ts
export interface UserView {
  user_key: number[];
  display_name: string | null;
  nonce: number;
  nodes: number[][];
  updated_at: number;
}
export const TARGET = "identity";
export const allUsers = (transport, {from = 0, limit = 256} = {}): Promise<UserView[]>  // query {all:{from,limit}} → replyVariant "users"
export const userOf = (transport, nodeKeyHex: string): Promise<UserView | null>         // query {user_of:{node_key: bytes}} → replyVariant "user"
export const getUser = (transport, userKeyHex: string): Promise<UserView | null>        // query {get:{user_key: bytes}}
export const submitRawMsg = (transport, msgJson: string): Promise<BlockEvent>           // JSON.parse and submit to "identity" (the tauri-signed payload)
export const setUserName = (transport, params: {displayName: string; origin: string}): Promise<BlockEvent>
export const hexToBytes = (hex: string): number[]  // shared helper (check app/src/domain/wire.ts first; reuse if one exists)
```

- [ ] **Step 1: Write failing client tests** — mirror `profiles-client.test.ts` (mock transport capturing `(target, payload)`, canned replies): `allUsers` sends `{all:{from:0,limit:256}}` and unwraps `users`; `userOf` hex→bytes conversion; `submitRawMsg` passes the parsed object through untouched.

- [ ] **Step 2: Run to verify failure** — `cd app && bun run test -- --run src/domain/identity-client.test.ts` → FAIL (module missing).

- [ ] **Step 3: Implement client + provider overlay.** In the provider, the overlay must be built AFTER profiles so user names win:

```ts
const authorNames: Record<string, string> = Object.fromEntries(
  profiles.map((p) => [hex(p.key), p.display_name]),
);
const nodeUsers: Record<string, { userKey: string; name: string | null }> = {};
for (const u of users) {
  const userKey = hex(u.user_key);
  for (const node of u.nodes) {
    const nodeHex = hex(node);
    nodeUsers[nodeHex] = { userKey, name: u.display_name };
    if (u.display_name) authorNames[nodeHex] = u.display_name;
  }
}
```

(`hex` = the provider's existing byte→hex helper used for profiles at `DucktapeProvider.tsx:173`.)

- [ ] **Step 4: Run tests** — `cd app && bun run test -- --run src/domain/identity-client.test.ts` → PASS; then the full app suite `cd app && bun run test -- --run 2>&1 | tail -5` → no regressions.

- [ ] **Step 5: Commit**

```bash
git add app/src/domain/identity-client.ts app/src/domain/identity-client.test.ts app/src/console/store
git commit -m "feat(app): identity client + user-name overlay onto author resolution"
```

---

### Task 8: auto-bind on workspace connect

**Files:**
- Modify: `app/src/console/store/actions.ts` — after the successful identity check on connect (`identityMatches` / `rejectImpostor` block, ~`:631-702`), fire-and-forget `autoBindUserIdentity(...)`.
- Create: `app/src/console/store/auto-bind.ts` (pure, testable helper) + `auto-bind.test.ts`.

**Interfaces:**
- Consumes: `isTauri()` (`app/src/domain/node-bootstrap.ts`), `invoke` (`@tauri-apps/api/core`), `userOf`, `getUser`, `submitRawMsg` (Task 7), `state.workspace` (`{ chainId, pubkey }` from `workspace-client.ts`).
- Produces: `autoBindUserIdentity(transport: NodeTransport, workspace: { chainId: string; pubkey: string }): Promise<"bound" | "already" | "skipped" | "failed">`:

```ts
// 1. desktop only: if (!isTauri()) return "skipped";
// 2. const bound = await userOf(transport, workspace.pubkey); if (bound) return "already";
// 3. const { pubkey: userKey } = await invoke("user_identity_status");
// 4. const user = await getUser(transport, userKey); const nonce = user?.nonce ?? 0;
// 5. const msg = await invoke("user_sign_bind", { chainId: workspace.chainId, nodePub: workspace.pubkey, nonce });
// 6. await submitRawMsg(transport, msg); return "bound";
// every step wrapped: any throw -> "failed" (silent; next connect retries; a
// nonce race between two devices is exactly this path)
```

- [ ] **Step 1: Write failing tests** for `autoBindUserIdentity` with a mocked `invoke` (vi.mock `@tauri-apps/api/core`) and stub transport: already-bound short-circuits (no invoke calls); fresh bind walks steps 3–6 with nonce 0; existing user with nonce 3 signs nonce 3; a rejected submit resolves `"failed"` not a throw.

- [ ] **Step 2: Run to verify failure** — `cd app && bun run test -- --run src/console/store/auto-bind.test.ts` → FAIL.

- [ ] **Step 3: Implement + hook into the connect action** (fire-and-forget with `.then(refresh identity state)` — after a successful bind re-run the provider's identity fetch or trigger the existing block-event refresh; check how profiles refresh after `setName` at `actions.ts:778-785` and do the same).

- [ ] **Step 4: Run tests** — auto-bind tests + `cd app && bun run test -- --run src/console/store 2>&1 | tail -5` (store suites regression) → PASS.

- [ ] **Step 5: Commit**

```bash
git add app/src/console/store
git commit -m "feat(app): auto-bind the workspace node to the machine user key on connect"
```

---

### Task 9: UI — members grouping + settings devices strip

**Files:**
- Modify: `app/src/console/views/members/MembersView.tsx` — group validator/observer rows by `nodeUsers[key]?.userKey`; bound groups render one user header (name or `shortKey(userKey)`) with node rows nested; unbound keys render exactly as today.
- Modify: `app/src/console/views/settings/SettingsView.tsx` (locate via `grep -rn "display name" app/src/console/views/settings/`) — add a "Devices" section: this workspace's node key (short), bind state ("Linked to <user name/short>" / "Not linked"), the user's other nodes; display-name edit calls `setUserName` (identity) when this node is bound, else the existing profiles `setName`.
- Modify tests: `MembersView` / `SettingsView` existing test files (grep `MembersView.test`), extend with one grouping case and one devices-render case using seeded `nodeUsers` state.

**Interfaces:**
- Consumes: `state.nodeUsers`, `state.authorNames`, `displayNameForKey` (unchanged signature — user names already overlaid by Task 7), `identity-client.setUserName`.

- [ ] **Step 1: Extend the view tests first** (failing): two members sharing a user render one group header with the user name and two node rows; settings shows "Linked" when `nodeUsers[workspace.pubkey]` exists.
- [ ] **Step 2: Run to verify failure** — `cd app && bun run test -- --run src/console/views/members src/console/views/settings` → FAIL.
- [ ] **Step 3: Implement the two views.** Keep markup minimal and consistent with each view's existing row components; no new CSS files — reuse the classes already in the views.
- [ ] **Step 4: Run tests** — same command → PASS; full app suite → no regressions.
- [ ] **Step 5: Commit**

```bash
git add app/src/console/views
git commit -m "feat(app): user-grouped members roster + linked-devices settings strip"
```

---

### Task 10: node-level integration test — one user, two nodes

**Files:**
- Modify: the existing multi-node test home — find it: `grep -rln "cluster_e2e\|network_joiner_full" bin/node/tests/` — add `identity_two_nodes_one_user` beside it (same harness helpers).

**Interfaces:**
- Consumes: the cluster harness's submit + query plumbing; `identity::{encode_msg, encode_query, decode_reply}`; `config::mint_bind_cert`.

- [ ] **Step 1: Write the test:** spin the harness's standard 2-validator network (chain id from the harness); create one user key (`from_seed(42)`); node A submits `BindNode` (cert nonce 0), await finalization; node B submits its own `BindNode` (cert nonce 1); query `UserOf(A)` and `UserOf(B)` on BOTH nodes → same `user_key`, `nodes` len 2, and both nodes' app-hash / identity module root agree. Then `UnbindNode(A)` signed nonce 2 submitted FROM node B → `UserOf(A)` → null on both.
- [ ] **Step 2: Run it** — `cargo test -p node-bin identity_two_nodes_one_user -- --nocapture 2>&1 | tail -15`
Expected: PASS.
- [ ] **Step 3: Commit**

```bash
git add bin/node/tests
git commit -m "test(identity): two nodes bind to one user across a live cluster"
```

---

### Task 11: docs + spec amendment

**Files:**
- Modify: `docs/superpowers/specs/2026-07-07-user-node-identity-split-design.md` — as-built amendment note at top: chain id + nonce reach the app from the workspace registry (`workspace.chainId`, Tauri) and the `Get` query; `/v1/status` was NOT extended (registry already carries `chainId`); user-key ops ride `ducktape-node` CLI verbs, not an in-shell crypto dep.
- Modify: `docs/pages/en/human/modules/product-modules.mdx` — an `identity` section (what it stores, bind/unbind/setname semantics, the nonce, chain scoping); mirror a matching section into `docs/pages/ko/human/modules/product-modules.mdx` (follow the file's existing ko voice).
- Modify: `docs/pages/en/human/network/network-and-membership.mdx` — one paragraph: membership stays per-node; the identity module maps nodes to users above it. Mirror to `docs/pages/ko/...` counterpart.
- Modify: `bin/node/src/config.rs:75` doc comment — "(and for now the user's)" is no longer true; reword to point at the identity module for user identity.

- [ ] **Step 1: Write all four doc edits.**
- [ ] **Step 2: Build docs if a build exists** — `ls docs/package.json` and if present `cd docs && bun run build 2>&1 | tail -3` (vocs); otherwise skip.
- [ ] **Step 3: Commit**

```bash
git add docs bin/node/src/config.rs
git commit -m "docs(identity): module docs + spec as-built amendments"
```

---

### Task 12: full verification sweep

- [ ] **Step 1: Rust** — `cargo test --workspace 2>&1 | tail -15` → all green; `cargo clippy -p identity -- -D warnings` → clean; node-bin clippy = no NEW warnings vs dev (known toolchain-drift caveat).
- [ ] **Step 2: App** — `cd app && bun run test -- --run 2>&1 | tail -6` → green; `bun run build 2>&1 | tail -3` (typecheck) → clean.
- [ ] **Step 3: Live smoke (headless)** — per the repo's tauri-debug/qa skills if feasible: create a workspace, confirm auto-bind fired (query `UserOf` via `curl -s localhost:<http>/v1/query` lane or the console UI), rename, see the members roster show the user name. If the desktop lane is too heavy, the Task 10 cluster test plus a `noded`-backed vitest e2e (`live-daemon.e2e.test.ts` pattern) is the accepted floor.
- [ ] **Step 4: Commit any fixes; then hand off to PR flow** (superpowers:verification-before-completion → PR to `dev`).
