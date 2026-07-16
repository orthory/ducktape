# Iced sim lane — transaction round-trips against simnode

**Date:** 2026-07-16
**Branch:** `feat/iced-sim-lane` → PR into `feat/iced-app`
**Status:** approved design

## Problem

The iced app has two QA lanes and neither can test a transaction:

- The in-process recipe lane (`shell/qa.rs`, PR #645) runs `iced_test::Simulator`
  plus the real `update()` loop, but has no node and no async runtime — `update`
  Tasks never execute, so a submit never leaves the shell.
- The fleet lane (`ops/iced-fleet`) has a real `ducktape-node`, so transactions
  work but nothing is deterministic and no committed state is scriptable.

The TS app closed this exact gap with `app/src/test/sim/` (vitest suites +
`useSimScenario`, which spawns `ducktape-simnode` and injects
`remoteTransport(base)` into the provider). The iced app needs its twin: UI
action → app submits → simnode commits → committed state renders back in the
UI, deterministically.

## Decision

Extend the in-process lane with the one thing it lacks: Task execution.
A Rust test harness in `ducktape-iced` spawns simnode, injects the transport,
and drains `update()` Tasks on a private tokio runtime. No Emulator, no recipe
schema change, no new preset.

Approaches rejected:

- **`iced_test::Emulator`** — unproven against the `iced::daemon` multi-window
  builder, selector interop was built for Simulator, and its real timers
  (AgentTick, polling ticks) reintroduce the nondeterminism this lane exists to
  kill. Revisit only if push-driven flows turn out to matter in this lane.
- **In-process sim (compose `noded::router` + `host::Host` in the test)** —
  re-implements simnode's `main.rs`; both existing harnesses (TS and Rust)
  spawn the binary and treat the wire as the contract.

## Components

New module `app/src-iced/src/test/sim/` (in-crate `#[cfg(test)]`, same pattern
as the #646 screen tests). Per-surface test files with short names, mirroring
the TS `app/src/test/sim/` layout.

### `SimNode` — spawn/control (~100 lines)

Copied-down spawn bits from `bin/simnode/tests/harness/mod.rs` (simnode has no
lib and its harness lives on `dev`; a shared crate is deferred until the
branches converge):

- Binary resolution: `DUCKTAPE_SIMNODE_BIN`, else
  `target/{debug,release}/ducktape-simnode` (TS precedent).
- Spawns with `--auto`, fresh `mkdtemp` storage per test, free port (bind 0),
  waits on `/v1/status`. `Drop` kills its own verified child — never `pkill`.
- **Vacuous-gate guard:** a missing binary skips loudly (eprintln naming the
  suite and the build command) and the existing `ui-qa` Make target gains a
  `cargo build -p simnode` step before the test run. A silent skip must never
  read as green.

### `SimShell` — the app-loop harness

- Boot: `preset::ui_demo()` + `replace_node_client(NodeClient::new(sim_base))`.
  No new preset; the fixture stays pure.
- `act(...)` — Simulator interaction via `by::role` (the existing selector
  authority), feeds resulting messages through `update()`, then pumps.
- `pump()` — the one new mechanism: each `Task<Message>` returned by `update()`
  is converted with `iced_runtime::task::into_stream` (present in the pinned
  iced_runtime 0.14.0) and run on a private tokio runtime; `Action::Output`
  messages feed back through `update()`; loop until quiescent. Hard deadline
  (~5s) that fails the test naming what was still pending. Non-output actions
  (window, widget, system) are ignored.
- `tick(msg)` — timer-driven refreshes are injected as explicit messages. No
  real timers, no subscriptions: deterministic by construction.
- Assertions: `has(role, name)` and `emitted` from `test/harness.rs`,
  `agent_wire::project_state` paths, and direct `Shell` fields (in-crate).

## Data flow of one test

spawn simnode(`--auto`) → boot shell with injected client → click/type via
Simulator → `update()` → `pump()` executes the submit HTTP against simnode,
receipt message returns, chained refresh Tasks fetch committed state →
assert the view/state renders it.

## Error handling

- Pump deadline fails with the pending-action inventory, not a hang.
- simnode child is killed on `Drop` by pid the harness itself spawned.
- Fresh storage dir per spawn is part of the determinism contract.

## Proof scenarios (the framework's own check)

1. **Chat round-trip:** create a channel and post a message through the UI;
   assert the committed message renders.
2. **Rejected submit:** a module rejection surfaces as UI feedback and corrupts
   no state.

Gates: `cargo clippy -p ducktape-iced --tests --no-deps`; the new suite runs
under `cargo test -p ducktape-iced` (with simnode prebuilt by the Make target).

## Cut on purpose

- Recipe `sim` lane (JSON schema + runner changes) — rides on this harness
  later if wanted.
- `/sim/step` held-mode and `peer_block` race tier — each is ~10 harness lines
  when a test first needs one; round-trip is the scope.
- Emulator spike; shared harness crate with `bin/simnode`.

## Known risk

A screen that refreshes only via the notifications ws push (which does not run
here) will need an explicit `tick()`/refresh message in its test. The first
such test tells us; it is a one-line harness affordance, not a redesign.
