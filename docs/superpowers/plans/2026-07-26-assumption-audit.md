# Assumption audit: the service plane's load-bearing claims, tested

- **Date:** 2026-07-26
- **Tree audited:** `origin/dev` @ `e0d773f68` (PR #818 airlock plug merged, #820
  build-gate deletion merged, #826 claim-lane merged). **`fix/announce-all-service-kinds`
  (#819) is NOT merged**, so several findings below are the defects it exists to fix.
- **Status:** read-only audit. Nothing was changed but this file.

## Why this document exists

A single question — *"doesn't `service enable` announce on chain?"* — exposed that
`commit_enable` is a local file write while the design says a user transaction
follows. Nobody had checked, across three merged PRs with zero review comments.

Every serious defect found that day came from the same place: **a doc comment, a
design claim, or a test name that asserted something the code did not do.** This
audit went looking for the rest, systematically.

It found **19 live defects, 12 guards that guard nothing, 24 stale or false
claims, and 8 doctrine violations.** Four findings were confirmed by mutating
production code and watching the guard stay green. Nine suspicions were refuted,
and every named cap in scope was checked against what real producers emit.

**Two of them are credential theft, and they should be fixed before anything else
in this document.** `crates/airlock/src/server.rs`'s grant gate authorizes a
*self-declared* account: `session()` takes no `HeaderMap`, so it cannot see the
mesh-verified caller identity the node's proxy injects, and instead trusts a
JSON field the caller chooses. The owner account it compares against is a public
field of the very record a borrower must already read. Any admitted network
member can therefore use any lender's credential (A15) — and overwrite it (A16).

The sharpest *methodological* result, because it is the thesis of this document
in one line: **a security lint whose doc says it scans "every `.rs` on the daemon
path" scans two of the three daemons**, and the one it misses is the one #818
added last week. A compiling private-key steal was planted in it and both guards
stayed green.

## How to read the evidence labels

The task asked for CONFIRMED (executed) vs PLAUSIBLE (read). That binary hid a
real distinction, so there are three:

| label | means |
|---|---|
| **EXECUTED** | a command was run, or production code was mutated and the guard watched. The strongest claim here. |
| **STATIC** | read from code where the fact is a single unambiguous expression — a hardcoded constant, a missing filter, a scan list. Not run, but not an inference either. |
| **PLAUSIBLE** | reasoned across several files. Could be wrong. |

Every mutation was reverted; `git status` is clean but for untracked plan docs.

---

## A. Live defects

Ranked by what bites hardest in production — **A15, A16, A17, A3, A2, A1, A12,
A18, A7, A8, A6, A19, A13, A4, A5, A14, A9, A10, A11**. Numbering is stable for
cross-reference, not an ordering. A15-A19 were found last and outrank everything
else: the first two are credential theft by any network member.

### A15. CRITICAL — the airlock grant gate authorizes a *self-declared* account

**The claim.** `crates/airlock/src/server.rs:494-496`: *"the session's claimed
account must be the owner or a granted account of the on-chain record. Refuse
before any handshake work — a session for an ungranted account never opens."*
And `bin/node/src/airlock.rs:19-24`: *"the node's route policy is a real
enforcement layer a direct bind would not have."*

**What the code does.** Every link verified in this tree:

1. `async fn session(State(st), Json(req))` (`server.rs:480-482`) — **takes no
   `HeaderMap`.** It structurally cannot observe transport identity.
2. The node's proxy *does* inject a mesh-verified identity:
   `bin/node/src/gateway_plane.rs:619` sets `x-duck-caller-account`, and
   `crates/modules/system/gateway/src/proxy.rs:420` proves a caller cannot forge
   it. Repo-wide that header has **6 hits — all producer or test. Zero readers in
   `crates/airlock` or `crates/services/airlock`.**
3. `grant_answer` (`server.rs:447-455`) keys entirely on `req.account_b64`, whose
   own doc says *"the account the caller **CLAIMS** to act on behalf of"*
   (`wire.rs:72-79`).
4. `credential_use_allowed` (`gateway/src/interface.rs:259-263`) returns true when
   `record.owner_account == account`.
5. `owner_account` is a **public field of `CredentialRecord`**
   (`interface.rs:220-227`) — the *same record a borrower must already read* to
   obtain `seal_pk` and `name`.
6. `bin/node/src/cred_cli.rs:513` publishes the route with
   `audience: RouteAudience::Network` — any admitted member — and
   `allow_authorization: true`.

**The exploit, complete.** Any admitted network member reads the public
credential record, copies `owner_account` out of it, POSTs
`{"sub":"<name>","account_b64":"<owner>"}` to `/session`, and receives a session
token granting full use of the lender's Claude/Codex subscription. No grant
needed. No discovery needed — step 5 hands them the account for free.
**`ducktape user cred grant` and `revoke` are decorative against a malicious
member.**

**CONFIRMED end-to-end** (each of the six links read and verified individually;
not run against a live pair).
**Fix shape:** extract `HeaderMap` in `session`; when `x-duck-caller-account` is
present, require hex-equality with `account_b64`, else refuse `account_mismatch`.
Production borrowers always traverse the proxy (`AirlockConfig::self_host` is
always `Remote`), so the header is always there.

---

### A16. HIGH — `POST /credential` is unauthenticated and network-reachable on the lender

**What the code does.** `server.rs:301-303` mounts `/attestation`,
`/credential` and `/session` on **one** router — so `/credential` inherits
exactly the `RouteAudience::Network` exposure A15 established.
`async fn credential(State(st), Json(up))` (`:414-440`) has **no auth extractor,
no grant check, no owner check**. It goes from `seal::unseal` straight to
`st.creds.lock().unwrap().insert(up.name, entry)`. Sealing is not a secret: the
seal public key is published on chain *and* served at `/attestation`.

**The failure it enables.** Any admitted member can **overwrite** a named
credential with an attacker-chosen bearer — and it persists, because the lazy
reloader only re-reads on artifact mtime change, which a POST does not touch.
Also: unbounded insertion into a map that never evicts, and one attacker-triggered
outbound OAuth POST to `console.anthropic.com` from the lender's IP per upload
(`needs_probe`, `:429-437`).

**CONFIRMED** (handler signature and mount read directly).
**Fix shape:** do not mount `/credential` on the self-host build — there the disk
store is the only legitimate writer.

---

### A17. HIGH — attestation never checks the DEBUG attribute, on either vendor

**The claim.** `crates/airlock/README.md:6-7` sells the property that *"the
operator of the credential side cannot read the credential out of it."*

**What the code does.** `verify.rs:168-194` (TDX) checks chain + TCB status +
MRTD. `:204-240` (SNP) checks VCEK-not-VLEK + chain + measurement.
A grep of the entire file for `debug`, `DEBUG`, `vmpl` and `policy` returns
**zero hits** — no TDX debug-attribute check, no SNP `policy` DEBUG bit, no
`vmpl`. The vendored dependency exposes exactly the missing bit; dcap-qvl's own
comment reads *"In TD debug mode, the CPU state and private memory are accessible
by the host VMM."*

**The failure it enables.** A malicious CVM operator boots the **same audited
image** with debug enabled: measurement unchanged, quote chains to real silicon,
verification passes, the credential is sealed to an "enclave" whose memory the
host can read.

**CONFIRMED that the check is absent** (zero grep hits). **PLAUSIBLE** for
exploitation — no TEE silicon here to demonstrate it on.
**Fix shape:** refuse a TDX quote whose debug attribute is set, an SNP report
whose `policy` DEBUG bit is set, and any `vmpl != 0`.

Compounding note: the SNP TCB-freshness gap is separately documented in-file as a
known limitation. With A17, SNP enclave trust rests on the launch measurement
**alone** — which makes the pair materially worse than either finding reads
separately.

---

### A18. `max_requests` — the signed per-session cap enforces nothing

**What the code does.** `Claims.max_requests` (`token.rs:17`) is written at
`server.rs:530` into the signed token and **read for a decision nowhere in the
repo** (verified by grep: every other hit is a config literal or a test fixture).
The live budget is `st.budgets.insert(req.sub, st.cfg.max_requests)`
(`server.rs:534`) — keyed on `req.sub`, the credential **name**, and reset
unconditionally on every `/session`.

**The failure it enables.** All borrowers of one credential share a single
counter, and any caller refills it by reopening a session. An operator reading
"4096 requests per session token" believes lending is bounded; it is not.

**CONFIRMED** by grep. Also belongs in §B — it is a cap that guards nothing.
**Fix shape:** spend the budget against the token's own claims, keyed on the
token, not on the credential name.

---

### A19. `seen_nonces` grows without bound, and its comment says otherwise

**The claim.** `server.rs:105-107`: *"Bounded by the request budget per name."*

**What the code does.** The insert (`:621-633`) runs **before** `open_request` and
before the budget spend — deliberately, per the review-fix comment at `:655-656`.
So a malformed body costs zero budget and still adds a permanent entry, the set is
never cleared, and A18 means the budget refills anyway. The fix for an earlier
review finding created this one.

**PLAUSIBLE. Fix shape:** bound the set by the token's expiry window and evict on
insert.

---

### A1. A rejected announce silences the node permanently — and logs success

**The claim.** `announce.rs:6-11`: *"state-driven (survives restart/late-join) and
idempotent: once the committed set matches, it stays quiet."*

**What the code does.** `decide()` latches `self.announced = Some(pair)` and
returns `None` for every later tick carrying the same pair
(`bin/node/src/validator/announce.rs:153-156`). The latch clears in exactly two
places: when the *committed* registry already matches (`:149`), or when
`node.submit` returns `Err` (`bin/node/src/validator/run/drain.rs:1178`).
`submit` returning `Ok` means *accepted for ordering*, not *committed*. There is
no execute-rejection feedback path at all.

**The failure it enables.** Any deterministic consensus rejection — over the
64-tag cap, roster full, node lost its valset standing, an operator spec minting
a tag `capability::validate_tag` refuses — leaves the latch set forever. The node
never re-announces, drops out of every rendezvous pool, and stops receiving work.
`drain.rs:1174` logs `info "capabilities announced"` on the very submit that was
thrown away, so the only trace is a success line. Recovery is a process restart.

The resident path already does this correctly, with an applied/rejected reply and
a retry (`bin/node/src/replica/park.rs`).

**STATIC** (the un-latch sites are enumerable; no live repro).
**Fix shape:** clear the latch on any tick where committed ≠ offered, or route the
execute outcome back the way the resident relay already does.

---

### A2. A dead compute daemon does not retract its announce if *any* other daemon is running

**The claim.** `announce.rs:20-26`: *"**live hello**: what a daemon is signaling to
this node RIGHT NOW … a stopped daemon retracts within the hello TTL."* The test
`a_daemon_that_stops_signaling_retracts_the_announce` (`announce.rs:359`) is named
for exactly this.

**What the code does.** `offered()` builds the live set by flat-mapping across
**every kind in the catalog**, with no kind filter:

```rust
let live: BTreeSet<&str> = signaling
    .iter()
    .flat_map(|entry| entry.capabilities.iter().map(String::as_str))
    .collect();                                   // announce.rs:108-111
```

then intersects it with the **compute** grant (`granted()`, `:76`).

**Why that is not academic.** `offered_capabilities` routes both `Daemon::Compute`
and `Daemon::Agent` to the same `discover_executors`
(`bin/node/src/services.rs:1222-1225`), which probes the same host and returns
`providers.capabilities()`. **The agent daemon's hello therefore carries a byte-identical
tag set to compute's.**

**The failure it enables.** On any node running both daemons — the shipped, documented
configuration — killing the compute daemon retracts nothing. The agent's hello keeps
every tag in `live`, the compute grant still intersects non-empty, and the node keeps
advertising compute capacity to the whole network. `saga` keeps rendezvous-assigning
runs to it and nothing executes them. This is precisely the availability failure the
wave-3 committed-grant plan (§4.1) says today's retraction protects against, and it is
already broken — which materially changes that plan's central trade-off.

Worse, `POST /v1/services/hello` is unauthenticated for an ungranted kind by design
(§8 of the grant-token plan). So **one local process posting a hello for kind
`zzz` with compute's tag list holds a dead node's announce alive indefinitely**,
for as long as it re-signals every 30 s. The catalog-squatting analysis assumed a
squatter must occupy the *granted* kind's row; it does not.

**STATIC** (the missing filter is one expression). The existing test never takes the
branch: it puts exactly one kind in the catalog.
**Fix shape:** filter the live set to the grant's own kind — `entry.kind == kind` —
inside the per-kind fold that #819 introduces.

---

### A3. Both guards on "a daemon must never hold the node's private key" are blind to the airlock daemon

**The claim.** `bin/node/src/services.rs:1613`: *"every `.rs` on the daemon path:
this file, plus **both** daemon module trees."* The lint's own doc lists three
things that "actually hold the line": the TYPE, the behavioural proof
`the_service_path_never_reads_the_node_key`, and the lint.

**What the code does.** `daemon_path_sources()` (`:1614-1620`) scans `services.rs`
plus `for module in ["compute", "agent"]`. `daemon_for` has had **three** arms
since #818, and `Daemon::Airlock => crate::airlock::serve(…)` (`:1161`) routes
into `bin/node/src/airlock.rs` — a flat file, not a module directory. It is never
scanned. `airlock.rs:52` already does `use crate::config;`.

**The failure it enables.** New airlock-daemon code can take `config::Resolved`
out of `config` — whose `signer` field is the node's ed25519 **private key** — and
every guard stays green. A daemon holding that needs no node surface at all: it
signs frames itself, which caps every authorization boundary wave 3 is designing
on `/v1` before that work starts.

**EXECUTED, twice, independently, with an A/B control.** A compiling steal
(`fn probe(p: &Path) -> Option<config::Resolved> { config::resolve(p).ok() }`) was
added to `airlock.rs`:

```
cargo test … services::tests::the_daemon_path_cannot_name_the_node_key  => PASSES
cargo test … the_service_path_never_reads_the_node_key                  => PASSES
```

The **identical** steal added to `bin/node/src/compute/mod.rs` (a scanned file)
FAILED with `compute/mod.rs takes 'Resolved' out of 'config'`. So the matcher is
sound; the scope list is stale. Both files were restored exactly.

**Fix shape:** add the airlock daemon's sources to `daemon_path_sources()`, and
derive the list from `daemon_for`'s arms rather than a hand-written array so the
next daemon cannot be forgotten.

---

### A4. Only the compute grant is ever announced — agent and airlock are invisible to the network

**What the code does.** `granted()` hardcodes `COMPUTE_KIND`
(`bin/node/src/validator/announce.rs:76`).

**The failure it enables.** #818 shipped the whole airlock lender plug —
`AIRLOCK_KIND`, `Daemon::Airlock`, `scopes_for`, a consent screen, an instance id
— and a node that enables it announces **nothing**. There is no `airlock` tag in
the registry, so no borrower can discover a lender through `capability`. The same
holds for `agent`.

**STATIC.** This is what #819 fixes; it is recorded here because #819 is not merged
and A2 shows the fix needs a per-kind live filter it may not currently carry.

---

### A5. `disable` tells the operator the announce is being retracted, for two kinds that were never announced

**The claim.** `bin/node/src/services.rs:1495-1498` prints, for every kind with a
first-party daemon: *"the node retracts its announce on the next tick (the grant is
re-read there)"*.

**What the code does.** The gate was generalized from `kind == COMPUTE_KIND` to
`daemon_for(&kind).is_some()` (`:1499`) when airlock was added — but the announcer
it describes still reads only the compute grant (A4). For `agent` and `airlock`
the sentence is false in both halves: no announce is retracted, and no grant is
re-read.

**STATIC. Fix shape:** print the retraction claim only for kinds the announcer
actually folds in — or fix A4 and make it true.

---

### A6. `disable` + `enable` permanently orphans every container from the previous consent epoch

**The claim.** `bin/node/src/compute/mod.rs:205-208`: *"ONE sweep, and no
retired-flat-label arm: this daemon's graph root is private now, so a pre-daemon
`capability-host` container lives in the node's OLD root and is not enumerable
through this socket. **Unreachable by construction, not pending cleanup.**"*

**What the code does.** Two identifiers with different lifetimes:

- the podman graph root is `podman_data_dir(service, **kind**)` —
  `<storage>/services/compute` (`bin/node/src/services.rs:60-62`). Keyed on kind,
  **stable across re-enables**.
- the reap label is `managed_label(&grant.display_id())` = `io.ducktape.managed=compute#<hex8>`
  (`compute/mod.rs:217`, `agent/mod.rs:154`). Keyed on the instance id, which
  `mint_instance` derives from a **fresh nonce on every enable** — deliberately, as
  the consent-epoch marker.

`reap_by_label` is the only container reaper in the tree (`reap_service_at` reaps
the podman *service process*, and is called from a test helper only).

**The failure it enables.** After `ducktape service disable compute && ducktape
service enable compute` — the documented recovery for a chain re-init, and the
only way to rotate consent — the new daemon boots, sweeps `compute#<new>`, finds
nothing, and starts serving. Every container wearing `compute#<old>` is still
running in the *same* graph root, fully enumerable through the *same* socket,
holding CPU, memory and image layers, and **no code path will ever remove it**.
The quoted justification covers only the pre-daemon flat label; it does not cover
a retired epoch of the same kind, which lives exactly where the new one does.

**STATIC. Fix shape:** at daemon boot, reap every `io.ducktape.managed=<kind>#*`
that is not this instance — the graph root is already private per kind, so that
sweep can only touch this service's own containers.

---

### A7. The origin allowlist's exact-match property is untestable as written

**The claim.** `origin_guard.rs` in-test comments: *"the prefix trap: a hostile host
that merely STARTS with our allowlist"*, and *"a DIFFERENT host than `localhost`,
and the reason the check is not a substring match."*

**What the code does.** Every hostile origin is checked against an **empty**
allowlist (`origin_allowed_with("http://localhost.evil.com", &[])`). With
`allowed` empty, `.iter().any(..)` is `false` for any predicate, so the
exact-vs-prefix distinction is never reached. The one non-empty case (`:118`)
compares `…1421` against `["…1420"]` — neither is a prefix of the other.

**The failure it enables.** Replacing `candidate == origin` with
`origin.starts_with(candidate.as_str())` — the classic port-suffix/subdomain hole
this file exists to close — reopens the control plane to hostile web content, and
the suite stays green.

**EXECUTED:** that one-line mutation applied; `cargo test -p noded --lib origin_guard`
→ **3 passed**. Reverted.
**Fix shape:** one case with a non-empty allowlist and an origin that *extends* an
entry (`allowed=["http://localhost:1420"]`, origin `http://localhost:14201`).

---

### A8. The credential vendor mapping is unguarded — a Claude session can be pointed at a Codex gateway

**The claim.** `crates/services/agent/src/lib.rs:574`, in its own words: *"a silent
mis-map here would send a Claude session to a Codex gateway."*

**What the code does.** The test compares `airlock_config(wire::Credential{ kind })`
against `AirlockConfig::self_host(&resolved)`. `self_host`
(`crates/services/broker/src/lib.rs:1441`) reads `authority`, `via`, `seal_pk`,
`name`, `account` — and **never reads `kind`**. The one field the test is about is
dropped by the function it compares through, on both sides of the `assert_eq!`.

**EXECUTED:** the arms of `airlock_config` were swapped
(`Claude => CredentialKind::Codex, Codex => CredentialKind::Claude`);
`cargo test -p agent-service --lib` → **8 passed**. Reverted.
**Fix shape:** assert on the mapped `CredentialKind` directly, before it reaches
`self_host`.

---

### A9. A saga with no provider and no deadline parks in consensus state forever

**What the code does.** When the provider pool empties between attempts,
`lease_and_request` re-leases with `assignee = None`;
`lease_expiry(_, None, None)` returns `None` (`saga/src/lib.rs:351-357`), so the
new attempt carries no lease and no expiry. If the trigger set no `deadline`,
`Crank`'s guard `if !deadline_hit && !lease_hit { continue; }` (`:1068-1072`) can
never fire again. `Prune` refuses it (non-terminal, `:1149`) and it drops out of
`NextExpiry` (`:1164-1176`), so the host crank pump stops looking.

**EXECUTED** (probe run against the real module, then reverted):

```
C crank@64:  status=Pending attempt=1 assignee=None lease=None
C crank@128: status=Pending attempt=1 assignee=None lease=None
C crank@192: status=Pending attempt=1 assignee=None lease=None
```

**Failure enabled:** a permanent consensus-state leak, one record per orphaned
dispatch. Reachable today from A2/A4: a node whose announce is wrong is exactly
how a pool empties mid-saga.
**Fix shape:** an unassigned re-lease keeps the previous window (or falls back to
`DEFAULT_LEASE_VIEWS`) so `Crank` can still terminate it.

---

### A10. A node removed from the valset can never retract, and keeps being assigned work

**What the code does.** `handle_announce` runs the `valset ∪ residents` gate
(`capability/src/lib.rs:295-308`) **before** the empty-set removal branch
(`:319-336`), so an ex-member's retraction is rejected on the same gate as a
forgery. `CapabilityQuery::Providers` (`:464-472`) and `CapableProviders`
(`:481-496`) walk the roster filtering on tags only — **no membership check** —
and `saga::assignment_pool` feeds that straight into `pick_assignee`.

**Worse than previously recorded:** `handle_announce` is the *only* writer of a
node record and `CapabilityMsg` has exactly two variants. There is **no
administrative removal path at all**. An ex-member's tags are permanent consensus
state that not even governance can delete, and work keeps being assigned to it.

**STATIC. Fix shape:** filter the provider scans by `valset ∪ residents` on the
read side — one change, and it makes valset removal drop a node from every pool
for free.

---

### A11. `SagaMsg::Reassign` errors where every sibling no-ops, and a module emits it as a same-block follow-up

**What the code does.** `saga/src/lib.rs:972-995` returns `Err` on three paths
(pinned saga, attempts exhausted, no alternate assignee). Every sibling op does the
opposite *with the reason written in-line*: `Cancel` — *"never an error (a finalized
foreign cancel must not abort the block)"* (`:1117-1119`); `Accept` (`:1006-1010`)
and `OracleResult` (`:862-865`) the same. And `dispatch/src/lib.rs:638-646` emits
`SagaMsg::Reassign` via `ctx.emit_msg` — the same-block follow-up shape the saga
header names as the poison class (`:29-35`).

