# Agent Soul = Curated Skills, Assembled Into the Executor's Native Context File

2026-07-12. Status: approved design, pre-implementation.

## The idea

An agent's "soul" is not a stored object. It is a **build product**: the agent's
curated skill set, assembled at run-provision time into the one file its
executor already auto-loads (`AGENTS.md` for codex, `CLAUDE.md` for claude),
and left on disk where the CLI finds it by its own convention.

This deletes a plane rather than adding one. Today an agent's persona lives as
an opaque blob (`prompt_hash`) in a store whose only remaining job would be
shipping that text, while its skills already live in duckfs and already mount
read-only into every run. Those are the same thing wearing two mechanisms. The
skill mechanism is the better one: content-addressed, snapshot-pinned,
consensus-committed, cross-node materializable — and editable in the Files/Pages
UI, diffable, revertable, reviewable through forge.

So: the persona becomes a skill. `prompt_hash` retires.

## Decisions (settled with the user)

- **Soul = the assembled context document**, not a new stored type. No `DocRef`,
  no new plane, no new consensus object beyond one flag.
- **Delivery is the CLI's own convention.** The assembled doc is written to the
  location the executor auto-loads from, declared as **spec data** — never
  branched on an executor name in Rust (capability-host's standing rule: no
  executor name appears in its code).
- **`SkillRef` gains a `load` mode**: `always` (full text inlined into the
  assembled doc — this is where today's prompt goes) or `on_demand` (name +
  description listed in the doc's index; the agent reads the body from its
  mount when relevant). Curation is per-agent, so the flag rides the agent's
  skill *reference*, not the skill document. Rejected alternative: `load:` in
  the skill's own frontmatter — zero schema change, but then "what does this
  agent always load" is invisible to consensus and to the UI.
- **`prompt_hash` is fully retired** (flag day), not kept as a fallback.

## 1. State — the agent module (FLAG DAY)

```rust
pub struct SkillRef {
    pub name: String,
    pub source_prefix: String,
    pub source_snapshot: Option<String>,
    pub load: LoadMode,          // NEW; default OnDemand
}

pub enum LoadMode { Always, OnDemand }

pub struct AgentRecord {
    // ...
    pub skills: Vec<SkillRef>,
    - pub prompt_hash: Vec<u8>,   // DELETED
}
```

`RegisterAgent` / `UpdateAgent` lose `prompt_hash` and gain the per-skill load
mode. Snapshot/root encoding changes → root-hash moves. This is a deliberate
flag day; existing agents re-register with their persona written into duckfs.
No in-consensus migration op — the repo re-genesises on flag days.

Determinism is unchanged in kind: today consensus commits *which prompt bytes*
ran (a hash); after this it commits *which skill snapshots* ran (pins). Both
are content addresses.

## 2. Assembly — one document, deterministic

At provision time, after the skill mounts materialize, the host assembles ONE
document from the agent's skills **in curation order**:

```markdown
<!-- inlined: every `always` skill, full body, in order -->
# <skill-1 name>
<skill-1 SKILL.md body>

# <skill-2 name>
...

<!-- then the index of `on_demand` skills -->
## Skills available on demand
Read the full text when the task calls for it; each lives under the directory
named by $DUCKTAPE_RUN_SKILLS.
- **<name>** — <description from SKILL.md frontmatter> (`$DUCKTAPE_RUN_SKILLS/<name>/SKILL.md`)
```

