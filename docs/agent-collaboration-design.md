# Consensus-Backed Agent Collaboration — Design

Status: as built (dissolution completed 2026-07-06; grounded in checked-in
code). Scope: how LLM agents collaborate deterministically over the consensus
log — the ordering contract the platform promises and the module architecture
that delivers it.

Principle: **no backwards compatibility.** This is an early-stage product.
Schema, wire, and root-preimage changes are flag-day changes on a fresh
genesis: no version bytes, no migration ops, no deprecated shims, no facade
or wire-surface preservation. Delete and replace.

## 1. The substrate (verified facts this design stands on)

- **Total order is real.** Simplex finalization orders sha256 frame digests;
  the ordered drain applies one frame per block with
  `BlockContext{height: view, consensus_time: view, origin}`.
- **`consensus_time` is a logical view number, not wall clock.** Deadlines and
  leases are view-denominated until real agreed timestamps land.
- **Intra-block cascades are atomic and ordered.** `ctx.emit_msg` follow-ups
  drain FIFO within one block (cap `MAX_DISPATCHES`); the host commits or
  aborts the whole touched set at the boundary.
- **Non-determinism only enters as ordered ops.** Effects surface
  post-finalization; a `reactor::Worker` does off-consensus work and returns a
  `Msg` submitted as an ordinary op (oracle-as-op). The worker can never
  mutate state directly.
- **Cross-module reads are consensus reads.** `ctx.query` routes through the
  host's live query routing, staged same-block writes included — so a module
  reading another module's state mid-block sees the same bytes on every
  validator.

## 2. The ordering contract

This is the contract the platform makes to agents. Everything in §3–§6 exists
to keep these seven promises cheap and the two non-promises honest.

**Promised:**

- **P1 — Global total order.** Every validator applies the identical op
  sequence with identical block coordinates.
- **P2 — Atomic causal cascades.** A root op and every follow-up it emits
  commit in one block or not at all. A user post → tag report → engagement
  delivery → pending entry → dispatch → saga trigger chain is one atomic
  unit. So is an agent registration and its dispatch recipe (the registry
  hook), and a watch and its tagging-plane subscription.
- **P3 — Per-channel monotonic sequence.** Message order within a channel is a
  deterministic, gap-free sequence assigned in-state at execute time.
  Happens-before within a channel = sequence order.
- **P4 — Anchored generation.** The ENTIRE model input is composed in
  consensus — the bounded transcript window ending at the anchor, the
  agent's prompt framing (pinned by the registered `prompt_hash`), and the
  strict output contract — and rides the dispatch as committed payload data.
  Any validator holds the exact prompt input as ordered state, and a reply is
  never presented as ordered before its anchor.
- **P5 — Result singularity.** Exactly one oracle result transitions a saga;
  duplicates and stale attempts are deterministic no-ops keyed by
  `(saga_id, attempt)`.
- **P6 — Callback adjacency, in dispatch-plane form: next-block delivery.**
  The saga's terminal transition and the dispatch module's contract-checked
  mailbox write commit in the result's block; the NEXT block's injected
  delivery hands the `ResultEvent` to the dispatcher, and the validated
  reply, the task writes, and a job-backed run's finalize all commit in that
  one delivery block (the never-pop-stack rule).
- **P7 — Deterministic deadlines.** Expiry, retry, and reassignment are
  ordinary ops gated by view-denominated deadlines; given the same op
  sequence, every validator times out identically. Liveness comes from
  permissionless crank submission; safety never depends on who cranks.

**Not promised (state these honestly, everywhere):**

- **N1 — No wall-clock latency bound.** A result lands in "some later block".
  No promise about which validator executes, or when.
- **N2 — No LLM determinism.** Validators agree on *the one finalized output*,
  never on reproducing it. Audit = input provenance (P4) + consensus
  laundering of the output.
- Also not promised: per-origin submission-order monotonicity within a round
  (key off in-state sequences, never send order); exactly-once worker
  *execution* (only exactly-once state transition); effect durability across
  a node crash (the deadline/crank path is load-bearing for liveness, not
  decoration); read-at-height external queries (audit re-derives context from
  the log, not live queries).

## 3. Architecture — the collaboration loop, dissolved

The original agent module stacked five jobs: registry, chat engagement
policy, run lifecycle, saga dispatch, and cross-module effect application.
Each landed in its proper home; what remains is a set of module-agnostic
planes plus two small agent-specific modules. Roles (all deterministic
except the driver):

