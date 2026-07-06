# Docs module & view redesign

**Status:** design approved, ready for planning
**Date:** 2026-07-06
**Surface:** `crates/apps/pages`, new `crates/apps/comments`, `app/src/console/views/pages`, `app/src/domain`, `app/src/console/store`

## Goal

Turn the Docs surface from a developer-facing block-tree debugger into a Notion-grade
document editor. Six asks, delivered together:

1. Remove the block-id/hash/count clutter (hash chips, `#id` under the title, "N blocks"
   counter, "block trees"/"blocks" labels).
2. Stop showing `Type '/' for commands` on every empty block.
3. A tab system to switch between open documents.
4. Replace the "New page title" form with an instant untitled-page flow.
5. Give "All pages" a nested folder structure.
6. Add a Notion-inspired comment system.

Two additions accepted during brainstorming:

- **`DeletePage`** consensus op — there is no page delete today, and a folder tree needs one.
- **Copy-link on hover** — block-id addressability is preserved as a hover action, replacing
  the always-on hash chip (rather than dropped entirely).

## Non-goals (v1)

- Text-range comment highlights (Notion's "select text → comment"). Comments anchor to a
  whole block or the page. Range anchoring that survives concurrent edits is deferred.
- Inline "child page" blocks inside a parent's body. Nesting lives in the sidebar tree only.
- Rich text / mentions inside comments (plain text for v1).
- Drag-to-reparent in the sidebar (menu "Move to…" is the baseline; DnD is a stretch).
- Comment reactions.

## Context / constraints

- **No backwards compatibility.** Fresh genesis is the norm here (see project memory): changing
  the enumeration index format, the `CreatePage`/`PageMeta` wire, and adding a module are
  flag-day changes with no migration path. Delete, don't deprecate.
- **Target is the `pages`-backed Docs view on `dev`** (confirmed), not the in-flight
  `feat+duckfs` `document` module.
- **Server-authoritative writes.** Every edit is one consensus op; the view commits text on
  blur and before any structural op, mirroring the rest of the console.
- **Authorship is derived, never claimed.** Comments copy `chat`'s model: the module derives
  the author from `Env.origin` (`AuthorRef`), never from a write payload.

## Current architecture (what exists today)

- **Consensus module** `crates/apps/pages`: a page is a block tree, one block per qmdb key
  (`sha256(block_id)`), block ids globally unique. Page roots are flat — a root's `parent` is
  always `None`. Enumeration rides a reserved sentinel key `\0page-index` holding a sorted
  `Vec<String>` of page ids. Writes stage in an in-memory overlay, flush in one batch at
  `commit_block`. State-sync delegates to commonware's qmdb sync.
- **`chat` module** is the template for comments: `AuthorRef` derived from origin, threaded
  replies, edit/delete-by-author, per-record size caps enforced before staging, a reserved
  index, qmdb state-sync.
- **Domain client** `app/src/domain/pages-client.ts`: typed mirror of the wire; `BlockRef`
  already exists as "a stable pointer to one block".
- **Store** `app/src/console/store`: `activePage: string | null` + `activePageBlocks:
  PageBlock[]` — a single open page. `DucktapeProvider` re-fetches `listPages` + `getPage` on
  refresh; `actions.ts` has `listPages`/`createPage`/`openPage`/`insertPageBlock`/… ;
  `optimistic.ts` applies block ops locally before the snapshot lands.
- **View** `app/src/console/views/pages/PagesView.tsx` + `pages-model.ts`: the `PageRail`
  (Docs header, New-page form, flat "All pages" list), the block editor (`BlockRow`, per-kind
  fonts, slash menu, markdown shortcuts, keyboard-first editing), and the title input.

## Design

### A. Nested pages (`crates/apps/pages`)

Nesting is an **orthogonal folder relation** on top of the existing block tree. A page's
content blocks are untouched; the parent/child relation between *pages* lives only in the
enumeration index. A sub-page therefore never appears in its parent's block preorder.

**Interface changes (`interface.rs`):**

- `PageMsg::CreatePage { page_id, title, parent: Option<String> }` — `parent` names the
  containing page (a `Page` block id) or `None` for top level. Idempotent re-create ignores
  `parent` (never re-nests, never renames), as today.
- New `PageMsg::SetPageParent { page_id, parent: Option<String> }` — re-nest a page.
- New `PageMsg::DeletePage { page_id }` — remove the page root, delete its whole block subtree,
  and **promote** its direct child pages to the deleted page's parent (no cascade delete).
- `PageMeta { id, title, parent: Option<String> }` — `ListPages` now carries parent so the
  frontend derives the forest.

**Storage / logic (`lib.rs`):**

- The reserved index value changes from `Vec<String>` to a canonical
  `BTreeMap<String, Option<String>>` (page id → parent id). `serde_json` serializes a
  `BTreeMap` with sorted keys, so the bytes stay canonical and every validator lands on the
  same root. Sibling ordering is **not** stored — it is derived at read time (by title), a
  query-only concern.
- `CreatePage`: validate `parent` (when `Some`) exists and is `BlockKind::Page`; reject
  otherwise (new error `ParentPageNotFound`). Insert `page_id → parent` into the index.
- `SetPageParent`: validate the page exists and is a `Page`; validate the new parent (when
  `Some`) exists, is a `Page`, and is not the page itself or a descendant page in the folder
  forest (walk index parent pointers; reuse the `MAX_DEPTH` loud-error guard). New error
  `PageCycle`. Re-stage the index.
- `DeletePage`: require the page root exists and is a `Page`. Re-point every index entry whose
  parent is `page_id` to the deleted page's parent (promotion). Remove `page_id` from the
  index. Delete the root's block subtree depth-first (same walk as `RemoveBlock`, but permitted
  on a root here). Errors: `PageNotFound` when it isn't a page.
