# Live Upgrade Part 5 — noded/simnode onto the composer — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The two daemons (`bin/noded`, `bin/simnode`) compose their module sets through `crates/noded/src/compose.rs` from a directory of `<id>.component.wasm` — the same composer, code-source shape, store source and host finisher `bin/node` uses — so there is ONE way a ducktape process builds its modules (spec §3 "One composer, three binaries").

**Architecture:** Promote the three pieces `bin/node` keeps privately (a dir-backed `host::CodeSource`, the qmdb store source, the `Host` finisher with the wasm module factory) into `crates/noded` next to `compose`, then replace each daemon's hand-built native module list with one `compose(selection, …)` call: `SIM_BASE` (all 14 wasm) for noded, `SIM_BASE` + `SIM_VALSET` (acl/governance wasm, kv/valset/lifecycle native) for simnode when validator keys are given. Composing governance as wasm wires its code registry, so the sim's "UpdateModule gated off" restriction disappears. The sim's golden root moves once (named); `bin/node`'s `GENESIS_ROOT_HASH` must NOT move.

**Tech Stack:** Rust; `noded::compose`; `topology`; `workspace_config::modules_dir`; wasmtime via `wasm_host`.

**Spec:** `docs/superpowers/specs/2026-08-27-live-upgrade-design.md` §3 (callers list, golden-hash note). Corrections to §3 recorded below.

## Global Constraints

- No compat/legacy/dual-path code: the native constructors leave the daemons entirely; no "compose OR hand-build" switch.
- `bin/node`'s `GENESIS_ROOT_HASH` (`bin/node/src/host_state.rs`) does not move in this PR (Task 1 is a pure relocation; Tasks 2–3 touch only the daemons).
- `bin/simnode/tests/topology_set.rs` `DEFAULT_GENESIS_ROOT_HASH` moves ONCE, in the Task 2 commit, named in its body.
- House rules: one `match` per discriminant, named predicates, early return, no boolean steering; node code uses `tracing` (`info!` at most once per boot); only touched code formatted (never `cargo fmt --all`).
- Per-crate gates: `cargo clippy -p noded -p noded-bin -p simnode -p node-bin --tests --no-deps` clean; `cargo check --workspace --all-targets` clean; `make wasm-modules-check` consistent (no component is rebuilt here).
- Host defect: rustc may die in its incremental dep-graph decode or a DWARF SIGSEGV — rerun with `CARGO_INCREMENTAL=0`; a corrupt rlib → `cargo clean -p <crate>`; never record an env prefix in the repo. One cargo job at a time.

## Corrections to spec §3 (rulings; the spec is updated in Task 4)

| Spec says | Reality (cited) | Ruling |
|---|---|---|
| `trait CodeBytes { fn bytes(&self, id) }` | the composer takes `&dyn host::CodeSource` (`crates/kernel/host/src/lib.rs:124-126`, `fetch(code_hash)` — keyed BY HASH) | a dir-backed `CodeSource` hashes the dir first and keeps hash → id (`crates/noded/tests/compose.rs:12-43` is the working template; Task 1 promotes it) |
| noded `--modules <dir>` default `~/.ducktape/modules` | `workspace_config::modules_dir()` (`crates/workspace-config/src/lib.rs:79-84`: `$DUCKTAPE_MODULES_DIR` else `<home>/modules`) is the one resolver `node init` uses | noded uses it; simnode (dev/test-only) defaults to the repo fixtures dir resolved from `CARGO_MANIFEST_DIR` so the app's tests need no install |
| "the daemon parity lane" pins `sim_base` against noded | no such lane exists anywhere (grep `parity` under bin/noded, crates/noded, Makefile, ops, .github → nothing); `bin/noded/tests/daemon_e2e.rs:411-429` asserts a hand-copied 14-id literal | daemon_e2e asserts `topology::SIM_BASE`; the doc comments naming the lane are corrected |
| noded and simnode golden hashes move | noded pins NO hash (`daemon_e2e.rs` reads `root_hash` only relatively); only the sim's `DEFAULT_GENESIS_ROOT_HASH` exists | one pin moves (Task 2) |
| identity/gateway chain ids | `compose` binds ONE `chain_id` to both identity and gateway (`compose.rs` `config_value`); today the daemons give identity `""` and gateway `"local"` | `chain_id = "local"` for both daemons (the gateway's existing value; identity's `""` was a "no chain" placeholder). If an app test signs a chain-UNSCOPED identity consent against the sim and now fails, the test learns the chain id from `/v1/status` the way the real app does — fix the test, not the binding. |

