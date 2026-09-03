//! deterministic forge item-context injection (M1): the instructions section
//! a forge-channel run's envelope carries in its `context` field, rendered
//! from COMMITTED tracker state at compose height (I1) and byte-capped.
//!
//! extended with duck:// reference injection: `[label](duck://page/<id>)` and
//! `[label](duck://files/<path>)` refs parsed from the trigger message text and
//! the injected item body resolve against COMMITTED pages/files state at
//! compose height — each referenced page's subtree and each attachment's text
//! render into the same `context` section. The ref grammar is the console's
//! `splitDuckRefs` twin (parity is load-bearing: a chip a human sees but the
//! agent's context skips, or vice-versa, is a lie about what the agent read).
//!
//! the wording is part of the committed prompt input (the envelope JSON is
//! the provider prompt), so the render is a pure function of the item record
//! + coordinates: same committed state, same bytes, on every validator.

use std::collections::BTreeMap;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use chat::MessageView;
use pages::{Block, BlockKind, PageQuery, PageReply};
use sdk::Ctx;

use crate::forge_source::{ForgeItem, ForgeItemKind};
use crate::{
    FilesQuery, FilesReply, ModuleId, RunsModule, SiblingReadBudget, files_decode_reply,
    files_encode_query,
};

/// the item-context byte cap (spec verbatim: 16 KiB, truncate-with-marker —
/// never fail on size). separate from — and earlier than — the whole-payload
/// dispatch cap, which stays the final guard.
pub(crate) const MAX_CONTEXT_BYTES: usize = 16 * 1024;

/// the deterministic truncation marker; the capped render always ends with it.
const TRUNCATION_MARKER: &str = "\n[item context truncated at 16 KiB]";

/// render the deterministic instructions section for a forge item run: item
/// kind/number/state, title, body, repo coordinates, the work branch, and —
/// for a PR — its source/target branches.
pub(crate) fn render_item_context(repo: &str, item: &ForgeItem, work_branch: &str) -> String {
    let mut out = format!(
        "Forge item context — you are working this item as a session.\n\
         Before finishing, inspect recent Git history and create a commit with a \
         repository-appropriate title and body. Follow the repository's conventions; \
         do not force Conventional Commits when it uses another style.\n\
         repo: {repo}\n\
         item: {} #{} ({})\n\
         title: {}\n\
         work branch: {work_branch}\n",
        item.kind.as_str(),
        item.number,
        item.state.as_str(),
        item.title,
    );
    if item.kind == ForgeItemKind::Pr {
        if let Some(src) = &item.source_branch {
            out.push_str(&format!("pr source branch: {src}\n"));
        }
        if let Some(tgt) = &item.target_branch {
            out.push_str(&format!("pr target branch: {tgt}\n"));
        }
    }
    out.push_str("\nItem body:\n");
    out.push_str(&item.body);
    crate::truncate_on_boundary(&out, MAX_CONTEXT_BYTES, TRUNCATION_MARKER)
}

// ---- duck:// reference injection (M2/M3) --------------------------------------
//
// refs use the unified markdown grammar the chat console renders
// (app/src/console/views/chat/duck-ref.ts `splitDuckRefs`) — the two MUST
// accept exactly the same refs, or the agent's injected context disagrees with
// the chip a human sees:
//   [label](duck://page/<id>)     -> the page's committed subtree
//   [label](duck://files/<path>)  -> the attachment's committed text
// file refs are confined to /shared/attachments/<dir>/<name> — the console's
// own confinement, and the only guard against a crafted ref pulling another
// duckfs path into the agent's context (reads are not authority-gated).

/// the page-section byte budget across ALL injected pages — separate from
/// (and additional to) the 16 KiB item-context cap above.
pub(crate) const PAGE_CONTEXT_BYTES: usize = 64 * 1024;

/// the deterministic page-section truncation marker.
const PAGE_TRUNCATION_MARKER: &str = "\n[page context truncated at 64 KiB]";
const PAGE_READ_TRUNCATION_MARKER: &str = "[page context truncated at bounded read limit]";
const PAGE_READ_TRUNCATION_BLOCK_ID: &str = "\0runs-page-read-truncated";

/// the confinement root for file refs (matches the console tokenizer).
const ATTACHMENTS_ROOT: &str = "/shared/attachments/";

