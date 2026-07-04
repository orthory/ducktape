# Spec — Deterministic admission boundary (heartbeat joiner-fork fix)

> Status: **design, validated (GPT-5.5 adversarial + confirming run), not yet implemented.** Date: 2026-07-04.
> Root cause **confirmed by reproduction**; naive "serve from a stable checkpoint" design judged **NOT READY** and hardened below.
> No backwards compatibility: this is a flag-day wire change on fresh genesis — delete old shapes, don't version them.

## 1. Problem & confirmed root cause

A 1-second `consensus.nop` heartbeat (`bin/node/src/main.rs:117`, pump `~2667`) keeps finalized views advancing on an otherwise idle chain. It works on a solo node but **forks a joining validator during multi-validator admission**.

**Confirmed mechanism (reproduced 3/3 under `yes`-on-all-cores load): stale, non-atomic admission-boundary promotion.**

1. The joiner parks, then syncs every module at manifest boundary `B` (`main.rs:1660`), fabricates a recovery checkpoint at `B` (`main.rs:1722-1758`), and reboots.
2. Because the heartbeat never lets the network quiesce, incumbents keep finalizing **state-changing** epoch-1 `directory` ops at heights `> B` (the test writes 5 `dir_set` filler ops, `bin/node/tests/invite_e2e.rs:157-170`) while the joiner is syncing/rebooting.
3. `OrderedNode::resume` installs the recovered height as `applied_floor` and drain skips `height <= floor` (`crates/kernel/node/src/lib.rs:785,807,927`). It performs **no catch-up**; `B+1..tip` are applied only if consensus later reports those views into `poll_delivered`.
4. A promoted joiner **cannot** obtain `B+1..tip`: while parked it black-holes the engine lanes (`main.rs:1554-1568`), so it has neither a local recovery journal (`Record::Block/Seal`, `crates/kernel/recovery/src/lib.rs:214-228,705-735`) nor seeded content-store frames (`main.rs:2092-2100`) for that range, and the consensus payload backfill is a bounded FIFO cache — "a peer that has fallen further behind than the cache window must rebuild through module state sync, not per-op fetch" (`crates/kernel/consensus/src/lib.rs:219-229`).
5. Result: the joiner's `directory` (a state-based `BTreeMap` snapshot module, `crates/examples/directory/src/lib.rs`) omits the post-`B` filler ops; its final app-hash disagrees with incumbents (`invite_e2e.rs:185` "incumbent and promoted validator disagree on state"). app-hash is module-roots-only / height-independent (`crates/kernel/state/src/lib.rs:35-47`), so `synced app_hash == recovered app_hash` proves self-consistency, **not** correctness.

**Not** a qmdb close/reopen bug: commonware sync flushes + verifies root before returning and reopen recomputes deterministically (high confidence). `directory` is the visible module only because it is the one the test mutates in epoch 1.

### Reproduction evidence (2026-07-04)
Instrumented `main.rs` to refetch the latest manifest immediately after `sync_all_modules`; ran `live_quorum_admits_a_fourth_validator` 6× under full CPU load → **3 PASS / 3 FAIL**, all fails = the real state disagreement (not a timeout). PASS runs had `latest.height > B.height` but **directory unchanged** (NOP-only window). One FAIL showed the exact signature `dir: host==manifest but latest!=host` (network moved directory during the sync); the others forked with the divergence developing just after the sync instant.

## 2. Why the naive fix is NOT READY

"Serve statesync from a stable checkpoint boundary" alone leaves four critical holes (GPT-5.5 adversarial review):

| # | Hole | Evidence |
|---|------|----------|
| C1 | Capture keyed by **height only** + `MAX_CAPTURES=4` eviction; snapshot chunks keyed by `{height,module,offset}` — not bound to app-hash | `crates/kernel/statesync/src/lib.rs:53,160,426,544,567` |
| C2 | Resolver lane still fetches the **live** qmdb target; `ManifestEntry` carries no pinned `{root,start,op_count}` | `crates/kernel/statesync/src/lib.rs:118`, `crates/kernel/statesync/src/qmdb.rs:216,301` |
| C3 | Floor cert not bound to boundary height — joiner only decodes it; commonware `Floor::assert` checks epoch/sig only | `main.rs:1704,1743` |
| C4 | qmdb retention not tied to active boundaries — prune bounded only by `sync_boundary` can invalidate a slow boundary | `crates/kernel/statesync/src/qmdb.rs:216-226`, commonware `db.rs:503` |

