# Admission Boundary Fork Fix — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop a joining validator from forking its state during multi-validator admission under the block-time heartbeat, by promoting only at a deterministic, floor-bound, non-stale statesync boundary.

**Architecture:** Make the statesync boundary a first-class `BoundaryId {height, app_hash}` object that is leased (never evicted mid-sync), whose resolver-lane target is pinned in the manifest (no live `fetch_target`), and whose floor cert is bound to the boundary height. Rewrite the joiner promotion path into a **sync-into-scratch → revalidate app_hash against the current network boundary → reopen-preflight → atomic promote** loop, so a joiner never freezes a boundary the network has already moved past.

**Tech Stack:** Rust; `commonware-storage` 2026.5.0 (qmdb, MMR-backed, `historical_proof`); `commonware-consensus` 2026.5.0 (simplex, finalization certs). Crates: `bin/node` (`node-bin`), `crates/kernel/statesync`, `crates/kernel/node`, `crates/kernel/recovery`, `crates/kernel/host`.

## Global Constraints

- **No backwards compatibility.** Flag-day wire change on fresh genesis. Delete the old request/manifest shapes; do NOT add v1/v2 variants or compatibility shims. (Ref: repo memory "No backwards compatibility".)
- **Do NOT "fix" by disabling the heartbeat**, weakening the test, or reducing load. The heartbeat must stay on.
- **Verification gate (hard):** `bin/node/tests/invite_e2e.rs::live_quorum_admits_a_fourth_validator` must pass **10/10 under `yes`-on-all-cores CPU load with the heartbeat enabled**.
- **Execution environment:** develop against the CURRENT dirty working tree (the heartbeat change is uncommitted in `bin/node/src/main.rs` + `crates/kernel/consensus/src/lib.rs`), OR apply `ship/heartbeat-repro.patch` onto `origin/dev`. Do NOT start from a clean `origin/dev` worktree — the repro depends on the heartbeat being present.
- **Commits stay with the human/Opus** (repo memory "Codex: no publishing"): the implementer leaves changes staged and tests green; it does not `git commit`/`push`/PR. The "Checkpoint" steps below mark logical stopping points for a human commit.
- `app_hash` is module-roots-only and **height-independent** (`crates/kernel/state/src/lib.rs:35-47`) — this is load-bearing: NOP-only height advances do not change `app_hash`, which is why the revalidation loop (Task 6) terminates.

---

## File structure

| File | Responsibility | Change |
|------|----------------|--------|
| `crates/kernel/statesync/src/lib.rs` | wire types + `SyncServer` capture/lease + manifest | `BoundaryId`; capture keyed by `BoundaryId`; lease map (no-evict-active); `ManifestEntry` pinned-target fields; `SyncRequest::{Chunk,Module}` carry `BoundaryId`; encode/decode |
| `crates/kernel/statesync/src/qmdb.rs` | qmdb resolver serve + retention | serve pinned target at `op_count`; retention floor from active leases |
| `bin/node/src/main.rs` | serving loop + `sync_all_modules` + joiner promotion | publish leased boundary + floor precondition; boundary-scoped + pinned sync into scratch; revalidation/reopen/atomic promote; floor-height assert |
| `crates/kernel/recovery/src/lib.rs` | reopen-preflight helper (reuse) | expose/reuse the post-recovery app-hash check ahead of promotion |
| `crates/kernel/statesync/tests/boundary.rs` (new) | protocol/lease unit tests | round-trip, wrong-app_hash reject, no-evict-active, pinned-target adopt, pruned-range error |
| `bin/node/tests/invite_e2e.rs` | e2e gate + revalidation e2e | keep the 10/10 gate; add a drift-forces-resync assertion |

Task order is dependency-ordered: the wire/type foundation (1–4) lands and is unit-tested before the promotion rewrite (5–6) consumes it; the gate (7) runs last.

---

### Task 1: `BoundaryId` type + boundary-keyed captures with leases

**Files:**
- Modify: `crates/kernel/statesync/src/lib.rs` (`Capture`, `SyncServer` ~`400-585`; `MAX_CAPTURES` `:53`; `captures: BTreeMap<u64, Capture>` `:426`; insert/evict `:567-582`)
- Test: `crates/kernel/statesync/tests/boundary.rs` (create)