---

### Task 1: Promote the code source, store source and host finisher into `crates/noded`

**Files:**
- Create: `crates/noded/src/bundle.rs`
- Modify: `crates/noded/src/lib.rs` (add `pub mod bundle;`)
- Modify: `crates/noded/Cargo.toml` only if `statesync` (for `QmdbStore`) / `commonware-runtime` / `wasm-host` / `host` are not already deps (the fact sheet says `statesync` and `wasm-host` are; verify `host` and `commonware-runtime`)
- Modify: `bin/node/src/config/resolve.rs` (delete `component_path`/`hash_bundle` bodies; re-export from `noded::bundle`)
- Modify: `bin/node/src/host_state.rs` (delete `WasmModuleFactory`, `finish`, `canonical_stores`; use `noded::bundle::{WasmModuleFactory, host_from, qmdb_stores}`)
- Modify: `crates/noded/tests/compose.rs` (use `noded::bundle::DirCodeSource` instead of the local `DirSource`/`hashes`)

**Interfaces (Produces):**
```rust
// crates/noded/src/bundle.rs
pub fn component_path(dir: &Path, id: &str) -> PathBuf;                       // verbatim from resolve.rs
pub fn hash_bundle(dir: &Path, ids: &[&str]) -> Result<BTreeMap<String, [u8; 32]>, String>; // verbatim
/// a `host::CodeSource` over a directory of `<id>.component.wasm`, keyed by each file's sha256.
pub struct DirCodeSource { dir: PathBuf, by_hash: BTreeMap<[u8; 32], String> }
impl DirCodeSource {
    /// hash every `<id>.component.wasm` for `ids` (the selection's wasm ids); returns the
    /// source and the id → hash map `Bindings::code_hashes` wants.
    pub fn open(dir: &Path, ids: &[&str]) -> Result<(Self, BTreeMap<String, [u8; 32]>), String>;
}
#[async_trait::async_trait(?Send)]
impl host::CodeSource for DirCodeSource { async fn fetch(&self, code_hash: &[u8]) -> Option<Vec<u8>>; }
/// every store-backed module `init`s its qmdb store under its own id in this runtime's storage root.
pub fn qmdb_stores<'a>(context: &'a commonware_runtime::tokio::Context)
    -> impl FnMut(&'static str) -> compose::BoxFut<'a, Result<Box<dyn sdk::MerkleStore>, String>> + 'a; // verbatim from canonical_stores
/// the wasm-runtime `host::ModuleFactory` (post-genesis admission instantiates from verified bytes).
pub struct WasmModuleFactory;
/// `Host::genesis(modules)` + `set_module_factory(WasmModuleFactory)` — every ducktape host admits post-genesis modules.
pub fn host_from(modules: Vec<Box<dyn sdk::Module>>) -> Result<host::Host, sdk::Error>;
```

