# Layer Contract Standardization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fill the six mockability holes and promote the trait/real/sim
pattern to the whole-tree standard, per
`docs/superpowers/specs/2026-07-21-layer-contract-standardization-design.md`.

**Architecture:** Small boundary traits at crate roots, each shipping a sim
arm in the same crate (feature `sim`); a shared dev-only `sdk-testkit` crate
replaces ~30 hand-rolled `Ctx` doubles; the mesh carrier gains a node-level
trait so multi-validator consensus runs in-process for the first time.

**Tech Stack:** Rust, async-trait (`?Send`, matching sdk), commonware
runtime/p2p (`simulated::Network` for the sim mesh arm), qmdb via existing
`MerkleStore`.

## Global Constraints

- **Delivery: every task ends with an OPEN PR against `dev`. NEVER merge —
  merging is the user's explicit call (2026-07-21 instruction).**
- Worktrees under `<primary-checkout>/.worktree/<branch-slug>`, forked from
  `origin/dev`. `CARGO_INCREMENTAL=0` (box rustc segfault fix).
- Gates per task: `touch` a `.rs` first (cached-cargo vacuous-gate trap), then
  `cargo clippy -p <touched-crate> --tests --no-deps` and
  `cargo test -p <touched-crate>`.
- Feature name for sim arms is `sim` (data-plane precedent). Never label
  anything "v2"/"legacy"; no backcompat shims (standing mandate).
- `sdk-testkit` may only ever appear in `[dev-dependencies]` — the Module Rule
  (modules link only sdk + wire surfaces) is untouched.
- Tests wait on events, never time. No sleep/spin loops.
- `tracing`, never `println!`, in node/kernel code.
- Commits end with `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`;
  PR bodies end with the standard Claude Code trailer.
- Known env-fails to not chase: voice/overlay_e2e audio, cluster_e2e
  reachability subtest, simnode governance_scenarios.

---

### Task 1: `sdk-testkit` crate + exemplar conversions (PR1)

Branch: `feat/sdk-testkit`. PR title: `feat(kernel): sdk-testkit — shared TestCtx + in-memory MerkleStore`.
This PR also carries the spec and this plan (`git add docs/superpowers/{specs,plans}/2026-07-21-*`).

**Files:**
- Create: `crates/kernel/sdk-testkit/Cargo.toml`, `crates/kernel/sdk-testkit/src/lib.rs`
- Modify: root `Cargo.toml` workspace members; `[dev-dependencies]` of
  `crates/modules/apps/runs`, `crates/modules/apps/automations`,
  `crates/modules/apps/files`
- Convert: `crates/modules/apps/runs/src/tests/mod.rs:38` (`CaptureCtx`),
  `crates/modules/apps/automations/src/tests.rs:45`,
  `crates/modules/apps/files/tests/harness/mod.rs:12`

**Interfaces (Produces — later tasks and PR2 rely on these exact names):**
```rust
// crates/kernel/sdk-testkit/src/lib.rs
pub struct TestCtx { /* env, handlers, captures */ }
impl TestCtx {
    pub fn at_height(height: u64) -> Self;          // consensus_time = height convention
    pub fn with_env(env: sdk::Env) -> Self;
    /// Programmable sibling response; handler gets the query payload bytes.
    pub fn on_query(self, target: &str,
        handler: impl FnMut(&[u8]) -> Result<Vec<u8>, sdk::QueryError> + 'static) -> Self;
    pub fn msgs(&self) -> &[sdk::Msg];              // captured emit_msg
    pub fn events(&self) -> &[sdk::Event];          // captured emit_event
    pub fn output(&self) -> Option<&[u8]>;          // captured set_output
}
// impl sdk::Ctx for TestCtx — mirror crates/kernel/sdk/src/lib.rs:385 exactly
// (env/module_root/query/emit_msg/emit_event/relay/set_output/author_origin).
// query() routes to registered handlers; unregistered target => QueryUnsupported
// (same default the 30 hand-rolled copies use today).

pub struct MemStore { /* BTreeMap<Vec<u8>, Vec<u8>> */ }
impl MemStore { pub fn new() -> Self; }
// impl sdk::MerkleStore for MemStore — mirror sdk/src/lib.rs:444
// (get/commit_batch/root/sync_target/serve_sync). root = sha256 over sorted
// (key, value) pairs — the WasmModule StateBacking::Map shape. sync_target /
// serve_sync return the same unsupported answers guest WitStore gives
// (crates/guests/guest-adapter/src/lib.rs:196,216-228).
```

