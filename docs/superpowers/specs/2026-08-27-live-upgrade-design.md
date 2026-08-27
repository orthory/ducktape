# Live upgrade — genesis wasm out of the binary, one composer, a tx-driven swap you can run

2026-08-27. Status: **APPROVED design, not implemented.** Zero live
networks, so every change below is a flag-day replacement — no compat arm,
no dual path, no version bump (`ducktape:genesis:v1:` stays v1).

## The goal, in one sentence

Everything that is wasm must be replaceable by a governance transaction
while the network runs, the binary must carry no wasm, and the code must say
exactly which modules are wasm and which are not — and why.

## What dev already has (audited 2026-08-27, `9720c0471`)

The upgrade pipeline exists end to end at the kernel/module level:

1. `POST /v1/admin/module-code/stage` (`crates/noded/src/module_code.rs`)
   ingests a component into the node's blob store and fans it out over the
   code plane (`bin/node/src/code_plane.rs`), answering per-peer receipts.
2. `GovAction::UpdateModule | RegisterModule | CancelModuleUpdate`
   (`crates/modules/system/governance/src/interface.rs`) → ballot →
   `LifecycleMsg::ScheduleSwap | ScheduleRegister | CancelSwap`.
3. Each validator self-submits `SwapReady` once `sha256(local bytes) ==
   committed hash` (`bin/node/src/validator/code_announce.rs`); the swap
   latches `ready` at R = n.
4. At `activation_height`, the injected `Advance` flips the committed active
   hash and `Host::realize_module_swaps` (`crates/kernel/host/src/lib.rs:725`)
   verifies bytes and swaps the component — in the live drain, recovery
   replay, and statesync catch-up alike, fail-closed on absent/tampered bytes.
5. Post-genesis admission instantiates a brand-new module through
   `WasmModuleFactory` at the same boundary.

What it does NOT have — the four items this spec delivers:

| Gap | Where |
|---|---|
| All 19 wasm tenants ship via `include_bytes!`; their sha256 seeds the lifecycle registry, so the **binary is the genesis wasm carrier** and two differently-built binaries fork at block 0. The genesis descriptor fingerprints validators only. | `bin/node/src/host_state.rs:45-234`, `crates/workspace-config/src/lib.rs:175` |
| No operator verb. `drive_membership_ceremony` covers AddResident/AddValidator/RemoveResident only; `UpdateModule` must be hand-crafted over `/v1/submit`. | `bin/node/src/cli.rs:1062` |
| No real-cluster proof. Tests stop at `crates/kernel/host/tests/{module_swap,module_register}.rs`; no `bin/node/tests/*_e2e.rs` drives stage → propose → ready → activation. simnode has `no with_code_registry, so UpdateModule proposals are gated off`. | `bin/simnode/src/lib.rs:875` |
| The topology (`crates/topology`) does not say which modules are native, which are wasm, or what host substrate each rides — `host_state.rs` hard-codes 19 × 3 constructor calls. `bin/noded` and `bin/simnode` compose NATIVE `Chat::new`/`Tasks::new`/`Pages::new`: a second copy of the logic a tx upgrade never reaches. | `bin/noded/src/main.rs:208-271`, `bin/simnode/src/lib.rs:840-915` |

## Which modules are not wasm, and why (the answer the spec asked for)

Production is 19 wasm tenants and 2 native modules after this change.

