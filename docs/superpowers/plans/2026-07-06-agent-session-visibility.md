# Agent-session visibility Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show each member's announced executor capabilities, mark/filter the runs the local user requested, and show which node is executing each in-flight run.

**Architecture:** Read-path only. A view-only `query_with` facade on the `dispatch` module cross-queries the `saga` module and surfaces the run's lease holder (`assignee`) on `DispatchView` — no persisted state, no app-hash change, no saga hook. The frontend gains a `capabilitiesByNode` client method and a new `dispatch-client`, joins them in the store, and renders capability chips on members plus a node badge / "you" chip / Mine-All filter on runs.

**Tech Stack:** Rust (consensus modules, `async-trait`, `serde_json`), TypeScript/React (Vite, Vitest), the node's generic query transport.

## Global Constraints

- **Topology is flat:** one member key == one node == one machine. "Node" and "member" are the same entity. No multi-machine layer.
- **Placement is view-only:** `assignee` is never persisted, never part of any module's `root()`/app-hash. It is resolved at query time from the saga.
- **In-flight only:** `assignee` is populated only when the dispatch is `AwaitingResult`; a delivered/terminal run reports `assignee = None`.
- **Key encoding:** all keys (members, capability providers, saga assignee, requester) are the same raw ed25519 bytes, rendered lowercase unprefixed hex via `keyHex` / `hexOf`. Join with `normalizeKey`/`sameKey` (`app/src/domain/names.ts`).
- **No backwards compatibility:** flag-day changes are fine (fresh genesis). Query-reply shape changes go backend + frontend in lockstep.
- **Commands:** backend tests `cargo test -p dispatch <name>`; frontend tests `cd app && npx vitest run <file>`; frontend typecheck `cd app && npm run typecheck`.

---

### Task 1: Backend — surface the saga assignee on `DispatchView`

**Files:**
- Modify: `crates/system/dispatch/src/interface.rs` (add `assignee` to `DispatchView`)
- Modify: `crates/system/dispatch/src/lib.rs` (imports, `Self::view` default, `saga_assignee` helper, `query_with` override, tests)
- Test: `crates/system/dispatch/src/lib.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `saga::{SagaQuery, SagaReply, SagaView, encode_query, decode_reply}`; `Ctx::query(target, req)` for the host-routed sibling read (pattern: `crates/system/upgrade/src/lib.rs:116-129`).
- Produces: `DispatchView.assignee: Option<Vec<u8>>` — the node key currently holding the run's saga lease, present only while `AwaitingResult`. Consumed by the frontend `dispatch-client` in Task 3.

- [ ] **Step 1: Add the view field**

In `crates/system/dispatch/src/interface.rs`, add `assignee` to `DispatchView` (after `outcome`):

```rust
/// a dispatch's observable state.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DispatchView {
    pub dispatch_id: String,
    pub recipe_id: String,
    /// the module the result event is delivered to — always the dispatching
    /// module (`Dispatch` is module-origin-only).
    pub receiver: String,
    pub status: DispatchStatus,
    /// the contract-checked outcome, present from `AwaitingDelivery` on.
    /// `Err` carries the saga failure or the contract violation.
    pub outcome: Option<Result<Vec<u8>, String>>,
    /// the node key currently holding the run's execution lease (the saga
    /// assignee), resolved at QUERY TIME by the read facade. `None` unless the
    /// dispatch is `AwaitingResult` — a delivered run runs nowhere. VIEW-ONLY:
    /// never committed state, never part of the app-hash.
    #[serde(default)]
    pub assignee: Option<Vec<u8>>,
    pub created_at: u64,
    pub updated_at: u64,
}
```

- [ ] **Step 2: Default `assignee` in `Self::view` and confirm it compiles**

In `crates/system/dispatch/src/lib.rs`, in `fn view` (around line 827), add `assignee: None`:

```rust
    fn view(d: &DispatchState) -> DispatchView {
        DispatchView {
            dispatch_id: d.dispatch_id.clone(),
            recipe_id: d.recipe_id.clone(),
            receiver: d.receiver.clone(),
            status: match d.status {
                Status::AwaitingResult => DispatchStatus::AwaitingResult {
                    saga_id: d.saga_id.clone(),
                },
                Status::AwaitingDelivery => DispatchStatus::AwaitingDelivery,
                Status::Delivered => DispatchStatus::Delivered,
            },
            outcome: d.outcome.clone(),
            assignee: None,
            created_at: d.created_at,
            updated_at: d.updated_at,
        }
    }
