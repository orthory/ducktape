---
name: module-dev
description: Use when creating a new ducktape consensus module, porting a native module to a wasm guest, or wiring a module into the genesis set — MODULE_IDS, host_state, crates/guests, Makefile wasm-modules. Also when a module change breaks the app-hash, the registry parity test, include_bytes compilation, or wasm-modules-check.
---

# Module development — the end-to-end wiring runbook

A module is four layers: native crate (the logic) → wasm guest (the packaging)
→ registration (across FOUR bins, not one) → proofs (parity + fixtures).

**REQUIRED BACKGROUND:** `docs/records/architecture/wasm-module-authoring.md`
— the guest contract (host-owned state, sibling reads, live update, the
cutover pattern). This skill is the wiring checklist that record doesn't cover.
External/third-party authoring rides `ducktape-quack`, not this path.

## Decide first: genesis registration is an app-hash break

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
- Native-only deps (derived index, unix IO, tokio) must sit behind a `native`
  feature or be absent — the guest build compiles this same crate to wasm32.

## 2. Wasm guest — `crates/guests/<id>-wasm`

Copy `tasks-wasm`: standalone workspace (own `[workspace]` table — NEVER a
root member; that isolation keeps wasm32 deps/features out of the native
build), `crate-type = ["cdylib"]`, deps = `guest-adapter` + the native crate
(`default-features = false` if it has a `native` feature). `component.wasm`
is a COMMITTED artifact.

`Makefile`: add a stanza to `wasm-modules` (cargo build wasm32 + `wasm-tools
component new`); if kernel tests pin a fixture, also the `cp` into
`crates/kernel/host/tests/fixtures/` and the matching `wasm-modules-check`
`cmp` line. Refresh the whole set together — bytes are toolchain-dependent.

## 3. Registration — four bins compose modules

| Bin | Runs | What to touch |
|---|---|---|
| `bin/node` | production set: native + wasm tenants | `constants.rs`: `MODULE_IDS` + `MODULE_STATE_SCHEMAS` (bump the `[..; N]` literals). `host_state.rs`: ~10 sites — grep an existing module id and mirror EVERY hit (`include_bytes!`, id const, `genesis_<id>_wasm`, `seeded_lifecycle`, `seed_genesis_components`, `ProductionModules` field, `compose`, `genesis_host`, `restore_host`, `sync_all_modules`). `Cargo.toml` dep. |
| `bin/noded` | daemon, composes native instances | grep `"tasks"`: id list, `use`, construct, register |
| `bin/simnode` | deterministic /v1 twin | same shape as noded |
| `bin/demo` | in-process walkthrough | same shape |

noded/simnode/demo run a SUBSET — a wasm-only tenant (e.g. `vaults`) appears
in `bin/node` alone. Decide whether the module belongs in the daemon/sim lanes;
if it should be testable in sim-lane or visible in the app, it does.

A new module joins the two live arrays — `MODULE_IDS` and
`MODULE_STATE_SCHEMAS` — and keep each array's declared `[..; N]` length equal
to its contents (the auto-merge count trap).

## 4. Gates — ordering is load-bearing

```
cargo test -p <id>                                        # 1. native logic
cd crates/guests/<id>-wasm && \
  cargo build --target wasm32-unknown-unknown --release   # 2. catches native-dep leaks
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
