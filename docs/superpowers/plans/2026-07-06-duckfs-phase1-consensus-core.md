# duckfs Phase 1 — Consensus Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rewrite the files module as the duckfs consensus core — split as one `files` crate: pure wasm-ready core + default-on `native` feature carrying disk backend, sdk glue, and the typed `FsCap` module capability: a content-addressed CoW filesystem (chunk/file/tree/snapshot objects on a disk odb, root over refs) with PutBlob staging, atomic Commit with per-path CAS, pins, watches, snapshot-addressable queries, deterministic GC, and an object-fetch sync lane — crate green in the workspace, old CAS wire deleted flag-day.

**Architecture:** Pure state machine (feature-gated core of `files`: no std::fs/sdk/async) over `ObjectStore`/`RefsStore` traits; native disk backend behind them; immutable content-addressed objects on a loose-object store; all mutable state (live head, pins, history window, staging, watches) in one small `Refs` struct whose canonical encoding is the `root()` preimage; execute stages into a pending block buffer, `commit_block` persists objects + refs file atomically (disk-cohort discipline).

**Tech Stack:** Rust (workspace member), sha2, serde/serde_json (wire), unicode-normalization (NFC), base64 (wire bytes), tempfile (dev).

**Spec:** `docs/superpowers/specs/2026-07-06-duckfs-real-filesystem-design.md` — binding. Read it before starting any task.

## Wave map (context, not tasks)

- **Phase 1 (this plan):** consensus core crate.
- **Phase 2:** node integration — registration sites, memory-module deletion, disk-cohort recovery wiring, statesync resolver, noded HTTP endpoints, restart/joiner e2e.
- **Phase 3:** `duckfs-client` checkout/commit engine + CLI.
- **Phase 4:** FUSE mount (feature-gated).
- **Phase 5:** app TS client + FilesView + docs.

## Global Constraints

