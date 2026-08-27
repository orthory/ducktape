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
networks re-genesis, and `GENESIS_ROOT_HASH` moves. Post-genesis admission (lifecycle
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
`crates/noded/src/index.rs` (the bundled include_bytes registry). The fold runs
ASYNC behind a fluent31 changes-mode trigger — views trail the op feed
observably (`/v1/index/status` `fold.{module}`), never atomically.

## 3. Registration — the topology, then three bins

| Bin | Runs | What to touch |
|---|---|---|
| `bin/node` | production set: native + wasm tenants | `crates/topology/src/lib.rs`: a `ModuleSpec` row in `MODULES` (`code`/`backing`/wiring) and the id in `PRODUCTION`; `host_state` composes genesis/restore/sync from that selection — nothing to mirror there. A native tenant also needs the `Cargo.toml` dep. The component is NOT embedded: `node init` hashes `<id>.component.wasm` out of `--modules <dir>` (default `$DUCKTAPE_MODULES_DIR`, else `~/.ducktape/modules`, filled by `make install-node`) into the descriptor, then copies those bytes into `<workspace>/modules/`. The kernel fixtures dir pins the same bytes. |
| `bin/noded` | daemon, composes native instances | grep `"tasks"`: id list, `use`, construct, register |
| `bin/simnode` | deterministic /v1 twin | same shape as noded |

noded/simnode run a SUBSET — a wasm-only tenant (e.g. `capability`) appears
in `bin/node` alone. Decide whether the module belongs in the daemon/sim lanes;
if it should be testable in sim-lane or visible in the app, it does.

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
```

## Common mistakes

| Mistake | Reality |
|---|---|
| Registering only in `bin/node` | noded/simnode compose their own instances; the module is invisible in daemon/sim lanes |
| Topology pins or `GENESIS_ROOT_HASH` left stale after adding/removing a module | `cargo test -p topology` and the root-hash pin fail; update both in the same commit |
| Guest added to root workspace members | guests are standalone BY DESIGN; membership poisons native feature unification |
| Node pins run before `make wasm-modules` | the fixtures dir lacks the component; `hash_bundle` refuses by name |
| Rebuilding one guest's component alone | bytes are toolchain-dependent; refresh the set together or `wasm-modules-check` fails |
| Native-only dep in the module crate | wasm32 build breaks; gate it behind the `native` feature (the `files` shape) |
