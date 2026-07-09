# Pages: editor UX pass — design

2026-07-09 · branch `feat/pages-ux` · app-only (no consensus/module/wire changes)

## Goal

Four user-reported problems with the Pages editing surface. Investigation showed
three of them are one defect wearing three hats, so they are fixed as one pass.

1. After typing the title there is no keyboard action that moves into the body.
2. The title and the body do not share a left edge — "indent main content one
   time just for the list does not make sense".
3. (Found while verifying 1–2.) Text does not *move*: Enter never splits at the
   caret, Backspace never merges into the previous block, and Cmd+Enter on a
   to-do creates a block instead of checking it.
4. Lists feel laggy.

## Unifying diagnosis

The editor treats a block as an **atomic text cell, not a position in a
document**. There is no caret offset that crosses a block boundary. Enter
commits the whole draft and appends a hardcoded empty sibling; Backspace only
fires on a wholly-empty block; every focus hop slams the caret to
`value.length`. Focus is a bare `focusId: string`, which cannot express *where*
in the target the caret belongs.

The fix is one structural idea applied three ways: replace `string` focus with a
`{ id, caret }` intent, and route every grammar key through a pure `resolveKey`
resolver. Alignment (2) is an independent CSS-geometry bug that shares no logic
with the rest. Latency (4) is a rendering-cost bug in the same two files.

## Scope

App-only, zero consensus impact, as in PR #250. **Deferred to their own spec:**
inline formatting (Cmd+B/I) and multi-block Cmd+A selection. Rows are plain
`<textarea>`s with no mark model; both need a real text model, which is a
`BlockKind` wire change and a lockstep upgrade. Caret *column* preservation on
vertical arrows is also out — the line-adjacency rule below fixes the reflex for
far less mechanism.

## Current state (verified against source)

- `RowHandlers.split(row, draftLeft)` is *declared* to receive split text
  (`BlockRow.tsx:157`). The caller passes the **whole** draft
  (`BlockRow.tsx:312`). The implementation drops argument 2 —
  `split: (row) => insertAfterRow(row, continuationKind(...))`
  (`PagesView.tsx:227`) — and `insertAfterRow` hardcodes `text: ""`
  (`PagesView.tsx:163`). The split was designed, typed, and never wired.
- Backspace is gated on `draft === ""` (`BlockRow.tsx:315`). No merge logic
  exists anywhere in the surface.
- The Enter guard is `event.key === "Enter" && !event.shiftKey`
  (`BlockRow.tsx:303`) — it does not exclude `metaKey`/`ctrlKey`, so Cmd+Enter
  splits. The checkbox is reachable only via the marker's `onClick`
  (`BlockRow.tsx:373`), so a to-do cannot be checked from the keyboard at all.
- `focusRow(undefined)` focuses the **title** (`PagesView.tsx:187-190`). The
  title's Enter/ArrowDown handler calls
  `focusRow(rows.find((r) => inputs.current.has(r.block.id)))`
  (`PagesView.tsx:466-470`); on a page with zero body blocks `rows` is empty,
  `find` returns `undefined`, and the title refocuses itself. Enter on a fresh
  page is a dead key.
- Both focus paths force the caret to the end —
  `setSelectionRange(el.value.length, el.value.length)` at `PagesView.tsx:107`
  and `:195`.
- Every row renders a `width: 20` marker box plus a `gap: 8` before the text
  (`BlockRow.tsx:490-511`), but `marker` is `null` for paragraph, heading,
  quote, code and callout (`BlockRow.tsx:416`). So prose is indented 28px to
  reserve space for a bullet it never has, while the title sits at `padding: 0`.
- `BlockRow` is a plain `export function` (`BlockRow.tsx:173`), not
  `React.memo`. `handlers` is a fresh object literal every render
  (`PagesView.tsx:225`). Every store patch re-renders and reconciles all N rows.
- `submitTracked` (`actions.ts:637-659`) applies the optimistic projection
  synchronously, then on failure calls `failOp` + `refresh()`. **There is no
  rollback** — the refresh re-fetches server truth, which silently erases a
  failed op's optimistic effect.