**Interfaces:**
- Produces:
  - `pub struct BoundaryId { pub height: u64, pub app_hash: StateRoot }` (derive `Clone, Copy, PartialEq, Eq, Hash, Debug`).
  - `SyncServer` captures keyed by `BoundaryId`.
  - `SyncServer::lease(&mut self, id: BoundaryId)` / `release(&mut self, id: BoundaryId)`; leased captures are exempt from `MAX_CAPTURES` eviction.
- Consumes: existing `Capture { app_hash, coords, modules }`, `StateRoot`.

- [ ] **Step 1: Write the failing test** — `crates/kernel/statesync/tests/boundary.rs`

```rust
// Two captures at the SAME height with DIFFERENT app_hash must coexist and be
// addressable independently (height-only keying was the C1 bug).
#[test]
fn captures_keyed_by_boundary_id_not_height() {
    let mut srv = SyncServer::new();
    let b1 = BoundaryId { height: 32, app_hash: StateRoot([1u8; 32]) };
    let b2 = BoundaryId { height: 32, app_hash: StateRoot([2u8; 32]) };
    srv.insert_capture_for_test(b1, capture_fixture(b1));
    srv.insert_capture_for_test(b2, capture_fixture(b2));
    assert!(srv.has_capture(b1) && srv.has_capture(b2));
}

// A leased capture is never evicted even past MAX_CAPTURES rotations.
#[test]
fn leased_capture_survives_eviction() {
    let mut srv = SyncServer::new();
    let held = BoundaryId { height: 10, app_hash: StateRoot([9u8; 32]) };
    srv.insert_capture_for_test(held, capture_fixture(held));
    srv.lease(held);
    for h in 100..100 + (MAX_CAPTURES as u64) + 3 {
        let b = BoundaryId { height: h, app_hash: StateRoot([(h % 251) as u8; 32]) };
        srv.insert_capture_for_test(b, capture_fixture(b));
    }
    assert!(srv.has_capture(held), "leased boundary must not be evicted");
}
```
(Add small `capture_fixture`/`insert_capture_for_test`/`has_capture` test helpers as `#[cfg(test)]` or `#[doc(hidden)]` methods on `SyncServer` — the fixture only needs a valid `Capture` shape.)

- [ ] **Step 2: Run to verify it fails** — `cargo test -p statesync --test boundary` → FAIL (type/methods absent).

- [ ] **Step 3: Implement** — in `crates/kernel/statesync/src/lib.rs`:
  - Add `BoundaryId` next to `Capture`.
  - Change `captures: BTreeMap<u64, Capture>` → `captures: BTreeMap<BoundaryId, Capture>`; add `leased: BTreeSet<BoundaryId>`.
  - `lease`/`release` insert/remove from `leased`.
  - In the eviction loop (`:575-582`), skip any key in `leased`; evict the oldest **unleased** capture only.
  - Update `ensure_capture` (`:538-583`) to key on `BoundaryId { height: finalized.height, app_hash: snapshot.app_hash }`.

- [ ] **Step 4: Run to verify it passes** — `cargo test -p statesync --test boundary` → PASS.

- [ ] **Step 5: Checkpoint** — `git add crates/kernel/statesync` (human commits: `feat(statesync): key captures by BoundaryId with leases`).

---

### Task 2: Manifest carries `BoundaryId` + pinned resolver target

**Files:**
- Modify: `crates/kernel/statesync/src/lib.rs` (`ManifestEntry` `:118-122`; `Manifest` `:133-149`; encode/decode of both; `try_handle` Manifest arm `:474-497`)
- Test: `crates/kernel/statesync/tests/boundary.rs`

**Interfaces:**
- Produces:
  - `ManifestEntry { module_id, root, kind, resolver_target: Option<ResolverTarget> }` where `pub struct ResolverTarget { pub root: SyncDigest, pub start: u64, pub op_count: u64 }`. `resolver_target` is `Some` for resolver-backed (qmdb) modules, `None` for snapshot modules.
  - `Manifest` exposes `pub fn boundary_id(&self) -> BoundaryId` (= `{height, app_hash}`).