- **`lifecycle`** (native, store-backed). Its `Advance` decides the arm set
  over the FROZEN committed end-of-(H-1) state via `get_committed` — never
  staged-over-committed (`crates/modules/system/lifecycle/src/lib.rs:13-40`).
  The WIT world exposes only the staged view; there is no committed-only read
  import. And it is the thing that decides which code runs: it cannot be a
  tenant of the mechanism it gates (who verifies lifecycle's own swap?). The
  host also reads it out of block (`lifecycle_module_status`).
- **`valset`** (native, store-backed). Its committed set is the consensus
  engine's participant set at epoch cutover, the join gate's roster, and the
  R = n denominator of every swap. A broken valset upgrade halts the chain
  with no governance quorum left to roll it back. Kernel coordinator, same
  class as lifecycle.
- **`files` / `forge`** are HALF wasm: the guest is the pure consensus core
  (CAS gate, refs, ownership, tracker); the host keeps the disk substrate —
  duckfs odb (`FilesOdbBacking`), git repos/packs/materialization
  (`ForgeOdbBacking`). The WIT world has no filesystem and the substrate is
  bytes-on-disk outside the root. Logic upgrades by tx; substrate by binary.

Outside modules, binary-only by nature: the kernel host (dispatch, staged
store, root-hash), consensus, the recovery journal/checkpoints, statesync,
the blob and code planes, mesh/overlay/WireGuard/reachability, the `.duck`
gateway HTTP, relay, services (sandbox microVM, agent runtime, airlock,
huddle), the index engine, and the app. These are IO, networking, and
determinism boundaries — the sweet spot the spec asked us to find, and the
place a "networking upgrade by tx" stops.

Also out of scope, recorded: `crates/noded/src/index.rs` embeds five
`index.wasm` mappers via `include_bytes!`. They are the derived tier — never
in any root — so they stay embedded; moving them is a separate task.

## Decisions (each was put to the user; answers recorded verbatim)

| # | Decision |
|---|---|
| 1-A | Genesis wasm source: descriptor carries hashes; bytes live in a workspace directory. |
| 1-B | `genesis_namespace` fingerprint INCLUDES the module hashes. |
| 1-C | Module shape (code origin, state backing, query mode) lives in `crates/topology` `ModuleSpec`, not `host_state`. |
| 1-D | `hello` and `directory` leave the production genesis set. |
| 2-A | `ducktape module update\|register <id> <wasm> [--after N]`; the CLI derives the absolute height. |
| 2-B | A fan-out receipt that is not all-ok REFUSES the proposal (prints the peers, exits non-zero). No flag. |
| 2-C | Verbs: `update` + `register` (+ `status`). No `cancel`, no `remove` (neither exists nor is needed; a `remove` would need a wiring gate — see Deferred). |
| 2-D | `ducktape module status` reads lifecycle `ModuleStatus`. |
| 3-A | The e2e runs register → swap → validator restart → cold joiner. |
| 4-A | Native/wasm is its own `code` field, orthogonal to `backing`. |
| 5 | `bin/noded` AND `bin/simnode` move onto the shared composer in this work. |

## §1 Genesis wasm out of the binary

### Descriptor

`NetworkDescriptor` gains:

```rust
pub struct ModuleCode { pub id: String, pub code_hash: String /* 64 hex */ }
pub modules: Vec<ModuleCode>   // sorted by id; REQUIRED (no serde default)
```

A `network.toml` without `modules` is not runnable — `from_toml` refuses it
by name. `genesis_namespace()` hashes, after the sorted validator lines, one
`\n{id}={code_hash}` line per module (sorted by id), under the unchanged
`ducktape:genesis:v1:` tag. Two nodes with different wasm sets are therefore
different networks, not a block-0 fork.

`validators` stays a set; `modules` is the second consensus-relevant list.
`bootstrap`, `reach`, `coordination` stay excluded exactly as today.

### Bundle and file naming

One naming convention, already in use by the kernel fixtures:
`<dir>/<id>.component.wasm`. Three directories use it:

- `crates/kernel/host/tests/fixtures/` — the repo copy, refreshed by `make
  wasm-modules` (already the case). Tests and the e2e harness point here.
- `~/.ducktape/modules/` — the operator's managed dir, populated by `make
  install-node` and `ops/dev.sh` with one `cp` loop over `BUILDER_MODULES`
  (the same shape as `~/.ducktape/executors`).
- `<workspace>/modules/` — the network's own bundle, written by `ducktape
  node init` (below), same id-named files. The loader verifies
  `sha256(bytes) == descriptor hash` on every read, so no content-addressed
  naming is needed.

### `ducktape node init --modules <dir>` (default `~/.ducktape/modules`)

For every `code: Wasm` id in `topology::PRODUCTION`: read
`<dir>/<id>.component.wasm`, sha256 it, append to `descriptor.modules`, copy
the file to `<workspace>/modules/<id>.component.wasm`. A missing file refuses
the founding and names the id. The descriptor is then saved as today; invites
carry it (the hashes ride the invite, the bytes do not).

### Dev shape (`peer_seeds`, no descriptor — the e2e clusters)

`node.toml` gains `modules = "<dir>"`. Resolve derives the module list and
hashes from the dir with the same loader; every node of a dev cluster points
at identical bytes (the e2e passes the repo fixtures path). The dev
namespace stays raw.

### Boot: bytes by hash, fail-closed

For each wasm id the node needs (descriptor at genesis; the lifecycle
registry's active hash after), resolve bytes in this order:

1. the persistent blob store (`NodeHandle` already opens
   `BlobHandle::persistent(<workspace>)`) — post-swap bytes land here and
   survive restart;
2. `<workspace>/modules/<sha256>.wasm` (the founder's bundle) — put into the
   blob store on first read;
3. the mesh blob lane (`blob_fetch::fetch_blob`) — a cold joiner has neither
   of the above; this is the same lane a post-swap straggler heals through;
4. otherwise fail closed with the id and hash (`reason =
   "code_bytes_absent"`).

`seeded_lifecycle` seeds from `descriptor.modules`. `realize_module_swaps`
is unchanged. `seed_genesis_components` is deleted (step 2 replaces it).

### What is deleted from `host_state.rs`

The 19 `*_WASM_COMPONENT` consts and `*_MODULE_ID` consts, every
`genesis_*_wasm()` / `*_wasm(store)` constructor, `seed_genesis_components`,
and the `ProductionModules` struct. `genesis_host` / `restore_host` /
`sync_all_modules` become three calls into the composer (§3) with different
store sources. `genesis_registry_matches_module_ids` stays;
`production_genesis_root_hash_is_pinned` builds its descriptor from the repo
fixtures by path (`std::fs::read`, no `include_bytes!` anywhere in the
binary or its tests).

### Admitted modules across restart and statesync

`restore_host` and `sync_all_modules` compose `topology::PRODUCTION ∪ {ids in
the lifecycle registry with a non-empty active hash that are not in
PRODUCTION}`. An admitted module is Map-backed (it was instantiated by
`WasmModuleFactory::instantiate`, i.e. `WasmModule::from_bytes`), so its
state is in the checkpoint/statesync manifest's snapshot lane already
(`host.module_roots()` iterates the whole registry); the composer installs
it exactly like `runs`. The lifecycle store is opened first so the set is
known before the rest compose. §5's e2e proves both paths.

### Production set

`hello` and `directory` leave `topology::PRODUCTION` (21 → 19) and the
topology universe; `greeter` and the dead `DEMO` selection go with them
(`bin/demo` was deleted 2026-08-26; nothing composes `DEMO`). `kv` stays
(`SIM_VALSET`). The `crates/guests/hello-wasm{,-replacement}` crates and
their fixtures stay — they are the kernel tests' and the e2e's swap subject.
The native `directory` crate stays for the kernel tests that construct it
directly. `GENESIS_ROOT_HASH` moves; the commit says so.

## §2 The topology says the shape

```rust
pub enum Code { Native, Wasm }
pub enum Backing { Map, Store, Odb }

pub struct ModuleSpec {
    pub id: &'static str,
    pub wiring: &'static [&'static str],
    pub config: &'static [&'static str],
    pub code: Code,
    pub backing: Backing,
    /// dispatch only: the guest's query lane is committed-only.
    pub committed_queries: bool,
}
```

| id | code | backing | notes |
|---|---|---|---|
| valset, lifecycle, kv | Native | Store | kernel coordinators (+ the sim's kv) |
| files, forge | Wasm | Odb | duckfs / git substrate host-side |
| runs | Wasm | Map | snapshot-lane tenant |
| dispatch | Wasm | Store | `committed_queries: true` |
| acl, agent, automations, capability, chat, gateway, governance, identity, inbox, pages, saga, tagging, tasks | Wasm | Store | |

Parity test in `host_state`: `{id | spec.code == Native}` equals `{id |
host module's code_hash().is_none()}`. Topology's own tests pin PRODUCTION
to 19, SIM_BASE to 14, SIM_VALSET to 5, and that every Odb/Map/committed
flag matches the table above.

## §3 One composer, three binaries

`crates/noded/src/compose.rs` — the noded library is already a dependency
of `bin/node`, `bin/noded`, and `bin/simnode`.

```rust
pub trait CodeBytes { fn bytes(&self, id: &str) -> Result<Vec<u8>, String>; }

pub struct Substrates { pub forge_repo: PathBuf, pub duckfs_dir: PathBuf,
                        pub blobs: BlobHandle }
pub struct Bindings   { pub invite: Vec<u8>, pub chain_id: String,
                        pub validators: Vec<Vec<u8>> }

pub async fn compose(
    selection: &[&str],
    code: &dyn CodeBytes,
    stores: &mut dyn FnMut(&'static str) -> BoxFuture<Box<dyn MerkleStore>>,
    substrates: Substrates,
    bindings: Bindings,
) -> Result<Vec<Box<dyn Module>>, String>
```

One loop over `selection`: `match spec.code` picks the native constructor
(`Valset::new` + seed, `Lifecycle::new`, `Kv::new`) or `code.bytes(id)`;
`match spec.backing` picks `with_store(stores(id))` / `with_odb(open
substrate)` / `from_bytes`; `spec.config` drives `seed_store_config`;
`spec.committed_queries` applies `.with_committed_queries()`. Sibling wiring
is compiled into the guests (it already is), so no `.with_*` calls survive.

Callers:

- `bin/node` genesis/restore: `stores = QmdbStore::init`; statesync:
  `stores = QmdbStore::sync_from(target, resolver)`; `code` = the §1 boot
  resolver. Seeding (valset validators, lifecycle hashes) happens inside the
  native arm from `bindings` / the descriptor.
- `bin/noded`: `--modules <dir>` (default `~/.ducktape/modules`), selection
  `SIM_BASE`, `code` = `<dir>/<id>.component.wasm`.
- `bin/simnode`: `SimOpts.modules_dir` (default: the repo fixtures dir,
  resolved from `CARGO_MANIFEST_DIR`); `SIM_BASE`, plus `SIM_VALSET` when
  `valset_keys` is non-empty — governance and acl compose as wasm with the
  invite riding `__config`, so the sim gains the real code registry (the
  "UpdateModule gated off" restriction disappears).

The noded and simnode golden hashes (`bin/simnode/tests/topology_set.rs`,
the daemon parity lane) move; each commit names the flag day. The app's
tests boot simnode and will pay the wasmtime load (19 components) — accepted.

## §4 CLI — `bin/node/src/module_cli.rs`

```
ducktape module update   <id> <component.wasm> [--after N]   # N default 50
ducktape module register <id> <component.wasm> [--after N]
ducktape module status
```

`update`/`register`:
1. read the file, `POST /v1/admin/module-code/stage` (fan-out on) through
   the node's owner-gated admin route; the reply is the digest + receipts.
2. Any receipt with `ok: false` → print `peer  status` per failing peer and
   exit 1. Re-running is idempotent (`already-have`).
3. `activation_height = /v1/status height + N`.
4. Ceremony: `drive_membership_ceremony` is generalized to take an open-
   proposal matcher; module verbs match on (action variant, `module_id`,
   `code_hash`) — the second member's computed height differs, so equality
   on the whole action would never join the founder's proposal. Propose if
   absent, cast yes, execute when decidable; print the lifecycle's own
   rejection verbatim if the registry refuses (min-lead, at-most-one
   pending, already-registered).
5. On success, print: `scheduled <id> → <hash> at height <h>; track with:
   ducktape module status`.

`status`: query lifecycle `ModuleStatus`; one row per module: `id`,
`active` (first 12 hex), `pending` (hash, `ready k/n`, `activation`), or
`—`.

## §5 Real-cluster e2e — `bin/node/tests/module_upgrade_e2e.rs`

`#[ignore]` like every cluster suite (runs under `make test`'s
`--ignored --test-threads=1` lane). `Cluster::new(&[1,2,3], &[1,2,3])`,
`modules =` the repo fixtures dir.

1. Spawn three validators; wait admitted/serving.
2. Each runs `ducktape module register hello <fixtures>/hello.component.wasm
   --after 10` (via `run_verb`). Wait until `module status` shows `hello`
   active (poll `status` through the harness's committed-event wait, no
   sleeps).
3. Submit hello `inc` on node 1; query `count == 1` on all three.
4. Each runs `module update hello <fixtures>/hello-replacement.component.wasm
   --after 10`; wait active.
5. `inc` → `count == 101` (the replacement steps by 100) on all three;
   `await_committed` root-hash equal across nodes.
6. Kill node 2, respawn; wait the `restart replayed the journal` marker;
   root equal; `count == 101` on node 2.
7. `spawn_joiner(4)`, sync-only; root equal; `count == 101` on the joiner.

Steps 6–7 are the proof for §1's "admitted modules across restart and
statesync". If either fails, §1 is incomplete — not the test.

## Deferred (recorded, not built)

- `ducktape module cancel` — `CancelModuleUpdate` exists; add the verb the
  first time a pending gets stuck.
- Module REMOVAL — no `RemoveModule`/`ScheduleRemove`/`Host::unregister`
  exists. Removing a module that another module's `wiring` names turns that
  module's sibling calls into deterministic `UnknownModule` rejections;
  a safe remove needs a wiring gate, and wiring lives in the topology, not on
  chain. Design when the first removal is wanted.
- `index.wasm` out of the binary (derived tier).
- Reproducible component builds across toolchains (`wasm-repro-check` exists;
  the descriptor makes the hashes explicit, it does not make them
  reproducible).

## Delivery order (one PR each, against `dev`, worktrees under `.worktree/`)

1. §2 topology fields + parity tests (no root-hash movement).
2. §3 composer + §1 descriptor/bundle/loader + hello/directory/greeter/DEMO
   removal (**flag day**: `GENESIS_ROOT_HASH` moves, named in the commit).
3. §4 CLI verbs + status.
4. §5 e2e (fixes anything it exposes in 2).
5. §3's noded/simnode callers (golden hashes move, named).

Gates per PR: `cargo clippy -p <crate> --tests --no-deps` for touched crates,
`cargo check --workspace --all-targets`, `make wasm-modules-check`, and the
touched test lanes; the e2e lane for PR 4 and the cluster-touching parts of
PR 2. `cargo check -p files --no-default-features` stays green.
