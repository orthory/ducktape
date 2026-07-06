# Quack System Base — Primitive Design

**Status:** design of record for `epic/quack-system-base`
**Date:** 2026-07-07
**ADR:** `docs/adr/2026-07-07-quack-packaged-modules.mdx` (the package standard this implements)
**Predecessor spec:** `docs/superpowers/specs/2026-07-06-packaged-module-agent-harness-design.md`

## Goal

Build the primitive system base that makes a Quack package installable, suspendable,
unpluggable, and provable on a Ducktape network — without yet building Wasm loading,
live-network module-set installs, or dynamic UI bundles. After this epic, a package
is: a signed manifest + resources capsule (`.quack`), a consensus `package` module
that owns lifecycle + action routing, a harness contract every package harness
implements, generalized agent prompts (`PromptRef`) and open action tags routed
probe/apply to owner modules, hook events on `pages`, a reference `docs-harness`
package proving the whole loop deterministically, and `ducktape-node package`
tooling.

## Research grounding (what exists today)

Verified against dev @ `d350e81` by five deep-dive passes:

- **Greenfield:** zero package/wasm/wit/manifest-standard code exists. All "quack"
  hits are the `quackbot` test fixture.
- **Module kernel:** `sdk::Module` is object-safe with opaque `Vec<u8>` payloads;
  cross-module writes are follow-up `Msg`s only (host drain, `MAX_DISPATCHES=1024`,
  same-block P2 atomicity); reads are `Ctx::query` (read-only, cycle-guarded).
  Registry is `BTreeMap<ModuleId, Box<dyn Module>>`, genesis-only; adding a module
  changes the app-hash (count + pair), so module-set changes are lockstep.
- **Origin model:** follow-ups are host-tagged `Origin::Module(emitter)` — spoof-proof.
  `agent` already accepts module-origin owners (`admin_origin`) and owner-gates every
  mutation (`owned_agent`). `SagaOrigin` is the persisted authorship mirror.
- **Agent pipeline:** `AgentRecord.prompt_hash` is a 32-byte pin that **nothing
  resolves** (`runs::render_payload` uses a generic `DEFAULT_PROMPT`; dispatch-oracle
  feeds payloads verbatim). Grants are already open strings (`allowed_actions:
  Vec<String>`) but validated against a closed `KNOWN_ACTIONS` list of 3; the
  response vocabulary is a closed `AgentAction` enum; `runs::validate_response` /
  `emit_response` hardwire tasks/chat as owners. No tombstone status exists; unplug
  would orphan the dispatch recipe.
- **Hooks:** three registration idioms exist. The **tagging plane** is the safe one
  (subscriber derived from `Origin::Module`, never payload); chat's
  `RegisterHook`/`SetMembership` accept any origin with the target as a payload field
  (the known hole); memory `RegisterWatch` is in between. `pages` has **no** event
  surface. Delivery is always follow-up `Msg` in the same block. Registration ops
  (riding the caller's block) may fail; delivery/apply arms (riding someone else's
  block) must be no-fail — automations' probe-before-emit + error-row
  (`RunRecord{action_ok:false}`) + `emit_event` breadcrumb is the idiom.
- **Upgrade machinery is BUILT** (contra older notes): `crates/system/upgrade` +
  governance `ScheduleUpgrade` + host `pending_advance`/`effective_version`/
  `set_active_version`. It flips a non-hashed `protocol_version: u32` for dual-path
  modules only; it does not add registry entries. Module-set changes remain a
  lockstep/new-genesis concern.
- **Reusable seeds:** ed25519 sign/verify shape from `wireguard-upgrade`
  (struct + detached sig + domain namespace); domain-separated sha256 from
  `genesis_namespace`; content digests = raw-bytes sha256 like `files` chunks; blob
  plane already shared by forge for artifact bodies; `Host::genesis` +
  `dispatch-oracle::StubProvider` + canned-oracle (`collaboration_loop.rs`) are the
  harness substrate; snapshot round-trip suites are the golden pattern; the node CLI
  is a hand-rolled verb dispatcher in `bin/node/src/main.rs` (`parse_flags`);
  generic RPC means new modules need no node RPC changes.

## Scope