```

Run: `cargo build -p dispatch`
Expected: compiles (all `DispatchView` constructors are through `Self::view`).

- [ ] **Step 3: Write the failing test (assignee surfaced while awaiting result)**

In `crates/system/dispatch/src/lib.rs`, in `mod tests`, extend `CaptureCtx` to optionally answer a saga `Get`, and add a saga-reply builder + the test. First, change the struct and its `new`/`Ctx::query` (around lines 954-997):

```rust
    struct CaptureCtx {
        env: Env,
        msgs: Vec<Msg>,
        saga_reply: Option<Vec<u8>>,
    }
    impl CaptureCtx {
        fn new() -> Self {
            Self {
                env: Env {
                    height: 0,
                    consensus_time: 0,
                    origin: Origin::System,
                    me: "dispatch".into(),
                    protocol_version: 0,
                },
                msgs: Vec::new(),
                saga_reply: None,
            }
        }
        fn at(mut self, height: u64) -> Self {
            self.env.height = height;
            self.env.consensus_time = height;
            self
        }
        fn from_origin(mut self, origin: Origin) -> Self {
            self.env.origin = origin;
            self
        }
        /// canned answer for a `saga` `Get` — how a query_with test stands in
        /// for the real sibling module.
        fn with_saga_reply(mut self, reply: Vec<u8>) -> Self {
            self.saga_reply = Some(reply);
            self
        }
    }
```

And the `Ctx::query` impl (around line 989):

```rust
        async fn query(&self, target: &str, _req: &[u8]) -> Result<Vec<u8>, Error> {
            match &self.saga_reply {
                Some(bytes) if target == "saga" => Ok(bytes.clone()),
                _ => Err(Error::QueryUnsupported),
            }
        }
```

Then add a saga-reply builder and the test at the end of `mod tests` (before its closing `}`):

```rust
    /// a `saga` `Get` reply carrying a Pending saga with the given assignee.
    fn saga_reply_with_assignee(assignee: Option<Vec<u8>>) -> Vec<u8> {
        use saga::{SagaStatus, SagaView, encode_reply as saga_encode_reply};
        saga_encode_reply(&SagaReply::Saga(Some(SagaView {
            origin: SagaOrigin::Module("dispatch".into()),
            reply_to: Some("dispatch".into()),
            reply_payload: Vec::new(),
            spec: Vec::new(),
            capability: Some("alpha".into()),
            status: SagaStatus::Pending,
            attempt: 0,
            max_attempts: 2,
            assignee,
            pinned_assignee: None,
            lease_views: None,
            lease_expires_at: None,
            deadline: None,
            result: None,
            error: None,
            created_at: 0,
            updated_at: 0,
        })))
    }

    fn query_dispatch(m: &DispatchModule, ctx: &CaptureCtx, key: &str) -> Option<DispatchView> {
        let (receiver, dispatch_id) = key.split_once(SEP).expect("composite key");
        let reply = block_on(m.query_with(
            ctx,
            &crate::encode_query(&DispatchQuery::Dispatch {
                receiver: receiver.into(),
                dispatch_id: dispatch_id.into(),
            }),
        ))
        .unwrap();
        match crate::decode_reply(&reply).unwrap() {
            DispatchReply::Dispatch(v) => v,
            other => panic!("expected Dispatch reply, got {other:?}"),
        }
    }

    #[test]
    fn query_with_surfaces_the_saga_assignee_while_awaiting_result() {
        let mut m = module();
        let key = registered_and_dispatched(&mut m, OutputContract::Text);
        let ctx = CaptureCtx::new()
            .with_saga_reply(saga_reply_with_assignee(Some(b"worker-key".to_vec())));
        let view = query_dispatch(&m, &ctx, &key).expect("a dispatch view");
        assert_eq!(view.assignee, Some(b"worker-key".to_vec()));
    }
```

Note: `SagaReply` and `SagaOrigin` are already imported at module scope (`SagaOrigin` via the top `use saga::{...}`); `SagaReply` becomes available after Step 5's import edit — so run this test only after Step 5. To keep TDD honest, temporarily add `use saga::SagaReply;` inside the test fn if you run it before Step 5.

- [ ] **Step 4: Run the test to verify it fails**

Run: `cargo test -p dispatch query_with_surfaces_the_saga_assignee_while_awaiting_result`
Expected: FAIL — `query_with` is not yet overridden, so it falls back to the committed `query` which reports `assignee: None` (asserts `Some != None`).

- [ ] **Step 5: Add the saga imports and implement `query_with` + `saga_assignee`**

In `crates/system/dispatch/src/lib.rs`, widen the saga import (around line 44):

```rust
use saga::{
    SagaCallback, SagaMsg, SagaOrigin, SagaOutcome, SagaQuery, SagaReply, decode_callback,
    decode_reply as saga_decode_reply, encode_msg as saga_encode_msg,
    encode_query as saga_encode_query,
};
```

Add the helper next to `fn view` (in the inherent `impl DispatchModule` block, right after `view`):

```rust
    /// the saga's current lease holder, read through the host-routed sibling
    /// lane — the filtered-facade pattern (cf. `upgrade::members`). read-only:
    /// this never stages, so it is safe inside `query_with`.
    async fn saga_assignee(
        &self,
        ctx: &dyn Ctx,
        saga_id: &str,
    ) -> Result<Option<Vec<u8>>, Error> {
        let reply = ctx
            .query(
                &self.saga,
                &saga_encode_query(&SagaQuery::Get {
                    saga_id: saga_id.to_string(),
                }),
            )
            .await?;
        match saga_decode_reply(&reply).map_err(Error::Module)? {
            SagaReply::Saga(saga) => Ok(saga.and_then(|v| v.assignee)),
            other => Err(Error::Module(format!("saga answered Get with {other:?}"))),
        }
    }