- Consumes: Task 1 `BoundaryId`; `qmdb::SyncTarget` (`root`, range) from `crates/kernel/statesync/src/qmdb.rs:48,216-226`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn manifest_roundtrip_carries_pinned_resolver_target() {
    let m = manifest_fixture_with_resolver("kv", ResolverTarget {
        root: SyncDigest([7u8; 32]), start: 5, op_count: 42,
    });
    let bytes = encode_response(&SyncResponse::Manifest(m.clone()));
    let SyncResponse::Manifest(back) = decode_response(&bytes).unwrap() else { panic!() };
    assert_eq!(back.boundary_id(), m.boundary_id());
    let e = back.entry("kv").unwrap();
    assert_eq!(e.resolver_target.as_ref().unwrap().op_count, 42);
    assert_eq!(e.resolver_target.as_ref().unwrap().start, 5);
}
```

- [ ] **Step 2: Run to verify it fails** — `cargo test -p statesync --test boundary` → FAIL.

- [ ] **Step 3: Implement**
  - Add `ResolverTarget` and the `resolver_target` field to `ManifestEntry`.
  - Extend the wire encode/decode for `SyncResponse::Manifest` (the `wire`-based codec around `:241-332`) to serialize `resolver_target` (tag byte for `Some`/`None`, then `root||start||op_count`). No version byte — flag-day.
  - Add `Manifest::boundary_id`.
  - In `try_handle`'s Manifest arm (`:481-497`), populate `resolver_target` for resolver-backed modules from the captured qmdb target; leave `None` for snapshot modules. (Capture must record the qmdb `SyncTarget` per resolver module — extend `CapturedModule` / `ensure_capture` accordingly.)

- [ ] **Step 4: Run to verify it passes** — `cargo test -p statesync --test boundary` → PASS.

- [ ] **Step 5: Checkpoint** — human commit: `feat(statesync): pin resolver target in manifest entries`.

---

### Task 3: Boundary-scoped `Chunk` / `Module` requests; reject non-leased

**Files:**
- Modify: `crates/kernel/statesync/src/lib.rs` (`SyncRequest` `:160-170`; encode/decode `:198-332`; `try_handle` Chunk/Module arms `:499-533`)
- Test: `crates/kernel/statesync/tests/boundary.rs`

**Interfaces:**
- Produces:
  - `SyncRequest::Chunk { boundary: BoundaryId, module_id, offset }` (was `{height, module_id, offset}`).
  - `SyncRequest::Module { boundary: BoundaryId, module_id, body }`.
  - `try_handle` returns `SyncResponse::Error` when `boundary` is not a currently-leased capture.
- Consumes: Task 1 lease map; Task 2 manifest.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn server_rejects_chunk_for_unleased_boundary() {
    let mut srv = SyncServer::new();
    let stale = BoundaryId { height: 5, app_hash: StateRoot([3u8; 32]) };
    let resp = block_on(srv.handle(&host_fixture(), None, &coords_fixture(),
        SyncRequest::Chunk { boundary: stale, module_id: "directory".into(), offset: 0 }));
    assert!(matches!(resp, SyncResponse::Error(_)), "unleased boundary must be rejected");
}
```

- [ ] **Step 2: Run to verify it fails** → FAIL.

- [ ] **Step 3: Implement** — change the two `SyncRequest` variants + their encode/decode; in `try_handle` look captures up by `boundary` and error if absent from `captures`/not leased. Update `SyncServer::handle`/`handle_frame` signatures if needed.

- [ ] **Step 4: Run to verify it passes** → PASS.

- [ ] **Step 5: Checkpoint** — human commit: `feat(statesync): boundary-scope chunk/module requests`.

---

### Task 4: qmdb serves pinned past target; retention floor from leases

**Files:**
- Modify: `crates/kernel/statesync/src/qmdb.rs` (`QmdbSyncReq` `:75-85`; `serve` `:208-240`; `Target`/`fetch_target` `:216-302`); the qmdb module `serve_sync` impls if a new req variant is added.
- Test: `crates/kernel/statesync/tests/boundary.rs` (or a qmdb-focused test file)