**In:** manifest crate + capsule format; `package` system module (lifecycle, action
routes, harness drive); harness contract (`HarnessMsg`, `PackageActionQuery/Reply/Msg`);
`PromptRef` + real prompt resolution; open `RequestedAction` tags with probe/apply
routing; agent tombstone + recipe teardown; pages hook events; chat hook-gating
hygiene; reference `docs-harness` package + deterministic package harness test;
`ducktape-node package inspect|build|verify|test`; genesis registration of `package`
in both binaries; Modules-view package metadata; vocs docs (en/ko).

**Out (deliberate, per ADR non-decisions):** Wasm/WIT artifacts and any interface-hash
enforcement beyond manifest digests; installing new module *code* on a live network
(needs the module-set upgrade path — documented, not built); dynamic UI bundle
loading (`ui/` is metadata only; views stay in-binary); governance-gated install
(v1 install is a member op); `SetMembership` hardening (private-messaging ADR
territory); stateful export.

## Design decisions

### D1. Capsule format (`.quack`)

A package *source* is a directory (`quack.toml` at root, resource files beside it).
A `.quack` file is a **deterministic ustar tar** of that directory: entries sorted
by path, mtime=0, uid/gid=0, no user names, regular files + dirs only. Rationale:
single-file distribution with reproducible bytes; format is an ADR non-decision so
we pick the simplest deterministic container. The CLI accepts both forms everywhere.

Hashes: every `hash = "sha256:<hex>"` in the manifest is the sha256 of the raw
referenced file bytes (identical discipline to `files` chunk digests). The
**manifest hash** is `sha256(b"ducktape:quack:manifest:v1:" ++ raw quack.toml bytes)`
— the file itself is the canonical artifact (it ships in the capsule), so no
canonical-TOML re-serialization is needed.

Signature: `signatures/package.sig` = JSON `{signer: <hex ed25519 pub>, sig: <hex>}`
over the manifest hash, namespace `b"ducktape:quack:sig:v1:"`, using the
`commonware_cryptography` ed25519 sign/verify shape from `wireguard-upgrade`.
Unsigned packages are inspectable/testable but `verify` reports them unsigned.

### D2. Manifest schema (v1, native modules)

`schema = 1` TOML per the ADR, with one v1 extension: `[[modules]]` entries carry
`kind = "native" | "wasm"`. v1 only accepts `kind = "native"`, where the module code
ships in the node binary and `artifact`/`abi` are omitted; `wasm` entries parse but
are rejected by v1 validation with an explicit "wasm loading not yet supported".
This keeps the ADR's package standard intact while the execution artifact stays
native (the ADR explicitly allows early builds to keep code in the binary).

Package-local ids (`logical`) are mapped to concrete `ModuleId`s at install; every
manifest cross-reference (`owner`, `agents[].prompt`, `engagements[].source`) is by
logical id. Tag shape rule is the platform-wide `[a-z0-9._-]{1,64}` (same as
`capability::validate_tag`). Agent action tags must be declared in `[[actions]]`.

### D3. `package` system module (consensus registry)

New module id `package` (`crates/system/package`), snapshot-bytes substrate
(memory/tasks pattern: canonical encode → sha256 root, `SnapshotBytes` state sync).

State:
- `packages: BTreeMap<String, PackageRow>` — `PackageRow { package, version,
  manifest_hash: [32]u8, status: PackageStatus, modules: BTreeMap<logical, ModuleId>,
  harness: ModuleId, installer: SagaOrigin, uninstall: UninstallPolicy,
  installed_at/updated_at heights }`.
- `routes: BTreeMap<String /*tag*/, ActionRouteRow { owner: ModuleId, package:
  Option<String>, schema_hash: Option<[32]u8> }>` — the tag→owner registry (ADR
  non-decision resolved: routes live in package-install state, served by this
  module). **Built-in routes** are seeded at genesis by the module constructor:
  `tasks.create → tasks`, `tasks.update_status → tasks` (built-in actions become
  action specs, per ADR). Route collisions reject the install.
- `PackageStatus { Installing, Active, Suspended, Unplugging, Inactive }` (ADR
  lifecycle; `Available` is the off-chain state of an uninstalled capsule).

