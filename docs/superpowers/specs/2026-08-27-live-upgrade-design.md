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
  install-node` with one `cp` loop over `BUILDER_MODULES` (the same shape as
  `~/.ducktape/executors`; `ops/dev.sh` does not fill it).
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
the lifecycle registry that are not in PRODUCTION}`, each admitted id seated on
`lifecycle::code_at(entry, checkpoint_height)` — an admission whose first
activation is past the checkpoint is seated with its first code and empty
state, and replay/`realize_module_swaps` moves it forward from there. An
admitted module is Map-backed (it was instantiated by
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

> **Amendment (2026-08-27, Ruling 10):** `directory` STAYS in `PRODUCTION` for
> now — the "unused" premise above was wrong. It is the write tenant of all 8
> process e2e suites and of `--dev-demo`
> (`bin/node/src/validator/engine.rs`, `bin/node/src/validator/run/drain.rs`).
> Porting that lane to another indexed tenant is a follow-up; `directory` leaves
> (and the root moves once more) with it. Part 1 ships 21 → 20, not 21 → 19.

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

### As built (2026-08-28, `feat/composer-callers`)

All three callers compose through `noded::compose::compose` now. What the
sketch above got wrong, and what it cost:

| Spec said | As built | Why |
|---|---|---|
| `trait CodeBytes { fn bytes(&self, id) }` | `host::CodeSource`, keyed BY HASH | the kernel already had the seam, and the ONE that a post-genesis swap uses. `DirCodeSource::open(dir, ids)` hashes the bundle first and answers by digest; the id→hash map it returns IS `Bindings::code_hashes`. An id-keyed source would have been a second, weaker lookup beside it. |
| the composer's helpers live in `compose.rs` | `crates/noded/src/bundle.rs` | `DirCodeSource`, `hash_bundle`, `qmdb_stores`, `WasmModuleFactory` and `host_from` are what a CALLER needs around the composer, not the composer. `bin/node`'s copies of the last three are deleted. |
| "the daemon parity lane" pins noded's set | there is no parity lane | `bin/noded/tests/daemon_e2e.rs` asserts `/v1/status` modules == `topology::SIM_BASE`. That is the pin; nothing else ever existed. |
| both golden hashes move | only the sim's moves: `af1078f7… → 49f49b10…` | noded publishes no golden root. `bin/node`'s `GENESIS_ROOT_HASH` does NOT move — Part 1 already put the production set on the composer, and this branch changes no bytes it composes. |
| (unstated) | `chain_id = "local"` in BOTH daemons, one const each (`SIM_CHAIN_ID`, `CHAIN_ID`) | the identity and gateway guests scope their records to it, so the composer must bind SOMETHING; noded's `/v1/status` now reports it. It reported `""` before, and the app's `named_chain` refused that — an add-key consent against noded was impossible to sign. |
| (unstated) | both echo oracles bid before answering | the saga guest runs `LeasePolicy::Strict`: a result from a worker that never claimed the attempt is refused. Each oracle now sends `SagaMsg::Accept` first, mirroring `crates/services/compute/src/lib.rs:131-154`. |

**Cost, and the follow-up it makes REQUIRED.** Genesis cranelift-compiles 14
components per daemon, per boot, with nothing shared between them:

- `bin/noded/tests/daemon_e2e.rs` readiness went 30 s → 180 s. 22 daemons
  compile in parallel under one `cargo test`; at 30 s the whole suite failed.
- the app lane (`cargo test -p ducktape-app`, which embeds simnode) went
  13.2 s → 19.1 s.

A shared compiled-component cache — one `wasmtime::Module` per (engine,
digest), reused across hosts in a process and across boots on disk — is NOT in
this branch and is the next thing to build. Both numbers above are what it has
to recover.

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

### Implementation notes (2026-08-28, `feat/module-cli`)

- `status` prints `ready k` / `ready ✓`, not `k/n`: the readiness set's size
  comes with the registry, the DENOMINATOR does not — `n` is the validator
  set, a second (governance `Members`) query for a number the `✓` already
  answers. `k` counts signalling validators; `✓` is the latch closed.
- The ceremony narration's "each runs:" line prints the SIGNER PUBKEY, not the
  real argv — it is `drive_membership_ceremony`'s wording, correct for the
  resident verbs it was written for and wrong for the module verbs (a member
  must re-run `ducktape module update <id> <component.wasm>`). Unfixed:
  changing it means teaching the shared ceremony its caller's argv.
- Validator-node governance is a MAJORITY of the voting power, so on a
  3-validator net the FIRST run reports `1 of 2 required voting power` and
  waits; the second member's run casts the deciding ballot and executes. The
  third run finds the registry already holding the bytes and says so.
- A lifecycle refusal at EXECUTE time rolls governance's whole op back
  in-kernel: the proposal keeps its `Open` status until its voting deadline,
  and there is no cancel-proposal action to clear it. So the CLI refuses every
  statically visible case BEFORE staging anything — `--after` inside the
  min-lead, `register` on a registered id or on a genesis id
  (`topology::PRODUCTION`, which a native module's absent registry entry
  cannot reveal), `update` on an unregistered id, a second pending swap — and
  when a tally never settles it names the registry's rules in the error.
- A dead peer no longer costs the 600 s dispatch reap: `service.open` in the
  code plane's push is wrapped in `OPEN_TIMEOUT` (15 s,
  `bin/node/src/code_plane.rs`), so an unreachable validator comes back as a
  failing receipt and the verb refuses in ~15 s, before any proposal exists.

