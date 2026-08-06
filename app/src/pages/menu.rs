//! The document's anchored menus: "/" opens the block palette at the caret,
//! the gutter "+" inserts a block below and opens the same palette, and the
//! dots handle opens the block menu (Turn into / Duplicate / Move / Delete).
//!
//! Which menu is up is EPHEMERAL VIEW STATE, held thread-local beside the
//! undo history rather than threaded through Ice state: `page_document`
//! reads it at view time, the edit route mutates it, and every path runs on
//! the one UI thread.

use super::history;
use iced::widget::text_editor::{Action, Content, Cursor, Edit, Position};
use std::cell::RefCell;
use ui_lang_runtime::rich_text_editor::{
    EditorMenu, GutterButton, MenuAnchor, MenuEvent, MenuItem,
};

use super::PageAction;

/// Every block shape a line can turn into: `(tag, label, marker)`. The
/// markers are the ones `sync::parse_line` reads back, so a pick round-trips
/// through the save plan as that exact kind.
const TURNS: &[(&str, &str, &str)] = &[
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

const BLOCK_ITEMS: &[(&str, &str)] = &[
    ("turn", "Turn into…"),
    ("duplicate", "Duplicate"),
    ("move-up", "Move up"),
    ("move-down", "Move down"),
    ("delete", "Delete"),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// The palette at the caret. `strip` is the column where the typed span
    /// starts on `line`; `slashed` when a literal "/" heads it (typed), not
    /// when the gutter "+" opened it.
    Slash {
        line: usize,
        strip: usize,
        slashed: bool,
    },
    /// The dots-handle menu for the block at `line`.
    Block { line: usize },
    /// The "Turn into…" follow-up for the block at `line`.
    Turn { line: usize },
}

struct Open {
    kind: Kind,
    selected: usize,
}

thread_local! {
    static OPEN: RefCell<Option<Open>> = const { RefCell::new(None) };
}

fn open(kind: Kind) {
    OPEN.with_borrow_mut(|state| *state = Some(Open { kind, selected: 0 }));
}

/// Close whichever menu is up — also the page-switch hook.
pub fn close() {
    OPEN.with_borrow_mut(|state| *state = None);
}

/// The menu the widget should show this frame, if one is up and still valid
/// against the buffer.
pub fn current(document: &Content) -> Option<EditorMenu> {
    let (kind, selected) =
        OPEN.with_borrow(|state| state.as_ref().map(|open| (open.kind, open.selected)))?;
    let (anchor, items) = match kind {
        Kind::Slash {
            line,
            strip,
            slashed,
        } => {
            let filter = slash_filter(document, line, strip, slashed)?;
            (MenuAnchor::Caret, turn_items(&filter))
        }
        Kind::Block { line } => (MenuAnchor::Line(line), block_items(document, line)),
        Kind::Turn { line } => (MenuAnchor::Line(line), turn_items("")),
    };
    if items.is_empty() {
        return None;
    }
    Some(EditorMenu {
        anchor,
        items,
        selected,
    })
}

/// The typed filter behind the slash span, or `None` once the caret left it —
/// the openness test the lifecycle and the view share.
fn slash_filter(document: &Content, line: usize, strip: usize, slashed: bool) -> Option<String> {
    let cursor = document.cursor().position;
    if cursor.line != line {
        return None;
    }
    let text = document.line(line)?.text.into_owned();
    let span_start = strip + usize::from(slashed);
    if cursor.column < span_start {
        return None;
    }
    if slashed && !text.get(strip..)?.starts_with('/') {
        return None;
    }
    Some(text.get(span_start..cursor.column)?.to_string())
}

fn turn_items(filter: &str) -> Vec<MenuItem> {
    let filter = filter.to_ascii_lowercase();
    TURNS
        .iter()
        .filter(|(tag, label, _)| {
            filter.is_empty()
                || label.to_ascii_lowercase().contains(&filter)
                || tag.contains(&filter)
        })
        .map(|(tag, label, _)| MenuItem {
            tag: (*tag).to_string(),
            label: (*label).to_string(),
        })
        .collect()
}

fn block_items(document: &Content, line: usize) -> Vec<MenuItem> {
    // A fence line has no marker to swap — turning it would unbalance the
    // fence pair, so the handle menu just drops the entry there.
    let on_fence = document
        .line(line)
        .is_some_and(|row| row.text.trim_start_matches([' ', '\t']).starts_with("```"));
    BLOCK_ITEMS
        .iter()
        .filter(|(tag, _)| !(on_fence && *tag == "turn"))
        .map(|(tag, label)| MenuItem {
            tag: (*tag).to_string(),
            label: (*label).to_string(),
        })
        .collect()
}

/// Route one widget menu event.
pub fn apply(document: Content, event: MenuEvent) -> Content {
    match event {
        MenuEvent::Select(index) => {
            OPEN.with_borrow_mut(|state| {
                if let Some(open) = state.as_mut() {
                    open.selected = index;
                }
            });
            document
        }
        MenuEvent::Dismiss => {
            close();
            document
        }
        MenuEvent::Pick(tag) => pick(document, &tag),
    }
}

fn pick(document: Content, tag: &str) -> Content {
    let Some(picked) = OPEN.with_borrow_mut(|state| state.take()) else {
        return document;
    };
    match picked.kind {
        Kind::Slash {
            line,
            strip,
            slashed,
        } => turn_from_slash(document, line, strip, slashed, tag),
        Kind::Turn { line } => {
            history::record(|| (document.text(), document.cursor()));
            let mut document = document;
            apply_turn(&mut document, line, tag);
            document
        }
        Kind::Block { line } => match tag {
            "turn" => {
                open(Kind::Turn { line });
                document
            }
            "duplicate" => duplicate_block(document, line),
            "move-up" => move_block(document, line, -1),
            "move-down" => move_block(document, line, 1),
            "delete" => delete_block(document, line),
            _ => document,
        },
    }
}

/// A pick from the caret palette: the typed span (slash and filter) comes out
/// of the line first, then the line turns.
fn turn_from_slash(
    mut document: Content,
    line: usize,
    strip: usize,
    slashed: bool,
    tag: &str,
) -> Content {
    if slash_filter(&document, line, strip, slashed).is_none() {
        return document;
    }
    history::record(|| (document.text(), document.cursor()));
    let column = document.cursor().position.column;
    super::replace_range(
        &mut document,
        Position {
            line,
            column: strip,
        },
        Position { line, column },
        "",
    );
    apply_turn(&mut document, line, tag);
    document
}

/// Rewrite `line` as the picked block shape, keeping its indent and content.
fn apply_turn(document: &mut Content, line: usize, tag: &str) {
    let Some(row) = document.line(line) else {
        return;
    };
    let text = row.text.into_owned();
    let trimmed = text.trim_start_matches([' ', '\t']);
    let indent = &text[..text.len() - trimmed.len()];
    if trimmed.starts_with("```") {
        return;
    }
    let content = strip_marker(trimmed);
    let Some(marker) = TURNS
        .iter()
        .find(|(turn_tag, ..)| *turn_tag == tag)
        .map(|(.., marker)| *marker)
    else {
        return;
    };
    let (replacement, caret) = match marker {
        // The fence pair wraps the content; the caret lands inside.
        "```" => (
            match content.is_empty() {
                true => format!("{indent}```\n{indent}```"),
                false => format!("{indent}```\n{indent}{content}\n{indent}```"),
            },
            Some(Position {
                line: line + 1,
                column: indent.len() + content.len(),
            }),
        ),
        // A divider is content-free — anything on the line survives below it.
        "---" => match content.is_empty() {
            true => (format!("{indent}---"), None),
            false => (format!("{indent}---\n{indent}{content}"), None),
        },
        _ => (format!("{indent}{marker}{content}"), None),
    };
    let end = text.len();
    super::replace_range(
        document,
        Position { line, column: 0 },
        Position { line, column: end },
        &replacement,
    );
    if let Some(position) = caret {
        document.move_to(Cursor {
            position,
            selection: None,
        });
    }
}

/// `trimmed` minus any leading block marker the grammar knows.
fn strip_marker(trimmed: &str) -> &str {
    const MARKERS: &[&str] = &[
        "### ", "## ", "# ", "- [x] ", "- [X] ", "- [ ] ", "!> ", "> ", "- ", "+ ", "* ",
    ];
    if let Some(rest) = MARKERS
        .iter()
        .find_map(|marker| trimmed.strip_prefix(marker))
    {
        return rest;
    }
    if trimmed == "---" {
        return "";
    }
    // An ordered marker: up to two digits, a `.` or `)`, a space.
    let digits = trimmed
        .bytes()
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    if (1..=2).contains(&digits) {
        let rest = &trimmed[digits..];
        if let Some(rest) = rest
            .strip_prefix('.')
            .or_else(|| rest.strip_prefix(')'))
            .and_then(|rest| rest.strip_prefix(' '))
        {
            return rest;
        }
    }
    trimmed
}

/// The gutter routes: "+" inserts an empty block below and opens the palette
/// on it; the handle opens the block menu.
pub fn gutter(document: Content, line: usize, button: GutterButton) -> Content {
    match button {
        GutterButton::Handle => {
            open(Kind::Block { line });
            document
        }
        GutterButton::Plus => plus_below(document, line),
    }
}

fn plus_below(mut document: Content, line: usize) -> Content {
    let Some(span) = span_of(&document, line) else {
        return document;
    };
    history::record(|| (document.text(), document.cursor()));
    let end = span.end - 1;
    let column = document.line(end).map_or(0, |row| row.text.len());
    super::replace_range(
        &mut document,
        Position { line: end, column },
        Position { line: end, column },
        "\n",
    );
    document.move_to(Cursor {
        position: Position {
            line: end + 1,
            column: 0,
        },
        selection: None,
    });
    open(Kind::Slash {
        line: end + 1,
        strip: 0,
        slashed: false,
    });
    document
}

/// Menu lifecycle around an ordinary editor action, AFTER it hit the buffer.
/// Navigation and picks never reach here — the widget owns those keys while
/// a menu is up — so an action is either typing (which filters or invalidates
/// the slash palette and closes the block menus) or a caret move (closes).
pub fn after_action(document: &Content, action: &PageAction) {
    let PageAction::Edit(inner) = action else {
        close();
        return;
    };
    let Action::Edit(edit) = inner else {
        close();
        return;
    };
    let kind = OPEN.with_borrow(|state| state.as_ref().map(|open| open.kind));
    match kind {
        None => {
            if matches!(edit, Edit::Insert('/')) {
                let cursor = document.cursor().position;
                let Some(strip) = cursor.column.checked_sub(1) else {
                    return;
                };
                open(Kind::Slash {
                    line: cursor.line,
                    strip,
                    slashed: true,
                });
            }
        }
        Some(Kind::Slash {
            line,
            strip,
            slashed,
        }) => {
            let alive = slash_filter(document, line, strip, slashed)
                .is_some_and(|filter| !turn_items(&filter).is_empty());
            if !alive {
                close();
            }
        }
        Some(Kind::Block { .. } | Kind::Turn { .. }) => close(),
    }
}

/// The line span (fence-aware) of the block containing `line` — start..end in
/// document lines. A code block spans opening fence, body, closing fence.
fn span_of(document: &Content, line: usize) -> Option<std::ops::Range<usize>> {
    spans_of(document)
        .into_iter()
        .find(|span| span.contains(&line))
}

fn spans_of(document: &Content) -> Vec<std::ops::Range<usize>> {
    let count = document.line_count();
    let mut spans = Vec::new();
    let mut index = 0;
    while index < count {
        let text = document
            .line(index)
            .map(|row| row.text.into_owned())
            .unwrap_or_default();
        let fenced = text.trim_start_matches([' ', '\t']).starts_with("```");
        if !fenced {
            spans.push(index..index + 1);
            index += 1;
            continue;
        }
        let close = (index + 1..count).find(|&body| {
            document
                .line(body)
                .is_some_and(|row| row.text.trim_start_matches([' ', '\t']).starts_with("```"))
        });
        let end = close.map_or(count, |close| close + 1);
        spans.push(index..end);
        index = end;
    }
    spans
}

fn document_lines(document: &Content) -> Vec<String> {
    document.text().split('\n').map(str::to_string).collect()
}

fn rebuilt(lines: Vec<String>, caret_line: usize) -> Content {
    let mut document = Content::with_text(&lines.join("\n"));
    let line = caret_line.min(lines.len().saturating_sub(1));
    document.move_to(Cursor {
        position: Position { line, column: 0 },
        selection: None,
    });
    document
}

fn delete_block(document: Content, line: usize) -> Content {
    let Some(span) = span_of(&document, line) else {
        return document;
    };
    history::record(|| (document.text(), document.cursor()));
    let mut lines = document_lines(&document);
    lines.drain(span.clone());
    if lines.is_empty() {
        lines.push(String::new());
    }
    rebuilt(lines, span.start)
}

fn duplicate_block(document: Content, line: usize) -> Content {
    let Some(span) = span_of(&document, line) else {
        return document;
    };
    history::record(|| (document.text(), document.cursor()));
    let mut lines = document_lines(&document);
    let copy: Vec<String> = lines[span.clone()].to_vec();
    lines.splice(span.end..span.end, copy);
    rebuilt(lines, span.end)
}

fn move_block(document: Content, line: usize, direction: i32) -> Content {
    let spans = spans_of(&document);
    let Some(index) = spans.iter().position(|span| span.contains(&line)) else {
        return document;
    };
    let Some(neighbor) = index.checked_add_signed(direction as isize) else {
        return document;
    };
    // The title line never moves and nothing moves above it.
    if neighbor >= spans.len() || spans[neighbor].contains(&0) || spans[index].contains(&0) {
        return document;
    }
    history::record(|| (document.text(), document.cursor()));
    let lines = document_lines(&document);
    let (first, second) = match direction < 0 {
        true => (&spans[neighbor], &spans[index]),
        false => (&spans[index], &spans[neighbor]),
    };
    let mut moved: Vec<String> = lines[..first.start].to_vec();
    let landing = match direction < 0 {
        true => moved.len(),
        false => first.start + second.len(),
    };
    moved.extend_from_slice(&lines[second.clone()]);
    moved.extend_from_slice(&lines[first.clone()]);
    moved.extend_from_slice(&lines[second.end..]);
    rebuilt(moved, landing)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(text: &str, line: usize, column: usize) -> Content {
        let mut content = Content::with_text(text);
        content.move_to(Cursor {
            position: Position { line, column },
            selection: None,
        });
        content
    }

    fn insert(document: Content, character: char) -> Content {
        let action = PageAction::Edit(Action::Edit(Edit::Insert(character)));
        let document = super::super::apply_page_action(document, action.clone());
        after_action(&document, &action);
        document
    }

    #[test]
    fn typing_a_slash_opens_the_palette_and_typing_filters_it() {
        close();
        let document = insert(doc("Title\n", 1, 0), '/');
        let menu = current(&document).expect("open");
        assert!(matches!(menu.anchor, MenuAnchor::Caret));
        assert_eq!(menu.items.len(), TURNS.len());
        let document = insert(document, 'h');
        let menu = current(&document).expect("filtered");
        assert!(menu.items.iter().all(|item| item.label.contains("eading")));
        // A filter no block matches closes the palette.
        let document = insert(document, 'q');
        assert!(current(&document).is_none());
        assert!(OPEN.with_borrow(|state| state.is_none()));
        let _ = document;
    }

    #[test]
    fn a_pick_strips_the_typed_span_and_turns_the_line() {
        close();
        let document = insert(insert(doc("Title\n", 1, 0), '/'), 'h');
        assert_eq!(document.text(), "Title\n/h");
        let turned = apply(document, MenuEvent::Pick("h1".into()));
        assert_eq!(turned.text(), "Title\n# ");
        assert_eq!(turned.cursor().position, Position { line: 1, column: 2 });
        assert!(current(&turned).is_none());
    }

    #[test]
    fn turning_into_code_wraps_the_content_with_the_caret_inside() {
        close();
        open(Kind::Turn { line: 1 });
        let turned = apply(doc("Title\n- item", 1, 6), MenuEvent::Pick("code".into()));
        assert_eq!(turned.text(), "Title\n```\nitem\n```");
        assert_eq!(turned.cursor().position, Position { line: 2, column: 4 });
    }

    #[test]
    fn the_plus_inserts_below_the_block_and_opens_the_palette() {
        close();
        let document = gutter(doc("Title\n```\ncode\n```", 1, 0), 2, GutterButton::Plus);
        assert_eq!(document.text(), "Title\n```\ncode\n```\n");
        assert_eq!(document.cursor().position, Position { line: 4, column: 0 });
        let menu = current(&document).expect("palette open");
        assert_eq!(menu.items.len(), TURNS.len());
        close();
    }

    #[test]
    fn the_handle_menu_deletes_duplicates_and_moves_whole_blocks() {
        close();
        let document = gutter(doc("Title\none\ntwo", 1, 0), 1, GutterButton::Handle);
        let menu = current(&document).expect("block menu");
        assert!(matches!(menu.anchor, MenuAnchor::Line(1)));
        let deleted = apply(document, MenuEvent::Pick("delete".into()));
        assert_eq!(deleted.text(), "Title\ntwo");

        open(Kind::Block { line: 1 });
        let doubled = apply(
            doc("Title\none\ntwo", 1, 0),
            MenuEvent::Pick("duplicate".into()),
        );
        assert_eq!(doubled.text(), "Title\none\none\ntwo");

        // A fence block moves as one unit, and nothing moves above the title.
        open(Kind::Block { line: 1 });
        let swapped = apply(
            doc("Title\n```\ncode\n```\npara", 1, 0),
            MenuEvent::Pick("move-down".into()),
        );
        assert_eq!(swapped.text(), "Title\npara\n```\ncode\n```");
        open(Kind::Block { line: 1 });
        let held = apply(doc("Title\nonly", 1, 0), MenuEvent::Pick("move-up".into()));
        assert_eq!(held.text(), "Title\nonly");
    }

    #[test]
    fn a_caret_move_closes_whatever_menu_is_up() {
        close();
        let document = insert(doc("Title\n", 1, 0), '/');
        assert!(current(&document).is_some());
        let action = PageAction::MoveTo(Cursor {
            position: Position { line: 0, column: 0 },
            selection: None,
        });
        let document = super::super::apply_page_action(document, action.clone());
        after_action(&document, &action);
        assert!(current(&document).is_none());
    }

    #[test]
    fn a_fence_line_offers_no_turn_into() {
        close();
        let document = gutter(doc("Title\n```\ncode\n```", 1, 0), 1, GutterButton::Handle);
        let menu = current(&document).expect("block menu");
        assert!(menu.items.iter().all(|item| item.tag != "turn"));
        close();
        let _ = document;
    }
}
