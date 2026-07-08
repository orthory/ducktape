# No-Downtime Node Upgrade — Design of Record

Status: Design of record. This pass produces artifacts only (spec + vocs docs +
skill). It does NOT implement the Rust consensus changes.
Date: 2026-07-04

## Summary

Ducktape needs a way to ship a **consensus-breaking** change — a change to any
module `root()` computation, op/wire encoding, or the module set — to a **live**
network with **zero network downtime**. The mechanism below delivers that for
changes that keep every module id stable (a `root()` or op-encoding change, the
common case); changing the module **set** (adding or removing a module id) is a
strictly harder class that this spec scopes OUT of the base guarantee (see Edge
cases and Non-goals). Today Ducktape can do neither: rolling a new binary
onto a running network forks or halts it the moment one validator computes a
different app-hash from another. That is exactly why the forge multi-repo change
(single committed head → canonical sorted hash over per-repo heads) could not be
rolled onto the live Ducktape-2 network.

This spec defines a **height-gated activation** mechanism with a **pre-shipped
dual-path binary**, backed by four pieces:

1. an **`upgrade` system module** that holds the agreed `current_version`, an
   optional pending `Upgrade { name, activation_height, to_version }`, and the
   validator readiness set — all folded into the app-hash;
2. **`GovAction::ScheduleUpgrade`** (and `CancelUpgrade`), authorized by the
   existing member-gated governance tally, which emits an upgrade-module
   follow-up exactly like `AddValidator` emits a valset follow-up;
3. a **`SignalReady` op**, validator-origin-gated, that records per-member
   readiness; activation **arms** only when EVERY current boundary validator has
   signaled (`R = n`, the arming policy — any straggler aborts the upgrade);
4. a **deterministic activation + migration at H**, landed atomically over the
   existing epoch teardown-respawn boundary, with a **version field added to the
   recovery and state-sync manifests**.

The whole design turns on one rule: **activation is execute-time, not
respawn-time.** The version bump, the pending-clear, and every per-module state
migration are deterministic in-block `execute()` transitions keyed on
`Env.height` and sealed into module roots. The teardown-respawn boundary carries
only the engine/BLS rekey. This is what makes a restarting node, a recovery
replay, and a state-sync joiner all reconstruct the activation byte-for-byte.

## The invariant that forces this shape

The global app-hash is a sorted hash over `(module_id, module_root)` pairs
(`state::global_root`, folded by `Host::app_hash` at
`crates/kernel/host/src/lib.rs:341-344`). Every validator MUST derive the
identical app-hash from the same agreed op order. An app-hash is committed only
via a `2f+1` quorum certificate; a node whose recomputed app-hash disagrees with
the finalized certificate is rejected at `capture_finalized_snapshot`
(`AppHashMismatch`, `crates/kernel/host/src/lib.rs:375-379`), and a recovering
node that recomputes a wrong root fails the final compose check
(`crates/kernel/recovery/src/lib.rs:971-975`).

Therefore ANY change to a module `root()`, op encoding, or the module set is
**consensus-breaking**: if one validator runs new logic and another runs old,
they diverge on the next block and the network **forks or halts**.

### Why no-downtime implies exactly one shape

For a consensus-breaking change, zero network downtime is only achievable via
**height-gated activation with a pre-shipped dual-path binary**:

- The new binary carries BOTH old and new logic and is **byte-identical in
  behavior** to the old binary for every height below the activation height `H`.
  Only the changed module branches on the active protocol version.
- Operators roll the new binary out node-by-node — no downtime, because each
  node keeps running OLD logic until `H`.
- At the agreed height `H`, every ready validator flips to NEW logic **together**.

This is the Ethereum / Tendermint app-version hard-fork model, and it maps onto
Ducktape's existing epoch teardown-and-respawn boundary
(`crates/kernel/consensus/src/lib.rs:110-120`;
`crates/kernel/consensus/src/valset_orchestrator.rs`), which was already built to
carry the V1→V2 BLS scheme migration: "this one teardown-and-respawn mechanism
backs both BLS migration and dynamic valset"
(`crates/kernel/consensus/src/lib.rs:117`).

## Core design principle: activation is execute-time

The single most load-bearing decision, and the direct resolution of the
state-sync / recovery hardening lens:

> The version bump, the pending-clear, and every per-module state migration MUST
> be deterministic in-block `execute()` transitions keyed on `Env.height` and
> sealed into module roots. Nothing about activation may live only in the
> consensus respawn path or the binary's control flow.

Recovery re-executes journaled frames via `apply_block → host.submit_at`
(`crates/kernel/recovery/src/lib.rs:996-1017`) and handles `Cutover` records by
updating ONLY epoch / view_base / participants
(`crates/kernel/recovery/src/lib.rs:840-852`) — it never invokes a module
migration. So any activation effect placed in the respawn side-path or in binary
control flow would be **invisible to replay**: best case, every node that
restarts across `H` fails the final compose check (mass straggler halt = the
downtime we are trying to avoid); worst case an out-of-band activation effect
makes a live node and a replayed node diverge = **fork**.

The teardown-respawn boundary therefore carries **only** the engine/BLS rekey
and the `(scheme, participants)` swap. All app-hash-visible activation is
execute-time.

