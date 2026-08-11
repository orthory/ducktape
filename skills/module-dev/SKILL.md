---
name: module-dev
description: Use when creating a new ducktape consensus module, porting a native module to a wasm guest, or wiring a module into the genesis set — MODULE_IDS, host_state, crates/guests, Makefile wasm-modules. Also when a module change breaks the root-hash, the registry parity test, include_bytes compilation, or wasm-modules-check.
---

# Module development — the end-to-end wiring runbook

A module is four layers: native crate (the logic) → wasm guest (the packaging)
→ registration (across FOUR bins, not one) → proofs (parity + fixtures).

**REQUIRED BACKGROUND:** `docs/records/architecture/wasm-module-authoring.md`
— the guest contract (host-owned state, sibling reads, live update, the
cutover pattern). This skill is the wiring checklist that record doesn't cover.
External/third-party authoring rides `ducktape-quack`, not this path.

## Decide first: genesis registration is a root-hash break

A module in `MODULE_IDS` joins the genesis set: every existing workspace fails
closed, dev networks re-genesis. Post-genesis admission (lifecycle
`ScheduleRegister`) exists, but the recovery/state-sync composers still
enumerate a fixed module set, so restore past an admitted module's first
checkpoint fails closed. A new module today ⇒ a new genesis — get that agreed
before wiring.
Experiments that shouldn't pay this cost live unwired in `crates/labs`.

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
`bin/noded/src/index.rs` (the bundled include_bytes registry). The fold runs
ASYNC behind a fluent31 changes-mode trigger — views trail the op feed
observably (`/v1/index/status` `fold.{module}`), never atomically.

## 3. Registration — four bins compose modules

| Bin | Runs | What to touch |
|---|---|---|
| `bin/node` | production set: native + wasm tenants | `constants.rs`: `MODULE_IDS` (bump the `[..; N]` literal). `host_state.rs`: ~10 sites — grep an existing module id and mirror EVERY hit (`include_bytes!`, id const, `genesis_<id>_wasm`, `seeded_lifecycle`, `seed_genesis_components`, `ProductionModules` field, `compose`, `genesis_host`, `restore_host`, `sync_all_modules`). `Cargo.toml` dep. |
| `bin/noded` | daemon, composes native instances | grep `"tasks"`: id list, `use`, construct, register |
| `bin/simnode` | deterministic /v1 twin | same shape as noded |
| `bin/demo` | in-process walkthrough | same shape |

noded/simnode/demo run a SUBSET — a wasm-only tenant (e.g. `vaults`) appears
in `bin/node` alone. Decide whether the module belongs in the daemon/sim lanes;
if it should be testable in sim-lane or visible in the app, it does.

A new module joins `MODULE_IDS`; keep its declared `[..; N]` length equal to
its contents (the auto-merge count trap).

## 4. Gates — ordering is load-bearing

```
cargo test -p <id>                                        # 1. native logic
cargo run -p guest-builder -- crates/modules/<plane>/<id> # 2. catches native-dep leaks
make wasm-modules                                         # 3. BEFORE bin/node compiles —
                                                          #    include_bytes! needs the artifact
cargo check --workspace --all-targets                     # 4. registry parity test gates
cargo clippy -p <id> --tests --no-deps                    #    constants↔live-module drift
make wasm-modules-check                                   # 5. committed copies byte-identical
```

## Common mistakes

| Mistake | Reality |
|---|---|
| Registering only in `bin/node` | noded/simnode/demo compose their own instances; the module is invisible in daemon/sim lanes |
| Array `[..; N]` length left stale after adding/removing a module | the declared length must equal the contents or the build fails (the auto-merge count trap) |
| Guest added to root workspace members | guests are standalone BY DESIGN; membership poisons native feature unification |
| `include_bytes!` before `make wasm-modules` | bin/node cannot compile until the component exists |
| Rebuilding one guest's component alone | bytes are toolchain-dependent; refresh the set together or `wasm-modules-check` fails |
| Native-only dep in the module crate | wasm32 build breaks; gate it behind the `native` feature (the `files`/`chat` shape) |
