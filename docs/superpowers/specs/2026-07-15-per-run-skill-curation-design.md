# Per-run skill curation — design

Date: 2026-07-15
Status: approved design, implemented
Related: PR #466 (agent soul = assembled context); the bounded-delegation
feature already on `dev` (`feat(runs): add bounded agent delegation waves`).

## The problem

The skill library exists. Curation does not reach the point of *assigning work*.

- The library is `/shared/skills/<name>/SKILL.md` on duckfs
  (`SKILL_LIBRARY_PREFIX`, `crates/apps/agent/src/interface.rs`).
- An agent's skills are curated **once, on the agent record**
  (`AgentRecord.skills`, on-chain, ≤64). Every run of that agent resolves the
  same set (`resolve_skills`, `crates/apps/runs/src/envelope.rs`).
- The two ways work is *assigned* to an agent — an operator's/orchestrator's
  explicit `RunsMsg::RequestRun`, and a parent agent's `DelegationRequest`
  (dev's delegation wave) — both compose the target's own curation and nothing
  task-specific.

So a skill an agent needs **for one task** had only two homes: permanently on
the agent record (bloating every unrelated run), or nowhere. The task-shaped
tier — *this assignment needs these skills* — was missing from both assignment
paths.

## What already exists (and is reused, not rebuilt)

- The wire is per-run already: `RunEnvelope.skills` → `RoMount` →
  `checkout_ro_mounts` → `assemble_context_doc` never knew the array came from
  the agent record. The host, sandbox, ro-mounts, assembler, and caps model
  change by **zero lines**.
- The three-tier context budget holds: `Always` inlines (64 KiB budget, over it
  the run fails loudly), `OnDemand` costs one index line, the rest of the
  library costs nothing until grepped.
- Dev's delegation (`DelegationRequest { agent_id, instruction }`, gated by
  `caps.subagent_budget`, non-escalating, one level deep) owns *agent-to-agent
  delegation*. This design does not add a competing delegation mechanism; it
  adds the missing skill-curation to the assignment paths that already exist.

## Design

Both assignment surfaces grow the same field — a list of **library skill
NAMES** — and both resolve it the same way.

### 1. `RunsMsg::RequestRun.skills: Vec<String>`

The operator/programmatic assignment path. `skills` is the explicit-request
surface only; the mention, page, forge, and jobs intakes have no requester to
choose skills and pass none. (This mirrors the existing per-request `demands`
field, which only this intake carries.)

### 2. `DelegationRequest.skills: Vec<String>`

The agent-to-agent path. A parent delegating a wave names library skills for
each child, on top of that child's own curation. The model authors this in its
final response, so the strict-output contract advertises the shape.

### 3. Names, not refs — the trust boundary

Both fields are **names**, expanded host-in-consensus by `library_skills`
(`crates/apps/runs/src/envelope.rs`) to
`SkillRef { name, source_prefix: "/shared/skills/<name>", source_snapshot: None,
load: OnDemand }`.

This is deliberate and load-bearing. The ro-mount that materializes a skill
(`checkout_ro_mounts`, `bin/noded/src/agent_provision.rs`) runs on the **node's
duckfs authority with no read-cap gate**, and an `Always` body is inlined at the
top of the target agent's context document. If a requester could name an
arbitrary `source_prefix` with `load: Always`, they could read **any duckfs
subtree** — another agent's private persona, a workspace — into the target's
persona, paired with injected exfiltration instructions.

Taking names instead of refs closes that by construction:

- **Library-confined**: the name is canonicalized as the last segment of the
  library prefix. `library_skills` requires the result to be exactly
  `["shared", "skills", <name>]` (three segments), so a name carrying a `/` or
  `..` lands at a different depth (or fails to canonicalize) and is refused. The
  duckfs canonicalizer already rejects `..`/`.`/empty/non-absolute.
- **On-demand, never inlined**: a requester offers a library skill; only an
  owner's own record can `Always`-inline a persona.
- **No pinned snapshot**: a requester tracks the committed head like any run.

### 4. `resolve_skills(agent, extra, head)` — additive union

The agent's own skills lead (persona assembles first, in `Always`-inline order),
and `extra` appends only the names the agent does not already carry. A task
**supplements** a persona; it never overrides or downgrades one. A name curated
on both sides keeps the agent's own ref.

Threaded through `portable_inputs` / `prepare_dispatch` /
`prepare_dispatch_with_context` (`extra: &[SkillRef]`); every non-requester
intake passes `&[]`.

### 5. Skills are not part of a run's identity

`run_id = (channel, anchor, agent)`, deduped by the turn claim. So a second
`RequestRun` at one anchor with a *different* curation is the same turn and
no-ops; a delegated child's turn is claimed the same way. Re-running an agent
with a different skill set means a new anchor. No cycle guard, no new state.

### 6. Library discoverability — one word

`SKILL_LIBRARY_SECTION` (`dispatch-oracle/src/soul.rs`) named `files_grep` and
`files_read` but omitted `ducktape_files_ls`, the only way to *enumerate* the
library (there is no index file). An agent told only to grep can find a skill it
can already describe and nothing else. `ls` is now named first. Host-only change
(the assembler runs on the host); it changes the delivered `AGENTS.md` prose,
not any consensus hash.

## Explicitly not built

- **A second delegation mechanism.** Dev's `DelegationRequest` owns
  agent-to-agent delegation. An earlier draft of this work added a session-lane
  `AgentAction::RequestRun`; it was dropped as redundant and contradictory once
  dev shipped the delegation wave — this design grafts skills onto dev's
  mechanism instead.
- **Raw `SkillRef` / arbitrary paths / requester-forced `Always`** on either
  assignment path. Names only (§3).
- **Skill fields on the mention, page, forge, and jobs intakes.**
- **A per-skill capability grant.** `ResourceCaps` live on the agent record and
  are untouched by an assignment; a delegated child runs with the *target*
  agent's caps, never wider. Confinement to the library (§3) is the boundary.

## Testing

- `resolve_skills`: additive union, order, collision keeps the agent's ref.
- `library_skills`: confines names, refuses traversal/slash/empty/dup.
- Operator `RequestRun.skills`: composed onto the agent's own; non-library name
  refused; not part of run identity (second request no-ops).
- `DelegationRequest.skills`: curated onto the child; non-library name fails the
  wave before any child stages.
- Empty `skills` composes byte-identically to before (regression pins via the
  existing envelope/snapshot tests).
- Wasm parity (`wasm_runs_parity`): a non-empty curated skill crosses the wasm
  boundary and both runtimes compose identical child-envelope bytes.

## Gates

- `cargo clippy -p {runs,agent,dispatch-oracle,mcp-bin} --tests --no-deps`.
- `cargo test -p runs -p agent -p dispatch-oracle -p mcp-bin -p simnode`.
- `cargo test -p host --test wasm_runs_parity --test wasm_agent_parity`.
- `make wasm-modules-check` (runs + agent module bytes rebuilt).

## Flag day

`runs` and `agent` are consensus wasm modules; their hashes move. Ships through
the height-gated upgrade path or a re-seed on a dev network. No compat shims, no
version tags.