- Work in the worktree `.claude/worktrees/feat+duckfs` (branch `feat/duckfs`, PR will target `dev`). All commands below run from the worktree root.
- Every commit: `git -c commit.gpgsign=false commit ...` (SSH signing hangs in this environment). End commit messages with `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.
- No backwards compatibility, no deprecation shims: the old files wire (`AddManifest`/`RemoveManifest`/old `Stat`/`List`/`GetChunk`) is deleted, not aliased.
- Every cap is enforced at execute time with **rejection** (`Error::Module`), so an oversized value never enters the root preimage.
- Determinism: no wall clock, no `HashMap` iteration order in any consensus path (use `BTreeMap`/`BTreeSet`), no OS-filesystem semantics in state (the odb is a byte bucket only).
- Comment style: lowercase, explain constraints not mechanics (match existing crate docblocks).
- Gates per task: `cargo test -p files` green, `cargo check -p files --no-default-features` green (wasm-purity gate, from Task 2 on), and `cargo check --workspace` green for tasks that touch anything outside the crate. `cargo fmt -p files -- --check` clean.
- Network constants (from spec, verbatim): chunk size **1 MiB** fixed; name ≤ 255 B; path ≤ 4,096 B; depth ≤ 128; dir entries ≤ 65,536; inline commit bytes ≤ 256 KiB; changed paths/commit ≤ 4,096; message ≤ 4 KiB; meta ≤ 16×(64 B,256 B); staging quota 1 GiB/owner, TTL 4,096 blocks; pins ≤ 1,024; history window 1,024; GC every 1,024 blocks; query page ≤ 256 + cursor; chunks/file ≤ 4,194,304 (2²²).
- Authority: `/home/<owner>/**` writable only by matching origin-derived owner (`Origin::Module(id)` → `id`, `Origin::External(b)` → `"ext:" + lowercase hex`, `Origin::System` → `"system"`); `/shared/**` writable by any origin; System writes anywhere; all other roots reject writes.

## REVISION D (binding; supersedes Revisions B/C): ONE crate, feature-gated purity

This revision **overrides file paths and some struct shapes in Tasks 2–15**
and adds Task 16. duckfs ships as ONE crate — `files` — with the pure core
(the future wasm unit) enforced by a cargo feature instead of a crate split.

**Crate layout:**

- `crates/apps/files` — everything. Cargo.toml:

  ```toml
  [features]
  default = ["native"]
  native = ["dep:sdk", "dep:async-trait"]

  [dependencies]
  sha2 = { workspace = true }
  serde = { workspace = true }
  serde_json = { workspace = true }
  base64 = { workspace = true }
  unicode-normalization = { workspace = true }
  sdk = { workspace = true, optional = true }
  async-trait = { workspace = true, optional = true }
  ```

  Always-compiled core modules (NO `std::fs`, NO sdk, NO async anywhere in
  them): `wire.rs` (the Task 2 `interface.rs` content verbatim, flattened at
  the crate root via `pub use wire::*`), `objects.rs` (Task 3), `paths.rs`
  (Task 4 — WITHOUT `owner_of`, which needs sdk::Origin and lives in the
  glue; core takes plain `actor: &str`), `store.rs` (Task 5 traits +
  `MemStore`/`MemRefs`), `state.rs` (Task 6 `Refs` + codec + `root_bytes`),
  `tree.rs` (Task 8), `fs.rs` (the `Fs<S>` state machine below), `queries.rs`
  (read side of Tasks 9/11/12), `gc.rs` (Task 13).

  Native-gated modules (`#[cfg(feature = "native")]`): `disk.rs` (Task 5 odb
  semantics as `DiskStore` + Task 6 refs-file envelope as `DiskRefs`),
  `cap.rs` (Task 16 `FsCap`), and `module.rs` (the `Files` type implementing `sdk::Module`, `owner_of`,
  error mapping `String` → `Error::Module("files: ..")`, watch-notification
  emission via `ctx.emit_msg`, GC watermark trigger in `commit_block`).
  `lib.rs` wires it together and re-exports so `files::Files`,
  `files::FilesMsg`, `files::ObjectStore` etc. all resolve.

- The fs capability (Task 16) is `files::FsCap` in `cap.rs`, gated by the
  same `native` feature (it wraps `sdk::Ctx`). No separate crate.

**Purity gate (add to every task's gates):**
`cargo check -p files --no-default-features` green. Any `std::fs`, sdk, or
async leak into a core module fails this gate.

**Core traits (in `files/src/store.rs`; exact signatures):**

```rust
pub trait ObjectStore {
    fn put(&mut self, kind: Kind, body: &[u8]) -> Result<ObjectId, String>;
    fn get(&self, id: &ObjectId) -> Result<Option<(Kind, Vec<u8>)>, String>;
    fn has(&self, id: &ObjectId) -> bool;
    fn remove(&mut self, id: &ObjectId) -> Result<(), String>;
    fn list(&self) -> Result<Vec<ObjectId>, String>;
}

pub trait RefsStore {
    /// None = fresh dir. Ok(Some((refs, height, gc_watermark))) otherwise.
    fn load(&self) -> Result<Option<(Refs, u64, u64)>, String>;
    fn save(&mut self, refs: &Refs, height: u64, gc_watermark: u64) -> Result<(), String>;
}
```

`MemStore` (`BTreeMap<ObjectId, (Kind, Vec<u8>)>`) and `MemRefs`
(`Option<(Refs, u64, u64)>` cell) live beside the traits.

**The core state machine (`files/src/fs.rs`):**

```rust
pub struct Fs<S: ObjectStore> {
    pub(crate) store: S,
    pub(crate) refs: Refs,
    pub(crate) pending: Option<Pending>,
}
pub(crate) struct Pending {
    pub refs: Refs,
    pub objects: Vec<(Kind, Vec<u8>)>,
    pub height: u64,
}
pub struct Notification { pub module_id: String, pub prefix: String, pub path: String, pub snapshot: String }

impl<S: ObjectStore> Fs<S> {
    pub fn new(store: S, refs: Refs) -> Self;
    pub fn root_bytes(&self) -> [u8; 32];             // committed refs only
    pub fn refs(&self) -> &Refs;
    pub fn putblob(&mut self, actor: &str, height: u64, bytes: &[u8]) -> Result<(), String>;
    pub fn commit(&mut self, actor: &str, height: u64, time: u64,
                  base: Option<String>, message: String, changes: Vec<Change>)
                  -> Result<Vec<Notification>, String>;
    pub fn pin(&mut self, actor: &str, snapshot: String, name: String) -> Result<(), String>;
    pub fn unpin(&mut self, actor: &str, name: String) -> Result<(), String>;
    pub fn watch(&mut self, actor: &str, is_module: bool, prefix: String, module_id: String) -> Result<(), String>;
    pub fn unwatch(&mut self, actor: &str, is_module: bool, prefix: String, module_id: String) -> Result<(), String>;
    /// returns (refs-to-persist, height) after flushing pending objects into
    /// the store; the CALLER (glue) persists via RefsStore then swaps refs in.
    pub fn commit_block(&mut self) -> Result<Option<(Refs, u64)>, String>;
    pub fn abort_block(&mut self);
    pub fn query(&self, q: FilesQuery) -> Result<FilesReply, String>;   // committed state
    pub fn serve_sync(&self, req: FilesSyncReq) -> Result<FilesSyncResp, String>;
    pub fn snapshot_refs(&self) -> Vec<u8>;
    pub fn install_refs(&mut self, bytes: &[u8], expected_root: [u8; 32]) -> Result<(), String>;
    pub fn missing_objects(&self, limit: usize) -> Result<Vec<ObjectId>, String>;
    pub fn ingest_object(&mut self, id: &ObjectId, kind: u8, body: &[u8]) -> Result<(), String>;
    pub fn gc(&mut self) -> Result<u64, String>;      // mark+sweep now; caller decides when
}
```

Expiry sweep runs at the top of `putblob`/`commit`/`pin`/`unpin`/`watch`/
`unwatch` (op-stream-driven determinism). The GC watermark/trigger policy
lives in the glue (`module.rs` `commit_block`) — per-node bookkeeping.

**Path remapping for Tasks 2–15 (binding):**

| Plan says | Build instead |
| --- | --- |
| `crates/apps/files/src/interface.rs` | `crates/apps/files/src/wire.rs` (flattened re-export unchanged) |
| `crates/apps/files/src/objects.rs` (Task 3) | same path; tests `crates/apps/files/tests/object_model.rs` WITHOUT the sdk harness (pure test — must also pass under `--no-default-features`) |
| `crates/apps/files/src/paths.rs` (Task 4) | same path minus `owner_of` (→ `module.rs`); pure tests |
| `crates/apps/files/src/odb.rs` (Task 5) | `store.rs` traits + Mem impls (pure, unit-tested) AND `disk.rs` `DiskStore` with the odb tests in `crates/apps/files/tests/disk.rs` |
| `crates/apps/files/src/state.rs` (Task 6) | same path (Refs + codec + root_bytes, pure); refs-FILE envelope → `DiskRefs` in `disk.rs`, its round-trip/corruption tests in `tests/disk.rs` |
| `crates/apps/files/src/tree.rs` (Task 8) | same path, pure; tests in-crate (unit) or `tests/tree_edit.rs` via the `testkit` facade |
| `exec_*` in `lib.rs` (Tasks 7/9/10) | `Fs::{putblob, commit, pin, unpin, watch, unwatch}` in `fs.rs` (pure); `module.rs` maps origin/env and emits notifications. Integration tests stay in `crates/apps/files/tests/` exactly as written (they exercise the sdk surface with default features). |
| `queries.rs` (Tasks 11/12) | same path, pure (`Fs::query`); glue delegates. Tests as written. |
| `gc.rs` (Task 13) | same path, pure (`Fs::gc`); watermark trigger in `module.rs`. Tests as written via `testkit::force_gc`. |
| `sync.rs` (Task 14) | core `Fs` methods above (no separate sync.rs needed — fold into `fs.rs`); glue exposes `Files::{snapshot_refs, install_refs, missing_objects, ingest_object, possession_complete, durable_height}` delegating. Tests as written. |

Gates: `cargo test -p files` + the purity gate above + `cargo check
--workspace` when anything outside is touched.

---

### Task 16: fs capability over the module-injected interface (`files::FsCap`)

Modules read duckfs through `Ctx::query` and write through `emit_msg` — this
crate makes that a typed capability (spec §"The fs capability").

**Files:**
- Create: `crates/apps/files/src/cap.rs` (`#[cfg(feature = "native")]`, re-exported as `files::FsCap`)
- Test: `crates/apps/files/tests/cap.rs`

**Interfaces:**
- Produces:

```rust
//! typed fs capability over the module-injected interface: reads are
//! host-routed committed-state queries; writes are emitted intents that
//! come back as follow-up ops under the EMITTING module's origin, so
//! /home/<module-id>/** authority applies naturally.

use crate::wire::*;
use sdk::{Ctx, Error, Msg};

pub struct FsCap<'a> {
    ctx: &'a mut dyn Ctx,
    target: String,
}

impl<'a> FsCap<'a> {
    pub fn new(ctx: &'a mut dyn Ctx) -> Self { Self::with_target(ctx, "files") }
    pub fn with_target(ctx: &'a mut dyn Ctx, target: impl Into<String>) -> Self;

    // reads (async — they ride Ctx::query)
    pub async fn stat(&self, path: &str, snapshot: Option<&str>) -> Result<Option<EntryInfo>, Error>;
    pub async fn ls(&self, path: &str, snapshot: Option<&str>, after: Option<&str>, limit: u64)
        -> Result<(Vec<EntryInfo>, Option<String>), Error>;
    pub async fn read_all(&self, path: &str, snapshot: Option<&str>) -> Result<Vec<u8>, Error>; // loops Read pages
    pub async fn grep(&self, pattern: &str, prefix: &str, snapshot: Option<&str>)
        -> Result<Vec<GrepHit>, Error>; // first page
    pub async fn refs(&self) -> Result<RefsInfo, Error>;

    // write intents (sync — they ride emit_msg)
    pub fn commit(&mut self, base: Option<String>, message: &str, changes: Vec<Change>);
    pub fn put_inline(&mut self, path: &str, bytes: &[u8], message: &str); // sugar: one-file commit vs live head (base = None is WRONG here — use refs().head first at call sites that need CAS; this sugar sends base: None + single Put and is documented as create-only)
    pub fn pin(&mut self, snapshot: &str, name: &str);
    pub fn watch(&mut self, prefix: &str, module_id: &str);
}

/// decode a duckfs watch notification arriving at a module's execute.
pub fn decode_notify(payload: &[u8]) -> Option<Notify>;
pub struct Notify { pub prefix: String, pub path: String, pub snapshot: String }
```

  `put_inline` with `base: None` only succeeds while the touched path did not
  exist at the empty base AND does not exist at head (per-path CAS) — i.e. it
  is create-only sugar; document that mutation flows must `refs()` +
  `commit(base = head)`.
- Consumes: the crate's own wire types and `sdk::{Ctx, Msg, Error}`. The test drives a real `Files` module over a tempdir through a fake `Ctx` — all in-crate.

- [ ] **Step 1: Failing test** — in-crate fake `Ctx` whose `query` routes to a real `Files` module (over a tempdir) and whose `emit_msg` collects: seed duckfs with two files via direct module execute; `FsCap::stat`/`ls`/`read_all`/`refs` round-trip typed values; `FsCap::commit` + `pin` emit correctly-shaped `FilesMsg` JSON to target `files`; `decode_notify` round-trips a Task 9-shaped `duckfs_notify` payload and returns `None` on foreign payloads.
- [ ] **Step 2: Run to fail** → `cargo test -p files --test cap` FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** → PASS; `cargo check --workspace`.
- [ ] **Step 5: Commit** — `feat(duckfs): typed fs capability over the module ctx (files::FsCap)`.

---

### Task 1: Relocate the op-receipt blob store into noded

The old `files::BlobHandle` is used by noded as the op-payload receipt store (`/v1/files/blob/{op_hash}`) and by forge. That is a daemon concern, not a files-module concern; it must move so Task 2 can delete the old crate contents without breaking the workspace.

**Files:**
- Create: `bin/noded/src/blobs.rs`
- Modify: `bin/noded/src/lib.rs` (imports + `blobs` field types)
- Modify: `bin/noded/src/main.rs` (imports)
- Modify: `bin/node/src/main.rs` and any other `files::BlobHandle` importer — find them all with `grep -rn "files::BlobHandle\|files::BlobStore" bin crates --include="*.rs"`
- Modify: `crates/apps/forge/src/lib.rs` + `crates/apps/forge/Cargo.toml` (forge's `with_blobs` seam: replace the `files::BlobHandle` parameter type with the noded-owned type via a local trait or by moving the type into a tiny shared crate — see Step 3)

**Interfaces:**
- Produces: `noded::blobs::{BlobStore, BlobHandle}` with the exact API the old crate had: `BlobHandle::put_chunk(Vec<u8>) -> [u8;32]`, `get_chunk(&[u8;32]) -> Option<Vec<u8>>`, `has_chunk(&[u8;32]) -> bool`, `Clone + Default`.
- Consumes: nothing from the new files crate (that is the point).

- [ ] **Step 1: Survey every importer**

Run: `grep -rn "files::BlobHandle\|files::BlobStore\|with_blobs\|blob_handle" bin crates --include="*.rs" | grep -v crates/apps/files`
Record the list; every hit must be edited in this task.

- [ ] **Step 2: Create `bin/noded/src/blobs.rs`**

Copy `BlobStore` + `BlobHandle` verbatim from `crates/apps/files/src/lib.rs:47-99` (including the sha256 helper they need), with the docblock rewritten to say this is the daemon's op-receipt/content lane, unrelated to consensus state.

```rust
//! node-local content-addressed byte store for the daemon's receipt lane:
//! op payloads staged at submit time and served back over
//! `GET /v1/files/blob/{digest}`. never consensus state, never in any root.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use sha2::{Digest as _, Sha256};

#[derive(Default)]
pub struct BlobStore {
    chunks: HashMap<[u8; 32], Vec<u8>>,
}

impl BlobStore {
    pub fn put_chunk(&mut self, bytes: Vec<u8>) -> [u8; 32] {
        let digest = sha256(&bytes);
        self.chunks.insert(digest, bytes);
        digest
    }

    pub fn get_chunk(&self, digest: &[u8; 32]) -> Option<&[u8]> {
        self.chunks.get(digest).map(Vec::as_slice)
    }

    pub fn has_chunk(&self, digest: &[u8; 32]) -> bool {
        self.chunks.contains_key(digest)
    }
}

#[derive(Clone, Default)]
pub struct BlobHandle(Arc<Mutex<BlobStore>>);

impl BlobHandle {
    pub fn put_chunk(&self, bytes: Vec<u8>) -> [u8; 32] {
        self.0.lock().expect("blob store poisoned").put_chunk(bytes)
    }

    pub fn get_chunk(&self, digest: &[u8; 32]) -> Option<Vec<u8>> {
        self.0
            .lock()
            .expect("blob store poisoned")
            .get_chunk(digest)
            .map(<[u8]>::to_vec)
    }

    pub fn has_chunk(&self, digest: &[u8; 32]) -> bool {
        self.0
            .lock()
            .expect("blob store poisoned")
            .has_chunk(digest)
    }
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().into()
}
```

- [ ] **Step 3: Decide forge's seam and apply it**

forge takes a `files::BlobHandle` in `Forge::with_blobs`. noded is a binary crate, so forge cannot import from it. Move the two types into a new tiny crate instead: create `crates/system/blobstore` with the code from Step 2 (crate name `blobstore`), have `bin/noded/src/blobs.rs` re-export it (`pub use blobstore::{BlobHandle, BlobStore};`), and change forge's `Cargo.toml`/imports from `files` to `blobstore`. Add the crate to the workspace `Cargo.toml` members. Every other importer from Step 1 switches to `blobstore::BlobHandle` too.

- [ ] **Step 4: Remove the re-export from files, leave the rest of files untouched**

In `crates/apps/files/src/lib.rs` delete the `BlobStore`/`BlobHandle` definitions and replace internal uses (`Files.blobs`, `with_blobs`, `put_chunk`/`get_chunk`/`has_chunk`, `blob_handle`) with `blobstore::BlobHandle` (add `blobstore` to `crates/apps/files/Cargo.toml`). The old module API stays functional for exactly one more task.

- [ ] **Step 5: Verify workspace green**

Run: `cargo check --workspace && cargo test -p files -p forge && cargo test -p noded --test router`
Expected: all green (noded router test exercises the blob receipt lane).

- [ ] **Step 6: Commit**

```bash
git add -A
git -c commit.gpgsign=false commit -m "refactor(blobstore): extract node-local blob store out of the files crate

the op-receipt lane is a daemon concern; duckfs (next commits) deletes the
CAS module surface it used to live beside.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 2: Crate reset — skeleton, constants, wire scaffolding

Flag-day reset of `crates/apps/files`: delete the CAS module and its tests; stand up the duckfs skeleton that compiles and roots over empty refs. Callers (`bin/node`, `bin/noded`, `bin/demo`) are mechanically updated to the new constructor.

**Files:**
- Rewrite: `crates/apps/files/src/lib.rs` (skeleton below)
- Rewrite: `crates/apps/files/src/interface.rs` (constants + wire types + codecs)
- Delete: `crates/apps/files/tests/files_module.rs`
- Create: `crates/apps/files/tests/harness/mod.rs` (shared TestCtx)
- Create: `crates/apps/files/tests/skeleton.rs`
- Modify: `crates/apps/files/Cargo.toml`, workspace `Cargo.toml` (deps: `unicode-normalization = "0.1"`, `base64 = "0.22"`, dev `tempfile`)
- Modify: every `Files::new(...)`/`Files::with_blobs(...)` construction site (`grep -rn "Files::new\|Files::with_blobs" bin crates --include="*.rs"`): change to `Files::open("files", <node data dir>.join("duckfs")).expect("duckfs open")`. In `bin/noded`/`bin/node` the node data dir already exists as a config value near the old construction site; in `bin/demo` use its per-node temp dir.

**Interfaces:**
- Produces: `Files::open(id: impl Into<ModuleId>, dir: PathBuf) -> Result<Files, Error>`; the full wire surface of `interface.rs` below (later tasks implement semantics, this task makes shapes + codecs real); test harness `harness::TestCtx` (used by every later test file).
- Consumes: `blobstore` is gone from this crate after this task (drop the dependency added in Task 1 Step 4 — noded owns it now; delete files' `with_blobs`, `blob_handle`, `put_chunk`, `get_chunk`, `has_chunk` and their call sites found via `grep -rn "blob_handle\|with_blobs" bin crates --include="*.rs"`; noded constructs its `blobstore::BlobHandle` directly instead of pulling one out of the files module).

- [ ] **Step 1: Write the new `interface.rs` (complete wire surface)**

```rust
//! the duckfs wire surface — types only. writes go via [`FilesMsg`] (json)
//! or the binary putblob frame; reads via [`FilesQuery`] -> [`FilesReply`];
//! the off-block object fetch speaks [`FilesSyncReq`] -> [`FilesSyncResp`].

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// a sha256-derived object id rendered as 64-char lowercase hex on the wire.
pub type DigestHex = String;

// ---- network constants (consensus; execute-time rejection on breach) ----
pub const CHUNK_SIZE: u64 = 1024 * 1024;
pub const MAX_NAME_BYTES: usize = 255;
pub const MAX_PATH_BYTES: usize = 4096;
pub const MAX_DEPTH: usize = 128;
pub const MAX_DIR_ENTRIES: usize = 65_536;
pub const MAX_INLINE_COMMIT_BYTES: usize = 256 * 1024;
pub const MAX_CHANGES_PER_COMMIT: usize = 4096;
pub const MAX_MESSAGE_BYTES: usize = 4096;
pub const MAX_META_ENTRIES: usize = 16;
pub const MAX_META_KEY_BYTES: usize = 64;
pub const MAX_META_VALUE_BYTES: usize = 256;
pub const MAX_CHUNKS_PER_FILE: usize = 4_194_304;
pub const MAX_SYMLINK_TARGET_BYTES: usize = 4096;
pub const STAGING_QUOTA_BYTES: u64 = 1024 * 1024 * 1024;
pub const STAGING_TTL_BLOCKS: u64 = 4096;
pub const MAX_PINS: usize = 1024;
pub const MAX_PIN_NAME_BYTES: usize = 128;
pub const MAX_WATCHES: usize = 256;
pub const MAX_WATCH_MODULE_ID_BYTES: usize = 128;
pub const HISTORY_WINDOW: usize = 1024;
pub const GC_PERIOD_BLOCKS: u64 = 1024;
pub const MAX_PAGE: u64 = 256;
pub const MAX_READ_BYTES: u64 = 1024 * 1024;
pub const MAX_GREP_SCAN_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_GREP_LINE_BYTES: usize = 256;
/// first byte of the binary putblob op frame. json msgs start with b'{',
/// so one leading byte disambiguates the whole op space.
pub const PUTBLOB_FRAME_TAG: u8 = 0x00;

// ---- write wire ----

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FilesMsg {
    /// atomic multi-path commit. `base_snapshot: None` means the empty tree
    /// (first commit). per-path CAS: every changed path must be identical
    /// between base and the live head or the whole commit rejects.
    Commit {
        base_snapshot: Option<DigestHex>,
        message: String,
        changes: Vec<Change>,
    },
    Pin { snapshot: DigestHex, name: String },
    /// owner-gated: only the pin's creator (or system) may unpin.
    Unpin { name: String },
    Watch { prefix: String, module_id: String },
    /// gated to the module that registered the watch.
    Unwatch { prefix: String, module_id: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Change {
    Put {
        path: String,
        exec: bool,
        #[serde(default)]
        meta: BTreeMap<String, String>,
        content: Content,
    },
    Mkdir { path: String },
    /// removes the entry at `path` (file, symlink, or whole subtree).
    Rm { path: String },
    Mv { from: String, to: String },
    Symlink { path: String, target: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Content {
    /// small files ride inside the commit op; the module chunks + hashes.
    Inline { b64: String },
    /// large files reference chunks staged via putblob (or already present).
    Chunks { size: u64, chunks: Vec<DigestHex> },
}

// ---- read wire ----

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FilesQuery {
    Stat { path: String, snapshot: Option<DigestHex> },
    Ls {
        path: String,
        snapshot: Option<DigestHex>,
        after: Option<String>,
        limit: u64,
    },
    Read {
        path: String,
        snapshot: Option<DigestHex>,
        offset: u64,
        len: u64,
    },
    Find {
        prefix: String,
        snapshot: Option<DigestHex>,
        after: Option<String>,
        limit: u64,
    },
    Grep {
        pattern: String,
        prefix: String,
        snapshot: Option<DigestHex>,
        cursor: Option<String>,
        limit: u64,
    },
    History { limit: u64 },
    Diff { from: DigestHex, to: DigestHex, prefix: String },
    Refs {},
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntryKindWire { File, Dir, Symlink }

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct EntryInfo {
    pub path: String,
    pub kind: EntryKindWire,
    pub size: u64,
    pub exec: bool,
    pub object: DigestHex,
    #[serde(default)]
    pub meta: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SnapshotInfo {
    pub id: DigestHex,
    pub parent: Option<DigestHex>,
    pub root_tree: DigestHex,
    pub author: String,
    pub height: u64,
    pub consensus_time: u64,
    pub message: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiffKind { Added, Removed, Modified }

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DiffEntry {
    pub path: String,
    pub kind: DiffKind,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct GrepHit {
    pub path: String,
    pub line: u64,
    pub text: String,
    /// `duck://files/<path>@<snapshot>#L<line>`
    pub uri: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RefsInfo {
    pub head: Option<DigestHex>,
    pub pins: BTreeMap<String, DigestHex>,
    pub window_len: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FilesReply {
    Stat(Option<EntryInfo>),
    Ls { entries: Vec<EntryInfo>, next: Option<String> },
    Read { b64: String, eof: bool },
    Find { entries: Vec<EntryInfo>, next: Option<String> },
    Grep { hits: Vec<GrepHit>, next: Option<String> },
    History(Vec<SnapshotInfo>),
    Diff(Vec<DiffEntry>),
    Refs(RefsInfo),
}

// ---- off-block object fetch (state sync / self-heal lane) ----

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FilesSyncReq {
    /// batched fetch; response order matches request order.
    GetObjects { ids: Vec<DigestHex> },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SyncObject {
    pub id: DigestHex,
    pub present: bool,
    /// object kind tag byte (`objects::Kind as u8`) — receivers re-derive the
    /// id as sha256(tag ‖ body) and reject mismatches.
    pub kind: u8,
    pub b64: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FilesSyncResp {
    Objects(Vec<SyncObject>),
}

// ---- codecs (same shape as every module in this repo) ----

pub fn encode_msg(m: &FilesMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}
pub fn decode_msg(b: &[u8]) -> Result<FilesMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_query(q: &FilesQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}
pub fn decode_query(b: &[u8]) -> Result<FilesQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_reply(r: &FilesReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}
pub fn decode_reply(b: &[u8]) -> Result<FilesReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_sync_req(r: &FilesSyncReq) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}
pub fn decode_sync_req(b: &[u8]) -> Result<FilesSyncReq, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_sync_resp(r: &FilesSyncResp) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}
pub fn decode_sync_resp(b: &[u8]) -> Result<FilesSyncResp, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

/// build a putblob op: `[PUTBLOB_FRAME_TAG] ++ raw chunk bytes`.
pub fn encode_putblob(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + bytes.len());
    out.push(PUTBLOB_FRAME_TAG);
    out.extend_from_slice(bytes);
    out
}

/// grep-hit evidence uri.
pub fn evidence_uri(path: &str, snapshot: &str, line: u64) -> String {
    format!("duck://files/{path}@{snapshot}#L{line}")
}

/// decode exactly 64 lowercase-hex chars into 32 bytes (uppercase rejected).
pub fn from_hex_32(s: &str) -> Option<[u8; 32]> {
    let bytes = s.as_bytes();
    if bytes.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, slot) in out.iter_mut().enumerate() {
        let hi = hex_val(bytes[2 * i])?;
        let lo = hex_val(bytes[2 * i + 1])?;
        *slot = (hi << 4) | lo;
    }
    Some(out)
}

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    }
}

pub fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}
```

- [ ] **Step 2: Write the new `lib.rs` skeleton**

```rust
//! duckfs — a consensus-replicated, copy-on-write, content-addressed
//! filesystem. every node holds every byte as consensus state; bytes travel
//! through blocks (putblob staging + atomic commits). immutable objects
//! (chunk/file/tree/snapshot) live in a disk odb; `root()` is sha256 over the
//! canonical encoding of the small mutable [`state::Refs`] only. spec:
//! docs/superpowers/specs/2026-07-06-duckfs-real-filesystem-design.md

mod interface;
pub use interface::*;

pub mod objects;
pub mod odb;
pub mod paths;
pub mod state;
mod tree;
mod queries;
mod gc;
mod sync;

use std::path::PathBuf;

use sdk::{Ctx, Error, Module, ModuleId, Msg, StateRoot, StateSyncHandle};

pub struct Files {
    id: ModuleId,
    dir: PathBuf,
    odb: odb::Odb,
    refs: state::Refs,
    /// per-block overlay: refs-next plus objects awaiting the disk commit.
    pending: Option<PendingBlock>,
}

struct PendingBlock {
    refs: state::Refs,
    objects: Vec<(objects::Kind, Vec<u8>)>,
}

impl Files {
    /// open (or create) the module over its data dir. the dir layout is
    /// `dir/objects/<aa>/<hex>` plus `dir/refs`; leftover `*.tmp` files from
    /// a crash are swept here.
    pub fn open(id: impl Into<ModuleId>, dir: PathBuf) -> Result<Self, Error> {
        let odb = odb::Odb::open(dir.join("objects"))
            .map_err(|e| Error::Module(format!("files: odb open: {e}")))?;
        let refs = state::Refs::load(&dir)
            .map_err(|e| Error::Module(format!("files: refs load: {e}")))?;
        Ok(Self { id: id.into(), dir, odb, refs, pending: None })
    }

    fn require_pending(&mut self) -> &mut PendingBlock {
        if self.pending.is_none() {
            self.pending = Some(PendingBlock {
                refs: self.refs.clone(),
                objects: Vec::new(),
            });
        }
        self.pending.as_mut().expect("just set")
    }
}

pub(crate) fn require(ok: bool, why: &str) -> Result<(), Error> {
    if ok { Ok(()) } else { Err(Error::Module(format!("files: {why}"))) }
}

#[async_trait::async_trait(?Send)]
impl Module for Files {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        self.refs.root()
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::ResolverBacked {
            backend: "duckfs-odb".into(),
            detail: "refs snapshot + GetObjects fetch to full possession".into(),
        })
    }

    async fn serve_sync(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        sync::serve(self, req)
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let env = ctx.env().clone();
        match msg.payload.first() {
            Some(&PUTBLOB_FRAME_TAG) => self.exec_putblob(&env, &msg.payload[1..]),
            _ => match decode_msg(&msg.payload).map_err(Error::Module)? {
                FilesMsg::Commit { base_snapshot, message, changes } => {
                    self.exec_commit(ctx, &env, base_snapshot, message, changes)
                }
                FilesMsg::Pin { snapshot, name } => self.exec_pin(&env, snapshot, name),
                FilesMsg::Unpin { name } => self.exec_unpin(&env, name),
                FilesMsg::Watch { prefix, module_id } => self.exec_watch(&env, prefix, module_id),
                FilesMsg::Unwatch { prefix, module_id } => {
                    self.exec_unwatch(&env, prefix, module_id)
                }
            },
        }
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        queries::serve(self, req)
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        let Some(pending) = self.pending.take() else { return Ok(()) };
        for (kind, body) in &pending.objects {
            self.odb
                .put(*kind, body)
                .map_err(|e| Error::Module(format!("files: odb put: {e}")))?;
        }
        self.refs = pending.refs;
        // deterministic housekeeping happens on refs BEFORE persist:
        // staging expiry each block, gc sweep every GC_PERIOD_BLOCKS (task 13).
        self.refs
            .save(&self.dir)
            .map_err(|e| Error::Module(format!("files: refs save: {e}")))?;
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending = None;
        Ok(())
    }
}
```

Stub the five `exec_*` methods and the `queries::serve`/`sync::serve`/module files so this compiles: each stub returns `Err(Error::Module("files: unimplemented".into()))` and each module file starts with only its type skeleton (`objects.rs`, `odb.rs`, `paths.rs`, `state.rs` get real content in Tasks 3–6; `tree.rs`, `queries.rs`, `gc.rs`, `sync.rs` in Tasks 8–14). For this task `state::Refs` needs the minimal real core so `root()` works: `#[derive(Clone, Default)] pub struct Refs { ... }` with `root()`, `load()` (Ok(Default) when file absent), `save()` (no-op returning Ok for now — Task 6 makes it real).

- [ ] **Step 3: Shared test harness `tests/harness/mod.rs`**

Copy the `TestCtx` pattern from the deleted `tests/files_module.rs` (it is in git history at `crates/apps/files/tests/files_module.rs:33-67`) — an `sdk::Ctx` impl over `Env { protocol_version: 0, height, consensus_time: height, origin, me: "files" }` — plus:

```rust
use std::collections::VecDeque;

pub struct TestCtx {
    pub env: Env,
    pub emitted: VecDeque<Msg>,
}
// emit_msg pushes into `emitted` (watch fan-out assertions need it);
// emit_event/request_effect stay no-ops; query returns QueryUnsupported.

pub fn open_files(dir: &tempfile::TempDir) -> files::Files {
    files::Files::open("files", dir.path().to_path_buf()).expect("open")
}

pub fn sha256(bytes: &[u8]) -> [u8; 32] { /* as in the old tests */ }
pub fn to_hex(bytes: &[u8]) -> String { files::to_hex(bytes) }
```

- [ ] **Step 4: Write `tests/skeleton.rs` (failing first is fine — it drives the stubs)**

```rust
mod harness;
use harness::*;

