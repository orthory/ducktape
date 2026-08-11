# Agent UX overhaul — mentions, provider picker, management view

Date: 2026-07-08 · Branch: `feat/agent-ux` · Status: approved-by-goal (autonomous build)

## Context

Three UX failures in the agent flow:

1. **Mentioning an agent in chat does nothing.** The full consensus pipeline
   (chat `collect_mentions` → tagging plane `TagEvent`/`EngagementEvent` → runs
   `TurnPolicy::Mention` → dispatch → oracle worker → threaded reply with
   `as_agent`) is built and proven by `simnode.scenario.test.tsx`. Two gaps,
   both frontend: the composer never emits a structured `Mark::Mention`
   (`chat-input.ts` leaves `@name` as plain text), and nothing creates the
   per-channel `mention`-policy watch that engagement routing requires.
2. **Provider selection is a free-text field** (`RunsOnField`), no way to pick
   model or reasoning effort. Consensus deliberately stores only an open-set
   `capability` tag; model/flags are host policy in per-tag TOML specs
   (`crates/kernel/capability-host/specs/`). Structured model/effort fields on
   `AgentRecord` would be consensus-breaking and re-open the removed
   "model routing" design — rejected.
3. **"Ask to respond" lives in the Agents management page** and asks for a
   channel + raw message sequence number. The affordance belongs on the
   message in chat.

## Non-goals

- No consensus/wire changes anywhere (verified: all work is frontend + the
  host-local capability-host spec loader + spec TOMLs).
- No dispatch-time model routing (`[models]`/`{model}` stays dead; each tag
  keeps a fixed argv).
- Human-user @mentions in the typeahead: follow-up, not this build (only
  `AuthorRef::Agent` mentions drive engagement; renderer already handles the
  rest).
- Live-progress streaming of runs into chat (user explicitly deferred it).

## WS-A — Mention → response (chat frontend)

- **Typeahead**: `@` in the composer opens a listbox popover of Active agents
  (filter on agent_id + display_name), modeled on the Pages `SlashMenu`
  (`PagesView.tsx:104-175`): `role="listbox"`, activeIndex keyboard nav
  (Up/Down/Enter/Escape/Tab), mousedown-to-pick. Picking inserts `@<agent_id> `.
- **Parse**: `parseMessageInput` gains a resolver (map of `agent_id →
  AuthorRef`). Tokens `@([a-z0-9._-]+)` whose capture resolves become spans
  with `{ mention: { agent: { module: "runs", agent_id } } }` — `module` MUST
  be `"runs"` (runs rejects tags where `tag.module != self.id`). Unknown
  `@tokens` stay plain text. `blocksToInput` maps the mark back to
  `@agent_id`.
- **Auto-watch**: in `actions.sendMessage`, when the parsed blocks mention ≥1
  agent and `state.watches` has no watch for the channel, submit
  `watchChannel(channelId, "mention")` and await the ack before posting. An
  existing watch of any policy is respected, never overwritten.
- **Per-message "ask to respond"**: hover action on a message → small popover
  of Active agents → `actions.requestRun(agentId, channelId, anchorSeq =
  that message's seq)`. This replaces the management-page form with the same
  RPC, anchored the way users actually think ("respond to this").

New files: `app/src/console/views/chat/mention.ts` (token tracking, resolver,
insertion), `app/src/console/views/chat/MentionMenu.tsx` (popover),
`app/src/console/views/chat/AskAgentButton.tsx` (hover popover). Edits:
`Composer.tsx`, `chat-input.ts`, `MessageItem.tsx`, `store/actions.ts`.

## WS-B — Provider / model / effort (capability-host + specs)

**Tag grammar (the contract the UI and specs share):**

- Base tags stay: `codex`, `claude` (provider default argv).
- Variant tags: `{provider}_{model}_{effort}`, split on `_`; model and effort
  therefore must not contain `_` (charset `[a-z0-9.-]`). Examples:
  `codex_gpt-5.5_xhigh`, `claude_opus_max`. Any other shape is treated by the
  UI as an opaque tag (still selectable as-is).