**Failure enabled:** `ReassignDispatch` on a saga whose attempts are spent aborts
the whole finalized block, discarding every unrelated op in it.
**STATIC. Fix shape:** make the three `Err`s deterministic no-ops, like their siblings.

---

### A12. The skill cap is enforced at 64 against a list that can legally hold 128

**The claim.** `crates/services/compute/src/soul.rs:82-89` — the assembler's cap is
*"the **SAME number** consensus enforces on an agent's curated list
(`agent::MAX_SKILLS_PER_AGENT`), deliberately re-exported rather than restated:
**two caps that could drift is how you get a record consensus happily accepts and
no run can load.**"* `Cargo.toml:12-14` repeats it.

**What the code does.** Two *independent* 64-caps feed one list, and the
re-export unifies neither:

- `crates/modules/apps/agent/src/lib.rs:304` — the agent record's curated list ≤ 64.
- `crates/modules/apps/runs/src/envelope.rs:258` — a requester's per-run library
  names ≤ 64, **all of them `OnDemand`**.
- `runs/src/envelope.rs:225-232` `resolve_skills` **unions** the two.
- `crates/services/compute/src/envelope.rs:272-278` applies **no count cap** at decode.
- `soul.rs:145` refuses at `on_demand.len() > 64`.

**The failure it enables.** An agent curating 1 on-demand skill plus a
`RequestRun` naming 64 library skills = 65 on-demand. Consensus **commits** the
run; `bin/noded/src/agent_provision.rs:337-349` performs up to **128 duckfs
checkouts**; then the assembler refuses. Headroom is **negative by up to 64** —
exactly the drift the doc says the re-export prevents. The refusal text
(*"curate fewer, or leave them in the shared skill library"*) then blames the
wrong party: the skills already are in the library, the requester's per-run list
is the cause, and the agent's owner cannot act on that advice.