```

Add the `query_with` override in `impl Module for DispatchModule`, immediately after `async fn query` (around line 920):

```rust
    async fn query_with(&self, ctx: &dyn Ctx, req: &[u8]) -> Result<Vec<u8>, Error> {
        // only the single-dispatch read is enriched with the live assignee;
        // every other variant (including the host's committed-only
        // PendingDeliveries injection read) stays on the plain `query` path.
        match decode_query(req).map_err(Error::Module)? {
            DispatchQuery::Dispatch {
                receiver,
                dispatch_id,
            } => {
                let key = dispatch_key(&receiver, &dispatch_id);
                let Some(d) = self.committed.dispatches.get(&key) else {
                    return Ok(encode_reply(&DispatchReply::Dispatch(None)));
                };
                let mut view = Self::view(d);
                // "running on" is only meaningful while the saga is live; a
                // delivered run has left every node.
                if matches!(d.status, Status::AwaitingResult) {
                    view.assignee = self.saga_assignee(ctx, &d.saga_id).await?;
                }
                Ok(encode_reply(&DispatchReply::Dispatch(Some(view))))
            }
            _ => self.query(req).await,
        }
    }
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p dispatch query_with_surfaces_the_saga_assignee_while_awaiting_result`
Expected: PASS.

- [ ] **Step 7: Write and run the terminal-case test (assignee omitted once delivered)**

Add to `mod tests`:

```rust
    #[test]
    fn query_with_omits_the_assignee_once_the_run_is_terminal() {
        let mut m = module();
        let key = registered_and_dispatched(&mut m, OutputContract::Text);
        // a Done callback moves the dispatch off AwaitingResult.
        callback_for(&mut m, &key, SagaOutcome::Done(b"quack".to_vec())).unwrap();
        commit(&mut m);
        // even though the saga would still answer with an assignee, a
        // non-AwaitingResult dispatch never calls saga and reports None.
        let ctx = CaptureCtx::new()
            .with_saga_reply(saga_reply_with_assignee(Some(b"worker-key".to_vec())));
        let view = query_dispatch(&m, &ctx, &key).expect("a dispatch view");
        assert_eq!(view.assignee, None);
    }
