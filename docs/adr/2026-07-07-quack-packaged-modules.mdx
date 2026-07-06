# ADR: Quack Packaged Modules

Date: 2026-07-07. Status: **Accepted as package standard, implementation
not scheduled**. This fixes the product/package boundary before Wasm module
loading, package install, and package uninstall work begins.

## Context

Ducktape modules are moving toward Wasm. That solves the portable execution
artifact, but it does not solve the product packaging problem by itself. A
usable product module often needs more than deterministic code:

- module interface metadata;
- initial consensus state;
- prompts and skill instructions;
- agent registrations;
- action permissions and action schemas;
- UI surface metadata or bundles;
- hook wiring and harness modules;
- install, suspend, unplug, and audit behavior.

The concrete example is Docs. Installing the Docs module should not mean only
"load `pages.wasm`". A real Docs package should also register `docs.editor`,
`docs.brainstorm`, and `docs.triage`; seed their prompt pins; wire comment
events into a harness; define what page/comment actions those agents may
request; and prove the package with deterministic fixtures.

Current architecture also constrains the shape:

- Agent records must exist before agents can be run. Agent registration is
  therefore part of package activation, not an optional after-step.
- `agent` is the registry; `runs` and `dispatch` own execution lifecycle.
- Capability specs are host-local operator config. A package can require a
  tag such as `codex`, but must not install binaries, credentials, or provider
  specs.
- Plug-out cannot mean instantly deleting native code from a running network.
  It must first disable consensus entry points and package-owned state. Actual
  code removal comes later through binary/Wasm availability policy.

## Decision

The Ducktape package format is named **Quack**. A `.quack` file is a signed
Ducktape app capsule that ships Wasm modules, agents, skills, UI, lifecycle
rules, and verification harnesses across networks.

Wasm is the deterministic module artifact inside the package. Quack is the
product distribution unit around it.

```text
docs.quack
|-- quack.toml
|-- modules/
|   `-- pages.wasm
|-- wit/
|   `-- pages.wit
|-- prompts/
|   |-- docs-editor.md
|   |-- docs-brainstorm.md
|   `-- docs-triage.md
|-- agents/
|   `-- docs-agents.toml
|-- actions/
|   |-- pages.comment.add.schema.json
|   |-- pages.block.update_text.schema.json
|   `-- pages.thread.resolve.schema.json
|-- ui/
|   `-- docs.bundle
|-- harness/
|   |-- fixtures/
|   `-- golden.json
`-- signatures/
    `-- package.sig
```

The manifest is the authoritative contract. Package-local ids are logical and
must be mapped to actual module ids during install.

```toml
schema = 1
package = "org.ducktape.docs"
version = "0.1.0"

[[modules]]
logical = "pages"
default_id = "pages"
artifact = "modules/pages.wasm"
abi = "wit/pages.wit"
hash = "sha256:..."

[[prompts]]
logical = "docs_editor_prompt"
path = "prompts/docs-editor.md"
hash = "sha256:..."

[[agents]]
id = "docs.editor"
display_name = "Docs Editor"
prompt = "docs_editor_prompt"
capability = "codex"
actions = ["pages.comment.add", "pages.block.update_text"]
status = "active"

[install]
register_modules = true
seed_state = true
register_agents = true
register_actions = true
wire_hooks = true
enable_jobs = true
run_harness = true

[uninstall]
remove_hooks = true
pause_agents = true
unregister_actions = true
pending_runs = "drain"
user_data = "preserve"
package_state = "tombstone"
```

## Lifecycle Standard

Every Quack package has an explicit lifecycle:

```text
Available -> Installing -> Active -> Suspended -> Unplugging -> Inactive
```

**Install** performs, atomically where consensus requires:

1. verify signature, hashes, ABI, and manifest shape;
2. map logical ids to network module ids;
3. register or activate package modules;
4. seed package-owned consensus state, including prompt refs;
5. register package-owned agents in the `agent` module;
6. register action routes and schemas;
7. wire hooks and harness subscriptions;
8. enable required execution paths, such as `runs` as a jobs worker;
9. run package harness tests before activation where possible.

**Active** means all required module entry points, hooks, agents, and action
routes are present. A storage/UI-only install is not an active Quack package if
its required agents were not registered.

**Suspend** disables new package activity without deleting state:

- remove or disable hooks;
- pause package-owned agents;
- stop creating new jobs or dispatches;
- keep user data and package audit state readable.

**Unplug** removes runtime entry points:

- unregister hooks;
- pause or tombstone package-owned agents;
- unregister package action routes;
- drain or cancel pending runs according to manifest policy;
- preserve user-created product data by default;
- tombstone package-owned config and prompt state for audit.

**Inactive** means no new package work can start. Code artifacts may still be
available to the runtime until a later binary/Wasm artifact cleanup removes
them.

## Agent and Harness Rule

Quack packages may include dedicated agents, but package-specific behavior must
live in a deterministic harness module, not in a host-local executor.

For Docs:

1. `pages` emits comment/page events to `docs-harness`.
2. `docs-harness` decides which package agent should run and submits an
   `agent/{id}` job.
3. `runs` and `dispatch` execute the agent through the normal agent lifecycle.
4. The model output is normalized into requested actions.
5. `runs` validates grants and routes package actions back to the owner
   harness.
6. `docs-harness` probes and applies safe `PageMsg` follow-ups.

The harness owns Docs semantics. `runs` remains the execution adapter, and
`agent` remains the registry.

## Sharing Standard

Fresh networks can install a Quack package at genesis. Existing networks need a
coordinated upgrade path if the package introduces new module code or ABI:

1. operators audit the `.quack` manifest and hashes;
2. the Wasm/runtime or node binary supports the package artifact;
3. governance schedules activation;
4. package install seeds state, agents, hooks, and action routes;
5. the package becomes active only if the whole install succeeds.

Package export has two modes:

- **template export:** code, UI, prompts, action schemas, agent definitions,
  lifecycle rules, and harness tests;
- **stateful export:** template plus selected committed state snapshots,
  explicitly accepted by the recipient.

Host-local provider specs, API keys, credentials, uncommitted index data, and
unverified blob bodies are never part of a Quack export.

## Consequences

- Quack is intentionally not "just Wasm". It is the portable app/product unit.
- Agents are first-class package lifecycle resources. A package that requires
  agents must register, pause, and unplug them explicitly.
- Plug-out is a reversible consensus-state operation by default. Destructive
  user-data deletion must be a separate explicit policy.
- Capability execution remains local operator opt-in. Packages can declare
  required tags but cannot smuggle executors or credentials into a network.
- Package harnesses become the safety boundary for module-specific actions.
  The LLM requests actions as data; harness modules validate and translate them
  into real module writes.
- The package manager must be able to inspect, test, install, suspend, unplug,
  and audit packages. A plain artifact loader is insufficient.

## Non-Decisions

- The exact compression/container format is not fixed here. `.quack` may be a
  zip, tar, or content-addressed bundle as long as the manifest and hashes are
  canonical.
- The UI bundle loading model is not fixed. Early builds may keep package UI in
  the node/app binary while still using Quack for the consensus package
  standard.
- The exact Wasm ABI is not fixed here. This ADR only requires that the ABI hash
  be package-visible and verified before activation.
