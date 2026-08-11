# Epic: Account/Workspace Separation — Discord × Keybase UX

Date: 2026-07-14. Status: **decomposition approved** (ideation session with the
operator), **refined same day** (second session: owner node control promoted to
its own work item, open questions settled, light users scoped out). This is the
epic ledger; each W-item gets its own brainstorm → spec → plan → PR cycle.
Normative model underneath:
`docs/adr/2026-07-14-account-node-access-model.mdx` (A1–A6).

## Vision

The app is framed by the **account** (Keybase-style identity: one key, many
devices, explicit custody), and networks are contexts inside it
(Discord-style: a rail of joined networks, live everywhere, instant switch).
Nodes remain technically subordinate to the account, but the user never has to
think "workspace" or "node" — they think "me" and "my networks".

Target experience: first run defines *who you are*; after that you land on
*your account*, and joining a network is one action from there.

**"Node" is not user vocabulary.** The user's verb is "join a network"; the
node is a side effect the app runs invisibly. Node surfaces (control panel,
diagnostics) are progressive disclosure for operators, never on the normal
user's path. User-facing errors say what to do, not what daemon failed.

## Session decisions (binding for the epic)

| Decision | Choice |
|----------|--------|
| Join model | **Join = local node** (current model kept). Client standing (#576) stays the remote-access exception, not the default join path. |
| Concurrency | **Single active network** (revised later same day; was: full Discord parity). One live node + one live connection at a time; switching stays the node-swap. Multi-network *membership* and the rail stay. Full parity (concurrent nodes, live cross-network badges, instant switch) = **W3/W4, deferred** — trigger: a second network the user actually lives in daily. |
| Ordering | **UX-first**: reframe the shell first. Active order: W1 → W2 → W5 → W6. |

Refinement session (same day) added:

| Decision | Choice |
|----------|--------|
| Owner node control | Its own work item (**W2**), ahead of the N-node substrate. Realizes ADR A2+A5 (public/private RPC split, owner-gated control) and closes the standing hole of `/v1/shutdown` + `/v1/admin/module-code/*` living on the public surface. |
| Control split | **Private RPC = node lifecycle only** (shutdown/restart, config, upgrade staging, logs/diagnostics, invite minting). Governance ops (admit/promote/demote/leave) are consensus ops → they move to **account-signed frames on the public surface** (A1); any connection with member standing can drive them. |
| Node-operation UX | **Three orthogonal capability layers, not modes**: data (standing), control (owner ∧ private-RPC reachable), process (this app manages the process). The remote-viewer / remote-owner / local-operator cases fall out as layer combinations; control UI is one implementation for local and remote. |
| Node survival | Formalized: nodes are detached (already true) and **app quit leaves them running** (validator uptime). The supervisor adopts running nodes on launch. |
| Light users | **Out of scope** for this epic (see Non-goals). |

## Node-operation capability layers (W2 normative sketch)

| Case | Data plane | Control plane (private RPC) | Process plane (local shell) |
|------|-----------|------------------------------|------------------------------|
| Remote, not owner | ✓ (standing) | — | — |
| Remote, owner | ✓ | ✓ when the node exposes it and it's reachable | — (a dead remote node can only be revived on its own box) |
| Local managed | ✓ | ✓ (loopback, always while running) | ✓ (spawn/adopt, cold start, forget, daemon.log) |

- Ownership is a **chain fact**, not a connection property: committed
  `BindNode` (identity module) matched against the logged-in account key,
  readable over the public RPC.
- Control-plane auth: per-request PoP signature by the owner account's bound
  key (precedent: coordinator auth #197). Exposure is the node's choice (A4):
  default loopback-only; remote exposure is opt-in (self-expose / tunnel).
- A5 nuance settled: a **non-owner sees no control chrome at all**; an
  **owner whose private RPC is unreachable** sees a one-line "control surface
  not reachable" state — ownership is public on-chain fact, so the app may
  say so.
- Local node control also rides the loopback private RPC: one control-UI code
  path; the Tauri lane shrinks to the process plane (spawn/adopt/kill/forget,
  log files).

## Work items

### W1. Account home + network rail + store network-scoping

The UX reframe itself, on top of today's mechanics.

- Terminology: user-facing "workspace" becomes **"network"**. Rename UI
  strings and align code you touch; no mass-churn of untouched internal
  identifiers.
- Kill the post-mnemonic auto-created workspace; land on an **account home**
  (profile, joined-network list, join/create CTAs).
- Rail details (question-bomb): join-order, no drag-reorder; icons = initial
  chip colored from the chain id (no network-avatar feature). Account home
  **replaces** the Home / Add workspace screens outright; content = profile
  card, custody card, network list, join/create CTAs — nothing more (W5's
  device list joins later). Account-scoped settings = **custody + devices
  only**; theme and general app settings stay app-level.
- Discord-style left rail: "me" (account home) on top, network icons below.
  Account-scoped settings (custody, devices) move out of network settings.
- Reshape `console/store` from a single `workspace` into per-network slices.
  Switching stays the current node-swap (sequential) — under the single-active
  premise this is the model, not an interim state. Reuse the existing
  multi-workspace registry (Home / Add workspace) unchanged.
- `nodeControlAvailable` becomes a per-network evaluation (still the interim
  `managed` form; W2 replaces the predicate).
- Remote/client connections keep working and get a rail seat (badged remote,
  no control chrome — A6). No regression of the #587 client mode.
- Existing users: no migration machinery — existing workspaces simply appear
  as networks in the rail (no-backcompat mandate; the auto-created-workspace
  kill affects fresh onboarding only).

Settled (was open):

- **duck:// scope**: refs stay implicitly scoped to the connected network —
  the URI has no network authority (confirmed in `duck-uri.ts`) and
  cross-network navigation is the rail's job. A cross-network ref form would
  be a new authority shape; out of epic scope.
- **Zero-network account home**: profile + custody card + join/create CTAs
  (existing onboarding forms relocated). W5's device list joins it later.

### W2. Owner control plane (ADR A2+A5 realized)

If the logged-in account owns a node and the node exposes its private RPC,
the app can control that node — local or remote, same UI.

- Node side: **namespaced control API on the same listener** (geth
  `admin_`/`debug_` style — question-bomb Q9; no separate port). An
  owner-gated admin namespace carries lifecycle (shutdown/restart), config,
  upgrade staging, log/diagnostic tail, and **invite minting**.
  `/v1/shutdown` and `/v1/admin/module-code/*` move into it (closes a real
  exposure today — they sit behind nothing but the browser-origin allowlist).
- Auth: owner = committed `BindNode` for this node's key; requests carry a
  per-request PoP signature by the owner account's bound key (#197 pattern).
- Exposure (A4): the admin namespace is PoP-gated wherever the listener is
  reachable; a node flag can additionally restrict admin to loopback (geth
  `--http.api` spirit). "No private API" = namespace disabled → control
  surface simply absent. W2 covers self-expose only; the WG-via-coordinator
  tunnel stays separate work.
- Governance ops (admit/promote/demote/leave) leave the bespoke local lane:
  the governance module authorizes **account-signed frames** from members
  (A1), so they work over any connection with standing. (Module-side ACL work;
  details in this W's session.)
- App side: `nodeControlAvailable` = owner(`BindNode`) ∧ private-RPC
  reachable, replacing the interim `managed` predicate. Capability layers per
  the matrix above; owner-only "unreachable" hint.
- Co-traveler / risk: **#599 CSP** — the app's remote reach is currently
  loopback/tailnet-only; remote owner control inherits that until it's fixed
  (fix rides this W or lands separately first).

### W3. N-node concurrent runtime (substrate) — DEFERRED

Deferred under the single-active decision. Trigger to revive: a second network
the user lives in daily. The **single-node slice is still live** — crash
auto-restart and adopt-hardening for the one supervised node have validator-
uptime value regardless of N; that slice **rides W2** (question-bomb Q13).
Body below kept as design notes for when this wakes.

- Shell supervises **all** joined networks' nodes instead of spawning only the
  active one. Supervision consumes W2's lifecycle endpoints where applicable.
- **Ignorance-tolerant by design** (users know nothing): crash auto-restart;
  an "start at OS login" decision item (auto-launch the app vs. background
  node start); adopt-on-launch stays idempotent (exists). User-facing failure
  = one line + one button, not a daemon.log dump (the developer surface
  remains behind it).
- Settled (was open): **app quit leaves nodes running** (already detached —
  formalized as the model); **ports** come from the existing per-workspace
  port registry (`ports.rs`), extended as-is; **resource ceiling** for N wasm
  runtimes: measure first, cap in this W's session (epic sets no number).
- Honest limit, accepted: a laptop's nodes sleep with the laptop. Other
  validators absorb the downtime; the node catches up on wake. Not this
  epic's problem to fix (see Non-goals).

### W4. Concurrent connections + cross-network badges — DEFERRED (needs W3)

Deferred with W3, same trigger. W1's store slicing keeps this a
connection-layer swap when it wakes — no door closes.

- Promote W1's per-network slices from one live connection to N (store shape
  is already right; this swaps the connection layer). Switching becomes
  instant.
- Unread/mention aggregation → rail badges; OS notifications gain network
  context (rides the existing notification plane, #311/#433/#437/#442).

### W5. Device/key management surface (the Keybase half)

- Device list aggregating per-network `BindNode` records; device labels and
  remote-unbind UI (the deferred items from #205/#227); recovery story
  surfaced (mnemonic reveal/restore exists).
- Decisions (question-bomb): labels are **on-chain** — an identity-module
  label op, so a device's label is visible from the user's other devices
  (consensus module change; genesis root-hash moves, re-seed accepted).
  Aggregation renders **cached last-known state** per network, refreshed on
  switch (single-active). Unbind is **per-network**; no bulk
  "remove everywhere" button.
- Key rotation is **out of scope** (consensus change; its own ADR if wanted).
- Hangs off W1's account home; data plane already exists.

### W6. Account profile propagation

- Display name/avatar defined once on the account, pushed to each joined
  network's identity module (`SetUserName` exists).
- Settled (was open): **avatar storage** rides the duckfs files plane (same
  plane as chat attachments, #541); per-network push mirrors `SetUserName`.
- Decisions (question-bomb): profile fields = name, avatar, **bio/status**.
  One global profile — no per-network nickname overrides. Propagation =
  **reconcile on next connect** per network (dirty flag + auto-push); no
  background fan-out under single-active.

## Dependencies and order

```
W1 (UX frame + store shape) ──→ W5, W6 (need the account home; otherwise parallel)
W2 (owner control plane)    ──→ W3 (deferred: supervisor would consume lifecycle RPC)
W1, W3 ──→ W4 (deferred with W3)
```

Execution (question-bomb Q2 + delivery decision): **W1, W2, W5, W6 all in
parallel**, one worktree/branch each, forked from the epic branch
`epic/account-workspace-separation`. Each W lands as its own PR **based on
the epic branch**; the epic branch carries this ledger and gets one **epic PR
against dev** — per-item PRs merge into the epic after review, the epic gets
a final clean-context review, then merges to dev. W3 → W4 stay deferred
(single-active premise).

Known merge overlaps, accepted up front: W5 and W6 both touch
`crates/system/identity` (label op vs profile ops — keep both diffs tight
and additive); W1 owns the account home that W5/W6 panels mount into — they
build self-contained panels and final placement reconciles inside the epic
branch.

## Non-goals (deliberate, not holes)

- **Light-user (client) onboarding + admin serving surface**: client invites,
  standing grant/revoke UI, the "serving node = exposed resident" pattern,
  public-RPC rate limits. There are no light users today; the path has hard
  external blockers (#599 CSP, A4 tunnel exposure); and deferring closes no
  doors — client standing (#576) stays working and W1 keeps its rail seat.
  Its own epic when real users exist.
- **Key rotation** (consensus change; own ADR).
- **Node hosting / uptime mitigation** for sleeping laptops (YAGNI; the
  network tolerates it).

## Relation to shipped work

Already in place (this epic builds on, does not redo): account key + mnemonic
custody (#227), `BindNode` identity module (#205), account-first UI / node
rail conditionality (#587, ADR A6), client ACL + user-signed submits
(#576/#579), keyless per-request PoP auth precedent (coordinator, #197),
statesync fail-closed for clients (#567), duckdns account naming
(ADR 2026-07-10), desktop notification plane (#311 et al.).
