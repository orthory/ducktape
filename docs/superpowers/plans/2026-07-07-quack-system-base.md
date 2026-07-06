# Quack System Base Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the Quack packaged-module primitive system base per
`docs/superpowers/specs/2026-07-07-quack-system-base-design.md` (read it first —
all type shapes and rationale live there; the ADR is
`docs/adr/2026-07-07-quack-packaged-modules.mdx`).

**Architecture:** One new pure crate (`crates/kernel/quack`), one new consensus
module (`crates/system/package`), generalizations to `agent`/`runs`/`tasks`/`pages`/
`chat`, a reference package (`crates/examples/docs-harness` + `packages/docs/`),
CLI verbs in `bin/node`, and read-only surface in `bin/noded` + `app/`.

**Tech Stack:** Rust (workspace conventions: serde_json wire enums in types-only
`interface.rs` at crate root; manual canonical byte encoding + sha2 for roots;
`commonware_cryptography` ed25519; `toml = "0.8"`), TypeScript (app), vocs (docs).

## Global Constraints

- Every task runs inside a worktree forked from `epic/quack-system-base`; PR-less
  merge back into the epic branch when green.
- Wire payloads are serde_json via `encode_*`/`decode_*` helpers in `interface.rs`
  (copy the `pages`/`memory` pattern). No sdk dep in interface types.
- Tag shape rule everywhere: `[a-z0-9._-]{1,64}` (mirror
  `crates/system/capability/src/interface.rs::validate_tag`).
