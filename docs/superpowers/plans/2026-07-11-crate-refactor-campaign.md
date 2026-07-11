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
- forge `MultiRepoV2` dual-path: NOT compat debt — it is the only real-module
  end-to-end exercise of the live height-gated upgrade mechanism
  (`bin/node/tests/upgrade_adversarial.rs`, `/upgrade` runbook). Batch 5 keeps
  it and deletes only the zero-value pieces around it (`norm_repo_at`, dead
  `active_version()` getter, legacy `ForgeMsg::Push`). Say the word and the
  whole v2 apparatus (~150 lines, version-branching in 5 methods) goes.
- consensus BLS/V2 scheme surface: deleted in batch 1 (production arms were
  `unimplemented!()` bails; this IS the "reserved enum space" the campaign
  licenses deleting). Re-adding is a fresh build, not a revert.

Wire breaks are flag days — precedent set 2026-07-09; each PR that breaks
wire says so in its description. The TS client (`app/`) is updated in the
same PR when a wire type it speaks changes.

## Status

- [x] Batch 1 — kernel (PR): state→host, reactor→host (worker seam; dead `Reactor` struct deleted), node gossip cluster deleted, consensus BLS/V2 surface deleted + spawn dedup, statesync/recovery compat-decode deletions (wire + checkpoint flag days), capability-host API diet, indexer trims. 53→51 crates.
- [ ] Batch 2 — system core
- [ ] Batch 3 — network cluster
- [ ] Batch 4 — storage & naming
- [ ] Batch 5 — apps A
- [ ] Batch 6 — apps B
- [ ] Batch 7 — bins
- [ ] Batch 8 — bin/node
- [ ] Batch 9 — shell
