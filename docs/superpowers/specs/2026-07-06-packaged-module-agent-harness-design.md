# Packaged Modules with Agent Skillsets and Harnesses

**Status:** proposed design
**Date:** 2026-07-06
**Surface:** package manifest/CLI, module registry/upgrades, `agent`, `runs`,
`jobs`, `pages`, package-owned harness modules, docs distribution

## Goal

Make a product module shareable as a package, not just as Rust source. A package
should carry the module code, interface contract, UI surface, seed data,
dedicated agents, their prompt skillset, allowed action vocabulary, and a
deterministic harness proving the whole thing works.

Example: the Docs package installs `pages`, a Docs UI, and dedicated agents:

- `docs.editor` edits blocks and comments with a summary.
- `docs.brainstorm` suggests alternatives on page or block comments.
- `docs.triage` watches unresolved threads and resolves or routes them.

When another Ducktape network imports that package, it should be able to audit
what will be added, map module ids and executor tags, run the harness locally,
then activate it through genesis or a coordinated upgrade.

## Current Constraints

The existing architecture gives us useful pieces, but not the full package
boundary yet.

- Modules are native Rust today. There is no runtime load/install API; every
  registered genesis module is active. A live network needs a binary upgrade
  before it can activate new module code.
- Capability specs are host-local trusted config. They are never fetched from a
  network package. A package may require tags such as `codex` or
  `docs-large`, but each operator still decides which local binaries/specs to
  run and announce.
- The `agent` module is only the registry. It records `agent_id`,
  `capability`, prompt pin, grants, and status. Execution lives in `runs` /
  `dispatch`.
- `runs` currently knows chat and jobs. It can run `agent/{id}` jobs, but job
  runs currently emit actions only from the fixed `AgentAction` vocabulary.
- `pages` now stores comments, but it does not yet emit hook events when
  comments or block edits happen.
- `agent.prompt_doc` is fixed to the old `document` module. A portable package
  wants prompt content in package-owned consensus state, such as `memory`,
  `pages`, or a package seed module, with a hash pin.

## Package Shape

A Ducktape module package is an archive plus a signed manifest. The archive is
data until a network operator/governance process chooses to install it.

```
docs.dpkg/
  ducktape.package.toml
  modules/
    pages/...
    docs-harness/...
  interfaces/
    pages-interface.hash
    docs-harness-interface.hash
  ui/
    module.json
    dist/...
  prompts/
    editor.md
    brainstorm.md
    triage.md
  actions/
    pages.comment.add.schema.json
    pages.block.update_text.schema.json
    pages.thread.resolve.schema.json
  harness/
    fixtures/
    fake-provider.toml
    golden-runs.json
  signatures/
    package.sig
```

The manifest is the stable contract:

```toml
schema = 1
package = "org.ducktape.docs"
version = "0.1.0"
description = "Docs module, comments, UI, and dedicated docs agents"

[requires]
protocol_min = 1
modules = ["agent", "runs", "jobs", "dispatch", "capability", "memory"]
capabilities = ["codex"]

[[modules]]
logical = "pages"
default_id = "pages"
kind = "consensus"
state = "qmdb"
interface_hash = "sha256:..."

[[modules]]
logical = "docs-harness"
default_id = "docs-harness"
kind = "consensus"
depends = ["pages", "agent", "runs", "jobs"]
interface_hash = "sha256:..."

[[prompts]]
logical = "editor_prompt"
path = "prompts/editor.md"
hash = "sha256:..."

[[actions]]
tag = "pages.comment.add"
owner = "docs-harness"
schema = "actions/pages.comment.add.schema.json"

[[actions]]
tag = "pages.block.update_text"
owner = "docs-harness"
schema = "actions/pages.block.update_text.schema.json"

[[agents]]
agent_id = "docs.editor"
display_name = "Docs Editor"
capability = "codex"
prompt = "editor_prompt"
actions = ["pages.comment.add", "pages.block.update_text"]
status = "active"

[[agents]]
agent_id = "docs.brainstorm"
display_name = "Docs Brainstorm"
capability = "codex"
prompt = "brainstorm_prompt"
actions = ["pages.comment.add"]
status = "active"

[[engagements]]
source = "pages"
event = "comment_added"
agent = "docs.editor"
policy = "mention_or_assigned"
```

Module ids in the package are logical. The installing network maps them to real
genesis ids. That avoids collisions when a network already has `pages` or wants
`team-docs`. Every manifest reference goes through that id map.

## Agent Skillset

A packaged agent is not just an `AgentRecord`. It is a skillset:

- prompt content, hash-pinned in consensus state;
- executor requirement, expressed as an open capability tag;
- allowed action tags;
- engagement rules that say when the package harness should run it;
- harness fixtures and golden tests.

The agent registry should move from the current fixed prompt/action shape to:

```rust
pub struct PromptRef {
    pub module: String,
    pub target: String,
    pub renderer: String,
    pub sha256: Vec<u8>,
}

pub struct AgentRecord {
    pub agent_id: String,
    pub owner: SagaOrigin,
    pub display_name: String,
    pub capability: String,
    pub prompt: Option<PromptRef>,
    pub allowed_actions: Vec<String>,
    pub status: AgentStatus,
    pub created_at: u64,
    pub updated_at: u64,
}
```

`allowed_actions` should validate only the common tag shape
(`[a-z0-9._-]`, <= 64 bytes), not a hardcoded platform list. Built-in actions
such as task creation become action specs owned by platform modules. Package
actions become action specs owned by package harness modules.

Model output should likewise become open:

```rust
pub struct AgentResponse {
    pub reply_blocks: Vec<ReplyBlock>,
    pub actions: Vec<RequestedAction>,
}

pub struct RequestedAction {
    pub action_id: String,
    pub tag: String,
    pub payload: serde_json::Value,
}
```

The response is still data until the deterministic validator accepts it.

## Package Harness

Each serious package gets a small consensus module called a harness. The harness
is the package's deterministic adapter between product events, agent runs, and
module-specific writes.

For Docs, `docs-harness` owns:

- package installation state: package version, module id map, enabled agents;
- action specs for pages/comment edits;
- engagement rules for comments and page events;
- idempotency keys for jobs it has submitted;
- no long-running LLM lifecycle state.

The harness owns the package agents by registering them from module origin. That
means later package upgrades can pause, resume, or retune those agents without
pretending to be a human operator.

## Event Flow for Docs Agents

Pages needs a hook/event surface:

```rust
pub enum PagesMsg {
    RegisterHook { module_id: String },
    UnregisterHook { module_id: String },
    // existing page/comment ops...
}

pub enum PagesEvent {
    CommentAdded { thread_id: String, comment_id: String, target: String },
    ThreadResolved { thread_id: String, resolved: bool },
    BlockUpdated { block_id: String, page_id: String },
}
```

`pages` emits one follow-up to each registered hook in the same block as the
content write. The event arm is no-fail for consumers: malformed or irrelevant
events become no-ops plus observability, never an abort of the user's edit.

Docs agent flow:

1. A user comments on a block and mentions `docs.editor`.
2. `pages` stores the comment and emits `PagesEvent::CommentAdded` to
   `docs-harness` in the same block.
3. `docs-harness` reads the page, block, thread, and package config through
   `Ctx::query`, builds a canonical job spec, and emits:
   `JobsMsg::Submit { kind: "agent/docs.editor", spec }`.
4. `jobs` notifies registered workers. `runs`, already registered as the agent
   worker, claims the job and dispatches `agent/docs.editor`.
5. The provider returns raw text. `runs` normalizes it into `AgentResponse`.
6. `runs` validates grants and action tags, probes the owner harness for each
   package action, and emits `docs-harness::ApplyAction` follow-ups only for
   valid actions.
7. `docs-harness` applies page writes as module-origin follow-ups to `pages`,
   such as `UpdateText`, `AddComment`, or `ResolveThread`.
8. The job finalizes with a compact result summary. The dispatch module keeps
   the run history.

This keeps run lifecycle in `dispatch`, agent correlation in `runs`, and
Docs-specific semantics in `docs-harness`.

## Action Routing

Package actions need a deterministic router. A minimal shape:

```rust
pub enum ActionQuery {
    Probe {
        action_id: String,
        tag: String,
        payload: Vec<u8>,
        run_context: Vec<u8>,
    },
}

pub enum ActionReply {
    Accepted,
    Rejected { reason: String },
}

pub enum ActionMsg {
    Apply {
        action_id: String,
        tag: String,
        payload: Vec<u8>,
        run_context: Vec<u8>,
    },
}
```

The package manifest maps `tag -> owner module`. Before `runs` emits an action,
it queries the owner module with `Probe`. The owner validates schema, target
existence, size caps, authorship rules, and whether the action is still safe.
Only accepted actions are emitted. `Apply` should be no-fail: if something still
cannot be applied, the harness records/reports the action failure without
poisoning the delivery block.

For Docs:

- `pages.comment.add`: payload `{target, thread_id?, text}`.
- `pages.block.update_text`: payload `{block_id, expected_hash?, text}`.
- `pages.block.insert_after`: payload `{parent, after?, kind, text}`.
- `pages.thread.resolve`: payload `{thread_id, resolved}`.