### Version is a pure derivation, never a stored flip

`effective_version(height)` is a pure function of the upgrade module's committed
(app-hash-included) state, evaluated per-block from `Env.height`:

```
effective_version(height) =
    if pending.armed && height >= pending.activation_height { pending.to_version }
    else { current_version }
```

It is NOT a mutable boolean mutated once by a side-effecting op at `H`. A stored
flip read in a subtly different order by a snapshot-restored node vs a
continuously-live node would produce different branch selections for the same
height — a silent fork the readiness gate cannot catch (both nodes think they are
"ready"). A pure derivation makes every module at every height — live,
recovery-replay, or state-sync — compute the identical branch with zero
dependence on apply-order or snapshot-boundary placement.

## Mechanism

### 1. The `upgrade` system module (consensus state)

A small dedicated system module composed into the app-hash, mirroring
valset/governance. Its committed state (all folded into its `root()`, so all
covered by `global_root`/`app_hash`):

- `current_version: u32` — monotonic.
- `pending: Option<Upgrade>` where `Upgrade { name: String, activation_height:
  u64, to_version: u32 }`. **At most one** pending upgrade ever exists.
- `readiness: BTreeMap<PubKey, ReadySignal>` — the set of validator readiness
  signals for the currently-pending upgrade, keyed by member pubkey (one member =
  one idempotent signal, last-write-wins).

`root()` is `sha256` over the canonical encoding of `(current_version, pending,
readiness)` in sorted/BTreeMap order, `StateRoot::ZERO` as the uninitialized
sentinel — matching governance's root discipline
(`crates/system/governance/src/lib.rs:337-339`).

**The upgrade module is a genesis constant.** It ships in the baseline (v0/v1)
module set of every binary from day one, exactly like valset and governance
(genesis vec at `bin/node/src/main.rs:242-278`; host registry
`BTreeMap<ModuleId,…>` wiring at `crates/kernel/host/src/lib.rs:272-301`).
Because introducing a new module id changes `global_root`'s module count and byte
layout, it cannot be introduced by a height-gated upgrade — there is no prior
in-hash version state to gate its own introduction on. Retrofitting it onto an
already-live network that lacks it (Ducktape-2) is a **one-time coordinated
stop-the-world genesis bump** (accompanied by a genesis-fingerprint change at
`bin/node/src/config.rs:210-222`, so old descriptors loudly fail to connect
rather than silently fork), NOT a no-downtime upgrade. The no-downtime guarantee
holds only for the **second and later** upgrades, once the module is present
everywhere.

**The upgrade module's OWN logic is version-invariant baseline (v0) logic**,
identical in every binary and NOT gated by the pending upgrade it orchestrates.
Only TARGET modules (e.g. forge) are dual-pathed by a scheduled upgrade. If the
upgrade module itself must ever change, that change is shipped as its own
separately-versioned, height-gated upgrade.

### 2. Governance authorizes the schedule

Add to `GovAction` (`crates/system/governance-interface/src/lib.rs:19-26`),
alongside `AddValidator`, `RemoveValidator`, `Signal`:

```rust
ScheduleUpgrade { name: String, activation_height: u64, to_version: u32 },
CancelUpgrade   { name: String },
```

Reuse the member-gated proposal + simple-majority tally
(`crates/system/governance/src/lib.rs:274-328`). On a passing proposal,
`handle_execute` emits an **upgrade-module follow-up** exactly as `AddValidator`
emits a valset `Join` follow-up (`crates/system/governance/src/lib.rs:307-322`):
the host drains it in the same block, and the upgrade module accepts it because
the origin is `Module(governance)`.

**Two thresholds, kept explicitly separate:**

- A **simple-majority** governance vote (`members.len()/2 + 1`,
  `crates/system/governance/src/lib.rs:296`) may only **SCHEDULE** (authorize)
  the pending upgrade.
- **Activation** additionally requires an independently-evaluated **`R = n`
  readiness quorum** (see §3). Authorization ≠ activation. Folding these
  into one check would let a bare majority (which can be `< 2f+1` for larger `n`)
  flip consensus logic below the BFT safety floor.

**The upgrade module's follow-up handler is the state authority** and enforces,
deterministically, on ingest of a `ScheduleUpgrade`:

- **Origin gate.** Accept only `Origin::Module(governance) | Origin::System`;
  reject `Origin::External` with a hard error — the exact valset origin gate
  (`crates/system/valset/src/lib.rs:240-247`). Governance is the sole author.
- **Monotonicity.** Reject `to_version <= current_version` (no downgrade;
  irreversible-by-default).
- **Minimum lead.** Reject `activation_height <= current_height +
  MIN_UPGRADE_LEAD`, where `MIN_UPGRADE_LEAD` is at least the orchestrator's
  `cutover_delay` window (`crates/kernel/consensus/src/valset_orchestrator.rs:225-232`)
  so the boundary is strictly in every node's future. Activation is **never
  retroactive**.
- **At most one pending.** Reject a `ScheduleUpgrade` while a pending upgrade
  exists (the operator must `CancelUpgrade` first). A single deterministic rule
  so every node agrees which pending is authoritative.

`CancelUpgrade` is gated identically (governance origin), valid only while
`current_height < activation_height`, and clears both `pending` and the
accumulated `readiness` set so stale signals cannot carry into a new upgrade.

