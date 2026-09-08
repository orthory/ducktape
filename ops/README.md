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
  logrotate drop-in for `daemon.log` / `<kind>.log`. `install.sh` runs the
  Linux install end to end (`--dry-run` prints it). The install, port and
  log recipe is `docs/deploy/node-service.md`; what to back up is
  `docs/deploy/backup-and-keys.md`.
- `node/dev.ducktape.node.plist` + `node/install-macos.sh` — the macOS half:
  a per-user LaunchAgent template and the script that renders it for one
  workspace and hands it to `launchctl bootstrap gui/$(id -u)`
  (`--dry-run` prints the rendered plist, `--uninstall` boots it out).

## Sandbox (microVM) hosts

- `build-guest-rootfs.sh` — builds the shared kernel and rootfs for Firecracker
  (Linux) or vz (macOS). Linux installs the pinned Rust and wasm-tools through
  `guest-rust-tools.sh` by default; `ROOTFS_SETUP` selects a custom setup.
- `macos-preflight.sh` — checks a macOS host for everything the vz backend
  needs (Hypervisor.framework, the CLT, `e2fsprogs`/`squashfs`/`zstd`, the musl
  target, the entitled `bin/duck-vz-shim` on PATH, the guest kernel + rootfs)
  and reports release-signing readiness — the Developer ID identities in the
  keychain and the `ICE_NOTARY_*` variables — informationally, since a local
  build needs neither. `--prompt` (what `make dev` passes) offers to run the
  fixes it can.
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
  read), else `<ducktape home>/agent-system-url`, else the active workspace in
  `<ducktape home>/registry.json` — the home being `$DUCKTAPE_HOME` when set,
  else `~/.ducktape`. It talks to a loopback node only, so a
  `DUCKTAPE_NODE` pointing at a remote one is refused by name rather than
  silently ignored; `use`, `help` and `cgroup` need no node and never read it.
- `completions/` — shell completions for the `ducktape` CLI.

## Networking and media harnesses

- `coordinator/` — systemd unit, env example, and Dockerfile for the UDP
  coordinator (see `coordinator/README.md`).
- `wg-smoke/` — WireGuard interop and bench harnesses (the `wg_interop`
  probe binary in two rootless podman containers — podman is only this
  harness's container runtime; the node itself has no container sandbox. No
  node.toml involved).
- `huddle-lane.sh` — two real nodes in the dev shape with userspace
  WireGuard between them, one channel, one user key per side: the live
  arrangement a huddle (voice/camera/screen share) actually breaks in.
- `beacon-collect/` — a standalone headless consumer for iced's frame
  telemetry (`cargo run -p ducktape-app --features iced/debug`), for QA rigs
  where the upstream GUI is useless; own `Cargo.toml`, not a workspace member.

## Dedicated Proxmox view lane

`proxmox-view-lane.py` uses the existing Proxmox SSH tools and node CLI; it
requires Python 3.11+ on a POSIX workstation. A local record lock refuses
concurrent operations on the same lane. Its record lives on the operator workstation, outside
container data. The record holds the exact host, randomly named lane, runtime
container IDs, release revisions and file hashes. It never adopts an existing
container. A partial failed provision remains recorded for manual inspection;
re-running provision with that record is refused.

```sh
# Inspect live VM/CT allocation and storage before selecting these resource names.
python3 ops/proxmox-view-lane.py --record "$LANE_RECORD" inventory
# TEMPLATE, STORAGE and BRIDGE come from that inventory / Proxmox configuration.
# SSH_PUBLIC_KEY is a public key whose private half the operator already owns.
python3 ops/proxmox-view-lane.py --record "$LANE_RECORD" provision \
  --template "$TEMPLATE" --storage "$STORAGE" --bridge "$BRIDGE" \
  --ssh-key "$SSH_PUBLIC_KEY"
python3 ops/proxmox-view-lane.py --record "$LANE_RECORD" check
python3 ops/proxmox-view-lane.py --record "$LANE_RECORD" rollout \
  --binary "$NODE_BINARY" --modules "$MODULES_DIR" \
  --revision "$REPOSITORY_SHA" --ui-revision "$UI_SHA" \
  --reason "integrated build; state layout unchanged"
```