#[test]
fn opens_empty_and_roots_deterministically() {
    let d1 = tempfile::tempdir().unwrap();
    let d2 = tempfile::tempdir().unwrap();
    let a = open_files(&d1);
    let b = open_files(&d2);
    assert_eq!(a.root(), b.root(), "empty refs root must be dir-independent");
    assert_ne!(a.root(), sdk::StateRoot::ZERO);
}

#[test]
fn unknown_json_op_rejects_and_putblob_frame_routes() {
    futures::executor::block_on(async {
        let d = tempfile::tempdir().unwrap();
        let mut f = open_files(&d);
        let err = f
            .execute(
                &mut TestCtx::new(sdk::Origin::System, 1),
                &sdk::Msg { target: "files".into(), payload: b"{\"nope\":{}}".to_vec() },
            )
            .await
            .expect_err("unknown json must reject");
        assert!(matches!(err, sdk::Error::Module(_)));
    });
}
```

- [ ] **Step 5: Update construction sites, run gates**

Apply the `Files::open` change at every site from the Files list above.
Run: `cargo test -p files && cargo check --workspace`
Expected: skeleton tests pass (putblob routes to the unimplemented stub — adjust the second test to expect the stub's `Error::Module("files: unimplemented")` for now); workspace compiles.

- [ ] **Step 6: Commit**

```bash
git add -A
git -c commit.gpgsign=false commit -m "feat(duckfs)!: flag-day reset of the files crate to the duckfs skeleton

