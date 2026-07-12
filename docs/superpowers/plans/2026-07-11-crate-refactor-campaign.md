# Crate-by-Crate Review & Refactor Campaign

**Goal:** review every workspace crate and refactor it, with an explicit
license to break backwards compatibility (no reserved enum space, no legacy
decode paths, no version-gated branches for retired binaries), and to join,
split, or delete crates.

**Ground truth at kickoff (2026-07-11):** ~140k lines of Rust in 53 crates.
318 grep hits for compat-debt markers (`reserved|legacy|deprecated|compat`).
Largest: `bin/node` 28.1k, `apps/runs` 10.9k, `app/src-tauri` 10.0k,
`bin/noded` 7.8k, `apps/chat` 5.7k, `system/nat-traversal` 5.0k.

## Approaches considered

- **A. Bottom-up dependency-ordered tier sweeps (chosen).** Review producers
  before consumers: interface/kernel crates first, then system modules, then
  apps, then bins. Interface changes (deleting reserved wire space, changing
  enum layouts) originate at the bottom and ripple up once; reviewing a
  consumer before its producer means reviewing it twice.
- **B. Debt-ranked (biggest first).** Hit `bin/node`/`runs` monsters first.
  Rejected: their bulk is consumption of kernel/system APIs — pruning those
  APIs first shrinks the monsters for free.
- **C. Theme sweeps** (one tree-wide compat-deletion PR, one consolidation
  PR, one split PR). Rejected as the primary axis: unreviewable diffs that
  cross every crate. Folded in instead: each tier visit runs the compat and
  consolidation checklists for its crates.

## Per-crate checklist (run at each visit)

1. **Read it fully** (subagent fan-out for >3k-line crates).
2. **Compat scaffolding — delete:** reserved enum variants / discriminant
   gaps, legacy decode/upcast paths, version-gated branches for pre-flag-day
   binaries, `#[deprecated]`/`#[allow(deprecated)]` items, dual codecs kept
   "for migration".
3. **Over-engineering — delete or inline:** single-impl traits, dead `pub`
   API with no callers outside tests, speculative config/generics,
   re-implemented helpers that exist in a dep or std.
4. **Crate verdict:** keep / merge into named neighbor / split / delete.
   Constraint that survives: types-only interface crates stay the only legal
   cross-module surface; app modules stay isolated modules (don't merge apps
   into each other). Tiny infra crates are fair game.
5. **File shape:** decompose >600-line mono-files touched by the refactor
   (standing user mandate); don't reformat untouched code.
6. **Gates:** `touch` sources then `cargo clippy -p <crate> --tests
   --no-deps` + `cargo test -p <crate>`; `cargo check -p files
   --no-default-features` when files/duckfs-core are touched;
   `cargo check --workspace` before every PR.
7. **Feature-regression gates (every batch, user mandate 2026-07-11):**
   (a) FULL `cargo test --workspace` on the branch, not just touched crates;
   (b) the TS app suite (`bun run typecheck` + `bun run test` in app/) —
   wire-shape changes only surface there; (c) a deleted-symbol cross-surface
   sweep: every deleted pub item / enum variant / struct field / serde
   default is grepped through app/src, docs/, ops/, skills/, Makefile and
   dynamic string-routed wire (the `CapabilityQuery::All` near-miss class);
   (d) clean-context PR review before merge.

## Batches (one worktree + one PR against dev each)

| # | Batch | Crates | ~lines |
|---|-------|--------|--------|
| 1 | Kernel | sdk, state, host, node, consensus, reactor, statesync, recovery, indexer, capability-host | 19k |
| 2 | System core | kv, valset, governance, identity, upgrade, capability, saga, dispatch, dispatch-oracle, tagging, blobstore | 15k |
| 3 | Network cluster | wireguard-upgrade, wireguard-effect, nat-traversal, reachability, data-plane, overlay-net | 17k |
| 4 | Storage & naming | duckfs-core/-disk/-client, duckdns-core, system/duckdns, gateway, bin/fs | 11k |
| 5 | Apps A | chat, forge, pages, agent, tagging consumers | 16k |
| 6 | Apps B | runs, tasks, vaults, automations, inbox, files, jobs | 17k |
| 7 | Bins | demo, noded, simnode, coordinator, examples/{directory,greeter} | 11k |
| 8 | bin/node | the 28k monster, post-diet | 28k |
| 9 | Shell | app/src-tauri | 10k |

