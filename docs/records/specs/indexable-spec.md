# Indexable — the per-module materialized-view contract

Status: shipped — the wasm index-guest architecture: host-written op feed,
engine-folded read models (fluent31 changes-mode triggers), per-module view
guests, boundary stamps, and index-archive shipping. Since the read-model
cutover, this tier IS the human-facing read surface: canonical module
queries serve dispatch alone (§5).
Code: `crates/kernel/indexer` (the host store: feed writer + guest converge),
`crates/kernel/index-guest` (the contract + authoring SDK; `testmap` is the
reference mapper), the chat/tasks/pages/inbox/saga modules' `src/index.rs`
(pure decision cores) + `src/index_guest.rs` (wasm shells, packaged by
`guest-builder --index`), `bin/noded` (the feed, the HTTP lanes, the shared
store construction with bundled guests), `bin/node` (the validator: live
feed, replay feed, boundary stamps).

## 1. Position: the derived tier

Ducktape state lives in two tiers with opposite contracts:

| | canonical tier | derived tier (this spec) |
| --- | --- | --- |
| substrate | qmdb / git — authenticated | fluent31 — ordered, scannable |
| in the root-hash | **always** (`Module::root()`) | **never** |
| cross-node | byte-identical by consensus | node-local, no determinism claim |
| reads | point lookups, module `query` | scans, search, partitions, views |
| consistency | exact at every height | **optimistic** — feed exact, views converge |
| crash story | replay / state-sync to the root | **rebuild** — replay the feed |

The canonical tier is deliberately not a database: `any::unordered` qmdb is
hashed keys and point lookups — no scans, no secondary indexes, no search.
Anything that needs those shapes is a *read model*, and read models are
derived: materialized from the finalized op stream, disposable by
construction, and invisible to consensus. Nothing a `root()` commits to may
ever live in — or be read back from — the derived tier.

NOTHING per-module is native on this tier. A module's mapper is wasm — like
its consensus logic — installed INSIDE its own index database and executed by
the engine. The node binaries hold zero per-module index code; that is what
lets an index-only node be a module-generic engine.

## 2. The two loops

The tier is two loops, coupled only through each module's database:

- **the host writer** (`IndexStore::apply_block`, called by the node's block
  loop): writes one borsh `OpRow` per applied dispatch under
  `op/{height:016x}/{seq:04x}` plus the watermark (`meta/height`), one atomic
  batch per module per block. No domain logic; this is the whole host side.
- **the fold** (the module's index guest): a fluent31 **changes-mode
  trigger** (`"fold"`) on the `op/` range delivers every committed op row to
  the guest's `on_apply` — exactly once, in commit order, value inline. The
  guest folds it into derived keys inside its own transaction: its writes and
  the event's consumption commit together (at-least-once invocation,
  exactly-once effects). Trigger-made writes never re-trigger; loops are
  impossible by engine construction.

