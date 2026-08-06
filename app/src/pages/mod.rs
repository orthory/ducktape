//! The page document surface — ONE editor over the whole page.
//!
//! The block scaffolding this replaces (click a line to select it, a per-kind
//! editor swapped in behind a button, a `+`/`⋮⋮` gutter cluster, an insert row
//! with a block-type dropdown parked at the right margin, a `/` menu) is gone.
//! A page is text. The caret goes where you click because there is nothing to
//! select first; a line becomes a heading because you typed `# `, which is the
//! same gesture the block-type menu existed to perform.
//!
//! The three layers, and where each one lives:
//!   * [`markdown`] paints — syntax carries the formatting, and hides itself
//!     off the caret's line.
//!   * this file edits — the list/indent/fence keys that make markdown feel
//!     like a document instead of a text area.
//!   * [`sync`] persists — the buffer resolved back into the module's own ops.
//!
//! NOTHING HERE TALKS TO THE NODE. Every key is a pure buffer edit, and the
//! dirty-gated save tick reconciles the whole document afterwards. That is what
//! keeps the surface responsive and the write path in one auditable place.

pub mod markdown;
pub mod sync;

/// The `sync` predicate at the extern boundary, which hands values, not
/// borrows.
pub fn has_unclosed_fence(text: String) -> bool {
    sync::has_unclosed_fence(&text)
}

use iced::advanced::text::Wrapping;
use iced::font::{Style as FontStyle, Weight};
use iced::widget::text_editor::{self, Action, Content, Cursor, Edit, Position};
use iced::{Border, Color, Element, Padding};
use std::hash::{Hash as _, Hasher as _};
use ui_lang_runtime::rich_text_editor::{ContentVersion, RichTextEditor};

pub use ui_lang_runtime::rich_text_editor::Action as PageAction;

use markdown::{BODY_LINE_HEIGHT, BODY_SIZE, Caret, DocumentHighlighter};

/// The document's left gutter. Wide enough that a hidden `### ` marker leaves
/// the heading on the same left edge as the paragraph under it.
const DOCUMENT_PAD_X: f32 = 2.0;

/// The page's writing surface.
pub fn page_document(document: &Content, dark: bool, disabled: bool) -> Element<'_, PageAction> {
    let cursor = document.cursor().position;
    let editor = RichTextEditor::new(document, content_version(document))
        .id("page-document")
        .placeholder("Write something… `#` for a heading, `-` for a list")
        .width(iced::Length::Fill)
        .font(crate::Ducktape::default_font())
        .size(BODY_SIZE)
        .line_height(BODY_LINE_HEIGHT)
        .wrapping(Wrapping::Word)
        .padding(Padding::from([0.0, DOCUMENT_PAD_X]))
        .highlight_with::<DocumentHighlighter>(
            Caret {
                line: cursor.line,
                column: cursor.column,
                dark,
            },
            u64::from(dark),
            move |mark| markdown::format(mark, dark),
        )
        .style(move |_theme, status| document_style(dark, status));
    if disabled {
        return editor.into();
    }
    editor.on_action(|action| action).into()
}

fn document_style(dark: bool, status: text_editor::Status) -> text_editor::Style {
    let (value, muted, selection) = match dark {
        true => (
            Color::from_rgb8(0xd4, 0xd2, 0xca),
            Color::from_rgb8(0x6b, 0x6a, 0x61),
            Color::from_rgb8(0x45, 0x44, 0x3c),
        ),
        false => (
            Color::from_rgb8(0x3a, 0x38, 0x33),
            Color::from_rgb8(0xb3, 0xb1, 0xa8),
            Color::from_rgb8(0xd4, 0xd2, 0xca),
        ),
    };
    text_editor::Style {
        background: Color::TRANSPARENT.into(),
        // The document IS the page — a focus ring around the whole body would
        // draw a box around the thing the window is already about.
        border: Border::default(),
        placeholder: muted,
        value: match status {
            text_editor::Status::Disabled => muted,
            _ => value,
        },
        selection,
    }
}

