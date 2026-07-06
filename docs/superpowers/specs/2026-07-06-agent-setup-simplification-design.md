# Agent View Setup Simplification — Design

Date: 2026-07-06 · Branch: `feat/agent-setup-simplify` (from `origin/dev`)

The Agents view (`app/src/console/views/agent/AgentView.tsx`) asks a
first-time user to reason about wire-level concepts before they can register an
agent. Two terms block them outright — **"capability"** and **"Agent ID"** —
and the surrounding surface (watches, run requests, the pending-runs timeline)
speaks in dispatch jargon. Goal: a novice can register a working agent and put
it to work without knowing the registry's vocabulary, while the record the node
stores is unchanged. Light theme only, inline-style + token system, flag-day
(no back-compat shim — the payloads don't change, only the UI producing them).

## Problems (confirmed in code)

1. **"Capability" means two different things on one screen.**
   - A free-text field labelled **Capability** (placeholder `capability tag…`,
     mono input) is the *routing tag* — `"codex"`, `"claude"`, `"ollama"` — that
     selects **which executor/model runs the agent**. Backend truth
     (`crates/apps/agent/src/interface.rs:88`): *"`capability` names WHAT the run
     needs … dispatch selects providers of that tag."*
   - A checkbox group *also* titled **CAPABILITIES** (register form, edit form,
     and the detail panel) is `allowed_actions` — i.e. **permissions**
     (`chat.post`, `tasks.create`, `tasks.update_status`).

   Same word, two unrelated concepts, both on the register form. This is the
   central confusion.

2. **"Agent ID" is a redundant slug in the novice path.** The register form
   surfaces a mono **Agent ID** input, but the code already derives it —
   `slug(agentIdInput || displayName)` (`AgentView.tsx:893`). A first-timer is
   asked to hand-author an identifier they don't need to think about.

3. **The typed tag is a blind guess.** Even relabelled, a free-text executor tag
   is unknowable to the user. But the network already holds the answer: the
   **capability registry** is queryable — `CapabilityQuery::All`
   (`crates/system/capability/src/interface.rs:63`) returns every node's
   announced executor tags, and the `capability` module is registered on the
   daemon (`bin/node/src/main.rs:554`), reachable through the generic
   `transport.query(target, query)`. There is no TS client for it yet.

4. **The rest of the view speaks dispatch.** "Turn policy", "Anchor sequence",
   "PENDING RUNS", front-line `run_id`/`dispatch_id` chips, and the jobs-worker
   subtitle ("opts the agent module into job-board work") all leak internals.

## Changes

### E. Layout overhaul (supersedes the original "relabel in place")

The first pass kept the four-quadrant console (roster │ detail + register on top,
watches + runs below) and only softened its words. That still shows a novice four
dense technical panels at once — the layout itself is the barrier. The view is
now a calm **master–detail with tabs**:

- A top segmented switch: **Agents · Auto-reply · Activity**, each carrying a
  live count, plus a persistent **＋ Add agent** primary button in the header.
- **Agents tab**: the roster on the left; the right pane shows exactly **one**
  thing — the selected agent's detail, *or* the focused Add-agent card (opened
  by ＋ Add agent, closed by Cancel / a successful submit), *or* a single
  call-to-action when there are no agents. Never detail *and* a register form
  competing side by side.
- **Auto-reply** and **Activity** each get their own full-width tab instead of a
  cramped quarter. The jobs-worker switch moves out of the header into its own
  labelled row on Activity, where background work lives.

The sub-panels (detail, register, watches, runs, the "Runs on" picker) are
unchanged components — only the shell that arranges them changed.

### A. Resolve the "capability" collision (the core fix)
- The **CAPABILITIES** checkbox group → **Permissions**, in all three sites
  (register form, edit form, detail panel), introduced as *"What this agent is
  allowed to do."* Option copy reads as abilities: *Reply in chat · Create
  tasks · Update task status* (drop the redundant "Allow" prefix in
  `ACTION_HINT`).
- The free-text **Capability** field → a **"Runs on"** model picker (§B).
- Detail panel `InfoRow label="capability"` → **"Runs on"**.

