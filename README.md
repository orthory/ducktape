# Ducktape

A consensus-based workplace super-app: one BFT-replicated state machine that
hosts isolated product modules — pages, forge, chat, agent workflows — the
way CosmWasm isolates contracts, but in native Rust.

Each module owns its authenticated state substrate and exposes exactly one
32-byte root to the host. The host dispatches modules over BFT consensus and
composes the sorted module roots into a global app-hash that consensus commits.
If two nodes agree on the app-hash, they agree on every module's state.

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
| `crates/kernel/` | The platform: `sdk` (module contract + codec), `host` (submit/execute loop, app-hash composition, the `host::worker` non-deterministic-effect seam), `node` (ordered replication), `consensus` (commonware Simplex BFT orderer), `statesync`, `recovery`, `indexer` (derived read-model tier), `wasm-host` (pinned wasmtime runtime for hot-swappable module components) |
| `crates/networking/` | The netstack: host-side transport infra — `wireguard`, `nat-traversal`, `reachability`, `data-plane`, `overlay-net` — plus the consensus modules that govern it, `duckdns` and `gateway` (the merged name→AccountId→route module) |
| `crates/modules/system/` | System modules and their host-side counterparts: `kv` (byte-KV), `valset` (ed25519 validator membership), `clients`, `governance`, `identity`, `lifecycle` (merged module registry + upgrade), `saga` (deterministic async continuations), `capability`, `dispatch`, `tagging`, `airlock` (exec/credential gateway), plus `blobstore`, `dispatch-oracle`, `capability-host` |
| `crates/modules/apps/` | Product modules: `forge` (git-backed project state), `pages` (documents), `chat`, `agent` (LLM-run orchestrator), `runs`, `tasks`, `vaults`, `inbox` (per-member notification queues), `automations` (rules over chat hooks), `files` (consensus manifests, node-local bytes; wraps `duckfs`), `jobs` (first-claim-wins work board) |
| `crates/duckfs/` | The versioned-filesystem engine: `core` (pure, wasm-ready — the `files` module wraps it), `disk`, `client` (OS-side) |
| `crates/guests/` | The wasm ports — one `*-wasm` guest per module (compiles the native crate to a component the node embeds) plus `guest-adapter` (the shared `ducktape:module` world binding) |
| `crates/examples/` | Reference modules: `directory` (also bin/node's liveness canary), `greeter` (types-only composition example) |
| `crates/labs/` | Quarantined experimental modules (`evm`, `multisig`): in-tree and tested but registered by NO genesis set, kept as a standalone crate EXCLUDED from the workspace so its heavy deps (revm, alloy) never tax the shipping build — gated via `make labs-gate` |
| `bin/` | Runnable binaries: `demo` (in-process walkthrough), `node` (validator), `noded` (app-facing daemon), `simnode` (deterministic /v1 twin), `coordinator` (STUN rendezvous), `fs` (duckfs CLI), `mcp` (MCP tool server), `airlock-gateway` / `airlock-broker` / `airlock-cli` (credential gateway) |
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
together and shows their roots moving under one composed app-hash:

```sh
cargo run -p demo
```

Run the real-socket cluster e2e — REAL node processes over localhost TCP,
driven through the rpc: BFT convergence, a chat product loop, a governance
vote, a live epoch cutover, a crash-fault liveness check, and a sync-only
joiner rebuilding every module to the identical app-hash:

```sh
cargo test -p node-bin --test cluster_e2e
```

Run the joiner state-sync proof — a fresh joiner rebuilds every module and
lands on the source app-hash:

```sh
cargo test -p demo --test network_joiner_full
```

Run everything the repo can verify locally (rust workspace including the e2e
suites, then the app suites against a freshly built daemon):

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

## Run The App

Ducktape has a native Rust desktop app under `app/src-iced` and a React web
twin under `app/`. The iced app owns every product screen and the desktop
lifecycle; pinned CEF is embedded only as the Browser pane. Both surfaces use
the same node HTTP/WebSocket contracts. The desktop app also owns the workspace
registry and starts or adopts each workspace's `ducktape` process.

Install the current platform's self-contained desktop package. Every default is
rootless: macOS uses `~/Applications`, Linux uses `~/.ducktape` plus
`~/.cargo/bin`, and Windows uses the current user's LocalAppData. Each package
already carries its exact matching node sidecar. A managed
Mac can opt into a shared location with `make install APP_DEST=/Applications`.

```sh
make install
```

Operators who also want the standalone node CLI in `~/.cargo/bin` can run
`make install-node` explicitly.

Web, for development — start the daemon, then the dev server:

```sh
DUCKTAPE_ALLOWED_ORIGINS=http://localhost:1420 \
  cargo run -p noded                      # http://127.0.0.1:8844, temp storage
# add -- --storage <dir> for persistent module state

cd app
bun install
bun run dev                               # http://localhost:1420
```

The web build dials `http://127.0.0.1:8844` by default; point it elsewhere
with `VITE_DUCKTAPE_NODE_URL`.

Desktop, for development — build the matching node and run the native shell:

```sh
make dev
```

On first launch the desktop app opens the onboarding gate: found a new network
or join one from an invite blob. Each becomes a **workspace** under
`~/.ducktape/workspaces/<id>/` (its own descriptor, ed25519 identity, storage,
and `daemon.log`), tracked in `~/.ducktape/registry.json`. Selecting a workspace
spawns/adopts its `ducktape` on the workspace's own port and dials it; a
joiner parks until a member admits it (Settings → Admit a joiner) and then
promotes itself, with the park→admitted→promoted phase shown live. The web build
has no registry — it dials a single configured node (`VITE_DUCKTAPE_NODE_URL`).

`make app` builds a self-contained native package under
`target/release/bundle`: an ad-hoc signed local-test `.app` plus zip on macOS, a relocatable
directory plus tarball on Linux, or a relocatable directory plus zip on
Windows. Every package includes the node sidecar and the Cargo-pinned CEF
runtime. `make web` still builds the independent static web twin to `app/dist`.
The macOS app requires macOS 14 or newer; set `DUCKTAPE_MACOS_SIGN_IDENTITY`
and `DUCKTAPE_MACOS_NOTARY_PROFILE` for Developer ID signing and notarization.

On macOS, validate the staged native window, close-to-menu-bar behavior, and
activation reopen before testing product flows (the invoking terminal needs
Accessibility permission):

```sh
make macos-smoke
make macos-cef-smoke
```

Then exercise the hardware/TCC paths that cannot be validated off-Mac:

- open Browser, navigate and resize it, and confirm CEF content stays below
  the native chrome;
- allow, deny, cancel, and retry microphone, camera, and screen sharing in a
  huddle; switch each available device and stop sharing;
- pop a huddle out and back in, close/reopen the main window, and activate a
  native notification;
- enable and use Touch ID after the normal account unlock; and
- confirm the app and workspace remain under the current user's directories
  and never request an administrator password.

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
registry, global app-hash, ordered node path, commonware Simplex orderer,
the saga async seam, and several root-backed product modules, plus state
sync for QMDB-backed, forge, and snapshot-style modules.

Still open — mostly live orchestration: network-backed module sync from a
running node, dynamic valset wiring around epoch cutover, snapshot-at-height
serving, and product depth for chat, agent, and tasks. See
[implementation status](docs/src/content/docs/en/human/reference/implementation-status.mdx)
and [what is left](docs/src/content/docs/en/human/roadmap/what-is-left.mdx).
