# Ducktape

A consensus-based workplace super-app: one BFT-replicated state machine that
hosts isolated product modules — pages, forge, chat, agent workflows — the
way CosmWasm isolates contracts, but in native Rust.

Each module owns its authenticated state substrate and exposes exactly one
32-byte root to the host. The host dispatches modules over BFT consensus and
composes the sorted module roots into a global root-hash that consensus commits.
If two nodes agree on the root-hash, they agree on every module's state.

## The Module Rule

Modules never link each other's implementation crates. A module depends only on:

- `sdk` — the module contract and deterministic system API, and
- the types-only wire types (payload/query/reply shapes + codecs) each module
  publishes at its own crate root, for the modules it needs to address.

Cross-module reads go through host-routed queries. Cross-module writes are
emitted as messages that the host drains as follow-up ops.

## Repository Layout

The tree groups by function into three layers — module / kernel / networking:

| Path | Contents |
| --- | --- |
| `crates/kernel/` | The platform: `sdk` (module contract + codec), `host` (submit/execute loop, root-hash composition, the `host::worker` non-deterministic-effect seam), `node` (ordered replication), `consensus` (commonware Simplex BFT orderer), `statesync`, `recovery`, `indexer` (derived read-model tier), `wasm-host` (pinned wasmtime runtime for hot-swappable module components) |
| `crates/networking/` | The netstack: host-side transport infra — `wireguard`, `nat-traversal`, `reachability`, `data-plane`, `overlay-net` — plus the consensus modules that govern it, `duckdns` and `gateway` (the merged name→AccountId→route module) |
| `crates/modules/system/` | System modules and their host-side counterparts: `kv` (byte-KV), `valset` (ed25519 validator membership), `clients`, `governance`, `identity`, `lifecycle` (module code registry), `saga` (deterministic async continuations), `capability`, `dispatch`, `tagging`, `airlock` (exec/credential gateway), plus `blobstore`, `dispatch-oracle` |
| `crates/modules/apps/` | Product modules: `forge` (git-backed project state), `pages` (documents), `chat`, `agent` (LLM-run orchestrator), `runs`, `tasks`, `vaults`, `inbox` (per-member notification queues), `automations` (rules over chat hooks), `files` (consensus manifests, node-local bytes; wraps `duckfs`), `jobs` (first-claim-wins work board) |
| `crates/services/` | Off-chain service crates — the host-side executors that serve a consensus module without being one: `compute` (the dispatch WorkSpec pool/ledger/gate), `provider` (executor spec layer + the `CliProvider` run loop), `sandbox` (node-private podman, egress firewall, backend probe), `broker` (run-scoped credential loopback + airlock client) |
| `crates/duckfs/` | The versioned-filesystem engine: `core` (pure, wasm-ready — the `files` module wraps it), `disk`, `client` (OS-side) |
| `crates/guests/` | Shared wasm-port infra only: `guest-adapter` (the `ducktape:module` world binding every port shares), the wasm32 dep stubs, and the kernel-fixture test guests. Every module carries its own port (`src/guest.rs` behind the `guest` feature) and `bin/guest-builder` synthesizes the packaging — no per-module crate lives here |
| `crates/examples/` | Reference modules: `directory` (also bin/node's liveness canary), `greeter` (types-only composition example) |
| `crates/labs/` | Quarantined experimental modules (`evm`, `multisig`): in-tree and tested but registered by NO genesis set, kept as a standalone crate EXCLUDED from the workspace so its heavy deps (revm, alloy) never tax the shipping build — gated via `make labs-gate` |
| `bin/` | Runnable binaries: `demo` (in-process walkthrough), `node` (validator), `noded` (app-facing daemon), `simnode` (deterministic /v1 twin), `coordinator` (STUN rendezvous), `fs` (duckfs CLI), `mcp` (MCP tool server), `airlock-gateway` / `airlock-broker` / `airlock-cli` (credential gateway), `guest-builder` (module → wasm component packaging tool) |
| `docs/` | Nimbus documentation site (human/agent tracks, English/Korean) |

Each module publishes its wire surface — types-only payload/query/reply shapes
and codecs — at its own crate root; those wire types plus host-routed queries
are the only legal cross-module surface. `kv` and `vaults` remain as crates but
are no longer registered in the production genesis module set.

### Layer contracts

Every layer boundary is a small trait, and each obeys the same three rules: the
contract lives at its crate root (opening the crate shows it first); every trait
ships a sim/test arm in the same crate — behind feature `sim` where the double
carries a build cost; and this table is the map from each boundary to its real
and swappable arms. Rationale and the full seam designs are in
[`docs/superpowers/specs/2026-07-21-layer-contract-standardization-design.md`](docs/superpowers/specs/2026-07-21-layer-contract-standardization-design.md)
(lands with PR #718). Rows tagged "(this campaign)" are the seams being added
now; their PRs are open and unmerged. Rows tagged "(C-stage)" come from the
block-apply reassembly campaign
([`docs/superpowers/specs/2026-07-22-c-stage-simnode-reassembly-design.md`](docs/superpowers/specs/2026-07-22-c-stage-simnode-reassembly-design.md)):
not swappable-arm traits but single shared paths that replace the old
validator/noded/simnode triplication (block projection, worker reactor, genesis
topology) plus the scripted-stepping ordering arm; their stacked PRs #724–#728
are open and unmerged.

| Contract (trait · crate) | Real arm(s) | Sim / test arm | Consumers |
| --- | --- | --- | --- |
| `Orderer` · `crates/kernel/node` | `SimplexOrderer`, `FollowerOrderer` | `RoundOrderer`, `ArrivalOrderer` | node replication loop, bin/node validator engine |
| `sdk::Module` / `sdk::Ctx` / `host::ModuleFactory` · `sdk`, `crates/kernel/host` | native module, `WasmModule` | dozens of in-crate test modules | host execute/dispatch loop |
| `host::worker::Worker` · `crates/kernel/host` | `DispatchPool` | `MockOracle`, `FlakyOracle`, `EchoWorker` | host non-deterministic-effect dispatch |
| `SyncClient` · `crates/kernel/statesync` | four fetch clients | `ChannelClient`, `StoreClient`, `LiarClient` | statesync joiner / backfill engine |
| `DataPlaneTransport` · `crates/networking/data-plane` | `OverlaySockets` | `SimEndpoint` (feature `sim`) | overlay demux + acceptor loops |
| `WireGuardEffect` · `crates/networking/wireguard` | defguard, userspace | `FakeWireGuardEffect` | mesh bring-up (bin/node boot) |
| commonware runtime `E` (`Clock` / `Storage` / `Rng`) | `tokio::Context` | `deterministic::Runner` | host, node, statesync |
| `ObjectStore` · `crates/duckfs/core` | `DiskStore` | `MemStore` | `files` module, duckfs client |
| `Blobs` · `blobstore` — (this campaign, PR #716 — unmerged) | `BlobHandle` (disk) | `MemBlobs` | bin/node blob_fetch/relay_runtime/explorer, statesync serve |
| `RefsStore` · `crates/duckfs/core` — (this campaign, PR #715 — unmerged) | `DiskRefs` | `MemRefs` | `files` module (`Files<S, R>`) |
| `IndexDisk` · `crates/kernel/indexer` — (this campaign, PR #717 — unmerged) | `DiskFs` (moved `std::fs`) | `MemDisk` (feature `sim`) | indexer derived-tier writes |
| `MeshCarrier` · `crates/kernel/consensus` — (this campaign, PR #719 — unmerged) | `DiscoveryMesh` (wraps the `authenticated::discovery` Network) | `SimMesh` (feature `sim`, wraps `simulated::Network`) | bin/node validator engine, in-process cluster test |
| commonware `Clock` seam (`context.current()`) + source-parsing lint · bin/node, statesync — (this campaign, PR #720 — unmerged) | `tokio::Context` | `deterministic::Runner` | validator run/drain/ingress, statesync monitor |
| `TestCtx` (`sdk::Ctx`) + `MemStore` (`sdk::MerkleStore`) · `crates/kernel/sdk-testkit` — (this campaign, PR #718/#721 — unmerged) | host runtime `Ctx`, `QmdbStore` | `TestCtx`, `MemStore` | module unit tests (runs, automations, files, governance, …) |
| `projection::project_block` · `noded` — (C-stage, PR #724 — unmerged) | one shared block-projection path (RootOp assembly + `block_row` bytes + index feed + stream publish) | golden test pins `block_row` bytes across old/new paths | validator drain, replica park, noded submit lane, simnode — **flag day (PR #728): a rejected op now journals a block, validator parity** |
| `Orderer` — scripted-stepping seam · `crates/kernel/node` — (C-stage, PR #725 — unmerged) | — (sim-only arm) | `StepOrderer` + `StepHandle` (FIFO; release-one / release-all) | simnode actor (`OrderedNode<StepOrderer>`) |
| `worker::drive` · `crates/kernel/host` — (C-stage, PR #726 — unmerged) | one shared reactor loop (offer events, budget rounds, follow-up `Msg`s + Nudge tail) | unit test on budget / Nudge behavior | validator drain, noded submit lane, simnode auto mode |
| `ModuleTopology` · `crates/kernel/host` — (C-stage, PR #727 — unmerged) | one genesis topology (ordered id set, wiring edges, genesis-config values; subsets `production` / `sim_base` / `sim_valset` / `demo`) | `genesis_registry_matches_module_ids` + subset derivation tests | node `ProductionModules` (wasm), simnode (native), demo |

## Quick Start

Run the workspace tests:

```sh
cargo test --workspace
```

Run the in-process super-app demo — registers the platform and product modules
together and shows their roots moving under one composed root-hash:

```sh
cargo run -p demo
```

Run the real-socket cluster e2e — REAL node processes over localhost TCP,
driven through the rpc: BFT convergence, a chat product loop, a governance
vote, a live epoch cutover, a crash-fault liveness check, and a sync-only
joiner rebuilding every module to the identical root-hash:

```sh
cargo test -p node-bin --test cluster_e2e
```

Run the joiner state-sync proof — a fresh joiner rebuilds every module and
lands on the source root-hash:

```sh
cargo test -p demo --test network_joiner_full
```

Run everything the repo can verify locally (the wasm-artifact drift gate, the
rust workspace including the e2e suites, the consensus sim-feature suite, and a
build of the noded + simnode binaries the test harnesses stage):

```sh
make test
```

Run and verify only the coordinator operator path:

```sh
make coordinator-smoke
make coordinator
target/release/coordinator --help
target/release/coordinator --listen 0.0.0.0:3478
```

### Build wasm module components (guest-builder)

Prerequisites (same as always): `rustup target add wasm32-unknown-unknown` and
`cargo install wasm-tools`.

Day to day you don't invoke the tool — `make wasm-modules` rebuilds every
module component (and refreshes the kernel test fixtures), and
`make wasm-modules-check` guards that the committed copies agree.

To build one module directly:

```sh
cargo run -p guest-builder -- crates/modules/apps/tasks
```

This synthesizes the packaging workspace under `target/guest-builder/`, builds
it for wasm32, componentizes, and writes the canonical committed artifact to
`crates/modules/apps/tasks/component.wasm` (path printed on stdout). If kernel
tests pin a fixture copy, refresh it too — `make wasm-modules` does both.

An out-of-tree module directory — the distributable-as-git case — builds the
same way:

```sh
cargo run -p guest-builder -- ~/src/my-module --out /tmp/my-module.component.wasm
```

`--platform-root` overrides the ducktape checkout supplying `guest-adapter` and
the wasm32 dep patches (it defaults to the checkout the tool was built from);
`--scratch` overrides the synthesis directory.

For the tool to accept a module it must declare the port contract: a
`guest = ["dep:guest-adapter"]` feature, an optional `guest-adapter` path dep,
and a `src/guest.rs` behind `#[cfg(feature = "guest")]` containing either
`guest_adapter::snapshot_guest!` (whole-state modules), `store_guest!`
(store-backed), or a hand-written `Guest` impl + `export_module!` (the `files`
shape). `crates/modules/apps/tasks` is the reference; the full wiring runbook
is `skills/module-dev/SKILL.md`.

## Run a node

The product ships as headless surfaces — there is no bundled desktop app in
this tree. Three binaries are runnable:

- **`node-bin`** (the `ducktape` daemon) — the networked node that serves the
  module HTTP/WebSocket surface. Release build with `make node`; run a
  throwaway dev daemon with temporary storage:

  ```sh
  cargo run -p noded                        # http://127.0.0.1:8844, temp storage
  # add -- --storage <dir> for persistent module state
  ```

  A browser or any HTTP client dials `http://127.0.0.1:8844` by default; the
  node exposes the same module contracts the rest of the platform speaks.

- **`simnode`** — a deterministic in-process twin of the node's `/v1` surface
  (`bin/simnode`), used by the test lanes; embed it in any crate's tests via
  `simnode::boot`.

- **`coordinator`** — the untrusted UDP rendezvous (see the operator path
  above).

Install the `ducktape` operator CLI into `~/.cargo/bin`:

```sh
make install          # == make install-node
```

Seed a local "demo" network preloaded with sample data — chat channels and
messages, a tasks board, pages, a registered agent, an inbox note, an
automation rule, plus gateway web-app routes — registered as a "demo"
workspace under `~/.ducktape`:

```sh
make demo-seed
```

## Documentation

The docs are a separate Nimbus project under `docs/` (package manager: Bun), so
Rust verification and docs verification stay decoupled:

```sh
cd docs
bun install
bun run docs:check   # docs gate
bun run dev          # local preview
```

Pages are split by reader (human vs. coding agent) and language (English,
Korean) under `docs/src/content/docs`.

## Status

The platform spine is checked in and verified: the module contract, host
registry, global root-hash, ordered node path, commonware Simplex orderer,
the saga async seam, and several root-backed product modules, plus state
sync for QMDB-backed, forge, and snapshot-style modules.

Still open — mostly live orchestration: network-backed module sync from a
running node, dynamic valset wiring around epoch cutover, snapshot-at-height
serving, and product depth for chat, agent, and tasks. See
[implementation status](docs/src/content/docs/en/human/reference/implementation-status.mdx)
and [what is left](docs/src/content/docs/en/human/roadmap/what-is-left.mdx).
