# Layer Contract Standardization — swap-ready seams, sim arms everywhere

**Status:** design approved in-session 2026-07-21. Delivery = open PRs against
`dev` with per-PR review; **PRs stay open — merging is the user's explicit
call** (instruction of 2026-07-21).
**Scope decision:** depth "B" — contract standardization without the simnode
reassembly (recorded below as follow-up "C").

## Goal

Two success criteria, both user-stated:

1. Every layer boundary can be swapped for a mock/stub without touching its
   consumers.
2. The trait surface itself documents the layer separation — opening a layer
   crate shows its contract first.

## What the 2026-07-21 inventory showed

The repo already runs on small, precise boundary traits. These seams are
**already swappable** and are not touched:

| Seam | Contract | Real arm(s) | Existing double(s) |
| --- | --- | --- | --- |
| Ordering | `node::Orderer` (node/src/lib.rs:575) | `SimplexOrderer`, `FollowerOrderer` | `RoundOrderer`, `ArrivalOrderer` |
| Module plane | `sdk::Module` / `sdk::Ctx` / `host::ModuleFactory` | native + `WasmModule` | dozens of test modules |
| Effects | `host::worker::Worker` (host/src/worker.rs:96) | `DispatchPool` | `MockOracle`, `FlakyOracle`, `EchoWorker` |
| Sync transport | `statesync::SyncClient` (statesync/src/lib.rs:1713) | 4 real clients | `ChannelClient`, `StoreClient`, `LiarClient` |
| Data plane | `DataPlaneTransport` (data-plane/src/transport.rs:39) | `OverlaySockets` | `SimEndpoint` (feature `sim`) |
| WG effect | `WireGuardEffect` (wireguard/src/effect.rs:25) | defguard / userspace | `FakeWireGuardEffect` |
| Time/rng/disk | commonware runtime `E` (Clock/Storage/Rng) | `tokio::Context` | `deterministic::Runner` |
| duckfs persistence | `ObjectStore` (duckfs/core/src/store.rs:10) | `DiskStore` | `MemStore` |

The work is therefore **hole-filling plus pattern promotion, not invention**.
The holes:

1. **Mesh carrier hardwired.** `bin/node/src/boot/mesh.rs` builds a concrete
   `authenticated::discovery` Network and hands raw channel pairs to
   `SimplexOrderer::spawn_with_resolver` (bin/node/src/validator/engine.rs:100).
   Multi-validator behavior is testable only by spawning real OS processes over
   real TCP (cluster_e2e, 60–180 s wall-clock budgets).
2. **No shared test `Ctx`.** ~30 hand-rolled `TestCtx`/`CaptureCtx` copies, all
   with `query()` hardwired to `QueryUnsupported`, so sibling-read paths
   (runs→dispatch/saga, automations→chat/tasks, governance→valset/identity)
   cannot be unit-tested.
3. **No in-memory `sdk::MerkleStore`.** The only impl is `QmdbStore`
   (statesync/src/qmdb.rs:435); a one-op assertion drags a real qmdb + tempdir
   or the deterministic runtime.
4. **blobstore is concrete.** `BlobHandle(Arc<Mutex<BlobStore>>)`
   (blobstore/src/lib.rs:150) with zero trait; consumers
   (bin/node/src/{blob_fetch,relay_runtime,explorer}.rs, statesync serve)
   cannot inject an in-mem or failing blob source.
5. **Own-seam bypasses.** `files` hard-binds `Fs<DiskStore>` + `DiskRefs`
   (apps/files/src/module.rs:32/34/53/62) although duckfs core is generic with
   a ready `MemStore`; the indexer calls raw `std::fs` at
   crates/kernel/indexer/src/lib.rs:959–1080.
6. **Wall-clock leaks.** Raw `Instant::now()` off the `Clock` seam:
   bin/node/src/validator/run.rs:300/314/316, run/drain.rs
   :335/339/357/386/996/1206/1251, run/ingress.rs:353/417, and
   crates/kernel/statesync/src/monitor.rs:98/128 — lease/settle/timeout logic
   cannot be advanced by a controlled clock.

## The three standard patterns (the "layer = contract" mechanism)

1. **Boundary traits export at the crate root.** Opening the crate shows the
   contract.
2. **Every boundary trait ships a sim arm in the same crate**, behind feature
   `sim` (precedents: data-plane `transport.rs`/`real.rs`/`sim.rs`,
   nat-traversal `simnat`). No single-impl traits: the sim/stub arm lands in
   the same PR that introduces the trait.
3. **A contract table** — trait / real arm / sim arm / consumers — in the
   README repository-layout section and the system-map artifact. The table is
   the layer-separation explainer.

Explicitly rejected: one mega-trait per layer. The existing small traits are
the right grain; a bundled God-interface would be harder to mock, not easier.

## Seam designs

### 1. `sdk-testkit` (new dev-only crate, `crates/kernel/sdk-testkit`)

- `TestCtx`: implements `sdk::Ctx`. Programmable sibling responses
  (`on_query(module_id, handler)`), env builder (`consensus_time = height`
  convention), captures emitted msgs/events and `set_output`. Replaces the
  ~30 per-crate copies.
