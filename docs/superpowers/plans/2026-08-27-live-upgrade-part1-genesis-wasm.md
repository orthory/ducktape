# Live upgrade, part 1 — topology shape + genesis wasm out of the binary

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `ducktape` with zero embedded wasm: the genesis descriptor
carries every module's code hash (and fingerprints them), the bytes ride an
id-named bundle directory, and ONE composer builds the module set from the
topology for genesis, restore, and statesync.

**Architecture:** `crates/topology` gains `code`/`backing`/`committed_queries`
per module (PR 1, no root-hash movement). `crates/noded/src/compose.rs` turns a
topology selection + a `host::CodeSource` + a store source into modules; the
three `bin/node/src/host_state.rs` builders become thin callers. `hello`,
`directory`, `greeter` and the dead `DEMO` selection leave the topology; the
production set goes 21 → 19 (PR 2, the flag day: `GENESIS_ROOT_HASH` moves).

> **Amendment (2026-08-27, Ruling 10):** `directory` STAYS. It is the write
> tenant of all 8 process e2e suites and of `--dev-demo`
> (`bin/node/src/validator/engine.rs`, `bin/node/src/validator/run/drain.rs`),
> so the plan's "unused" premise was wrong. Porting that lane to another indexed
> tenant is a follow-up; `directory` leaves (and the root moves once more) with
> it. Everywhere below that says `directory` leaves or `PRODUCTION` is 19, read
> `directory` stays and 20 (21 → 20).

**Tech Stack:** Rust workspace; `wasm-host` (wasmtime), `sdk::MerkleStore` /
`statesync::qmdb::QmdbStore`, `blobstore::BlobHandle`, serde/toml descriptors,
clap CLI. Gates: `cargo clippy -p <crate> --tests --no-deps`,
`cargo check --workspace --all-targets`, `make wasm-modules-check`.

**Spec:** `docs/superpowers/specs/2026-08-27-live-upgrade-design.md` (§1, §2, §3
callers for `bin/node`; the noded/simnode callers, the CLI verbs, and the e2e are
parts 2–4, planned after this lands).

## Global Constraints

- No compat, no legacy arms, no version bumps: `ducktape:genesis:v1:` stays the
  fingerprint tag; an old `network.toml` without `modules` simply fails to load.
- No `include_bytes!`/`include_str!` of any `.wasm` remains in `bin/node`
  (production code OR tests). Tests read fixtures by path.
- One file naming convention everywhere: `<dir>/<id>.component.wasm`.
- `tracing`, never `println!`, for node events; `reason` fields are snake_case
  tokens. CLI stdout stays `println!`.
- House rules: named predicates, one `match` per discriminant, no boolean
  steering flags, tests wait on events not time.
- Edit files with the Edit tool per hunk — no sed/python edit scripts.
- Work in a worktree under `<checkout>/.worktree/<branch>` forked from
  `origin/dev`; PRs target `dev`.
- Every task that changes consensus-visible bytes (module set, seeded records)
  updates `GENESIS_ROOT_HASH` in the SAME commit and names the change.

---

## File map

| File | Responsibility after this plan |
|---|---|
| `crates/topology/src/lib.rs` | `Code`, `Backing`, `ModuleSpec { code, backing, committed_queries }`; PRODUCTION (19), SIM_BASE (14), SIM_VALSET (5). No DEMO. |
| `crates/workspace-config/src/lib.rs` | `ModuleCode`, `NetworkDescriptor.modules`, fingerprint over validators + modules. |
| `crates/workspace-config/src/node_toml.rs` | `DevSeedToml.modules: String` (a dir). |
| `bin/node/src/config/resolve.rs` | `GenesisModules { hashes, bundle_dir }` on `Resolved`; `load_valid_descriptor` refuses empty `modules`. |
| `crates/kernel/host/src/lib.rs` | `pub fn module_code_hash(&self, id)`; `lifecycle_module_status` made `pub`. |
| `crates/noded/src/compose.rs` (new) | `compose` / `compose_module`: the ONE module builder. |
| `bin/node/src/host_state.rs` | `BundleCodeSource`, `seed_bundle`, thin `genesis_host`/`restore_host`/`sync_all_modules`, `adopt_admitted_modules`, the parity + root pins. |
| `bin/node/src/cli.rs`, `cli_args.rs` | `node init --modules <dir>` writes `descriptor.modules` + the workspace bundle. |
| `bin/node/tests/common/mod.rs`, `bin/node/examples/node*.toml`, `ops/*.sh` | every dev-shape/init site names a modules dir. |
| `Makefile`, `ops/demo-seed.sh` | `install-node` copies components to `~/.ducktape/modules`; demo-seed passes `--modules`. |

---

## PR 1 — `feat/topology-module-shape`

### Task 1: `Code` / `Backing` on `ModuleSpec`

**Files:**
- Modify: `crates/topology/src/lib.rs:20-40` (struct), `:91-117` (table), tests at the bottom.

**Interfaces:**
- Produces: `topology::Code { Native, Wasm }`, `topology::Backing { Map, Store, Odb }`, `ModuleSpec { id, wiring, config, code, backing, committed_queries }`, `ModuleTopology::wasm_ids(&self, selection) -> Vec<&'static str>`.

- [ ] **Step 1: Write the failing tests** (append inside `mod tests`):

```rust
    /// The shape table is consensus-adjacent: a wrong `backing` composes the
    /// wrong root, a wrong `code` sends a native module to the wasm loader.
    #[test]
    fn shape_table_pins_native_odb_map_and_committed_queries() {
        let native: Vec<&str> = MODULES.iter().filter(|m| m.code == Code::Native).map(|m| m.id).collect();
        assert_eq!(sorted(&native), ["greeter", "kv", "lifecycle", "valset"]);
        let odb: Vec<&str> = MODULES.iter().filter(|m| m.backing == Backing::Odb).map(|m| m.id).collect();
        assert_eq!(sorted(&odb), ["files", "forge"]);
        let map: Vec<&str> = MODULES.iter().filter(|m| m.backing == Backing::Map).map(|m| m.id).collect();
        assert_eq!(sorted(&map), ["directory", "greeter", "hello", "runs"]);
        let committed: Vec<&str> = MODULES.iter().filter(|m| m.committed_queries).map(|m| m.id).collect();
        assert_eq!(committed, ["dispatch"]);
    }

    #[test]
    fn wasm_ids_selects_only_wasm_specs_in_selection_order() {
        let ids = TOPOLOGY.wasm_ids(SIM_VALSET);
        assert_eq!(ids, ["acl", "governance"]);
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p topology`
Expected: compile error — `Code`, `Backing`, `wasm_ids` do not exist.

- [ ] **Step 3: Implement**

Replace the `ModuleSpec` struct with:

```rust
/// Where a module's CODE comes from: compiled into the binary, or a wasm
/// component the code registry (lifecycle) can swap at a height boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Code {
    Native,
    Wasm,
}

/// Where a module's COMMITTED state lives — the substrate `root()` is computed
/// from. One per module by definition (`wasm_host::StateBacking` is an enum).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backing {
    /// host-KV map; root = sha256(canonical kv); rides the snapshot lane.
    Map,
    /// host-constructed authenticated store (qmdb); root = store merkle root.
    Store,
    /// host-side disk substrate (duckfs odb / git); root = sha256(refs image).
    Odb,
}

pub struct ModuleSpec {
    pub id: &'static str,
    pub wiring: &'static [&'static str],
    pub config: &'static [&'static str],
    pub code: Code,
    pub backing: Backing,
    /// the guest's query lane is COMMITTED-ONLY regardless of caller
    /// (`WasmModule::with_committed_queries`). dispatch only.
    pub committed_queries: bool,
}
```

Add to `impl ModuleTopology`:

```rust
    /// the `code == Wasm` ids of `selection`, in selection order.
    pub fn wasm_ids(&self, selection: &[&'static str]) -> Vec<&'static str> {
        selection
            .iter()
            .copied()
            .filter(|id| self.spec(id).is_some_and(|m| m.code == Code::Wasm))
            .collect()
    }
```

Rewrite the `MODULES` table (keep existing `wiring`/`config` values verbatim):