The fold is ASYNC and OPTIMISTIC by design (the user's contract: "optimistic
now, consensus-consistent later"). The watermark vouches for the FEED alone —
"every block at or below H is fully in the feed" — and EVERY module's
watermark advances on EVERY applied block, so `watermark < H` always means
"blocks are missing", never "the module was quiet". Derived views trail the
feed by the trigger backlog, which is observable, never guessed:
`IndexStore::fold_status` (pending + last drain error) rides
`GET /v1/index/status` as `fold.{module}`.

Every module gets the feed, the watermark, and the backfill floor
(`meta/backfill`, §6) with zero module code. A module becomes **indexable**
by shipping an index guest.

## 3. The index guest

A module's mapper is a fluentabi module (core wasm32) installed under the
name `index` in the module's own database, with up to two roles:

- **fold** (`on_apply` export) — consumes the feed as above;
- **view** (`query` export) — the module's read projection, served read-only
  at one MVCC snapshot (`POST /v1/index/{module}/view`).

Ship both, either, or none: a fold-only guest maintains scan-served keys; a
module with no `query` role answers `ViewUnsupported` (§5). The store
converges each database on the bundled artifact at open — install
(overwrite-put; replacing bytes IS the upgrade path), trigger registration
when the guest folds, teardown of both when a module stops shipping one. A
converge marker (`meta/guest`: artifact hash + roles) written last makes a
warm boot free — a matching marker skips every wasm compile.

Because the guest lives in the database's engine keyspace, **code travels
with the data**: a shipped index archive (§7) carries its mapper, and no
wipe this tier performs can touch it.

### 3.1 Authoring shape: decide pure, write thin

A mapper is two files in the module crate:

- `src/index.rs` — the DECISION core, compiled everywhere: pure
  `fold_op(&OpRow, &impl StateRead) -> Result<Writes, Fail>` and
  `serve_view(&impl StateRead, &[u8]) -> Result<Vec<u8>, Fail>` over the
  `index_guest` contract crate (borsh `OpRow`, `StateRead` reads + paging,
  `Writes` command lists, the `search` posting helpers). Unit-tested
  natively against a plain `BTreeMap` — the map backend and `apply_to_map`
  are the test twins of the engine backend.
- `src/index_guest.rs` — the SHELL, behind the crate's `index-guest`
  feature: backs `StateRead` with the engine ABI (`EngineRead`), applies the
  decided writes, and exports the roles via `index_guest::fold!`/`view!`.
  The whole file is ~15 lines; `guest-builder --index` packages it into the
  committed `index.wasm` (`make wasm-modules` refreshes, `wasm-modules-check`
  guards presence).

Within one op a read never sees that op's own writes (they apply after the
decision); across ops in one feed batch it sees everything earlier — the
engine transaction and the native test harness agree on this by
construction. The decision core depends on exactly two internal things: the
`index_guest` contract crate and the module's crate-root wire types — never
the module's state machinery, never `sdk`/`host`/`indexer`.

### 3.2 Fold rules

1. **Applied ops only, all of them.** The feed carries exactly the
   dispatches consensus applied — root ops *and* follow-ups — in drain
   order. A failed op aborts its whole block and never reaches the feed.
   Mirror module semantics on that assumption (`tasks` files every
   `CreateTask` as `Open` because a duplicate create would have aborted;
   `chat` mirrors `head_seq` because every applied post assigned exactly the
   next sequence).
2. **Deterministic data-in/data-out.** No IO beyond the module's own index,
   no clock, no randomness — the fluentabi runtime enforces most of this
   (no WASI, no imports beyond the engine ABI, canonicalized floats, fuel).