Secondary: for a curated-only record the check is **vacuous** (on-demand ⊆ total
≤ 64), and its stated justification — *"a record registered before the cap
existed still carries whatever it carries"* — is a backcompat rationale for a
tree with zero live networks.

**STATIC. Fix shape:** cap the union in `resolve_skills`, and reword the refusal
to name the per-run list.

---

### A13. `SpawnKind::TeardownOwner` is documented as an isolation obligation the only production host does not honor

**The claim.** `crates/services/compute/src/pool.rs:58-61`: *"**Hosts must isolate
`SpawnKind::TeardownOwner` from shared runtime workers.**"*

**What the code does.** The one production host collapses both arms
(`bin/node/src/compute/mod.rs:264-271`):

```rust
SpawnKind::Queued | SpawnKind::TeardownOwner => { tokio::spawn(future); }
```

onto the daemon's shared multi-thread runtime, justified by *"here the whole
process is that lane's owner"* — which does not follow: owning the process does
not stop a **blocking `Drop`** from occupying a shared tokio worker.

**And the test wires the isolation production lacks.** `pool.rs:1815-1843`
(`forced_owner_drop_keeps_the_runtime_responsive_and_admission_held`) gives
`TeardownOwner` a **dedicated OS thread with its own current-thread runtime**.
`DispatchPool::with_limit` has exactly one production call site
(`compute/mod.rs:306`); the other two are tests. So the green test asserts a
property the shipped binary does not have — the same replica-instead-of-call-site
defect the airlock plug was just fixed for.

