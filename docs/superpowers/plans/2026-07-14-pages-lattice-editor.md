# Pages Lattice Editor Restyle + Range-Comment UX Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Apply the layout.html ("Lattice") reference editor design to Pages — everything except its color palette — and finish the range-comment UX: selection guide menu, dismissal rules, per-range threads, and a docked side comment card that nudges the content column left when there is room.

**Architecture:** Frontend-only. The range-comment wire (Thread.anchor + rebasing) already exists end-to-end (commit ab87908f5); this plan restyles the surface to the reference spec and converts the fixed comment popover into a width-tiered surface (wide = docked margin card in a right rail inside the doc scroller; narrow = existing fixed popover). All colors map to theme tokens (`app/src/console/theme/tokens.ts`) — the reference's palette is explicitly excluded.

**Tech Stack:** React + inline styles + theme tokens; vitest/jsdom for behavior; fleet CEF for layout QA.

**Reference:** scratchpad `reference-spec-core.md` + `reference-spec-extra.md` (distilled from duckfs `/shared/layout.html@39eae33f`). Key numbers repeated inline below so this plan stands alone.

## Global Constraints

- NO color palette from the reference — every color = existing token (`color.*`, `accentVar`, `tint()`). No new hex values.
- No Rust/wire changes. Thread anchor model stays `{ target: blockId, anchor: {start,end} }`.
- Keep #490 contracts: unconditional `loadPageThreads` rollback, `pageThreadsToken` supersede guard, `isComposing` guards on Escape/Enter, hidden Edit/Delete/Resolve while thread op pending.
- Keep BlockRow memo discipline (field comparison, stable handlers) — `pages-render-cost.test.tsx` must stay green.
- TS tests live in existing suite files next to the components (`PagesView.test.tsx`, `CommentCard.test.tsx`) or `app/src/test/sim/` — SHORT names.
- Vitest runs from `app/` (`npx vitest run`), never repo root.
- ~600-line soft cap per file (avoid-mono-files mandate).

---

### Task 1: Reference geometry into pages-style + block frames (restyle core)

**Files:**
- Modify: `app/src/console/views/pages/pages-style.ts` (kindFont scale)
- Modify: `app/src/console/views/pages/PagesView.tsx:493-500` (column: maxWidth 780→700, padding top 44 / bottom 100)
- Modify: `app/src/console/views/pages/BlockShell.tsx` (code block: padding 18px 20px, radius 8 (`radius.md`), mono `13.5px/1.85`, bg `color.panel`, border `color.borderSoft`; divider: 1px `color.borderSoft` margin `18px 0` row-adjusted; callout: `3px` left border, `tint()` wash, radius `0 8px 8px 0`, padding `14px 16px`; quote unchanged bar but `15px/1.7` body)
- Test: `app/src/console/views/pages/PagesView.test.tsx` (existing snapshots of kind styling — adjust)

**Interfaces:**
- Produces: `kindFont(kind)` returns: h1 `700 26px/1.25`, h2 `700 22px/1.3`, h3 `650 18px/1.4`, code `400 13.5px/1.85 mono`, default `400 15px/1.7`.

**Steps:**
- [ ] kindFont scale: h2 20→22, h3 17→18, code 13/1.7→13.5/1.85, body 15/1.75→15/1.7.
- [ ] Column geometry in PagesView (700 / `44px COLUMN_PAD_X 100px`).
- [ ] BlockShell code/divider/callout frames per numbers above.
- [ ] `npx vitest run src/console/views/pages` green; fix any style assertions.
- [ ] Commit `style(pages): adopt Lattice document geometry and block frames`.

### Task 2: SelectionToolbar → reference guide menu (11a/12a shape)

**Files:**
- Modify: `app/src/console/views/pages/SelectionToolbar.tsx`
- Modify: `app/src/console/views/pages/BlockRow.tsx` (pass `onTurnInto`, dismissal wiring)
- Modify: `app/src/console/views/pages/use-row-handlers.ts` (expose setKind for toolbar if not reachable)
- Test: `app/src/console/views/pages/PagesView.test.tsx`

