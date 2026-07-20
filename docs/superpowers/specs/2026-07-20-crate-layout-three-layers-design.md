# Crate layout: three layers (module / kernel / networking)

**Date:** 2026-07-20
**Status:** approved

## Problem

The project describes itself in three layers — modules, the platform, and the
netstack — but the crate tree doesn't match, and the word "system" is
ambiguous: `crates/system/` holds consensus-infrastructure *modules*, while
"system" in architecture conversation tends to mean the platform itself.
Worse, seven netstack crates (the networking layer) live inside
`crates/system/`, straddling two layers.

## Decision

Reorganize `crates/` so the tree mirrors the three layers. Directory moves
only — **no crate/package renames, no consensus changes**.

```
crates/
  kernel/       ← platform, unchanged (sdk, host, node, consensus,
                  statesync, recovery, indexer, wasm-host)
  networking/   ← wireguard, nat-traversal, overlay-net, reachability,
                  data-plane, gateway, duckdns        (from crates/system/)
  modules/
    system/     ← kv, valset, clients, governance, identity, upgrade,
                  saga, capability, capability-host, blobstore, dispatch,
                  dispatch-oracle, tagging, modreg    (from crates/system/)
    apps/       ← chat, pages, forge, agent, runs, tasks, vaults,
                  automations, inbox, files, jobs     (from crates/apps/)
  guests/       ← the wasm ports: every *-wasm wrapper + guest-adapter
                  (from crates/examples/ — production packaging, not examples)
  duckfs/       ← unchanged
  examples/     ← directory, greeter only (the true reference modules)
  labs/         ← unchanged
  module-view-host/ ← unchanged
```

After this, "system" only ever means *system modules* (`crates/modules/system/`).
The platform layer keeps its established name **kernel** — deliberately not
renamed to "system", so no directory ever changes meaning across history.

## Notes

- The `crates/networking/` crates are still consensus modules (they appear in
  `MODULE_IDS`); the tree groups by function, not mechanism. The workspace
  header comment states this explicitly.
- `MODULE_IDS`, `MODULE_STATE_SCHEMAS`, app-hash ordering, and all consensus
  state are untouched.

## Mechanical scope

1. `git mv` per the table above.
2. Root `Cargo.toml`: members list, path deps, header comment rewritten to
   describe the three layers.
3. Crate-level manifests with relative `path = "../…"` deps: recomputed for
   the new locations.
4. Source/script/doc files referencing `crates/system` / `crates/apps`
   literally (wasm parity tests, labs, bin, agent docs) updated. Dated
   historical specs/plans keep their old paths.

## Verification

- `cargo check --workspace` clean.
- Wasm parity tests compile-check (they reference module crates by literal
  path — the canary for missed references). Known pre-existing failure:
  `wasm_duckdns_parity` fails on clean origin/dev; not a regression gate.
