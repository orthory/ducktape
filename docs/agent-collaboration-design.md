# Consensus-Backed Agent Collaboration — Design

Status: proposed (full-module review 2026-07-02; grounded in checked-in code).
Scope: how LLM agents collaborate deterministically over the consensus log —
the ordering contract the platform promises, the module architecture that
delivers it, and the module-by-module dispatch plan to build it.

Principle: **no backwards compatibility.** This is an early-stage product.
Schema, wire, and root-preimage changes are flag-day changes on a fresh
genesis: no version bytes, no migration ops, no deprecated shims, no facade
or wire-surface preservation. Delete and replace.

## 1. The substrate (verified facts this design stands on)

- **Total order is real.** Simplex finalization orders sha256 frame digests;
  `SimplexReporter` buffers `(view, bytes)` into a `FinalizedInbox` and
  `poll_delivered` releases the longest all-ready prefix in ascending-view
  order. `OrderedNode::drain_delivered` applies one frame per block with
  `BlockContext{height: view, consensus_time: view, origin}`
  (`crates/kernel/node/src/lib.rs:477-502`).
- **`consensus_time` is a logical view number, not wall clock.** Deadlines and
  leases must be view-denominated until real agreed timestamps land.
- **Intra-block cascades are atomic and ordered.** `ctx.emit_msg` follow-ups
  drain FIFO within one block (cap `MAX_DISPATCHES = 1024`); the host commits
  or aborts the whole touched set at the boundary
  (`crates/kernel/host/src/lib.rs:297-329`).
- **Non-determinism only enters as ordered ops.** Effects surface
  post-finalization (`OrderedNode::take_effects`); a `reactor::Worker` does
  off-consensus work and returns a `Msg` submitted as an ordinary op
  (oracle-as-op). The worker can never mutate state directly.
- **saga is the only effect user today** and is single-shot:
  Trigger→Pending(+`WorkerRequest` effect)→OracleResult→Done, first-result-wins
  idempotent. It has **no requester callback, no lease, no deadline, no
  cancel, no GC** (`crates/system/saga/src/lib.rs:237-271`).
- **agent and chat are transcript facades over messaging** with no agent or
  product semantics. messaging stores each channel's whole history as one JSON
  blob under one key with a decode-only 1 MiB cap — a poison-pill and an O(n)
  rewrite per post (`crates/apps/messaging/src/lib.rs:97,283-296`).
- **`Env.origin` is available but unused.** No module authenticates authorship;
  frames carry attacker-chosen origin bytes with no signature binding
  (`crates/kernel/node/src/lib.rs:334-339`).
- **The reactor is not wired into the live node.** `bin/node`'s pump loop never
  calls `take_effects` (`bin/node/src/main.rs:301-321`).

## 2. The ordering contract

This is the contract the platform makes to agents. Everything in §3–§7 exists
to keep these seven promises cheap and the two non-promises honest.

**Promised:**

- **P1 — Global total order.** Every validator applies the identical op
  sequence with identical block coordinates. (Simplex finalization + ordered
  drain; proven by `simplex_agreed_order.rs`.)
- **P2 — Atomic causal cascades.** A root op and every follow-up it emits
  commit in one block or not at all. A message → hook notification → run
  creation → saga trigger chain is one atomic unit.
- **P3 — Per-channel monotonic sequence.** Message order within a channel is a
  deterministic, gap-free sequence assigned in-state at execute time.
  Happens-before within a channel = sequence order.
- **P4 — Anchored generation.** Every LLM output is pinned to the exact
  transcript prefix it saw: `ContextPin{channel_id, up_to_seq, context_hash}`
  rides in the effect spec and the run record. The platform never presents an
  agent response as ordered before its anchor, and any validator can re-derive
  the prompt input (not the output) from the log.
- **P5 — Result singularity.** Exactly one oracle result transitions a saga;
  duplicates and stale attempts are deterministic no-ops keyed by
  `(saga_id, attempt)`.