```rust
const fn store(id: &'static str, wiring: &'static [&'static str], config: &'static [&'static str]) -> ModuleSpec {
    ModuleSpec { id, wiring, config, code: Code::Wasm, backing: Backing::Store, committed_queries: false }
}

const MODULES: &[ModuleSpec] = &[
    store("acl", NONE, NONE),
    store("agent", &["saga", "runs"], NONE),
    store("automations", &["chat", "tasks", "inbox"], NONE),
    store("capability", NONE, NONE),
    store("chat", &["tagging"], NONE),
    ModuleSpec { id: "directory", wiring: NONE, config: NONE, code: Code::Wasm, backing: Backing::Map, committed_queries: false },
    ModuleSpec { id: "dispatch", wiring: &["saga"], config: NONE, code: Code::Wasm, backing: Backing::Store, committed_queries: true },
    ModuleSpec { id: "files", wiring: NONE, config: NONE, code: Code::Wasm, backing: Backing::Odb, committed_queries: false },
    ModuleSpec { id: "forge", wiring: &["chat"], config: NONE, code: Code::Wasm, backing: Backing::Odb, committed_queries: false },
    store("gateway", &["identity"], CHAIN_ID),
    store("governance", &["valset", "lifecycle", "identity"], INVITE),
    ModuleSpec { id: "greeter", wiring: NONE, config: NONE, code: Code::Native, backing: Backing::Map, committed_queries: false },
    ModuleSpec { id: "hello", wiring: NONE, config: NONE, code: Code::Wasm, backing: Backing::Map, committed_queries: false },
    store("identity", NONE, CHAIN_ID),
    store("inbox", NONE, NONE),
    ModuleSpec { id: "kv", wiring: NONE, config: NONE, code: Code::Native, backing: Backing::Store, committed_queries: false },
    ModuleSpec { id: "lifecycle", wiring: &["valset"], config: NONE, code: Code::Native, backing: Backing::Store, committed_queries: false },
    store("pages", &["tagging"], NONE),
    ModuleSpec { id: "runs", wiring: &["chat", "saga", "tagging", "dispatch", "agent", "tasks", "files", "pages"], config: NONE, code: Code::Wasm, backing: Backing::Map, committed_queries: false },
    store("saga", NONE, NONE),
    store("tagging", &["runs"], NONE),
    store("tasks", NONE, NONE),
    ModuleSpec { id: "valset", wiring: NONE, config: NONE, code: Code::Native, backing: Backing::Store, committed_queries: false },
];
```

(`directory` is `Wasm`/`Map` because production runs its wasm port; the native
`Directory` crate is a kernel-test fixture only.)

- [ ] **Step 4: Run the crate's tests**

Run: `cargo test -p topology`
Expected: PASS (existing selection pins + the two new tests).

- [ ] **Step 5: Commit**

```bash
git add crates/topology/src/lib.rs
git commit -m "feat(topology): a ModuleSpec says where its code and state come from"
```

### Task 2: the native/wasm parity pin in `host_state`

**Files:**
- Modify: `crates/kernel/host/src/lib.rs` (add one accessor near `module_root`, ~line 731)
- Modify: `bin/node/src/host_state.rs` tests module (~line 1497)

**Interfaces:**
- Produces: `Host::module_code_hash(&self, id: &str) -> Option<Vec<u8>>` — `None` for native or unregistered.

- [ ] **Step 1: Write the failing test** in `host_state.rs` `mod tests`, next to `genesis_registry_matches_module_ids`:

```rust
    /// the topology's `code` column is what the loader branches on; if it
    /// disagrees with what the composed host actually runs, a native module is
    /// sent to the wasm loader (or a wasm tenant is never reconciled).
    #[test]
    fn topology_code_column_matches_the_composed_host() {
        let native_by_topology: Vec<String> = topology::TOPOLOGY
            .modules
            .iter()
            .filter(|m| MODULE_IDS.contains(&m.id) && m.code == topology::Code::Native)
            .map(|m| m.id.to_string())
            .collect();
        let native_by_host = genesis_native_ids();
        let mut want = native_by_topology;
        want.sort_unstable();
        assert_eq!(native_by_host, want);
    }
```

Add beside `compose_genesis_facts` a sibling that composes once and returns the
native ids (same thread/stack shape as `genesis_facts`):

```rust
    fn genesis_native_ids() -> Vec<String> {
        std::thread::Builder::new()
            .name("production-genesis-native-ids".into())
            .stack_size(GENESIS_TEST_STACK_BYTES)
            .spawn(|| {
                let dir = tempfile::tempdir().expect("tempdir");
                let cfg = commonware_runtime::tokio::Config::default()
                    .with_storage_directory(dir.path().join("storage"));
                let executor = commonware_runtime::tokio::Runner::new(cfg);
                executor.start(|context| async move {
                    let host = genesis_host(
                        &context,
                        &dir.path().join("forge"),
                        &dir.path().join("duckfs"),
                        &[],
                        PIN_BINDINGS,
                        blobstore::BlobHandle::default(),
                    )
                    .await;
                    host.module_roots()
                        .into_iter()
                        .map(|(id, _)| id)
                        .filter(|id| host.module_code_hash(id).is_none())
                        .collect()
                })
            })
            .expect("spawn")
            .join()
            .unwrap_or_else(|payload| std::panic::resume_unwind(payload))
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p node-bin --bin ducktape topology_code_column`
Expected: compile error — `module_code_hash` missing on `Host`.

- [ ] **Step 3: Implement** in `crates/kernel/host/src/lib.rs`, after `module_root`:

```rust
    /// the sha256 of the component a registered module currently RUNS, or
    /// `None` for a native module (no swappable code) and for an unknown id.
    /// per-node realization state, never part of `root()`.
    pub fn module_code_hash(&self, id: &str) -> Option<Vec<u8>> {
        self.registry.get(id).and_then(|m| m.code_hash())
    }
```

- [ ] **Step 4: Run**

Run: `cargo test -p node-bin --bin ducktape topology_code_column`
Expected: PASS — `valset`, `lifecycle` on both sides.

- [ ] **Step 5: Gates + commit**

Run: `cargo clippy -p topology -p host --tests --no-deps && cargo clippy -p node-bin --tests --no-deps`
Expected: no new warnings.

```bash
git add crates/kernel/host/src/lib.rs bin/node/src/host_state.rs
git commit -m "test(node): pin the topology's code column to the composed host"
```

### Task 3: PR 1

- [ ] `cargo check --workspace --all-targets` green.
- [ ] Push branch, `gh pr create --base dev --title "feat(topology): ModuleSpec carries code/backing/committed_queries" --body "..."` (body: what the two columns mean, that no root-hash moves, the parity pin). End the body with the Claude Code footer.
- [ ] Merge only when green and the diff is understood (CLAUDE.md merge rule). Then sweep the worktree (`ops/worktree-clean.sh --yes`).

---

## PR 2 — `feat/genesis-wasm-out-of-binary`

### Task 4: `NetworkDescriptor.modules` + fingerprint

**Files:**
- Modify: `crates/workspace-config/src/lib.rs:92-197` and its tests (~1260)
- Modify (mechanical): every `NetworkDescriptor {` literal — 51 sites in `bin/node/src/cli.rs`, `bin/node/src/config/resolve.rs`, `crates/workspace-config/src/invite.rs`, `crates/workspace-config/src/lib.rs` — gains `modules: Vec::new(),` (or the test helper below where the test exercises resolve).

**Interfaces:**
- Produces: `ModuleCode { id: String, code_hash: String }`, `NetworkDescriptor.modules: Vec<ModuleCode>`, `NetworkDescriptor::module_hashes(&self) -> Result<BTreeMap<String, [u8; 32]>, String>`.

- [ ] **Step 1: Failing tests** (in `workspace-config` `mod tests`):

```rust
    fn modules_fixture() -> Vec<ModuleCode> {
        vec![
            ModuleCode { id: "pages".into(), code_hash: "11".repeat(32) },
            ModuleCode { id: "chat".into(), code_hash: "22".repeat(32) },
        ]
    }

    #[test]
    fn genesis_namespace_fingerprints_the_module_hashes() {
        let a = ed25519::PrivateKey::from_seed(5).public_key();
        let mut d = NetworkDescriptor {
            chain_id: "net#00000000".into(),
            validators: vec![hex_bytes(a.as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
            modules: modules_fixture(),
        };
        let base = d.genesis_namespace();
        // a different component for one module is a different network.
        d.modules[0].code_hash = "33".repeat(32);
        assert_ne!(d.genesis_namespace(), base);
        // order-independent: canonical over the id-sorted list.
        let mut reversed = d.clone();
        reversed.modules.reverse();
        assert_eq!(reversed.genesis_namespace(), d.genesis_namespace());
    }

    #[test]
    fn descriptor_without_modules_does_not_parse() {
        let text = "chain_id = \"net#00000000\"\nvalidators = []\n";
        let err = NetworkDescriptor::from_toml(text).unwrap_err();
        assert!(err.contains("modules"), "{err}");
    }

    #[test]
    fn module_hashes_decode_and_refuse_bad_lengths() {
        let mut d = NetworkDescriptor {
            chain_id: "n".into(), validators: vec![], bootstrap: vec![], reach: vec![],
            coordination: None, modules: modules_fixture(),
        };
        let map = d.module_hashes().unwrap();
        assert_eq!(map["pages"], [0x11u8; 32]);
        d.modules.push(ModuleCode { id: "x".into(), code_hash: "ab".into() });
        assert!(d.module_hashes().unwrap_err().contains("x"));
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p workspace-config genesis_namespace_fingerprints_the_module_hashes`
Expected: compile error — no `modules` field / `ModuleCode`.