Ops (`PackageMsg`, serde_json like every module):
- `Install(InstallSpec)` — non-empty External or Module origin (v1 posture: any
  authenticated member may install; governance gating is a later layer). Validates:
  unknown package id, all mapped module ids exist (`ctx.module_root`), harness
  mapped, tags valid + unrouted, prompt hashes well-formed, caps. Stages the row
  (`Installing`), stages routes, then emits **in the same block**: one
  `memory::Publish` per prompt seed, then `HarnessMsg::InstallPackage{spec}` to the
  harness. The harness (module origin!) registers its own agents and hooks — which
  is what makes tagging-idiom hook registration and `agent` module-origin ownership
  line up. The final follow-up `PackageMsg::MarkActive{package}` (module-origin,
  self-emitted via the harness ack — see D4) flips `Installing → Active`.
  **Atomicity is host-lent:** any failing step aborts the whole block, so a partial
  install cannot land (ADR "commits or aborts atomically").
- `Suspend{package}` / `Resume{package}` — installer- or harness-origin-gated;
  emits `HarnessMsg::SuspendPackage/ResumePackage`; status flips
  `Active ↔ Suspended`.
- `Unplug{package}` — emits `HarnessMsg::UnplugPackage`; removes the package's
  routes; status → `Inactive` (tombstoned row preserved for audit). User data is
  untouched (preserve-by-default per ADR).

Queries (`PackageQuery`): `ActionOwner{tag} → Option<ModuleId>`, `Get{package}`,
`List`, `RoutesForOwner{module}`.

### D4. Harness contract

Types live in `crates/system/package`'s `interface.rs` (types-only crate root, the
established cross-module vocabulary pattern). A harness module:

```rust
pub enum HarnessMsg {
    InstallPackage { package: String, spec: InstallSpec },
    SuspendPackage { package: String },
    ResumePackage  { package: String },
    UnplugPackage  { package: String },
}
```

- Routed by origin: a harness handles `HarnessMsg` only when
  `env.origin == Origin::Module(<package module id>)`.
- Install arm MAY fail (it rides the installer's own block — registration posture).
  On success it emits its `AgentMsg::RegisterAgent`s (owner = harness, `PromptRef`
  pointing at the seeded memory paths), its hook registrations
  (`PageMsg::RegisterHook` etc., self-derived), and finally
  `PackageMsg::MarkActive{package}` back to the package module (module-origin,
  gated to the recorded harness).
- Suspend/Resume/Unplug arms are lifecycle ops emitted from the package module's
  block; they pause/resume/tombstone the harness's agents (`AgentMsg::PauseAgent` /
  `ResumeAgent` / `TombstoneAgent`), unregister hooks, and stop minting jobs.

Package actions (the ADR contract, verbatim):

```rust
pub enum PackageActionQuery { Probe { action_id: String, tag: String, payload: Vec<u8>, run_context: Vec<u8> } }
pub enum PackageActionReply { Accepted, Rejected { reason: String } }
pub enum PackageActionMsg   { Apply { action_id: String, tag: String, payload: Vec<u8>, run_context: Vec<u8> } }
```

- `Probe` arrives via `Ctx::query` (read-only) — the owner validates schema, target
  existence, caps, authorship, idempotency against staged-or-committed state.
- `Apply` arrives as a follow-up `Msg` riding the **delivery block** and is
  **no-fail**: decode-or-`Ok(())`, re-probe cheaply, apply real writes as own-state
  stages or follow-ups; on late conflict push an error row + `emit_event` breadcrumb,
  always `Ok(())` (the dispatch-receiver contract).
- `run_context` = serde_json `{ run_id, agent_id, package? }`.

### D5. `PromptRef` (agent) + real prompt resolution (runs)

```rust
pub struct PromptRef { pub module: String, pub target: String, pub renderer: String, pub sha256: Vec<u8> /*32*/ }
```

- `AgentRecord.prompt: Option<PromptRef>` replaces `prompt_hash: Vec<u8>`.
  Registration validates: known module (`ctx.module_root`), 32-byte pin, renderer
  in the v1 set. **v1 renderers:** `"memory.generation"` — target is
  `"<path>@<generation>"`, resolved via `MemoryQuery::Read{path, generation}`;
  content must be `Body::Inline` and `sha256(body) == pin`.