- **P6 — Callback adjacency.** The requesting module learns a saga's terminal
  outcome in the same block the result lands (FIFO follow-up drain).
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
  (the frame sort is byte-lexicographic — key off in-state sequences, never
  send order); exactly-once worker *execution* (only exactly-once state
  transition); effect durability across a node crash (the deadline/crank path
  is load-bearing for liveness, not decoration); read-at-height external
  queries (audit re-derives context from the log, not live queries).

## 3. Architecture — the collaboration loop

Roles (all deterministic except the driver):

| Piece | Role |
| --- | --- |
| `chat` (messaging v2) | The collaboration surface: channels, threads, blocks; channel **hooks** notify subscriber modules of posts |
| `agent` v2 | The orchestrator: agent registry, run lifecycle, turn policy, plan chaining, LLM-output validation |
| `saga` v2 | The domain-agnostic async-RPC ledger: one effect ↔ one result, with leases, deadlines, retries, callbacks |
| reactor driver (node) | The only impure piece: drains finalized effects, runs the leased worker, submits results as ops |
| `valset` / epoch membership | The executor-assignment domain (after membership ops are authenticated) |

The run flow — each ✦ is a consensus op (one block):

1. ✦ A human posts to a channel (`chat.PostMessage`). Chat appends the message
   (seq N), then emits one follow-up `Msg` per registered channel hook —
   same-block, atomic (P2).
2. The agent module receives the hook notification (`Origin::Module(chat)`
   enforced), applies the channel's turn policy (§5), and for each engaged
   agent creates a `RunRecord` with `run_id = f(channel, anchor_seq,
   agent_id)` (duplicate = deterministic no-op — the turn claim), computes the
   `ContextPin` by querying the transcript prefix (staged writes visible —
   deterministic), and emits `saga.Trigger{spec: LlmRequest{run_id, agent_id,
   config_hash, ContextPin}, reply_to: "agent", reply_payload: run_id,
   deadline, max_attempts}` — still the same block.
3. Saga stages the pending record, computes the lease
   (`executor = members[H(saga_id ‖ attempt ‖ height) % n]`), and emits the
   `WorkerRequest{saga_id, attempt, spec, deadline, assignee}` effect.
   Block commits: message + run + pending saga are one atomic unit.
4. Off-consensus: every validator's driver sees the effect; only the assignee
   executes (others hold it as a lease-watch). The worker re-derives the
   context from its replica, **verifies it hashes to `context_hash`**, calls
   the LLM, and ✦ submits `saga.OracleResult{saga_id, attempt, outcome}`.
5. Saga validates (pending, attempt matches, origin == assignee under strict
   policy), transitions to Done, and emits the callback
   `Msg{target: "agent", SagaCallback{saga_id, reply_payload, outcome}}` —
   same block (P6).
6. The agent module decodes the `AgentOutput{reply_blocks, actions[]}`,
   validates it deterministically (schema, size caps, per-agent allowed-action
   set), and emits the cross-module writes: `chat.PostMessage` (authored as
   the agent — `Origin::Module("agent")` + `AuthorRef::Agent(agent_id)`),
   `tasks.*`, `document.*`, or the next `saga.Trigger` for multi-step plans.
   All in the same block as the result (P2, P6).
7. Timeouts/stalls: any node's driver ✦ submits `saga.Crank`; saga
   deterministically expires past-deadline leases (reassign, `attempt+1`) or
   fails the saga (callback fires, agent marks the run Failed and may post an
   error notice).

Multi-agent pipelines (planner→worker→reviewer) are **requester-driven
chains**: the agent module's callback handler emits the next Trigger. Fan-out
is N triggers with `reply_payload = {plan_id, step, branch}`; fan-in is a
decrement-and-join in agent state. Saga never learns plan semantics.

Prior-art shape: Temporal's workflow/activity split (agent execute = the
deterministic workflow function over recorded results; saga+driver = activity
with durable timers), CosmWasm's submessage/reply (`reply_to` +
`reply_payload` echo), Autonolas' off-chain-compute/on-chain-agree, and the
`wireguard-upgrade` patterns already in-tree (domain-separated namespaces,
consensus-context binding, hash-chained request/response lineage).