- [ ] **Step 3: Implement.** Above `NetworkDescriptor`:

```rust
/// one genesis module: the consensus-visible id and the sha256 (hex) of the
/// component bytes every node seeds into the code registry at block zero.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ModuleCode {
    pub id: String,
    pub code_hash: String,
}
```

Add the field (REQUIRED — no `serde(default)`; an old descriptor fails to load):

```rust
    /// the genesis wasm set: `(id, sha256 hex)` per wasm tenant, sorted by id.
    /// IN the genesis fingerprint — a node built against different components
    /// is a different network, never a block-0 fork. the bytes travel
    /// out-of-band (the workspace bundle, the blob plane).
    pub modules: Vec<ModuleCode>,
```

In `from_toml`, after the validator canonicalization:

```rust
        for m in &mut d.modules {
            m.code_hash = m.code_hash.trim().to_ascii_lowercase();
        }
        d.modules.sort_by(|a, b| a.id.cmp(&b.id));
```

In `genesis_namespace`, after the validator loop and before `finalize`:

```rust
        let mut modules: Vec<&ModuleCode> = self.modules.iter().collect();
        modules.sort_by(|a, b| a.id.cmp(&b.id));
        for m in modules {
            hasher.update(b"\n");
            hasher.update(m.id.as_bytes());
            hasher.update(b"=");
            hasher.update(m.code_hash.trim().to_ascii_lowercase().as_bytes());
        }
```

Update the doc comment on `genesis_namespace` to say "sorted validator set +
sorted `id=code_hash` module lines". Add:

```rust
    /// the genesis code hashes, decoded: `id -> sha256`. a hash that is not 32
    /// bytes of hex names its module in the error.
    pub fn module_hashes(&self) -> Result<std::collections::BTreeMap<String, [u8; 32]>, String> {
        let mut out = std::collections::BTreeMap::new();
        for m in &self.modules {
            let bytes = unhex(&m.code_hash).map_err(|e| format!("module {} code_hash: {e}", m.id))?;
            let digest: [u8; 32] = bytes
                .try_into()
                .map_err(|_| format!("module {} code_hash is not 32 bytes", m.id))?;
            if out.insert(m.id.clone(), digest).is_some() {
                return Err(format!("duplicate module {} in network {}", m.id, self.chain_id));
            }
        }
        Ok(out)
    }
```

- [ ] **Step 4: Fix every struct literal.** Build and follow the errors:

Run: `cargo check --workspace --all-targets 2>&1 | grep -c "missing field \`modules\`"`

For each site add `modules: Vec::new(),` — EXCEPT tests in
`bin/node/src/config/resolve.rs` that call `resolve`/`resolve_service` on a
network-shape workspace; those use a helper added to that test module (Task 6
makes empty `modules` unrunnable):

```rust
    fn fake_modules() -> Vec<config::ModuleCode> {
        vec![config::ModuleCode { id: "pages".into(), code_hash: "11".repeat(32) }]
    }
```

- [ ] **Step 5: Run**

Run: `cargo test -p workspace-config && cargo check --workspace --all-targets`
Expected: PASS / green.

- [ ] **Step 6: Commit**

```bash
git add crates/workspace-config bin/node/src
git commit -m "feat(config): the genesis descriptor names every wasm module's code hash, and fingerprints them"
```

### Task 5: `DevSeedToml.modules` + `GenesisModules` on `Resolved`

**Files:**
- Modify: `crates/workspace-config/src/node_toml.rs:141-165` (`DevSeedToml`)
- Modify: `bin/node/src/config/resolve.rs` (`Resolved`, `load_valid_descriptor`, `resolve_network_shape`, `resolve_dev_shape`, tests)
- Modify: `bin/node/src/config/mod.rs` — re-export `ModuleCode` if `config::NetworkDescriptor` is re-exported there (mirror it).

**Interfaces:**
- Produces:
  ```rust
  pub struct GenesisModules {
      /// id -> sha256 of the genesis component, for every wasm tenant.
      pub hashes: std::collections::BTreeMap<String, [u8; 32]>,
      /// where `<id>.component.wasm` files live: `<workspace>/modules` (network
      /// shape) or the dev shape's `modules` dir.
      pub bundle_dir: PathBuf,
  }
  pub fn hash_bundle(dir: &Path, ids: &[&str]) -> Result<BTreeMap<String, [u8; 32]>, String>;
  pub fn component_path(dir: &Path, id: &str) -> PathBuf;   // <dir>/<id>.component.wasm
  ```
  `Resolved.genesis: GenesisModules`.

- [ ] **Step 1: Failing tests** in `resolve.rs` `mod tests`:

```rust
    #[test]
    fn dev_shape_hashes_its_modules_dir() {
        let dir = tempfile::tempdir().unwrap();
        let modules = dir.path().join("modules");
        std::fs::create_dir_all(&modules).unwrap();
        for id in topology::TOPOLOGY.wasm_ids(topology::PRODUCTION) {
            std::fs::write(component_path(&modules, id), id.as_bytes()).unwrap();
        }
        let cfg = dir.path().join("node.toml");
        std::fs::write(&cfg, format!(
            "id = 1\nnamespace = \"t\"\npeer_seeds = [1]\nlisten = \"127.0.0.1:0\"\nmodules = {:?}\n",
            modules.to_str().unwrap()
        )).unwrap();
        let r = resolve(&cfg).unwrap();
        assert_eq!(r.genesis.bundle_dir, modules);
        assert_eq!(r.genesis.hashes["pages"], sha2::Sha256::digest(b"pages").into());
    }

    #[test]
    fn dev_shape_names_a_missing_component() {
        let dir = tempfile::tempdir().unwrap();
        let modules = dir.path().join("modules");
        std::fs::create_dir_all(&modules).unwrap();
        let cfg = dir.path().join("node.toml");
        std::fs::write(&cfg, format!(
            "id = 1\nnamespace = \"t\"\npeer_seeds = [1]\nlisten = \"127.0.0.1:0\"\nmodules = {:?}\n",
            modules.to_str().unwrap()
        )).unwrap();
        let err = resolve(&cfg).unwrap_err();
        assert!(err.contains("acl.component.wasm"), "{err}");
    }

    #[test]
    fn network_shape_refuses_an_empty_module_list() {
        // build a founded workspace the way the other network-shape tests do,
        // but with `modules: vec![]`; resolve must name the problem.
        let (dir, cfg) = founded_workspace_with_modules(Vec::new());
        let err = resolve(&cfg).unwrap_err();
        assert!(err.contains("no modules"), "{err}");
        drop(dir);
    }
```

`founded_workspace_with_modules` is the existing network-shape test scaffold in
this module (the one that writes `network.toml` + `node.toml` + `identity.key`)
parameterized on `modules`; refactor the existing scaffold to take the list and
have the old call sites pass `fake_modules()`.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p node-bin --bin ducktape dev_shape_hashes_its_modules_dir`
Expected: compile error (`component_path`, `genesis`).

- [ ] **Step 3: Implement.**

`node_toml.rs`, in `DevSeedToml` after `validator_seeds`:

```rust
    /// the directory holding `<id>.component.wasm` for every wasm tenant — the
    /// dev shape has no descriptor, so its genesis code set is DERIVED from
    /// these files (every node of a dev cluster must point at identical bytes).
    pub modules: String,
```

`resolve.rs`:

```rust
pub struct GenesisModules {
    pub hashes: std::collections::BTreeMap<String, [u8; 32]>,
    pub bundle_dir: PathBuf,
}

/// `<dir>/<id>.component.wasm` — the one component file naming convention
/// (kernel fixtures, `~/.ducktape/modules`, and every workspace bundle).
pub fn component_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.component.wasm"))
}

/// sha256 every `<id>.component.wasm` in `dir` for `ids`; a missing file names
/// its path.
pub fn hash_bundle(dir: &Path, ids: &[&str]) -> Result<std::collections::BTreeMap<String, [u8; 32]>, String> {
    use sha2::Digest as _;
    let mut out = std::collections::BTreeMap::new();
    for id in ids {
        let path = component_path(dir, id);
        let bytes = std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        out.insert((*id).to_string(), sha2::Sha256::digest(&bytes).into());
    }
    Ok(out)
}
```

Add `pub genesis: GenesisModules,` to `Resolved` (doc: "the genesis wasm set
and where its bytes are").

`load_valid_descriptor`: after the validators check:

```rust
    if descriptor.modules.is_empty() {
        return Err(format!("network {} has no modules — re-found it with `node init --modules <dir>`", descriptor.chain_id));
    }
