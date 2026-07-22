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
`/v1/admin/shutdown`. For merged-worktree cleanup, dry-run
`ops/worktree-clean.sh` and then use `--yes`; its retired-workflow reaper is
intentionally preserved for old external homes and never uses `pkill -f`.