- The index value is still guarded by `MAX_BLOCK_LEN`; ~80 bytes/page ⇒ ≈9.5k pages ceiling,
  acceptable for v1 and unchanged in mechanism.
- `ListPages`: read the index map, join live titles from roots (unchanged), return
  `PageMeta { id, title, parent }`.

**Invariants preserved:** block ops still cannot mint/convert to `Page`; `MoveBlock`/
`RemoveBlock` still reject roots; a page root's block-`parent` stays `None` (folder parent is
separate). Cross-page block moves still rejected.

**Tests (mirror existing style):** create-with-parent; set-parent re-nests; cycle rejected;
delete promotes children and removes blocks; `ListPages` returns parents; index stays canonical
across validators (root equality); oversized index rejected before staging.

### B. Comments (new `crates/apps/comments` module)

A near-copy of the `chat`/`pages` qmdb skeleton (staging overlay, `commit_block`/`abort_block`,
`sync_target`/`sync_from`, reserved-index pattern). Registered at genesis alongside `pages`
and `chat` (trace the module registry wiring during planning — where `chat`/`pages` are
constructed and handed to the host).

**Model:**

- `Anchor { module: String, target: String }` — `module` is the pages module id (e.g.
  `"pages"`), `target` a block id or a page id. General enough to anchor comments on any
  module's addressable record later.
- `AuthorRef` — the comments interface **defines its own** copy of the enum (`User(bytes) |
  Agent{module,agent_id} | Module(String) | System`), matching chat's shape. This follows the
  convention that each module's `interface.rs` is self-contained types with no cross-crate dep;
  it is not imported from `chat`. Derived from `Env.origin`.
- `Thread { id, anchor, opener: AuthorRef, created_at, resolved: bool, resolved_by:
  Option<AuthorRef>, comment_count: u64 }`.
- `Comment { id, thread_id, author: AuthorRef, text: String, created_at, edited_at:
  Option<u64>, deleted: bool }`.

**Ops (`CommentMsg`):**