```

`resolve_network_shape`: build `genesis` before the `Ok(Resolved { .. })`:

```rust
    let genesis = GenesisModules {
        hashes: descriptor.module_hashes()?,
        bundle_dir: base.join("modules"),
    };
```
and add `genesis,` to the literal.

`resolve_dev_shape`: before moving fields out of `raw`:

```rust
    let bundle_dir = PathBuf::from(&raw.modules);
    let genesis = GenesisModules {
        hashes: hash_bundle(&bundle_dir, &topology::TOPOLOGY.wasm_ids(topology::PRODUCTION))?,
        bundle_dir,
    };
```
and add `genesis,` to the literal. (`node-bin` already depends on `topology` and `sha2`.)

- [ ] **Step 4: Run**

Run: `cargo test -p node-bin --bin ducktape config::`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/workspace-config/src/node_toml.rs bin/node/src/config
git commit -m "feat(node): resolve carries the genesis module hashes and the bundle dir for both shapes"
```

### Task 6: the composer — `crates/noded/src/compose.rs`

**Files:**
- Create: `crates/noded/src/compose.rs`
- Modify: `crates/noded/src/lib.rs` (`pub mod compose;`), `crates/noded/Cargo.toml` (add `wasm-host`, `topology`, `valset`, `lifecycle`, `kv`, `files`, `async-trait` if not present — mirror `bin/node/Cargo.toml` lines; `forge`, `sha2`, `host`, `sdk`, `blobstore` are already there).
- Test: `crates/noded/tests/compose.rs`

**Interfaces:**
- Consumes: `topology::{TOPOLOGY, ModuleSpec, Code, Backing}`, `host::CodeSource`, `wasm_host::WasmModule::{from_bytes, with_store, with_odb, with_committed_queries, install}`, `files::FilesOdbBacking::open(id, dir)`, `forge::ForgeOdbBacking::open(id, repo, blobs)`, `valset::Valset::{new, seed, finish_seed}`, `lifecycle::Lifecycle::{new, seed, finish_seed}`, `kv::Kv::new`, `sdk::{genesis_config, store_key, MerkleStore, StateRoot}`.
- Produces:

```rust
pub type BoxFut<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + 'a>>;
pub type StoreSource<'a> = dyn FnMut(&'static str) -> BoxFut<'a, Box<dyn sdk::MerkleStore>> + 'a;
pub type SnapshotSource<'a> = dyn FnMut(&'static str) -> BoxFut<'a, Result<Option<(Vec<u8>, StateRoot)>, String>> + 'a;

pub struct Substrates { pub forge_repo: PathBuf, pub duckfs_dir: PathBuf, pub blobs: BlobHandle }
pub struct Bindings<'a> {
    pub invite: &'a [u8],
    pub chain_id: &'a str,
    pub validators: &'a [Vec<u8>],
    pub code_hashes: &'a BTreeMap<String, [u8; 32]>,
}
pub enum Boot<'a> {
    /// fresh stores; native registries seeded from `Bindings`; Map tenants empty.
    Genesis,
    /// reopened/synced stores; `snapshots(id)` installs Map (and forge on the
    /// sync path) state; `None` means "nothing to install for this id".
    Reopen { snapshots: &'a mut SnapshotSource<'a> },
}

pub async fn compose(
    selection: &[&'static str], code: &dyn host::CodeSource, stores: &mut StoreSource<'_>,
    substrates: &Substrates, bindings: &Bindings<'_>, boot: Boot<'_>,
) -> Result<Vec<Box<dyn sdk::Module>>, String>;

pub async fn compose_module(
    spec: &ModuleSpec, code: &dyn host::CodeSource, stores: &mut StoreSource<'_>,
    substrates: &Substrates, bindings: &Bindings<'_>, boot: &mut Boot<'_>,
) -> Result<Box<dyn sdk::Module>, String>;
```

- [ ] **Step 1: Failing test** `crates/noded/tests/compose.rs` — compose `SIM_VALSET`'s wasm ids plus `runs` from the repo fixtures over in-memory stores, at genesis, and check ids/roots exist; then Reopen with a snapshot for `runs` round-trips its root:

```rust
use std::collections::BTreeMap;
use std::path::PathBuf;

use noded::compose::{compose, Bindings, Boot, Substrates};
use sdk::Module as _;

struct DirSource(PathBuf, BTreeMap<[u8; 32], &'static str>);

#[async_trait::async_trait(?Send)]
impl host::CodeSource for DirSource {
    async fn fetch(&self, code_hash: &[u8]) -> Option<Vec<u8>> {
        let digest: [u8; 32] = code_hash.try_into().ok()?;
        let id = self.1.get(&digest)?;
        std::fs::read(self.0.join(format!("{id}.component.wasm"))).ok()
    }
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../kernel/host/tests/fixtures")
}

fn hashes(ids: &[&'static str]) -> (BTreeMap<String, [u8; 32]>, BTreeMap<[u8; 32], &'static str>) {
    use sha2::Digest as _;
    let mut by_id = BTreeMap::new();
    let mut by_hash = BTreeMap::new();
    for id in ids {
        let bytes = std::fs::read(fixtures().join(format!("{id}.component.wasm"))).unwrap();
        let h: [u8; 32] = sha2::Sha256::digest(&bytes).into();
        by_id.insert(id.to_string(), h);
        by_hash.insert(h, *id);
    }
    (by_id, by_hash)
}

#[test]
fn composes_wasm_store_map_and_native_over_injected_stores() {
    use commonware_runtime::Runner as _;
    let dir = tempfile::tempdir().unwrap();
    let cfg = commonware_runtime::tokio::Config::default().with_storage_directory(dir.path().join("s"));
    commonware_runtime::tokio::Runner::new(cfg).start(|context| async move {
        let selection: &[&'static str] = &["kv", "valset", "acl", "governance", "lifecycle", "runs"];
        let (by_id, by_hash) = hashes(&["acl", "governance", "runs"]);
        let code = DirSource(fixtures(), by_hash);
        let validators = vec![vec![7u8; 32]];
        let bindings = Bindings { invite: b"t", chain_id: "t", validators: &validators, code_hashes: &by_id };
        let substrates = Substrates { forge_repo: dir.path().join("forge"), duckfs_dir: dir.path().join("duckfs"), blobs: blobstore::BlobHandle::default() };
        let mut stores = |id: &'static str| -> noded::compose::BoxFut<'_, Box<dyn sdk::MerkleStore>> {
            let context = context.child(id);
            Box::pin(async move { Box::new(statesync::qmdb::QmdbStore::init(context, id).await) as Box<dyn sdk::MerkleStore> })
        };
        let modules = compose(selection, &code, &mut stores, &substrates, &bindings, Boot::Genesis).await.unwrap();
        let ids: Vec<String> = modules.iter().map(|m| m.id().to_string()).collect();
        assert_eq!(ids, selection.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        let host = host::Host::genesis(modules).unwrap();
        assert!(host.module_code_hash("valset").is_none());
        assert_eq!(host.module_code_hash("acl").unwrap(), by_id["acl"].to_vec());
    });
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p noded --test compose`
Expected: compile error — `noded::compose` missing.

- [ ] **Step 3: Implement** `crates/noded/src/compose.rs`:

```rust
//! the ONE module composer: a topology selection + a code source + a store
//! source → the module set. genesis, restore, and statesync in `bin/node`, and
//! the noded/simnode daemons, all build their hosts here — a module's SHAPE
//! (native vs wasm, map/store/odb, committed queries, genesis config) is read
//! from `topology`, never hand-written per composer.

use std::collections::BTreeMap;
use std::path::PathBuf;

use sdk::{MerkleStore, Module, StateRoot};
use topology::{Backing, Code, ModuleSpec, TOPOLOGY};
use wasm_host::WasmModule;

pub type BoxFut<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + 'a>>;
pub type StoreSource<'a> = dyn FnMut(&'static str) -> BoxFut<'a, Box<dyn MerkleStore>> + 'a;
pub type SnapshotSource<'a> =
    dyn FnMut(&'static str) -> BoxFut<'a, Result<Option<(Vec<u8>, StateRoot)>, String>> + 'a;

pub struct Substrates {
    pub forge_repo: PathBuf,
    pub duckfs_dir: PathBuf,
    pub blobs: blobstore::BlobHandle,
}

pub struct Bindings<'a> {
    pub invite: &'a [u8],
    pub chain_id: &'a str,
    pub validators: &'a [Vec<u8>],
    pub code_hashes: &'a BTreeMap<String, [u8; 32]>,
}

pub enum Boot<'a> {
    Genesis,
    Reopen { snapshots: &'a mut SnapshotSource<'a> },
}

pub async fn compose(
    selection: &[&'static str],
    code: &dyn host::CodeSource,
    stores: &mut StoreSource<'_>,
    substrates: &Substrates,
    bindings: &Bindings<'_>,
    mut boot: Boot<'_>,
) -> Result<Vec<Box<dyn Module>>, String> {
    let mut out = Vec::with_capacity(selection.len());
    for id in selection {
        let spec = TOPOLOGY
            .spec(id)
            .ok_or_else(|| format!("module {id} is not in the topology"))?;
        out.push(compose_module(spec, code, stores, substrates, bindings, &mut boot).await?);
    }
    Ok(out)
}

pub async fn compose_module(
    spec: &ModuleSpec,
    code: &dyn host::CodeSource,
    stores: &mut StoreSource<'_>,
    substrates: &Substrates,
    bindings: &Bindings<'_>,
    boot: &mut Boot<'_>,
) -> Result<Box<dyn Module>, String> {
    match spec.code {
        Code::Native => native(spec, stores, bindings, boot).await,
        Code::Wasm => wasm(spec, code, stores, substrates, bindings, boot).await,
    }
}

async fn native(
    spec: &ModuleSpec,
    stores: &mut StoreSource<'_>,
    bindings: &Bindings<'_>,
    boot: &mut Boot<'_>,
) -> Result<Box<dyn Module>, String> {
    let is_genesis = matches!(boot, Boot::Genesis);
    let store = stores(spec.id).await;
    match spec.id {
        "valset" => {
            let mut valset = valset::Valset::new(spec.id, store);
            if is_genesis {
                for v in bindings.validators {
                    valset.seed(v.clone()).await.map_err(|e| format!("valset seed: {e}"))?;
                }
                valset.finish_seed().await.map_err(|e| format!("valset seed: {e}"))?;
            }
            Ok(Box::new(valset))
        }
        "lifecycle" => {
            let mut reg = lifecycle::Lifecycle::new(spec.id, store, "valset");
            if is_genesis {
                for (id, hash) in bindings.code_hashes {
                    reg.seed(id, hash.to_vec()).await.map_err(|e| format!("lifecycle seed {id}: {e}"))?;
                }
                reg.finish_seed().await.map_err(|e| format!("lifecycle seed: {e}"))?;
            }
            Ok(Box::new(reg))
        }
        "kv" => Ok(Box::new(kv::Kv::new(spec.id, store))),
        other => Err(format!("native module {other} has no constructor in the composer")),
    }
}

async fn wasm(
    spec: &ModuleSpec,
    code: &dyn host::CodeSource,
    stores: &mut StoreSource<'_>,
    substrates: &Substrates,
    bindings: &Bindings<'_>,
    boot: &mut Boot<'_>,
) -> Result<Box<dyn Module>, String> {
    let hash = bindings
        .code_hashes
        .get(spec.id)
        .ok_or_else(|| format!("module {} has no genesis code hash", spec.id))?;
    let bytes = code
        .fetch(hash)
        .await
        .ok_or_else(|| format!("code bytes absent for module {} (hash {}) — fail-closed", spec.id, hex(hash)))?;
    let mut module = match spec.backing {
        Backing::Map => WasmModule::from_bytes(spec.id, &bytes),
        Backing::Store => {
            let mut store = stores(spec.id).await;
            if matches!(boot, Boot::Genesis) {
                seed_store_config(&mut *store, spec, bindings).await?;
            }
            WasmModule::with_store(spec.id, &bytes, store)
        }
        Backing::Odb => match spec.id {
            "files" => {
                let backing = files::FilesOdbBacking::open(spec.id, substrates.duckfs_dir.clone())
                    .map_err(|e| format!("files open: {e}"))?;
                WasmModule::with_odb(spec.id, &bytes, Box::new(backing))
            }
            "forge" => {
                let backing = forge::ForgeOdbBacking::open(spec.id, substrates.forge_repo.clone(), substrates.blobs.clone())
                    .map_err(|e| format!("forge open: {e}"))?;
                WasmModule::with_odb(spec.id, &bytes, Box::new(backing))
            }
            other => return Err(format!("odb module {other} has no backing in the composer")),
        },
    }
    .map_err(|e| format!("{} component loads: {e}", spec.id))?;
    if spec.committed_queries {
        module = module.with_committed_queries();
    }
    if let Boot::Reopen { snapshots } = boot {
        if let Some((snapshot, root)) = snapshots(spec.id).await? {
            module
                .install(&snapshot, root)
                .map_err(|e| format!("{} install: {e}", spec.id))?;
        }
    }
    Ok(Box::new(module))
}

/// commit a STORE-BACKED tenant's genesis `__config` record from the topology's
/// config keys; idempotent (a store already carrying one is left untouched).
async fn seed_store_config(
    store: &mut dyn MerkleStore,
    spec: &ModuleSpec,
    bindings: &Bindings<'_>,
) -> Result<(), String> {
    if spec.config.is_empty() {
        return Ok(());
    }
    let key = sdk::store_key(sdk::genesis_config::CONFIG_KEY);
    let already = store.get(&key).await.map_err(|e| format!("{} genesis config read: {e}", spec.id))?;
    if already.is_some() {
        return Ok(());
    }
    let params: Vec<(&str, &[u8])> = spec
        .config
        .iter()
        .map(|k| match *k {
            topology::CONFIG_INVITE => (*k, bindings.invite),
            topology::CONFIG_CHAIN_ID => (*k, bindings.chain_id.as_bytes()),
            other => panic!("topology config key {other} has no binding"),
        })
        .collect();
    let config = sdk::genesis_config::encode_config(&params);
    store
        .commit_batch(vec![(key, Some(config))])
        .await
        .map_err(|e| format!("{} genesis config seeds: {e}", spec.id))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
```

Notes for the implementer: `match *k` on `&'static str` consts needs the consts
to be `const`, which they are (`CONFIG_INVITE`, `CONFIG_CHAIN_ID`). `Lifecycle::seed`
takes `(&str, Vec<u8>)` — check `lifecycle/src/lib.rs:117` and adapt the call.
`WasmModule::install` signature: `(&mut self, bytes: &[u8], expected: StateRoot)`.

- [ ] **Step 4: Run**

Run: `cargo test -p noded --test compose`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/noded/src/compose.rs crates/noded/src/lib.rs crates/noded/Cargo.toml crates/noded/tests/compose.rs Cargo.lock
git commit -m "feat(noded): one composer builds a module set from the topology, a code source, and a store source"
```

### Task 7: `host_state.rs` on the composer; delete the embedded bytes

**Files:**
- Modify: `bin/node/src/host_state.rs` (most of lines 39–981 and 1029–1426 are replaced; `FilesOdb` adapter at 983–1027 and the tests stay)
- Modify: `crates/kernel/host/src/lib.rs:676` — `async fn lifecycle_module_status` → `pub async fn`.
- Modify: `bin/node/src/validator/boot.rs`, `bin/node/src/replica/park.rs:538,1961,2267`, `bin/node/src/boot/sync_only.rs:174` — call sites gain `&resolved.genesis` (thread it the way `blobs` is threaded: every fn between `resolve()` and these sites that takes `blobs` takes `genesis: &config::GenesisModules` next to it).

**Interfaces:**
- Produces:
  ```rust
  pub(super) struct NetworkBindings<'a> { invite: &'a [u8], identity_chain_id: &'a str }  // unchanged
  pub(super) struct BundleCodeSource { dir: PathBuf, ids_by_hash: BTreeMap<[u8;32], String> }
  pub(super) fn seed_bundle(blobs: &BlobHandle, genesis: &GenesisModules) -> Result<(), String>;
  pub(super) async fn genesis_host(context, forge_repo, duckfs_dir, validators, bindings, blobs, genesis: &GenesisModules) -> Host;
  pub(super) async fn restore_host(context, forge_repo, duckfs_dir, manifest, blobs, genesis: &GenesisModules) -> Result<Host, String>;
  pub(super) async fn sync_all_modules<C>(context, client, manifest, substrates, attempt, genesis: &GenesisModules) -> Result<Host, String>;
  ```

- [ ] **Step 1: Failing test** — a bundle-seeding round trip, in `host_state` `mod tests`:

```rust
    /// the bundle is the founder's ONLY source of genesis bytes: every file must
    /// hash to the descriptor's entry and land in the blob store, and a
    /// mismatch names its module instead of seeding.
    #[test]
    fn seed_bundle_verifies_every_component_against_the_descriptor() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("pages.component.wasm"), b"pages-bytes").unwrap();
        let mut hashes = std::collections::BTreeMap::new();
        hashes.insert("pages".to_string(), sha2::Sha256::digest(b"pages-bytes").into());
        let genesis = crate::config::GenesisModules { hashes: hashes.clone(), bundle_dir: dir.path().to_path_buf() };
        let blobs = blobstore::BlobHandle::default();
        seed_bundle(&blobs, &genesis).unwrap();
        assert!(blobs.has_chunk(&hashes["pages"]));

        std::fs::write(dir.path().join("pages.component.wasm"), b"tampered").unwrap();
        let err = seed_bundle(&blobstore::BlobHandle::default(), &genesis).unwrap_err();
        assert!(err.contains("pages"), "{err}");
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p node-bin --bin ducktape seed_bundle_verifies`
Expected: compile error — `seed_bundle` missing.

- [ ] **Step 3: Rewrite `host_state.rs`.** Delete lines 39–234 (all `*_WASM_COMPONENT`
/ `*_MODULE_ID` consts and their docs) EXCEPT `BlobCodeSource` (51–63) and
`WasmModuleFactory` (65–75); delete `seeded_lifecycle`, `seed_genesis_components`,
every `genesis_*_wasm` / `*_wasm(...)` constructor (236–554), `seed_store_config`
(556–579), `ProductionModules` and its `impl` (581–643). Keep `NetworkBindings`,
`SyncSubstrates`, `FilesOdb`. Add:

```rust
use crate::config::{GenesisModules, component_path};
use noded::compose::{Bindings, Boot, Boot::Genesis, BoxFut, Substrates, compose, compose_module};

