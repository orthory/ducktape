# Docs Module & View Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the Docs surface into a Notion-grade editor — nested pages (folder tree), a comment system, a document tab bar, an instant new-page flow, and removal of the block-id/placeholder clutter.

**Architecture:** Two consensus-layer changes (extend `crates/apps/pages` for page nesting; add a new `crates/apps/comments` module modeled on `crates/apps/chat`) plus a frontend overhaul of `app/src/console/views/pages` with supporting domain clients and store state. Page nesting is an orthogonal folder relation stored in the pages enumeration index — block trees are untouched. Comments anchor to a block or page via `{module,target}`, with authorship derived from the dispatch origin.

**Tech Stack:** Rust (commonware qmdb modules, `async_trait`, serde_json wire), TypeScript/React (Vite console, no external state lib — a hand-rolled store with `patch`/`submitTracked`/optimistic reducers), Vitest.

## Global Constraints

- **No backwards compatibility** — fresh genesis is the norm. Changing the pages index format, `CreatePage`/`PageMeta` wire, and adding a module are flag-day changes; no migration code. Delete, don't deprecate.
- **Authorship is derived, never claimed** — comment authors come from `Env.origin` (`AuthorRef`), never a write payload (mirror `crates/apps/chat`).
- **Write-time size caps before staging** — the qmdb codec's 1 MiB cap is decode-only; an oversized committed value poisons every validator's next read. Every module enforces a byte cap before `stage`.
- **One consensus op per user change** — the view commits text on blur and before any structural op.
- **Reserved index sentinel** — the pages/comments enumeration indices ride a leading-NUL logical key; every op naming it is rejected before any storage touch.
- **Module id strings** — pages module id is `"pages"`; the new comments module id is `"comments"`.
- **Placeholder copy** — the focused-empty-block placeholder text is exactly `Write, or press '/' for commands`.
- **Commit signing** — this repo hangs on SSH commit signing; commit with `git commit --no-gpg-sign`.
- **Branch** — all work targets `dev`; do it in a worktree forked from `origin/dev`, merged back via one PR.

---

## Phase 1 — Pages backend: nested pages

Reference the spec: `docs/superpowers/specs/2026-07-06-docs-module-redesign-design.md` §A.

### Task 1: Pages interface — nesting wire types

**Files:**
- Modify: `crates/apps/pages/src/interface.rs`

**Interfaces:**
- Produces: `PageMsg::CreatePage { page_id: String, title: String, parent: Option<String> }`, `PageMsg::SetPageParent { page_id: String, parent: Option<String> }`, `PageMsg::DeletePage { page_id: String }`, `PageMeta { id: String, title: String, parent: Option<String> }`.

- [ ] **Step 1: Write the failing test** — append to the `#[cfg(test)]` block at the bottom of `crates/apps/pages/src/interface.rs` (add one if none exists there; the module's behavior tests live in `lib.rs`, so a small serde test here is fine):

```rust
#[cfg(test)]
mod interface_tests {
    use super::*;

    #[test]
    fn create_page_carries_optional_parent() {
        let m = PageMsg::CreatePage {
            page_id: "p2".into(),
            title: "child".into(),
            parent: Some("p1".into()),
        };
        let round: PageMsg = decode_msg(&encode_msg(&m)).unwrap();
        assert_eq!(round, m);
        // top-level create serializes parent as null.
        let top = PageMsg::CreatePage { page_id: "p1".into(), title: "root".into(), parent: None };
        assert!(String::from_utf8(encode_msg(&top)).unwrap().contains("\"parent\":null"));
    }

    #[test]
    fn set_parent_and_delete_round_trip() {
        for m in [
            PageMsg::SetPageParent { page_id: "p2".into(), parent: None },
            PageMsg::DeletePage { page_id: "p2".into() },
        ] {
            assert_eq!(decode_msg(&encode_msg(&m)).unwrap(), m);
        }
    }

    #[test]
    fn page_meta_carries_parent() {
        let meta = PageMeta { id: "p2".into(), title: "t".into(), parent: Some("p1".into()) };
        let bytes = serde_json::to_vec(&meta).unwrap();
        let back: PageMeta = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back, meta);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pages interface_tests 2>&1 | tail -20`
Expected: FAIL to compile — `CreatePage` has no `parent` field, `SetPageParent`/`DeletePage` unknown, `PageMeta` has no `parent`.

- [ ] **Step 3: Add the `parent` field and new variants.** In `crates/apps/pages/src/interface.rs`, change the `CreatePage` variant and add two variants to `PageMsg`:

```rust
    /// create a page: a root block of kind `Page` whose text is `title`.
    /// `parent`, when `Some`, nests this page under another page (a folder
    /// relation stored only in the enumeration index — content blocks are
    /// untouched). idempotent: re-creating an existing page is a benign no-op
    /// that changes neither the title NOR the parent.
    CreatePage {
        page_id: String,
        title: String,
        parent: Option<String>,
    },
```

Add after `RemoveBlock`:

```rust
    /// re-nest a page under a (possibly new) parent page, or to top level with
    /// `None`. rejected when the target is not a page root, the parent is not a
    /// page, or the move would form a cycle in the folder forest.
    SetPageParent {
        page_id: String,
        parent: Option<String>,
    },
    /// delete a page: remove its root and whole block subtree, and PROMOTE its
    /// direct child pages to the deleted page's parent (no cascade). rejected
    /// when the id is not a page root.
    DeletePage { page_id: String },
```

- [ ] **Step 4: Add `parent` to `PageMeta`:**

```rust
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PageMeta {
    pub id: String,
    pub title: String,
    /// the containing page id (folder parent), or `None` for a top-level page.
    pub parent: Option<String>,
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p pages interface_tests 2>&1 | tail -20`
Expected: the three tests pass. `lib.rs` will not compile yet (its `CreatePage` uses, `PageMeta` construction, and match arms are now incomplete) — that is fixed in Task 2. If `cargo test -p pages` blocks the interface tests from running due to lib errors, temporarily run only this file's doctest-free unit via `cargo test -p pages --lib interface_tests` after Task 2; for now compile-check the interface with `cargo check -p pages --lib 2>&1 | tail -30` and confirm the only errors are in `lib.rs`.

- [ ] **Step 6: Commit**

```bash
git add crates/apps/pages/src/interface.rs
git commit --no-gpg-sign -m "feat(pages): nesting wire — CreatePage.parent, SetPageParent, DeletePage, PageMeta.parent"
```

---

### Task 2: Pages index → parent map; CreatePage parent; ListPages parent

**Files:**
- Modify: `crates/apps/pages/src/lib.rs`

**Interfaces:**
- Consumes: Task 1 wire types.
- Produces: index stored as canonical `BTreeMap<String, Option<String>>`; `load_index`/`index_add(page_id, parent)` helpers; `CreatePage` validates + records parent; `ListPages` returns `PageMeta.parent`; new `PageError::ParentPageNotFound`.

- [ ] **Step 1: Write the failing test** — add to the `tests` mod in `crates/apps/pages/src/lib.rs`. Also update the existing `seed_page` helper's `CreatePage` call to pass `parent: None` (do this now so the file compiles):

```rust
    #[test]
    fn create_with_parent_records_folder_edge() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            apply_commit(&mut p, &PageMsg::CreatePage {
                page_id: "root".into(), title: "Root".into(), parent: None,
            }).await;
            apply_commit(&mut p, &PageMsg::CreatePage {
                page_id: "child".into(), title: "Child".into(), parent: Some("root".into()),
            }).await;
            let pages = list_pages(&p).await;
            let child = pages.iter().find(|m| m.id == "child").unwrap();
            assert_eq!(child.parent.as_deref(), Some("root"));
            let root = pages.iter().find(|m| m.id == "root").unwrap();
            assert_eq!(root.parent, None);
        });
    }

    #[test]
    fn create_under_missing_or_nonpage_parent_is_rejected() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            seed_page(&mut p, "p1").await; // p1 + blocks b1,b2,b3
            // parent does not exist
            apply_expect_err(&mut p, &PageMsg::CreatePage {
                page_id: "x".into(), title: "x".into(), parent: Some("ghost".into()),
            }, "parent page not found").await;
            // parent exists but is a non-page block
            apply_expect_err(&mut p, &PageMsg::CreatePage {
                page_id: "y".into(), title: "y".into(), parent: Some("b1".into()),
            }, "parent page not found").await;
        });
    }
```

Every other `PageMsg::CreatePage { .. }` literal already in the `lib.rs` test module (there are several, plus `seed_page` and `list_pages_enumerates_sorted_with_live_titles`) must gain `parent: None`. **Also** update the 3 `CreatePage` literals in the integration test file `crates/apps/pages/tests/sync_round_trip.rs` (lines ~110, ~127-ish, ~181) to add `parent: None`, or `cargo test -p pages` fails to compile that file.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pages 2>&1 | tail -30`
Expected: compile errors (index type mismatch / missing arm) then, once compiling, FAIL on the new tests.

- [ ] **Step 3: Switch the index to a parent map.** In `crates/apps/pages/src/lib.rs` replace the `load_index`/`index_add` helpers and the `PAGE_INDEX_KEY` doc to hold `BTreeMap<String, Option<String>>`:

```rust
    /// load the enumeration index — page id → folder parent — through the
    /// staged-over-committed overlay. absent reads as the empty map; a decode
    /// failure is corruption. `BTreeMap` serializes with SORTED keys, so the
    /// bytes are canonical and every validator commits the same index root.
    async fn load_index(&self) -> Result<BTreeMap<String, Option<String>>, Error> {
        match self.get(PAGE_INDEX_KEY.as_bytes()).await {
            Some(b) => serde_json::from_slice(&b).map_err(|e| Error::Module(e.to_string())),
            None => Ok(BTreeMap::new()),
        }
    }

    /// re-stage the whole index map (canonical serialization).
    fn stage_index(&mut self, index: &BTreeMap<String, Option<String>>) -> Result<(), PageError> {
        let bytes = serde_json::to_vec(index).expect("index is always serializable");
        self.stage(PAGE_INDEX_KEY, bytes)
    }

    /// add `page_id -> parent` to the index if absent (idempotent create keeps
    /// the existing entry, so re-create never re-nests).
    async fn index_add(&mut self, page_id: &str, parent: Option<String>) -> Result<(), PageError> {
        let mut index = self.load_index().await.map_err(to_page_err)?;
        if !index.contains_key(page_id) {
            index.insert(page_id.to_string(), parent);
            self.stage_index(&index)?;
        }
        Ok(())
    }
```

`BTreeMap` is already imported (`use std::collections::BTreeMap;`). Add `PageError::ParentPageNotFound` to the enum and its `Display` arm (`"parent page not found"`).

- [ ] **Step 4: Validate + record parent in `CreatePage`.** Replace the `PageMsg::CreatePage` arm in `apply`:

```rust
            PageMsg::CreatePage { page_id, title, parent } => {
                match self.load_block(&page_id).await.map_err(to_page_err)? {
                    Some(b) if b.kind == BlockKind::Page => Ok(()), // idempotent no-op
                    Some(_) => Err(PageError::DuplicateBlock),
                    None => {
                        // a named parent must exist AND be a page root.
                        if let Some(par) = &parent {
                            match self.load_block(par).await.map_err(to_page_err)? {
                                Some(b) if b.kind == BlockKind::Page => {}
                                _ => return Err(PageError::ParentPageNotFound),
                            }
                        }
                        self.index_add(&page_id, parent).await?;
                        self.store_block(&Block {
                            id: page_id.clone(),
                            parent: None, // block-parent stays None; folder parent is in the index
                            page: page_id,
                            kind: BlockKind::Page,
                            text: title,
                            checked: false,
                            children: Vec::new(),
                        })
                    }
                }
            }
```

Also extend the reserved-sentinel `named` guard tuple for the new variants (Step in Task 3/4 adds their arms; for now `CreatePage` still contributes `[page_id, ""]` — unchanged).

- [ ] **Step 4b: Keep the module compiling** — Task 1 added `SetPageParent`/`DeletePage` to `PageMsg`, so both the reserved-sentinel `named` match and the main `match msg` in `apply` are now non-exhaustive. Add TEMPORARY stubs so `lib.rs` compiles and Task 2's tests can run; Tasks 3–4 replace them. In the `named` match, add:

```rust
            PageMsg::SetPageParent { page_id, parent } => {
                [page_id.as_str(), parent.as_deref().unwrap_or("")]
            }
            PageMsg::DeletePage { page_id } => [page_id.as_str(), ""],
```

In the main `match msg` in `apply`, add (returns a benign error until Tasks 3–4):

```rust
            PageMsg::SetPageParent { .. } | PageMsg::DeletePage { .. } => {
                Err(PageError::Corrupt) // stub — real logic in Tasks 3–4
            }
```

- [ ] **Step 5: Return parent from `ListPages`.** In the `PageQuery::ListPages` arm of `query`, iterate the map and read live titles:

```rust
            PageQuery::ListPages => {
                let index = self.load_index().await?;
                let mut pages = Vec::with_capacity(index.len());
                for (id, parent) in index {
                    let root = self
                        .load_block(&id)
                        .await?
                        .filter(|b| b.kind == BlockKind::Page)
                        .ok_or_else(|| Error::Module(PageError::Corrupt.to_string()))?;
                    pages.push(PageMeta { id, title: root.text, parent });
                }
                Ok(encode_reply(&PageReply::PageList(pages)))
            }
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p pages 2>&1 | tail -30`
Expected: all pages tests pass, including the two new ones and the pre-existing `list_pages_enumerates_sorted_with_live_titles` (BTreeMap iteration is sorted by id, preserving that ordering).

- [ ] **Step 7: Commit**

```bash
git add crates/apps/pages/src/lib.rs
git commit --no-gpg-sign -m "feat(pages): parent-map index; CreatePage validates+records folder parent; ListPages returns parent"
```

---

### Task 3: `SetPageParent` with cycle rejection

**Files:**
- Modify: `crates/apps/pages/src/lib.rs`

**Interfaces:**
- Consumes: index parent map from Task 2.
- Produces: `SetPageParent` apply arm; `PageError::NotAPage`, `PageError::PageCycle`; `folder_ancestry_excludes` helper.

- [ ] **Step 1: Write the failing test** (append to `tests` mod):

```rust
    #[test]
    fn set_page_parent_renests_and_rejects_cycles() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            for id in ["a", "b", "c"] {
                apply_commit(&mut p, &PageMsg::CreatePage {
                    page_id: id.into(), title: id.into(), parent: None,
                }).await;
            }
            // b under a, c under b.
            apply_commit(&mut p, &PageMsg::SetPageParent { page_id: "b".into(), parent: Some("a".into()) }).await;
            apply_commit(&mut p, &PageMsg::SetPageParent { page_id: "c".into(), parent: Some("b".into()) }).await;
            let parent_of = |pages: &[PageMeta], id: &str| pages.iter().find(|m| m.id == id).unwrap().parent.clone();
            let pages = list_pages(&p).await;
            assert_eq!(parent_of(&pages, "b"), Some("a".into()));
            assert_eq!(parent_of(&pages, "c"), Some("b".into()));
            // a under c would cycle (a -> c -> b -> a).
            apply_expect_err(&mut p, &PageMsg::SetPageParent { page_id: "a".into(), parent: Some("c".into()) }, "page cycle").await;
            // self-parent cycles too.
            apply_expect_err(&mut p, &PageMsg::SetPageParent { page_id: "a".into(), parent: Some("a".into()) }, "page cycle").await;
            // detach to top level.
            apply_commit(&mut p, &PageMsg::SetPageParent { page_id: "b".into(), parent: None }).await;
            assert_eq!(parent_of(&list_pages(&p).await, "b"), None);
            // target must be a page root.
            seed_page(&mut p, "pg").await;
            apply_expect_err(&mut p, &PageMsg::SetPageParent { page_id: "b1".into(), parent: None }, "not a page").await;
            // parent must be a page.
            apply_expect_err(&mut p, &PageMsg::SetPageParent { page_id: "a".into(), parent: Some("b1".into()) }, "parent page not found").await;
        });
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pages set_page_parent 2>&1 | tail -20`
Expected: compile error (missing `SetPageParent` arm), then FAIL.

- [ ] **Step 3: Add errors + cycle helper.** Add `NotAPage` (`"not a page"`) and `PageCycle` (`"page cycle"`) to `PageError` + `Display`. Add a folder-forest ancestry walk near `ancestry_excludes`:

```rust
    /// walk FOLDER parents (the index map) up from `start`, erroring with
    /// `PageCycle` if `forbidden` is met — that would nest a page inside its own
    /// folder subtree. `MAX_DEPTH` turns a corrupt loop into a loud error.
    async fn folder_ancestry_excludes(&self, start: &str, forbidden: &str) -> Result<(), PageError> {
        let index = self.load_index().await.map_err(to_page_err)?;
        let mut cur = Some(start.to_string());
        for _ in 0..MAX_DEPTH {
            match cur {
                None => return Ok(()),
                Some(id) => {
                    if id == forbidden {
                        return Err(PageError::PageCycle);
                    }
                    cur = index.get(&id).cloned().flatten();
                }
            }
        }
        Err(PageError::Corrupt)
    }
