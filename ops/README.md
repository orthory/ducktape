# Operator scripts

Repo-side helpers for running, seeding, and maintaining a ducktape node. The
runnable surfaces are the node daemon (`node-bin`/`noded`), the deterministic
`simnode`, the UDP coordinator, and the native Iced desktop app (`app/`,
`cargo run -p ducktape-app`) — the scripts here drive the node side, and
`demo-seed.sh` seeds a workspace the app can then open. Most scripts back a
`make` target; see the repository `Makefile`.

## Dev and demo network

```bash
make dev         # ops/dev.sh        — the app dev loop: seed "demo" once, start its node + forge, keep it up
make demo-seed   # ops/demo-seed.sh  — seed a solo "demo" workspace with sample data
make demo-app    # ops/demo-app.sh   — serve the user-hosted app behind its gateway route
make dev-clear   # ops/dev-clear.sh  — stop make dev's background runtime; preserve state
make demo-clear  # ops/demo-clear.sh — stop and delete the demo workspace
```

`demo-gateway.mjs` and `demo-kanban.mjs` publish the demo's gateway web-app
routes (a network-hosted DuckFS site and a user-hosted loopback app).

## Running a node as a service

- `node/` — `ducktape-node@.service` (instance = workspace selector for
  `ducktape node run -n`), `ducktape-service@.service` (instance = kind for
  `ducktape service run compute|agent|airlock`) and the `copytruncate`
  logrotate drop-in for `daemon.log` / `<kind>.log`. The install, port and
  log recipe is `docs/deploy/node-service.md`; what to back up is
  `docs/deploy/backup-and-keys.md`.

## Sandbox (microVM) hosts

- `build-guest-rootfs.sh` — builds the guest kernel + rootfs image a Linux
  host's Firecracker sandbox boots each run from.
- `macos-preflight.sh` — checks a macOS host for the vz backend (the
  Virtualization.framework shim in `bin/duck-vz-shim`, the Kata kernel, the
  file-descriptor limit the app lane needs).
- `firecracker/` — `boot-bench.sh` and `snapshot-bench.sh`, the cold-boot and
  snapshot-restore timing lanes for the microVM sandbox.

## Forge

- `dogfood-forge.sh` (`make dogfood-forge`) — mirror GitHub `origin/dev` into
  the local node's Forge `dev` without moving release-only `main`; needs a
  running node.

## Node operator CLI

- `agent-system` — a compact operator CLI over a running node's module surface
  (raw query/submit, agent list/pause/resume); takes the node from
  `DUCKTAPE_NODE` (the same variable the `ducktape` CLI, the app, and every run
  read), else `~/.ducktape/agent-system-url`, else the active workspace in
  `~/.ducktape/registry.json`. It talks to a loopback node only, so a
  `DUCKTAPE_NODE` pointing at a remote one is refused by name rather than
  silently ignored; `use`, `help` and `cgroup` need no node and never read it.
- `completions/` — shell completions for the `ducktape` CLI.

## Networking and media harnesses

- `coordinator/` — systemd unit, env example, and Dockerfile for the UDP
  coordinator (see `coordinator/README.md`).
- `wg-smoke/` — WireGuard interop and bench harnesses (the `wg_interop`
  probe binary in two rootless podman containers — podman is only this
  harness's container runtime; the node itself has had no container sandbox
  since #1176. No node.toml involved).
- `huddle-lane.sh` — two real nodes in the dev shape with userspace
  WireGuard between them, one channel, one user key per side: the live
  arrangement a huddle (voice/camera/screen share) actually breaks in.
- `beacon-collect/` — a standalone headless consumer for iced's frame
  telemetry (`cargo run -p ducktape-app --features iced/debug`), for QA rigs
  where the upstream GUI is useless; own `Cargo.toml`, not a workspace member.

## Hosted auth page

- `auth-page/` — the `auth.ducktape.industries` WebAuthn relying-party page
  (`index.html`), its result-relay Worker (`worker.js`, `wrangler.toml`) and
  the dependency-free gate `node ops/auth-page/test.mjs`; see its README.

## Wasm guests

- `wasm-repro-check.sh` (`make wasm-repro-check`) — builds one guest component
  from this checkout and from a copy of the tree at a different absolute path
  and asserts the bytes are identical, so a committed artifact never depends on
  the builder's `/home/...`. Needs the wasm32 target and `wasm-tools`.

## Worktree cleanup

Always dry-run `ops/worktree-clean.sh` before removing merged worktrees, then
pass `--yes`. It refuses a worktree that is dirty, carries a commit not in
`dev`, or has live processes under it, and it finds those processes by cwd —
never `pkill -f`.