/// read every genesis component out of the bundle dir, verify it against the
/// descriptor's hash, and put it in the (persistent) blob store — the founder's
/// and a reopened workspace's source of genesis bytes. idempotent.
pub(super) fn seed_bundle(blobs: &blobstore::BlobHandle, genesis: &GenesisModules) -> Result<(), String> {
    for (id, want) in &genesis.hashes {
        if blobs.has_chunk(want) {
            continue;
        }
        let path = component_path(&genesis.bundle_dir, id);
        let bytes = std::fs::read(&path).map_err(|e| format!("module {id}: read {}: {e}", path.display()))?;
        let got: [u8; 32] = sha2::Sha256::digest(&bytes).into();
        if got != *want {
            return Err(format!("module {id}: {} hashes to {} but the descriptor says {} — fail-closed", path.display(), hex32(&got), hex32(want)));
        }
        blobs.put_chunk(bytes);
    }
    Ok(())
}

fn hex32(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// the module set every production host composes: the topology's production
/// selection plus every module the lifecycle registry ADMITTED post-genesis
/// (a non-empty active hash for an id outside the selection).
fn substrates(forge_repo: &std::path::Path, duckfs_dir: &std::path::Path, blobs: blobstore::BlobHandle) -> Substrates {
    Substrates { forge_repo: forge_repo.to_path_buf(), duckfs_dir: duckfs_dir.to_path_buf(), blobs }
}

fn bindings<'a>(net: &NetworkBindings<'a>, validators: &'a [Vec<u8>], genesis: &'a GenesisModules) -> Bindings<'a> {
    Bindings { invite: net.invite, chain_id: net.identity_chain_id, validators, code_hashes: &genesis.hashes }
}

fn finish(modules: Vec<Box<dyn sdk::Module>>) -> Result<Host, sdk::Error> {
    let mut host = Host::genesis(modules)?;
    host.set_module_factory(Box::new(WasmModuleFactory));
    Ok(host)
}
```

`genesis_host`:

```rust
pub(super) async fn genesis_host(
    context: &commonware_runtime::tokio::Context,
    forge_repo: &std::path::Path,
    duckfs_dir: &std::path::Path,
    genesis_validators: &[ed25519::PublicKey],
    net: NetworkBindings<'_>,
    blobs: blobstore::BlobHandle,
    genesis: &GenesisModules,
) -> Host {
    seed_bundle(&blobs, genesis).expect("genesis bundle");
    let validators: Vec<Vec<u8>> = genesis_validators.iter().map(|k| k.as_ref().to_vec()).collect();
    let code = BlobCodeSource(std::sync::Arc::new(blobs.clone()));
    let mut stores = |id: &'static str| -> BoxFut<'_, Box<dyn sdk::MerkleStore>> {
        let child = context.child(id);
        Box::pin(async move { Box::new(QmdbStore::init(child, id).await) as Box<dyn sdk::MerkleStore> })
    };
    let modules = compose(
        topology::PRODUCTION,
        &code,
        &mut stores,
        &substrates(forge_repo, duckfs_dir, blobs),
        &bindings(&net, &validators, genesis),
        Genesis,
    )
    .await
    .expect("genesis compose");
    finish(modules).expect("genesis host")
}
```

`restore_host` (the manifest supplies Map snapshots only; Odb/Store reopen
themselves; the network bindings are already committed store records, so
`invite`/`chain_id` are unused on this path — pass empties):

```rust
pub(super) async fn restore_host(
    context: &commonware_runtime::tokio::Context,
    forge_repo: &std::path::Path,
    duckfs_dir: &std::path::Path,
    manifest: &Manifest,
    blobs: blobstore::BlobHandle,
    genesis: &GenesisModules,
) -> Result<Host, String> {
    seed_bundle(&blobs, genesis)?;
    let code = BlobCodeSource(std::sync::Arc::new(blobs.clone()));
    let mut stores = |id: &'static str| -> BoxFut<'_, Box<dyn sdk::MerkleStore>> {
        let child = context.child(id);
        Box::pin(async move { Box::new(QmdbStore::init(child, id).await) as Box<dyn sdk::MerkleStore> })
    };
    let mut snapshots = |id: &'static str| -> BoxFut<'_, Result<Option<(Vec<u8>, StateRoot)>, String>> {
        let is_map = topology::TOPOLOGY.spec(id).is_some_and(|s| s.backing == topology::Backing::Map);
        let got = is_map.then(|| manifest_snapshot(manifest, id)).transpose();
        Box::pin(async move { got })
    };
    let net = NetworkBindings { invite: &[], identity_chain_id: "" };
    let modules = compose(
        topology::PRODUCTION,
        &code,
        &mut stores,
        &substrates(forge_repo, duckfs_dir, blobs),
        &bindings(&net, &[], genesis),
        Boot::Reopen { snapshots: &mut snapshots },
    )
    .await?;
    let mut host = finish(modules).map_err(|e| format!("restore host: {e}"))?;
    adopt_admitted_modules(&mut host, &code, &mut |id| manifest_snapshot(manifest, id).map(Some)).await?;
    Ok(host)
}

fn manifest_snapshot(manifest: &Manifest, id: &str) -> Result<(Vec<u8>, StateRoot), String> {
    let bytes = manifest.snapshot(id).ok_or_else(|| format!("checkpoint has no snapshot for module {id}"))?;
    let root = manifest.root(id).ok_or_else(|| format!("checkpoint has no root for module {id}"))?;
    Ok((bytes.to_vec(), root))
}

/// register every module the lifecycle registry admitted post-genesis: an id
/// with a non-empty ACTIVE hash that the topology selection did not compose.
/// Map-backed by construction (admission instantiates `from_bytes`), so its
/// state is the manifest's snapshot for that id.
async fn adopt_admitted_modules(
    host: &mut Host,
    code: &dyn host::CodeSource,
    snapshot: &mut dyn FnMut(&str) -> Result<Option<(Vec<u8>, StateRoot)>, String>,
) -> Result<(), String> {
    let Some(registry) = host.lifecycle_module_status().await else {
        return Ok(());
    };
    for m in registry {
        let already_composed = host.module_root(&m.module_id).is_some();
        let not_yet_admitted = m.active_code_hash.is_empty();
        if already_composed || not_yet_admitted {
            continue;
        }
        let bytes = code
            .fetch(&m.active_code_hash)
            .await
            .ok_or_else(|| format!("code bytes absent for admitted module {} — fail-closed", m.module_id))?;
        let mut module = WasmModule::from_bytes(m.module_id.as_str(), &bytes)
            .map_err(|e| format!("admitted module {} loads: {e}", m.module_id))?;
        if let Some((snap, root)) = snapshot(&m.module_id)? {
            module.install(&snap, root).map_err(|e| format!("admitted module {} install: {e}", m.module_id))?;
        }
        host.register(Box::new(module));
    }
    Ok(())
}
```

`sync_all_modules`: keep the manifest/`fetch_target`/`snapshot_of`/scratch
plumbing and the files-possession block VERBATIM (lines 1050–1088, 1216–1229,
1315–1353), then replace the per-module blocks (1093–1215, 1231–1313, 1355–1365,
1371–1395) with:

```rust
    let code = crate::blob_fetch::FetchingCodeSource::new(
        blobs.clone(),
        client.clone(),
        crate::constants::MAX_MODULE_CODE_BYTES,
        crate::constants::BLOB_FETCH_ATTEMPTS,
    );
    let mut stores = |module: &'static str| -> BoxFut<'_, Box<dyn sdk::MerkleStore>> {
        let child = scratch_context.child(child_label(module));
        let target = fetch_target(module);
        Box::pin(async move {
            let (target, resolver) = target.await.expect("pinned target");
            Box::new(QmdbStore::sync_from(child, module, target, resolver).await.expect("sync_from"))
                as Box<dyn sdk::MerkleStore>
        })
    };
    // snapshot lane: Map tenants AND forge (its refs image rides the snapshot
    // lane; files is possession-synced separately below).
    let mut snapshots = |module: &'static str| -> BoxFut<'_, Result<Option<(Vec<u8>, StateRoot)>, String>> {
        let spec = topology::TOPOLOGY.spec(module);
        let on_snapshot_lane = module == "forge" || spec.is_some_and(|s| s.backing == topology::Backing::Map);
        let fut = snapshot_of(module);
        Box::pin(async move {
            if !on_snapshot_lane {
                return Ok(None);
            }
            fut.await.map(Some)
        })
    };
    let net = NetworkBindings { invite: &[], identity_chain_id: "" };
    let modules = compose(
        topology::PRODUCTION,
        &code,
        &mut stores,
        &substrates(forge_repo, files_scratch.dir(), blobs.clone()),
        &bindings(&net, &[], genesis),
        Boot::Reopen { snapshots: &mut snapshots },
    )
    .await?;
    let mut host = finish(modules).map_err(|e| format!("compose synced host: {e}"))?;
    let mut fetch_admitted = |id: &str| -> Result<Option<(Vec<u8>, StateRoot)>, String> {
        // admitted ids are not `&'static`; fetch through the same lane by value.
        let id: &'static str = Box::leak(id.to_string().into_boxed_str());
        futures::executor::block_on(snapshot_of(id)).map(Some)
    };
    adopt_admitted_modules(&mut host, &code, &mut fetch_admitted).await?;