/// The Ice `editor` state is a bare `Content` with no revision counter, so the
/// text's hash is the change key — but hashed LINE BY LINE over the borrowed
/// rope. `document.text()` allocates the whole document into a fresh `String`,
/// and this runs on every view build; the chat composer can afford that
/// (`crate::editor::content_version`), a page-length buffer cannot.
// ponytail: still O(n) hashing per frame — an app-side revision counter plus
// `change_hint` is the upgrade if profiling ever names this.
fn content_version(document: &Content) -> ContentVersion {
    let mut hasher = std::hash::DefaultHasher::new();
    for index in 0..document.line_count() {
        if let Some(line) = document.line(index) {
            line.text.hash(&mut hasher);
            hasher.write_u8(b'\n');
        }
    }
    ContentVersion::new(0, hasher.finish())
}

/// Apply one interaction to the buffer.
///
/// The structural keys are intercepted BEFORE the native edit, because each is
/// a different edit than the one the key would otherwise make: Enter after
/// `- item` inserts a newline AND a fresh marker, Backspace at the start of
/// list content removes the marker rather than the newline above it.
pub fn apply_page_action(mut document: Content, action: PageAction) -> Content {
    let PageAction::Edit(edit_action) = action else {
        let PageAction::MoveTo(cursor) = action else {
            return document;
        };
        document.move_to(cursor);
        return document;
    };
    let Action::Edit(edit) = &edit_action else {
        document.perform(edit_action);
        return document;
    };
    let handled = match edit {
        Edit::Enter => close_fence(&mut document) || continue_list(&mut document),
        Edit::Backspace => remove_list_marker(&mut document),
        Edit::Indent => shift_indent(&mut document, 1),
        Edit::Unindent => shift_indent(&mut document, -1),
        _ => false,
    };
    if handled {
        return document;
    }
    document.perform(edit_action);
    document
}

/// The document's text, for the save tick's dirty check and its plan.
///
/// Owned, not borrowed: the `sync` extern boundary hands state fields by value.
/// The trailing newline goes — `Content` always carries one for the caret to
/// sit after, and it is not a line of the document.
pub fn page_text(document: Content) -> String {
    document.text().trim_end_matches(['\n', '\r']).to_string()
}

/// Enter at the end of an UNMATCHED ``` line closes the fence: the newline is
/// typed and a closing ``` appears below the caret, so typing continues INSIDE
/// the fence and the save tick never reads the rest of the page as code. The
/// reference editor auto-closes on the same gesture.
fn close_fence(document: &mut Content) -> bool {
    let cursor = document.cursor();
    if cursor.selection.is_some() {
        return false;
    }
    let Some(line) = document.line(cursor.position.line) else {
        return false;
    };
    let text = line.text.into_owned();
    let trimmed = text.trim_start_matches([' ', '\t']);
    let at_line_end = cursor.position.column >= text.len();
    if !trimmed.starts_with("```") || !at_line_end {
        return false;
    }
    if !sync::has_unclosed_fence(&document.text()) {
        return false;
    }
    let indent = &text[..text.len() - trimmed.len()];
    let opened = format!("\n\n{indent}```");
    replace_range(document, cursor.position, cursor.position, &opened);
    document.move_to(Cursor {
        position: Position {
            line: cursor.position.line + 1,
            column: indent.len(),
        },
        selection: None,
    });
    true
}

/// Enter on a list line carries the marker down; Enter on an EMPTY list item
/// ends the list instead of stacking another empty bullet.
fn continue_list(document: &mut Content) -> bool {
    let cursor = document.cursor();
    if cursor.selection.is_some() {
        return false;
    }
    let Some(line) = document.line(cursor.position.line) else {
        return false;
    };
    let text = line.text.into_owned();
    let Some(marker) = list_marker(&text) else {
        return false;
    };
    // Splitting mid-marker is not a list continuation, it is ordinary typing.
    if cursor.position.column < marker.content {
        return false;
    }
    let item_is_empty = text[marker.content..].trim().is_empty();
    if item_is_empty {
        replace_range(
            document,
            Position {
                line: cursor.position.line,
                column: 0,
            },
            Position {
                line: cursor.position.line,
                column: text.len(),
            },
            "",
        );
        return true;
    }
    let carried = format!("\n{}", marker.next_prefix(&text));
    replace_range(document, cursor.position, cursor.position, &carried);
    true
}