- Registration/admin op arms MAY return `Err` (they ride the caller's block).
  Event/`Apply` arms MUST be no-fail: decode-or-`Ok(())`, error rows +
  `ctx.emit_event` breadcrumbs, always `Ok(())`.
- Any change to a module's committed canonical encoding updates its
  `tests/snapshot_round_trip.rs` in the same commit (byte-identical root + tamper
  rejection), and `bin/demo/tests/joiner_rebuilds_global_app_hash.rs` must pass.
- `cargo fmt` + `cargo clippy --workspace --all-targets` clean; commit after each
  green test cycle with conventional-commit messages ending in the Claude co-author
  trailer.
- Do not touch `main`; the epic PRs into `dev` at the end.

---

### Task W1: `feat/quack-manifest` — manifest crate + CLI inspect/build/verify

**Files:**
- Create: `crates/kernel/quack/Cargo.toml`, `crates/kernel/quack/src/lib.rs`,
  `crates/kernel/quack/src/manifest.rs`, `crates/kernel/quack/src/capsule.rs`,
  `crates/kernel/quack/src/sign.rs`
- Create: `packages/docs/quack.toml`, `packages/docs/prompts/docs-editor.md`,
  `packages/docs/actions/pages.comment.add.schema.json`,
  `packages/docs/actions/pages.block.update_text.schema.json`,
  `packages/docs/actions/pages.thread.resolve.schema.json`
- Create: `bin/node/src/package.rs`
- Modify: root `Cargo.toml` (workspace member), `bin/node/src/main.rs` (verb
  dispatch `match` + `mod package;`), `bin/node/Cargo.toml` (dep on `quack`)
- Test: `crates/kernel/quack/src/*` inline `#[cfg(test)]` + `bin/node` CLI smoke
  via `cargo test -p quack -p ducktape-node`

**Interfaces (Produces):**
```rust
// quack::manifest
pub struct PackageManifest { pub schema: u32, pub package: String, pub version: String,
  pub requires: Requires, pub modules: Vec<ModuleEntry>, pub prompts: Vec<PromptEntry>,
  pub actions: Vec<ActionEntry>, pub agents: Vec<AgentEntry>,
  pub engagements: Vec<EngagementEntry>, pub install: InstallPolicy, pub uninstall: UninstallPolicy }
pub struct Requires { pub protocol_min: u32, pub modules: Vec<String>, pub capabilities: Vec<String> }
pub struct ModuleEntry { pub logical: String, pub default_id: String, pub kind: ModuleKind /* Native|Wasm */,
  pub artifact: Option<String>, pub abi: Option<String>, pub hash: Option<String> }
pub struct PromptEntry { pub logical: String, pub path: String, pub hash: String }
pub struct ActionEntry { pub tag: String, pub owner: String, pub schema: Option<String> }
pub struct AgentEntry { pub id: String, pub display_name: String, pub prompt: String,
  pub capability: String, pub actions: Vec<String>, pub status: String }
pub struct EngagementEntry { pub source: String, pub event: String, pub agent: String, pub policy: String }
pub fn parse_manifest(toml_bytes: &[u8]) -> Result<PackageManifest, ManifestError>;
pub fn validate(m: &PackageManifest) -> Result<(), ManifestError>;   // v1: rejects kind=wasm; tag shape; cross-refs resolve; dup logical ids
pub fn manifest_hash(toml_bytes: &[u8]) -> [u8; 32];                 // sha256("ducktape:quack:manifest:v1:" ++ bytes)
pub fn validate_tag(tag: &str) -> Result<(), ManifestError>;         // [a-z0-9._-]{1,64}
// quack::capsule
pub struct Capsule { /* holds files: BTreeMap<String, Vec<u8>> */ }
pub fn open_dir(path: &Path) -> Result<Capsule, CapsuleError>;
pub fn open_tar(bytes: &[u8]) -> Result<Capsule, CapsuleError>;
pub fn build_tar(c: &Capsule) -> Vec<u8>;                            // deterministic ustar: sorted paths, mtime 0, uid/gid 0
pub fn verify_digests(c: &Capsule, m: &PackageManifest) -> Result<(), CapsuleError>; // every hash field vs file bytes
// quack::sign
pub const SIG_NAMESPACE: &[u8] = b"ducktape:quack:sig:v1:";
pub fn sign_manifest(signer: &ed25519::PrivateKey, manifest_hash: &[u8;32]) -> PackageSig;
pub fn verify_manifest_sig(sig: &PackageSig, manifest_hash: &[u8;32]) -> bool;
pub struct PackageSig { pub signer: Vec<u8>, pub sig: Vec<u8> }       // JSON in signatures/package.sig
```
CLI verbs (hand-rolled dispatcher, copy `cmd_*` + `parse_flags` style):
`ducktape-node package inspect <dir|.quack>` (print manifest summary + hashes),
`package build <dir> [-o out.quack] [--key <keyfile>]`,
`package verify <dir|.quack>` (digests + signature status; exit non-zero on
mismatch). `package test` lands in W6.

- [ ] **Step 1:** Write failing tests in `crates/kernel/quack`: parse the
  `packages/docs/quack.toml` fixture; reject wasm-kind; reject bad tag; reject
  dangling `owner`/`prompt` logical refs; `manifest_hash` is stable and
  domain-separated; deterministic tar bytes are byte-identical across two builds
  and reorderings of input map insertion; digest mismatch detected; sign/verify
  round-trip + wrong-key rejection.
- [ ] **Step 2:** `cargo test -p quack` → all fail (crate skeleton only).
- [ ] **Step 3:** Implement manifest.rs / capsule.rs / sign.rs minimally to green.
- [ ] **Step 4:** Author the real `packages/docs/` fixture (manifest per ADR §Decision
  example with `kind = "native"`, prompts, three action schemas; hashes computed by
  `package build --emit-hashes` or a test helper that rewrites them).
- [ ] **Step 5:** Wire `bin/node/src/package.rs` verbs + dispatcher entry; smoke
  test: `cargo run -p ducktape-node -- package verify packages/docs` passes;
  `inspect` prints agents/actions/modules.
- [ ] **Step 6:** fmt/clippy/test; commit per green cycle.

---

### Task W2: `feat/quack-package-module` — the `package` consensus module

**Files:**
- Create: `crates/system/package/Cargo.toml`, `src/lib.rs`, `src/interface.rs`,
  `tests/package_module.rs`, `tests/snapshot_round_trip.rs`
- Modify: root `Cargo.toml`; `bin/node/src/main.rs` (`genesis_host`+`restore_host`
  module vecs + `MODULE_IDS` 22→23); `bin/noded/src/main.rs` (genesis 15→16);
  `bin/demo/src/main.rs`, `bin/simnode/src/main.rs` (register so shared tests keep
  passing); `bin/demo/tests/joiner_rebuilds_global_app_hash.rs` (expected set)

**Interfaces (Produces — full shapes in design doc D3/D4):**
```rust
// package::interface (types-only, serde_json)
pub const MODULE_PACKAGE: &str = "package";
pub enum PackageStatus { Installing, Active, Suspended, Unplugging, Inactive }
pub struct InstallSpec { pub package: String, pub version: String, pub manifest_hash: Vec<u8>,
  pub modules: Vec<ModuleBinding>, pub harness: String /*logical*/,
  pub prompts: Vec<PromptSeed>, pub agents: Vec<AgentSeed>, pub actions: Vec<ActionRoute>,
  pub engagements: Vec<EngagementRule>, pub uninstall: UninstallPolicy }
pub struct ModuleBinding { pub logical: String, pub module_id: String }
pub struct PromptSeed { pub logical: String, pub path: String, pub content: String, pub sha256: Vec<u8> }
pub struct AgentSeed { pub agent_id: String, pub display_name: String, pub capability: String,
  pub prompt: String /*prompt logical*/, pub actions: Vec<String>, pub active: bool }
pub struct ActionRoute { pub tag: String, pub owner: String /*logical*/ }
pub struct EngagementRule { pub source: String, pub event: String, pub agent: String, pub policy: String }
pub struct UninstallPolicy { pub pending_runs: String /*"drain"|"cancel"*/, pub user_data: String /*"preserve"*/ }
pub enum PackageMsg { Install(InstallSpec), MarkActive { package: String },
  Suspend { package: String }, Resume { package: String }, Unplug { package: String } }
pub enum PackageQuery { ActionOwner { tag: String }, Get { package: String }, List,
  RoutesForOwner { module: String } }
pub enum PackageReply { Owner(Option<String>), Package(Option<PackageView>), Packages(Vec<PackageView>), Routes(Vec<String>) }
pub enum HarnessMsg { InstallPackage { package: String, spec: InstallSpec },
  SuspendPackage { package: String }, ResumePackage { package: String }, UnplugPackage { package: String } }
pub enum PackageActionQuery { Probe { action_id: String, tag: String, payload: Vec<u8>, run_context: Vec<u8> } }
pub enum PackageActionReply { Accepted, Rejected { reason: String } }
pub enum PackageActionMsg { Apply { action_id: String, tag: String, payload: Vec<u8>, run_context: Vec<u8> } }
// + encode_/decode_ helpers for each enum (pages/memory pattern)
```
Module behavior: snapshot-bytes substrate (copy `memory`'s pending/commit/abort +
`snapshot()/install()` shape); constructor
`PackageModule::new(id, memory_module_id, builtin_routes: Vec<(String, String)>)`;
genesis wiring seeds `[("tasks.create","tasks"),("tasks.update_status","tasks")]`.
Install validation + same-block emissions per design D3; `MarkActive` accepted only
from the recorded harness's module origin; `Suspend/Resume/Unplug` gated to
installer origin or harness origin.

- [ ] **Step 1:** Failing tests in `tests/package_module.rs` (TestCtx pattern from
  `crates/apps/memory/tests/memory_module.rs`): install stages row+routes and emits
  prompt publishes + `HarnessMsg::InstallPackage` (assert staged `Msg` targets and
  payload decode); unknown module id in bindings rejects; route collision (incl. a
  builtin tag) rejects; `MarkActive` from wrong origin rejects, from harness origin
  flips `Installing→Active`; suspend/resume/unplug flip status + emit the matching
  `HarnessMsg`; unplug removes routes but preserves the row (Inactive) and route
  queries return None; `ActionOwner` resolves builtin and installed tags.
- [ ] **Step 2:** `cargo test -p package` → fails.
- [ ] **Step 3:** Implement interface.rs then lib.rs to green.
- [ ] **Step 4:** `tests/snapshot_round_trip.rs`: byte-identical root after
  snapshot/install; tampered/truncated/reordered snapshots rejected leaving target
  byte-identical (copy the agent suite's structure).
- [ ] **Step 5:** Genesis registration in all four binaries + `MODULE_IDS`; run
  `cargo test -p ducktape-demo --test joiner_rebuilds_global_app_hash` and fix the
  expected module set.
- [ ] **Step 6:** fmt/clippy/full-workspace test; commit per green cycle.

---

### Task W3: `feat/quack-promptref` — PromptRef + tombstone + prompt resolution

**Files:**
- Modify: `crates/apps/agent/src/interface.rs` (PromptRef, `prompt: Option<PromptRef>`
  replacing `prompt_hash`, `AgentStatus::Tombstoned`, `AgentMsg::TombstoneAgent`,
  `AgentEvent::Tombstoned`, shape-only `validate` for actions),
  `crates/apps/agent/src/lib.rs` (state, canonical encoding, arms, origin gates),
  `crates/apps/agent/tests/snapshot_round_trip.rs`
- Modify: `crates/apps/runs/src/lib.rs` (`render_payload` resolution + fail-run on
  pin mismatch; `on_agent_event` recipe teardown on Tombstoned),
  `crates/apps/runs/tests/collaboration_loop.rs`, `crates/apps/runs/tests/snapshot_round_trip.rs`
- Modify (only if `RemoveRecipe` absent): `crates/system/dispatch/src/interface.rs` + `src/lib.rs`
- Modify: `bin/demo/src/main.rs` (demo agent registration), `bin/demo/tests/joiner_rebuilds_global_app_hash.rs`,
  `bin/noded/tests/daemon_e2e.rs`, `bin/node/tests/dispatch_e2e.rs` (fixtures)
- Modify (TS): `app/src/domain/agent-client.ts` (PromptRef shape, register/update
  payloads), `app/src/console/store/actions.ts` (publish prompt to memory path
  `/agents/prompts/<agent_id>` via MemoryMsg::Publish, then register with
  `{module:"memory", target:"<path>@<generation>", renderer:"memory.generation", sha256}`),
  `app/src/console/views/agent/AgentView.tsx` (display)

**Interfaces (Produces):**
```rust
pub struct PromptRef { pub module: String, pub target: String, pub renderer: String, pub sha256: Vec<u8> }
pub const RENDERER_MEMORY_GENERATION: &str = "memory.generation"; // target = "<path>@<generation>"
// AgentRecord.prompt: Option<PromptRef>; AgentStatus::{Active,Paused,Tombstoned}
// AgentMsg::TombstoneAgent { agent_id: String }  (owner-gated, terminal)
// AgentEvent::Tombstoned { agent_id: String }
```
runs resolution: in payload composition, `Some(prompt)` → `ctx.query(prompt.module,
MemoryQuery::Read{path, generation})`; require `Body::Inline`, `sha256(body)==pin`;
prepend content to the payload before `DEFAULT_PROMPT` instructions. Mismatch/missing
→ deterministic failed run (existing fail path + `note()` breadcrumb), not an abort.

- [ ] **Step 1:** Failing agent tests: register with PromptRef validates renderer
  + 32-byte pin + known module; tombstone is owner-gated + terminal (resume
  rejected); canonical encoding round-trips; snapshot tamper suite updated.
- [ ] **Step 2:** Implement agent changes to green (`cargo test -p agent`).
- [ ] **Step 3:** Failing runs tests (collaboration_loop pattern): agent with a
  memory-seeded prompt gets it prepended in the dispatched payload (assert payload
  contains the seeded text); pin mismatch → run fails with breadcrumb, no abort;
  tombstone → recipe removed (dispatch of a new run for that agent rejects).
- [ ] **Step 4:** Implement runs resolution + teardown to green (`cargo test -p runs`).
- [ ] **Step 5:** Fix every fixture site (demo/noded/node tests listed above);
  full `cargo test --workspace`.
- [ ] **Step 6:** TS mirrors + `cd app && bun run typecheck && bun test` (or the
  repo's standard app check). Commit per green cycle.

---

### Task W4: `feat/quack-pages-events` — pages hooks/events + chat hook gating

**Files:**
- Modify: `crates/apps/pages/src/interface.rs` (`PageMsg::RegisterHook{}/UnregisterHook{}`,
  `PageEvent` enum + `encode_page_event/decode_page_event`, `MAX_PAGE_HOOKS: usize = 8`),
  `crates/apps/pages/src/lib.rs` (hook set under reserved `\0hooks` qmdb key;
  post-apply fan-out in `execute` for AddComment/ResolveThread/UpdateText),
  `crates/apps/pages/tests/sync_round_trip.rs` (hooks in root)
- Modify: `crates/apps/chat/src/lib.rs` (`stage_register_hook`: Module origin ⇒
  `module_id == emitter`), `crates/apps/chat/src/interface.rs` (doc comment),
  chat tests (`crates/apps/chat/tests/channel_system.rs`)

**Interfaces (Produces):**
```rust
pub enum PageEvent {
  CommentAdded { page_id: String, target: String, thread_id: String, comment_id: String,
                 author: AuthorRef, text: String },
  ThreadResolved { page_id: String, thread_id: String, resolved: bool },
  BlockUpdated { page_id: String, block_id: String },
}
```
Registration: tagging idiom — subscriber = `Origin::Module` emitter, external
origins rejected, target-known (`ctx.module_root`) not needed (self-registration),
not-self guard vs pages itself, idempotent, cap 8. Delivery: one follow-up `Msg`
per hook post-`apply`, same block.

- [ ] **Step 1:** Failing pages tests: module-origin RegisterHook stores + is
  idempotent + capped; external-origin rejected; AddComment stages one event `Msg`
  per hook with decodable `CommentAdded` carrying author+text; ResolveThread /
  UpdateText likewise; hooks fold into root (round-trip test) and survive
  state-sync.
- [ ] **Step 2:** Implement to green (`cargo test -p pages`).
- [ ] **Step 3:** Failing chat test: module-origin RegisterHook with a foreign
  `module_id` rejected; self-registration and external operator wiring still pass.
- [ ] **Step 4:** Implement chat gate to green (`cargo test -p chat`), confirm
  automations host_integration still green (`cargo test -p automations`).
- [ ] **Step 5:** fmt/clippy/workspace test; commit per green cycle.

---

### Task W5: `feat/quack-open-actions` — RequestedAction + probe/apply routing (after W2+W3 merge)

**Files:**
- Modify: `crates/apps/agent/src/interface.rs` (delete `AgentAction` +
  `KNOWN_ACTIONS` + `ACTION_TASKS_*`; add `RequestedAction`; `AgentResponse.actions:
  Vec<RequestedAction>`; keep `ACTION_CHAT_POST` as the reply-block grant),
  `crates/apps/agent/src/lib.rs` (`validate_actions` shape-only)
- Modify: `crates/apps/runs/src/lib.rs` (constructor gains `package: String`;
  `validate_response` → grant/owner/probe pipeline; `emit_response` → generic
  `PackageActionMsg::Apply`; delete `task_status` inline probes), plus its inline
  tests and `tests/collaboration_loop.rs`
- Modify: `crates/apps/tasks/Cargo.toml` (+dep `package` interface),
  `crates/apps/tasks/src/lib.rs` (implement `PackageActionQuery::Probe` in
  `query_with` + no-fail `Apply` arm in `execute`; failure breadcrumbs)
- Modify: wiring sites constructing `RunsModule::new` (bin/node, bin/noded,
  bin/demo, bin/simnode, runs tests) to pass the package module id
- Test: runs inline fixtures (`AgentAction::` sites) rewritten as `RequestedAction`
  JSON

**Interfaces (Consumes):** `package::interface::{PackageQuery::ActionOwner,
PackageActionQuery, PackageActionReply, PackageActionMsg}` (W2),
shape-only grants (W3). **Produces:**
```rust
pub struct RequestedAction { pub action_id: String, pub tag: String, pub payload: serde_json::Value }
// tasks payloads: {"task_id": String, "title": String} / {"task_id": String, "status": String}
```

- [ ] **Step 1:** Failing tasks tests: Probe rejects missing task / duplicate id /
  bad status value, accepts valid; Apply creates/updates; Apply on late conflict
  (task deleted between probe and apply is impossible in-block, so: duplicate
  create) records breadcrumb + `Ok(())`.
- [ ] **Step 2:** Implement tasks owner to green.
- [ ] **Step 3:** Failing runs tests: response with granted+routed tag probes owner
  and emits Apply (assert staged Msg to tasks decodes as Apply); ungranted tag
  dropped with breadcrumb; unrouted tag dropped; Rejected probe dropped; the
  collaboration_loop end-to-end still creates the task via the new path.
- [ ] **Step 4:** Implement runs pipeline to green; delete dead enum sites
  (`grep -rn "AgentAction" crates/ app/ bin/` must be empty on the Rust side).
- [ ] **Step 5:** TS: `app/src/domain/agent-client.ts` `KNOWN_ACTIONS` becomes the
  builtin route list (`["chat.post","tasks.create","tasks.update_status"]` kept as
  UI defaults); AgentView grant checkboxes render open tags (free-text add).
- [ ] **Step 6:** fmt/clippy/full workspace + app typecheck; commit per green cycle.

---

### Task W6: `feat/quack-docs-harness` — reference package + e2e proof + CLI `package test` (after W1,W2,W4,W5)

**Files:**
- Create: `crates/examples/docs-harness/Cargo.toml`, `src/lib.rs`, `src/interface.rs`,
  `tests/package_loop.rs`, `tests/snapshot_round_trip.rs`
- Modify: `packages/docs/quack.toml` + `packages/docs/harness/golden.json`
  (fixtures aligned with the real wire shapes)
- Modify: `bin/node/src/package.rs` (`package test`: verify + run golden harness via
  a native catalog `fn native_modules(spec) -> Vec<Box<dyn Module>>`), `bin/node/Cargo.toml`
  (+dep docs-harness)
- Modify: root `Cargo.toml` member

**Interfaces (Consumes):** everything above. Behavior per design D9:
`HarnessMsg` arms (install registers `docs.editor` with PromptRef →
`/packages/org.ducktape.docs/prompts/docs-editor.md@<gen>`, `RegisterHook` on pages,
`MarkActive` ack); `PageEvent` intake (origin==pages, no-fail) with
`mention_or_assigned` policy (`@docs.editor` substring in comment text) minting one
idempotent `JobsMsg::Submit{kind:"agent/docs.editor"}`; Probe/Apply for
`pages.comment.add`, `pages.block.update_text` (with `expected_hash` guard),
`pages.thread.resolve` translating to `PageMsg` follow-ups + error rows.

- [ ] **Step 1:** Failing module tests: install arm registers agent + hook + acks
  MarkActive (assert staged msgs); non-package-origin HarnessMsg ignored; comment
  event mentioning the agent mints exactly one job (idempotent on redelivery);
  non-mention is a no-op; suspend pauses agent + stops minting; unplug tombstones
  agent + unregisters hook; Probe validates against pages state; Apply edits the
  block / adds the comment / resolves the thread; malformed Apply records an error
  row and returns Ok.
- [ ] **Step 2:** Implement to green (`cargo test -p docs-harness`).
- [ ] **Step 3:** `tests/package_loop.rs` (Host::genesis with package, memory,
  pages, tagging, saga, capability, dispatch, jobs, agent, runs, tasks, chat,
  docs-harness + canned oracle from collaboration_loop): drive
  `PackageMsg::Install` built from `packages/docs/` via `quack` → assert ADR
  harness checklist: prompts seeded with expected hashes; agent registered
  harness-owned; routes owned by docs-harness; mention → one job; canned oracle
  output with `pages.block.update_text` edits the intended block; unauthorized
  action mutates nothing + records failure; malformed page event no-ops; suspend
  stops new jobs, pages preserved; unplug removes routes/hooks, tombstones agent,
  pages data intact; all module snapshot round-trips reproduce roots.
- [ ] **Step 4:** CLI `package test packages/docs` runs verify + the golden loop
  in-process and prints a pass/fail table; smoke-test in `bin/node` tests.
- [ ] **Step 5:** fmt/clippy/workspace; commit per green cycle.

---

### Task W7: `feat/quack-surface` — status surface + docs (after W2)

**Files:**
- Modify: `bin/noded/src/lib.rs` (`ModuleStatus` + `package: Option<String>`,
  `package_version: Option<String>`, `lifecycle: Option<String>` populated from
  `PackageQuery::List`), `bin/noded/src/main.rs` (status assembly)
- Modify: `app/src/domain/transport.ts` (TS twin), `app/src/console/views/modules/ModulesView.tsx`
  (package column/badge + MODULE_INFO entry for `package`)
- Create: `docs/pages/en/human/architecture/packages.mdx`,
  `docs/pages/ko/human/architecture/packages.mdx`
- Modify: `docs/vocs.config.ts` (sidebar en+ko, after module-model),
  `docs/scripts/check-docs-structure.mjs` if it enumerates pages
- Test: noded status unit test + app typecheck + `cd docs && bun run build` (or the
  repo's docs check)

- [ ] **Step 1:** Failing noded test: status rows for a host with an installed
  package carry package/version/lifecycle; uninstalled modules carry None.
- [ ] **Step 2:** Implement + green.
- [ ] **Step 3:** TS twin + ModulesView badge; app typecheck green.
- [ ] **Step 4:** Write packages.mdx (en, then ko mirror) covering: what a .quack
  is, lifecycle states, action routing, harness contract, v1 native scope; register
  in vocs sidebar; docs build green.
- [ ] **Step 5:** Commit.

---

### Task W8: Integration — merge train, epic verification, PR

- [ ] Merge order into epic: W1 → W2 → W3 → W4 (waves can land as each goes green;
  resolve `bin/node/src/main.rs` dispatch/genesis overlaps at merge) → W5 → W6 → W7.
- [ ] After each merge: `cargo test --workspace` in the epic worktree.
- [ ] Final: fmt/clippy/workspace tests + `bin/demo` joiner test + app typecheck +
  docs build; `/code-review` pass on the epic diff; fix findings.
- [ ] Push `epic/quack-system-base` + open PR against `dev` titled
  "feat(quack)!: packaged-module system base" with the design doc linked and the
  flag-day (app-hash/module-set) callout in the body.