- [ ] **Step 1: Write `bundle.rs`** with the items above. `component_path`/`hash_bundle` bodies copied verbatim from `bin/node/src/config/resolve.rs:161-185` (keep the doc comments). `DirCodeSource::open`:
```rust
pub fn open(dir: &Path, ids: &[&str]) -> Result<(Self, BTreeMap<String, [u8; 32]>), String> {
    let by_id = hash_bundle(dir, ids)?;
    let by_hash = by_id.iter().map(|(id, h)| (*h, id.clone())).collect();
    Ok((Self { dir: dir.to_path_buf(), by_hash }, by_id))
}
```
`fetch`: `let digest: [u8; 32] = code_hash.try_into().ok()?; let id = self.by_hash.get(&digest)?; std::fs::read(component_path(&self.dir, id)).ok()`. `qmdb_stores` = `canonical_stores` verbatim (`host_state.rs:213-224`). `WasmModuleFactory` + impl verbatim (`host_state.rs:71-81`). `host_from` = `finish` verbatim (`host_state.rs:206-210`). Module doc: "the pieces every ducktape process needs around [`compose`]: where component bytes come from on disk, where stores come from, and how a composed set becomes a `Host`."
- [ ] **Step 2: Wire `bin/node`** — `resolve.rs`: `pub use noded::bundle::{component_path, hash_bundle};` replacing the two fns; `host_state.rs`: `use noded::bundle::{host_from, qmdb_stores, WasmModuleFactory};` and delete the three private items, renaming call sites (`finish(` → `host_from(`, `canonical_stores(` → `qmdb_stores(`). `BlobCodeSource` stays in `bin/node` (blob-plane specific).
- [ ] **Step 3: Wire the composer test** — `crates/noded/tests/compose.rs`: delete `DirSource`, `hashes`, `ByHash`; `let (code, by_id) = DirCodeSource::open(&fixtures(), &["acl", "governance", "runs"])?;` at each site.
- [ ] **Step 4: Verify** — `cargo test -p noded` (composer tests green), `cargo test -p node-bin --bin ducktape` (435; `GENESIS_ROOT_HASH` unchanged — a moved pin here means the relocation was not pure), `cargo clippy -p noded -p node-bin --tests --no-deps`.
- [ ] **Step 5: Commit** — `refactor(noded): the dir code source, qmdb store source and host finisher live beside the composer`.

---

### Task 2: simnode composes through `compose`

**Files:**
- Modify: `bin/simnode/src/lib.rs` (`SimOpts.modules_dir`, `boot`, `run_sim`)
- Modify: `bin/simnode/src/main.rs` (`--modules <dir>` flag)
- Modify: `bin/simnode/tests/topology_set.rs` (`DEFAULT_GENESIS_ROOT_HASH`)
- Modify: `bin/simnode/tests/harness/mod.rs` only if the spawned binary needs `--modules` (it should not: the default resolves from `CARGO_MANIFEST_DIR`)

**Interfaces:**
- Consumes: Task 1's `noded::bundle::{DirCodeSource, host_from, qmdb_stores}`, `noded::compose::{compose, Bindings, Boot, Substrates}`, `topology::{SIM_BASE, SIM_VALSET, TOPOLOGY}`.
- Produces: `SimOpts { …, pub modules_dir: Option<PathBuf> }` (`None` → `PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crates/kernel/host/tests/fixtures")`), binary flag `--modules <DIR>`.

