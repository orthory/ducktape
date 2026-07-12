//! deterministic forge item-context injection (M1): the instructions section
//! a forge-channel run's envelope carries in its `context` field, rendered
//! from COMMITTED tracker state at compose height (I1) and byte-capped.
//!
//! extended by M2 with `[[page:<id>]]` page-spec injection: refs parsed from
//! the trigger message text and the injected item body resolve against
//! COMMITTED pages state at compose height and render each referenced page's
//! subtree into the same `context` section.
//!
//! the wording is part of the committed prompt input (the envelope JSON is
//! the provider prompt), so the render is a pure function of the item record
//! + coordinates: same committed state, same bytes, on every validator.

use std::collections::BTreeMap;

use chat::MessageView;
use pages::{Block, BlockKind, PageQuery, PageReply};
use sdk::Ctx;

use crate::RunsModule;
use crate::forge_source::{ForgeItem, ForgeItemKind};

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

// ---- `[[page:<id>]]` page-spec injection (M2) ---------------------------------

/// the page-section byte budget across ALL injected pages — separate from
/// (and additional to) the 16 KiB item-context cap above.
pub(crate) const PAGE_CONTEXT_BYTES: usize = 64 * 1024;

/// the deterministic page-section truncation marker.
const PAGE_TRUNCATION_MARKER: &str = "\n[page context truncated at 64 KiB]";

/// the ref syntax: `[[page:` `<id>` `]]`.
const PAGE_REF_OPEN: &str = "[[page:";
const PAGE_REF_CLOSE: &str = "]]";

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

/// parse `[[page:<id>]]` refs from the given sources in order, first-seen
/// dedupe across all of them. a malformed ref (empty id, whitespace or
/// bracket chars, unterminated) is skipped — NEVER a failure.
pub(crate) fn parse_page_refs(sources: &[&str]) -> Vec<String> {
    let mut refs: Vec<String> = Vec::new();
    for source in sources {
        let mut rest = *source;
        while let Some(open) = rest.find(PAGE_REF_OPEN) {
            rest = &rest[open + PAGE_REF_OPEN.len()..];
            let Some(close) = rest.find(PAGE_REF_CLOSE) else {
                break; // unterminated — nothing after this can close.
            };
            let id = &rest[..close];
            rest = &rest[close + PAGE_REF_CLOSE.len()..];
            let malformed =
                id.is_empty() || id.contains(|c: char| c.is_whitespace() || c == '[' || c == ']');
            if !malformed && !refs.iter().any(|r| r == id) {
                refs.push(id.to_string());
            }
        }
    }
    refs
}

