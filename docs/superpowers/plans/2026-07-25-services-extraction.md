# Proposal: dissolve capability-host — services move to `crates/services/{name}`

- **Date:** 2026-07-25
- **Status:** proposal (not a plan — expand into per-step plans after approval)
- **Background:** the ducktape service protocol ideation (artifact): "module =
  onchain / service = offchain". This proposal is the first physical step of
  that split and covers **zero behavior change, mechanical moves/carves only**.
  ducktaped, the socket, and plug conversion are all out of scope — the goal
  here is to pre-draw the crate boundaries so that the future plug conversion
  becomes "swap the ServiceCtx implementation".

## Diagnosis (exploration summary)

`crates/modules/system/capability-host` bundles five service concerns in one
crate (~14k lines):

| file | lines | what it is |
|---|---|---|
| `lib.rs` | 5,604 | `Provider`/`ProviderSet`/`CliProvider` run loop — the **real knot** stitching broker, sandbox, session, and workspace together |
| `broker.rs` | 3,323 | run-scoped credential broker (Codex/Anthropic loopback) + airlock client |
| `podman_api.rs` + `sandbox.rs` | 1,911 | node-private rootless podman (libpod REST) + egress nft + backend probe |
| `spec.rs` + `variants.rs` + `session.rs` + `workspace.rs` | 2,535 | executors-as-data spec layer (operator trust domain) |
| `interactive.rs` | 1,039 | pty-backed `InteractiveSession` (the terminal plane's muscle) |

Meanwhile **dispatch-host** (`crates/modules/system/dispatch/host` — pool,
ledger, gate, provisioning) and **airlock** (barrier library) are already
separate crates. Reverse deps are only bin/node, bin/noded, and dispatch-host —
a small blast radius.

Key observation: **the crate already splits itself along file boundaries.**
No new abstraction is needed — carve along the existing seams. dispatch-host's
injected seams (`SpawnFn`/`DeliverFn`/`WorkspaceProvisioner`/
`CredentialResolver`) are already narrow interfaces, so the evidence for the
service boundary exists in code before we draw it.

## Target tree

```
crates/services/
  compute/    ← move dispatch/host              [service] enrolled: run assignments, leases
  agent/      ← carve interactive.rs+session.rs [service] enrolled: pty sessions, continuity
  provider/   ← rename/move capability-host rest [library] spec layer + CliProvider executor
  sandbox/    ← carve podman_api.rs+sandbox.rs   [library] shared muscle (podman, egress, probe)
  broker/     ← carve broker.rs                  [library] run-scoped credential loopback
```

Dependency direction (no cycles):

```
bin/node, bin/noded ─→ compute · agent · provider · sandbox
compute ─→ provider, broker      agent ─→ provider
provider ─→ sandbox, broker      broker ─→ airlock(client·verify)
```

The side effect that is the real prize: **`crates/modules/` becomes
onchain-only** — just the consensus modules (capability, gateway, dispatch)
remain, and the tree matches the design's duality (module = onchain /
service = offchain).

Crate naming: directories are `{name}`; packages are `{name}-service` for
services and `{name}-host` for libraries (`agent` must not collide with the
apps/agent consensus module; `sandbox`/`broker` are kept distinct from
potential future collisions). Bikeshed-level — finalize during execution.

## Explicitly NOT moving (KEEP)

- **The three consensus modules** — `capability` (registry), `gateway`
  (credential records), `dispatch` (WorkSpec/saga). They stay onchain-side.
