# Operator scripts

Repo-side helpers for running, seeding, and maintaining a ducktape node. The
runnable surfaces are the node daemon (`node-bin`/`noded`), the deterministic
`simnode`, the UDP coordinator, and the native Iced desktop app (`app/`,
`cargo run -p ducktape-app`) — the scripts here drive the node side, and
`demo-seed.sh` seeds a workspace the app can then open. Most scripts back a
`make` target; see the repository `Makefile`.

## Demo network

```bash
make demo-seed   # ops/demo-seed.sh  — seed a solo "demo" workspace with sample data
make demo-app    # ops/demo-app.sh   — serve the user-hosted app behind its gateway route
make dev-clear   # ops/dev-clear.sh  — stop make dev's background runtime; preserve state
make demo-clear  # ops/demo-clear.sh — stop and delete the demo workspace
```

`demo-gateway.mjs` and `demo-kanban.mjs` publish the demo's gateway web-app
routes (a network-hosted DuckFS site and a user-hosted loopback app).

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
  probe binary in rootless podman; no node.toml involved).

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
