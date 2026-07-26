# Proposal: decouple node operation — `ducktape service` family, standalone daemons

- **Date:** 2026-07-25
- **Status:** proposal (wave 2 — depends on the services-extraction wave being
  merged; see 2026-07-25-services-extraction.md)
- **Asks covered:** (1) remove `--compute` from `node init`; (2) decide where
  `ducktape service …` lives; (3) run services as standalone daemons while
  staying API compatible.

## Design calls (made here, argued below)

1. **`--compute` dies in a flag day.** No alias, no deprecation shim (house
   rule: dual-path is a defect). Its two jobs split: the *toggle* moves to a
   local per-workspace service config managed by `ducktape service
   enable|disable`; the *execution* moves out of the node process entirely.
2. **`ducktape service` is a subfamily of the one `ducktape` binary**, not a
   new binary. The CLI unification (one bin, families) is settled doctrine;
   the daemon is `ducktape service run <name>` — the same busybox-style
   re-exec the extraction proposal already reserved. systemd-friendly:
   one unit per `ducktape service run compute`.
3. **Interim stance is autonomous-only.** There is no roles module yet, so
   activation source = local config (the autonomous stance from the design
   artifact). The enrolled stance (chain member set as activation source)
   arrives with the roles module later and changes nothing about the daemon
   shape — only where "desired" comes from.
4. **API compatible = the node's existing surface is the service API.** The
   daemon talks to its node over localhost HTTP `/v1` (+ ws), the same surface
   the CLI and app already use. Additive endpoints only where a real gap
   shows; zero changes to consensus wire shapes, op formats, or `/v1`
   semantics. To the rest of the network a daemon-hosted pool is
   indistinguishable from today's in-process pool.
5. **Airlock binaries unify into `ducktape` modes — with one enclave
   exception.** The self-host lender gateway becomes the future
   `ducktape service run airlock` plug (first-party services are ducktape
   subcommands; no new multi-mode binary — the one binary already exists).
   Non-enclave airlock binaries (cli/broker) get audited for folding into
   ducktape families at that step. The TEE enclave gateway stays a separate
   MINIMAL binary: attestation measurement covers the whole binary, so
   folding ducktape into the enclave would churn the lender-pinned
   measurement on every unrelated release and ship borrower/node code into
   the lender's trust boundary. Small enclave binary = cheap trust. The
   borrower-side run-broker is not a binary at all — per-run ephemeral
   loopback, stays the broker-host library inside the host process.
6. **Compute first; agent stays in-node.** The agent/pty carve is blocked
   behind the `Provider::spawn_interactive` trait-boundary change (extraction
   step 4 finding). Do not daemonize the terminal plane in this wave.
   Precedent that standalone works: the airlock gateway binaries already run
   out-of-process.

## Seam → API mapping (what "API compatible" means concretely)

The in-process closures that feed compute-service today, and what replaces
each in daemon mode:

| in-process seam today | daemon-mode replacement |
|---|---|
| `SpawnFn` (run futures on node's task lane) | daemon's own tokio runtime — trivial |
| `DeliverFn` (deliver OracleResult op into node's submit lane) | existing `/v1` op submit route (same path the CLI uses) |
| `CredentialResolver` (NodeCommand::Query into gateway/identity/saga) | existing `/v1` committed-query routes (`--json` read verbs already exercise them); add a route only if a specific query has no HTTP twin |
| `WorkspaceProvisioner` (op submit + blobstore + duckfs checkout) | existing blob routes (`POST/GET /v1/files/blob`) + duckfs client over node HTTP |
| capability announce at boot (node announces discover()'d tags) | offered tags travel in the daemon's **hello signal** to the node's local catalog; the announce **tx is submitted by the user** at `service enable` (normal CLI signing flow). Neither node nor daemon auto-announces — a daemon can never place itself on chain |
| work intake (pool fed in-process from consensus events) | watch = ws changed hints + committed-query re-evaluation. This is protocol verb `watch` being born in its cheapest form; a dedicated cursor feed is a later, additive upgrade |
| `__egress-hook` subcommand dispatch in node main | moves to the daemon binary (it owns podman now) |
| `OutputSink` → node stream hub (live run tail) | ws publish via existing stream surface; if no ingress route exists, add ONE additive endpoint (`POST /v1/runs/{id}/output` shape, decided at impl time) |

Gap policy: every row must land on an existing route; a new route needs a
one-line justification in the PR body and must be additive.

## CLI surface — signaling-first (plug-and-play order)

The flow is run → signal → list → enable (user tx follows) → operate:

```
ducktape service run compute         # 1. operate the daemon (foreground; systemd unit target)
                                     # 2. it signals the node: hello{name, offered tags, needed scopes}
ducktape service list                # 3. user sees signaling services and their state
ducktape service enable compute      # 4. user grants + the onchain tx follows (user-signed)
ducktape service disable compute
ducktape service status              # per-service: signaling / enabled / enabled-but-absent
```

- **`run` is a first-party convenience launcher only** — it re-execs services
  compiled into the `ducktape` binary and grants them nothing special.
  Third-party services launch themselves (own binary, systemd, container),
  signal the same hello, and appear in `list`/`enable` with identical
  standing. The launcher differs; the protocol path is uniform. No
  `run --exec` third-party launcher — process management stays systemd's job.
- **Hello is unprivileged and ephemeral.** Any local daemon may signal
  presence over the node's local API (same localhost surface + workspace
  auth the CLI uses); presence = live connection, disconnect = gone from the
  signaling list. The node keeps only a local catalog — nothing durable,
  nothing onchain.
- **Enable is the consent boundary.** It is only valid against a currently
  signaling service: the user reviews the hello (name, offers, requested
  scopes), the grant is minted from that reviewed hello, enablement persists
  in workspace config, and the follow-up transaction — announce record now,
  `RoleJoin` once the roles module exists — is constructed and signed through
  the normal user CLI flow. The tx follows the user, never the daemon.
- `enabled-but-absent` (enabled in config, daemon not signaling) is surfaced
  by `status` as an operational warning, not an error.
- Config lives per workspace (`~/.ducktape/workspaces/<chain-id>/`), next to
  node.toml — services are per-network, matching the multi-workspace story.
- `node init --compute` is REMOVED. The #767 one-command cred flow becomes:
  `node init` → `service run compute` (signals) → `service enable compute` →
  `user cred add` (unchanged). Docs and help text updated in the same PR.
- `ducktape service run` hosts the compute-service crate (post-extraction
  `crates/services/compute`) + provider discovery + sandbox/broker libs. The
  node binary stops constructing `PodmanService`/`discover()`/oracle pool
  entirely — that wiring (validator/run.rs, replica/park.rs, noded
  oracle_pool) is deleted, not gated.

## Interactive UX — enable-at-run

`service run <kind>` on a TTY offers enablement inline, so plug-and-play is
one command instead of two terminals:

```
$ ducktape service run compute
  compute · signaling to dukenet#03f6df3d · not enabled

  ? Enable compute on this node now? [Y/n]

  ✓ enabled  compute#deadbeef   (grant minted, announce tx signed by you)
  ● serving  2 runs · 0 leases
```

Rules — the prompt is a convenience wrapper over `service enable`, never a
second code path:

- **TTY + not enabled** → prompt once at startup. Declining keeps the daemon
  running and signaling, with a one-line hint naming `ducktape service enable
  compute`. Never re-prompt in a loop.
- **Non-TTY (systemd, CI, containers) → never prompt.** Emit one line
  ("signaling, not enabled — run …") and keep serving the signal. A unit file
  has no stdin; prompting there would hang boot. Explicit flags cover
  automation: `--enable` (assume yes, non-interactive) and `--no-enable`
  (never offer).
- **Already enabled** → no prompt at all; straight to serving.
- **Locked user key** → the enable path uses the existing TTY secret-prompt
  helper to unlock for signing; refusal or timeout leaves the daemon
  signaling-but-not-enabled rather than failing the process.
- The prompt IS the consent boundary (decision 7): same grant mint, same
  user-signed tx, same code path as `service enable`.

Tooling bar for the whole family: colored/aligned status output with state
glyphs, live status line while `run` is in the foreground, `--json` on every
read verb for scripts, and full respect for non-TTY (no ANSI, no spinners,
plain log lines) and `NO_COLOR`.

**No new dependency is needed — the tree already carries everything** (checked
2026-07-25): `anstyle`/`anstream`/`colorchoice` arrive transitively via clap
4.6, and `anstream::AutoStream` implements the TTY + `NO_COLOR` + Windows
rules by construction (promote them to direct workspace deps — already in the
graph, zero new supply chain). `bin/node/src/tty.rs` is the existing TTY
helper to extend for the Y/n confirm. `std::io::IsTerminal` is stdlib, so no
`atty`/`is-terminal`. `textwrap`/`unicode-width` are in the lock for CJK-safe
column alignment. Explicitly rejected: dialoguer (a yes/no confirm does not
justify it), comfy-table (format-width specifiers cover a three-column list),
indicatif (one `\r` rewrite on TTY), crossterm/ratatui/inquire. Revisit only
if a real multi-select or fuzzy picker becomes necessary — then `inquire`
alone gets evaluated.

## Service identity — kind tags and instance ids

Services have no sovereign identity (no keypair — identity and transport
belong to the node), so a service id must be *granted*, not self-declared.
It is therefore minted at the consent boundary:

- **kind**: a plain string tag (`compute`, `storage`, `airlock`) — same shape
  as capability tags. Version is hello metadata, never part of identity.
- **instance id** = `sha256(domain-separator kind-byte ‖ node_id ‖ kind ‖
  grant_nonce)`, minted when `service enable` creates the grant. The 32-byte
  digest is canonical; the display form is house-style `compute#deadbeef`
  (like chain ids). The kind-byte reuses the established preimage
  domain-separation discipline.
- **lifetime = grant lifetime.** Daemon restarts keep the id (the grant
  persists in workspace config); `disable` revokes the grant and kills the
  id; re-enable mints a fresh one. The id doubles as a consent-epoch marker
  for audit ("this id existed under this consent window").
- **hello has no id.** The signaling list keys on (connection, declared
  kind) only — a daemon cannot choose its own identity.
- **node-scoped.** node_id in the preimage makes global collisions
  negligible; references resolve through the announce record (which carries
  the node), giving `duck://service/compute#deadbeef` for free. Two enables
  of the same kind on one node yield two grants and two ids — permitted by
  construction, not encouraged.

## Flag day semantics

- A node whose operator never runs the compute daemon simply has no compute
  capacity — exactly like a node without `--compute` today. Nothing else
  about the node changes.
- No node-side fallback pool, no "embedded mode" flag. One path.
- Litmus tests: (a) existing agent-run + pty QA recipes pass with the daemon
  running beside the node; (b) a network peer cannot tell which mode produced
  a run result; (c) root hash untouched.

## Steps (PR-sized, after the extraction wave merges)

1. **CLI + signaling.** Add the `ducktape service` family (run skeleton /
   list / enable / disable / status), the node-side hello endpoint + local
   catalog, remove `--compute` from `node init`, migrate the #767 flow docs.
   `run` may still temporarily host the pool via the same in-process
   constructors — the point of this PR is the surface, the hello/enable
   consent boundary, and the flag day on the toggle.
2. **Daemonize compute.** `service run compute` becomes a real standalone
   process speaking only `/v1`+ws to its node; node-side pool wiring deleted.
   Seam-by-seam per the table above. This is the big one. Ships with unit
   coverage only — no integration smoke (see Integration QA).
   One debt from step 1 closes here: `bin/noded`'s oracle_pool (left
   untouched in step 1 because it is env-gated and `--compute` never reached
   it) is reconciled with the daemon model.

   Note — two items originally scheduled for this step were pulled INTO step 1
   under the no-legacy rule rather than being staged: `enable` requiring a
   live hello (achievable once step 1 ships a minimal `service run`), and
   service state living in its own file so `service` commands never write
   node.toml. Staging them would have meant guarding a path already slated
   for deletion.
3. **Gap endpoints (only if step 2 surfaces them).** Additive `/v1` routes
   with per-route justification.
4. **Agent carve + daemon.** The `Provider::spawn_interactive` trait-boundary
   change (10 impls, 2 terminal call sites) followed by daemonizing agent.
   Structural, so it is scoped and confirmed before it starts — but it is on
   the critical path, not optional: integration QA cannot run without it.
5. **Airlock plug.** Lender gateway logic becomes `crates/services/airlock`
   (`ducktape service run airlock`, autonomous stance); the enclave binary
   stays separate and minimal; the airlock contract lib relocates out of
   `crates/modules/system/` in this same step.
6. **Integration QA pass.** The single terminal verification described below.

## Service dependencies — there is no dependency graph

The tempting framing ("agent needs compute", "compute needs sandbox") does not
survive contact with the architecture:

- **sandbox is a library, not a service** (`sandbox-host` crate) — compute and
  agent link it. Linking is not a runtime dependency.
- **agent does not depend on compute.** Their bus is the chain: an agent's run
  request leaves as a runs op, dispatch places it, and the placement may land
  on a *different node's* compute. What agent needs is *compute capacity
  somewhere in the network*, which is a placement question, not a startup
  ordering question. With no compute anywhere, runs queue unplaced — agent
  still starts, still serves.

Therefore:

- **Declared capability needs, surfaced as warnings.** hello may declare "I am
  only useful if capability X exists in the network"; `service list`/`status`
  render that as a warning line (`agent · enabled · no compute capacity in
  network`). Informational only.
- **No dependency graph, no topological start order, no plug-to-plug private
  calls.** Each plug runs its own reconcile loop and converges independently,
  so ordering is irrelevant — a late starter catches up on its own. This is a
  standing non-goal, not a deferral.

## agent and compute are siblings, not layers

Agent execution has two paths with different owners — **headless runs**
(dispatch-placed, owned by compute) and **interactive pty sessions**
(user-attached, owned by agent) — but both drive the same muscle:
`provider-host` (CliProvider) + `sandbox-host` + `broker-host`.

So agent does not run "on top of" compute. Both link the same libraries and
spawn their own sandboxes. Delegating agent's execution to compute would
require a plug-to-plug private call (banned) or a chain round trip (absurd
latency for an interactive pty). Cross-node interactive sessions already work
through the mesh term plane; that is a node-to-node path, not plug-to-plug.

**The real integration hazard: two daemons driving podman on one box.**
Today's orphan reaping keys on a single `PODMAN_MANAGED_LABEL`, so agent's
reaper would happily kill compute's containers. Resolution:

- **Each daemon runs its own podman service**, not a shared one: `kill_on_drop`
  on a shared service child would re-couple the failure domains the carve
  exists to separate. This turned out to isolate more strongly than labels
  do — per-service graph roots, runroots and sockets mean neither daemon can
  even *enumerate* the other's containers, and `pasta` (per-container
  userspace networking) removes the CNI/IPAM collision surface entirely, so
  the disjoint labels are defense in depth rather than the mechanism.
  Operator-visible cost: **one image store per service** (~230-250 MiB with
  `node:22-slim`, and it scales linearly — a 2-3 GiB custom agent image
  duplicates in full). `additionalimagestore` is rejected twice over: it would
  expose the operator's image store to sandboxed runs, and a shared store must
  be read-only for the consumer, which concentrates pulls in a primary and
  reintroduces exactly the liveness coupling being removed.