old CAS wire (AddManifest/RemoveManifest/GetChunk) deleted; new wire types,
constants, and module scaffold in place; callers moved to Files::open(dir).

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 3: Object model (`objects.rs`)

Content-addressed immutable objects: canonical encodings, strict decoders, domain-separated ids.

**Files:**
- Create: `crates/apps/files/src/objects.rs` (replacing the Task 2 stub)
- Test: `crates/apps/files/tests/object_model.rs`

**Interfaces:**
- Produces:
  - `pub type ObjectId = [u8; 32];`
  - `pub enum Kind { Chunk = 0, File = 1, Tree = 2, Snapshot = 3 }` with `Kind::from_u8(u8) -> Option<Kind>`
  - `pub fn object_id(kind: Kind, body: &[u8]) -> ObjectId` — `sha256([kind as u8] ‖ body)`
  - `pub struct FileObj { pub size: u64, pub chunks: Vec<ObjectId>, pub meta: BTreeMap<String, String> }`
  - `pub enum EntryKind { File = 0, Dir = 1, Symlink = 2 }`
  - `pub struct TreeEntry { pub kind: EntryKind, pub id: ObjectId, pub exec: bool, pub size: u64 }`
  - `pub struct TreeObj { pub entries: BTreeMap<String, TreeEntry> }`
  - `pub struct SnapshotObj { pub root: ObjectId, pub parent: Option<ObjectId>, pub author: String, pub consensus_time: u64, pub height: u64, pub message: String }`
  - each Obj type: `pub fn encode(&self) -> Vec<u8>` and `pub fn decode(bytes: &[u8]) -> Result<Self, String>` (strict: trailing bytes rejected, counts capped, utf-8 enforced, tree names strictly ascending, meta keys strictly ascending)
  - `pub fn verify_chunk_len(file: &FileObj, index: usize, got_len: u64) -> Result<(), String>` — the load-bearing exact-length rule: `CHUNK_SIZE` for all but the last chunk, `size - (n-1)*CHUNK_SIZE` for the last (checked_sub; inconsistent size/chunks is an error)
- Consumes: constants from `interface.rs`.

Encoding formats (little-endian, length-prefixed strings as `u64 len ‖ bytes` — the house style from the old `encode_manifests`):

- File body: `size u64 ‖ chunk_count u32 ‖ chunk ids (32 B each) ‖ meta_count u16 ‖ (key, value) pairs`
- Tree body: `entry_count u32 ‖ per entry: name ‖ kind u8 ‖ id 32 B ‖ exec u8 (0/1) ‖ size u64`, entries in strict ascending name order
- Snapshot body: `root 32 B ‖ has_parent u8 ‖ parent 32 B (if 1) ‖ author ‖ consensus_time u64 ‖ height u64 ‖ message`

- [ ] **Step 1: Write failing tests** — golden id stability, round-trips, strict-decode rejections:

```rust
mod harness;
use files::objects::*;
use harness::sha256;

#[test]
fn object_ids_are_domain_separated_and_stable() {
    let body = b"hello".to_vec();
    let chunk_id = object_id(Kind::Chunk, &body);
    let mut pre = vec![0u8]; // Kind::Chunk tag
    pre.extend_from_slice(&body);
    assert_eq!(chunk_id, sha256(&pre), "id = sha256(tag || body)");
    assert_ne!(chunk_id, object_id(Kind::File, &body), "tags must separate domains");
}

#[test]
fn file_tree_snapshot_round_trip() {
    let f = FileObj { size: 5, chunks: vec![[7u8; 32]], meta: [("kind".into(), "skill".into())].into() };
    assert_eq!(FileObj::decode(&f.encode()).unwrap(), f);

    let t = TreeObj { entries: [("a.txt".to_string(), TreeEntry { kind: EntryKind::File, id: [1; 32], exec: false, size: 5 })].into() };
    assert_eq!(TreeObj::decode(&t.encode()).unwrap(), t);

    let s = SnapshotObj { root: [2; 32], parent: None, author: "system".into(), consensus_time: 9, height: 9, message: "m".into() };
    assert_eq!(SnapshotObj::decode(&s.encode()).unwrap(), s);
}

#[test]
fn strict_decode_rejects() {
    // trailing bytes
    let f = FileObj { size: 1, chunks: vec![[0; 32]], meta: Default::default() };
    let mut b = f.encode();
    b.push(0);
    assert!(FileObj::decode(&b).is_err(), "trailing bytes");
    // truncation at every length is also rejected
    let enc = f.encode();
    for cut in 0..enc.len() {
        assert!(FileObj::decode(&enc[..cut]).is_err(), "truncated at {cut}");
    }
    // tree names must be strictly ascending — hand-encode b then a
    // (build two single-entry trees, splice bodies; assert decode Err)
}

#[test]
fn chunk_len_rule() {
    use files::CHUNK_SIZE;
    let f = FileObj { size: CHUNK_SIZE + 1, chunks: vec![[0; 32], [1; 32]], meta: Default::default() };
    assert!(verify_chunk_len(&f, 0, CHUNK_SIZE).is_ok());
    assert!(verify_chunk_len(&f, 1, 1).is_ok());
    assert!(verify_chunk_len(&f, 1, 0).is_err(), "empty-chunk spoof caught by length");
    assert!(verify_chunk_len(&f, 2, 1).is_err(), "index out of range");
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p files --test object_model` → FAIL (unresolved items).

- [ ] **Step 3: Implement `objects.rs`** — reuse the strict `read_u64`/`read_string` helper style from git history (`crates/apps/files/src/lib.rs:435-457` pre-reset), adding `read_u32`, `read_u16`, `read_bytes32`, and an `Eof`-safe cursor; enforce `MAX_CHUNKS_PER_FILE`, `MAX_META_ENTRIES`/key/value caps, `MAX_DIR_ENTRIES`, `MAX_NAME_BYTES`, `MAX_MESSAGE_BYTES`, strict ascending order for tree names and meta keys, and `off == bytes.len()` at the end of every decode.

- [ ] **Step 4: Run tests** — `cargo test -p files --test object_model` → PASS.

- [ ] **Step 5: Commit** — `git add -A && git -c commit.gpgsign=false commit -m "feat(duckfs): content-addressed object model with strict canonical codecs"` (+ trailer).

---

### Task 4: Path canonicalization and authority (`paths.rs`)