```

(`stores` cannot return `Result` — the `StoreSource` type is infallible by
design so genesis stays simple; the `expect`s above are on the SAME failures
the old code propagated with `?`. If the reviewer prefers, change `StoreSource`
to return `Result<Box<dyn MerkleStore>, String>` in Task 6 and drop the
`expect`s — do it in Task 6, not here.)

The files possession block moves BEFORE `compose` (it needs `files_scratch`
first), and the post-gate canonical reopen becomes:

```rust
    host.register(
        compose_module(
            topology::TOPOLOGY.spec("files").expect("files is in the topology"),
            &code,
            &mut stores,
            &substrates(forge_repo, duckfs_dir, blobs.clone()),
            &bindings(&net, &[], genesis),
            &mut Boot::Genesis,
        )
        .await
        .map_err(|e| format!("duckfs reopen: {e}"))?,
    );
```

The two root-hash gates (`host.root_hash() != manifest.root_hash`) stay exactly
where they are, around the promotion.

Update the three call sites to pass `genesis`; thread `&config::GenesisModules`
from `Resolved` to each (follow `blobs`).

Update the tests: `compose_genesis_facts` and `genesis_native_ids` pass a
`GenesisModules` built from the fixtures:

```rust
    fn fixtures_dir() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crates/kernel/host/tests/fixtures")
    }
    fn fixture_genesis() -> crate::config::GenesisModules {
        let dir = fixtures_dir();
        let hashes = crate::config::hash_bundle(&dir, &topology::TOPOLOGY.wasm_ids(topology::PRODUCTION)).expect("fixtures");
        crate::config::GenesisModules { hashes, bundle_dir: dir }
    }
```

- [ ] **Step 4: Build + run the pins**

Run: `cargo test -p node-bin --bin ducktape host_state`
Expected: `seed_bundle_verifies…` PASS, `genesis_registry_matches_module_ids` PASS,
`topology_code_column…` PASS, `production_genesis_root_hash_is_pinned` PASS
(the set is still 21 and the seeded hashes are the same bytes — the root must
NOT move in this task; if it does, something is composed differently: compare
`host.module_roots()` against `origin/dev` before continuing).

- [ ] **Step 5: Confirm no wasm is embedded**

Run: `grep -rn "include_bytes\|include_str" bin/node/src | grep -i wasm`
Expected: no output.

- [ ] **Step 6: Commit**

```bash
git add bin/node/src crates/kernel/host/src/lib.rs
git commit -m "feat(node): compose every host from the topology and the descriptor's code hashes; no embedded wasm"
```

### Task 8: `ducktape node init --modules <dir>` writes hashes + bundle

**Files:**
- Modify: `bin/node/src/cli_args.rs:491-500` (`InitArgs`), `bin/node/src/cli.rs:429-530` (`cmd_init`)
- Modify: `crates/workspace-config/src/lib.rs` — add `pub fn modules_dir() -> Result<PathBuf, String>` next to `executor_dir` (`$DUCKTAPE_MODULES_DIR`, else `<ducktape_home>/modules`).
- Test: `bin/node/tests/workspace_registry_cli.rs` (existing CLI-driven suite) — add one test.

- [ ] **Step 1: Failing test** (append to `bin/node/tests/workspace_registry_cli.rs`, using its existing `Command::new(env!("CARGO_BIN_EXE_ducktape"))` pattern):

```rust
#[test]
fn init_writes_module_hashes_and_the_bundle() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = tmp.path().join("ws");
    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../crates/kernel/host/tests/fixtures");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_ducktape"))
        .args(["node", "init", "--name", "bundled", "--primary-coordinator", "none", "--dir"])
        .arg(&ws)
        .args(["--listen", "127.0.0.1:0", "--advertised", "127.0.0.1:1", "--modules"])
        .arg(&fixtures)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let d = workspace_config::NetworkDescriptor::load(&ws.join("network.toml")).unwrap();
    let ids: Vec<&str> = d.modules.iter().map(|m| m.id.as_str()).collect();
    let mut want = topology::TOPOLOGY.wasm_ids(topology::PRODUCTION);
    want.sort_unstable();
    assert_eq!(ids, want);
    for m in &d.modules {
        let bytes = std::fs::read(ws.join("modules").join(format!("{}.component.wasm", m.id))).unwrap();
        assert_eq!(workspace_config::hex_bytes(&sha2::Sha256::digest(&bytes)), m.code_hash);
    }
}

#[test]
fn init_names_the_missing_component() {
    let tmp = tempfile::tempdir().unwrap();
    let empty = tmp.path().join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_ducktape"))
        .args(["node", "init", "--name", "x", "--primary-coordinator", "none", "--dir"])
        .arg(tmp.path().join("ws"))
        .args(["--listen", "127.0.0.1:0", "--advertised", "127.0.0.1:1", "--modules"])
        .arg(&empty)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("acl.component.wasm"));
}
```

(Add `topology`, `workspace-config`, `sha2` to `node-bin`'s dev-deps if not present.)

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p node-bin --test workspace_registry_cli init_writes_module_hashes`
Expected: FAIL — `--modules` unknown argument.

- [ ] **Step 3: Implement.** `InitArgs`:

```rust
    /// directory of `<id>.component.wasm` files to found the network's genesis
    /// wasm set from (default: $DUCKTAPE_MODULES_DIR, else ~/.ducktape/modules)
    #[arg(long, value_name = "DIR")]
    pub modules: Option<PathBuf>,
```

`workspace-config`:

```rust
/// where `make install-node` / `ops/dev.sh` put the module components a founder
/// seeds its network from: `$DUCKTAPE_MODULES_DIR`, else `<ducktape_home>/modules`.
pub fn modules_dir() -> Result<PathBuf, String> {
    if let Some(dir) = std::env::var_os("DUCKTAPE_MODULES_DIR") {
        return Ok(PathBuf::from(dir));
    }
    Ok(ducktape_home()?.join("modules"))
}
```

`cmd_init`, right before `let mut descriptor = config::NetworkDescriptor {`:

```rust
    let modules_src = match args.modules {
        Some(dir) => dir,
        None => config::modules_dir()?,
    };
    let wasm_ids = topology::TOPOLOGY.wasm_ids(topology::PRODUCTION);
    let hashes = config::hash_bundle(&modules_src, &wasm_ids)
        .map_err(|e| format!("{e} — pass --modules <dir> holding every <id>.component.wasm"))?;
    let bundle = dir.join("modules");
    std::fs::create_dir_all(&bundle)?;
    for id in &wasm_ids {
        std::fs::copy(config::component_path(&modules_src, id), config::component_path(&bundle, id))?;
    }
    let modules: Vec<config::ModuleCode> = hashes
        .iter()
        .map(|(id, h)| config::ModuleCode { id: id.clone(), code_hash: hex_bytes(h) })
        .collect();
```

and `modules,` in the descriptor literal. (`hash_bundle`/`component_path` live in
`config::resolve` — re-export them from `config/mod.rs`.) Print one more line
after "network … initialized": `eprintln!("modules: {} components bundled from {}", modules.len(), modules_src.display());`.

- [ ] **Step 4: Run**

