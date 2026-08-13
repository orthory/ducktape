# ACL Module — Validator / Full-Node / User Separation — Design

Status: design of record for the `acl` module (principal model, policy table,
capability federation). Implementation is phased; v1 changes no wire format.
Where this document and shipped code disagree, the code is authoritative.

> **2026-08-13 — v1 shipped**, with deviations the code owns: the module is
> NATIVE store-backed (like valset), composed in `PRODUCTION` and
> `SIM_VALSET`; the policy table is EMPTY at genesis (pure allow-all — no
> seeded system-module entries; each module's own origin gates carry that
> protection); the dispatch gate lives in the kernel host's drain
> (`Host::require_submit_standing`); mutation rides
> `GovAction::SetAclPolicy`. The capability-federation half
> (`capability_policy`) is NOT implemented. Alongside this, the relay submit
> door dropped its standing check entirely (any validly signed frame enters
> consensus) and the client-standing plane was deleted — `Standing` here is
> `validator | node | user | open`, with `user` resolved through the identity
> account plane rather than a client set.

## The model in one paragraph

Validators are validators, full nodes are full nodes, and users are users.
A **node** (validator or resident) is a capability provider and a sync point;
a **user** is a mnemonic-derived ed25519 key that owns nodes and acts through
them. Using the network is user-key business; running it is node-key business;
governing it is validator business. The `acl` module makes that separation a
committed, governable policy instead of a convention scattered through module
code — the cosmos account/validator split, adapted to ducktape's staged
membership (resident → validator) and capability mesh.

## Principals — two key types, three standings

Nothing here mints a new identity concept; the module names what already
exists in committed state:

- **User keys** — the BIP39-mnemonic identity (`bin/node/src/userkey.rs`).
  The `identity` module (see `2026-07-07-user-node-identity-split-design.md`)
  replicates **user → {node keys}** bindings, verified inside `execute`.
- **Node keys** — `identity.key`, the transport/consensus identity. A node's
  standing is committed `valset` state: **validator** (quorum seat) or
  **resident** (granted full node: mesh-admitted, boundary-following,
  serving reads, relaying submits, announcing capabilities — no quorum
  weight).

A principal, resolved at op-execution time, is the pair
`(node standing via valset, owning user via identity)`. Both are reads of
committed state at the executing boundary — deterministic on every node.

Deliberately untouched: the `Frame` wire format and signer (the node key),
mesh tracking sets, quorum arithmetic, and the one-user-many-nodes decision
(a user running three validators holds three seats and three votes — the
identity-split spec's resolved decision stands).

## The `acl` module

A small consensus module (id `"acl"`), profiles-shaped like `valset` and
`governance`: staged overlay, canonical snapshot bytes, state-based sha256
root, trust-free snapshot/install.

### State

```
policy:            map<target, Standing>    // target = module id, or "*"
capability_policy: {
    default_allow: bool,                    // true at genesis
    entries: map<(node_key | "*", tag), Allow | Deny>,
}
```

`Standing` is the required class to SUBMIT to a target:

```
validator  — origin node holds a quorum seat
node       — validator ∪ resident (any granted standing)
user       — any node bound to a user identity
open       — anyone the mesh admitted (today's behavior)
```

### Enforcement — dispatch, not per-module

The gate lives where ops are routed to modules (the drain's dispatch step),
not inside each module: before a finalized op reaches its target, dispatch
resolves the origin's principal and checks it against `policy[target]`
(falling back to `policy["*"]`, falling back to `open`). A failed check is a
**deterministic rejection** — the identical no-op every honest validator
makes, exactly like a module rejection today — so consensus semantics are
unchanged and a byzantine proposer still cannot halt honest nodes by
finalizing a forbidden op.

Modules keep their own *semantic* origin checks (profiles' origin-gated
`SetName`, governance's proposer/voter rules); the ACL is the coarse
standing gate above them, in one place instead of N.

### Mutation — governance only

Policy changes ride the existing governance ceremony as a proposal action
(`SetAclPolicy { target, standing }` / `SetCapabilityPolicy { … }`-shaped):
proposed, balloted by the validator majority, executed — so who-may-do-what
changes carry a ballot trail. The `acl` module accepts these mutations only
from governance execution, never from direct submits.

### Seed policy (genesis defaults)

```
governance, valset, upgrade   → validator
acl                           → validator   (via governance execution only)
capability (announce)         → node        (residents announce — the
                                             resident-capability-announce
                                             lane, unchanged)
oracle results, submit relay  → node
everything else ("*")         → open        (tightens to `user` with v2)
```

Default `open` for app modules keeps a fresh network exactly as usable as
today; the table exists so tightening is one proposal, not a release.

## Capability federation — announce is open, activation is a decision

Two registries with different authority:

- **Announced** (exists): any tracked node publishes its discovered provider
  tags (`capability` module). Stays open at `node` standing — it is
  inventory, not authority. Rogue nodes announcing exotic capabilities
  merely exist in the registry.
- **Active** (new, `acl.capability_policy`): what routing actually uses.
  `active(node, tag) = announced(node, tag) ∧ policy allows (node, tag)`,
  where policy is `default_allow` plus explicit allow/deny entries. The
  dispatch-oracle's lease/routing step consults ACTIVE, not ANNOUNCED.

Default allow-all, per the founding decision: a small network has zero
friction and behaves exactly as before this module. As the network grows —
many nodes joining as non-validators purely as capability proxies — one
governance proposal denies a tag or a node, or flips `default_allow` to
allowlist mode, and work stops routing to unvetted providers. The members UI
renders announced-but-inactive greyed out, so the distinction is visible
rather than silent.

## Trajectory: v1 resolves, v2 signs

- **v1 (this design):** ops remain node-key-signed; the ACL resolves
  node → user through the identity module. No wire change, no client change
  — policy over state that already exists. Limit, stated honestly: a user at
  a *foreign* node cannot act as themselves — attribution follows the node's
  binding.
- **v2 (named here, built later):** user-signed op envelopes. The app signs
  the payload with the mnemonic key; **any** full node relays it (the
  resident submit relay already has exactly these custody semantics: verify
  before pin, exactly-once digest gate, cutover carry); the node key
  degrades to pure transport for user ops. Requires an envelope format with
  the user signature and a per-user nonce — the wire change that makes it
  phase two. With v2, app-module policy tightens from `open` to `user`, and
  the account/validator split is complete: using the network is done from
  the mnemonic, everywhere, from any node.

## What this deliberately does not do

- No per-object or per-row ACLs — modules own their data semantics; this is
  a standing gate per target, not a permission system inside modules.
- No new quorum or vote weighting — validator-set mechanics are `valset`'s
  and untouched.
- No capability *verification* — activation is a trust decision by
  governance, not an attestation protocol; providers still BYO credentials
  (docs/records/specs/capability-spec.md) and announce only what the host installed.