- **Scope container ownership by service instance id** —
  `io.ducktape.managed=compute#deadbeef`. Each service reaps only its own
  label. `disable compute` then reaps exactly compute's containers and leaves
  agent's sessions untouched.
- This is the second reason instance ids must survive restart: a service
  re-adopts its own containers after an upgrade only if it returns with the
  same id.
- Separate storage roots per service; capacity declared per service in hello.
- The label-scheme change is a one-time flag day at daemonization, paired with
  a cleanup sweep for containers still wearing the old flat label. That sweep
  is disposable runtime-state cleanup, not a lasting compat arm.
- Fallback if per-id scoping proves messy in practice: merge agent into
  compute as one plug. Not the plan — the pty-wedge failure-domain argument
  and the differing grant scopes still hold — but it is the honest escape
  hatch.

## Credential path: broker is mandatory, airlock is optional

- **broker-host is always in the path.** Every run gets a per-run loopback
  endpoint that the sandboxed child dials with an opaque bearer; the real
  credential never enters the sandbox. This is the isolation boundary and it
  does not depend on where the credential came from.
- **airlock is one credential SOURCE, not a requirement.** An operator's own
  credential (local OAuth file or API key) resolves locally and never touches
  an airlock at runtime. Airlock enters only when the credential is *lent* by
  another member — and the airlock plug is then needed on the **lender's**
  node, not the borrower's.