**Interfaces:**
- Produces: `SelectionToolbar` props gain `onTurnInto(kind: BlockKind)`; visual per spec: width 236, padding 6, `radius.lg`, `shadow.pop`, `color.paper` bg; row 1 = eyebrow `TEXT STYLE` (`600 10px`, ls .05em, `color.muted2`) + six 30px-high cells H1 H2 H3 B I U (grid), active cell = `color-mix(accentVar 22%, paper)` + 1px accent border; keep S and `<>` cells (our editor has them; reference has no strike/code but removing features is not restyling); hairline divider; `Comment` row = `500 13px`, chat icon right, `⌘/` hint `600 11px color.muted2`.
- Dismissal contract (the "any other action closes it" rule): closes on (a) selection collapse (existing onSelect path), (b) outside mousedown, (c) any scroll (capture), (d) Escape, (e) window blur. Typing keeps textarea focus → selection collapses → (a) covers it.
- `Cmd/Ctrl+/` with a non-collapsed selection opens the comment composer for that range (same as clicking Comment).

**Steps:**
- [ ] Failing tests: toolbar shows H1/H2/H3 cells; clicking H2 calls setKind; toolbar disappears on document scroll event; `⌘/` opens comment card with range.
- [ ] Implement restyle + onTurnInto + dismissal listeners (scroll capture + Escape live in SelectionToolbar effect; keep onBlur).
- [ ] Vitest green.
- [ ] Commit `feat(pages): Lattice selection menu with turn-into and dismissal rules`.

### Task 3: Per-range thread scoping