**The failure it enables.** Podman `stop`/`wait` under `kill_on_drop` and Tart VM
deletes are the modelled blocking teardowns. Up to
`DEFAULT_MAX_CONCURRENT_RUNS = 4` simultaneous teardowns can occupy 4 shared
workers; on a 2-4 core box that starves the runtime driving the daemon's
`/v1/query` intake.

**STATIC. Fix shape:** give `TeardownOwner` its own thread in `compute/mod.rs`
(mirroring the test), or delete `SpawnKind` and the "must isolate" claim.

---

### A14. `close_all`'s epoch does not close the create/forget window "by construction"

**The claim.** `crates/services/agent/src/lib.rs:96-100`, `:183-188`: *"a pty that
finishes starting under a stale epoch is one the node has already forgotten, so it
is torn down instead of registered. **Without this the sweep cannot see a session
that is not in the map yet**… Bumping first is what makes that create notice, on
its way out."*

**What the code does.** The epoch is loaded at `:310`; the map insert happens at
`:322-333` — two separate critical sections. `close_all` does `epoch.fetch_add`
(`:188`) and then takes the sessions lock (`:191`). The daemon runs a
**multi-thread** runtime (`bin/node/src/agent/mod.rs:69`) and `TermCreate` is
dispatched on its own spawned task (`bin/node/src/agent/link.rs:241-244`), so
*create reads epoch → close_all bumps and snapshots (empty) → create inserts* is
genuinely reachable. There is no `.await` between the two, but there is true
parallelism across threads.

**The failure it enables.** A session registered under an epoch the node has
already forgotten: no `TermCreated` reader, no close path, an orphaned container
burning the operator's subscription until `MAX_SESSION_LIFETIME` (4 h).

**PLAUSIBLE** (narrow window; not reproduced).
**Fix shape:** re-check the epoch **inside** the sessions-lock critical section at
`:322`.

---

## B. Guards that guard nothing

A test named for a property it does not exercise is worse than no test, because it
stops anyone looking. A3, A7, A8 above are in this category too — they are listed
as live defects because a real hole sits behind each.

