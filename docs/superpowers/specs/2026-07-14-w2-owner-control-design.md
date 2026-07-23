# W2 — Owner Control Plane (ADR A2+A5)

Date: 2026-07-14. Branch: `feat/w2-owner-control`, forked from
`epic/account-workspace-separation`. Realizes the epic ledger §W2 and ADR
`2026-07-14-account-node-access-model.mdx` rules **A2** (public/private RPC
split) and **A5** (owner-gated control). Binding parent: the epic ledger.

## Problem

`/v1/shutdown` and `/v1/admin/module-code/*` sit on the node's **public**
listener behind nothing but the browser-origin allowlist (`origin_guard`). Any
non-browser client that can reach the port can shut the node down or stage code.
That is the standing exposure hole the epic promotes W2 to close: control ops
must live on a **separate, owner-gated namespace**, distinct from the data
surface — not merely a permission check on one route.

## Decision: a namespaced control API on the same listener

Following geth's `admin_`/`debug_` spirit (ledger Q9): one listener, a
`/v1/admin/*` namespace that is **gated as a unit**, never a second port.

Two orthogonal gates, both enforced by one middleware layered only on the admin
sub-router:

| Regime (`AdminExposure`, flag `DUCKTAPE_ADMIN`) | Reach | Auth |
|------|----------|-----------|
| `Disabled` | routes never registered — surface **absent** (404) | — |
| `Loopback` (default) | loopback peers only | **loopback-trust, no PoP** |
| `Public` | any peer | **owner PoP required** |

**Refinement (from the interim design):** `Loopback` trusts a loopback peer with
no PoP rather than requiring a signature on every local call. This matches
`origin_guard`'s own model (a loopback process can already read `user.key`, so
loopback is a boundary this layer cannot tighten) and the capability matrix's
"local control is always available on loopback" — frictionless local control,
no bootstrap hazard. The owner PoP gate (A5) protects the **`Public` (off-box)**
surface: the actual new capability and the real threat. Local control therefore
needs no app-side signing; only remote (`Public`) owner control does.

The owner gate (A5, `Public` only) is a per-request proof-of-possession (PoP)
by the owner account's key — the **#197 coordinator-auth pattern** — against the
committed `BindNode` owner.

### Owner = a chain fact, not a connection property

Ownership is resolved live from committed state, never trusted from the
connection: query this node's own `identity` module `OfNode { node_key }` → the
owning `AccountView` → the PoP subject key must be one of its `member_keys`.
This is the exact resolver the app's auto-bind already uses (`accountOfNode`).

### PoP shape (mirrors `nat-traversal::auth`, #197)

- Sign `ADMIN_REQ_NS` over
  `method ‖ 0x1f ‖ path_and_query ‖ 0x1f ‖ node_key(32) ‖ 0x1f ‖ ts_be(8)` —
  the **target node's consensus key is folded in** (review MEDIUM-2b), so a
  signature minted for node X can never be replayed against another node the
  same owner controls. The fixed-width tail keeps the layout unambiguous.
- Headers: `x-ducktape-admin-key` (hex ed25519 account pubkey),
  `x-ducktape-admin-ts` (unix seconds), `x-ducktape-admin-sig` (hex ed25519 sig).
- Freshness window ±30 s (replay-bounded), same as the coordinator.
- The app mints a PoP **only after confirming ownership** (review MEDIUM-2a):
  the connect path resolves `nodeOwnedBy` first and probes `/v1/admin/ping`
  signed only when it holds — a non-owner connection never hands a signature,
  even a scoped one, to an arbitrary remote node.
- **Never front a `Loopback`-exposure node with a reverse proxy** (review
  LOW-3): the proxy's loopback dial launders every remote caller into a trusted
  local peer. Off-box access is `Public` + owner PoP (+ operator TLS), only.
- The loopback peer check **fails closed** (review LOW-2): a request with no
  `ConnectInfo` is refused, never granted local trust; every serve path
  (noded, bin/node, simnode) threads connect-info.
- **The body is deliberately NOT signed.** `module-code/stage` streams a large
  wasm artifact the store never parks in memory; buffering it in middleware to
  hash it would regress that path. Ceiling accepted: on a *non-TLS public*
  exposure a network attacker can tamper the body of an owner-issued request
  within the 30 s window. TLS termination is the operator's job for hostile-
  network exposure, exactly as for any HTTP service. `ponytail:` documented.

### Bootstrap window

