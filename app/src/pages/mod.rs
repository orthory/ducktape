//! Pages save planning and comment anchors. Editing and history live in the guest.

pub mod guest_document;
#[path = "../../../crates/views/pages/src/editor_indent.rs"]
mod indent;
#[path = "../../../crates/views/pages/src/editor_inline.rs"]
// Pages guest consumes document links; native Chat consumes inline emphasis.
#[allow(dead_code)]
pub mod inline;
#[cfg(test)]
#[path = "../../../crates/views/pages/src/editor_markdown.rs"]
pub mod markdown;
pub mod sync;

use std::collections::BTreeMap;

/// The `sync` predicate at the extern boundary, which hands values, not
/// borrows.
pub fn has_unclosed_fence(text: String) -> bool {
    sync::has_unclosed_fence(&text)
}

/// Resolve an accepted guest caret line against persisted block anchors.
/// The title and unsaved fresh lines select the page itself.
pub fn block_at_line_target(blocks: Vec<crate::backend::PageBlock>, line: i64) -> String {
    let line = usize::try_from(line).unwrap_or(0);
    sync::block_at_line(&blocks, line)
}

/// The document lines wearing a commented block's wash, for the highlighter.
pub fn commented_lines(blocks: &[crate::backend::PageBlock], targets: &[String]) -> Vec<i64> {
    let mut lines = Vec::new();
    for (id, start, len) in sync::line_spans(blocks) {
        if !targets.contains(&id) {
            continue;
        }
        for line in start..start + len {
            lines.push(line as i64);
        }
    }
    lines
}

/// Where a comment thread anchors, in the reader's own words: the opening of
/// the block it marks, quoted, or the page itself. No line number — the
/// editor draws none, so "line 3" named nothing the reader could find.
/// THE COMPOSER AT THE CARD'S FOOT ALWAYS OPENS A NEW THREAD on the scope the
/// card is showing — never a reply, which has its own box inside its thread.
/// `scope` is a block id, or empty (or the page's own id) for the page.
pub fn comment_compose_hint(
    blocks: &[crate::backend::PageBlock],
    scope: &str,
    page_id: &str,
) -> String {
    if is_page_scope(scope, page_id) {
        return "Comment on this page".into();
    }
    format!(
        "New thread on {}",
        anchor_label(&comment_anchor_labels(blocks), scope, page_id)
    )
}

/// The card's title: the block it is scoped to, quoted, or the page and how
/// many open threads it is carrying.
pub fn comment_scope_label(
    blocks: &[crate::backend::PageBlock],
    scope: &str,
    page_id: &str,
    open_threads: i64,
) -> String {
    if !is_page_scope(scope, page_id) {
        return anchor_label(&comment_anchor_labels(blocks), scope, page_id);
    }
    match open_threads {
        1 => "This page · 1 thread".into(),
        count => format!("This page · {count} threads"),
    }
}

fn is_page_scope(scope: &str, page_id: &str) -> bool {
    scope.is_empty() || scope == page_id
}

/// One thread-list row with its document anchor already resolved. The Ice
/// view reads the scalar; it never clones and searches the whole block list
/// once per thread.
#[derive(Clone, Debug, Hash, PartialEq, serde::Serialize)]
pub struct PageCommentThreadRow {
    pub thread: crate::backend::PageCommentThread,
    pub anchor: String,
}

pub fn page_comment_thread_rows(
    blocks: Vec<crate::backend::PageBlock>,
    threads: Vec<crate::backend::PageCommentThread>,
    page_id: String,
) -> Vec<PageCommentThreadRow> {
    let labels = comment_anchor_labels(&blocks);
    threads
        .into_iter()
        .map(|thread| PageCommentThreadRow {
            anchor: anchor_label(&labels, &thread.target, &page_id),
            thread,
        })
        .collect()
}

pub fn comment_rows_for_target(
    rows: Vec<PageCommentThreadRow>,
    target: String,
) -> Vec<PageCommentThreadRow> {
    if target.is_empty() {
        return rows;
    }
    rows.into_iter()
        .filter(|row| row.thread.target == target)
        .collect()
}

fn comment_anchor_labels(blocks: &[crate::backend::PageBlock]) -> BTreeMap<String, String> {
    let text_by_id: BTreeMap<&str, &str> = blocks
        .iter()
        .map(|block| (block.id.as_str(), block.text.as_str()))
        .collect();
    sync::line_spans(blocks)
        .into_iter()
        .map(|(id, _, _)| {
            let text = text_by_id.get(id.as_str()).copied().unwrap_or_default();
            (id, format!("“{}”", anchor_snippet(text)))
        })
        .collect()
}

fn anchor_label(labels: &BTreeMap<String, String>, target: &str, page_id: &str) -> String {
    if target.is_empty() || target == page_id {
        return "this page".into();
    }
    labels
        .get(target)
        .cloned()
        .unwrap_or_else(|| "a removed block".into())
}

fn anchor_snippet(text: &str) -> String {
    let text = text.trim();
    match text.char_indices().nth(36) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None if text.is_empty() => "an empty line".into(),
        None => text.to_owned(),
    }
}