- [ ] **Step 1: failing test.** In `sdk-testkit` itself: a test module with a
  toy `impl Module` that queries a sibling via `ctx.query("dispatch", …)` and
  asserts `TestCtx::on_query` served it, plus a `MemStore` round-trip test
  (`commit_batch` then `get`, root changes and is deterministic across two
  identically-filled stores). Run: `cargo test -p sdk-testkit` — FAIL (crate empty).
- [ ] **Step 2: implement `TestCtx` + `MemStore`** per the interface block.
  Read `crates/kernel/sdk/src/lib.rs:385-475` first and mirror signatures
  exactly (`#[async_trait(?Send)]`).
- [ ] **Step 3: gates.** `cargo clippy -p sdk-testkit --tests --no-deps`,
  `cargo test -p sdk-testkit` — PASS. Commit.
- [ ] **Step 4: exemplar conversions.** Replace the three listed hand-rolled
  doubles with `sdk-testkit::TestCtx`, preserving each test's behavior. Where
  runs/automations previously could NOT test a sibling read (query hardwired
  to `QueryUnsupported`), add ONE new test each exercising a real sibling
  response via `on_query` (runs→dispatch, automations→chat) — the new
  capability this crate exists for.
- [ ] **Step 5: gates for touched crates** (`-p runs -p automations -p files`,
  clippy + test; plus `cargo check -p files --no-default-features` since files
  was touched). Commit.
- [ ] **Step 6: open PR** against `dev` (spec+plan included). Request
  clean-context review; fix findings; leave OPEN.

### Task 2: TestCtx sweep (PR2, stacked on PR1)

Branch: `feat/testkit-sweep` forked FROM `feat/sdk-testkit`; PR base =
`feat/sdk-testkit` (stacked — do NOT base on dev, PR1 is unmerged).
PR title: `refactor(tests): adopt sdk-testkit TestCtx tree-wide`.

**Files (the remaining hand-rolled `impl sdk::Ctx` doubles — from the
2026-07-21 inventory; re-grep `impl Ctx for` + `impl sdk::Ctx` to catch
drift):** tasks `task_module.rs:61` & `job_module.rs:110`, inbox
`inbox_module.rs:98`, dispatch `src/lib.rs:944` (`CaptureCtx`), saga
`src/lib.rs:1237`, identity `src/tests.rs:14`, kv `src/lib.rs:318`, valset
`src/lib.rs:391`, capability `src/lib.rs:609`, tagging `src/lib.rs:483`,
chat `channel_system.rs:24`, forge `src/lib.rs:916`, lifecycle
`src/tests.rs:10`, pages `src/tests/mod.rs:37`, examples/directory
`tests/…:31`, forge integration `tests/*:49`, duckfs-client
`tests/support/mod.rs:46`, plus any others the grep finds.

- [ ] **Step 1:** per crate: add `sdk-testkit` dev-dep, delete the local
  double, adopt `TestCtx`, keep assertions identical. One commit per crate
  (rebase-friendly).
- [ ] **Step 2:** capture-style doubles (dispatch/runs `CaptureCtx`) map to
  `TestCtx::msgs()/events()`; doubles with bespoke behavior beyond
  capture+query get the closest `TestCtx` composition — if one genuinely
  cannot be expressed, KEEP it and note why in the PR body (do not force).
- [ ] **Step 3: gates.** clippy + test for every touched crate (batch:
  `cargo clippy --workspace --tests --no-deps` is acceptable here; list any
  pre-existing red in the PR body).
- [ ] **Step 4: open stacked PR**; review; leave OPEN.

### Task 3: blobstore `Blobs` contract (PR3)

Branch: `feat/blobs-trait`. PR title: `feat(blobstore): Blobs trait — disk + mem arms, dyn consumers`.

**Files:**
- Modify: `crates/modules/system/blobstore/src/lib.rs` (trait at root;
  `impl Blobs for BlobHandle`; `MemBlobs` behind `#[cfg(any(test, feature = "sim"))]`)
- Modify consumers to `Arc<dyn Blobs>`: `bin/node/src/blob_fetch.rs`,
  `bin/node/src/relay_runtime.rs`, `bin/node/src/explorer.rs`, the statesync
  serve wiring in `bin/node/src/host_state.rs`, plus `bin/simnode/src/lib.rs`
  blob uses (`:455/:1133/:1305`) if signatures ripple there.

**Interfaces (Produces):**
```rust
pub trait Blobs: Send + Sync + 'static { /* mirror BlobHandle's current
    public surface exactly — read blobstore/src/lib.rs:150+ and lift each
    public method verbatim (put_chunk, get, has, …). No new semantics. */ }
pub struct MemBlobs { /* BTreeMap<[u8;32], Vec<u8>> */ }
```

- [ ] **Step 1: failing test.** In blobstore: `MemBlobs` passes the same
  put/get/content-address assertions `BlobHandle` unit tests use (write one
  shared `fn exercise(blobs: &dyn Blobs)` helper and run it against BOTH arms
  — the contract test pattern).