Provision allocates three free IDs from the live cluster inventory, at or above
200, and creates unprivileged 4 GiB / 2 core / 12 GiB containers. It does not
create or alter bridges or storage. It installs/enables SSH only inside those
new containers. CT descriptions and inner owner files must both match before
any later operation. DHCP IPv4 addresses are read from the actual containers at
rollout; all three configurations use concrete WireGuard addresses, loopback
RPC/HTTP and no public coordinator. A DHCP address change requires another
rollout to regenerate the peer configuration.

Rollout packages the executable and runtime module directory separately, hashes
every file, stages/checks all three copies before stopping services, then writes
all configurations before starting any service. It records the caller-supplied
source revisions and actual byte hashes; it does not infer build provenance.
`started_unverified` in `<record>.events.jsonl` means services were started,
not that consensus or views were verified. A failed stage leaves running
services alone; a failure after stopping services remains visible in
`pending_release` and requires operator repair. Three validators need all three
online for consensus progress.

For a breaking schema/ABI/state change, archive diagnostics and run
`reset-network --reason "<specific breaking change>"`, then repeat rollout.
Reset stops all three owned services before removing only their fixed
`/var/lib/ducktape-view-lane/network` directories. It preserves owner markers,
release files and the workstation record/journal. It never destroys a CT.

Use ordinary SSH forwards through the Proxmox host to each recorded CT's
`127.0.0.1:8844`, with the public key installed at provision. Keep host-key
verification enabled (the CT's host public key can be read via `pct exec`).
Pass the three resulting loopback URLs to the Node 22+ observer:

```sh
node ops/proxmox-view-observe.mjs "$NODE_A_WS" "$NODE_B_WS" "$NODE_C_WS"
```

The URLs must end in `/v1/ws`. The observer requires a new committed height
beyond all three starting heights and an identical root at that height. Repeated
unchanged heartbeats do not pass; mismatched roots, regressing heights, socket
failure or the 180-second deadline fail. Keep its JSON result with the rollout
journal. This proves consensus progress/root agreement, not view activation or
rendering. View replacement/removal still uses the existing `module update`
ceremony, verified active artifact hashes on every node, and the actual app
host's rendering/asset/removal checks.

Offline command/ownership and heartbeat checks (no SSH/Proxmox access):

```sh
python3 -B ops/proxmox-view-lane-test.py
node ops/proxmox-view-observe-test.mjs
```

## Hosted auth page

- `auth-page/` — the `auth.ducktape.industries` WebAuthn relying-party page
  (`index.html`), its result-relay Worker (`worker.js`, `wrangler.toml`) and
  the dependency-free gate `node ops/auth-page/test.mjs`; see its README.

## Wasm guests

- `wasm-repro-check.sh` (`make wasm-repro-check`) — builds one guest component
  twice, in two scratch directories, and asserts the bytes are identical and
  carry no host path, so a committed artifact never depends on the builder's
  `/home/...`. Needs the wasm32 target, `wasm-tools` and a pushed HEAD.
- `make wasm-rebuild-check` — the other reproducibility gate, a Makefile target
  with no script here: it rebuilds every committed guest (each `component.wasm`
  and `index.wasm`) out of the repository at HEAD, seeded from its committed
  `guest.lock`, and cmps it against the bytes in the tree. Same needs.
- `make audit` — the third out-of-band gate: `cargo deny check advisories` over
  the committed `Cargo.lock`, under the repo-root `deny.toml` where every
  carried advisory names why it is carried and what clears it. Needs `cargo
  deny` and network, so it is not in the offline `test` gate; run it when the
  lock moves.

## Worktree cleanup

Always dry-run `ops/worktree-clean.sh` before removing merged worktrees, then
pass `--yes`. It refuses a worktree that is dirty, carries a commit not in
`dev`, or has live processes under it, and it finds those processes by cwd —
never `pkill -f`.