### 3. The readiness gate

Each validator, once running the new binary, emits `SignalReady { name,
to_version }`. It is a **validator-origin op**:

- **Origin gate.** Accept only `Origin::External(pubkey)` where `pubkey` is a
  CURRENT valset member (the ordered lane authenticates the frame origin, so a
  signal is attributable to exactly one member key and no validator can forge
  another's — the same guarantee that protects governance ballots,
  `crates/system/governance-interface/src/lib.rs:10-13`). Reject
  Module/System/unauthenticated origins.
- **Identity scope.** Accept only when `name` matches the currently-pending
  upgrade's `name` (and `to_version` matches). Signals for any other identity are
  ignored/rejected, so a replayed or stale signal from an aborted round cannot
  count toward a fresh upgrade.
- **Idempotent.** Keyed by member pubkey, last-write-wins — `N` signals from one
  member count as one, like the governance ballot box
  (`crates/system/governance/src/lib.rs:268-269`).
- **Machine-generated, not operator-asserted.** A node emits `SignalReady` only
  when its own compile-time `MAX_PROTOCOL_VERSION >= to_version`, so "ready" is a
  truthful statement about the running binary, not a hand-typed assertion.

**Readiness quorum denominator.** The `R = n` readiness quorum is measured against
the valset **as-of the activation boundary** — the `EpochMembership` the respawn
reads from frozen state (`crates/kernel/consensus/src/valset_orchestrator.rs:237-256`),
NOT the proposal-time set. Governance `AddValidator`/`RemoveValidator` can move
the set between `ScheduleUpgrade` and `H`, so both the numerator (signals whose
key is in the boundary set) and the arming threshold — `R = n`, where `n` is the
boundary-set size (with `2f+1` its mathematical floor) — are recomputed against
the valset module root of the evaluation block. A signal from a key no longer a
member is dead weight, exactly like a removed member's stale ballot
(`crates/system/governance/src/lib.rs:288-290`). Never freeze the set or
threshold at schedule time.

#### Chosen policy: `R = n` arms, straggler-aborts-the-upgrade

Activation **arms** only when **`R = n`** — EVERY current boundary validator — has
signaled ready. This is THE arming policy (default), not a recommendation layered
over a lower baseline. Any non-signaler (offline-honest OR withholding-Byzantine)
means the flip does not fire ⇒ the deterministic clean abort runs ⇒ the network
keeps running OLD logic ⇒ the operator reschedules. This
"straggler-aborts-the-upgrade" posture is the unconditional no-downtime guarantee:
a live-but-unupgraded network is strictly safer than a fired-but-unfinalizable
flip. A node that nonetheless reaches `H` without the new logic **halts loud**
rather than applying ambiguous logic. Options considered:

- **(a) `R = n`, all boundary validators must signal — CHOSEN.** One straggler
  aborts the upgrade cleanly (the network keeps running, the operator reschedules);
  it never induces the halt-at-`H` liveness failure below.
- **(b) bare supermajority (`R = 2f+1`) arms** — REJECTED as the default: `2f+1` is
  the mathematical safety floor, not a liveness-safe flip threshold (§ Threshold
  hardening).
- **(c) operator-forced height, no gate** — simplest, most dangerous: nothing
  prevents flipping while too few nodes actually run the new logic.

`R = 2f+1` remains the mandatory mathematical **safety floor** — arming below it is
never allowed — but arming AT exactly `2f+1` is explicitly rejected as the policy,
for the liveness reason in the next section.

#### Threshold hardening: why bare `2f+1` is not enough for a LIVE flip

`2f+1` guarantees **safety** (no fork) unconditionally, but it does **not**
guarantee a live flip under the `≤ f` Byzantine model. With `n = 3f+1`, if the
`f` Byzantine validators all emit a valid `SignalReady` yet keep running OLD
logic (or crash right after signaling), the module sees `2f+1` ready and arms —
but post-`H` only `(2f+1) − f = f+1` nodes actually run new logic, which is
**below** the `2f+1` needed to finalize a new-app-hash block. The chain **halts**
at `H` with no abort recourse (state said the quorum was met). This is a full
liveness failure induced by a within-budget adversary — not a fork, but exactly
the network downtime the mechanism exists to prevent.

To guarantee `≥ 2f+1` **honest** nodes flip together you need
`R − f ≥ 2f+1 ⇒ R ≥ 3f+1 = n`. Therefore:

> **Arming policy: `R = n` (all current boundary validators).** This is THE
> policy, not a recommendation over a `2f+1` baseline; it leans on the abort path
> for stragglers. Any non-signaler (offline honest OR withholding Byzantine)
> simply means `R = n` is unmet ⇒ the flip does not fire ⇒ the deterministic clean
> abort runs ⇒ the network keeps running OLD logic ⇒ the operator reschedules. This
> is the **"straggler-aborts-the-upgrade"** posture and the unconditional
> no-downtime claim: a live-but-unupgraded network is strictly safer than a
> fired-but-unfinalizable flip.

