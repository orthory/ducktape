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