- [ ] **Step 2:** define trait, impl both arms, run: PASS. Commit.
- [ ] **Step 3:** convert consumers to `Arc<dyn Blobs>`; `cargo check -p
  node-bin -p noded -p simnode`; clippy + test touched crates. Commit.
- [ ] **Step 4: open PR**; review; leave OPEN. PR body notes this is the
  foundation for the deferred forge→blobstore seam (#687).

### Task 4: files storage injection (PR4)

Branch: `feat/files-store-injection`. PR title: `feat(files): inject ObjectStore/RefsStore — mem-backed files module`.

**Files:**
- Modify: `crates/duckfs/core/src/state.rs` (or sibling) — `RefsStore` trait +
  `MemRefs`; `crates/duckfs/disk/src/disk.rs` — `impl RefsStore for DiskRefs`;
  `crates/modules/apps/files/src/module.rs` — genericize.

**Interfaces (Produces):**
```rust
// duckfs core, exported at crate root:
pub trait RefsStore { fn load(&self) -> Result<Refs, Error>;
                      fn save(&self, refs: &Refs) -> Result<(), Error>; }
pub struct MemRefs { /* Mutex<Refs> */ }
// files module:
pub struct Files<S: ObjectStore = DiskStore, R: RefsStore = DiskRefs> { … }
impl Files { pub fn open(path: &Path) -> … }         // unchanged signature
impl Files<MemStore, MemRefs> { pub fn in_mem() -> Self; }
```
Adjust the trait's exact error/return types to what `DiskRefs` does today —
lift, don't redesign. Host registration stays `Box<dyn Module>`.

- [ ] **Step 1: failing test.** `Files::in_mem()` executes one manifest op and
  serves the matching query with zero disk (assert no tempdir needed).
- [ ] **Step 2:** implement trait + genericize module. Run files tests: PASS.
- [ ] **Step 3: gates.** `cargo clippy -p files -p duckfs-core -p duckfs-disk
  --tests --no-deps`; `cargo test -p files`; **`cargo check -p files
  --no-default-features` (wasm-readiness — MUST stay green; `RefsStore`/`MemRefs`
  are pure, no std::fs in core)**; `cargo check -p node-bin`. Commit.
- [ ] **Step 4: open PR**; review; leave OPEN.

### Task 5: clock discipline (PR5)

Branch: `fix/clock-seam`. PR title: `fix(kernel): route wall-clock reads through the Clock seam + lint test`.

**Files:**
- Modify (every raw `Instant::now()` site): `bin/node/src/validator/run.rs:300/314/316`,
  `bin/node/src/validator/run/drain.rs:335/339/357/386/996/1206/1251`,
  `bin/node/src/validator/run/ingress.rs:353/417`,
  `crates/kernel/statesync/src/monitor.rs:98/128`
- Create: lint tests `bin/node/tests/clock_lint.rs`,
  `crates/kernel/statesync/tests/clock_lint.rs`

**Approach:** the validator loop already holds a commonware context — replace
`Instant::now()` with `context.current()` (`Clock`), carrying `SystemTime`
where an `Instant` was stored (durations via `duration_since`). The statesync
monitor becomes generic over `C: Clock` like the rest of statesync
(`QmdbStore<E>` precedent) — callers pass the context they already own.
Behavior-preserving: same intervals, same comparisons.

- [ ] **Step 1: lint tests first** (they are the spec's enforcement): walk
  `bin/node/src/validator/**/*.rs` / `crates/kernel/statesync/src/**/*.rs`
  sources at test time and fail on `Instant::now(` or `SystemTime::now(`.
  Run — FAIL (sites exist).
- [ ] **Step 2:** convert the sites. Lint tests PASS.
- [ ] **Step 3: gates.** clippy + `cargo test -p statesync`; validator unit
  lane `cargo test -p node-bin --bin ducktape-node`; because drain timing was
  touched, run `cargo test -p node-bin --test cluster_e2e cluster_lifecycle`
  (known env-fail subtests excepted). Commit.
- [ ] **Step 4: open PR**; review; leave OPEN.

### Task 6: indexer disk seam (PR6)

Branch: `feat/indexer-disk-seam`. PR title: `feat(indexer): disk trait — mem arm for the derived tier`.

**Files:**
- Modify: `crates/kernel/indexer/src/lib.rs` (raw `std::fs` at
  :959,966,1016,1018,1025,1036,1037,1051,1069,1076,1078,1080)
- Create: `crates/kernel/indexer/src/disk.rs` (real arm = today's code moved),
  `crates/kernel/indexer/src/mem.rs` (`#[cfg(any(test, feature = "sim"))]`)

**Interfaces (Produces):** a root-exported trait named for what the sites
actually need — read them first; expected shape ≈
```rust
pub trait IndexDisk: Send + Sync { fn read(&self, path) -> …; fn write(&self, path, bytes) -> …;
    fn rename(&self, from, to) -> …; fn create_dir_all(&self, path) -> …; /* only ops the 12 sites use */ }
```
`IndexStore` holds `Box<dyn IndexDisk>`; constructors default to the disk arm
so existing callers don't change.

- [ ] **Step 1: failing test.** An `IndexStore` apply/scan round-trip on
  `MemDisk` (no tempdir).
- [ ] **Step 2:** extract trait, move disk code, add mem arm. PASS.
- [ ] **Step 3: gates.** clippy + `cargo test -p indexer`; `cargo check -p
  noded -p node-bin`. Commit.
- [ ] **Step 4: open PR**; review; leave OPEN.

### Task 7: mesh carrier (PR7) — the flagship

Branch: `feat/mesh-carrier`. PR title: `feat(consensus): MeshCarrier seam — in-process multi-validator consensus`.

**Files:**
- Modify: `crates/kernel/consensus/src/lib.rs` (trait at root; sim arm behind
  `feature = "sim"` wrapping commonware `simulated::Network` — promote the
  wiring already used in `consensus/tests`), `bin/node/src/boot/mesh.rs` +
  `bin/node/src/validator/engine.rs` (real arm: `MeshHead` implements the trait;
  `spawn_with_resolver` consumes the trait instead of loose channel pairs)
- Create: `crates/kernel/consensus/tests/in_process_cluster.rs`

**Design constraints (from spec):** `SimplexOrderer::build` is already
generic over the channel pairs (`consensus/src/lib.rs:1273-1278`) — the trait
is a *named bundle* of what `boot/mesh.rs` produces today (vote/cert/resolver/
payload/fetch sender-receiver pairs + oracle registration), associated types
per pair. Consensus-crate signature changes minimal; NO change to ordering
semantics or frame bytes. If the generics fight back, the fallback shape is a
concrete enum of the two carriers — flag it in the PR body rather than
force-fitting (ask-first rule for boundary shape changes).

- [ ] **Step 1: failing test** (`in_process_cluster.rs`): 3 validators — 3
  `OrderedNode<SimplexOrderer>` over `Host`s with the native `directory`
  example module — on one `simulated::Network` in one process; submit ops via
  each node; wait on delivered frames (event-driven, no sleeps); assert all
  app-hashes equal. FAIL (no way to construct without the carrier).
- [ ] **Step 2:** define `MeshCarrier`, implement sim arm, make the test pass.
- [ ] **Step 3:** convert `boot/mesh.rs`/`engine.rs` to the real arm.
  `cargo check -p node-bin`; clippy consensus + node-bin; `cargo test -p
  consensus`; `cargo test -p node-bin --test cluster_e2e cluster_lifecycle`
  (real-mesh regression). Commit.
- [ ] **Step 4: open PR**; review; leave OPEN.

### Task 8: contract table (PR8)

Branch: `docs/layer-contract-table`. PR title: `docs: layer contract table — trait / real arm / sim arm / consumers`.

**Files:** `README.md` (Repository Layout section gains the table);
`docs/superpowers/specs/2026-07-21-layer-contract-standardization-design.md`
already carries the rationale — link it.

- [ ] **Step 1:** write the table covering BOTH pre-existing seams (Orderer,
  Module/Ctx, Worker, SyncClient, DataPlaneTransport, WireGuardEffect,
  ObjectStore, runtime `E`) and the six new ones (testkit doubles, Blobs,
  RefsStore, Clock discipline, IndexDisk, MeshCarrier), one row each:
  contract, real arm, sim arm, consumers. Note rows PR1–PR7 introduce as
  "(this campaign, PR #N)" since none are merged yet.
- [ ] **Step 2:** `cd docs && bun run docs:check` only if docs/src/content/docs touched;
  README-only change needs no gate. Open PR; review; leave OPEN.

---

## Self-review (done at write time)

- Spec coverage: 6 seams → Tasks 1–7; 3 patterns → trait-at-root (each task),
  `sim` feature (Tasks 3/6/7), contract table (Task 8); exclusions honored
  (no simnode/router/UDP/ModuleRuntime tasks). PR map matches spec's.
- No-merge rule stated globally and per task.
- Type consistency: `TestCtx`/`MemStore` names used identically in Tasks 1–2;
  `Blobs`/`MemBlobs`, `RefsStore`/`MemRefs`, `IndexDisk`, `MeshCarrier`
  defined once each, no cross-task drift.