## Design

### 1. `block-keys.ts` — the pure seam (new module)

Key grammar becomes a pure, DOM-free function. `PagesView.tsx` (616) and
`BlockRow.tsx` (576) are both at the repo's ~600-line cap, so this logic cannot
land in either.

```ts
export type Caret = "start" | "end" | number;
export interface FocusIntent { id: string; caret: Caret }

export const caretOffset = (caret: Caret, len: number): number =>
  caret === "start" ? 0 : caret === "end" ? len : Math.max(0, Math.min(caret, len));

export function splitText(value: string, caret: number): { left: string; right: string };
export function mergeText(prev: string, cur: string): { text: string; seam: number };

export interface KeyContext {
  key: string; shiftKey: boolean; metaKey: boolean; ctrlKey: boolean; altKey: boolean;
  value: string; caretStart: number; caretEnd: number;
  kind: BlockKind; slashOpen: boolean; prevKind: BlockKind | null;
}

export type KeyIntent =
  | { type: "split"; left: string; right: string }
  | { type: "merge-prev" }
  | { type: "remove-empty" }
  | { type: "remove-divider-above" }
  | { type: "exit-to-paragraph" }
  | { type: "toggle-check" }
  | { type: "toggle-collapse" }
  | { type: "indent" } | { type: "outdent" }
  | { type: "move-up" } | { type: "move-down" }
  | { type: "focus-prev" } | { type: "focus-next" }
  | { type: "none" };

export function resolveKey(ctx: KeyContext): KeyIntent;
```

`resolveKey` rules, in order:

1. `slashOpen` → `none` (the menu owns the key).
2. `mod = metaKey || ctrlKey`. `mod && key === "Enter"` → `toggle-check` for
   `todo`, `toggle-collapse` for `toggle`, else `none`. **Never `split` under a
   modifier.**
3. `Enter && !shiftKey`:
   - empty (`value.trim() === ""`) and `emptyEnterExits(kind)` →
     `exit-to-paragraph`;
   - `kind === "code"` → `none` (the textarea inserts a real newline — Enter
     inside a code block must not split it);
   - else → `split` at `caretStart`.
4. `Backspace`:
   - `value === ""` → `remove-empty`;
   - `caretStart === 0 && caretEnd === 0 && prevKind === "divider"` →
     `remove-divider-above`;
   - `caretStart === 0 && caretEnd === 0` → `merge-prev`;
   - else `none`.
5. `Tab` → `indent` / `outdent` (shift). `Alt+Arrow` → `move-up` / `move-down`.
6. Collapsed-caret edges: `ArrowUp` or `ArrowLeft` at offset 0 → `focus-prev`;
   `ArrowDown` or `ArrowRight` at `value.length` → `focus-next`.
7. Otherwise `none`.

Shift+Enter stays unintercepted, so the textarea keeps inserting a real `\n`.

### 2. Caret lands on the adjacent line, not a fixed end

The rule is line-adjacency, and it is the opposite of what a first reading
suggests:

- **`focus-prev` → `caret: "end"`.** ArrowUp from block N's first line lands on
  block N−1's *last* line. Landing at its start would jump the caret several
  lines in a multi-line block. Today's behavior is already correct here.
- **`focus-next` → `caret: "start"`.** ArrowDown from block N's last line lands
  on block N+1's *first* line. Today it forces `value.length`, which is the
  actual bug.
- ArrowLeft at offset 0 → previous block, `"end"`. ArrowRight at the end → next
  block, `"start"`.
- `merge-prev` → numeric caret at the seam (the previous block's original
  length).
- Title descent (Enter / ArrowDown) → first block, `"start"`, since descending
  is a downward move.

`focusId: string | null` (`PagesView.tsx:43`) becomes `FocusIntent | null`. The
queued-focus effect (`:102-110`) and `focusRow` (`:187-197`) resolve
`caretOffset(intent.caret, el.value.length)` instead of hardcoding
`el.value.length`.