**Files:**
- Create: `crates/apps/files/src/paths.rs`
- Test: `crates/apps/files/tests/paths.rs`

**Interfaces:**
- Produces:
  - `pub fn canonical(path: &str) -> Result<Vec<String>, String>` — validates and splits into segments. Rules: must start with `/`; UTF-8 (given by `&str`); NFC-normalized (reject if `path != path.nfc()` — `use unicode_normalization::UnicodeNormalization`); no empty / `.` / `..` segments; no `\0` or `/` inside segments; total ≤ `MAX_PATH_BYTES`; depth ≤ `MAX_DEPTH`; each segment ≤ `MAX_NAME_BYTES`.
  - `pub fn owner_of(origin: &sdk::Origin) -> String` — verbatim from git history (`ext:` + hex domain separation).
  - `pub fn check_authority(owner: &str, segments: &[String]) -> Result<(), String>` — `system` passes always; `["home", o, ..]` requires `o == owner` **and** at least the two lead segments (writing `/home` or `/home/<o>` itself as a file is rejected — only paths *under* the home root); `["shared", ..]` with ≥2 segments passes; everything else rejects.
- Consumes: `interface.rs` constants.

- [ ] **Step 1: Failing test table** (every row is one assertion; exact expected error substrings):

| input | expect |
| --- | --- |
| `/shared/a.txt` (ext owner) | Ok, segments `["shared","a.txt"]` |
| `/home/ext:aa/x` by owner `ext:aa` | Ok |
| `/home/ext:aa/x` by owner `ext:bb` | Err `not the home owner` |
| `/home/ext:aa` (the home root itself) | Err `home root is not writable` |
| `/etc/passwd` | Err `outside /home and /shared` |
| `shared/x` (no leading slash) | Err `must be absolute` |
| `/shared//x`, `/shared/./x`, `/shared/../x` | Err `empty or dot segment` |
| `/shared/caf\u{0065}\u{0301}` (NFD é) | Err `not NFC` |
| `/shared/caf\u{00e9}` (NFC é) | Ok |
| name of 256 bytes | Err `name exceeds` |
| 129-deep path | Err `depth` |
| any path by `system` origin, incl. `/etc` | Ok (authority only; still canonical) |

```rust
mod harness;
use files::paths::*;

#[test]
fn canonical_and_authority_table() {
    assert_eq!(canonical("/shared/a.txt").unwrap(), vec!["shared", "a.txt"]);
    assert!(canonical("shared/x").unwrap_err().contains("absolute"));
    assert!(canonical("/shared//x").unwrap_err().contains("segment"));
    assert!(canonical("/shared/../x").unwrap_err().contains("segment"));
    assert!(canonical("/shared/cafe\u{0301}").unwrap_err().contains("NFC"));
    assert!(canonical("/shared/caf\u{00e9}").is_ok());
    assert!(canonical(&format!("/shared/{}", "x".repeat(256))).unwrap_err().contains("name"));
    let deep = format!("/shared{}", "/d".repeat(128));
    assert!(canonical(&deep).unwrap_err().contains("depth"));

    let seg = |p: &str| canonical(p).unwrap();
    assert!(check_authority("ext:aa", &seg("/home/ext:aa/x")).is_ok());
    assert!(check_authority("ext:bb", &seg("/home/ext:aa/x")).unwrap_err().contains("home owner"));
    assert!(check_authority("ext:aa", &seg("/home/ext:aa")).unwrap_err().contains("home root"));
    assert!(check_authority("ext:aa", &seg("/etc/passwd")).unwrap_err().contains("outside"));
    assert!(check_authority("system", &seg("/etc/passwd")).is_ok());
    assert!(check_authority("chat", &seg("/shared/x")).is_ok());
}
```

- [ ] **Step 2: Run to verify fail** — `cargo test -p files --test paths` → FAIL.
- [ ] **Step 3: Implement** (straightforward; NFC check is `path.chars().nfc().collect::<String>() == path` — do it on the whole path once).
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** — `feat(duckfs): canonical paths (NFC, strict segments) and home/shared authority`.

---

### Task 5: Disk object store (`odb.rs`)

**Files:**
- Create: `crates/apps/files/src/odb.rs`
- Test: `crates/apps/files/tests/odb.rs`

**Interfaces:**
- Produces `pub struct Odb` with:
  - `pub fn open(dir: PathBuf) -> std::io::Result<Odb>` — creates dirs, sweeps `*.tmp` leftovers (crash debris).
  - `pub fn put(&mut self, kind: objects::Kind, body: &[u8]) -> std::io::Result<objects::ObjectId>` — computes id, writes `dir/<aa>/<hex[2..]>` via `tmp file in same dir → write [kind u8 ‖ body] → sync_all → rename`; existing file = no-op (idempotent, content-addressed).
  - `pub fn get(&self, id: &objects::ObjectId) -> std::io::Result<Option<(objects::Kind, Vec<u8>)>>` — reads, splits tag byte, **re-verifies** `object_id(kind, body) == *id` (corrupt file → `Err(InvalidData)`, never silently wrong bytes).
  - `pub fn has(&self, id: &objects::ObjectId) -> bool`
  - `pub fn remove(&mut self, id: &objects::ObjectId) -> std::io::Result<()>` (GC uses it)
  - `pub fn list(&self) -> std::io::Result<Vec<objects::ObjectId>>` (GC sweep enumeration; sorted)
- File format on disk: `[kind u8] ‖ body`. The filename is the hex id; `get` re-derives and checks.

- [ ] **Step 1: Failing tests**

```rust
mod harness;
use files::objects::{object_id, Kind};
use files::odb::Odb;

#[test]
fn put_get_has_roundtrip_and_idempotence() {
    let d = tempfile::tempdir().unwrap();
    let mut odb = Odb::open(d.path().join("objects")).unwrap();
    let id = odb.put(Kind::Chunk, b"bytes").unwrap();
    assert_eq!(id, object_id(Kind::Chunk, b"bytes"));
    assert!(odb.has(&id));
    assert_eq!(odb.get(&id).unwrap(), Some((Kind::Chunk, b"bytes".to_vec())));
    let id2 = odb.put(Kind::Chunk, b"bytes").unwrap();
    assert_eq!(id, id2, "idempotent re-put");
    assert_eq!(odb.get(&object_id(Kind::Chunk, b"absent")).unwrap(), None);
}

#[test]
fn corrupt_object_is_an_error_not_bad_bytes() {
    let d = tempfile::tempdir().unwrap();
    let mut odb = Odb::open(d.path().join("objects")).unwrap();
    let id = odb.put(Kind::Chunk, b"bytes").unwrap();
    // flip a byte on disk behind the store's back
    let hex = files::to_hex(&id);
    let path = d.path().join("objects").join(&hex[..2]).join(&hex[2..]);
    let mut raw = std::fs::read(&path).unwrap();
    raw[3] ^= 0xff;
    std::fs::write(&path, raw).unwrap();
    assert!(odb.get(&id).is_err(), "hash mismatch must surface as an error");
}

#[test]
fn open_sweeps_tmp_debris_and_list_enumerates() {
    let d = tempfile::tempdir().unwrap();
    let dir = d.path().join("objects");
    let mut odb = Odb::open(dir.clone()).unwrap();
    let a = odb.put(Kind::Chunk, b"a").unwrap();
    let b = odb.put(Kind::Tree, b"b").unwrap();
    std::fs::write(dir.join("junk.tmp"), b"crash leftovers").unwrap();
    let odb2 = Odb::open(dir.clone()).unwrap();
    assert!(!dir.join("junk.tmp").exists(), "tmp debris swept at open");
    let mut want = vec![a, b];
    want.sort();
    assert_eq!(odb2.list().unwrap(), want);
}
```

- [ ] **Step 2: Run to fail** → `cargo test -p files --test odb` FAIL.
- [ ] **Step 3: Implement** — fan-out subdirs created lazily; tmp names `"<hex>.tmp"` written in the *destination* subdir so rename is same-directory atomic; `list` walks two levels, parses hex filenames, ignores non-hex.
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** — `feat(duckfs): loose-object disk store with tmp/fsync/rename and verified reads`.

### Task 6: Refs state and root (`state.rs`)

The only mutable consensus state. Its canonical encoding is the `root()` preimage; the refs *file* wraps that encoding with recovery metadata that is deliberately **not** part of the root.

**Files:**
- Create: `crates/apps/files/src/state.rs` (replacing the Task 2 minimal stub)
- Test: `crates/apps/files/tests/state_root.rs`

**Interfaces:**
- Produces:
  - `pub struct PinEntry { pub snapshot: ObjectId, pub owner: String }`
  - `pub struct Staged { pub owner: String, pub len: u64, pub expires_at: u64 }`
  - `#[derive(Clone, Default, PartialEq, Debug)] pub struct Refs { pub head: Option<ObjectId>, pub window: VecDeque<ObjectId>, pub pins: BTreeMap<String, PinEntry>, pub staging: BTreeMap<ObjectId, Staged>, pub watches: BTreeSet<(String, String)> }`
  - `pub fn encode_refs(r: &Refs) -> Vec<u8>` / `pub fn decode_refs(b: &[u8]) -> Result<Refs, String>` (strict: sorted, no trailing bytes; window ≤ `HISTORY_WINDOW`, pins ≤ `MAX_PINS`, watches ≤ `MAX_WATCHES`)
  - `impl Refs { pub fn root(&self) -> StateRoot // sha256(encode_refs) ; pub fn load(dir: &Path) -> io::Result<(Refs, u64 /*height*/, u64 /*gc_watermark*/)> ; pub fn save(dir: &Path, refs: &Refs, height: u64, gc_watermark: u64) -> io::Result<()> }`
- **Root-preimage rule (load-bearing):** `height` and `gc_watermark` live only in the refs-file envelope, never in `encode_refs`. The root must not move on empty blocks, and recovery metadata is per-node bookkeeping. Adjust the Task 2 `Files` struct: `refs: Refs, durable_height: u64, gc_watermark: u64`, and `PendingBlock { refs: Refs, objects: Vec<(Kind, Vec<u8>)>, height: u64 }` (execute stamps `pending.height = env.height`).

Refs-file format at `dir/refs`: `b"DUCKFS1\n" ‖ height u64 ‖ gc_watermark u64 ‖ payload_len u64 ‖ payload (= encode_refs) ‖ sha256(payload) 32 B`, written tmp → fsync → rename. `load` with no file → `(Refs::default(), 0, 0)`; corrupt magic/checksum/trailing → `Err(InvalidData)`.

Preimage encoding (little-endian, strings length-prefixed u64, ids raw 32 B):
`head: u8 flag ‖ [32 B] ; window: u32 count ‖ ids ; pins: u32 count ‖ (name ‖ snapshot ‖ owner) in name order ; staging: u32 count ‖ (digest ‖ owner ‖ len u64 ‖ expires_at u64) in digest order ; watches: u32 count ‖ (prefix ‖ module_id) in order`

- [ ] **Step 1: Failing tests**