**Spec loader**: `CapabilitySpec` gains optional `[[variants]]`, each
`{ suffix, args }` — the variant inherits `bin`/`env`/`prompt`/`output`/
`timeout_secs` from the parent and registers tag `{tag}_{suffix}` with its own
**full literal argv** (no placeholder substitution, no insertion logic; the
"argv is verbatim" invariant holds per tag). Loader rejects: suffix charset
violations, suffix collisions, composed tags failing `validate_tag`
(≤64 bytes). `discover()` probes per unique `bin`, not per tag.

**Curated matrix (built-in):**

- `codex.toml`: models `gpt-5.5`, `gpt-5.4`, `gpt-5.4-mini`,
  `gpt-5.3-codex-spark` × efforts `low medium high xhigh` → args `["exec","--json","--sandbox","read-only",
  "--skip-git-repo-check","-m","<model>","-c",
  "model_reasoning_effort=\"<effort>\"","-"]` (8 variants).
- `claude.toml`: models `fable`, `opus`, `sonnet`, `haiku` × efforts `low
  medium high max` → base args + `["--model","<model>","--effort","<effort>"]`
  (16 variants).

**Frontend**: `RunsOnPicker.tsx` replaces `RunsOnField` in both forms —
cascading native selects (Runs on → Model → Effort) derived by decomposing
`state.capabilities`; single-select fallback for opaque tags; offline-tag
pinning behavior preserved. Composes back into one `capability` string through
the existing register/update payloads. No RPC changes.

## WS-C — Agents view rework

- Delete `RunRequestForm` ("ASK TO RESPOND") — superseded by WS-A's
  per-message action + mentions.
- Split the 2092-line `AgentView.tsx` monolith (repo mono-file mandate) into
  `app/src/console/views/agent/`: thin `AgentView.tsx` (tabs/layout),
  `parts.tsx` (shared atoms), `RosterList.tsx`, `AgentDetail.tsx`,
  `RegisterAgentForm.tsx`, `AgentEditForm.tsx`, `RunsOnPicker.tsx` (WS-B),
  `WatchesPanel.tsx`, `RunsTimeline.tsx`. Behavior-preserving except the two
  deliberate changes (picker, form removal).

## WS-D — Spawning overhaul (user-approved scope: prompt fix + thread
continuity + agentic workspaces/tools)

**Found bug:** `runs::render_payload` embeds only `DEFAULT_PROMPT`; no host
code resolves `prompt_hash`, so every agent's registered prompt is silently
ignored. **Found gap:** failed runs never surface in chat (saga Err lands in
the Activity tab only).

**Keystone: a structured run envelope.** The runs module (consensus) stops
flattening everything into one string and composes a JSON envelope — still
deterministic bytes, still committed as ordered state, still ≤
`MAX_PAYLOAD_BYTES`:

```json
{ "ducktape_run": 2,
  "agent_id": "…",
  "prompt_hash": "<64-hex or null>",
  "thread_key": "<channel_id>#<thread_root-or-anchor seq>",
  "instructions": "<DEFAULT_PROMPT — used only when prompt_hash is null>",
  "contract": "<STRICT_OUTPUT_INSTRUCTION>",
  "conversation": "<rendered transcript block>" }
```

The host-side worker (dispatch-oracle / capability-host) detects the envelope
(legacy flat strings still pass through verbatim — mixed in-flight ops across
an upgrade keep working) and assembles the final model input:
`resolved-prompt-or-instructions + contract + conversation`.
Auditability holds: consensus commits the content **hash**; the blob is
content-addressed, so the exact prompt bytes stay verifiable. Blob resolution
failure → loud run failure (never a silent fallback to the generic prompt).

**D1 (consensus side, `crates/apps/runs`):**
- `render_payload`/`render_job_payload` → envelope composition (with
  `prompt_hash` + `thread_key` from run entry state).
