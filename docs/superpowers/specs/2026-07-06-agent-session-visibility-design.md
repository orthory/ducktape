# Agent-session visibility — design

**Date:** 2026-07-06
**Status:** approved (pending spec review)
**Topology assumption:** current flat model — one member key == one node == one machine. No multi-machine-per-member layer. "Node" and "member" are the same entity throughout.

## Problem

Today the console can't answer "agent A's session X is running on member Y", and it can't show what a member is capable of or which runs the local user requested. The raw ingredients all exist in the backend but are never joined up to the UI:

- The **capability registry** maps node key → announced executor tags, but `capability-client.ts` flattens it to a deduped tag list and discards the node key.
- The **saga `assignee`** (the node key currently holding a run's execution lease — `crates/system/saga/src/interface.rs:253`) is the "running on Y" signal, but it is never surfaced above the saga layer.
- `PendingRun.requester` (`crates/apps/runs/src/interface.rs`) is populated but unused in the UI.

## Goals

Four features, on the current topology:

| # | Feature | Surface |
|---|---------|---------|
| 1 | Show each member's announced capabilities (executor tags) | `MembersView` — chips per row |
| 2 | Mark/filter the runs the local user requested | `AgentView` Activity tab |
| 3 | Show which node is executing each in-flight run | `AgentView` `RunRow` — node badge |

Non-goals (YAGNI):
- No placement for terminal/completed runs — "running on" is inherently about in-flight work, and terminal runs leave `state.pendingRuns`.
- No persisted assignee history and no new saga hook/event.
- No presence/liveness on members (stays "not exposed by this node").
- No multi-machine-per-member model (explicitly dropped).

## Approach — placement read path (feature 3)

The saga `assignee` is pull-only (saga emits no hook; its only outbound message is the terminal callback). The `saga_id` is **deterministic**: `saga_id = "dispatch\x1f{receiver}\x1f{dispatch_id}"`, and for every run `receiver == "runs"`, so `saga_id == "dispatch\x1fruns\x1f{PendingRun.dispatch_id}"` (derivation: `crates/system/dispatch/src/lib.rs:57-65`; `dispatch_id_for` in `crates/apps/runs/src/lib.rs:204-206`). `SagaQuery::Get{saga_id} → SagaReply::Saga(SagaView{ assignee })` already exists end-to-end (`crates/system/saga/src/interface.rs:224-227, 253`; handler `crates/system/saga/src/lib.rs:1005-1007`).

**Chosen: (B) — a view-only `query_with` facade on the dispatch module.**

Dispatch currently implements the context-free `Module::query` (`crates/system/dispatch/src/lib.rs:896`), which cannot cross-query siblings. The host routes all external/frontend queries through `Module::query_with(ctx, req)` with a read-capable `Ctx` (`crates/kernel/host/src/lib.rs:337-360`). Converting dispatch to override `query_with` — mirroring the existing `upgrade` module facade (`crates/system/upgrade/src/lib.rs:423-426`) — lets it read the saga assignee and expose it as a **view-only** field on `DispatchView`. No persisted state, no hook, **no app-hash impact**.

Alternatives considered and rejected:
- **(A) saga→runs hook baking `assignee` into persisted `PendingRun`** — requires a new `SagaEvent` enum + hook field on the consensus-critical saga module, emission on every assignee change (assignee churns per attempt), and a flag-day encoding change to persisted `PendingState`. Heaviest, largest blast radius.
- **(C) frontend saga-client that hardcodes the `"dispatch\x1fruns\x1f…"` key format** — zero Rust, but leaks a consensus-internal key encoding into TS; breaks silently if the dispatch key format changes.

### Key-encoding invariants (relied upon)

- Member/observer keys and capability provider node-keys and saga assignee keys are all the **same raw ed25519 key bytes**, rendered as lowercase unprefixed hex on the frontend (`keyHex`). They join directly with no conversion. Join helpers `normalizeKey`/`sameKey` already exist (`app/src/domain/names.ts:6-12`).
- `PendingRun.requester` is a `SagaOrigin`. On a **networked consensus node** the signed origin is the submitter's own pubkey, so `hexOf(run.requester.external) === state.workspace.pubkey === that node's member key`; the "requested by me" join works. On the **local single-node daemon** the requester is UTF-8 of the author string and there is no valset/workspace — the marker is degenerate there and simply won't match, which is acceptable.
- `DispatchView` only carries `saga_id` (inside `status = AwaitingResult`) while the run is in flight — exactly the window a "running on" badge cares about. Terminal dispatches have no `saga_id` and resolve `assignee = None`.

## Components

### Backend (Rust) — `crates/system/dispatch/`

1. `interface.rs`: add view-only `assignee: Option<Vec<u8>>` to `DispatchView`.
2. `lib.rs`: replace the plain `Module::query` with a `query_with(ctx, req)` override. For the `Dispatch{ receiver, dispatch_id }` query, when the resolved status is `AwaitingResult{ saga_id }`, cross-query `SagaQuery::Get{ saga_id }` and copy `SagaView.assignee` into the returned view; otherwise `assignee = None`. All other query variants unchanged. No persisted state added.

### Frontend — `app/src/domain/`

3. `capability-client.ts`: add `capabilitiesByNode(): Promise<Map<string, string[]>>` — reuse the identical `transport.query("capability", "all")` call (the reply is already `Vec<(node_key, tags)>`), returning `keyHex(node) → tags` instead of discarding the key. Keep the existing flattened `capabilities()` for the "Runs on" picker.
4. new `dispatch-client.ts` (mirrors `runs-client.ts`): typed `DispatchView` including hex-encoded `assignee`, plus a `dispatch(dispatchId, receiver = "runs")` query method (`receiver` is always `"runs"` for run placement, but kept as a parameter to match `DispatchQuery::Dispatch{ receiver, dispatch_id }`).

### Frontend store — `app/src/console/store/`

5. `state.ts`: add `capabilitiesByNode: Map<string, string[]>` and `runAssignee: Map<string, string>` (runId → hex node key).
6. `DucktapeProvider.tsx` `refresh()`:
   - fetch `capabilitiesByNode` (best-effort, swallow to empty like the existing `capabilities()` call).
   - for each in-flight `PendingRun`, issue one `dispatch.dispatch(dispatchId)` query and build `runAssignee`. In-flight runs only, so N is small; fan-out is bounded by `state.pendingRuns.length`.

### Frontend views

7. `MembersView.tsx`:
   - Capability chips per member row: join `member.keyNorm` → `capabilitiesByNode`. Empty → render nothing (degrades cleanly on bare nodes).
8. `AgentView.tsx` `RunRow`:
   - Node badge: `runAssignee.get(run.run_id)` → display name via `state.authorNames` (fallback short key).
   - "you" chip when `run.requester` is `{external}` and `sameKey(hexOf(run.requester.external), state.workspace.pubkey)`.
   - Activity tab: a lightweight **Mine / All** toggle filtering `RunsTimeline` by the same requester predicate.

## Data flow

```
refresh():
  valset.validators/observers ──► state.members / state.observers
  capability.all ─────────────► state.capabilitiesByNode   (node → tags)
  runs.pendingRuns ───────────► state.pendingRuns
       └─ per in-flight run ─► dispatch.dispatch(dispatch_id) ─► assignee
                                                   └─► state.runAssignee (runId → node)

MembersView: member.key ⋈ capabilitiesByNode   → capability chips
RunRow:      run.run_id ⋈ runAssignee            → node badge
             run.requester == workspace.pubkey   → "you" chip / Mine filter
```

## Testing

**Rust (`crates/system/dispatch/`):**
- Unit: a dispatch in `AwaitingResult` returns the saga's `assignee` in its view; a terminal/absent dispatch returns `assignee = None`.
- Extend `bin/node/tests/dispatch_e2e.rs` (`mention_routes_to_the_announced_provider_across_nodes`, ~:351): after the run routes, assert the dispatch view reports the announced provider node as `assignee`.

**Frontend:**
- `capability-client`: `capabilitiesByNode` maps `(node, tags)` pairs to hex-keyed entries and preserves per-node grouping.
- `dispatch-client`: decodes `assignee` bytes → hex; `None` stays undefined.
- Join helpers: member ⋈ capabilities, run ⋈ assignee (reverse), requester == me — including the `module`/`system` requester variants (no key → never "mine").

## Build order

1. Backend dispatch `query_with` + `DispatchView.assignee` (+ Rust tests).
2. `capabilities-by-node` client method (+ test).
3. `dispatch-client` (+ test).
4. Store fields + `refresh()` wiring.
5. `MembersView` capability chips.
6. `RunRow` node badge + "you" chip + Mine/All toggle.