| # | guard | name claims | body does | evidence |
|---|---|---|---|---|
| B1 | `announce.rs:420` `empty_tags_force_empty_resources` | empty tags force empty resources | `decide(&[], &[], &{})` hits the *genesis-silence* early return; `effective_resources`' value is computed and discarded. Deleting the invariant leaves it green (the sibling `a_daemon_that_stops_signaling_retracts_the_announce` goes red instead — the rule is covered, by a differently-named test) | **EXECUTED** |
| B2 | `announce.rs:359` `a_daemon_that_stops_signaling_retracts_the_announce` | a daemon stops signaling | constructs a **new** empty catalog and a **new** announcer instead of advancing past `HELLO_TTL`. TTL lapse never happens; the fresh announcer's latch is `None`, so the retract-while-latched transition is skipped too. Also the only reason A2 is invisible: one kind, ever | STATIC |
| B3 | `services.rs:1740` `the_daemon_path_cannot_name_the_node_key` | the daemon path | 2 of 3 daemons — see A3 | **EXECUTED** |
| B4 | `noded/services.rs:794` `no_admission_path_reads_this_node_s_build_stamp` | no admission path | scans `stream.rs` + itself. The real link admission is `TerminalSessions::attach` in `term.rs:684`, never scanned. A reintroduced `if build_identity().is_none() { return None }` inside `attach` defeats the lint *and* passes the behavioural half | PLAUSIBLE |
| B5 | `agent/mod.rs:196`, `compute/mod.rs:376` `the_managed_label_separates_agent_from_compute` / `…scoped_to_one_service_instance` | neither reaper can see the other's containers | assert `managed_label("agent#deadbeef") == "io.ducktape.managed=agent#deadbeef"`. Pure string-formatter tests wearing an end-to-end name; `reap_by_label` filtering is never exercised. This is the pair that should have caught A6 | STATIC |
| B6 | `capability/src/lib.rs:836` `root_is_state_based_order_independent` | order independence | its own body admits *"the production qmdb root is op-log-derived"* — the named property is false on the store production runs, and is asserted on a `MemStore` where it holds by construction | STATIC |
| B7 | `sandbox/src/sandbox.rs:465` `tart_concurrency_cap_is_two` | a concurrency cap | `assert_eq!(TART_MAX_CONCURRENT, 2)` — a constant equal to itself — plus the semaphore's initial permit count. Deleting the single acquire site (`provider/src/lib.rs:1331`) leaves it green | STATIC |
| B8 | `capability/src/lib.rs:809`, `:1037` `malformed_*_are_rejected` | a specific rule rejects | `matches!(err, Error::Module(_))`. *Every* rejection in the module is `Error::Module`, including the member gate — so these pass if the wrong rule fires. Sibling tests in the same file match on substrings and do it right | STATIC |
| B9 | `provider/src/lib.rs:4136` `every_backend_accepts_the_credential_broker_shape` | accepts | `if let Err(e) = … { assert!(!e.contains("cannot host a credential broker")) }` — nothing asserted on `Ok`, and any *other* error passes. "Accepts" is never positively established | STATIC |
| B10 | `soul.rs:49-53` | the library prefix is *"re-exported … **never restated here**"* | `SKILL_LIBRARY_SECTION` (`:64`) is a `&'static str` hardcoding `/shared/skills` **four times**; the `pub use` never reaches it. Drift *is* caught — but by a test asserting `doc.contains(SKILL_LIBRARY_PREFIX)`, not by the re-export the comment credits | STATIC |
| B11 | `pool.rs:84-86` | a resolve `Err` means *"unknown credential, non-account origin, ungranted account"* | the enumeration looks exhaustive and is not. `bin/node/src/compute/cred.rs:57-59` propagates every `/v1/query` transport failure with `?`, so **node down**, **module absent** and **unreadable reply** all fail the attempt as the same `Err`. Only `authorize` emits a stable token (`credential_not_granted`, `cred.rs:190`); the rest is prose. This is the known *"the lender refuses you"* vs *"the lender's node did not answer"* defect, one layer down | STATIC |
| B12 | `bin/node/src/agent_plane.rs:213-217` | — | a forever-retry loop with **no logging at all**: `Err(_) => { sleep(RETRY).await; continue; }` retries `service.open` every 3 s silently, forever. `fanout_loop` (`:179-198`) re-spawns forever with no counter either. Doctrine's inverse failure — a peer stream that can never open is completely invisible | STATIC |

Also weak, not counted above: `compute/intake.rs:624` `…and_stays_due_until_it_is_sent`
(nothing is ever sent), `agent_cli.rs:650` `cred_kind_wins_and_contradiction_is_an_error`
(tests a 2-arm enum map; neither named branch is called), `agent_cli.rs:695`
`term_ended_frame_ends_the_attach` (tests the predicate, never the loop that must break on it),
`services.rs:2076` `the_hello_cap_admits_a_real_hosts_capability_set` (synthesizes `tag0..tagN`,
so the real tag strings never meet `MAX_ITEM_LEN`), `announce.rs:407`
`a_direct_backend_announcer_carries_no_resources` (empty in, empty out).

---

## C. Stale or false claims

Different response required: these need a doc edit, not a code fix — but each one
is currently teaching the next reader something untrue.