```

Run: `cargo test -p dispatch query_with_`
Expected: both `query_with_*` tests PASS.

- [ ] **Step 8: Full module test + commit**

Run: `cargo test -p dispatch`
Expected: PASS (no regressions in existing dispatch tests).

```bash
git add crates/system/dispatch/src/interface.rs crates/system/dispatch/src/lib.rs
git commit -m "feat(dispatch): surface the saga assignee on DispatchView via a read facade"
```

---

### Task 2: Frontend — `capabilitiesByNode` client method

**Files:**
- Modify: `app/src/domain/capability-client.ts` (add `capabilitiesByNode`)
- Test: `app/src/domain/capability-client.test.ts`

**Interfaces:**
- Consumes: `transport.query("capability", "all")` → `RegistryEntry[]` (`[number[], string[]]`); `keyHex` from `./chat-client`.
- Produces: `capabilitiesByNode(transport): Promise<Map<string, string[]>>` — hex node key → announced tags. Consumed by the store (Task 4) and `MembersView` (Task 5).

- [ ] **Step 1: Write the failing test**

In `app/src/domain/capability-client.test.ts`, change the import and add a `describe`:

```ts
import { capabilities, capabilitiesByNode } from "./capability-client";
```

```ts
describe("capabilitiesByNode", () => {
  it("keeps the node key: hex(node) -> its announced tags", async () => {
    const transport = stubTransport({
      all: [
        [[1, 2], ["codex", "claude"]],
        [[3, 4], ["ollama"]],
      ],
    });
    const map = await capabilitiesByNode(transport);
    expect(map.get("0102")).toEqual(["codex", "claude"]);
    expect(map.get("0304")).toEqual(["ollama"]);
    expect(transport.query).toHaveBeenCalledWith("capability", "all");
  });

  it("reads an empty registry as an empty map", async () => {
    const transport = stubTransport({ all: [] });
    expect((await capabilitiesByNode(transport)).size).toBe(0);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd app && npx vitest run src/domain/capability-client.test.ts`
Expected: FAIL — `capabilitiesByNode` is not exported.

- [ ] **Step 3: Implement `capabilitiesByNode`**

In `app/src/domain/capability-client.ts`, add the `keyHex` import and the function:

```ts
import { keyHex } from "./chat-client";
```

```ts
/** The registry as a per-node map: hex node key -> the executor tags that node
 *  announced. Same `All` query as `capabilities`, but keeps the node key so a
 *  member row can show what THAT node runs. Empty map when nothing is
 *  announced. */
export const capabilitiesByNode = (
  transport: NodeTransport,
): Promise<Map<string, string[]>> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, "all"))
    .then((reply) => replyVariant<RegistryEntry[]>(reply, "all"))
    .then((entries) => new Map(entries.map(([key, tags]) => [keyHex(key), tags])));
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd app && npx vitest run src/domain/capability-client.test.ts`
Expected: PASS (all capability-client tests).

- [ ] **Step 5: Commit**

```bash
git add app/src/domain/capability-client.ts app/src/domain/capability-client.test.ts
git commit -m "feat(app): add capabilitiesByNode client (per-node executor tags)"
```

---

### Task 3: Frontend — `dispatch-client`

**Files:**
- Create: `app/src/domain/dispatch-client.ts`
- Test: `app/src/domain/dispatch-client.test.ts`

**Interfaces:**
- Consumes: `transport.query("dispatch", { dispatch: { receiver, dispatch_id } })` → `DispatchView | null` (the `dispatch` reply variant); `keyHex` from `./chat-client`.
- Produces: `dispatch(transport, { dispatchId, receiver? }): Promise<DispatchView | null>` and `assigneeHex(view): string | null`. Consumed by the store (Task 4).

- [ ] **Step 1: Write the failing test**

Create `app/src/domain/dispatch-client.test.ts`:

```ts
// The dispatch client mirrors the dispatch module's single-dispatch read. The
// only field this feature consumes is `assignee` (the saga lease holder), so
// the tests pin the query address and the hex projection.

import { describe, expect, it, vi } from "vitest";

import { assigneeHex, dispatch, type DispatchView } from "./dispatch-client";
import type { NodeTransport } from "./transport";

const stubTransport = (reply?: unknown): NodeTransport => ({
  submit: vi.fn().mockResolvedValue({ height: 1, appHash: "aa".repeat(32) }),
  query: vi.fn().mockResolvedValue(reply),
  view: vi.fn(),
  putBlob: vi.fn().mockResolvedValue("ab".repeat(32)),
  getBlob: vi.fn().mockResolvedValue(new Uint8Array()),
  status: vi.fn(),
  metrics: vi.fn(),
  blocks: vi.fn(),
  onBlock: vi.fn(),
});

describe("dispatch", () => {
  it("addresses the dispatch under receiver 'runs' by default", async () => {
    const view = { dispatch_id: "d1", assignee: [1, 2] };
    const transport = stubTransport({ dispatch: view });
    await expect(dispatch(transport, { dispatchId: "d1" })).resolves.toEqual(view);
    expect(transport.query).toHaveBeenCalledWith("dispatch", {
      dispatch: { receiver: "runs", dispatch_id: "d1" },
    });
  });

  it("returns null when the dispatch is unknown", async () => {
    const transport = stubTransport({ dispatch: null });
    await expect(dispatch(transport, { dispatchId: "gone" })).resolves.toBeNull();
  });
});

describe("assigneeHex", () => {
  it("hex-encodes the assignee bytes", () => {
    expect(assigneeHex({ assignee: [1, 2] } as DispatchView)).toBe("0102");
  });

  it("is null with no assignee or no view", () => {
    expect(assigneeHex({ assignee: null } as DispatchView)).toBeNull();
    expect(assigneeHex(null)).toBeNull();
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd app && npx vitest run src/domain/dispatch-client.test.ts`
Expected: FAIL — the module does not exist.

- [ ] **Step 3: Implement the client**

Create `app/src/domain/dispatch-client.ts`:

```ts
// Typed client for the node's `dispatch` module read surface — the
// single-dispatch view, which the console joins to a PendingRun (by
// `dispatch_id`) to show WHICH node is executing a run. `assignee` is the
// saga's lease holder, resolved at query time by the dispatch read facade;
// it is present only while the run is in flight (`awaiting_result`) and null
// once the result has delivered. Pure functions over an injected NodeTransport.

import { keyHex } from "./chat-client";
import type { NodeTransport } from "./transport";
import { replyVariant } from "./wire";

const TARGET = "dispatch";

/** Where a dispatch is in its lifecycle (mirrors `DispatchStatus`). */
export type DispatchStatus =
  | { awaiting_result: { saga_id: string } }
  | "awaiting_delivery"
  | "delivered";

/** One dispatch's observable state (mirrors `DispatchView`). `assignee` is the
 *  node key (raw bytes) holding the saga lease — present only while
 *  `awaiting_result`, null otherwise. `outcome` is unused here (left `unknown`
 *  to avoid pinning the Result wire shape). */
export interface DispatchView {
  dispatch_id: string;
  recipe_id: string;
  receiver: string;
  status: DispatchStatus;
  outcome: unknown;
  assignee: number[] | null;
  created_at: number;
  updated_at: number;
}

/** One dispatch, addressed as its receiver knows it. `receiver` is "runs" for
 *  every agent run. Null when the dispatch is unknown (already pruned). */
export const dispatch = (
  transport: NodeTransport,
  params: { dispatchId: string; receiver?: string },
): Promise<DispatchView | null> =>
  Promise.resolve()
    .then(() =>
      transport.query(TARGET, {
        dispatch: {
          receiver: params.receiver ?? "runs",
          dispatch_id: params.dispatchId,
        },
      }),
    )
    .then((reply) => replyVariant<DispatchView | null>(reply, "dispatch"));

/** The run's current executor node as a hex key, or null when it isn't
 *  in-flight/assigned. The join value for `state.runAssignee`. */
export const assigneeHex = (view: DispatchView | null): string | null =>
  view?.assignee ? keyHex(view.assignee) : null;
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd app && npx vitest run src/domain/dispatch-client.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add app/src/domain/dispatch-client.ts app/src/domain/dispatch-client.test.ts
git commit -m "feat(app): add dispatch-client (single-dispatch view + assignee hex)"
```

---

### Task 4: Frontend — store fields + `refresh` wiring

**Files:**
- Modify: `app/src/console/store/state.ts` (two fields, defaults, snapshot, projection)
- Modify: `app/src/console/store/DucktapeProvider.tsx` (fan out capability + per-run dispatch reads)

**Interfaces:**
- Consumes: `capabilityClient.capabilitiesByNode`, `dispatchClient.dispatch`, `dispatchClient.assigneeHex` (Tasks 2, 3).
- Produces: `state.capabilitiesByNode: Map<string, string[]>` and `state.runAssignee: Map<string, string>` (run_id → hex node key). Consumed by `MembersView` (Task 5) and `AgentView` (Task 6).

- [ ] **Step 1: Add the two state fields, defaults, snapshot, and projection**

In `app/src/console/store/state.ts`, in the `ConsoleState` interface, right after `pendingRuns: PendingRun[];` (line 115):

```ts
  /** hex node key -> the executor tags that node announced (the `capability`
   *  registry, kept per-node instead of flattened). Members view shows what
   *  each member runs; empty when nothing is announced. */
  capabilitiesByNode: Map<string, string[]>;
  /** run_id -> hex node key currently executing it (the saga assignee, via the
   *  dispatch read facade). Only in-flight runs appear; empty otherwise. */
  runAssignee: Map<string, string>;
```

In the initial-state object (after `pendingRuns: [],` at line 263):

```ts
    capabilitiesByNode: new Map(),
    runAssignee: new Map(),
```

In the `ConsoleSnapshot` interface (after `pendingRuns: PendingRun[];` at line 299):

```ts
  capabilitiesByNode: Map<string, string[]>;
  runAssignee: Map<string, string>;
```

In `applySnapshot` (the returned object starting line 306), add — place next to the other agent-slice fields (e.g. after the `pendingRuns:` line):

```ts
  capabilitiesByNode: snapshot.capabilitiesByNode,
  runAssignee: snapshot.runAssignee,
```

- [ ] **Step 2: Wire the reads into `refresh`**

In `app/src/console/store/DucktapeProvider.tsx`, add the client import after line 18:

```ts
import * as dispatchClient from "../../domain/dispatch-client";
```

Add `capabilitiesByNode` to the `Promise.all` fan-out — insert immediately after the `capabilityClient.capabilities(...)` entry (line 119):

```ts
          capabilityClient.capabilities(live).catch((): string[] => []),
          // the same registry, kept per-node so a member row can show what it
          // runs — best-effort like everything else in the snapshot.
          capabilityClient
            .capabilitiesByNode(live)
            .catch((): Map<string, string[]> => new Map()),
```

Add `capabilitiesByNode` to the destructured results array (after `capabilities,` at line 145):

```ts
        capabilities,
        capabilitiesByNode,
```

Replace the inner `messages` chain (lines 164-190) — the block starting `return Promise.resolve()` through the `applySnapshot({...})` `dispatch` call — with a version that also resolves `runAssignee`:

```ts
        return Promise.all([
          active ? chatClient.latestMessages(live, active) : [],
          // one dispatch read per in-flight run → its executor node. bounded by
          // pendingRuns.length; each is best-effort so one miss never fails the
          // refresh.
          Promise.all(
            pendingRuns.map((run) =>
              dispatchClient
                .dispatch(live, { dispatchId: run.dispatch_id })
                .then(
                  (view) =>
                    [run.run_id, dispatchClient.assigneeHex(view)] as const,
                )
                .catch(() => [run.run_id, null] as const),
            ),
          ),
        ]).then(([messages, assigneePairs]) => {
          const runAssignee = new Map<string, string>();
          for (const [runId, hex] of assigneePairs) if (hex) runAssignee.set(runId, hex);
          return dispatch({
            type: "patch",
            patch: applySnapshot({
              connected: true,
              status,
              channels,
              members,
              observers,
              proposals,
              forgeHead,
              activeChannel: active,
              messages,
              authorNames,
              pages,
              activePageBlocks: pageBlocks ?? [],
              agents,
              capabilities,
              capabilitiesByNode,
              watches,
              pendingRuns,
              runAssignee,
              files,
              blocks,
            }),
          });
        });
```

- [ ] **Step 3: Typecheck**

Run: `cd app && npm run typecheck`
Expected: no errors. (`applySnapshot` requires all `ConsoleSnapshot` keys; the two new ones are now supplied.)

- [ ] **Step 4: Run the full frontend test suite (no regressions)**

Run: `cd app && npm test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add app/src/console/store/state.ts app/src/console/store/DucktapeProvider.tsx
git commit -m "feat(app): thread capabilitiesByNode + runAssignee through the store"
```

---

### Task 5: Frontend — capability chips on Members

**Files:**
- Modify: `app/src/console/views/members/MembersView.tsx` (`MemberVM`, `makeMembers` signature + export, list chips, detail row)
- Test: `app/src/console/views/members/MembersView.test.tsx` (new)

**Interfaces:**
- Consumes: `state.capabilitiesByNode` (Task 4); `MemberVM.keyNorm` for the join.
- Produces: `export function makeMembers(members, observers, authorNames, workspace, capabilitiesByNode)` and `MemberVM.capabilities: string[]`.

- [ ] **Step 1: Write the failing test**

Create `app/src/console/views/members/MembersView.test.tsx`:

```tsx
import { describe, expect, it } from "vitest";

import { makeMembers } from "./MembersView";

describe("makeMembers capabilities", () => {
  it("attaches each node's announced tags by normalized key", () => {
    const caps = new Map([["ab".repeat(32), ["codex", "claude"]]]);
    const [row] = makeMembers([`0x${"AB".repeat(32)}`], [], {}, null, caps);
    expect(row.capabilities).toEqual(["codex", "claude"]);
  });

  it("defaults to no capabilities when the node announced nothing", () => {
    const [row] = makeMembers(["cd".repeat(32)], [], {}, null, new Map());
    expect(row.capabilities).toEqual([]);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd app && npx vitest run src/console/views/members/MembersView.test.tsx`
Expected: FAIL — `makeMembers` is not exported / takes no `capabilitiesByNode` arg.

(If importing `MembersView.tsx` throws under the node test env, extract `MemberVM` + `makeMembers` into a new pure `app/src/console/views/members/members-model.ts`, import it back into `MembersView.tsx`, and point the test there. Prefer the in-place export first.)

- [ ] **Step 3: Add `capabilities` to `MemberVM` and thread it through `makeMembers`**

In `app/src/console/views/members/MembersView.tsx`, add to the `MemberVM` interface (after `searchText: string;`):

```ts
  /** Executor tags this node announced to the capability registry. */
  capabilities: string[];
```

Change the `makeMembers` signature to `export` and take the map (line 112):

```ts
export function makeMembers(
  members: string[],
  observers: string[],
  authorNames: Record<string, string>,
  workspace: { pubkey: string; founder: boolean; member: boolean } | null,
  capabilitiesByNode: Map<string, string[]>,
): MemberVM[] {
```

Inside `toVM`, set `capabilities` in the returned object (after `searchText: ...`):

```ts
      searchText: `${displayName} ${key} ${role}`.toLowerCase(),
      capabilities: capabilitiesByNode.get(keyNorm) ?? [],
    };
```

- [ ] **Step 4: Pass the map from the view and run the test**

In the `MembersView` component, update the `rows` memo (line 971) to pass the map and add the dep:

```ts
  const rows = useMemo(
    () =>
      makeMembers(
        state.members,
        state.observers,
        state.authorNames,
        state.workspace,
        state.capabilitiesByNode,
      ),
    [
      state.authorNames,
      state.capabilitiesByNode,
      state.members,
      state.observers,
      state.workspace,
    ],
  );
```

Run: `cd app && npx vitest run src/console/views/members/MembersView.test.tsx`
Expected: PASS.

- [ ] **Step 5: Render chips in the list row**

In `MemberRow`, immediately after the meta `<div>` that renders `{member.shortKey} · {member.status}` (closes at line 466), add a chips block:

```tsx
          <div
            title={member.key}
            style={{
              marginTop: 3,
              font: `400 10.5px ${font.mono}`,
              color: color.muted2,
              whiteSpace: "nowrap",
              overflow: "hidden",
              textOverflow: "ellipsis",
            }}
          >
            {member.shortKey} · {member.status}
          </div>
          {member.capabilities.length > 0 && (
            <div style={{ marginTop: 5, display: "flex", flexWrap: "wrap", gap: 4 }}>
              {member.capabilities.map((tag) => (
                <span
                  key={tag}
                  style={{
                    padding: "1px 6px",
                    borderRadius: 4,
                    background: color.paper,
                    border: `1px solid ${color.borderStrong}`,
                    font: `500 9.5px ${font.mono}`,
                    color: color.muted2,
                  }}
                >
                  {tag}
                </span>
              ))}
            </div>
          )}
```

- [ ] **Step 6: Add a detail-panel row**

In the detail panel, after the `presence` `InfoRow` (line 958), add:

```tsx
          <InfoRow label="presence" value="not exposed by this node" />
          <InfoRow
            label="capabilities"
            value={
              member.capabilities.length
                ? member.capabilities.join(", ")
                : "none announced"
            }
          />
```

- [ ] **Step 7: Typecheck + commit**

Run: `cd app && npm run typecheck`
Expected: no errors.

```bash
git add app/src/console/views/members/MembersView.tsx app/src/console/views/members/MembersView.test.tsx
git commit -m "feat(app): show each member's announced capabilities on Members"
```

---

### Task 6: Frontend — run node badge, "you" chip, Mine/All filter

**Files:**
- Modify: `app/src/console/views/agent/AgentView.tsx` (`runIsMine` helper + export, `RunRow` props/badge, `RunsTimeline` props, Activity tab filter + toggle)
- Test: `app/src/console/views/agent/AgentView.runmine.test.ts` (new)

**Interfaces:**
- Consumes: `state.runAssignee`, `state.authorNames`, `state.workspace?.pubkey` (Task 4); `PendingRun.requester` (`SagaOrigin`); `hexOf` (local, line 128); `displayNameForKey`, `shortKey`, `sameKey` from `../../../domain/names`.
- Produces: `export const runIsMine(run, workspacePubkey): boolean`.

- [ ] **Step 1: Write the failing test**

Create `app/src/console/views/agent/AgentView.runmine.test.ts`:

```ts
import { describe, expect, it } from "vitest";

import type { PendingRun } from "../../../domain/runs-client";
import { runIsMine } from "./AgentView";

const run = (requester: PendingRun["requester"]): PendingRun =>
  ({ run_id: "r", requester }) as PendingRun;

describe("runIsMine", () => {
  it("matches an external requester equal to my pubkey (hex, any case)", () => {
    expect(runIsMine(run({ external: [0xab, 0xcd] }), "ABCD")).toBe(true);
  });

  it("rejects a different external requester", () => {
    expect(runIsMine(run({ external: [0x01, 0x02] }), "abcd")).toBe(false);
  });

  it("is false for module/system requesters and when I have no pubkey", () => {
    expect(runIsMine(run({ module: "tagging" }), "abcd")).toBe(false);
    expect(runIsMine(run("system"), "abcd")).toBe(false);
    expect(runIsMine(run({ external: [0xab, 0xcd] }), null)).toBe(false);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd app && npx vitest run src/console/views/agent/AgentView.runmine.test.ts`
Expected: FAIL — `runIsMine` is not exported.

- [ ] **Step 3: Add the `names` import and the `runIsMine` helper**

In `app/src/console/views/agent/AgentView.tsx`, add the import (near the other domain imports, after line 15):

```ts
import { displayNameForKey, sameKey, shortKey } from "../../../domain/names";
```

Add the helper next to `hexOf` (after line 129):

```ts
/** Whether a run was requested by the local user. On a networked node the
 *  requester's `external` bytes ARE the submitter's pubkey (== workspace
 *  pubkey), so this is a hex-key equality. Module/system requesters (chat,
 *  jobs) never match, and no local pubkey means "not mine". */
export const runIsMine = (
  run: PendingRun,
  workspacePubkey: string | null,
): boolean =>
  typeof run.requester === "object" &&
  "external" in run.requester &&
  sameKey(hexOf(run.requester.external), workspacePubkey);
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd app && npx vitest run src/console/views/agent/AgentView.runmine.test.ts`
Expected: PASS.

- [ ] **Step 5: Add the badge + "you" chip to `RunRow`**

Extend `RunRow`'s props (line 1514) and render the chips. Change the destructure/type:

```tsx
function RunRow({
  run,
  agents,
  channels,
  op,
  onCancel,
  assigneeName,
  mine,
}: {
  run: PendingRun;
  agents: AgentRecord[];
  channels: Channel[];
  /** The run's finalization record (a cancel keys by run id). */
  op: OpRecord | undefined;
  onCancel: (id: string) => void;
  /** Display name of the node executing this run, or null when unknown. */
  assigneeName?: string | null;
  /** This run was requested by the local user. */
  mine?: boolean;
}) {
```

In the chip row (lines 1593-1597), add the two chips:

```tsx
          <div style={{ display: "flex", gap: 7, flexWrap: "wrap" }}>
            <FinalizationMark op={op} />
            <Chip text={label} tone={statusTone.blue} />
            {run.thread_root !== null && <Chip text={`thread ${run.thread_root}`} />}
            {assigneeName ? <Chip text={`on ${assigneeName}`} tone={statusTone.agent} /> : null}
            {mine ? <Chip text="you" tone={statusTone.neutral} /> : null}
          </div>
```

- [ ] **Step 6: Thread the data through `RunsTimeline`**

Extend `RunsTimeline`'s props (line 1617) and compute per-run values in the map:

```tsx
function RunsTimeline({
  runs,
  agents,
  channels,
  ops,
  onCancel,
  runAssignee,
  authorNames,
  workspacePubkey,
}: {
  runs: PendingRun[];
  agents: AgentRecord[];
  channels: Channel[];
  /** The store's finalization ledger — run rows draw their marks. */
  ops: OpLedger;
  onCancel: (id: string) => void;
  /** run_id -> hex node key executing it (the saga assignee). */
  runAssignee: Map<string, string>;
  /** hex key -> display name, for the executor badge. */
  authorNames: Record<string, string>;
  /** The local user's pubkey, for the "you" marker. */
  workspacePubkey: string | null;
}) {
```

Replace the `runs.map(...)` block (lines 1659-1668) with:

```tsx
          {runs.map((run) => {
            const assigneeKey = runAssignee.get(run.run_id) ?? null;
            const assigneeName = assigneeKey
              ? (displayNameForKey(assigneeKey, authorNames) ?? shortKey(assigneeKey))
              : null;
            return (
              <RunRow
                key={run.run_id}
                run={run}
                agents={agents}
                channels={channels}
                op={ops[opKey.run(run.run_id)]}
                onCancel={onCancel}
                assigneeName={assigneeName}
                mine={runIsMine(run, workspacePubkey)}
              />
            );
          })}
```

- [ ] **Step 7: Add the Mine/All filter state, toggle, and filtered runs**

In the `AgentView` component, add filter state right after the `tab` state (line 1826):

```ts
  const [tab, setTab] = useState<AgentTab>("agents");
  const [runFilter, setRunFilter] = useState<"all" | "mine">("all");
```

Replace the Activity-tab body (the `<main>…</main>` at lines 2007-2022) with a version that adds the toggle and passes the new props + filtered runs:

```tsx
        <main style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: 22 }}>
          <div style={{ maxWidth: 720, margin: "0 auto" }}>
            <JobsWorkerRow
              on={jobWorkerOn}
              op={state.ops[opKey.jobWorker()]}
              onToggle={toggleJobWorker}
            />
            <div style={{ display: "flex", gap: 6, marginBottom: 12 }}>
              {(["all", "mine"] as const).map((f) => (
                <button
                  key={f}
                  type="button"
                  onClick={() => setRunFilter(f)}
                  style={{
                    ...secondaryButton,
                    minHeight: 26,
                    padding: "3px 10px",
                    background: runFilter === f ? color.dark : color.paper,
                    color: runFilter === f ? color.onDark : color.muted2,
                  }}
                >
                  {f === "all" ? "All" : "Requested by you"}
                </button>
              ))}
            </div>
            <RunsTimeline
              runs={
                runFilter === "mine"
                  ? state.pendingRuns.filter((run) =>
                      runIsMine(run, state.workspace?.pubkey ?? null),
                    )
                  : state.pendingRuns
              }
              agents={state.agents}
              channels={state.channels}
              ops={state.ops}
              onCancel={actions.cancelRun}
              runAssignee={state.runAssignee}
              authorNames={state.authorNames}
              workspacePubkey={state.workspace?.pubkey ?? null}
            />
          </div>
        </main>
```

- [ ] **Step 8: Typecheck + full frontend tests + commit**

Run: `cd app && npm run typecheck && npm test`
Expected: no type errors; all tests PASS.

```bash
git add app/src/console/views/agent/AgentView.tsx app/src/console/views/agent/AgentView.runmine.test.ts
git commit -m "feat(app): run node badge, requested-by-you chip, and Mine/All filter"
```

---

### Task 7: End-to-end verification in the real app

**Files:** none (verification only).

Backend unit tests + frontend unit tests cover the join logic deterministically. The saga→assignee correctness over a real network is already covered by `bin/node/tests/dispatch_e2e.rs::mention_routes_to_the_announced_provider_across_nodes` (a run provably executes on the announced provider). A live-window e2e assertion on the transient `AwaitingResult` `assignee` would be timing-flaky, so it is intentionally omitted — this task verifies the UI instead.

- [ ] **Step 1: Build gate**

Run: `make install` (type-checks the build + tests per the repo gate) and `cargo test -p dispatch`.
Expected: green.

- [ ] **Step 2: Drive the real window**

Use the `tauri-debug` skill (or `qa` for a fleet worktree) against a networked workspace with ≥1 announced executor and an agent mid-run:
- Members view: each member row shows its executor tags as chips; the detail panel shows a `capabilities` row. A member with nothing announced shows none.
- Agent → Activity: an in-flight run shows an `on <node>` badge; a run you requested shows a `you` chip; the **Requested by you** toggle filters to your runs.

- [ ] **Step 3: Request review**

Invoke `superpowers:requesting-code-review` (or the repo's adversarial review) on the branch before merge — per the repo's "adversarial-review before merging agent diffs" practice.

## Self-Review

**Spec coverage:**
- Feature 1 (capability chips on members) → Task 2 (client) + Task 4 (store) + Task 5 (view). ✅
- Feature 2 (requested-by-you) → Task 6 (`runIsMine`, "you" chip, Mine/All filter). ✅
- Feature 3 (node badge on runs) → Task 1 (backend assignee) + Task 3 (client) + Task 4 (store) + Task 6 (badge). ✅
- Approach (B) view-only `query_with` facade, no app-hash impact → Task 1. ✅
- Testing (Rust unit for query_with both cases; frontend client + join-helper tests) → Tasks 1, 2, 3, 5, 6. ✅
- Non-goals honored: no members-side "sessions running here" (dropped), no terminal placement, no saga hook, no presence. ✅

**Placeholder scan:** every code step carries complete code; no TBD/TODO/"handle errors". ✅

**Type consistency:** `capabilitiesByNode: Map<string,string[]>` and `runAssignee: Map<string,string>` are named identically across `state.ts`, `DucktapeProvider.tsx`, `MembersView.tsx`, and `AgentView.tsx`. `DispatchView.assignee` is `Option<Vec<u8>>` (Rust) ↔ `number[] | null` (TS). `runIsMine(run, workspacePubkey)` and `makeMembers(..., capabilitiesByNode)` signatures match their call sites. ✅
