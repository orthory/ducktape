# User-Key / Node-Key Split — Design of Record

Status: design of record for the identity split (`feat/identity-split`). Goal:
**one user can own multiple nodes.** A user is an ed25519 keypair held by the
person (app-side); a node keeps its own ed25519 `identity.key`. A replicated
`identity` module binds node keys to user keys so every peer resolves any
verified submit origin (a node key) to the human who owns that node.

## Why now, and what stays put

Today one key is everything: `identity.key` is the mesh identity, the
validator seat, the frame signer, and — because the networked node signs every
submit with it and discards the app's claimed origin
(`bin/node/src/main.rs` submit handler) — the on-chain author, the profile
owner, and every module ACL principal. `bin/node/src/config.rs` says it
outright: "it is the node's (and for now the user's) whole identity."

The split keeps every layer that is *correctly* per-node exactly as it is:

- **Mesh / p2p tracking** (`oracle.track` sets of ed25519 keys) — per node.
- **valset seats, quorum arithmetic, governance votes** — per node. One user
  running three validators holds three seats and three votes; that is a
  deliberate v1 decision (see Resolved decisions). Second devices should join
  as **observers**, which carry no quorum weight.
- **NAT rendezvous `NodeKey`, coordinator PoP/caps, WireGuard endpoint
  records, overlay ULAs** — per node.
- **The `Frame` wire format and where it is verified** (in the ordered drain)
  — untouched. The frame signer remains the node key; `Origin::External`
  remains node-key bytes everywhere it is stored today.

What changes: a **user keypair** exists above the node, a consensus module
records **user → {node keys}**, and the app renders authorship, rosters, and
"who am I" at the user level by resolving node keys through that module.

## Architecture

Three units, one new seam:

1. **`crates/system/identity`** — the replicated binding registry (new
   module, id `"identity"`). Profiles-shaped: staged overlay, canonical
   snapshot bytes, state-based sha256 root, trust-free `snapshot`/`install`.
   Verifies user-key signatures deterministically inside `execute`
   (ed25519 verify is pure; same determinism class as `decode_frame` running
   in the drain).
2. **User key custody in the desktop app** (`app/src-tauri`) — a per-OS-user
   keypair at `~/.ducktape/user.key` (hex seed, 0600, `create_new`), shared by
   every workspace on the machine. The Tauri layer generates/loads it and
   signs bind certificates; the private key never leaves the app process and
   is never sent to the node or the network — only signatures travel.
3. **App resolution layer** — fetch the identity registry alongside profiles,
   build `nodeKeyHex → user`, and prefer user display names wherever node
   keys render today (chat authors, members roster, governance, explorer).

### The identity module

State (committed, canonical order):

```text
users: BTreeMap<UserKey(Vec<u8>), UserRecord {
    display_name: Option<String>,   # user-level name, may shadow profiles
    nonce: u64,                     # replay guard, +1 per accepted user-signed op
    nodes: BTreeSet<Vec<u8>>,       # bound node keys (32-byte ed25519)
    updated_at: u64,                # consensus time of last change
}>
node_index: BTreeMap<Vec<u8>, Vec<u8>>   # node → user, derived, rebuilt on install
```

`root()` = sha256 over the canonical bytes of `users` only (the index is
derived state). Snapshot/install mirror profiles: strict decode, strictly
increasing keys, root recomputed and compared before adopting.

Ops (`IdentityMsg`, JSON like every product module):

- **`BindNode { user_key, user_sig }`** — origin-gated: the submitting node
  binds ITSELF. The module verifies
  `verify(user_key, IDENTITY_BIND_NS, chain_id ‖ node_key ‖ nonce, user_sig)`
  where `node_key = ctx.env().origin` bytes and `nonce` is the user's current
  nonce (0 for a first-ever bind that creates the record). Rejections: origin
  not a valset member/observer (when valset-gated), node already bound to a
  *different* user (no silent takeover; unbind first), malformed keys, bad
  signature. Re-bind to the same user is an idempotent no-op (does not bump
  the nonce). Both consents are proven in one op: the node consents by being
  the verified origin; the user consents by the signature.