- `runs::render_payload` becomes resolution-aware: when the agent has a `PromptRef`,
  runs queries the source module at compose time, verifies the pin, and prepends the
  content to the payload. **Pin mismatch or missing content fails the run
  deterministically** (failed outcome + breadcrumb, never a block abort) — the ADR
  "a run fails or skips if the prompt source does not hash to the registered pin".
  Agents with `prompt: None` keep today's `DEFAULT_PROMPT` behavior.
- This changes the agent module's canonical committed encoding — a **flag-day
  app-hash change**, accepted repo practice (lockstep upgrade, same as the video
  channel-bank move). Snapshot round-trip suites move in the same commit.

### D6. Open actions (`RequestedAction`) + routing in runs

```rust
pub struct RequestedAction { pub action_id: String, pub tag: String, pub payload: serde_json::Value }
pub struct AgentResponse   { pub reply_blocks: Vec<ReplyBlock>, pub actions: Vec<RequestedAction> }
```

- `agent::validate_actions` becomes shape-only (tag rule), dropping the
  `KNOWN_ACTIONS` membership test. `KNOWN_ACTIONS`/`AgentAction` are deleted.
- `runs::validate_response` per action: tag shape → grant (`allowed_actions`
  contains tag) → owner lookup (`Ctx::query(package, ActionOwner{tag})`) → owner
  `Probe` → keep only `Accepted`. Rejected actions are dropped with a breadcrumb
  and counted in the finalize summary (mutate nothing, record failure).
- `runs::emit_response` emits `PackageActionMsg::Apply` follow-ups to the owner —
  the generic path replaces the hardwired `TaskMsg` emission. `chat.post` stays the
  reply-block grant gate (replies are not actions).