## 4. Saga v2 — the async-RPC ledger

Stays **single-shot and domain-agnostic**: one effect, one agreed result. All
plan/branching semantics live with the requester. (Both design probes and the
architect converged here independently; a plan-tree in saga would duplicate
the agent module and still not solve turn-taking.)

State per saga:

```
Saga {
  origin: SagaOrigin,            // canonical mirror of sdk::Origin; cancel auth
  reply_to: Option<ModuleId>,    // callback target, validated at Trigger time
  reply_payload: Vec<u8>,        // opaque requester correlation (echoed back)
  spec: Vec<u8>,                 // opaque work spec (e.g. LlmRequest)
  status: Pending | Done | Failed | TimedOut | Cancelled,
  attempt: u32, max_attempts: u32,
  assignee: Vec<u8>,             // lease holder (executor pubkey)
  lease_expires_at: u64,         // view-denominated
  deadline: Option<u64>,         // absolute view; whole-saga bound
  result: Option<Vec<u8>>, error: Option<String>,
  updated_at: u64,
}
```

Ops: `Trigger` (duplicate `saga_id` = no-op — today it silently resets and
re-fires the worker, a bug), `OracleResult{saga_id, attempt, outcome:
Result<Vec<u8>, String>}` (first per attempt wins; `Err` consumes an attempt
and re-leases while `attempt < max_attempts`), `Crank` (permissionless,
bounded batch: expire leases → reassign or fail; fire past-deadline
timeouts), `Cancel` (origin-gated to the trigger origin), `Prune`
(origin-gated removal of terminal sagas + lazy retention sweep).

Design rules learned from the probes:

- **Callback-poison rule.** The saga terminal transition and the requester
  callback commit in the same block; a callback that errors aborts the
  finalized block, which replays as a deterministic no-op — wedging the saga
  at Pending *forever*. Therefore: `reply_to` is validated against
  `ctx.module_root` at Trigger time, and requester callback arms must be
  no-fail by construction (decode failure = staged no-op + event). Pin this
  with tests.