**Interfaces:**
- Produces:
  - qmdb can serve the target for a **pinned** `op_count` (either add `QmdbSyncReq::TargetAt { op_count }` served via `historical_proof`, or have the joiner skip target-fetch entirely and adopt the manifest's `ResolverTarget`). Prefer the latter: no new qmdb request, the manifest is authoritative.
  - Retention: a hook `SyncServer` exposes so qmdb pruning uses `min(sync_boundary, oldest_active_lease_start_for_module)`.
- Consumes: Task 1 leases; Task 2 `ResolverTarget`; commonware `historical_proof(op_count, start_loc, max_ops)` (`qmdb.rs:236`).

- [ ] **Step 1: Write the failing test**

```rust
// A pinned range that has been pruned below sync_boundary surfaces a typed
// error the caller can turn into "refetch manifest", not a silent wrong root.
#[test]
fn pruned_pinned_range_errors_for_refetch() {
    let db = qmdb_with_pruned_boundary(/* prune above op_count 10 */);
    let err = block_on(serve_historical(&db, /*op_count*/ 5)).unwrap_err();
    assert!(is_pruned_error(&err), "pruned pinned range must be a typed refetch error");
}
```

- [ ] **Step 2: Run to verify it fails** → FAIL.

- [ ] **Step 3: Implement** — make the server serve the manifest's pinned target range via `historical_proof`; surface `ItemPruned`/`HistoricalFloorPruned` (commonware `qmdb/mod.rs`) as a distinct error variant. Wire the per-module lease minimum into whatever calls qmdb prune so it never prunes below an active lease's `start`.

- [ ] **Step 4: Run to verify it passes** → PASS.

- [ ] **Step 5: Checkpoint** — human commit: `feat(statesync): serve pinned qmdb target, lease-aware retention`.

---

### Task 5: Floor cert bound to boundary height

**Files:**
- Modify: `bin/node/src/main.rs` (serving/publication `:2390-2408`, `:2484-2510`; joiner floor validation `:1704-1714`; floor write `:1743-1748`)
- Test: `bin/node/tests/invite_e2e.rs` (a focused negative test) or a `#[cfg(test)]` unit near the assert helper.

**Interfaces:**
- Produces:
  - Serving publishes a boundary's floor only when a durable `FloorCert { height: B.height }` exists (publication precondition, not a serve-time filter).
  - `assert_floor_binds(boundary, decoded_finalization) -> Result<(), _>`: fails unless `boundary.view_base + finalization.view() == boundary.height`.
- Consumes: `decode_finalization(&scheme, &cert)` (`main.rs:1704`, `crates/kernel/consensus/src/lib.rs:1575`); `FloorCert { epoch, height, cert }`.

- [ ] **Step 1: Write the failing test** (unit on the assertion helper)

```rust
#[test]
fn floor_cert_view_must_map_to_boundary_height() {
    // view_base=30, boundary height=36 => finalization.view() must be 6.
    assert!(assert_floor_binds(&boundary(30, 36), &finalization_at_view(6)).is_ok());
    assert!(assert_floor_binds(&boundary(30, 36), &finalization_at_view(4)).is_err());
}
```

- [ ] **Step 2: Run to verify it fails** → FAIL.

- [ ] **Step 3: Implement** — add `assert_floor_binds`; call it in the joiner path after `decode_finalization` and before `write_floor_cert`/checkpoint (`main.rs:1704`→`:1743`). On the serving side, gate boundary publication on a durable floor at `B.height`.

- [ ] **Step 4: Run to verify it passes** → PASS.

- [ ] **Step 5: Checkpoint** — human commit: `feat(node): bind admission floor cert to boundary height`.

---

### Task 6: Joiner promotion rewrite — scratch sync + revalidation + reopen preflight

**Files:**
- Modify: `bin/node/src/main.rs` (`sync_all_modules` `:427-560` to be boundary-scoped + pinned target + scratch dirs; joiner loop + promotion `:1600-1758`)
- Reuse: `crates/kernel/recovery/src/lib.rs:968-975` (post-recovery app-hash check) as the reopen preflight.
- Test: `bin/node/tests/invite_e2e.rs` (drift-forces-resync e2e) + the existing gate (Task 7).

**Interfaces:**
- Consumes: Tasks 1–5 (`BoundaryId`, pinned targets, boundary-scoped requests, `assert_floor_binds`).
- Produces: a joiner that promotes only at a non-stale, floor-bound, reopen-verified boundary.

- [ ] **Step 1: Write the failing e2e** — extend `invite_e2e.rs`: a variant of `live_quorum_admits_a_fourth_validator` that submits a directory op **between** the joiner finishing sync and promoting (inject via a test hook / timing), asserting the joiner still converges (no fork). Run: `cargo test -p node-bin --test invite_e2e <name> -- --exact`. Expected: FAIL before the rewrite (fork).

- [ ] **Step 2: Implement `sync_all_modules` boundary-scoping** — thread `BoundaryId` into every `Chunk`/`Module` request; for resolver modules adopt `ManifestEntry.resolver_target` instead of `RemoteQmdbResolver::fetch_target()` (`main.rs:452`); sync each module into a **scratch** storage namespace (fresh per attempt) rather than the canonical id.

- [ ] **Step 3: Implement the promotion loop** — replace `main.rs:1642-1758` with:

```text
loop {
    let m = fetch_manifest(&client).await?;              // BoundaryId B = m.boundary_id()
    guard admissible (self in participants; floor present if m.height > m.view_base);
    let host = sync_all_modules(&ctx, &client, &m, scratch(attempt)).await?;  // boundary-scoped
    ensure host.app_hash() == m.app_hash;                // internal consistency
    if m.height > m.view_base { assert_floor_binds(&m, &decode_finalization(..)?)?; }
    let latest = fetch_manifest(&client).await?;         // REVALIDATE
    if latest.app_hash == m.app_hash {                   // invariant (3): no state drift
        reopen_preflight(&host)?;                        // close+reopen scratch, re-check app_hash
        promote_atomic(scratch(attempt) -> live);        // then write checkpoint + floor
        break;
    }
    // real op finalized above B during sync: drop scratch, retry (attempt += 1)
}
```
Keep the existing lease: open a lease on `B` at manifest fetch, release on completion/timeout (Task 1).

- [ ] **Step 4: Run the e2e** — `cargo test -p node-bin --test invite_e2e <name> -- --exact` → PASS.

- [ ] **Step 5: Checkpoint** — human commit: `fix(node): promote joiner only at a revalidated, floor-bound boundary`.

---

### Task 7: Verification gate — 10/10 under load with heartbeat

**Files:** none (runs the existing gate).

- [ ] **Step 1: Build** — `cargo build -p node-bin --bin ducktape-node`. Expected: clean.
- [ ] **Step 2: Load** — `for i in $(seq 1 $(sysctl -n hw.ncpu)); do yes >/dev/null & done`
- [ ] **Step 3: Run 10×** — `for r in $(seq 1 10); do cargo test -p node-bin --test invite_e2e live_quorum_admits_a_fourth_validator -- --exact || echo "FAIL run $r"; done`
- [ ] **Step 4: Kill load** — `pkill -x yes`
- [ ] **Step 5: Confirm** — all 10 PASS. Also run `cargo test -p statesync`, `cargo test -p node-bin --test invite_e2e` (both scenarios), `cargo test -p consensus`, and the other e2e suites (`restart_e2e`, `cluster_e2e`) for regressions.
- [ ] **Step 6: Checkpoint** — human commit / handoff summary.

---

## Self-review

**Spec coverage:** §3.1 invariant → Tasks 4,5,6. §3.2 promotion loop → Task 6. §3.3 boundary artifact + leases → Tasks 1,3. §3.4 pinned resolver target + retention → Tasks 2,4. §3.5 floor binding → Task 5. §5 test plan → Tasks 1–7 (protocol round-trip=T2, wrong-app_hash reject=T1, no-evict-active=T1, unleased reject=T3, pruned-range=T4, floor mismatch=T5, revalidation drift=T6, reopen preflight=T6, gate=T7). No spec section is unmapped.

**Placeholder scan:** internal Rust syntax that depends on unread codec/struct bodies is expressed as a precise change-contract at an exact file:line rather than invented line-for-line — appropriate for a code-reading implementer; every step names the concrete type/field/variant and location, not "handle appropriately".

**Type consistency:** `BoundaryId {height, app_hash}` (T1) is consumed unchanged by T2 (`boundary_id()`), T3 (request fields), T6 (loop). `ResolverTarget {root,start,op_count}` (T2) is consumed by T4 (serve) and T6 (adopt). `assert_floor_binds` (T5) is called in T6. Names are stable across tasks.

## Execution handoff (per user's "1 → 2")

Implementation is delegated to **GPT-5.5 (Codex)** per the repo tiering. The implementer works the tasks in order against the heartbeat-present working tree, runs each task's tests, and leaves changes **staged, not committed** (repo memory "Codex: no publishing" — commits/PR stay with Opus/human). The Task 7 gate (10/10 under load) is the ship bar.
