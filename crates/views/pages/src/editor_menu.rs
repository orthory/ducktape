//! Pure Pages menu edits. The binding proposes these edits; Commit owns history.
use crate::editor::{self, Doc, EditorCursor, EditorDecision, EditorHistoryEffect, EditorPosition};
use std::ops::Range;

/// The existing Pages block palette, shared by painting and its edit reducer.
pub const TURNS: &[(&str, &str, &str)] = &[
    ("text", "Text", ""),
    ("h1", "Heading 1", "# "),
    ("h2", "Heading 2", "## "),
    ("h3", "Heading 3", "### "),
    ("todo", "To-do list", "- [ ] "),
    ("bullet", "Bulleted list", "- "),
    ("number", "Numbered list", "1. "),
    ("toggle", "Toggle list", "+ "),
    ("quote", "Quote", "> "),
    ("callout", "Callout", "!> "),
    ("code", "Code", "```"),
    ("divider", "Divider", "---"),
];

fn finish(before: &Doc, after: Doc) -> EditorDecision {
    if before == &after {
        EditorDecision::Noop
    } else {
        editor::restore(before, &after, EditorHistoryEffect::NewGroup)
    }
}

fn replace(document: &Doc, range: Range<usize>, replacement: &str) -> Doc {
    let mut next = document.clone();
    next.text.replace_range(range.clone(), replacement);
    next.cursor = EditorCursor {
        position: next.position_at(range.start + replacement.len()),
        selection: None,
    };
    next
}

/// Rewrite one line's block marker while retaining its content and indentation.
pub fn turn(document: &Doc, line: usize, tag: &str) -> EditorDecision {
    finish(document, turned(document, line, tag))
}

fn turned(document: &Doc, line: usize, tag: &str) -> Doc {
    let Some(text) = document.line(line) else {
        return document.clone();
    };
    let trimmed = text.trim_start_matches([' ', '\t']);
    if trimmed.starts_with("```") {
        return document.clone();
    }
    let indent = &text[..text.len() - trimmed.len()];
    let content = strip_marker(trimmed);
    let Some((_, _, marker)) = TURNS.iter().find(|(name, ..)| *name == tag) else {
        return document.clone();
    };
    let (replacement, caret) = match *marker {
        "```" => (
            if content.is_empty() {
                format!("{indent}```\n{indent}```")
            } else {
                format!("{indent}```\n{indent}{content}\n{indent}```")
            },
            Some(EditorCursor::at(line + 1, indent.len() + content.len())),
        ),
        "---" => (
            if content.is_empty() {
                format!("{indent}---")
            } else {
                format!("{indent}---\n{indent}{content}")
            },
            None,
        ),
        _ => (format!("{indent}{marker}{content}"), None),
    };
    let start = document.offset(EditorPosition::new(line, 0));
    let mut next = replace(document, start..start + text.len(), &replacement);
    if let Some(cursor) = caret {
        next.cursor = cursor;
    }
    next
}

pub fn turn_from_slash(
    document: &Doc,
    line: usize,
    start: usize,
    slashed: bool,
    tag: &str,
) -> EditorDecision {
    let Some(row) = document.line(line) else {
        return EditorDecision::Noop;
    };
    let end = document.cursor.position.column as usize;
    if document.cursor.position.line as usize != line
        || row.get(start..end).is_none()
        || (slashed && !row[start..end].starts_with('/'))
        || !TURNS.iter().any(|(name, ..)| *name == tag)
    {
        return EditorDecision::Noop;
    }
    let offset = document.offset(EditorPosition::new(line, 0));
    // An empty palette filter is an empty replacement, never Backspace: the
    // preceding block must survive a pick from the gutter's fresh blank line.
    let stripped = replace(document, offset + start..offset + end, "");
    finish(document, turned(&stripped, line, tag))
}

