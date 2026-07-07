# Ducktape

A consensus-based workplace super-app: one BFT-replicated state machine that
hosts isolated product modules — documents, forge, chat, agent workflows — the
way CosmWasm isolates contracts, but in native Rust.

Each module owns its authenticated state substrate and exposes exactly one
32-byte root to the host. The host dispatches modules over BFT consensus and
composes the sorted module roots into a global app-hash that consensus commits.
If two nodes agree on the app-hash, they agree on every module's state.

## The Module Rule

Modules never link each other's implementation crates. A module depends only on:

- `sdk` — the module contract and deterministic system API, and
- the types-only `*-interface` crates of modules it needs to address.

Cross-module reads go through host-routed queries. Cross-module writes are
emitted as messages that the host drains as follow-up ops.

## Repository Layout

| Path | Contents |
| --- | --- |
| `crates/kernel/` | The platform: `sdk` (module contract), `state` (app-hash composition), `host` (registry + dispatch + block lifecycle), `node` (transport seam), `consensus` (commonware Simplex BFT orderer), `reactor` (worker loop for non-deterministic effects) |
| `crates/system/` | Consensus-infrastructure modules: `kv` (QMDB byte-KV), `valset` (ed25519 validator membership), `saga` (deterministic async continuations), `wireguard-upgrade` |
| `crates/apps/` | Product modules: `forge` (git-backed project state), `document`, `chat`, `agent` (LLM-run orchestrator: registry, watches, runs), `tasks`, `vaults`, `inbox` (per-member notification queues), `automations` (rules over chat hooks), `files` (consensus manifests, node-local bytes), `memory` (generation-pinned shared agent workspace), `jobs` (first-claim-wins work board) |
| `crates/examples/` | Demo and test scaffolding modules: `directory`, `greeter` |
| `bin/` | Runnable binaries: `demo` (in-process walkthrough), `node` (real-socket validator process), `coordinator` (untrusted UDP rendezvous/STUN helper) |
| `docs/` | Vocs documentation site (human/agent tracks, English/Korean) |

`*-interface` crates alongside each module are the only legal cross-module
surface.

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
cargo test -p demo --test joiner_rebuilds_global_app_hash
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

The app is one React console with two builds, both clients of the node daemon
(`ducktape-noded`): the web build dials it over http/ws; the desktop build
spawns it as a detached subprocess (an orphan that keeps running after the
window closes) and talks to it the same way.

Install everything (daemon → `~/.cargo/bin`, `Ducktape.app` → `/Applications`):

```sh
make install
```

Web, for development — start the daemon, then the dev server:

```sh
cargo run -p noded                        # http://127.0.0.1:8844, temp storage
# cargo run -p noded -- --storage <dir>   # persistent module state

cd app
bun install
bun run dev                               # http://localhost:1420
```

The web build dials `http://127.0.0.1:8844` by default; point it elsewhere
with `VITE_DUCKTAPE_NODE_URL`.

Desktop, for development — `tauri dev` stages the daemon sidecar itself:

```sh
cd app
bun install
bun run tauri dev
```

On first launch the desktop app opens the onboarding gate: found a new network
or join one from an invite blob. Each becomes a **workspace** under
`~/.ducktape/workspaces/<id>/` (its own descriptor, ed25519 identity, storage,
and `daemon.log`), tracked in `~/.ducktape/registry.json`. Selecting a workspace
spawns/adopts its `ducktape-node` on the workspace's own port and dials it; a
joiner parks until a member admits it (Settings → Admit a joiner) and then
promotes itself, with the park→admitted→promoted phase shown live. The web build
has no registry — it dials a single configured node (`VITE_DUCKTAPE_NODE_URL`).

`make app` builds the distributable desktop bundle (`.app`/`.dmg` under
`target/release/bundle`); `make web` builds the static web bundle to
`app/dist`.

## Documentation

The docs are a separate Vocs project under `docs/` (package manager: Bun), so
Rust verification and docs verification stay decoupled:

```sh
cd docs
bun install
bun run docs:check   # docs gate
bun run dev          # local preview
```

Pages are split by reader (human vs. coding agent) and language (English,
Korean) under `docs/pages`.

## Status

The platform spine is checked in and verified: the module contract, host
registry, global app-hash, ordered node path, commonware Simplex orderer,
saga/reactor async seam, and several root-backed product modules, plus state
sync for QMDB-backed, forge, and snapshot-style modules.

Still open — mostly live orchestration: network-backed module sync from a
running node, dynamic valset wiring around epoch cutover, snapshot-at-height
serving, and product depth for chat, agent, and tasks. See
[implementation status](docs/pages/en/human/reference/implementation-status.mdx)
and [what is left](docs/pages/en/human/roadmap/what-is-left.mdx).
