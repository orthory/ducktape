# duckfs — the files module as a reproducible execution filesystem

**Date:** 2026-07-06
**Status:** approved (brainstorm complete; implementation plan next)
**Supersedes:** the CAS manifest-registry files module (`crates/apps/files`) and the
`memory` module (`crates/apps/memory`), both deleted flag-day on fresh genesis.

## Summary

Rebuild `files` as **duckfs**: a consensus-replicated, copy-on-write,
content-addressed filesystem whose product role is reproducible execution, not
generic storage. Every node holds every byte as consensus state (full
replication — bytes travel through blocks). CoW snapshots define byte-exact
inputs for sandboxed workloads; commits define atomic output diffs; the history
answers what a job saw, what it changed, and how to reproduce or audit it. A
checkout/commit engine materializes subtrees onto the real OS filesystem. The
`memory` module's filesystem-shaped verb
surface (ls / stat / read / find / grep, watches, generations, citable URIs) is
absorbed into duckfs and `memory` is deleted.

Naming: the wire module id stays `files`; the filesystem is called **duckfs**
everywhere humans see it (mount type, `.duckfs/` working-copy dir, CLI, docs).

Product boundary:

- `forge` owns code repositories, Git history, branches, review workflows, and
  smart-HTTP clone/fetch/push. duckfs may hold artifacts referenced by a repo,
  but it is not a Git substrate.
- `jobs` owns scheduling, claiming, retry, and runner orchestration. duckfs owns
  the byte-exact workspace state those jobs consume and produce.
- FUSE, SMB, NFS, and WebDAV-style access are compatibility adapters over a
  checkout or pinned snapshot, not the product identity. A read-write adapter is
  a working-copy editor followed by a duckfs commit, not a coherent shared disk.
- Git-LFS-style use is a pointer/manifest pattern: a forge repo may reference
  duckfs snapshots or objects for large artifacts, but duckfs stays the
  execution evidence layer rather than becoming a blob CDN.

Sharding and KZG-style data-availability commitments are **out of scope** for
this wave, but the object model is chosen so that adopting them later is a
replication-policy change, not a schema change.

## Motivation and alternatives considered