| Piece | Role |
| --- | --- |
| `chat` | The collaboration surface: channels, threads, blocks; reports posts (mentions included) to the tagging plane |
| `tagging` | The engagement plane: content modules report tags, subscriber modules receive engagement events — router only, module-agnostic |
| `agent` | The REGISTRY, and nothing more: agent records (owner, capability tag, prompt pin, granted actions, status) + the formal response wire spec (`agent-interface`). 100% self-contained — no other module's interface crosses the crate |
| `runs` | The ACTOR: channel watches, run orchestration, in-consensus payload composition, response validation and delivery. Reads the registry by query; holds no registry state |
| `dispatch` | The task plane: consensus-registered recipes (capability, routing, output contract), saga-backed execution, contract judging, mailbox + next-block `ResultEvent` delivery |
| `capability` | Per-node capability announcements — the provider domain recipes route over |
| `saga` | The domain-agnostic async-RPC ledger underneath dispatch: one effect ↔ one result, leases, deadlines, retries |
| `jobs` | The work board: submitted jobs fan out to registered workers; the runs module claims `agent/{id}` jobs |
| dispatch-oracle (node) | The only impure piece: resolves a work spec's capability tag to a machine-local provider CLI, feeds the payload VERBATIM, submits the raw answer as an oracle op. No prompt composed, no output parsed, no credentials touched |

Two hard rules fell out of the dissolution:

- **The registry never references another module.** The agent crate imports
  only platform vocabulary (`sdk`, `SagaOrigin`, the capability tag shape
  rule). Its one seam is the registry hook: `RegisterAgent`/`UpdateAgent`
  emit an `AgentEvent` follow-up to a genesis-configured hook id (the runs
  module), which registers/retunes the agent's dispatch recipe in the same
  block. A squatted recipe id aborts the registration — record and recipe
  stay one atomic unit — without the registry knowing the dispatch plane
  exists.
- **The dispatcher composes the entire model input.** The runs module builds
  the full payload (prompt doc + window + contract) in consensus; the
  dispatch plane and the oracle treat it as opaque bytes.

**Agent identity.** Chat derives the module half of an
`AuthorRef::Agent{module, agent_id}` from the posting ORIGIN, so agents'
wire identity is `runs/{agent_id}` — the module that acts for them, not the
registry that records them. Mentions and tagging-plane `EntityRef`s use the
same ref, so mentioning a reply's author round-trips into an engagement.

The run flow — each ✦ is a consensus op (one block):

1. ✦ A human posts to a channel (`chat.PostMessage`). Chat appends the
   message (seq N) and reports the post — structured mention tags included —
   to the tagging plane; the plane delivers one engagement event per
   subscriber of that channel. Same block, atomic (P2).
2. The runs module receives the engagement (`Origin::Module(tagging)`
   enforced), applies the channel's turn policy (§5) against the registry
   (read by query — staged same-block registrations visible), and for each
   engaged agent: claims the turn (`chat\x1f{channel}\x1f{seq}\x1f{agent}`;
   duplicate = deterministic no-op), composes the FULL payload (P4), stages
   a pending correlation entry, and emits
   `dispatch.Dispatch{dispatch_id, recipe_id: agent/{id}, payload}` — still
   the same block.
