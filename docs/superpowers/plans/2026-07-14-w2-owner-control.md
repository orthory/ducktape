# W2 — Owner Control Plane: implementation plan

Spec: `docs/superpowers/specs/2026-07-14-w2-owner-control-design.md`.
Two PRs against `epic/account-workspace-separation`.

## PR 1 — Node admin control plane + app seam + CSP

### Node (bin/noded)
- **`bin/noded/src/admin.rs` (new)** — `AdminExposure`, `AdminConfig`,
  `ADMIN_REQ_NS`, `sign_admin` / `verify_pop`, `admin_guard` middleware
  (exposure + owner PoP + bootstrap fallback), `owner_of` (identity `OfNode`
  resolve over the actor lane), `admin_router` (shutdown, logs/tail, ping,
  module-code). Unit tests: PoP sign/verify, freshness, exposure matrix,
  bootstrap fallback.
- **`bin/noded/src/lib.rs`** — drop `/v1/shutdown` + module-code from the public
  router; conditionally `.merge(admin::admin_router(...))`; export admin types;
  `serve()` uses `into_make_service_with_connect_info::<SocketAddr>()` so the
  guard can read the peer.
- **`bin/noded/src/handle.rs`** — `admin: AdminConfig` field + `with_admin`.
- **`bin/noded/src/main.rs`** — embedded config = `Loopback`, `node_key = None`;
  read `DUCKTAPE_ADMIN` for standalone runs.
- **`bin/node/src/boot/surfaces.rs` + `BindConfig` + `main.rs` caller** — thread
  `node_key` (the signer pubkey) + `admin_exposure` (from `DUCKTAPE_ADMIN`).
- **Tests** — `bin/noded/tests/router.rs`, `daemon_e2e.rs`: `/v1/shutdown` →
  `/v1/admin/shutdown`.

### App
- **`app/src-tauri/tauri.conf.json`** — CSP `connect-src` widened (#599).
- **`bin/node/src/userkey_cli.rs`** — `user-sign-admin` verb (stdin password;
  signs method/path/ts under `ADMIN_REQ_NS`).
- **`app/src-tauri/src/user_identity.rs` + `main.rs` + `build.rs`** —
  `user_sign_admin` command, allowlisted.
- **`app/src/domain/admin-client.ts` (new)** — build signed admin requests;
  `probeAdmin()` (`/v1/admin/ping`); `adminShutdown()`.
- **`app/src/console/store/state.ts`** — `nodeControlAvailable = owner ∧
  adminReachable`; add `owner` / `adminReachable` fields (default false).
- **connect path** — set `owner` (from the auto-bind chain reads already
  happening) + `adminReachable` (probe) at connect; the "control surface not
  reachable" hint (`owner ∧ !adminReachable`).
- **TS sim test** — `app/src/test/sim/owner-control.test.tsx`: predicate truth
  table.

### Gates (PR 1)
- `cargo clippy -p noded --tests --no-deps`
- `cargo clippy -p node --tests --no-deps`
- `cargo check -p files --no-default-features` (only if files touched — it is not)
- `bun` typecheck / vitest for the TS suite.

## PR 2 — Single-node supervision
- **`app/src-tauri/src/daemon.rs`** — `watch_node_exit`: bounded-backoff
  respawn on unexpected exit; expose a stop-intent flag so an operator stop
  suppresses the respawn.
- **`app/src-tauri/src/workspaces/mod.rs`** — harden adopt/respawn idempotence.
- Gate: `cargo clippy -p <touched app-tauri crate> --tests --no-deps`.

## Deferred (reported, not holes)
- Governance consensus migration (spec §Governance) — own PR, simnode-gated,
  live-QA'd; moves genesis hash.
- WireGuard-tunnel exposure (A4 option 2).
- Config-edit + invite-mint HTTP surface.