The MCP tool-plane instruction (today's `TOOL_PLANE_INSTRUCTION` in the
envelope's runtime section) moves into this document — it is exactly an
always-loaded ambient instruction.

Headings and index entries are keyed by **`SkillRef.name`** — the curated name
consensus committed, never a name read out of the document (a doc must not be
able to rename itself). Descriptions come from each skill's `SKILL.md` YAML
frontmatter (`description`), the convention this repo's own `skills/` already
follows. A skill with no frontmatter degrades to name-only — a cosmetic parse
must never fail a run. A missing or unreadable body for an
**`always`** skill DOES fail the run loudly: that is the agent's persona.

**Placement of the assembly logic** respects the existing reachability wall:
the pure assembler (`Vec<(name, description, body, load)> -> String`) lives in
`dispatch-oracle` and is unit-testable with no filesystem; the node binary's
provisioner — which just materialized the files and is the only layer allowed
to touch the OS-side checkout engine — reads them and calls it. The assembled
string rides back as plain data on `ProvisionedWorkspace`, into
`RunContext.context_doc: Option<String>`.

## 3. Delivery — spec data chooses the door

`CapabilitySpec` gains an optional `[context]` section with a **closed set** of
location kinds (never a raw path — no traversal surface, matching the crate's
posture on `output.format` and `workspace.mode`):

```toml
[context]
# where this CLI auto-loads instructions from.
#   "config-home:<file>"      -> <the run's fresh config home>/<file>
#                                (requires [isolation] config_home_env)
#   "workspace-parent:<file>" -> <parent of the run's checkout>/<file>
path = "config-home:AGENTS.md"     # codex
# path = "workspace-parent:CLAUDE.md"  # claude
```

`capability-host` owns the delivery decision, because the spec is what decides:

- **Spec has `[context]`** → write `ctx.context_doc` to the resolved path before
  spawn. The CLI loads it natively.
- **Spec has no `[context]`** (a raw provider — ollama, any `text`-output CLI)
  → prepend `ctx.context_doc` to the prompt fed on stdin. Today's semantics,
  preserved, with no second rule: one assembly, two doors, chosen by data.

**Both locations sit outside the commit scan** — the property that makes this
safe:

- `config-home:` lives under `<workdir>/.ducktape-run/<slot>/`, which the
  provisioner already deletes before duckfs/forge snapshot scans (the reserved
  runtime dir from the credential-broker work).
- `workspace-parent:` sits beside the checkout, and `commit` scans only *under*
  the checkout dir — the same guarantee the `-ro` skill sibling already relies
  on.

So the assembled soul never lands in the agent's output snapshot or PR, and it
never collides with a repository's own `AGENTS.md`/`CLAUDE.md` — those stay
inside the checkout and **layer on top** of the soul (both CLIs merge
parent-directory instructions with project ones). Overwriting a repo's
instructions was the trap this placement avoids.

**Sandbox interaction:** under Podman/Tart the container mounts the workdir at
its identical path, so a `config-home:` doc is visible for free. A
`workspace-parent:` doc is *outside* that mount, so the sandbox wrapper must
bind it read-only at its identical path — one more entry in the existing
`ro_paths` list.

## 4. Envelope — what's left

`RunEnvelope` loses `prompt_hash` and loses the runtime section (both moved
into the assembled doc). It keeps context, the output contract, and the
conversation. The generic `instructions` fallback survives for an agent with
no `always` skill at all.

This structurally kills a known trap: today `instructions` is silently dropped
whenever `prompt_hash` is `Some`. After this there is no `prompt_hash`, and the
fallback is reached by the absence of always-skills — a state you can see.

## 5. What gets deleted

- `prompt_hash` on the agent record, its registration/update arguments, and its
  envelope field.
- The blob-store *prompt* path: the app's prompt upload, the host's
  hash→blob→text resolution, and the prompt-specific mesh fetch-on-miss. duckfs
  already does content-addressed, consensus-replicated, cross-node
  materialization — proven daily by the skill mounts. The blob plane itself
  stays (it still carries replies and artifacts); prompts leave it.
- The envelope's `runtime_section` skill-listing text.

## 6. What it unlocks (not built here)

The soul's inputs are ordinary duckfs documents, so they are already editable in
Files/Pages, versioned, diffable, revertable, and PR-able through forge — an
agent's personality change can be *reviewed*. And because runs write to duckfs,
an agent proposing an edit to its own soul is a door this opens rather than a
feature this builds. YAGNI: v1 mounts the soul read-only.

## 7. Error handling

- Skill mount materialization failure: fails the run (existing W1 behavior).
- `always` skill body missing/unreadable: fails the run loudly (it is the
  persona; running without it would silently produce a different agent).
- Frontmatter missing/malformed on an `on_demand` skill: degrade to name-only in
  the index, log, continue.
- `[context]` declares `config-home:` but the spec has no `[isolation]
  config_home_env`: hard **load** error (spec-time, not run-time).
- Assembled doc cannot be written to its path: fails the run loudly — never a
  silent unsouled agent.

## 8. Testing

- Pure assembler: inline order follows curation order; `on_demand` skills appear
  only as index entries; frontmatter-less skill degrades to name-only; the
  tool-plane instruction is present exactly once.
- capability-host delivery: `[context]` spec writes the doc at the resolved
  path; a spec without `[context]` prepends it to the stdin prompt; a
  `workspace-parent:` doc appears as a read-only mount in the podman argv;
  `config-home:` without `config_home_env` fails at spec load.
- agent module: `SkillRef.load` round-trips through snapshot/install; the root
  moves when a load mode changes.
- e2e: an agent with one `always` skill and one `on_demand` skill runs against a
  real node; the persona reaches the model and the on-demand skill does not
  inflate the prompt.

## 9. Open items (implementation-time)

- Confirm on the box that codex reads `$CODEX_HOME/AGENTS.md` as global
  instructions and that claude merges a parent-directory `CLAUDE.md` with a
  project one. Both are the load-bearing conventions; if either is wrong, that
  executor's `[context]` kind changes — the design survives, the spec data
  changes.
- Decide the app's agent-edit surface: persona editing becomes "edit this
  document in Files", so the agent form links to the duckfs doc instead of
  holding a textarea. Scope it in the plan.