## §5 Real-cluster e2e — `bin/node/tests/module_upgrade_e2e.rs`

**As built (2026-08-28, `feat/module-upgrade-e2e`).** ONE plain `#[test]`,
`a_registered_module_survives_a_live_swap_a_restart_and_statesync`, serialized
by `common::serial()` — NOT `#[ignore]`d: it runs on a plain
`cargo test -p node-bin --test module_upgrade_e2e -- --test-threads=1` in
~97 s. `Cluster::new(&[1, 2, 3, 4], &[1, 2, 3])`: the fourth peer is DECLARED
in the layout from the start and spawned only in step 7 — statesync is
fail-closed for a peer with no committed standing, and `Cluster::spawn_joiner`
appends its id to `peer_ids`, so it would declare id 4 twice.
`spawn_founders` (`tests/common/module_verbs.rs`) sets `wireguard = true` —
the code bytes travel over the overlay and nothing else — plus
`primary_coordinator = "none"`; `modules =` the repo fixtures dir; the suite
pins `checkpoint_blocks = 100000` so the restart REPLAYS rather than restoring
from a checkpoint that happened to land after the swap; and the verbs use
`--after 60` (`module_verbs::AFTER` — the lead a three-run ceremony needs to
keep its activation above the lifecycle floor on a loaded box).

1. `spawn_founders` — three validators up, each waited to `converged
   root_hash=`, `module-code plane: overlay stream bound` and a tunnel
   carrying traffic.
2. Each founder runs `ducktape module register hello
   <fixtures>/hello.component.wasm --after 60`; wait `module status --json` to
   show `hello` active at the fixture's sha256 on all three (the harness's
   committed-event wait, no sleeps).
3. Submit hello `inc` on node 1; `count == 1` on all three.
4. Each runs `module update hello <fixtures>/hello-replacement.component.wasm
   --after 60`; wait the replacement's hash active on all three.
5. `inc` → `count == 101` (the replacement steps by 100) on all three;
   `await_committed` the founders' `root_hash` to agree (`root_after_swap`).
6. `kill(2)` (SIGKILL) then `spawn(2)` over the same storage. Wait the boot
   marker `recovered root_hash=` (`bin/node/src/validator/boot.rs`) — there is
   no `restart replayed the journal` line — and assert: it equals
   `root_after_swap`, no `genesis root_hash=` marker ever appeared,
   `count == 101` on node 2, and the founders' roots still agree at
   `root_after_swap`. Observed: `recovered root_hash=… height=137 epoch=0
   replayed=5 already_on_disk=127` — the five Map-cohort blocks that span the
   register, the pre-swap `inc`, the swap and the post-swap `inc`.
