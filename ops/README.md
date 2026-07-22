# Operator scripts

Repo-side helpers for running, seeding, and maintaining a ducktape node. There
is no desktop app in this tree anymore — the native iced shell was removed; the
runnable surfaces are the node daemon (`node-bin`/`noded`), the deterministic
`simnode`, and the UDP coordinator. Most scripts here back a `make` target; see
the repository `Makefile`.

## Demo network

```bash
make demo-seed   # ops/demo-seed.sh  — seed a solo "demo" workspace with sample data
make demo-app    # ops/demo-app.sh   — serve the user-hosted app behind its gateway route
make demo-clear  # ops/demo-clear.sh — stop and delete the demo workspace
```

`demo-gateway.mjs` and `demo-kanban.mjs` publish the demo's gateway web-app
routes (a network-hosted DuckFS site and a user-hosted loopback app).

## Forge

- `dogfood-forge.sh` (`make dogfood-forge`) — mirror GitHub `origin/dev` into
  the local node's Forge `dev` without moving release-only `main`; needs a
  running node.
- `mirror-forge-pr.sh` — deliver a merged canonical Forge PR onto GitHub `dev`
  (`mirror-forge-pr.test.sh` is its test).

## Node operator CLI

- `agent-system` — a compact operator CLI over a running node's module surface
  (raw query/submit, agent list/pause/resume); reads the selected node from
  `~/.ducktape/agent-system-url`.
- `completions/` — shell completions for the `ducktape` CLI.

## Networking and media harnesses

- `coordinator/` — systemd unit, env example, and Dockerfile for the UDP
  coordinator (see `coordinator/README.md`).
- `wg-smoke/` — WireGuard smoke, interop, and bench harnesses.

## Worktree cleanup

Current native QA has no Fleet configuration or external instance manager.
`ops/worktree-clean.sh` intentionally retains a self-contained, identity-
verified reaper for homes left by the retired Fleet workflow. Always dry-run it
before removing merged worktrees, then pass `--yes`; it refuses dirty or
unmerged work and never uses `pkill -f`.
