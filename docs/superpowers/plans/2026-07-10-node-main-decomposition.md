# Node main.rs Decomposition Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Carve `bin/node/src/main.rs` (8,641 lines; `run_node` alone is 5,614 lines, 3027–8641) into a standard domain-module layout per issue #213, leaving `main.rs` as a thin conductor.

**Architecture:** Three sequential PRs of pure-move commits. PR 1 relocates the ~55 pre-`run_node` helpers into their natural domain homes (the exact pattern of precedent commit `3997c92f refactor(node): split main responsibilities`). PR 2 extracts `run_node`'s boot prelude (`boot/`) and the entire joiner/replica role (`replica/`). PR 3 extracts the validator role (`validator/`). Modules are split at **natural responsibility boundaries only** — one concept per module, directory modules per role/domain, no cap-driven fragmentation. State crossing phase seams travels in carrier structs (`BootEnv`, `Surfaces`, `MeshHead`), following the existing `NetworkBindings`/`SyncSubstrates` precedent in `host_state.rs`.

**Tech Stack:** Rust bin crate `node-bin` (binary `ducktape-node`), commonware runtime, tokio. No new dependencies, no new crates (the code is all one-consumer wiring; a lib split would be ceremony).

## Global Constraints

- **Move-only discipline (issue #213, verbatim):** "no behavior or wire changes, suites stay green per split commit". Code is relocated verbatim; the only permitted rewrites are visibility (`pub(crate)`/`pub(super)`), `use` paths, the two closure→fn/struct conversions explicitly called out (Tasks C2, C3), and carrier-struct pack/unpack at seams.
- **Natural boundaries over the 600-line soft cap (user directive for this refactor):** a module that is one cohesive responsibility (one event loop, one plane) stays whole even when large. `replica/park.rs` (~1.7k, the park loop) and `validator/run.rs` (~1.8k, the consensus loop + its state) are accepted as-is and flagged in the PR bodies. Do NOT invent intra-loop seams to satisfy the cap.
- **Lint gate per commit:** `touch bin/node/src/*.rs bin/node/src/**/*.rs && cargo clippy -p node-bin --tests --no-deps` → 0 new warnings. The `touch` is mandatory — cached clippy passes vacuously (nothing recompiles).
- **Unit-test gate per commit:** `cargo test -p node-bin --bin ducktape-node` → all pass (runs `main_tests`, `joiner_mesh_tests`, and inline module tests).
- **E2E gate per PR (not per commit):** `cargo test -p node-bin` → all pass (spawns real `ducktape-node` binaries; serialized via `common::serial()`, slow — run once per PR before opening it, plus after PR 3's Task C4).
- **No `cargo fmt --all`.** Format only files you touched.
- **Branching:** each PR works in an isolated worktree forked from `origin/dev`, PR based on `dev`, clean-context review before merge (CLAUDE.md rules). PR 2 forks after PR 1 merges; PR 3 after PR 2.
- **`fn main` / `fn run` / `init_tracing` stay in `main.rs`** (binary entry concerns).
- **Line anchors:** all line numbers below are valid at dev `1d2111ca`. Before cutting each block, re-anchor with the task's grep landmark — do not trust raw numbers after any upstream merge.
- **Visibility recipe:** moved items become `pub(crate)` (or `pub(super)` for role-internal items). `main.rs` re-imports what `run_node` still needs with plain `use <module>::{…};` lines — `main_tests.rs` does `use super::*;`, which picks up those imports, so tests keep compiling with zero test-file edits. Exception: `joiner_mesh_tests.rs` names `super::joiner_epoch_mesh` explicitly (Task A3 fixes that one `use`). Do NOT remove the existing external re-imports (`directory::{encode_msg, …}`, `recovery::{Manifest, Recovery}`) from `main.rs`'s use block — `main_tests` reaches them through the glob.
- **Struct fields read by tests need field-level `pub(crate)`:** `ReadinessSignaller::signaled`, `ManifestFetchRetry::{log_line, announce}`, `PostRebootCatchupApply::{applied, blocks}` (+ `IndexFold`'s fields, constructed from `run_node`).

## Target File Structure

Standard Rust bin layout: flat modules for shared cross-role concerns, directory modules per role/domain. Existing modules (`cli.rs`, `config.rs`, `voice.rs`, `host_state.rs`, `relay_runtime.rs`, …) are untouched.

```
bin/node/src/
  main.rs                  # mod decls, use lines, main(), run(), init_tracing, run_node() conductor (~350–500 final)
  util.rs                  # hex, unix_ms, diag_log, epoch_floor, participant_bytes, resident_bytes
  constants.rs             # channel ids + engine_channels(), MODULE_IDS, timings, size caps (+static asserts)
  host_reads.rs            # read-model over Host: read_valset_*, read_upgrade_*, read_members/redemptions, joiner_epoch_mesh, resume_*_keys
  rpc.rs                   # operator RPC surface: RpcRequest/Reply/Status/Job, JoinRequest{Record,View}, spawn_rpc_listener
  explorer.rs              # derived index/explorer fold: explorer_root_op, *_block_row, IndexFold, heal/ship/stage index
  reachability_plane.rs    # wire_reachability_plane + reachability_plane (matches voice_plane.rs / statesync_plane.rs)
  sync/
    mod.rs                 # pub(crate) mod serve; pub(crate) mod catchup;
    serve.rs               # floor verify, boundary checkpoints, replica backfill/verifier, SyncStateRequest, drive_sync_request
    catchup.rs             # post-reboot catch-up chain, PostRebootCatchup*, BootP2pSyncClient, derive_pending_boot
  boot/
    mod.rs                 # pub(crate) mod env; mod surfaces; mod mesh; mod sync_only;
    env.rs                 # P0: BootEnv carrier + derive()
    surfaces.rs            # P1: Surfaces carrier + bind() (listeners + service threads)
    mesh.rs                # P3: MeshHead carrier + build() (discovery/overlay/Network/oracle)
    sync_only.rs           # P4: terminal sync-only mode
  replica/
    mod.rs                 # role entry run(...) -> ! ; reboot_self
    promotion.rs           # PromotionBoundary{,Source}, ManifestFetchRetry, choose_promotion_boundary
    wiring.rs              # P6a: channel registration, first-contact, lobby, network.start()
    park.rs                # P6b–P6d: serve-state + park loop + promotion checkpoint (one loop = one file)
  validator/
    mod.rs                 # role entry: boot → wiring → engine → run
    announce.rs            # ReadinessSignaller, CapabilityAnnouncer, dispatch_pending_deliveries, saga_next_expiry
    boot.rs                # P7: genesis-vs-restore BootState + P11: post-reboot catch-up
    wiring.rs              # P8–P10: membership, channel bank, media/reachability, network.start() + P12 ingress bridges
    engine.rs              # EpochSpawner (ex spawn_epoch closure) + OrderedNode/orchestrator resume
    run.rs                 # P13+P14: ValidatorLoopState + the consensus select loop (one loop = one file)
```

Rationale for the flat ones: `util`/`constants` are the standard tiny leaf modules; `host_reads`, `rpc`, `explorer`, `sync/` are consumed by BOTH roles so they cannot live under either role directory; `reachability_plane.rs` follows the existing `*_plane.rs` sibling convention.

## Decisions Taken (flag if you disagree)

1. **Three PRs, sequential** — per #213's "small reviewable diffs beat one mega-refactor". Each PR leaves the tree green and reviewable alone.
2. **`replica/park.rs` and `validator/run.rs` stay whole** (one event loop each, ~1.7–1.8k lines). Natural unit beats the cap per the user's directive; recorded in each PR body. If a later need arises, the loops' internal lanes (drain pass, detection lane) are the seams — follow-up, not this plan.
3. **Only two closure conversions** (they cross module seams): `spawn_epoch` → `EpochSpawner` struct (Task C3), `mesh_at` → free fn (Task C2). `send_announce`/`not_serving`/`graceful_checkpoint!` stay as they are — they live and die inside one module now.
4. **The replica/validator drain-mirror duplication (hazard 6) is NOT unified** — unifying across `FollowerOrderer`/`SimplexOrderer` generics is a behavior-risk refactor, out of scope for move-only. File a follow-up issue after PR 3.
5. **No `lib.rs`, no new crates** — nothing outside this binary consumes the wiring; e2e tests drive the built binary, not the crate API.

---

# PR 1 — Shared helpers to their domain homes (`refactor/node-main-helpers`)

Setup: create worktree from `origin/dev`, branch `refactor/node-main-helpers`. Every task below is one commit. Task order is dependency-first: A1 → A2 → … → A9.

**Shared step recipe** (referenced as "Gate + Commit" in each task; run exactly this):

```bash
touch bin/node/src/*.rs bin/node/src/**/*.rs 2>/dev/null
cargo clippy -p node-bin --tests --no-deps    # expect: finishes with 0 warnings
cargo test -p node-bin --bin ducktape-node    # expect: all unit tests pass
git add -A bin/node/src && git commit -m "<message from task>"
```

Sanity check per commit: `git show --stat HEAD` — lines removed from `main.rs` ≈ lines added to the new module (± use/visibility lines). A large mismatch means dropped or duplicated code.

### Task A1: `util.rs` — leaf utilities

**Files:**
- Create: `bin/node/src/util.rs`
- Modify: `bin/node/src/main.rs`

**Interfaces:**
- Produces: `pub(crate) fn hex(root: &StateRoot) -> String`, `pub(crate) fn unix_ms() -> u64`, `pub(crate) fn diag_log(line: impl AsRef<str>)`, `pub(crate) fn epoch_floor(namespace: &[u8], epoch: u64) -> Digest`, `pub(crate) fn participant_bytes(…)`, `pub(crate) fn resident_bytes(…)`
- Consumed by: every later module + `run_node` + `main_tests` (via the main.rs use-glob).

- [ ] **Step 1: Move the items.** Cut from `main.rs` (landmarks; re-grep each): `epoch_floor` (278–291), `participant_bytes` (292–301), `resident_bytes` (302–313), `hex` (682–685), `diag_log` (776–796), `unix_ms` (2177–2184). Paste verbatim into new `bin/node/src/util.rs`, prefixing each `fn` with `pub(crate)`. Copy the exact `use`s these fns need from main.rs's use block (StateRoot, Digest + hashing types, SystemTime, std::io bits for diag_log); compile errors name any missed.
- [ ] **Step 2: Rewire `main.rs`.** Add `mod util;` beside the existing mod block (lines 65–86) and `use util::{diag_log, epoch_floor, hex, participant_bytes, resident_bytes, unix_ms};` beside the existing `use host_state::{…};` (line 88).
- [ ] **Step 3: Gate + Commit** — `refactor(node): move leaf utilities out of main.rs`

### Task A2: `constants.rs` — wiring constants + channel math

**Files:**
- Create: `bin/node/src/constants.rs`
- Modify: `bin/node/src/main.rs`

**Interfaces:**
- Produces (all `pub(crate)`): consts `CONSENSUS_SCHEME`, `MAX_PROTOCOL_VERSION`, `PEER_SET`, `NOP_TARGET`, `BOOT_SYNC_REQUEST_TIMEOUT`, `NUDGE_INTERVAL`, `POST_REBOOT_CATCHUP_MAX_ITERS`, `POST_REBOOT_CATCHUP_MAX_ATTEMPTS`, `MAX_MESSAGE_SIZE`, `MAX_BACKLOG`, `DRAIN_TICK`, `CHANNEL_SUBMIT_RELAY`, `CHANNEL_STATE_SYNC`, `CHANNEL_LOBBY`, `CHANNEL_REACHABILITY`, `CHANNEL_VOICE`, `CHANNEL_VIDEO`, `LOBBY_ANNOUNCE_EVERY`, `JOINER_POLL`, `RESIDENT_FALLBACK_POLL`, `EPOCH_CHANNEL_BANK`, `CUTOVER_DELAY`, `MODULE_IDS`, `SUBMIT_HOLD`; plus `pub(crate) fn engine_channels(epoch: u64) -> (u64, u64, u64, u64, u64)` (channel-bank math lives next to the channel ids it derives from).

- [ ] **Step 1: Move.** Cut the constants region of `main.rs` (94–262, landmark: `const CONSENSUS_SCHEME` through `const SUBMIT_HOLD`) — **including the two `const _: () = assert!(…)` static asserts at 163–164 and every doc comment** — plus `engine_channels` (270–277), into `bin/node/src/constants.rs`. The asserts reference `node::MAX_FRAME_BYTES` and `duckfs_core::MAX_SYNC_REPLY_BYTES`; bring those `use`s along.
- [ ] **Step 2: Rewire `main.rs`.** `mod constants;` + `use constants::*;` (glob is right here: 25 crate-internal items; it also keeps the `main_tests` glob chain intact).
- [ ] **Step 3: Gate + Commit** — `refactor(node): move wiring constants out of main.rs`

### Task A3: `host_reads.rs` — read-model over Host

**Files:**
- Create: `bin/node/src/host_reads.rs`
- Modify: `bin/node/src/main.rs`, `bin/node/src/joiner_mesh_tests.rs`

**Interfaces:**
- Produces (all `pub(crate)`): `async fn read_valset_members(host: &Host) -> Vec<Vec<u8>>`, `async fn read_valset_residents(…)`, `fn joiner_epoch_mesh(…)`, `async fn read_upgrade_state(…) -> consensus::BoundaryUpgrade<ed25519::PublicKey>`, `async fn read_upgrade_version_fields(…) -> (u32, Option<sdk::UpgradeCoords>)`, `async fn read_members_from_host(…)`, `async fn read_upgrade_status_raw(…) -> Option<upgrade::UpgradeStatus>`, `async fn read_redemptions_from_host(…) -> Vec<governance::RedemptionView>`, `fn resume_member_keys(…)`, `fn resume_resident_keys(…)`

- [ ] **Step 1: Move.** Cut `read_valset_members` (314) through `read_redemptions_from_host` (479–499) as one contiguous region, plus `resume_member_keys` (2044–2063) and `resume_resident_keys` (2064–2080), into `bin/node/src/host_reads.rs`. `pub(crate)` on each.
- [ ] **Step 2: Rewire.** `mod host_reads;` + `use host_reads::{joiner_epoch_mesh, read_members_from_host, read_redemptions_from_host, read_upgrade_state, read_upgrade_status_raw, read_upgrade_version_fields, read_valset_members, read_valset_residents, resume_member_keys, resume_resident_keys};` in `main.rs`.
- [ ] **Step 3: Fix the one explicit test import.** In `joiner_mesh_tests.rs`, change `use super::joiner_epoch_mesh;` → `use crate::host_reads::joiner_epoch_mesh;`.
- [ ] **Step 4: Gate + Commit** — `refactor(node): move host/valset read helpers out of main.rs`

### Task A4: `sync/serve.rs` — statesync serve/floor/checkpoint machinery

**Files:**
- Create: `bin/node/src/sync/mod.rs`, `bin/node/src/sync/serve.rs`
- Modify: `bin/node/src/main.rs`

**Interfaces:**
- `sync/mod.rs`: `pub(crate) mod serve;` (Task A5 adds `pub(crate) mod catchup;`)
- `serve.rs` produces (all `pub(crate)`): `fn assert_floor_binds_view(…)`, `fn reopen_preflight_synced_host(host: &Host, expected: StateRoot) -> Result<(), String>`, `fn verify_manifest_floor(…)`, `async fn reopen_recovery(…)`, `type ServedSeal = (…)`, `async fn replica_backfill<C>(…)`, `fn replica_verifier(namespace: &[u8], participant_keys: &[Vec<u8>]) -> simplex_ed25519::Scheme`, `fn replica_orchestrator_at(…)`, `async fn write_boundary_checkpoint<E>(…)`, `fn to_node_disposition(disposition: statesync::FrameDisposition) -> node::Disposition`, `fn to_sync_disposition(…)`, `fn recovery_frame_to_sync(…)`, `enum SyncStateRequest`, `struct SyncBoundary`, `async fn drive_sync_request(…)` — keep the existing generic params (`<C>`, `<E>`) exactly.

- [ ] **Step 1: Move.** Cut lines 686–702 (`assert_floor_binds_view`) and 797–1291 (`reopen_preflight_synced_host` through `drive_sync_request`) into `sync/serve.rs` (~560 lines). Cross-module uses: `use crate::host_reads::read_upgrade_version_fields;` (called inside `write_boundary_checkpoint`, old 1020), `use crate::util::{diag_log, hex};`.
- [ ] **Step 2: Rewire.** `mod sync;` + `use sync::serve::{assert_floor_binds_view, drive_sync_request, recovery_frame_to_sync, reopen_preflight_synced_host, reopen_recovery, replica_backfill, replica_orchestrator_at, replica_verifier, to_node_disposition, verify_manifest_floor, write_boundary_checkpoint, ServedSeal, SyncBoundary, SyncStateRequest};`
- [ ] **Step 3: Gate + Commit** — `refactor(node): move statesync serve/checkpoint machinery into sync::serve`

### Task A5: `sync/catchup.rs` — post-reboot catch-up

**Files:**
- Create: `bin/node/src/sync/catchup.rs`
- Modify: `bin/node/src/sync/mod.rs`, `bin/node/src/main.rs`

**Interfaces:**
- Produces (all `pub(crate)`): `async fn apply_verified_suffix_frame(…)`, `async fn apply_and_journal_verified_frame<E>(…)`, `struct PostRebootCatchupApply` (**fields `applied`, `blocks` → `pub(crate)`**, `main_tests` reads them), `async fn apply_post_reboot_catchup_frames<E>(…)`, `fn catchup_pending_cutover_view(…)`, `async fn write_post_reboot_catchup_checkpoint<E>(…)`, `struct PostRebootCatchup`, `enum PostRebootCatchupError`, `async fn catch_up_post_reboot_frames<C, E>(…)`, `struct BootP2pSyncClient<S, R>` (+impl), `fn advance_next_seq_from_frames(next_seq: &mut u64, frames: &[Vec<u8>], me: &[u8])`, `fn derive_pending_boot(manifest: &Manifest, rec: &recovery::Recovered) -> Option<u64>`

- [ ] **Step 1: Move.** Cut lines 1567–2043 (`apply_verified_suffix_frame` through `BootP2pSyncClient`'s impl) plus 2081–2118 (`advance_next_seq_from_frames`, `derive_pending_boot`) into `sync/catchup.rs`. Cross-module uses: `use crate::explorer::IndexFold;` — **wait, explorer moves in A6; order this task AFTER A6** — see note below. Alternatively `use crate::IndexFold;` still resolves while `IndexFold` sits at the crate root; the recipe of using crate-root paths during the transition is fine, but cleaner: **do A6 before A5** (the executor should follow the order A1, A2, A3, A4, A6, A5, A7, A8, A9 — dependency edges: catchup→explorer(IndexFold), catchup→serve(to_node_disposition)). Other uses: `use crate::sync::serve::to_node_disposition;`, `use crate::util::{diag_log, hex};`, `use crate::constants::POST_REBOOT_CATCHUP_MAX_ITERS;`.
- [ ] **Step 2: Rewire.** Add `pub(crate) mod catchup;` to `sync/mod.rs`; in `main.rs`: `use sync::catchup::{advance_next_seq_from_frames, apply_post_reboot_catchup_frames, apply_verified_suffix_frame, catch_up_post_reboot_frames, derive_pending_boot, write_post_reboot_catchup_checkpoint, BootP2pSyncClient, PostRebootCatchupApply, PostRebootCatchupError};`
- [ ] **Step 3: Gate + Commit** — `refactor(node): move post-reboot catch-up into sync::catchup`

### Task A6: `explorer.rs` — index/explorer fold (execute BEFORE A5)

**Files:**
- Create: `bin/node/src/explorer.rs`
- Modify: `bin/node/src/main.rs`

**Interfaces:**
- Produces: `pub(crate) fn explorer_root_op(…)`, `pub(crate) fn sealed_frame_block_row(…)`, `pub(crate) fn boundary_block_row(height: u64, app_hash: &StateRoot) -> Vec<u8>`, `pub(crate) struct IndexFold<'a>` (+ its `recovery::ReplaySink` impl; fields `pub(crate)` — constructed from `run_node`), `pub(crate) async fn heal_index(index: &indexer::IndexStore, host: &Host, boundary: u64, label: &str)`, `pub(crate) fn ship_index_blobs(…)`, `pub(crate) async fn stage_shipped_index<C: statesync::SyncClient>(…)`

- [ ] **Step 1: Move.** Cut lines 1292–1566 (landmark: `fn explorer_root_op` through end of `stage_shipped_index`) into `bin/node/src/explorer.rs`. `use crate::util::hex;`.
- [ ] **Step 2: Rewire.** `mod explorer;` + `use explorer::{boundary_block_row, explorer_root_op, heal_index, sealed_frame_block_row, ship_index_blobs, stage_shipped_index, IndexFold};`
- [ ] **Step 3: Gate + Commit** — `refactor(node): move explorer/index fold into explorer module`

### Task A7: `rpc.rs` — operator RPC surface

**Files:**
- Create: `bin/node/src/rpc.rs`
- Modify: `bin/node/src/main.rs`

**Interfaces:**
- Produces (all `pub(crate)`; fields constructed from `run_node` → fields `pub(crate)`): `enum RpcRequest`, `struct JoinRequestRecord`, `struct JoinRequestView`, `struct RpcReply` (+impl), `struct RpcStatus`, `type RpcJob`, `fn spawn_rpc_listener(…)`

- [ ] **Step 1: Move.** Cut lines 2144–2176 and 2185–2274 (skipping `unix_ms`, already in A1). `use crate::util::hex;` (JoinRequestView).
- [ ] **Step 2: Rewire.** `mod rpc;` + `use rpc::{spawn_rpc_listener, JoinRequestRecord, JoinRequestView, RpcJob, RpcReply, RpcRequest, RpcStatus};`
- [ ] **Step 3: Gate + Commit** — `refactor(node): move operator RPC surface out of main.rs`

### Task A8: `reachability_plane.rs`

**Files:**
- Create: `bin/node/src/reachability_plane.rs` (~650 lines — one cohesive plane, same shape as `voice_plane.rs`)
- Modify: `bin/node/src/main.rs`

**Interfaces:**
- Produces: `pub(crate) fn wire_reachability_plane<S, R>(…)` (keep the exact `<S: P2pSender, R: P2pReceiver>` generics); `async fn reachability_plane(…)` stays private to the module (only `wire_…` calls it).

- [ ] **Step 1: Move.** Cut lines 2379–3026 (landmark: `fn wire_reachability_plane` through the end of `reachability_plane`). Bring `use crate::constants::{CHANNEL_REACHABILITY, NUDGE_INTERVAL};` — grep the body for any other constants it names.
- [ ] **Step 2: Rewire.** `mod reachability_plane;` + `use reachability_plane::wire_reachability_plane;`
- [ ] **Step 3: Gate + Commit** — `refactor(node): move reachability plane out of main.rs`

### Task A9: role stubs — `replica/promotion.rs` + `validator/announce.rs` + PR close-out

**Files:**
- Create: `bin/node/src/replica/mod.rs`, `bin/node/src/replica/promotion.rs`, `bin/node/src/validator/mod.rs`, `bin/node/src/validator/announce.rs`
- Modify: `bin/node/src/main.rs`

**Interfaces:**
- `replica/mod.rs`: `pub(crate) mod promotion;` + `pub(crate) fn reboot_self() -> !` (used only by the replica role today; PR 2's entry fn joins it here).
- `replica/promotion.rs` (all `pub(crate)`): `enum PromotionBoundarySource` (+impl), `enum PromotionBoundary<'a>`, `struct ManifestFetchRetry` (**fields `log_line`, `announce` → `pub(crate)`**), `fn joiner_manifest_fetch_retry(…) -> ManifestFetchRetry`, `fn latest_boundary_has_floor(latest: &statesync::Manifest) -> bool`, `fn choose_promotion_boundary<'a>(…) -> PromotionBoundary<'a>`
- `validator/mod.rs`: `pub(crate) mod announce;` (PR 3 adds the rest).
- `validator/announce.rs`: `pub(crate) struct ReadinessSignaller` (**field `signaled` → `pub(crate)`**; `main_tests` assigns it) with `pub(crate) fn new`/`decide`; `pub(crate) struct CapabilityAnnouncer` (+impl); `pub(crate) async fn dispatch_pending_deliveries(host: &Host) -> u64`; `pub(crate) async fn saga_next_expiry(host: &Host) -> Option<u64>` — this cluster is validator-loop-only (call sites 7046/7926/7964), hence the role home; `use crate::constants::MAX_PROTOCOL_VERSION;`.

- [ ] **Step 1: Move promotion.** Cut lines 703–775 (`enum PromotionBoundarySource` through `choose_promotion_boundary` — fully self-contained) into `replica/promotion.rs`; create `replica/mod.rs` with the mod decl and move `reboot_self` (2119–2143) into it as `pub(crate)`.
- [ ] **Step 2: Move announce.** Cut lines 500–681 (`struct ReadinessSignaller` through `saga_next_expiry`) into `validator/announce.rs`; create `validator/mod.rs`.
- [ ] **Step 3: Rewire.** In `main.rs`: `mod replica; mod validator;` + `use replica::{reboot_self, promotion::{choose_promotion_boundary, joiner_manifest_fetch_retry, ManifestFetchRetry, PromotionBoundary, PromotionBoundarySource}};` + `use validator::announce::{dispatch_pending_deliveries, saga_next_expiry, CapabilityAnnouncer, ReadinessSignaller};`
- [ ] **Step 4: Gate + Commit** — `refactor(node): seed replica/ and validator/ role modules`
- [ ] **Step 5: PR-1 wide gate.** `cargo test -p node-bin` (full e2e, slow). Expected: all green. Verify shrink: `wc -l bin/node/src/main.rs` ≈ 5,900.
- [ ] **Step 6: Open PR** against `dev`: title `refactor(node): carve main.rs helpers into domain modules (#213, 1/3)`, body lists the new layout, states move-only, links issue #213 and precedent `3997c92f`. Clean-context review before merge.

---

# PR 2 — boot prelude + replica role (`refactor/node-main-replica`)

Fork from `origin/dev` **after PR 1 merges**. Line anchors shift after PR 1 (main.rs ≈ 5,900); re-anchor by grep landmark only. Phases refer to the phase map in the appendix.

### Task B1: `boot/env.rs` — P0 config destructure

**Files:**
- Create: `bin/node/src/boot/mod.rs` (`pub(crate) mod env;`), `bin/node/src/boot/env.rs`
- Modify: `bin/node/src/main.rs`

**Interfaces:**
- Produces: `pub(crate) struct BootEnv { … }` and `pub(crate) fn derive(resolved: config::Resolved, sync_only: bool) -> BootEnv` (match the exact `Resolved` type `run_node` already receives).
- `BootEnv` fields (names must match the existing local bindings verbatim; copy each type from its `let` in P0): `signer: ed25519::PrivateKey`, `label`, `namespace: Vec<u8>`, `identity_chain_id`, `duckdns_chain_id`, `peers: Vec<ed25519::PublicKey>`, `validators`, `bootstrappers`, `coordinated`, `listen`, `advertised`, `storage: PathBuf`, `wireguard_listen`, `wireguard_effect`, `wireguard_key_file`, `invite_listen`, `invite_token`, `invite_wireguard`, `invite_fronts`, `coordination`, `coord_cap`, `workspace`, `sync_candidates`, `chain_id: String`, `mesh_state_file: PathBuf`, `duckdns_publications`, `duckdns_announcements`, `checkpoint_blocks`, `dev_demo`, `sync_index`, `announce_capabilities`, `promoted: bool`, `joiner: bool`.

- [ ] **Step 1: Move.** Cut P0 (`run_node`'s opening through the last derivation before the first `TcpListener::bind`; original 3032–3261) into `boot::env::derive`, ending with `BootEnv { signer, label, namespace, … }` (field-init shorthand — the moved code already produces bindings with these exact names).
- [ ] **Step 2: Rewire.** In `run_node`, replace the cut region with:

```rust
let boot::env::BootEnv {
    signer, label, namespace, identity_chain_id, duckdns_chain_id, peers,
    validators, bootstrappers, coordinated, listen, advertised, storage,
    wireguard_listen, wireguard_effect, wireguard_key_file,
    invite_listen, invite_token, invite_wireguard, invite_fronts,
    coordination, coord_cap, workspace, sync_candidates, chain_id,
    mesh_state_file, duckdns_publications, duckdns_announcements,
    checkpoint_blocks, dev_demo, sync_index, announce_capabilities,
    promoted, joiner,
} = boot::env::derive(resolved, sync_only);
```

The full destructure means the remaining ~5,400 lines compile unchanged. (If P0 consumes another of `run`'s params, thread it as an argument — compile errors will name it.)
- [ ] **Step 3: Gate + Commit** — `refactor(node): extract run_node boot-env derivation` (same Gate recipe as PR 1).

### Task B2: `boot/surfaces.rs` — P1 listener binds + service threads

**Files:**
- Create: `bin/node/src/boot/surfaces.rs`
- Modify: `bin/node/src/boot/mod.rs`, `bin/node/src/main.rs`

**Interfaces:**
- Produces: `pub(crate) struct Surfaces { rpc_listener: std::net::TcpListener, http_cmds: /* NodeCommand receiver — copy exact type */, stream_hub, index: indexer::IndexStore, voice_requests: /* mpsc::Receiver<CallSessionRequest> */, blobs, agent_provisioner, duckdns_plane_slot, duckdns_files }` and `pub(crate) fn bind(env: &BootEnv, log_ring: noded::LogRing) -> Result<Surfaces, Box<dyn std::error::Error>>`.
- The two OS-thread spawns (duckdns-ingress, app-surface HTTP server) move inside `bind` verbatim.

- [ ] **Step 1: Move.** Cut P1 (original 3263–3357; landmark: the `rpc_listener` bind through the app-surface thread spawn) into `boot::surfaces::bind`, returning the struct. Pass `&BootEnv` plus owned extras as compile errors dictate — do not restructure the moved bodies.
- [ ] **Step 2: Rewire.** In `run_node`: `let surfaces = boot::surfaces::bind(&env_bits…, log_ring)?;` followed by a full destructure (same pattern as B1 Step 2).
- [ ] **Step 3: Gate + Commit** — `refactor(node): extract listener binds and service threads`

### Task B3: `boot/mesh.rs` — P3 shared runtime head

**Files:**
- Create: `bin/node/src/boot/mesh.rs`
- Modify: `bin/node/src/boot/mod.rs`, `bin/node/src/main.rs`

**Interfaces:**
- Produces: `pub(crate) struct MeshHead { metrics: noded::NodeMetrics, mesh_participants: /* Set<ed25519::PublicKey> */, status_public_key: String, sync_sources, sync_source, advertised_reach, network, oracle, quota }` and `pub(crate) async fn build(context: &…, /* the BootEnv fields P3 reads */, overlay_slot: …) -> MeshHead`. Copy each field's exact type from the existing `let` bindings; `network`/`oracle` remain the concrete `discovery`/`overlay_net` types.

- [ ] **Step 1: Move.** Cut P3 (original 3380–3481, the closure head from `NodeMetrics` registration through `quota`) into `boot::mesh::build`, verbatim, returning `MeshHead { … }` via field-init shorthand.
- [ ] **Step 2: Rewire.** In the closure: full destructure into the same local names (`let boot::mesh::MeshHead { metrics, mesh_participants, status_public_key, sync_sources, sync_source, advertised_reach, mut network, mut oracle, quota } = boot::mesh::build(…).await;`).
- [ ] **Step 3: Gate + Commit** — `refactor(node): extract shared mesh/network head`

### Task B4: `boot/sync_only.rs` — P4 terminal branch

**Files:**
- Create: `bin/node/src/boot/sync_only.rs`
- Modify: `bin/node/src/boot/mod.rs`, `bin/node/src/main.rs`

**Interfaces:**
- Produces: `pub(crate) async fn run(…)` — parameters are exactly what the branch consumes (seam analysis): `context`, `network`, `oracle`, `quota`, `mesh_participants`, `sync_sources`, `storage_for_sync`, `namespace`, `identity_chain_id`, `duckdns_chain_id`, `blobs`, `voice_requests` (dropped inside). Returns `()`.

- [ ] **Step 1: Move.** Cut the whole `if sync_only { … }` body (original 3483–3635; landmark: the sync-only channel black-holing through `sync_all_modules` and the branch's `return`) into `boot::sync_only::run`. The branch's `return` becomes the fn's natural end; `run_node` keeps `if sync_only { boot::sync_only::run(…).await; return; }`.
- [ ] **Step 2: Gate + Commit** — `refactor(node): extract sync-only branch`

### Task B5: `replica/` — P6 joiner/replica role + PR close-out

**Files:**
- Create: `bin/node/src/replica/wiring.rs`, `bin/node/src/replica/park.rs`
- Modify: `bin/node/src/replica/mod.rs`, `bin/node/src/main.rs`

**Interfaces:**
- NOTE (from PR 1): `replica/mod.rs` already hosts the pre-existing fold helpers (`CertAnchor`/`FoldStep`/`plan_fold`, ~220 lines — the old `replica.rs`, git-mv'd there in Task A9) plus `reboot_self` and `mod promotion;`. If adding the role entry crowds it, split the fold helpers to `replica/fold.rs` first (pure move, own commit).
- `replica/mod.rs` adds: `pub(crate) async fn run(…) -> !` — the whole `if !checkpoint_seats_me && !validators.contains(…) { … }` block (original 3680–5749). Every exit is `reboot_self()` or `std::process::exit`, so the return type is `!`; `run_node` calls it and the validator path below stays untouched.
- `wiring.rs`: `pub(super) struct ReplicaChannels { replica_store, head_wake, cert_bridge, sync_tx, sync_rx, reach_cmd, relay_tx, relay_rx, lobby_tx, lobby_rx }` + `pub(super) async fn wire(…) -> ReplicaChannels` (P6a: epoch bank registration, first-contact spawn, lobby, `network.start()` — the start call stays INSIDE wiring; all registration for this role completes before it, hazard 2).
- `park.rs`: `pub(super) async fn park(channels: ReplicaChannels, …) -> !` — P6b (serve-state build) + P6c (the park loop) + P6d (promotion checkpoint + `reboot_self()`), verbatim as one unit (~1.7k lines — the natural boundary is the loop; accepted, decision 2). The `send_announce`/`not_serving` closures stay closures — they never leave this fn.

- [ ] **Step 1: Create the entry.** `replica::run` body: destructure args → `let channels = wiring::wire(…).await;` → `park::park(channels, …).await`.
- [ ] **Step 2: Move P6a** (3680–4013) into `wiring::wire` verbatim, ending with `ReplicaChannels { … }`.
- [ ] **Step 3: Move P6b–P6d** (4015–5749) into `park::park` verbatim.
- [ ] **Step 4: Rewire `run_node`:** keep the role condition byte-for-byte (`if !checkpoint_seats_me && !validators.contains(…)`) and make its body `replica::run(…).await;` — pass the P3/P5 seam values (`network`, `oracle`, `quota`, `mesh_participants`, `sync_sources`, `sync_source`, `recovery`, `manifest`, `forge_repo`, `duckfs_dir`, the surfaces fields the branch uses — `rpc_listener`, `http_cmds`, `stream_hub`, `index`, `blobs` — plus the `BootEnv` fields from the seam list). Let compile errors finalize the parameter list; bundle nothing new beyond `ReplicaChannels`.
- [ ] **Step 5: Gate + Commit** — `refactor(node): extract joiner/replica role into replica/`
- [ ] **Step 6: PR-2 wide gate + PR.** `cargo test -p node-bin` full run — the joiner/replica e2e files (`live_admission_e2e.rs`, `resident_*_e2e.rs`, `restart_e2e.rs`) are the real gate. Expected: all green. `wc -l bin/node/src/main.rs` ≈ 3,600. Open PR: `refactor(node): carve boot prelude + replica role out of main.rs (#213, 2/3)`.

---

# PR 3 — validator role (`refactor/node-main-validator`)

Fork from `origin/dev` after PR 2 merges. This PR touches the consensus-driving loop — run the full e2e suite after C4 as well as at PR end (`cluster_e2e.rs`, `upgrade_e2e.rs`, `dispatch_e2e.rs` are the gate).

### Task C1: `validator/boot.rs` — P7 BootState + P11 catch-up

**Files:**
- Create: `bin/node/src/validator/boot.rs`
- Modify: `bin/node/src/validator/mod.rs`, `bin/node/src/main.rs`

**Interfaces:**
- Produces: `pub(crate) async fn boot(…) -> BootState` (P7: genesis-vs-restore; the `BootState` type alias already exists near old 5759 — move it here as `pub(crate)`), and `pub(crate) async fn post_reboot_catchup(…)` (P11: the whole `if promoted_validator_boot { … }` body, old 6137–6474) which takes and returns the rebound seam values (`host`, `resumed`, `next_seq`, `prev_ckpt`, `recovery_manifest_for_resume`, `sync_tx`, `sync_rx`) as a tuple, rebound at the call site.

- [ ] **Step 1: Move P7** (old 5750–5906) into `boot`, verbatim. `boot_fold` is dropped in P12 — grep `boot_fold`; if it crosses the fn boundary, thread `&mut boot_fold` as a param rather than moving its construction.
- [ ] **Step 2: Move P11** into `post_reboot_catchup`; call-site rebinding: `let (host, resumed, next_seq, prev_ckpt, recovery_manifest_for_resume, sync_tx, sync_rx) = validator::boot::post_reboot_catchup(…).await;`
- [ ] **Step 3: Gate + Commit** — `refactor(node): extract validator boot and post-reboot catch-up`

### Task C2: `validator/wiring.rs` — P8+P9+P10 + P12 ingress bridges

**Files:**
- Create: `bin/node/src/validator/wiring.rs`
- Modify: `bin/node/src/validator/mod.rs`, `bin/node/src/main.rs`

**Interfaces:**
- Produces: `pub(crate) struct ValidatorMesh { initial_member_keys, initial_resident_keys, initial_resume_epoch, mesh_oracle, bank_base, channel_bank, sync_tx, sync_rx, lobby_tx, lobby_rx, relay_tx, relay_rx, media_peers, reach_cmd, sync_ingress-related handles (sync_state_tx, sync_state_rx, sync_plane_book), lobby_ingress, relay_ingress }` + `pub(crate) async fn wire(…) -> ValidatorMesh`. `network.start()` (old 6135) stays INSIDE `wire` — all validator channel registration completes before it (hazard 2). The P12 ingress bridges (statesync ingress + serve task, lobby/relay ingress; old ~6560–6700) move here too — they are lane wiring, not engine.
- **Closure conversion (explicit):** `mesh_at` (old 5950, captures `peers`) becomes `pub(crate) fn mesh_at(peers: &[…], /* the closure's params */) -> …` — copy the closure body, take the captures as params. Update the P8 call sites and the P14 cutover call site.

- [ ] **Step 1: Move P8–P10** (old 5908–6135) into `wire`, verbatim.
- [ ] **Step 2: Move the P12 ingress bridges** (statesync ingress bridge + serve task + lobby/relay ingress; grep landmarks `sync_ingress`, `lobby_ingress`, `relay_ingress` between old 6560–6715) into `wire`, extending `ValidatorMesh`.
- [ ] **Step 3: Convert `mesh_at`** to the free fn; update its call sites (grep `mesh_at`).
- [ ] **Step 4: Gate + Commit** — `refactor(node): extract validator mesh wiring`

### Task C3: `validator/engine.rs` — EpochSpawner + engine resume

**Files:**
- Create: `bin/node/src/validator/engine.rs`
- Modify: `bin/node/src/validator/mod.rs`, `bin/node/src/main.rs`

**Interfaces:**
- Produces: `pub(crate) struct EpochSpawner { channel_bank: Vec<Option<(…)>>, namespace: Vec<u8>, signer: ed25519::PrivateKey, oracle: …, bank_base: u64, label: String, context: … }` with `pub(crate) fn spawn(&mut self, epoch: u64, /* copy the closure's exact params */) -> …` — the body is the `spawn_epoch` closure body (old 6716) verbatim with captures replaced by `self.` fields. Plus `pub(crate) async fn resume(…)` — the engine-resume tail of P12 (boot_store seed, boot_floor read, first `spawn`, `OrderedNode` resume/with_sink, `watch_module("valset")`, `ValsetOrchestrator` resume, ceiling re-arm, dev-demo submit; old ~6720–6889) returning `(node, orchestrator, last_cert_height, latest_floor, participants, resume_epoch, member_keys)`.
- Consumed at: boot (here) and the P14 cutover (`spawn_epoch(…)` → `spawner.spawn(…)`).

- [ ] **Step 1: Create `EpochSpawner`**, move the closure body into `spawn`, replace both call sites.
- [ ] **Step 2: Move the engine-resume tail** into `resume`.
- [ ] **Step 3: Gate + Commit** — `refactor(node): extract epoch spawner and engine resume`

### Task C4: `validator/run.rs` — P13 state + P14 loop

**Files:**
- Create: `bin/node/src/validator/run.rs`
- Modify: `bin/node/src/validator/mod.rs`, `bin/node/src/main.rs`

**Interfaces:**
- Produces: `pub(super) struct ValidatorLoopState { … }` — every P13→P14 seam item (fields `pub(super)`): `node`, `orchestrator`, `mesh_oracle`, `epoch_spawner`, `sync_plane_book`, `media_peers`, `reach_cmd`, `rpc_ingress`, `http_ingress`, `lobby_ingress`, `relay_ingress`, `sync_state_rx`, `oracle_results`, `pending_submits`, `pending_relays`, `validator_relay`, `join_requests`, `next_seq`, `blocks_since_checkpoint`, `prev_ckpt`, `latest_floor`, `last_cert_height`, `last_reach_view`, `last_flush`, `last_crank`, `last_nudge`, `pending_retarget`, `heartbeat_disabled`, `workers`, `signaller`, `announcer`, `duckdns_announcer`, `upgrade_armed_latch`, `upgrade_pending_seen`, `sigterm`, `sigint`, `next_drain`, `applied`, `converged`, `last_published`, `expected`, `metrics`, `index`, `blobs`, `signer`, `validators`, `namespace`, `member_keys`, `participants`, `resume_epoch` — copy exact types from the existing bindings. Plus `pub(crate) async fn run(state: ValidatorLoopState) -> Result<(), …>` — the `loop { select_biased! { … } }` (old 7130–8637) whole (~1.8k lines with state init; the natural boundary is the loop; accepted, decision 2).

- [ ] **Step 1: Probe the node type.** Try `type ValidatorNode = node::OrderedNode<SimplexOrderer, recovery::Recovery<commonware_runtime::tokio::Context>>;` (fill generics from the actual `let node = …` binding; `cargo check -p node-bin` decides). **If it names cleanly**, use it for the `node` field. **If it does not** (unnameable generics), make `ValidatorLoopState` generic over the orderer/store params with the bounds the compiler demands — mechanical, no logic change. `graceful_checkpoint!` (old 7103) moves as the `macro_rules!` it is, into `run.rs` — do not convert it in a move-only PR.
- [ ] **Step 2: Move P13** (old 6891–7128, loop-state init incl. RPC bridge + signal streams) into `validator::run` construction code (in `validator/mod.rs`'s role entry or a `ValidatorLoopState::init(…)` — whichever reads naturally), ending with the struct via field-init shorthand.
- [ ] **Step 3: Move the loop** into `run`, mechanically prefixing state locals with `state.`. `select_biased!` polls multiple `state.` receivers — if the borrow checker rejects cross-arm disjoint borrows, destructure the receivers out of `state` into locals before the loop (note which ones in the commit message); do not restructure arm bodies.
- [ ] **Step 4: Gate + Commit** — `refactor(node): extract validator run loop` — then run the full e2e suite NOW (not just at PR end): `cargo test -p node-bin`, all green, paying special attention to `cluster_e2e`, `upgrade_e2e` (epoch cutover exercises `EpochSpawner` + the moved loop).

### Task C5: run_node conductor + PR 3 close-out

**Files:**
- Modify: `bin/node/src/main.rs`, `bin/node/src/validator/mod.rs`

- [ ] **Step 1: Tidy the role entry.** `validator/mod.rs` gets `pub(crate) async fn run_validator(…)`: `boot::boot` → (`post_reboot_catchup` if promoted) → `wiring::wire` → `engine::resume` → `run::run`. `run_node` becomes the conductor (~150 lines): `boot::env::derive` → `boot::surfaces::bind` → runtime build (stays inline, ~20 lines) → closure: `boot::mesh::build` → `if sync_only { boot::sync_only::run(…).await; return; }` → P5 recovery preamble (stays inline, ~40 lines — it computes `checkpoint_seats_me`, the role fork, which reads clearest at the fork itself) → `if <not seated> { replica::run(…).await; }` → `validator::run_validator(…).await`. Remove now-unused `use` lines flagged by clippy.
- [ ] **Step 2: Verify shrink.** `wc -l bin/node/src/main.rs` — expected ≈ 350–500 (mod decls, uses, `main`, `run`, `init_tracing`, conductor `run_node`).
- [ ] **Step 3: Full gates.** `touch` + `cargo clippy -p node-bin --tests --no-deps` (0 warnings) + `cargo test -p node-bin` (all green, full e2e).
- [ ] **Step 4: Commit** — `refactor(node): run_node becomes a boot conductor` — and open PR: `refactor(node): carve validator role out of main.rs (#213, 3/3)`. PR body: move-only statement, the two accepted-large loop files, the two closure conversions, e2e evidence.
- [ ] **Step 5: File the follow-up issue** for hazard 6 (replica/validator drain-mirror unification across the orderer generic) referencing #213, and tick the `bin/node/src/main.rs` checkbox on #213.

---

# Analysis Appendix (source maps the tasks reference)

## run_node phase map (line anchors at dev `1d2111ca`)

| Phase | Lines | Content | Terminal? |
|---|---|---|---|
| P0 | 3032–3261 | config destructure, chain ids, mesh dial seeds, fail-closed warnings | no |
| P1 | 3263–3357 | std TCP binds (rpc/duckdns/http), NodeHandle, index store, voice lane, agent provisioner, 2 OS service threads | no |
| P2 | 3358–3379 | runtime build, `executor.start(...)` closure opens | no |
| P3 | 3380–3481 | metrics, mesh participants, discovery config, overlay, `Network::new` → `network`+`oracle` | no |
| P4 | 3483–3635 | sync-only: black-hole channels, statesync once, `return` | **yes** |
| P5 | 3637–3679 | recovery open, manifest read, `checkpoint_seats_me` role decision | no |
| P6 | 3680–5749 | joiner/replica: 6a wiring (→4013, `network.start()`), 6b serve-state (→4364), 6c park loop (→5700), 6d promotion+`reboot_self()` | **yes** |
| P7 | 5750–5906 | BootState: genesis vs restore, journal replay | no |
| P8 | 5908–5970 | membership derivation, `mesh_at`, mesh oracle prime | no |
| P9 | 5972–6008 | epoch channel bank + sync/lobby/relay registration | no |
| P10 | 6009–6135 | voice/video lanes, reachability plane, `network.start()` | no |
| P11 | 6137–6474 | promoted-validator post-reboot catch-up (conditional) | no |
| P12 | 6476–6889 | final heal, ingress bridges, `spawn_epoch`, `OrderedNode`/orchestrator resume | no |
| P13 | 6891–7128 | loop-state init, RPC bridge, signal streams, `graceful_checkpoint!` | no |
| P14 | 7130–8637 | validator `select_biased!` loop (8 arms; drain arm 7174–8075) | **yes** |

## Hazards the tasks encode

1. **Orderer generics:** replica uses `OrderedNode<FollowerOrderer, _>`, validator `OrderedNode<SimplexOrderer, _>` — never unify their helpers (decision 4); alias each concretely in its own module.
2. **`network.register` only before `network.start()`**, and there are three `start()` sites (P4/P6a/P10) — each stays inside the module that owns its role's registration.
3. **Closures capturing state:** only `spawn_epoch` (C3) and `mesh_at` (C2) are converted — they cross module seams. `send_announce`/`not_serving` (inside `replica/park.rs`) and `graceful_checkpoint!` (inside `validator/run.rs`) stay as-is.
4. **Role fork is two sequential `if`s + fall-through**, not a match — the conductor preserves that exact order (`sync_only` → `checkpoint_seats_me` → validator).
5. **`std::process::exit(1)` fatal sites** stay verbatim where they are — converting them to `Result` is behavior-adjacent and out of scope.
6. **Replica/validator drain mirror** (~320 lines duplicated) — deliberately NOT unified; follow-up issue in C5.
7. **`next_seq` threading:** any moved submit-issuing code takes `&mut next_seq` (or lives on `ValidatorLoopState`).

## Test-reachability contract (from main_tests / joiner_mesh_tests)

- `main_tests.rs` uses `use super::*;` — keeping plain `use module::{…};` lines in `main.rs` preserves every name it needs. Do not remove the existing external re-imports (`directory::{encode_msg, …}`, `recovery::{Manifest, Recovery}`) from `main.rs`'s use block.
- `joiner_mesh_tests.rs` → one-line fix in Task A3.
- Field-level `pub(crate)`: `ReadinessSignaller::signaled`, `ManifestFetchRetry::{log_line, announce}`, `PostRebootCatchupApply::{applied, blocks}` (+ `IndexFold`'s fields, constructed from `run_node`).
