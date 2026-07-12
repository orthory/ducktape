# Agent soul, part 4 — the desktop app surface

Branch `soul-app` (off `compute-capability-p1`). Territory: `app/src/**` only; no
Rust, no `bin/**`.

## What changed

**The wire (`app/src/domain/agent-client.ts`)**

- `prompt_hash` is gone from `AgentRecord`, `RegisterAgent` and `UpdateAgent`.
- New `LoadMode = "always" | "on_demand"` and `SkillRef { name, source_prefix,
  source_snapshot?, load }`; `AgentRecord.skills?: SkillRef[]` (absent on the
  wire when empty, matching the Rust `skip_serializing_if`).
- `registerAgent` omits `skills` entirely when the set is empty; `updateAgent`
  sends `skills: null` when untouched and the full array when provided — an
  update REPLACES the curated set wholesale (`[]` clears it).
- `hexToBytes` stayed: it is no longer a prompt helper but identity-client and
  chat-client re-export it (`keyBytes`), so deleting it would have rippled into
  three unrelated modules. Only its doc comment changed.

**The blob-upload path is deleted (`app/src/console/store/actions.ts`)**

`registerAgent` / `updateAgent` no longer call `transport.putBlob`. Registration
now commits pins only. `transport.putBlob` itself stays — the blob plane still
carries run replies and forge packs.

**Skill curation in the agent form**

- New `app/src/console/views/agent/skills.ts` — the pure half: `personaPrefix`,
  `skillDocPath`, `cleanPrefix`, `cleanSkills`, `skillTemplate`, `skillsSummary`.
- New `app/src/console/views/agent/SkillsField.tsx` — the shared field used by
  both the register and edit forms. One row per skill: name, duckfs folder, an
  **Always load** checkbox, Remove, plus **Open in Files** and **Create doc**.
  Two seed buttons: `+ Persona (always loaded)` and `+ Skill (on demand)`.
- The one-line copy carrying the distinction: *"Always-loaded skills are pasted
  into every run — together they are the agent's persona. The rest are listed by
  name, and the agent opens them from its skill folder only when the job calls
  for one."* An agent with no always-skill gets an honest warning rather than a
  blocked submit (the spec keeps the `instructions` fallback for that state).
- `AgentDetail` drops the `prompt <hex>` InfoRow for a `skills: 1 always · 2 on
  demand` row plus a SKILLS section: one row per skill with an ALWAYS /
  ON DEMAND chip, its `…/SKILL.md` path, and an Open button.

**Persona editing = a duckfs document, not a textarea**

- The `System prompt` / `New prompt` textareas are gone from both forms.
- **Create doc** works: it is an ordinary duckfs commit through the existing
  client (`files-client.uploadFile`, which the files browser already writes
  through — `put` auto-creates intermediate directories, so no mkdir chain is
  needed). It stats first and REFUSES to overwrite an existing document, so it
  can never clobber a persona. The seeded `SKILL.md` carries YAML frontmatter
  (`name`, `description`) — the fields the assembler's on-demand index reads.
- **Open in Files** is a real deep link: new one-shot `state.filesFocus` +
  `actions.openFiles(path)`, copied verbatim from the existing `forgeFocus`
  idiom (provider retires it on screen leave; `FilesView` consumes it in an
  effect and calls its own `navigate`).

## Degraded honestly

- **There is no in-app text editor for a skill document, and I did not build
  one.** `FilePreview` is preview + download only. So the loop is: *Create doc*
  seeds a starter `SKILL.md` from the agent form → the operator edits the text
  in the Files surface by replacing the file (drag-drop / upload), or via
  duckfs / forge outside the app. The form points at the document and can create
  it; it cannot rewrite its body. If in-app persona editing matters, the missing
  piece is an edit-and-save affordance in `FilePreview` — a separate change,
  useful to every duckfs text file, not just agent souls.
- **No skill picker / browser.** The duckfs folder is a typed path (prefilled
  with `/shared/agents/<agent-id>/persona` for the persona). Curating an
  existing shared skill means typing or pasting its prefix. A picker over
  `files.find` is easy later; it was not needed to make the model legible.
- **`source_snapshot` is carried on the wire but has no UI.** Every skill is
  therefore pinned to the run's head. Snapshot pinning is a power-user knob;
  the type round-trips it, so a form control can be added without a wire change.
- **`/shared/agents/<agent-id>/…` is a convention, not an enforced layout.** The
  field accepts any prefix (the Rust module validates).

## Tests

`cd app && bun run test` → **887 passed, 14 skipped** (100 files: 97 passed,
3 skipped — the binary-gated simnode / live-daemon e2e suites, skipped before
this change too). `bun run typecheck` → clean.

New/changed coverage:

- `domain/agent-client.test.ts` — exact-match assertions that **no `prompt` or
  `prompt_hash` key is ever sent**; skills ride with their `load` mode in
  curation order; an empty set is omitted on register; `skills: []` clears on
  update; `skills: null` when untouched.
- `views/agent/skills.test.ts` (new) — the pure helpers: persona path, doc path,
  prefix normalization, `cleanSkills` dropping half-typed rows while preserving
  order and load mode, template frontmatter, summary line.
- `views/agent/AgentView.test.tsx` — no prompt textarea in the register pane;
  curating a persona + an on-demand skill produces the exact `skills` payload;
  the Always-load toggle flips `always` ⇄ `on_demand`; the detail pane's Open
  hands off to `openFiles`; **Create doc** commits a `put` of the starter
  `SKILL.md` and, on a second click with the doc present, refuses to overwrite.
- `views/files/FilesView.test.tsx` — a `filesFocus` hand-off lands the browser
  on the handed-off directory instead of the default `/shared`.

## Concerns / coupling

- This branch codes against the NEW wire the sibling agents are landing. Until
  the Rust `SkillRef.load` / `prompt_hash` removal merges, the app's register
  payload will be rejected by an old node (missing required `prompt_hash`).
  Flag day, as designed — the three parts must land together.
- `simnode.scenario.test.tsx` sends raw `register_agent` JSON; I dropped
  `prompt_hash` from its two fixtures. That suite only runs with a built
  `simnode` binary (it skipped here), so it is untested against the new Rust —
  worth one run after the merge.
- Nothing in the app writes `load: "always"` for more than one skill by default,
  but the field allows several; the assembler's inline order is the curation
  order, which is exactly the row order the form submits.