Tolerating `s` known-offline stragglers with `R = n − s` is a **DISCOURAGED,
operator-documented DOWNGRADE** of the guarantee, never the default. It is
permitted only with out-of-band attestation that every signaler genuinely runs
the new binary before `H`, and it MUST be recorded at schedule time as a
**conditional** (not unconditional) guarantee: *no network downtime only if fewer
than `R − 2f` of the signalers are Byzantine* (which is `0` at the `R = 2f+1`
floor). **The spec cannot claim unconditional no-downtime below `R = n`.**
`R ≥ 2f+1` is the mandatory mathematical safety floor; `R = n` is the arming
policy; any `R` between them trades liveness margin for straggler tolerance and
must be documented as a conditional guarantee at schedule time.

**Readiness is a liveness hint, never a safety input.** Safety at `H` comes
entirely from the app-hash quorum certificate: a node that signals-then-diverges
is simply out-voted and never finalizes. Defense-in-depth (recommended, not
required): have `SignalReady` carry a commitment to the new logic — the expected
NEW app-hash of a known canonical state, or a build/commit fingerprint matched
against the genesis-scheme-fingerprint pattern
(`bin/node/src/config.rs:210-222`) — so a signal that does not match the new code
path is rejected at ingest, converting a silent halt-risk into a visible refusal.

### 4. Deterministic activation + migration at H

At the agreed boundary the dual-path modules switch on `to_version`, and any
state migration runs as a **deterministic function of agreed state** — same
contract as `execute`: no wall-clock, no IO, no RNG, sorted/BTreeMap iteration
only. The post-migration app-hash is quorum-checked, so a nondeterministic
migration fails loud rather than forking.

**Threading the version into execution.** `Module::root(&self)` receives no
`Ctx`, no `Env`, no height, no version (`crates/kernel/sdk/src/lib.rs:236`;
`global_root` folds `(id, root())` with `root()` taking only `&self`). So a
root()-changing module has **no host-supplied input to branch on inside
root()**. The design therefore uses two complementary threads:

1. **`protocol_version: u32` on `BlockContext` and `Env`** (added to
   `crates/kernel/host/src/lib.rs:46-53` and `crates/kernel/sdk/src/lib.rs:140-149`),
   stamped by the node layer at `crates/kernel/node/src/lib.rs:972` as
   `effective_version(height)` (the pure derivation above), NOT the raw stored
   `current_version` — which at the start of block `H` still holds the OLD value.
   Stamping the raw field would dispatch block `H` under OLD logic while each
   changed module's `active_version` already selects the NEW branch in `root()`, a
   deterministic but real off-by-one (H hashes new, dispatches old); stamping the
   derivation keeps dispatch and hashing agreed that block `H` runs `to_version`.
   It is a **read-only dispatch input**: consumed only inside an explicit dual-path
   branch of the one changed module, and **NEVER hashed into any module root
   preimage or op encoding**. Unchanged modules ignore the field, so their roots
   stay byte-identical below and above `H`.
2. **`active_version: u32` as INTERNAL module state** for any root()-changing
   module (e.g. `Forge.active_version`), read by `root()`/`snapshot()`/`install()`
   from `&self`, set deterministically by the activation hook at the `H`
   boundary. The cached version selects the branch but is **not itself part of
   the root preimage**.

**Version-gate the WHOLE changed-module surface**, not just `root()`: `execute`,
`query`, `snapshot`, `install`, `root`, and op/wire semantics (e.g. forge's
repo-field routing). Below `H` the dual-path binary must replicate old behavior
**byte-for-byte** for every op at every height — same accept/reject decisions,
same snapshot serialization, same roots — otherwise two binaries diverge in the
state they build BELOW `H` and fork before activation.

**Fencepost at H.** The version governing every block is
`effective_version(height)` derived from **committed state (end of `H−1`)** before
dispatch; the stamped `BlockContext.protocol_version` and each module's
`active_version` are both set to that single derived value — never a separately
read/mutated `current_version`, never mid-drain — so block `H` runs under exactly
one version (`to_version`) on every node. The upgrade module reconciles its stored
`current_version` to `to_version` by the same deterministic activation transition,
but it is the derivation, not the stored field, that governs dispatch and hashing
at the fencepost. Realize this through the existing epoch
teardown-respawn cutover (`crates/kernel/consensus/src/lib.rs:110-120`;
`Manifest.pending_cutover_view` at `crates/kernel/recovery/src/lib.rs:347`).

**One boundary carries both concerns.** The `ValsetOrchestrator` holds only ONE
pending cutover and returns `Pending`, ignoring a second arm while one is in
flight (`crates/kernel/consensus/src/valset_orchestrator.rs:129, 214-217`). So do
NOT arm a competing cutover for the version flip. Instead, model activation as a
boundary **READ** of `effective_version(H)` at the SAME finalized boundary the
orchestrator already crosses — the respawn already reads the boundary valset from
frozen app state (`crates/kernel/consensus/src/valset_orchestrator.rs:237-256`);
read the active protocol version there too (extend `RespawnPlan` /
`boundary_members` read to carry the boundary version). When a membership change
and a pending-upgrade-at-`H` land in the same window, ONE respawn applies BOTH
atomically (new valset AND new `to_version`) at a single finalized view.
Alternatively, deterministically reject scheduling `H` inside an armed cutover
window; the merge option is preferred since the orchestrator already refuses
concurrent arms.