**Files:**
- Modify: `app/src/console/views/pages/PagesView.tsx` (commentCard state gains `range`; filter threads passed to card)
- Modify: `app/src/console/views/pages/CommentCard.tsx` (accept `threads` already filtered; header label shows quoted range)
- Modify: `app/src/console/views/pages/BlockRow.tsx:525-533` (click inside a commented range opens THAT range's threads only)
- Test: `app/src/console/views/pages/CommentCard.test.tsx`, `PagesView.test.tsx`

**Interfaces:**
- Produces: pure helper `threadsForRange(threads: ThreadView[], range: RelativeAnchor | undefined): ThreadView[]` in `PagesView.tsx` (or pages-model.ts): with a range → threads whose anchor overlaps `[start,end)`; without → ALL threads for the target (block-level affordance keeps showing everything).
- Clicking highlighted range R → card scoped to overlapping threads, composer pre-anchored to R's exact thread anchor (reply-first UX). New selection + Comment → composer anchored to the selection; a distinct range in the same block always creates a NEW thread.

**Steps:**
- [ ] Failing tests: two threads on ranges A and B in one block; clicking inside A shows only A's thread; new-thread composer from selection C creates a third thread with anchor C.
- [ ] Implement filter + wiring.
- [ ] Vitest green.
- [ ] Commit `feat(pages): scope comment card to the clicked range`.

### Task 4: Width-tiered comment surface — docked side card + content nudge

**Files:**
- Modify: `app/src/console/views/pages/PagesView.tsx` (doc column wrapper becomes flex row with right rail; dock-tier decision; content-relative anchor capture)
- Modify: `app/src/console/views/pages/CommentCard.tsx` (a `docked` mode: `position:absolute` in rail at anchor top, width 340, no scroll-dismiss; keep fixed-popover mode for narrow)
- Test: `app/src/console/views/pages/PagesView.test.tsx`

**Interfaces:**
- Tier rule (pure, exported for test): `commentSurface(viewportW: number): "docked" | "popover"` → `viewportW >= 1180 ? "docked" : "popover"` (700 column + 340 card + gutters/padding).
- Layout: doc-scroll content = `display:flex; justify-content:center` → `[div flex:1 min 0]` `[column 700]` `[rail flex:0 0 (docked ? 380 : 0); transition: flex-basis 200ms; position:relative]`. When rail opens the auto-centering pushes the column left — the "nudge".
- Docked card: `position:absolute; top: anchorTopInContent; right: 16; width: 340` inside the rail; scrolls WITH the document (no scroll dismissal in docked mode; outside-mousedown + Escape still close). `anchorTopInContent` = anchor viewport Y − content box top, clamped ≥ 0, captured when the card opens.
- Popover mode unchanged (existing behavior + dismissal).

**Steps:**
- [ ] Failing tests: with `window.innerWidth=1600` card renders inside `[data-comment-rail]` (absolute), rail has nonzero flex-basis, and a scroll event does NOT close it; with `innerWidth=1000` card is `position:fixed` and scroll closes it.
- [ ] Implement rail + docked mode.
- [ ] Vitest green; `pages-render-cost.test.tsx` still green.
- [ ] Commit `feat(pages): docked side comment card nudges content when space allows`.

### Task 5: Margin comment badge per commented row (12c/12d)

**Files:**
- Modify: `app/src/console/views/pages/BlockRow.tsx` (right-margin affordance: replace/upgrade current comment button with 💬+count badge; open-state accent vs resting muted; opens block threads — clicking the highlight itself stays range-scoped)
- Test: `app/src/console/views/pages/PagesView.test.tsx`

**Steps:**
- [ ] Failing test: block with 2 threads renders badge text "2"; block with none renders the plain hover-only chat icon.
- [ ] Implement: chat icon + `650 11.5px` count, `color.muted2` resting / `accentVar` while card open on that target, `right:-40`-equivalent placement in the existing right tray.
- [ ] Vitest green.
- [ ] Commit `feat(pages): per-block margin comment badge`.

### Task 6: Panels/menus restyle to reference (SlashMenu, BlockGutter menu, CommentCard/Thread)

**Files:**
- Modify: `app/src/console/views/pages/SlashMenu.tsx` (width 220, panel bg, radius `radius.lg`, padding 4–6, eyebrow `600 10px` ls .04em, item 30–32px h, radius 6, icon slot 16)
- Modify: `app/src/console/views/pages/BlockGutter.tsx` (menu same recipe; + button active = accent fill; handle glyph spacing)
- Modify: `app/src/console/views/pages/CommentCard.tsx` + `CommentThread.tsx` (card radius 12, padding 14; entry: avatar 26, name `600 13px` + time muted `margin-left:6`, body `13.5px/1.5 color.inkSoft`, hairline dividers between entries; composer row: avatar + input + 26px `radius.sm` accent send button with ↑; anchored-quote header keeps amber left bar)
- Test: existing suites (assertions on labels/roles unchanged — visual-only diffs)

**Steps:**
- [ ] Apply each restyle; keep all aria labels/roles/test ids.
- [ ] `npx vitest run src/console/views/pages` green.
- [ ] Commit `style(pages): Lattice panels — slash menu, gutter menu, comment threads`.

### Task 7: Gates + fleet QA + papercut fixes

**Steps:**
- [ ] `cd app && npx tsc --noEmit` clean; full `npx vitest run` green.
- [ ] Fleet instance up (qa skill), live-drive: select text → menu appears/dismisses per contract; comment via menu; per-range threads; docked card + nudge at wide viewport; popover at narrow; badge counts; drag-handle & slash menu styling.
- [ ] Fix every papercut found (each its own commit); re-run suite.
- [ ] PR to dev with before/after screenshots; clean-context review; merge on high confidence only.

## Deferred (explicitly out of scope)

- SKILLS section of the selection menu (agent actions on a range) — needs agent-plane wiring; the menu leaves no dead space without it.
- Multi-block ranges (wire change), reference status pills/share popover/page-depth tree (separate surfaces).
- Reference font (Plus Jakarta Sans) — palette-adjacent identity choice; we keep `font.sans`.