Consolidation candidates flagged upfront (final call at the batch review):
`state` (122 lines) → into `sdk`; `blobstore` (240) → into its one consumer
surface; `duckdns-core` + `system/duckdns` → one crate; `reactor` (414) →
into `host`; network cluster likely 6 → 3-4 crates; `examples/*` deleted if
nothing outside them depends on them.

Flagged user decisions (default = the conservative side, veto either way):
- forge `MultiRepoV2` dual-path: RESOLVED 2026-07-12 — v2 ADOPTED as the only
  layout (flag day: every forge root moves, snapshots always carry the FGv2
  magic) and the dual-path machinery deleted (`ForgeLayout`, `forge_layout`,
  `active_version` field + `set_active_version` override/inherent, version
  branches in root/snapshot/install). User call: "not on production, we can be
  flexible; merge v1 v2". The upgrade mechanism keeps kernel-level coverage
  (recovery tests' synthetic dual modules, `upgrade_e2e.rs`, and the surviving
  mid-window-admission leg of `upgrade_adversarial.rs`).
- consensus BLS/V2 scheme surface: deleted in batch 1 (production arms were
  `unimplemented!()` bails; this IS the "reserved enum space" the campaign
  licenses deleting). Re-adding is a fresh build, not a revert.

Wire breaks are flag days — precedent set 2026-07-09; each PR that breaks
wire says so in its description. The TS client (`app/`) is updated in the
same PR when a wire type it speaks changes.

## Status

- [x] Batch 1 — kernel (PR #391): state→host, reactor→host (worker seam; dead `Reactor` struct deleted), node gossip cluster deleted, consensus BLS/V2 surface deleted + spawn dedup, statesync/recovery compat-decode deletions (wire + checkpoint flag days), capability-host API diet, indexer trims. 53→51 crates.
- [x] Batch 2 — system core (PR #394): shared `sdk::codec`, `valset::mesh` deleted, section-encoding collapse, dispatch poison fixes, dispatch-oracle prune.
- [x] Batch 3 — network cluster (PR #400): wireguard-upgrade + wireguard-effect merged into `system/wireguard` (effect submodule), relay/dial-failure apparatus deleted, nat/data-plane/overlay trims.
- [x] Batch 4 — storage & naming (PR #398): duckdns-core merged into `system/duckdns`, gateway on `sdk::codec`, duckfs trims, bin/fs readdir fix.
- [x] Batch 5 — apps A (PR #403): chat/forge/pages/agent trims, MoveBlock indexer fix.
- [x] Batch 6 — apps B (PR #404): v2 run-envelope machinery deleted, small apps on `sdk::codec`, saga-wedge cap, `FsCap` deleted.
- [x] Batch 7 — bins (PR #407): demo's toy state-sync stack deleted, noded lib split into 7 modules with forge/duckfs reuse.
- [x] Batch 8 — bin/node (PR #411): config.rs split into `config/`, hex/OverlayBook/members dedup, module-registry compile guard, catchup/drain extraction.
- [x] Batch 9 — shell (PR #408): dead gateway window path + legacy identity command + notify dead-target machinery deleted, workspaces split.

Bonus fixes surfaced by the sweep: PR #396 (simnode registers `evm` —
noded↔simnode genesis parity restored), PR #402 (consensus descent
`debug_assert` regression that panicked a mid-epoch joiner, removed).

Capstone (architecture-docs refresh to the post-refactor crate set): in
progress.