fn strip_marker(text: &str) -> &str {
    const MARKERS: &[&str] = &[
        "### ", "## ", "# ", "- [x] ", "- [X] ", "- [ ] ", "!> ", "> ", "- ", "+ ", "* ",
    ];
    if let Some(rest) = MARKERS.iter().find_map(|marker| text.strip_prefix(marker)) {
        return rest;
    }
    if text == "---" {
        return "";
    }
    let digits = text.bytes().take_while(u8::is_ascii_digit).count();
    if (1..=2).contains(&digits) {
        let rest = &text[digits..];
        if let Some(rest) = rest
            .strip_prefix('.')
            .or_else(|| rest.strip_prefix(')'))
            .and_then(|rest| rest.strip_prefix(' '))
        {
            return rest;
        }
    }
    text
}

pub fn toggle_todo(document: &Doc, line: usize) -> EditorDecision {
    let Some(text) = document.line(line) else {
        return EditorDecision::Noop;
    };
    let trimmed = text.trim_start_matches([' ', '\t']);
    if !["- [ ] ", "- [x] ", "- [X] "]
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
    {
        return EditorDecision::Noop;
    }
    let tick = text.len() - trimmed.len() + 3;
    let replacement = if text[tick..].starts_with(['x', 'X']) {
        " "
    } else {
        "x"
    };
    let start = document.offset(EditorPosition::new(line, tick));
    finish(document, replace(document, start..start + 1, replacement))
}

fn spans(document: &Doc) -> Vec<Range<usize>> {
    spans_text(&document.text)
}
fn spans_text(text: &str) -> Vec<Range<usize>> {
    let lines: Vec<_> = text.split('\n').collect();
    let mut result = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let end = if lines[index]
            .trim_start_matches([' ', '\t'])
            .starts_with("```")
        {
            (index + 1..lines.len())
                .find(|&line| {
                    lines[line]
                        .trim_start_matches([' ', '\t'])
                        .starts_with("```")
                })
                .map_or(lines.len(), |close| close + 1)
        } else {
            index + 1
        };
        result.push(index..end);
        index = end;
    }
    result
}
fn span(document: &Doc, line: usize) -> Option<Range<usize>> {
    spans(document)
        .into_iter()
        .find(|span| span.contains(&line))
}
fn rebuilt(lines: &[&str], caret: usize) -> Doc {
    Doc::new(
        lines.join("\n"),
        EditorCursor::at(caret.min(lines.len().saturating_sub(1)), 0),
    )
}

pub fn insert_below(document: &Doc, line: usize) -> EditorDecision {
    let Some(span) = span(document, line) else {
        return EditorDecision::Noop;
    };
    let last = span.end - 1;
    let column = document.line(last).map_or(0, str::len);
    let offset = document.offset(EditorPosition::new(last, column));
    finish(document, replace(document, offset..offset, "\n"))
}

pub fn delete(document: &Doc, line: usize) -> EditorDecision {
    let Some(span) = span(document, line).filter(|span| !span.contains(&0)) else {
        return EditorDecision::Noop;
    };
    let mut lines = document.lines();
    lines.drain(span.clone());
    finish(document, rebuilt(&lines, span.start))
}

pub fn duplicate(document: &Doc, line: usize) -> EditorDecision {
    let Some(span) = span(document, line).filter(|span| !span.contains(&0)) else {
        return EditorDecision::Noop;
    };
    let mut lines = document.lines();
    let copied = lines[span.clone()].to_vec();
    lines.splice(span.end..span.end, copied);
    finish(document, rebuilt(&lines, span.end))
}

