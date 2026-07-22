# C-stage Implementation Plan — Block-Apply Seam & simnode Reassembly

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development semantics apply (fresh implementer per task, adversarial review per PR, fix loops). Tasks form a LINEAR STACK — each branch forks from the previous task's branch; each PR's base is the previous branch.

**Goal:** one shared block-projection path, one reactor loop, one genesis
topology, and simnode running on `OrderedNode<StepOrderer>` — per
`docs/superpowers/specs/2026-07-22-c-stage-simnode-reassembly-design.md`.
**Evidence file every implementer reads first:**
`.superpowers/sdd/c-stage-duplication-map.md` (exact anchors; do not re-derive).

## Global Constraints

- **Delivery: every task ends with an OPEN PR. NEVER merge anything.**
- Stack: base branch `integration/layer-contracts` (pushed; current dev +
  eight B branches). Task N forks from task N−1's branch; PR base = previous
  branch (C1's PR base = `integration/layer-contracts`). Do not rebase earlier
  stack members; fixes go on top of the owning branch.
- Worktrees under `<primary>/.worktree/<slug>`; `CARGO_INCREMENTAL=0`; cache
  seed via `cp -al` allowed, `cargo clean -p <crate>` on stale-artifact
  weirdness.
- Gates per task: `touch` a `.rs` first; `cargo clippy -p <touched> --tests
  --no-deps`; `cargo test -p <touched>`; task-specific gates below. node-bin
  unit lane is `cargo test -p node-bin --bin ducktape`. Known reds to never
  chase: voice unit, overlay_e2e audio, cluster_e2e reachability subtest,
  dogfood forge_push grant-drop.
- Tests wait on events, never time. `tracing` only. No backcompat shims; the
  approved flag day (rejected op journals a block) is the ONLY sanctioned
  behavior change, isolated in its own commit.
- Commits end `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`; PR
  bodies end with the Claude Code trailer and state the stack position
  ("stacked on <base>; merge order C1→C6 after B lands").

### Task C1 — block projection seam (branch `feat/c1-block-projection`, base `integration/layer-contracts`)

**Files:** noded lib (new `projection` module exporting `project_block` and the
generalized RootOp assembly lifted from `bin/node/src/explorer.rs::explorer_root_op`
+ `bin/node/src/drain_actions.rs::block_actions` ordering); adopt in
`bin/node/src/validator/run/drain.rs`, `bin/node/src/replica/park.rs`,
`bin/node/src/drain_actions.rs` (which shrinks to a thin wrapper or dies).
**Produces (later tasks consume):** `noded::projection::project_block(...)` —
exact signature chosen from the map's step tables (inputs: height,
consensus_time, drained frames OR BatchOutcome-shaped members + system
dispatches, `&BlobHandle` [B's `dyn Blobs` if present on the base], `&IndexStore`,
`&StreamHub`); returns the per-block records it wrote (for receipt shaping).
Resolve the `dispatch_info` divergence to the validator's `external:<name>` shape.
**Steps:** golden test FIRST pinning `block_row` bytes from the CURRENT
validator path for a fixture set (applied, rejected, System dispatch,
multi-member, empty-batch-no-row) → extract seam → adopt validator+replica →
golden test still passes byte-identically. Gates: `-p noded -p node-bin`
clippy/unit + `cluster_e2e cluster_lifecycle`. Open PR; leave open.

### Task C2 — kernel: StepOrderer + OrderedNode policies (branch `feat/c2-step-orderer`, base C1)

**Files:** `crates/kernel/node/src/lib.rs` (or a sibling module file).
**Produces:** `StepOrderer` (impl `Orderer`; FIFO; parks until released) +
`StepHandle { release(n) / release_all() }`; `OrderedNode` time policy —
`ConsensusTimePolicy { HeightIsTime, Epoch { base_ms, block_ms } }` replacing
the hardcoded `consensus_time = height` (map §1 step 7), default preserves
today's byte behavior; sim-gated (`feature = "sim"`) pre-decoded ingress
`submit_decoded(BlockOp)` bypassing decode_member for unsigned sim lanes —
NO wire change.
**Steps:** TDD — kernel tests: FIFO release order (vs RoundOrderer's sort),
one-per-step release, release_all, Epoch policy stamping, submit_decoded
landing in the next block. Existing kernel tests stay green (HeightIsTime
default proves no validator change). Gates: `-p node` clippy/test (kernel
crate), `cargo check -p node-bin`. Open stacked PR.

### Task C3 — noded onto shared lanes (branch `feat/c3-noded-shared-lanes`, base C2)

**Files:** `bin/noded/src/main.rs` (submit_one/submit_and_drain), `crates/kernel/host/src/worker.rs`.
**Produces:** `host::worker::drive(...)` — the pure reactor loop (offer events,
budget rounds via MAX_WORKER_ROUNDS, return follow-up Msgs + Nudge tail);
noded's submit lane projects via `noded::projection::project_block` and drives
workers via `host::worker::drive`. noded's wall-clock consensus_time and
1-op-1-block lane are UNCHANGED (spec non-goal).
**Steps:** extract drive from the three copies' common shape (map §2 step 8,
§1 drain.rs L891, §3 offer_effects) with a unit test on budget/Nudge behavior;
adopt in noded + validator drain; delete noded's private RootOp assembly and
`dispatch_info` copy. Gates: `-p noded -p host -p node-bin` clippy/unit;
`cluster_e2e cluster_lifecycle` (validator reactor touched). Open stacked PR.

### Task C4 — genesis topology single source (branch `feat/c4-genesis-topology`, base C3)

**Files:** `bin/node/src/host_state.rs` + `bin/node/src/constants.rs`,
`bin/simnode/src/lib.rs` (composition section only), `bin/demo/src/main.rs`.
**Produces:** `ModuleTopology` (single source of: ordered id set, wiring
edges, genesis-config values incl. NetworkBindings, named subsets `production`
/ `sim_base` / `sim_valset` / `demo`) — home: a small module in the noded crate
or bin/node lib such that simnode/demo can consume it without violating crate
direction (implementer picks, states why). node's `ProductionModules::compose`
consumes `production`; simnode composes `sim_base`(+`sim_valset`) natively;
demo composes `demo`. `MODULE_IDS`, `BASE_MODULE_IDS`, `VALSET_MODULE_IDS`
become derivations; `genesis_registry_matches_module_ids` derives from
topology; NO `[&str; N]` count annotation survives.
**Steps:** topology + derivation tests first (production set == today's 20;
sim_base == today's 14; demo set == today's); swap the four sites; app-hash
of a default simnode genesis must be byte-identical before/after (assert in
test). Gates: `-p node-bin -p simnode -p demo` clippy/test + node-bin unit
lane. Open stacked PR.

### Task C5 — simnode reassembly (branch `feat/c5-simnode-reassembly`, base C4) — the big one

**Files:** `bin/simnode/src/lib.rs` (+ split into modules if it shrinks the
file per the mono-file cap), `bin/simnode/tests/*`.
**Produces:** Sim actor = `OrderedNode<StepOrderer, NullSink>` with
`ConsensusTimePolicy::Epoch{SIM_EPOCH_MS, SIM_BLOCK_MS}`; client submits ride
`node.submit_frame` (decode_frame verification unchanged); `/sim/step` =
StepHandle release + flush + `drain_delivered` + `project_block`; auto mode =
release_all loop + `host::worker::drive`; peer/hex lanes via `submit_decoded`;
receipts shaped from projection returns. DELETE: `commit`, `commit_batch`,
private offer_effects/drain_oracle_budgeted, private RootOp/row assembly,
status hand-build (share noded's builder where reachable). PRESERVE the map §6
keep-list items 1–12 and 14 exactly.
**FLAG DAY commit (isolated):** rejected single-op now journals a block;
update the sim-lane/e2e tests that asserted no-block-on-reject; commit message
names the approved semantic change.
**Steps:** port the simnode e2e suite expectations first where mechanical;
reassemble; run `cargo test -p simnode` (excluding pre-existing
governance_scenarios red if still present on the base — classify, don't chase);
run the embedded sim-lane consumers if reachable (`iced` sim tests) and report
which ran. Gates: `-p simnode` clippy/test; node-bin unit lane; golden
projection test still byte-identical. Open stacked PR.

### Task C6 — docs (branch `docs/c6-c-stage-docs`, base C5)

README Layer-contracts table gains rows: `StepOrderer` (sim arm of the
ordering seam), `noded::projection` (block projection), `host::worker::drive`,
`ModuleTopology`; link the C spec; note the flag day in simnode's row. Update
the spec's status line to "implemented (stacked PRs, unmerged)". Open stacked PR.

## Self-review (write-time)

Spec §decisions 1–7 ↔ tasks: 3→C1, 1+2→C2, 4→C3, 5→C4, 6+7→C5, docs→C6. Flag
day isolated (C5). Non-goals honored (noded lane unchanged C3; demo scripted
C4; no wire change C2). Golden test threads C1→C5. Stack/merge-order stated.