- **Lease policy is genesis config.** `LeasePolicy::Strict` (result accepted
  only from `assignee`'s verified origin) for live nets;
  `LeasePolicy::Open` (current first-wins behavior) for demo/tests. Strict is
  security theater until frames are signature-verified (§7 precondition) —
  ship the field now, enforce when auth lands.
- **Assignment domain = admitted/epoch membership**, not the raw
  permissionless valset (Sybil risk). Until valset ops are authenticated,
  Open policy + first-wins is the honest default.
- Result blobs get a size cap (consensus constant) so LLM outputs don't
  bloat the root preimage and joiner snapshots. No version byte or migration
  machinery — encoding changes are flag-day (see principle).

## 5. Agent v2 — registry, runs, turns

The agent module stops being a messaging facade and becomes a real module:

- `AgentRecord{agent_id, owner (origin), display_name, model_ref,
  prompt_hash, config_hash, allowed_actions, status}` — registration is an
  ordered op, so *which model and prompt an agent runs is part of the
  app-hash* and auditable. Prompt/config content may live in `document`;
  the hashes are what consensus commits to.
- `RunRecord{run_id, agent_id, trigger: Hook{channel, anchor_seq} |
  Explicit{origin}, status: Pending | AwaitingOracle(saga_id) | Done | Failed
  | Cancelled, context_hash, created_at, updated_at}`.
- Ops: `RegisterAgent`/`UpdateAgent`/`PauseAgent` (owner-gated by origin),
  `OnMessage` (accepted only from `Origin::Module(chat)`), `RequestRun`
  (explicit external invocation), `HandleSagaCallback` (accepted only from
  `Origin::Module(saga)`), `CancelRun`.
- **Turn policy per channel hook**: `Mention` (agents named in message blocks
  — mentions are structured spans, so parsing is deterministic), `Assigned`,
  `RoundRobin(anchor_seq % n)`, `All`. For externally-raced claims,
  `run_id = f(channel, anchor_seq, agent_id)` dedup makes the first claim in
  consensus order win; later claims are no-ops. No randomness, no clock.
- **Output validation is the safety boundary.** `AgentOutput{reply_blocks,
  actions[]}` is data until the agent module deterministically validates it
  (schema, caps, `allowed_actions`) and emits the cross-module msgs. Fan-out
  is bounded per run (MAX_DISPATCHES blast-radius rule).
- Transcripts stay in chat channels (agent sessions are channels); the agent
  module holds only registry/run state. First slice state-based
  (BTreeMap + canonical-encoding root, saga-style); qmdb later if needed.

## 6. Chat v2 — block-based Slack-parity model (messaging)

Per-record layout over qmdb (hashed keys ⇒ no range scans ⇒ pagination is
computed-key point lookups; `get_many` batches them):

```
channel/{id}              -> Channel{name, created_at, head_seq, post_policy, hooks[], pinned[]}
msg/{chan}/{seq:be8}      -> MessageHead{message_id, author: AuthorRef, blocks: Vec<Block>,
                              created_at, rev: u32, edited_at, deleted: bool,
                              thread: Option<root_seq>, reply_count, last_reply_seq}
rev/{chan}/{seq}/{rev:be4} -> prior revision (immutable edit history)
react/{chan}/{seq}/{emoji} -> BTreeSet<AuthorRef>
msgid/{message_id}         -> (chan, seq)   // global dedup + O(1) thread-root lookup
member/{chan}/{user}, memberidx/{chan}
```

- **Author derives from `Env.origin` — the `author` payload field is gone.**
  `AuthorRef::User(pubkey) | Agent(agent_id via Origin::Module("agent")) |
  System`. Empty external origin is rejected (the demo's
  `Origin::External(vec![])` default must not pass as an authenticated user).
- **Blocks**: minimal enum — `Paragraph(spans)`, `Code{lang}`, `Quote`,
  `Divider`; spans carry marks (bold/italic/link) and `Mention(AuthorRef)`.
  Mentions being structured is what makes agent turn-triggering
  deterministic.
- **Edits**: head is last-applied-by-total-order (LWW); every edit appends an
  immutable `rev` record; `base_rev` is recorded, not rejected (only the
  author may edit their own message, so conflicts are same-author multi-device
  races — Slack semantics). Revision cap per message.
- **Deletes**: tombstones — content cleared, skeleton (seq, thread linkage,
  reply_count) preserved so thread integrity and P3 survive.
- **Reactions**: set semantics per (emoji, author) — idempotent add, exact
  remove.
- **Hooks**: a channel carries registered hook module ids (admin-gated op);
  `PostMessage` emits one follow-up `Msg` per hook. Chat stays agent-agnostic
  — hooks are the Slack-webhook seam, and the agent module is just one
  subscriber.
- **Caps enforced at write time** (the poison-pill lesson: the 1 MiB codec cap
  is decode-only — an oversized value commits, then panics every validator on
  the next read). Message ≤ 64 KiB serialized, bounded query limits,
  channel-index pagination later.
- **Facades: deleted, not preserved.** `messaging`/`messaging-interface`
  fold into `chat`/`chat-interface` — one storage-owning module, id `chat`.
  The `agent` facade and its session wire surface are deleted in Wave 1
  (agent v2 in Wave 3 is a new module, not a wrapper). All dual-mode
  machinery (registered-backing, embedded split) goes with them — facade
  hops erase the submitter origin (`Origin::Module(facade)`), which is
  incompatible with authenticated authorship anyway. Clients submit
  directly to `chat`.
- Migration: none — fresh genesis (see principle).

## 7. Kernel preconditions (from the full-module review)

Ranked; the first three gate the agent protocol's security story, the rest
gate production quality. Full per-module detail lives in the review corpus.

1. **Frame authentication + per-origin seq enforcement** — origin bytes are
   attacker-chosen and seq is decorative today; every ACL, lease, and
   authorship rule upstream depends on this
   (`crates/kernel/node/src/lib.rs:334-339`).
2. **Valset membership authentication** — proof-of-possession on Join, signed
   Leave; today anyone can register/evict any key
   (`crates/system/valset/src/lib.rs:228-235`). Prerequisite for lease
   assignment over membership.
3. **Live reactor driver** — wire `take_effects` → workers →
   `OrderedNode::submit` into `bin/node`'s pump loop with per-node seq
   allocation and the lease filter (`bin/node/src/main.rs:301-321`).
4. **Resolver backstop in bin/node** — `spawn_with_relay` silently drops a
   finalized digest whose gossip was missed: permanent fork.
   `spawn_with_resolver` exists and is tested; switch. [S]
5. **Write-time size caps everywhere** (kv, messaging, document) — the
   poison-value crash vector. [S each]
6. **Commit-phase policy** — close the half-commit hole (`host/src/lib.rs:309`
   propagates `?` mid-loop; abort errors swallowed at :323): choose
   halt-don't-fork, document it, pin with a Boom-module test. [M]
7. **`Host::query` env** — zeroed Env with `Origin::System` hands untrusted
   RPC callers the most privileged origin; thread last-committed height/time +
   unprivileged origin, return the height. [S]
8. Unbounded memory (FinalizedInbox/ContentStore/`OrderedNode.effects`),
   ascending-view assertion, applied-height watermark + restart replay
   contract, op batching per frame, shared app-definition crate (three
   binaries hardcode three different registries), epoch `app_height` mapping.

## 8. Dispatch plan (module by module)

Each wave is independently shippable; items within a wave are parallel.

**Wave 1 — now**
- **W1a chat v2** (§6): fold messaging into chat, delete the agent facade
  and all dual-mode machinery; per-record model, blocks, edits, tombstones,
  reactions, origin authorship, hooks, pagination, write-time caps,
  per-channel counter. Independent of everything else. [L]
- **W1b kernel quick wins**: resolver backstop in bin/node; write-time size
  caps in kv/messaging/document; tasks `state_sync_handle` +
  snapshot-validation symmetry; docs staleness fixes (tasks-module claim,
  valset-orchestrator split, module inventory, mesh crates). [S×many]

**Wave 2 — after W1 merges**
- **W2 `saga` v2** (§4): callbacks, attempts, leases (Open policy default),
  Crank/Timeout/Cancel/Prune, version-byte encoding, callback-poison tests. [M/L]

**Wave 3 — after W2**
- **W3a `agent` v2** (§5): registry, runs, hooks intake, turn policy,
  ContextPin, AgentOutput validation, plan chaining. [L]
- **W3b reactor driver in bin/node** (§7.3): take_effects pump, LlmWorker +
  DeadlineWorker (crank timer), per-node seq. [M]

**Wave 4 — hardening + depth**
- Frame auth + seq enforcement; valset op authentication; commit-phase
  policy; Host::query env; memory bounds; watermark/restart contract. [M×many]
- `tasks` v2 (schema, origin attribution, idempotent conflict semantics,
  qmdb) — the agents' shared work ledger. [M/L]
- `document` (register it, origin ACL, CAS versioning, delete, events) and
  `forge` (replay idempotence, nested paths, RefUpdate wire path) depth. [L]

## 9. Deliberately deferred

- Committee/quorum execution of high-stakes LLM effects (needs an output
  canonicalization/judging protocol that doesn't exist; leases + module-side
  validation are the honest v1).
- Real agreed timestamps (consensus_time = view is sufficient for ordering
  and leases-in-views; wall-clock deadlines need consensus timestamp wiring).
- CRDT text editing in `document` (anchor-based structural ops under total
  order first; keystroke-level merge is a later, explicit decision).
- Dynamic module install via governance op (module set is genesis-fixed;
  `Host::register`'s silent overwrite should be guarded meanwhile).