- Failure surfacing: on a saga `Err` outcome, `on_result_event` posts a
  threaded reply as the agent — `⚠ <display_name> failed: <sanitized excerpt>`
  — instead of dying silently (dedup via the existing
  `reply_message_id(run_id)` idempotency).

**D2 (host side, `capability-host` + `dispatch-oracle` + `bin/node`,
sequenced after WS-B lands in the same files):**
- `Provider::run` gains a `RunContext { agent_id, thread_key, prompt: Option<resolved> }`
  (worker resolves `prompt_hash` from the node blob store before invoking).
- **Workspaces:** per-agent persistent workdir
  `<data>/agent-workspaces/<agent_id>/` replaces the empty scratch dir.
  Spec opt-in `[workspace] mode = "persistent"`; set for codex + claude.
- **Tools:** codex specs move `--sandbox read-only` → `--sandbox
  workspace-write`; claude specs drop `--max-turns 1` and add
  `--permission-mode acceptEdits` (edits confined to the workspace cwd; bash
  stays denied in v1). Output contract unchanged — the final message must
  still be the strict JSON.
- **Thread continuity:** spec gains `[session]` — capture the provider session
  id from the run output (claude JSON `session_id`; codex JSONL session
  event, tolerant parse) into a host-local store
  `<data>/agent-sessions/<agent_id>/<sha256(thread_key)>`; subsequent runs
  with the same key resume: claude appends `--resume <id>`, codex swaps to
  the spec's `resume_args` (documented single `{session_id}` slot — host-local
  plumbing, NOT the removed consensus model routing). Missing/stale session →
  cold start (also the cross-node fallback: sessions are assignee-local by
  design).

## Testing

- `chat-input` unit tests: mention parse/round-trip, unknown-token inertness.
- Simnode scenario: type-@-mention → auto-watch → engagement → agent reply
  (mirrors the existing hand-built-mark scenario, now through the composer
  path).
- Component tests: MentionMenu keyboard nav; RunsOnPicker compose/decompose
  incl. opaque tags; AgentView split smoke.
- Rust: variant expansion, collision/charset rejection, existing fail-loud
  unknown-field tests stay green. Gates: `cargo clippy -p capability-host
  --tests --no-deps`; app `bun test` + typecheck.
- Live QA: fleet app — register agent with picker, @mention it, watch the
  threaded reply; per-message ask-to-respond.

## Rollout

**WS-D1 is lockstep-class.** The runs envelope + ⚠ failure posts change
consensus **execution**: `render_payload` commits different payload bytes,
and `on_result_event` writes chat posts that old validators never write — so
mixed-version validators root-hash-fork at the first agent dispatch (or the
first failed-run delivery). State *shape* and snapshot codecs are untouched
(no migration, forward-only), but live networks (Ducktape-2 class) must take
the height-gated lockstep upgrade path — same class as PR #232; the
no-downtime upgrade machinery covers it. Secondary hazard even with
validators in lockstep: an OLD node's oracle worker fed a NEW envelope
payload has no `ducktape_run` handling and passes the raw JSON to the
provider CLI verbatim — degraded (but non-forking) output until every
*executing* node is upgraded too.

WS-A/B/C remain rollout-neutral: operator-dropped spec files continue to
override built-ins by tag, and existing agents with bare `codex`/`claude`
tags keep working (base tags remain).

**Prompt-blob durability (known gap, partially closed).** The envelope
commits only the prompt *hash*; the bytes live in the node-local blob store.
That store now writes through to disk (`<storage>/blobstore/`,
content-addressed and self-verifying), so a daemon **restart** no longer
loses every registered prompt. Still open: a run leased by a node that never
held the blob (cross-node assignees — the app only uploads to the node it is
connected to) fails loudly until the owner re-saves the prompt there.
Follow-up options: blob replication between nodes, or a consensus lane for
prompt bytes (size-capped, like duckfs chunks).