Run: `cargo test -p node-bin --test workspace_registry_cli`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add bin/node/src/cli.rs bin/node/src/cli_args.rs bin/node/src/config/mod.rs crates/workspace-config/src/lib.rs bin/node/tests/workspace_registry_cli.rs bin/node/Cargo.toml
git commit -m "feat(cli): node init bundles the genesis components and writes their hashes into the descriptor"
```

### Task 9: every founder / dev-shape site names a modules dir

**Files:**
- Modify: `bin/node/tests/common/mod.rs:268-312` (`init_founder`: add `"--modules", FIXTURES`), `:796-830` (`config_path`: add `modules = "<FIXTURES>"`), and the same in `NetworkShapeCluster`'s friend/joiner config writers if they emit a dev-shape toml.
- Modify: `bin/node/examples/node0.toml … node3.toml` — add `modules = "crates/kernel/host/tests/fixtures"` (relative to the repo root the examples are run from).
- Modify: `ops/huddle-lane.sh`, `ops/wg-smoke/run-smoke.sh` — add the `modules = ` line where they write dev-shape tomls (`grep -n "peer_seeds" ops/`).
- Modify: `ops/demo-seed.sh:69` — add `--modules "$REPO_ROOT/crates/kernel/host/tests/fixtures"` to `node init` (derive `REPO_ROOT` from `SCRIPT_DIR/..` as the script already does for its own helpers).
- Modify: `Makefile` `install-node`:

```make
install-node:
	$(CARGO) install --path bin/node --locked
	mkdir -p "$${DUCKTAPE_MODULES_DIR:-$$HOME/.ducktape/modules}"
	@for m in $(BUILDER_MODULES); do \
	  id=$$(basename $$m) && \
	  cp $$m/component.wasm "$${DUCKTAPE_MODULES_DIR:-$$HOME/.ducktape/modules}/$$id.component.wasm" || exit 1; \
	done
	@echo "installed module components into $${DUCKTAPE_MODULES_DIR:-$$HOME/.ducktape/modules}"
```

In `common/mod.rs` add once:

```rust
pub const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../crates/kernel/host/tests/fixtures");
```

- [ ] **Step 1: Run the fast cluster suites**

Run: `TMPDIR=$(pwd)/.worktree-tmp cargo test -p node-bin --test cluster_e2e -- --ignored --test-threads=1 --nocapture`
Expected: PASS (dev shape resolves the fixtures dir; nodes seed the bundle).

Run: `cargo test -p node-bin --test invite_e2e -- --ignored --test-threads=1`
Expected: PASS (network shape: `init_founder` bundles; the invite carries hashes; the joiner fetches bytes over the blob lane).

- [ ] **Step 2: Commit**

```bash
git add bin/node/tests/common/mod.rs bin/node/examples ops Makefile
git commit -m "chore: every founder and dev-shape site names its module components dir"
```

### Task 10: the flag day — hello/directory/greeter/DEMO out

> **Amendment (2026-08-27, Ruling 10):** as shipped, `directory` STAYS in
> `PRODUCTION` (21 → 20, not 19) — it is the write tenant of all 8 process e2e
> suites and of `--dev-demo` (`bin/node/src/validator/engine.rs`,
> `bin/node/src/validator/run/drain.rs`). Only `hello`, `greeter` and `DEMO`
> leave here. Porting that lane to another indexed tenant, then dropping
> `directory` (20 → 19, one more root move), is a follow-up.

**Files:**
- Modify: `crates/topology/src/lib.rs` — remove `directory`, `greeter`, `hello` from `MODULES`; remove `"hello"` and `"directory"` from `PRODUCTION`; delete `DEMO` and the `demo` field on `ModuleTopology` + `TOPOLOGY`; update the pins (`PRODUCTION.len() == 19`, the sorted lists, the shape test's expected native/map lists → native `["kv","lifecycle","valset"]`, map `["runs"]`).
- Modify: `bin/node/src/constants.rs:118-123` doc ("today's 19").
- Modify: `bin/node/src/host_state.rs` — `GENESIS_ROOT_HASH` (from the failing test's message) and `PIN_BINDINGS` doc; delete `directory` from `bin/node/Cargo.toml` deps if nothing else in `bin/node` uses it (`grep -rn "directory::" bin/node/src`).
- Modify: `Makefile` `BUILDER_MODULES` — keep `crates/examples/directory` (kernel fixture) — no change; `wasm-modules-check` unchanged.
- Modify: `docs/records/architecture/wasm-module-authoring.md` lines 5–12 and 106–111 ("the node embeds the canonical artifact" → "the founder bundles it; the descriptor commits its hash"), `.claude/skills/module-dev/SKILL.md` §3 (`host_state.rs: ~10 sites` → "add the id to `topology::PRODUCTION` with its `code`/`backing`; `host_state` composes from the topology") and the gates block (`include_bytes! needs the artifact` → "the fixtures dir needs the artifact").

- [ ] **Step 1: Make the topology change; run its tests**

Run: `cargo test -p topology`
Expected: PASS with the updated pins.

- [ ] **Step 2: Run the node pins; take the new root**

Run: `cargo test -p node-bin --bin ducktape production_genesis_root_hash_is_pinned`
Expected: FAIL with "the production genesis root hash MOVED … set GENESIS_ROOT_HASH to <hex>". Set the constant to that hex.

Run again. Expected: PASS. Also `genesis_registry_matches_module_ids` and `topology_code_column…` PASS (19 ids).

- [ ] **Step 3: simnode/noded are untouched by this task** (they compose natively and never selected hello/directory); confirm:

Run: `cargo test -p simnode --test topology_set`
Expected: PASS (its 14-module pin does not change).

- [ ] **Step 4: Commit — name the flag day**

```bash
git add crates/topology bin/node docs .claude/skills/module-dev/SKILL.md
git commit -m "feat(genesis)!: hello and directory leave the production set (21 -> 19); GENESIS_ROOT_HASH moves

The reference tenant and the first wasm-port example were never used by the
app, the CLI, or any e2e; the kernel keeps their crates as fixtures. greeter and
the dead DEMO selection go with them. This is the deliberate flag day the
root-hash pin exists to make explicit."
```

### Task 11: gates + PR 2

- [ ] `cargo clippy -p topology -p workspace-config -p noded -p host --tests --no-deps` and `cargo clippy -p node-bin --tests --no-deps` — no new lints.
- [ ] `cargo check --workspace --all-targets` green; `cargo check -p files --no-default-features` green; `make wasm-modules-check` green.
- [ ] `cargo test -p workspace-config -p topology -p noded -p host` green.
- [ ] `cargo test -p node-bin --bin ducktape` green (the unit lane).
- [ ] The cluster lane for the touched paths, serially:
  `cargo test -p node-bin --test cluster_e2e --test restart_e2e --test replica_restart_e2e --test invite_e2e --test statesync_fail_closed_e2e --test network_joiner_full -- --ignored --test-threads=1`
  Expected: PASS. `restart_e2e` covers `restore_host`; `invite_e2e` + `network_joiner_full` cover `sync_all_modules` + the joiner's blob-lane fetch of genesis bytes.
- [ ] `grep -rn "include_bytes\|include_str" bin/node/src crates/noded/src | grep -i "component\|\.wasm"` → only the five `index.wasm` lines in `crates/noded/src/index.rs` (spec: out of scope).
- [ ] Push, `gh pr create --base dev` titled `feat(node): genesis wasm rides the descriptor and a bundle, never the binary`. Body: the spec link, the four decisions (1-A…1-D), the root-hash move, what stays embedded (`index.wasm`) and why, the e2e lanes run. Claude Code footer.
- [ ] Merge on green with high confidence; sweep the worktree.

---

## Self-review (done while writing)

- Spec §1 descriptor/fingerprint → Task 4; bundle + `node init` → Task 8; dev
  shape → Task 5/9; boot resolution order (blob → bundle → mesh → fail) →
  Task 7 (`seed_bundle` + `BlobCodeSource` on genesis/restore,
  `FetchingCodeSource` on sync); deletions → Task 7; admitted modules → Task 7
  `adopt_admitted_modules`; hello/directory/greeter/DEMO out → Task 10;
  `index.wasm` stays → Task 11 check. Spec §2 → Tasks 1–2. Spec §3 composer →
  Task 6; its noded/simnode callers → part 4 (not this plan).
- Types: `GenesisModules { hashes: BTreeMap<String,[u8;32]>, bundle_dir }`,
  `component_path`, `hash_bundle` (Task 5) are what Tasks 7–9 use;
  `Bindings.code_hashes: &BTreeMap<String,[u8;32]>` matches; `StoreSource` /
  `SnapshotSource` signatures identical in Tasks 6 and 7;
  `Host::module_code_hash` (Task 2) used in Task 6's test.
- Open risk named in Task 7: `StoreSource` is infallible (expect on
  `sync_from`); acceptable because the old code's `?` on the same call also
  aborted the whole sync attempt. Reviewer may ask for `Result` — decided in
  Task 6 if so.