### 3. Title → body

Enter or ArrowDown on the title commits it, then:

- if the page has at least one body row → focus the first row at `"start"`;
- if the page has **no** body rows → append a paragraph block (reusing
  `appendBlock`) and focus it.

This is the fix for the reported dead key. `focusRow(undefined)` keeps its
"focus the title" meaning for the ArrowUp-from-first-block path, which is
correct and stays.

### 4. Text must never be destroyed by a keystroke

A split is two ops: `updatePageBlockText(cur, left)` and
`insertPageBlock(new, right)`. They settle independently, and a failed op is not
rolled back — it is erased by the next authoritative `refresh()`. So if the
truncation commits and the insert fails, **the text after the caret is
permanently lost.**

Chaining (await the insert, then truncate) is correct but reintroduces the
latency this pass exists to remove: the tail would sit duplicated on screen for
a full consensus round-trip.

**Decision — compensating write.** Submit both immediately, so both optimistic
projections land in the same tick and the UI stays instant. If the *additive* op
rejects, restore the merged text:

```ts
// split
const inserted = actions.insertPageBlock({ blockId, parent, after, kind, text: right });
actions.updatePageBlockText({ blockId: cur.id, text: left });
inserted.catch(() => actions.updatePageBlockText({ blockId: cur.id, text: left + right }));

// merge-prev
const merged = actions.updatePageBlockText({ blockId: prev.id, text: prev.text + cur.text });
actions.removePageBlock(cur.id);
merged.catch(() => actions.insertPageBlock({ ...cur, after: prev.id, text: cur.text }));
```