A node with **no committed owner yet** (fresh network, before the first
`BindNode` commits) has nobody to authenticate against. Rather than lock the
owner out of their own fresh node, admin falls back to **loopback-trust** until
the first bind commits — never reachable off-box, and it collapses to the full
owner gate the moment ownership exists on chain. The embedded single-writer
daemon (`bin/noded`, no consensus, `node_key = None`) is permanently in this
state: loopback-trust, which is exactly its threat model today.

This is consistent with `origin_guard`'s own stated model — a local process can
already read `user.key` off disk, so loopback is a trust boundary this layer
cannot meaningfully tighten. The PoP gate is what protects the **exposed**
(`Public`) surface, which is the new capability W2 adds.

## What moves / is added under `/v1/admin`

- `POST /v1/admin/shutdown` — moved from `/v1/shutdown` (flag-day; no alias).
- `POST /v1/admin/module-code/stage`, `GET /v1/admin/module-code/{digest}` —
  already under `/v1/admin/*`; now behind the owner gate (upgrade staging).
- `GET /v1/admin/logs/tail?after=<seq>&limit=<n>` — a simple log-ring tail
  (diagnostics), reading the same ring the ws `logs` topic streams.
- `GET /v1/admin/ping` — a cheap authenticated liveness probe the app uses to
  evaluate *admin-namespace reachable* for `nodeControlAvailable`.

Deferred (net-new HTTP surface, not an existing exposure): **config edit**,
**invite minting over HTTP** (a CLI/`config::mint_invite_token` path today —
the admin namespace is its right home when wired), and **restart** (a
supervisor/process-plane action = admin-shutdown + respawn, not a node
endpoint). Documented, not holes.

## Exposure flag

`DUCKTAPE_ADMIN` = `off | loopback | public`, read at node boot (default
`loopback`). The full node threads it + its own consensus key into the
`NodeHandle` admin config; the desktop sets it via the node command env when an
owner opts into remote control. `off` unregisters the namespace — control
surface simply absent.

## App side (narrow, seam-shaped for the W1 merge)

W1 concurrently reshapes `console/store` into per-network slices and keeps
`nodeControlAvailable` in its interim `managed` form, explicitly leaving the
predicate replacement to W2. To keep the epic merge mechanical, W2 touches only:

- The predicate body at its documented seam (`state.ts` `nodeControlAvailable`):
  `(workspace ∧ managed) || (owner ∧ adminReachable)` — the non-regressing form
  the interim comment predicted. The first disjunct keeps LOCAL managed control
  (the **process plane**: control even while the node is stopped, so Start still
  shows — a naive swap to `owner ∧ adminReachable` alone would regress that); the
  second adds REMOTE owner control (the A2 split). A non-owner remote satisfies
  neither ⇒ no control chrome. `managed` keeps its distinct process-plane meaning
  (StatusView Stop/Start, LogsTab). Three-layer model: data (standing) / control
  (owner ∧ admin reachable) / process (`managed`).
- Two connect-time fields (`owner`, `adminReachable`) and a self-contained
  `admin-client.ts` that signs PoP and probes `/v1/admin/ping`.
- A node `user-sign-admin` verb + `user_sign_admin` Tauri command (mirrors
  `user_sign_bind`) so the account key signs each admin request. Local control
  keeps working because the app is the owner and signs; a non-owner sends no
  valid PoP and sees **no control chrome** (predicate false); an owner whose
  admin surface is unreachable sees a one-line "control surface not reachable"
  hint (`owner ∧ !adminReachable`).
- The **#599 CSP fix**: widen `connect-src` to admit user-entered node origins
  so remote owner control is not loopback/tailnet-only. **Accepted ONLY because
  `script-src` stays `'self'`** — no inline or remote script can run in the
  webview, so nothing untrusted exists to exfiltrate over the widened
  `connect-src`. Any future `script-src` widening re-opens this decision. The
  proper post-#599 follow-up is a runtime per-endpoint allowlist (admit exactly
  the node origins the user has connected to), tracked outside this W.

## Governance ops — account-signed frames (SHIPPED, `feat/w2-governance-frames`)

Operator decision Q11 reversed the earlier deferral: **all governance ops move
at once**. admit/promote/demote/leave leave the bespoke node-local re-signing
lane and become **account-signed frames on the public surface** (A1), authorized
by the governance module's own standing ACL. Implemented as a separate PR
against the epic branch (the last code before epic QA).

### Electorate decision (Q11 — recorded)

**Quorum/governance stay PER-NODE in v1 — N validators = N votes.** So
validator-mode ballots stay **node-keyed** (share mode is unchanged, account-
keyed). An account-signed op is resolved to a governance ACTOR and casts the
ballot of **every bound electorate node** — the exact power the account held when
each node voted for itself; re-voting overwrites by node key, so nothing
double-counts. This was chosen over account-keyed validator ballots because it
keeps v1's per-node quorum semantics (`consensus_time = block height`, N=N)
untouched — the migration changes *who may author* a governance op, not *how
votes are weighed*.

