# Wave 3, onchain half: service announcement, discovery, placement

- **Date:** 2026-07-26
- **Status:** proposal — investigation complete, nothing implemented
- **Base:** `origin/dev` @ `18564f1a4` (+ open PR #818, airlock plug)
- **Covers:** the consensus-side machinery that lets a service kind be
  announced, discovered across nodes, and assigned; and the enrolled stance
  that wave 3 adds beside the autonomous one.

## The headline

**No new consensus module. Announcement, discovery and placement are already
built — they just are not being fed the service kind.**

`capability` is, verbatim from its own doc header, "the network-wide registry
of what each node's host can execute … node key -> announced tag set,
replicated in consensus state so every node holds an identical view of who
provides what" (`crates/modules/system/capability/src/interface.rs:1-10`).
That is the announcement plane. `saga` already rendezvous-assigns over it
(`crates/modules/system/saga/src/lib.rs:503-563`). The node already submits the
announce, derived from `grant ∩ live hello`
(`bin/node/src/validator/announce.rs:106-201`).

The gap is one line of intent, not one module of machinery: the announcer's
grant read hardcodes `COMPUTE_KIND` (`bin/node/src/validator/announce.rs:76`),
so an agent or airlock grant is never announced at all, and no node's KIND is
ever a tag anyone can query.

So the plan splits sharply:

| Want | Costs | Flag day |
|---|---|---|
| announce / discover / place a service **kind** | one host file | **none** |
| the **enrolled** stance (notice-bound membership) | 3 op variants inside `capability` | genesis hash moves |
| a `roles` module | all of the above + 21st-module registration across 4 bins | genesis hash moves |

Recommendation: ship the first row now. Specify the second row here, build it
when the assigned-k storage tier is actually being built. Reject the third row
outright.

---

## 1. What exists onchain today (survey)

### `capability` — the registry, and it is already the right shape

`CapabilityMsg` has exactly two variants
(`crates/modules/system/capability/src/interface.rs:191-212`):

- `Announce { capabilities: Vec<String>, resources: BTreeMap<String,u64> }` —
  a **declarative replace** of the submitter's whole set. Identity is the
  verified `Origin::External` key, never payload data; an empty set removes the
  node; re-announcing the current set stages nothing and cannot move the root
  (`lib.rs:274-368`, idempotence at `:342-344`).
- `ClaimClass { class }` — module-origin-only, first-claim-wins, permanent.

Announces are gated to `valset ∪ residents` (`lib.rs:295-308`, via
`valset::members_and_residents`) — so a joined-but-unpromoted resident can
announce, which is exactly the node shape a service daemon runs on.

Reads (`interface.rs:216-240`, answered `lib.rs:462-527`): `Providers`,
`CapableProviders { capability, demands }`, `Node`, `Resources`, `All`,
`ResolveClass`, `Classes`.

Bounds that matter: `MAX_CAPABILITIES = 64` tags per node (`lib.rs:86`),
`MAX_TAG_LEN = 64`, charset `[a-z0-9._-]` (`interface.rs:32-57`).

### A service kind is already a legal capability tag

Not "could be made into" — **is**. The hello boundary's kind grammar is
`1..32` bytes of `[a-z0-9-]` (`bin/noded/src/services.rs:248-254`), a strict
subset of `capability::validate_tag`'s `1..64` bytes of `[a-z0-9._-]`. The
comment in `noded::services` even says so: "kinds are capability-tag shaped"
(`:47`). No new type, no new validation, no conversion.

### `dispatch` / `saga` — the placement lane, already wired to `capability`

- `Recipe { capability: String, routing: Rendezvous | Pinned(node), … }`
  (`crates/modules/system/dispatch/src/interface.rs:92-109`).
- `SagaMsg::Trigger { capability: Option<String>, demands, pinned_assignee, … }`
  (`saga/src/interface.rs:127-164`).
- `Saga::assignment_pool` queries `capability` with `Providers` (no demands) or
  `CapableProviders` (with demands), and falls back to the raw valset only for
  an untagged saga (`saga/src/lib.rs:503-545`).
- `pick_assignee` = `sha256(saga_id ‖ attempt ‖ height) % pool.len()` over the
  sorted pool — every input agreed, so every validator derives the same
  assignee (`saga/src/lib.rs:547-563`).
- The daemon-side claim lane exists and is live: `SagaQuery::UnassignedPending`
  → `SagaMsg::Accept`, first accept in consensus order wins
  (`saga/src/interface.rs:341-361`; `bin/node/src/compute/intake.rs:399-427`
  and `:537`).

### `gateway` — inbound is already solved, do not re-solve it

A service has no keypair, so it has no overlay identity to bind. Overlay
ingress lands on the node's `Service::Gateway` plane, which authenticates the
WireGuard peer, maps it to an account, enforces the signed `RouteStatement`,
and only then dials a loopback listener (PR #818 does exactly this for the
airlock plug). Gateway routes are already consensus state. **An announced
service therefore needs no endpoint field of its own.**

### Verdict on reuse

**`capability` can be reused, and reusing it costs nothing.** Announcing the
kind as a tag makes "find a node running kind X" resolve to
`CapabilityQuery::Providers { capability: "X" }` — an existing query, already
answered, already the pool saga draws from. Zero module bytes change.

What `capability` **cannot** express, and no amount of tag cleverness fixes:

1. **Departure is instantaneous and unilateral.** `Announce` is a declarative
   replace; a node drops out the moment it re-announces, and today's announcer
   retracts within one `HELLO_TTL` (30 s, `bin/noded/src/services.rs:32`) of a
   daemon going quiet (`announce.rs:359-388` tests this on purpose).
2. **There is no join height.** You cannot prove a node owed you anything
   between two heights, because the registry holds only the current set.

Those two are the entire enrolled delta. Everything else about enrolment —
volunteering, discovery, placement — `capability` already does.

---

## 2. What gets announced, field by field

Every field below has to survive one question: *does a placement or an
admission decision on another node depend on it?* If not, it stays local.

### Earns consensus (all of it already exists except the first row)

| Field | Why it earns it |
|---|---|
| **the kind, as a capability tag** (`compute`, `agent`, `airlock`, later `storage`) | placement queries it. It is the ONLY new announced datum wave 3 needs. Cost: one string per enabled kind inside the existing `capabilities` vec. |
| executor tags (`claude`, `codex.gpt-5-codex`, …) | already announced, already the dispatch address space. Unchanged. |
| `resources` (`cores`, `mem_gb`) | already the `CapableProviders` demand filter (`saga/src/lib.rs:512-521`). Unchanged. |

That is the whole list. Three rows, two of which are already shipped.

### Deliberately NOT consensus, with the argument

- **instance id** (`compute#deadbeef`). Nothing on chain routes to an instance —
  placement addresses a NODE key. The id exists to scope podman labels
  (`io.ducktape.managed=compute#deadbeef`) and to mark a consent epoch, both
  strictly local. Keeping it off chain also kills a real hazard: a re-enable
  mints a fresh id, and if the id were announced, every `disable`/`enable`
  cycle would be a consensus write.
- **version and build identity.** Skew is already refused at the hello
  boundary, fail-closed, with a nameable reason
  (`bin/noded/src/services.rs:113-115`, `:341-348`). Putting a version on chain
  would create an admission gate keyed on a version number — explicitly
  forbidden by the repo's no-versioning doctrine.
- **grant scopes.** A scope is one node's consent boundary against its own
  daemon. No other node's decision reads it.
- **declared needs.** Display-only by construction, and documented as a
  standing non-goal (`bin/noded/src/services.rs:84-93`).
- **endpoint / address.** Covered by the `gateway` module's route records. A
  second address plane would be duplication.
- **capacity honesty / health.** Placement is deliberately health-blind. Health
  belongs to audit and eviction, not to the announce.

### Enrolled-only additional state (the roles delta, §5)

Per `(node, role)`: `joined_at: u64`, `leaving_at: Option<u64>`. Two integers.
Argued in §5 and §6 — they are what a durable shard map needs and what an
obligation window is made of. Nothing else.

---

## 3. Placement — it plugs in, it does not duplicate

A node with work to place finds a node running kind X through the lane that
already exists, with **no new placement code at all**:

```
DispatchMsg::Dispatch { recipe_id, payload, demands }
  → recipe.capability = "<kind or executor tag>", routing = Rendezvous
  → SagaMsg::Trigger { capability, demands }
  → Saga::assignment_pool  ──query──▶ capability::CapableProviders
  → pick_assignee (sha256 rendezvous over the sorted pool)
  → WorkerRequest { assignee }
```

and for work nobody holds, the daemon claim lane already running in production:

```
SagaQuery::UnassignedPending ──▶ daemon ──▶ SagaMsg::Accept ──▶ first accept wins
```

Announcing the kind as a tag composes with this at the only seam that matters:
the tag string. A recipe whose `capability` is `compute` places on any node
whose compute daemon is enabled and signaling; a recipe whose `capability` is
`claude` places on any node that can run that executor. Both are the same
query. **Duplicating this would be the dual-path defect** — if a role-scoped
placement ever grows a second pool query, that is the smell to reject.

One honest limitation to record: `pick_assignee` picks **one** node. A k-of-n
placement (the assigned-k tier) is the same hash over the same sorted pool,
taking the top k for a blob digest instead of the top 1 for a saga id. That
generalization is cheap and does not need a module — see §6.

---

## 4. The flag day — measured, not guessed

### The mechanism (state it, because it is counter-intuitive)

`bin/node/src/host_state.rs:220-257` seeds the Lifecycle registry with
`sha256(component.wasm)` for every wasm tenant — `capability` at `:252-257`.
Lifecycle is a `MerkleStore`-backed module, so those digests are **consensus
state**. `Host::root_hash` is `global_root` over every registered module's root
(`crates/kernel/host/src/lib.rs:821-824`).

Therefore: **touching a module's SOURCE moves the genesis app hash, even when
the change is a read-only query variant whose own module root is provably
unchanged.** This was measured in wave-2 step 2 (`e899bbeb8`): genesis moved
`7909ee5e → dccfce1a` while saga's module root stayed byte-identical at
`af5570f5`.

### What each step of this plan moves

**Step A (announce the kind) — moves NOTHING.**
It touches `bin/node/src/validator/announce.rs` and `bin/node/src/services.rs`.
No file under `crates/modules/**` changes, so no `component.wasm` is rebuilt,
so no seeded digest changes, so the genesis root is byte-identical. The
registry's *content* changes at runtime, which is ordinary state, not a flag
day. This is the litmus test for the step: `git diff --stat crates/modules/`
must be empty.

**Step C (the role plane in `capability`) — moves the genesis app hash.**
- `crates/modules/system/capability/src/**` changes → `guest-builder` writes a
  new `component.wasm` → new `sha256` → Lifecycle's root moves → genesis moves.
- **`capability`'s own module root does NOT move at genesis.** Its store is
  empty at genesis regardless of code (its own tests derive the empty root that
  way, `lib.rs:659`), so a code-only change leaves it identical. The entire
  genesis movement comes from Lifecycle's seeded digest. Same pattern as
  wave-2 step 2.
- Blast radius outside consensus is small because `capability` is in
  `PRODUCTION` only (`crates/kernel/host/src/topology.rs:123-147`) — `SIM_BASE`,
  `SIM_VALSET` and `DEMO` do not compose it, so noded/simnode/demo are
  untouched.
- **Trap, from the wave-2 post-mortem:** `make wasm-modules` rebuilds all 16
  `BUILDER_MODULES` (`Makefile:101-109`) to different bytes because of
  pre-existing toolchain drift. Rebuild **only**
  `crates/modules/system/capability` and copy only its fixture, or you ship an
  unrelated 16-module flag day.
- Consequence: the live dukenet pair re-initializes at its next upgrade. The
  user has accepted exactly this before ("merge now, rebuild dukenet later"),
  and the credential grant on that pair was still TODO anyway.

**A `roles` module would move the same genesis hash and cost strictly more.**
Per `skills/module-dev/SKILL.md`: `MODULE_IDS` count bump, ~10 sites in
`host_state.rs`, `topology.rs` universe + `PRODUCTION` 20→21 + three pinned
count tests, a new guest + `BUILDER_MODULES` entry + fixture, registration in
noded/simnode/demo if it is to be sim-testable, **and** a new sibling wiring on
`saga` so placement can read it — `saga` currently knows exactly one registry
id. Post-genesis admission (`LifecycleMsg::ScheduleRegister`) does not rescue
this: the skill records that recovery/state-sync composers still enumerate a
fixed module set, so restore past an admitted module's first checkpoint fails
closed. **Same flag day, several times the surface, for state that fits in two
integers on a record `capability` already keeps. Rejected.**

---

## 5. Enrolled and autonomous — one discriminant, one match

### The discriminant

It lives on the grant, because the grant is already the per-kind consent record
(`bin/node/src/services.rs:111-130`):

```rust
/// How a granted service's standing is kept honest. A closed domain, not a
/// migration axis: both stances are products.
enum Stance { Autonomous, Enrolled }
```

`ServiceGrant` gains `stance: Stance` as a **required** field (no `serde`
default — `Services` is already `deny_unknown_fields` and validates on load, so
a grant with no stance fails loudly rather than defaulting into an obligation
nobody consented to). `services.toml` is local operator state, so this costs a
re-`enable` on dev workspaces and nothing more.

### The one match

Exactly one place branches on it — the announcer's op selection:

```rust
match grant.stance {
    Stance::Autonomous => announce_tags(offered),   // CapabilityMsg::Announce
    Stance::Enrolled   => role_ops(offered),        // RoleJoin / RoleLeave
}
```

No `_` arm, so a third stance would fail the build until routed. Nothing else
in the node reads the stance: both stances write into the **same** registry and
are picked by the **same** `assignment_pool` rendezvous. The stance changes
only (a) which op writes the entry and (b) whether departure is instant or
notice-bound. **A second placement lane keyed on stance would be the dual-path
defect — that is the rule to enforce in review.**

### Why this is not a compat shim

They are different products at different rungs of the assurance ladder, and
each has a shipped or planned instance that the other cannot serve:

- **Autonomous** = freedom, bilateral trust, no obligation, retract any time.
  The airlock lender plug (PR #818) is the shipped instance and is
  *structurally incapable* of being enrolled: it spawns nothing, skips the
  sandbox probe entirely, and is meant to run on "a laptop with no container
  runtime at all". Obligation windows on a laptop are a lie.
- **Enrolled** = obligation, notice-bound exit, auditable. This is what a
  durable storage shard needs, and nothing else in the design gives it.

Deleting either deletes a product. This is a closed two-variant domain enum,
permanently — not an old path and a new one.

### The role plane's shape (step C, when it is built)

Three additions inside `capability`, mirroring the module's existing
conventions (per-record key + sorted roster + count/byte caps):

- `CapabilityMsg::RoleJoin { role }` — external origin, `valset ∪ residents`
  gated exactly like `Announce`, records `joined_at = height`. Idempotent
  re-join stages nothing.
- `CapabilityMsg::RoleLeave { role }` — external origin, self-scoped, records
  `leaving_at = height`. **The record is not deleted.** The reads derive both
  sets from it:
  - placement set = members with `leaving_at == None`
  - obligation set = placement ∪ members with `height < leaving_at + NOTICE`
  Expired leavers are reaped **on write** when a later `RoleJoin` walks the
  roster — the prune-on-write shape the codebase already uses elsewhere
  (`gateway_ws_token`, cited as precedent in `bin/noded/src/services.rs:17-20`).
  No sweeper task, no lazy retention.
- `CapabilityQuery::RoleMembers { role }` — the current committed placement set,
  sorted, the same shape `Providers` returns so a k-of-n rendezvous can consume
  it unchanged.

`NOTICE` is a **chain constant** in the module, never self-reported — a
node-chosen notice period is not a notice.

**No `RoleEvict` in v1.** Governance can already remove a node from the valset,
and once the read-side membership filter below exists, that removal drops the
node from every placement set for free. A dedicated evict op is the v2
enforcement rung (auto-revoke), and the ideation already scheduled it there.

---

## 6. What this unblocks — and what it does not

### assigned-k durability: **CONFIRMED dependent**, on a smaller thing than "a module"

The rendezvous itself needs nothing new. It is `pick_assignee`'s hash over a
sorted pool (`saga/src/lib.rs:547-563`) generalized from top-1-over-saga-id to
top-k-over-blob-digest, reading the same sorted node list `Providers` already
returns.

What it genuinely cannot get from `capability` as it stands:

1. **A membership whose departure is not instantaneous.** Today a daemon
   restart empties the announce within one 30 s `HELLO_TTL`
   (`bin/noded/src/services.rs:32`; the retraction is deliberate and tested,
   `announce.rs:359-388`). Over a shard map, a 30-second flap re-maps every
   shard on the ring and triggers a network-wide re-replication storm for a
   node that came back. The notice period is the fix, and it must be consensus
   state — a node-local grace period is unauditable.
2. **A `joined_at` height.** An obligation is a window, and you cannot audit a
   missed shard without one.

So: **yes, assigned-k needs the enrolled stance. It does not need a `roles`
module** — it needs `RoleJoin`/`RoleLeave` and a `joined_at`/`leaving_at` pair
on a record `capability` is already keeping per node.

Two things assigned-k needs that are **out of scope here and must not be
smuggled in**:

- **Historical membership.** `RoleMembers` answers the *current* committed set.
  Proving "node X was in the storage set at height H-10000" needs retention of
  departed records, which is unbounded state growth. Deferred, explicitly.
- **A frozen doctrine it contradicts.** `crates/modules/system/blobstore/src/lib.rs:15-21`
  says, in the file: "SCOPE IS FROZEN (2026-07-13 storage-plane review) …
  anything needing replication, GC, authority, or auditability belongs in
  duckfs … two prior attempts to grow shared-byte planes beside duckfs were
  both deleted after converging on duckfs. don't start a third." assigned-k is
  a third plane. That is a user-level decision to make explicitly, not a side
  effect of this step.

### lease-singleton workloads: **REFUTED — no roles dependency**

The fencing mechanism is already in `saga`, in production:

- `SagaMsg::RenewLease` is gated to the current external assignee, refuses at or
  past expiry, throttles to the second half of the window, and **consumes no
  attempt** (`saga/src/lib.rs:923-959`). An alive holder can renew forever.
- An expired lease consumes one attempt and re-leases to a fresh rendezvous
  pick via the permissionless `Crank` (`saga/src/lib.rs:1089-1098`).
- `Reassign` bumps the attempt, fencing every late heartbeat and result from
  the old holder (`saga/src/lib.rs:960-1000`).

"No lease, no run" is therefore a **daemon-side rule over an existing op**: the
holder self-stops when it cannot observe its own `RenewLease` commit. RPO =
snapshot cadence, exactly as the ideation sold it.

What a lease-singleton actually needs and does not have, neither of which is a
roles question:

1. **A home for the `WorkloadSpec`.** `dispatch::Recipe` is close but wrong-shaped:
   its `OutputContract` is `Text | Json` over a *terminating* run
   (`dispatch/src/interface.rs:79-109`), not a long-lived process.
2. **An honest answer to finite `max_attempts`.** It is a `u32` and every lease
   expiry burns one, so a workload survives exactly `max_attempts` failovers
   and then `Failed`s with "lease attempts exhausted". That is a real ceiling
   to either raise deliberately or design around.

A workload runner does need a **capability tag** so placement can find it —
which step A delivers for free.

---

## 7. Defects found on the way (they ride these steps)

Three real problems, all in the blast radius, all found by reading the code
rather than by running it:

1. **A node removed from the valset can never retract its announce, and keeps
   receiving work.** `handle_announce` runs the member gate *before* everything
   else (`capability/src/lib.rs:295-308`), so an ex-member's empty-set removal
   is rejected too — its tags are stuck in the registry permanently. `Providers`
   does not filter by membership (`lib.rs:466-472`), so `assignment_pool` keeps
   rendezvous-assigning to a node that is no longer a member. Fix is read-side:
   filter the provider scans by `valset ∪ residents`. Touches `capability`, so
   it rides step C's flag day. *(I found no test asserting the bad behavior —
   the code path is unambiguous, but this has not been reproduced live.)*

2. **A rejected announce wedges the validator announcer permanently.** `decide`
   latches the in-flight pair and only clears it when the committed set matches
   (`announce.rs:136-158`). The drain pump un-latches only when the *submit*
   fails (`bin/node/src/validator/run/drain.rs:1152-1187`, un-latch at `:1177`)
   — there is no
   execute-rejection feedback, so an op that submits fine and is rejected at
   execute leaves the node silently announcing nothing forever. The resident
   path already does this right, with an applied/rejected reply and a retry
   (`bin/node/src/replica/park.rs:948-966`). Step A must close the asymmetry,
   because step A is what makes rejection likelier:

3. **`MAX_CAPABILITIES = 64` has less headroom than it looks.** The two
   built-in provider specs expand to 37 tags (16 + 19 `[[variants]]` plus 2 base
   tags), and the node announces the grant intersection of them today. Adding
   kind tags leaves ~25 tags of headroom before a stock host's announce is
   rejected at consensus — and per defect 2, rejected means silent forever. One
   operator spec dir with 25 variants crosses it. The hello boundary caps at
   512 (`bin/noded/src/services.rs:57`) and does not protect this. Step A must
   either bound the announce below 64 host-side with a loud refusal, or raise
   the constant — and raising it touches `capability`, so it would pull step A
   into the flag day. **Bound it host-side.**

Wave-2's record is that live QA caught three dead-on-arrival bugs of exactly
this class with unit gates green. Do not accept "gates pass" as done here.

---

## 8. Steps, cost, and what is irreversible

### Step A — announce the kind *(no flag day; do this now)*

- `bin/node/src/validator/announce.rs`: `granted()` stops hardcoding
  `COMPUTE_KIND` (`:76`) and folds every grant in `services.toml`; the offered
  set becomes `⋃over granted kinds (grant.capabilities ∪ {kind}) ∩ live hello`.
  The existing `grant ∩ live hello` invariant is preserved per kind — neither
  side may widen the other.
- Close defect 2 (execute-rejection feedback / un-latch) and defect 3 (bound the
  announce below `MAX_CAPABILITIES` with a nameable refusal reason).
- Tests: kind appears in the announce; two granted kinds union correctly; an
  absent daemon retracts only its own kind; an over-cap set refuses loudly
  instead of wedging.
- **Litmus:** `git diff --stat crates/modules/` is empty; genesis root
  unchanged.
- Cost: one host file plus tests. Small. Fully reversible.

### Step B — placement proof *(no flag day)*

A recipe whose `capability` is a kind places on a node running that kind, on
the dev-box ↔ macmini tailnet lane. No new code expected; if any is needed,
that is the signal that step A got the tag wrong. Cost: QA only.

### Step C — the role plane *(FLAG DAY; do this when assigned-k is being built)*

- `capability`: `RoleJoin` / `RoleLeave` / `RoleMembers`, the `NOTICE` chain
  constant, prune-on-write, and the read-side membership filter (defect 1).
- Split `capability/src/lib.rs` (1139 lines today) into `tags.rs` / `classes.rs`
  / `roles.rs` inside the same crate. This respects the ~600-line mono-file
  mandate, adds no crate, changes no module id, and is a structural refactor —
  so it is announced as its own step, not bundled silently.
- `bin/node/src/services.rs`: the `Stance` discriminant on `ServiceGrant`,
  `ducktape service enable --stance`, and the one `match` in the announcer.
- Rebuild **only** capability's `component.wasm` + its fixture.
- Cost: medium. **This is the irreversible part** — the genesis app hash moves
  and dukenet re-initializes. Get that agreed before wiring, per
  `skills/module-dev/SKILL.md`.

### Explicitly not in this plan

The assigned-k rendezvous, the storage service, the `WorkloadSpec` home, the
lease-singleton runner, `RoleEvict`, historical membership queries, and any
economics. §6 states precisely what each of them would need from this step; it
does not design any of them.