The invariant: **no keystroke may lose text.** The worst case degrades to a
*visible duplicate* (the tail exists in both blocks), never to silent loss, and
the user already sees the op failure via the existing `fail(err)` surface
(PR #200).

This requires `insertPageBlock` and `updatePageBlockText` to return a promise
rather than `void` (`actions.ts:256-264`). It resolves **`boolean`**, not
`void`: `submitTracked`'s `.catch` already handles and *swallows* the failure
(`failOp` + `fail(err)` + `refresh()`, `actions.ts:655-659`), so its promise
resolves either way and a caller's `.catch()` would never fire. Making it
reject instead would turn every existing caller — all of which ignore the
result — into an unhandled rejection. So it resolves `true` on commit and
`false` on a surfaced failure, and only an explicit `false` triggers
compensation.

### 5. `emptyEnterExits` (pages-model.ts)

Enter on an *empty* block exits to a paragraph. Today the branch keys on
`continuationKind(kind) === kind` (`BlockRow.tsx:306`), true only for
`bulleted`/`numbered`/`todo`. Add a predicate beside `continuationKind`:

`emptyEnterExits(kind)` → true for `bulleted`, `numbered`, `todo`, `quote`,
`code`, `callout`, `toggle`; false for `paragraph`, headings, `divider`, `page`.

### 6. Alignment — markers hang, prose is flush

The text column becomes the canonical left edge. The marker leaves the flex flow
and hangs into the left margin:

- marker box → `position: absolute; left: -MARKER_HANG` (28px) on a
  `position: relative` row; the text column becomes the first in-flow item, at
  offset 0.
- Paragraph/heading/quote/code/callout text now aligns exactly with the title
  input, which keeps `padding: 0`.
- Bullets, numbers, checkboxes and toggle chevrons stay visually where they are
  today relative to their own text.
- Nesting is unaffected: `marginLeft: depth * INDENT` (26) still shifts the whole
  row, marker and all.
- The add-block button's `padding: "8px 0 8px 28px"` (`PagesView.tsx:530`)
  becomes `"8px 0"` — it was compensating for the same phantom gutter.
- The quote's `paddingLeft: 12` (`BlockRow.tsx:432`) is inside the text column
  and is unaffected.

Also: headings get `marginTop: headingTopSpace(kind)` (uniform `padding: "2.5px 0"`
today, `BlockRow.tsx:497`), and the dead `paddingTop` for `heading1`/`heading2`
on the always-empty marker box (`BlockRow.tsx:509`) is deleted.

New module `pages-style.ts`: `MARKER_HANG`, `INDENT`, `headingTopSpace(kind)`.

The focused-only placeholder (`BlockRow.tsx:462`) is **left as-is** — Notion also
shows the hint only on the focused block, and the empty-canvas hint already lives
at `PagesView.tsx:536`.

### 7. Latency — confirmed mechanism, corrected explanation, honest residual

**Confirmed:** `BlockRow` is un-memoized (`BlockRow.tsx:173`), fed a fresh
`handlers` literal (`PagesView.tsx:225`) and a fresh `blocks`/`rows` array on
every store patch. So **every store write re-renders and reconciles all N rows**.
Typing a character is cheap (`setDraft` re-renders only the focused row); the
O(N) cost lands on Enter and on the finalize + `refresh()` patches that follow
each op.

**Refuted, and not carried into this design:** the claim that lists lag because
`continuationKind` makes lists "grow to large N faster". Enter adds exactly one
block whether the result is a paragraph or a bullet. `continuationKind` sets the
*kind*, not the *rate*.

**The real list-specific factor is interaction density.** Building a list is a
burst of back-to-back Enters with no typing between them. Each Enter fires two
ops (`maybeCommit()` then `split`, `BlockRow.tsx:311-312`), and each op's settle
triggers `finalizeOp` + a full `refresh()` (`actions.ts:652-658`). Prose spaces
the same Enter hits out with long runs of cheap keystrokes, which hides them.

**The one genuinely list-only residual `React.memo` cannot fix:** numbered-list
`listIndex` (`pages-model.ts:59-67`). Inserting or deleting inside a numbered run
shifts `listIndex` for every trailing numbered row, so those rows *must*
re-render. This is correct behavior, not a bug.

**The fix, and why it is not cargo-cult `React.memo`:** it works only if both
conditions hold.

1. The memo comparator must key on `row.block`, **not** `row` — `buildRows`
   allocates a fresh `{ block, depth }` wrapper on every recompute, so a default
   shallow compare on `row` never matches and the memo is inert.
2. `handlers` must be stabilized (a `useMemo` whose callbacks read live state
   through refs), or the fresh literal defeats the memo by itself.

**Residual, stated plainly rather than papered over:** each op's settle also runs
a ~17-query `Promise.all` `refresh()` that deserializes the full snapshot and
dispatches a whole-state patch — O(payload) main-thread work, independent of row
count, which the memo does not touch. `holdPages` (`DucktapeProvider.tsx:223`)
already *discards* the pages slice of that refresh while a page op is pending,
yet the refresh still runs. De-duplicating it is plausibly co-dominant.

**Deliberate scope call:** refresh-dedupe changes store behavior for *every*
view (chat, files, agents, runs), not just Pages. It does not belong in an
app-only Pages UX PR. This pass ships the memo + stable handlers, proves the
render-count drop with a test, and files refresh-dedupe as a follow-up. **This
means the lag will be reduced, not necessarily eliminated.** Anyone reading a
"fixed latency" claim on the PR should read this paragraph instead.

## Testing

**Pure (vitest, no DOM) — the bulk:**

- `block-keys.test.ts`: `splitText` (mid, both edges, clamp high and low),
  `mergeText`, `caretOffset` (start/end/numeric/clamp), and `resolveKey` across
  every intent, including the regressions: Cmd+Enter on todo → `toggle-check`;
  Ctrl+Enter on toggle → `toggle-collapse`; Cmd+Enter on paragraph → `none`;
  Shift+Enter → `none`; `slashOpen` → `none`; Enter in `code` → `none`;
  Backspace at 0/0 non-empty → `merge-prev`; Backspace at 0/0 under a divider →
  `remove-divider-above`; empty list → `exit-to-paragraph`.
- `pages-model.test.ts`: `emptyEnterExits` across every kind.
- `pages-style.test.ts`: `headingTopSpace`, `MARKER_HANG === 28`.

**Component (vitest + jsdom, `PagesView.test.tsx`):**

- Split leaves `left` in the current block, `right` in the new one, focused at
  `selectionStart === 0`.
- Backspace at 0/0 merges `prev + cur`, removes the current block, caret at
  `prev.length`.
- Cmd+Enter on a to-do toggles `checked` and inserts **no** block.
- Title Enter on an empty page inserts a paragraph and focuses it; on a page with
  a body, focuses the first row at `start` and inserts nothing.
- ArrowDown lands at `start`; ArrowUp lands at `end`.
- A render-count assertion: on a 50-block page, one edit-boundary commit
  re-renders ~1 row after the fix, versus all 50 before it.

**jsdom caveat:** a freshly focused textarea's selection is not guaranteed to sit
at 0 or `value.length`. Every caret/arrow/Backspace test must call
`setSelectionRange` before dispatching the key.

**Genuinely needs the real app (`tauri-debug` skill):** the perceived-jank claim,
and the split/merge tear under a real daemon settling two ops independently.
jsdom render counts approximate the first; only WebKitGTK with a live node shows
the second.

## Implementation order

Both host files are at the line cap and all groups touch them, so the *edits*
serialize even though the investigation parallelized.

1. `block-keys.ts` + `block-keys.test.ts` — pure, no host-file edits.
2. `pages-model.ts` `emptyEnterExits`, `pages-style.ts` + tests — pure, additive.
3. Alignment (`BlockRow.tsx` render styles, `PagesView.tsx:530`) — touches layout
   lines, disjoint from the keydown/focus lines, so it lands cleanly first.
4. **One combined edit** of `onKeyDown` (`BlockRow.tsx:277-356`) and the focus
   machinery (`PagesView.tsx:43, 102-110, 187-197`, `handlers`, `split`). The
   caret and text-model changes rewrite the same lines and cannot be separated.
   `RowHandlers` widens once: `split(row, left, right)`, `mergePrev(row)`,
   `removeDividerAbove(row)`, `focusRelative(row, delta, caret)`.
5. Latency memo + stable handlers **last** — it wraps the final `BlockRow` export
   and rebuilds `handlers`, so it must see step 4's final handler set.

`insertAfterRow` is inlined into `split` and removed only after confirming no
other creator needs it.

### Keeping both host files under the cap

The pass adds behavior to two files that were already at ~600 lines, so it also
takes two responsibilities out of them:

- `use-row-handlers.ts` — row intents → store ops + caret placement, the
  stable-`handlers` hook. Out of `PagesView.tsx`.
- `SlashMenu.tsx` — the "/" palette. Out of `BlockRow.tsx`.

Net effect: `PagesView.tsx` 616 → 539, `BlockRow.tsx` 576 → 574. The monsters
shrink rather than grow.

### The memo comparator cannot use reference equality

`applySnapshot` (`state.ts:542`) passes freshly deserialized objects straight
through, so after any authoritative `refresh()` every `PageBlock` is a new
object even when nothing about it changed. A comparator keyed on `row.block`
identity would therefore go inert for exactly the patches that follow each
Enter. It compares the fields `BlockRow` actually reads — `id`, `kind`, `text`,
`checked`, plus `row.depth` and `row.listIndex`.

`pages-render-cost.test.tsx` measures this directly: it mocks `headingTopSpace`
(called once per row render) as a render counter, and asserts that the cost of a
patch is **independent of N** by running the same mutation on a 20-row and an
80-row page. Verified to fail without the memo, and to fail again if `handlers`
is made unstable — both conditions are guarded, not assumed.

## Risks

- The compensating write (§4) is best-effort: if the compensation *itself*
  fails, the tail is lost. Accepted — it degrades a silent loss into a loud one.
- The memo's correctness depends on `handlers` staying stable. A future edit that
  reintroduces a fresh literal silently un-fixes the latency. The render-count
  test guards this.
- Enter in a code block changing from "split" to "newline" is a behavior change
  for anyone who relied on the old behavior. Judged strictly better.
- `focus-next → start` changes ArrowDown's landing position. Deliberate.
