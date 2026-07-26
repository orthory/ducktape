# Work admission: whose work will this node run?

- **Date:** 2026-07-26
- **Tree analysed:** `origin/dev` @ `c47d77917` (PR #833 merged). Read from
  `.worktree/fix-airlock-grant-gate` @ `754feb0f3`, whose tree is
  byte-identical to `c47d77917` (`git diff --stat 754feb0f3 c47d77917` → empty).
- **Status:** designed, then **built** on `feat/work-admission`. This file is the
  design of record; §5 is what actually shipped.
- **Predecessors:** `2026-07-26-assumption-audit.md` (A15/A16 → PR #833),
  `2026-07-26-wave2-integration-qa.md` (the runbook this changes, amended in the
  same PR).

---

## The premise everything below rests on — read this first

`POST /v1/submit` on a real node **discards the caller's claimed submitter id and
re-signs the op with the node's own signer.** `bin/node/src/validator/run/ingress.rs:542-558`,
in its own words:

> *`origin` is the caller's CLAIMED submitter identity — meaningful on the
> embedded daemon, but this lane signs frames, and the signed origin IS this
> node's pubkey … a caller that needs to be its OWN author signs a frame and
> takes the `SubmitFrame` arm below.*

So a saga's committed `SagaOrigin::External(bytes)` is **the submitting node's
key, proven by a verified signature** — derived, never asserted. The mesh
`PeerId` the terminal plane is handed is the same class of fact, proven by the
WireGuard transport.

**If that ever changes, the compute-lane admission silently becomes decorative**
— it would be keying on something a caller can choose, which is precisely the
defect PR #833 existed to delete. `work_admission::tests::the_submit_lane_still_resigns_with_the_node_key`
parses that file and fails the build if either half of the rule goes away.

(The embedded daemon's `SubmitRequest.origin` is documented as *"a
TRUSTED-CLIENT convention, not authentication"* at `bin/noded/src/lib.rs:450-457`.
That lane is loopback-local — see §7.2.)

---

## 0. What #833 settled, and what it left

#833 established the decisive property: **the session token a sandbox holds
names only `sub` — which credential — and nothing about who is acting**
(`crates/airlock/src/wire.rs:80-89`). Identity therefore enters at exactly one
place, the gateway hop, where the node's proxy stamps `x-duck-caller-account`
from the mesh-verified peer and refuses a caller-supplied copy. Every
caller-asserted account was deleted from the compute layer.

The consequence is stated in the lender's own gate
(`crates/airlock/src/server.rs:565-573`):

> *a lender granting an account is lending to that account's NODE, for whatever
> workload it runs.*

So the credential boundary is now exactly as strong as the answer to a question
nothing in the tree asks: **whose work will this node run?** A credential's
owner is always granted to its own node
(`credential_use_allowed`, `crates/modules/system/gateway/src/interface.rs:259-263`),
so on a default network any party who can place work on the owner's node spends
the owner's subscription — without ever touching the token.

**This document designs the missing answer.** It is a host-side policy: a node
deciding what it will do with its own CPU and its own credential. It is not the
compute layer asserting somebody's identity to a third party, which is the thing
#833 correctly deleted.

---

## 1. The reachable set

Built independently and cross-checked against two full sweeps of the tree. Every
line reference was read.

### 1.1 The four doors

Any admission is bounded by which door a caller comes through.

| door | enforced at | admits |
|---|---|---|
| **D1 — mesh peer** | `crates/networking/data-plane/src/plane.rs:474` calling `AdmissionPolicy::permits`; for the terminal plane that is `bin/node/src/term_plane.rs:129-131` — `service == TermSession && flow == term_flow() && self.peers.contains(peer)` | any WireGuard-authenticated overlay peer, i.e. **validators ∪ residents** (`MediaPeers::set_peers`, `bin/node/src/voice_plane.rs:76`) |
| **D2 — remote op submitter** | `bin/node/src/relay.rs:205-225` `verify_relay_submit` | a signed frame from a key holding committed **resident or client** standing |
| **D3 — local process** | `bin/noded/src/origin_guard.rs:86-99` — a request with **no `Origin` header passes untouched** | anything that can dial the node's HTTP port or the JSON-lines RPC port |
| **D4 — owner** | `bin/noded/src/admin.rs:685` `admin_guard` | `/v1/admin/*` only |

Correction to the task framing, and it matters: **`service.accept()` is not
where the term plane's roster check is missing — the check is there and it
passes every mesh peer.** `permits` runs at `plane.rs:474` before the hello is
acked, for every intent including `CONTROL_INTENT`. What is absent is anything
*narrower* than mesh membership. The gap is a missing policy, not a missing
gate, and that distinction decides where the fix goes.

### 1.2 The paths

Ordered by sharpness. "Sharp" = the caller chooses the executing node **and**
can name a credential.

| # | path | door | node chosen by | credential nameable | admission today |
|---|---|---|---|---|---|
| **P1** | cross-node pty (`agent pty --host-node B --cred X`) | D1 | **caller** | **yes, any registered name** | mesh membership only |
| **P2** | pinned headless run (`agent sched --host-node B --cred X`) | D2/D3 | **caller** | **yes** | **nothing** |
| P3 | rendezvous saga (unpinned `SagaMsg::Trigger`) | D2/D3 | consensus | yes | nothing on the trigger; host checks capacity + capability |
| P4 | `RunsMsg::RequestRun` | D2/D3 | recipe routing | via recipe only | **"a non-empty submitter id"** |
| P5 | chat mention → run | D2 | consensus | via recipe | chat `check_post_policy` + channel watch |
| P6 | pages-comment mention → run | D2 | consensus | via recipe | pages comment gate; **no channel watch required** |
| P7 | forge issue/PR comment → run | D2 | consensus | via recipe | as P5 |
| P8 | jobs board → run | D2 | consensus | via recipe | jobs `Submit` + agent Active |
| P9 | agent→agent delegation (`ducktape_delegate`) | D2 in-run | consensus | via recipe | per-run bearer + bound session key + budget + caps |
| **P10** | **chat-driven Shared pty** | D2 | n/a (already running) | n/a (already lent) | **`PostPolicy::Open` — any member** |
| P11 | node-local pty (`POST /v1/term/sessions {agent}`) | D3 | self | no | `origin_guard` only |
| P12 | own-node pty (`--cred`, no `--host-node`) | D3 → D1 loopback | self | yes | as P1 |
| P13 | `POST /v1/submit`, `/v1/submit/frame` | D3 | as P2/P3/P4 | yes | `origin_guard`; the frame lane verifies the signature but applies **no standing check** locally |
| P14 | JSON-lines RPC `Submit` (`127.0.0.1:8845`) | D3 | as P2/P3/P4 | yes | **nothing at all** |
| P15 | gateway loopback proxy | D1 | n/a | n/a | signed `RouteStatement` + `audience_allows`; **unconfirmed** whether any config ever maps `/v1` as an upstream |

Non-spawning, checked and cleared: airlock (`bin/node/src/airlock.rs:39` —
*"it spawns nothing"*), forge git smart-HTTP (libgit2 vendored, no
`Command::new`, no hooks), duckfs `/v1/files/*` and workspaces
(`spawn_blocking` is a thread hop, not a process), MCP read/write tools, the
code plane (content-addressed, self-verifying), voice/call/presence, the echo
oracle.

### 1.3 What each sharp path actually lets a caller do

**P2 — pinned headless run.** `bin/node/src/agent_cli.rs:349-397` composes a
`SagaMsg::Trigger` with `pinned_assignee: Some(target)` and submits it to
`/v1/submit`. The CLI is not the boundary — the module is, and any party with
submit standing can hand-craft the same op.

The trigger handler (`crates/modules/system/saga/src/lib.rs:759-854`) validates
**shape only**: duplicate id, `max_attempts != 0`, spec size, reply-payload
size, non-empty capability tag, `validate_resources`, non-empty pinned key
(`:808-816`), reply-to module exists. `ctx.env().origin` is recorded at `:835`
and **never gated on**. `lease_and_request` (`:681-698`) then takes the pin
verbatim:

```rust
let assignee = match &saga.pinned_assignee {
    Some(key) => Some(key.clone()),
    None => self.compute_assignee(...).await,
};
```

— so a pin bypasses `compute_assignee` entirely: no capability check, no
membership check, no consent. The victim's compute daemon discovers the work by
polling its own committed projection (`SagaQuery::AssignedPending`,
`bin/node/src/compute/intake.rs:414-426`) and the only host-side gate is
capacity + capability resolution (`crates/services/compute/src/lib.rs:129-172`).

The caller controls: target node, credential name, capability tag,
`cores`/`mem_gb` demands, the entire prompt payload. Not the image (node.toml
`[sandbox]`) and not the argv (the capability spec).

**Origin is authenticated, and it is a NODE key** — the premise stated at the
top of this document, and the reason the compute lane has anything to key on.

**P1 — cross-node pty.** `bin/noded/src/term.rs:1223 create_session` →
`create_remote` → `bin/node/src/term_plane.rs:830 client_create` → CONTROL
stream → the victim's `term_plane.rs:504 serve_create` →
`sessions.create_for_peer` → the agent daemon → `spawn_interactive_session` →
`Command::new("podman")` + `openpty`.

`serve_create`'s admission is: sandbox present (`:512`), the credential name
exists in committed gateway state (`:524`), the record is `Some` (`:636`),
`cpu ≤ 8` / `mem_gb ≤ 32` (`:642-652`), the provider tag does not contradict the
vendor (`:653`). No caller check of any kind. The file says why, and the reason
is correct for what it refuses to do (`term_plane.rs:517-523`):

> *The creator's ACCOUNT is deliberately not looked up. It used to be, and it
> used to be shipped to the lender as the grant subject … A creator account
> resolved here could only ever be a second, uncheckable answer to a question
> the lender has already answered.*

That reasoning kills a *claim about a third party*. It does not touch a *host's
own policy*, and the file already flags the consequence at `:691-694`.

**P10 — chat-driven Shared pty, and this one is not on anyone's list.** A shared
session's command channel is created `PostPolicy::Open`
(`bin/noded/src/term_consensus.rs:37-51`), so **any network member can post a
command the projector feeds into a live pty** — including one running on a lent
credential. Its own doc says so:

> *`PostPolicy::Open` means ANY network member can post a command the projector
> feeds into the pty — NOT only the session's participants … treat a shared
> session as open-to-the-network by construction.*

This is a real hole in the same family, it is **not** closed by anything below,
and its fix is the channel's post policy (participants roster), not a compute
admission. Recorded here so it is not lost; see §7.

### 1.4 What the reachable set says about scope

Two facts fall out, and they set the design's shape:

1. **Every credential-naming path with a caller-chosen executing node is either
   P1 (term) or P2 (saga with `SagaOrigin::External`).** P4–P8 reach the same
   compute intake, but their saga is emitted by `dispatch` as a module follow-up
   (`crates/modules/system/dispatch/src/lib.rs:552-576` uses `ctx.emit_msg`), so
   their committed origin is `SagaOrigin::Module("dispatch")`, and their payload
   is composed **in consensus** by the runs module's own deterministic
   serializer (`crates/modules/apps/runs/src/envelope.rs:291-322`, fixed field
   order), which has **no credential field at all** — the client never supplies
   payload bytes on that lane. (`agent sched`, by contrast, composes its
   envelope client-side and can put anything in it:
   `crates/services/compute/src/envelope.rs:211-232 compose_headless`.) Those
   lanes are free compute, not credential draw.
2. **D3 is the ceiling.** P11/P13/P14 mean anything that can reach the node's
   HTTP or RPC port already *is* the operator: `/v1/submit` re-signs as this
   node, so its origin is always this node's own key, and the RPC listener has
   no gate whatsoever. Any admission designed below is worth exactly as much as
   those ports being loopback-bound — and `gateway_can_start`
   (`bin/node/src/main.rs:271-285`) only **warns** when `http_listen` is not
   loopback. See §7.

---

## 2. The admission

### 2.1 Reuse before inventing — what was considered and rejected

| candidate | why not |
|---|---|
| **`RouteAudience::Accounts`** (`crates/modules/system/gateway/src/interface.rs:105`) — an on-chain, signed, sorted, bounded, validated, *enforced* account allowlist with no CLI and no production constructor | right type, wrong plane. It gates inbound gateway HTTP to a published route (`gateway_plane.rs:434`, `:1028`). Reusing the type in a local config would couple a local file to a consensus wire encoding for no gain. Its existence is worth a separate note: it is a finished mechanism nothing can set. |
| **`ServiceGrant` / `services.toml`** (`bin/node/src/services.rs:186-203`) | kind-keyed, not account-keyed, and a policy per grant is **two lists that must agree** — the dual-path defect. Also the file is deleted when the last grant goes (`services.rs:361-369`), so a node-wide policy stored there evaporates on `service disable`. |
| **Capability registry annotation** (an on-chain audience on the announce) | attractive — a submitter could pre-check and `pick_assignee` could filter the pool. Rejected: it needs a `crates/modules/` change and a genesis flag day; it builds a security gate on an announce pump the audit shows is broken four ways (A1, A2, A4, C5); and per A10 a node removed from the valset **can never update its own announce**, i.e. can never tighten its own admission. A host that cannot revoke is not a policy. |
| **`AdminExposure`** (`bin/noded/src/admin.rs:145-153`) | ternary, process-wide, namespace-scoped, env-configured. No per-account dimension and no room for one. |
| **`data_plane::AdmissionPolicy`** (`plane.rs:45-47`) | this is the mesh roster gate and it is already doing its job (§1.1). Narrowing it would refuse the peer's *other* flows (chunk, command, telemetry) as collateral. |

**Naming warning:** the tree already has **two** unrelated types called
`AdmissionPolicy` — `crates/networking/data-plane/src/plane.rs:45` (mesh roster)
and `crates/modules/system/dispatch/src/interface.rs:168` (queue-vs-fail-fast
capacity). Do not add a third. The names below are deliberately distinct.

### 2.2 Home

**Its own file in the workspace, `work-admit.toml`, re-read on every decision.**

```toml
# whose work this node will execute — the accounts this node admits, on top
# of its own owner (always admitted) and its own submissions.
# managed by `ducktape node work admit|revoke`; re-read on every decision.
# ["anyone"] admits any network member: this node then runs a stranger's
# workload AND lets it draw on every credential this node is granted.
admit = [
  "<64-hex account id>",
]
```

One key, one type (a list of strings), one decode. `"anyone"` is the same token
in the file and on the CLI — deliberately not a config spelling plus a CLI
spelling that must be kept in sync — and it may not be mixed with account ids
(a wildcard is a statement, not an entry). An **absent file is the default**,
exactly as an absent `services.toml` means "no grants"; `Owner` **removes** the
file rather than writing `admit = []`, so there is one representation per policy
and no husk to read stale.

**The design said `node.toml`; the build did not, and the reason is worth
recording.** `write_node_toml` (`bin/node/src/config/node_toml.rs:340`) rewrites
the WHOLE config on every `init`/`join` merge — *"always writing the merged
result makes init/join idempotent AND partial-flag-safe"*. A list living in
`node.toml` would therefore need a `NodeToml` field **and** a `Plumbing` field
**and** a `merged_plumbing` merge **and** an emission ordered against the
`[sandbox]` table that must stay last: five touch points, any one of which
silently erases a security policy on the next `join`. Its own file has two (a
loader and a writer) and cannot be erased by an unrelated verb.

Why not the alternatives, all of which were checked first:

- **`services.toml`** is kind-keyed, so a policy per grant is *two lists that
  must agree* — the dual-path defect. And it is deleted when the last grant goes
  (`services.rs:361-369`), taking a node-wide policy with it.
- **`RouteAudience::Accounts`** (`crates/modules/system/gateway/src/interface.rs:105`)
  is a finished on-chain account allowlist — signed, sorted, bounded, validated,
  *enforced* — with no CLI and no production constructor. Right type, wrong
  plane: it gates inbound gateway HTTP to a published route. Worth knowing it
  exists.
- **An on-chain capability annotation** would let a submitter pre-check and let
  `pick_assignee` filter the pool. Rejected: a `crates/modules/` change and a
  genesis flag day, built on an announce pump the audit shows broken four ways
  (A1, A2, A4, C5) — and per A10 a node removed from the valset can never update
  its own announce, i.e. **could never tighten its own admission**. A host that
  cannot revoke is not a policy.
- **`AdminExposure`** is ternary, process-wide and env-configured; no per-account
  dimension.
- **`data_plane::AdmissionPolicy`** is the mesh roster gate and is already doing
  its job (§1.1); narrowing it would refuse the peer's chunk/command/telemetry
  flows as collateral.

**Naming warning:** the tree already has **two** unrelated `AdmissionPolicy`
types — `crates/networking/data-plane/src/plane.rs:45` (mesh roster) and
`crates/modules/system/dispatch/src/interface.rs:168` (queue vs fail-fast
capacity). This is a third concept and deliberately shares neither name.

**Read per decision, not at boot.** The sched lane's gate runs in the compute
daemon and the term lane's in the node; a boot-time read would make one process
see a policy change immediately and the other only after a restart — the same
policy with two effective times, which is the divergence this design exists to
avoid. A ~200-byte TOML read costs nothing beside the committed queries each
decision already makes, and `ducktape node work admit` then takes effect
everywhere at once with no reload machinery. Precedent: `gateway-routes.json` is
reloaded **per request** for exactly this reason, and `services.toml` is re-read
every announce tick.

**Both call sites are in the same binary.** `ducktape service run compute` is a
subcommand of `ducktape`, so `bin/node/src/` code is compiled into both roles;
one reader and one decision serve both with no new crate and no crate boundary
to duplicate across. The daemon reads the policy file directly and never
`config::Resolved` — that type holds the node's private key, and the daemon-path
lint exists to keep it out (`bin/node/src/services.rs:1614-1620`; that lint's own
scope list is stale — audit A3, not this PR).

### 2.3 Unit

**The account.** Not the node key, though both lanes hand us a node key
first-hand and unforgeably (the saga's committed `SagaOrigin::External(node_key)`;
the mesh peer's `PeerId`).

The node key is one query cheaper. The account is right anyway:

- It is the unit `x-duck-caller-account` carries, the unit
  `credential_use_allowed` decides on, and the unit
  `ducktape user cred grant <NAME> <ACCOUNT>` already takes ("a hex account id
  or a display name", `bin/node/src/cred_cli.rs:73-79`). A second identity unit
  in the same security story is the divergence, not the saving.
- A person with two nodes is one grant, not two.

Node key → account is `identity::IdentityQuery::OfNode`, committed, already used
on both lanes' code paths (`bin/node/src/compute/cred.rs:80-91` in the daemon;
`bin/node/src/gateway_plane.rs:494` and `bin/noded/src/admin.rs:355` elsewhere).

**Configured by / changed by:** the workspace operator, via
`ducktape node work {list,admit,revoke}` writing `work-admit.toml`, or by
editing that file. The same authority that runs `service enable` and holds
`identity.key` — a node's own policy needs no signature, because anyone who can
write the workspace can already run anything on the box. The CLI takes a hex
account id, a display name, or the literal `anyone`, reusing
`cred_cli::resolve_account` rather than reimplementing the lookup, and refuses
`revoke <account>` while the policy is `anyone` (it would write a file that
changes nothing and print success — the fail-quiet this repo's refusal doctrine
exists to prevent).

### 2.4 The decision

Five types in one new file, `bin/node/src/work_admission.rs`:

- **`WorkAdmission`** — the policy. `Owner` | `Accounts(BTreeSet)` | `Anyone`.
- **`WorkSource`** — what a lane knows first-hand: a mesh `Peer(node_key)` or a
  committed `Saga(&SagaOrigin)`.
- **`WorkCaller`** — who is asking, as far as committed state can say. FIVE
  states, because "could not ask" and "no account bound" are different operator
  problems — the lesson `airlock::server::GrantAnswer` already paid for.
- **`WorkRefusal`** — `NotAdmitted` | `CallerUnbound` | `PolicyUnreadable`, each
  deriving its own stable token from the variant so a typo cannot silently
  downgrade a refusal (the `AdminRefusal` pattern).
- **`WorkVerdict`** — `Admitted` | `Refused(WorkRefusal)` | `AuthorityUnavailable`.

`verdict` is pure and total; the reads live in `admit` and the effects in each
lane's own writer. **The shipped code, with the ordering that matters and why,
is §5.1 — it is written once, there, rather than sketched here and drifting.**

### 2.5 The two call sites, and why that is one mechanism

The two lanes execute in two processes **by design** — that is what the wave-2
daemon split is. So there are two call sites. There is exactly **one policy
file, one parser, one `WorkCaller` resolver, and one `verdict` function**;
neither site may re-decide anything. Two checks that must agree would be the
dual-path defect. One function called twice is not.

The resolver needs committed reads over two different transports (the daemon
holds a `NodeLink`; `term_plane` holds a `NodeCommand` channel), so it takes a
one-method `CommittedReader` trait — `async fn read(target, request)` — with a
~5-line impl per transport. The *resolution* is therefore shared too, and the
only per-lane code is the transport shim.

**Term lane — `term_plane::serve_create`.** The gate is the FIRST thing the
function does, ahead of even the sandbox check: the caller question depends on
nothing about the credential, and deciding it last would mean reading committed
state on a stranger's behalf and disclosing this host's capabilities to someone
it will refuse anyway. `peer.0` is the caller node key, `control.commands` is
the reader. `Refused(reason)` and `AuthorityUnavailable` both become
`SessionControlReply::Refused { reason, detail }`, which already exists with a
stable-token contract. `admit_create` is untouched — it answers *what this host
can do*, a different question. Mutation **M11** moves the gate below the sandbox
check and the behavioural test reddens.

**Sched lane — `bin/node/src/compute/intake.rs::WorkPump`,** before the request
is offered to the pool. This placement matters: it is upstream of
`provisioner.provision`, so a refused run never triggers the up-to-128 duckfs
checkouts of audit A12, and upstream of any paid call.

The pump already carries the discriminant and the machinery:

- it knows the lane (`enum Lane { Assigned, Unassigned }`, `intake.rs:399-407`);
- it has a per-attempt latch with a `Stage::Due(Msg)` for follow-up ops it must
  submit.

So the writes are:

| lane | verdict | write |
|---|---|---|
| `Unassigned` (the claim/bid lane) | `Refused` | do **not** submit `SagaMsg::Accept`; latch so it is neither re-decided nor re-logged. Costs the saga nothing — no attempt is burned, another node may still claim it. |
| `Assigned` (pinned, already leased to us) | `Refused` | `Stage::Due(SagaMsg::OracleResult{ outcome: Err("work_not_admitted") })` so the saga terminates instead of parking. A pinned saga re-pins on every attempt, so it reaches `Failed` after `max_attempts` and the submitter sees the reason. |
| either | `AuthorityUnavailable` | do nothing this pass; retry on the next. Never burn an attempt on a read we could not make — the pump's existing *"an unreadable projection is not an empty one"* doctrine (`intake.rs:15-30`). |

The caller for the sched lane comes from `SagaQuery::Get { saga_id }` →
`SagaView.origin` (`crates/modules/system/saga/src/interface.rs:380-381`), which
the pump already queries for a different purpose (`attempt_projection`,
`intake.rs:428+`).

**Logging.** One `warn!(target: "ducktape::service", reason = "work_not_admitted",
node = %hex8(caller_node), …)` per (saga, attempt) — latched on the entry the
pump already keeps, because this runs in a poll loop and an unlatched warn is a
log bomb. **The account is never logged**, per the refusal rule. The caller's
**node key prefix** is, deliberately: it is public routing metadata already
logged at boot, it is not an account, and without it the operator is told
"someone was refused" with no way to find out who to admit. `ducktape node work
list` resolves a node key to its account for them.

### 2.6 Consensus: zero module changes

`git diff origin/dev --name-only crates/modules/` stays **empty**. The design
uses only queries that already exist:

- `SagaQuery::Get` → `SagaView { origin, assignee, … }`
- `identity::IdentityQuery::OfNode` → `AccountView { account_id, … }`

In particular **`WorkerRequest` is NOT extended**. Adding `origin` to it would
be the obvious move and it is the wrong one: it changes a `crates/modules/`
wire, hence the module wasm, hence the genesis root
(`bin/node/src/host_state.rs`'s `GENESIS_ROOT_HASH`), for a fact one existing
query already returns. A flag day is affordable; an unnecessary one is not.

**The residual this creates, stated plainly.** `WorkCaller::NotAnAccountOrigin`
is admitted, so P4–P8 (chat/pages/forge/jobs/`RequestRun` → dispatch → saga) are
**not** gated by this policy: their committed origin is `Module("dispatch")` and
the submitter is one hop further back, in `runs`' own state
(`stage_dispatch_run` records `requester`,
`crates/modules/apps/runs/src/admin.rs:116-123, :158-160`). Refusing them
instead would break the flagship chat-agent product, which is not a trade this
change gets to make. What bounds the residual:

- those lanes **cannot name a credential** (§1.4), so the exposure is free
  compute, not a credential draw;
- they **cannot choose the victim node** — placement is the recipe's routing or
  consensus rendezvous, not the caller's;
- each has its own gate, the weakest being P4's *"a non-empty submitter id"*.

Attributing them properly (saga → `reply_to`/`reply_payload` → dispatch → runs
`requester`) is a named follow-up, not this PR. Its right answer is probably
"the agent's owner authorizes", which is a different policy than this one.

---

## 3. The default

**`WorkAdmission::Owner` — this node runs its owner's work, and its own.**

Chosen over the two alternatives because it is the only one that is both safe
and not a stop-the-world change: `Anyone` re-opens the exact hole; a
literal-nobody default breaks even `agent sched` with no `--host-node`, which
pins to the caller's own node.

`Owner` is precisely the shape of the record it protects — `credential_use_allowed`
also admits the owner implicitly and everyone else explicitly.

**What it breaks, honestly:**

1. **Cross-account placement stops working until admitted.** Two boxes owned by
   two accounts (the dukenet/QA topology: `eddy` on the dev box, `duke` on the
   macmini) can no longer run each other's work. Fix is one verb on the
   *executing* node: `ducktape node work admit <account>`. See §6.
2. **Rendezvous placement onto a stranger's node goes silent.** An unpinned
   saga announced to the pool will simply not be claimed by nodes that do not
   admit the submitter. With no `deadline` set, that saga parks forever
   (audit A9) — a pre-existing defect this default makes *reachable in normal
   operation*. The refusal is therefore logged — once per announcement, latched
   — and the runbook's X-1a asserts it (§6). **A silent park is worse than a
   loud refusal**, which is exactly what that step exists to catch.
3. **Nothing else.** Single-node workspaces, the whole bootstrap window before
   `user account-init`, `agent sched` with no `--host-node`, own-node `--cred`
   pty (P12, which loops through the same mesh lane back to itself) and every
   Tier-1 QA step all take the `WorkCaller::ThisNode` fast path and never
   consult the policy or the identity module at all.

**No flag day.** An existing workspace has no `work-admit.toml`, and an absent
file IS the default — nothing to migrate, nothing to rewrite, no config that can
fail to parse. The behaviour change is real; the config change is not.

`admit anyone` prints a one-line consent summary naming what it exposes
(any registered credential this node is granted, at up to
`MAX_SESSION_CORES`/`MAX_SESSION_MEM_GB` per session) — the `service enable`
consent-screen precedent, not a new pattern.

---

## 4. Delegation — after, not with

### 4.1 What it is, and that it is confirmed possible

Today the lender authorizes the **executing** node's account, so a run submitted
by A and executed on B draws as B. Delegation would let it draw as A, with B
supplying only a **pointer**, never a claim:

1. `SessionRequest` gains a required work reference. The wire is
   `deny_unknown_fields` with no `serde(default)` anywhere by design
   (`crates/airlock/src/wire.rs:5-9`), so it must be a **required tagged field**,
   not an `Option` — e.g. `work: WorkRef` with arms `Direct` and
   `Saga { saga_id }`. Every producer then says which, explicitly.
2. `GrantCheck` widens from `Fn(String, Vec<u8>)`
   (`crates/airlock/src/server.rs:162-163`) to carry `(sub, vouched_account,
   work)`.
3. The lender's `grant_answer` (`bin/node/src/airlock.rs:375`) verifies from its
   **own committed state**: `SagaQuery::Get { saga_id }` gives `origin` and
   `assignee`; require `account_of(assignee) == vouched_caller` (so only the
   node actually holding the lease may present the pointer), then authorize on
   `account_of(origin)`.
4. Plumbing is already threaded: `CredentialResolver::resolve` still takes
   `saga_id` (`bin/node/src/compute/cred.rs:145`, currently `_saga_id`), and
   `resolve_credential_into` passes it (`crates/services/compute/src/pool.rs:116`).

Nothing in that is a layering violation: B asserts no identity, it hands over an
id, and the lender reads the answer out of consensus.

### 4.2 Why it ships second

- **Admission closes the hole on both lanes; delegation closes it on one.**
  Keyed on `origin`, delegation would refuse a stranger's *saga* draw — but the
  pty lane has no committed record of who asked, so its subject stays "the
  vouched caller", and a stranger's pty on my node still spends my grant.
  Shipping delegation first would leave P1 wide open while looking like a fix.
- **Delegation restores nothing that admission takes away.** A's run on B where
  B is granted still works after admission (A must also be admitted on B —
  one verb). A's run on B drawing on **A's** grant has *never* worked, so
  sequencing regresses nothing.
- **Review surface.** Together they touch the airlock wire, the broker, the
  compute pool, the term plane, node config, and the CLI, in one security PR.
  Separately they are two reviewable changes with two live-QA passes on the
  lending pair.
- Delegation also makes the admission *safer* afterwards: once the grant subject
  is the work's origin, admitting a friend for compute no longer implies letting
  them spend the host's subscription. That is an argument for doing it soon —
  not for doing it in the same diff.

**The asymmetry to decide when that PR is written, not now:** after delegation
the two lanes have different grant subjects — the saga's committed origin vs the
vouched caller. One rule ("the subject is whatever the lender can verify:
committed state where it exists, its own transport otherwise"), two evaluations.
The alternative — giving the pty lane a committed record so both lanes read the
same way — is a much larger change and should be argued on its own.

---

## 5. What shipped

One PR, `feat/work-admission`, off `origin/dev` @ `c47d77917`.

| file | what |
|---|---|
| `bin/node/src/work_admission.rs` (new) | `WorkAdmission` / `WorkSource` / `WorkCaller` / `WorkRefusal` / `WorkVerdict`, the `CommittedReader` seam, the policy file loader+writer, `attributable`, `resolve_caller`, the pure `verdict`, and the one public entry point `admit` |
| `bin/node/src/work_admission/tests.rs` (new) | 22 tests: the pure decision, the policy file, the zero-read and half-failure paths, and the two source-parsing lints |
| `bin/node/src/term_plane.rs` | `impl CommittedReader for fmpsc::Sender<NodeCommand>`; `ControlState.workspace`; the gate at the TOP of `serve_create`; one behavioural refusal test |
| `bin/node/src/compute/intake.rs` | `impl CommittedReader for NodeLink`; `WorkPump.workspace`; `gate` (reads) + `record` (writes); `admits`; `refusal_op`; `saga_origin`; `claimed: HashSet` → `claims: HashMap<_, ClaimState>` |
| `bin/node/src/cli_args.rs`, `cli.rs` | `ducktape node work {list,admit,revoke}` |
| `bin/node/src/cred_cli.rs` | `resolve_account` → `pub(crate)` (reused, not reimplemented) |
| `bin/node/src/validator/mod.rs`, `replica/park.rs`, `compute/mod.rs` | thread the workspace path to the two spawn sites and the pump |
| `bin/node/tests/sched_pinned_run.rs` | a new **DIRECTION 0** on the existing two-node delegated test (§5.5) |

**Zero module changes.** `git diff origin/dev --name-only crates/modules/` is
empty. The design uses only queries that already exist — `SagaQuery::Get` →
`SagaView.origin`, `identity::IdentityQuery::OfNode` → `AccountView.account_id`.

**`WorkerRequest` was deliberately NOT extended.** Adding `origin` to it is the
obvious move and it is the wrong one: `WorkerRequest` is a `crates/modules/`
wire, so a field there changes the saga module's wasm, hence the genesis root
(`bin/node/src/host_state.rs`'s `GENESIS_ROOT_HASH`) — a flag day for a fact
`SagaQuery::Get` already returns.

### 5.1 The decision, as built

```rust
fn verdict(policy: &WorkAdmission, owner: Option<&[u8]>, caller: &WorkCaller) -> WorkVerdict {
    match policy {
        WorkAdmission::Anyone      => WorkVerdict::Admitted,
        WorkAdmission::Owner       => admits(owner, caller, &BTreeSet::new()),
        WorkAdmission::Accounts(a) => admits(owner, caller, a),
    }
}

fn admits(owner: Option<&[u8]>, caller: &WorkCaller, extra: &BTreeSet<Vec<u8>>) -> WorkVerdict {
    match caller {
        WorkCaller::ThisNode           => WorkVerdict::Admitted,
        WorkCaller::NotAnAccountOrigin => WorkVerdict::Admitted,
        WorkCaller::Unresolved         => WorkVerdict::AuthorityUnavailable,
        WorkCaller::NodeWithoutAccount => WorkVerdict::Refused(WorkRefusal::CallerUnbound),
        WorkCaller::Account(id)        => { /* owner or admitted */ }
    }
}
```

Two matches, each on one discriminant, no `_` arm. **Policy first, caller
second** — so an `Anyone` node keeps running work while the identity module is
silent, which is the only thing that policy can mean. `Owner` is `Accounts(∅)`,
written once rather than twice.

`WorkCaller::ThisNode` is **derived, never asserted**: the compared key is either
a verified frame signer or the mesh `PeerId` (or, on the terminal plane's
own-node loopback, `control.me` itself). And it costs **zero committed reads** —
the owner lookup is skipped for every caller that is not an `Account`, because a
read there would make a single-node workspace and the whole pre-`account-init`
window depend on an identity module that has nothing to say yet.
`our_own_work_makes_no_committed_read` uses a reader that **panics** on any read,
so a reintroduced lookup fails loudly rather than quietly hanging a create. (That
is not hypothetical: the first cut of `admit` read the owner unconditionally, and
it hung an existing terminal-plane test against a query nobody answered.)

**Both reads are three-state, and the second one is easy to miss.** An account
caller is decided by comparing TWO committed reads — the caller's account and
this node's owner. `Unresolved` covers a failed caller read; a failed *owner*
read would, if folded into the comparison, produce `work_not_admitted` for a
caller who might already BE the owner — telling the operator to admit an account
that needs no admitting. So it returns `AuthorityUnavailable` too, and
`a_failed_owner_read_is_unavailable_not_refused` drives exactly that asymmetric
half-failure. `Ok(None)` — a node with no owner yet — stays a real state and
matches no account.

### 5.2 The writes

| lane | verdict | write |
|---|---|---|
| term (`serve_create`) | Refused | `SessionControlReply::Refused { reason, detail }` — instantly, before the credential record is read and before any host capability is disclosed |
| term | AuthorityUnavailable | `Refused { reason: "work_authority_unavailable" }` — a retryable door, not a missing admission |
| compute lease (`record`) | Refused | an entry carrying `Stage::Due(OracleResult(Err("work_not_admitted")))`, submitted by the pump's ordinary due machinery, so a pinned saga reaches `Failed` with a named reason instead of parking |
| compute lease | AuthorityUnavailable | dropped from the pass, **not tracked** — no attempt is burned on a read that did not answer |
| compute claim (`tick_claims`) | Refused | no `Accept` is submitted and the announcement is latched `ClaimState::NotAdmitted` — costs the saga nothing, another node may still claim it |
| compute claim | AuthorityUnavailable | `continue` **without latching** — an unanswered read must not settle an announcement this node might well run |

Why the lease-lane refusal must be an op and not a skip: a pinned saga with no
`deadline` can never be cranked out of `Pending` (audit A9), so a silent skip
would leak one consensus record per refusal. **A silent park is worse than a
loud refusal**, and this default makes that failure mode reachable in normal
operation (§3.2).

### 5.3 Logging

Refusals are `warn` with a stable snake_case `reason`, **latched to fire once
per attempt / per create** — the compute lane runs in a poll loop, and an
unlatched warn there is the log bomb doctrine forbids. Undecided is `debug`
(per-op).

**No account is ever logged.** The term lane logs the peer's **node key prefix**
(`hex8`) instead, deliberately: a node key is public routing metadata already
logged at boot, it is not an account, and without it the operator is told
"someone was refused" with no way to find out whom to admit.

### 5.5 Proven on a real two-node cluster

`sched_pinned_run::a_delegated_run_draws_as_the_executing_node_not_the_submitter`
is #833's own delegated test: two real validators, two accounts, a real compute
daemon on node 1, a real `service run airlock` lender on node 0, containers, and
a mock upstream. It went **red** on this branch — and correctly: node 1 now
refuses node 0's work before the credential lane is reached, so the lender's
refusal token never arrived.

Rather than weaken it, it grew a third direction, and the result is the best
evidence in this PR — two independent gates, in order, on live nodes:

| direction | state | outcome |
|---|---|---|
| **0 (new)** | neither admitted nor granted | `Failed` — **`work_not_admitted`**. No container, no gateway hop, no session. |
| **1** | `work-admit.toml` written on node 1 — **no restart** | `Failed` — `credential_not_granted`, the LENDER's own token, now reachable |
| **2** | plus the owner's grant | `Done`, the mock upstream's `PONG` committed into the saga result |

Direction 0 → 1 changes one file on the executor and nothing else, which is what
makes "two consents in opposite directions" a demonstrated fact rather than a
claim — and the absence of a restart is what proves the per-decision read.

The sibling test, `a_granted_scheduled_run_executes_against_the_mock_upstream`
(a run pinned to the submitter's OWN node), passes untouched: it takes the
`ThisNode` path and never consults a policy.

### 5.4 The lint that guards the shape

`work_admission::tests::both_lanes_route_through_one_verdict` parses
`term_plane.rs` and `compute/intake.rs` (comments stripped, **test code
included** — the audit's A3 lesson about truncating a lint at `#[cfg(test)]`)
and asserts:

- each lane contains **exactly one** `work_admission::admit(` call — a second
  call site fails;
- neither lane names `WorkAdmission::`, `work_admission::load(` or
  `work_admission::verdict` — the three tokens that mean *"I touched the policy
  myself"*. A lane may freely name a `WorkVerdict`/`WorkRefusal`; it must, to
  consume one;
- `work_admission.rs` defines exactly one `fn verdict(`.

It earned its keep during the build: it went red on a **test fixture** in
`term_plane.rs` that constructed a `WorkAdmission::Accounts` directly. Rather
than carve an exception into it, the fixture moved to
`work_admission::admit_account_fixture`, where the policy lives.

## 6. Cost to the wave-2 QA runbook — amended in this PR

`2026-07-26-wave2-integration-qa.md` is edited here, not just described. Under
the new default the two boxes have **different owner accounts** (T1-6 runs
`user account-init --name eddy`, T2-1 `--name duke`), so the default bites on
every cross-box step.

- **New step T2-4b — admit the submitter on the executor.** Right after T2-4's
  `user cred grant`, on the macmini: `ducktape node work admit <dev-box-account>`.
  It opens by stating the thing that will otherwise be the first support
  question: **a credential grant and a work admission are two consents in
  OPPOSITE directions** — the lender grants the executor, the executor admits
  the submitter — and a cross-node run needs both, on different boxes.
- **T2-5 (lent-credential run, cross-box)** gains a deliberate-failure half run
  **before** T2-4b: the saga must reach `Failed` carrying `work_not_admitted`,
  the warn must appear once per attempt (not per 15 s tick), and no container
  may be created. Then admit and re-run **with no restart** — that single
  variable is the step.
- **T2-6 (pty on a lent credential)** gains the cheaper negative, and it goes
  first: the refusal is **immediate**, with no attempt burned and no timeout,
  because the admission is the first thing `serve_create` asks. Its assertion
  also pins that the log carries the peer's node-key prefix and **never** an
  account.
- **§8 X-1 — the step that could not fail.** It submits an *unpinned* run and
  waits for the macmini to claim it. Under the default the macmini will not bid,
  and an unpinned saga with no `deadline` can never be cranked out of `Pending`
  (audit A9) — so the symptom is a **silent park**: no error, no timeout,
  forever. Split into **X-1a** (the negative: exactly one `work_not_admitted`
  warn, saga `Pending`, `assignee: null`, no container — and *"a silent refusal
  is the one outcome this amendment exists to forbid"*) and **X-1b** (admit, then
  a fresh run succeeds with no restart).
- **§0.4 ("what this pass does NOT prove")** gains two entries: work admission
  gates only the caller-chosen lanes (module-origin runs are admitted and cannot
  name a credential), and its guarantee is bounded by `/v1`'s exposure.
- Step counts updated: Tier 2 6 → 7, total 46 → 47.

## 7. What this does not close — say it, do not imply coverage

1. **P10, the chat-driven Shared pty — dispatched as its own PR.**
   `PostPolicy::Open` lets any member post
   commands into a live session, including one running on a lent credential
   (`bin/noded/src/term_consensus.rs:37-51`, which says so itself). Its fix is
   the channel's post policy — participants roster, `MembersOnly` — a different
   mechanism entirely, so `term_consensus.rs` is deliberately **untouched**
   here. Arguably sharper than the hole this document closes,
   because the session is already authenticated and already paying.
2. **D3 is the ceiling.** `POST /v1/submit` re-signs as this node (so its origin
   always takes the `ThisNode` fast path), `/v1/submit/frame` applies no local
   standing check, and the JSON-lines RPC listener has no gate at all. This
   admission is worth exactly as much as those ports being loopback-bound —
   and `gateway_can_start` (`bin/node/src/main.rs:271-285`) only **warns** on a
   non-loopback `http_listen`.

   **DECIDED: the warn stays.** Refusing would break the tailnet-bound
   deployment that exists today, and it would not close the class: `/v1` is not
   merely *exposed* when it is non-loopback, it is **wide open** —
   `origin_guard` passes every caller that sends no `Origin` header
   (`bin/noded/src/origin_guard.rs:23-27`, deliberate). Half-doing it here costs
   the tailnet and buys nothing. The class is scoped as its own campaign,
   `2026-07-26-wave3-scope-enforcement.md`, which turns `/v1` from
   trusted-local into authenticated across every CLI verb, duckfs and forge
   push. **The interaction is the thing to carry forward: work admission's
   guarantee is exactly as strong as `/v1`'s exposure, and the module header
   says so.**
3. **P15**, whether a published gateway route can ever map the node's own `/v1`
   port as an upstream, is **unconfirmed**. No shipped config appears to do it.
   If one can, it is a full bypass of item 2 for any peer inside the route
   audience.
4. **The module-origin lanes** (§2.6) — free compute for any member, bounded by
   each module's own gate and unable to name a credential.
5. **`grant.scopes` still enforce nothing** (audit E, and the runbook's §0.4).
   Nothing here changes that; do not read a scope as a boundary.

---

## 8. Decisions taken

1. **`Owner` is the default.** Confirmed. §3 lists what it breaks; §6 pays for
   it in the runbook.
2. **`ducktape node work {list,admit,revoke}` ships** alongside the file.
   `node.toml` is not the home (§2.2); `work-admit.toml` is, and hand-editing it
   works — the CLI is the consent point and the validating writer.
3. **Delegation ships next, not with** (§4).
4. **P10 goes to its own PR**, dispatched separately (§7.1).
5. **Non-loopback `http_listen` keeps its warn** (§7.2), and the interaction is
   recorded rather than half-fixed.