```rust
mod harness;
use files::state::*;

#[test]
fn root_is_content_only_and_deterministic() {
    let a = Refs::default();
    let b = Refs::default();
    assert_eq!(a.root(), b.root());
    let mut c = Refs::default();
    c.head = Some([9; 32]);
    assert_ne!(a.root(), c.root(), "head change moves the root");
}

#[test]
fn refs_file_round_trips_and_checks_integrity() {
    let d = tempfile::tempdir().unwrap();
    let mut r = Refs::default();
    r.head = Some([1; 32]);
    r.staging.insert([2; 32], Staged { owner: "ext:aa".into(), len: 5, expires_at: 100 });
    Refs::save(d.path(), &r, 42, 7).unwrap();
    let (r2, h, gw) = Refs::load(d.path()).unwrap();
    assert_eq!((r2, h, gw), (r, 42, 7));

    // corrupt the payload; load must error, not return wrong refs
    let raw = std::fs::read(d.path().join("refs")).unwrap();
    let mut bad = raw.clone();
    let n = bad.len();
    bad[n - 40] ^= 0xff; // inside payload
    std::fs::write(d.path().join("refs"), bad).unwrap();
    assert!(Refs::load(d.path()).is_err());
}

#[test]
fn decode_refs_is_strict() {
    let r = Refs::default();
    let mut b = encode_refs(&r);
    b.push(0);
    assert!(decode_refs(&b).is_err(), "trailing bytes");
    for cut in 0..encode_refs(&r).len() {
        assert!(decode_refs(&encode_refs(&r)[..cut]).is_err(), "truncated {cut}");
    }
}
```

- [ ] **Step 2: Run to fail** → `cargo test -p files --test state_root` FAIL.
- [ ] **Step 3: Implement**, update `lib.rs`: `Files::open` uses `Refs::load`; `commit_block` persists `Refs::save(&self.dir, &pending.refs, pending.height, self.gc_watermark)` and sets `self.durable_height = pending.height`.
- [ ] **Step 4: Run** → PASS (also `cargo test -p files` — skeleton still green).
- [ ] **Step 5: Commit** — `feat(duckfs): refs state, canonical root preimage, atomic refs file with recovery envelope`.

---

### Task 7: PutBlob staging

**Files:**
- Modify: `crates/apps/files/src/lib.rs` (`exec_putblob`, expiry sweep helper)
- Test: `crates/apps/files/tests/putblob.rs`

**Interfaces:**
- Produces: `Files::exec_putblob(&mut self, env: &Env, bytes: &[u8]) -> Result<(), Error>`; `pub(crate) fn sweep_expired(refs: &mut Refs, height: u64)` (called at the top of **every** `execute` before dispatch — expiry is driven by the op stream, so it is deterministic; document that expiry lands at the first files-activity block at-or-after `expires_at`).
- Consumes: `state::Refs`, `objects::object_id`.

Semantics:
1. `bytes` non-empty and `len ≤ CHUNK_SIZE`, else reject.
2. `digest = object_id(Kind::Chunk, bytes)`. Already in odb (or in `pending.objects`) → Ok, no-op (already durable). Already staged → Ok, no-op (no double quota).
3. Quota: sum of `staging` lens for this owner (in the pending refs view) + `len` must be ≤ `STAGING_QUOTA_BYTES`, else reject `staging quota exceeded`.
4. Stage: `pending.refs.staging.insert(digest, Staged { owner, len, expires_at: env.height + STAGING_TTL_BLOCKS })` **and** `pending.objects.push((Kind::Chunk, bytes.to_vec()))` — staged bytes are consensus state and must be durable at this block's commit; the staging table keeps them GC-reachable (Task 13 marks staging digests as roots).
5. Expiry sweep (in `execute`, before dispatch): remove staging entries with `expires_at ≤ env.height`. Swept chunks become unreachable and fall to the next GC.

- [ ] **Step 1: Failing tests**

```rust
mod harness;
use harness::*;
use files::{encode_putblob, CHUNK_SIZE, STAGING_TTL_BLOCKS};

fn putblob(f: &mut files::Files, origin: sdk::Origin, h: u64, bytes: &[u8]) -> Result<(), sdk::Error> {
    futures::executor::block_on(f.execute(
        &mut TestCtx::new(origin, h),
        &sdk::Msg { target: "files".into(), payload: encode_putblob(bytes) },
    ))
}

#[test]
fn stages_within_caps_and_rejects_breaches() {
    futures::executor::block_on(async {
        let d = tempfile::tempdir().unwrap();
        let mut f = open_files(&d);
        putblob(&mut f, sdk::Origin::External(b"a".to_vec()), 1, b"hello").expect("stage");
        assert!(putblob(&mut f, sdk::Origin::System, 1, &[]).is_err(), "empty chunk");
        let big = vec![0u8; CHUNK_SIZE as usize + 1];
        assert!(putblob(&mut f, sdk::Origin::System, 1, &big).is_err(), "oversized chunk");
        f.commit_block().await.unwrap();
        // idempotent re-put after durability
        putblob(&mut f, sdk::Origin::External(b"b".to_vec()), 2, b"hello").expect("no-op re-put");
    });
}

#[test]
fn quota_is_per_owner_and_expiry_frees_it() {
    futures::executor::block_on(async {
        let d = tempfile::tempdir().unwrap();
        let mut f = open_files(&d);
        // fill alice's quota with max-size chunks of distinct bytes
        let n = (files::STAGING_QUOTA_BYTES / CHUNK_SIZE) as usize;
        for i in 0..n {
            let mut c = vec![0u8; CHUNK_SIZE as usize];
            c[..8].copy_from_slice(&(i as u64).to_le_bytes());
            putblob(&mut f, sdk::Origin::External(b"alice".to_vec()), 1, &c).expect("fill");
        }
        let mut extra = vec![1u8; CHUNK_SIZE as usize];
        extra[..2].copy_from_slice(b"xx");
        assert!(putblob(&mut f, sdk::Origin::External(b"alice".to_vec()), 1, &extra).is_err(), "quota");
        putblob(&mut f, sdk::Origin::External(b"bob".to_vec()), 1, &extra).expect("bob unaffected");
        f.commit_block().await.unwrap();
        let r0 = f.root();
        // expiry: first files op at/after height 1 + TTL sweeps alice's entries
        putblob(&mut f, sdk::Origin::External(b"carol".to_vec()), 1 + STAGING_TTL_BLOCKS, b"tick").unwrap();
        f.commit_block().await.unwrap();
        assert_ne!(f.root(), r0, "sweep must move the root (staging is state)");
        putblob(&mut f, sdk::Origin::External(b"alice".to_vec()), 2 + STAGING_TTL_BLOCKS, &extra).expect("quota freed");
    });
}
```

Note the quota-fill loop stages 1,024 × 1 MiB chunks (~1 GiB disk in the tempdir). If CI disk is a concern, add `pub(crate) fn set_staging_quota_for_tests` gated behind `#[doc(hidden)]` — decide at implementation time; default is running it honestly (`#[ignore]` + a small-quota variant is acceptable only if CI cannot afford 1 GiB).

- [ ] **Step 2: Run to fail** → FAIL.
- [ ] **Step 3: Implement** per semantics above.
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** — `feat(duckfs): putblob staging with per-owner quota and deterministic ttl sweep`.

---

### Task 8: Tree read/edit engine (`tree.rs`)

**Files:**
- Create: `crates/apps/files/src/tree.rs`
- Test: `crates/apps/files/tests/tree_edit.rs`

**Interfaces:**
- Produces:
  - `pub(crate) struct Store<'a> { pub odb: &'a odb::Odb, pub pending: &'a [(Kind, Vec<u8>)] }` with `fn get(&self, id: &ObjectId) -> Result<Option<(Kind, Vec<u8>)>, String>` (pending buffer first — in-block chained commits read objects not yet on disk — then odb).
  - `pub(crate) fn snapshot_root_tree(store: &Store, snapshot: &ObjectId) -> Result<ObjectId, String>` (decode SnapshotObj, return `.root`).
  - `pub(crate) fn entry_at(store: &Store, root_tree: Option<ObjectId>, segs: &[String]) -> Result<Option<TreeEntry>, String>` — walk; `None` root = empty tree.
  - `pub(crate) enum Node { Ref(TreeEntry), Dir(BTreeMap<String, Node>) }` and `pub(crate) struct TreeEdit { root: BTreeMap<String, Node> }` with:
    - `fn load(store, root_tree: Option<ObjectId>) -> TreeEdit` (root as lazy `Ref`s: children stay `Node::Ref` until a path forces loading)
    - `fn put(&mut self, store, segs, entry: TreeEntry) -> Result<(), String>` (auto-creates parent dirs; rejects if a non-dir sits on the way)
    - `fn mkdir(&mut self, store, segs) -> Result<(), String>` (rejects if entry exists)
    - `fn rm(&mut self, store, segs) -> Result<(), String>` (rejects if absent)
    - `fn get(&self, store, segs) -> Result<Option<TreeEntry>, String>` (view over the edit — `Mv` = get + rm + put by the caller in Task 9)
    - `fn build(self, out: &mut Vec<(Kind, Vec<u8>)>) -> Result<Option<ObjectId>, String>` — post-order: encode each loaded dir as `TreeObj`, enforce `MAX_DIR_ENTRIES`, push `(Kind::Tree, body)` to `out`, return root tree id (`None` for a completely empty root — the empty filesystem).
- Consumes: `objects::*`, `odb::Odb`.

Tree-entry `size` bookkeeping: for `dir` entries, `size` = number of direct entries (cheap, deterministic); for files, byte size; for symlinks, target length.

- [ ] **Step 1: Failing tests** — build a small tree via `TreeEdit` against an odb, then read it back with `entry_at`:

```rust
mod harness;
use files::objects::*;
use files::odb::Odb;
// (tree.rs is pub(crate); make the test go through a #[doc(hidden)] pub facade
//  `files::testkit` exposing Store/TreeEdit/entry_at re-exports for integration
//  tests — add that one-line module in this task.)
use files::testkit::*;

#[test]
fn edit_builds_shared_cow_trees() {
    let d = tempfile::tempdir().unwrap();
    let mut odb = Odb::open(d.path().join("objects")).unwrap();
    let mut out = Vec::new();

    // v1: /shared/a.txt + /shared/deep/b.txt
    let store = Store { odb: &odb, pending: &[] };
    let mut e = TreeEdit::load(&store, None);
    let leaf = |id| TreeEntry { kind: EntryKind::File, id, exec: false, size: 1 };
    e.put(&store, &segs("/shared/a.txt"), leaf([1; 32])).unwrap();
    e.put(&store, &segs("/shared/deep/b.txt"), leaf([2; 32])).unwrap();
    let root1 = e.build(&mut out).unwrap().unwrap();
    for (k, b) in out.drain(..) { odb.put(k, &b).unwrap(); }

    // v2: touch only a.txt — deep/ subtree id must be REUSED (structural CoW)
    let store = Store { odb: &odb, pending: &[] };
    let mut e = TreeEdit::load(&store, Some(root1));
    e.put(&store, &segs("/shared/a.txt"), leaf([3; 32])).unwrap();
    let root2 = e.build(&mut out).unwrap().unwrap();
    assert_ne!(root1, root2);
    let deep1 = entry_at(&store, Some(root1), &segs("/shared/deep")).unwrap().unwrap();
    for (k, b) in out.drain(..) { odb.put(k, &b).unwrap(); }
    let deep2 = entry_at(&store, Some(root2), &segs("/shared/deep")).unwrap().unwrap();
    assert_eq!(deep1.id, deep2.id, "untouched subtree object shared by hash");
}

#[test]
fn edit_rules() {
    let d = tempfile::tempdir().unwrap();
    let odb = Odb::open(d.path().join("objects")).unwrap();
    let store = Store { odb: &odb, pending: &[] };
    let mut e = TreeEdit::load(&store, None);
    assert!(e.rm(&store, &segs("/shared/nope")).is_err(), "rm absent");
    e.mkdir(&store, &segs("/shared/dir")).unwrap();
    assert!(e.mkdir(&store, &segs("/shared/dir")).is_err(), "mkdir exists");
    let leaf = TreeEntry { kind: EntryKind::File, id: [1; 32], exec: false, size: 1 };
    e.put(&store, &segs("/shared/dir/f"), leaf.clone()).unwrap();
    assert!(e.put(&store, &segs("/shared/dir/f/child"), leaf).is_err(), "file in the way");
    e.rm(&store, &segs("/shared/dir")).unwrap(); // rm removes whole subtree entry
    assert!(e.get(&store, &segs("/shared/dir/f")).unwrap().is_none());
}

fn segs(p: &str) -> Vec<String> { files::paths::canonical(p).unwrap() }
```

- [ ] **Step 2: Run to fail** → FAIL.
- [ ] **Step 3: Implement** (`testkit` facade included).
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** — `feat(duckfs): lazy cow tree edit engine with structural sharing`.

---

### Task 9: Commit execution (the atomic write path) + Stat (for observability)

**Files:**
- Modify: `crates/apps/files/src/lib.rs` (`exec_commit`)
- Modify: `crates/apps/files/src/queries.rs` (implement `Stat` only; the rest stay stubs until Task 11)
- Test: `crates/apps/files/tests/commit.rs`

**Interfaces:**
- Produces: `Files::exec_commit(&mut self, ctx: &mut dyn Ctx, env: &Env, base: Option<DigestHex>, message: String, changes: Vec<Change>) -> Result<(), Error>`; working `FilesQuery::Stat` (path + optional snapshot → `Option<EntryInfo>`; committed state only).
- Consumes: `tree::*`, `paths::*`, `state::Refs`, `objects::*`.

Validation/apply order (all-or-nothing; any failure rejects the whole op with no pending mutation — build into a scratch `Refs`/objects vec and only merge into `self.pending` on success):

1. `message.len() ≤ MAX_MESSAGE_BYTES`; `1 ≤ changes.len() ≤ MAX_CHANGES_PER_COMMIT`.
2. Resolve base: `None` → empty tree; `Some(hex)` → parse via `from_hex_32`, must be the head, in the window, or pinned (`refs` view of the pending block), else `base snapshot not resolvable`.
3. `owner = paths::owner_of(&env.origin)`. Canonicalize every path; authority-check every written path (`Put`/`Mkdir`/`Rm`/`Symlink` path; **both** `Mv.from` and `Mv.to`).
4. Collect all touched paths (Mv touches two); duplicates within the commit reject (`duplicate path in commit`) — order-independence for CAS and apply.
5. Inline budget: total decoded inline bytes ≤ `MAX_INLINE_COMMIT_BYTES` (strict base64; reject bad padding).
6. `Content::Chunks { size, chunks }` consistency: parse each digest; `chunks.len() ≤ MAX_CHUNKS_PER_FILE`; if `size == 0` then `chunks` must be empty (**empty files are legal in duckfs** — deliberate departure from the old module); else `ceil(size / CHUNK_SIZE) == chunks.len()` and `(n-1)*CHUNK_SIZE < size ≤ n*CHUNK_SIZE` (checked math). Every chunk digest must be available: in staging, in the odb, or produced by this commit/pending block.
7. Per-path CAS: `effective_head` = pending refs head (in-block chaining) else committed head. For each touched path: `tree::entry_at(base_root_tree, path) == tree::entry_at(effective_head_root_tree, path)` (compare full `TreeEntry`), else `conflict: <path> changed since base`.
8. Apply through `TreeEdit` on the effective head: inline content → chunk the bytes at `CHUNK_SIZE`, emit `(Kind::Chunk, bytes)` objects + a `FileObj`; `Chunks` content → `FileObj` referencing the ids; `Symlink` → target ≤ `MAX_SYMLINK_TARGET_BYTES`, one chunk object + `FileObj`, entry kind Symlink; `Mv` → `get` + `rm` + `put` (reject if `from` absent or `to` present).
9. Build trees; new `SnapshotObj { root, parent: effective_head, author: owner, consensus_time: env.consensus_time, height: env.height, message }`; compute id; stage all new objects.
10. Merge into pending: push objects; `refs.head = Some(snap_id)`; window push-back (pop-front past `HISTORY_WINDOW`); consume referenced staged chunks (`staging.remove` — quota freed; the chunk object is now reachable via the tree).
11. Watch fan-out: for every touched path × registered `(prefix, module_id)` where `path.starts_with(prefix)`: `ctx.emit_msg(Msg { target: module_id, payload: serde_json::to_vec(&serde_json::json!({"duckfs_notify": {"prefix": prefix, "path": path, "snapshot": to_hex(&snap_id)}})).unwrap() })`.