**Arm/abort is a deterministic self-transition evaluated exactly once at `H`.**
Against the frozen readiness set and boundary valset: if armed → flip; else →
emit a **clear-pending follow-up** op (exactly like governance emits valset
follow-ups) so the clear lands at one finalized view on every node. No timers, no
node-local readiness view, no operator step. Signals arriving after `H` are
inert. After a clean abort the operator simply reschedules with a later `H`.

### Manifest version fields (recovery + state-sync)

Add to **both** the recovery `Manifest`
(`crates/kernel/recovery/src/lib.rs:329-365`, mirroring the existing
`pending_cutover_view`) and the state-sync `Manifest`
(`crates/kernel/statesync/src/lib.rs:132-150`, and its wire codec
`encode_response` ~`crates/kernel/statesync/src/lib.rs:241-263`):

- `current_version: u32` at the captured boundary;
- the pending `Upgrade { name, activation_height, to_version }` coordinates;
- `required_min_version: u32` = the highest protocol version any block at/after
  this boundary needs (`to_version` when the served height `>= activation_height`,
  else `current_version`).

On resume/join the node **preflight-checks** its binary's
`MAX_PROTOCOL_VERSION` against `required_min_version` at boot and refuses EARLY
with a clear "height N requires binary vX" — matching the existing fail-loud
pre-recovery boot posture — instead of an opaque post-replay app-hash mismatch at
`crates/kernel/recovery/src/lib.rs:971-975`. A pending upgrade is **re-armed on
resume** exactly as `pending_cutover_view` re-arms a mid-window cutover
(`crates/kernel/consensus/src/valset_orchestrator.rs:146-169`), so a node that
crashed mid-window rejoins the identical deterministic `H`, and `active_version`
is restored per replayed/synced height.

These manifest fields are an **unauthenticated preflight hint** under the
existing state-sync trust model (server untrusted,
`crates/kernel/statesync/src/lib.rs:21-29`). The **authoritative** version stays
derivable from the replayed/committed upgrade-module state and is confirmed by
the final app-hash compose — so a lying manifest can at worst mis-preflight a
joiner, never induce a fork.

## Relationship to commonware (and the namespace invariant)

Commonware — the pinned `commonware-*` `2026.5.0` primitives Ducktape builds on —
provides **no** upgrade scheme: no software-upgrade coordinator, no halt-at-height,
no scheduled-upgrade primitive, no state-migration framework, and no
protocol/software **version negotiation** in its p2p handshake. (The halt-at-height
`x/upgrade` pattern is Cosmos/Tendermint, not commonware.) The entire mechanism in
this spec therefore lives in the Ducktape app layer (the `upgrade` module +
governance + valset + the height gate); commonware only supplies the deterministic
finalized-height boundary we anchor on.

Two properties of the substrate shape the design:

**1. Reuse the per-epoch engine teardown-respawn; do not reconfigure in place.** A
`commonware_consensus::simplex::Engine` fixes its `(scheme, participants, epoch)` at
construction and exposes only `new()`/`start()` — there is no in-place reconfigure.
Commonware's own model of a validator-set or scheme change is exactly Ducktape's:
finalize through the OLD engine, then tear down and re-spawn a new engine for the
new epoch (`crates/kernel/consensus/src/lib.rs:110-120`, `valset_orchestrator.rs`).
Activation hangs on that same finalized boundary, honoring the
finalize-through-the-old-engine-first invariant. Do NOT hand-roll
epoch/finalization, and do NOT duplicate commonware's deferred `bls12381_threshold`
+ DKG-resharing reconfiguration path — Ducktape consciously chose the multisig
`V2Bls` scheme instead (`crates/kernel/consensus/src/lib.rs:99-100`).

**2. THE NAMESPACE INVARIANT: a no-downtime upgrade MUST NOT change the network
namespace.** commonware's p2p/stream handshake does no version negotiation; its ONLY
compatibility gate is a shared `namespace` byte string bound into the noise-style
handshake transcript — a mismatched namespace ⇒ the peers cannot connect at all.
Ducktape's namespace is `chain_id@genesis_fingerprint`, where the fingerprint is
`sha256(scheme || sorted genesis validators)`
(`NetworkDescriptor::genesis_namespace`, `bin/node/src/config.rs:202-233`, applied
as the runtime `namespace` at `bin/node/src/config.rs:734` and threaded into the
mesh/engine at `bin/node/src/main.rs:1294,1432,1697`); by its own doc it
domain-separates the discovery handshake, the simplex scheme, and the epoch genesis
floor. The consequence is structural and load-bearing:

- A `root()`/op-encoding upgrade changes NONE of `(chain_id, scheme, genesis
  validators)`, so the namespace is **automatically stable** — old and new
  dual-path binaries keep handshaking throughout the rolling window. This is a
  deeper reason the no-downtime class is exactly "module id + scheme + genesis
  unchanged": version gating rides the app/consensus payload (the `upgrade` module's
  app-hash state + `protocol_version`/`active_version`), never the handshake.
