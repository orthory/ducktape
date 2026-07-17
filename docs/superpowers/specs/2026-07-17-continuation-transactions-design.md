# Continuation transactions — design

Date: 2026-07-17. Status: draft for review.

## Problem

Async module execution has a reentry gap. The canonical flow:

1. a user asks the chat module for agent work; the agent/runs plane
   dispatches the node's local harness through the worker seam
   (dispatch → saga → `WorkerRequest` → host worker, off-consensus);
2. mid-run, the harness decides it needs a NETWORK effect — trigger some
   other module's op (add a reaction on a chat thread, write a file, poke a
   page);
3. that op is fire-and-forget. Nothing brings control back to the run.
   The only built-in return path is the worker's own result
   (`SagaMsg::OracleResult` → dispatch's mailbox → the DISPATCHING module),
   which cannot express "after this unrelated chat op lands, resume run X."

Without a first-class answer, every multi-step flow needs bespoke session
state somewhere ("when the emoji lands, poke runs with run_id X"), or —
worse — pressure builds for a host-level synchronous cross-module call so a
module can just "do the thing and come back." That API is banned, forever.

## Doctrine (why this shape)

**The host seam never grows synchronous cross-module execution.** Today's
`Ctx` (crates/kernel/sdk/src/lib.rs) exposes exactly: `query` (read-only),
`emit_msg` (an async follow-up intent, re-dispatched by the host after
`execute` returns — never a reentrant call), and `emit_event` (leaves the
state machine). Dispatch's never-pop-stack rule
(crates/system/dispatch/src/interface.rs) is the same doctrine at the worker
seam: a result is NEVER returned into the requester's call path; it re-enters
as an ordered op. This spec extends the doctrine to its conclusion: ALL
transfer of execution between modules re-enters via a transaction.

The continuation field is what makes that doctrine livable. If every
cross-module step is its own transaction, "what happens next" must be DATA
carried by the envelope — composed and signed up front, inspectable at
admission, bounded by construction — not orchestration state living in a
module or a client.

## Decisions (locked)

- Every transaction envelope gains an optional **`continue`** field carrying
  one complete continuation body `(target, payload)`. The concept is called
  a **continuation**; `continue` is the wire-facing field name (Rust
  identifiers use `continuation` — keyword).
- **Depth is exactly 1**, enforced structurally: the continuation body has
  no `continue` slot of its own, so nesting is unrepresentable in the wire
  format — barred by shape, not by a validator's vigilance. Module payloads
  are opaque bytes and continuations exist only at the envelope layer, so a
  payload cannot smuggle one either.
- **Module-generic.** Nothing in the mechanism knows about agent, chat, or
  runs. It is a lower-level transaction-handling primitive, positioned BELOW
  saga and dispatch; those stay unchanged as the higher-level orchestration.
- The released continuation executes as a **module-sent transaction** — a
  new, segregated op class (below). It is never signed, never gossiped,
  never accepted from the wire.
- **The continuation always fires**: the parent applying, rejecting, or its
  deferred work failing all release the continuation, with the outcome
  relayed. Dropping it on failure would strand the very flow (agent
  reentry) the system exists for.
- The parent's outcome reaches the continuation through a **`Ctx`-visible
  relay slot**, never by splicing bytes into the opaque continuation
  payload.

## Envelope: op-frame v3

The wire frame (crates/kernel/node/src/lib.rs, `FRAME_NS`) goes
`v2 → v3`. The signed preimage gains one optional trailing section:

```
v2: len(origin) origin  seq  len(target) target  len(payload) payload  | sig
v3: …same…  cont_flag(u8)  [len(cont_target) cont_target
                            len(cont_payload) cont_payload]            | sig
```

- `cont_flag` is `0` (absent) or `1` (present); any other value fails to
  parse. Both arms are inside the signed preimage: **the signature binds the
  continuation to the parent op**, so nobody can graft a continuation onto
  someone else's transaction, and stripping one off invalidates the frame.
- `FRAME_NS` bumps to `ducktape:op-frame:v3` so no v2-era signature can
  verify under the new parse and vice versa.
- Caps: the whole frame stays under `MAX_FRAME_BYTES`. The continuation
  payload additionally has its own cap, `MAX_CONTINUATION_BYTES = 64 KiB`
  (the saga `MAX_REPLY_PAYLOAD_BYTES` precedent): a continuation is
  control-plane reentry, not a data lane — bulk bytes ride the parent op or
  the blob lanes. The cap also bounds committed pending-registry state
  (deferred lane below).
- Admission checks at the submit boundary: frame parses, caps hold. A
  continuation naming an unregistered target is NOT an admission error (the
  registry is consensus state; admission is not) — it rejects
  deterministically at dispatch time, like any op naming a missing module.

`decode_frame` continues to yield only `Origin::External(pubkey)` — a wire
frame can never claim module or system authorship. That invariant is
load-bearing for the next section.

## Module-sent transactions (the segregated op class)

A released continuation is dispatched as a **module-sent transaction**:

- `Origin::Module(parent_target)` — the module whose op completed is the
  SENDING LANE of the continuation. (The AUTHOR is someone else; see the
  authorization rule.)
- It is **derived, never submitted**: no signature exists or is checked.
  Its authenticity is consensus itself — it is computed deterministically by
  every validator from a committed parent op, exactly like the host's
  System-origin `DispatchMsg::DeliverPending` injection ("never submitted by
  anyone"). A network frame that tried to impersonate one cannot exist:
  `decode_frame` only ever produces `Origin::External`.
- It appears in the block's `DispatchRecord` trace and the indexer like any
  op — every cross-module transfer stays ordered, indexed, observable.
- **v1 scope**: continuation release is the ONLY producer of module-sent
  transactions. A module-sent transaction cannot itself carry a `continue`
  (it is not a wire frame; it has no envelope slot). Future producers
  (modules authoring fresh envelopes) are compatible with the class but out
  of scope — see Finiteness for why this restriction is the load-bearing
  half of the recursion bound.

## Execution semantics — the two lanes

Definitions. The **parent** is the op the envelope's `(target, payload)`
names. The **continuation** fires at the parent's **semantic completion**,
which is context-dependent — this is the section that formalizes it.

### Inline lane (default): parent completes in-block

For an ordinary op, semantic completion is the parent's terminal disposition
in its own consensus unit at height H: **applied** (execute returned `Ok`,
stage committed) or **rejected** (deterministic no-op, stage rolled back).

The host then dispatches the continuation **in the same consensus unit,
immediately after the parent's disposition settles**, as its OWN root-level
unit with batch-member isolation — NOT as a follow-up inside the parent's
stage. This placement is forced, not stylistic:

- a rejected parent's stage rolls back, and follow-ups of a rejected op
  never run — but the continuation must still fire (with the `Err` relay);
- a continuation that itself rejects is isolated exactly like a rejecting
  batch member (crates/kernel/host, `BatchOutcome`): the parent's committed
  stage survives. This is why same-unit release is safe here while
  dispatch's mailbox needed next-block delivery — there, delivery was a
  follow-up that could poison the block carrying the result; here the
  continuation is a separately-isolated member. The failure-domain goal of
  never-pop-stack is met by isolation instead of by delay.

The continuation dispatch counts against the block's `MAX_DISPATCHES`
budget like everything else. Depth 1 means it adds at most one extra
dispatch chain per envelope; it cannot ping-pong.

Relay outcome: `Ok(output)` where `output` is the parent's declared output
(see relay slot) — or `Err(reason)` carrying the parent's deterministic
rejection string.

### Deferred lane (module opt-in): parent's work completes off-chain

For a parent whose semantic work is asynchronous (it triggered off-chain
work through dispatch/saga), firing at end-of-execute would relay "I started
it," which is useless for reentry. The parent's target module — the only
component that KNOWS its op is async — claims the continuation instead:

1. **Defer.** During `execute`, the module calls
   `ctx.defer_continuation() -> Result<ContinuationTicket, Error>`. Errors
   if no continuation is present (the module can also see whether one is
   attached — a module may reject an op that arrives without the
   continuation its flow requires, or vice versa). The ticket id is
   DETERMINISTIC: `frame_id(parent) || defer-ordinal`, identical on every
   validator. The module stores the ticket next to its async state (its
   `dispatch_id` / saga id / run id).
2. **Stage.** A deferred continuation does not fire at H. After the parent
   settles (applied — a rejected parent's defer rolled back with its stage,
   and the inline lane fires with `Err` instead), the host injects a
   System-origin `ContinuationMsg::Stage` op into the **continuation
   registry module** (below), committing the pending continuation: ticket,
   `(target, payload)`, relay skeleton (author origin, parent target,
   parent frame id), and an absolute deadline view
   (`consensus_time + deadline_views`, defaulted and capped by consensus
   constants).
3. **Resolve.** When the async outcome lands — the module's saga callback /
   dispatch `ResultEvent` arrives, itself ≥1 block after the oracle result
   committed, per the existing machinery — the module emits (via
   `emit_msg`) `ContinuationMsg::Resolve { ticket, outcome }`.
   MODULE-ORIGIN ONLY, gated to the module that deferred. `outcome` is
   `Result<Vec<u8>, String>` under the same caps as the relay slot.
   Resolving early with `Err` is the cancellation path; there is no
   third-party cancel. Duplicate resolves are deterministic no-ops (first
   wins).
4. **Release — never-pop-stack.** The resolution commits at H′. The host
   injects System-origin `ContinuationMsg::Release` at the start of a LATER
   block's drain (≥ H′+1, the `DeliverPending` pattern verbatim), which
   dispatches up to `MAX_RELEASES_PER_BLOCK` (mirror
   `MAX_DELIVERIES_PER_BLOCK = 32`) resolved continuations as module-sent
   transactions — `Origin::Module(parent_target)`, relay slot populated —
   and deletes them from the registry. The remainder stays pending for the
   next block. A quiet chain is pumped by the existing `Nudge`-class no-op
   discipline: any successful block carries the injection.
5. **Expiry.** The `Release` injection also fires every pending
   continuation whose deadline has passed, with
   `Err("continuation_deadline")` — resolution-independent, so a module
   that defers and then wedges (or is swapped out) can never leak a pending
   continuation or strand its composer. Deadlines make the registry
   self-bounding alongside the 64 KiB payload cap.

Why not let the off-chain worker carry the continuation and submit it as
its own op when done? Because then reentry is only as reliable as one
node's process table: a crashed worker, a dropped lease hand-off, or a
malicious executor silently deletes the composer's "and then." Pending
continuations are COMMITTED STATE with crank-style expiry — the guarantee
that reentry happens (with SOME outcome) is exactly the point of the
system, so it must not depend on any off-consensus component behaving.

Determinism inventory for the lane: ticket ids are frame-derived; the
registry is a `BTreeMap` in a module root (part of the app-hash); stage,
resolve, and release are all ops or deterministic drain injections;
deadlines compare against consensus views, never wall clock. Nothing in the
lane consults node-local state.

## The relay slot

New `Ctx` surface, populated ONLY for a continuation dispatch:

```rust
/// present iff the current dispatch is a released continuation.
fn relay(&self) -> Option<&Relay>;

pub struct Relay {
    /// the AUTHENTICATED composer of the envelope: the parent frame's
    /// verified external key. Origin-typed for forward-compat with future
    /// module-authored envelopes; in v1 always `Origin::External`.
    pub author: Origin,
    /// the parent op's target module — the sending lane.
    pub parent_target: ModuleId,
    /// the parent frame's content id, for correlation.
    pub parent_frame: [u8; 32],
    /// the parent's outcome. `Ok`: the parent's declared output bytes
    /// (empty unless the module set one). `Err`: the deterministic
    /// rejection string, or the deferred resolution's error.
    pub outcome: Result<Vec<u8>, String>,
}
```

The `Ok` arm's source is a second small `Ctx` addition:
`set_output(&mut self, bytes: Vec<u8>)` — an op's declared output, staged
with the op (rolled back on rejection), capped at
`MAX_OUTPUT_BYTES = 256 KiB` (= saga `MAX_RESULT_BYTES`). Modules that
relay nothing pay nothing: the default output is empty. For the deferred
lane the resolving module passes the outcome explicitly in `Resolve`.

The `Err` arm carries the parent's deterministic rejection string, capped
at saga's `MAX_ERROR_BYTES = 16 KiB`. Note the bounded doctrine delta: for
plain frames a rejection reason stays node-local observability
(`DrainedFrame::reason` — never sealed or hashed); for a
continuation-carrying frame the reason becomes consensus input by
construction, because it rides a consensus op. Saga's `Failed` arm already
commits error strings to a root preimage — same class, same caps. Modules
should keep reasons stable snake_case tokens (the logging doctrine's rule),
which also keeps this arm small and greppable.

## The authorization rule (the security core)

**Attaching a continuation grants nothing.** The invariant every target
module must hold:

> handling payload `P` as a continuation authored by `O` requires exactly
> the authorization of `P` submitted directly by `O`.

The continuation dispatch's `Origin` is `Module(parent_target)` — that is
the LANE, useful for tracing and rate policy, and deliberately segregated.
The AUTHOR — the identity that composed and signed the envelope — is
`relay.author`, and it is authenticated (it is the parent frame's verified
signature origin, carried by the host, never module-supplied bytes).

To make the right thing the easy thing, the sdk adds:

```rust
/// the identity to authorize against: relay.author for a continuation
/// dispatch, the dispatch origin otherwise. one call, correct in both.
fn author_origin(&self) -> &Origin;
```

Corollary for MODULE-ORIGIN-ONLY op arms (e.g. `DispatchMsg::Dispatch`,
`SagaMsg` resolve paths): a continuation is EXTERNAL-authored work in a
module lane, so those arms must reject it. The gate is one check — the arm
requires `Origin::Module` AND `ctx.relay().is_none()`; the sdk ships a
`require_module_origin(ctx)` helper enforcing both so no module hand-rolls
it wrong. Without this corollary, `continue` would be privilege escalation:
any external key could reach module-origin-only arms by bouncing off an
innocent parent op.

## Worked examples

**Inline — harness reentry (the motivating n+1 loop).** A run's harness,
holding its session key, wants a chat reaction and then to keep working:

```
frame {
  target:  chat,  payload: AddReaction { thread, emoji, … },
  continue: { target: runs, payload: ResumeRun { run_id, … } },
}   signed by the run session key
```

Block H: chat applies `AddReaction` (sync op — inline lane). Same unit,
next member: runs receives `ResumeRun` as a module-sent transaction,
`Origin::Module("chat")`, relay `{ author: session-key, outcome: Ok(ack) }`.
Runs authorizes the SESSION KEY against `run_id` (the authorization rule —
the chat lane grants nothing), then re-dispatches the harness through
dispatch/saga as it already does. One envelope, guaranteed reentry, zero
session state outside the run record. Each further round trip is a fresh
externally-signed envelope: n steps = n envelopes, each depth-1.

**Deferred — notify on completion.** A user starts a run and wants the
reply posted back to a thread:

```
frame {
  target:  runs, payload: StartRun { … },
  continue: { target: chat, payload: PostReply { thread, … } },
}   signed by the user key
```

Runs knows `StartRun` is async, so during execute it defers → ticket
staged with the pending run. Blocks pass; the harness executes; the
oracle result lands; dispatch's mailbox delivers the `ResultEvent` to runs
at H′. Runs resolves the ticket with the run outcome. At ≥ H′+1 the
release injection dispatches `PostReply` to chat with
`relay.outcome = Ok(run output)` — or `Err(saga timeout)` if the run died,
because the continuation fires EITHER WAY. Fire-at-execute would have
posted the reply before the run existed; this is why semantic completion
is module-declared, not host-guessed.

## Finiteness

- **Intra-envelope**: depth 1, structural. An envelope is at most two
  dispatch chains (parent + continuation), both under `MAX_DISPATCHES`.
- **Cross-envelope**: a continuation cannot carry a continuation, and in v1
  the continuation lane is the ONLY producer of module-sent transactions —
  so the transaction layer cannot self-perpetuate. Every chain is rooted in
  an externally-signed frame; continuing a flow past two hops requires a
  fresh external decision (the harness signing again), which is precisely
  the n+1 recursion the system wants: finite, attributable rounds.
- Existing loop vectors are unchanged and stay individually bounded:
  `emit_msg` fan-out (dispatch budget), saga retries (attempts/deadlines).
  The continuation system adds no unbounded vector. This invariant is WHY
  future module-authored envelopes are out of scope: lifting the
  v1 producer restriction reopens cross-envelope recursion and must arrive
  together with its own budget design (lineage counters or quota), not as a
  quiet extension.

## The continuation registry module

A new, tiny system module, id `continuation` (a genesis-constant the host
knows, like `DISPATCH_MODULE_ID`). It exists because pending continuations
are committed state and all committed state lives in a module root — the
host holds no hidden state. Surface:

- `Stage { ticket, target, payload, relay_skeleton, deadline }` —
  SYSTEM-ORIGIN ONLY, host-injected when a settled parent deferred.
- `Resolve { ticket, outcome }` — MODULE-ORIGIN ONLY, gated to the
  deferring module recorded on the ticket. First resolve wins; duplicates
  and unknown tickets are deterministic no-ops.
- `Release {}` — SYSTEM-ORIGIN ONLY, drain-injected when resolved or
  expired pendings exist; dispatches up to `MAX_RELEASES_PER_BLOCK`, plus
  deadline expiry. Released tickets are deleted — GC is intrinsic, no
  retention sweep.
- `ContinuationQuery::PendingReleases` — the injection's read, the
  `DispatchQuery::PendingDeliveries` analog.

The module depends on nothing above the sdk (not saga, not dispatch) —
keeping it the LOWER-level primitive the positioning demands. Two
placement rules follow from the existing machinery:

- The `Release` injection joins the host's existing generic injection
  family (upgrade `Advance`, modreg `Advance`, dispatch `DeliverPending` —
  bin/node/src/host_state.rs): keyed ONLY on committed state via the
  query above, and INERT until the module is registered — a net without
  the module never injects anything.
- The module stays **native**, joining the machinery-gating coordinator
  set (dispatch, upgrade, modreg, valset), for dispatch's exact reason:
  the host's injection read must serve COMMITTED-ONLY state, and the
  module is part of the transaction machinery itself rather than a tenant
  of it.

## Wasm guest compatibility (verified against the cutover)

The post-cutover guest world (crates/kernel/module-guest/wit/module.wit)
is fully SYNCHRONOUS: a guest cannot suspend mid-execute; host reads
bridge by trap-and-replay memoization over a pure `execute`
(crates/kernel/wasm-host/src/lib.rs), with bounded distinct reads and a
never-parking poll executor in the guest adapter. The continuation design
was checked against this and is clean — it demands NO new async ability:

- `defer_continuation` and `set_output` are INTENTS, the `emit-msg`
  class: sync-shaped calls whose effects are collected by the host and
  applied only from the final (post-replay) run. Idempotent across
  replays by the same rule that already governs emitted msgs; the defer
  ordinal that feeds ticket ids counts only the final run's calls.
- `relay` / `author_origin` are DISPATCH INPUTS, the `Env` class:
  read-only values fixed before execute starts, trivially memo-safe.

So the three additions are two new intent imports and one input in the
WIT world — no suspension, no clock, no nondeterminism, and native and
wasm modules see identical semantics. Modules never had effectful awaits
to lose (the sdk contract already restricts every await to deterministic
resources), and this system leans on that: semantic completion of async
work re-enters as ops (saga callbacks, result events), never as a guest
awaiting the future.

## Observability

Continuation dispatches ride the existing `DispatchRecord` trace and
indexer rows (origin `Module(parent_target)`, payload visible). Logging per
the doctrine: `target: "ducktape::continuation"`; contract events
`event = "continuation_staged" | "continuation_released" |
"continuation_expired"` — staged/released at `debug` (per-op), expired at
`warn` with `reason = "continuation_deadline"`. Never log payloads.

## Rollout

Frame v3 acceptance and continuation semantics gate on a protocol version
via the upgrade module (the no-downtime, height-gated path — see
2026-07-04-no-downtime-node-upgrade-design.md). Before activation,
admission rejects any frame whose `cont_flag ≠ 0`.

1. **Phase 0** — types + codec + admission-reject (v3 parse lands, feature
   fenced). Gates green.
2. **Phase 1** — inline lane: host release, relay slot, `set_output`,
   `author_origin`, batch-member isolation tests.
3. **Phase 2** — continuation registry module + deferred lane
   (defer/stage/resolve/release/expiry), drain injection.
4. **Phase 3** — adoption: runs/chat wire the two worked examples; harness
   composes reentry envelopes under its session key.

## Not in scope

- Module-authored envelopes (modules minting new PARENT transactions) and
  any second producer of module-sent transactions — blocked on a
  cross-envelope budget design (see Finiteness).
- Multi-continuation fan-out (`continue` is one body, not a list).
- Changes to saga/dispatch semantics, leases, or the provider contract.
- Any UI surface.

## Testing

- **Codec**: v3 round-trip; signature covers the continuation (mutating
  either continuation field fails verification); `cont_flag` ∉ {0,1} fails
  parse; caps enforced at submit; v2 signatures never verify under v3.
- **Inline lane**: applied parent → continuation fires same unit with
  `Ok(output)`; rejected parent → fires with `Err(reason)`; rejecting
  continuation is isolated (parent's commit survives, app-hash reflects
  both dispositions); `MAX_DISPATCHES` accounting includes the
  continuation chain.
- **Deferred lane**: defer → stage on parent settle; rejected parent's
  defer vanishes with its stage (inline `Err` fires instead); resolve →
  release ≥1 block later; duplicate/foreign resolve no-ops; expiry fires
  `Err` past deadline with no resolve; registry drains to empty (GC).
- **Authorization**: module-origin-only arm rejects a continuation
  dispatch; `author_origin` returns the composer for continuations and the
  submitter otherwise.
- **Determinism**: two hosts fed identical frames produce identical
  app-hashes and `DispatchRecord` traces across both lanes, including
  ticket ids and release order.
- **Gating**: pre-activation frames with continuations reject at
  admission; the flag-day boundary is height-gated and app-hash-continuous
  (upgrade-lane test pattern).