The harness, not the LLM and not `runs`, translates these into real `PageMsg`
follow-ups.

## Sharing to Another Network

There are two install modes.

### Fresh Network / New Genesis

This is the practical path with the current native module boundary.

1. Recipient runs `duck package inspect docs.dpkg`.
2. Recipient maps logical module ids to genesis ids.
3. Recipient chooses executor tag mappings, for example `codex -> codex` or
   `codex -> internal-codex-large`.
4. Operators install any local capability specs they trust. The package never
   installs executor specs automatically.
5. Recipient runs the package harness against an in-memory host and fake
   provider.
6. Genesis includes the package modules and seed ops:
   - create prompt records;
   - register action specs;
   - register package agents from `docs-harness` origin;
   - register `docs-harness` as a pages hook;
   - enable `runs` as a jobs worker.

The network starts with the package active and with a deterministic initial
root.

### Existing Network / Coordinated Upgrade

A live network cannot safely import new native module code by downloading a
package. It must first upgrade to a binary that contains the package code.

1. Package is audited and built into the next node binary.
2. Governance schedules a protocol upgrade at height `H`.
3. At `H`, the binary activates the new module code path.
4. A `PackageInstall` governance op registers module ids, seeds prompt state,
   registers action specs, registers agents, and wires hooks.
5. The install op commits or aborts atomically. If any package-owned agent
   recipe cannot register, no partial package lands.

The package manifest hash and code/interface hashes should be part of the
proposal so every operator can verify they are activating the same artifact.

## Exporting a Package

There are two export flavors:

- **Template export:** module code, UI, empty initial state, prompt skillsets,
  action schemas, and harness tests. This is the default shareable package.
- **Stateful export:** template plus selected committed state snapshots, such as
  pages content or prompt documents. The exported state is anchored by module
  roots and installed only if the recipient explicitly accepts importing that
  data.

Stateful export must never include host-local provider specs, credentials,
files chunk bodies outside their verified manifests, or uncommitted index data.

## Security Rules

- A package is never allowed to install or fetch host executors. Capability
  specs stay local and operator-trusted.
- Package prompts are consensus data and hash-pinned. A run fails or skips if
  the prompt source does not hash to the registered pin.
- Package agents are owned by the package harness or by the installing
  governance authority, never by an arbitrary external key minted by the
  package author.
- Package action tags are namespaced and owner-routed. An agent can request
  `pages.block.update_text`, but only the owner harness can validate and turn it
  into a pages write.
- Hooks and action apply arms are no-fail where they ride another module's
  block. Bad package data should fail the run or action, not wedge the network.
- Module id mapping is explicit. A package cannot silently squat `pages`,
  `agent`, or any existing module id.

## Harness Requirements

Every package should ship deterministic tests that a recipient can run before
activation:

- install seeds prompts and registers agents with the expected hashes;
- package action specs register under the expected owner module;
- comment mentioning `docs.editor` creates exactly one `agent/docs.editor` job;
- fake provider output with `pages.block.update_text` edits the intended block;
- fake provider output with an unauthorized action is rejected and mutates
  nothing;
- malformed page events and bad action payloads are no-op/failure records, not
  block aborts;
- state-sync/snapshot round trips reproduce the package modules' roots;
- capability absence leaves jobs pending or skipped, never corrupts state.

The fake provider is a local capability spec in the harness directory. It is
used only for tests and is not installed into a real node automatically.

## Implementation Slices

1. Define package manifest parsing and `duck package inspect/test` tooling.
2. Generalize `AgentRecord` prompt pins to `PromptRef`.
3. Replace the fixed `AgentAction` enum with open `RequestedAction` tags while
   re-expressing built-in task actions as action specs.
4. Add action owner routing/probe/apply between `runs` and harness modules.
5. Add hook events to `pages`.
6. Build `docs-harness` with install state, event handling, action validation,
   and agent registration.
7. Add package install support for fresh genesis.
8. Add governance/upgrade install support for live networks.
9. Package the Docs UI bundle and expose package metadata in the Modules view.

## Open Questions

- Should action specs live in a new `actions` system module, or should the
  package manifest be the only tag-to-owner registry after install?
- Should package UI be native to the node binary for now, or should the console
  eventually support signed UI bundles?
- Should prompt content standardize on `memory` paths, `pages`, or a dedicated
  package seed module?
- Should a package harness be able to submit direct dispatches, or should
  package-triggered work always flow through jobs for operator visibility?

The conservative v1 answer is: native UI in the binary, prompts in `memory`,
package-triggered work through jobs, and action ownership stored in package
install state.