- **Compile-time dep ≠ runtime dep.** broker-host links `airlock`
  (client+verify) because that is how the lending path is implemented; do NOT
  cargo-feature-gate it — two build configurations would be exactly the
  dual-path defect the repo forbids. Only the runtime path is conditional.
- Consequence for QA: the base integration tier needs no airlock at all.

## Upgradeability

- **Skew is refused, never tolerated.** Node and daemon are separate processes
  with independent restart timing (even one binary on disk: a running process
  keeps its old inode). hello carries a build identity; mismatch is refused
  with a nameable reason so the operator restarts the daemon. No version
  negotiation machinery — that matches the repo's no-versioning doctrine, and
  a loud refusal beats silent misbehavior. Third-party plugs live by the same
  rule: speak the current protocol or be refused. (Acceptable pre-live-network;
  once a network is live the protocol freezes, which is the entire point of a
  thin waist.)
- **Upgrading does not re-open the consent boundary.** The instance id is the
  grant hash and the grant persists in workspace config, so a version bump
  returns as the same `compute#deadbeef` with obligations intact. This is why
  version is hello metadata and never part of the id preimage.
- **What survives a daemon restart:** live containers (the existing
  `PODMAN_MANAGED_LABEL` adoption/reaping path), and leases if the daemon
  returns within the fencing grace window. Consequence — an operational rule
  worth stating plainly: **an upgrade window must be shorter than the lease
  TTL**, or accept workload re-placement. In-flight ephemeral runs lose result
  delivery and are retried by dispatch.