- **`UnbindNode { node_key, user_sig }`** — user-signed over
  `IDENTITY_UNBIND_NS, chain_id ‖ node_key ‖ nonce`; submitted from ANY
  external origin (lost-device recovery: a user's surviving node can evict a
  compromised one). Removes the node from the user's set; bumps the nonce so
  captured bind certs for the evicted node are dead forever.
- **`SetUserName { display_name }`** — origin-gated, no user signature: the
  origin must be a bound node of some user; sets that user's display name
  (trim-empty clears; > 64 bytes rejected — profiles' exact limits). A bound
  node is trusted hardware; requiring a user sig here would add ceremony
  without a threat model.

Signing namespaces (mirroring `INVITE_GRANT_NAMESPACE` posture):
`IDENTITY_BIND_NS = b"ducktape-identity-bind-v1"`,
`IDENTITY_UNBIND_NS = b"ducktape-identity-unbind-v1"`. The preimage includes
the `chain_id` so a cert minted for network A can never bind a (compromised)
node key on network B. The per-user `nonce` kills same-chain replay of stale
certs. The module receives `chain_id` at construction (genesis wiring knows
it), like capability receives the valset id.

Member gating copies capability: constructed with `Some("valset")` on the
networked node, `BindNode` requires the origin to be a current validator OR
observer (read via `Ctx::query`); without a valset (the `noded` daemon) the
gate is off. `UnbindNode`/`SetUserName` need no member gate (unbind is
user-sig-protected; setname requires an existing binding).

Queries: `All { from, limit }` (users, paginated), `Get { user_key }`,
`UserOf { node_key }` → `UserView { user_key, display_name, nonce, nodes,
updated_at }`. `UserOf` is also the cross-module read projection future
consumers (inbox's opaque `member`, chat admin gating) can resolve through
`Ctx::query` — none of them change in v1.

### User key custody and the bind flow

- `app/src-tauri/src/user_identity.rs` (new): `load_or_generate_user_key()`
  at `~/.ducktape/user.key` — same file discipline as `identity.key` (hex,
  0600, `create_new`), plus `sign_bind(node_pub, chain_id, nonce)` /
  `sign_unbind(...)`. Uses the ed25519 crate already in the workspace
  dependency graph; no new key format.
- New Tauri commands: `user_identity_status()` → `{ userKeyHex }`, and
  `user_sign_bind(nodePubHex, chainId, nonce)` → the signed `IdentityMsg`
  payload. The TS side submits it through the normal `transport.submit`
  lane to target `"identity"`; the node frames and signs it as usual.
- **Auto-bind on connect**: when the app connects a workspace and the node is
  synced + a member/observer, it queries `UserOf(status.publicKey)`; if
  unbound, it queries `Get(user_key)` for the user's current nonce (absent
  record → nonce 0), signs, and submits `BindNode`. Idempotent, retried on
  next connect if the network isn't accepting ops yet; a nonce race (two
  devices binding concurrently) just rejects one op and the retry re-reads
  the nonce. `/v1/status` gains `chainId` alongside `publicKey` so the app
  can build the preimage.
- **Second machine**: copying `~/.ducktape/user.key` to the other machine
  makes its workspaces bind to the same user (documented; QR/sync transport
  is deferred). Same-machine second workspace shares the file automatically —
  the fleet/multi-workspace path exercises this with zero extra steps.

### App resolution layer

- `DucktapeProvider` fetches `identity All` alongside profiles and builds
  `nodeToUser: Record<hexNodeKey, { userKeyHex, name }>`.
- Name preference order everywhere a key renders:
  **identity user name → profiles name (node-level) → shortKey**.
  Implemented once in `names.ts` (`displayNameForKey` grows a `nodeToUser`
  argument) so chat authors, members roster, governance proposers/voters, and
  the explorer all inherit it.
