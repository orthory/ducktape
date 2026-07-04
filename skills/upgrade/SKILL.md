---
name: upgrade
description: Drive a no-downtime, height-gated Ducktape node upgrade end-to-end — build the dual-path binary, schedule the upgrade through governance, roll it validator-by-validator, gate activation on full-set (R=n) readiness, and verify app-hash continuity across the boundary. Use when the user says upgrade the network, roll out a new node binary, schedule a coordinated upgrade, cut over Ducktape-2 to a new binary, or invokes /upgrade.
---

# Upgrade

Use this to ship a **consensus-breaking** change — a change to a module `root()`
or op/wire encoding (module id unchanged) — to a **live** Ducktape network
with **zero network downtime**. (Changing the module *set* — adding or removing a
module id — is a harder class that is out of scope here; see Guardrails.) The
mechanism is height-gated activation of a
pre-shipped dual-path binary: the new binary is byte-identical to the old below
the activation height `H`, and every ready validator flips to new logic together
at `H`.

The design of record is
`docs/superpowers/specs/2026-07-04-no-downtime-node-upgrade-design.md`. Read it
before driving a real upgrade; this skill is the operational runbook for that
spec. The `ScheduleUpgrade` / `SignalReady` op surface and the `upgrade` system
module referenced below ship **in the dual-path binary** per that spec — confirm
they exist in the build you are rolling before you begin.

## Guardrails (non-negotiable ordering)

These encode the upgrade policy. Never trade them for speed.

- **Never let `H` pass before the readiness quorum is met.** The single job of
  this runbook is to reach quorum *before* the boundary. If `H` is approaching
  and quorum is not met, do not "let it ride" — cancel and reschedule.
- **`R = n` is the arming policy; `2f+1` is the never-arm-below floor, not the
  policy.** Activation arms only when **every current boundary validator** has
  signaled ready (`R = n`). Treat it as *straggler-aborts-the-upgrade*: any
  non-signaler — offline-honest OR withholding-Byzantine — means the flip does
  not fire, which triggers the clean deterministic abort, the network keeps
  running old logic, and you reschedule. That is the unconditional no-downtime
  posture. `2f+1` is only the mathematical **safety floor** — arming below it is
  never allowed — but arming at *exactly* `2f+1` is explicitly rejected as the
  default: a within-budget adversary (`f` validators signal, then run old logic)
  or a crash-after-signal can leave only `f+1` honest nodes on new logic, so the
  new-app-hash block cannot gather `2f+1` and the chain **HALTS at `H`** (a
  liveness failure). That is why `R = n` is chosen. `R = n - s` (tolerating `s`
  known-offline stragglers) is a DISCOURAGED, operator-documented **downgrade**
  of the guarantee — permitted only with out-of-band attestation that every
  signaler genuinely runs the new binary, and it MUST be recorded at schedule
  time as a *conditional* (not unconditional) guarantee. It is never the default.
  Note the correctness point: `R = n` buys **liveness** margin — enough honest
  nodes flip together to finalize the first new-version block — NOT additional
  safety. Safety / no-fork is guaranteed independently by the app-hash `2f+1`
  quorum certificate at *any* value of `R`: a node that signals then diverges is
  simply out-voted and never finalizes. `R = n` prevents the halt-at-`H`
  liveness failure, not forks.
- **Abort cleanly if quorum is missed by `H`.** A live-but-unupgraded network is
  strictly safer than a fired-but-unfinalizable flip. Cancel before `H` (see
  Abort) and reschedule with a later `H`. Do not force the flip.
- **Fail loud, never fork.** A node that reaches `H` without the new logic must
  HALT, not apply ambiguous logic. Halting one straggler is acceptable; a fork
  is not.
- **Version is monotonic; recovery is roll-forward only.** No downgrades, no
  state rewinds. Fix a bad upgrade with a new `ScheduleUpgrade` to
  `to_version + 1`, never by rolling back committed state.
- **This runbook covers the SECOND-and-later upgrade.** It assumes the `upgrade`
  module is already genesis-embedded and live on every node. Retrofitting the
  module onto a network that lacks it (Ducktape-2 as it stands) is a one-time
  coordinated genesis bump, NOT a no-downtime operation — out of scope here.
