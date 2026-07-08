# Async agent execution — design

Date: 2026-07-07. Status: approved for implementation (autonomous tailored call).

## Problem

Agent runs execute as external CLI subprocesses via
`DispatchWorker::run` → `CliProvider::run` (timeout up to 300s), awaited
**inline** in:

- bin/node: the drain arm of the main `select_biased!` loop
  (`bin/node/src/main.rs:8551-8554` — `for eff in node.take_effects() { …
  w.run(&eff).await }`), blocking block drain, RPC ingress, heartbeats;
- bin/noded: `offer_effects` inside the single serial command loop
  (`bin/noded/src/main.rs:550-578` via `submit_and_drain`), blocking all
  subsequent Query/Status/Submit commands.

Consequences: one run at a time per node; the desktop UI's `status()`
heartbeat starves during long runs and flips to the "reconnecting" banner.

## Decision: spawn execution off-loop; result returns as a normal submit

The oracle-as-op contract (`crates/kernel/reactor/src/lib.rs:85-98`,
dispatch's never-pop-stack mailbox) already treats the result as an op
arriving in a later block — execution timing is invisible to consensus.
**Zero consensus impact.**

Mechanics (per binary, mirroring existing background-lane precedents):

1. **Split the worker step**: lease gating + decode stay inline (fast,
   deterministic). The expensive `provider.run(...)` moves to a spawned
   background task.
   - The `Worker` trait is `async_trait(?Send)` — do NOT try to spawn the
     worker future itself. Restructure so the worker (or a wrapper around
     the offer step) extracts what it needs (capability, payload, saga
     reply skeleton) and hands the Send-able provider execution to the
     spawner, returning "handled, result later" immediately.
2. **bin/node**: spawn via the existing commonware
   `context.child(label).spawn(...)` lane pattern (precedents at
   main.rs:5209-5588). Completed runs send their `SagaMsg::OracleResult`
   op through an mpsc lane consumed by the select loop (mirror the
   `rpc_ingress` channel shape, main.rs:5715) and submitted through the
   normal submit path.
3. **bin/noded**: spawn on an async task; on completion inject the result
   as a Submit command into the existing command channel (`cmds`), exactly
   as the HTTP thread does. The command loop must no longer await provider
   execution inline.
4. **Concurrency policy**: allow N concurrent runs with a small semaphore
   cap (default 4), env-overridable as `DUCKTAPE_MAX_CONCURRENT_RUNS`
   (follow the `DUCKTAPE_PROVIDER_TIMEOUT_SECS` precedent in
   capability-host). Over-cap effects queue for the spawner (do not block
   the loop).
5. **In-flight dedup**: a redelivered `WorkerRequest` for a saga attempt
   already executing locally must be skipped (in-flight set keyed by
   saga id + attempt, pruned on completion). Preserve existing semantics:
   foreign-assignee skip, unassigned → Accept claim, error/timeout results
   still submitted so saga completes/retries. `kill_on_drop(true)` already
   guards child cleanup on shutdown.

## File layout (mono-file mandate)

Do not grow bin/node/src/main.rs or bin/noded/src/main.rs beyond minimal
wiring. New logic goes in new files, e.g. `bin/node/src/oracle_pool.rs` and
`bin/noded/src/oracle_pool.rs` (or one shared helper crate/module if the
shapes converge — implementer's call, ~600-line soft cap per file).

## Not in scope

- Changing dispatch/saga/runs consensus semantics, lease durations, or the
  provider CLI contract. No UI changes required (pending-runs timeline
  already exists; the fix makes status/queries responsive during runs).
- bin/node's `run_verb`/Tauri-side verbs (different mechanism, not agent
  runs).

## Testing

- Concurrency: two runs with slow providers execute overlapping (wall-clock
  assertion or in-flight counter), node still answers Status/Query while a
  run is in flight (this is the headline fix — assert it directly in a
  noded-level test: submit a slow run, then Status must return before the
  run completes).
- Dedup: redelivered WorkerRequest for an in-flight attempt does not spawn
  a second child (extend the exactly-once accept-race coverage pattern from
  bin/node/tests/dispatch_e2e.rs).
- Existing dispatch_e2e suite must stay green (it exercises
  announce→resolve→execute end-to-end).
- Failure path: provider error/timeout still produces the failure
  OracleResult and saga retry behavior unchanged.