3. **Fail loudly, never guess — but know what failure means.** A fold
   `Fail` holds the module's OWN queue: events are retained, the engine
   backs off and retries, `fold_status` surfaces depth and reason. It no
   longer poisons the store, and it no longer touches other modules. That
   cuts both ways: interface drift (an undecodable APPLIED op) should fail —
   the queue holds until a fixed guest ships, then folds on. But a
   DETERMINISTIC failure wedges the fold forever, so a mapper whose view is
   not worth a wedged queue (saga's billing ledger) documents skip-and-
   continue instead. Choose per mapper; write it down.
4. **Reserved namespaces.** `op/` and `meta/` are host-written; the trigger
   range spans `op/` alone, so bookkeeping writes never reach the guest.
   Guests must not write into either (nothing enforces it engine-side — the
   prefixes are ordinary user keys there; the contract is this spec).
5. **Pre-index history is out of scope.** An op referencing state the feed
   never carried (enabled mid-life, boundary stamp) folds to a no-op. The
   honest fix is replaying the chain through the feed, not a guessed
   backfill.

### 3.3 View rules

1. **Module-defined wire, like `Module::query`.** Request and reply are the
   module's own JSON shapes (externally tagged enums by house convention);
   the daemon and the store treat both as opaque bytes. A guest `Fail`
   surfaces as the view error, message intact.
2. **One snapshot per call.** Every read of a view invocation sees the same
   MVCC snapshot, concurrent with both writers.
3. **Read-only.** The engine rejects writes in a `query` invocation
   (`EROFS`), and the role check happens at the boundary.
4. **Views may be absent.** `ViewUnsupported` is a first-class answer, not a
   failure — see §5.

## 4. The endpoints (noded)

Reads never cross the node-actor lane; they run on the HTTP runtime against
snapshots, like the blob and telemetry lanes.

| route | serves |
| --- | --- |
| `GET /v1/index/status` | per-module watermarks, backfill floors, fold health (backlog + last error), the poison flag |
| `GET /v1/index/{module}/ops?after=&limit=` | the feed, paged — borsh rows projected to the JSON envelope (`payload` when the payload is JSON, `payload_hex` otherwise) |
| `GET /v1/index/{module}/scan?prefix=&after=&limit=` | raw derived keys (debugging, generic consumers) |
| `POST /v1/index/{module}/view` | **the module's own endpoint** — its guest's `query` role |

A module with no guest (or a guest with no `query` role) answers 404 on
`view`.

## 5. The substrate doctrine — where reads live

The split is a DOCTRINE, not a per-module judgment call:

- **canonical state is authenticated point-read state, nothing more.** qmdb
  is used for what it is — hash-addressable records behind a merkle root.
  A module's canonical `query` surface exists FOR dispatch: the point and
  computed reads other modules' `execute()` paths (and the host) consume.
  Consensus can never read the derived tier, so those reads stay canonical
  by necessity — and they are the ONLY reads that do.
- **no scan machinery in canonical state.** stored enumeration lists,
  stand-in range-index records, and counter-driven pagination that exists
  only to serve a human surface are defects: the engine on this tier
  iterates natively, so that is where iteration lives. (a record dispatch
  itself needs — pages' folder-parent map for cycle checks, chat's bounded
  per-message emoji index for caps and tombstone cleanup — stays canonical:
  it serves a consensus decision, not a scan.)
- **everything a human lists, scrolls, or searches is a view here.** the
  UI's read surface is `/v1/index/{module}/view`, uniformly — chat pages
  and threads, the pages sidebar and comment panels, boards, feeds, search.
- **modules hold no state in RAM.** the in-memory snapshot-bytes cohort is
  a transitional shape scheduled for re-platforming onto qmdb + this tier
  (phase 2+, in dependency order: small registries first, the hot
  consensus-loop modules — valset, dispatch, saga, runs — last).

Exemptions are substrate facts, not preferences: forge's state *is* a git
repo (cloneable, greppable — an index would be a worse second copy), and
files' chunk bytes bypass consensus (no op stream to fold). A bounded
typed registry whose canonical reads are all dispatch-consumed (identity,
valset) simply has no read model to move.

The shipped mappers:

| module | read model | dispatch reads kept canonical |
| --- | --- | --- |
| chat | channel list, message pages, threads, revisions, reactions, members, `search` + `tags` | `Channel`, `MessagesRange` (agent context windows), `Message` (id probes) |
| pages | page list, per-target comment threads, `search` | `GetPage`/`GetBlock`, `CommentThread`/`GetComment`, `TargetThreadCount` (cap probe) |
| tasks | task `by_status` pages, job pages + census | task `List` (unpaged, dispatch-consumed), `JobsQuery::Get` |
| inbox | per-member notification pages, unread counts | none — nothing in consensus reads an inbox |
| saga | `usage` — the executor billing ledger | `Get`, `NextExpiry`, `AssignedPending` (all host/dispatch-required) |

## 6. Rebuild and failure story

There is ONE derivation path: the feed. When canonical state advances
WITHOUT the op stream — state-sync installs a boundary, an index directory
is wiped, recovery skips re-executing durable blocks — the module is stamped
BACKFILLED at the boundary (`mark_backfilled`): trigger torn down first
(discarding pending events — a wipe's deletes must never reach the guest as
feed traffic), watermark dropped, user keys cleared (the engine keyspace,
guest included, is invisible to the sweep and survives), watermark + floor
stamped, trigger re-registered. Its feed and views honestly BEGIN there,
visibly via `meta/backfill`; history below a boundary re-enters only by
replaying blocks through the feed or adopting a shipped archive (§7).
The former from-state rebuild lane (mappers re-deriving rows from canonical
`Module::query` state) is deleted with the native mappers: one fold path,
no second derivation with its own degradation matrix.

- The index directory stays disposable: delete `<storage>/index` (or one
  module's subdirectory) and the tier heals — boundary stamps at the next
  boot, content accruing from new blocks behind a visible floor.
- noded resumes its local block counter **above** the index watermark so
  feed heights stay monotonic across restarts; at startup it stamps any
  module whose watermark trails the resume floor.
- The consensus validator feeds the identical contract from three sources:
  the live drain (every SEALED frame — a rejected frame feeds empty, it
  still consumed its height), the journal replay, and post-reboot frame
  catch-up; whatever they cannot reproduce converges on the boundary stamp.
- A standing RESIDENT feeds like a validator: the replica fold driver folds
  finalized frames (unified-node phase 2) and applies the per-block index
  fold from their dispatches. Boundary stamps fire only where state jumped
  WITHOUT frames — the join bootstrap and backfilled heights — and the
  blocks database gains one honest boundary row there
  (`IndexStore::apply_block_record`).
- **Host-write failures poison** (writes refuse, reads keep serving,
  remedy = rebuild). **Guest-fold failures never poison** — they hold that
  module's queue, observably (§3.2 rule 3). Two failure domains, two
  observables, one status route.
- Durability is `SyncMode::Periodic` (bounded loss window, torn tails
  truncate on recovery) — correct for a tier whose worst case is a rebuild.

## 7. State-sync: index-archive shipping

A joiner state-syncs **state**, not op history, so the feed has nothing to
carry — and a synced node with empty views renders its modules poorly. The
shipped answer is the archive lane (the former lane 2, now the only lane):

fluent31 fork archives are complete database directories; a source node cuts
one per module database plus `_blocks` (`IndexStore::checkpoint_files` —
fork, read into memory, fork deleted) and ships the file sets alongside
state-sync. The joiner stages them under `<storage>/index/_staging` (every
file fsynced, a `.complete` marker LAST) and the promoted reboot's
`IndexStore::open` adopts the staging directory before any engine open — a
torn fetch is discarded, never adopted. The archive carries rows, watermark,
floor, AND the installed guest + its trigger/queue state, so a shipped index
resumes folding mid-stream on the joiner.

Contents are NOT root-verifiable (the derived tier has no root by design),
so the lane trusts the serving node — accepted, because the read model is
how a node renders at all, and a joiner already trusted its sync source
enough to join through it. The lane is ON by default (`sync_index` in
node.toml, node-local operator policy); `sync_index = false` opts a node
down to consensus-only — it boundary-stamps instead (§6): views begin at
the boundary, exact and honest. Shipped watermarks land at the source's fold tip, so a module whose
watermark reaches the joiner's boundary skips its stamp (warm), and
anything stale, missing, or refused falls to `watermark < boundary` and
stamps exactly as if nothing was shipped. The blocks database rides along
verbatim — the only path by which a joiner ever gets pre-boundary
`/v1/blocks` history.

## 8. The index-only node (direction)

The tier's shape is chosen so an index-only node is a module-generic
engine: block stream in (it must be able to READ applied dispatch traces
from the wire — the one open spec item), per-module databases with their
guests installed, generic feed writes, generic view/scan/ops routes out.
Zero per-module native code; mapper upgrades arrive as data (the module-code
registry distributes index guests exactly like consensus components). That
node is fluent31's serving surface plus this crate's feed — nothing else.