- **Module-SET changes (adding or removing a module id) are out of scope.** This
  mechanism ships `root()`/op-encoding changes with a stable module id. A module's
  presence is itself part of `global_root`, so a new binary that adds/removes a
  module would diverge from the old one *during* rollout (before `H`), and a
  respawn-time registry change is invisible to recovery replay — it needs a
  height-gated registry-composition mechanism that does not yet exist. Do NOT
  drive a module-set change here; defer it or use a coordinated genesis bump. See
  the spec's Edge cases / Non-goals.

## Step 0: Frame The Upgrade

1. Confirm the change is genuinely consensus-breaking (touches a `root()` or op
   encoding). If it is not, it needs no coordinated upgrade — ship it as a normal
   rolling restart. If it changes the module **set** (adds or removes a module
   id), STOP — that is out of scope for this runbook (see Guardrails).
2. Confirm the new binary is **dual-path**: below `H` it must reproduce old
   behavior byte-for-byte for `execute`, `query`, `snapshot`, `install`, `root`,
   and op/wire semantics, branching to new logic only on the active protocol
   version. If the branch is not there, the change is not ready to schedule.
3. Compute the fault budget and thresholds from the current validator count `n`:

```bash
n=<current validator count>          # e.g. 4
f=$(( (n - 1) / 3 ))                 # max Byzantine (n = 3f+1)
quorum_floor=$(( 2 * f + 1 ))        # safety floor — never arm below this
target=$n                            # R = n: THE arming policy — drive to this
echo "n=$n f=$f quorum_floor=$quorum_floor arming_policy(R=n)=$target"
```

   `R = n` is the arming policy this runbook drives toward: shepherd **every**
   boundary validator to signal so the flip arms at `R = n`. `2f+1` is only the
   never-arm-below safety floor, not a target to settle at — arming at exactly
   `2f+1` can still HALT the chain at `H` (a liveness failure; see Guardrails).
   Arming below `R = n` — at `R = n - s`, tolerating `s` known-offline stragglers
   — is a documented **conditional downgrade** requiring out-of-band attestation
   that every signaler runs the new binary, and it is never the default.

4. Pick `H` strictly in every node's future, with lead at least the
   orchestrator's `cutover_delay` window so the boundary can be armed on every
   node before it arrives. Leave generous slack for the node-by-node roll.

## Step 1: Build The Dual-Path Binary

Build the release binary from the branch that carries the dual-path change and
its bumped `MAX_PROTOCOL_VERSION`.

```bash
cargo build --release --bin ducktape-node   # crate: node-bin, at bin/node
```

Record an identity for the artifact so every operator can confirm they run the
same code (the spec's defense-in-depth: signals should attest to the running
binary, not a hand-typed claim):

```bash
sha256sum target/release/ducktape-node
git rev-parse HEAD
```

Sanity-check the binary still matches old behavior below `H` before you trust
it in production — run the cluster e2e suite, which asserts genesis and
converged app-hashes agree across nodes:

```bash
cargo test -p node-bin --test cluster_e2e
```

## Step 2: Distribute To The Fleet

Distribution mechanics (package channels, signatures, staging) are an ops
concern, not a consensus concern — the mechanism only requires the correct
binary be present per the readiness/preflight rules. Stage the artifact on every
validator host **without restarting yet**; each node keeps running old logic
until you roll it in Step 4.

- Production validators: copy the artifact and its checksum to each host, verify
  the checksum on arrival, stage it beside the running binary.
- Local test net / worktree fleet: rebuild and redeploy per node.

```bash
# local multi-node test net driven by the fleet harness
ops/fleet.sh status
ops/fleet.sh up <branch…>     # bring per-worktree apps up on the new build
```

**Admission gate during an open window:** any validator admitted after a
`ScheduleUpgrade` MUST be provisioned new-binary-first. A node whose first
executed height is `>= H` must run the new binary, or it will halt at `H` and
withhold the quorum.

## Step 3: Schedule The Upgrade Through Governance

Governance is the sole author of the pending upgrade. Drive it exactly like
membership admission is driven (`GovMsg::Propose` / `Vote` / `Execute` over a
member node's local RPC — the same path `invite-accept` wraps for
`AddValidator`), with the `ScheduleUpgrade` action:

