---
name: qa
description: Verify a running Ducktape node and cluster — the node's /v1 surface, module transaction round-trips, the real-socket cluster e2e, and the desktop app's own unit lane. The agent-driven desktop QA (iced-agent bridge, headless fleet, recipe lanes) is retired; app/ itself is live and tested with cargo test -p ducktape-app.
---

# Node QA

The AGENT-DRIVEN desktop QA — the iced-agent bridge, the `ops/iced-fleet`
headless fleet, and the `qa/recipes/*.json` recipe lanes — is retired, along
with the `app/src-iced` and `app/src-tauri` shells and the TypeScript those
lanes drove.

**`app/` itself was rewritten in place, not removed.** `ducktape-app` is a live
workspace member (`Cargo.toml`), a native Iced client with its UI in
`app/src/ui/*.ice` and its own `#[cfg(test)]` suites. It has no headless
driving lane; its unit tests run like any other crate and belong in every QA
pass. What is gone is the way we USED to drive it, not the app.

## What to run

Node and module semantics — deterministic, in-process:

```bash
cargo test -p simnode                        # the deterministic /v1 twin's suites
cargo test -p node-bin --test cluster_e2e    # real 4-node cluster over localhost TCP
cargo test -p ducktape-app                   # the desktop app's own suites
make test                                    # full local gate: wasm drift + workspace + sim
```

`cargo test -p ducktape-app` is not optional and not covered by the node lanes
above: this list omitted it for as long as this skill claimed `app/` had been
removed, and every QA pass that followed the list silently skipped the crate.

### On macOS: raise the fd limit first

```bash
ulimit -n 4096      # macOS defaults the SOFT limit to 256; the hard limit is unlimited
```

Without it, `cargo test -p ducktape-app` fails on macOS with
`Too many open files` inside qmdb init, from the tests that boot a simnode in
process. A node at rest holds ~340 fds and **317 of them are path-backed**
(qmdb journal blobs) — fixed at boot, not scaling with peers — so 256 is not
close to enough for even one in-process node.

The shipped binary is unaffected: `resource_limits::raise_open_file_limit()`
runs in `bin/node`'s `main()` and lifts the soft limit toward 65,536. A test
harness never goes through that `main`, which is why only the test lane sees
it. Measured 2026-07-28 on macmini-duke (macOS 26.5.2, arm64): `cargo check
-p ducktape-app --tests` is clean and 85/86 tests pass — the one failure is
exactly this limit.

`bin/simnode` boots a deterministic node in-process for any crate's `#[test]`.
For the embedding harness (`simnode::boot`) and the chat wire facts, see the
`sim-lane` skill.

### On macOS: the first exec of a fresh build pays Gatekeeper

The FIRST exec of any freshly built binary is scanned whole-file by
syspolicyd before it runs. Measured 2026-08-06 on macmini-duke: 3.2 s wall at
0% process CPU for the ~1 GB debug `ducktape`, 0.012 s once cached — and the
cache keys on the binary's hash, so **every rebuild pays it again on first
run**. Signing does not help: arm64 binaries are already ad-hoc signed by the
linker, and a Developer ID identity doesn't survive a rebuild's new hash
either (notarization is for quarantined downloads, not local builds).

Two remedies, use both:
- The app pre-warms its signer CLI at launch-window open (`hub_state()`
  spawns `ducktape --version` fire-and-forget), so the scan finishes while
  the password is being typed.
- On a dev Mac, grant the terminal the **Developer Tools** exception
  (System Settings → Privacy & Security → Developer Tools; surface the pane
  with `sudo spctl developer-mode enable-terminal`). Locally built products
  of an exempted terminal skip the first-run scan entirely.

A Linux box structurally cannot reproduce this class — when a Mac feels
seconds slower than the rig on a first action after a rebuild, time the CLI
twice on the Mac before suspecting the app.

## Live node inspection

A running daemon (`cargo run -p noded`, or a workspace node seeded by
`make demo-seed`) serves the full `/v1` surface at `http://127.0.0.1:8844` by
default. Query it directly, or drive its module surface with the
`ops/agent-system` operator CLI (raw query/submit, agent list/pause/resume).
Do not expose capability-bearing URL paths, keys, passwords, or recovery
phrases in reports.

## Process safety

Never use `pkill -f` — a pattern match will cheerfully kill an editor, a grep,
or an unrelated node. Identify a process by executable, process cwd, and the
workspace's `--config` before signalling it, or use the node's own graceful
`/v1/admin/shutdown`. Every `/v1/admin/*` route needs a credential — WHICH one
is decided by the node's `DUCKTAPE_ADMIN` exposure, and by nothing else. Read
the node's env before reaching for a token; owning an account does not change
the answer.

**`loopback` — the default, and what an unset `DUCKTAPE_ADMIN` gives you.** The
OPERATOR credential, on an on-box caller. Loopback presence alone is not
authority (a service daemon is a loopback peer too), so present the secret the
node minted 0600 into its own workspace:

```
curl -XPOST localhost:$PORT/v1/admin/shutdown \
  -H "x-ducktape-admin-token: $(cat "$WORKSPACE/admin.token")"
```

**`public` — only when the operator set `DUCKTAPE_ADMIN=public`.** The surface
is reachable off-box, so the OWNER proof-of-possession is the gate for every
peer, loopback included. The operator token is NOT accepted and NOT a fallback
there; mint a per-request PoP with the account key instead:

```
ducktape user sign-admin --key "$WORKSPACE/user.key" \
  --method POST --path /v1/admin/shutdown --node-key "$NODE_KEY"
# one json line {"key","ts","sig"} -> x-ducktape-admin-key / -ts / -sig
```

A `public` node with no committed owner yet (before its first `BindNode`) falls
back to the operator token until one commits — so on a fresh network both
recipes work, and after `user account-init` only the PoP does.

The refusals tell the two apart: a token presented to an owned `public` node is
`401 owner_signature_invalid` (wrong credential TYPE), never `403
operator_token_mismatch` (right type, wrong secret). `DUCKTAPE_ADMIN=off`
removes the routes entirely — 404, and no token is minted at all.

Never paste either credential (or a token file's contents) into a report. For
merged-worktree cleanup, dry-run
`ops/worktree-clean.sh` and then use `--yes`; it finds live processes by cwd and
never uses `pkill -f`.