- **The structural win:** service upgrades never touch consensus — root hash
  unchanged, no flag day, no coordination, roll one node at a time. This is
  fully decoupled from validator wasm swaps (the lifecycle module's R=n
  admission gate). Sole exception: the airlock enclave binary, whose
  attestation measurement changes on every rebuild and forces lenders to
  re-pin — another reason to keep that binary small and rarely changed.

## Is a gateway primitive needed? No

Three different things wear the name; none produces a new primitive:

- **The `gateway` consensus module** (credential records) stays onchain. Not a
  service.
- **The lender gateway binaries** become the future `crates/services/airlock`
  autonomous plug — already planned, not a new primitive.
- **Correction (2026-07-26, verified in code):** the design's earlier claim
  that serving plugs "bind the overlay address directly, like a tailnet app"
  is WRONG for this codebase. Overlay identity *is* the node's mesh keypair,
  and a service has no keypair, so there is no address for it to bind.
  Overlay ingress for a plug actually lands on the node's `Service::Gateway`
  plane, which authenticates the WireGuard peer, maps it to a caller account,
  enforces the signed `RouteStatement` policy (audience / methods / body
  caps), and only then dials `RouteTarget::LoopbackHttp`. So the accurate
  statement is: **the plug binds a loopback listener and publishes via a
  user-signed route; the node reverse-proxies overlay ingress to it as
  transport.** This is better than direct binding — it adds a policy layer a
  bound daemon would not have. The invariant that survives unchanged: no
  `/v1` request carries the traffic and nothing is ever pushed to a plug over
  its node link, so no-reverse-calls holds.
- **A generic ingress/proxy primitive is explicitly rejected.** Decision 6 puts
  inbound traffic outside the protocol: serving plugs bind the overlay
  directly. A gateway primitive would re-centralize what was deliberately
  decentralized and would need reverse calls into plugs, violating the
  no-reverse-calls rule. The one real gap it would fill — public-internet
  exposure and TLS termination for non-member users — is itself just another
  autonomous plug if it is ever wanted.

## Integration QA — one terminal pass, after full implementation

**No phased integration gating.** Integration QA runs ONCE, against the
complete serviced system. A half-migrated tree (compute daemonized while agent
still runs in-node) is a configuration nobody will ever operate, so verifying
it spends effort on a state that ships to no one. Per-PR gates stay as they
are — clippy, unit tests, targeted builds, root-hash invariance — because
merging unbuildable code is not the alternative being proposed here; what is
deferred is the *workflow* verification.

**Precondition — "implementation complete" means all of:**