7. Seat the declared joiner, then let it statesync as a fresh resident:
   - `admit_validator(Cluster::identity(4))` — governance `AddValidator`:
     Propose on 0 → `Open` → Vote on 0 and 1 → both ballots → Execute on 1 →
     `Passed`, then `cutover complete: epoch 1` on all three founders, crossed
     on their own idle blocks (no filler traffic needed).
   - `run_sync_only(3, 180 s)`: its `synced root_hash=` must equal the
     founders' POST-cutover root — the ceremony moved governance/valset state,
     so the joiner is held to that root, not step 6's.
   - `spawn(3)` LIVE. A sync-only run binds no rpc, so the count needs a live
     boot; and a non-genesis key ALWAYS enters the replica park
     (`bin/node/src/main.rs:663-672` routes it there regardless of what is on
     disk, and a sync-only run writes no recovery manifest). So the live boot
     is a re-sync, not a reopen: it prints `synced root_hash=` again (asserted
     equal to the sync-only run's — same boundary state), then `promoted:
     validator at epoch 1 boundary H; seating in-process`. Then `count == 101`
     read from node 3 — state it never executed, whose bytes it could only
     pull over the blob plane — and all four roots agree. The count is waited
     on node 0's block feed: node 3 answers `None` until it serves, and the
     chain's blocks are the wait seam either way.

Steps 6–7 are the proof for §1's "admitted modules across restart and
statesync". If either fails, §1 is incomplete — not the test.

### What it found: replay ran pre-swap blocks on post-swap code

Step 6 failed on its first run. The lifecycle registry is disk-durable and
reopens at the TIP carrying no record of what came before, and
`realize_module_swaps(h)` read that tip's `active_code_hash` — so every
replayed block, whatever its height, ran on the POST-swap component. Real on
any node: a crash within the checkpoint cadence after a swap, with ops on both
sides of it. §1's "admitted modules across restart" assumed the registry
replays; it did not.

Fixed (user-ruled) in the registry, not the test:

- `ModuleEntry.history: Vec<Activation { height, code_hash }>` — every
  activation appended in block order (the genesis seed at 0, `RegisterModule`
  at its height, the `Advance` flip at its flip height). An admission appends
  nothing until it flips.
- `lifecycle::code_at(entry, h)` — the armed pending, else the latest history
  entry at or before `h`, else the first, else `None`. One rule, three callers:
  the host's `realize_module_swaps`, restore adoption and statesync adoption.
- `ScheduledSwap.ready: bool` → `ready_at: Option<u64>`, with
  `armed_at(h) = ready_at.is_some_and(|latched| latched < h) &&
  activation_height <= h` replacing all four inline copies of the arm
  predicate. The STRICT `<` closes a second, pre-existing hole: readiness that
  latches in block `L` is invisible to the drain until `L+1`, so a replay of
  `[activation_height, L]` used to read the pending as armed while the live
  node ran those blocks on the old code.
- `ModuleEntry.active_code_hash` is no longer a field — it is derived from
  `history.last()`, so no second copy can disagree with the history. The
  `ModuleCode` projection is unchanged.

Committed shape, so the root moved twice on this branch: `GENESIS_ROOT_HASH`
`0f71fe9f… → a7988ac7… → b290fe31…` (flag day; zero live networks). Lifecycle
is `Code::Native`, so no component, descriptor or bundle hash moved and no
wasm was rebuilt.

### The gap this suite does NOT close

Restore over a POST-SWAP CHECKPOINT never runs live. Step 6 pins
`checkpoint_blocks = 100000` on purpose — replay is the hard path — and step
7's joiner statesyncs rather than restoring. So `adopt_admitted_modules`
seating `code_at(entry, checkpoint_height)` is covered by a unit test only
(`bin/node/src/host_state.rs`,
`adoption_seats_the_code_at_the_checkpoint_height`). Closing it live needs a
cluster run with the default cadence and a swap placed before a checkpoint
boundary.

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
5. §3's noded/simnode callers (the SIM golden moves, named; noded has none).

Gates per PR: `cargo clippy -p <crate> --tests --no-deps` for touched crates,
`cargo check --workspace --all-targets`, `make wasm-modules-check`, and the
touched test lanes; the e2e lane for PR 4 and the cluster-touching parts of
PR 2. `cargo check -p files --no-default-features` stays green.
