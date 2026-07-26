---
name: qa
description: Verify a running Ducktape node and cluster — the node's /v1 surface, module transaction round-trips, and the real-socket cluster e2e. The native Iced desktop QA (iced-agent bridge, fleet, recipe lanes) was retired when app/ was removed; node- and cluster-level verification is what remains.
---

# Node QA

The native Iced desktop and its agent-driven QA — the iced-agent bridge, the
`ops/iced-fleet` headless fleet, the `qa/recipes/*.json` recipe lanes, the
`app/src-iced/src/test/` screen unit tests, and `cargo test -p ducktape-iced` —
were **retired with the removal of app/**. There is no desktop app to drive in
this tree. What remains is node- and cluster-level verification.

## What to run

Node and module semantics — deterministic, in-process:

```bash
cargo test -p simnode                        # the deterministic /v1 twin's suites
cargo test -p node-bin --test cluster_e2e    # real 4-node cluster over localhost TCP
make test                                    # full local gate: wasm drift + workspace + sim
```

`bin/simnode` boots a deterministic node in-process for any crate's `#[test]`.
For the embedding harness (`simnode::boot`), the chat wire facts, and the
`iced_test::Simulator` traps, see the `sim-lane` skill — that skill's embeddable
half survives; only its iced-UI half was retired.

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
`ops/worktree-clean.sh` and then use `--yes`; its retired-workflow reaper is
intentionally preserved for old external homes and never uses `pkill -f`.