duckfs exists for the **sandbox/agent-workspace bet**, not file storage per se:
(1) the `jobs` module lets any node claim work — a replicated FS with pinned
snapshots is what makes "claim anywhere, get byte-identical inputs" true;
(2) today's module commits promises, not bytes — a committed manifest can be
unreadable on every node, and all bytes die on restart; (3) agent work needs
audit and rollback — snapshots + diffs answer "what did the sandbox see, what
did it change, revert it"; (4) the repo already committed to
agents-want-filesystems (memory's NoKV philosophy), currently split across two
half-filesystems.

Alternatives weighed:

- **Mandatory replication on the existing CAS module** — the honest "do less"
  option: persist the blob store, require every node to fetch committed
  chunks. Fixes availability and restart-loss cheaply, but yields none of the
  filesystem semantics (mutation, directories, atomic multi-file commits,
  snapshots, reproducible checkouts). Correct choice only if the sandbox bet
  is abandoned.
- **External object store (MinIO/Garage/SeaweedFS)** — a second service to
  operate (breaks the one-binary thesis); no atomic cross-file commits or
  snapshots; contents can't be attested by the app-hash.
- **Syncthing-style sync** — eventual consistency with conflict-copies;
  cannot be consensus state or pin reproducible inputs.
- **Git/forge as the substrate** — rejected: sha1, packfile behavior on
  multi-GiB binaries, no empty dirs, libgit2 on a hot consensus path. Forge
  remains the answer for code repos specifically.
- **IPFS/iroh + pinning** — imports a foreign DHT/networking model alongside
  ducktape's direct peering, yet the consensus-anchored namespace, snapshots,
  refs, and authority (the hard part) would still have to be built here;
  availability would rest on pinning policy, not consensus-verified
  possession.
- **NFS/NAS as the product shape; CRDT filesystems** — single point of
  failure, coherence/lock semantics pull the design toward mutable shared disk,
  and merge semantics are meaningless for arbitrary bytes. duckfs can expose
  adapters, but its core is snapshot inputs plus committed output diffs.

Every off-the-shelf option either cannot be consensus state or solves only the
storage half while leaving the namespace/snapshot/authority layer to us
anyway. The one genuine competitor is the first bullet, and choosing duckfs
over it is the product decision that sandboxed agent workspaces are real.

## Decisions (settled during brainstorm)

| Question | Decision |
| --- | --- |
| History model | CoW snapshots (immutable, git/ZFS-like); no in-place mutation |
| Relationship to `memory` | duckfs absorbs it; `memory` deleted in the same wave |
| Size envelope | Disk-limited — no designed total ceiling; caps protect liveness, not capacity |
| Wave scope | Consensus module + checkout/commit engine (FUSE mount cut in the 2026-07-13 revision below) |
| Write authority | Owner-gated home subtrees: `/home/<owner>/**` (owner only), `/shared/**` (any member), system writes anywhere |
| Name | duckfs (module id `files` unchanged) |
| Product identity | Reproducible execution filesystem: snapshot inputs, atomic output diffs, citable history; not NAS, Git, or a generic blob store |
| Operation layer boundary | Runners execute outside the consensus hot path against pinned snapshots; duckfs records inputs, outputs, diffs, and operation receipts through ordinary commits |

## Execution filesystem framing

duckfs treats a file tree as the boundary of work. A job or agent receives a
pinned snapshot as its input workspace, runs in a sandbox owned by `jobs` /
runner code, and returns a commit against that base. The commit message and/or
result files can carry the operation id/version, runner identity, logs, and
receipt hashes, but the consensus module does not run expensive
transformations itself. Consensus verifies bytes, paths, authority, per-path
CAS, and object possession; execution stays outside `execute`.

This keeps the "storage plus operations" idea without turning duckfs into a
second VM inside every validator. The operation layer is
file-native rather than KV-native: `stat`, `read`, `ls`, `find`, `grep`, `diff`,
and commit receipts are the substrate. The durable fact is not "a remote
process ran"; it is "this snapshot was transformed into that snapshot by this
declared operation, with this auditable diff."

## Determinism boundary (the rule everything hangs on)

The consensus surface (`execute → state → root()`) never touches the OS
filesystem's semantics and never depends on host libraries: the tree is our own
pure-Rust data structure; any library on that path is vendored and
version-pinned into the node binary (the forge/libgit2 precedent); disk
persistence sits *below* the root computation (the root hashes logical
content, never storage layout — the kv/qmdb precedent).

OS-specific code (checkout/commit materialization) lives strictly on the
client side of the wire. The only thing that crosses the wire is an op with
explicit bytes and canonical paths, applied identically by every validator.
Cross-OS quirks (APFS case-insensitivity, NFD normalization, mtime
granularity) can therefore affect only a node's local working copy, never
consensus state.

Boundary rules that make this hold:

1. **Canonical paths in consensus:** UTF-8, NFC-normalized (non-NFC rejected at
   execute), `/`-separated, no empty / `.` / `..` segments.
2. **Consensus metadata is minimal and ours:** size, content hash, exec bit,
   author, `consensus_time` + height. Never OS uid/gid/mode/mtime.
3. **Bytes are bytes:** no line-ending or encoding translation anywhere.

## Object model (consensus state)

Four content-addressed, immutable object kinds; id = `sha256(kind_tag ‖
canonical_encoding)` (the kind tag domain-separates the hash):

- **Chunk** — raw bytes; fixed chunk size **1 MiB** (a network constant,
  replacing the old per-manifest `chunk_size`). The existing
  digest-plus-exact-length verification discipline (`verify_chunk`) carries
  forward as the object integrity check.
- **File** — ordered chunk-id list + total size + optional meta map
  (≤16 entries, key ≤64 B, value ≤256 B — memory's caps; powers conventions
  like `kind=skill`). A symlink is a File holding the target string, with the
  symlink kind on the tree entry.
- **Tree** — a directory: sorted `name → (kind: file|dir|symlink, object_id,
  exec_bit, size)` entries. Empty trees are legal (empty dirs exist).
- **Snapshot** — a commit: root tree id, parent snapshot id, origin-derived
  author, `consensus_time`, height, message (≤4 KiB).

Mutable state is small and is exactly what `root()` commits (sha256 over its
canonical encoding — forge's root-over-HEADs pattern):

- **live ref** — the current head snapshot id;
- **pin table** — named pinned snapshots (`Pin`/`Unpin`);
- **history window** — the last 1,024 snapshot ids (ring);
- **staging table** — uploaded-but-uncommitted chunk digests with owner,
  byte count, and expiry height;
- **watch set** — `(prefix, module_id)` pairs.

CoW and dedup are structural: unchanged subtrees are shared by hash across
snapshots; identical bytes are stored once cluster-wide.

## Namespace and authority

- Two writable roots: `/home/<owner>/**` — writable only by the matching
  origin-derived owner (`owner_of` reused verbatim: module id, `ext:<hex>`,
  `system`); `/shared/**` — writable by any member. The system origin writes
  anywhere (genesis seeding). All other top-level paths reject writes.
- Reads are cluster-visible everywhere: **`/home` gates writes only**. Secrets
  do not belong in duckfs (vaults remain the secrets surface).
- The skills convention moves to `/shared/skills/`.
- Case-only-different sibling names are legal in consensus; materialization on
  case-insensitive filesystems fails loudly (client-side rule).

## Verb surface

### Write ops

- **`PutBlob`** — one chunk of raw bytes into the staging table. Binary op
  frame (not base64 JSON) — explorer payload previews already render binary
  lossily by contract. Staged bytes are quota'd at 1 GiB per owner and expire
  4,096 blocks after upload (expiry executes deterministically in
  `commit_block`; the staging table is part of the root, so expiry is
  consensus). Staging is keyed by digest, so an interrupted upload resumes by
  skipping already-staged chunks.
- **`Commit { base_snapshot, message, changes, inline_bytes }`** — the atomic
  unit. `changes` is ≤4,096 path entries (`Put`, `Mkdir`, `Rm`, `Mv`,
  `Symlink`); chunk references resolve against the staging table or
  already-present odb objects (dedup); files totaling ≤256 KiB may carry bytes
  inline and skip staging. Concurrency is **per-path compare-and-swap**: every
  changed path must be unchanged between `base_snapshot` and the live head,
  else the whole commit rejects (client rebases). Disjoint concurrent commits
  from different sandboxes interleave without false conflicts. A successful
  commit appends a Snapshot, advances the live ref, and fans out watch
  notifications atomically with the triggering op (memory's P2 pattern).
- **`Pin { snapshot_id, name }` / `Unpin { name }`** — ≤1,024 pins total; a
  pin records its origin-derived owner and only that owner (or system) may
  `Unpin` it. Only snapshots resolvable from the history window, pins, or the
  live head may be pinned (a GC'd snapshot id cannot).
- **`Watch { prefix, module_id }` / `Unwatch`** — module-to-module prefix
  watches; `Unwatch` is gated to the module that registered the watch.

Authority applies to every changed path in a commit — including both the
source and destination of a `Mv` — under the `/home` / `/shared` rules above.
`base_snapshot` must be resolvable (live head, history window, or pinned);
a commit against a GC'd base rejects and the client re-checks out.

All caps are enforced at execute time with rejection, so an oversized value
never enters the root preimage (unchanged house discipline).

### Read queries (committed state; all snapshot-addressable)

`Stat`, `Ls` (cursor-paged), `Read { path, offset, len }` (byte-range),
`Find`, `Grep` (budgeted scan, cursor-paged), `History`,
`Diff { from, to, prefix }`, `Refs`. Query pages clamp at 256 entries with a
cursor for continuation (fixes the old un-paginatable `List`). Grep hits carry
`duck://files/<path>@<snapshot>#L<line>` evidence URIs (continuing memory's
citable-URI scheme with `memory` → `files`).

## Disk substrate and durability

- The object database (odb) is a loose-object store under the node data dir:
  `objects/<aa>/<hash-rest>`, one file per object, written
  `tmp → fsync → rename`. No new dependencies; streaming-friendly. Bounded
  in-memory read cache for hot objects.
- Mutable state (refs, pins, window, staging index, watches) lives in one
  small **refs file** rewritten atomically per block, stamped with the height.
- duckfs joins the **disk cohort** of the existing torn-commit recovery
  discipline: `commit_block` writes new objects (content-addressed ⇒
  idempotent re-writes), fsyncs, then atomically writes the refs file. At boot
  the module reports its durable height from the refs file; kernel WAL replay
  re-applies later frames idempotently. Crash mid-write leaves tmp files
  (swept at boot) and the previous refs file.
- Net change vs today: **bytes survive restart** because bytes are consensus
  state with per-block durability, not an in-memory side-store.

## State sync and self-healing

`state_sync_handle()` returns `ResolverBacked { backend: "duckfs-odb" }`.
Joiner flow:

1. Install the refs snapshot (small), verified against the module root.
2. Walk reachability from live head + pins + history window; fetch missing
   objects over `serve_sync` (`GetObjects { ids } → objects`, batched). Every
   received object re-hashes against its id before adoption — the existing
   dishonest-server-proof fetch lane generalized from chunks to all object
   kinds.
3. Report ready only at **full possession** of every reachable object. That is
   the operational meaning of "fully replicated."

The same fetch lane doubles as self-healing: a node detecting a missing or
corrupt reachable object re-fetches from peers.

## Garbage collection

Reachability roots: live head + pin table + history window. Snapshot parent
pointers are metadata, **not** GC edges — history older than the window
survives only if pinned. Every 1,024 blocks, at a deterministic height, each
node mark-sweeps unreachable object files locally. Because `root()` covers
refs only and reachability is identical on every node, the sweep is
consensus-neutral by construction; dropping unreachable objects can never move
the root.

## Deletions and integration changes (flag-day, fresh genesis)

- `memory` module deleted. Verb mapping: `Publish` → `Commit` (a snapshot is a
  generation); `(path, generation)` → `(path, snapshot)`;
  `Snapshot`/`DropSnapshot` → `Pin`/`Unpin`; watches carry over; per-file meta
  moves onto the File object; `duck://memory/...` → `duck://files/...`.
- Old files wire deleted: `AddManifest` / `RemoveManifest` / `Stat`+`List`
  (old shapes) / `FilesSyncReq::GetChunk`.
- `BlobHandle` (op-receipt store) moves into noded — it was always a daemon
  concern. `/v1/files/blob` endpoints are replaced by duckfs HTTP endpoints
  (stage, commit, read, ls, history) that wrap ops/queries.
- Registration sites updated: bin/node MODULE_IDS / genesis / joiner arrays,
  bin/demo counts, noded module registration. `memory` out; `files` (duckfs)
  in. Fresh genesis; no migration.
- App frontend: memory views deleted; FilesView rebuilt on duckfs verbs
  (browse at snapshot, upload, history); TS client (`files-client.ts`)
  rewritten to the new wire.

## Modularization and wasm readiness

duckfs ships as ONE crate — `files` — whose pure core is enforced by a cargo
feature rather than a crate boundary:

- **`crates/apps/files`, always-compiled core** (the wasm migration unit):
  wire types, objects, canonical paths, refs/root, CoW tree engine, the
  `Fs<S>` state machine, queries, GC marking, and the two I/O traits it is
  generic over — `ObjectStore` (put/get/has/remove/list) and `RefsStore`
  (load/save of the refs envelope) — with `MemStore`/`MemRefs` in-memory
  implementations (tests today, the wasm default tomorrow). This half
  depends only on sha2/serde/serde_json/base64/unicode-normalization: no
  `std::fs`, no sdk, no async.
- **`crates/apps/files`, `native` feature (default-on)**: `DiskStore`
  (loose-object odb, tmp→fsync→rename, verified reads, tmp sweep),
  `DiskRefs` (atomic refs file with height/gc-watermark envelope), and the
  sdk glue — the `Files` type implementing `sdk::Module`, origin→owner
  mapping, watch-notification emission. The sdk/async-trait dependencies are
  optional and enabled only by this feature.
- **`crates/apps/files`, `native` feature, `cap.rs`**: the fs capability over
  the module-injected interface (below) — `files::FsCap`.

The wasm-readiness proof is a standing build gate, not a promise:
`cargo check -p files --no-default-features` must stay green — it compiles
exactly the future wasm surface and fails on any accidental `std::fs`/sdk
leak into the core.

## The fs capability (module-injected interface)

Modules gain a typed filesystem capability through the interface they already
receive, with no new host machinery. It ships as `files::FsCap` (native
feature): the capability is injected via `Ctx` — the interface every module
already holds — and consumer modules take a normal dependency on the files
crate, the same way the capability module depends on valset:

- **Reads** are deterministic during execute today via `Ctx::query` routed to
  the files module. `files::FsCap<'a>` wraps `&'a dyn Ctx` with typed
  methods — `stat`, `ls`, `read`, `find`, `grep`, `history`, `refs` — that
  encode/decode the files wire internally. Committed state as of dispatch
  start, same as any cross-module query.
- **Writes** are write-intents, matching the existing `emit_msg` contract:
  `FsCap::commit(...)`, `pin`, `watch` build the corresponding `FilesMsg` and
  emit it as a follow-up op (never executed reentrantly). The follow-up's
  origin is the emitting module, so `/home/<module-id>/**` authority applies
  naturally.
- Watch notifications (`duckfs_notify`) complete the loop: a module registers
  a watch through `FsCap::watch` and receives change ops on its own execute
  path.

Because `FsCap` is pure sugar over `Ctx`, it moves unchanged onto whatever
Ctx-shim a future wasm module ABI provides; today it rides the same `native`
feature as the rest of the sdk-facing surface.

## Client stack

### Checkout/commit engine (`duckfs-client`)

Rust crate shared by CLI, daemon, and sandbox runner; TS mirror for the app.

- `checkout(prefix, snapshot?)` materializes a subtree into a real directory
  with a `.duckfs/` index (base snapshot; per-path hash/size/mtime — git's
  index discipline).
- `status`/`diff`: mtime+size fast path, rehash on suspicion.
- `commit`: chunk changed files; `PutBlob`-stage only chunks the cluster lacks;
  submit one atomic `Commit` with per-path CAS against the base. On conflict:
  refetch head; auto-rebase if the touched paths are upstream-untouched;
  otherwise fail with a conflict report. No silent merges, no last-writer-wins.
- Daemon RPC: sandbox lifecycle — create-workspace (managed checkout under the
  node data dir), commit-and-close. This is the seam the jobs/agent modules
  use to give a sandbox a reproducible, isolated workspace on any node.
- CLI: `ducktape fs ls|cat|stat|checkout|status|commit|history|diff|pin`.

### Mount surface — CUT (2026-07-13 revision)

The FUSE `mount` verb was built to smoke level in wave 1 (`bin/fs`, opt-in
`fuse` feature, e2e at e7b4e1d1) and REMOVED in this revision; no
watch/auto-commit daemon replaces it. Both consumers a mount would serve are
already covered without kernel surface: agent workspaces get explicit
checkout → run → commit (run boundary = commit boundary, which is better for
reproducibility than background auto-commit snapshotting mid-run noise), and
human sharing goes through FilesView/HTTP upload. Lazy reads buy nothing
under full replication (bytes are already local), and macFUSE's kext
approval flow is a product-killer on macOS. The `NodeApi` trait seam in
`duckfs-client` stays — it is the adapter point if genuine mount demand ever
appears (NFS loopback or FSKit before resurrecting FUSE; a folder-sync watch
daemon is a ~day of work on top of the existing scan/status/commit engine).

## Caps (execute-time rejection)

| Cap | Value |
| --- | --- |
| name | ≤ 255 bytes |
| path | ≤ 4,096 bytes, depth ≤ 128 |
| directory entries | ≤ 65,536 |
| chunk size | 1 MiB (fixed network constant) |
| inline commit bytes | ≤ 256 KiB |
| changed paths per commit | ≤ 4,096 |
| commit message | ≤ 4 KiB |
| file meta | ≤ 16 entries; key ≤ 64 B; value ≤ 256 B |
| staging quota | 1 GiB per owner; TTL 4,096 blocks |
| pins | ≤ 1,024 |
| history window | 1,024 snapshots |
| GC period | every 1,024 blocks |
| query page | ≤ 256 + cursor |
| chunks per file | ≤ 4,194,304 (2²²) ⇒ max file size 4 TiB |

## Operational realities (read before being surprised)

1. A working copy is not a shared disk: writes are invisible cluster-wide
   until commit; concurrent same-path edits meet as commit conflicts. duckfs
   is an execution workspace with commit discipline, not NFS.
2. Ingest speed is consensus speed: capacity is disk-limited but throughput is
   block-limited; every uploaded byte is paid by every validator's disk and
   bandwidth. Reads are local and free. (Sharding/KZG later changes exactly
   this.)
3. Deletion is eventual: `rm` removes a path from the live tree, but bytes
   remain fetchable until the referencing snapshots leave the history window /
   pins and GC sweeps on every node.
4. History is bounded unless pinned: snapshots older than the 1,024-window are
   GC'd. Fully replicated ≠ kept forever.
5. Case-insensitive filesystems (macOS) cannot materialize case-colliding
   siblings; checkout fails loudly by design.
6. Overlapping concurrent edits reject; auto-rebase covers disjoint work only.

## Testing

- **Module:** validation tables (path canonicalization incl. NFC rejection,
  authority gating, per-path CAS conflicts, staging quota/expiry, cap
  boundaries); GC-never-drops-reachable property test; snapshot/install
  round-trips; root determinism across encode/decode.
- **Kernel integration:** torn-commit recovery for the disk cohort; restart
  e2e proving bytes survive reboot; statesync joiner e2e reaching full
  possession; app-hash continuity across all of the above.
- **Client:** checkout/commit round-trip; conflict + auto-rebase + genuine
  conflict report; cross-OS path edges (NFC forms, simulated case collision).
- **App:** FilesView + files-client vitest suites.

## Out of scope (deferred, seams reserved)

- **Sharding / KZG data-availability commitments:** chunks are already
  content-addressed leaves; "which nodes must hold which leaves" becomes a
  replication-policy change later. Full replication is the wave-1 policy.
- **Content-defined chunking (FastCDC):** fixed 1 MiB chunks for wave 1;
  CDC would improve dedup for in-place edits of large files.
- **Auto-merge of conflicting commits.**
- **Partial replication / attestation:** if some content should NOT live on
  every node, the answer is the pointer pattern first (a small consensus
  manifest referencing node-local bytes, the forge-objects precedent);
  possession attestation only if that proves insufficient.

## Revision 2026-07-13 (post-review deltas)

Settled in the storage-plane review; these amend the sections above.

- **FUSE mount: cut** (see Mount surface) — the wave-1 smoke-level
  implementation is deleted from `bin/fs`. A folder-sync watch daemon is also
  not built — demand-gated, cheap to add later on the existing client engine.
- **Prerequisites for exposing duckfs beyond a trusted team network** (not
  wave-1 blockers, but named so nobody ships public without them):
  1. **Auth on the noded `/v1/files` lane.** Everything rides
     `DEFAULT_ORIGIN` today; the lane must authenticate callers and stamp the
     real origin (reuse the agent session-key plane) before `/home/<owner>`
     authority means anything over HTTP.
  2. **Per-owner committed-storage quota.** Staging has a 1 GiB/owner cap but
     committed state has none; an owner can grow the replicated tree without
     bound. Enforcing a committed-bytes budget per owner at execute time
     changes op admission = **flag day** (named here so it can ride the next
     scheduled one).
- **S3-shaped facade** over the existing module (PUT = one-change commit,
  GET = Read, LIST = Ls) is the sanctioned "replicated S3" surface — new
  endpoints only, no engine change.

## Implementation shape (for the planning step)

New/changed surfaces: `crates/apps/files` (single crate: pure core +
`native` feature incl. `FsCap`, per the modularization section), `crates/apps/memory` (deleted), a new `duckfs-client` crate, noded (blob-receipt store internalized; duckfs HTTP
endpoints; sandbox workspace RPC), bin/node + bin/demo registration sites,
kernel statesync resolver wiring for `duckfs-odb`, app TS client + FilesView,
docs (en+ko module pages). Branching per `.project/work.md`: worktree from
`origin/dev`, PR to `dev`.