```text
GovAction::ScheduleUpgrade { name: "<upgrade-name>", activation_height: H, to_version: <v> }
```

1. From a member node, `Propose` the `ScheduleUpgrade` and shepherd votes to a
   simple majority (`members.len()/2 + 1`), then `Execute`. The passing proposal
   emits an upgrade-module follow-up that records the pending upgrade in agreed,
   app-hash-included state.
2. Confirm the pending upgrade is live and identical on every node. Query the
   `upgrade` module (mirror the governance `GovQuery::Proposal` query pattern):

```bash
# per the spec's query surface, e.g.:
ducktape-node upgrade-status --config <member node.toml>
# expect: current_version, pending { name, activation_height=H, to_version }, readiness=0
```

3. Ingest is deterministically gated — a schedule is rejected if
   `to_version <= current_version` (no downgrade), if `H` is not strictly in the
   future by the minimum lead, or if a pending upgrade already exists. If it
   rejects, fix the inputs; do not work around the gate.

Authorization is not activation. A passing vote only SCHEDULES; activation still
requires the R=n readiness quorum below.

## Step 4: Roll Node-By-Node And Confirm SignalReady

Restart validators onto the new binary **one at a time**, confirming each is
healthy and has signaled before moving to the next. There is no network downtime
because every not-yet-rolled node keeps running old logic until `H`.

For each validator:

1. Swap in the new binary and restart the node.
2. Confirm it rejoined and is finalizing (watch its log markers):

```bash
# app-hash progress markers the node prints; all peers must agree per height
grep -E "converged app_hash=|synced app_hash=|recovered app_hash=" <node.log> | tail
```

3. Confirm it emitted `SignalReady { name, to_version }`. A node signals only
   when its own `MAX_PROTOCOL_VERSION >= to_version`, so a signal is a truthful
   statement about the running binary. Watch the readiness count climb:

```bash
ducktape-node upgrade-status --config <member node.toml>   # readiness += 1 per rolled node
```

Signals are validator-origin, idempotent (one member = one signal,
last-write-wins), and scoped to this exact `name` + `to_version` — stale signals
from an aborted round cannot count.

**Do not downgrade a signaled node.** A reneged signal still counts toward
arming but the node halts at `H`. Keep every signaled node on the new binary
through `H`.

## Step 5: Confirm The Readiness Quorum Before H

Before the boundary arrives, verify the readiness set against the valset
**as-of `H`** (governance may have moved membership since scheduling; the
denominator and threshold are recomputed against the boundary valset, and a
signal from a non-member is dead weight).

```bash
ready=$(ducktape-node upgrade-status --config <member node.toml> | ready-count)
echo "ready=$ready  floor(2f+1)=$quorum_floor  arming_policy(R=n)=$target"
```

- If `ready >= target` (`R = n`): the arming policy is met — every boundary
  validator has signaled. Proceed; the flip fires with the unconditional
  no-downtime guarantee.
- If `quorum_floor <= ready < target` (`R = n - s`): you are above the safety
  floor but below the `R = n` policy. This is a DISCOURAGED downgrade, not the
  default: proceed ONLY with out-of-band attestation that every signaler
  genuinely runs the new binary, and record the guarantee as *conditional* (not
  unconditional) at schedule time; otherwise keep rolling to `R = n` or
  reschedule. Note that arming at exactly `2f+1` can still HALT the chain at `H`
  — a within-budget adversary or a crash-after-signal can leave only `f+1`
  honest nodes on new logic, so the new-app-hash block cannot finalize (a
  liveness failure, not a fork).
- If `ready < quorum_floor` as `H` nears: **you would be arming below the safety
  floor — never allowed. Abort.** Do not let `H` pass. Cancel and reschedule
  (see Abort).

This is the hard gate. The activation height must not pass below the `R = n`
policy unless the `R = n - s` downgrade is explicitly attested and recorded as
conditional — and it must never pass with `ready < 2f+1`. Under `R = n` any
straggler aborts the upgrade cleanly: the flip does not fire, the network keeps
running old logic, and you reschedule.