### Module ACL (`crates/system/governance`)

- `resolve_actor(origin)`: an origin that is an account MEMBER key (identity
  `OfMember`) is an `Actor::Account { account_id, nodes }` acting for its
  committed `BindNode` nodes; any other origin is `Actor::Node` acting as
  itself. A node key that IS a member takes an **identity-free fast path** (a
  validator's own key, and hosts without an identity module); an identity query
  error ⇒ no accounts exist ⇒ the origin is its own node key.
- Standing gate: the submitter must hold validator-set standing — directly a
  member node, or an account member key bound to one. Rejection: "no
  validator-set standing".
- A validator node stays a **first-class governance actor** (its own automation/
  tooling) — the `Actor::Node` arm is the model, not a compat alias.
- Node side needs NO change: `/v1/submit/frame` already reaches governance with
  `Origin::External(signer)`.

### App side

- `governance` added to `CLIENT_SIGNABLE_TARGETS` (the only control module that
  is; the rest stay refused). Governance ops sign on EVERY connection via a new
  `transport.submitControl` + `signControlPayload` (wired local AND remote) —
  local content stays frameless, but governance is always account-signed so the
  module's `BindNode` ACL resolves the user. `governanceClient.driveMembership`
  runs the client-side propose→vote→execute ceremony (the replacement for the
  deleted `drive_membership_ceremony`).
- **Deleted (not aliased):** the app's bespoke lane — the Tauri
  `workspace_admit`/`workspace_promote`/`workspace_demote`/
  `workspace_resident_remove`/`workspace_request_leave` commands, their `ws.*`
  client methods, registration (main.rs/build.rs/trusted.toml), and their
  entries in the daemon's app-verb allowlist. The standalone `ducktape-node
  <verb>` operator CLI (node-principal governance, invoked directly, not via the
  app allowlist) is retained — it is the headless-operator interface and backs
  the node cluster e2e tests, and remains valid under the new ACL as a
  first-class `Actor::Node`; it is not an app path, so it is not the "bespoke
  lane" the migration deletes.

### Root-hash + gates

Genesis root-hash MOVES (governance-wasm rebuild) — re-seed required (accepted).
Gates: `cargo test -p simnode` (standing gate — extended with `governance_frames.rs`:
account-signed admit/demote ceremonies over the real frame wire + a no-standing
rejection); governance/identity module tests (+ account-member-key path);
`wasm_governance_parity` with the regenerated component; app typecheck + vitest.

## Supervision slice (rides W2, ledger §W3 note)

Single managed node, shell side, self-contained (a third commit on this branch):

- **Crash auto-restart**: `watch_node_exit` only reaped + logged. It now revives
  a node that dies UNEXPECTEDLY (non-zero / signal) — validator uptime for a user
  who knows nothing of daemons — under a hard restart cap (`MAX_AUTO_RESTARTS=5`)
  and a constant backoff, so a node that fails preflight every boot stops rather
  than spins. The policy is a pure `should_auto_restart(code, stopping, restarts)`
  (unit-tested); the process-spawning guards sit on top.
- **Stop-intent flag**: teardown escalates to TERM/KILL, which looks exactly like
  a crash. A per-workspace `Arc<AtomicBool>` (registered at spawn, raised by
  `stop_workspace_node` *before* it kills, cleared after) tells a deliberate stop
  apart from a crash, so the supervisor never fights the teardown releasing the
  ports.
- **Adopt-hardening**: `workspace_select` already adopts a live node idempotently
  (`port_listening`). The respawn re-checks the stop flag *and* `port_listening`
  after its backoff, so a crash-restart racing a user re-select never
  double-spawns — whoever bound the port first wins, the other adopts.

Accepted limits (`ponytail:`): a fixed cap + constant backoff (a crash-rate
window would be more precise if flapping ever shows up); a node adopted from a
*previous* app instance has no `Child` handle, so it is watched/revived only once
this shell has spawned it (a crash while the app is closed is caught on next
launch's spawn, per the detached-survival model).

## Non-goals (this W)

WireGuard-tunnel exposure (A4 second option — stays separate work); N-node
concurrent supervision (W3 deferred); config-edit / invite-mint HTTP surface
(deferred, documented above). The governance consensus migration is now SHIPPED
(§Governance, `feat/w2-governance-frames`), no longer a non-goal.
