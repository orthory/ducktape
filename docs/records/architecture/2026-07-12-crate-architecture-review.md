# Crate Architecture Review — post refactor campaign (2026-07-12)

Capstone of the 9-batch crate-by-crate refactor campaign (PRs #391, #394,
#398, #400, #403, #404, #407, #408, #411; plan:
`docs/superpowers/plans/2026-07-11-crate-refactor-campaign.md`). Reviewed
read-only at the merged tip of `dev`. Verdict summary first; evidence below.

| Tier | Verdict | The one thing to fix |
|---|---|---|
| kernel/ (8) | Sound | Move `capability-host` to `system/` — the tier's only normal-dep edge into system/, and its own doc calls it a module's "machine-local counterpart", not platform. **(Applied in this PR.)** |
| system/ (18) | Sound | Nothing structural; absorbs `capability-host`. |
| apps/ (11) | Sound | Nothing — the types-only rule holds under import-level inspection (`runs`, `automations`, `forge` verified). |
| examples/ (3) | Needs a product call | `evm` self-describes as experimental but is genesis-registered in the real daemon (`bin/noded`) — every node pays its root-hash cost. Either feature-gate its registration or own it as product (rename/move). **Flagged for the user, not changed here** — it shipped deliberately this week. |
| duckfs/ (3, untiered) | Needs docs only | The core/disk/client split is correct (the wasm gate forces it); it just needs a line in the workspace header + the types-only list. **(Applied.)** |
| bin/ + shell | Sound | Header silence on noded/simnode/coordinator/fs was a docs gap. **(Applied.)** |

## Tier integrity

- kernel/ discipline is real: host, node, consensus, recovery, statesync
  fence every concrete-module reference into `[dev-dependencies]` (their
  manifests say so explicitly). `indexer` earns kernel placement by being
  domain-agnostic (no sdk/host dep, 9-crate fan-in).
- `capability-host` was the misfit: kernel-tier but host-side,
  single-domain, 3 consumers, and the workspace's ONLY kernel→system
  normal dependency (`capability` for `validate_tag`). Its twin pattern
  (`dispatch`/`dispatch-oracle`) already lives entirely in system/. Moved.
- `directory` (examples/) is genesis-registered in `bin/node` as the
  liveness canary — contained (not in noded, the shipped daemon), but the
  "examples" framing undersells it; its Cargo doc now says so.

## Dependency-graph legibility

- No system→apps edges (all 18 manifests grepped: zero).
- apps→apps edges are all wire-types-only, verified at the import level:
  `runs` imports only `*Msg`/`*Query`/`*Reply` + `encode_*`/`decode_*`
  from agent/chat/tasks/jobs/pages; `runs`→`forge` is dev-only with a
  written rationale (keeps vendored libgit2 out of the production graph).
- kernel→system: only host→upgrade/dispatch, both labeled WIRE-TYPES-ONLY
  in the manifest — legitimate and narrow. (capability-host's edge is
  gone with the move.)
- The near-universal pattern of module crates dev-depending on `host`
  solely to prove root composition in e2e tests is what keeps the
  production graph honest — worth preserving as a convention.

## Merge/split candidates — examined and REJECTED

- `duckfs-disk` (737 lines) must NOT merge into core: the split is the
  mechanism behind the `cargo check -p files --no-default-features` wasm
  gate ("No sdk and no disk persistence" is core's charter).
- `greeter` (81 lines) stays: it is the reference example of cross-module
  composition through wire types, and the one examples crate that never
  touches a shipped binary.
- `blobstore` (240 lines) stays: three independent cross-tier consumers,
  none a natural parent.

The 8/18/11 resting shape is right. The network cluster
(wireguard/nat-traversal/reachability/data-plane/overlay-net) reads as
small-crates-clean-seams, not fragmentation.

## Naming

- The workspace Cargo.toml header was rewritten (this PR): it named 4 of 8
  kernel crates, called system/ "consensus infrastructure (kv, valset)"
  when ~half the tier is explicitly off-consensus, omitted duckfs/ and
  evm entirely, and glossed `host` as "(dispatch)" — a live crate name
  belonging to a different crate.
- The `-host`/`-oracle` twin suffix ("the impure host-side counterpart of
  a consensus module") is a good convention; keep using it.

## sdk health + the gate for future additions

`sdk` is still one file + one codec module + async-trait. The campaign's
additions (`codec`, `Origin::actor_string`, `require_non_empty`) each
replaced ≥3 near-identical per-module copies and operate on generic
shapes only. The standing rule:

> A change belongs in `sdk` only if (a) it is currently duplicated,
> near-identically, across three or more module crates, AND (b) it
> operates on generic shapes (bytes, strings, Origin, Error) with zero
> knowledge of any module's domain types. `sdk` is a grammar, not a
> library of conveniences.
