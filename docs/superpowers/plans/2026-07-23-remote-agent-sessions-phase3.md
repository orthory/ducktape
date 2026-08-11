# Remote Agent Sessions Phase 3 — `ducktape agent sched` (pinned headless runs)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `ducktape agent sched --cred jess-fable-1 --node alice --cpu 1 --mem 2g -- "<prompt>"` submits a **durable** headless provider run pinned to a named node, executed with a named credential resolved on the executing node, its live output on the existing `run-output:<id>` topic and its final result in the run's own committed saga record. The target node may be offline at submit time and run on reconnect — durability is the entire point of making `sched` a consensus op.

**Architecture:** A `sched` run is a **directed saga**, not a new module intake. `SagaMsg::Trigger` already carries `pinned_assignee` (static per-attempt binding to one node key) and `demands` (`cores`/`mem_gb` → the executing node's Podman `IsolationSpec`); the spec it carries is a `dispatch::WorkSpec` whose payload is a v3 run envelope. The ONLY new plumbing is a **credential name** riding that envelope and an **executing-node resolver** that turns the name into an `AirlockConfig::self_host` (Phase 1's programmatic broker config) against committed gateway-module state, verifying the grant for the run's committed saga origin. Output and result are already surfaced for every saga-driven run by the oracle pool — no runs-module involvement, and **no consensus-module change, no app-hash flag day, no wasm regen** anywhere in this phase.

**Tech Stack:** Rust; existing crates only — `dispatch-oracle` (envelope + pool, host-side), `capability-host` (`RunContext`, broker — host library, NOT a wasm module), `bin/node` (resolver wiring), `saga`/`gateway`/`identity` (read-only, via committed queries). No new dependencies.

**Spec:** `docs/superpowers/specs/2026-07-23-remote-agent-sessions-design.md` (Scheduling section, CLI surface).
**Phase 1:** `docs/superpowers/plans/2026-07-23-remote-agent-sessions-phase1.md` — this phase consumes its Task 5 interfaces (`ResolvedCredential`, `AirlockConfig::self_host`, `RunAuth.airlock`) and Task 7 (`SessionRequest.account_b64` gateway-side grant gate). Phase 1 MUST be merged first.

---

## Intake decision (from the code) — a directed saga, NOT the runs intake

The design names two candidate intakes; the code settles it decisively:

- **The runs intake is agent-shaped and would fork every path.** `RunsMsg::RequestRun` (`crates/modules/apps/runs/src/admin.rs:104`) is keyed on `(agent_id, channel_id, anchor_seq)` — a real chat anchor: it looks up an `AgentRecord`, calls `prepare_dispatch` which pins a chat transcript window (`dispatch_flow.rs:245`, requires an `AgentRecord`), validates the model's reply against the agent's `allowed_actions` and **posts it to a chat channel** (`response.rs`), and records history in a ring (`RunRecord`) keyed by `(agent_id, channel_id, anchor_seq)`. A headless `sched [<provider>] --cred -- "<prompt>"` has **no agent, no channel, no anchor**. Threading it through runs means a parallel headless twin of every one of those paths — the dual-path defect the house rules forbid — and runs would ultimately stage a dispatch → saga anyway.
- **The tasks board is first-claim, the opposite of directed.** `tasks` (`crates/modules/apps/tasks/src/interface.rs`) is a job board any capable worker claims. `sched` must PIN to a chosen node, not offer the work to a pool.
- **A directed saga already IS this run.** `SagaMsg::Trigger.pinned_assignee` (`crates/modules/system/saga/src/interface.rs:162`) "leases every attempt to exactly this node key … a dark pinned node burns attempts through lease expiry until the saga fails or times out" — precisely the offline-executes-on-reconnect durability the design requires. `demands` ride the `Trigger` and the re-emitted `WorkerRequest` → `WorkSpec.demands` → the oracle sets `RunContext.limits` (`dispatch-oracle/src/pool.rs:407`) → Podman `--cpus`/`--memory` (`RunContext.limits` doc, `capability-host/src/lib.rs`). The oracle already streams provider output to `run-output:<run_key>` (`RunContext.run_key`, keyed by the saga id's last `\x1f`-segment, `pool.rs:80`) and the final bytes land in `SagaView.result`. `SagaMsg::Trigger` is not origin-gated (only `Cancel`/`Prune` are) so a member submits it directly at `{node}/v1/submit` with `target:"saga"`.
- **Attribution is cryptographic for free.** The frameless `/v1/submit` lane stamps the submitting node's key as the op origin, and the saga records it (`SagaView.origin`). Because a member always submits on **their own** node, that origin maps (via `IdentityQuery::OfNode`) to the submitter's account — the account the credential grant is checked against. The executing node reads it from **committed saga state**, never from user-supplied envelope bytes.

Net: this phase adds **one envelope field + one host-side resolver + wiring**, and touches zero wasm modules.

---

## Global Constraints

- Work in a worktree at `<primary>/.worktree/remote-agent-sched-phase3`, branch `feat/remote-agent-sched-phase3` off `origin/dev` (after Phase 1 merges); deliver as PR(s) against `dev`. Create it with the superpowers:using-git-worktrees skill before Task 1.
- **No `module-dev` skill, no wasm regen, no schema bump.** This phase is entirely host-side (`dispatch-oracle`, `capability-host`, `bin/node`). If a step ever seems to require touching a `crates/modules/**/guest.rs` or `MODULE_STATE_SCHEMAS`, stop — the intake decision has been violated.
- Lint gate per touched crate: `cargo clippy -p <crate> --tests --no-deps`. Format only code you touched; never `cargo fmt --all`.
- `tracing` only in node/library code — never `println!`/`eprintln!`. Never log key material or a gateway route/token; `reason` fields are snake_case tokens. The `error`/`warn`/`info`/`debug` discipline from `CLAUDE.md` applies (a per-run resolve is `debug`, a refusal is `warn` at most once per run).
- Tests synchronize on events (channel recv, stream frame, committed-state read), never on sleeps.
- No versioned names anywhere (`v2`/`v3` type/field/route names). The run envelope's existing `ducktape_run` marker is not renamed.
- Explicit control flow / named-predicate / one-visible-dispatch house rules bind the resolver and the pool seam.
- Commits end with `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.

---

### Task 1: Run envelope carries a credential name; the pool surfaces it

**Files:**
- Modify: `crates/modules/system/dispatch-oracle/src/envelope.rs` (`WireEnvelope` :46, `Prepared` :109, `prepare` :121)
- Test: the `envelope.rs` unit tests (existing `#[cfg(test)] mod tests`)

**Interfaces:**
- Produces (Task 2 and the CLI builder depend on these exact names):
  - `WireEnvelope` gains `#[serde(default, skip_serializing_if = "Option::is_none")] credential: Option<String>`.
  - `Prepared` gains `pub credential: Option<String>` (surfaced from the decoded envelope; `None` for every existing composer output, which omits the key).
  - `pub fn compose_headless(run_id: &str, prompt: &str, credential: Option<&str>) -> String` — the ONE composer for a `sched` payload, so the CLI builder never hand-rolls envelope internals and the schema lives in one place. Emits a minimal valid v3 envelope: `ducktape_run = RUN_ENVELOPE_VERSION`, `agent_id`/`agent_display_name = "sched"`, `run_id`, `instructions = prompt`, empty `contract`/`conversation`, `thread_key: null`, `workspace = { "kind":"duckfs", "source_prefix":"/shared/agent-workspaces/sched", "source_snapshot": null }` (a fresh per-run checkout — a headless prompt has no pinned workspace), `skills: []`, `library_readable: false`, `result_contract = { "ducktape_runner_result": RUNNER_RESULT_VERSION }`, and `credential` when given.

- [ ] **Step 1: Write failing tests** in the existing test module:

```rust
#[test]
fn headless_envelope_round_trips_the_credential() {
    let json = compose_headless("sched\u{1f}d1", "summarize this", Some("jess-fable-1"));
    let prepared = prepare(&json).expect("a valid v3 envelope");
    assert_eq!(prepared.credential.as_deref(), Some("jess-fable-1"));
    assert!(prepared.input.contains("summarize this"));
}

#[test]
fn credentialless_envelope_prepares_with_none() {
    // an ordinary composer envelope (no `credential` key) still decodes.
    let json = compose_headless("sched\u{1f}d2", "hello", None);
    assert!(prepare(&json).expect("valid").credential.is_none());
}
```

- [ ] **Step 2: Run to verify failure.** `cargo test -p dispatch-oracle credential` → FAIL (field/fn absent).
- [ ] **Step 3: Implement.** Add the field to `WireEnvelope` and `Prepared`, set `credential: envelope.credential` in `prepare` (alongside the existing `ctx`/`workspace` build), and add `compose_headless`. Keep `prepare`'s existing loud-error behavior for non-v3 payloads unchanged — the new field is additive.
- [ ] **Step 4: Run tests.** `cargo test -p dispatch-oracle` → all PASS.
- [ ] **Step 5: Lint + commit.**

```bash
cargo clippy -p dispatch-oracle --tests --no-deps
git add crates/modules/system/dispatch-oracle/src/envelope.rs
git commit -m "feat(dispatch-oracle): run envelope carries a credential name + headless composer"
```

---

### Task 2: Executing-node credential resolution — pool seam + node resolver

**Files:**
- Modify: `crates/modules/system/capability-host/src/lib.rs` (`RunContext` struct ~:the definition read at review time; broker-construction seam `resolve_anthropic_upstream` / `RunAuth` build — Phase 1 Task 5 left `RunAuth.airlock` here)
- Modify: `crates/modules/system/dispatch-oracle/src/pool.rs` (`DispatchPool` struct :~, `with_limit` :217, the post-`prepare` block :400-412)
- Create: `bin/node/src/cred_resolve.rs` (the `CredentialResolver` impl; keep `oracle_pool.rs` from growing — mono-file rule)
- Modify: `bin/node/src/oracle_pool.rs` (`build` :40 — accept + thread the resolver), `bin/node/src/boot/surfaces.rs` (:212 — build the resolver from `http_handle`, beside `agent_provisioner`), `bin/node/src/validator/run.rs` (:346), `bin/node/src/replica/park.rs` (:543), `bin/node/src/main.rs` (module decl)
- Test: `pool.rs` unit tests (resolver seam), `cred_resolve.rs` unit tests (grant refusal, unknown cred, non-external origin)

**Interfaces:**
- Consumes: Task 1 (`Prepared.credential`); Phase 1 Task 5 (`capability_host::ResolvedCredential`, `capability_host::AirlockConfig::self_host`, `RunAuth.airlock`); Phase 1 Task 7 (`airlock::SessionRequest.account_b64` gateway grant gate); `gateway::{CredentialRecord, CredentialKind, credential_use_allowed, GatewayQuery::Credential}`; `saga::{SagaQuery::Get, SagaView, SagaOrigin}`; `identity::IdentityQuery::OfNode`.
- Produces (Task 3 + the CLI builder depend on these):
  - In `capability-host`: `RunContext` gains `pub airlock: Option<AirlockConfig>` (the resolved per-run broker config) and `pub on_behalf: Option<Vec<u8>>` (the account the run acts for — the credential-grant subject). The broker-construction seam feeds `ctx.airlock` into `RunAuth.airlock` (takes precedence over `from_env`, per Phase 1 Task 5) and `ctx.on_behalf` into `SessionRequest.account_b64` (Phase 1 Task 7). When both are `None`, behavior is exactly today's (`from_env` / host credential).
  - In `dispatch-oracle`: `pub trait CredentialResolver: Send + Sync { async fn resolve(&self, credential: &str, saga_id: &str) -> Result<Resolved, String>; }` with `pub struct Resolved { pub airlock: AirlockConfig, pub on_behalf: Vec<u8> }`, and `pub type SharedCredentialResolver = std::sync::Arc<dyn CredentialResolver>`. `DispatchPool` gains an `Option<SharedCredentialResolver>` set by a chainable `pub fn with_credential_resolver(self, r: SharedCredentialResolver) -> Self` (an Option field, NOT a new `with_limit` positional arg — the ~8 existing `with_limit` test call sites stay untouched).
  - In `bin/node`: `pub struct NodeCredentialResolver` implementing `CredentialResolver`, constructed from a `http_handle` clone (the same committed-state query lane `agent_provisioner` uses); `oracle_pool::build` gains a `resolver: Option<SharedCredentialResolver>` parameter threaded to `with_credential_resolver`.

- [ ] **Step 1: Pool seam — write the failing test.** In `pool.rs` tests, mirror the existing `pool_with_capacity` harness. A fake resolver returns a fixed `Resolved`; assert that a job whose envelope carries `credential` reaches the provider with `RunContext.airlock`/`on_behalf` set, and that a resolver ERROR becomes the saga's `OracleResult(Err(...))` (never a spawned provider run). A credential-less job runs untouched.

```rust
#[tokio::test]
async fn a_credential_envelope_resolves_into_the_run_context() {
    let resolver: SharedCredentialResolver = Arc::new(FixedResolver::ok(sample_airlock(), b"acct".to_vec()));
    let (pool, mut rx) = pool_with_capacity(providers_capturing_ctx(), 1, Default::default());
    let pool = pool.with_credential_resolver(resolver);
    pool.run(&effect_with_payload("s1", 0, Some(b"me"),
        &crate::envelope::compose_headless("sched\u{1f}d1", "hi", Some("jess-fable-1")).into_bytes()))
        .await.unwrap();
    let ctx = last_run_ctx().await; // the capture provider stashes its RunContext
    assert!(ctx.airlock.is_some());
    assert_eq!(ctx.on_behalf.as_deref(), Some(b"acct".as_slice()));
}

#[tokio::test]
async fn a_resolver_refusal_fails_the_run_without_spawning() {
    let resolver: SharedCredentialResolver = Arc::new(FixedResolver::err("credential_not_granted"));
    let (pool, mut rx) = pool_with_capacity(counting_providers(), 1, Default::default());
    let pool = pool.with_credential_resolver(resolver);
    pool.run(&effect_with_payload("s2", 0, Some(b"me"),
        &crate::envelope::compose_headless("sched\u{1f}d2", "hi", Some("missing")).into_bytes()))
        .await.unwrap();
    let (_, _, outcome) = next_result(&mut rx).await;
    assert!(outcome.unwrap_err().contains("credential_not_granted"));
    assert_eq!(spawn_count(), 0, "a refused credential never launches a provider");
}
```

- [ ] **Step 2: Run to verify failure**, then implement the pool seam. In the owner future, after `envelope::prepare` succeeds and before `execute` (`pool.rs:402-412`), add ONE named-predicate block: `if let Some(name) = prepared.credential.take()` → require the resolver (absent resolver + a credential = the error `"this node has no credential resolver"`), `resolver.resolve(&name, &job.saga_id).await?`, set `prepared.ctx.airlock` / `prepared.ctx.on_behalf`. A resolve `Err` short-circuits the `run` async block to that error string — which the existing pool path already turns into the `OracleResult(Err)` for the attempt. No credential + no resolver = the block is skipped and the run is unchanged.
- [ ] **Step 3: capability-host — thread `ctx.airlock`/`ctx.on_behalf`.** Add the two `RunContext` fields (both `Default` `None`). In the broker-construction seam Phase 1 Task 5 touched (`resolve_anthropic_upstream` / `RunAuth` build), set `RunAuth.airlock = ctx.airlock.clone()` when present (Phase 1 already gives it precedence over `from_env`), and pass `ctx.on_behalf` to the session open so `SessionRequest.account_b64` carries it (Phase 1 Task 7's field). Add one unit test: a `RunContext { airlock: Some(cfg), .. }` produces a `RunAuth` that selects `cfg` even with `DUCKTAPE_AIRLOCK_*` unset.
- [ ] **Step 4: Node resolver — write failing tests** in `cred_resolve.rs`. Drive `NodeCredentialResolver::resolve` against a fake committed-query handle returning canned `GatewayReply::Credential` / `SagaReply::Saga` / `IdentityReply`. Cases: owner account resolves (Ok, `on_behalf == owner`); granted account resolves; ungranted account → `Err("credential_not_granted")`; unknown name → `Err("unknown credential: …")`; saga origin `Module`/`System` (not `External`) → `Err("scheduled run has no account origin")`; node key not bound to an account → `Err`.
- [ ] **Step 5: Implement `NodeCredentialResolver`.** `resolve(credential, saga_id)`:
  1. `GatewayQuery::Credential { name: credential }` → record, else `Err("unknown credential: {credential}")`.
  2. `SagaQuery::Get { saga_id }` → `SagaView.origin`; `let SagaOrigin::External(node_key) = origin else { return Err("scheduled run has no account origin") }`.
  3. `IdentityQuery::OfNode { node_key }` → `account`, else `Err("submitting node is not bound to an account")`.
  4. `let allowed = gateway::credential_use_allowed(&record, &account); if !allowed { return Err("credential_not_granted") }` (fast local gate — the owner's gateway is still the final word).
  5. Build `ResolvedCredential { name: record.name, kind: map_kind(record.kind), authority: RouteName::named("airlock").to_string(), via: record.publisher_node, seal_pk: record.seal_pk }` (mirror Phase 1 Task 7's `resolved_credential_from`; when `record.publisher_node == this node` the Phase-1 loopback short-circuit applies). Return `Resolved { airlock: AirlockConfig::self_host(&resolved), on_behalf: account }`. `map_kind` maps `gateway::CredentialKind` → `capability_host::CredentialKind` (the node owns this mapping — capability-host does not depend on the gateway crate, per Phase 1 Task 5).
- [ ] **Step 6: Wire it.** In `boot/surfaces.rs:212` build `let cred_resolver: dispatch_oracle::SharedCredentialResolver = Arc::new(cred_resolve::NodeCredentialResolver::new(http_handle.clone()));` beside `agent_provisioner`, and flow it to both `oracle_pool::build` sites (`validator/run.rs:346`, `replica/park.rs:543`) through the same struct/channel `agent_provisioner` already rides. `oracle_pool::build` passes it to `with_credential_resolver`.
- [ ] **Step 7: Run gates.**

```bash
cargo test -p dispatch-oracle -p capability-host
cargo test -p node cred_resolve
```

- [ ] **Step 8: Lint + commit.**

```bash
cargo clippy -p dispatch-oracle --tests --no-deps
cargo clippy -p capability-host --tests --no-deps
cargo clippy -p node --tests --no-deps
git add crates/modules/system/dispatch-oracle crates/modules/system/capability-host bin/node
git commit -m "feat(node): resolve a run's named credential to a self-host airlock on the executing node"
```

---

### Task 3: Two-node e2e — submit on A pinned to B, B runs against a mock upstream

**Files:**
- Create: `bin/node/tests/sched_pinned_run.rs` (or extend the Phase 1 `cred_lending.rs` cluster harness if it already spins the two-node fixture — check `bin/node/tests/` and reuse the fixture)
- Test: itself

**Interfaces:**
- Consumes: everything above; the airlock testkit mock upstream (`airlock::testkit`, as Phase 1 Task 7 used); the existing two-node real-socket cluster fixture the `qa` skill names.

- [ ] **Step 1: Write the test.** Event-driven throughout; no sleeps.

```rust
#[tokio::test]
async fn a_pinned_scheduled_run_executes_on_the_target_with_the_named_credential() {
    let cluster = two_node_cluster().await;
    let (submitter, target) = (cluster.node(0), cluster.node(1));

    // target owns + serves a self-host credential, and it is registered + granted
    // to the submitter's account (Phase 1 machinery).
    seed_cred_dir(target.storage(), "owner-claude-1", bearer("tok-sched"));
    submit_signed_set_credential(&target, "owner-claude-1", seal_pk_of(target.storage())).await;
    submit_signed_grant(&target, "owner-claude-1", submitter.account_id()).await;
    cluster.wait_committed().await;

    // submitter builds the directed saga: pinned to the target, cred in the envelope,
    // demands = cpu/mem, and submits it at its OWN node (origin = submitter's node key).
    let dispatch_id = "sched-e2e-1";
    let saga_id = format!("sched\u{1f}{dispatch_id}");
    let mut out = subscribe_run_output(&submitter, dispatch_id).await; // ws run-output:<id>
    submit_sched_trigger(&submitter, &saga_id, target.node_key(),
        &dispatch_oracle::envelope::compose_headless(&saga_id, "PING", Some("owner-claude-1")),
        &[("cores", 1), ("mem_gb", 2)]).await;

    // the target executes; the mock upstream saw the credential's bearer, live output
    // streamed to the submitter's run-output topic, and the terminal result committed.
    let line = out.next_line().await.expect("live output crosses to the submitter node");
    assert!(line.contains("PONG"));
    let view = wait_saga_done(&submitter, &saga_id).await; // event-driven on committed status
    assert!(view.result.is_some(), "the final result committed to the saga record");

    // negative: a run whose submitter account was never granted fails at resolve —
    // no provider spawn, the saga carries the refusal.
    let stranger = cluster_with_ungranted_account().await;
    let err = wait_saga_failed(&stranger, /* pinned to target, same cred */).await;
    assert!(err.contains("credential_not_granted"));
}
```

Adjust helper names to the fixture's real API; the assertions + event-driven waits are the contract. The mock upstream is the airlock testkit's, wired as the target gateway's `anthropic_base` (Phase 1 Task 7 pattern). The submit helper POSTs `{ target: "saga", payload: saga::encode_msg(&trigger) }` to the submitter's `/v1/submit`.

- [ ] **Step 2: Run.** `cargo test -p node --test sched_pinned_run` → PASS. Rerun touched suites: `cargo test -p dispatch-oracle -p capability-host`.
- [ ] **Step 3: Lint + commit.**

```bash
git add bin/node
git commit -m "test(node): two-node pinned scheduled-run e2e with a granted credential"
```

---

## Interfaces for the CLI builder (`ducktape agent sched` — separate task)

The CLI verb is **out of scope** here. A separate builder wires `ducktape agent sched [<provider>] --cred <name> [--node <name>] [--cpu <cores>] [--mem <size>] -- "<prompt>"` on top of this phase using EXACTLY these contracts. No consensus op is added — `sched` is a plain `SagaMsg::Trigger` submit.

**Submit payload (the whole intake):**

1. Resolve `--node <name>` → target node key: `IdentityQuery::OfMember`/`Accounts` → match `display_name` → `AccountView.nodes[].node_key` (error listing candidates when an account operates >1 node; also accept a raw node key). No `--node` = this node's own key (a local durable run).
2. Resolve `--cred <name>`: query the local node `GatewayQuery::Credential { name }` for existence + to derive the provider when `<provider>` is omitted (`record.kind`); an explicit `<provider>` contradicting `record.kind` is an error. `--cred` is REQUIRED for `sched` (a headless guest run must bring a credential — spec).
3. Pick `dispatch_id` = a run nonce; set `saga_id = format!("sched\u{1f}{dispatch_id}")` so the oracle's `run_key_for` yields `<dispatch_id>` and output streams to `run-output:<dispatch_id>`.
4. Compose the payload with `dispatch_oracle::envelope::compose_headless(&saga_id, prompt, Some(cred_name))` (Task 1). Wrap it:

```rust
let demands = BTreeMap::from([("cores".into(), cpu), ("mem_gb".into(), mem_gb)]); // absent flags omitted
let spec = dispatch::encode_work_spec(&dispatch::WorkSpec {
    kind: dispatch::WORK_SPEC_KIND.into(),
    dispatch_id: dispatch_id.into(),
    capability: provider_capability_tag.into(),       // the executing node's provider tag (claude/codex spec tag)
    payload: compose_headless(&saga_id, prompt, Some(cred_name)).into_bytes(),
    demands: demands.clone(),
    admission: dispatch::AdmissionPolicy::Queue,
});
let trigger = saga::SagaMsg::Trigger {
    saga_id: saga_id.clone(),
    spec,
    reply_to: None,                                   // fire-and-forget; output+result surface elsewhere
    reply_payload: Vec::new(),
    deadline: None,                                   // or an absolute view bound if --deadline is added later
    max_attempts: 3,
    lease_views: None,
    capability: Some(provider_capability_tag.into()), // recorded; a pin ignores it for assignment
    demands,                                          // recorded (pin ignores for assignment)
    pinned_assignee: Some(target_node_key),           // THE pin — every attempt leases to the target
};
// POST {node}/v1/submit  { "target": "saga", "payload": saga::encode_msg(&trigger) }   (redeem-invite pattern)
```

Provider-availability preflight (spec): before submit, query the TARGET node's announced providers in the capability registry and fail with a clear error when it advertises no provider for `provider_capability_tag`; a dark pinned node cannot be preflighted, so a pinned offline node surfaces as run failure (saga lease-expiry), which is the durability contract, not a bug.

**Query for the run id / output / result:**
- `sched` prints `saga_id` — the durable handle (`println!`, program output, not logging).
- Live output: subscribe the existing ws topic `run-output:<dispatch_id>` on the user's own node (it fans in from the target node over the existing agent-telemetry lane).
- Terminal result: `SagaQuery::Get { saga_id }` → `SagaReply::Saga(Some(view))`; `view.status` (`Done`/`Failed`/`TimedOut`/`Cancelled`), `view.result` (the `Ok` output bytes on `Done`), `view.error` (the failure/refusal string, e.g. `credential_not_granted`).
- Completion files (`ops/completions/ducktape.{bash,zsh}`) must carry `sched` and its long flags `--cred --node --cpu --mem`, or the drift guard (`bin/node/src/cli.rs`) fails — the CLI builder's concern, noted here so it is not missed.

---

## Testing (summary)

- **Unit:** envelope credential round-trip + `compose_headless` (Task 1); pool resolver seam maps a credential into `RunContext` and turns a resolve refusal into an unspawned `OracleResult(Err)` (Task 2 Step 1); `capability-host` threads `ctx.airlock`→`RunAuth` (Task 2 Step 3); `NodeCredentialResolver` owner/grantee/ungranted/unknown-cred/non-external-origin (Task 2 Step 4).
- **Two-node e2e (real-socket lane):** submit on node A pinned to node B, B executes against the airlock testkit mock upstream with the named credential, live output observed on A's `run-output:<id>` topic and the terminal result committed to the saga; plus the ungranted-account refusal. Event-driven waits only (Task 3).
- No app-hash/parity/wasm gates run in this phase — it changes no consensus module. `make wasm-modules-check` and the genesis-registry parity test are expected untouched and green.

Merge policy per repo rules: high confidence + green gates → PR(s) to `dev`. Natural PR split: Task 1 (envelope) + Task 2 (resolver) as one stack, Task 3 (e2e) on top; or all three as one PR given the small surface.