3. The dispatch module resolves the recipe, stages the saga trigger
   (rendezvous over the capability's providers, or statically pinned), and
   the saga emits the `WorkerRequest` effect. Block commits: message + tag
   report + engagement + pending entry + dispatch + trigger are one atomic
   unit.
4. Off-consensus: the dispatch-oracle resolves the capability tag to a local
   provider, feeds the payload verbatim, and ✦ submits
   `saga.OracleResult{saga_id, attempt, outcome}` with the model's RAW text.
5. Saga transitions, and the dispatch module judges the recipe's output
   contract and commits the outcome into its MAILBOX — nothing reaches the
   runs module this block (never pop-stack).
6. ✦ The NEXT block injects the delivery. The runs module receives the
   `ResultEvent` (`Origin::Module(dispatch)` enforced), normalizes the raw
   text into an `AgentResponse` (deterministic string processing = consensus
   work), validates it deterministically — grants, caps, and probes for
   everything the follow-ups could make chat or tasks reject — and emits the
   cross-module writes: the chat reply (authored `as_agent`, threaded like
   its anchor), `tasks.*`, and a job-backed run's finalize. All in the one
   delivery block (P2, P6). The pending entry prunes; the dispatch module
   keeps the history.
7. Timeouts/stalls ride the saga's crank path (P7); a failed or expired
   dispatch delivers an `Err` ResultEvent through the same one result path.

The jobs lane rides the same plane: a submitted `agent/{id}` job is claimed
and dispatched in the submit cascade (compose BEFORE claiming — an
uncomposable job stays on the board), and the delivery block finalizes the
board item with the validated response. Job runs never carry reply blocks —
there is no channel to deliver them to.

## 4. The no-fail intake discipline (design §4 everywhere)

A module intake that rides another op's block must never `Err` — an error
aborts that block, and for a mailbox-backed delivery every retry aborts
again: the permanent-abort loop. The rules, as implemented:

- The engagement intake rides the user's posting block: malformed events,
  unwatched channels, failed pins, drifted prompt docs, oversized payloads —
  all staged no-ops plus an observability event.
- The result intake rides the delivery block: unknown dispatch ids are
  no-ops; a response that fails validation FAILS THE RUN (breadcrumb +
  pruned entry + job finalize), never the block. Anything an emitted
  follow-up could make chat or tasks reject is probed deterministically
  first — an emitted follow-up must be valid by construction (a squatted
  reply message id, a full thread, a duplicate task id).
- The jobs intake rides the submit block: same discipline; the single
  claiming-worker cascade rule makes the emitted claim safe.
- Saga- and chat-origin arms are dead-letter tombstones on BOTH agent-facing
  modules: any submitter can point a saga trigger's `reply_to` anywhere, and
  that callback must never abort the saga's terminal block.
- The ONE deliberate exception: the registry hook (`Origin::Module(agent)`
  at the runs module) MAY error — it rides the registry write's own block,
  and aborting that write is exactly the atomicity the recipe seam needs.

## 5. The registry and the runs module

**`agent` — the record book.**

- `AgentRecord{agent_id, owner (origin), display_name, capability,
  prompt_hash, allowed_actions, status}` — registration is an ordered op, so
  *which capability and prompt an agent runs is part of the app-hash* and
  auditable. `capability` names WHAT a run needs (an open-set registry tag);
  HOW it executes — binary, flags, model — is host policy in each provider's
  spec, invisible to consensus. Prompt CONTENT is content-addressed in the
  node's blob store under `prompt_hash`; consensus commits to the hash and the
  host resolves the content.
- Ops: `RegisterAgent`/`UpdateAgent`/`PauseAgent`/`ResumeAgent` (owner-gated
  by origin). Registration and capability changes notify the hook (§3).
- The response wire spec (`AgentResponse{reply_blocks, actions[]}`) lives in
  `agent-interface` as the formal contract; reply blocks are the spec's OWN
  vocabulary, mapped to chat blocks by the consumer at emission.
- Agent ids reject the reserved `\x1f` unit separator — the runs module keys
  run records with it.

**`runs` — the actor.**

- State: channel watches (`TurnPolicy` per channel, mirrored 1:1 with a
  tagging-plane subscription, staged atomically) and the pending map — one
  correlation entry per in-flight dispatch, holding exactly what acting on
  the eventual ResultEvent needs (where the reply goes, which job to
  finalize, who may cancel), pruned on delivery. Run LIFECYCLE is
  per-dispatched-task and lives in the dispatch module — never agent-owned.
- **Turn policy per watched channel**: `Mention` (structured mention tags
  naming `runs/{agent_id}` entities), `Assigned`, `RoundRobin(seq % n)`,
  `All`. Only ACTIVE registered agents engage. The run-id keyspaces claim
  turns: first creation in consensus order wins, later claims no-op.
- Ops: `WatchChannel`/`UnwatchChannel`, `EnableJobWorker`, `RequestRun`
  (explicit external invocation; rejects on failed preparation — it is the
  root op of its own block), `CancelRun` (requester- or owner-gated; cancels
  through the dispatch plane, and the plane's `Err("cancelled")` delivery
  prunes through the one result path — no second lifecycle machine).
- **Output validation is the safety boundary.** The response is data until
  every check passes; only then do its follow-ups exist. Fan-out is bounded
  per run (`MAX_ACTIONS_PER_RUN`).
- Both modules are state-based (BTreeMap + canonical-encoding root,
  saga-style) with snapshot/install joiner support; the snapshot IS the root
  preimage, and install re-derives the root before adopting.

## 6. Deliberately deferred

- Committee/quorum execution of high-stakes LLM effects (needs an output
  canonicalization/judging protocol that doesn't exist; leases + module-side
  validation are the honest v1).
- Real agreed timestamps (consensus_time = view is sufficient for ordering
  and leases-in-views; wall-clock deadlines need consensus timestamp wiring).
- Generalizing the recipe manifest layer (the `feat/agent-recipes` thread):
  richer what-to-run manifests — prompt as dispatch payload, output
  contracts beyond Text/Json, static binding — ride the existing dispatch
  recipe shape when they land.
