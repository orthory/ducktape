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
- Agent spawning/run-execution overhaul (session continuity, multi-turn,
  workspaces): scoped separately with the user.

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

- `codex.toml`: models `gpt-5.5`, `gpt-5.5-codex` × efforts `low medium high
  xhigh` → args `["exec","--json","--sandbox","read-only",
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

No app-hash movement, no lockstep concern: consensus modules untouched.
Operator-dropped spec files continue to override built-ins by tag. Existing
agents with bare `codex`/`claude` tags keep working (base tags remain).