- `MemStore`: implements `sdk::MerkleStore` over a `BTreeMap`; root = sha256
  over sorted (k, v) — same shape as `WasmModule` `StateBacking::Map`.
  **Rule: tests never assert root equality across store backends.**
- Consumed as a **dev-dependency only**, never a runtime dep — the Module Rule
  is untouched.

### 2. blobstore contract (`Blobs`)

Trait at the blobstore crate root mirroring `BlobHandle`'s public surface;
`impl Blobs for BlobHandle` (disk, real) + `MemBlobs` (sim arm). Consumers
switch to `Arc<dyn Blobs>`. This is also the foundation the deferred
forge→blobstore seam was waiting for (#687 design note).

### 3. files storage injection

duckfs core gains a small `RefsStore` trait (persist/load `Refs`); `DiskRefs`
implements it; `MemRefs` lands beside `MemStore` in core. The `files` module
becomes generic (`Files<S: ObjectStore, R: RefsStore>`) with
`Files::open(path)` → disk arms and `Files::in_mem()` → mem arms. Host
registration stays `Box<dyn Module>`, so generics do not leak.
Gate: `cargo check -p files --no-default-features` stays green.

### 4. Clock discipline

All wall-clock reads in the validator loop and statesync go through the
commonware `Clock` seam (`context.current()`); the sites listed in hole 6 are
converted. The statesync monitor takes a clock (generic or handle —
implementer's choice, bounded to the crate). A source-parsing lint test bans
`Instant::now(` / `SystemTime::now(` in `bin/node/src/validator` and
`crates/kernel/statesync` so the hole cannot silently reopen.

### 5. Indexer disk seam

A small in-crate disk trait at the indexer crate root (write/read/rename/list
— exactly what the current `std::fs` sites need); the disk arm is the moved
current code, the mem arm sits behind feature `sim`. The indexer tier becomes
drivable on a mock disk.

### 6. Mesh carrier ★

A node-level `MeshCarrier` trait abstracting what `boot/mesh.rs` hands to
`SimplexOrderer::spawn_with_resolver`: the vote/cert/resolver/payload/fetch
channel pairs plus the oracle. Real arm wraps the current
`authenticated::discovery` Network (`MeshHead`); sim arm wraps commonware
`simulated::Network` — already used inside consensus tests, so this is
promotion, not invention. Constraint: `SimplexOrderer::build` is already
generic over the channel types, so consensus-crate changes should be
minimal-to-none; the trait lives at the bin/node boundary.
**Deliverable proof: an in-process multi-validator test** (3 validators, one
process, simulated network) converging to the same root-hash in seconds — the
first mesh-free multi-validator coverage in the repo.

## Exclusions (recorded; do not silently re-add)

- **Block-apply seam + simnode reassembly + genesis unification** → follow-up
  campaign "C". simnode's duplicated commit pipeline and the 4× genesis
  composition remain, guarded by the existing parity tests.
- **Host router trait** — no consumer; testkit `TestCtx` covers the
  module-side need. Speculative.
- **Reachability raw UDP** — right shape is reusing `DataPlaneTransport`, but
  that is a hole-punch rewiring; separate campaign.
- **ModuleRuntime (test-selectable native/wasm)** — the 16 `wasm_*_parity`
  tests already anchor parity.

## Delivery

PR campaign against `dev`; worktrees under `.worktree/`; every PR gets a
clean-context adversarial review; **no PR is merged by the assistant**.

| PR | Content | Depends on |
| --- | --- | --- |
| PR1 | `sdk-testkit` + exemplar conversions (runs, automations, files) + this spec/plan | — |
| PR2 | mechanical sweep: remaining `TestCtx` copies → testkit | PR1 (stacked) |
| PR3 | blobstore `Blobs` + `MemBlobs` + consumer conversion | — |
| PR4 | files `ObjectStore`/`RefsStore` injection | — |
| PR5 | clock discipline + lint test | — |
| PR6 | indexer disk seam | — |
| PR7 | mesh carrier + in-process multi-validator test | — |
| PR8 | contract table (README + docs) | conceptually last |

Gates per PR: `cargo clippy -p <touched> --tests --no-deps` (touch a `.rs`
first — cached-cargo vacuous-gate trap), `cargo test -p <touched>`; PR4 adds
the files wasm gate; PR5 re-runs `cluster_e2e cluster_lifecycle` if drain
timing is touched; PR7's new test is its own proof. No wasm guest
regeneration expected: no wire or consensus-byte changes (testkit is
dev-only; the files change is native-side wiring).

## Risks

- **PR7 generics explosion** (commonware channel types). Mitigation: the
  trait lives at the bin/node boundary, only two impls must compile,
  associated types per channel pair.
- **PR2 churn vs in-flight branches.** Mechanical, per-crate commits,
  rebase-friendly.
- **MemStore root ≠ QmdbStore root.** Documented rule above; reviews check
  that no test asserts cross-backend root equality.
- **Clock threading ripple in statesync.** Bounded to the crate; the monitor
  is a leaf.