| # | location | the claim | the truth |
|---|---|---|---|
| C1 | `services.rs:1181-1183` | *"Compute's list is empty here and stays that way until its own seams are audited"* | ten lines below, `scopes_for` returns `["saga.runs", "credential.lent"]` for Compute (`:1194`). The doc contradicts the code inside the same doc comment |
| C2 | `services.rs:1613` | *"both daemon module trees"* | three daemons since #818 — the live hole in A3 |
| C3 | `noded/services.rs:118-125` | *"`ClientMsg` and every `agent_service::wire` type carry `deny_unknown_fields` **and default nothing**"* | `Hello` itself defaults three of its six fields (`capabilities`, `scopes`, `needs`, `:80-93`), and `ClientMsg::Subscribe.resume` defaults. This is the justification #820 used to delete the build gate, so it is load-bearing prose |
| C4 | `capability/src/lib.rs:19` | *"without a valset (**the single-node daemon**) any external key may self-announce"* | no such composition exists. `capability` is in `PRODUCTION` only; `SIM_BASE` (the daemon set) does not contain it. The ungated branch exists solely for unit tests, and its doc names a deployment that cannot exist — over a security-relevant branch |
| C5 | `capability/src/lib.rs:85-86` | `MAX_CAPABILITIES = 64` is *"far above any real host's executor count"* | the real producer emits **37** — 58% of the cap — proved by a passing test, `provider/src/variants.rs:622` (`2 bases + 19 codex + 16 claude`). One more CLI family of comparable size lands at 56; two puts a stock host over. Over-cap rejects the *whole* announce, which then triggers A1 and the node goes dark permanently |
| C6 | `capability/src/interface.rs:17-23` | the class plane is *"the primary router for that address space"* and *"the single source of truth"* | `ClaimClass`, `ResolveClass`, `Classes`, `parse_classed_address`, `class_of`, `NodeSelector`, `MAX_CLASS_LEN`, `CLASS_ROSTER_KEY` have **zero non-test callers repo-wide**. Dispatch routes on the plain tag. The no-unclaim rule justifies permanent unrevocable consensus state by pointing at "every address already routed through it" — addresses that do not exist |
| C7 | `saga/src/lib.rs:493-495` | the pool is the registry's *"committed"* view | capability's `query` reads through `StagedStore` (`capability/src/lib.rs:459-462`), so an `Announce` staged earlier in the same block is already in the pool. Deterministic, so not a safety bug — but it is the reasoning error that produces one later |
| C8 | `saga/src/lib.rs:565-566` | `pick_assignee` runs *"over the sorted assignment pool"* | the pool *is* sorted today (capability and valset both maintain sorted rosters), but nothing at the pick site sorts, asserts, or documents that as a contract. A `Providers` that ever returns hash order silently reshuffles every assignee on the network with no test failing |
| C9 | `topology.rs:113` | the file is the single source for module wiring | it declares `saga` `wiring: NONE` while `saga/src/guest.rs:52-63` wires `valset` + `capability` and calls itself *"EXACTLY the production wiring"*. The file's stated purpose is preventing exactly this drift |
| C10 | `provider/src/lib.rs:561-564` | *"a compute daemon and **(later)** an agent daemon"* | the agent daemon shipped; "later" is now |
| C11 | `2026-07-25-service-daemons.md` | *"`disable compute` then reaps exactly compute's containers"* | `disable` reaps nothing — it removes a record and prints (`services.rs:1479-1506`). Honestly restated in the grant-token plan §4.5, but the wave-2 design still says it |
| C12 | `pool.rs:3472` | *"a rename in EITHER crate must fail THIS test"* | `compute-service` has no `runs` dependency; the test deserializes into locally-declared mirrors. A rename in `runs` changes nothing and this stays green. Structurally forced, but it is a one-sided pin sold as a contract test |
| C13 | `compute/src/lib.rs:17-18`, `Cargo.toml:9` | *"**NO credentials are touched** (BYO CLI auth)"* / *"no credential handling"* | `pool.rs:87-119` defines `CredentialResolver`/`Resolved` and `resolve_credential_into` sets `prepared.ctx.airlock` before the provider spawns; `envelope.rs:82-84` carries the `credential` field. Added by `f130fb7aa` without updating the header. The true statement is "no *secrets* cross; a named credential is resolved into `ctx.airlock` on the executing node" |
| C14 | `provision.rs:42-44` | *"`false` on an envelope composed **before the field existed**: the conservative default"* | `library_readable` is a **required** field of `WireEnvelope` (`envelope.rs:74`, no `serde(default)`, `deny_unknown_fields`) — an omitting envelope fails decode. The `serde(default)` existed historically and was deleted. A backcompat justification for a path that no longer exists, in a tree that forbids the path |
| C15 | `provision.rs:262`, `soul.rs:10`, `pool.rs:569`, `pool.rs:2824`, `cred.rs:195-196` | six references to `capability-host` | the crate is **`provider-host`** since `788f679ea`. (`compute/mod.rs:207`'s `capability-host` is a historical *container* name and is fine) |
| C16 | `soul.rs:58` | cites `bin/mcp/src/tools/read.rs` | `bin/mcp` was deleted in the CLI unification; it is `bin/node/src/mcp/tools/read.rs` |
| C17 | `agent/src/wire.rs:167` | *"`SpawnFailed`: the interactive spawn **itself** failed (image absent, podman error…)"* | `lib.rs:279-284` returns it for a **malformed session id** (the node's own protocol violation) and `:310-317` for *"the node link dropped while starting"* — the `no_agent_service`/503 condition. The repo's 503-vs-`spawn_failed` diagnosis ladder loses a rung across the process boundary; only free-text `detail` saves the operator |
| C18 | `pool.rs:8-9` | *"minutes-long under a **300s default timeout**"* | 300 s is the **IDLE** window, refreshed by any child output (`provider/src/lib.rs:546-548`); the absolute cap is `idle × HARD_TIMEOUT_FACTOR` where the factor is **36** (`provider/src/lib.rs:70`). True ceiling **3 hours**. Anyone sizing a saga deadline off this header is off by 36× |
| C19 | `agent/mod.rs:21` | *"after this, **`bin/node`** constructs none of them"* | `bin/node` still constructs provider sets in three places (`services.rs:1217`, `compute/mod.rs:105`, `agent/mod.rs:100`). The intended claim — that the `node run` path constructs none — is TRUE and is stated correctly at `compute/mod.rs:3-5`; only this wording is wrong |
| C20 | `agent/src/lib.rs:171-174` | `live()` is *"how many sessions are **live**"* | it returns `active`, whose own doc at `:91-94` says **"reserved-or-live"**. A create stuck in a cold image pull counts as live on the status line |
| C22 | `crates/airlock/README.md:150-152` | *"the token-signing and seal keys are memory-only, so every outstanding token and body key dies with the process"* | **false on the shipping self-host lender** — `seal.key` is written to disk by design (`services/airlock/lib.rs:302`). Two consequences: no forward secrecy (whoever later reads it decrypts recorded sealed bodies and can impersonate the gateway), and **revoking a grant on chain does not end a live session** — the gate runs only at `/session`, never in `proxy_inner`, for the full 3600 s TTL |
| C23 | design docs + `noded/services.rs:118-125` | *"the protocol decodes every frame at its boundary, so skew degrades to a named refusal"* | holds for all 16 provider/broker/gateway config structs, and **fails completely for airlock**: `crates/airlock/src/wire.rs:11-87` has **seven tolerant wire types and zero `deny_unknown_fields`**. Worse, `SessionRequest.account_b64` is `#[serde(default)]`, so a client typo silently becomes `None` → a 403 `credential_not_granted` — the exact misdiagnosis the three-state `GrantAnswer` taxonomy was built to prevent |
| C24 | `crates/airlock/src/wire.rs:17-18` | *"`Claude` → Anthropic, `Codex` → OpenAI"* | env overrides falsify it (`services/airlock/lib.rs:97-100` lets env vars redirect the lender's real credential), and `server.rs:710` plants the real access token on a request to whatever host the env named. Same root cause as A8, second victim |
| C21 | `pool.rs:553-570` | — | 18 lines documenting `execute`'s entire contract (provision → bind → run → commit → assemble → cleanup, cancellation, R4 degradation, the #298 threshold) are `///`-attached to **`fn workspace_step_timeout()`** (`:571`). `async fn execute` (`:579`) has no doc comment at all, and rustdoc renders the run bracket's contract as the docs for a `Duration` helper |

---

## D. Doctrine violations

The repo's rules are not advisory; these are defects by the repo's own definition.

- **D1 — `WORKER_CONTROL_VERSION` is a version-keyed admission gate.**
  `saga/src/interface.rs:242-246` declares it with *"bump before changing the
  meaning of any existing command"*, and `:417-427` refuses decode on mismatch
  (`"unsupported worker control version"`). CLAUDE.md: *"no protocol-version bumps
  … no admission gates keyed on a version number."* With zero live networks a
  control effect that fails to decode is a bug, not skew. `WORKER_CONTROL_KIND`
  alone already does the disambiguation the check was added for.
- **D2 — `RouteStatement.version` is signed but never validated.**
  `gateway/src/interface.rs:161-162` declares it; `:607` folds it into the signed
  preimage; `validate_route_statement` never checks it. Repo-wide it has exactly
  one use: that push. An unconstrained free byte inside a signed preimage — two
  publishers stamping different values produce different signatures over
  semantically identical routes, both accepted.
- **D3 — omit-and-default wire fields are decode tolerance.**
  `dispatch/src/interface.rs:160-163` and `:236-238` (`admission`), `saga/src/interface.rs:175-177`
  (`usage`) carry `#[serde(default, skip_serializing_if)]`, and dispatch *pins*
  the tolerance with `queue_admission_is_omitted_and_defaults_on_decode` (`:357-387`).
  The sibling module holds the opposite position on purpose:
  `announce_requires_the_resources_field` exists so a tags-only node must send an
  explicit `"resources":{}`. Two modules in one seam, opposite wire policies.
- **D4 — `MAX_CAPABILITY_BYTES` is a second source of truth.**
  `saga/src/interface.rs:63-68` hand-copies `64` with the comment *"matches the
  capability registry's own tag cap"*. Saga already depends on `capability`, and
  the sibling got it right: `dispatch/src/interface.rs:40` writes
  `= saga::MAX_RESULT_BYTES`. Two things kept in sync by a comment is the defect.
- **D5 — `CredentialKind` hardcodes vendor names into consensus state.**
  `gateway/src/interface.rs:204-206`: `enum CredentialKind { Claude, Codex }`,
  borsh-tagged, inside `CredentialRecord`, therefore inside the root hash. The
  capability registry forbids exactly this one crate over — *"tags are open-set
  strings so a new kind of executor is data, not code"* — and the provider specs
  are proud that no executor name appears in their code. A third vendor is a
  consensus module change and a genesis flag day.
- **D6 — `eprintln!` on the commit-failure path.**
  `crates/services/compute/src/pool.rs:701` and `:718`:
  `eprintln!("[oracle] commit failed for {}: {e}", spec.run_id)`. These are the
  **only** two `println!`-family calls in the whole compute/agent service surface,
  and they fire on the degraded-receipt path — precisely the event an operator
  needs in the Logs tab, where it is invisible and unfilterable by `RUST_LOG`.
  `[oracle]` also names a module retired two renames ago
  (`dispatch-oracle → dispatch-host → compute-service`).
  Fix: `tracing::warn!(target: "ducktape::saga", run = %spec.run_id, reason = "commit_failed", …)`.
- **D8 — "one secret writer" is asserted where there are eight.**
  The grant-token plan (§4.3) says *"`mint_link_token` already has the right
  helper (`noded/services.rs:151-161`)"*, and the token plan's §2.1 says to reuse
  it. In fact the tree has **eight independent hand-rolled 0600 writers**:
  `bin/noded/src/services.rs:232`, `crates/services/airlock/src/lib.rs:316`,
  `bin/node/src/userkey.rs:363`, `bin/node/src/config/identity.rs:55`,
  `bin/node/src/config/mod.rs:498`, `bin/node/src/config/invite.rs:120`,
  `crates/networking/reachability/src/keys.rs:55`, and
  `crates/services/provider/src/lib.rs:1253`. The duplication is *acknowledged in
  a comment* — `crates/services/airlock/src/lib.rs:309` reads *"mirrors
  `userkey::write_user_key_new`"* — which is the tell.
  They are **not equivalent**, which is why this is a defect and not tidiness:
  `noded/services.rs` pre-`remove_file`s and therefore overwrites;
  `services/airlock` is `create_new`-only and unlinks on write failure;
  `provider/src/lib.rs` chmods *after* `fs::write`. Three semantics for one
  primitive, chosen ad hoc at each site. Note the crate boundary makes naive
  reuse impossible — `crates/services/*` cannot depend on `bin/noded` — so the
  fix is to put the helper in a crate both sides can see, not to copy it a ninth
  time.
- **D7 — a `v3` protocol name survived the v1 flag day.**
  `crates/services/compute/src/envelope.rs:207` (*"a minimal valid **v3** run
  envelope"*) and `:385`. `RUN_ENVELOPE_VERSION = 1` (`:32`) and `prepare` rejects
  anything else. Written in `1aef2bfe5`, which predates `949c69d32` *"drop
  legacy/compat code — no protocol versioning, one codec, everything v1"*. These
  are the only `v2`/`v3` hits in the audited surface. Fix: `s/v3/v1/`.

---

## E. Answers to the specific items flagged as unverified

| item | verdict |
|---|---|
| `grant.scopes` is rendered but never read — is a grant decorative? | **CONFIRMED decorative.** Every consumer is a renderer or the copy into the record: `services.rs:453, 469, 622, 665, 1011`. Zero authorization decisions anywhere in `bin/` or `crates/`. |
| `disable` stops the announce; ws link, containers and in-flight work survive | **CONFIRMED, and worse.** For agent/airlock it does not even stop the announce, because there is none (A4/A5). Containers survive *and are then unreachable forever* (A6). |
| the instance id — does anything key on it? | **REFUTED — it is load-bearing.** It is the podman ownership label (`managed_label(display_id())`, `compute/mod.rs:217`, `agent/mod.rs:154`) and the only thing scoping one daemon's reaper off another's containers. Note the label carries only the first 4 bytes of the 32-byte id. Its rotation-by-design is what causes A6. |
| `capabilities` and `needs` — read for anything but display? | **SPLIT.** `needs` is display-only, as documented — genuinely fine. `capabilities` is **not** display-only: it is half the announce intersection and therefore has a consensus-visible effect, which is what makes A2 a real attack rather than cosmetic. |
| `origin_guard` passes every `Origin`-less caller; no daemon sends any auth header | **CONFIRMED, and it is documented as deliberate** (`origin_guard.rs:23-27`). `NodeLink` sets exactly one header, `content-type` (`node_link.rs:120`). Not a hidden defect — but see A7 for the part of that file that is broken. |
| `ducktape node run` binds HTTP before publishing its identity | **CONFIRMED, and the code says so in a doc comment**: `services.rs:145-147` — *"`bin/node` still binds its HTTP listener before publishing, so a supervisor co-starting a node and a daemon reaches this every time."* A daemon that loses the race exits loudly (`run_service` has no retry there, `:1077`), which is the contract — but it is a shorter fuse than before `node_identity` moved ahead of the sandbox probe. |
| a daemon can read `identity.key` from the workspace it is handed | **CONFIRMED, structural.** `service run --workspace <dir>` hands the process the directory the key lives in; nothing in-process can gate a `read(2)` by the same uid. Honestly named as a ceiling in the scope-enforcement plan (§1e). The type-level guard only makes it *unrepresentable in `ServiceConfig`*, not unreadable. |
| `MAX_ANNOUNCED_NODES` (1024) has no host-side cover | **CONFIRMED.** Enforced only at `capability/src/lib.rs:352`; zero references anywhere in `bin/`. The 1025th node's announce is rejected — and per A1 a rejected announce wedges the announcer permanently, so that node is silently dark for the rest of its process lifetime. |
| pre-consent catalog squatting: one local process can overwrite another kind's catalog entry | **CONFIRMED and broader than stated.** `admit`'s `entries.insert` is last-write-wins (`noded/services.rs`). Because of A2 the squatter does not even need to occupy the target kind — any kind name will do, since the live set is unioned across all of them. |

---

## F. Refuted suspicions

Reporting these plainly, because a refuted suspicion is a result.

1. **The instance id is not decorative** — it scopes container ownership (above).
2. **`needs` really is display-only**, as its doc claims, and
   `an_unmet_declared_need_is_a_warning_and_nothing_more` genuinely proves it.
3. **`capability` is not composed by any sim/demo topology** — PRODUCTION only
   (`topology.rs:132`), pinned by membership assertions at `:237-260`. The
   `valset_id: None` self-announce branch has no production instance (only its
   justification is stale — C4).
4. **`bin/node/src/airlock.rs`'s own tests are honest.**
   `the_route_lives_exactly_as_long_as_the_daemon` drives the real `serve_until`
   and observes the route from inside the daemon's own `select!`, so it holds the
   arm-before-publish ordering rather than a replica of it —
   which is exactly the mutation that defeated its predecessor. Likewise
   `credentials_are_counted_without_opening_one` proves its claim *structurally*
   (a dir with no login artifact counts 3 but yields 2 seeds), which only a path
   that never opens a file can do.
5. **`bin/noded/src/term.rs` and `admin.rs` are the strongest modules in scope.**
   The admin PoP suite gives path, node and timestamp binding each a negative
   case, tests staleness in both directions *and* at the exact boundary, and
   forges with a real second key. `term.rs`'s
   `a_create_whose_caller_went_away_is_closed_not_leaked` really drops the future
   and waits for the compensating close. Every wait is event-driven; no sleeps.
   Also clean: `wire.rs:260` `skew_in_either_direction_is_a_named_decode_error`
   (real `expect_err` on both directions), `podman_api.rs:1476`
   `egress_ruleset_orders_allow_before_private_drop`, and the broker-aiming tests
   at `provider/src/lib.rs:4162/4314` (they check the *removal* of
   `OPENAI_API_KEY` — `Some(&None)`, not merely absent — and that the control
   token stays out of argv).
6. **`soul.rs` really is pure** — no fs, no duckfs, no `SkillRef`; every function
   is a function of its arguments, and the fs half genuinely lives in
   `bin/noded/src/agent_provision.rs`.
7. **The compute/agent daemons have no log bombs and no `info!` doctrine
   violations.** `compute/mod.rs:165-189` (1 + every 15th), `compute/link.rs:62-71`
   and `agent/link.rs:62-71` (1 + every 30th) all carry `attempts`. Every `info!`
   in that surface is per-{boot, session, connection, state transition}. (The one
   exception is B12, which fails the *opposite* way — total silence.)
8. **`provider/src/lib.rs:1253`'s chmod-after-write is NOT a TOCTOU.** It looked
   like the classic world-readable window (D8 lists it as a third semantics), but
   the comment at `:1243-1249` does the work: the config home is already 0700 via
   `create_private_dir`, the container is not started until later, and the file
   holds only the throwaway loopback bearer, never a real credential. Three
   independent reasons, all checkable, all true. A model for how a deliberate
   corner should be documented.
9. **The build-gate deletion's justification holds where it counts.**
   `cargo test -p agent-service` is green including
   `skew_in_either_direction_is_a_named_decode_error`, and the
   `deserialize_with = "Option::deserialize"` trick on `Create::credential`
   genuinely defeats serde's missing-`Option` fallback. Only the *"and default
   nothing"* half of the sentence is false (C3).

### Caps that were checked and are genuinely well sized

The audit checked every named cap in scope against what real producers emit.
Most are fine, and that is worth recording so nobody re-litigates them:

- `MAX_ALWAYS_BYTES = 64 KiB` — largest real producer in this repo is
  `skills/module-dev/SKILL.md` at 6,621 B (**9.9× headroom**); all five repo
  skills inlined together are 17,746 B (27% of the cap).
- `clean_error MAX = 2048` vs `saga::MAX_ERROR_BYTES = 16 KiB` — **8× headroom**.
- `assemble_runner_result` vs `saga::MAX_RESULT_BYTES = 256 KiB` — holds, and the
  three-rung degrade ladder is proven by `a_pathological_receipt_still_fits_the_saga_cap`.
- `noded::services::MAX_CAPABILITIES = 512` (the hello cap) vs 37 real tags —
  **13× headroom**, and correctly sized *against reality* after a live
  `malformed_hello` outage taught the lesson. Note this is a different constant
  from the consensus-side 64 in C5; the tight one is the one that bites.

Two that are correct but read thinner than their prose suggests:
`MAX_DESCRIPTION_CHARS = 200` (3 of this repo's own 5 skills exceed it at 417,
319 and 290 chars, so the doc's *"half a sentence still does that"* edge case is
the common case), and `TERM_READ_BUF = 32 KiB` → 43,692 B base64 per frame
against `TERM_RING_MAX_BYTES = 256 KiB`, so scrollback retains ~6 full-size
chunks ≈ 192 KiB of raw pty output rather than 256 KiB.

---

## G. One more thing worth naming

`take_service_link` (`stream.rs:1000-1020`) collapses **two different operator
problems into one sentence**: *"present this node's service-link token, and only
one agent service may attach"*. `TerminalSessions::attach` (`term.rs:684-701`)
distinguishes them internally — but returns `Option`, so the caller cannot.

And the branch that matters is the silent one. Both token failures log a stable
reason (`no_link_token`, `bad_link_token`); the **already-attached** branch
(`term.rs:693-696`) returns `None` with **no log line at all** — despite its own
doc calling it *"a boundary, not a nicety — a second attacher could otherwise
displace the live daemon and receive the create commands (including
lent-credential records) meant for it."*

So the single most operationally likely refusal (an agent daemon restarting while
its previous ws connection has not torn down) is unloggable, uncountable, and
reported to the operator as a credential problem. This is the same taxonomy defect
that `bin/node/src/airlock.rs:368-371` was just fixed for — *"Fail closed, but say
WHICH closed door"* — one file away, unfixed.

**Fix shape:** give `attach` a typed refusal (`HelloRefusal`'s shape), with
`link_already_held` as its own reason, and let `take_service_link` render it.

---

## H. What this audit did NOT cover — say it, do not imply coverage

The point of this document is that unverified claims are the defect. So:

- **`crates/services/sandbox/` egress enforcement** was not traced beyond the one
  ruleset-ordering test (which is good). Whether the nft rules are actually
  installed on every run path, and what happens when installation fails, is
  unverified. Related and known-open: Tart has no egress firewall at all.
- **TEE attestation was audited for what it OMITS (A17), not for whether what it
  does check is correct.** The DCAP/VCEK chain validation itself — signature
  verification, CRL handling, cert-chain construction — was not reviewed. There is
  no TEE silicon here to test against.
- **`crates/services/provider/src/lib.rs` (5,619 lines)** — only the parts the
  service plane touches (`discover`, `capabilities`, `managed_label`,
  `SpawnKind`, the broker-aiming env handling) were read. The `CliProvider` run
  loop itself was not.
- **Nothing was verified on a live node.** Every finding here is source-level.
  A15 and A16 are trivially reproducible on the dukenet pair with `curl` and
  should be, immediately. A2, A6 and A12 are also cheap and worth doing before
  the fixes are designed.

### Crown jewels that WERE audited and are sound

Recorded so nobody re-audits them: **no nonce reuse anywhere** (fresh 96-bit
`OsRng` per message; per-stream salted keys make the counter nonces safe), **no
swallowed decrypt failures**, **exactly one implementation each** of key
derivation, nonce derivation, seal/unseal and token minting, **no key material in
any `Debug` impl**, **no token or URI-path logging** (the crate emits zero log
statements, and `verify.rs` deliberately strips the KDS URL because it carries the
chip id), **path traversal guarded and tested**, and the historical
`seal_pk_mismatch` masking is **fixed and correctly documented**. The cryptography
is not where the problem is — the authorization is.

## Counts

| category | count |
|---|---|
| **Live defects** (A1-A19) | **19** |
| **Guards that guard nothing** (B1-B12; 5 more weak, uncounted) | **12** |
| **Stale or false claims** (C1-C24) | **24** |
| **Doctrine violations** (D1-D8) | **8** |
| **Refuted suspicions** (F1-F9, plus the crypto set in §H) | **9** |
| confirmed by executing a mutation against a live guard | 4 (A3, A7, A8, B1) |
| confirmed by running code | 2 more (A9, C5) |

Everything else is STATIC (an unambiguous single expression — a hardcoded
constant, a missing filter, a scan list, a handler signature) except A14, A19,
B4 and C7, which are PLAUSIBLE, and A17's exploitability, which cannot be tested
without TEE silicon.

**Suggested order.** A15 and A16 are not "next sprint" items — any admitted
network member can steal a lender's credential today, and the fix for A15 is
roughly ten lines. Do those two first, together, and re-run the lending QA lane.
Then A17. Then A3 (a security lint blind to brand-new code) → A2 + A4 (fix
together, inside #819, because #819's per-kind fold is where the missing filter
belongs) → A1 (the wedge that turns every other announce bug into a permanent
outage) → A12 → A18 → A7 → A8 → A6 → A13 → the rest. The C- and D-lists are cheap
and can ride any PR that touches the file.

**One structural note.** Six of the eleven live defects are in the announce path,
and four of them compound: A4 means only compute is announced, A2 means its
retraction does not work, A1 means any rejection is permanent, and C5 means
rejection is closer than the constant's own doc claims. The wave-3
committed-grant plan proposes deleting that whole pump, and its central
trade-off — *"a crashed daemon no longer retracts"* (§4.1), stated as the one
thing worth vetoing over — is measured against a retraction that **A2 shows does
not currently work on any node running two daemons.** That plan's cost/benefit
should be re-derived against this finding before the veto question is answered.