- [ ] **Step 1: RED** — in `topology_set.rs` add, next to the default-set test, a test that boots the sim with `--with-valset <one key>` and asserts `/v1/status` lists `SIM_BASE ++ SIM_VALSET` AND that a governance `UpdateModule` proposal is no longer refused with `"no code registry wired"` (submit `GovMsg::Propose { action: GovAction::UpdateModule { name: "x".into(), module_id: "chat".into(), activation_height: 10_000, code_hash: vec![0; 32] }, voting_period: 600_000, proposal_id: "u".into() }` through the sim's `/v1/submit` as the harness does elsewhere, and assert the reply is accepted — the lifecycle will refuse the unknown hash at execute, which is fine; the point is governance now has a registry). Run: fails today with the "no code registry wired" refusal.
- [ ] **Step 2: `SimOpts.modules_dir`** — add the field + doc ("the directory of `<id>.component.wasm` the sim composes its wasm tenants from; `None` = the repo's kernel fixtures, so an embedder in this checkout needs no install"), `Default` → `None`; destructure it in `boot`; pass through `run_sim` as `modules_dir: PathBuf` (resolved: `modules_dir.unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crates/kernel/host/tests/fixtures"))`).
- [ ] **Step 3: Replace the composition** (`lib.rs:773-911`) with:
```rust
    executor.start(|context| async move {
        // genesis: the topology selection composed the way bin/node composes —
        // wasm tenants from the modules dir, the native registries seeded from
        // the sim's bindings. governance composes as wasm, so its code registry
        // is wired and UpdateModule proposals are live in the sim.
        let selection: Vec<&'static str> = module_ids.clone();
        let wasm_ids = TOPOLOGY.wasm_ids(&selection);
        let (code, code_hashes) = DirCodeSource::open(&modules_dir, &wasm_ids).expect("sim modules dir");
        let substrates = Substrates { forge_repo, duckfs_dir, blobs: blobs.clone() };
        let bindings = Bindings {
            invite: &invite_binding,
            chain_id: "local",
            validators: &valset_keys,
            code_hashes: &code_hashes,
        };
        let mut stores = qmdb_stores(&context);
        let mut modules = compose(&selection, &code, &mut stores, &substrates, &bindings, Boot::Genesis)
            .await
            .expect("sim genesis composes");
        // anything the topology does not know stays pushed after the composed set (the echo oracle)
        <keep the existing `echo_oracle` push here, unchanged>
        let host = host_from(modules).expect("genesis");
```
  Delete the 14 native constructors and the `if !valset_keys.is_empty()` block; delete their `use` lines; `module_ids` stays the status list. Check what `TOPOLOGY.wasm_ids` returns (`Vec<&'static str>`) and adapt `DirCodeSource::open`'s `ids: &[&str]` call accordingly.
- [ ] **Step 4: Binary flag** — `main.rs`: `"--modules" => modules_dir = Some(PathBuf::from(args.next().ok_or("--modules needs a dir")?))`, passed into `SimOpts`.
- [ ] **Step 5: GREEN + golden** — `cargo test -p simnode`: the Step 1 test passes; `the_default_genesis_root_is_pinned` (or whatever `topology_set.rs`'s default test is named) FAILS with the new root — set `DEFAULT_GENESIS_ROOT_HASH` to it. Then `cargo test -p ducktape-app` (the app boots the sim in `app/src/backend/tests/{messages,wire}.rs`) — expect green; if an identity-consent test fails on the chain id, apply the ruling in the corrections table (the test reads `chain_id` from `/v1/status`). Record the app lane's wall time before/after (the wasm load is the accepted cost).
- [ ] **Step 6: Commit** — `feat(simnode): the sim composes its genesis through the topology composer (sim golden <old> → <new>)`.

---

### Task 3: noded composes through `compose`

**Files:**
- Modify: `bin/noded/src/main.rs` (`--modules` flag, the composition in `run_node`)
- Modify: `bin/noded/Cargo.toml` (add `workspace-config` if absent)
- Modify: `bin/noded/tests/daemon_e2e.rs` (spawn with `--modules <fixtures>`; assert `topology::SIM_BASE`)

**Interfaces:**
- Consumes: Task 1's `noded::bundle::*`, `noded::compose::*`, `workspace_config::modules_dir()`.
- Produces: `ducktape-noded --modules <DIR>` (default `workspace_config::modules_dir()`; a missing dir refuses at boot with the same remedy sentence `node init` prints: "fill `~/.ducktape/modules` (`make install-node`)").

- [ ] **Step 1: RED** — `daemon_e2e.rs:411-429`: replace the literal with `topology::SIM_BASE.iter().map(|s| s.to_string()).collect::<Vec<_>>()` (add `topology` to noded-bin's dev-deps if not a dep — it is a normal dep per the fact sheet). This is not red by itself (same 14 ids) — the RED is the spawn: add `.arg("--modules").arg(concat!(env!("CARGO_MANIFEST_DIR"), "/../../crates/kernel/host/tests/fixtures"))` to the harness's `Command` at `daemon_e2e.rs:55-58`; today the binary rejects the flag ("unexpected arg") → the suite fails to boot.
- [ ] **Step 2: Flag + composition** — `main.rs:64-71`: `"--modules" => modules = Some(PathBuf::from(args.next().ok_or("--modules needs a dir")?))`; resolve `let modules_dir = match modules { Some(dir) => dir, None => workspace_config::modules_dir()? };` and refuse a missing dir before anything else binds. Replace `run_node`'s body at `main.rs:203-322` with the same shape as Task 2 Step 3 (`selection = topology::SIM_BASE.to_vec()`, `validators: &[]`, `invite: b""` — unused by a set with no governance, `chain_id: "local"`), then `let mut host = host_from(modules).expect("genesis");`. Keep `op_blobs` and everything after the genesis log untouched. Delete the native constructors and their `use` lines.
- [ ] **Step 3: GREEN** — `cargo test -p noded-bin` (daemon_e2e green with the composed set; the genesis log line still prints the root), `cargo clippy -p noded-bin --tests --no-deps`.
- [ ] **Step 4: Commit** — `feat(noded): the daemon composes its genesis from a modules dir through the topology composer`.

---

### Task 4: Dependency cleanup, docs, gates, PR

**Files:**
- Modify: `bin/noded/Cargo.toml`, `bin/simnode/Cargo.toml` — remove every per-module crate dep that no longer has a `use` in the crate (the 14–19 native tenants; keep any still used by tests or the echo oracle — the compiler's `unused_crate_dependencies` is not on, so grep each dep name in `src/` and `tests/` before removing).
- Modify: `docs/superpowers/specs/2026-08-27-live-upgrade-design.md` §3 — dated note (2026-08-28): the corrections table above (CodeSource not CodeBytes; `bundle.rs`; no parity lane — `daemon_e2e` pins `SIM_BASE`; only the sim golden moved; `chain_id = "local"`).
- Modify: `crates/topology/src/lib.rs:176-178`, `bin/simnode/tests/topology_set.rs:4-6`, `bin/simnode/src/lib.rs:370` — the three doc comments citing "the daemon parity lane" → "`bin/noded/tests/daemon_e2e.rs` pins the same `sim_base`".
- Modify: `skills/sim-lane/SKILL.md` (mention `SimOpts.modules_dir` / `--modules`, default = repo fixtures), `skills/qa/SKILL.md:172` (`cargo run -p noded` → `cargo run -p noded-bin`, `--modules`).

- [ ] **Step 1: Dep cleanup + `cargo check -p noded-bin -p simnode --all-targets`**; commit `chore(daemons): drop the per-module deps the composer made unused`.
- [ ] **Step 2: Docs** (Edit tool per hunk); commit `docs: spec §3 as built — one composer, three binaries`.
- [ ] **Step 3: Gates** — paste tails with exit codes (`${PIPESTATUS[0]}`):
  - `cargo clippy -p noded -p noded-bin -p simnode -p node-bin --tests --no-deps`
  - `cargo check --workspace --all-targets`
  - `make wasm-modules-check`
  - `cargo test -p noded`, `cargo test -p noded-bin`, `cargo test -p simnode`, `cargo test -p ducktape-app`
  - `cargo test -p node-bin --bin ducktape` (pin unchanged), `cargo test -p node-bin --test module_cli -- --test-threads=1` (bin/node's composer path still green end to end)
- [ ] **Step 4: Push, `gh pr create --base dev --title "feat(daemons): noded and simnode compose through the topology composer (sim golden moves; UpdateModule live in the sim)"`.** Body: spec §3 link + the corrections table; what moved into `crates/noded/src/bundle.rs`; the two daemons' new `--modules` flags and defaults; the sim golden `<old> → <new>` and why `GENESIS_ROOT_HASH` did not move; the app lane's wall time before/after; follow-ups (the app tests boot 14 components per sim — a shared compiled-component cache if it hurts; `directory`→`tasks` port PR next). Claude Code footer. Do NOT merge.

---

## Self-review

- **Spec coverage:** §3's three callers → bin/node (Part 1, plus Task 1's relocation), simnode (Task 2), noded (Task 3); "no `.with_*` calls survive" → the native constructors are deleted (Tasks 2–3); the golden note → Task 2's pin; the app's wasmtime cost → measured in Task 2 Step 5. §3's `CodeBytes` and "parity lane" are corrected, not implemented.
- **Placeholders:** the one `<keep the existing echo_oracle push here, unchanged>` marker points at code that exists today (`simnode/src/lib.rs` — grep `echo_oracle` in `run_sim`); the implementer moves it below `compose`. Everything else is concrete.
- **Type consistency:** `DirCodeSource::open(&Path, &[&str]) -> (DirCodeSource, BTreeMap<String,[u8;32]>)` consumed by Tasks 2–3 as `&code` (`&dyn host::CodeSource`) and `&code_hashes`; `qmdb_stores(&context)` returns the `FnMut` `compose` takes as `&mut StoreSource<'_>` (`&mut stores`); `host_from(Vec<Box<dyn Module>>) -> Result<Host, sdk::Error>`; `Bindings<'a>` fields are borrows (`&invite_binding`, `&valset_keys`) — `invite_binding: Vec<u8>` and `valset_keys: Vec<Vec<u8>>` are owned by `run_sim`, so the borrows live through the compose.
