---
name: module-dev
description: Use when creating a new ducktape consensus module, porting a native module to a wasm guest, or wiring a module into the genesis set — topology::PRODUCTION, host_state, crates/guests, Makefile wasm-modules. Also when a module change breaks the root-hash pin, the registry parity test, a missing component in the fixtures/modules dir, or wasm-modules-check.
---

# Module development — the end-to-end wiring runbook

A module is four layers: native crate (the logic) → wasm guest (the packaging)
→ registration (the topology, then THREE bins) → proofs (parity + fixtures).

**REQUIRED BACKGROUND:** `docs/records/architecture/wasm-module-authoring.md`
— the guest contract (host-owned state, sibling reads, live update, the
cutover pattern). This skill is the wiring checklist that record doesn't cover.
External/third-party authoring rides `ducktape-quack`, not this path.

## Decide first: genesis registration is a root-hash break

A module in `topology::PRODUCTION` (`bin/node`'s `MODULE_IDS` IS that
selection) joins the genesis set: every existing workspace fails closed, dev
networks re-genesis, and `GENESIS_ROOT_HASH` moves. A genesis module ⇒ a new
genesis — get that agreed before wiring.

A POST-genesis module does NOT move the root and needs no genesis edit, no
topology row, and no bin change — it is one operator command against a LIVE
network:

```
ducktape module register <id> <component.wasm> [--after N]  # admit a new id
ducktape module update   <id> <component.wasm> [--after N]  # swap live code
ducktape module status                                      # the registry
```