pub fn move_block(document: &Doc, line: usize, direction: i32) -> EditorDecision {
    if ![-1, 1].contains(&direction) {
        return EditorDecision::Noop;
    }
    let spans = spans(document);
    let Some(index) = spans.iter().position(|span| span.contains(&line)) else {
        return EditorDecision::Noop;
    };
    let Some(neighbor) = index
        .checked_add_signed(direction as isize)
        .filter(|&i| i < spans.len())
    else {
        return EditorDecision::Noop;
    };
    if spans[index].contains(&0) || spans[neighbor].contains(&0) {
        return EditorDecision::Noop;
    }
    let (first, second) = if direction < 0 {
        (&spans[neighbor], &spans[index])
    } else {
        (&spans[index], &spans[neighbor])
    };
    let lines = document.lines();
    let mut next = lines[..first.start].to_vec();
    let landing = if direction < 0 {
        first.start
    } else {
        first.start + second.len()
    };
    next.extend_from_slice(&lines[second.clone()]);
    next.extend_from_slice(&lines[first.clone()]);
    next.extend_from_slice(&lines[second.end..]);
    finish(document, rebuilt(&next, landing))
}

pub fn drop_boundaries(document: &Doc) -> Vec<usize> {
    drop_boundaries_text(&document.text)
}
pub fn drop_boundaries_text(text: &str) -> Vec<usize> {
    let mut boundaries: Vec<_> = spans_text(text)
        .into_iter()
        .map(|span| span.start)
        .filter(|&line| line > 0)
        .collect();
    boundaries.push(text.split('\n').count());
    boundaries
}

pub fn drop_move(document: &Doc, line: usize, boundary: usize) -> EditorDecision {
    let Some(span) = span(document, line).filter(|span| !span.contains(&0)) else {
        return EditorDecision::Noop;
    };
    if (span.start..=span.end).contains(&boundary) || !drop_boundaries(document).contains(&boundary)
    {
        return EditorDecision::Noop;
    }
    let mut lines = document.lines();
    let block = lines[span.clone()].to_vec();
    lines.drain(span.clone());
    let landing = if boundary > span.end {
        boundary - span.len()
    } else {
        boundary
    };
    lines.splice(landing..landing, block);
    finish(document, rebuilt(&lines, landing))
}