/// render the page section from resolved refs, in the given (first-seen)
/// order: each page's preorder subtree depth-first to markdown, an
/// unresolvable page as a one-line marker, the whole section capped at
/// [`PAGE_CONTEXT_BYTES`]. pure — same input, same bytes.
pub(crate) fn render_pages_section(pages: &[(String, Option<Vec<Block>>)]) -> String {
    let rendered: Vec<String> = pages
        .iter()
        .map(|(page_id, blocks)| render_page(page_id, blocks.as_deref()))
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
fn render_page(page_id: &str, blocks: Option<&[Block]>) -> String {
    let Some((root, rest)) = blocks.and_then(|b| b.split_first()) else {
        return format!("[[page:{page_id} — not found]]");
    };
    let mut out = format!("[[page:{page_id}]] — {}", root.text);
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

impl RunsModule {
    /// the `[[page:<id>]]` page section for a run (M2): parse refs from the
    /// given sources (the trigger message text, then the injected item body),
    /// resolve each against COMMITTED pages state at compose height — the
    /// same cross-module query lane as the forge tracker reads (I1) — and
    /// render. `None` when no pages module is wired or no ref parses; an
    /// unresolvable ref renders its marker, NEVER a failure.
    pub(crate) async fn page_context(&self, ctx: &dyn Ctx, sources: &[&str]) -> Option<String> {
        let pages = self.pages.clone()?;
        let refs = parse_page_refs(sources);
        if refs.is_empty() {
            return None;
        }
        let mut resolved = Vec::new();
        for page_id in refs {
            let blocks = self.page_blocks(ctx, &pages, &page_id).await;
            resolved.push((page_id, blocks));
        }
        Some(render_pages_section(&resolved))
    }

    /// one committed page in preorder, or `None` when it does not exist —
    /// a query/decode error degrades to `None` too (page injection is
    /// context garnish; it never fails a run).
    pub(crate) async fn page_blocks(
        &self,
        ctx: &dyn Ctx,
        pages: &str,
        page_id: &str,
    ) -> Option<Vec<Block>> {
        let reply = ctx
            .query(
                pages,
                &pages::encode_query(&PageQuery::GetPage {
                    page_id: page_id.to_string(),
                }),
            )
            .await
            .ok()?;
        match pages::decode_reply(&reply) {
            Ok(PageReply::Page(blocks)) => blocks,
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge_source::{ForgeItem, ForgeItemKind, ForgeItemState};

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
    fn page_refs_parse_across_sources_deduped_in_first_seen_order() {
        let refs = parse_page_refs(&[
            "see [[page:plan]] and [[page:spec]] and [[page:plan]] again",
            "the body cites [[page:spec]] then [[page:notes]]",
        ]);
        assert_eq!(refs, vec!["plan", "spec", "notes"]);
    }

    #[test]
    fn malformed_page_refs_are_skipped_never_a_failure() {
        let refs = parse_page_refs(&[
            "[[page:]]",              // empty id
            "[[page:has space]]",     // whitespace
            "[[page:unterminated",    // no close
            "[[page:a]b]]",           // bracket inside the id
            "[page:not-a-ref]]",      // wrong open
            "trailing [[page:ok]] ok",
        ]);
        assert_eq!(refs, vec!["ok"]);
    }

    #[test]
    fn a_page_renders_headings_todos_lists_code_and_nesting_from_preorder() {
        let section = render_pages_section(&[("plan".into(), Some(preorder_page()))]);
        assert!(section.starts_with("Referenced pages:"), "{section}");
        assert!(section.contains("[[page:plan]] — Project Plan"), "{section}");
        assert!(section.contains("\nthe intro\n"), "{section}");
        assert!(section.contains("\n# Goals\n"), "{section}");
        assert!(section.contains("\n## Near term\n"), "{section}");
        assert!(section.contains("\n### This week\n"), "{section}");
        // todos carry the M2.3-targetable block id inline.
        assert!(section.contains("\n- [ ] ship it [blk:b-todo]\n"), "{section}");
        assert!(section.contains("\n- [x] design it [blk:b-done]\n"), "{section}");
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
        let section = render_pages_section(&[
            ("plan".into(), Some(preorder_page())),
            ("gone".into(), None),
        ]);
        assert!(section.contains("[[page:gone — not found]]"), "{section}");
        // an empty reply Vec is as unresolvable as None.
        let empty = render_pages_section(&[("void".into(), Some(Vec::new()))]);
        assert!(empty.contains("[[page:void — not found]]"), "{empty}");
    }

    #[test]
    fn the_page_section_truncates_at_its_budget_with_a_marker() {
        let big = vec![
            block("root", None, BlockKind::Page, "Big"),
            block("b1", Some("root"), BlockKind::Paragraph, &"x".repeat(128 * 1024)),
        ];
        let section = render_pages_section(&[("big".into(), Some(big))]);
        assert_eq!(section.len(), PAGE_CONTEXT_BYTES);
        assert!(section.ends_with(PAGE_TRUNCATION_MARKER), "{}", &section[section.len() - 60..]);
    }

    #[test]
    fn page_truncation_respects_utf8_boundaries() {
        let big = vec![
            block("root", None, BlockKind::Page, "Big"),
            block("b1", Some("root"), BlockKind::Paragraph, &"é".repeat(64 * 1024)),
        ];
        let section = render_pages_section(&[("big".into(), Some(big))]);
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
        let a = render_pages_section(&pages());
        let b = render_pages_section(&pages());
        assert_eq!(a.as_bytes(), b.as_bytes());
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
                    ChatBlock::paragraph("see [[page:plan]]"),
                    ChatBlock::Code {
                        lang: None,
                        text: "and [[page:spec]]".into(),
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
            reactions: Vec::new(),
            channel_head_seq: 1,
        };
        let text = message_text(&message);
        assert!(text.contains("see [[page:plan]]"), "{text}");
        assert!(text.contains("and [[page:spec]]"), "{text}");
    }
}