- **The airlock crate** — an already-extracted barrier library. It is a
  two-party protocol CONTRACT (`client`/`server`/`verify`/`testkit` features):
  the borrower side (broker-host) consumes client+verify, the lender side
  (bin/airlock-* gateway binaries) consumes server. Merging it into
  broker-host would make lender binaries depend on borrower muscle — wrong
  dependency direction, dead feature isolation (same reason the `capability`
  consensus module never merged into capability-host: a contract's twin must
  not fold into one side's implementation). Airlock sources ultimately split
  three ways: contract lib (stays standalone), client muscle (broker-host,
  done), lender serving (a future `crates/services/airlock` autonomous plug —
  the gateway binaries' logic moves there in its own servicization step).
  Known placement debt, accepted deliberately: the contract lib (and
  blobstore) still sit under crates/modules/system/, so "modules = onchain
  only" is not yet literally true — relocate the contract lib together with
  the airlock plug step, not before, to avoid moving it twice / squatting the
  plug's name.
- **bin/node glue** — `cred_resolve.rs`, `cred_cli.rs`, `agent_cli.rs`. Keeping
  the gateway↔capability-host `CredentialKind` mapping in the node is a
  deliberate wall (it keeps capability-host free of consensus-module deps).
  Do not absorb it.
- **Plain-data vocabulary** — `RunContext`, `OutputLine`, `TokenUsage`,
  `WorkspaceReceipt`, etc. move with their consumers, shape unchanged.
  `WorkspaceReceipt` is bound to `runs::WorkspaceReceipt` by a cross-crate
  field-mirror wire test — only that test's paths get updated on move.
- **The `lib.rs` run loop internals** — carving *inside* the 5.6k-line
  CliProvider is out of scope here. This proposal moves file boundaries only;
  internal decomposition is a separate effort.

## Steps (one PR = one step, each lands alone)

1. **Move compute.** `git mv crates/modules/system/dispatch/host
   crates/services/compute` + package rename + update bin/node·noded
   references. Logic diff 0. Cheapest step, and it makes `crates/services/`
   exist.
2. **Carve sandbox.** `podman_api.rs`+`sandbox.rs` (+ the egress hook fn) →
   `crates/services/sandbox`; capability-host depends on it. Only re-export
   paths change (`PodmanService`, `egress_nftables`, `run_egress_hook`) —
   including the node's `__egress-hook` subcommand callback.
3. **Carve broker.** `broker.rs` → `crates/services/broker`. The lib.rs call
   sites (`start_broker`/`apply_auth_env`/`broker_argv`) already cross pub-type
   boundaries, so this is file move + re-export cleanup.
4. **Carve agent — DROPPED from the parallel wave (found during execution,
   2026-07-25).** A mechanical carve is impossible:
   `Provider::spawn_interactive` (a default trait method, lib.rs:381-390)
   returns `InteractiveSession`, so extracting it creates an
   agent-service ↔ capability-host Cargo cycle, and `impl CliProvider`'s
   inherent impl cannot live in another crate. Also `session.rs` belongs to
   the spec data layer (the diagnosis table above was right; this step list
   was wrong) — both files ride along with step 5's whole-crate move. The
   agent carve becomes a **separate serial follow-up** after the merge:
   scope = a `Provider` trait boundary change (10 impls, 2 terminal call
   sites; ask-first structural change).
5. **Move provider.** Rename/move the remaining capability-host (spec layer +
   CliProvider + `discover`) to `crates/services/provider`. At this point no
   offchain execution crate is left under `crates/modules/system/`.

Per-step gates: `cargo clippy -p <touched> --tests --no-deps` +
`cargo check -p files --no-default-features` (must stay green) + the unit
lane. Behavior verification: after steps 1 and 5, an agent-run round trip +
pty session smoke (existing QA recipes).

## Risks / traps

- **cp -al cache poisoning** — phantom E0432/signature errors right after a
  move are the hardlink cache, not the source. `cargo clean` first (diagnosis
  recipe exists in memory).
- **Guest wasm rebuilds** — this move touches no consensus module, so the
  root hash must not change. wasm-modules-check staying green is the litmus
  test for "zero behavior change".
- **Path-derived config** — the podman socket path and AgentDirs roots derive
  from the workspace, so crate moves don't affect them; still verify the
  `DUCKTAPE_*` env-injection fns are read only at binary boundaries after the
  move (libraries do not read env).
- **The `--no-deps` rationale as usual** — compute/provider pull heavy
  dev-deps; each step is accountable only for lints in crates it touched.

## Done criteria

- `crates/services/{compute,agent,provider,sandbox,broker}` exist
  (agent arrives via the serial follow-up), zero offchain execution crates
  under `crates/modules/`.
- Workspace-wide logic diff 0 (moves, renames, import updates only).
- Agent-run, pty, and credential-lending smokes pass unchanged.
- Follow-up work (the ducktaped socket, plug conversion) can happen entirely
  inside `crates/services/*`.
