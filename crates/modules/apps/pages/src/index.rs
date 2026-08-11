//! pages' read model: the page list, per-target comment-thread panels, and
//! full-text search over the block tree — folded from the applied-op feed.
//!
//! canonical pages state is the authenticated block tree serving DISPATCH
//! point reads (whole pages, blocks, single threads/comments, the
//! thread-count cap probe); everything a human enumerates — the sidebar's
//! page list, a page render's comment panels, search — is served here.
//!
//! block ids are globally unique (the module's addressability contract), so
//! rows key on the id alone. the fold mirrors just enough of the tree to stay
//! correct under `RemoveBlock` — which removes a whole SUBTREE — by keeping
//! each row's child-id set (membership only; sibling ORDER is not mirrored,
//! no view needs it).
//!
//! key spaces (inside pages' per-module index database):
//! - `blk/{block_id}`         — the block's current [`PageBlockRow`].
//! - `page/{page_id}`         — one [`PageRow`] per page root (title +
//!   folder parent); enumeration IS the keyspace.
//! - `cthread/{thread_id}`    — one [`ThreadRow`]: the thread plus its
//!   ordered comments, tombstones included.
//! - `ctgt/{hex(target)}/{thread_id}` — one marker per thread anchored to a
//!   target; a page render's panel is one scan per target. the target
//!   component is hex-encoded because ids may legitimately contain `/`
//!   (path-form ids), which would otherwise bleed prefix scans.
//! - `cmt/{comment_id}`       — comment id → owning thread id.
//! - `tok/{token}/{block_id}` — one posting per (token, block), value =
//!   [`TokRef`]; rewritten whole on every text change.
//!
//! this file is the DECISION core — pure functions over [`StateRead`],
//! compiled natively and unit-tested against a plain map. the wasm shell
//! (`src/index_guest.rs`, feature `index-guest`) wires it into the engine.
//! within one op a read never sees that op's own writes (they apply after
//! the decision); across ops in one feed batch it sees everything earlier —
//! identical in the engine transaction and the native test harness.

use index_guest::search::{self, DEFAULT_POSTING_CAP};
use index_guest::{Fail, OpRow, OriginKind, OriginTag, StateRead, Writes};
use serde::{Deserialize, Serialize};

use crate::{BlockKind, MAX_QUERY_TARGETS, PageMsg, RelativeAnchor, decode_msg};

const DEFAULT_SEARCH_LIMIT: usize = 20;
const MAX_SEARCH_LIMIT: usize = 100;
/// default and max page size for the page-list view.
const DEFAULT_PAGE_LIMIT: usize = 50;
const MAX_PAGE_LIMIT: usize = 256;

/// [`Fail`] code: an applied op's payload did not decode.
const FAIL_OP_DECODE: i32 = 2;
/// [`Fail`] code: a stored row did not decode — a damaged read model.
const FAIL_ROW_DECODE: i32 = 3;
/// [`Fail`] code: a view request this mapper does not speak.
const FAIL_BAD_REQUEST: i32 = 4;

/// the stored row of one page block, as search results return it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageBlockRow {
    pub block_id: String,
    /// the page (root block id) this block belongs to; a root names itself.
    pub page_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub kind: BlockKind,
    pub text: String,
    /// child block ids, membership only (order lives in canonical state).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<String>,
    pub height: u64,
    pub time: u64,
}

/// a token posting's value: rank (time) plus the row address and its page.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct TokRef {
    block_id: String,
    page_id: String,
    time: u64,
}

/// one page root: what the sidebar's page list renders.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageRow {
    pub id: String,
    pub title: String,
    /// the containing page id (folder parent), or `None` at top level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

/// one comment thread with its ordered comments, tombstones included (the
/// view filters live ones, mirroring the canonical `ThreadView`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThreadRow {
    pub id: String,
    pub target: String,
    /// rendered opener: `user:{id}`, `agent:{module}/{agent}`, `module:{id}`,
    /// or `system`.
    pub opener: String,
    pub created_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<RelativeAnchor>,
    pub resolved: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_by: Option<String>,
    pub comments: Vec<CommentRow>,
}

/// one comment of a thread; `deleted` tombstones content but keeps order.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommentRow {
    pub id: String,
    pub author: String,
    pub text: String,
    pub created_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited_at: Option<u64>,
    pub deleted: bool,
}

/// the threads anchored to one target, as `threads_for_targets` groups them.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TargetThreadsRow {
    pub target: String,
    pub threads: Vec<ThreadRow>,
}

/// pages' view requests, externally tagged:
/// `{"search": {"text": "...", "page_id": "...", "limit": 20}}`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PagesViewQuery {
    /// the page list, ascending by id, cursor-paged.
    ListPages {
        #[serde(default)]
        after: Option<String>,
        #[serde(default)]
        limit: Option<usize>,
    },
    /// every thread anchored to any of `targets`, grouped by target — the
    /// one call a page render makes with all visible block ids + the page
    /// id. `targets` beyond [`MAX_QUERY_TARGETS`] are rejected.
    ThreadsForTargets { targets: Vec<String> },
    Search {
        text: String,
        #[serde(default)]
        page_id: Option<String>,
        #[serde(default)]
        limit: Option<usize>,
    },
}