- `tasks` becomes the first built-in action owner: implements `Probe` (existence /
  duplicate-id / status-value checks — ported from today's inline logic in runs)
  and a no-fail `Apply` arm (create/update, error breadcrumb on late conflict).
  Payload schemas: `tasks.create {task_id, title}`,
  `tasks.update_status {task_id, status}`.
- `runs` gains the package module id as a constructor arg (present in every
  binary's genesis after this epic).

### D7. Agent tombstone + recipe teardown

`AgentStatus::Tombstoned` + owner-gated `AgentMsg::TombstoneAgent` + a new
`AgentEvent::Tombstoned` hook arm; `runs::on_agent_event` tears down the dispatch
recipe for tombstoned agents (adding `RemoveRecipe` to dispatch if absent).
Tombstoned agents never engage, never claim jobs, and cannot be resumed
(audit-preserving terminal state per ADR unplug).

### D8. Pages events + chat hook hygiene

`pages` adopts the tagging idiom:

- `PageMsg::RegisterHook {}` / `UnregisterHook {}` — subscriber **derived from
  `Origin::Module`** (payload carries nothing), rejected for external origins;
  target-known + not-self validated; idempotent; `MAX_PAGE_HOOKS = 8`. Stored as a
  `BTreeSet<ModuleId>` under a reserved `\0hooks` key, folded into the qmdb root.
- `PageEvent { CommentAdded { page_id, target, thread_id, comment_id, author, text },
  ThreadResolved { page_id, thread_id, resolved },
  BlockUpdated { page_id, block_id } }` — emitted post-`apply` in `execute` as one
  follow-up per hook, same block (P2). Emitter is atomic like chat's; receivers own
  no-fail.
- Chat hygiene (scoped): `ChatMsg::RegisterHook/UnregisterHook` keep the payload
  `module_id` for operator (External) wiring, but a **Module origin may only
  register itself** (`module_id == emitter`). `SetMembership` is explicitly out of
  scope here (private-messaging ADR owns it).

### D9. Reference package: `docs-harness`

`crates/examples/docs-harness` — module id `docs-harness`, snapshot-bytes substrate.
Owns the ADR's worked example end-to-end:

- `HarnessMsg` arms per D4; registers `docs.editor` (v1 single agent; brainstorm/
  triage are content, not new machinery) with a `PromptRef` into the seeded
  `/packages/org.ducktape.docs/prompts/docs-editor.md` memory path.
- `PageEvent` intake (origin == pages, no-fail): `mention_or_assigned` policy —
  comment text mentioning `@docs.editor` mints one idempotent
  `JobsMsg::Submit { kind: "agent/docs.editor", spec }` (idempotency key stored).
- Owns tags `pages.comment.add`, `pages.block.update_text`, `pages.thread.resolve`:
  `Probe` validates against pages via `Ctx::query` (target exists, size caps,
  expected_hash guard); `Apply` translates to `PageMsg` follow-ups with error rows
  on late conflict.
- Package source at `packages/docs/` (quack.toml, prompts/, actions/*.schema.json,
  harness/golden.json) — the CLI-built `docs.quack` and the deterministic harness
  fixture in one place.
- The end-to-end proof is `crates/examples/docs-harness/tests/package_loop.rs`
  (collaboration_loop pattern): Host::genesis(package, memory, pages, tagging,
  dispatch, saga, jobs, agent, runs, docs-harness, tasks, chat) → Install op →
  assert agents/hooks/routes seeded with expected hashes → comment mentioning the
  agent → exactly one job → canned oracle returns a `pages.block.update_text`
  action → block edited; unauthorized/malformed actions mutate nothing + record
  failure; suspend stops new jobs, preserves pages; unplug removes routes/hooks,
  tombstones agents, preserves user data; snapshot round-trips reproduce roots.

### D10. CLI + genesis + surface

- `ducktape-node package inspect|build|verify|test <dir|.quack>` in
  `bin/node/src/package.rs`, wired into the existing verb dispatcher. `test` runs
  manifest/digest/signature verification plus the golden harness for packages whose
  modules exist in the binary's native catalog (v1: the docs package).
- `package` module registered in both genesis sets (`bin/node` 22→23,
  `bin/noded` 15→16, `MODULE_IDS`, demo/simnode as needed) — flag-day app-hash
  change, one lockstep bump for the whole epic.
- `noded::ModuleStatus` gains `package: Option<String>`, `package_version`,
  `lifecycle` (from the package module registry); TS twin + ModulesView rows.
  Install/suspend UI is deferred; the view stays read-only but package-aware.
- Docs: `docs/pages/{en,ko}/human/architecture/packages.mdx` + vocs sidebar; the
  agent-track operator lifecycle page rides the same slice.

## Security posture (v1)

- Hook/subscription registration: module origin = self only (tagging idiom)
  everywhere new; chat gains the module-self rule.
- Agents: package agents are owned by the harness module (existing owner-gating);
  never by an author-minted external key.
- Actions: grants are per-agent tag lists; routing only through recorded routes;
  owners re-validate on both Probe and Apply. An `Apply` from a module that isn't
  `runs` is admitted by origin-class but validated identically — no worse than
  today's ungated `TaskMsg` surface; tightening to a runs-id allowlist is noted
  follow-up hygiene.
- Install: any authenticated member (desktop = workspace key). Governance-gated
  install and capability-absence surfacing are documented follow-ups.
- Packages never carry capability provider specs/credentials (enforced by schema:
  no such fields exist).

## Workload split (branches from `epic/quack-system-base`)

| Branch | Content | Depends on |
|---|---|---|
| `feat/quack-manifest` | D1+D2 crate `crates/kernel/quack` + CLI inspect/build/verify + `packages/docs/` skeleton | — |
| `feat/quack-package-module` | D3+D4 `crates/system/package` + genesis registration | — |
| `feat/quack-promptref` | D5+D7 agent/runs + snapshot suites + TS mirrors | — |
| `feat/quack-pages-events` | D8 pages events + chat hook gating | — |
| `feat/quack-open-actions` | D6 runs routing + tasks owner + collaboration_loop | package-module, promptref |
| `feat/quack-docs-harness` | D9 reference package + package_loop e2e + CLI `package test` | all above |
| `feat/quack-surface` | D10 ModuleStatus/ModulesView + docs en/ko | package-module |

Wave 1 = the four independent branches in parallel; wave 2 = open-actions; wave 3 =
docs-harness + surface. Each branch merges back into the epic; the epic PRs into
`dev` as one lockstep consensus change.