- `AddComment { thread_id, comment_id, anchor, text }` — opens the thread with `anchor` when
  `thread_id` is new (author = origin), else appends a comment (anchor must match the existing
  thread's, else reject). Bumps `comment_count`.
- `EditComment { comment_id, text }` — stored-author-only; updates `edited_at`.
- `DeleteComment { comment_id }` — stored-author-only tombstone; when it was the thread's last
  live comment, the thread record is removed (and its anchor-index entry pruned).
- `ResolveThread { thread_id, resolved }` — toggle; records `resolved_by` = origin.

**Queries (`CommentQuery` / `CommentReply`):**

- `ThreadsForAnchors { module, targets: Vec<String> }` → threads grouped by target, each with
  its comments in order. This is what a page render calls once with all visible block ids +
  the page id.
- `Thread { thread_id }` → one thread with its full comment list.

**Index:** a reserved per-anchor entry `anchor-index(module,target)` → canonical sorted
`Vec<thread_id>`, so `ThreadsForAnchors` resolves without scanning. Leading-NUL sentinel and
reserved-id rejection mirror pages' index guard. Size caps: `MAX_COMMENT_TEXT_BYTES`,
`MAX_THREADS_PER_ANCHOR`, `MAX_QUERY_TARGETS`, enforced before staging (the qmdb 1 MiB codec
cap is decode-only — an oversized committed value would poison every validator's next read).

**Lifecycle:** a deleted pages block orphans its threads (a ref may dangle, like a deleted chat
message) — no cross-module cascade. Orphaned threads simply don't render inline; a future
"all comments" view can still surface them.

**Tests (mirror chat):** open/reply; edit/delete author-enforcement (non-author rejected);
last-comment-delete removes the thread; resolve/reopen records resolver; anchor mismatch
rejected; caps rejected before staging; `ThreadsForAnchors` batching; state-sync round-trip
(byte-identical root from a peer).

### C. Frontend

**C1. Clutter removal.** Delete the per-block hash chip, the `#id`+`FinalizationMark` row under
the title, the header "N blocks" counter, and the "block trees"/"blocks" subtitles. The block
`FinalizationMark` (pending/failed consensus state) is kept but only shown while a block op is
pending/failed, not as steady chrome. Header becomes `Docs / <page title>` breadcrumb.

**C2. Copy-link on hover.** On block-row hover, a small link/`⋯` action exposes "Copy block
link" (writes the block id / a `BlockRef`-shaped handle to the clipboard). Invisible until
hover; no permanent chip. Preserves the addressability contract in the UI.

**C3. Placeholder fix.** Track the focused block. `placeholder` renders **only** when a block
is both focused and empty; copy becomes `Write, or press '/' for commands`. All other empty
blocks are silent. Per-kind placeholders (heading/list/etc.) likewise only on focus.

**C4. New-page flow.** Remove the "New page title" label + input + submit form. A single
`+ New page` button (top of the sidebar) and a per-row `+` (add child) create an untitled page
via `CreatePage { title: "", parent }`, open it in a new tab, and focus the title input with
the cursor ready. Naming happens in the document.

**C5. Tab system.** A tab strip atop the Docs main area. Store replaces `activePage` with
`openTabs: string[]` (ordered page ids) + `activeTab: string | null`. Opening a page from the
tree adds/activates its tab; `×` / middle-click closes; closing the active tab activates a
neighbor. `activePageBlocks` tracks `activeTab` and re-fetches on switch. `openTabs`/`activeTab`
persist to `localStorage` keyed per workspace so tabs survive restart. Tab labels use live page
titles ("Untitled" fallback).

**C6. Nested sidebar tree.** The flat "All pages" list becomes a collapsible tree built from
`PageMeta.parent`. A page with children renders a disclosure chevron; expand/collapse state is
local (persisted per workspace). Hover a row → `+` (add child page) and a `⋯` menu:
Rename (inline), Delete (`DeletePage`, with confirm), Move to… (a picker → `SetPageParent`).
Drag-to-reparent is a noted stretch; menu-move is the baseline. Root-level pages sort by title;
children sort by title within each parent.

**C7. Comments UI.** Per block, on hover, a speech-bubble action in the right gutter; a small
count badge when the block has live threads. Click opens that block's thread. A toggleable
right-hand **comments panel** lists every thread on the open page (grouped by block, page-level
threads first), each with resolve/reopen, a reply composer, and edit/delete on own comments.
A "Comment on page" entry in the header opens a page-anchored thread. Author names resolve
through the existing `authorName` / `AuthorNames` path (same as chat). The page render issues
one `ThreadsForAnchors` with the visible block ids + the page id; results are held in store
state (`pageThreads`, keyed by target) and refreshed after any comment op.

**Domain / store additions.**

- `pages-client.ts`: `createPage` gains `parent?`; add `setPageParent`, `deletePage`;
  `PageMeta` gains `parent`.
- New `comments-client.ts` mirroring `chat-client.ts` (typed msgs/queries, `AuthorRef`
  re-exported or shared).
- Store: tab state + reducer; `pageThreads` state; actions `addComment`, `editComment`,
  `deleteComment`, `resolveThread`, `loadPageThreads`, `setPageParent`, `deletePage`,
  `createChildPage`. `optimistic.ts` extended for comment add/resolve where it improves feel
  (optional — safe to skip and rely on refresh).

### D. Testing summary

- **Rust:** extend `pages` unit tests (§A); full new `comments` unit suite (§B) including a
  `sync_round_trip` test like `crates/apps/pages/tests/sync_round_trip.rs`.
- **TS:** update `pages-model` / `optimistic` / `PagesView` tests for the new chrome and tree;
  new tab-reducer tests; new `comments-client` + comment-store tests.
- **Real-window verification:** validate the editor, tree, tabs, and comments in the live Tauri
  window (project norm — the vite preview lacks daemon-backed data), not only unit tests.

## Rollout

Single spec → single plan → single PR into `dev` (per the "all at once" decision), built in a
worktree forked from `origin/dev` per the branching rule. The plan should still sequence the
work so it is reviewable in stages: (1) pages backend nesting, (2) comments module, (3) frontend
clutter/placeholder/new-page/tabs, (4) frontend tree + comments UI — but it merges as one branch.

## Wiring anchors (located during design)

- **Comments module registration** mirrors `pages` exactly. Host binaries construct the module
  and add it to `Host::genesis(vec![…])`:
  - `bin/node/src/main.rs` — `genesis_host` (~L609/L624) **and** `restore_host` (~L715). Add
    `let comments = Comments::init(context.child("comments"), "comments").await;` and
    `Box::new(comments)` to the registry vec.
  - `bin/noded/src/main.rs` (~L281), `bin/simnode/src/main.rs` (~L402), and `bin/demo/src/main.rs`
    if it registers pages — same two-line change each.
  - Being a qmdb (disk-substrate) module, `comments` reopens itself at its committed position on
    restore like `pages`/`document`/`kv` — it needs **no** checkpoint-snapshot install in
    `restore_host`. Confirm against how `restore_host` treats the qmdb cohort.
- **`activePage` → tab refactor touchpoints** (bounded): `console/store/state.ts` (type +
  defaults + snapshot), `console/store/actions.ts` (`enterPage` ~L334, `getState().activePage`
  ~L698, the three reset sites ~L1066/1105/1210), `console/store/DucktapeProvider.tsx` (refresh
  fetch ~L90/112–114/178, reset ~L361), `console/store/optimistic.ts` (operates on
  `activePageBlocks`), `console/views/pages/PagesView.tsx`. `activePageBlocks` stays as the
  active-tab's blocks; `activePage` becomes derived from `activeTab`.

## Open risks

- **Index growth** for pages (folder map) and comments (per-anchor) is bounded by
  `MAX_BLOCK_LEN`; fine for expected scale, revisit if pages/threads reach thousands.
- **Optimistic comments** are optional; if skipped, comment ops feel one-round-trip slow but
  stay correct. Decide during implementation.