/// the trigger message's plain text — the ref-parse source for a chat anchor.
pub(crate) fn message_text(message: &MessageView) -> String {
    message
        .head
        .blocks
        .iter()
        .map(|block| match block {
            chat::Block::Paragraph(spans) | chat::Block::Quote(spans) => {
                spans.iter().map(|s| s.text.as_str()).collect()
            }
            chat::Block::Code { text, .. } => text.clone(),
            chat::Block::Divider => String::new(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// the duck:// refs a message carries: page ids and confined attachment paths,
/// each in first-seen order (deduped across all sources).
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct DuckRefs {
    pub pages: Vec<String>,
    /// absolute `/shared/attachments/<dir>/<name>` paths — already confined.
    pub files: Vec<String>,
}

/// parse every `[label](duck://page|files/…)` markdown ref from the sources, in
/// order, first-seen deduped — the Rust twin of the console's `splitDuckRefs`.
/// A malformed ref (bad url, out-of-confinement file path) is skipped, NEVER a
/// failure. The `![..]` embed marker and the label are ignored: only the
/// referenced id/path matters for injection.
///
/// parity ceiling: on a hand-typed ADVERSARIALLY-nested body (a ref whose
/// label brackets wrap another ref, e.g. `[a](b[c](duck://page/p))`) this
/// under-reads relative to the console's regex — the refs parsed here are
/// always a SUBSET of what a human's chips show, never a superset. That is the
/// safe direction (the agent can miss a contrived ref; it can never be fed a
/// file the author didn't reference), and the composer only ever emits clean,
/// non-nested refs.
pub(crate) fn parse_duck_refs(sources: &[&str]) -> DuckRefs {
    let mut out = DuckRefs::default();
    for source in sources {
        let mut rest: &str = source;
        while let Some(open) = rest.find('[') {
            let after = &rest[open + 1..];
            // label runs to the first `]`; none anywhere ⇒ no ref can close.
            let Some(close) = after.find(']') else { break };
            let label = &after[..close];
            let tail = &after[close + 1..];
            if label.contains('\n') || !tail.starts_with('(') {
                rest = &rest[open + 1..]; // not a link here — step past this `[`
                continue;
            }
            let url_area = &tail[1..];
            // url ends at the first `)` or whitespace; a `)` must actually close.
            let end = url_area
                .find(|c: char| c == ')' || c.is_whitespace())
                .unwrap_or(url_area.len());
            if !url_area[end..].starts_with(')') {
                rest = &rest[open + 1..];
                continue;
            }
            classify_duck_url(&url_area[..end], &mut out);
            rest = &url_area[end + 1..];
        }
    }
    out
}

/// classify one extracted `duck://…` url into the ref lists — the same rules
/// as the console's `classify`: a page id is a single non-empty segment; a file
/// path is exactly `<dir>/<name>` (non-empty, non-dot) under the attachments
/// root. Anything else is dropped.
fn classify_duck_url(url: &str, out: &mut DuckRefs) {
    // `?net=<digest>` names the network a produced link belongs to. A run
    // resolves refs against the chain it is executing on and no other, so the
    // component is dropped here — but it must be dropped rather than folded
    // into the id, or a pasted produced link resolves to nothing.
    let url = url.split_once('?').map(|(head, _)| head).unwrap_or(url);
    if let Some(id) = url.strip_prefix("duck://page/") {
        if !id.is_empty() && !id.contains('/') && !out.pages.iter().any(|p| p == id) {
            out.pages.push(id.to_string());
        }
        return;
    }
    let Some(path) = url.strip_prefix("duck://files") else {
        return;
    };
    let Some(rest) = path.strip_prefix(ATTACHMENTS_ROOT) else {
        return;
    };
    let segs: Vec<&str> = rest.split('/').collect();
    let confined = segs.len() == 2
        && segs
            .iter()
            .all(|s| !s.is_empty() && *s != "." && *s != "..");
    if confined && !out.files.iter().any(|f| f == path) {
        out.files.push(path.to_string());
    }
}

/// render the page section from resolved refs, in the given (first-seen)
/// order: each page's preorder subtree depth-first to markdown, an
/// unresolvable page as a one-line marker, the whole section capped at
/// [`PAGE_CONTEXT_BYTES`]. pure — same input, same bytes.
pub(crate) fn render_pages_section(
    pages: &[(String, Option<Vec<Block>>)],
    net_query: &str,
) -> String {
    let rendered: Vec<String> = pages
        .iter()
        .map(|(page_id, blocks)| render_page(page_id, blocks.as_deref(), net_query))
        .collect();
    crate::truncate_on_boundary(
        &format!("Referenced pages:\n\n{}", rendered.join("\n\n")),
        PAGE_CONTEXT_BYTES,
        PAGE_TRUNCATION_MARKER,
    )
}

/// one page: a `[[page:<id>]] — <title>` header line, then every block of the
/// preorder subtree as one markdown line (nesting indents by tree depth; the
/// parent of any block precedes it in preorder, so depth resolves in one
/// pass). an unresolvable page is its one-line marker.
fn render_page(page_id: &str, blocks: Option<&[Block]>, net_query: &str) -> String {
    let Some((root, rest)) = blocks.and_then(|b| b.split_first()) else {
        return format!("[page {page_id} — not found]");
    };
    // the header IS the live ref: an agent echoing it produces a working chip
    // (the retired `[[page:]]` syntax would not).
    //
    // it names its network in `?net=` like every other link the product mints
    // — the module reads the chain id out of its genesis `__config` record
    // (`sdk::genesis_config::CHAIN_ID`), the only way a fixed component learns
    // which network it is running on. an unwired chain id (dev tools, tests)
    // renders the bare hand-typed form, which resolves against whichever
    // network the reader is connected to.
    let mut out = format!("[{}](duck://page/{page_id}{net_query})", root.text);
    let mut depth = BTreeMap::from([(root.id.as_str(), 0usize)]);
    for block in rest {
        let d = block
            .parent
            .as_deref()
            .and_then(|p| depth.get(p))
            .copied()
            .unwrap_or(0)
            + 1;
        depth.insert(block.id.as_str(), d);
        out.push('\n');
        out.push_str(&"  ".repeat(d - 1));
        match block.kind {
            BlockKind::Heading1 => out.push_str(&format!("# {}", block.text)),
            BlockKind::Heading2 => out.push_str(&format!("## {}", block.text)),
            BlockKind::Heading3 => out.push_str(&format!("### {}", block.text)),
            BlockKind::Bulleted => out.push_str(&format!("- {}", block.text)),
            BlockKind::Numbered => out.push_str(&format!("1. {}", block.text)),
            // todos carry the block id inline so the model can target them
            // with M2.3 `pages.set_checked` actions.
            BlockKind::Todo => out.push_str(&format!(
                "- [{}] {} [blk:{}]",
                if block.checked { "x" } else { " " },
                block.text,
                block.id,
            )),
            BlockKind::Code => out.push_str(&format!("```\n{}\n```", block.text)),
            BlockKind::Divider => out.push_str("---"),
            // Page (a nested subpage title), Paragraph, Toggle, Quote,
            // Callout: the text verbatim. comments are not in `GetPage` —
            // omitted by construction.
            BlockKind::Page
            | BlockKind::Paragraph
            | BlockKind::Toggle
            | BlockKind::Quote
            | BlockKind::Callout => out.push_str(&block.text),
        }
    }
    out
}

fn page_render_reaches_budget(page_id: &str, blocks: &[Block], net_query: &str) -> bool {
    let section_header_bytes = "Referenced pages:\n\n".len();
    section_header_bytes.saturating_add(render_page(page_id, Some(blocks), net_query).len())
        >= PAGE_CONTEXT_BYTES
}

enum PageBlocksRead {
    Complete(Option<Vec<Block>>),
    Partial(Vec<Block>),
}

fn partial_blocks(blocks: Vec<Block>) -> PageBlocksRead {
    if blocks.is_empty() {
        PageBlocksRead::Complete(None)
    } else {
        PageBlocksRead::Partial(blocks)
    }
}

fn append_page_read_marker(blocks: &mut Vec<Block>) {
    let Some(root) = blocks.first() else {
        return;
    };
    blocks.push(Block {
        id: PAGE_READ_TRUNCATION_BLOCK_ID.into(),
        parent: Some(root.id.clone()),
        page: root.page.clone(),
        kind: BlockKind::Callout,
        text: PAGE_READ_TRUNCATION_MARKER.into(),
        marks: Vec::new(),
        checked: false,
        children: Vec::new(),
    });
}

fn mark_page_section_truncated(section: String) -> String {
    if section.ends_with(PAGE_TRUNCATION_MARKER) {
        return section;
    }
    crate::truncate_on_boundary(
        &format!("{section}\n{PAGE_READ_TRUNCATION_MARKER}"),
        PAGE_CONTEXT_BYTES,
        PAGE_TRUNCATION_MARKER,
    )
}

// ---- duck://files attachment injection ----------------------------------------

/// the attachment-section byte budget across ALL injected attachments — also
/// the per-file read cap, so one attachment can't blow the section.
pub(crate) const ATTACHMENT_CONTEXT_BYTES: usize = 64 * 1024;

/// the deterministic attachment-section truncation marker.
const ATTACHMENT_TRUNCATION_MARKER: &str = "\n[attachment context truncated at 64 KiB]";
const ATTACHMENT_READ_TRUNCATION_MARKER: &str =
    "\n[attachment context truncated at bounded read limit]";

/// one referenced attachment's resolved content.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Attachment {
    /// UTF-8 text content, inlined into the agent's context.
    Text(String),
    /// non-UTF-8 bytes (an image, a binary): named, never inlined — the agent
    /// input plane is text-only.
    Binary,
    /// the ref resolved to nothing at compose height.
    NotFound,
}

/// the attachment's display name — the path's last segment.
fn attachment_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// render the attachment section from resolved refs, in first-seen order, the
/// whole section capped at [`ATTACHMENT_CONTEXT_BYTES`]. pure — same input,
/// same bytes.
pub(crate) fn render_attachments_section(items: &[(String, Attachment)]) -> String {
    let rendered: Vec<String> = items
        .iter()
        .map(|(path, content)| {
            let name = attachment_name(path);
            match content {
                Attachment::Text(text) => format!("[attachment: {name}]\n{text}"),
                Attachment::Binary => {
                    format!("[attachment: {name} — binary content, not shown]")
                }
                Attachment::NotFound => format!("[attachment: {name} — not found]"),
            }
        })
        .collect();
    crate::truncate_on_boundary(
        &format!("Referenced attachments:\n\n{}", rendered.join("\n\n")),
        ATTACHMENT_CONTEXT_BYTES,
        ATTACHMENT_TRUNCATION_MARKER,
    )
}

fn mark_attachment_section_truncated(section: String) -> String {
    if section.ends_with(ATTACHMENT_TRUNCATION_MARKER) {
        return section;
    }
    crate::truncate_on_boundary(
        &format!("{section}{ATTACHMENT_READ_TRUNCATION_MARKER}"),
        ATTACHMENT_CONTEXT_BYTES,
        ATTACHMENT_READ_TRUNCATION_MARKER,
    )
}

impl RunsModule {
    /// the `duck://page/<id>` page section for a run (M2): parse refs from the
    /// given sources (the trigger message text, then the injected item body),
    /// resolve each against COMMITTED pages state at compose height — the
    /// same cross-module query lane as the forge tracker reads (I1) — and
    /// render. `None` when no pages module is wired or no ref parses; an
    /// unresolvable ref renders its marker, NEVER a failure.
    pub(crate) async fn page_context(
        &self,
        ctx: &dyn Ctx,
        sources: &[&str],
        remaining_queries: &mut usize,
        budget: &SiblingReadBudget,
    ) -> Option<String> {
        let pages = self.pages.clone()?;
        let refs = parse_duck_refs(sources).pages;
        if refs.is_empty() {
            return None;
        }
        let net_query = self.net_query();
        let mut resolved = Vec::new();
        for page_id in refs {
            if *remaining_queries == 0 {
                let section = render_pages_section(&resolved, &net_query);
                return Some(mark_page_section_truncated(section));
            }
            match self
                .page_blocks_with_budget(ctx, &pages, &page_id, remaining_queries, budget)
                .await
            {
                PageBlocksRead::Complete(blocks) => resolved.push((page_id, blocks)),
                PageBlocksRead::Partial(blocks) => {
                    resolved.push((page_id, Some(blocks)));
                    let section = render_pages_section(&resolved, &net_query);
                    return Some(mark_page_section_truncated(section));
                }
            }
            let context_is_full =
                render_pages_section(&resolved, &net_query).len() >= PAGE_CONTEXT_BYTES;
            if context_is_full {
                break;
            }
        }
        Some(render_pages_section(&resolved, &net_query))
    }

    /// One committed page in preorder, assembled from bounded Pages replies.
    /// `None` means the first read found no page (or failed); a later failure
    /// keeps the blocks already read because page injection is degrade-only.
    #[cfg(test)]
    pub(crate) async fn page_blocks(
        &self,
        ctx: &dyn Ctx,
        pages: &str,
        page_id: &str,
    ) -> Option<Vec<Block>> {
        let budget = SiblingReadBudget::default();
        self.page_blocks_for_execute(ctx, pages, page_id, &budget)
            .await
    }

    pub(crate) async fn page_blocks_for_execute(
        &self,
        ctx: &dyn Ctx,
        pages: &str,
        page_id: &str,
        budget: &SiblingReadBudget,
    ) -> Option<Vec<Block>> {
        let mut remaining_queries = crate::MAX_SIBLING_QUERY_READS;
        match self
            .page_blocks_with_budget(ctx, pages, page_id, &mut remaining_queries, budget)
            .await
        {
            PageBlocksRead::Complete(blocks) => blocks,
            PageBlocksRead::Partial(mut blocks) => {
                append_page_read_marker(&mut blocks);
                Some(blocks)
            }
        }
    }

    async fn page_blocks_with_budget(
        &self,
        ctx: &dyn Ctx,
        pages: &str,
        page_id: &str,
        remaining_queries: &mut usize,
        budget: &SiblingReadBudget,
    ) -> PageBlocksRead {
        let mut after = None;
        let mut blocks = Vec::new();
        while *remaining_queries > 0 {
            let query = pages::encode_query(&PageQuery::GetPage {
                page_id: page_id.to_string(),
                after: after.clone(),
                limit: pages::MAX_PAGE_QUERY_LIMIT,
            });
            if !budget.reserve_query(pages, &query) {
                return partial_blocks(blocks);
            }
            *remaining_queries -= 1;
            let reply = match ctx.query(pages, &query).await {
                Ok(reply) => reply,
                Err(_) => return partial_blocks(blocks),
            };
            let page = match pages::decode_reply(&reply) {
                Ok(PageReply::Page(Some(page))) => page,
                _ => return partial_blocks(blocks),
            };
            let previous_len = blocks.len();
            blocks.extend(page.blocks);
            let made_progress = blocks.len() > previous_len;
            if !made_progress {
                return partial_blocks(blocks);
            }
            let cursor_repeated = page.next_after.is_some() && page.next_after == after;
            if cursor_repeated {
                return partial_blocks(blocks);
            }
            let page_is_complete = page.next_after.is_none();
            let render_is_full = page_render_reaches_budget(page_id, &blocks, &self.net_query());
            if page_is_complete || render_is_full {
                return PageBlocksRead::Complete(Some(blocks));
            }
            after = page.next_after;
        }
        partial_blocks(blocks)
    }

    /// the `duck://files` attachment section for a run: parse the confined file
    /// refs from the same sources, resolve each against COMMITTED files state at
    /// compose height, and render its TEXT content. Images/binaries are named,
    /// not inlined (agent input is text-only); an unresolvable ref renders its
    /// marker. `None` when no files module is wired or no file ref parses;
    /// NEVER a failure.
    pub(crate) async fn attachment_context(
        &self,
        ctx: &dyn Ctx,
        sources: &[&str],
        remaining_queries: &mut usize,
        budget: &SiblingReadBudget,
    ) -> Option<String> {
        let files = self.files.clone()?;
        let paths = parse_duck_refs(sources).files;
        if paths.is_empty() {
            return None;
        }
        let mut resolved = Vec::new();
        for path in paths {
            let query = files_encode_query(&FilesQuery::Read {
                path: path.clone(),
                snapshot: None,
                offset: 0,
                len: ATTACHMENT_CONTEXT_BYTES as u64,
            });
            let read_budget_exhausted =
                *remaining_queries == 0 || !budget.reserve_query(&files, &query);
            if read_budget_exhausted {
                let section = render_attachments_section(&resolved);
                return Some(mark_attachment_section_truncated(section));
            }
            *remaining_queries -= 1;
            let content = self.attachment_content(ctx, &files, &query).await;
            resolved.push((path, content));
            let section = render_attachments_section(&resolved);
            if section.ends_with(ATTACHMENT_TRUNCATION_MARKER) {
                return Some(section);
            }
        }
        Some(render_attachments_section(&resolved))
    }

    /// one committed attachment's content at compose height. a query/decode
    /// failure degrades to `NotFound`; non-UTF-8 bytes (an image, a binary) are
    /// `Binary` — the agent plane cannot ingest them. bounded by the section
    /// budget so a single file can't blow the read.
    async fn attachment_content(
        &self,
        ctx: &dyn Ctx,
        files: &ModuleId,
        query: &[u8],
    ) -> Attachment {
        let Ok(reply) = ctx.query(files, query).await else {
            return Attachment::NotFound;
        };
        let b64 = match files_decode_reply(&reply) {
            Ok(FilesReply::Read { b64, .. }) => b64,
            _ => return Attachment::NotFound,
        };
        let Ok(bytes) = STANDARD.decode(b64.as_bytes()) else {
            return Attachment::NotFound;
        };
        decode_attachment(&bytes)
    }
}

/// classify read bytes as text or binary. A read capped at the budget can slice
/// a UTF-8 file mid-codepoint, so a failure confined to the final ≤3 bytes (a
/// truncated trailing char) still counts as TEXT — the valid prefix is inlined.
/// A failure earlier in the buffer is genuinely non-UTF-8 (an image, a binary).
/// pure — same bytes, same classification, on every validator.
fn decode_attachment(bytes: &[u8]) -> Attachment {
    match std::str::from_utf8(bytes) {
        Ok(text) => Attachment::Text(text.to_string()),
        Err(e) => {
            let valid = e.valid_up_to();
            if valid > 0 && bytes.len() - valid <= 3 {
                // only the truncated trailing codepoint is invalid.
                Attachment::Text(
                    std::str::from_utf8(&bytes[..valid])
                        .expect("valid_up_to is a utf-8 boundary")
                        .to_string(),
                )
            } else {
                Attachment::Binary
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge_source::{ForgeItem, ForgeItemKind, ForgeItemState};

    /// the `?net=` of a network whose chain id is `<name>#d0cdf950`.
    const TEST_NET: &str = "?net=d0cdf950";

    fn issue(body: &str) -> ForgeItem {
        ForgeItem {
            number: 7,
            kind: ForgeItemKind::Issue,
            title: "Fix the flaky gate".into(),
            state: ForgeItemState::Open,
            body: body.into(),
            source_branch: None,
            target_branch: None,
        }
    }

    fn pr() -> ForgeItem {
        ForgeItem {
            number: 8,
            kind: ForgeItemKind::Pr,
            title: "Wire the thing".into(),
            state: ForgeItemState::Open,
            body: "please review".into(),
            source_branch: Some("feature/x".into()),
            target_branch: Some("dev".into()),
        }
    }

    #[test]
    fn an_issue_context_carries_coordinates_title_branch_and_body() {
        let ctx = render_item_context("app", &issue("repro:\n- run it twice"), "agent/item-7");
        assert!(ctx.contains("repo: app"), "{ctx}");
        assert!(ctx.contains("issue #7"), "{ctx}");
        assert!(ctx.contains("(open)"), "{ctx}");
        assert!(ctx.contains("title: Fix the flaky gate"), "{ctx}");
        assert!(ctx.contains("work branch: agent/item-7"), "{ctx}");
        assert!(ctx.contains("inspect recent Git history"), "{ctx}");
        assert!(ctx.contains("do not force Conventional Commits"), "{ctx}");
        assert!(ctx.contains("repro:\n- run it twice"), "{ctx}");
        assert!(
            !ctx.contains("source branch") && !ctx.contains("target branch"),
            "an issue renders no PR branch lines: {ctx}"
        );
    }

    #[test]
    fn a_pr_context_carries_its_source_and_target_branches() {
        let ctx = render_item_context("app", &pr(), "feature/x");
        assert!(ctx.contains("pr #8"), "{ctx}");
        assert!(ctx.contains("pr source branch: feature/x"), "{ctx}");
        assert!(ctx.contains("pr target branch: dev"), "{ctx}");
        assert!(ctx.contains("work branch: feature/x"), "{ctx}");
    }

    #[test]
    fn rendering_is_byte_deterministic() {
        let a = render_item_context("app", &issue("body"), "agent/item-7");
        let b = render_item_context("app", &issue("body"), "agent/item-7");
        assert_eq!(a.as_bytes(), b.as_bytes());
    }

    #[test]
    fn an_oversized_body_truncates_to_the_cap_with_a_marker() {
        let ctx = render_item_context("app", &issue(&"x".repeat(64 * 1024)), "agent/item-7");
        assert_eq!(
            ctx.len(),
            MAX_CONTEXT_BYTES,
            "the capped render fills the budget exactly for ascii input"
        );
        assert!(
            ctx.ends_with(TRUNCATION_MARKER),
            "a truncated context states so: …{}",
            &ctx[ctx.len() - 60..]
        );
        assert!(ctx.starts_with("Forge item context"), "{ctx}");
    }

    #[test]
    fn a_context_within_the_cap_is_untouched() {
        let ctx = render_item_context("app", &issue("short body"), "agent/item-7");
        assert!(ctx.len() <= MAX_CONTEXT_BYTES);
        assert!(!ctx.contains(TRUNCATION_MARKER));
    }

    #[test]
    fn truncation_respects_utf8_boundaries() {
        // a body of multibyte chars — the byte budget will not land on a char
        // boundary; the cut must back up, never split a char.
        let ctx = render_item_context("app", &issue(&"é".repeat(32 * 1024)), "agent/item-7");
        assert!(ctx.len() <= MAX_CONTEXT_BYTES);
        assert!(ctx.ends_with(TRUNCATION_MARKER));
        // re-validating utf-8 is implicit — `ctx` is a String — but pin the
        // budget is nearly filled (backed up at most one char, not one chunk).
        assert!(ctx.len() > MAX_CONTEXT_BYTES - 4);
    }

    #[test]
    fn truncation_is_deterministic() {
        let a = render_item_context("app", &issue(&"y".repeat(64 * 1024)), "agent/item-7");
        let b = render_item_context("app", &issue(&"y".repeat(64 * 1024)), "agent/item-7");
        assert_eq!(a.as_bytes(), b.as_bytes());
    }

    // ---- `[[page:<id>]]` page-spec injection (M2) -----------------------------

    /// a page block with derived-by-the-module fields filled in by hand.
    fn block(id: &str, parent: Option<&str>, kind: BlockKind, text: &str) -> Block {
        Block {
            id: id.into(),
            parent: parent.map(str::to_string),
            page: "root".into(),
            kind,
            text: text.into(),
            marks: Vec::new(),
            checked: false,
            children: Vec::new(),
        }
    }

    /// a whole page in preorder: root first, each subtree before the next
    /// sibling — exactly what `GetPage` serves.
    fn preorder_page() -> Vec<Block> {
        let mut todo_done = block("b-done", Some("root"), BlockKind::Todo, "design it");
        todo_done.checked = true;
        vec![
            block("root", None, BlockKind::Page, "Project Plan"),
            block("b-intro", Some("root"), BlockKind::Paragraph, "the intro"),
            block("b-h1", Some("root"), BlockKind::Heading1, "Goals"),
            block("b-h2", Some("root"), BlockKind::Heading2, "Near term"),
            block("b-h3", Some("root"), BlockKind::Heading3, "This week"),
            block("b-todo", Some("root"), BlockKind::Todo, "ship it"),
            todo_done,
            block("b-list", Some("root"), BlockKind::Bulleted, "a bullet"),
            // nesting: a child of the bullet rides preorder-adjacent, one
            // level deeper.
            block("b-sub", Some("b-list"), BlockKind::Bulleted, "a sub bullet"),
            block("b-num", Some("root"), BlockKind::Numbered, "a step"),
            block("b-code", Some("root"), BlockKind::Code, "let x = 1;"),
            block("b-quote", Some("root"), BlockKind::Quote, "quoted words"),
            block("b-div", Some("root"), BlockKind::Divider, ""),
        ]
    }

    #[test]
    fn duck_page_refs_parse_across_sources_deduped_in_first_seen_order() {
        let refs = parse_duck_refs(&[
            "see [Plan](duck://page/plan) and [Spec](duck://page/spec) and [P](duck://page/plan)",
            "the body cites [S](duck://page/spec) then [N](duck://page/notes)",
        ]);
        assert_eq!(refs.pages, vec!["plan", "spec", "notes"]);
        assert!(refs.files.is_empty());
    }

    /// A produced link names its network (`?net=<digest>`). The run resolves
    /// against the chain it runs on, so the component is dropped — never
    /// folded into the id, which would make every copied link resolve to
    /// nothing.
    #[test]
    fn a_ref_that_names_its_network_resolves_to_the_bare_id() {
        let refs = parse_duck_refs(&[
            "[Plan](duck://page/plan?net=d0cdf950)",
            "[Shot](duck://files/shared/attachments/u1/s.png?net=d0cdf950)",
        ]);
        assert_eq!(refs.pages, vec!["plan"]);
        assert_eq!(refs.files, vec!["/shared/attachments/u1/s.png"]);
    }

    #[test]
    fn malformed_or_non_duck_refs_are_skipped_never_a_failure() {
        let refs = parse_duck_refs(&[
            "[empty](duck://page/)",        // empty id
            "[ext](https://example.com)",   // not a duck scheme
            "[unterminated](duck://page/x", // no closing paren
            "[nested](duck://page/a/b)",    // page id must be one segment
            "plain [not a link] text",      // no url
            "ok [Good](duck://page/ok) ok",
        ]);
        assert_eq!(refs.pages, vec!["ok"]);
    }

    #[test]
    fn duck_file_refs_are_confined_to_the_attachments_root() {
        let refs = parse_duck_refs(&[
            "![img](duck://files/shared/attachments/u1/cat.png)", // ok (embed marker ignored)
            "[doc](duck://files/shared/attachments/u2/notes.md)", // ok
            "[home](duck://files/home/alice/secret.txt)",         // outside root — dropped
            "[skill](duck://files/shared/skills/a/b)",            // wrong subtree — dropped
            "[deep](duck://files/shared/attachments/a/b/c)",      // wrong depth — dropped
            "[dots](duck://files/shared/attachments/../secret)",  // dot-segment — dropped
        ]);
        assert_eq!(
            refs.files,
            vec![
                "/shared/attachments/u1/cat.png",
                "/shared/attachments/u2/notes.md",
            ],
        );
        assert!(refs.pages.is_empty());
    }

    #[test]
    fn a_message_mixing_page_and_file_refs_parses_both() {
        let refs = parse_duck_refs(&[
            "context in [Plan](duck://page/plan) and see ![shot](duck://files/shared/attachments/u/s.png)",
        ]);
        assert_eq!(refs.pages, vec!["plan"]);
        assert_eq!(refs.files, vec!["/shared/attachments/u/s.png"]);
    }

    #[test]
    fn a_shallow_file_path_under_the_root_is_not_confined() {
        // exactly one segment under the root (no <dir>/<name>) — dropped, same
        // as the console tokenizer.
        let refs = parse_duck_refs(&["[x](duck://files/shared/attachments/only)"]);
        assert!(refs.files.is_empty());
    }

    #[test]
    fn a_utf8_file_cut_mid_codepoint_is_still_text_not_binary() {
        // the read cap can slice a multibyte char; the valid prefix is text.
        let mut bytes = "grüße".repeat(4).into_bytes();
        bytes.push(0xC3); // a dangling UTF-8 lead byte (a truncated 'ü')
        assert!(matches!(decode_attachment(&bytes), Attachment::Text(_)));
        // but a genuine image (non-utf8 EARLY) stays binary.
        assert_eq!(
            decode_attachment(&[0x89, 0x50, 0x4e, 0xff, 0xfe, 0x01]),
            Attachment::Binary
        );
    }

    #[test]
    fn a_page_renders_headings_todos_lists_code_and_nesting_from_preorder() {
        let section = render_pages_section(&[("plan".into(), Some(preorder_page()))], TEST_NET);
        assert!(section.starts_with("Referenced pages:"), "{section}");
        assert!(
            section.contains("[Project Plan](duck://page/plan?net=d0cdf950)"),
            "{section}"
        );
        assert!(section.contains("\nthe intro\n"), "{section}");
        assert!(section.contains("\n# Goals\n"), "{section}");
        assert!(section.contains("\n## Near term\n"), "{section}");
        assert!(section.contains("\n### This week\n"), "{section}");
        // todos carry the M2.3-targetable block id inline.
        assert!(
            section.contains("\n- [ ] ship it [blk:b-todo]\n"),
            "{section}"
        );
        assert!(
            section.contains("\n- [x] design it [blk:b-done]\n"),
            "{section}"
        );
        assert!(section.contains("\n- a bullet\n"), "{section}");
        // preorder nesting: the child bullet indents one level.
        assert!(section.contains("\n  - a sub bullet\n"), "{section}");
        assert!(section.contains("\n1. a step\n"), "{section}");
        assert!(section.contains("\n```\nlet x = 1;\n```\n"), "{section}");
        assert!(section.contains("\nquoted words\n"), "{section}");
        assert!(section.contains("\n---"), "{section}");
    }

    #[test]
    fn a_missing_page_renders_the_one_line_marker() {
        let section = render_pages_section(
            &[
                ("plan".into(), Some(preorder_page())),
                ("gone".into(), None),
            ],
            TEST_NET,
        );
        assert!(section.contains("[page gone — not found]"), "{section}");
        // an empty reply Vec is as unresolvable as None.
        let empty = render_pages_section(&[("void".into(), Some(Vec::new()))], TEST_NET);
        assert!(empty.contains("[page void — not found]"), "{empty}");
    }

    #[test]
    fn the_page_section_truncates_at_its_budget_with_a_marker() {
        let big = vec![
            block("root", None, BlockKind::Page, "Big"),
            block(
                "b1",
                Some("root"),
                BlockKind::Paragraph,
                &"x".repeat(128 * 1024),
            ),
        ];
        let section = render_pages_section(&[("big".into(), Some(big))], TEST_NET);
        assert_eq!(section.len(), PAGE_CONTEXT_BYTES);
        assert!(
            section.ends_with(PAGE_TRUNCATION_MARKER),
            "{}",
            &section[section.len() - 60..]
        );
    }

    #[test]
    fn page_truncation_respects_utf8_boundaries() {
        let big = vec![
            block("root", None, BlockKind::Page, "Big"),
            block(
                "b1",
                Some("root"),
                BlockKind::Paragraph,
                &"é".repeat(64 * 1024),
            ),
        ];
        let section = render_pages_section(&[("big".into(), Some(big))], TEST_NET);
        assert!(section.len() <= PAGE_CONTEXT_BYTES);
        assert!(section.len() > PAGE_CONTEXT_BYTES - 4);
        assert!(section.ends_with(PAGE_TRUNCATION_MARKER));
    }

    #[test]
    fn page_rendering_is_byte_deterministic() {
        let pages = || {
            vec![
                ("plan".to_string(), Some(preorder_page())),
                ("gone".to_string(), None),
            ]
        };
        let a = render_pages_section(&pages(), TEST_NET);
        let b = render_pages_section(&pages(), TEST_NET);
        assert_eq!(a.as_bytes(), b.as_bytes());
    }

    /// A page link the injector renders NAMES ITS NETWORK, from the chain id
    /// the guest reads out of genesis config — so an agent echoing the header
    /// into chat posts a link a reader on another network is REFUSED, instead
    /// of one that silently resolves against their own store.
    #[test]
    fn a_rendered_page_link_carries_the_network_it_was_rendered_on() {
        let on_a_network = RunsModule::new(
            "runs", "chat", "saga", "tagging", "dispatch", "agent", None, None,
        )
        .with_chain_id("dognet#d0cdf950");
        assert_eq!(on_a_network.net_query(), TEST_NET);
        let section = render_pages_section(
            &[("plan".into(), Some(preorder_page()))],
            &on_a_network.net_query(),
        );
        assert!(
            section.contains("[Project Plan](duck://page/plan?net=d0cdf950)"),
            "{section}"
        );
        // an unwired chain id (dev tools, tests) renders the hand-typed form.
        let nowhere = RunsModule::new(
            "runs", "chat", "saga", "tagging", "dispatch", "agent", None, None,
        );
        assert_eq!(nowhere.net_query(), "");
        let bare = render_pages_section(
            &[("plan".into(), Some(preorder_page()))],
            &nowhere.net_query(),
        );
        assert!(bare.contains("[Project Plan](duck://page/plan)"), "{bare}");
    }

    #[test]
    fn message_text_concatenates_the_anchor_blocks() {
        use chat::{Block as ChatBlock, MessageHead};
        let message = MessageView {
            channel_id: "general".into(),
            seq: 1,
            head: MessageHead {
                message_id: "m1".into(),
                author: chat::AuthorRef::User(vec![1; 32]),
                blocks: vec![
                    ChatBlock::paragraph("see [Plan](duck://page/plan)"),
                    ChatBlock::Code {
                        lang: None,
                        text: "and [Spec](duck://page/spec)".into(),
                    },
                ],
                created_at: 0,
                rev: 0,
                edited_at: None,
                base_rev: None,
                deleted: false,
                thread: None,
                reply_count: 0,
                last_reply_seq: None,
            },
        };
        let text = message_text(&message);
        assert!(text.contains("see [Plan](duck://page/plan)"), "{text}");
        assert!(text.contains("and [Spec](duck://page/spec)"), "{text}");
    }

    // ---- duck://files attachment injection ------------------------------------

    #[test]
    fn attachments_render_text_binary_and_missing() {
        let section = render_attachments_section(&[
            (
                "/shared/attachments/u1/notes.md".into(),
                Attachment::Text("# Plan\nship it".into()),
            ),
            ("/shared/attachments/u2/cat.png".into(), Attachment::Binary),
            (
                "/shared/attachments/u3/gone.txt".into(),
                Attachment::NotFound,
            ),
        ]);
        assert!(section.starts_with("Referenced attachments:"), "{section}");
        // text content is inlined under a name header (name = last segment).
        assert!(
            section.contains("[attachment: notes.md]\n# Plan\nship it"),
            "{section}"
        );
        // an image is named, never inlined (the agent plane is text-only).
        assert!(
            section.contains("[attachment: cat.png — binary content, not shown]"),
            "{section}"
        );
        assert!(
            section.contains("[attachment: gone.txt — not found]"),
            "{section}"
        );
    }

    #[test]
    fn the_attachment_section_truncates_at_its_budget_with_a_marker() {
        let big = render_attachments_section(&[(
            "/shared/attachments/u/big.txt".into(),
            Attachment::Text("x".repeat(128 * 1024)),
        )]);
        assert_eq!(big.len(), ATTACHMENT_CONTEXT_BYTES);
        assert!(
            big.ends_with(ATTACHMENT_TRUNCATION_MARKER),
            "{}",
            &big[big.len() - 60..]
        );
    }

    #[test]
    fn attachment_truncation_respects_utf8_boundaries() {
        let big = render_attachments_section(&[(
            "/shared/attachments/u/big.txt".into(),
            Attachment::Text("é".repeat(64 * 1024)),
        )]);
        assert!(big.len() <= ATTACHMENT_CONTEXT_BYTES);
        assert!(big.len() > ATTACHMENT_CONTEXT_BYTES - 4);
        assert!(big.ends_with(ATTACHMENT_TRUNCATION_MARKER));
    }

    #[test]
    fn attachment_rendering_is_byte_deterministic() {
        let items = || {
            vec![
                (
                    "/shared/attachments/u/a.md".to_string(),
                    Attachment::Text("hi".into()),
                ),
                (
                    "/shared/attachments/u/b.png".to_string(),
                    Attachment::Binary,
                ),
            ]
        };
        assert_eq!(
            render_attachments_section(&items()).as_bytes(),
            render_attachments_section(&items()).as_bytes(),
        );
    }
}