/// Menu state belongs to the guest snapshot. Proposed edits return a successor;
/// callers adopt it only after Commit (or the read-only interaction callback).
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Menu {
    open: Option<Open>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Open {
    kind: Kind,
    selected: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Kind {
    Slash {
        line: usize,
        strip: usize,
        slashed: bool,
    },
    Block {
        line: usize,
    },
    Turn {
        line: usize,
    },
}

pub struct MenuView {
    /// None anchors at the caret; Some anchors at the specified document line.
    pub line: Option<usize>,
    pub items: Vec<(&'static str, &'static str)>,
    pub selected: usize,
}

const BLOCK_ITEMS: &[(&str, &str)] = &[
    ("turn", "Turn into…"),
    ("duplicate", "Duplicate"),
    ("move-up", "Move up"),
    ("move-down", "Move down"),
    ("delete", "Delete"),
];

impl Menu {
    fn opened(kind: Kind) -> Self {
        Self {
            open: Some(Open { kind, selected: 0 }),
        }
    }

    pub fn close(&mut self) {
        self.open = None;
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    pub fn current(&self, document: &Doc) -> Option<MenuView> {
        let open = self.open.as_ref()?;
        let (line, items) = match open.kind {
            Kind::Slash {
                line,
                strip,
                slashed,
            } => (
                None,
                turn_items(slash_filter(document, line, strip, slashed)?),
            ),
            Kind::Block { line } => (Some(line), block_items(document, line)?),
            Kind::Turn { line } => (Some(line), turn_items("")),
        };
        if items.is_empty() {
            return None;
        }
        let selected = open.selected.min(items.len() - 1);
        Some(MenuView {
            line,
            items,
            selected,
        })
    }

    pub fn block(&mut self, document: &Doc, line: usize) {
        if document.line(line).is_some() && line > 0 {
            *self = Self::opened(Kind::Block { line });
        }
    }

    pub fn select(&mut self, document: &Doc, selected: usize) {
        if let Some(view) = self.current(document)
            && let Some(open) = self.open.as_mut()
        {
            open.selected = selected.min(view.items.len() - 1);
        }
    }

    /// Called only for an accepted text edit. Caret/navigation and source
    /// replacement call close instead; a paste containing '/' does not open.
    pub fn after_edit(&mut self, document: &Doc, inserted_slash: bool) {
        let kind = self.open.as_ref().map(|open| open.kind);
        match kind {
            None => self.after_closed_edit(document, inserted_slash),
            Some(Kind::Slash { .. }) => self.retain_filter(document),
            Some(Kind::Block { .. } | Kind::Turn { .. }) => self.close(),
        }
    }

    fn after_closed_edit(&mut self, document: &Doc, inserted_slash: bool) {
        if inserted_slash && let Some(strip) = document.cursor.position.column.checked_sub(1) {
            let line = document.cursor.position.line as usize;
            let strip = strip as usize;
            if slash_filter(document, line, strip, true).is_some() {
                *self = Self::opened(Kind::Slash {
                    line,
                    strip,
                    slashed: true,
                });
            }
        }
    }

    fn retain_filter(&mut self, document: &Doc) {
        match self.current(document) {
            Some(view) => self.select(document, view.selected),
            None => self.close(),
        }
    }

    pub fn plus(&self, document: &Doc, line: usize) -> (EditorDecision, Self) {
        let decision = insert_below(document, line);
        let next = match &decision {
            EditorDecision::Apply { cursor, .. } => Self::opened(Kind::Slash {
                line: cursor.position.line as usize,
                strip: cursor.position.column as usize,
                slashed: false,
            }),
            _ => self.clone(),
        };
        (decision, next)
    }

    pub fn pick(&self, document: &Doc, tag: &str) -> (EditorDecision, Self) {
        let offered = self
            .current(document)
            .is_some_and(|view| view.items.iter().any(|item| item.0 == tag));
        if !offered {
            return (EditorDecision::Noop, self.clone());
        }
        let Some(open) = self.open.as_ref() else {
            return (EditorDecision::Noop, self.clone());
        };
        match open.kind {
            Kind::Slash {
                line,
                strip,
                slashed,
            } => (
                turn_from_slash(document, line, strip, slashed, tag),
                Self::default(),
            ),
            Kind::Turn { line } => (turn(document, line, tag), Self::default()),
            Kind::Block { line } => self.pick_block(document, line, tag),
        }
    }

    fn pick_block(&self, document: &Doc, line: usize, tag: &str) -> (EditorDecision, Self) {
        if tag == "turn" {
            return (EditorDecision::Noop, Self::opened(Kind::Turn { line }));
        }
        let decision = match tag {
            "duplicate" => duplicate(document, line),
            "move-up" => move_block(document, line, -1),
            "move-down" => move_block(document, line, 1),
            "delete" => delete(document, line),
            _ => return (EditorDecision::Noop, self.clone()),
        };
        (decision, Self::default())
    }
}

fn slash_filter(document: &Doc, line: usize, strip: usize, slashed: bool) -> Option<&str> {
    let cursor = document.cursor.position;
    if cursor.line as usize != line {
        return None;
    }
    let text = document.line(line)?;
    if slashed && !text.get(strip..)?.starts_with('/') {
        return None;
    }
    text.get(strip.checked_add(usize::from(slashed))?..cursor.column as usize)
}

fn turn_items(filter: &str) -> Vec<(&'static str, &'static str)> {
    let filter = filter.to_ascii_lowercase();
    TURNS
        .iter()
        .filter(|(tag, label, _)| {
            label.to_ascii_lowercase().contains(&filter) || tag.contains(&filter)
        })
        .map(|(tag, label, _)| (*tag, *label))
        .collect()
}

fn block_items(document: &Doc, line: usize) -> Option<Vec<(&'static str, &'static str)>> {
    let on_fence = document
        .line(line)?
        .trim_start_matches([' ', '\t'])
        .starts_with("```");
    Some(
        BLOCK_ITEMS
            .iter()
            .copied()
            .filter(|(tag, _)| !(on_fence && *tag == "turn"))
            .collect(),
    )
}
