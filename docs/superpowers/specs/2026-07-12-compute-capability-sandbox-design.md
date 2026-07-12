# Compute-Aware Capability Scheduling + Sandboxed Agent Execution

2026-07-12. Status: approved design, pre-implementation.

## Goal

Route agent work to nodes by declared compute demand (cores/GB) and actually
enforce those limits on the executing sandbox. Node participation is opt-in:
by default a node serves no agent work at all.

## Decisions (settled with the user)

- Scheduling AND enforcement, one campaign, phased.
- Compute demand is declared **explicitly per run/dispatch** — never inferred.
- Demand/capacity unit is **numeric**: `cores`, `mem_gb` (open-set map, so a
  later `gpu` dimension is data, not schema).
- OS is explicit, expressed as **existing capability tags** (`os.linux`,
  `os.macos`) — discrete facts ride tags, numbers ride the new resources map.
- Sandbox backends: **podman rootless** on Linux, **tart** on macOS. No
  docker (daemon/root), no bwrap (no resource caps).
- **Default OFF**: a node announces nothing until its operator explicitly
  opts in. The announce IS the switch — in the registry ⇔ serving.

## 1. Consensus — capability module extension

`CapabilityMsg::Announce` gains `resources: BTreeMap<String, u64>` beside
`capabilities: Vec<String>`. Empty map = node offers no metered capacity
(tags-only announce stays legal — a direct-spawn node).

- Snapshot/root encoding extends to include the resources map → app-hash
  moves: **flag day** (routine on dev).
- `CapabilityReply::Node`/`All` expose resources; `Providers` unchanged.
- Same validation posture: bounded map size, bounded key length, key charset
  reuses `validate_tag`.

## 2. Dispatch — WorkSpec demands

`WorkSpec` gains `demands: BTreeMap<String, u64>` (empty = legacy demandless
job). The run-creation surface (UI / agent intake) sets it per run; an OS
requirement travels as a required capability tag, not a demand entry.
Demands ride the unassigned announcement verbatim so every node can gate on
them.

## 3. Scheduling — consensus filter + host-local admission

(Amended at planning time: saga rendezvous-assigns each attempt over the
capability's announced providers — the accept race only exists for empty
pools. Scheduling therefore has two layers.)

**Consensus layer (deterministic):** a new registry query
`CapableProviders { capability, demands }` returns providers whose ANNOUNCED
total resources cover every demanded dimension. Saga's assignment pool uses
it when a trigger carries demands, so a job can never be rendezvous-pinned
to a node that could never fit it. Total capacity only — consensus never
sees load.

**Host layer (load):** each host keeps a local reservation ledger (sum of
demands of its currently RUNNING jobs):

- `gate()`: per dimension, if `announced capacity − reserved < demand` →
  `Skip` (quiet). A demand naming a dimension the node never announced never
  matches (absent ≠ infinite). A skipped own lease expires via existing saga
  semantics and the next attempt rendezvouses elsewhere — costs one lease
  window; a `Decline` op for instant rotation is the upgrade path.
- The announcement/accept path (empty pool) runs the same check before
  claiming. Reservations are held for the execution's lifetime (RAII guard),
  released on every exit path.
- Demandless jobs bypass the ledger entirely and match any serving node with
  the tag (current behavior, preserved).
- A job nobody can take (over-capacity network, all busy) fails via the
  existing saga deadline — no new queue.

## 4. Serving switch — default OFF, explicit opt-in

- Fresh node: announces nothing. Not in the registry → dispatch never sees
  it. **Behavior change**: today detected executors auto-announce; after
  upgrade, existing nodes drop out of the registry until their operator opts
  in. This is the intended default-off migration, called out in release
  notes.
- Turning ON presents a mode choice:
  - Linux: `direct` (current unsandboxed spawn) or `podman`.
  - macOS: `tart` (serves `os.macos` jobs, hard concurrency cap 2) or
    `podman machine` (serves `os.linux` jobs). One backend per node in v1.
- The chosen mode + detected executors + (sandboxed modes only) probed
  numeric capacity compose the announce. Direct mode announces tags with an
  empty resources map, so it can never match a demands-carrying job.
- Turning OFF announces the empty set → existing removal semantics.
- On/off + mode persist as host-local node settings. No new consensus state.

## 5. Enforcement — SandboxBackend seam in capability-host

One seam in the spawn path: `SandboxBackend = Direct | Podman | Tart`. A
backend wraps the spec's argv; specs themselves stay backend-agnostic.

- **podman** (rootless): `podman run --cpus={demand.cores}
  --memory={demand.mem_gb}g`, workspace mounted rw, host executor binary +
  its auth dirs (`~/.codex`, `~/.claude`, …) + `bin/mcp` mounted ro. The
  container must reach the node's HTTP API (MCP is the only door) — network
  mode (`--network=host` vs gateway address) is an implementation detail to
  settle on the real box.
- **tart** (Apple Silicon): APFS COW clone of a base image → `tart run
  --cpu --memory`, host dirs via virtiofs (same mount strategy as podman).
  Backend-local concurrency cap of 2 (Apple Virtualization.framework limit
  on macOS guests). Fair Source license — verify terms at implementation
  time.
- Backend spawn failure returns an honest `oracle_result` error. **No silent
  fallback to unsandboxed execution.**

## 6. Onboarding — preflight + agent-assisted setup

- Node operator view gains a sandbox section: detection checklist (backend
  binary present, base image pulled, cgroup delegation on Linux) and the
  on/off + mode switch.
- Red checklist items offer a "set up with an agent" button: it creates a
  canned run **pinned to this node** ("install tart / pull base image /
  verify, report results"). No bootstrap paradox: a pre-opt-in node's setup
  run executes exactly like today's unsandboxed host runs. The operator
  clicking the button is the consent for host mutation. No new
  infrastructure — one prewritten prompt through the existing run pipeline.

## Phases (independently shippable)

1. **Schema + scheduling + podman + default-off switch** — Linux
   end-to-end: registry resources (flag day), WorkSpec demands, admission
   ledger, podman backend, opt-in announce.
2. **tart backend** — needs a real Mac pass; isolated behind the seam.
3. **Preflight UI + agent-assisted onboarding button.**

Phase 1 alone is a working system; 2 and 3 widen it.

## Error handling

- Un-satisfiable demands → saga deadline failure (existing path), error
  names the demand.
- Backend present at announce time but broken at spawn time → oracle error
  result; the node's next boot re-probes and re-announces truthfully.
- Reservation ledger is process-local; a crashed node's leases already
  expire via existing saga lease semantics.

## Testing

- capability: snapshot/install round-trip with resources; root moves when
  resources change; validation rejects oversized maps/keys.
- dispatch-oracle: admission gate unit tests (capacity math, reserve on
  accept, release on lose/finish/fail); demandless jobs bypass the ledger.
- Cross-crate wire test for the extended WorkSpec (rename must fail tests,
  not production).
- podman path: live-gated test (runs only where podman is installed),
  asserting the limits actually apply (`/sys/fs/cgroup` readback).
- tart: real-Mac manual QA recipe (phase 2).

## Open items (implementation-time)

- Verify cgroups v2 cpu-controller delegation for rootless podman on the
  Debian 13 fleet (systemd 25x should default-delegate; confirm on box).
- Container→node HTTP network mode for `bin/mcp`.
- tart license terms for the org size.
- Base image contents/versioning for podman and tart.
