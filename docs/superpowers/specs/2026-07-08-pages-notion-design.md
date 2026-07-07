# Pages: Notion-like editing surface — design

2026-07-08 · branch `feat/pages-notion` · app-only (no consensus/module changes)

## Goal

Seven user-reported problems with the Docs/Pages surface, fixed as one coherent
Notion-like pass:

1. Clicking a block's comment button opens the full comments panel (the list of
   every thread on the page). Wanted: a floating card at the block.
2. "Add a block" does not respond to clicks.
3. "Copy block link" is noise — remove it.
4. The document renders as a bordered card, so the page visibly "ends".
   Wanted: an endless full-bleed canvas.
5. No way to create a page inside a page from the editor (Notion subpages).
6. Cmd+W closes (hides) the whole app window instead of the active doc tab.
7. Everything pages-related QA'd against the real app, self-heal loop ×5.

## Current state (found in exploration)

- `PagesView.tsx` is a 1322-line mono-file: `BlockRow` + `SlashMenu` +
  `PageRail` + the view. Violates the repo's ~600-line file mandate.
- Block comment button → `openComments()` → sets `panelOpen` → the right-hand
  `CommentsPanel` renders **all** threads flat (problem 1).
- "Add a block" is a real `<button onClick>` — but any focused block textarea
  blur-commits first, the store patch re-renders the tree between mousedown and
  mouseup, and the click is lost. `SlashMenu` already dodges this exact trap
  with `onMouseDown` + `preventDefault` (problem 2).
- "Copy block link" is the hover `hash` button on every row (problem 3).
- The doc body is a `maxWidth: 820` bordered/shadowed card with a visible
  bottom edge on a gray canvas (problem 4).
- The `pages` module already supports nesting: `create_page{parent}`,
  `set_page_parent`, `PageMeta.parent`; the rail tree renders it. The editor
  just has no way to create/see subpages from inside a page (problem 5).
- ⌘W is deliberately unhandled in the view; the Tauri shell intercepts window
  close as hide-to-tray, and on macOS the default menu's "Close Window"
  (Cmd+W) fires that before the webview ever sees the key (problem 6).

## Design

### 1. Floating comment card (replaces panel-on-block-click)

New `CommentCard.tsx`: a floating card (position: fixed, clamped to viewport,
~340px wide) anchored to the clicked comment button's rect. Contents: the
target's threads + reply/resolve/edit/delete, and a composer (auto-focused when
the target has no threads; behind an "Add comment" button when it does).
Closes on Escape, outside click, or doc scroll.

- Block comment button → floating card for that block. No panel.
- Header "Comment" button → floating card targeting the page id.
- Header "Comments" button keeps toggling the existing full `CommentsPanel`
  (the explicit "show me everything" surface stays; it just never opens as a
  side effect of a block click).
- Thread rendering (`ThreadCard`, composer, button styles) moves from
  `CommentsPanel.tsx` into a shared `CommentThread.tsx` consumed by both the
  panel and the card.

### 2. Clickable add-block + endless canvas (fixes 2 and 4 together)

- The doc card chrome goes away: no border, no shadow, no max-width box on a
  gray backdrop. The scroll area is full-bleed `paper`; the content column
  stays centered (max-width ~760px padding-based, Notion-like).
- Below the last block, a click-to-append zone grows to fill the remaining
  viewport (`flex: 1`, `minHeight: 40vh`): `onMouseDown` + `preventDefault` →
  append a paragraph block and focus it. The page never shows an "end".
- The explicit "Add a block" affordance stays for discoverability but becomes
  `onMouseDown`-driven (same blur-commit dodge as `SlashMenu`), so it works
  even while another block is being edited. On an empty page it reads
  "Start writing — or press '/' for commands" as today.

### 3. Remove "Copy block link"

Delete the hover `hash` button from `BlockRow`. Block ids stay resolvable via
`getBlock` for future BlockRef surfaces; there is just no manual copy chrome.

### 4. Notion-like subpages (app-only)

- `SLASH_KINDS` gains `{ kind: "page", label: "Page", hint: "new subpage" }`.
  Picking it does NOT `set_kind` — it calls `createChildPage(activePage)`
  (existing action: creates an untitled child page and opens it in a tab, with
  the cursor in the title). The slash-command draft is cleared.
- New `Subpages.tsx` section in the doc body, under the title: one row per
  child page of the open page (`state.pages` filtered by `parent === active`),
  page icon + live title, click to open; plus a trailing "New subpage" row.
  Rendered only when the page has children (the "New subpage" affordance also
  lives in the `/page` command and the rail).
- Deliberate call: subpage links are **derived from the page-parent index**,
  not stored as inline blocks. Inline page-link blocks would need either a new
  wire `BlockKind` (consensus change, lockstep upgrade) or a magic text
  convention that leaks into search and other surfaces, and can dangle after
  deletes. The derived section can never be stale.

### 5. Cmd/Ctrl+W closes the active doc tab

- The Docs-scoped keydown handler gains: meta/ctrl + `KeyW` (no shift/alt) →
  `preventDefault` + close the active tab (`closeTab`), matching the existing
  ⌘⇧[/] tab-cycling. With no open tab it does nothing and does not bubble into
  a window hide.
- macOS shell: the default Tauri menu's "Close Window" (Cmd+W) fires before
  the webview sees the key. `main.rs` gets a `#[cfg(target_os = "macos")]`
  menu override that keeps the default menu minus the Cmd+W close item, so the
  webview handler owns the key. Linux/WebKitGTK has no such menu — webview
  handler alone suffices (that's what headless QA exercises; the macOS branch
  is compile-checked only).
- Outside the Pages view Cmd+W becomes a no-op (previously: hide-to-tray via
  the menu on macOS). Deliberate: no surprise hides; the tray/close button
  still hides the window.

### 6. Mono-file split

`PagesView.tsx` (1322 lines) splits by responsibility, each file < ~600 lines:

- `PagesView.tsx` — state wiring, keyboard shortcuts, layout composition.
- `BlockRow.tsx` — the editable row + `SlashMenu` + kind typography.
- `PageRail.tsx` — the left rail (header, new-page, tree host, collapse
  persistence).
- `CommentCard.tsx` — the floating card (new).
- `CommentThread.tsx` — shared thread card + composers (extracted from
  `CommentsPanel.tsx`).
- `Subpages.tsx` — the subpages section (new).

## Testing

- Unit (vitest): pages-model slash filtering incl. `page`; PagesView tests
  updated for card-not-panel, add-block via mousedown, no hash button, ⌘W tab
  close; new CommentCard tests (compose, reply, resolve, close-on-escape);
  Subpages render/click; existing CommentsPanel tests keep passing against the
  extracted internals.
- E2E: drive the real headless app via the fleet/tauri-debug endpoint on this
  worktree's app. QA script: create pages, nest subpages via `/page`, edit
  blocks (enter/tab/alt-arrows/slash), comment via block button and header,
  reply/resolve, close tabs with ⌘W, click-below-append, delete pages.
- Self-heal loop ×5: themed passes (blocks/editing, comments, subpages+tree,
  tabs+keyboard, delete/edge cases) — find → fix → re-verify each pass.

## Out of scope

Inline page-link blocks (consensus), block drag-and-drop, comment anchoring
highlights, cross-page backlinks.
