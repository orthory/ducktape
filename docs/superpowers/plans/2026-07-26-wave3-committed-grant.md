# Wave 3: make `service enable` the transaction, and shrink the announce pump to a liveness watcher

- **Date:** 2026-07-26
- **Status:** proposal — investigation complete, **nothing implemented**
- **Base read:** `origin/dev` @ `e0d773f68` (PR #818 merged; #820-#824 merged).
  Plus the open PR **#819** (`fix/announce-all-service-kinds`, `f01739573`).
  Every file:line below was read in that tree.
- **Predecessors:** `2026-07-25-services-extraction.md` (wave 1),
  `2026-07-25-service-daemons.md` (wave 2), `2026-07-26-wave3-announcement.md`
  (the on-chain survey this plan acts on).

---

> **Revision note (same day, after review).** The first draft of this plan
> recommended deleting the pump outright and claimed there was "no honest
> half-measure" for the crash-retraction problem. **That claim was wrong on two
> counts**, and a measured reproduction (§4B) narrowed the problem to one
> configuration. The recommendation has changed: §4C-§4G now propose a
> **~95-line liveness watcher** in place of the ~840-line pump, and §4H states
> what I got wrong and why. §0, §3 and §10 reflect the revised shape.

## 0. The shape, in one page

The wave-2 design says: daemon signals → operator reviews → **a transaction
follows**. What shipped is a local file write (`commit_enable`,
`bin/node/src/services.rs:990-1030`) plus a background reconcile loop
(`bin/node/src/validator/announce.rs`, `bin/node/src/resident_announce.rs`)
that discovers the file change on its own and submits the on-chain half later.

**The proposal: `service enable`/`disable` submit the `capability::Announce`
themselves, and the ~840-line reconcile loop is replaced by a ~95-line
liveness watcher that submits only on an alive/dead transition.**

Consent and liveness are two different facts — which is exactly why today's
pump computes `grant ∩ live hello` rather than announcing the grant. The
change is not to merge them; it is to stop routing the *consent* half through
a background loop that cannot see it, and to leave the *liveness* half with a
watcher small enough to be obviously correct. One committed record, one submit
door, two triggers.

Three facts make this almost free, and all three were verified in code:

1. **The op already exists and is already the right shape.**
   `CapabilityMsg::Announce` is a *declarative replace* of the submitter's whole
   tag set (`crates/modules/system/capability/src/lib.rs:274-368`). "This node
   offers exactly these tags" is precisely what an enable/disable produces. No
   new op, no new module, no module source change.

2. **The submit helper already exists and already blocks on the outcome.**
   `bin/node/src/node_http.rs:13-23` — `submit(base, target, payload) -> height`.
   `/v1/submit` is **settle-then-answer**: the handler awaits the actor oneshot
   (`bin/noded/src/lib.rs:677-712`), a 2xx carries `SubmitReceipt{height,…}`
   meaning *applied at that height*, and a 400 carries the module's verbatim
   rejection reason. Bounded by `SUBMIT_HOLD = 10s`
   (`bin/node/src/constants.rs:153`). Residents relay and get the same
   Applied/Rejected/Refused answer (`bin/node/src/validator/run/drain.rs:250-274`).
   **There is no wait-for-commit helper to build. There is nothing to poll.**

3. **`enable` already has every input it needs.** It already requires a live
   node (`node_identity` → `/v1/status`, `services.rs:121-127`) and a live hello
   (`plan_enable`, `services.rs:949-987`). `config::resolve_service` already
   yields `sandbox_capacity`, whose own doc comment says it is "both the dispatch
   pool's ledger and the capability announce's resources"
   (`bin/node/src/config/resolve.rs:193-196`).

The whole on-chain half of `enable` is therefore roughly:

```rust
crate::node_http::submit(&base, "capability", &serde_json::to_value(
    &capability::CapabilityMsg::Announce { capabilities, resources },
)?)?
```

and the liveness watcher (§4C) calls the *same* helper against the node's own
loopback `/v1`, on a transition rather than on a tick. Net: **~780 of the
~840 production lines of pump are deleted** (see §3, §4C).

---

## 1. History verdict — it was NEVER built as a transaction

**Answer: never built, never removed. There is no dropped submit path and no
recorded decision to drop one.**

Evidence, in the order it settles the question:

- **`bin/node/src/services.rs` has eight commits, and none of them ever
  contained a submit.** Full list: `9ce82a59a`, `62ac6294a`, `e5415ae2b`,
  `6b960bb07`, `bb26baa10`, `f3cb03646`, `68e392baa`, `889a96ac2`. Grepping every
  one of those diffs for `submit|Announce|CapabilityMsg|sign|Frame|transaction`
  returns only prose in doc comments — the sole code hit is `889a96ac2` adding
  the *string* `"saga.runs"` to `scopes_for` with a comment about the compute
  daemon's `/v1/submit` use. `commit_enable` has been `load → mint → insert →
  save` since the file was born.

- **`CapabilityMsg::Announce` has only ever had one submitter.**
  `git log --all -S'CapabilityMsg::Announce' -- bin/ crates/services/` returns
  exactly two commits: `b3aa755e6` (2026-07-05, *"register capability module,
  announce local providers (BYO)"* — which created the node's self-announcer)
  and `5135455ce` (which relocated it into `validator/announce.rs`). The
  self-announcer **predates the entire services wave by three weeks.** Wave-2
  step 1 did not replace it; it rewired its *source* from `discover()` to
  `grant ∩ live hello` (`e5415ae2b`, which swapped the `announce_capabilities:
  bool` config key for `granted_capabilities: Vec<String>`).

- **No PR discussion exists.** #815 has **zero** comments, #817 has zero, #816
  has one (about `dispatch_e2e`'s claim lane, unrelated). No review round, no
  scope cut, no stated reason.

- **The plan documents are untracked.** All six wave-2/3 docs are `??` in
  `git status`, so the design language *"the announce **tx is submitted by the
  user** at `service enable`"* (`2026-07-25-service-daemons.md:61`) has **no git
  history**. I cannot date it and cannot find a commit that dropped it. See §11.

### The one thing that WAS decided, and it is a real correction

The design language says a **user-signed** transaction. That is
**structurally impossible for this op**, and it is worth stating plainly
because it may be the unexamined premise that stalled the work:

`handle_announce` takes the announcing node's identity from
`ctx.env().origin` — *"identity comes from the verified submit origin, never
the payload"* (`capability/src/lib.rs:283-294`) — and then gates it on
`valset ∪ residents` (`capability/src/lib.rs:295-308`, via
`valset::members_and_residents`). A user account key is neither. A user-signed
announce would either register the *user's* key as a provider or, on any real
network, be rejected outright.

Further: **PR #822 deliberately removed the node's private key from the whole
`service` family** ("a daemon must not be able to hold the node's private
key"), replacing `config::resolve` with the keyless `config::resolve_service`
and pinning it with a source-parsing lint test
(`bin/node/src/services.rs:1729-1814`). `service enable` therefore holds **no
signing key at all** and must not acquire one.

Both facts point the same way, and it is the cheap way: **`enable` POSTs the op
to `/v1/submit`, and the node re-signs it with its own key inside the node
process.** `Origin::External(node_key)` is exactly the registry identity, it
respects #822's boundary, and it is the same door `user account-init`,
`user cred add` and the compute daemon's `OracleResult` already use.

**Conclusion: this is a gap, not a reversal.** Nothing found argues against
building it. The only design correction is that "user-signed" is wrong and
"node-signed, user-triggered, keyless CLI" is right.

---

## 2. Where the committed grant belongs — `capability`, via the op that exists

**It belongs in `capability`, as `CapabilityMsg::Announce`, and it needs no new
op, no new field and no module source change.**

The argument is one sentence: *the announcement IS the committed form of the
consent.* There are not two facts here. "The operator consented to this node
running kind X with tags T" and "the network knows this node offers X and T"
are the same statement, and `capability` already holds it, keyed on the only
identity that matters (the node), replicated to every validator.

### Reuse before adding — walked down, honestly

- **A new `roles` module.** Rejected, and the wave-3 announcement plan already
  costed it (`2026-07-26-wave3-announcement.md:249-259`): `MODULE_IDS` bump,
  ~10 sites in `host_state.rs`, `topology.rs` PRODUCTION 20→21 plus three pinned
  count tests, a new guest crate + `BUILDER_MODULES` entry + fixture,
  registration in noded/simnode/demo, **and** a new sibling wiring on `saga`
  which today knows exactly one registry id. It moves the genesis app hash. And
  placement would then need a second pool query beside
  `Saga::assignment_pool` (`saga/src/lib.rs:503-545`) — the dual-path defect,
  written into the architecture. Everything it would buy, `capability` already
  provides.
- **A new op on `capability`** (e.g. `GrantService { kind, instance, scopes }`).
  Rejected. It is still a module *source* change, so it is still a genesis flag
  day (§6) — and it would commit state that **no other node reads**: nothing on
  chain routes to an instance id, and a scope is one node's boundary against its
  own daemon. Committing it buys nothing and costs a network re-init.
- **A new op on another existing module** (`identity`, `gateway`, `lifecycle`).
  Rejected for the same flag day plus a scope violation — none of those modules
  is the registry of what a host can execute, and `capability`'s own doc header
  says it is.
- **A new field on `Announce`.** Rejected. `resources` and `capabilities` are
  already the full set of announced data that a *placement* decision reads
  (`saga/src/lib.rs:503-545` queries `Providers` / `CapableProviders` and
  nothing else). Adding a field is a module change for data no decider consults.

### What actually changes on chain

Nothing about the encoding. Only the **content** and the **submitter of record**:

| | today | after |
|---|---|---|
| tag set | `grant ∩ live hello`, recomputed ~10×/s by the node | `⋃ grants ({kind} ∪ grant.capabilities)`, computed once by the verb |
| resources | `resolved.sandbox_capacity`, forced empty when tags empty | identical rule, same source |
| who submits | the node's drain/park pump | the node, on behalf of a `/v1/submit` from the CLI |
| when | whenever the pump notices a difference | exactly when a human runs `enable`/`disable` |

**#819's kind-union fix is preserved verbatim** — a granted kind contributes its
own tag even when it offers no executors (the airlock plug's case:
`offered_capabilities` returns `Vec::new()` for `Daemon::Airlock`,
`services.rs:1217-1227`). That composition just moves from the pump to the verb.

---

## 3. The announce pump, responsibility by responsibility

Measured against **PR #819** (`fix/announce-all-service-kinds`), which is the
version about to merge: `announce.rs` becomes 1224 lines (632 production + 592
test), it deletes `resident_announce.rs` (-207), and it carries ~100 lines of
wiring across `validator/run/drain.rs` and `replica/park.rs`.

Your framing was right about the tag filter and mostly right about the rest.
The one correction: the filter and the cap **survive as rules but change
location**, and in the new location they are strictly stronger.

| responsibility | #819 site | fate | why |
|---|---|---|---|
| `granted()` — per-tick `services.toml` read | `announce.rs` | **SHRINKS** | the watcher reads it on a transition, not at 10 Hz |
| `grant_unreadable` latch | `announce.rs` | **DIES** | a corrupt toml is an error the verb returns; the watcher's throttled error log covers the rest |
| `offered()` — `grant ∩ live hello` per tick | `announce.rs` | **SURVIVES as `announced_set`, shared by verb + watcher** | ~35 lines, called on a transition instead of a tick |
| `legal()` + `note_illegal` — illegal-tag filter | `announce.rs` | **SURVIVES, MOVES** to `plan_enable` + `Services::validate` | see below — this gets *better* |
| `within_cap()` + `note_cap` + `MAX_ANNOUNCED_TAGS` | `announce.rs` | **SURVIVES, MOVES** to `plan_enable` as a refusal | see below |
| `the_announce_cap_matches_the_modules_own` (source-parsing pin) | `announce.rs` tests | **SURVIVES verbatim** | the mirrored constant still needs pinning |
| `effective_resources()` — empty tags ⇒ empty resources | `announce.rs` | **SURVIVES, MOVES** | 3 lines at the one call site; it is a module rule |
| `decide()` + the `announced` latch | `announce.rs` | **DIES** | nothing re-decides; there is no tick |
| `InFlight` / `sent()` / `owns()` / `submit_failed()` | `announce.rs` | **DIES** | `/v1/submit` returns the fate synchronously |
| `Rearm::Silence` / `Rearm::Unordered` / `Expiry` / `on_blocks` / `rearm_if_stale` / `ANNOUNCE_RETRY` / `ANNOUNCE_RETRY_BLOCKS` | `announce.rs` | **DIES** | two give-up budgets exist only because a fire-and-forget internal submit has no reply path. The CLI submit has one. |
| `Fate` / `on_outcome` / `rejections` / `REJECTION_REPORT_EVERY` / `rejection_report` | `announce.rs` | **DIES** | the 400 body carries the module's verbatim reason to a human who is standing there |
| `maybe_announce()` — 2 committed queries per drain tick | `announce.rs` | **DIES** | replaced by **one** query at watcher start (§4F.2); `service status` reads on demand |
| `pump_capability_announce` + the `current_members` check | `drain.rs:1203-1250` | **DIES** | the member gate is the module's, and its rejection is now readable |
| the drain's announce-fate route + `announce_is_ours` + `on_blocks` call | `drain.rs:294-327`, `:378` | **DIES** | *see the note below* |
| resident announcer + relay-Reply route | `park.rs:681,948,2078-2100` | **DIES** | |
| `resident_announce.rs` (207 lines) | whole file | **DIES** | #819 already deletes it |
| retraction when a daemon stops signaling | `announce.rs` test `a_daemon_that_stops_signaling_retracts_the_announce` | **SURVIVES — moves to the watcher** | §4B measured that deleting it costs real runs; the test survives nearly verbatim against the watcher |
| the kind tag in the announce (#819's actual fix) | `announce.rs` `kinds.insert` | **SURVIVES** in `announced_set`, used by both triggers | |

**Total deleted: ~630 production lines in `announce.rs` + ~100 lines of wiring
+ 207 lines of `resident_announce.rs` ≈ 840, minus the ~95-line watcher (~60 of
it net new, §4C) = **~780 net production lines deleted**, and both call sites
that had to stay in step collapse to one submit door.**

### Why the tag filter gets stronger, not just relocated

Today the filter is *downstream* of consent: `service enable` copies a hello's
tags into `services.toml` verbatim, `Services::validate`
(`services.rs:235-268`) checks kind/instance/nonce and **never checks the
tags**, and `legal()` silently drops the bad ones at announce time — after the
operator has already approved them on a consent screen that showed them.

Move it to the consent boundary and the invariant becomes provable:

- `plan_enable` refuses a hello whose tags fail `capability::validate_tag`,
  naming them. The operator fixes the daemon's capability spec *before*
  consenting.
- `Services::validate` gains the same check, so an illegal tag cannot enter the
  file by any route.
- The announce set is `⋃ ({kind} ∪ grant.capabilities)`; kinds are already a
  strict subset of the tag grammar (1..32 of `[a-z0-9-]` ⊂ 1..64 of
  `[a-z0-9._-]`). Therefore **the announce can never contain an illegal tag**,
  and there is nothing left to filter.

Same for the cap: `plan_enable` refuses an enable that would push the node past
`MAX_ANNOUNCED_TAGS` (64), naming the count and the offender — instead of
silently truncating a set the operator believes was announced whole.

Both changes trade a latched warn for a refusal. Loud, local, at the moment a
human can act. Note the one behavioural consequence to accept deliberately: an
existing `services.toml` containing an illegal tag will now **fail to load**,
which fails the node's boot path (`gate_on_compute_grant` →
`services::grant_for`, `config/resolve.rs:296-311`). That is correct
fail-closed behaviour on a file only the CLI writes, and there are zero live
networks.

### One thing the drain deletion gives up, named

`drain.rs:294-327` is currently the *only* route that reports the consensus
fate of an internal submit. Its own comment says the others — oracle results,
upgrade readiness, code-ready signals — still fall through the `continue` and
have their rejections swallowed whole. Deleting the announce route does not
make that worse (those paths are unrouted today either way), but it does remove
the one worked example. **Recommend keeping #819's generic seam idea on the
backlog as its own item**; it is not this change's job, and bundling it would
be a structural refactor outside the seam.

---

## 4. Crash retraction — measured, then answered

This is the one thing worth vetoing over, so it gets its own section, and it is
the part of the first draft that was wrong.

### 4A. What a planned stop costs: nothing

`service disable <kind>` submits the retraction and returns the height it
committed at — a *stronger* guarantee than today's "retracts on the next tick".
Planned stops, upgrades and reconfigurations are fully covered by the verb. Only
an **unplanned** daemon death is in question.

### 4B. The reproduction — measured, not derived

The first draft derived the failure mode from source and flagged it as
unreproduced. It has now been reproduced, as a throwaway unit test against
`crates/modules/system/saga` on `origin/dev` (written, run, reverted; the tree
is unchanged). Setup: one capability-tagged saga, a **single** announced
provider, `max_attempts: 3`, cranked at each lease expiry.

| | `lease_views: None` *(the recipe default)* | `lease_views: Some(n)` |
|---|---|---|
| **no retraction** (dead node stays announced) | re-leased onto the **same dead node** each crank → `Failed` ("lease attempts exhausted") | re-leased onto the **same dead node** each crank → `Failed` |
| **retraction** (pool goes empty) | `assignee: None`, **`lease_expires_at: None`, attempt frozen at 1, `Pending` indefinitely** — it waits on the claim lane | `assignee: None` but **still burns an attempt per crank → `Failed`, identically** |

Verbatim from the run:

```
A crank@15: status=Failed  attempt=2 assignee=Some(9) err=Some("lease attempts exhausted")
B crank@15: status=Failed  attempt=2 assignee=None    err=Some("lease attempts exhausted")
C crank@64/128/192: status=Pending attempt=1 assignee=None lease=None err=None
D crank@192: status=Failed attempt=2 assignee=Some(9) err=Some("lease attempts exhausted")
```

Three conclusions, all now evidence rather than inference:

1. **The single-provider failure mode is real** (A, D). `pick_assignee` is
   `pool[(hash % pool.len())]` (`saga/src/lib.rs:547-563`); with `len() == 1`
   the `attempt` reshuffle cannot help, so every re-lease returns the dead node.
   dukenet is exactly this shape — one compute provider on the macmini.
2. **Retraction fixes it only for `lease_views: None`** (C vs D) — and that is
   the default: every dispatch recipe fixture in the tree leaves it unset
   (`dispatch/src/lib.rs:961,1051,1064,1077`). For those, retraction is the
   difference between *"the run waits and completes when the daemon comes back
   and `Accept`s it"* (the claim lane, `bin/node/src/compute/intake.rs:419-424`)
   and *"the run `Failed`s"*. **That is a real product difference, and it is the
   common case.**
3. **Retraction is worthless for `lease_views: Some(n)`** (B vs A) — the saga
   fails identically either way.

### 4B-bis. A latent bug found on the way (not this plan's to fix)

Conclusion 3 has a cause worth recording separately:
`lease_expiry` (`saga/src/lib.rs:351-357`) returns `Some(height + views)`
whenever `lease_views` is set — **regardless of whether there is an assignee**:

```rust
match (assignee, lease_views) {
    (_, Some(views)) => Some(height.saturating_add(views)),   // ← unassigned too
    (Some(_), None)  => Some(height.saturating_add(DEFAULT_LEASE_VIEWS)),
    (None, None)     => None,
}
```

So an **announcement nobody holds** still carries a lease expiry, still gets
cranked, and still consumes the attempt budget — burning retries against
nobody until the saga `Failed`s, while a daemon that could have claimed it was
merely slow to arrive. Case B is that bug, observed. The `(None, Some(_))` arm
should almost certainly be `None`, matching `(None, None)`.

Filed here because it was found here; **it is not in this plan's scope** (it is
a `crates/modules/` change, therefore a flag day) and it should be its own item.
It does not change this plan's recommendation — case C is the default and case C
already behaves correctly.

### 4C. The half-measure exists, and it is ~95 lines

The first draft claimed the latch / `Rearm` / `Fate` / `Expiry` machinery is
inherent to any pump. **It is inherent to any pump that submits the way this one
does** — `OrderedNode::submit` is fire-and-forget *by design*
(`crates/kernel/node/src/lib.rs:1315-1324`: *"does NOT touch the local host …
returns the frame's `FrameId` so the caller can recognize this op's outcome in
`take_drained`"*). A pump living inside the drain loop structurally cannot await
its own commit — the drain loop is what produces commits — so it must track the
frame, latch, re-arm and route the fate back. That is the entire complex.

**A watcher outside the drain loop does not have that problem.** It POSTs to its
own node's loopback `/v1/submit` — the same settle-then-answer door the CLI and
all three daemons already use — and gets `Ok(height)` or `Err(module reason)`
inline, bounded by `SUBMIT_HOLD`. `http_listen` is already threaded through boot
(`bin/node/src/boot/env.rs:29,94`).

```
loop {
    wait for the alive-set to change (or tick)
    let want = announced_set(&load(workspace)?, &service)?;   // shared with the verb
    if want != last_submitted {
        match node_http::submit(&base, "capability", …) {
            Ok(_)  => last_submitted = want,
            Err(e) => log_throttled(e),                        // next transition retries
        }
    }
}
```

No latch (the POST blocks). No `Rearm`/`Expiry` (`SUBMIT_HOLD` bounds it). No
`Fate`/`on_outcome` (the answer is inline). No `FrameId`. No drain wiring, no
park wiring, no `resident_announce.rs`, no two-tier give-up budget.

**Quantified, against #819's 632 production lines in `announce.rs` + ~100 lines
of drain/park wiring + 207 lines of `resident_announce.rs` ≈ 840:**

| piece | lines | note |
|---|---|---|
| `load` + read the live catalog | ~16 | existing helpers |
| `announced_set` composition (kinds ∪ executors, legality, cap) | ~35 | **shared with the verb — needed anyway** |
| compare to last-submitted | ~3 | |
| POST + throttled error log | ~20 | `node_http::submit` |
| task/tick wiring | ~20 | |
| **total** | **~95** | **~60 net new** |

**~780 of ~840 still die.** Everything in §3's table marked DIES still dies; the
watcher resurrects none of it.

### 4D. Where the grant lives in this shape — and why it is NOT a second record

Since the watcher already reads the local grant to compute the intersection,
**committing the grant separately would be a consensus write that nothing off-node
ever reads.** So there is no second record:

- **The announce remains the single committed projection**, and its content is
  `grant ∩ alive` — unchanged from today.
- **What changes is who triggers a submit, and how**: the *verb* on a consent
  change (synchronous, reported to the human, §7); the *watcher* on a liveness
  transition (§4C). Both through `/v1/submit`.
- **Still exactly one writer of the announce** — the node. No daemon writes
  anything, so the N-daemons-clobber-one-declarative-set hazard never arises.

This is the coordinator's split, and it is the right factoring. The plan's title
is therefore slightly off: the honest statement is *"the announce becomes the
committed record of consent, and consent stops travelling through a loop that
cannot see it."*

### 4E. Write amplification — bounded by the TTL, and no worse than today

A daemon flapping must not become a consensus write amplifier. Measured against
the existing constants (`HELLO_TTL = 30s`, heartbeat `TTL/3 = 10s`,
`services.rs:1038-1042`):

- **Restart / upgrade inside 30 s: ZERO writes.** The catalog entry never
  lapses, so there is no transition to submit. This is the common case and it is
  free.
- **Crash-loop with a cycle longer than 30 s: 2 writes per cycle** (alive→dead,
  dead→alive), unbounded in principle. Module-side idempotence does **not** bound
  it — `Announce` stages nothing only for an *identical* set
  (`capability/src/lib.rs:342-344`), and a flap alternates between two different
  sets.
- **But this is not a regression.** Today's `decide()` re-announces on exactly
  the same transitions, so the merged pump has the identical property. Nothing
  here makes it worse.
- **If it ever needs bounding, the knob is asymmetric and cheap**: retract
  immediately (safety — stop taking work), re-announce only after the kind has
  been continuously alive for one full TTL. One timestamp, one named predicate.
  Not built speculatively; named so it is not re-derived under pressure.

### 4F. The other three things the loop covered — the watcher keeps two of them

Because the watcher compares the **derived set** (`grant ∩ alive`) and not a
bare alive/dead bit, it covers more than crash retraction:

**1. A daemon upgrade that drops an executor.** **Covered.** The hello's offered
tags change, so `want` changes, so the watcher narrows the announce — exactly as
today. No extra code, and no need for the boot-time grant-vs-discovery check the
first draft proposed as a substitute.

**2. Nothing re-announces after a genesis flag day or a late admission.**
**Covered, for ~5 lines.** Seed `last_submitted` from **one** committed query
(`CapabilityQuery::Node { node }`) when the watcher starts, then never query
again. A steady-state boot then submits nothing (`want == last_submitted`),
while a boot after a chain re-init or after late admission finds the registry
empty and submits once. This is the whole of the first draft's
"`disable && enable` on a flag day" workaround, deleted.

*(Note the contrast with today's pump, which issues **two** committed queries
per 10 Hz drain tick, forever. One query per process lifetime is the same
information at ~six orders of magnitude less cost.)*

**3. The pump's weak "the file wins" property.** **Partially covered, and it was
theatre anyway.** The watcher overwrites a rogue direct announce on the next
transition, not immediately. It does not matter: the node's `/v1` surface is
unauthenticated by design (`bin/noded/src/origin_guard.rs:25-27`) and any
same-uid process can already POST anything. The real fix is the wave-3
grant-token / scope-enforcement pair, not a loop.

### 4G. Verdict, and what the operator actually experiences

**The split wins. Build the watcher.** The deciding evidence is §4B conclusion 2:
for the default recipe shape (`lease_views: None`), retraction is the difference
between a run that completes and a run that fails, and dukenet's single-provider
topology is exactly where it bites.

The operator experience under each shape, so the trade is concrete:

| daemon crashes at 12:00 | merged shape (no watcher) | split shape (watcher) |
|---|---|---|
| network stops placing on the node | **never** — only when someone runs `service disable` | within `HELLO_TTL` (≤30 s), then one submit |
| in-flight run (`lease_views: None`) | re-leased onto the dead node each expiry → **`Failed`, "lease attempts exhausted"** | goes unassigned, `Pending`, **completes when the daemon returns and `Accept`s it** |
| what the operator sees | `service status` shows `enabled but not signaling`; the chain still lists the node. Nothing in the log says the network is still routing work at it | the same `status` line, plus one `info` naming the retraction and its height |
| what the operator must type | `ducktape service disable <kind>` — **and they must know to** | nothing; `service run <kind>` to come back |

The merged shape's row 1 is the part that cannot be shipped quietly: *"nothing
retracts until a human runs `disable`"* is a product decision, not an
implementation detail. The watcher costs ~60 net new lines and removes the
decision entirely, which is why it is the recommendation rather than an option.

### 4H. What the first draft got wrong

Recorded because the reasoning error is reusable, not out of ceremony.

1. **"The latch/`Rearm`/`Fate` machinery is inherent to any pump."** False. It is
   inherent to a pump that submits through `OrderedNode::submit`, which is
   fire-and-forget *by design* and lives inside the drain loop that produces the
   very commits it would need to await. I generalised a property of one call site
   into a property of the problem. The `/v1/submit` door — which the same draft
   correctly identified as settle-then-answer for the CLI — was available the
   whole time.
2. **"The split requires committing the grant, and then N daemons clobber."** Two
   errors stacked. The split does not require committing the grant (§4D: it would
   be a write nobody reads), and the clobbering hazard only exists if *daemons*
   submit — which no version of this proposal has them do. I reached for the
   strongest objection to a shape I had not actually drawn.

Both errors ran the same direction: defending an already-drafted conclusion
instead of drawing the alternative. The measured matrix in §4B is what broke it,
and it took one throwaway test — which is the argument for reproducing before
concluding, not after.

### Divergence between the file and the chain: designed out, not managed

Order the two writes **submit first, persist second**, and there is no
divergence state to detect or repair:

- Announce **rejected** → nothing written, non-zero exit, the module's reason
  printed. Re-running the verb is the whole retry story.
- Announce **applied**, file write fails → the CLI errors naming both facts. The
  window is an over-announce, and it is inert: `serve_kind` refuses to serve
  without a grant (`services.rs:1122-1180`), so nothing executes. Re-running the
  verb re-announces (the module stages nothing for an identical set,
  `capability/src/lib.rs:342-344`) and writes.

No two-phase commit, no rollback, no reconciler, no repair verb.

---

## 5. The local/committed boundary

**The rule: consent is local; the *consequence* of consent is committed; the
crossing happens exactly once, in the verb the human ran.**

### Cannot move on chain — and pre-consent signaling is the load-bearing case

- **The hello catalog** (`bin/noded/src/services.rs`, in-memory, 30 s TTL). A
  daemon says "I am here" **before** any operator has approved it. Committing
  that would let a daemon place itself on chain, inverting the consent order the
  entire design rests on — and it is re-asserted every `HELLO_TTL/3`, so it
  would be a consensus write per heartbeat per daemon. It stays a volatile local
  catalog. `service list` / `service status` keep reading it.
- **`grant.scopes`.** One node's consent boundary against its own daemon. No
  other node's placement or admission decision reads it. (It is also still
  unenforced — see `2026-07-26-wave3-scope-enforcement.md`; that is a sibling
  problem and this plan does not touch it.)
- **`grant.instance` / `grant.nonce`.** The instance id scopes podman labels
  (`io.ducktape.managed=compute#deadbeef`) and marks a consent epoch. Nothing on
  chain routes to an instance — placement addresses a node key. Announcing it
  would additionally make every disable/enable cycle a consensus write.
- **`grant.granted_unix`.** Local audit only.
- **Build identity / version.** Already refused at the hello boundary with a
  nameable reason. On chain it would be an admission gate keyed on a version
  number — forbidden by the repo's no-versioning doctrine.
- **Declared `needs`.** Display-only by construction, a documented standing
  non-goal.
- **The compute backend gate** (`gate_on_compute_grant`,
  `config/resolve.rs:296-311`) reads grant *presence* only. Stays local, unchanged.

### Becomes committed

- **The announced tag set**: `⋃ over grants ({kind} ∪ grant.capabilities)`.
- **`resources`**: `ServiceConfig::sandbox_capacity`, forced empty when the tag
  set is empty (the module's own rule).

That is the whole crossing. Note `grant.capabilities` sits on **both** sides
and that is not a dual source of truth: locally it is *what the operator
reviewed and approved, per kind* (the consent record, and what `disable` needs
to compute the remaining union); on chain it is *what this node offers the
network* (a union, kindless). Different facts, one derivation, one direction,
one writer.

---

## 6. Flag-day cost — measured

**Zero. The genesis app hash does not move.**

The mechanism that would move it: `seeded_lifecycle`
(`bin/node/src/host_state.rs:220-328`) seeds the Lifecycle registry with
`sha256(component.wasm)` for every wasm tenant — capability at
`host_state.rs:251-256` — and Lifecycle is a `MerkleStore` module, so those
digests are consensus state. Touching a module's **source** rebuilds its
`component.wasm`, changes the seeded digest, and moves the genesis root even for
a read-only change (measured in wave-2 step 2, `e899bbeb8`).

This plan touches:

- `bin/node/src/services.rs`
- `bin/node/src/validator/announce.rs` (deleted)
- `bin/node/src/resident_announce.rs` (deleted)
- `bin/node/src/validator/run/drain.rs`, `bin/node/src/replica/park.rs`,
  `bin/node/src/validator/run.rs`, `bin/node/src/main.rs` (wiring removal)
- tests

**Nothing under `crates/modules/`.** `capability::validate_tag`,
`capability::validate_resources` and `capability::encode_msg` are already `pub`
(`interface.rs:38-57`, `:97-113`, `:253-255`) and `bin/node` already links the
crate natively — `config/resolve.rs` calls `validate_resources` today. Linking
an existing `pub` fn rebuilds no wasm.

**Litmus for the PR:** `git diff --stat crates/modules/` is empty.

Two traps to state so nobody walks into them:

1. **Do not make `MAX_CAPABILITIES` `pub`.** It is a private const at
   `capability/src/lib.rs:86`; changing the visibility keyword rebuilds
   `component.wasm` and buys a flag day for nothing. Keep #819's host-side
   mirror plus its source-parsing pin test
   (`the_announce_cap_matches_the_modules_own`), which survives this change
   verbatim.
2. **`make wasm-modules` rebuilds all 16 `BUILDER_MODULES` to different bytes**
   because of pre-existing toolchain drift (`Makefile:101-127`, warned at
   `:91-94`). This plan should never run it. If a diff shows a `component.wasm`,
   something is wrong.

**Measurement caveat, stated because it limits the proof:** there is **no
absolute PRODUCTION genesis-hash pin in the test suite.** The only golden root
constant is `bin/simnode/tests/topology_set.rs:16` and it pins `SIM_BASE`, which
does **not** compose `capability` (`crates/kernel/host/src/topology.rs:132`
puts capability in `PRODUCTION` only). So "zero flag day" is provable by the
empty `crates/modules/` diff, not by a hash assertion. The existing lockstep
proof `wasm_capability_parity.rs` is unaffected either way.

### FOLLOW-UP A — pin the PRODUCTION genesis root (own item, not this PR)

A campaign that has now asserted *"the root hash did not move"* across wave 1,
wave 2 and wave 3 should have **one test that fails if it does.** It does not.
Every such claim so far has rested on reading a diffstat.

The shape is the one `topology_set.rs:16` already uses for `SIM_BASE`: build the
`PRODUCTION` host state at genesis, assert `Host::root_hash`
(`crates/kernel/host/src/lib.rs:821-824`) against a checked-in constant, with the
constant's update procedure in a comment naming `skills/module-dev`. It is
cheap, and it converts "flag day" from a claim into a failing test with a
deliberate one-line update — which is exactly the friction a genesis move should
have.

Sized as its own PR because **writing it moves nothing and blocks nothing**, and
because bundling a new golden-root test into a change that asserts the root did
not move is circular. Recommend it lands *before* this plan's PR, so this plan's
"zero flag day" claim is the first one the test actually verifies.

---

## 7. Operator-facing consequences

### It does NOT need a key, and it does NOT need a caught-up-check of its own

- **No key.** #822 made the family keyless on purpose and pinned it with a
  source lint (`services.rs:1729-1814`). `enable` POSTs to `/v1/submit`; the
  node signs. A user key could not do this anyway (§1).
- **No new liveness requirement.** `enable` already fails without a running node
  (`node_identity` → `/v1/status`) and already fails without a live hello. The
  wave-2 QA runbook already records this as "#822: `service enable` now needs a
  live node".
- **Slower: bounded by `SUBMIT_HOLD = 10s`,** and only in the failure case — a
  healthy chain answers in about one block.

### `ducktape service enable <kind>`

```
$ ducktape service enable compute
  <the existing consent screen: kind, chain, offers, scopes in red>
  ? Enable compute on this node? [Y/n] y

compute#deadbeef                                   ← stdout, unchanged contract
  ✓ enabled compute#deadbeef · announced at height 41207
    start it with: ducktape service run compute
```

**It blocks until commit — because that is free.** The 2xx *is* the commit. The
stdout contract (`$(ducktape service enable compute)` = the id and nothing else)
is unchanged; the height goes to stderr.

New failure modes, each with the operator's actual next move:

| condition | source | what the operator sees |
|---|---|---|
| node not yet a member/resident | `capability/src/lib.rs:295-308` | *"this node holds no standing on `<chain>` yet — it must be an admitted validator or resident before it can announce. Nothing was enabled."* |
| chain not finalizing | `drain.rs` `SUBMIT_HOLD` | *"the chain did not finalize within 10s. Nothing was enabled; try again."* |
| resident with no reachable validator | `park.rs` `not_serving()` | *"this node cannot reach a validator to relay the announce."* |
| illegal tag in the hello | **refused before submit**, `plan_enable` | names the tags and the grammar |
| would exceed 64 announced tags | **refused before submit**, `plan_enable` | names the count and the total |

In every one of them **nothing is written** and re-running the verb is the
entire recovery story.

### `ducktape service disable <kind>`

Same shape, reversed: submit the reduced union, then remove from the file. Its
current line *"the node retracts its announce on the next tick"*
(`services.rs:1495-1502`) becomes *"retracted at height N"* — a stronger and
simpler claim. The existing "stop the daemon too" hint is unchanged and now
matters more (§4.1).

### `ducktape service run <kind>` — the enable-at-run prompt

This is the one place the change is felt. Today `offer_enable`
(`services.rs:1355-1400`) writes a file and continues immediately. After, it
submits.

**The governing rule: a failed announce must never stop the daemon.** The
existing posture already says declining leaves the daemon running and
signaling; a rejected announce takes the same exit.

- **TTY, not enabled** → prompt once (unchanged) → submit → on success print the
  id and the height, then serve. On failure print the reason and the retry
  command, keep signaling, serve nothing (there is no grant). Never re-prompt.
- **Non-TTY** → never prompts (unchanged).
- **`--enable`** on a node not yet admitted → boots, emits one `warn` naming the
  reason and `ducktape service enable <kind>`, keeps signaling. **Not a retry
  loop** — that would re-create the log bomb this plan exists to delete.
- **Already enabled** → no prompt, no submit, straight to serving (unchanged).
  The node is already in the registry; consensus state is durable.

### `ducktape service status`

Gains one committed column, via one `node_http::query`
(`CapabilityQuery::Node { node }`) — so the operator can see what the chain
actually holds beside what the file says. Read-only, on demand, no pump. It is
also the natural home for the §4.3 flag-day hint.

---

## 8. Implementation shape (house rules)

Small enough to state completely.

- **One discriminant, one dispatch.** The composition is a pure fn:
  `announced_set(&Services, &ServiceConfig) -> Result<(Vec<String>, BTreeMap<String,u64>), CapRefusal>`.
  Decide-fn: reads the loaded grants and the config, validates, returns the pair
  or a named refusal. It writes nothing and does no I/O.
- **`CapRefusal` is a closed enum** — `IllegalTag { tags }`, `OverCap { total,
  cap }` — matched once by the renderer, no `_` arm.
- **Named writers, in order.** `enable` = `plan → confirm → announced_set →
  announce(base, set) → persist(workspace, grant)`. `disable` =
  `locate → announced_set(without) → announce → persist`. Two writers
  (`announce`, `persist`), one order, one place.
- **`announce()` is the single submit site**, thin over `node_http::submit`.
  Both verbs call it; nothing else does.
- **Delete, do not gate.** `validator/announce.rs`, `resident_announce.rs`, the
  `CapabilityAnnouncer` field on the drain/park state, `granted_capabilities`
  threading through `validator/run.rs` and `replica/park.rs`. No flag, no
  fallback, no "embedded announcer" mode.

### Tests (all unit; they wait on nothing)

- `announced_set` composes the kind union across two grants; a kind with no
  executors still contributes its own tag (**inherit #819's
  `a_kind_with_no_executors_still_announces_itself`**).
- Empty grants ⇒ empty tags ⇒ empty resources (the module rule).
- An illegal tag is refused at `plan_enable`, named, and never reaches a submit.
- An over-cap union is refused at `plan_enable`, named.
- `Services::validate` rejects a file carrying an illegal tag.
- `the_announce_cap_matches_the_modules_own` — kept verbatim from #819.
- `the_service_path_never_reads_the_node_key` — must stay green (#822).
- Cluster e2e: `bin/node/tests/resident_announce_e2e.rs` is rewritten from
  "wait for the pump to converge" to "run `enable`, assert the receipt height,
  then assert the committed registry" — which is a *shorter* test than either
  the current one or #819's.

Gates: `cargo clippy -p node-bin -p noded --tests --no-deps`;
`cargo check -p files --no-default-features`;
`git diff --stat crates/modules/` empty.

---

## 9. Sequencing

### Merge #819 now. Do not hold it.

1. The forever-retry wedge it fixes is **live on `dev`**: an announce that
   submits fine and is rejected at execute leaves the decide latch set forever,
   and the node is silently out of every rendezvous pool. That is a defect on
   the running dukenet pair today.
2. It is a bug fix; this is a design change the user has to approve. Coupling
   them delays the fix behind a decision.
3. Its *rules* are what the enable path inherits — the kind union, the tag
   legality check, the cap bound, and the `MAX_ANNOUNCED_TAGS` pin test all move
   rather than die (§3). Merging it is not wasted work; it is the specification
   of the new verb, already written and already tested.

**What of #819 this change then deletes:** `Rearm`, `Expiry`, `InFlight`,
`Fate`, `ANNOUNCE_RETRY`, `ANNOUNCE_RETRY_BLOCKS`, `REJECTION_REPORT_EVERY`,
`decide`, `sent`, `owns`, `submit_failed`, `on_outcome`, `rejection_report`,
`on_blocks`, `rearm_if_stale`, `maybe_announce`, `granted`, `grant_unreadable`,
`note_illegal`/`note_cap`'s latching (the checks themselves move), the whole
`CapabilityAnnouncer` struct, both wiring sites, and every test that exercises
the latch/retry/re-arm. Roughly `announce.rs` in its entirety.

### Relative to the wave-2 integration QA pass — land this BEFORE it

`2026-07-26-wave2-integration-qa.md` has not been run. Three of its steps assert
behaviour that either does not exist or is about to be deleted:

- The lifecycle step asserts *"enable → id minted, config persisted, **tx
  submitted and committed**"* — **today that is unverifiable, because no tx
  exists.** Running QA first would certify a step the code does not implement.
- **§X-2** ("the kind tag is in the committed registry") becomes a synchronous
  assertion on the `enable` command's own receipt instead of a poll for the pump
  to converge — strictly easier to run and easier to fail honestly.
- **§X-3** ("a rejected announce reports, then re-arms") **ceases to exist**;
  its replacement is "a rejected announce fails the `enable` verb, prints the
  module's reason, and writes nothing".
- The observable tables at §1385-1391 lose `capability_announce_rejected` and
  keep `announce_tag_illegal` / `announce_over_cap` only as *pre-submit
  refusals*.

Running QA first means re-running the whole placement/announce section
afterwards. **Recommendation: #819 merges now → this change lands next, and
rewrites the lifecycle step + §X-2/§X-3 + the observable table in the same PR →
then the QA pass runs once, against a tree whose runbook matches it.**

The honest counter-argument, since it is real: this is the campaign's only
end-to-end verification, it is already written, and inserting another change
ahead of it delays it again — and §4.1's availability regression is exactly the
kind of thing that pass would have caught. If the user prefers, running QA on
`dev + #819` first is defensible; the cost is re-running ~4 steps and knowingly
certifying one assertion the code does not implement.

### FOLLOW-UP B — commit the plan documents

All six wave-2/3 plan documents are **untracked** (`git status` reports
`2026-07-25-service-daemons.md`, `2026-07-25-services-extraction.md`,
`2026-07-26-wave2-integration-qa.md`, `2026-07-26-wave3-announcement.md`,
`2026-07-26-wave3-grant-tokens.md`, `2026-07-26-wave3-scope-enforcement.md` and
this file as `??`). Every earlier plan in `docs/superpowers/plans/` **is**
tracked, so this is drift, not policy — `.gitignore:18-19` ignores only
`.superpowers/`, never `docs/`.

The cost is concrete and this investigation paid it: §1 could establish that no
submit path ever existed in *code*, but could not date, attribute or bisect the
*"a user transaction follows"* language that specified one — because the
document carrying it has no history. **The next design gap will be equally
unbisectable.** Commit all six (one `docs:` commit, no review burden), and keep
new plan documents tracked from creation.

### Explicitly not in this plan

The `Stance` discriminant and the `RoleJoin`/`RoleLeave` role plane (wave-3
announcement §5, step C, a real flag day); grant-scope enforcement; the
per-grant bearer token; assigned-k; the `lease_expiry` unassigned-arm bug
(§4B-bis — a `crates/modules/` change, therefore its own flag day). This plan
changes **who submits the announce and when**, and nothing else.

---

## 10. Decision the user is being asked to make

1. **Approve the shape:** `enable`/`disable` submit `capability::Announce`
   through `/v1/submit` on a consent change; a **~95-line node-local liveness
   watcher** submits through the same door on an alive/dead transition;
   `announce.rs` + `resident_announce.rs` + the drain and park wiring are
   deleted. One committed record, one submit door, two triggers, one writer.
   Cost: **zero flag day, ~780 net production lines deleted.**
2. **The crash-retraction question is settled, not open.** §4B measured it: for
   the default recipe shape (`lease_views: None`) retraction is the difference
   between a run completing and a run failing, and dukenet's single-provider
   topology is where it bites. The watcher keeps it. *(This reverses the first
   draft's recommendation — see §4G/§4H.)*
3. **Confirm the sequencing:** FOLLOW-UP A (genesis-root pin) → #819 → this →
   the wave-2 QA pass. FOLLOW-UP B (commit the plan docs) at any time.

Two items are raised here but deliberately **not** bundled: the `lease_expiry`
unassigned-arm bug (§4B-bis) and the missing PRODUCTION genesis-root pin
(FOLLOW-UP A). Both are their own PRs.

---

## 11. What I could not substantiate

Flagged rather than guessed.

1. **Why "a user transaction follows" was written and never built, I do not
   know.** All six wave-2/3 plan documents are **untracked** (`git status`
   reports them `??`), so that language has no git history and cannot be dated
   or attributed to a commit. #815 and #817 have zero PR comments and #816's
   only comment is unrelated. If the decision was made in conversation rather
   than in the repo, no record of it exists in this tree. What I *can* prove is
   that no submit path was ever present in the code (§1) — I cannot prove nobody
   ever discussed removing one.
2. **I ran exactly one build and one test** — the §4B reproduction against
   `crates/modules/system/saga`, written, run and reverted (`git status` clean
   afterwards). Everything else is from reading source. The "zero flag day"
   claim is structural (no file under `crates/modules/` changes) and, as §6
   notes, there is no absolute PRODUCTION genesis-hash pin in the suite to
   measure it against — hence FOLLOW-UP A.
3. **§4B was reproduced at the module level, not on dukenet.** The saga
   module's own `CaptureCtx` harness drove it deterministically, which is
   stronger evidence than a live run for *this* question (it pins the exact
   `lease_views` × retraction matrix). What it does **not** establish is which
   `lease_views` real dukenet recipes carry: I confirmed every in-tree dispatch
   fixture leaves it `None` (`dispatch/src/lib.rs:961,1051,1064,1077`) but did
   not read the recipes registered on the live pair. If a live recipe sets
   `lease_views`, §4B row 2 says the watcher buys that recipe nothing — worth
   one `ducktape` query against dukenet before the change lands.
4. **The ~95-line watcher estimate is an estimate.** The pieces are named and
   the helpers exist, but nothing was written. The claim that survives without
   it: `/v1/submit` is settle-then-answer
   (`bin/noded/src/lib.rs:677-712`) and `OrderedNode::submit` is not
   (`crates/kernel/node/src/lib.rs:1315-1324`) — that asymmetry is what makes
   the watcher small, and it is verified.
4. **I did not audit the `bin/noded` single-node daemon path.** `capability`
   constructed with `valset_id: None` lets any external key self-announce
   (`capability/src/lib.rs:15-19`), and `capability` is not in `SIM_BASE`,
   `SIM_VALSET` or `DEMO` (`topology.rs:145-195`), so I expect no impact — but I
   verified topology membership, not every noded boot path.
5. **The `bin/node/tests/resident_announce_e2e.rs` rewrite is scoped, not
   designed.** #819 already reshapes that file substantially; I read its diffstat
   (379 lines changed) but not the resulting test body line by line.