## Step 6: Watch The Activation Boundary

At `H` the flip is a deterministic self-transition evaluated once against the
frozen readiness set and boundary valset, landed atomically over the epoch
teardown-respawn boundary. There is no operator action in the flip itself.

- **Armed → flip.** Every ready node switches to `to_version` at one finalized
  view; the changed module's `active_version` flips and its `root()` recomputes
  under the new preimage. Block `H` runs under exactly one version on every node.
- **Not armed → clean abort.** The pending upgrade is deterministically cleared
  (a clear-pending follow-up lands at one finalized view), the readiness set is
  reset, and the network continues on old logic. Reschedule with a later `H`.

Watch the boundary land and confirm the network is still finalizing past `H`:

```bash
# the network keeps producing converged app_hashes across and beyond H
grep -E "converged app_hash=" <node.log> | tail
ducktape-node upgrade-status --config <member node.toml>   # current_version == to_version, pending cleared
```

## Step 7: Verify App-Hash Continuity Across H

Prove the flip was atomic and no honest node forked or silently diverged.

1. **Agreement per height (no fork).** For each of `H-1`, `H`, and a few blocks
   after, collect the `converged app_hash=` marker from every honest node and
   assert they are byte-identical at each height:

```bash
# per node, extract the app_hash at a given height and compare across all nodes
for log in <node-0.log> <node-1.log> …; do
  grep "converged app_hash=" "$log" | grep "height=H" ;   # all must match
done
```

2. **Expected change at `H`.** The changed module's contribution to the app-hash
   SHOULD differ from `H-1` (that is the whole point of a `root()` change) — but
   the app-hash must be **identical across nodes** at `H`. Same-across-nodes is
   the safety property; changed-vs-previous is the intended effect.
3. **No halt on honest nodes.** Confirm no honest node emitted an
   `AppHashMismatch` or fail-loud halt at `H`. Any straggler that halted (wrong
   binary) is expected downtime for that node only — it never formed a competing
   old-hash quorum. Recover it in place (see Abort).
4. **Restart / late-join across `H`.** If any node restarted or a joiner synced
   across the boundary, confirm its `recovered app_hash=` / `synced app_hash=`
   matches the live network at that height — the manifest version fields and
   per-height `active_version` restoration must have selected the correct branch.

If any per-height app-hash disagrees between honest nodes, treat it as a
consensus incident immediately — that is the fork this whole mechanism exists to
prevent.

## Step 8: Report

Report concisely:

- The upgrade `name`, `activation_height H`, and `to_version`.
- The binary identity rolled (`git rev-parse HEAD` + `sha256sum`).
- The final readiness count vs `n` and the `2f+1` floor; whether arming hit
  `R = n` or a documented lower margin.
- Whether the flip ARMED at `H` or ABORTED cleanly (and the reschedule if so).
- App-hash continuity result: per-height agreement across honest nodes at
  `H-1`/`H`/`H+`, that the changed root moved as intended, and that no honest
  node forked or halted.
- Any straggler halts and their recovery status.

## Abort And Straggler Recovery

- **Missed quorum before `H`.** `CancelUpgrade { name }` (governance-gated, valid
  only while `current_height < H`) stops the upgrade before the boundary; on
  cancel the readiness set is cleared. Then reschedule with a later `H`. If you
  do nothing and quorum is genuinely unmet at `H`, the mechanism aborts
  deterministically anyway — but cancel explicitly rather than racing the
  boundary.
- **Straggler halted at `H`.** A node whose binary predates `to_version` halts
  fail-loud rather than executing block `H` under stale rules. Its state up to
  `H-1` is valid and under the app-hash, so no wipe is needed: install the new
  dual-path binary, restart, let recovery replay through `H` under the new
  branch, and rejoin.
- **Bug found AFTER activation.** Committed state at/above `H` is immutable — no
  rollback. Recover roll-forward only: a new `ScheduleUpgrade` to `to_version + 1`
  at a new `H'` with the corrected logic and, if needed, a corrective
  deterministic migration. Every new-logic branch and migration must obey the
  execute purity contract (no wall-clock, IO, or RNG); run a determinism /
  replay-equivalence check before scheduling.