And, most importantly, it does not address the **core defect**: there is no mechanism that closes the `B+1..tip` gap before promotion. Live consensus backfill **cannot** be relied on for already-finalized history the joiner was parked through.

## 3. Design

### 3.1 Central invariant

> A joiner may promote **only** at a boundary `B` for which all three hold at the instant of promotion:
> 1. **Internally consistent**: the installed host's `app_hash` equals `B.app_hash` (already enforced by per-module root checks + `host.app_hash()` recompose, `main.rs:607`).
> 2. **Floor-bound**: `B` carries a durable finalization cert that certifies **exactly** `B.height`, i.e. `B.view_base + finalization.view() == B.height`.
> 3. **Not stale**: `B.app_hash` equals the network's **current latest-boundary app_hash**. Because app-hash is height-independent, NOP-only advances above `B` do not change it — so this holds as soon as no *state-changing* op has been finalized above `B` that the joiner would skip.

Invariant (3) is the piece the current code is missing. It is enforced by a **post-sync revalidation loop** (§3.2). It terminates for any finite burst of state-changing ops: once the burst is finalized and only NOPs follow, the synced app-hash matches the latest boundary's and the joiner promotes. Under a pathological unbounded write stream the joiner simply stays parked (safe) rather than forking.

### 3.2 Promotion algorithm (the core fix)

Replace the current "sync once, break, fabricate checkpoint" (`main.rs:1642-1758`) with:

```
loop {
    B      = fetch_manifest()                      // pinned boundary id {height, app_hash}
    require B is admissible (self in participants, floor present if height > view_base)
    host   = sync_all_modules(B)  into SCRATCH storage   // §3.3, §3.4 — all requests boundary-scoped
    assert host.app_hash() == B.app_hash                  // internal consistency
    assert_floor_binds(B)                                  // §3.5
    latest = fetch_manifest()                              // REVALIDATE
    if latest.app_hash == B.app_hash {                     // invariant (3): no state drift
        promote(B, host)      // atomic scratch->live, write checkpoint + floor, reboot
        break
    }
    // state drifted during sync (a real op finalized above B): discard scratch, retry at latest
}
```

`promote()` does the scratch→live swap only after a **reopen preflight** (close the scratch substrates, reopen from their final ids, recompute app-hash, assert it still equals `B.app_hash`) so a reopen-drift fails **before** the checkpoint is written rather than bricking the validator after (`crates/kernel/recovery/src/lib.rs:968-975` already does the post-recovery check; we move an equivalent check ahead of promotion).

### 3.3 First-class boundary artifact

Introduce `BoundaryId { height: u64, app_hash: StateRoot }` and make every state-sync fetch carry it.

- **Server capture** keyed by `BoundaryId`, not height (`statesync/lib.rs:426,544,567`). Captures backing an **active lease** are never evicted; `MAX_CAPTURES` (`:53`) applies only to unleased captures.
- **Wire** (flag-day, no versioning):
  - `Manifest` returns `BoundaryId` (it already has `height`, `app_hash`, `view_base`, `epoch`, `participants`, `floor_cert`).
  - `SyncRequest::Chunk { boundary: BoundaryId, module_id, offset }` (was `{height, module_id, offset}`, `:160`).
  - `SyncRequest::Module { boundary: BoundaryId, module_id, body }`; the server rejects a module request whose `BoundaryId` is not an active lease.
- **Lease lifecycle**: a `Manifest` fetch opens a lease on its `BoundaryId`; the lease is released on sync completion or a timeout. While leased, the capture and every module's pinned resolver range (§3.4) are retained.

### 3.4 Pinned resolver target (qmdb lane)

- `ManifestEntry` for a resolver-backed module carries the pinned target `{ root, start, op_count }` (add fields at `statesync/lib.rs:118`). `op_count` is the exact anchor `historical_proof` needs (`qmdb.rs:78,236`).
- The joiner **must not** call live `RemoteQmdbResolver::fetch_target()` (`qmdb.rs:301`) for the adoption target; it syncs exactly the manifest's pinned target and requests ops via `historical_proof(op_count, start_loc, …)`.
- **Retention**: each active boundary lease registers a per-module minimum retained range; qmdb compaction must not prune below `min(sync_boundary, oldest_active_lease_start)`. If a pruned-range error (`ItemPruned`/`HistoricalFloorPruned`, commonware `qmdb/mod.rs`) is hit mid-sync, the sync fails and the promotion loop (§3.2) refetches at a newer boundary.