- Anything that WOULD move the namespace — a consensus-scheme migration (V1→V2 BLS),
  the upgrade-module/genesis retrofit onto Ducktape-2, or any change that bumps the
  genesis fingerprint — **severs the mesh** (old and new nodes cannot connect) and
  is therefore a stop-the-world / coordinated-partition operation, NOT a rolling
  no-downtime upgrade. The implementer MUST NOT bump the namespace or genesis
  fingerprint as part of a no-downtime upgrade; doing so mid-rollout partitions the
  network. This is a further reason scheme changes and the retrofit sit outside the
  zero-downtime guarantee (see Non-goals).

## Policy guarantees (this IS the no-downtime upgrade policy)

- **No NETWORK downtime.** Rolling binary rollout; behavior byte-identical below
  `H`; all ready nodes flip at `H` together. `R = n` is the arming policy (§3):
  the unconditional no-downtime claim holds at `R = n`, with `2f+1` only the
  mathematical safety floor beneath it.
- **NEVER FORK — fail loud instead.** A node that reaches `H` without the new
  binary/logic **HALTS** (like the fail-loud pre-recovery boot) rather than
  applying ambiguous logic. That is downtime for THAT straggler, never a fork; it
  refuses to vote so it cannot even help form a competing old-hash quorum, and
  the network proceeds on the ready supermajority. Enforcement points already
  exist: the live `AppHashMismatch` at
  `crates/kernel/host/src/lib.rs:375-379` and the recovery compose check at
  `crates/kernel/recovery/src/lib.rs:971-975`. The design adds only the earlier
  preflight; it never weakens these.
- **Version is agreed state.** A synced or late-joining node knows exactly which
  logic to run for any height (statesync + recovery manifests gain the version
  fields; the authority is the app-hash, not the manifest).
- **Migrations deterministic and versioned; version is MONOTONIC** (no
  downgrade). **Irreversible-by-default:** recover from a bad upgrade by rolling
  FORWARD (a new `ScheduleUpgrade` to `to_version + 1` with corrected logic and,
  if needed, a corrective deterministic migration), never by rewinding committed
  state.
- **Abort path.** If the readiness quorum is not met by `H`, the pending upgrade
  does NOT activate — it is deterministically cleared and the network continues
  on old logic; the operator reschedules.

### The unconditional safety argument

1. The flip is a **deterministic predicate over agreed state** — readiness count
   in the in-app-hash upgrade module plus `H` reached — evaluated at the same
   frozen teardown-respawn boundary every honest node reads identically
   (`crates/kernel/consensus/src/valset_orchestrator.rs:237-256`, boundary set
   read from discard-ceiling-frozen state). Honest nodes UNANIMOUSLY flip-or-abort;
   they never split.
2. An app-hash is committed only via a `2f+1` quorum certificate
   (`crates/kernel/host/src/lib.rs:341-344`); a divergent old-hash (or lying
   new-hash) block cannot gather `2f+1` against the honest supermajority, so it
   never finalizes — the diverging node is out-voted, its state stays local.
3. Two conflicting `2f+1` certificates would need `≥ 4f+2 > n = 3f+1` votes —
   impossible.

Therefore Byzantine readiness can at worst DENY a quorum (halt), never fork.

## Worked example: the forge multi-repo change

The forge change altered `Module::root()` composition: a single committed head →
a canonical sorted hash over per-repo heads (`compose_root` at
`crates/apps/forge/src/lib.rs:214`, `root()` at
`crates/apps/forge/src/lib.rs:790`). It keeps module id `"forge"`, so the
registry set is unchanged — a `root()`-only change, the safe worked example.

Shipping it with no downtime:

1. **Prerequisite:** the `upgrade` module is already genesis-embedded and live.
2. **Pre-ship dual-path forge.** The new binary carries `Forge.active_version`.
   Below `H`, `root()` computes the OLD 20-byte preimage
   (`sha256(default_head)`, `StateRoot::ZERO` when unborn); at/after `H` it
   computes the sorted `compose_root` (`sha256(u32(len) ++ name ++ oid)` per
   repo, sorted). The whole surface is gated: below `H` the dual-path forge
   collapses/ignores the multi-repo `repo` field to the default
   (`norm_repo`, `crates/apps/forge/src/lib.rs:141`), refuses to materialize any
   non-default repo, and serializes snapshots in the old single-head format — so
   its accept/reject and every root match the old binary byte-for-byte below `H`.
3. **`ScheduleUpgrade { name: "forge-multi-repo", activation_height: H,
   to_version: 2 }`** via governance.
4. Operators roll the dual-path binary out node-by-node; each node emits
   `SignalReady` once running it.
5. At `H`, arming quorum met → every ready node flips atomically over the
   teardown-respawn boundary: `Forge.active_version` flips and `root()` recomputes
   from the existing in-memory `RepoState.head` values under the new preimage
   layout.

The forge migration is a **zero-data-movement** migration — only the in-memory
head root encoding flips; the on-disk odb and blob plane are never consulted
during the switch, which is exactly why forge is the safe worked example.

## Edge cases

- **Restart / late-join across `H`.** Manifest version fields + per-height
  `active_version` restoration select the correct dual-path branch before
  replay/serve; preflight refuses an under-versioned binary early. (§ Manifest
  version fields.)
