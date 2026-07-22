# C-stage — Block-Apply Seam & Full simnode Reassembly

**Status:** implemented (stacked PRs, unmerged) — design approved in-session
2026-07-22 (full-OrderedNode depth chosen explicitly; honesty-invariant flag day
approved). Delivery = stacked PRs #724–#728 (C1→C5) on `feat/c*`, review,
**never merged by the assistant**.
**Base:** the B campaign's PRs (#715–#722) are open; C stacks on the pushed
integration base `integration/layer-contracts` (current dev + all eight B
branches). When B lands on dev the stack retargets mechanically.
**Evidence:** `.superpowers/sdd/c-stage-duplication-map.md` (precise per-lane
step tables with anchors) — the plan's tasks cite it instead of repeating it.

## Goal

Kill the three-lane duplication the B inventory exposed: the block projection
pipeline (validator / noded / simnode), the worker drive loop (×3), the genesis
composition (×4 sites + hand-copied id lists), and the status projection (×2) —
by reassembling simnode ON `OrderedNode` so daemon parity becomes a fact of
shared code instead of a promise kept by parity tests.

## Design decisions (all approved)

1. **`StepOrderer`** (crates/kernel/node, beside `RoundOrderer`): submissions
   park; release is **FIFO** (RoundOrderer's byte-sort would reorder scripted
   scenarios); an external `StepHandle` commands "release one" / "release all"
   (auto mode); views stamp monotonically. Orderer-local concerns only — reply
   correlation, oracle-before-held interleave, batching, and the clock stay one
   layer up, per the duplication map §5.
2. **OrderedNode changes** (kernel, minimal): (a) the hardcoded
   `consensus_time = height` becomes a **named time policy** (default
   `HeightIsTime`; sim passes `Epoch{base_ms, block_ms}` — the logical clock);
   (b) a **sim-gated pre-decoded ingress** (`feature = "sim"`) accepting
   already-decoded `BlockOp`s for simnode's `/sim/peer-block`/`hex:` lanes —
   the wire codec is a machine contract and gains no unsigned variant; the
   client lane keeps real `decode_frame` verification.
3. **Block projection seam** in the noded crate (where `block_row` already
   lives): one `project_block` path covering RootOp assembly (generalizing
   `explorer_root_op`), row bytes, index feed, and stream publish — consumed by
   validator drain, replica park, noded submit lane, and simnode. Member-then-
   System ordering comes from today's `drain_actions::block_actions` verbatim.
   A **golden test** pins row bytes before/after. The noded `dispatch_info`
   divergence (`external:<name>` vs flattened `external`) is resolved to the
   validator's shape — one projection, one shape.
4. **One reactor drive loop** in `host::worker`: a pure drive fn (offer events
   to workers, budget rounds, return follow-up `Msg`s + Nudge tail); each lane
   applies the follow-ups through its own submit path. The three copies die.
5. **Genesis topology single source**: one `ModuleTopology` describing the id
   set, wiring (chat→tagging, agent→files/pages, …), and genesis-config VALUES
   (NetworkBindings et al.), consumed by node's `ProductionModules` (wasm
   backend), simnode (native backend), and demo. Instantiation stays
   per-backend — wasm and native roots differ by design; one topology is NOT
   one app-hash. `BASE_MODULE_IDS`/`VALSET_MODULE_IDS` hand-copies and
   `[&str; N]` count annotations die; subsets become named selections validated
   against the topology (the #706 accident class becomes unrepresentable).
6. **simnode reassembly on `OrderedNode<StepOrderer>`**: the Sim actor keeps
   its public surface (`/sim/step`, auto mode, peer lanes, persona, EchoWorker,
   lib-safe halt, watermark resume, embeddable boot — the 14-item keep-list in
   the duplication map §6) but internally becomes flush → step-release → drain
   → shared projection. `NullSink` stays (no WAL; restart = qmdb + index
   watermark, as today).
7. **FLAG DAY (user-approved):** a rejected single op now journals a block,
   exactly like the validator. Sim-lane tests asserting the old
   no-block-on-reject behavior are updated in-campaign. No compat path.

## Non-goals

- No wire/codec change anywhere. No app-hash unification across backends.
- No changes to SimplexOrderer/consensus semantics; no mesh involvement.
- demo stays a scripted walkthrough (adopts topology, not OrderedNode).
- noded stays on its 1-op-1-block lane (adopts projection + reactor, not the
  orderer) — its wall-clock consensus_time is its lane's contract.

## Verification spine

- Projection golden test: identical inputs → byte-identical `block_row` output
  across old/new paths (pinned before the swap, held after).
- simnode e2e suite + embedded sim lane (iced) green, with the flag-day test
  updates isolated in their own commit for reviewability.
- Validator regression: `cluster_e2e cluster_lifecycle` (real processes).
- Genesis: `genesis_registry_matches_module_ids` upgraded to derive from the
  topology; simnode/demo composition checked against the same source.
- Full-workspace gate on the stack tip; pre-existing reds classified against
  the integration base, never chased.