### B. "Runs on" — registry-backed executor picker
- New `app/src/domain/capability-client.ts` (mirrors `agent-client.ts`):
  `capabilities(transport)` issues `query("capability", "all")`, then flattens
  the `Vec<(node_key, Vec<tag>)>` reply into a **deduped, sorted** tag list.
  Add `capabilities.test.ts` for the query shape + flatten/dedupe/sort.
- Store: load into `state.capabilities: string[]` during `refresh()`, alongside
  agents/channels (`app/src/console/store/actions.ts`). A query failure resolves
  to `[]` — never a thrown error, never a blocked form.
- Picker behaviour by registry size:
  - **0 announced** → fall back to the labelled text field + helper
    (*"name of an executor your node can run, e.g. codex"*). Setup is never
    blocked before an executor announces.
  - **exactly 1** → auto-selected, rendered read-only ("Runs on: **Codex**").
  - **≥2** → a `<select>`, defaulting to the first tag.
- Display: title-case the tag for the label (`codex`→`Codex`); the **raw tag
  stays the value**. No hardcoded model catalogue — the list is whatever the
  network announced (stays data-driven).
- **Offline-tag guard (edit path):** when editing an agent whose stored
  `capability` is absent from the current registry, pin that tag as a selectable
  option ("Codex (offline)") so an edit never silently rewrites which executor
  the agent runs on.

### C. Agent ID — out of the novice path
- Register form: **remove** the visible Agent ID input from the main flow.
  Auto-derive from the display name and show it as a quiet read-only hint
  (*"saved as `triage-agent`"*).
- An **Advanced** disclosure (collapsed by default) holds an editable ID
  override for anyone who wants a specific id. Duplicate/invalid ids still
  surface through the existing finalization mark — no new validation path.
- Detail panel keeps `agent_id` as the de-emphasized subtitle it already is.

### D. Whole-view softening
- **Watches → "Auto-reply channels."** Section title and header pill relabelled
  (`WATCHES` → `AUTO-REPLY`). "Turn policy" → **"When to reply"** with plain
  option labels: `mention`→*When mentioned*, `all`→*Every message*,
  `round_robin`→*Take turns*, `assigned`→*Only a chosen agent*. Empty-state copy
  rewritten in plain language. (`POLICY_LABEL` map + `WatchForm`/`WatchRow`.)
- **Request run → "Ask to respond now."** Default the anchor to the channel's
  latest message (`head_seq`); move **Anchor sequence** into the Advanced
  disclosure. Primary control reads: pick a channel → *Ask [agent] to respond.*
  Keep the existing "no channel messages yet" guard.
- **Pending runs → "In progress."** Section title and header pill relabelled
  (`PENDING RUNS` → `IN PROGRESS`, `PENDING` → `IN PROGRESS`). Lead each row with
  agent name + channel/context + "started …" + a subtle *Working…* state. Raw
  `run_id`/`dispatch_id` move **off the front line into a details affordance**
  (hover tooltip / expandable line) rather than being removed — power users and
  debugging keep them, the default surface stays clean. Keep Cancel.
- **Jobs worker** subtitle *"opts the agent module into job-board work"* →
  *"Let agents pick up background jobs."*

## Non-goals
- No change to any node payload or wire type. `registerAgent`/`updateAgent`
  still send a `capability` tag + `allowed_actions`; only the UI producing them
  changes. No governance/dispatch behaviour change.
- The Add-agent flow is an on-demand pane, not a stepped multi-screen wizard or
  a modal overlay. No curated preset templates ("Triage bot", etc.) this pass —
  presets can layer on later.
- No dark mode (doesn't exist). No new capability *announce* UI — this only
  *reads* the registry to populate the picker.

## Verify
- Typecheck + `make test`. Update `AgentView.test.tsx` for the new labels/roles;
  new `capabilities.test.ts`; store test covering the `capabilities` load and
  the empty-registry text-field fallback. Preserve all `aria-label`s.
- Drive the live app (tauri-debug socket): register an agent end-to-end with a
  registry that has 0 / 1 / ≥2 announced executors; confirm the picker degrades
  correctly, "Permissions" reads cleanly, Agent ID is absent from the default
  flow, and a run request defaults to the latest message. Screenshot before/after.
- Adversarial review workflow on the diff before commit/push.
