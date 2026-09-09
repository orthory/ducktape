//! Pages save planning and comment anchors. Editing and history live in the guest.

pub mod guest_document;
#[path = "../../../crates/views/pages/src/editor_indent.rs"]
mod indent;
#[path = "../../../crates/views/pages/src/editor_inline.rs"]
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

/// The block a document LINE sits in — where a new comment anchors. The line
/// arrives from the ice `editor_cursor_line` inspector, which BORROWS the
/// buffer: an `editor`-valued sync argument is a `Content::clone`, and that
/// clone REBUILDS FROM TEXT — the cursor resets to the origin. "" on the
/// title line (and on unsaved fresh lines) reads as "the page".
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

/// Where a comment thread anchors, in the reader's own words: the line number
/// and a snippet of the block it marks, or the page itself.
/// The composer's own caption: where a NEW comment will anchor.
pub fn comment_compose_hint(
    blocks: &[crate::backend::PageBlock],
    target: &str,
    page_id: &str,
) -> String {
    format!(
        "New comment on {}",
        anchor_label(&comment_anchor_labels(blocks), target, page_id)
    )
}

pub fn comment_anchor_label(
    blocks: Vec<crate::backend::PageBlock>,
    target: String,
    page_id: String,
) -> String {
    anchor_label(&comment_anchor_labels(&blocks), &target, &page_id)
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

fn comment_anchor_labels(blocks: &[crate::backend::PageBlock]) -> BTreeMap<String, String> {
    let text_by_id: BTreeMap<&str, &str> = blocks
        .iter()
        .map(|block| (block.id.as_str(), block.text.as_str()))
        .collect();
    sync::line_spans(blocks)
        .into_iter()
        .map(|(id, start, _)| {
            let text = text_by_id.get(id.as_str()).copied().unwrap_or_default();
            (id, format!("line {start} · {}", anchor_snippet(text)))
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
    fn comment_anchors_read_as_lines_and_wash_every_line_of_the_block() {
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
        assert_eq!(
            comment_anchor_label(blocks.clone(), "page-id".into(), "page-id".into()),
            "this page"
        );
        assert_eq!(
            comment_anchor_label(blocks.clone(), "para".into(), "page-id".into()),
            "line 1 · para"
        );
        assert_eq!(
            comment_anchor_label(blocks.clone(), "gone".into(), "page-id".into()),
            "a removed block"
        );
        // The code block owns lines 2..=5 (fence, two body lines, fence).
        assert_eq!(commented_lines(&blocks, &["a\nb".into()]), vec![2, 3, 4, 5]);
        // A caret line inside the code body anchors a comment on that block —
        // resolved by LINE (Content::clone resets the cursor, so an
        // editor-valued sync could never read it).
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

    }

}