### 3.5 Floor ↔ boundary binding

- **Server** publishes a boundary only with a durable `FloorCert { height: B.height }` when `B.height > B.view_base` (it already filters `latest_floor` to the current finalized height, `main.rs:2390-2404`; make that a publication precondition rather than a serving-time filter).
- **Joiner** (`assert_floor_binds`): after `decode_finalization(&scheme, &cert)` (`main.rs:1704`), also decode the finalization's view and assert `B.view_base + finalization.view() == B.height` **before** writing the checkpoint/floor (`main.rs:1743`). A cert that verifies cryptographically but certifies a different height is a hard admission failure.

## 4. Code touch-points

| Area | File:line | Change |
|------|-----------|--------|
| Promotion loop + revalidation + scratch/reopen | `bin/node/src/main.rs:1642-1758` | rewrite per §3.2 |
| Boundary-scoped sync requests | `bin/node/src/main.rs:443-560` (`sync_all_modules`, `fetch_target`, `snapshot_of`) | key by `BoundaryId`; use pinned qmdb target; sync into scratch |
| Floor binding assert | `bin/node/src/main.rs:1704,1743` | add `B.view_base + cert.view == B.height` |
| Serving = publish leased boundary | `bin/node/src/main.rs:2390-2408` | serve leased `BoundaryId`, floor as publication precondition |
| Capture keyed by `BoundaryId` + leases | `crates/kernel/statesync/src/lib.rs:53,118,133,160,426,544,567` | `BoundaryId`, lease map, no-evict-active, pinned resolver fields in `ManifestEntry` |
| qmdb pinned target + retention floor | `crates/kernel/statesync/src/qmdb.rs:216,236,301` | serve pinned target; register lease min-range in prune bound |
| Recovery reopen preflight (reuse) | `crates/kernel/recovery/src/lib.rs:968-975` | equivalent check moved ahead of promotion |

## 5. Test plan

Gate (must pass before shipping the heartbeat): `live_quorum_admits_a_fourth_validator` **10/10 under `yes`-on-all-cores load with the heartbeat enabled** (`bin/node/tests/invite_e2e.rs:123`). This test explicitly forces incumbents to finalize past cutover, so it exercises invariant (3).

New targeted tests:
- **Protocol**: `BoundaryId` wire round-trips; server rejects a `Chunk`/`Module` request whose `BoundaryId` is not an active lease; capture cache rejects a wrong-`app_hash` request at the same height.
- **Resolver**: joiner adopts the manifest's pinned `{root,start,op_count}` and never calls live `fetch_target`; a pruned pinned range surfaces an error that forces refetch.
- **Floor**: a floor cert whose certified view ≠ `B.height - B.view_base` fails admission.
- **Revalidation**: with a scripted source that finalizes a state-changing op between the joiner's sync and its revalidation, the joiner resyncs and only promotes at a matching `app_hash`.
- **Promotion**: sync `kv/document/chat` + snapshot modules into scratch, close/reopen from final ids, assert app-hash before checkpoint write.
- **No-eviction**: a slow joiner spanning `> MAX_CAPTURES` boundary rotations still completes against its leased boundary.

## 6. Risks & open items

- **Liveness under sustained writes**: the revalidation loop may not converge if state-changing ops never pause. Mitigation: bounded attempts; the joiner stays parked (safe) rather than forking. Acceptable — correctness over admission latency. (Real workloads have write gaps; NOPs are free.)
- **Lease exhaustion / DoS**: many concurrent joiners each pin a boundary. Mitigation: lease timeout + cap on concurrent leases; excess joiners retry.
- **Scratch storage cost**: syncing into scratch then atomically promoting doubles transient disk for the sync window. Acceptable; it is what a correct retry-safe promotion requires.
- **The separate `crates/kernel/consensus/src/lib.rs` propose-pacing change** in the working tree is unrelated to this fork and slows cutover ~3×; decide independently whether to keep it.

## 7. Rollout
Flag-day: land the wire/protocol change and the promotion rewrite together on fresh genesis; no v1/v2 shims. Do **not** ship the heartbeat/pacing until the 10/10 gate passes with this fix in place.