1. `ducktape service` family + hello signaling + `--compute` flag day.
2. compute daemonized; node-side pool/provider/podman construction deleted.
3. Any gap `/v1` endpoints that step 2 surfaced.
4. agent carved and daemonized — this pulls the deferred
   `Provider::spawn_interactive` trait-boundary change onto the critical path
   (10 impls, 2 terminal call sites; ask-first structural change).
5. airlock lender gateway shipped as the `crates/services/airlock` autonomous
   plug (`ducktape service run airlock`).

Only then does the matrix below run. broker and sandbox never appear as
enable-able services — they are libraries linked by the plugs above, and the
QA asserts exactly that (no `service enable broker` exists).

**The matrix, executed as one pass on the dev-box ↔ macmini tailnet lane:**

- **Lifecycle, per service kind (compute, agent, airlock).** Signal → appears
  in `list`; enable → id minted, config persisted, tx submitted and committed;
  daemon restart → same `kind#hex8` returns with obligations intact; disable →
  id retired, config clean. Non-TTY never prompts and emits no ANSI; TTY
  enable-at-run prompt takes the same path as `service enable`.
- **Tier 1 — no airlock (the base path).** compute + agent enabled, credential
  owned by the operator: headless agent run round trip AND an interactive pty
  session, both completing with zero airlock involvement at runtime. This tier
  is the proof that airlock is a credential source, not a dependency.
- **Tier 2 — everything on at once.** Add airlock; full agent run round trip
  using a real *lent* credential through the airlock plug — the end-to-end
  product path.
- **podman co-tenancy.** With compute and agent both running: each service
  reaps only its own id-labelled containers; killing one service's containers
  never touches the other's; `disable compute` while an agent pty session is
  live leaves that session running.
- **On/off isolation matrix (the real payoff).** With all three enabled,
  toggle each independently and prove the others are unaffected; repeat with a
  deliberately crashing plug. Separate processes must mean separate failure
  domains — this is the claim the whole architecture rests on.
- **Restart/upgrade behavior.** Kill a daemon mid-run: container adoption via
  `PODMAN_MANAGED_LABEL` plus dispatch retry. Restart inside vs beyond the
  lease fencing window: expect resume vs re-placement. Build-identity skew:
  hello refused with a nameable reason.
- **Cross-node placement (proves no local dependency).** agent on node A,
  compute only on node B — the run completes via B. Then disable compute
  everywhere: runs queue unplaced while agent stays healthy.
- **Cross-cutting invariants.** Root hash unchanged throughout; a network peer
  cannot distinguish daemon-produced from in-process-produced results; `/v1`
  changes additive only.

Accepted cost of testing only at the end: defects surface with several
unverified steps stacked beneath them. Mitigation is strong per-PR unit
coverage (each step ships its own tests) plus a deliberately generous scope
for this single pass — not earlier integration runs.

## Risks

- **Latency on the run path** — in-process closures become localhost HTTP.
  Run submission and result delivery are not hot loops (per-run, not
  per-frame); acceptable. Live output tail is the one seam to measure; ws
  keeps it streaming.
- **Watch fidelity** — polling + changed-hints must not miss assignments.
  Mitigation: re-evaluate on every changed hint AND on a slow timer; the
  chain is the source of truth, so missed hints delay but never lose work.
- **Capacity announce race** — daemon boots before node has synced → announce
  op rejected. Daemon retries with backoff (attempt-counted logging per house
  rules, first + every Nth).
- **Orphaned sandboxes across daemon restarts** — `PODMAN_MANAGED_LABEL`
  reaping already handles adoption; the daemon inherits that logic with the
  sandbox lib.

## Done criteria

- `node init --compute` gone; `ducktape service` family shipped; help/docs
  coherent.
- Compute runs only as a standalone daemon; the node constructs no pool, no
  provider discovery, no provisioner, no credential resolver. **Podman
  construction leaves the node only after step 4** — the pty plane still
  spawns through `PodmanService` until agent is carved out, so during steps
  2-3 the node owns the podman service and the daemon is its client. That
  temporary co-ownership is exactly why container ownership had to become
  instance-id-scoped rather than a single flat label.
- All parity litmus tests green; `/v1` changes additive-only.