/// Backspace at the first character of a list item's CONTENT deletes the
/// marker, turning the item into a paragraph — the standard escape from a list
/// that does not also eat the line above.
fn remove_list_marker(document: &mut Content) -> bool {
    let cursor = document.cursor();
    if cursor.selection.is_some() {
        return false;
    }
    let Some(line) = document.line(cursor.position.line) else {
        return false;
    };
    let text = line.text.into_owned();
    let Some(marker) = list_marker(&text) else {
        return false;
    };
    if cursor.position.column != marker.content {
        return false;
    }
    replace_range(
        document,
        Position {
            line: cursor.position.line,
            column: marker.indent,
        },
        Position {
            line: cursor.position.line,
            column: marker.content,
        },
        "",
    );
    true
}

/// Tab / Shift+Tab move the caret's line by one nesting step. Two spaces is
/// the depth unit the block projection already speaks (`load::block_prefix`).
fn shift_indent(document: &mut Content, steps: i32) -> bool {
    let cursor = document.cursor();
    let Some(line) = document.line(cursor.position.line) else {
        return false;
    };
    let text = line.text.into_owned();
    let indent = text.len() - text.trim_start_matches([' ', '\t']).len();
    if steps > 0 {
        replace_range(
            document,
            Position {
                line: cursor.position.line,
                column: 0,
            },
            Position {
                line: cursor.position.line,
                column: 0,
            },
            "  ",
        );
        return true;
    }
    let removable = indent.min(2);
    if removable == 0 {
        return true;
    }
    replace_range(
        document,
        Position {
            line: cursor.position.line,
            column: 0,
        },
        Position {
            line: cursor.position.line,
            column: removable,
        },
        "",
    );
    true
}

/// A list line's shape: where its indent ends, where its content starts, and
/// the marker the NEXT item should wear.
struct ListMarker {
    indent: usize,
    content: usize,
    next: String,
}

impl ListMarker {
    fn next_prefix(&self, text: &str) -> String {
        format!("{}{}", &text[..self.indent], self.next)
    }
}

fn list_marker(text: &str) -> Option<ListMarker> {
    let trimmed = text.trim_start_matches([' ', '\t']);
    let indent = text.len() - trimmed.len();
    let bytes = trimmed.as_bytes();
    let (mut cursor, next) = match *bytes.first()? {
        bullet @ (b'-' | b'+' | b'*') => (1, format!("{} ", char::from(bullet))),
        byte if byte.is_ascii_digit() => {
            let digits = bytes
                .iter()
                .take_while(|byte| byte.is_ascii_digit())
                .count();
            let delimiter = *bytes.get(digits)?;
            if !matches!(delimiter, b'.' | b')') {
                return None;
            }
            let number: u64 = trimmed[..digits].parse().ok()?;
            (
                digits + 1,
                format!(
                    "{} ",
                    number.saturating_add(1).to_string() + &char::from(delimiter).to_string()
                ),
            )
        }
        _ => return None,
    };
    if bytes.get(cursor) != Some(&b' ') {
        return None;
    }
    cursor += 1;
    // A task marker carries down UNTICKED — the next thing you write is not
    // already done.
    let ticked = matches!(
        bytes.get(cursor..cursor + 4),
        Some(b"[ ] " | b"[x] " | b"[X] ")
    );
    if ticked {
        cursor += 4;
        return Some(ListMarker {
            indent,
            content: indent + cursor,
            next: format!("{next}[ ] "),
        });
    }
    Some(ListMarker {
        indent,
        content: indent + cursor,
        next,
    })
}

/// Select `start..end` and type over it. The widget has no range-edit action,
/// so a replacement is a selection plus one edit — the same route the reference
/// editor takes.
fn replace_range(document: &mut Content, start: Position, end: Position, replacement: &str) {
    document.move_to(Cursor {
        position: end,
        selection: (start != end).then_some(start),
    });
    if replacement.is_empty() {
        document.perform(Action::Edit(Edit::Backspace));
        return;
    }
    document.perform(Action::Edit(Edit::Paste(std::sync::Arc::new(
        replacement.to_string(),
    ))));
}