- **New validator admitted between `ScheduleUpgrade` and `H`.** It legitimately
  enters the boundary valset, so it is counted in the readiness **denominator**;
  if provisioned with the OLD binary it can never `SignalReady`, so `R = n` is
  unmet and the deterministic abort fires rather than a partial flip. The runbook MUST gate admission during an open upgrade window:
  any validator admitted after a `ScheduleUpgrade` is provisioned
  **new-binary-first**. A node whose first executed height is `>= H` MUST run the
  new binary.
- **Signaled node downgraded before `H`.** The ready signal is a best-effort
  promise about the binary at signal time, not a proof; it survives a
  downgrade-restart and still counts toward arming, but the reneged node halts at
  `H`. Mitigations: the `R = n` arming policy (which sits above the bare `2f+1`
  safety floor), make it a runbook invariant that a signaled node MUST NOT be
  downgraded, and optionally epoch-scope the signal so the teardown-respawn
  invalidates stale promises and validators re-affirm on the new binary.
- **Re-schedule after abort.** `SignalReady` is scoped to the specific pending
  upgrade identity (`name` + `to_version`); clearing `pending` (on abort OR
  activation) clears the readiness set, so stale signals from an aborted round
  cannot arm a fresh upgrade.
- **Wrong-version leader at `H`.** A straggler/Byzantine leader that proposes an
  old-logic block is rejected by honest ready nodes (its app-hash never gathers
  `2f+1`); the view nullifies and rotates. This is normal simplex leader
  rotation — bounded extra views at `H`, expected, not an error. It reinforces
  the `R = n` policy: with every boundary validator ready you are guaranteed
  `≥ 2f+1` honest ready nodes, so a correct leader is reachable within `f+1`
  rotations. (This is a LIVENESS margin — leader reachability — not a safety
  input; the app-hash certificate prevents forks at any `R`.)
- **Duplicate / replayed ops.** `SignalReady` is idempotent (keyed by pubkey);
  duplicate `ScheduleUpgrade` via re-`Execute` is already blocked because a
  settled proposal is terminal (`crates/system/governance/src/lib.rs:283`) and
  duplicate `proposal_id`s are rejected at propose
  (`crates/system/governance/src/lib.rs:224`).
