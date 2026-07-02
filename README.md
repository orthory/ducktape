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
| `crates/apps/` | Product modules: `forge` (git-backed project state), `document`, `chat`, `tasks` |
| `crates/examples/` | Demo and test scaffolding modules: `directory`, `greeter` |
| `bin/` | Runnable binaries: `demo` (in-process walkthrough), `node` (real-socket validator process) |
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

Run two real nodes as separate OS processes, converging on a byte-identical
app-hash over localhost TCP via a live Simplex BFT engine:

```sh
cd bin/node
./examples/demo-2node.sh
```

Run the joiner state-sync proof — a fresh joiner rebuilds every module and
lands on the source app-hash:

```sh
cargo test -p demo --test joiner_rebuilds_global_app_hash
```

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
