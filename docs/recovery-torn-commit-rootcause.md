# Root cause — fresh workspace bricks on daemon restart after one tx

**Status:** diagnosed (read-only), high confidence. Surfaced by the UI-QA live pass; reproduced on a brand-new solo genesis workspace.

## Symptom / minimal repro
Fresh solo genesis node boots healthy, commit **one** tx (block height climbs), kill + restart the daemon → FATAL, locally unrecoverable:
- `recovery state torn: block <N>: touched modules are neither all-applied nor all-unapplied — wipe app state and re-sync (keep the consensus journal)` — where `<N>` is exactly the block the first tx committed.
- variant: `recovery verification failed: recomposed app_hash StateRoot(..) != sealed tip StateRoot(..) at height Some(<N>)`.
- both then: `app state cannot be locally recovered. wipe the app-state partitions and re-sync from a peer` — a **solo** node has no peer ⇒ bricked.

## Root cause: non-atomic per-block commit across two durability regimes
A finalized block is persisted across **two** durability classes with **different durability points and no barrier / WAL / 2PC** between them, so a crash (or unclean restart) between them tears the block.

- **Disk-backed modules** (qmdb: `kv`, `document`, `chat`; git: `forge`) durably commit their **own block-N state immediately, per block**, inside `commit_block` (e.g. `kv` `db.commit()` at `crates/system/kv/src/lib.rs:355-374`).
- **In-memory modules** (`automations`, `inbox`, `directory`, `tasks`, `profiles`, `memory`, `jobs`, `agent`, `saga`, `valset`, `governance`, `vaults`) — `commit_block` only mutates RAM; they become durable **only at the periodic checkpoint** (`checkpoint_blocks = 32`, `bin/node/src/config.rs:689`, driven at `bin/node/src/main.rs:3002-3048`) or on a **graceful RPC shutdown** (`bin/node/src/main.rs:2726-2748`).

The host commit loop persists a block by looping `commit_block()` over the touched modules **with no cross-module fence** (`crates/kernel/host/src/lib.rs:470-480`).

A single user block routinely spans **both** regimes: a `chat` post fans out to its registered hooks in the poster's own block (`crates/apps/chat/src/lib.rs:1057-1063`), and `automations` subscribes to chat/tasks/inbox/memory hooks (`bin/node/src/main.rs:281`, `:596`). So one post durably commits `chat` (disk, height N) while `automations`/`inbox`/`tasks` exist only in RAM.

On a **hard restart** — and `bin/node` installs **no SIGTERM/SIGINT handler** that routes to the graceful checkpoint (only `process::exit`; no `tokio::signal`/`ctrl_c` anywhere in `bin/node` or the kernel) — recovery (`crates/kernel/recovery/src/lib.rs:922`) restores the in-memory cohort from the **last checkpoint** (genesis / `height=None` for a fresh node) while disk modules recover themselves to N. Replaying block N's seal, the changed set spans a disk module already at its **post**-root and an in-memory module still at its **pre**-root → neither all-applied nor all-unapplied → `Error::Torn` (`crates/kernel/recovery/src/lib.rs:1005-1042`); the app_hash-mismatch variant is the same gap when the per-block torn test passes but the recomposed composite still diverges (`:1087-1095`).

The recovery crate's own invariant note (`crates/kernel/recovery/src/lib.rs:24-37`) documents the **now-stale** assumption it relies on — "no block commits to more than one disk substrate and the only cross-module dispatch stays in the in-memory cohort." The **hook fan-out broke that.** Even a single-disk-substrate block is exposed, because the disk substrate's per-block durability lags the in-memory cohort's 32-block checkpoint.

## Key code locations
- `crates/kernel/host/src/lib.rs:454-513` (`submit_at`; the `:470-480` commit loop, no barrier)
- `crates/kernel/node/src/lib.rs:952-1050` (pre_apply WAL → submit_at fsync → seal(append, not synced))
- `crates/system/kv/src/lib.rs:355-374` (disk module: durable per-block `db.commit()`)
- `crates/apps/inbox/src/lib.rs:274-287` (in-memory module: durable only at checkpoint snapshot)
- `bin/node/src/main.rs:3002-3048` (periodic checkpoint every 32), `:2726-2748` (graceful-only checkpoint), `:281`/`:596` (automations hook wiring)
- `bin/node/src/config.rs:689` (`DEFAULT_CHECKPOINT_BLOCKS = 32`)
- `crates/kernel/recovery/src/lib.rs:1005-1042` (Torn), `:1087-1095` (Verify), `:24-37` (stale invariant)

## Fix directions (no code changed here)
1. **Immediate mitigation (small, closes the common path):** install SIGTERM/SIGINT handlers that run the same graceful checkpoint + journal sync as `RpcRequest::Shutdown`. A supervised/app-driven restart then takes the safe path. This directly closes the reproduced brick (the desktop shell SIGTERMs the daemon on quit with no checkpoint). Does **not** fix a true crash. ⚠️ touches `bin/node/src/main.rs`, which currently has uncommitted WIP.
2. **Structural (a):** drive the in-memory cohort's durability from the WAL — on restart, roll in-memory modules **forward** from the synced op-journal to the disk tip; for a torn block apply staged writes only to modules whose live root is still at *pre* (per-module root-compare replay), replacing the all-or-nothing gate at `recovery/src/lib.rs:1015-1042`.
3. **Structural (b):** stop disk modules committing per block — stage and `db.commit()` only at the same checkpoint boundary as the in-memory cohort, so every module recovers to the last checkpoint and the WAL replays the suffix uniformly (restores the all-at-pre invariant recovery expects).
4. **2PC alternative:** per-commit height cursor in qmdb's commit metadata slot (as `recovery/src/lib.rs:36-37` itself proposes) + fence the in-memory checkpoint behind all disk commits for the block.
5. Update the stale invariant note (`recovery/src/lib.rs:24-37`).

Relates to the "reboot-window / frame-replay" residual tracked in `handoff-blocktime-fork.md` and the blocktime memory.