- **Module-SET-changing upgrades (OUT of the base guarantee — harder class).**
  Forge multi-repo keeps its id, so the set is unchanged and the base mechanism
  covers it. An upgrade that ADDS or REMOVES a module id is NOT covered by the
  base mechanism, because the naive recipe ("register/remove the module at the `H`
  teardown-respawn") is unsafe two ways. **(1) Pre-`H` fork during rollout.**
  `global_root` iterates the host registry
  (`crates/kernel/host/src/lib.rs:272-301`) over a genesis-constant module vec
  (`bin/node/src/main.rs:242-278`); a new binary that includes the added module
  composes a DIFFERENT `global_root` than an old binary for every block BELOW `H`,
  so the two diverge during the node-by-node pre-ship window — the exact fork the
  mechanism exists to prevent. The `active_version`/`protocol_version` trick cannot
  paper over this: `protocol_version` is deliberately never hashed, and a module's
  mere presence in the registry IS its contribution to `global_root`. **(2)
  Cross-`H` recovery/state-sync mass-halt.** The recovery `Cutover` handler mutates
  only epoch/view_base/participants and never the registry
  (`crates/kernel/recovery/src/lib.rs:840-852`); a registry mutation performed in
  the respawn is invisible to replay, so every node that restarts or state-syncs
  across such an `H` reconstructs the WRONG module set and fails the compose check
  — network-wide, not one straggler. **What a module-set change additionally
  requires:** registry composition must itself become a pure, height-gated function
  of `effective_version(height)` — `global_root` folds a module id at a height iff
  that id is active at that height — reproduced IDENTICALLY in the live-execute,
  recovery-replay, AND state-sync-install paths (which today all assume a fixed
  genesis registry). That is a separate, larger design; until it exists this spec
  scopes module-set changes OUT of the no-downtime guarantee (see Non-goals).

## Failure / abort / operator recovery

- **Readiness quorum not met by `H` (clean abort).** Evaluated once,
  deterministically, at the `H` boundary as a pure function of the frozen
  readiness set + boundary valset. Not armed ⇒ emit a clear-pending follow-up ⇒
  every node discards `pending` and readiness at one finalized view and continues
  on OLD logic. The operator then reschedules with a later `H`. No timers, no
  local view, no operator step in the clear itself.
- **Straggler halt at `H`.** A node whose `MAX_PROTOCOL_VERSION < to_version`
  HALTS fail-loud at the activation boundary (the same posture as the
  pre-recovery boot halt) rather than executing block `H` under stale rules —
  downtime for that node only, never a competing old-hash quorum. Its state up to
  `H−1` is valid and under the app-hash, so **recovery needs no wipe** (unlike the
  damaged-dir pre-recovery case): install the new dual-path binary, restart, let
  recovery replay through `H` under the new branch, and rejoin.
- **A binary that predates the upgrade module entirely** diverges even earlier —
  at the block where a `ScheduleUpgrade` follow-up first mutates the in-hash
  upgrade module — which is why the rollout gate is "upgrade-module-aware
  dual-path binary on every node BEFORE `ScheduleUpgrade` commits", not merely
  before `H`.
- **Bug discovered AFTER activation.** Committed state at/above `H` is under the
  app-hash and immutable; state rollback is forbidden (it would rewrite hashed
  history and fork against any node already synced past `H`). Recovery is
  **roll-forward only**: a new `ScheduleUpgrade` to `to_version + 1` at a new
  `H'`. Because after `H` there is no rollback fallback, the spec MANDATES that
  every new-logic branch and every migration obey the execute purity contract
  (no wall-clock, IO, or RNG) and requires a pre-schedule determinism /
  replay-equivalence test as policy. A non-deterministic migration would already
  have diverged AT `H` and be unrecoverable.
- **Mis-scheduled but validly-armed upgrade.** `CancelUpgrade` (governance-gated,
  valid only while `current_height < H`) stops it before the boundary; on cancel
  the readiness set is cleared.

## Non-goals

- **No Rust implementation in this pass.** This document is the design of record;
  it specifies the mechanism precisely enough to implement later. No consensus
  code changes ship here.
- **Binary DISTRIBUTION mechanics are out of scope for the consensus mechanism.**
  How operators fetch, verify, and stage the dual-path binary (package channels,
  signatures, rollout choreography) belongs to the upgrade **skill/ops runbook**,
  not the on-consensus mechanism. The mechanism only cares that the correct
  binary is present per the readiness/preflight rules.
- **Retrofitting the upgrade module onto a network that lacks it** (Ducktape-2 as
  it stands) is explicitly NOT a no-downtime operation — it is a one-time
  coordinated genesis bump, out of scope for the zero-downtime guarantee.
- **Module-SET changes (adding or removing a module id) are out of the base
  no-downtime guarantee.** The base mechanism covers `root()`/op-encoding changes
  with a stable module id. A set change needs height-gated registry composition
  reproduced identically across the live, recovery-replay, and state-sync-install
  paths (see Edge cases); until that exists a module-set change is either deferred
  or shipped via the one-time coordinated genesis bump, never a no-downtime
  upgrade.
- **Downgrades / state rewinds** are not supported. Version is monotonic;
  recovery is roll-forward only.
- **Changing the upgrade module's own logic** is not covered by a single
  scheduled upgrade of a target module; it would be its own separately-versioned,
  height-gated upgrade.

## Code anchors

- app-hash composition: `crates/kernel/host/src/lib.rs:341-344`
  (`Host::app_hash` → `state::global_root`); `AppHashMismatch` enforcement
  `crates/kernel/host/src/lib.rs:375-379`.
- teardown-respawn / rekey contract:
  `crates/kernel/consensus/src/lib.rs:110-120`; orchestrator
  `crates/kernel/consensus/src/valset_orchestrator.rs` — single pending slot
  l.129 / l.214-217, `ScheduledCutover` armed at `observed + cutover_delay`
  l.225-232, `respawn_if_due` boundary read l.241-265, `resume`/re-arm
  l.146-169, `EpochMembership` l.9-34.
- governance: `crates/system/governance-interface/src/lib.rs:19-26` (`GovAction`);
  `crates/system/governance/src/lib.rs:274-328` (`handle_execute` tally then
  valset follow-up), simple-majority l.296, terminal-proposal l.283, duplicate
  propose-guard l.224, ballot last-write-wins l.268-269.
- valset origin gating: `crates/system/valset/src/lib.rs:240-247`.
- recovery manifest (gains version fields):
  `crates/kernel/recovery/src/lib.rs:329-365`; replay `apply_block →
  host.submit_at` l.996-1017; `Cutover` handler touches no module state
  l.840-852; final compose check l.971-975.
- state-sync manifest (gains version fields):
  `crates/kernel/statesync/src/lib.rs:132-150`; wire codec `encode_response`
  ~l.241-263; untrusted-server trust model l.21-29.
- `root()` signature (no version input): `crates/kernel/sdk/src/lib.rs:236`;
  `Env` l.140-149; `BlockContext` `crates/kernel/host/src/lib.rs:46-53`; node
  stamping seam `crates/kernel/node/src/lib.rs:972-976`.
- module set hard-coded vec: `bin/node/src/main.rs:242-278`; host registry wiring
  `crates/kernel/host/src/lib.rs:272-301`.
- consensus-scheme compile-time const / genesis fingerprint:
  `bin/node/src/main.rs:75`; `bin/node/src/config.rs:26, 202-233`.
- commonware substrate + namespace gate: pinned `commonware-* 2026.5.0` (primitives
  only, no upgrade scheme); per-epoch engine teardown-respawn
  `crates/kernel/consensus/src/lib.rs:110-120`, deferred threshold/DKG note l.99-100;
  p2p handshake gates only on `namespace` — `genesis_namespace`
  `bin/node/src/config.rs:202-233`, applied `bin/node/src/config.rs:734`, threaded
  into mesh/engine `bin/node/src/main.rs:1294,1432,1697`.
- forge worked example: `crates/apps/forge/src/lib.rs:141` (`norm_repo`), l.214
  (`compose_root`), l.529/798 (`snapshot`/`state_sync_handle`), l.606-607
  (`install` root gate), l.790 (`root`).
- NOTE: `crates/system/wireguard-upgrade` is UNRELATED — it is the WireGuard
  transport-tunnel bring-up protocol, NOT software upgrade. Do not conflate them.