/// The composer font at a weight — the document shares the app's default face.
#[allow(dead_code)]
fn document_font(weight: Weight, style: FontStyle) -> iced::Font {
    iced::Font {
        weight,
        style,
        ..crate::Ducktape::default_font()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(text: &str, line: usize, column: usize) -> Content {
        let mut content = Content::with_text(text);
        content.move_to(Cursor {
            position: Position { line, column },
            selection: None,
        });
        content
    }

    /// The buffer's exact text after one key. NOT trimmed: a carried list
    /// marker ends in the space the caret sits after, and trimming it away
    /// would let a regression that drops it pass.
    fn press(content: Content, edit: Edit) -> String {
        apply_page_action(content, PageAction::Edit(Action::Edit(edit))).text()
    }

    #[test]
    fn enter_carries_a_bullet_down() {
        let content = typed("- one", 0, 5);
        assert_eq!(press(content, Edit::Enter), "- one\n- ");
    }

    #[test]
    fn enter_increments_an_ordered_marker() {
        let content = typed("3. three", 0, 8);
        assert_eq!(press(content, Edit::Enter), "3. three\n4. ");
    }

    #[test]
    fn enter_on_an_empty_item_ends_the_list() {
        let content = typed("- one\n- ", 1, 2);
        assert_eq!(press(content, Edit::Enter), "- one\n");
    }

    #[test]
    fn a_task_carries_down_unticked() {
        let content = typed("- [x] done", 0, 10);
        assert_eq!(press(content, Edit::Enter), "- [x] done\n- [ ] ");
    }

    #[test]
    fn enter_outside_a_list_is_an_ordinary_newline() {
        let content = typed("plain", 0, 5);
        assert_eq!(press(content, Edit::Enter), "plain\n");
    }

    #[test]
    fn enter_on_an_open_fence_closes_it_with_the_caret_inside() {
        // Line 0 is the title, so the fence sits on line 1.
        let content = typed("Title\n```", 1, 3);
        let mut after = apply_page_action(content, PageAction::Edit(Action::Edit(Edit::Enter)));
        assert_eq!(after.text(), "Title\n```\n\n```");
        // The caret parks on the blank line between the fences.
        assert_eq!(after.cursor().position.line, 2);
        after.perform(Action::Edit(Edit::Insert('x')));
        assert_eq!(after.text(), "Title\n```\nx\n```");
    }

    #[test]
    fn enter_on_a_closing_fence_is_an_ordinary_newline() {
        let content = typed("Title\n```\ncode\n```", 3, 3);
        assert_eq!(press(content, Edit::Enter), "Title\n```\ncode\n```\n");
    }

    #[test]
    fn a_nested_open_fence_closes_at_its_own_indent() {
        let content = typed("Title\n  ```", 1, 5);
        let after = apply_page_action(content, PageAction::Edit(Action::Edit(Edit::Enter)));
        assert_eq!(after.text(), "Title\n  ```\n\n  ```");
        assert_eq!(after.cursor().position.column, 2);
    }

    #[test]
    fn backspace_at_the_content_edge_drops_the_marker_not_the_line_above() {
        let content = typed("one\n- two", 1, 2);
        assert_eq!(press(content, Edit::Backspace), "one\ntwo");
    }

    #[test]
    fn backspace_inside_the_text_is_an_ordinary_delete() {
        let content = typed("- two", 0, 5);
        assert_eq!(press(content, Edit::Backspace), "- tw");
    }

    #[test]
    fn tab_nests_by_the_projection_s_own_two_space_step() {
        let content = typed("- one", 0, 5);
        assert_eq!(press(content, Edit::Indent), "  - one");
    }

    #[test]
    fn shift_tab_lifts_one_step_and_stops_at_the_left_margin() {
        let nested = typed("    - one", 0, 9);
        assert_eq!(press(nested, Edit::Unindent), "  - one");
        let flat = typed("- one", 0, 5);
        assert_eq!(press(flat, Edit::Unindent), "- one");
    }

    #[test]
    fn page_text_drops_the_buffer_s_trailing_newline() {
        let content = Content::with_text("one\ntwo\n");
        assert_eq!(page_text(content), "one\ntwo");
    }
}
