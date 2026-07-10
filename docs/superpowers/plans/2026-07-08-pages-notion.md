# Notion-like Pages Overhaul Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Pages surface Notion-like: floating per-block comment card, working add-block affordance, endless canvas, in-editor subpages, ⌘W closes the doc tab — and split the 1322-line `PagesView.tsx` mono-file.

**Architecture:** App-only changes in `app/src/console/views/pages/` plus one macOS-gated menu module in the Tauri shell. No node/consensus changes — subpages ride the existing `create_page{parent}` / `PageMeta.parent` wire. Spec: `docs/superpowers/specs/2026-07-08-pages-notion-design.md`.

**Tech Stack:** React 18 + inline styles from `theme/tokens`, vitest + @testing-library/react, Tauri v2 (Rust shell), bun.

## Global Constraints

- Every file < ~600 lines (repo mandate: no mono-files).
- No changes under `crates/` or `bin/` (consensus safety); `app/src-tauri` only for the macOS menu.
- Existing store actions (`ConsoleActions`) are the only data surface — no new transport calls.
- Tests: run from `app/` with `bun run test -- <file>`; full suite `bun run test` + `bun run typecheck` must pass before PR.
- Working dir: `/home/eddy/dev/ducktape/.claude/worktrees/pages-notion` (branch `feat/pages-notion`).
- All aria-labels referenced in tests must match exactly.

---

### Task 0: Worktree deps

**Files:** none (setup)

- [ ] **Step 1:** `cd app && bun install` (fresh worktree has no `node_modules`).
- [ ] **Step 2:** Baseline: `bun run test -- src/console/views/pages` → all pass; `bun run typecheck` → clean.

### Task 1: Split PagesView.tsx (pure move, no behavior change)

**Files:**
- Create: `app/src/console/views/pages/BlockRow.tsx`
- Create: `app/src/console/views/pages/PageRail.tsx`
- Modify: `app/src/console/views/pages/PagesView.tsx`
- Modify: `app/src/console/views/pages/pages-model.ts` (receives `EDIT_BOUNDARY_MS`)
- Test: existing `PagesView.test.tsx` (unchanged, must stay green)

**Interfaces:**
- Produces `BlockRow.tsx`: `export interface RowHandlers` (verbatim from PagesView incl. `openComments`), `export function BlockRow({row,index,expanded,op,threadCount,handlers})`, plus private `SlashMenu`, `kindFont`, `focusPlaceholder`.
- Produces `PageRail.tsx`: `export function PageRail({pages,activePage,onNewPage,onAddChild,onOpen,onDelete,onMove,onRefresh})` — tree-collapse state (`TREE_COLLAPSE_KEY`, load/save, `useState`) moves INSIDE PageRail; the `collapsed`/`onToggleCollapse` props disappear from its signature.
- Produces `pages-model.ts`: `export const EDIT_BOUNDARY_MS = 700;`
- `PagesView.tsx` re-exports for test compat: `export { EDIT_BOUNDARY_MS } from "./pages-model";`