pub fn comment_marks(blocks: &[crate::backend::PageBlock], hits: &[String]) -> Vec<(usize, usize)> {
    let mut marks = Vec::new();
    for (id, start, _len) in sync::line_spans(blocks) {
        let count = hits.iter().filter(|hit| *hit == &id).count();
        if count > 0 {
            marks.push((start, count));
        }
    }
    marks
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn comment_anchors_quote_the_block_and_wash_every_line_of_it() {
        let block = |kind: &str, text: &str| crate::backend::PageBlock {
            key: 0,
            id: text.into(),
            parent: String::new(),
            kind: kind.into(),
            text: text.into(),
            pending: false,
            checked: false,
            prefix: String::new(),
            child_count: 0,
        };
        let blocks = vec![block("Text", "para"), block("Code", "a\nb")];
        // The card's title names the scope it is showing: the page and its
        // outstanding count, or the block's opening, quoted.
        assert_eq!(
            comment_scope_label(&blocks, "page-id", "page-id", 4),
            "This page · 4 threads"
        );
        assert_eq!(
            comment_scope_label(&blocks, "", "page-id", 1),
            "This page · 1 thread"
        );
        assert_eq!(comment_scope_label(&blocks, "para", "page-id", 9), "“para”");
        assert_eq!(
            comment_scope_label(&blocks, "gone", "page-id", 9),
            "a removed block"
        );
        // The composer at the card's foot always opens a NEW thread on that
        // same scope — never a reply, which composes inside its own thread.
        assert_eq!(
            comment_compose_hint(&blocks, "", "page-id"),
            "Comment on this page"
        );
        assert_eq!(
            comment_compose_hint(&blocks, "para", "page-id"),
            "New thread on “para”"
        );
        // A thread row still quotes the block it anchors to, page threads
        // included — that is the group header in page scope.
        let thread = |target: &str| crate::backend::PageCommentThread {
            id: "t".into(),
            target: target.into(),
            author: "Reader".into(),
            meta: String::new(),
            resolved: false,
            comment_count: 0,
            comments: Vec::new(),
        };
        let rows = page_comment_thread_rows(
            blocks.clone(),
            vec![thread("page-id"), thread("para")],
            "page-id".into(),
        );
        assert_eq!(rows[0].anchor, "this page");
        assert_eq!(rows[1].anchor, "“para”");
        // The code block owns lines 2..=5 (fence, two body lines, fence).
        assert_eq!(commented_lines(&blocks, &["a\nb".into()]), vec![2, 3, 4, 5]);
        // A caret line inside the code body anchors a comment on that block —
        // resolved by the accepted guest cursor line.
        assert_eq!(block_at_line_target(blocks.clone(), 3), "a\nb");
        assert_eq!(block_at_line_target(blocks, 0), "");
    }

    /// THE CHIP CARRIES ITS COUNT, AND THE COUNT IS THE REPETITION IN `hits`.
    /// Both folds that build `hits` used to `dedup()`, so the number was thrown
    /// away three layers before the chip that needed it and every commented
    /// line drew the same three dots.
    #[test]
    fn margin_badges_sit_on_the_first_line_of_a_block_and_count_its_threads() {
        let block = |kind: &str, text: &str| crate::backend::PageBlock {
            key: 0,
            id: text.into(),
            parent: String::new(),
            kind: kind.into(),
            text: text.into(),
            pending: false,
            checked: false,
            prefix: String::new(),
            child_count: 0,
        };
        // `para` is line 1; the code block owns lines 2..=5 (fence, two body
        // lines, fence) and its chip rides line 2, not one per wrapped row.
        let blocks = vec![block("Text", "para"), block("Code", "a\nb")];
        let hits = |ids: &[&str]| ids.iter().map(|id| (*id).to_owned()).collect::<Vec<_>>();

        assert_eq!(
            comment_marks(&blocks, &hits(&["para", "a\nb"])),
            vec![(1, 1), (2, 1)],
            "one thread each"
        );
        // Three threads on the code block, one on the paragraph.
        assert_eq!(
            comment_marks(&blocks, &hits(&["a\nb", "para", "a\nb", "a\nb"])),
            vec![(1, 1), (2, 3)],
            "the repetition IS the count"
        );
        // A block nobody commented on gets no chip at all.
        assert_eq!(comment_marks(&blocks, &hits(&["para"])), vec![(1, 1)]);
        assert_eq!(comment_marks(&blocks, &[]), Vec::new());
        // A hit naming a block that is gone marks nothing.
        assert_eq!(comment_marks(&blocks, &hits(&["deleted"])), Vec::new());

        // AND THE BADGE COUNTS WHAT IS OPEN. A resolved thread is filed away
        // behind the card's own toggle, so it must not keep a mark burning in
        // the margin: the fold that builds `hits` drops it before it is
        // counted, and the page's own threads mark no line at all.
        let thread = |target: &str, resolved: bool| crate::backend::PageCommentThread {
            id: format!("{target}-{resolved}"),
            target: target.into(),
            author: "Reader".into(),
            meta: String::new(),
            resolved,
            comment_count: 1,
            comments: Vec::new(),
        };
        let open_hits = crate::backend::commented_targets_of(
            vec![
                thread("a\nb", false),
                thread("a\nb", true),
                thread("page-id", false),
                thread("para", false),
            ],
            "page-id".into(),
        );
        assert_eq!(open_hits, ["a\nb", "para"]);
        assert_eq!(comment_marks(&blocks, &open_hits), vec![(1, 1), (2, 1)]);
    }
}