- **MembersView** groups seats under their user: one user card listing its
  validator/observer node rows (unbound nodes render as today).
- **Settings** gains a "Devices" strip: this machine's user key
  (short form), this workspace's node key, its bind state, and the user's
  other bound nodes. Display-name editing writes `SetUserName` (user-level)
  instead of profiles when the node is bound; profiles remains the fallback
  writer for unbound nodes.
- Chat "mine"/avatar grouping renders user-level; **edit/delete stays
  node-scoped** because chat enforces author-equality on-chain. Migrating
  per-module authorship/ACLs to user keys is the explicit follow-on epic
  (module-by-module, height-gated), not v1.

## Consensus impact and rollout

Adding the module is a **module-set change**: the genesis app-hash moves even
at zero root. Fresh networks simply include it. Existing dev networks take
the established coordinated path (same class as the video-calls engine-bank
change): rebuild or genesis-bump in lockstep. Nothing else moves — no frame
change, no existing module root change, no p2p channel addition, no namespace
change. `MODULE_IDS` grows by `"identity"` and the genesis host registers the
module on every binary that composes the full set (`bin/node`, `bin/noded`,
`bin/simnode`, demo/test hosts) so app-hash parity holds across node styles.

## Error handling

- Bad signature / wrong nonce / unknown ops → deterministic module rejection
  (the op is a no-op; honest nodes never diverge).
- Bind while bound to another user → rejected with an explicit error; the app
  surfaces "this device is already linked to <user>".
- Unbind of the user's LAST node is allowed (the user record persists with an
  empty set; their name survives re-binding).
- A node whose valset standing was revoked keeps its binding (identity is not
  membership); the app simply shows it as offline/former.
- App-side: missing/corrupt `user.key` → regenerate only via `create_new`
  discipline (never overwrite silently); a corrupt file is an explicit error
  surfaced in Settings, since overwriting would orphan the on-chain user.

## Testing

- **Module matrix** (`crates/system/identity`): bind happy path; bind with
  wrong-user sig / wrong chain / stale nonce; rebind same user (idempotent);
  rebind different user (rejected); unbind + nonce bump kills replayed bind;
  unbind from a different origin (recovery); setname bound/unbound/overlong;
  member-gate on/off; staged-overlay read-your-writes; commit/abort;
  snapshot/install roundtrip + strict-decode rejections; root stability.
- **Host/genesis**: module registers in every host composition; app-hash
  parity across `bin/node`/`noded`/`simnode` genesis.
- **Node integration**: two in-process nodes (cluster-style test), one user
  key binds both; `UserOf` resolves identically on both; convergent app-hash.
- **App**: unit tests for `nodeToUser` resolution order and the auto-bind
  state machine; existing store tests keep passing (profiles fallback).
- **e2e (manual/QA)**: fleet or tauri-debug run — two workspaces on one
  machine, one user; members roster shows one user with two nodes; chat
  messages from both nodes render one author name.

## Resolved decisions (flagged for review)

1. **Quorum/governance stay per-node in v1.** A user with N validators holds
   N seats/votes. Per-user vote weighting is a governance-policy question,
   deferred; the recommended posture is second devices join as observers.
   *(vetoable — say the word and v1 adds a governance guard instead)*
2. **The frame format does not change.** Authorship migration to user keys
   (chat `AuthorRef`, vaults owners, etc.) is the follow-on epic enabled by
   this module, done per-module with height-gated upgrades.
3. **User key lives in the app, not the node.** The node never holds user
   secrets; a headless node simply has no user (bindable later from an app).
4. **No user-key rotation/recovery in v1** beyond unbind-and-rebind under a
   new user key. Social recovery / key rotation is out of scope.
5. **`identity` is a system crate** (`crates/system/identity`): it is
   platform identity infrastructure (like capability/valset), not a product
   surface.