- [ ] **Step 1: Failing tests** — the table (each row a focused test fn; harness helpers `commit(f, origin, h, base, changes)` and `stat(f, path)` mirror the old suite's shape):

1. inline `Put` at `/shared/hello.txt` → commit_block → `stat` shows kind File, size, exec=false; root moved.
2. staged-chunk `Put` (putblob two chunks, commit references them) → staging drained (root reflects), file readable in Task 11 (assert via stat size).
3. **empty file**: `Content::Chunks { size: 0, chunks: [] }` → Ok; stat size 0.
4. CAS conflict: A commits `/shared/x` at base S0; B (base S0) commits different `/shared/x` next block → reject `conflict`.
5. disjoint chaining: same block, two commits base S0 touching different paths → both Ok; second's snapshot parents onto the first's; both visible after commit_block.
6. `Rm` file; `Rm` absent → reject; `Mv` happy; `Mv` onto existing → reject; `Mkdir` then stat kind Dir; `Symlink` stat kind Symlink.
7. authority: ext:bob writing `/home/ext:alice/x` → reject; system writing `/genesis/seed` → Ok.
8. duplicate path in one commit → reject.
9. unknown chunk digest → reject `chunk not available`.
10. base unresolvable (random hex) → reject.
11. staged add then `abort_block` → root unchanged, refs file untouched, stat none.
12. watch fan-out: register watch (Task 10 not yet — insert directly into refs via `testkit` setter added this task), commit under prefix, assert `TestCtx::emitted` has the notify msg with correct target/payload.
13. rejected op never moves the root (empty-name path, oversized message, >4096 changes — one assertion each mirroring the old `validation_table` style).

- [ ] **Step 2: Run to fail** → FAIL.
- [ ] **Step 3: Implement `exec_commit` + `Stat`.**
- [ ] **Step 4: Run** → `cargo test -p files` all green.
- [ ] **Step 5: Commit** — `feat(duckfs): atomic commit with per-path cas, inline+staged content, watch fan-out; stat query`.

---

### Task 10: Pin / Unpin / Watch / Unwatch

**Files:**
- Modify: `crates/apps/files/src/lib.rs` (four `exec_*` bodies)
- Test: `crates/apps/files/tests/refs_ops.rs`

**Interfaces:**
- Produces: the four executes. Rules:
  - `Pin`: name non-empty ≤ `MAX_PIN_NAME_BYTES`; `pins.len() < MAX_PINS`; name not taken; snapshot hex parses and is resolvable (head / window / already-pinned id); stores `PinEntry { snapshot, owner }`.
  - `Unpin`: entry exists; `owner == pin.owner || owner == "system"`, else reject.
  - `Watch`: origin must be `Origin::Module(m)` with `m == module_id`, or System (may register for any module); prefix canonical-ish (must start with `/`, ≤ `MAX_PATH_BYTES`); `watches.len() < MAX_WATCHES`; duplicate pair rejects.
  - `Unwatch`: pair exists; same origin rule as Watch.
- Consumes: Task 9's commit tests already prove fan-out; this task proves registration gating.

- [ ] **Step 1: Failing tests** — pin happy/duplicate-name/unresolvable/cap; unpin owner-gate (bob can't, alice can, system can); watch module-origin rule (`Origin::External` rejects; `Origin::Module("automations")` registering for `"chat"` rejects; for itself Ok); unwatch gate; all four move/restore the root appropriately (pin→root moves; unpin→root returns).
- [ ] **Step 2: Run to fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** — `feat(duckfs): pins and watches with owner/module gating`.

---

### Task 11: Queries — Ls, Read, Refs (with cursors and snapshot addressing)

**Files:**
- Modify: `crates/apps/files/src/queries.rs`
- Test: `crates/apps/files/tests/queries_core.rs`

**Interfaces:**
- Produces: `pub(crate) fn serve(f: &Files, req: &[u8]) -> Result<Vec<u8>, Error>` handling Stat (from Task 9), Ls, Read, Refs; plus `pub(crate) fn resolve_snapshot(f: &Files, s: &Option<DigestHex>) -> Result<Option<ObjectId>, String>` — `None` → committed head; `Some` → must be head, in window, or pinned, else `snapshot not resolvable`. Queries read **committed** state only (`self.refs`, never pending).
- Semantics:
  - `Ls { path, snapshot, after, limit }`: path must resolve to a Dir (or `/` root); entries in name order, starting strictly after `after`; `limit` clamped to `MAX_PAGE`; `next = Some(last_name)` iff more remain. Each `EntryInfo.meta` is populated for File/Symlink entries by decoding the FileObj; `object` = entry id hex; `path` = full path.
  - `Read { path, snapshot, offset, len }`: File or Symlink only; `len` clamped to `MAX_READ_BYTES`; slice across chunks via odb (chunk index = offset / CHUNK_SIZE); reads past EOF return the available suffix (empty `b64` if `offset ≥ size`); `eof = offset + returned == size`.
  - `Refs {}`: head hex, pin name → snapshot hex map, window length.

- [ ] **Step 1: Failing tests** — commit 300 files under `/shared/bulk/` (inline, distinct bodies) + one `/shared/other`; `Ls` page of 256 with `next`, second page via `after` returns 44 + `next: None`; `Ls` with limit 3; snapshot-addressed `Ls` at an old snapshot shows the old view after later commits; `Read` full round-trip of a 2.5-chunk file staged via putblob (exact bytes reassembled across three reads at `CHUNK_SIZE` boundaries with `eof` flags); `Read` offset past EOF → empty + eof; unresolvable snapshot → `Error::Module`; `Refs` reflects head/pins.
- [ ] **Step 2: Run to fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** — `feat(duckfs): ls/read/refs queries with cursors and snapshot addressing`.

---

### Task 12: Queries — Find, Grep, History, Diff

**Files:**
- Modify: `crates/apps/files/src/queries.rs`
- Test: `crates/apps/files/tests/queries_search.rs`

**Interfaces:**
- Produces the remaining four:
  - `Find { prefix, snapshot, after, limit }`: DFS over the tree, lexicographic full-path order, paths starting with `prefix` (string prefix over the full path), cursor semantics as Ls.
  - `Grep { pattern, prefix, snapshot, cursor, limit }`: **literal substring** match (not regex — deterministic and cheap); pattern non-empty ≤ `MAX_GREP_LINE_BYTES`; scans Files (not symlinks) under `prefix` in path order starting after `cursor`; per-call scan budget `MAX_GREP_SCAN_BYTES` — a file larger than the remaining budget ends the call with `next = Some(its path minus nothing)` i.e. the cursor points at the previous fully-scanned file so the next call resumes at the big file with a fresh budget; a single file larger than the whole budget is skipped deterministically (documented limitation); hit lines are 1-based, truncated to `MAX_GREP_LINE_BYTES`, `uri = evidence_uri(path, snapshot_hex, line)` where `snapshot_hex` is the resolved snapshot id.
  - `History { limit }`: window newest-first (head first), `limit` clamped to `MAX_PAGE`; each `SnapshotInfo` decoded from the odb.
  - `Diff { from, to, prefix }`: both must resolve (head/window/pin); walk both trees pruning shared subtree ids (CoW makes diff cheap); emit `Added`/`Removed`/`Modified` (entry present in both but different `TreeEntry`) sorted by path, filtered by `prefix`. Cap output at `MAX_PAGE * 16` entries, reject beyond with `diff too large, narrow the prefix` (bounded responses).
- Consumes: Task 11's `resolve_snapshot`.

- [ ] **Step 1: Failing tests** — Find prefix + cursor paging; Grep finds a needle on the correct 1-based line with a correct `duck://files/...@...#L..` uri, respects prefix, resumes over the budget boundary (construct with a 2 MiB filler file and tiny needle files around it using a lowered test budget via `testkit`), pattern in a *removed* file at an old snapshot found when snapshot-addressed; History order + limit; Diff added/removed/modified triple with prefix filter, and `Diff` of a snapshot against itself is empty.
- [ ] **Step 2: Run to fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** — `feat(duckfs): find/grep/history/diff with budgets, cursors, and evidence uris`.

---

### Task 13: Garbage collection (`gc.rs`)

**Files:**
- Create: `crates/apps/files/src/gc.rs`
- Modify: `crates/apps/files/src/lib.rs` (`commit_block` trigger)
- Test: `crates/apps/files/tests/gc.rs`

**Interfaces:**
- Produces:
  - `pub(crate) fn mark(refs: &Refs, odb: &Odb) -> Result<BTreeSet<ObjectId>, String>` — roots: head + every window snapshot + every pin snapshot + every staging digest; walk Snapshot→root Tree→entries→File→chunks. A root snapshot object missing from the odb is an error (corruption — never sweep on partial marks); a *parent* pointer of a snapshot is NOT followed (parents are metadata, not GC edges).
  - `pub(crate) fn sweep(odb: &mut Odb, live: &BTreeSet<ObjectId>) -> std::io::Result<u64>` (count removed).
  - Trigger in `commit_block`, after refs persist: `if pending.height / GC_PERIOD_BLOCKS > self.gc_watermark / GC_PERIOD_BLOCKS { run; self.gc_watermark = pending.height; re-save refs envelope }`. GC is consensus-neutral (root covers refs only; unreachable is unreachable on every node) — the watermark is per-node bookkeeping in the refs-file envelope, not in the root preimage. Trigger cadence is deterministic *given the op stream* (first files-commit block past each boundary); nodes may lag each other in wall time but never diverge in state.
- Consumes: `tree::Store` for walks.

- [ ] **Step 1: Failing tests**

1. Reachability property: perform ~50 randomized-but-seeded commits (fixed seed, mix of puts/rms/pins across two owners), force GC via `testkit::force_gc(&mut f)`; then walk head + every window snapshot + every pin fully — every object `get()`s; every staged digest still present.
2. Unpinned history beyond the window is swept: shrink window via `testkit` to 4, make 6 commits each with a unique large-ish file, force GC; snapshots 1–2's exclusive objects are gone from the odb (`list()` shrank; their file objects absent), window snapshots intact; then `History` still serves 4.
3. Pin rescues: same as 2 but pin snapshot 1 → its objects survive.
4. `Read` of a shared chunk still works after the sweep (dedup: same body committed in an old, swept snapshot and a live one — object survives because the live tree references it).

- [ ] **Step 2: Run to fail.**
- [ ] **Step 3: Implement + wire trigger.**
- [ ] **Step 4: Run** → PASS (`cargo test -p files` full suite).
- [ ] **Step 5: Commit** — `feat(duckfs): consensus-neutral mark-and-sweep gc from head+window+pins+staging`.

---

### Task 14: Sync lane, refs snapshot/install, replay discipline (`sync.rs`)

**Files:**
- Create: `crates/apps/files/src/sync.rs`
- Modify: `crates/apps/files/src/lib.rs` (public helpers)
- Test: `crates/apps/files/tests/sync_replay.rs`

**Interfaces:**
- Produces (Phase 2's resolver builds on exactly these — signatures are load-bearing):
  - `pub(crate) fn serve(f: &Files, req: &[u8]) -> Result<Vec<u8>, Error>` — `GetObjects { ids }` (≤ 256 ids per request, reject beyond); per id: absent → `SyncObject { present: false, kind: 0, b64: "" }`; present → kind tag + base64 body. Answered from committed odb, outside any block.
  - `impl Files`:
    - `pub fn snapshot_refs(&self) -> Vec<u8>` — `state::encode_refs(&self.refs)`.
    - `pub fn install_refs(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error>` — strict decode, root check (`files: refs snapshot root mismatch`), replace refs, clear pending, persist envelope with current durable height.
    - `pub fn missing_objects(&self, limit: usize) -> Result<Vec<ObjectId>, Error>` — BFS from the GC roots; collect ids not in the odb up to `limit`; children of missing parents are undiscoverable this round (the fetch loop iterates: install refs → loop { missing → fetch → ingest } until empty).
    - `pub fn ingest_object(&mut self, id: &ObjectId, kind: u8, body: &[u8]) -> Result<(), Error>` — `Kind::from_u8` + `object_id(kind, body) == *id` or reject (`object id mismatch`) — the dishonest-server-proof rule; then odb put.
    - `pub fn possession_complete(&self) -> Result<bool, Error>` — `missing_objects(1)?.is_empty()`.
    - `pub fn durable_height(&self) -> u64`.
- Consumes: everything prior.

- [ ] **Step 1: Failing tests**

```text
1. two_nodes_sync_to_full_possession: source = commits (incl. multi-chunk file,
   pin, watch, staged-but-unconsumed chunk); target = fresh dir;
   target.install_refs(source.snapshot_refs(), source.root());
   loop { m = target.missing_objects(64); fetch via source.serve_sync(GetObjects);
          target.ingest_object each } until possession_complete;
   assert roots equal, Stat/Ls/Read/History replies byte-identical.
2. install_rejects_wrong_root (StateRoot::ZERO) and tampered_object_rejected
   (flip one body byte before ingest → "object id mismatch"; absent objects
   simply stay missing — the loop must not livelock: missing_objects returns
   the same id, test asserts error surfaced after N attempts is a caller concern,
   crate-level we assert ingest rejection).
3. replay_is_idempotent: build state over 3 blocks; capture root + refs file
   bytes; REOPEN Files::open (same dir) → root preserved, durable_height() == 3;
   re-execute block 3's exact ops + commit_block again → same root, same refs
   file payload (envelope may differ only in nothing — assert byte equality).
4. crash_debris: write junk.tmp files into objects/ and a valid refs file;
   reopen; debris gone; root intact.
5. abort_after_execute_persists_nothing: execute a commit, abort_block, reopen
   from disk → root is the pre-block root (refs file was never rewritten).
```

- [ ] **Step 2: Run to fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** — `feat(duckfs): getobjects sync lane, refs install, full-possession walk, idempotent replay`.

---

### Task 15: Host-level e2e, determinism suite, crate docs

**Files:**
- Test: `crates/apps/files/tests/host_e2e.rs`
- Modify: `crates/apps/files/src/lib.rs` (final docblock), `crates/apps/files/src/interface.rs` (doc polish)

**Interfaces:**
- Consumes: `host::Host` (`Host::genesis`, `submit_at`, `query`, `root_hash`) exactly as the old `host_dispatch_moves_root_hash_and_serves_query` test did (git history `tests/files_module.rs:663-699`).

- [ ] **Step 1: Write the e2e tests**

1. `host_flow`: genesis with `Files::open("files", tempdir)`; submit putblob ops + a commit + a pin through `submit_at` with `Origin::External(b"tester")`; root-hash moves on each; `query` Stat/Ls/Read round-trip; owner recorded as `ext:<hex>`.
2. `two_hosts_converge`: two hosts over different tempdirs fed the identical op sequence (interleaved putblobs, commits, a rejected conflict, pins, an unwatch) → equal root-hashes after every block. This is the cross-node determinism gate for the whole crate.
3. `rejects_never_move_root_hash`: every rejection class from Tasks 7–10 submitted through the host; root-hash byte-identical before/after each.

- [ ] **Step 2: Run** → `cargo test -p files` all green.
- [ ] **Step 3: Final crate docblock** — rewrite `lib.rs` header: the two-plane story is gone; document objects/refs/root, the staging→commit byte path, disk-cohort durability, GC neutrality, the sync lane, and a pointer to the spec. Doc-check: `cargo doc -p files --no-deps` clean.
- [ ] **Step 4: Full gates**

Run: `cargo test -p files && cargo check --workspace && cargo fmt -p files -- --check && cargo clippy -p files --tests -- -D warnings`
Expected: all green.

- [ ] **Step 5: Commit**

```bash
git add -A
git -c commit.gpgsign=false commit -m "feat(duckfs): host e2e + cross-host determinism suite; final crate docs

phase 1 (consensus core) complete: objects, odb, refs root, putblob staging,
atomic cas commits, pins/watches, snapshot-addressed queries, gc, sync lane,
idempotent replay.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Phase-boundary note

Task order is 1 → 2 → … → 15 → 16. At the end of this plan the workspace is green, but the node still runs duckfs with an **empty verb surface exposed over HTTP** (noded's generic submit/query lanes work; the dedicated duckfs endpoints, statesync resolver registration, memory-module deletion, and restart/joiner e2e are Phase 2 — planned after this lands and merges to `dev`). Do not delete the memory module in this phase.