```

- [ ] **Step 4: Replace the combined stub** (`PageMsg::SetPageParent { .. } | PageMsg::DeletePage { .. } => Err(PageError::Corrupt)` from Task 2 Step 4b) with the real `SetPageParent` arm plus a still-stubbed `DeletePage` (filled in Task 4):

```rust
            PageMsg::DeletePage { .. } => Err(PageError::Corrupt), // stub — Task 4
            PageMsg::SetPageParent { page_id, parent } => {
                // the target must be an existing page root.
                let root = self.require_block(&page_id, PageError::NotAPage).await?;
                if root.kind != BlockKind::Page {
                    return Err(PageError::NotAPage);
                }
                if let Some(par) = &parent {
                    // parent must exist and be a page …
                    match self.load_block(par).await.map_err(to_page_err)? {
                        Some(b) if b.kind == BlockKind::Page => {}
                        _ => return Err(PageError::ParentPageNotFound),
                    }
                    // … and nesting under self or a descendant would cycle.
                    self.folder_ancestry_excludes(par, &page_id).await?;
                }
                let mut index = self.load_index().await.map_err(to_page_err)?;
                index.insert(page_id, parent);
                self.stage_index(&index)
            }
```

The reserved-sentinel `named` guard arms for `SetPageParent`/`DeletePage` were already added in Task 2 Step 4b — no change needed here.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p pages 2>&1 | tail -30`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add crates/apps/pages/src/lib.rs
git commit --no-gpg-sign -m "feat(pages): SetPageParent re-nests pages with folder-cycle rejection"
```

---

### Task 4: `DeletePage` — remove subtree, promote children

**Files:**
- Modify: `crates/apps/pages/src/lib.rs`

**Interfaces:**
- Consumes: index map, subtree-delete walk from `RemoveBlock`.
- Produces: `DeletePage` apply arm.

- [ ] **Step 1: Write the failing test** (append to `tests` mod):

```rust
    #[test]
    fn delete_page_removes_subtree_and_promotes_children() {
        deterministic::Runner::default().start(|context| async move {
            let mut p = Pages::init(context, "pages").await;
            // grand -> parent -> child ; parent also has a content block pb1.
            apply_commit(&mut p, &PageMsg::CreatePage { page_id: "grand".into(), title: "G".into(), parent: None }).await;
            apply_commit(&mut p, &PageMsg::CreatePage { page_id: "parent".into(), title: "P".into(), parent: Some("grand".into()) }).await;
            apply_commit(&mut p, &PageMsg::CreatePage { page_id: "child".into(), title: "C".into(), parent: Some("parent".into()) }).await;
            apply_commit(&mut p, &PageMsg::InsertBlock { parent: "parent".into(), after: None, block: para("pb1", "body") }).await;

            apply_commit(&mut p, &PageMsg::DeletePage { page_id: "parent".into() }).await;

            // parent's root + content block are gone …
            assert!(get_block(&p, "parent").await.is_none());
            assert!(get_block(&p, "pb1").await.is_none());
            assert!(get_page(&p, "parent").await.is_none());
            // … child was PROMOTED to grand (parent's parent), not deleted.
            let pages = list_pages(&p).await;
            assert!(pages.iter().all(|m| m.id != "parent"));
            let child = pages.iter().find(|m| m.id == "child").unwrap();
            assert_eq!(child.parent.as_deref(), Some("grand"));

            // deleting a non-page id is rejected.
            seed_page(&mut p, "pg").await;
            apply_expect_err(&mut p, &PageMsg::DeletePage { page_id: "b1".into() }, "not a page").await;
        });
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pages delete_page 2>&1 | tail -20`
Expected: compile error (missing arm), then FAIL.

- [ ] **Step 3: Replace the `DeletePage` stub** (`PageMsg::DeletePage { .. } => Err(PageError::Corrupt)` from Task 3) with the real arm:

```rust
            PageMsg::DeletePage { page_id } => {
                let root = self.require_block(&page_id, PageError::NotAPage).await?;
                if root.kind != BlockKind::Page {
                    return Err(PageError::NotAPage);
                }
                // promote direct child pages to the deleted page's parent, then
                // drop the deleted page's own index entry.
                let mut index = self.load_index().await.map_err(to_page_err)?;
                let promoted_to = index.get(&page_id).cloned().flatten();
                for parent in index.values_mut() {
                    if parent.as_deref() == Some(page_id.as_str()) {
                        *parent = promoted_to.clone();
                    }
                }
                index.remove(&page_id);
                self.stage_index(&index)?;
                // delete the whole block subtree, root included (depth-first).
                let mut stack = vec![root];
                while let Some(cur) = stack.pop() {
                    for child in &cur.children {
                        stack.push(self.require_block(child, PageError::Corrupt).await?);
                    }
                    self.delete_block(&cur.id);
                }
                Ok(())
            }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p pages 2>&1 | tail -30`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/apps/pages/src/lib.rs
git commit --no-gpg-sign -m "feat(pages): DeletePage removes the block subtree and promotes child pages"
```

---

## Phase 2 — Comments module (new `crates/apps/comments`)

Reference the spec §B. Modeled on `crates/apps/chat` (author from origin) and `crates/apps/pages` (qmdb skeleton, reserved index, staging overlay). No materialized-view tier — the reserved per-anchor index in committed state serves `ThreadsForAnchors`.

### Task 5: Comments crate scaffold + interface

**Files:**
- Create: `crates/apps/comments/Cargo.toml`
- Create: `crates/apps/comments/src/interface.rs`
- Create: `crates/apps/comments/src/lib.rs` (stub, replaced in Task 6)
- Modify: `Cargo.toml` (workspace members)

**Interfaces:**
- Produces: `Anchor`, `AuthorRef`, `Thread`, `Comment`, `CommentMsg`, `CommentQuery`, `CommentReply`, `ThreadView`, `AnchorThreads`, `PostedComment`, encode/decode fns, `DEFAULT_COMMENTS_TARGET`, and the byte caps.

- [ ] **Step 1: Add the crate to the workspace.** In the root `Cargo.toml` members list, add after `"crates/apps/chat",`:

```toml
    "crates/apps/comments", 
```

- [ ] **Step 2: Create `crates/apps/comments/Cargo.toml`** (copy of pages' manifest, minus the `indexer` dep since there is no view tier):

```toml
[package]
name = "comments"
edition.workspace = true
version.workspace = true

[dependencies]
sdk = { workspace = true }
serde = { workspace = true }
# kernel state-sync platform surface: the shared qmdb serve/wire helpers.
statesync = { workspace = true }
async-trait = "0.1"
serde_json = "1"
commonware-storage = { workspace = true }
commonware-runtime = { workspace = true }
commonware-cryptography = { workspace = true }
commonware-parallel = "2026.5.0"
commonware-codec = "2026.5.0"
commonware-utils = { workspace = true }

[dev-dependencies]
state = { workspace = true }
tempfile = "3"
tokio = { workspace = true }
```

- [ ] **Step 3: Write the failing test** — create `crates/apps/comments/src/interface.rs` with types + a `#[cfg(test)]` serde test:

```rust
//! the comments module's public wire surface — types only, no logic, no sdk dep.
//!
//! a comment thread anchors to one addressable record ([`Anchor`] = module +
//! target id — a pages block id or page id) and holds an ordered list of
//! [`Comment`]s. authorship is derived from the dispatch origin, never a
//! payload (mirrors the chat module). the anchor makes a thread resolvable from
//! whatever holds the target, so a page render can batch-fetch every visible
//! block's threads with one [`CommentQuery::ThreadsForAnchors`].

use serde::{Deserialize, Serialize};

pub const DEFAULT_COMMENTS_TARGET: &str = "comments";

// write-time caps (consensus constants) — enforced BEFORE staging; the qmdb
// codec's 1 MiB cap is decode-only, so an oversized committed value poisons
// every validator's next read.
pub const MAX_COMMENT_TEXT_BYTES: usize = 64 * 1024;
pub const MAX_COMMENTS_PER_THREAD: usize = 4096;
pub const MAX_THREADS_PER_ANCHOR: usize = 1024;
pub const MAX_QUERY_TARGETS: usize = 512;

/// what a thread is attached to: a module id plus a target record id (a pages
/// block id or page id). general so comments can anchor to any addressable
/// record later.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Anchor {
    pub module: String,
    pub target: String,
}

/// who authored a comment — derived from `Env.origin`, never a payload. own
/// copy of chat's shape (each module's interface is self-contained).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AuthorRef {
    User(Vec<u8>),
    Agent { module: String, agent_id: String },
    Module(String),
    System,
}

/// a comment thread: an anchor, its opener, resolve state, and the ordered ids
/// of its comments (tombstoned comments stay listed until the whole thread is
/// removed on last-live-delete).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Thread {
    pub id: String,
    pub anchor: Anchor,
    pub opener: AuthorRef,
    pub created_at: u64,
    pub resolved: bool,
    pub resolved_by: Option<AuthorRef>,
    pub comment_ids: Vec<String>,
}

/// one comment. `deleted` tombstones content but keeps the record so ordering
/// and the thread skeleton survive until the thread is removed.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    pub id: String,
    pub thread_id: String,
    pub author: AuthorRef,
    pub text: String,
    pub created_at: u64,
    pub edited_at: Option<u64>,
    pub deleted: bool,
}

/// write intents. author + timestamp are derived by the module, never here.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommentMsg {
    /// open a thread (when `thread_id` is new) with `anchor` and this first
    /// comment, or append `comment_id` to an existing thread (whose anchor must
    /// match). author = origin.
    AddComment {
        thread_id: String,
        comment_id: String,
        anchor: Anchor,
        text: String,
    },
    /// replace a comment's text; stored-author-only. rejected on a tombstone.
    EditComment { comment_id: String, text: String },
    /// tombstone a comment; stored-author-only. when it was the thread's last
    /// live comment, the whole thread record is removed.
    DeleteComment { comment_id: String },
    /// toggle a thread's resolved flag; records the resolver as origin.
    ResolveThread { thread_id: String, resolved: bool },
}

/// read requests.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommentQuery {
    /// every thread anchored to any of `targets` in `module`, grouped by
    /// target. a page render calls this once with all visible block ids + the
    /// page id. `targets` beyond [`MAX_QUERY_TARGETS`] are rejected.
    ThreadsForAnchors { module: String, targets: Vec<String> },
    /// one thread with its live comments.
    Thread { thread_id: String },
}

/// a thread plus its live (non-tombstoned) comments in order.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ThreadView {
    pub thread: Thread,
    pub comments: Vec<Comment>,
}

/// the threads anchored to one target.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AnchorThreads {
    pub target: String,
    pub threads: Vec<ThreadView>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommentReply {
    Anchored(Vec<AnchorThreads>),
    Thread(Option<ThreadView>),
}

pub fn encode_msg(m: &CommentMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}
pub fn decode_msg(b: &[u8]) -> Result<CommentMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_query(q: &CommentQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}
pub fn decode_query(b: &[u8]) -> Result<CommentQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_reply(r: &CommentReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}
pub fn decode_reply(b: &[u8]) -> Result<CommentReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

#[cfg(test)]
mod interface_tests {
    use super::*;

    #[test]
    fn msg_round_trips() {
        let m = CommentMsg::AddComment {
            thread_id: "t1".into(),
            comment_id: "c1".into(),
            anchor: Anchor { module: "pages".into(), target: "b1".into() },
            text: "hi".into(),
        };
        assert_eq!(decode_msg(&encode_msg(&m)).unwrap(), m);
    }

    #[test]
    fn reply_round_trips() {
        let r = CommentReply::Anchored(vec![AnchorThreads {
            target: "b1".into(),
            threads: vec![],
        }]);
        assert_eq!(decode_reply(&encode_reply(&r)).unwrap(), r);
    }
}
```

- [ ] **Step 4: Create a compiling `lib.rs` stub** so the crate builds and Step 5 can run:

```rust
//! comments module — placeholder, replaced by the qmdb impl in Task 6.
mod interface;
pub use interface::*;
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p comments interface_tests 2>&1 | tail -20`
Expected: both serde tests pass.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/apps/comments/
git commit --no-gpg-sign -m "feat(comments): crate scaffold + wire interface (threads anchored to a block/page)"
```

---

### Task 6: Comments qmdb skeleton + Module impl

**Files:**
- Modify: `crates/apps/comments/src/lib.rs`

**Interfaces:**
- Consumes: interface types.
- Produces: `Comments<E>` with `init`, storage helpers (`get`/`stage`/`store_thread`/`store_comment`/`delete_key`/`load_thread`/`load_comment`/`load_anchor_index`/`stage_anchor_index`), the `Module` impl (`root`/`serve_sync`/`execute`/`query`/`commit_block`/`abort_block`), `sync_target`/`into_resolver`/`sync_from`, `author_from_origin`. `apply`/`query` dispatch exists but every mutating arm returns `Err(CommentError::Unsupported)` and queries return empties (filled in Tasks 7–10).

- [ ] **Step 1: Write the failing test** — add a `#[cfg(test)]` mod to `lib.rs` mirroring pages' `write_moves_root` shape, plus a helper harness:

```rust
    #[test]
    fn a_staged_write_moves_the_root() {
        deterministic::Runner::default().start(|context| async move {
            let mut c = Comments::init(context, "comments").await;
            let r0 = c.root();
            // stage a raw thread record directly (apply arms are stubs in Task 6).
            c.pending.insert(b"t:probe".to_vec(), Some(b"{}".to_vec()));
            c.commit_block().await.unwrap();
            assert_ne!(c.root(), r0, "a committed write must move the root");
        });
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p comments a_staged_write 2>&1 | tail -20`
Expected: FAIL to compile — `Comments` does not exist.

- [ ] **Step 3: Write the skeleton.** Replace `crates/apps/comments/src/lib.rs` with the qmdb skeleton. It is a near-verbatim copy of `crates/apps/pages/src/lib.rs` lines 47–318 and 605–783 (the storage plumbing + `Module` state-sync impl), with `Pages`→`Comments`, `PagesDb`→`CommentsDb`, `PageKey`→`CommentKey`, `MAX_BLOCK_LEN`→a per-record guard, and the pages-specific block helpers replaced by comment helpers. Full file:

```rust
//! qmdb-backed comments module — threads anchored to an addressable record.
//!
//! one record per qmdb key (`sha256(logical_key)`): a thread under `t:<id>`, a
//! comment under `c:<id>`, and a reserved per-anchor index under
//! `\0a:<json(anchor)>` holding the sorted thread ids on that anchor. writes
//! stage in an in-memory overlay and flush in one batch at `commit_block`
//! (`abort_block` drops it), exactly like the pages/chat modules; state-sync
//! delegates to commonware's qmdb sync engine.

mod interface;
pub use interface::*;

use std::collections::BTreeMap;
use std::num::{NonZeroU16, NonZeroU64, NonZeroUsize};
use std::sync::Arc;

use commonware_codec::RangeCfg;
use commonware_cryptography::{Hasher, Sha256};
use commonware_parallel::Sequential;
use commonware_runtime::{BufferPooler, buffer::paged::CacheRef};
use commonware_storage::{
    Context, journal, mmr,
    qmdb::{
        any::{VariableConfig, unordered::variable::Db},
        sync::{self, DbResolver, Target, engine::Config as SyncConfig},
    },
    translator::TwoCap,
};
use commonware_utils::range::NonEmptyRange;

use sdk::{Ctx, Env, Error, Module, ModuleId, Msg, Origin, ResolverSyncTarget, StateRoot, StateSyncHandle};

/// write-time cap on ONE serialized record (thread, comment, or index value).
/// leaves the same framing margin the pages module keeps under the 1 MiB codec
/// bound.
pub const MAX_RECORD_LEN: usize = 768 * 1024;

/// reserved logical-key prefix for the per-anchor thread index. its leading NUL
/// makes it uncollidable with a `t:`/`c:` record key.
const ANCHOR_INDEX_PREFIX: &str = "\u{0}a:";

type CommentKey = <Sha256 as Hasher>::Digest;
type CommentsDb<E> = Db<mmr::Family, E, CommentKey, Vec<u8>, Sha256, TwoCap, Sequential>;
type CommentsConfig = VariableConfig<TwoCap, ((), (RangeCfg<usize>, ())), Sequential>;
pub type CommentsTarget = Target<mmr::Family, CommentKey>;

fn hash_key(k: &[u8]) -> CommentKey {
    let mut h = Sha256::new();
    h.update(k);
    h.finalize()
}

fn thread_key(id: &str) -> String { format!("t:{id}") }
fn comment_key(id: &str) -> String { format!("c:{id}") }
fn anchor_index_key(anchor: &Anchor) -> String {
    format!("{ANCHOR_INDEX_PREFIX}{}", serde_json::to_string(anchor).expect("anchor serializable"))
}

fn comments_config<E>(context: &E, id: &str) -> CommentsConfig
where
    E: Context + BufferPooler,
{
    let page_cache = CacheRef::from_pooler(
        context,
        NonZeroU16::new(128).unwrap(),
        NonZeroUsize::new(64).unwrap(),
    );
    let codec_config = ((), (RangeCfg::from(0..=1 << 20), ()));
    VariableConfig {
        merkle_config: mmr::full::Config {
            journal_partition: format!("{id}-merkle-journal"),
            metadata_partition: format!("{id}-merkle-meta"),
            items_per_blob: NonZeroU64::new(64).unwrap(),
            write_buffer: NonZeroUsize::new(1024).unwrap(),
            strategy: Sequential,
            page_cache: page_cache.clone(),
        },
        journal_config: journal::contiguous::variable::Config {
            partition: format!("{id}-log"),
            items_per_section: NonZeroU64::new(64).unwrap(),
            write_buffer: NonZeroUsize::new(1024).unwrap(),
            compression: None,
            codec_config,
            page_cache,
        },
        translator: TwoCap,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CommentError {
    EmptyOrigin,
    ThreadNotFound,
    CommentNotFound,
    DuplicateComment,
    AnchorMismatch,
    NotAuthor,
    TextTooLarge,
    TooManyComments,
    TooManyThreads,
    TooManyTargets,
    ReservedId,
    Corrupt,
    Unsupported,
}

impl core::fmt::Display for CommentError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let s = match self {
            CommentError::EmptyOrigin => "empty origin",
            CommentError::ThreadNotFound => "thread not found",
            CommentError::CommentNotFound => "comment not found",
            CommentError::DuplicateComment => "duplicate comment id",
            CommentError::AnchorMismatch => "anchor mismatch",
            CommentError::NotAuthor => "not the comment author",
            CommentError::TextTooLarge => "comment text too large",
            CommentError::TooManyComments => "too many comments in thread",
            CommentError::TooManyThreads => "too many threads on anchor",
            CommentError::TooManyTargets => "too many query targets",
            CommentError::ReservedId => "reserved id",
            CommentError::Corrupt => "stored comment state is corrupt",
            CommentError::Unsupported => "unsupported",
        };
        f.write_str(s)
    }
}

fn to_err(_e: Error) -> CommentError { CommentError::Corrupt }

/// derive the author from the dispatch origin (mirrors chat). the pre-consensus
/// default `Origin::External(vec![])` must never pass as a real user.
fn author_from_origin(origin: &Origin) -> Result<AuthorRef, CommentError> {
    match origin {
        Origin::External(bytes) if bytes.is_empty() => Err(CommentError::EmptyOrigin),
        Origin::External(bytes) => Ok(AuthorRef::User(bytes.clone())),
        Origin::Module(id) => Ok(AuthorRef::Module(id.to_string())),
        Origin::System => Ok(AuthorRef::System),
    }
}

pub struct Comments<E>
where
    E: Context + BufferPooler,
{
    id: ModuleId,
    db: CommentsDb<E>,
    pending: BTreeMap<Vec<u8>, Option<Vec<u8>>>,
}

impl<E> Comments<E>
where
    E: Context + BufferPooler,
{
    pub async fn init(context: E, id: impl Into<ModuleId>) -> Self {
        let id = id.into();
        let cfg = comments_config(&context, &id);
        let db = CommentsDb::<E>::init(context, cfg).await.expect("qmdb init failed");
        Self { id, db, pending: BTreeMap::new() }
    }

    async fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        if let Some(staged) = self.pending.get(key) {
            return staged.clone();
        }
        self.db.get(&hash_key(key)).await.expect("get failed")
    }

    fn stage(&mut self, key: &str, bytes: Vec<u8>) -> Result<(), CommentError> {
        if bytes.len() > MAX_RECORD_LEN {
            return Err(CommentError::TextTooLarge);
        }
        self.pending.insert(key.as_bytes().to_vec(), Some(bytes));
        Ok(())
    }

    fn delete_key(&mut self, key: &str) {
        self.pending.insert(key.as_bytes().to_vec(), None);
    }

    async fn load_thread(&self, id: &str) -> Result<Option<Thread>, CommentError> {
        match self.get(thread_key(id).as_bytes()).await {
            Some(b) => Ok(Some(serde_json::from_slice(&b).map_err(|_| CommentError::Corrupt)?)),
            None => Ok(None),
        }
    }

    async fn load_comment(&self, id: &str) -> Result<Option<Comment>, CommentError> {
        match self.get(comment_key(id).as_bytes()).await {
            Some(b) => Ok(Some(serde_json::from_slice(&b).map_err(|_| CommentError::Corrupt)?)),
            None => Ok(None),
        }
    }

    fn store_thread(&mut self, t: &Thread) -> Result<(), CommentError> {
        self.stage(&thread_key(&t.id), serde_json::to_vec(t).expect("thread serializable"))
    }

    fn store_comment(&mut self, c: &Comment) -> Result<(), CommentError> {
        self.stage(&comment_key(&c.id), serde_json::to_vec(c).expect("comment serializable"))
    }

    async fn load_anchor_index(&self, anchor: &Anchor) -> Result<Vec<String>, CommentError> {
        match self.get(anchor_index_key(anchor).as_bytes()).await {
            Some(b) => serde_json::from_slice(&b).map_err(|_| CommentError::Corrupt),
            None => Ok(Vec::new()),
        }
    }

    fn stage_anchor_index(&mut self, anchor: &Anchor, ids: &[String]) -> Result<(), CommentError> {
        if ids.is_empty() {
            self.delete_key(&anchor_index_key(anchor));
            Ok(())
        } else {
            self.stage(&anchor_index_key(anchor), serde_json::to_vec(ids).expect("ids serializable"))
        }
    }

    /// apply one decoded msg with the derived author/time. every arm is a stub
    /// until Tasks 7–10.
    async fn apply(&mut self, _msg: CommentMsg, _author: AuthorRef, _now: u64) -> Result<(), CommentError> {
        Err(CommentError::Unsupported)
    }

    // ---- state-sync (verbatim from pages) ----
    pub async fn sync_target(&self) -> CommentsTarget {
        let end = self.db.bounds().await.end;
        let start = self.db.sync_boundary();
        Target {
            root: self.db.root(),
            range: NonEmptyRange::new(start..end).expect("committed store has a non-empty op range"),
        }
    }
    pub fn into_resolver(self) -> Arc<CommentsDb<E>> {
        Arc::new(self.db)
    }
    pub async fn sync_from<R>(
        context: E,
        id: impl Into<ModuleId>,
        target: CommentsTarget,
        resolver: R,
    ) -> Result<Self, String>
    where
        R: DbResolver<CommentsDb<E>>,
    {
        let id = id.into();
        let db_config = comments_config(&context, &id);
        let config = SyncConfig {
            context,
            resolver,
            target,
            max_outstanding_requests: 1,
            fetch_batch_size: NonZeroU64::new(64).unwrap(),
            apply_batch_size: 1024,
            db_config,
            update_rx: None,
            finish_rx: None,
            reached_target_tx: None,
            max_retained_roots: 8,
        };
        let db = sync::sync(config).await.map_err(|e| format!("qmdb sync: {e:?}"))?;
        Ok(Self { id, db, pending: BTreeMap::new() })
    }
}

#[async_trait::async_trait(?Send)]
impl<E> Module for Comments<E>
where
    E: Context + BufferPooler,
{
    fn id(&self) -> ModuleId { self.id.clone() }
    fn root(&self) -> StateRoot { StateRoot(self.db.root().0) }
    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::ResolverBacked {
            backend: "qmdb".into(),
            detail: "serve_sync answers qmdb op-range requests (statesync wire)".into(),
        })
    }
    async fn serve_sync(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        statesync::qmdb::serve_bytes(&self.db, req).await
    }
    async fn resolver_sync_target(&self) -> Result<ResolverSyncTarget, Error> {
        statesync::qmdb::resolver_sync_target(&self.db).await
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let env: &Env = ctx.env();
        let author = author_from_origin(&env.origin).map_err(|e| Error::Module(e.to_string()))?;
        let now = env.consensus_time;
        let m = decode_msg(&msg.payload).map_err(Error::Module)?;
        self.apply(m, author, now).await.map_err(|e| Error::Module(e.to_string()))
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            CommentQuery::ThreadsForAnchors { .. } => {
                Ok(encode_reply(&CommentReply::Anchored(Vec::new())))
            }
            CommentQuery::Thread { .. } => Ok(encode_reply(&CommentReply::Thread(None))),
        }
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let mut batch = self.db.new_batch();
        for (key, value) in &self.pending {
            batch = batch.write(hash_key(key), value.clone());
        }
        let batch = batch.merkleize(&self.db, None::<Vec<u8>>).await.expect("merkleize failed");
        self.db.apply_batch(batch).await.expect("apply_batch failed");
        self.db.commit().await.expect("commit failed");
        self.pending.clear();
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_runtime::{Runner as _, deterministic};

    // the a_staged_write_moves_the_root test (Task 6, Step 1) goes here.
    // Task 7+ add a TestCtx + apply_commit harness mirroring pages' tests.
}
```

Move the Task-6 Step-1 test into this `tests` mod.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p comments 2>&1 | tail -20`
Expected: `a_staged_write_moves_the_root` passes; interface tests still pass.

- [ ] **Step 5: Commit**

```bash
git add crates/apps/comments/src/lib.rs
git commit --no-gpg-sign -m "feat(comments): qmdb skeleton + Module impl (author from origin, state-sync)"
```

---

### Task 7: `AddComment` — open/append threads + anchor index + queries

**Files:**
- Modify: `crates/apps/comments/src/lib.rs`

**Interfaces:**
- Consumes: skeleton helpers + `apply` signature `apply(&mut self, msg, author, now)`.
- Produces: `AddComment` arm; `thread_view(&self, thread_id) -> Result<Option<ThreadView>>` helper; `ThreadsForAnchors`/`Thread` query arms; a `tests` harness (`TestCtx::new(origin)`, `user`, `apply_commit`, `apply_err`, `anchored`, `thread_of`).

- [ ] **Step 1: Write the test harness + failing test.** Replace the `tests` mod body with the harness and tests (keep `a_staged_write_moves_the_root`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use commonware_runtime::{Runner as _, deterministic};
    use sdk::{Env, Origin};

    struct TestCtx { env: Env }
    impl TestCtx {
        fn new(origin: Origin) -> Self {
            Self { env: Env { protocol_version: 0, height: 0, consensus_time: 7, origin, me: "comments".into() } }
        }
    }
    #[async_trait::async_trait(?Send)]
    impl Ctx for TestCtx {
        fn env(&self) -> &Env { &self.env }
        fn module_root(&self, _t: &str) -> Option<StateRoot> { None }
        async fn query(&self, _t: &str, _r: &[u8]) -> Result<Vec<u8>, Error> { Err(Error::QueryUnsupported) }
        fn emit_msg(&mut self, _m: Msg) {}
        fn emit_event(&mut self, _e: sdk::Event) {}
        fn request_effect(&mut self, _e: sdk::Effect) {}
    }
    fn user(name: &str) -> Origin { Origin::External(name.as_bytes().to_vec()) }
    fn wire(m: &CommentMsg) -> Msg { Msg { target: "comments".into(), payload: encode_msg(m) } }

    async fn apply_commit<E: Context + BufferPooler>(c: &mut Comments<E>, m: &CommentMsg, origin: Origin) {
        c.execute(&mut TestCtx::new(origin), &wire(m)).await.unwrap();
        c.commit_block().await.unwrap();
    }
    async fn apply_err<E: Context + BufferPooler>(c: &mut Comments<E>, m: &CommentMsg, origin: Origin, needle: &str) {
        let e = c.execute(&mut TestCtx::new(origin), &wire(m)).await.expect_err("must reject");
        assert!(matches!(e, Error::Module(ref s) if s.contains(needle)), "unexpected: {e:?}");
        c.abort_block().await.unwrap();
    }
    async fn anchored<E: Context + BufferPooler>(c: &Comments<E>, module: &str, targets: &[&str]) -> Vec<AnchorThreads> {
        let q = CommentQuery::ThreadsForAnchors { module: module.into(), targets: targets.iter().map(|s| s.to_string()).collect() };
        match decode_reply(&c.query(&encode_query(&q)).await.unwrap()).unwrap() {
            CommentReply::Anchored(v) => v, _ => panic!("expected Anchored"),
        }
    }
    async fn thread_of<E: Context + BufferPooler>(c: &Comments<E>, thread_id: &str) -> Option<ThreadView> {
        match decode_reply(&c.query(&encode_query(&CommentQuery::Thread { thread_id: thread_id.into() })).await.unwrap()).unwrap() {
            CommentReply::Thread(v) => v, _ => panic!("expected Thread"),
        }
    }
    fn anchor(target: &str) -> Anchor { Anchor { module: "pages".into(), target: target.into() } }

    #[test]
    fn a_staged_write_moves_the_root() {
        deterministic::Runner::default().start(|context| async move {
            let mut c = Comments::init(context, "comments").await;
            let r0 = c.root();
            c.pending.insert(b"t:probe".to_vec(), Some(b"{}".to_vec()));
            c.commit_block().await.unwrap();
            assert_ne!(c.root(), r0);
        });
    }

    #[test]
    fn add_opens_then_appends_and_batches_by_anchor() {
        deterministic::Runner::default().start(|context| async move {
            let mut c = Comments::init(context, "comments").await;
            // open thread t1 on block b1.
            apply_commit(&mut c, &CommentMsg::AddComment {
                thread_id: "t1".into(), comment_id: "m1".into(), anchor: anchor("b1"), text: "first".into(),
            }, user("alice")).await;
            // append m2 to t1.
            apply_commit(&mut c, &CommentMsg::AddComment {
                thread_id: "t1".into(), comment_id: "m2".into(), anchor: anchor("b1"), text: "second".into(),
            }, user("bob")).await;
            // a second thread t2 on the same block.
            apply_commit(&mut c, &CommentMsg::AddComment {
                thread_id: "t2".into(), comment_id: "m3".into(), anchor: anchor("b1"), text: "other".into(),
            }, user("alice")).await;
            // and one on a different block b2.
            apply_commit(&mut c, &CommentMsg::AddComment {
                thread_id: "t3".into(), comment_id: "m4".into(), anchor: anchor("b2"), text: "elsewhere".into(),
            }, user("alice")).await;

            let groups = anchored(&c, "pages", &["b1", "b2"]).await;
            let b1 = groups.iter().find(|g| g.target == "b1").unwrap();
            assert_eq!(b1.threads.len(), 2);
            let t1 = b1.threads.iter().find(|v| v.thread.id == "t1").unwrap();
            assert_eq!(t1.comments.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(), ["first", "second"]);
            assert_eq!(t1.thread.opener, AuthorRef::User(b"alice".to_vec()));
            assert_eq!(t1.comments[1].author, AuthorRef::User(b"bob".to_vec()));
            let b2 = groups.iter().find(|g| g.target == "b2").unwrap();
            assert_eq!(b2.threads.len(), 1);
        });
    }

    #[test]
    fn append_rejects_anchor_mismatch_and_duplicate_comment() {
        deterministic::Runner::default().start(|context| async move {
            let mut c = Comments::init(context, "comments").await;
            apply_commit(&mut c, &CommentMsg::AddComment {
                thread_id: "t1".into(), comment_id: "m1".into(), anchor: anchor("b1"), text: "x".into(),
            }, user("alice")).await;
            // wrong anchor on an existing thread.
            apply_err(&mut c, &CommentMsg::AddComment {
                thread_id: "t1".into(), comment_id: "m2".into(), anchor: anchor("b2"), text: "y".into(),
            }, user("alice"), "anchor mismatch").await;
            // reused comment id (globally).
            apply_err(&mut c, &CommentMsg::AddComment {
                thread_id: "t1".into(), comment_id: "m1".into(), anchor: anchor("b1"), text: "z".into(),
            }, user("alice"), "duplicate comment id").await;
            // empty origin is rejected.
            apply_err(&mut c, &CommentMsg::AddComment {
                thread_id: "t9".into(), comment_id: "m9".into(), anchor: anchor("b1"), text: "z".into(),
            }, Origin::External(vec![]), "empty origin").await;
        });
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p comments 2>&1 | tail -20`
Expected: the three new tests fail (`AddComment` returns `Unsupported`; queries return empties).

- [ ] **Step 3: Implement the `AddComment` arm.** Replace the stub `apply` body:

```rust
    async fn apply(&mut self, msg: CommentMsg, author: AuthorRef, now: u64) -> Result<(), CommentError> {
        match msg {
            CommentMsg::AddComment { thread_id, comment_id, anchor, text } => {
                if thread_id.is_empty() || comment_id.is_empty()
                    || thread_id.starts_with('\u{0}') || comment_id.starts_with('\u{0}') {
                    return Err(CommentError::ReservedId);
                }
                if text.len() > MAX_COMMENT_TEXT_BYTES {
                    return Err(CommentError::TextTooLarge);
                }
                if self.load_comment(&comment_id).await?.is_some() {
                    return Err(CommentError::DuplicateComment);
                }
                match self.load_thread(&thread_id).await? {
                    Some(mut thread) => {
                        if thread.anchor != anchor {
                            return Err(CommentError::AnchorMismatch);
                        }
                        if thread.comment_ids.len() >= MAX_COMMENTS_PER_THREAD {
                            return Err(CommentError::TooManyComments);
                        }
                        let comment = Comment {
                            id: comment_id.clone(), thread_id: thread_id.clone(), author,
                            text, created_at: now, edited_at: None, deleted: false,
                        };
                        thread.comment_ids.push(comment_id);
                        self.store_comment(&comment)?;
                        self.store_thread(&thread)
                    }
                    None => {
                        let mut ids = self.load_anchor_index(&anchor).await?;
                        if ids.len() >= MAX_THREADS_PER_ANCHOR {
                            return Err(CommentError::TooManyThreads);
                        }
                        let comment = Comment {
                            id: comment_id.clone(), thread_id: thread_id.clone(), author: author.clone(),
                            text, created_at: now, edited_at: None, deleted: false,
                        };
                        let thread = Thread {
                            id: thread_id.clone(), anchor: anchor.clone(), opener: author,
                            created_at: now, resolved: false, resolved_by: None,
                            comment_ids: vec![comment_id],
                        };
                        if !ids.contains(&thread_id) {
                            ids.push(thread_id);
                            ids.sort();
                            self.stage_anchor_index(&anchor, &ids)?;
                        }
                        self.store_comment(&comment)?;
                        self.store_thread(&thread)
                    }
                }
            }
            // Edit/Delete/Resolve added in Tasks 8–9.
            CommentMsg::EditComment { .. }
            | CommentMsg::DeleteComment { .. }
            | CommentMsg::ResolveThread { .. } => Err(CommentError::Unsupported),
        }
    }
```

- [ ] **Step 4: Add the `thread_view` helper** (a `&self` method on `Comments<E>`, next to the load helpers):

```rust
    /// a thread plus its LIVE (non-tombstoned) comments in order. `None` when
    /// the thread is absent. a listed comment missing from the store is
    /// corruption, surfaced loudly.
    async fn thread_view(&self, thread_id: &str) -> Result<Option<ThreadView>, CommentError> {
        let thread = match self.load_thread(thread_id).await? {
            Some(t) => t,
            None => return Ok(None),
        };
        let mut comments = Vec::new();
        for cid in &thread.comment_ids {
            let c = self.load_comment(cid).await?.ok_or(CommentError::Corrupt)?;
            if !c.deleted {
                comments.push(c);
            }
        }
        Ok(Some(ThreadView { thread, comments }))
    }
```

- [ ] **Step 5: Implement the query arms.** Replace the `query` method's match:

```rust
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        let err = |e: CommentError| Error::Module(e.to_string());
        match decode_query(req).map_err(Error::Module)? {
            CommentQuery::ThreadsForAnchors { module, targets } => {
                if targets.len() > MAX_QUERY_TARGETS {
                    return Err(err(CommentError::TooManyTargets));
                }
                let mut out = Vec::with_capacity(targets.len());
                for target in targets {
                    let anchor = Anchor { module: module.clone(), target: target.clone() };
                    let ids = self.load_anchor_index(&anchor).await.map_err(err)?;
                    let mut threads = Vec::new();
                    for tid in ids {
                        if let Some(view) = self.thread_view(&tid).await.map_err(err)? {
                            threads.push(view);
                        }
                    }
                    out.push(AnchorThreads { target, threads });
                }
                Ok(encode_reply(&CommentReply::Anchored(out)))
            }
            CommentQuery::Thread { thread_id } => {
                let view = self.thread_view(&thread_id).await.map_err(err)?;
                Ok(encode_reply(&CommentReply::Thread(view)))
            }
        }
    }
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p comments 2>&1 | tail -20`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add crates/apps/comments/src/lib.rs
git commit --no-gpg-sign -m "feat(comments): AddComment opens/appends threads; ThreadsForAnchors + Thread queries"
```

---

### Task 8: `EditComment` / `DeleteComment` — author enforcement + empty-thread removal

**Files:**
- Modify: `crates/apps/comments/src/lib.rs`

**Interfaces:**
- Consumes: `AddComment`, `thread_view`, load/store helpers.
- Produces: `EditComment`/`DeleteComment` arms.

- [ ] **Step 1: Write the failing test** (append to `tests`):

```rust
    #[test]
    fn edit_and_delete_are_author_only() {
        deterministic::Runner::default().start(|context| async move {
            let mut c = Comments::init(context, "comments").await;
            apply_commit(&mut c, &CommentMsg::AddComment {
                thread_id: "t1".into(), comment_id: "m1".into(), anchor: anchor("b1"), text: "orig".into(),
            }, user("alice")).await;
            // a non-author cannot edit or delete.
            apply_err(&mut c, &CommentMsg::EditComment { comment_id: "m1".into(), text: "hacked".into() }, user("bob"), "not the comment author").await;
            apply_err(&mut c, &CommentMsg::DeleteComment { comment_id: "m1".into() }, user("bob"), "not the comment author").await;
            // the author edits.
            apply_commit(&mut c, &CommentMsg::EditComment { comment_id: "m1".into(), text: "edited".into() }, user("alice")).await;
            let v = thread_of(&c, "t1").await.unwrap();
            assert_eq!(v.comments[0].text, "edited");
            assert_eq!(v.comments[0].edited_at, Some(7));
        });
    }

    #[test]
    fn deleting_last_live_comment_removes_the_thread() {
        deterministic::Runner::default().start(|context| async move {
            let mut c = Comments::init(context, "comments").await;
            apply_commit(&mut c, &CommentMsg::AddComment {
                thread_id: "t1".into(), comment_id: "m1".into(), anchor: anchor("b1"), text: "a".into(),
            }, user("alice")).await;
            apply_commit(&mut c, &CommentMsg::AddComment {
                thread_id: "t1".into(), comment_id: "m2".into(), anchor: anchor("b1"), text: "b".into(),
            }, user("alice")).await;
            // delete m1: thread survives, only m2 shows.
            apply_commit(&mut c, &CommentMsg::DeleteComment { comment_id: "m1".into() }, user("alice")).await;
            let v = thread_of(&c, "t1").await.unwrap();
            assert_eq!(v.comments.iter().map(|x| x.text.as_str()).collect::<Vec<_>>(), ["b"]);
            // delete m2: last live comment gone -> whole thread removed, anchor index emptied.
            apply_commit(&mut c, &CommentMsg::DeleteComment { comment_id: "m2".into() }, user("alice")).await;
            assert!(thread_of(&c, "t1").await.is_none());
            let groups = anchored(&c, "pages", &["b1"]).await;
            assert!(groups[0].threads.is_empty());
        });
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p comments edit_and_delete deleting_last 2>&1 | tail -20`
Expected: FAIL (`Unsupported`).

- [ ] **Step 3: Implement the arms.** Replace the combined stub arm in `apply` with:

```rust
            CommentMsg::EditComment { comment_id, text } => {
                if text.len() > MAX_COMMENT_TEXT_BYTES {
                    return Err(CommentError::TextTooLarge);
                }
                let mut c = self.load_comment(&comment_id).await?.ok_or(CommentError::CommentNotFound)?;
                if c.deleted {
                    return Err(CommentError::CommentNotFound);
                }
                if c.author != author {
                    return Err(CommentError::NotAuthor);
                }
                c.text = text;
                c.edited_at = Some(now);
                self.store_comment(&c)
            }
            CommentMsg::DeleteComment { comment_id } => {
                let mut c = self.load_comment(&comment_id).await?.ok_or(CommentError::CommentNotFound)?;
                if c.deleted {
                    return Ok(()); // idempotent
                }
                if c.author != author {
                    return Err(CommentError::NotAuthor);
                }
                c.deleted = true;
                c.text = String::new();
                let thread_id = c.thread_id.clone();
                self.store_comment(&c)?;
                // if no live comments remain, remove the whole thread.
                let thread = self.load_thread(&thread_id).await?.ok_or(CommentError::Corrupt)?;
                let mut any_live = false;
                for cid in &thread.comment_ids {
                    let cc = self.load_comment(cid).await?.ok_or(CommentError::Corrupt)?;
                    if !cc.deleted {
                        any_live = true;
                        break;
                    }
                }
                if !any_live {
                    for cid in &thread.comment_ids {
                        self.delete_key(&comment_key(cid));
                    }
                    self.delete_key(&thread_key(&thread.id));
                    let mut ids = self.load_anchor_index(&thread.anchor).await?;
                    ids.retain(|t| t != &thread.id);
                    self.stage_anchor_index(&thread.anchor, &ids)?;
                }
                Ok(())
            }
            CommentMsg::ResolveThread { .. } => Err(CommentError::Unsupported),
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p comments 2>&1 | tail -20`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/apps/comments/src/lib.rs
git commit --no-gpg-sign -m "feat(comments): author-only edit/delete; last-live-delete removes the thread"
```

---

### Task 9: `ResolveThread`

**Files:**
- Modify: `crates/apps/comments/src/lib.rs`

**Interfaces:**
- Produces: `ResolveThread` arm.

- [ ] **Step 1: Write the failing test** (append to `tests`):

```rust
    #[test]
    fn resolve_toggles_and_records_resolver() {
        deterministic::Runner::default().start(|context| async move {
            let mut c = Comments::init(context, "comments").await;
            apply_commit(&mut c, &CommentMsg::AddComment {
                thread_id: "t1".into(), comment_id: "m1".into(), anchor: anchor("b1"), text: "a".into(),
            }, user("alice")).await;
            apply_commit(&mut c, &CommentMsg::ResolveThread { thread_id: "t1".into(), resolved: true }, user("bob")).await;
            let v = thread_of(&c, "t1").await.unwrap();
            assert!(v.thread.resolved);
            assert_eq!(v.thread.resolved_by, Some(AuthorRef::User(b"bob".to_vec())));
            apply_commit(&mut c, &CommentMsg::ResolveThread { thread_id: "t1".into(), resolved: false }, user("alice")).await;
            let v = thread_of(&c, "t1").await.unwrap();
            assert!(!v.thread.resolved);
            assert_eq!(v.thread.resolved_by, None);
            // resolving a missing thread errors.
            apply_err(&mut c, &CommentMsg::ResolveThread { thread_id: "ghost".into(), resolved: true }, user("alice"), "thread not found").await;
        });
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p comments resolve_toggles 2>&1 | tail -20`
Expected: FAIL (`Unsupported`).

- [ ] **Step 3: Implement the arm.** Replace `CommentMsg::ResolveThread { .. } => Err(CommentError::Unsupported),` with:

```rust
            CommentMsg::ResolveThread { thread_id, resolved } => {
                let mut thread = self.load_thread(&thread_id).await?.ok_or(CommentError::ThreadNotFound)?;
                thread.resolved = resolved;
                thread.resolved_by = if resolved { Some(author) } else { None };
                self.store_thread(&thread)
            }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p comments 2>&1 | tail -20`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/apps/comments/src/lib.rs
git commit --no-gpg-sign -m "feat(comments): ResolveThread toggles resolved + records resolver"
```

---

### Task 10: Caps enforced before staging

**Files:**
- Modify: `crates/apps/comments/src/lib.rs`

**Interfaces:**
- Produces: verification that text/thread/target caps reject before staging (logic already present from Tasks 7/8; this task pins it with tests).

- [ ] **Step 1: Write the failing test** (append to `tests`):

```rust
    #[test]
    fn caps_reject_before_staging() {
        deterministic::Runner::default().start(|context| async move {
            let mut c = Comments::init(context, "comments").await;
            let huge = "x".repeat(MAX_COMMENT_TEXT_BYTES + 1);
            apply_err(&mut c, &CommentMsg::AddComment {
                thread_id: "t1".into(), comment_id: "m1".into(), anchor: anchor("b1"), text: huge,
            }, user("alice"), "comment text too large").await;
            assert!(c.pending.is_empty(), "a rejected op stages nothing");

            // query target cap.
            let targets: Vec<String> = (0..=MAX_QUERY_TARGETS).map(|i| format!("t{i}")).collect();
            let q = CommentQuery::ThreadsForAnchors { module: "pages".into(), targets };
            assert!(c.query(&encode_query(&q)).await.is_err(), "over-cap query is rejected");
        });
    }
```

- [ ] **Step 2: Run test to verify it passes** (logic already exists from Tasks 7–8)

Run: `cargo test -p comments caps_reject 2>&1 | tail -20`
Expected: PASS. If the text-cap check somehow runs after staging, move the `text.len()` guard above any `store_*` call (it already is in the Task 7 code).

- [ ] **Step 3: Commit**

```bash
git add crates/apps/comments/src/lib.rs
git commit --no-gpg-sign -m "test(comments): pin write-time caps reject before staging"
```

---

### Task 11: Comments state-sync round-trip test

**Files:**
- Create: `crates/apps/comments/tests/sync_round_trip.rs`

**Interfaces:**
- Consumes: `Comments::{init, sync_target, into_resolver, sync_from}`, wire types.

- [ ] **Step 1: Write the test.** Model on `crates/apps/pages/tests/sync_round_trip.rs` (the `TestCtx` there uses `Origin::System`; comments' `author_from_origin` accepts `System`, so it drives fine). Create the file:

```rust
//! state-sync round-trip: a fresh `Comments` reconstructs a byte-identical qmdb
//! root by pulling a source store's op range. the source ADDS, EDITS, and
//! DELETES comments so the op log carries overwrites AND deletes — only a real
//! sync of the proven op range lands on the same root.

use commonware_runtime::{Runner as _, deterministic};
use comments::{
    Anchor, CommentMsg, CommentQuery, CommentReply, Comments, ThreadView, decode_reply,
    encode_msg, encode_query,
};
use sdk::{Ctx, Env, Error, Module, Msg, Origin, StateRoot};

struct TestCtx { env: Env }
impl TestCtx {
    fn new() -> Self {
        Self { env: Env { protocol_version: 0, height: 0, consensus_time: 3, origin: Origin::External(b"u".to_vec()), me: "comments".into() } }
    }
}
#[async_trait::async_trait(?Send)]
impl Ctx for TestCtx {
    fn env(&self) -> &Env { &self.env }
    fn module_root(&self, _t: &str) -> Option<StateRoot> { None }
    async fn query(&self, _t: &str, _r: &[u8]) -> Result<Vec<u8>, Error> { Err(Error::QueryUnsupported) }
    fn emit_msg(&mut self, _m: Msg) {}
    fn emit_event(&mut self, _e: sdk::Event) {}
    fn request_effect(&mut self, _e: sdk::Effect) {}
}
async fn apply_commit<E>(c: &mut Comments<E>, m: &CommentMsg)
where E: commonware_storage::Context + commonware_runtime::BufferPooler {
    let msg = Msg { target: "comments".into(), payload: encode_msg(m) };
    c.execute(&mut TestCtx::new(), &msg).await.unwrap();
    c.commit_block().await.unwrap();
}
async fn thread_of<E>(c: &Comments<E>, id: &str) -> Option<ThreadView>
where E: commonware_storage::Context + commonware_runtime::BufferPooler {
    match decode_reply(&c.query(&encode_query(&CommentQuery::Thread { thread_id: id.into() })).await.unwrap()).unwrap() {
        CommentReply::Thread(v) => v, _ => panic!("expected Thread"),
    }
}
fn anchor(t: &str) -> Anchor { Anchor { module: "pages".into(), target: t.into() } }

#[test]
fn synced_store_reconstructs_source_root() {
    deterministic::Runner::default().start(|context| async move {
        let mut src = Comments::init(context.child("src"), "src").await;
        apply_commit(&mut src, &CommentMsg::AddComment { thread_id: "t1".into(), comment_id: "m1".into(), anchor: anchor("b1"), text: "draft".into() }).await;
        apply_commit(&mut src, &CommentMsg::AddComment { thread_id: "t1".into(), comment_id: "m2".into(), anchor: anchor("b1"), text: "doomed".into() }).await;
        apply_commit(&mut src, &CommentMsg::EditComment { comment_id: "m1".into(), text: "final".into() }).await; // overwrite
        apply_commit(&mut src, &CommentMsg::DeleteComment { comment_id: "m2".into() }).await; // delete rides the log
        let src_root = src.root();
        assert_ne!(src_root, StateRoot::ZERO);

        let target = src.sync_target().await;
        let resolver = src.into_resolver();
        let synced = Comments::sync_from(context.child("dst"), "dst", target, resolver).await.expect("sync_from");

        assert_eq!(synced.root(), src_root, "synced root must equal source root");
        let v = thread_of(&synced, "t1").await.unwrap();
        assert_eq!(v.comments.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(), ["final"]);
    });
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p comments --test sync_round_trip 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/apps/comments/tests/sync_round_trip.rs
git commit --no-gpg-sign -m "test(comments): state-sync round-trip reconstructs the source root"
```

---

### Task 12: Register `comments` in the host binaries

**Files:**
- Modify: `bin/node/src/main.rs` (`genesis_host` ~L609/L624 and `restore_host` ~L715)
- Modify: `bin/noded/src/main.rs` (~L281)
- Modify: `bin/simnode/src/main.rs` (~L402)
- Modify: `bin/demo/src/main.rs` (only if it registers `pages`)

**Interfaces:**
- Consumes: `comments::Comments`.
- Produces: a `comments` module registered at genesis + restore in every host binary, so the module set (and app-hash) includes it identically on every node.

- [ ] **Step 1: Add `comments` as a dependency of each bin.** In `bin/node/Cargo.toml`, `bin/noded/Cargo.toml`, `bin/simnode/Cargo.toml`, and `bin/demo/Cargo.toml`, add under `[dependencies]` (matching how `pages` is listed there):

```toml
comments = { workspace = true }
```

Confirm `pages = { workspace = true }` is present in each; if a bin has `pages = { path = ... }`, mirror that form for `comments`. Also add `comments` to the root `Cargo.toml` `[workspace.dependencies]` if `pages` is declared there (grep: `grep -n '^pages ' Cargo.toml`); if pages uses `{ workspace = true }` in the bins, add `comments = { path = "crates/apps/comments" }` to `[workspace.dependencies]`.

- [ ] **Step 2: Import and construct in `bin/node/src/main.rs`.** Add `use comments::Comments;` near `use pages::Pages;`. In BOTH `genesis_host` and `restore_host`, after the `let pages = Pages::init(context.child("pages"), "pages").await;` line, add:

```rust
    let comments = Comments::init(context.child("comments"), "comments").await;
```

And in `genesis_host`'s `Host::genesis(vec![…])` list, after `Box::new(pages),` add `Box::new(comments),`. In `restore_host`, add `Box::new(comments),` to the equivalent module vec at the same relative position (find where `pages` is boxed in the restore vec and add alongside).

- [ ] **Step 3: Repeat for `bin/noded`, `bin/simnode`, `bin/demo`.** Same two changes each (construct + box). Use the existing `pages` line in each file as the exact anchor. `grep -n "Pages::init\|Box::new(pages)" bin/*/src/main.rs` to find every site.

- [ ] **Step 4: Build to verify wiring**

Run: `cargo build -p node -p noded -p simnode 2>&1 | tail -20`
Expected: clean build. (If `restore_host` installs checkpoint snapshots per module, `comments` — a qmdb disk module — needs none, exactly like `pages`; confirm the restore path doesn't demand a snapshot for it.)

- [ ] **Step 5: Full workspace test to confirm nothing regressed**

Run: `cargo test -p pages -p comments 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add bin/*/src/main.rs bin/*/Cargo.toml Cargo.toml
git commit --no-gpg-sign -m "feat(node): register the comments module at genesis + restore"
```

---

## Phase 3 — Domain clients

### Task 13: `pages-client.ts` — parent, setPageParent, deletePage

**Files:**
- Modify: `app/src/domain/pages-client.ts`
- Test: `app/src/domain/pages-client.test.ts` (create)

**Interfaces:**
- Produces: `createPage(t, {pageId, title, parent?})`, `setPageParent(t, {pageId, parent})`, `deletePage(t, pageId)`, `PageMeta { id, title, parent: string | null }`.

- [ ] **Step 1: Write the failing test** — create `app/src/domain/pages-client.test.ts` with a fake transport capturing the wire payload:

```ts
import { describe, expect, it } from "vitest";
import { createPage, setPageParent, deletePage } from "./pages-client";
import type { NodeTransport } from "./transport";

function fakeTransport(sink: unknown[]): NodeTransport {
  return {
    submit: (target: string, payload: unknown) => {
      sink.push({ target, payload });
      return Promise.resolve({ height: 1, opHash: "x" } as never);
    },
    query: () => Promise.resolve({} as never),
    view: () => Promise.resolve({} as never),
  } as unknown as NodeTransport;
}

describe("pages-client nesting", () => {
  it("createPage carries snake_case parent", async () => {
    const sink: any[] = [];
    await createPage(fakeTransport(sink), { pageId: "p2", title: "c", parent: "p1" });
    expect(sink[0].payload).toEqual({ create_page: { page_id: "p2", title: "c", parent: "p1" } });
  });
  it("createPage without parent sends null", async () => {
    const sink: any[] = [];
    await createPage(fakeTransport(sink), { pageId: "p1", title: "r" });
    expect(sink[0].payload).toEqual({ create_page: { page_id: "p1", title: "r", parent: null } });
  });
  it("setPageParent + deletePage shapes", async () => {
    const sink: any[] = [];
    await setPageParent(fakeTransport(sink), { pageId: "p2", parent: null });
    await deletePage(fakeTransport(sink), "p2");
    expect(sink[0].payload).toEqual({ set_page_parent: { page_id: "p2", parent: null } });
    expect(sink[1].payload).toEqual({ delete_page: { page_id: "p2" } });
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd app && npx vitest run src/domain/pages-client.test.ts 2>&1 | tail -20`
Expected: FAIL — `parent`/`setPageParent`/`deletePage` not exported/handled.

- [ ] **Step 3: Update `pages-client.ts`.** Change `createPage` and `PageMeta`, add two functions:

```ts
export const createPage = (
  transport: NodeTransport,
  params: { pageId: string; title: string; parent?: string | null },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    create_page: { page_id: params.pageId, title: params.title, parent: params.parent ?? null },
  });

export const setPageParent = (
  transport: NodeTransport,
  params: { pageId: string; parent: string | null },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    set_page_parent: { page_id: params.pageId, parent: params.parent },
  });

export const deletePage = (
  transport: NodeTransport,
  pageId: string,
): Promise<BlockEvent> =>
  transport.submit(TARGET, { delete_page: { page_id: pageId } });
```

And extend `PageMeta`:

```ts
export interface PageMeta {
  id: string;
  title: string;
  /** Folder parent page id, or null for a top-level page. */
  parent: string | null;
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd app && npx vitest run src/domain/pages-client.test.ts 2>&1 | tail -20`
Expected: PASS. (If `NodeTransport`'s real shape differs from the fake's minimal cast, keep the `as unknown as NodeTransport` cast — the test only exercises `submit`.)

- [ ] **Step 5: Commit**

```bash
git add app/src/domain/pages-client.ts app/src/domain/pages-client.test.ts
git commit --no-gpg-sign -m "feat(pages-client): parent on createPage; setPageParent + deletePage"
```

---

### Task 14: `comments-client.ts` (new)

**Files:**
- Create: `app/src/domain/comments-client.ts`
- Test: `app/src/domain/comments-client.test.ts`

**Interfaces:**
- Produces: types `Anchor`, `AuthorRef`, `Thread`, `Comment`, `ThreadView`, `AnchorThreads`; fns `addComment`, `editComment`, `deleteComment`, `resolveThread`, `threadsForAnchors`, `getThread`. `AuthorRef` is imported from `chat-client.ts` (same JS boundary; identical shape) to reuse `authorName`.

- [ ] **Step 1: Write the failing test** — `app/src/domain/comments-client.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { addComment, resolveThread, threadsForAnchors } from "./comments-client";
import type { NodeTransport } from "./transport";

function fake(sink: unknown[], reply: unknown = {}): NodeTransport {
  return {
    submit: (target: string, payload: unknown) => { sink.push({ target, payload }); return Promise.resolve({} as never); },
    query: (target: string, payload: unknown) => { sink.push({ target, payload }); return Promise.resolve(reply as never); },
    view: () => Promise.resolve({} as never),
  } as unknown as NodeTransport;
}

describe("comments-client", () => {
  it("addComment wire shape", async () => {
    const sink: any[] = [];
    await addComment(fake(sink), { threadId: "t1", commentId: "c1", anchor: { module: "pages", target: "b1" }, text: "hi" });
    expect(sink[0]).toEqual({ target: "comments", payload: { add_comment: { thread_id: "t1", comment_id: "c1", anchor: { module: "pages", target: "b1" }, text: "hi" } } });
  });
  it("resolveThread wire shape", async () => {
    const sink: any[] = [];
    await resolveThread(fake(sink), { threadId: "t1", resolved: true });
    expect(sink[0].payload).toEqual({ resolve_thread: { thread_id: "t1", resolved: true } });
  });
  it("threadsForAnchors decodes the anchored reply", async () => {
    const sink: any[] = [];
    const reply = { anchored: [{ target: "b1", threads: [] }] };
    const out = await threadsForAnchors(fake(sink, reply), { module: "pages", targets: ["b1"] });
    expect(out).toEqual([{ target: "b1", threads: [] }]);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd app && npx vitest run src/domain/comments-client.test.ts 2>&1 | tail -20`
Expected: FAIL — module not found.

- [ ] **Step 3: Create `app/src/domain/comments-client.ts`** (mirror `pages-client.ts` structure — `replyVariant` for reply decoding):

```ts
// Typed client for the node's `comments` module — the TS mirror of
// `crates/apps/comments`. A thread anchors to {module, target} (a pages block
// or page id); authorship is derived by the module from the submit origin, so
// it appears only in replies. Pure functions over an injected NodeTransport,
// same contract as pages-client/chat-client.

import type { NodeTransport } from "./transport";
import type { AuthorRef } from "./chat-client";
import { replyVariant } from "./wire";

export type { AuthorRef };

export interface Anchor {
  module: string;
  target: string;
}

export interface Comment {
  id: string;
  thread_id: string;
  author: AuthorRef;
  text: string;
  created_at: number;
  edited_at: number | null;
  deleted: boolean;
}

export interface Thread {
  id: string;
  anchor: Anchor;
  opener: AuthorRef;
  created_at: number;
  resolved: boolean;
  resolved_by: AuthorRef | null;
  comment_ids: string[];
}

export interface ThreadView {
  thread: Thread;
  comments: Comment[];
}

export interface AnchorThreads {
  target: string;
  threads: ThreadView[];
}

const TARGET = "comments";

export const addComment = (
  transport: NodeTransport,
  params: { threadId: string; commentId: string; anchor: Anchor; text: string },
): Promise<unknown> =>
  transport.submit(TARGET, {
    add_comment: {
      thread_id: params.threadId,
      comment_id: params.commentId,
      anchor: params.anchor,
      text: params.text,
    },
  });

export const editComment = (
  transport: NodeTransport,
  params: { commentId: string; text: string },
): Promise<unknown> =>
  transport.submit(TARGET, { edit_comment: { comment_id: params.commentId, text: params.text } });

export const deleteComment = (
  transport: NodeTransport,
  commentId: string,
): Promise<unknown> =>
  transport.submit(TARGET, { delete_comment: { comment_id: commentId } });

export const resolveThread = (
  transport: NodeTransport,
  params: { threadId: string; resolved: boolean },
): Promise<unknown> =>
  transport.submit(TARGET, { resolve_thread: { thread_id: params.threadId, resolved: params.resolved } });

export const threadsForAnchors = (
  transport: NodeTransport,
  params: { module: string; targets: string[] },
): Promise<AnchorThreads[]> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, { threads_for_anchors: { module: params.module, targets: params.targets } }))
    .then((reply) => replyVariant<AnchorThreads[]>(reply, "anchored"));

export const getThread = (
  transport: NodeTransport,
  threadId: string,
): Promise<ThreadView | null> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, { thread: { thread_id: threadId } }))
    .then((reply) => replyVariant<ThreadView | null>(reply, "thread"));
```

Verify `AuthorRef` is exported from `chat-client.ts` (grep: `grep -n "AuthorRef" app/src/domain/chat-client.ts`); it is used by `authorName`. If the export name differs, import that type instead.

- [ ] **Step 4: Run test to verify it passes**

Run: `cd app && npx vitest run src/domain/comments-client.test.ts 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add app/src/domain/comments-client.ts app/src/domain/comments-client.test.ts
git commit --no-gpg-sign -m "feat(comments-client): typed client for the comments module"
```

---

## Phase 4 — Store

### Task 15: Tab state — `openTabs`, persistence, open/close

**Files:**
- Modify: `app/src/console/store/state.ts`
- Modify: `app/src/console/store/actions.ts`
- Modify: `app/src/console/store/finalization.ts` (opKey additions)
- Test: `app/src/console/store/tabs.test.ts` (create)

**Interfaces:**
- Produces: `ConsoleState.openTabs: string[]`; `loadDocTabs()`/`saveDocTabs()`; pure helpers `addTab(tabs, id)` / `removeTab(tabs, active, id)` in `state.ts`; actions `openPage` (tab-aware) and `closeTab(pageId)`; `opKey.comment`/`opKey.commentThread`.

- [ ] **Step 1: Write the failing test** — `app/src/console/store/tabs.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { addTab, removeTab } from "./state";

describe("doc tabs", () => {
  it("addTab appends unique, preserves order", () => {
    expect(addTab([], "a")).toEqual(["a"]);
    expect(addTab(["a"], "b")).toEqual(["a", "b"]);
    expect(addTab(["a", "b"], "a")).toEqual(["a", "b"]);
  });
  it("removeTab drops the id and picks a neighbor as next active", () => {
    // closing the active middle tab activates the following neighbor.
    expect(removeTab(["a", "b", "c"], "b", "b")).toEqual({ tabs: ["a", "c"], active: "c" });
    // closing the active last tab activates the previous.
    expect(removeTab(["a", "b"], "b", "b")).toEqual({ tabs: ["a"], active: "a" });
    // closing a non-active tab keeps the active one.
    expect(removeTab(["a", "b", "c"], "a", "c")).toEqual({ tabs: ["a", "b"], active: "a" });
    // closing the last remaining tab clears active.
    expect(removeTab(["a"], "a", "a")).toEqual({ tabs: [], active: null });
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd app && npx vitest run src/console/store/tabs.test.ts 2>&1 | tail -20`
Expected: FAIL — `addTab`/`removeTab` not exported.

- [ ] **Step 3: Add state field + helpers + persistence to `state.ts`.** Add `openTabs: string[];` to `ConsoleState` (right after `activePageBlocks`), default `openTabs: loadDocTabs(),` in `createInitialState` (it does NOT come from the snapshot — leave `ConsoleSnapshot`/`applySnapshot` untouched). Add near the view-mode persistence block:

```ts
// ── Doc tab persistence ─────────────────────────────────
// The open Docs tabs survive restart as a single id list; on load they are
// filtered against the live page enumeration (a stale id from another
// workspace simply drops), so no per-workspace keying is needed.
const DOC_TABS_KEY = "ducktape.docTabs";

export const loadDocTabs = (): string[] => {
  try {
    const raw = localStorage.getItem(DOC_TABS_KEY);
    const parsed = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed) ? parsed.filter((x): x is string => typeof x === "string") : [];
  } catch {
    return [];
  }
};

export const saveDocTabs = (tabs: string[]): void => {
  try {
    localStorage.setItem(DOC_TABS_KEY, JSON.stringify(tabs));
  } catch {
    // best-effort
  }
};

/** Append `id` if absent (order preserved). */
export const addTab = (tabs: string[], id: string): string[] =>
  tabs.includes(id) ? tabs : [...tabs, id];

/** Remove `id`; if it was active, pick the following neighbor (else previous,
 *  else null) as the next active tab. */
export const removeTab = (
  tabs: string[],
  active: string | null,
  id: string,
): { tabs: string[]; active: string | null } => {
  const idx = tabs.indexOf(id);
  const next = tabs.filter((t) => t !== id);
  if (active !== id) return { tabs: next, active };
  const neighbor = next[idx] ?? next[idx - 1] ?? null;
  return { tabs: next, active: neighbor };
};
```

- [ ] **Step 4: Add opKeys** to `finalization.ts` `opKey`:

```ts
  comment: (commentId: string) => `comment/${commentId}`,
  commentThread: (threadId: string) => `comment-thread/${threadId}`,
```

- [ ] **Step 5: Make `openPage` tab-aware + add `closeTab`.** In `actions.ts`, the `enterPage` helper currently patches `activePage`. Wrap tab bookkeeping around it. Replace `enterPage` and the `openPage: enterPage,` binding:

```ts
  const enterPage = (pageId: string) => {
    const live = getNode();
    if (!live || !pageId) return;
    const tabs = addTab(getState().openTabs, pageId);
    saveDocTabs(tabs);
    patch({ activePage: pageId, activePageBlocks: [], openTabs: tabs });
    Promise.resolve()
      .then(() => pagesClient.getPage(live, pageId))
      .then((blocks) => patch({ activePageBlocks: blocks ?? [] }))
      .then(() => loadPageThreads()) // defined in Task 17
      .catch(fail);
  };
```

Add a `closeTab` action in the returned actions object (near `openPage`):

```ts
    closeTab: (pageId: string) => {
      const { tabs, active } = removeTab(getState().openTabs, getState().activePage, pageId);
      saveDocTabs(tabs);
      if (active && active !== getState().activePage) {
        enterPage(active); // load the newly-active tab's tree
        return;
      }
      patch({ openTabs: tabs, activePage: active, activePageBlocks: active ? getState().activePageBlocks : [] });
    },
```

Import `addTab`, `removeTab`, `saveDocTabs`, `loadDocTabs` from `./state`. Declare `openPage`, `closeTab` in the actions type (`actions.ts` interface near `openPage(pageId: string): void;`): add `closeTab(pageId: string): void;`.

Note: until Task 17 defines `loadPageThreads`, stub it as `const loadPageThreads = () => Promise.resolve();` at the top of `createActions`, replaced in Task 17.

- [ ] **Step 6: Filter tabs against enumeration.** In `DucktapeProvider.tsx` refresh, after `pages` is fetched, drop tabs whose page no longer exists. In the `applySnapshot(...)` patch call, the provider dispatches a `patch`; add an openTabs reconciliation. Simplest: in the `.then([...])` handler, compute `const liveIds = new Set(pages.map((p) => p.id)); const keptTabs = stateRef.current.openTabs.filter((id) => liveIds.has(id));` and include `openTabs: keptTabs` plus `activePage: liveIds.has(stateRef.current.activePage ?? "") ? stateRef.current.activePage : (keptTabs[0] ?? null)` in the dispatched patch (merge into the object passed to `dispatch`). Persist with `saveDocTabs(keptTabs)`.

- [ ] **Step 7: Run tests to verify they pass**

Run: `cd app && npx vitest run src/console/store/tabs.test.ts 2>&1 | tail -20`
Expected: PASS. Then `cd app && npx tsc --noEmit 2>&1 | tail -20` — no type errors from the new field.

- [ ] **Step 8: Commit**

```bash
git add app/src/console/store/state.ts app/src/console/store/actions.ts app/src/console/store/finalization.ts app/src/console/store/tabs.test.ts app/src/console/store/DucktapeProvider.tsx
git commit --no-gpg-sign -m "feat(store): doc tabs — openTabs state, persistence, open/close, enumeration reconcile"
```

---

### Task 16: Pages actions — child page, set parent, delete page

**Files:**
- Modify: `app/src/console/store/actions.ts`
- Modify: `app/src/console/store/actions.ts` interface (the `DucktapeActions`-style type near the top)

**Interfaces:**
- Consumes: `pagesClient.createPage/setPageParent/deletePage`, `submitTracked`, `opKey.page`, `enterPage`, `closeTab`.
- Produces: `createPage(title?)` (now allows empty → "Untitled" untitled flow via `createChildPage`), `createChildPage(parent: string | null)`, `setPageParent({pageId, parent})`, `deletePage(pageId)`.

- [ ] **Step 1: Write the failing test** — extend an existing store action test or create `app/src/console/store/pages-actions.test.ts`. Use the existing store test harness pattern (see `optimistic.test.ts` / `DucktapeProvider.test.tsx`) with a fake node transport that records submits. Minimal shape:

```ts
import { describe, expect, it, vi } from "vitest";
import { createActions } from "./actions";
// NOTE: follow the existing harness in DucktapeProvider.test.tsx for building
// `deps` (dispatch/getState/getNode/ws/bootstrap). Assert that createChildPage
// submits a create_page with the given parent and then opens the new page.

it("createChildPage submits create_page with parent then opens it", async () => {
  const submits: any[] = [];
  const node = {
    submit: (t: string, p: any) => { submits.push({ t, p }); return Promise.resolve({ height: 1, opHash: "h" }); },
    query: () => Promise.resolve(null),
    getPage: () => Promise.resolve([]),
  };
  // ... build deps with getNode: () => node, getState returning a state with openTabs: [] ...
  // const actions = createActions(deps);
  // actions.createChildPage("parent");
  // await flush();
  // expect(submits[0].p.create_page.parent).toBe("parent");
});
```

Because the store harness is nontrivial, if wiring a full `createActions` unit test is heavy, instead assert the behavior at the client seam (already covered by Task 13) and add a focused reducer/helper test only for `addTab`/`removeTab` (Task 15). Mark this step done once either a `createActions`-level test passes OR the Task 13 client test + a manual real-window check (Task 23) covers it. Prefer the `createActions` test if the harness in `DucktapeProvider.test.tsx` is reusable.

- [ ] **Step 2: Implement the actions.** In `actions.ts`, replace `createPage` and add the three new actions in the `// ── Docs ──` region:

```ts
    // create a page (optionally nested) and open it; an empty title is allowed
    // — the doc title input is where naming happens (Notion-style instant page).
    createChildPage: (parent: string | null) => {
      const pageId = crypto.randomUUID();
      submitTracked(
        opKey.page(pageId),
        (live) => pagesClient.createPage(live, { pageId, title: "", parent }),
        (prev) => optimistic.pageCreated(prev, { pageId, title: "" }),
      ).then(() => enterPage(pageId));
    },

    // kept for callers that pass a title (e.g. programmatic/tests); empty is a
    // no-op only when a title string is explicitly required. Prefer createChildPage.
    createPage: (title: string) => {
      const pageId = crypto.randomUUID();
      submitTracked(
        opKey.page(pageId),
        (live) => pagesClient.createPage(live, { pageId, title: title.trim() }),
        (prev) => optimistic.pageCreated(prev, { pageId, title: title.trim() }),
      ).then(() => enterPage(pageId));
    },

    setPageParent: ({ pageId, parent }: { pageId: string; parent: string | null }) => {
      submitTracked(opKey.page(pageId), (live) => pagesClient.setPageParent(live, { pageId, parent }));
    },

    deletePage: (pageId: string) => {
      if (!pageId) return;
      submitTracked(opKey.page(pageId), (live) => pagesClient.deletePage(live, pageId))
        .then(() => actions.listPages())
        .catch(fail);
      // close its tab immediately (optimistic UX).
      closeTabById(pageId);
    },
```

`closeTabById` = the `closeTab` action logic (Task 15). If `closeTab` is defined in the returned object, call it via a local helper hoisted above, or inline the `removeTab` bookkeeping. Add `createChildPage`, `setPageParent`, `deletePage` to the actions type interface.

- [ ] **Step 3: Run type check + tests**

Run: `cd app && npx tsc --noEmit 2>&1 | tail -20 && npx vitest run src/console/store 2>&1 | tail -20`
Expected: no type errors; store tests pass.

- [ ] **Step 4: Commit**

```bash
git add app/src/console/store/actions.ts
git commit --no-gpg-sign -m "feat(store): createChildPage (instant nested page), setPageParent, deletePage"
```

---

### Task 17: Comment store state + actions

**Files:**
- Modify: `app/src/console/store/state.ts`
- Modify: `app/src/console/store/actions.ts`
- Test: `app/src/console/store/comments-actions.test.ts` (create, best-effort per Task 16's harness note)

**Interfaces:**
- Produces: `ConsoleState.pageThreads: AnchorThreads[]`; actions `loadPageThreads()`, `addComment({threadId?, anchor, text})`, `editComment({commentId, text})`, `deleteComment(commentId)`, `resolveThread({threadId, resolved})`.

- [ ] **Step 1: Add `pageThreads` state.** In `state.ts` add `pageThreads: AnchorThreads[];` (import `AnchorThreads` from `../../domain/comments-client`) in the Docs section, default `pageThreads: [],` in `createInitialState`. Not part of the snapshot.

- [ ] **Step 2: Implement `loadPageThreads` + comment actions.** In `actions.ts`, define `loadPageThreads` (replacing the Task-15 stub) and the comment write actions. `loadPageThreads` gathers the active page id + its block ids as targets:

```ts
  const loadPageThreads = (): Promise<void> => {
    const live = getNode();
    const page = getState().activePage;
    if (!live || !page) {
      patch({ pageThreads: [] });
      return Promise.resolve();
    }
    const targets = [page, ...getState().activePageBlocks.map((b) => b.id)];
    return commentsClient
      .threadsForAnchors(live, { module: "pages", targets })
      .then((pageThreads) => patch({ pageThreads }))
      .catch(fail);
  };
```

Comment write actions (in the returned object, after the Docs block):

```ts
    loadPageThreads: () => { void loadPageThreads(); },

    addComment: ({ threadId, anchor, text }: { threadId?: string; anchor: { module: string; target: string }; text: string }) => {
      const clean = text.trim();
      if (!clean) return;
      const tid = threadId ?? crypto.randomUUID();
      const commentId = crypto.randomUUID();
      submitTracked(opKey.commentThread(tid), (live) =>
        commentsClient.addComment(live, { threadId: tid, commentId, anchor, text: clean }),
      ).then(() => loadPageThreads());
    },

    editComment: ({ commentId, text }: { commentId: string; text: string }) => {
      const clean = text.trim();
      if (!clean) return;
      submitTracked(opKey.comment(commentId), (live) =>
        commentsClient.editComment(live, { commentId, text: clean }),
      ).then(() => loadPageThreads());
    },

    deleteComment: (commentId: string) => {
      submitTracked(opKey.comment(commentId), (live) => commentsClient.deleteComment(live, commentId))
        .then(() => loadPageThreads());
    },

    resolveThread: ({ threadId, resolved }: { threadId: string; resolved: boolean }) => {
      submitTracked(opKey.commentThread(threadId), (live) =>
        commentsClient.resolveThread(live, { threadId, resolved }),
      ).then(() => loadPageThreads());
    },
```

Import `* as commentsClient from "../../domain/comments-client"` (match how `pagesClient` is imported at the top of `actions.ts`). Add all five to the actions type interface.

- [ ] **Step 3: Type check + tests**

Run: `cd app && npx tsc --noEmit 2>&1 | tail -20 && npx vitest run src/console/store 2>&1 | tail -20`
Expected: no type errors; store tests pass.

- [ ] **Step 4: Commit**

```bash
git add app/src/console/store/state.ts app/src/console/store/actions.ts
git commit --no-gpg-sign -m "feat(store): comment threads state + add/edit/delete/resolve/load actions"
```

---

## Phase 5 — View

The view work lands in focused files under `app/src/console/views/pages/`. Split `PagesView.tsx` (currently ~1100 lines) so each new surface is its own file: `DocTabs.tsx`, `PageTree.tsx`, `CommentsPanel.tsx`. `BlockRow` and the editor stay in `PagesView.tsx` (or extract to `BlockRow.tsx` if the diff gets unwieldy).

### Task 18: Clutter removal + focus-only placeholder + copy-link on hover

**Files:**
- Modify: `app/src/console/views/pages/PagesView.tsx`
- Test: `app/src/console/views/pages/PagesView.test.tsx`

**Interfaces:**
- Produces: a `BlockRow` with no permanent hash chip (a hover-only "Copy link"), placeholder only on the focused empty block, and a header/rail with no block-count or "block trees" chrome.

- [ ] **Step 1: Write the failing test.** Extend `PagesView.test.tsx` to assert the clutter is gone and the placeholder is focus-gated. Add:

```tsx
it("does not render block hash chips or a block-count in steady state", () => {
  // render PagesView with an open page of 2 blocks (reuse the file's existing
  // render helper / store harness).
  // The mono short-id chip text pattern (e.g. /[0-9a-f]{8}…/) must be absent,
  // and no "blocks" counter text in the header.
  const { queryByText, container } = renderPagesWithPage(twoBlockPage);
  expect(container.querySelector('[data-testid="block-hash-chip"]')).toBeNull();
  expect(queryByText(/\bblocks\b/i)).toBeNull();
});

it("shows the placeholder only on the focused empty block", () => {
  const { getAllByRole } = renderPagesWithPage(twoEmptyBlocksPage);
  const areas = getAllByRole("textbox");
  // none focused → no placeholder text on either.
  expect(areas.every((a) => (a as HTMLTextAreaElement).placeholder === "")).toBe(true);
  areas[0].focus();
  // after focus, only the focused one carries the copy.
  expect((areas[0] as HTMLTextAreaElement).placeholder).toBe("Write, or press '/' for commands");
  expect((areas[1] as HTMLTextAreaElement).placeholder).toBe("");
});
```

If `PagesView.test.tsx` lacks a `renderPagesWithPage` helper, add one modeled on the file's existing setup (a store provider seeded via `activePage`/`activePageBlocks`).

- [ ] **Step 2: Run test to verify it fails**

Run: `cd app && npx vitest run src/console/views/pages/PagesView.test.tsx 2>&1 | tail -20`
Expected: FAIL — chips present, placeholder always shown.

- [ ] **Step 3: Remove the clutter in `PagesView.tsx`.**
  - In `BlockRow`, delete the hash-chip `<button>` (the one rendering `<Icon name="hash" />` + `shortId(block.id)`, ~L496–517) and replace with a hover-only "Copy link" control. Track hover via `onMouseEnter`/`onMouseLeave` on the row wrapper (`const [hover, setHover] = useState(false)`), and render the copy control only when `hover`:

```tsx
{hover ? (
  <button
    type="button"
    aria-label={`Copy link to block ${blockNumber}`}
    title="Copy block link"
    onClick={() => { void navigator.clipboard?.writeText(block.id); }}
    style={{ all: "unset", cursor: "pointer", width: 20, height: 20, borderRadius: 5,
      color: color.muted2, display: "flex", alignItems: "center", justifyContent: "center" }}
  >
    <Icon name="link" size={11} />
  </button>
) : null}
```

(If `Icon` has no `"link"` glyph, use an existing one — grep `app/src/console/components/Icon.tsx` for available names; `hash` is acceptable as a fallback but prefer a link/anchor glyph.)
  - Remove the `#shortId(root.id)` + `FinalizationMark` row under the title in `PagesView` (the block ~L1072–1089). Keep the title input.
  - In the main `<header>`, remove the `{root.text || "Untitled"}` mono id line and the `{rows.length} blocks` counter (~L958–981). Leave the "Docs" label + page title breadcrumb.
  - In `PageRail`, remove the `block trees` subtitle (~L601–603) and the `{pages.length}` counter (~L605–607); keep the "Docs" heading.

- [ ] **Step 4: Focus-gate the placeholder.** In `BlockRow`, add `const [focused, setFocused] = useState(false)` and wire `onFocus={() => setFocused(true)}` / `onBlur={() => { setFocused(false); maybeCommit(); }}` on the `<textarea>`. Change the `placeholder` prop:

```tsx
placeholder={focused && draft === "" ? focusPlaceholder(block.kind) : ""}
```

Add a helper replacing `kindPlaceholder` usage for the paragraph default:

```tsx
function focusPlaceholder(kind: BlockKind): string {
  const k = kindPlaceholder(kind);
  return k === "Type '/' for commands" ? "Write, or press '/' for commands" : k;
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd app && npx vitest run src/console/views/pages/PagesView.test.tsx 2>&1 | tail -20`
Expected: PASS. Fix any pre-existing tests that asserted the old chips/counter.

- [ ] **Step 6: Commit**

```bash
git add app/src/console/views/pages/PagesView.tsx app/src/console/views/pages/PagesView.test.tsx
git commit --no-gpg-sign -m "feat(docs-view): drop block-id/count clutter; focus-only placeholder; hover copy-link"
```

---

### Task 19: Instant new-page flow

**Files:**
- Modify: `app/src/console/views/pages/PagesView.tsx`
- Test: `app/src/console/views/pages/PagesView.test.tsx`

**Interfaces:**
- Consumes: `actions.createChildPage`.
- Produces: `PageRail` with no title-input form; a single `+ New page` button calling `createChildPage(null)`; the title input auto-focuses on a freshly created empty page.

- [ ] **Step 1: Write the failing test:**

```tsx
it("New page button creates a page without a title form", () => {
  const createChildPage = vi.fn();
  const { getByRole, queryByLabelText } = renderRail({ actions: { createChildPage } });
  expect(queryByLabelText("New page title")).toBeNull(); // the form is gone
  getByRole("button", { name: /new page/i }).click();
  expect(createChildPage).toHaveBeenCalledWith(null);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd app && npx vitest run src/console/views/pages/PagesView.test.tsx -t "New page" 2>&1 | tail -20`
Expected: FAIL — the form still exists.

- [ ] **Step 3: Replace the form.** In `PageRail`, delete the `<form onSubmit={onCreate}>` block (the "New page title" label + input + submit button, ~L610–658) and its props (`newTitle`, `setNewTitle`, `onCreate`). Add a `+ New page` button at the top of the list region:

```tsx
<button
  type="button"
  aria-label="New page"
  onClick={() => onNewPage()}
  style={{ all: "unset", cursor: "pointer", display: "flex", alignItems: "center", gap: 8,
    margin: "12px 10px", padding: "8px 10px", borderRadius: radius.sm, background: color.dark,
    color: color.onDark, font: `600 12.5px ${font.sans}` }}
>
  <Icon name="plus" size={14} strokeWidth={1.9} /> New page
</button>
```

Thread a new `onNewPage: () => void` prop into `PageRail`; in `PagesView` pass `onNewPage={() => actions.createChildPage(null)}`. Remove `newTitle`/`setNewTitle` state and the `create` handler from `PagesView`.

- [ ] **Step 4: Auto-focus the title on an empty page.** In `PagesView`, after `enterPage` opens a page whose root text is empty, focus `titleRef`. Add an effect:

```tsx
useEffect(() => {
  if (root && root.text === "" && titleDraft === "") titleRef.current?.focus();
}, [root?.id]);
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd app && npx vitest run src/console/views/pages/PagesView.test.tsx 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add app/src/console/views/pages/PagesView.tsx app/src/console/views/pages/PagesView.test.tsx
git commit --no-gpg-sign -m "feat(docs-view): instant untitled-page flow (no title form)"
```

---

### Task 20: Document tab strip

**Files:**
- Create: `app/src/console/views/pages/DocTabs.tsx`
- Modify: `app/src/console/views/pages/PagesView.tsx`
- Test: `app/src/console/views/pages/DocTabs.test.tsx`

**Interfaces:**
- Consumes: `state.openTabs`, `state.pages` (for titles), `state.activePage`, `actions.openPage`, `actions.closeTab`.
- Produces: `<DocTabs open={string[]} active={string|null} titleOf={(id)=>string} onSelect onClose />`.

- [ ] **Step 1: Write the failing test** — `DocTabs.test.tsx`:

```tsx
import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/react";
import { DocTabs } from "./DocTabs";

it("renders a tab per open page and fires select/close", () => {
  const onSelect = vi.fn(), onClose = vi.fn();
  const { getByRole } = render(
    <DocTabs open={["p1", "p2"]} active="p1" titleOf={(id) => (id === "p1" ? "Alpha" : "")} onSelect={onSelect} onClose={onClose} />,
  );
  getByRole("tab", { name: /alpha/i }).click();
  expect(onSelect).toHaveBeenCalledWith("p1");
  getByRole("button", { name: /close .*untitled/i }).click(); // p2 has empty title → "Untitled"
  expect(onClose).toHaveBeenCalledWith("p2");
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd app && npx vitest run src/console/views/pages/DocTabs.test.tsx 2>&1 | tail -20`
Expected: FAIL — module not found.

- [ ] **Step 3: Create `DocTabs.tsx`:**

```tsx
import type { CSSProperties } from "react";
import { Icon } from "../../components/Icon";
import { color, font, radius } from "../../theme/tokens";

export function DocTabs({
  open,
  active,
  titleOf,
  onSelect,
  onClose,
}: {
  open: string[];
  active: string | null;
  titleOf: (id: string) => string;
  onSelect: (id: string) => void;
  onClose: (id: string) => void;
}) {
  if (open.length === 0) return null;
  return (
    <div
      role="tablist"
      aria-label="Open documents"
      style={{ display: "flex", alignItems: "stretch", gap: 2, height: 38, flexShrink: 0,
        padding: "0 8px", borderBottom: `1px solid ${color.borderSoft}`, background: color.sidebar,
        overflowX: "auto" }}
    >
      {open.map((id) => {
        const isActive = id === active;
        const label = titleOf(id) || "Untitled";
        const tabStyle: CSSProperties = {
          display: "flex", alignItems: "center", gap: 6, padding: "0 8px 0 11px", maxWidth: 200,
          cursor: "pointer", borderTopLeftRadius: radius.sm, borderTopRightRadius: radius.sm,
          borderBottom: isActive ? `2px solid ${color.dark}` : "2px solid transparent",
          background: isActive ? color.paper : "transparent",
          color: isActive ? color.ink : color.inkSofter, font: `${isActive ? 600 : 500} 12px ${font.sans}`,
        };
        return (
          <div
            key={id}
            role="tab"
            aria-selected={isActive}
            aria-label={label}
            tabIndex={0}
            onClick={() => onSelect(id)}
            onKeyDown={(e) => { if (e.key === "Enter") onSelect(id); }}
            onAuxClick={(e) => { if (e.button === 1) onClose(id); }}
            style={tabStyle}
          >
            <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{label}</span>
            <button
              type="button"
              aria-label={`Close ${label}`}
              onClick={(e) => { e.stopPropagation(); onClose(id); }}
              style={{ all: "unset", cursor: "pointer", width: 16, height: 16, borderRadius: 4,
                display: "flex", alignItems: "center", justifyContent: "center", color: color.muted2 }}
            >
              <Icon name="close" size={10} />
            </button>
          </div>
        );
      })}
    </div>
  );
}
```

- [ ] **Step 4: Mount it in `PagesView`** above the `<header>` inside `<main>`:

```tsx
<DocTabs
  open={state.openTabs}
  active={state.activePage}
  titleOf={(id) => state.pages.find((p) => p.id === id)?.title ?? ""}
  onSelect={actions.openPage}
  onClose={actions.closeTab}
/>
```

Import `DocTabs`. Verify `color.inkSofter`/`color.muted2` exist in `theme/tokens` (they are used elsewhere in this file).

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd app && npx vitest run src/console/views/pages/DocTabs.test.tsx 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add app/src/console/views/pages/DocTabs.tsx app/src/console/views/pages/DocTabs.test.tsx app/src/console/views/pages/PagesView.tsx
git commit --no-gpg-sign -m "feat(docs-view): document tab strip"
```

---

### Task 21: Nested sidebar tree

**Files:**
- Create: `app/src/console/views/pages/PageTree.tsx`
- Create: `app/src/console/views/pages/page-tree.ts` (pure forest builder)
- Modify: `app/src/console/views/pages/PagesView.tsx` (replace the flat list in `PageRail`)
- Test: `app/src/console/views/pages/page-tree.test.ts`

**Interfaces:**
- Produces: `buildForest(pages: PageMeta[]): TreeNode[]` where `TreeNode { id, title, depth, children: TreeNode[] }` (sorted by title within each parent); `<PageTree nodes activeId collapsed onToggle onOpen onAddChild onDelete onMove />`.

- [ ] **Step 1: Write the failing test** — `page-tree.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { buildForest, flattenVisible } from "./page-tree";
import type { PageMeta } from "../../../domain/pages-client";

const pm = (id: string, title: string, parent: string | null): PageMeta => ({ id, title, parent });

describe("page forest", () => {
  it("nests children under parents, sorted by title", () => {
    const forest = buildForest([
      pm("a", "Alpha", null),
      pm("b", "Bravo", "a"),
      pm("c", "Able", "a"),
      pm("d", "Delta", null),
    ]);
    expect(forest.map((n) => n.id)).toEqual(["a", "d"]); // roots by title: Alpha, Delta
    const a = forest[0];
    expect(a.children.map((n) => n.title)).toEqual(["Able", "Bravo"]); // sorted
    expect(a.children[0].depth).toBe(1);
  });
  it("orphans (missing parent) surface at root so nothing is hidden", () => {
    const forest = buildForest([pm("x", "X", "ghost")]);
    expect(forest.map((n) => n.id)).toEqual(["x"]);
  });
  it("flattenVisible hides children under a collapsed node", () => {
    const forest = buildForest([pm("a", "A", null), pm("b", "B", "a")]);
    const rows = flattenVisible(forest, new Set(["a"]));
    expect(rows.map((r) => r.id)).toEqual(["a"]);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd app && npx vitest run src/console/views/pages/page-tree.test.ts 2>&1 | tail -20`
Expected: FAIL — module not found.

- [ ] **Step 3: Create `page-tree.ts`:**

```ts
import type { PageMeta } from "../../../domain/pages-client";

export interface TreeNode {
  id: string;
  title: string;
  depth: number;
  children: TreeNode[];
}

const label = (m: PageMeta) => m.title || "Untitled";

/** Build the folder forest from the flat enumeration. A page whose parent is
 *  missing (or points outside the set) surfaces at the root, so nothing is ever
 *  hidden by a dangling edge. Children are sorted by title (case-insensitive). */
export function buildForest(pages: PageMeta[]): TreeNode[] {
  const byId = new Map(pages.map((p) => [p.id, p]));
  const childrenOf = new Map<string | null, PageMeta[]>();
  for (const p of pages) {
    const parent = p.parent && byId.has(p.parent) ? p.parent : null;
    const list = childrenOf.get(parent) ?? [];
    list.push(p);
    childrenOf.set(parent, list);
  }
  const build = (parent: string | null, depth: number): TreeNode[] =>
    (childrenOf.get(parent) ?? [])
      .slice()
      .sort((a, b) => label(a).toLowerCase().localeCompare(label(b).toLowerCase()))
      .map((p) => ({ id: p.id, title: label(p), depth, children: build(p.id, depth + 1) }));
  return build(null, 0);
}

export interface VisibleRow { id: string; title: string; depth: number; hasChildren: boolean; }

/** Preorder flatten, skipping the subtree of any collapsed node. */
export function flattenVisible(forest: TreeNode[], collapsed: ReadonlySet<string>): VisibleRow[] {
  const out: VisibleRow[] = [];
  const walk = (nodes: TreeNode[]) => {
    for (const n of nodes) {
      out.push({ id: n.id, title: n.title, depth: n.depth, hasChildren: n.children.length > 0 });
      if (n.children.length > 0 && !collapsed.has(n.id)) walk(n.children);
    }
  };
  walk(forest);
  return out;
}
```

- [ ] **Step 4: Run the pure test to verify it passes**

Run: `cd app && npx vitest run src/console/views/pages/page-tree.test.ts 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 5: Create `PageTree.tsx`** — renders `flattenVisible` rows with a disclosure chevron, per-row hover `+`/`⋯` (add child, delete, move-to picker). Model row styling on the existing flat-list button in `PageRail` (indent = `depth * 14`). Wire `onOpen(id)`, `onToggle(id)`, `onAddChild(id)`, `onDelete(id)`, `onMove(id, newParent)`. Move-to is a small dropdown listing every OTHER page (and "Top level"); guard against selecting a descendant client-side (reuse `buildForest` to compute the moved node's subtree). Provide `aria-label`s: row `Open <title>`, chevron `Collapse/Expand <title>`, add `Add page under <title>`, menu `More actions for <title>`.

```tsx
// Structure (abbreviated — full styling mirrors PageRail's list button):
export function PageTree({ nodes, activeId, collapsed, onOpen, onToggle, onAddChild, onDelete, onMove }: PageTreeProps) {
  const rows = flattenVisible(nodes, collapsed);
  return (
    <div role="tree" aria-label="Pages">
      {rows.map((row) => (
        <div role="treeitem" key={row.id} aria-expanded={row.hasChildren ? !collapsed.has(row.id) : undefined}
             style={{ display: "flex", alignItems: "center", paddingLeft: 8 + row.depth * 14 }}>
          {row.hasChildren ? (
            <button aria-label={`${collapsed.has(row.id) ? "Expand" : "Collapse"} ${row.title}`}
              onClick={() => onToggle(row.id)} /* chevron */>
              <Icon name="chevronRight" size={12} style={{ transform: `rotate(${collapsed.has(row.id) ? 0 : 90}deg)` }} />
            </button>
          ) : <span style={{ width: 12 }} />}
          <button aria-label={`Open ${row.title}`} onClick={() => onOpen(row.id)} /* label; active highlight when row.id === activeId */>
            {row.title}
          </button>
          {/* hover-revealed: onAddChild(row.id), and a ⋯ menu → onDelete(row.id) / onMove(row.id, target) */}
        </div>
      ))}
    </div>
  );
}
```

- [ ] **Step 6: Replace the flat list in `PageRail`.** Swap the `pages.map(...)` list body for `<PageTree nodes={buildForest(pages)} activeId={activePage} collapsed={collapsed} onOpen={openPage} onToggle={toggle} onAddChild={(id) => actions.createChildPage(id)} onDelete={(id) => confirmThenDelete(id)} onMove={(id, parent) => actions.setPageParent({ pageId: id, parent })} />`. Keep the empty-state card. Hold `collapsed` tree state in `PagesView` (a `useState<Set<string>>`), persist it best-effort to localStorage (`ducktape.docTreeCollapsed`) like `saveDocTabs`. `confirmThenDelete` uses `window.confirm(\`Delete "<title>" and its contents?\`)` before `actions.deletePage(id)`.

- [ ] **Step 7: Run the view tests + type check**

Run: `cd app && npx vitest run src/console/views/pages 2>&1 | tail -20 && npx tsc --noEmit 2>&1 | tail -20`
Expected: PASS, no type errors.

- [ ] **Step 8: Commit**

```bash
git add app/src/console/views/pages/PageTree.tsx app/src/console/views/pages/page-tree.ts app/src/console/views/pages/page-tree.test.ts app/src/console/views/pages/PagesView.tsx
git commit --no-gpg-sign -m "feat(docs-view): nested sidebar page tree with add-child/move/delete"
```

---

### Task 22: Comments UI — per-block bubble + panel

**Files:**
- Create: `app/src/console/views/pages/CommentsPanel.tsx`
- Modify: `app/src/console/views/pages/PagesView.tsx`
- Test: `app/src/console/views/pages/CommentsPanel.test.tsx`

**Interfaces:**
- Consumes: `state.pageThreads` (`AnchorThreads[]`), `state.authorNames`, `actions.addComment/editComment/deleteComment/resolveThread/loadPageThreads`, `authorName` from `chat-client`.
- Produces: `<CommentsPanel threads authorNames onReply onResolve onEdit onDelete />`; a per-`BlockRow` comment bubble with a live-thread count badge that opens the panel scrolled to that block's thread; a header "Comment on page" entry.

- [ ] **Step 1: Write the failing test** — `CommentsPanel.test.tsx`:

```tsx
import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/react";
import { CommentsPanel } from "./CommentsPanel";
import type { AnchorThreads } from "../../../domain/comments-client";

const threads: AnchorThreads[] = [{
  target: "b1",
  threads: [{
    thread: { id: "t1", anchor: { module: "pages", target: "b1" }, opener: { user: [1] } as any, created_at: 1, resolved: false, resolved_by: null, comment_ids: ["c1"] },
    comments: [{ id: "c1", thread_id: "t1", author: { user: [1] } as any, text: "hello", created_at: 1, edited_at: null, deleted: false }],
  }],
}];

it("lists threads and resolves", () => {
  const onResolve = vi.fn();
  const { getByText, getByRole } = render(
    <CommentsPanel threads={threads} authorNames={{}} onReply={vi.fn()} onResolve={onResolve} onEdit={vi.fn()} onDelete={vi.fn()} />,
  );
  getByText("hello");
  getByRole("button", { name: /resolve/i }).click();
  expect(onResolve).toHaveBeenCalledWith("t1", true);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd app && npx vitest run src/console/views/pages/CommentsPanel.test.tsx 2>&1 | tail -20`
Expected: FAIL — module not found.

- [ ] **Step 3: Create `CommentsPanel.tsx`** — a right-hand panel listing every `ThreadView` across `pageThreads` (page-anchored threads first, then per block), each showing its comments (author via `authorName(author, authorNames)`), a reply composer (`onReply(threadId, text)`), a resolve/reopen toggle (`onResolve(threadId, !resolved)`), and edit/delete on comments authored by the local user. Use the theme tokens and match the chat thread panel's visual language. Provide `aria-label`s: resolve button `Resolve thread`/`Reopen thread`, reply box `Reply to thread`.

```tsx
// Shape (abbreviated); full styling mirrors the chat thread panel.
export function CommentsPanel({ threads, authorNames, onReply, onResolve, onEdit, onDelete }: CommentsPanelProps) {
  const flat = threads.flatMap((g) => g.threads);
  if (flat.length === 0) return <EmptyState label="No comments yet" />;
  return (
    <aside aria-label="Comments" style={{ width: 320, borderLeft: `1px solid ${color.borderSoft}`, background: color.paper, display: "flex", flexDirection: "column" }}>
      {flat.map((view) => (
        <ThreadCard key={view.thread.id} view={view} authorNames={authorNames}
          onReply={onReply} onResolve={onResolve} onEdit={onEdit} onDelete={onDelete} />
      ))}
    </aside>
  );
}
```

- [ ] **Step 4: Per-block bubble + panel toggle in `PagesView`.**
  - Compute a `threadsByTarget = new Map(state.pageThreads.map((g) => [g.target, g.threads]))`; pass each `BlockRow` its live-thread count (`threadsByTarget.get(block.id)?.length ?? 0`). In `BlockRow`, render a hover comment bubble in the right gutter (next to the copy-link); when the block has threads, show it always with a count badge. Clicking it opens the panel (`setPanelOpen(true)`) and calls `onOpenComments(block.id)` (which opens a thread composer/anchor for that block).
  - Add a `state`-driven `panelOpen` local state + a header "Comment on page" button that opens the panel and starts a page-anchored thread (`anchor = { module: "pages", target: activePage }`).
  - Load threads when a page opens: `useEffect(() => { actions.loadPageThreads(); }, [state.activePage])`.
  - Render `<CommentsPanel .../>` to the right of the editor `<main>` when `panelOpen`, wiring `onReply={(threadId, text) => actions.addComment({ threadId, anchor: /* thread's anchor */, text })}`, `onResolve={(threadId, resolved) => actions.resolveThread({ threadId, resolved })}`, `onEdit={(commentId, text) => actions.editComment({ commentId, text })}`, `onDelete={(commentId) => actions.deleteComment(commentId)}`. New (composer) threads call `actions.addComment({ anchor, text })` with no `threadId` (a fresh id is minted in the action).

- [ ] **Step 5: Run tests + type check**

Run: `cd app && npx vitest run src/console/views/pages 2>&1 | tail -20 && npx tsc --noEmit 2>&1 | tail -20`
Expected: PASS, no type errors.

- [ ] **Step 6: Commit**

```bash
git add app/src/console/views/pages/CommentsPanel.tsx app/src/console/views/pages/CommentsPanel.test.tsx app/src/console/views/pages/PagesView.tsx
git commit --no-gpg-sign -m "feat(docs-view): Notion-style comments — per-block bubbles + thread panel"
```

---

### Task 23: Full build, lint/typecheck, real-window verification

**Files:**
- None (verification + any fixes surfaced).

**Interfaces:**
- Consumes: everything.

- [ ] **Step 1: Rust — full test + build.**

Run: `cargo test -p pages -p comments 2>&1 | tail -30 && cargo build -p node -p noded -p simnode 2>&1 | tail -20`
Expected: all green.

- [ ] **Step 2: Frontend — typecheck + tests + `make install` gate.**

Run: `cd app && npx tsc --noEmit 2>&1 | tail -20 && npx vitest run 2>&1 | tail -30`
Then the repo build gate: `make install 2>&1 | tail -20` (the split-tsconfig build gate — see project memory; a node-using test harness must not be named `*.test.ts` outside the excludes). Expected: clean.

- [ ] **Step 3: Real-window verification** (project norm — the vite preview lacks daemon-backed data). Using the `tauri-debug` skill (single running app) or the `qa` fleet skill (per-worktree), drive the live Ducktape window and verify, taking a screenshot at each:
  - Create a page via `+ New page` → it opens instantly, cursor in the title, no title form.
  - Type blocks → the `Type '/' for commands` spam is gone; the focused empty block shows `Write, or press '/' for commands`; no hash chips under the title or per block; no "N blocks" counter.
  - Create a child page from a row `+` → it nests in the sidebar tree; collapse/expand works; "Move to…" re-nests; delete (with confirm) promotes children.
  - Open several pages → tabs appear; switching and closing works; tabs persist across an app restart.
  - Add a comment on a block and on the page → bubble + count badge appear; the panel lists threads; reply/resolve/reopen/edit/delete-own all work; author names resolve.

- [ ] **Step 4: Commit any fixes** surfaced by verification, then the branch is ready for the PR into `dev`.

```bash
git add -A
git commit --no-gpg-sign -m "fix(docs): real-window verification fixes"
```

---

## Coverage check (spec → task map)

- Clutter removal (hashes/counts/labels) → Task 18.
- Placeholder only on focus → Task 18.
- Copy-link on hover (scope add) → Task 18.
- Instant new-page flow → Tasks 16 (`createChildPage`), 19 (button/form removal).
- Tab system → Tasks 15 (state/persist/open/close), 20 (strip).
- Nested folder tree → Tasks 1–3 (backend parent + set-parent), 13 (client), 16 (actions), 21 (tree UI).
- DeletePage (scope add) → Tasks 4 (backend), 13/16 (client/action), 21 (tree delete).
- Comment system (block + page) → Tasks 5–11 (module), 12 (registration), 14 (client), 17 (store), 22 (UI).
- No-backwards-compat / caps / author-from-origin / reserved index → enforced in Tasks 2–10.

## Notes for the executor

- Work in a worktree forked from `origin/dev`; land as one PR into `dev`.
- Rust modules are the correctness-critical part ("do not make mistake" was about comments) — do not skip the state-sync round-trip tests (Tasks 11).
- Frontend styling is iterated in the real window (Task 23); the component code here is functional scaffolding, not final pixels — match the surrounding `PagesView`/chat-panel visual language.
- If any `Icon` glyph name used here (`link`) is absent, pick an existing one from `app/src/console/components/Icon.tsx`.