/// pages' view replies, externally tagged like the requests.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PagesViewReply {
    /// one cursor page of the page list, ascending by id.
    Pages {
        pages: Vec<PageRow>,
        has_more: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        next_after: Option<String>,
    },
    /// live threads grouped per requested target, request order.
    Threads(Vec<TargetThreadsRow>),
    /// search hits, newest first.
    Hits(Vec<PageBlockRow>),
}

fn blk_key(id: &str) -> String {
    format!("blk/{id}")
}

fn page_key(id: &str) -> String {
    format!("page/{id}")
}

fn cthread_key(thread_id: &str) -> String {
    format!("cthread/{thread_id}")
}

/// the per-target thread marker. the target component is hex-encoded so a
/// `/` inside an id can never bleed one target's scan into another's.
fn ctgt_key(target: &str, thread_id: &str) -> String {
    format!("ctgt/{}/{thread_id}", hex_lower(target.as_bytes()))
}

fn ctgt_prefix(target: &str) -> String {
    format!("ctgt/{}/", hex_lower(target.as_bytes()))
}

fn cmt_key(comment_id: &str) -> String {
    format!("cmt/{comment_id}")
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// rendered author, mirroring how the module derives authorship: the origin
/// decides, `as_agent` refines a module origin into an agent author.
fn render_author(origin: &OriginTag, as_agent: Option<&str>) -> String {
    let id = origin.id.as_deref().unwrap_or_default();
    match (origin.kind, as_agent) {
        (OriginKind::Module, Some(agent)) => format!("agent:{id}/{agent}"),
        (OriginKind::Module, None) => format!("module:{id}"),
        (OriginKind::External, _) => format!("user:{id}"),
        (OriginKind::System, _) => "system".to_string(),
    }
}

fn tok_key(token: &str, id: &str) -> String {
    format!("tok/{token}/{id}")
}

fn read_row(read: &impl StateRead, id: &str) -> Result<Option<PageBlockRow>, Fail> {
    let Some(bytes) = read.get(blk_key(id).as_bytes()) else {
        return Ok(None);
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|e| Fail::new(FAIL_ROW_DECODE, e.to_string()))
}

fn encode_row(row: &PageBlockRow) -> Result<Vec<u8>, Fail> {
    serde_json::to_vec(row).map_err(|e| Fail::new(FAIL_ROW_DECODE, e.to_string()))
}

fn put_row(out: &mut Writes, row: &PageBlockRow) -> Result<(), Fail> {
    index_guest::put(out, blk_key(&row.block_id), encode_row(row)?);
    Ok(())
}

/// stage a row plus one posting per token, so every write path produces
/// byte-identical entries.
fn put_row_and_toks(out: &mut Writes, row: &PageBlockRow) -> Result<(), Fail> {
    put_row(out, row)?;
    let tok_ref = serde_json::to_vec(&TokRef {
        block_id: row.block_id.clone(),
        page_id: row.page_id.clone(),
        time: row.time,
    })
    .map_err(|e| Fail::new(FAIL_ROW_DECODE, e.to_string()))?;
    for token in search::tokens(&row.text) {
        index_guest::put(out, tok_key(&token, &row.block_id), tok_ref.clone());
    }
    Ok(())
}

fn delete_toks(out: &mut Writes, row: &PageBlockRow) {
    for token in search::tokens(&row.text) {
        index_guest::delete(out, tok_key(&token, &row.block_id));
    }
}

/// drop a whole subtree depth-first — rows, postings, AND every comment
/// thread anchored to a removed block (the module's cascade purge, mirrored).
/// a child this index never saw is skipped (the mirror only holds what was
/// folded).
fn delete_subtree(read: &impl StateRead, out: &mut Writes, root: PageBlockRow) -> Result<(), Fail> {
    let mut stack = vec![root];
    while let Some(row) = stack.pop() {
        delete_toks(out, &row);
        index_guest::delete(out, blk_key(&row.block_id));
        if row.kind == BlockKind::Page {
            index_guest::delete(out, page_key(&row.block_id));
        }
        purge_target_threads(read, out, &row.block_id)?;
        for child in &row.children {
            if let Some(child_row) = read_row(read, child)? {
                stack.push(child_row);
            }
        }
    }
    Ok(())
}

/// delete every thread anchored to `target`: rows, markers, and comment
/// pointers. one scan pass per removed block, mirroring the module's
/// `purge_comments_for_target`.
fn purge_target_threads(
    read: &impl StateRead,
    out: &mut Writes,
    target: &str,
) -> Result<(), Fail> {
    let prefix = ctgt_prefix(target);
    let mut after: Option<Vec<u8>> = None;
    loop {
        let page = read.scan_page(prefix.as_bytes(), after.as_deref(), MAX_PAGE_LIMIT);
        for (key, _) in &page.entries {
            let marker = String::from_utf8_lossy(key);
            let Some(thread_id) = marker.strip_prefix(&prefix) else {
                continue;
            };
            if let Some(thread) = read_thread(read, thread_id)? {
                for comment in &thread.comments {
                    index_guest::delete(out, cmt_key(&comment.id));
                }
            }
            index_guest::delete(out, cthread_key(thread_id));
            index_guest::delete(out, marker.to_string());
        }
        if !page.has_more {
            return Ok(());
        }
        after = page.next_after.map(String::into_bytes);
    }
}

fn read_page(read: &impl StateRead, id: &str) -> Result<Option<PageRow>, Fail> {
    let Some(bytes) = read.get(page_key(id).as_bytes()) else {
        return Ok(None);
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|e| Fail::new(FAIL_ROW_DECODE, e.to_string()))
}

fn put_page(out: &mut Writes, row: &PageRow) -> Result<(), Fail> {
    let bytes = serde_json::to_vec(row).map_err(|e| Fail::new(FAIL_ROW_DECODE, e.to_string()))?;
    index_guest::put(out, page_key(&row.id), bytes);
    Ok(())
}

fn read_thread(read: &impl StateRead, thread_id: &str) -> Result<Option<ThreadRow>, Fail> {
    let Some(bytes) = read.get(cthread_key(thread_id).as_bytes()) else {
        return Ok(None);
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|e| Fail::new(FAIL_ROW_DECODE, e.to_string()))
}

fn put_thread(out: &mut Writes, row: &ThreadRow) -> Result<(), Fail> {
    let bytes = serde_json::to_vec(row).map_err(|e| Fail::new(FAIL_ROW_DECODE, e.to_string()))?;
    index_guest::put(out, cthread_key(&row.id), bytes);
    Ok(())
}

/// resolve a comment id to its owning thread row via the `cmt/` pointer. an
/// absent pointer means the comment predates this index — a deterministic
/// skip, like every pre-index record.
fn thread_of_comment(
    read: &impl StateRead,
    comment_id: &str,
) -> Result<Option<ThreadRow>, Fail> {
    let Some(pointer) = read.get(cmt_key(comment_id).as_bytes()) else {
        return Ok(None);
    };
    let thread_id = String::from_utf8(pointer)
        .map_err(|_| Fail::new(FAIL_ROW_DECODE, "comment pointer is not utf-8"))?;
    read_thread(read, &thread_id)
}

/// drop a whole thread — row, target marker, and comment pointers.
fn delete_thread(out: &mut Writes, thread: &ThreadRow) {
    for comment in &thread.comments {
        index_guest::delete(out, cmt_key(&comment.id));
    }
    index_guest::delete(out, ctgt_key(&thread.target, &thread.id));
    index_guest::delete(out, cthread_key(&thread.id));
}

/// fold one applied op into derived writes.
pub fn fold_op(op: &OpRow, read: &impl StateRead) -> Result<Writes, Fail> {
    let msg = decode_msg(&op.payload).map_err(|e| Fail::new(FAIL_OP_DECODE, e))?;
    let mut out = Writes::new();
    match msg {
        PageMsg::CreatePage { page_id, title } => {
            // idempotence mirror: re-creating an existing page is a no-op
            // that changes neither the title nor the parent.
            if read_row(read, &page_id)?.is_some() {
                return Ok(out);
            }
            put_page(
                &mut out,
                &PageRow {
                    id: page_id.clone(),
                    title: title.clone(),
                    parent: None,
                },
            )?;
            let row = PageBlockRow {
                page_id: page_id.clone(),
                block_id: page_id,
                parent: None,
                kind: BlockKind::Page,
                text: title,
                children: Vec::new(),
                height: op.height,
                time: op.time,
            };
            put_row_and_toks(&mut out, &row)?;
        }
        PageMsg::InsertBlock { parent, block, .. } => {
            // the page is derived from the parent — a parent this index
            // never saw (pre-index tree) leaves the whole insert out.
            let Some(mut parent_row) = read_row(read, &parent)? else {
                return Ok(out);
            };
            let page_id = if block.kind == BlockKind::Page {
                put_page(
                    &mut out,
                    &PageRow {
                        id: block.id.clone(),
                        title: block.text.clone(),
                        parent: Some(parent_row.page_id.clone()),
                    },
                )?;
                block.id.clone()
            } else {
                parent_row.page_id.clone()
            };
            let row = PageBlockRow {
                block_id: block.id.clone(),
                page_id,
                parent: Some(parent.clone()),
                kind: block.kind,
                text: block.text,
                children: Vec::new(),
                height: op.height,
                time: op.time,
            };
            put_row_and_toks(&mut out, &row)?;
            parent_row.children.push(block.id);
            put_row(&mut out, &parent_row)?;
        }
        PageMsg::UpdateText { block_id, text, .. } => {
            let Some(mut row) = read_row(read, &block_id)? else {
                return Ok(out);
            };
            // a root rename shows in the page list too.
            if row.kind == BlockKind::Page
                && let Some(mut page) = read_page(read, &block_id)?
            {
                page.title = text.clone();
                put_page(&mut out, &page)?;
            }
            // delete BEFORE re-putting: tokens shared by the old and new
            // text stage a delete then a put, and the last command wins.
            delete_toks(&mut out, &row);
            row.text = text;
            row.height = op.height;
            row.time = op.time;
            put_row_and_toks(&mut out, &row)?;
        }
        PageMsg::SetKind { block_id, kind } => {
            let Some(mut row) = read_row(read, &block_id)? else {
                return Ok(out);
            };
            row.kind = kind;
            put_row(&mut out, &row)?;
        }
        PageMsg::MoveBlock {
            block_id, parent, ..
        } => {
            // Re-home the membership edge. Page rows keep their own page id
            // while non-page rows stay within their page.
            let Some(mut row) = read_row(read, &block_id)? else {
                return Ok(out);
            };
            // a same-parent move is a sibling reorder: membership is
            // unchanged and this index does not mirror order (the module
            // special-cases it too). re-reading the parent below would see
            // the pre-op row (this op's writes apply after the decision)
            // and re-push a duplicate child — so: no-op.
            if row.parent == parent {
                return Ok(out);
            }
            if let Some(old_parent) = &row.parent
                && let Some(mut old) = read_row(read, old_parent)?
            {
                old.children.retain(|c| c != &block_id);
                put_row(&mut out, &old)?;
            }
            if let Some(parent) = &parent
                && let Some(mut new_parent) = read_row(read, parent)?
            {
                new_parent.children.push(block_id.clone());
                put_row(&mut out, &new_parent)?;
            }
            row.parent = parent;
            put_row(&mut out, &row)?;
        }
        PageMsg::RemoveBlock { block_id } => {
            let Some(row) = read_row(read, &block_id)? else {
                return Ok(out);
            };
            // unhook from the parent's membership set…
            if let Some(parent) = &row.parent
                && let Some(mut parent_row) = read_row(read, parent)?
            {
                parent_row.children.retain(|c| c != &block_id);
                put_row(&mut out, &parent_row)?;
            }
            // …then drop the whole subtree, rows and postings both.
            delete_subtree(read, &mut out, row)?;
        }
        // checked state carries no searchable text.
        PageMsg::SetChecked { .. } | PageMsg::SetSpanMark { .. } => {}
        PageMsg::AddComment {
            thread_id,
            comment_id,
            target,
            text,
            anchor,
            as_agent,
            ..
        } => {
            let author = render_author(&op.origin, as_agent.as_deref());
            let comment = CommentRow {
                id: comment_id.clone(),
                author: author.clone(),
                text,
                created_at: op.time,
                edited_at: None,
                deleted: false,
            };
            let thread = match read_thread(read, &thread_id)? {
                Some(mut thread) => {
                    thread.comments.push(comment);
                    thread
                }
                None => {
                    // a fresh thread: the opener is this comment's author and
                    // the target marker makes it scannable per target.
                    index_guest::put(
                        &mut out,
                        ctgt_key(&target, &thread_id),
                        Vec::new(),
                    );
                    ThreadRow {
                        id: thread_id.clone(),
                        target,
                        opener: author,
                        created_at: op.time,
                        anchor,
                        resolved: false,
                        resolved_by: None,
                        comments: vec![comment],
                    }
                }
            };
            index_guest::put(&mut out, cmt_key(&comment_id), thread_id.into_bytes());
            put_thread(&mut out, &thread)?;
        }
        PageMsg::MoveCommentThread {
            thread_id,
            target,
            anchor,
        } => {
            let Some(mut thread) = read_thread(read, &thread_id)? else {
                return Ok(out);
            };
            index_guest::delete(&mut out, ctgt_key(&thread.target, &thread.id));
            index_guest::put(&mut out, ctgt_key(&target, &thread.id), Vec::new());
            thread.target = target;
            thread.anchor = anchor;
            put_thread(&mut out, &thread)?;
        }
        PageMsg::EditComment {
            comment_id, text, ..
        } => {
            let Some(mut thread) = thread_of_comment(read, &comment_id)? else {
                return Ok(out);
            };
            let Some(comment) = thread.comments.iter_mut().find(|c| c.id == comment_id)
            else {
                return Ok(out);
            };
            comment.text = text;
            comment.edited_at = Some(op.time);
            put_thread(&mut out, &thread)?;
        }
        PageMsg::DeleteComment { comment_id } => {
            let Some(mut thread) = thread_of_comment(read, &comment_id)? else {
                return Ok(out);
            };
            let Some(comment) = thread.comments.iter_mut().find(|c| c.id == comment_id)
            else {
                return Ok(out);
            };
            comment.deleted = true;
            comment.text = String::new();
            // the module removes the whole thread record when its last live
            // comment tombstones — mirror that.
            let all_deleted = thread.comments.iter().all(|c| c.deleted);
            if all_deleted {
                delete_thread(&mut out, &thread);
            } else {
                put_thread(&mut out, &thread)?;
            }
        }
        PageMsg::ResolveThread {
            thread_id,
            resolved,
        } => {
            let Some(mut thread) = read_thread(read, &thread_id)? else {
                return Ok(out);
            };
            thread.resolved = resolved;
            thread.resolved_by = resolved.then(|| render_author(&op.origin, None));
            put_thread(&mut out, &thread)?;
        }
    }
    Ok(out)
}

fn reply_json(reply: &PagesViewReply) -> Result<Vec<u8>, Fail> {
    serde_json::to_vec(reply).map_err(|e| Fail::new(FAIL_BAD_REQUEST, e.to_string()))
}

/// serve one materialized-view request.
pub fn serve_view(read: &impl StateRead, req: &[u8]) -> Result<Vec<u8>, Fail> {
    let query: PagesViewQuery =
        serde_json::from_slice(req).map_err(|e| Fail::new(FAIL_BAD_REQUEST, e.to_string()))?;
    match query {
        PagesViewQuery::ListPages { after, limit } => {
            let limit = limit.unwrap_or(DEFAULT_PAGE_LIMIT).clamp(1, MAX_PAGE_LIMIT);
            let page = read.scan_page(b"page/", after.as_deref().map(str::as_bytes), limit);
            let mut pages = Vec::with_capacity(page.entries.len());
            for (_key, value) in &page.entries {
                pages.push(
                    serde_json::from_slice(value)
                        .map_err(|e| Fail::new(FAIL_ROW_DECODE, e.to_string()))?,
                );
            }
            reply_json(&PagesViewReply::Pages {
                pages,
                has_more: page.has_more,
                next_after: page.next_after,
            })
        }
        PagesViewQuery::ThreadsForTargets { targets } => {
            if targets.len() > MAX_QUERY_TARGETS {
                return Err(Fail::new(FAIL_BAD_REQUEST, "too many targets"));
            }
            let mut groups = Vec::with_capacity(targets.len());
            for target in targets {
                let prefix = ctgt_prefix(&target);
                let mut threads = Vec::new();
                let mut after: Option<Vec<u8>> = None;
                loop {
                    let page = read.scan_page(prefix.as_bytes(), after.as_deref(), MAX_PAGE_LIMIT);
                    for (key, _) in &page.entries {
                        let marker = String::from_utf8_lossy(key);
                        let Some(thread_id) = marker.strip_prefix(&prefix) else {
                            continue;
                        };
                        if let Some(thread) = read_thread(read, thread_id)? {
                            threads.push(thread);
                        }
                    }
                    if !page.has_more {
                        break;
                    }
                    after = page.next_after.map(String::into_bytes);
                }
                groups.push(TargetThreadsRow { target, threads });
            }
            reply_json(&PagesViewReply::Threads(groups))
        }
        PagesViewQuery::Search {
            text,
            page_id,
            limit,
        } => {
            let tokens: Vec<String> = search::tokens(&text).into_iter().collect();
            if tokens.is_empty() {
                return Err(Fail::new(FAIL_BAD_REQUEST, "search text has no tokens"));
            }
            // each token matches as a prefix (search-as-you-type). block ids
            // are global, so postings carry no page segment — the page filter
            // applies to the intersected refs instead.
            let mut refs: Vec<TokRef> =
                search::intersect_prefix(read, "tok/", &tokens, DEFAULT_POSTING_CAP)
                    .into_iter()
                    .filter_map(|hit| serde_json::from_slice(&hit.value).ok())
                    .filter(|r: &TokRef| page_id.as_ref().is_none_or(|p| &r.page_id == p))
                    .collect();
            refs.sort_by(|a, b| (b.time, &b.block_id).cmp(&(a.time, &a.block_id)));
            let limit = limit
                .unwrap_or(DEFAULT_SEARCH_LIMIT)
                .clamp(1, MAX_SEARCH_LIMIT);
            let mut hits = Vec::new();
            for r in refs.into_iter().take(limit) {
                if let Some(bytes) = read.get(blk_key(&r.block_id).as_bytes()) {
                    hits.push(
                        serde_json::from_slice(&bytes)
                            .map_err(|e| Fail::new(FAIL_ROW_DECODE, e.to_string()))?,
                    );
                }
            }
            reply_json(&PagesViewReply::Hits(hits))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NewBlock, encode_msg};
    use index_guest::{OriginTag, apply_to_map};
    use std::collections::BTreeMap;

    type Map = BTreeMap<Vec<u8>, Vec<u8>>;

    fn op(height: u64, seq: u32, msg: &PageMsg) -> OpRow {
        OpRow {
            height,
            seq,
            time: 1_000 + height,
            origin: OriginTag::external("jess"),
            payload: encode_msg(msg),
            assigned: Vec::new(),
        }
    }

    fn insert(parent: &str, id: &str, text: &str) -> PageMsg {
        PageMsg::InsertBlock {
            parent: parent.into(),
            after: None,
            block: NewBlock {
                id: id.into(),
                kind: BlockKind::Paragraph,
                text: text.into(),
                marks: Vec::new(),
            },
        }
    }

    fn create(id: &str, title: &str, parent: Option<&str>) -> PageMsg {
        match parent {
            None => PageMsg::CreatePage {
                page_id: id.into(),
                title: title.into(),
            },
            // a foldered page is a Page-kind block inserted under its parent.
            Some(parent) => PageMsg::InsertBlock {
                parent: parent.into(),
                after: None,
                block: NewBlock {
                    id: id.into(),
                    kind: BlockKind::Page,
                    text: title.into(),
                    marks: Vec::new(),
                },
            },
        }
    }

    fn add(thread: &str, comment: &str, target: &str, text: &str) -> PageMsg {
        PageMsg::AddComment {
            thread_id: thread.into(),
            comment_id: comment.into(),
            target: target.into(),
            text: text.into(),
            anchor: None,
            mentions: Vec::new(),
            as_agent: None,
        }
    }

    fn apply(map: &mut Map, height: u64, msgs: &[PageMsg]) {
        for (seq, msg) in msgs.iter().enumerate() {
            let writes = fold_op(&op(height, seq as u32, msg), map).expect("fold");
            apply_to_map(map, writes);
        }
    }

    fn view(map: &Map, req: serde_json::Value) -> PagesViewReply {
        let bytes = serve_view(map, &serde_json::to_vec(&req).unwrap()).expect("view");
        serde_json::from_slice(&bytes).expect("reply decodes")
    }

    fn search(map: &Map, req: serde_json::Value) -> Vec<PageBlockRow> {
        match view(map, req) {
            PagesViewReply::Hits(hits) => hits,
            other => panic!("expected hits, got {other:?}"),
        }
    }

    fn list(map: &Map) -> Vec<PageRow> {
        match view(map, serde_json::json!({"list_pages": {}})) {
            PagesViewReply::Pages { pages, .. } => pages,
            other => panic!("expected pages, got {other:?}"),
        }
    }

    fn threads(map: &Map, targets: &[&str]) -> Vec<TargetThreadsRow> {
        match view(map, serde_json::json!({"threads_for_targets": {"targets": targets}})) {
            PagesViewReply::Threads(groups) => groups,
            other => panic!("expected threads, got {other:?}"),
        }
    }

    #[test]
    fn page_titles_and_nested_blocks_are_searchable() {
        let mut map = Map::new();
        apply(
            &mut map,
            1,
            &[
                PageMsg::CreatePage {
                    page_id: "p1".into(),
                    title: "roadmap draft".into(),
                },
                insert("p1", "b1", "quarter goals"),
                insert("b1", "b2", "nested milestone detail"),
            ],
        );

        // the title, a child, and a grandchild all resolve to page p1.
        for term in ["roadmap", "goals", "milestone"] {
            let hits = search(&map, serde_json::json!({"search": {"text": term}}));
            assert_eq!(hits.len(), 1, "{term}");
            assert_eq!(hits[0].page_id, "p1", "{term}");
        }

        // recreate is a no-op: the title survives.
        apply(
            &mut map,
            2,
            &[PageMsg::CreatePage {
                page_id: "p1".into(),
                title: "usurper".into(),
            }],
        );
        assert!(search(&map, serde_json::json!({"search": {"text": "usurper"}})).is_empty());
        assert_eq!(
            search(&map, serde_json::json!({"search": {"text": "roadmap"}})).len(),
            1
        );
    }

    #[test]
    fn page_blocks_start_and_remove_their_own_search_scope() {
        let mut map = Map::new();
        apply(
            &mut map,
            1,
            &[
                PageMsg::CreatePage {
                    page_id: "root".into(),
                    title: "root document".into(),
                },
                PageMsg::InsertBlock {
                    parent: "root".into(),
                    after: None,
                    block: NewBlock {
                        id: "child".into(),
                        kind: BlockKind::Page,
                        text: "child document".into(),
                        marks: Vec::new(),
                    },
                },
                insert("child", "inside", "nested body"),
            ],
        );

        for term in ["child", "nested"] {
            let hits = search(&map, serde_json::json!({"search": {"text": term}}));
            assert_eq!(hits.len(), 1, "{term}");
            assert_eq!(hits[0].page_id, "child", "{term}");
        }

        apply(
            &mut map,
            2,
            &[PageMsg::RemoveBlock {
                block_id: "child".into(),
            }],
        );
        assert!(search(&map, serde_json::json!({"search": {"text": "child"}})).is_empty());
        assert!(search(&map, serde_json::json!({"search": {"text": "nested"}})).is_empty());
    }

    #[test]
    fn remove_block_unindexes_the_whole_subtree() {
        let mut map = Map::new();
        apply(
            &mut map,
            1,
            &[
                PageMsg::CreatePage {
                    page_id: "p1".into(),
                    title: "home".into(),
                },
                insert("p1", "b1", "toggle section"),
                insert("b1", "b2", "hidden inner text"),
                insert("p1", "b3", "sibling survivor"),
            ],
        );
        apply(
            &mut map,
            2,
            &[PageMsg::RemoveBlock {
                block_id: "b1".into(),
            }],
        );

        assert!(search(&map, serde_json::json!({"search": {"text": "toggle"}})).is_empty());
        assert!(search(&map, serde_json::json!({"search": {"text": "hidden"}})).is_empty());
        assert_eq!(
            search(&map, serde_json::json!({"search": {"text": "survivor"}})).len(),
            1
        );
    }

    #[test]
    fn same_parent_move_does_not_duplicate_membership() {
        let mut map = Map::new();
        apply(
            &mut map,
            1,
            &[
                PageMsg::CreatePage {
                    page_id: "p1".into(),
                    title: "home".into(),
                },
                insert("p1", "b1", "first"),
                insert("p1", "b2", "second"),
            ],
        );
        // two sibling reorders under the same parent — each used to re-read
        // the stale parent row and re-push the child, duplicating membership.
        apply(
            &mut map,
            2,
            &[PageMsg::MoveBlock {
                block_id: "b1".into(),
                parent: Some("p1".into()),
                after: Some("b2".into()),
            }],
        );
        apply(
            &mut map,
            3,
            &[PageMsg::MoveBlock {
                block_id: "b1".into(),
                parent: Some("p1".into()),
                after: None,
            }],
        );

        let hits = search(&map, serde_json::json!({"search": {"text": "home"}}));
        assert_eq!(hits.len(), 1);
        assert_eq!(
            hits[0].children.iter().filter(|c| *c == "b1").count(),
            1,
            "sibling reorders must not duplicate membership: {:?}",
            hits[0].children
        );
    }

    #[test]
    fn update_text_renames_and_page_filter_applies() {
        let mut map = Map::new();
        apply(
            &mut map,
            1,
            &[
                PageMsg::CreatePage {
                    page_id: "p1".into(),
                    title: "alpha".into(),
                },
                PageMsg::CreatePage {
                    page_id: "p2".into(),
                    title: "beta".into(),
                },
                insert("p1", "b1", "shared term"),
                insert("p2", "b2", "shared term"),
            ],
        );

        let hits = search(&map, serde_json::json!({"search": {"text": "shared"}}));
        assert_eq!(hits.len(), 2);
        let hits = search(
            &map,
            serde_json::json!({"search": {"text": "shared", "page_id": "p2"}}),
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].block_id, "b2");

        // renaming the page root retokenizes the title.
        apply(
            &mut map,
            2,
            &[PageMsg::UpdateText {
                block_id: "p1".into(),
                text: "gamma".into(),
                marks: None,
            }],
        );
        assert!(search(&map, serde_json::json!({"search": {"text": "alpha"}})).is_empty());
        assert_eq!(
            search(&map, serde_json::json!({"search": {"text": "gamma"}})).len(),
            1
        );
    }

    #[test]
    fn page_list_orders_by_id_with_live_titles_and_folder_edges() {
        let mut map = Map::new();
        // out-of-order creates; `mid` folders under `alpha`.
        apply(
            &mut map,
            1,
            &[
                create("zebra", "Z", None),
                create("alpha", "A", None),
                create("mid", "M", Some("alpha")),
            ],
        );
        let got: Vec<(String, Option<String>)> = list(&map)
            .into_iter()
            .map(|r| (r.id, r.parent))
            .collect();
        assert_eq!(
            got,
            [
                ("alpha".into(), None),
                ("mid".into(), Some("alpha".into())),
                ("zebra".into(), None),
            ]
        );

        // a root rename shows in the list; a recreate stays a no-op.
        apply(
            &mut map,
            2,
            &[
                PageMsg::UpdateText {
                    block_id: "zebra".into(),
                    text: "Zed".into(),
                    marks: None,
                },
                create("alpha", "usurper", None),
            ],
        );
        let titles: Vec<(String, String)> = list(&map)
            .into_iter()
            .map(|r| (r.id, r.title))
            .collect();
        assert_eq!(
            titles,
            [
                ("alpha".into(), "A".into()),
                ("mid".into(), "M".into()),
                ("zebra".into(), "Zed".into()),
            ]
        );

        // cursor paging: the reply's `next_after` resumes the ascending scan.
        let PagesViewReply::Pages {
            pages,
            has_more,
            next_after,
        } = view(&map, serde_json::json!({"list_pages": {"limit": 1}}))
        else {
            panic!("expected pages")
        };
        assert_eq!(pages[0].id, "alpha");
        assert!(has_more);
        let after = next_after.expect("a partial page carries a cursor");
        let PagesViewReply::Pages { pages, .. } = view(
            &map,
            serde_json::json!({"list_pages": {"after": after, "limit": 2}}),
        ) else {
            panic!("expected pages")
        };
        let rest: Vec<String> = pages.into_iter().map(|r| r.id).collect();
        assert_eq!(rest, ["mid", "zebra"]);
    }

    #[test]
    fn removing_a_subpage_block_unindexes_its_page_subtree() {
        let mut map = Map::new();
        // grand -> parent -> child; parent also carries a content block.
        apply(
            &mut map,
            1,
            &[
                create("grand", "G", None),
                create("parent", "P", Some("grand")),
                create("child", "C", Some("parent")),
                insert("parent", "pb1", "doomed body"),
            ],
        );
        apply(
            &mut map,
            2,
            &[PageMsg::RemoveBlock {
                block_id: "parent".into(),
            }],
        );

        // the subtree removal takes parent AND its nested child page rows.
        let got: Vec<(String, Option<String>)> = list(&map)
            .into_iter()
            .map(|r| (r.id, r.parent))
            .collect();
        assert_eq!(got, [("grand".into(), None)]);
        // the deleted page's block subtree left the search index with it.
        assert!(search(&map, serde_json::json!({"search": {"text": "doomed"}})).is_empty());
    }

    #[test]
    fn threads_group_per_target_and_keep_tombstones_verbatim() {
        let mut map = Map::new();
        apply(
            &mut map,
            1,
            &[
                create("p1", "home", None),
                insert("p1", "b1", "first block"),
                insert("p1", "b2", "second block"),
                add("t1", "m1", "b1", "first"),
                add("t1", "m2", "b1", "second"),
                add("t2", "m3", "b1", "other"),
                add("t3", "m4", "b2", "elsewhere"),
            ],
        );

        // one group per REQUESTED target, in request order; absent = empty.
        let groups = threads(&map, &["b2", "b1", "ghost"]);
        let names = |group: &TargetThreadsRow| -> Vec<String> {
            group.threads.iter().map(|t| t.id.clone()).collect()
        };
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].target, "b2");
        assert_eq!(names(&groups[0]), ["t3"]);
        assert_eq!(names(&groups[1]), ["t1", "t2"]);
        assert!(groups[2].threads.is_empty());
        let t1 = &groups[1].threads[0];
        assert_eq!(t1.opener, "user:jess");
        let texts: Vec<&str> = t1.comments.iter().map(|c| c.text.as_str()).collect();
        assert_eq!(texts, ["first", "second"]);

        // a tombstone STAYS in the row and the view returns it VERBATIM
        // (rendering a tombstone is the consumer's call, not a view filter);
        // edits and resolution fold in place.
        apply(
            &mut map,
            2,
            &[
                PageMsg::DeleteComment {
                    comment_id: "m1".into(),
                },
                PageMsg::EditComment {
                    comment_id: "m2".into(),
                    text: "reworded".into(),
                    mentions: Vec::new(),
                },
                PageMsg::ResolveThread {
                    thread_id: "t1".into(),
                    resolved: true,
                },
            ],
        );
        let groups = threads(&map, &["b1"]);
        let t1 = &groups[0].threads[0];
        assert!(t1.resolved);
        assert_eq!(t1.resolved_by.as_deref(), Some("user:jess"));
        assert!(t1.comments[0].deleted, "the tombstone is served, not hidden");
        assert_eq!(t1.comments[0].text, "", "tombstoned content is emptied");
        assert_eq!(t1.comments[1].text, "reworded");
        assert_eq!(t1.comments[1].edited_at, Some(1_002));

        // deleting a thread's LAST live comment removes the whole thread …
        apply(
            &mut map,
            3,
            &[PageMsg::DeleteComment {
                comment_id: "m3".into(),
            }],
        );
        assert_eq!(names(&threads(&map, &["b1"])[0]), ["t1"]);

        // … and a moved thread re-homes to its new target's group.
        apply(
            &mut map,
            4,
            &[PageMsg::MoveCommentThread {
                thread_id: "t3".into(),
                target: "b1".into(),
                anchor: None,
            }],
        );
        let groups = threads(&map, &["b1", "b2"]);
        assert_eq!(names(&groups[0]), ["t1", "t3"]);
        assert!(groups[1].threads.is_empty());
    }

    #[test]
    fn threads_for_targets_rejects_over_cap_target_lists() {
        let map = Map::new();
        let targets: Vec<String> = (0..=MAX_QUERY_TARGETS).map(|i| format!("t{i}")).collect();
        let req =
            serde_json::to_vec(&serde_json::json!({"threads_for_targets": {"targets": targets}}))
                .unwrap();
        assert!(
            serve_view(&map, &req).is_err(),
            "an over-cap grouped read must refuse"
        );
    }
}
