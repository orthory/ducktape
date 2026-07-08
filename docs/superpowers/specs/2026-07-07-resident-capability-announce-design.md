# Capability announce for joined (non-genesis-validator) nodes — design

Date: 2026-07-07. Status: approved for implementation (autonomous tailored call).

## Goal

A node that joins the network announces its capabilities without needing to be
a genesis validator. Today a *promoted* validator does announce (the pump is
state-driven in the validator loop), but a **resident** (joined → admitted →
synced, not yet promoted) never announces: the host announcer only runs in the
validator loop, and the capability module rejects non-validator submitters.

## Decision

Two coordinated changes, mirroring the identity module's precedent:

### 1. Module gate: validators ∪ residents

`crates/system/capability/src/lib.rs` — `members()` currently queries only
`ValsetQuery::Validators` (lib.rs:97-107) and `execute(Announce)` rejects
non-members (lib.rs:306-314). Relax to **validators ∪ residents**, exactly as
`crates/system/identity/src/lib.rs:109-139` chains
`ValsetQuery::Validators` + `ValsetQuery::Residents` for its BindNode gate.
Keep: external-origin-only, tag validation, declarative-replace semantics.

**Consensus impact**: this changes op-validation behavior → divergent for
mixed binaries. Check whether an in-tree `Env::protocol_version` gating
pattern exists (the video-call ops were ADR'd as version-gated); if a
concrete pattern is in-tree, mirror it. If not, ship unconditional and mark
the PR clearly: **consensus: requires lockstep upgrade of live networks**
(repo precedent: FRAME_NS v2, quack base, identity genesis move).

### 2. Host: pump the announcer at resident tier

`bin/node/src/main.rs` — construct a `CapabilityAnnouncer` (main.rs:516-581)
in the joiner/resident park loop, active once the node has synced (after the
`synced app_hash=` point, main.rs:6434) and holds committed resident
standing. Reuse `capability_host::discover()` for the payload and the
state-driven `maybe_announce()` idempotence (it compares against the
committed registry each tick, so re-announce loops and restarts stay quiet
once matched). Deliver the op via the **submit-relay lane**
(`bin/node/src/relay.rs` — `verify_relay_submit` already admits committed
residents). Respect the existing `announce_capabilities` config flag.
The validator-loop announce path stays as is (covers promotion).

### 3. Dispatch must not regress (hard requirement)

Saga rendezvous-assigns work over announced providers
(`assignment_pool` ← `CapabilityQuery::Providers`). An announced resident
that runs no worker would stall attempts until lease expiry. The implementer
MUST close this one of two ways, choosing after reading the code, and state
the choice in the PR:

- **(a) preferred if plumbing allows**: run the dispatch worker pump for a
  synced resident in the park-loop serve window (it has synced state to
  observe WorkerRequests and the relay lane to submit results), so an
  assigned resident actually executes; or
- **(b) fallback**: filter saga's `assignment_pool` to current validators
  (saga already has valset wired via `with_assignment`), making resident
  announce registry-visibility-only until promotion. Also consensus-
  affecting; rides the same lockstep note.

Announce-only with residents entering the assignment pool and stalling
leases is NOT acceptable.

## Testing

- Module unit tests (extend `crates/system/capability/src/lib.rs` tests):
  resident-origin announce admitted; non-member still rejected; validator
  still admitted (extend `member_gate_rejects_non_members_and_admits_members`
  pattern with a valset exposing residents).
- If (a): a test that an assigned resident executes (mirror
  `dispatch_e2e.rs` shapes if a lighter harness isn't available; keep the
  poll budgets of the existing e2e).
- If (b): saga unit test that `assignment_pool` excludes announced
  non-validators.
- Existing capability/host/dispatch tests must stay green.

## Non-goals

- Changing promotion/valset semantics, invite flow, or the accept-lane
  (`announce_capabilities=false`) behavior.
- noded (single-node oracle shape) changes — it has no join flow.