`register`/`update` stage the component at this node's owner-gated admin route
(which fans it out to every validator and returns their receipts), then drive
the governance proposal that schedules the admission/swap; it activates at
`height + N` (`N > MIN_SWAP_LEAD`, i.e. `> 3`; default 50 to leave room for the
ceremony's own blocks). `status` prints one row per module — `id  active
pending`, a pending swap carrying `ready k` (validators that signalled) or
`ready ✓`. `adopt_admitted_modules` composes every admitted id on restore and
state sync, and a module admitted after the last checkpoint restores empty and
is rebuilt by replay (unit-pinned in `host_state.rs`).

The CLI stages bytes, it never builds them: the component still comes from
`make wasm-modules` / `guest-builder` (§2).
Experiments that shouldn't pay the genesis cost live unwired in `crates/labs`.

## 1. Native crate — `crates/modules/{apps|system}/<id>`

Clone the `tasks` shape:
- `src/interface.rs` — wire types + codecs at crate root. The ONLY surface
  other modules may import.
- `src/lib.rs` — the struct + `impl sdk::Module` (`root`, `execute`, `query`,
  `commit_block`) + `snapshot()`/`install()`. Re-export `interface` at root.
- `tests/` — happy path, every rejection, snapshot→install root round-trip.
- Root `Cargo.toml`: `members` entry + `[workspace.dependencies]` alias.
- Native-only deps (media engines, unix IO, tokio) must sit behind a `native`
  feature or be absent — the guest builds compile this same crate to wasm32.

## 2. Wasm guest — `src/guest.rs` in the module crate, packaged by guest-builder

The module carries its OWN port (the `tasks`/`chat`/`files` shape): a
`src/guest.rs` behind a wasm-only `guest = ["dep:guest-adapter"]` feature —
the doc header, the id consts, and ONE dispatch-shell macro
(`guest_adapter::snapshot_guest!` for whole-state `SnapshotBytes` modules,
`store_guest!` for store-backed ones, or a hand-written `Guest` impl +
`export_module!` for odd tenants like files). `#[cfg(feature = "guest")] mod
guest;` in lib.rs. No packaging crate is checked in: `bin/guest-builder`
synthesizes the ephemeral cdylib workspace (wasm32 dep resolution + the
getrandom/blst patch set, isolated from the host workspace) and writes the
canonical COMMITTED `component.wasm` into the module directory.

`Makefile`: add the module to `BUILDER_MODULES` — that one entry covers the
build, the fixture `cp`, and the `wasm-modules-check` `cmp`.

## 2b. Index guest (optional) — the module's derived-tier mapper

A module that wants a materialized view (search, listings — anything qmdb's
point lookups can't serve) ships a SECOND wasm artifact: the index guest
(spec: `docs/records/specs/indexable-spec.md`). Same two-file shape in the
module crate:
- `src/index.rs` — the pure decision core: `fold_op`/`serve_view` over the
  `index_guest` contract crate (dep `index_guest = { workspace = true }`,
  never `indexer`). Unit-test natively against a `BTreeMap`.
- `src/index_guest.rs` behind `index-guest = ["index_guest/guest"]` — the
  ~15-line engine shell (`EngineRead`, `apply`, `index_guest::fold!`/`view!`).

`guest-builder --index <module-dir>` writes the committed `index.wasm`; add
the module to `INDEX_MODULES` in the Makefile and to `index_guest_wasm()` in
`crates/noded/src/index.rs` (the bundled include_bytes registry). The fold runs
ASYNC behind a fluent31 changes-mode trigger — views trail the op feed
observably (`/v1/index/status` `fold.{module}`), never atomically.

The engine's side of that contract — no backfill at registration, at-least-once
invocation with exactly-once effects, a row above the inline cap arriving
key-only, a failing fold holding its queue with backoff — is fluent31's own
`SKILL.md` ("Priors that do not transfer"), at the rev the root `Cargo.toml`
pins; read it before writing a mapper. A guest's `log` output is a `debug`
event under `fluent31::wasm::guest` — turn that one target up to see it.

## 3. Registration — `module register`, or (for genesis) the topology

Post-genesis is the whole of this section for most modules: `ducktape module
register <id> <component.wasm>` admits the id on a live network and `ducktape
module update <id> <component.wasm>` swaps its code later. The registry is
consensus state, so nothing below needs editing — every node composes the
admitted module from it.

The table is the GENESIS path: the flag day that moves the root hash. There
is ONE source: `crates/topology/src/lib.rs`. Every binary composes from it
through `crates/noded/src/compose.rs` — `bin/node` from `PRODUCTION`, `bin/noded`
and `bin/simnode` from `SIM_BASE` (+ `SIM_VALSET` under simnode's
`--with-valset`) — so a wasm store-backed module touches no bin at all.

| Where | What to touch |
|---|---|
| `crates/topology/src/lib.rs` | a `ModuleSpec` row in `MODULES` (`code`/`backing`/`config`) and the id in the selection(s) it joins. The siblings a module reads are compiled into its guest, not declared here; `host_state` composes genesis/restore/sync from the selection — nothing to mirror there. The component is NOT embedded: `node init` hashes `<id>.component.wasm` out of `--modules <dir>` (default `$DUCKTAPE_MODULES_DIR`, else `<ducktape home>/modules` — `$DUCKTAPE_HOME` when set, else `~/.ducktape` — filled by `make install-node`) into the descriptor, then copies those bytes into `<workspace>/modules/`. The kernel fixtures dir pins the same bytes. |
| `crates/noded/src/compose.rs` | ONLY for a `Code::Native` tenant (an arm in `native`, plus the `Cargo.toml` dep) or a `Backing::Odb` tenant (an arm in `open_odb` opening its disk substrate). A wasm store-backed module needs neither. |
| the indexer | `open_index_store` opens a database for EVERY id in the selection, so joining or leaving a selection gains or loses one — nothing to touch for a module with no mapper. A module that ships one also needs its arm in `index_guest_wasm()` (`crates/noded/src/index.rs`, `include_bytes!` of the committed `index.wasm`) and its `INDEX_MODULES` entry in the `Makefile`. |

`SIM_BASE` is 15 of production's 19; the four it leaves out — `acl`,
`governance`, `lifecycle`, `valset` — are exactly what simnode's
`--with-valset` appends (with native `kv`). Decide which selection a new
module joins: `SIM_BASE` if it should boot by default (testable in sim-lane,
visible in the app), `SIM_VALSET` if it is governance-shaped.

A new module joins `topology::PRODUCTION`; update the topology's count and
membership pins and `host_state.rs`'s `GENESIS_ROOT_HASH` in the SAME commit
(the failing pin prints the new hex) and name the flag day in the message.

## 4. Gates — ordering is load-bearing

```
cargo test -p <id>                                        # 1. native logic
cargo run -p guest-builder -- crates/modules/<plane>/<id> # 2. catches native-dep leaks
make wasm-modules                                         # 3. BEFORE the node pins run —
                                                          #    the fixtures dir needs the artifact
cargo check --workspace --all-targets                     # 4. registry parity test gates
cargo clippy -p <id> --tests --no-deps                    #    topology↔composed-host drift
make wasm-modules-check                                   # 5. committed copies byte-identical
make wasm-index-check                                     # 6. index guests match a rebuild (needs wasm32)
```

## Common mistakes

| Mistake | Reality |
|---|---|
| Adding the id to `PRODUCTION` only | noded/simnode compose `SIM_BASE`/`SIM_VALSET`; the module is invisible in daemon/sim lanes until it joins one of those too |
| Topology pins or `GENESIS_ROOT_HASH` left stale after adding/removing a module | `cargo test -p topology` and the root-hash pin fail; update both in the same commit |
| Guest added to root workspace members | guests are standalone BY DESIGN; membership poisons native feature unification |
| Node pins run before `make wasm-modules` | the fixtures dir lacks the component; `hash_bundle` refuses by name |
| Rebuilding one guest's component alone | bytes are toolchain-dependent; refresh the set together or `wasm-modules-check` fails |
| Native-only dep in the module crate | wasm32 build breaks; gate it behind the `native` feature (the `files` shape) |
