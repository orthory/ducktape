# Indexable — the per-module materialized-view contract

Status: shipped — the fold/view contract (chat, tasks, document, pages), the
from-state rebuild (§7 lane 1), and the consensus-validator wiring.
Code: `crates/kernel/indexer` (the contract + store), the chat/tasks/document/
pages modules' `src/index.rs` (per-module implementations), `bin/noded` (the feed, the HTTP lanes, and the
shared store construction), `bin/node` (the validator: live fold, replay
fold, boundary heals).

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
  EVERY module's watermark advances on EVERY applied block — not only the
  dispatched modules' — so `watermark < H` always means "blocks are missing",
  never "the module was quiet". that exactness is what the rebuild triggers
  (§7) key off;
- the **backfill floor** — `meta/backfill`, present only after a from-state
  rebuild: rows derived that way carry boundary-stamped coordinates and the
  op log starts above it. `GET /v1/index/status` reports it per module.

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

A mapper lives in the module crate's `index` submodule (`<module>::index`),
and depends on exactly two internal things: the `indexer` contract crate and
the module's **crate-root wire types** — never the module's state machinery,
never `sdk`/`host`. The indexer crate itself
is domain-agnostic and depends on no module code (the same layering rule that
keeps hydration generic; break it and the dependency cycles return).

### 3.2 Fold rules

1. **Applied ops only, all of them.** The fold sees exactly the dispatches
   consensus applied — root ops *and* follow-ups — in drain order. A failed op
   aborts its whole block and never reaches the index. Mirror module
   semantics on that assumption (`tasks::index` files every `CreateTask` as
   `Open` because a duplicate create would have aborted; `chat::index` mirrors
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
| `GET /v1/index/status` | per-module watermarks, backfill floors (§7), the poison flag |
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
  module's subdirectory) and the tier heals — mappers that declare a
  from-state rebuild (§7) re-derive at the next boot's boundary, everything
  else repopulates from new blocks behind a visible backfill floor.
- noded resumes its local block counter **above** the index watermark so
  op-log heights stay monotonic across restarts; wiping the index therefore
  also resets the counter's floor. At startup noded heals any module whose
  watermark trails the resume floor from local canonical state.
- The consensus validator folds the identical contract from three feeds: the
  live drain (every SEALED frame — a rejected frame folds empty, it still
  consumed its height), the journal replay (`recovery::ReplaySink` — blocks
  replay re-executes fold their dispatch trace; blocks it skips as
  already-durable are unreproducible and STOP the fold), and post-reboot
  frame catch-up. Whatever the fold could not reproduce is healed by a
  from-state rebuild at the verified boundary: after checkpoint restore
  (pre-replay) and again at the boot tip once every path converged.
- A serving RESIDENT never folds — it observes state boundaries, not sealed
  frames — so its entire feed is the heal: on every followed boundary whose
  verified app-hash moved, every module re-derives (or re-stamps) at that
  boundary, and the blocks database gains one honest boundary row
  (`IndexStore::apply_block_record` — verified height + app-hash,
  frame-derived fields empty). An unchanged app-hash is an idle stride and
  writes nothing, mirroring the validator's nop gate.
- Poisoned means poisoned: no gap-skipping, no partial patching. Reads stay
  up; the remedy is a rebuild — automatic at the next boot's heal, or an
  operator wipe. `GET /v1/index/status` is the observable.
- Durability is `SyncMode::Periodic` (bounded loss window, torn tails
  truncate on recovery) — correct for a tier whose worst case is a rebuild.

## 7. State-sync and the from-state rebuild (both lanes shipped)

A joiner state-syncs **state**, not op history, so the fold has nothing to
consume — and a synced node with empty views renders its modules poorly. The
index comes together with the sync. Two lanes, in preference order:

1. **State backfill (the contract extension — SHIPPED).** The mapper trait
   grew `supports_rebuild` + `rebuild_from_state(state, meta, out)`: after
   canonical state installs and verifies at a boundary, each mapper
   re-derives its rows from its module's committed state through
   `indexer::StateReader` — a domain-agnostic bytes-in/bytes-out adapter over
   `Module::query` (never `serve_sync`, whose wire is qmdb op-range transfer,
   not logical rows) — streamed through a bounded-batch `Backfill` writer and
   stamped at the boundary. The derivation stays local, rooted in *verified*
   state, and works identically for a wiped index on a live node.

   The store side is `IndexStore::rebuild_module`: watermark drops FIRST
   (an interrupted rebuild re-triggers — crash-safe by re-trigger), the
   database clears (op history starts at the boundary), rows stream in, and
   the watermark + backfill floor stamp LAST. A module whose mapper declares
   no rebuild is stamped backfilled instead (`mark_backfilled`) — its content
   visibly begins at the boundary rather than silently claiming coverage.

   Per-module degradations, documented at each mapper: heights collapse to
   the boundary everywhere; document/pages also lose per-row time (hit sets
   exact, ranking falls to id order); tasks loses `created_by` and both
   heights; chat — the named case — recovers time from `created_at`, so its
   ranking survives intact.

   Triggered wherever `watermark < boundary` (exact, because every module's
   watermark advances on every applied block): noded startup against the
   resume floor, the validator after checkpoint restore and at the boot tip
   (state-sync install, replay gaps, wiped directories all converge there),
   and the resident on every state-changing boundary it follows (§6 — the
   resident's only feed).

2. **Index checkpoint shipping (the optimization — SHIPPED).** fluent31
   checkpoints are complete database directories; a source node ships them
   alongside state-sync for instant warm views. Contents are NOT
   root-verifiable (the derived tier has no root by design), so this lane
   trusts the serving node and stays optional — `sync_index = true` in
   node.toml (node-local operator policy, default off); a verifying joiner
   backfills via lane 1 and compares nothing.

   The mechanics: the serving pump answers the first `IndexModules` request
   per leased boundary by cutting a transient checkpoint of every module
   database plus `_blocks` (`IndexStore::checkpoint_files` — cut, read into
   memory, archive deleted) and attaching the framed blobs to that capture,
   so their lifetime rides the existing lease/evict lifecycle and joiners
   that never ask cost nothing. The joiner fetches the set in chunks over
   the same sync connection, stages it under `<storage>/index/_staging`
   (every file fsynced, a `.complete` marker LAST), and the promoted
   reboot's `IndexStore::open` adopts the staging directory before any
   engine open — a torn fetch is discarded, never adopted.

   Composition with lane 1 is a single comparison, the one that already
   exists: shipped watermarks land at the source's fold tip, so a module
   whose watermark reaches the joiner's boundary skips its heal (warm), and
   anything stale, missing, or refused falls to `watermark < boundary` and
   rebuilds exactly as if nothing was shipped. Cross-binary skew degrades
   the same way: unknown shipped databases are skipped at staging, old
   servers answer the new request tags with an error, and the joiner treats
   every failure as "heal instead", never as an abort. The blocks database
   rides along verbatim — the only path by which a joiner ever gets
   pre-boundary `/v1/blocks` history, since `_blocks` is deliberately
   outside the rebuild story.
