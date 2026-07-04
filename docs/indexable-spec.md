# Indexable — the per-module materialized-view contract

Status: shipped with the first indexer slice (chat, tasks, document, pages).
Code: `crates/kernel/indexer` (the contract + store), `crates/apps/*-index`
(per-module implementations), `bin/noded` (the feed and the HTTP lanes).

## 1. Position: the derived tier

Ducktape state lives in two tiers with opposite contracts:

| | canonical tier | derived tier (this spec) |
| --- | --- | --- |
| substrate | qmdb / git — authenticated | fluent31 — ordered, scannable |
| in the app-hash | **always** (`Module::root()`) | **never** |
| cross-node | byte-identical by consensus | node-local, no determinism claim |
| reads | point lookups, module `query` | scans, search, partitions, views |
| crash story | replay / state-sync to the root | **rebuild** — delete and re-fold |

The canonical tier is deliberately not a database: `any::unordered` qmdb is
hashed keys and point lookups — no scans, no secondary indexes, no search.
Anything that needs those shapes is a *read model*, and read models are
derived: materialized from the finalized op stream, disposable by
construction, and invisible to consensus. Nothing a `root()` commits to may
ever live in — or be read back from — the derived tier.

## 2. What a module gets

Every module registered at genesis gets its own index database (one fluent31
`Db` under `<storage>/index/<module-id>/`) fed by the node as blocks commit.
Two things exist for every module with zero module code:

- the **op log** — `op/{height:016x}/{seq:04x}` → a JSON envelope holding the
  applied op payload verbatim (`seq` is the block-wide dispatch index, so
  drain order survives the per-module split);
- the **watermark** — `meta/height`, moved in the same atomic batch as the
  block's rows: every block at or below it is fully reflected, no gaps.

A module becomes **indexable** by shipping a mapper; that is what turns raw
storage into *its* materialized view with *its own endpoint*.

## 3. The mapper contract (`indexer::ModuleIndexer`)

A mapper is one type implementing:

- `module() -> &str` — the genesis module id it consumes.
- `index_op(ctx, meta, payload, out) -> Result<()>` — the **fold**: map one
  applied op into derived writes.
- `serve_view(reader, req) -> Result<Vec<u8>>` — the **view**: the module's
  read projection over its derived keys. Optional; the default declares
  `ViewUnsupported`.

### 3.1 Placement

A mapper lives in a dedicated `<module>-index` crate beside the module's
`<module>-interface` crate, and depends on exactly two internal things: the
`indexer` contract crate and the module's **types-only interface crate** —
never the module implementation, never `sdk`/`host`. The indexer crate itself
is domain-agnostic and depends on no module code (the same layering rule that
keeps hydration generic; break it and the dependency cycles return).

### 3.2 Fold rules

1. **Applied ops only, all of them.** The fold sees exactly the dispatches
   consensus applied — root ops *and* follow-ups — in drain order. A failed op
   aborts its whole block and never reaches the index. Mirror module
   semantics on that assumption (tasks-index files every `CreateTask` as
   `Open` because a duplicate create would have aborted; chat-index mirrors
   `head_seq` because every applied post assigned exactly the next sequence).
2. **Deterministic data-in/data-out.** No IO, no clock, no randomness. Inputs
   are `OpMeta` (height, consensus time, block-wide seq, origin tag), the op
   payload, and `ctx` reads of the module's own index. Anything else makes
   the rebuild diverge from the original fold.
3. **Reads see the block so far.** `ApplyCtx::get` overlays the current
   block's earlier staged writes on committed state, so read-modify-write
   folds (post-then-edit in one block) never lose writes.
4. **Writes ride one atomic batch.** Everything staged through `Derived`
   lands with the op rows and the watermark in a single `WriteBatch`; a
   read model can never be half a block ahead of the op log.
5. **Reserved namespaces.** `op/` and `meta/` belong to the store; a derived
   write into them is refused.
6. **Errors poison, never guess.** The op was applied — a fold that cannot
   mirror it (interface drift, damaged row) has no honest fallback. An error
   poisons the store: writes refuse from then on, reads keep serving
   stale-but-consistent state, and the remedy is a rebuild.
7. **Pre-index history is out of scope.** An op referencing state the index
   never saw (enabled mid-life) folds to a no-op. The honest fix is a
   rebuild from genesis, not a guessed backfill.

### 3.3 View rules

1. **Module-defined wire, like `Module::query`.** Request and reply are the
   module's own JSON shapes (externally tagged enums by house convention);
   the daemon and the store treat both as opaque bytes.
2. **One snapshot per call.** Every `get`/`scan` of a `serve_view` call reads
   the same MVCC snapshot (`ViewReader`), concurrent with the writer.
3. **Read-only.** A view never writes; there is no API for it to do so.
4. **Views may be absent.** `ViewUnsupported` is a first-class answer, not a
   failure — see §5.

## 4. The endpoints (noded)

Reads never cross the node-actor lane; they run on the HTTP runtime against
snapshots, like the blob and telemetry lanes.

| route | serves |
| --- | --- |
| `GET /v1/index/status` | per-module watermarks + the poison flag |
| `GET /v1/index/{module}/ops?after=&limit=` | the op log, paged, envelopes verbatim |
| `GET /v1/index/{module}/scan?prefix=&after=&limit=` | raw derived keys (debugging, generic consumers) |
| `POST /v1/index/{module}/view` | **the module's own endpoint** — its mapper's `serve_view` |

A module with no mapper (or a mapper with no view) answers 404 on `view`.

## 5. When NOT to index

Not every module earns a mapper. Skip one when:

- **the substrate is already a read model.** forge's state *is* a git repo —
  cloneable, greppable, log-walkable over the existing smart-HTTP lane. An
  index would be a worse second copy.
- **canonical queries already fit the read shape.** profiles is a small
  origin→name map; `query` answers it point-wise. Listing modules with
  nothing to scan or search gains nothing from the derived tier.
- **the data never leaves the node-local plane.** files' chunk bytes bypass
  consensus entirely; there is no op stream to fold.

The first shipped mappers and why they exist:

| module | view | why the canonical tier can't |
| --- | --- | --- |
| chat | `search` — full-text over message heads | no scans over hashed keys; search is a read model |
| document | `search` — full-text over blocks | same |
| pages | `search` — full-text over the block tree | same; subtree removal mirrored via child membership |
| tasks | `byStatus` pages + `task` lookup | canonical read is one unpaged `List` |

## 6. Rebuild and failure story

- The index directory is disposable: delete `<storage>/index` (or one
  module's subdirectory) and the folds repopulate from new blocks.
- noded resumes its local block counter **above** the index watermark so
  op-log heights stay monotonic across restarts; wiping the index therefore
  also resets the counter's floor. On a consensus node the op stream replays
  from the journal/state-sync instead — same fold, same contract.
- Poisoned means poisoned: no gap-skipping, no partial patching. Reads stay
  up; the operator wipes and rebuilds. `GET /v1/index/status` is the
  observable.
- Durability is `SyncMode::Periodic` (bounded loss window, torn tails
  truncate on recovery) — correct for a tier whose worst case is a rebuild.
