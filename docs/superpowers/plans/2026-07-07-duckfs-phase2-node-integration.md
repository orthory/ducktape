# duckfs Phase 2 — Node Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.
>
> **STATUS: EXECUTING** stacked on `feat/duckfs` (Phase 1 PR #185 pending review; controller directive 2026-07-07: push through all phases without stopping). Rebases onto dev if #185 review changes the files wire.

**Goal:** Wire the duckfs `files` module into the live node: delete the superseded `memory` module and its consumers, flip duckfs state sync from the `SnapshotBytes` phase-1 bridge to a real `ResolverBacked` object-fetch resolver, register duckfs across every binary's genesis/joiner path, expose duckfs over noded's HTTP surface, and prove it under a real multi-node cluster e2e — a fresh-genesis flag-day cutover.

**Architecture:** duckfs joins the disk-cohort recovery discipline (like `kv`/`forge`); the kernel statesync resolver drives `Files::{missing_objects, serve_sync/GetObjects, ingest_objects}` to full possession; `memory` is deleted wholesale (fresh genesis, no migration, per the no-backwards-compat house rule). The `automations` memory-watch trigger is **removed entirely** (controller decision 2026-07-07: chat-hook triggers stay; filesystem-change automations are out of scope for this wave).

**Tech Stack:** Rust (workspace), the Phase-1 `files` crate, `crates/kernel/statesync`, `crates/kernel/recovery`, `bin/node`, `bin/noded`, `bin/demo`, `bin/simnode`.

**Spec:** `docs/superpowers/specs/2026-07-06-duckfs-real-filesystem-design.md` — §"Deletions and integration changes" + §"State sync and self-healing" are binding for this phase.

## Global Constraints

- Work in a worktree stacked on the Phase-1 branch (or `dev` after #185 merges). All commands from the worktree root.
- Every commit: `git -c commit.gpgsign=false commit ...`, trailer `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.
- No backwards compatibility: `memory` is deleted, not deprecated; fresh genesis. No migration shims.
- Fresh-genesis discipline: any node started against an old genesis is expected to fork — this is a flag-day cutover, coordinated by the operator (documented, not code).
- Determinism unchanged: the duckfs core stays pure; only the native glue + node wiring change. Purity gate `cargo check -p files --no-default-features` stays green.
- Registration-site invariant: `MODULE_IDS`, the genesis module vec, the joiner restore ladder, and every binary's module set must agree on the SAME module list. `memory` leaves all of them atomically within one task.
- Gates per task: `cargo test -p <touched crates>` green; `cargo check --workspace` green; `cargo fmt -- --check` clean on touched crates; the node/cluster e2e suites named per task green.
- Comment style: lowercase, explain constraints not mechanics.

## Task list

### Task 1: Sever the automations→memory coupling (drop the memory-watch trigger)

`automations` decodes `memory::MemoryEvent`, uses `memory::META_KIND`, takes a `memory` module-id constructor arg, and has a memory-watch trigger arm (`crates/apps/automations/src/lib.rs:97,131-132,143-148,1322`). Per the controller decision the trigger is **removed** (not migrated). This must land BEFORE the memory crate is deleted (Task 2) or the workspace won't compile.

**Files:**
- Modify: `crates/apps/automations/src/lib.rs` (drop the `memory` dep import, the `MemoryEvent`/`META_KIND` uses, the `memory: ModuleId` field, the constructor param, the `Origin::Module(memory)` trigger arm, and the docblock lines describing memory triggers)
- Modify: `crates/apps/automations/Cargo.toml` (drop the `memory` dependency)
- Modify: `crates/apps/automations/tests/host_integration.rs` (delete the memory-trigger tests; keep chat-hook tests)
- Modify: every `Automations::new(...)` call site (bin/node:854, bin/demo, bin/simnode, bin/noded — drop the trailing `"memory"` arg)

**Interfaces:**
- Produces: `Automations::new(id, chat, tasks, inbox)` — the `memory` param removed; `Trigger` enum loses its memory-watch variant.
- Consumes: nothing new.

- [ ] **Step 1:** grep every `Automations::new` call site and the `Trigger` variants; enumerate what references memory. RED: after dropping the memory arm, the memory-trigger tests fail to compile — delete them, keep chat-hook coverage.
- [ ] **Step 2:** Remove the memory field/param/arm + docblock lines; update call sites.
- [ ] **Step 3:** `cargo test -p automations` green (chat-hook triggers intact); `cargo check --workspace` still green (memory crate still present, just unused by automations).
- [ ] **Step 4:** Commit `refactor(automations)!: drop the memory-watch trigger ahead of the memory module removal`.

### Task 2: Delete the memory module and every registration of it

With automations severed, memory has no consumers. Delete the crate and remove it from every binary's module list atomically.

**Files:**
- Delete: `crates/apps/memory/` (whole crate)
- Modify: workspace `Cargo.toml` (drop the `memory` member + workspace dep)
- Modify: `bin/node/src/main.rs` — `MODULE_IDS` (23→22, drop `"memory"` at :213), the genesis vec (drop `Box::new(Memory::new("memory"))` :671), the two joiner restore ladders (drop the `snapshot_of("memory")` blocks at :817 and :1075), the `use memory::Memory` (:99)
- Modify: `bin/demo/src/main.rs`, `bin/simnode/src/main.rs`, `bin/noded/src/main.rs` (drop `use memory::Memory` + the `Memory::new("memory")` registration + any module-count docblocks)
- Modify: any `MODULE_IDS`-derived counts/docblocks (bin/demo count string, etc.)

**Interfaces:**
- Produces: a 22-module genesis; `MODULE_IDS: [&str; 22]`.
- Consumes: Task 1's severed automations.

- [ ] **Step 1:** Delete the crate + workspace member. RED: `cargo check --workspace` fails on every `memory::` reference — that grep is the worklist.
- [ ] **Step 2:** Remove every registration site; fix the `[&str; 23]`→`[&str; 22]` and any hardcoded counts.
- [ ] **Step 3:** `cargo check --workspace` green; `cargo test -p node --test <genesis/registration suite>` green; grep `memory::` returns nothing outside git history.
- [ ] **Step 4:** Commit `feat(duckfs)!: delete the memory module — duckfs is the filesystem now`.

### Task 3: Flip duckfs state sync to ResolverBacked + wire the kernel resolver

Phase 1 left `state_sync_handle` returning `SnapshotBytes(self.snapshot())` (refs only) as a bridge. Flip it to `ResolverBacked{backend:"duckfs-odb"}` and implement the resolver loop in the kernel statesync layer using the Phase-1 `Files::{missing_objects, serve_sync, ingest_objects, possession_complete}` methods.

**Files:**
- Modify: `crates/apps/files/src/module.rs` (`state_sync_handle` → `ResolverBacked`; update the phase-1-bridge comment)
- Modify: `crates/kernel/statesync/src/lib.rs` (add a `duckfs-odb` resolver path: install refs snapshot → loop { missing_objects → GetObjects over the peer transport → ingest_objects } until possession_complete → report ready ONLY at full possession)
- Modify: `bin/node/src/main.rs` restore/join ladders (the `files` install now goes through the resolver, threading the sync-target height from Task-14's `install(bytes, root, height)`)
- Test: `crates/kernel/statesync/tests/duckfs_resolver.rs` (a two-node in-process sync: source with rich duckfs state, target reaches full possession, roots + byte-identical query replies)

**Interfaces:**
- Consumes: `Files::missing_objects/serve_sync/ingest_objects/possession_complete/durable_height` (Phase-1 Task 14).
- Produces: a working `ResolverBacked` duckfs sync that other modules' resolvers can pattern-match.

- [ ] Steps: flip the handle (RED: the SnapshotBytes-only bridge no longer transfers bytes → the resolver test fails until wired) → implement the resolver loop → thread sync-target height → full-possession gate → commit `feat(duckfs): resolverbacked object-fetch state sync to full possession`.

### Task 4: noded HTTP surface for duckfs

Replace noded's old CAS `/v1/files/blob` endpoints (already decoupled to `blobstore` in Phase-1 Task 1 for op receipts) with duckfs product endpoints that wrap the module's ops/queries.

**Files:**
- Modify: `bin/noded/src/lib.rs` (routes: `POST /v1/files/stage` (putblob frame), `POST /v1/files/commit`, `GET /v1/files/ls`, `GET /v1/files/read`, `GET /v1/files/stat`, `GET /v1/files/history` — each encodes the module wire and submits/queries through the existing actor seam)
- Modify: `bin/noded/tests/daemon_e2e.rs` / `router.rs` (duckfs endpoint round-trips against a real spawned daemon: stage chunks → commit a manifest → read it back)

**Interfaces:**
- Consumes: the node's submit/query actor seam + the duckfs wire.
- Produces: the HTTP surface the app TS client (Phase 5) will consume.

- [ ] Steps: add routes → e2e round-trip → commit `feat(duckfs): noded http surface (stage/commit/ls/read/stat/history)`.

### Task 5: Disk-cohort recovery wiring + restart/joiner cluster e2e

duckfs already implements the disk-cohort durability ordering internally (Phase-1 Task 6). This task confirms the kernel recovery layer treats it as a disk-cohort module (durable-height reporting, WAL replay above the boundary) and proves it under the real multi-process cluster harness.

**Files:**
- Modify: `crates/kernel/recovery/src/lib.rs` and/or `bin/node/src/main.rs` recovery registration (ensure `files` is in the disk cohort, reports `durable_height()`, and replay re-applies frames above it idempotently)
- Test: extend `bin/node/tests/restart_e2e.rs` (a duckfs commit survives a SIGKILL+restart — bytes readable via the duckfs read path after recovery, the thing the old CAS module failed) and the cluster/joiner e2e (a fresh joiner reaches full duckfs possession over the real network transport)

**Interfaces:**
- Consumes: Task 3's resolver, Phase-1's durability ordering.
- Produces: the production durability + join proof.

- [ ] Steps: wire the cohort registration → restart e2e (bytes survive reboot) → joiner e2e (full possession over the wire) → commit `test(duckfs): restart + joiner cluster e2e proves durable bytes and full-possession sync`.

### Task 6: Docs + hygiene fold-ins

**Files:**
- Modify: `docs/src/content/docs/en/human/modules/product-modules.mdx` (rewrite the Files section for the real FS; DELETE the Memory section)
- Modify: `docs/src/content/docs/.../roadmap/*` (mark duckfs Phase 1–2 shipped; memory removed)
- Fold in the Phase-1 whole-branch backlog: unify the `"files: "` error prefix (putblob's neighbors omit it); dedupe the `state.rs`/`objects.rs` cursor codecs into a shared pure `codec` submodule; the clippy `--no-deps` reconciliation note for host/dispatch/saga (either a workspace-hygiene sweep or documenting the `--no-deps` gate form).

- [ ] Steps: docs rewrite → hygiene commits (each its own commit) → `feat(duckfs): docs for the real filesystem + phase-1 hygiene fold-ins`.

## Phase-boundary note

At the end of Phase 2 the live node genesises duckfs as its filesystem, syncs it over a real resolver, survives restart, and serves it over HTTP — but nothing yet MOUNTS it. The checkout/commit engine (`duckfs-client` + `.duckfs/` index), the CLI, and FUSE are Phase 3–4; the app TS client + FilesView rebuild + memory-view deletion are Phase 5. Each is planned after its predecessor lands.

## Open decision carried into execution

The `automations` memory-watch trigger is **removed** (controller decision). If filesystem-change automations are wanted later, they return as a NEW duckfs-watch trigger built on `FsCap::decode_notify` (Phase-1 Task 16) — a fresh feature, not a migration. Noted so the capability isn't assumed lost by accident.