- [ ] **Step 1:** Move `EDIT_BOUNDARY_MS` (PagesView.tsx:50) into `pages-model.ts`; re-export from PagesView.
- [ ] **Step 2:** Create `BlockRow.tsx` with `kindFont`, `focusPlaceholder`, `SlashMenu`, `RowHandlers`, `BlockRow`, `INDENT`, `sectionLabelStyle` only if used (it isn't — leave it), importing `EDIT_BOUNDARY_MS` from `./pages-model`. Delete those from PagesView; import `{ BlockRow }` and `type { RowHandlers }` from `./BlockRow`.
- [ ] **Step 3:** Create `PageRail.tsx` with `PageRail`, `sectionLabelStyle`, `TREE_COLLAPSE_KEY`, `loadTreeCollapsed`, `saveTreeCollapsed`; own the collapse state internally (`useState(loadTreeCollapsed)` + toggle that persists). Remove `treeCollapsed` state + `toggleTreeCollapse` from PagesView and drop the two props at the call site.
- [ ] **Step 4:** `bun run test -- src/console/views/pages && bun run typecheck` → green. `wc -l PagesView.tsx` < 700.
- [ ] **Step 5:** Commit `refactor(pages): split PagesView mono-file into BlockRow + PageRail`.

### Task 2: Extract shared CommentThread.tsx from CommentsPanel

**Files:**
- Create: `app/src/console/views/pages/CommentThread.tsx`
- Modify: `app/src/console/views/pages/CommentsPanel.tsx`
- Test: existing `CommentsPanel.test.tsx` (unchanged, must stay green)

**Interfaces:**
- Produces `CommentThread.tsx`:
  - `export interface ComposerTarget { target: string; label: string }` (moves from CommentsPanel)
  - `export function NewThreadComposer({ composer, onSubmit, onCancel })`
  - `export function ThreadCard({ view, authorNames, onReply, onResolve, onEdit, onDelete })`
  - style consts stay private to this file.
- `CommentsPanel.tsx` keeps its public signature, imports the three from `./CommentThread`, and re-exports `export type { ComposerTarget } from "./CommentThread";`

- [ ] **Step 1:** Move `NewThreadComposer`, `ThreadCard`, `miniBtn/composerStyle/primaryBtn/ghostBtn`, `ComposerTarget` into `CommentThread.tsx`; update CommentsPanel imports.
- [ ] **Step 2:** `bun run test -- src/console/views/pages/CommentsPanel.test.tsx && bun run typecheck` → green.
- [ ] **Step 3:** Commit `refactor(pages): extract shared comment thread components`.

### Task 3: CommentCard — the floating card (TDD)

**Files:**
- Create: `app/src/console/views/pages/CommentCard.tsx`
- Test: Create `app/src/console/views/pages/CommentCard.test.tsx`

**Interfaces:**
- Produces:
  ```ts
  export interface CommentAnchor { x: number; y: number }  // viewport coords, card renders near it
  export function CommentCard({ target, label, anchor, threads, authorNames,
    onClose, onSubmitNew, onReply, onResolve, onEdit, onDelete }: {
    target: string;
    label: string;               // "this page" | "this block"
    anchor: CommentAnchor;
    threads: ThreadView[];       // threads for THIS target only
    authorNames: AuthorNames;
    onClose: () => void;
    onSubmitNew: (target: string, text: string) => void;
    onReply: (threadId: string, text: string) => void;
    onResolve: (threadId: string, resolved: boolean) => void;
    onEdit: (commentId: string, text: string) => void;
    onDelete: (commentId: string) => void;
  })
  ```
- Consumes `ThreadCard`, `NewThreadComposer`, `ComposerTarget` from `./CommentThread` (Task 2).

Behavior contract (each gets a test):
1. renders `role="dialog"` `aria-label={`Comments on ${label}`}`, `position: fixed`, width 340, maxHeight `min(480px, 70vh)`, left/top clamped into the viewport from `anchor` (left = `max(8, min(anchor.x - 340, innerWidth - 348))`, top = `max(8, min(anchor.y + 8, innerHeight - 120))`), internal `overflowY: auto`, `border`, `radius.lg`, `background: color.paper`, `boxShadow: shadow.card`, `zIndex: 40`.
2. `threads.length === 0` → `NewThreadComposer` rendered immediately (autofocus textarea, label `New comment on ${label}`); submitting calls `onSubmitNew(target, text)`; its Cancel calls `onClose`.
3. `threads.length > 0` → ThreadCards render; composer hidden behind a `+ Add comment` button (aria-label "Add comment thread"); clicking reveals `NewThreadComposer` (whose Cancel now just hides it, not `onClose`).
4. Escape anywhere → `onClose`. Outside `mousedown` → `onClose`; inside mousedown → not closed.
5. document scroll (capture) whose target is OUTSIDE the card → `onClose`; the card's own internal scroll does NOT close (filter `event.target` containment).

- [ ] **Step 1:** Write `CommentCard.test.tsx` covering contracts 1–5 (render with fake `ThreadView`s as in CommentsPanel.test.tsx; `fireEvent.keyDown(document, { key: "Escape" })`; `fireEvent.mouseDown(document.body)`; `fireEvent.scroll(document)`).
- [ ] **Step 2:** `bun run test -- src/console/views/pages/CommentCard.test.tsx` → FAIL (module missing).
- [ ] **Step 3:** Implement `CommentCard.tsx` (~170 lines): the dialog with header (label + count + close button aria-label "Close comments card"), thread list, composer logic; `useEffect` document listeners (keydown Escape, mousedown outside via ref, scroll capture with target filter), cleaned up on unmount.
- [ ] **Step 4:** Test → PASS. `bun run typecheck` → green.
- [ ] **Step 5:** Commit `feat(pages): floating comment card component`.

### Task 4: Wire the card into PagesView; retire panel-on-click; remove copy-block-link

**Files:**
- Modify: `app/src/console/views/pages/PagesView.tsx`
- Modify: `app/src/console/views/pages/BlockRow.tsx`
- Test: Modify `app/src/console/views/pages/PagesView.test.tsx`

**Interfaces:**
- `RowHandlers.openComments` becomes `openComments(blockId: string, anchor: { x: number; y: number }): void` — BlockRow's comment button passes `{ x: rect.left, y: rect.bottom }` from `event.currentTarget.getBoundingClientRect()`.
- PagesView state: `const [commentCard, setCommentCard] = useState<{ target: string; label: string; anchor: CommentAnchor } | null>(null);` replaces `composerTarget`.
- CommentsPanel now always receives `composer={null}` (its composer path is retired from the view; component API untouched).

- [ ] **Step 1:** Update PagesView tests:
  - block comment button click → a `role="dialog"` named `Comments on this block` appears; `openComments` panel (`aria-label="Comments"` aside) does NOT appear.
  - header "Comment" button → dialog named `Comments on this page`.
  - header "Comments" toggle still opens the aside panel (unchanged assertion).
  - `queryByRole("button", { name: /Copy link to block/ })` → null (was: present on hover).
  - submitting the card composer calls `addComment({ target, text })`; Escape closes the dialog.
- [ ] **Step 2:** Run → FAIL.
- [ ] **Step 3:** Implement:
  - BlockRow: comment button `onClick={(e) => handlers.openComments(block.id, { x: e.currentTarget.getBoundingClientRect().left, y: e.currentTarget.getBoundingClientRect().bottom })}`. Delete the entire hash/copy-link button block.
  - PagesView: `openBlockComments(blockId, anchor)` → `setCommentCard({ target: blockId, label: "this block", anchor })`; `commentOnPage(anchor)` → `{ target: activePage, label: "this page", anchor }`. Render `<CommentCard …threads={state.pageThreads.find(g => g.target === commentCard.target)?.threads ?? []} onClose={() => setCommentCard(null)} onSubmitNew={(target, text) => { actions.addComment({ target, text }); }} …/>` after the doc body; page-switch effect clears `commentCard` (replaces the old `setComposerTarget(null)`).
  - Panel: `composer={null}`, drop `onSubmitNew/onCancelNew` wiring to composer state (keep panel's own props satisfied: `onSubmitNew` can keep calling `addComment`, `onCancelNew` a no-op arrow).
- [ ] **Step 4:** `bun run test -- src/console/views/pages && bun run typecheck` → green.
- [ ] **Step 5:** Commit `feat(pages): block comments open a floating card, drop copy-block-link`.

### Task 5: Clickable add-block + endless canvas

**Files:**
- Modify: `app/src/console/views/pages/PagesView.tsx`
- Test: Modify `app/src/console/views/pages/PagesView.test.tsx`

- [ ] **Step 1:** Tests:
  - `fireEvent.mouseDown(getByRole("button", { name: "Add a block" }))` → `insertPageBlock` called with `{ parent: "p1", after: "b", kind: "paragraph", text: "" }` (blockId is a fresh uuid — assert `expect.objectContaining`). Plain `fireEvent.click` no longer needs to fire it (mousedown is the trigger).
  - the canvas filler (`data-testid="page-canvas-filler"`) exists when a page is open; `fireEvent.mouseDown(filler)` → same `insertPageBlock` shape.
  - no bordered card: the doc scroll container has `background: color.paper` (assert via style on `data-testid="doc-scroll"` if needed — prefer behavioral asserts; a light style assert on background is acceptable here).
- [ ] **Step 2:** Run → FAIL.
- [ ] **Step 3:** Implement in PagesView:
  - Scroll container: `background: color.paper`, `display: "flex"`, `flexDirection: "column"`, keep `overflowY: auto`; add `data-testid="doc-scroll"`. On scroll the CommentCard already closes via its own capture listener (no wiring needed).
  - Replace the bordered card div (`maxWidth: 820, border, borderRadius, boxShadow, background, padding`) with a plain content column: `{ width: "100%", maxWidth: 820, margin: "0 auto", padding: "36px 44px 0", boxSizing: "border-box" }` — no border/shadow/minHeight.
  - "Add a block" button: replace `onClick={appendBlock}` with `onMouseDown={(e) => { e.preventDefault(); appendBlock(); }}` (blur-commit dodge, same as SlashMenu) and reduce bottom padding to `8px 0 8px 28px`.
  - After the content column, sibling filler: `<div data-testid="page-canvas-filler" aria-hidden="true" onMouseDown={(e) => { e.preventDefault(); appendBlock(); }} style={{ flex: 1, minHeight: "40vh", cursor: "text" }} />` rendered only when `root`.
  - Empty state (`!root`) branch unchanged.
- [ ] **Step 4:** `bun run test -- src/console/views/pages && bun run typecheck` → green.
- [ ] **Step 5:** Commit `feat(pages): endless canvas — mousedown add-block + click-below-to-append, drop card chrome`.

### Task 6: Subpages — `/page` slash command + Subpages section

**Files:**
- Modify: `app/src/console/views/pages/pages-model.ts`
- Modify: `app/src/console/views/pages/BlockRow.tsx`
- Modify: `app/src/console/views/pages/PagesView.tsx`
- Create: `app/src/console/views/pages/Subpages.tsx`
- Test: Modify `pages-model.test.ts`, `PagesView.test.tsx`; create tests inside `PagesView.test.tsx` (Subpages renders inside the view — no separate file needed for a ~60-line presentational component).

**Interfaces:**
- `pages-model.ts`: `SLASH_KINDS` gains `{ kind: "page", label: "Page", hint: "new subpage" }` as the LAST entry ("page" is a legal `BlockKind`; ordering keeps text/heading muscle memory).
- `RowHandlers` gains `createSubpage(): void`.
- Produces `Subpages.tsx`:
  ```ts
  export function Subpages({ pages, activePage, onOpen }: {
    pages: PageMeta[]; activePage: string; onOpen: (id: string) => void;
  })
  ```
  Renders null when no `pages` entry has `parent === activePage`; otherwise a section labeled "Subpages" with one row per child: `aria-label={`Open subpage ${title || "Untitled"}`}`, pages icon + title, `onClick={() => onOpen(id)}`.

- [ ] **Step 1:** pages-model test: `filterSlashKinds("pag")` contains kind "page"; `filterSlashKinds("")` still returns every entry (now 13). Run → FAIL. Add the entry. → PASS.
- [ ] **Step 2:** BlockRow: in `pickSlash`, special-case before `setKind`:
  ```ts
  const pickSlash = (kind: BlockKind) => {
    setDraft("");
    setSlashDismissed(false);
    if (kind === "page") {
      handlers.createSubpage();
      return;
    }
    if (kind !== block.kind) handlers.setKind(block.id, kind);
  };
  ```
- [ ] **Step 3:** PagesView: `handlers.createSubpage = () => actions.createChildPage(state.activePage)` (existing action creates untitled child + opens its tab, cursor in title). Render `<Subpages pages={state.pages} activePage={root.id} onOpen={actions.openPage} />` between the title input and the block rows.
- [ ] **Step 4:** PagesView tests:
  - with `pages` containing `{ id: "p3", title: "Child", parent: "p1" }` → button `Open subpage Child` renders; click → `openPage("p3")`.
  - with no children → `queryByText("Subpages")` null.
  - type `/` into block a's textarea (`Edit paragraph block 1`), pick option "Page" via mousedown → `createChildPage` called with `"p1"`, and `setPageBlockKind` NOT called.
- [ ] **Step 5:** `bun run test -- src/console/views/pages && bun run typecheck` → green.
- [ ] **Step 6:** Commit `feat(pages): Notion-like subpages — /page slash command + Subpages section`.

### Task 7: ⌘/Ctrl+W closes the active doc tab

**Files:**
- Modify: `app/src/console/views/pages/PagesView.tsx` (keydown handler + its header comment)
- Test: Modify `PagesView.test.tsx`

- [ ] **Step 1:** Test: render with `openTabs: ["p1", "p2"], activePage: "p1"`; `fireEvent.keyDown(document, { code: "KeyW", metaKey: true })` → `spies.closeTab` with `"p1"`; with `activePage: null, openTabs: []` → not called. Also ctrlKey variant. Run → FAIL.
- [ ] **Step 2:** In the Docs-scoped `onKey` (after the bracket handling, before T/N):
  ```ts
  if (!event.shiftKey && event.code === "KeyW") {
    if (!state.activePage) return;
    event.preventDefault();
    actions.closeTab(state.activePage);
    return;
  }
  ```
  Rewrite the comment block: ⌘W now closes the active doc tab; with none open it falls through untouched.
- [ ] **Step 3:** Tests + typecheck green.
- [ ] **Step 4:** Commit `feat(pages): cmd/ctrl+W closes the active doc tab`.

### Task 8: macOS menu — stop "Close Window" from eating Cmd+W

**Files:**
- Create: `app/src-tauri/src/menu.rs`
- Modify: `app/src-tauri/src/main.rs` (mod decl + setup hook)

**Interfaces:** `menu::install(app: &tauri::App) -> tauri::Result<()>` — no-op body on non-macOS.

- [ ] **Step 1:** `menu.rs`:
  ```rust
  //! macOS app menu. Tauri's default menu binds Cmd+W to Close Window, which
  //! swallows the key before the webview can close a doc tab (the window
  //! itself only hides to the tray anyway — see `tray.rs`). Rebuild the menu
  //! without that item: app + Edit (system clipboard bindings) + Window,
  //! no Close Window accelerator. Other platforms have no default menu.

  #[cfg(target_os = "macos")]
  pub fn install(app: &tauri::App) -> tauri::Result<()> {
      use tauri::menu::{AboutMetadata, Menu, PredefinedMenuItem, Submenu};
      let handle = app.handle();
      let name = &app.package_info().name;
      let app_menu = Submenu::with_items(
          handle,
          name,
          true,
          &[
              &PredefinedMenuItem::about(handle, None, Some(AboutMetadata::default()))?,
              &PredefinedMenuItem::separator(handle)?,
              &PredefinedMenuItem::services(handle, None)?,
              &PredefinedMenuItem::separator(handle)?,
              &PredefinedMenuItem::hide(handle, None)?,
              &PredefinedMenuItem::hide_others(handle, None)?,
              &PredefinedMenuItem::show_all(handle, None)?,
              &PredefinedMenuItem::separator(handle)?,
              &PredefinedMenuItem::quit(handle, None)?,
          ],
      )?;
      let edit = Submenu::with_items(
          handle,
          "Edit",
          true,
          &[
              &PredefinedMenuItem::undo(handle, None)?,
              &PredefinedMenuItem::redo(handle, None)?,
              &PredefinedMenuItem::separator(handle)?,
              &PredefinedMenuItem::cut(handle, None)?,
              &PredefinedMenuItem::copy(handle, None)?,
              &PredefinedMenuItem::paste(handle, None)?,
              &PredefinedMenuItem::select_all(handle, None)?,
          ],
      )?;
      let window = Submenu::with_items(
          handle,
          "Window",
          true,
          &[
              &PredefinedMenuItem::minimize(handle, None)?,
              &PredefinedMenuItem::maximize(handle, None)?,
              &PredefinedMenuItem::separator(handle)?,
              &PredefinedMenuItem::fullscreen(handle, None)?,
          ],
      )?;
      app.set_menu(Menu::with_items(handle, &[&app_menu, &edit, &window])?)?;
      Ok(())
  }

  #[cfg(not(target_os = "macos"))]
  pub fn install(_app: &tauri::App) -> tauri::Result<()> {
      Ok(())
  }
  ```
- [ ] **Step 2:** main.rs: add `mod menu;`, and in `.setup(...)` after `tray::init`: `menu::install(app)?;`
- [ ] **Step 3:** `cd app/src-tauri && cargo check` → green (checks the non-mac path; the mac branch is compile-risk accepted — flagged in the PR; mirror-of-docs API only).
- [ ] **Step 4:** Commit `feat(shell): macOS menu without Cmd+W Close Window so the webview owns the key`.

### Task 9: Full verification

- [ ] **Step 1:** `cd app && bun run test` → all green; `bun run typecheck` → clean.
- [ ] **Step 2:** `cargo check` for the shell (`app/src-tauri`) and `cargo fmt --check` on touched Rust.
- [ ] **Step 3:** Commit anything outstanding; then e2e QA (goal item 7) via the qa/fleet skill: 5 themed self-heal passes — (1) block editing incl. add-block/canvas, (2) comments card+panel, (3) subpages `/page` + rail tree, (4) tabs + ⌘W/⌘⇧[] keys, (5) delete/edge cases — fix → re-verify each.

## Self-Review

- Spec coverage: 1→Tasks 3–4, 2→Task 5, 3→Task 4, 4→Task 5, 5→Task 6, 6→Tasks 7–8, split→Tasks 1–2, QA→Task 9. No gaps.
- Placeholders: none — every code step carries the code or an exact move boundary.
- Type consistency: `CommentAnchor {x,y}` used in Tasks 3–4; `RowHandlers.openComments(blockId, anchor)` and `createSubpage()` consistent across 4/6; `Subpages({pages, activePage, onOpen})` matches call site.
