//! The page document surface — ONE editor over the whole page.
//!
//! The block scaffolding this replaces (click a line to select it, a per-kind
//! editor swapped in behind a button, an insert row with a block-type dropdown
//! parked at the right margin) is gone. A page is text. The caret goes where
//! you click because there is nothing to select first; a line becomes a
//! heading because you typed `# ` — or because you picked it from the block
//! affordances that ride ON the text ([`menu`]): "/" opens the palette at the
//! caret, and the hovered line wears a "+"/handle gutter aligned to it.
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

pub mod history;
pub mod markdown;
pub mod menu;
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
pub fn commented_lines(blocks: Vec<crate::backend::PageBlock>, targets: Vec<String>) -> Vec<i64> {
    let mut lines = Vec::new();
    for (id, start, len) in sync::line_spans(&blocks) {
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
    blocks: Vec<crate::backend::PageBlock>,
    target: String,
    page_id: String,
) -> String {
    format!(
        "New comment on {}",
        comment_anchor_label(blocks, target, page_id)
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
#[derive(Clone, Debug, Hash, PartialEq)]
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

use iced::advanced::text::Wrapping;
use iced::font::{Style as FontStyle, Weight};
use iced::widget::text_editor::{self, Action, Content, Cursor, Edit, Position};
use iced::{Border, Color, Element, Padding};
use std::hash::{Hash as _, Hasher as _};
use ui_lang_runtime::rich_text_editor::{
    ContentVersion, GUTTER_WIDTH, GutterButton, MARGIN_WIDTH, MenuEvent, RichTextEditor,
};

pub use ui_lang_runtime::rich_text_editor::Action as PageAction;

/// Everything the page surface can emit: an ordinary editor interaction, a
/// checkbox tick (a consumed line press over a todo's `[ ]`), a link the
/// reader asked to open, an anchored-menu event, a gutter press, or a press
/// on a commented block's margin badge.
#[derive(Clone, Debug)]
pub enum PageEvent {
    Action(PageAction),
    ToggleTodo(usize),
    OpenLink(String),
    Menu(MenuEvent),
    Gutter(usize, GutterButton),
    GutterDrop(usize, usize),
    OpenComments,
}

/// Classify a left press over `(line, position)` — the widget's
/// `on_line_press` seam. `Some` consumes the press.
fn line_press(line: &str, position: Position) -> Option<PageEvent> {
    if let Some(range) = todo_box_columns(line)
        && range.contains(&position.column)
    {
        return Some(PageEvent::ToggleTodo(position.line));
    }
    let url = link_at(line, position.column)?;
    Some(PageEvent::OpenLink(url))
}

/// The CHARACTER columns of a todo line's `[ ]` box, tick included.
fn todo_box_columns(line: &str) -> Option<std::ops::Range<usize>> {
    let trimmed = line.trim_start_matches([' ', '\t']);
    let indent = line.len() - trimmed.len();
    let ticked = trimmed.starts_with("- [ ] ")
        || trimmed.starts_with("- [x] ")
        || trimmed.starts_with("- [X] ");
    ticked.then(|| indent + 2..indent + 5)
}

/// The http(s) link under the CHARACTER column, if any. Only web schemes are
/// openable — this hands a string to the OS.
fn link_at(line: &str, column: usize) -> Option<String> {
    let byte = line
        .char_indices()
        .nth(column)
        .map_or(line.len(), |(offset, _)| offset);
    crate::editor::inline_marks(line)
        .into_iter()
        .find(|(range, inline)| {
            matches!(inline, crate::editor::Inline::Link) && range.contains(&byte)
        })
        .map(|(range, _)| line[range].to_string())
        .filter(|url| url.starts_with("http://") || url.starts_with("https://"))
}

/// The link an event carries — "" otherwise, so a flat handler can guard on
/// emptiness.
pub fn page_link_of(event: PageEvent) -> String {
    match event {
        PageEvent::OpenLink(url) => url,
        PageEvent::Action(_)
        | PageEvent::ToggleTodo(_)
        | PageEvent::Menu(_)
        | PageEvent::Gutter(..)
        | PageEvent::GutterDrop(..)
        | PageEvent::OpenComments => String::new(),
    }
}

/// True for a margin-badge press — the handler's guard for opening the rail.
pub fn page_opens_comments(event: PageEvent) -> bool {
    matches!(event, PageEvent::OpenComments)
}

/// The first line of each commented run — where a margin badge sits.
// ponytail: two ADJACENT commented blocks merge into one run and share one
// badge; split on block spans if that ever reads wrong.
/// One chip per commented block, carrying HOW MANY threads sit on it.
///
/// `hits` holds the target of every unresolved thread, repeats and all (see
/// `backend::load::commented_targets`), so counting a block's entries IS its
/// thread count. The chip rides the block's FIRST line: a block can wrap over
/// several document lines and one plate per visual row would be a column of
/// identical badges down the margin.
fn comment_marks(blocks: &[crate::backend::PageBlock], hits: &[String]) -> Vec<(usize, usize)> {
    let mut marks = Vec::new();
    for (id, start, _len) in sync::line_spans(blocks) {
        let count = hits.iter().filter(|hit| *hit == &id).count();
        if count > 0 {
            marks.push((start, count));
        }
    }
    marks
}

use markdown::{BODY_LINE_HEIGHT, BODY_SIZE, Caret, DocumentHighlighter};

/// The breathing room past the text on each side; the left side adds the
/// widget's own hover-gutter strip on top of it.
const DOCUMENT_PAD_X: f32 = 2.0;

/// The page's writing surface. `commented` is the document lines wearing a
/// commented block's wash.
pub fn page_document(
    document: &Content,
    dark: bool,
    disabled: bool,
    blocks: Vec<crate::backend::PageBlock>,
    hits: Vec<String>,
) -> Element<'_, PageEvent> {
    let cursor = document.cursor().position;
    let commented = commented_lines(blocks.clone(), hits.clone());
    let marks = comment_marks(&blocks, &hits);
    let editor = RichTextEditor::new(document, content_version(document))
        .id("page-document")
        .placeholder("Write something… `#` for a heading, `-` for a list")
        .width(iced::Length::Fill)
        // Fill, so the widget owns a FINITE viewport: its caret-reveal and
        // scrolling are internal, and an outer scrollable would hand it
        // infinite height and turn both into no-ops.
        .height(iced::Length::Fill)
        .font(crate::Ducktape::default_font())
        .size(BODY_SIZE)
        .line_height(BODY_LINE_HEIGHT)
        .wrapping(Wrapping::Word)
        // The left inset carries the widget's hover gutter ("+" and the
        // handle), the right one the comment badges — both aligned to the
        // line they belong to.
        .padding(Padding {
            left: GUTTER_WIDTH + DOCUMENT_PAD_X,
            right: MARGIN_WIDTH + DOCUMENT_PAD_X,
            ..Padding::from([0.0, DOCUMENT_PAD_X])
        })
        .highlight_with::<DocumentHighlighter>(
            Caret {
                line: cursor.line,
                column: cursor.column,
                dark,
                commented,
            },
            u64::from(dark),
            move |mark| markdown::format(mark, dark),
        )
        .style(move |_theme, status| document_style(dark, status));
    if disabled {
        return editor.into();
    }
    editor
        .on_action(PageEvent::Action)
        .on_line_press(line_press)
        .margin_marks(marks, |_| PageEvent::OpenComments)
        // The chip is painted inside the widget, so it can take no ice
        // `label=` tooltip — this is the only thing that names it.
        .margin_label("Open comments")
        // Line 0 is the title — it takes no gutter, like the reference
        // editor's title row.
        .on_gutter(|line, button| (line > 0).then_some(PageEvent::Gutter(line, button)))
        .on_gutter_drop(menu::drop_boundaries(document), |from, boundary| {
            Some(PageEvent::GutterDrop(from, boundary))
        })
        .menu(menu::current(document))
        .on_menu(PageEvent::Menu)
        .into()
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

/// Apply one surface event to the buffer. Edits record an undo snapshot
/// first (coalesced — see [`history`]); a checkbox tick is its own buffer
/// edit; an opened link never touches the buffer (the handler owns the side
/// effect).
pub fn apply_page_event(document: Content, event: PageEvent) -> Content {
    match event {
        PageEvent::Action(action) => {
            let document = apply_page_action(document, action.clone());
            menu::after_action(&document, &action);
            document
        }
        PageEvent::ToggleTodo(line) => toggle_todo(document, line),
        PageEvent::OpenLink(_) | PageEvent::OpenComments => document,
        PageEvent::Menu(event) => menu::apply(document, event),
        PageEvent::Gutter(line, button) => menu::gutter(document, line, button),
        PageEvent::GutterDrop(from, boundary) => menu::drop_move(document, from, boundary),
    }
}

/// Flip the `[ ]`/`[x]` box on `line`. The buffer is the only thing edited —
/// the save tick reads the drift and plans `SetChecked` like any other edit.
fn toggle_todo(mut document: Content, line: usize) -> Content {
    let Some(row) = document.line(line) else {
        return document;
    };
    let text = row.text.into_owned();
    let Some(range) = todo_box_columns(&text) else {
        return document;
    };
    history::record(|| (document.text(), document.cursor()));
    let tick = range.start + 1;
    let ticked = text[tick..].starts_with(['x', 'X']);
    let replacement = match ticked {
        true => " ",
        false => "x",
    };
    replace_range(
        &mut document,
        Position { line, column: tick },
        Position {
            line,
            column: tick + 1,
        },
        replacement,
    );
    document
}

/// WHICH history move the Cmd/Ctrl+Z / +Shift+Z chord asks for — "undo",
/// "redo", or "" for every other press. Split out of [`page_history_key`] so
/// the keyboard handler can resolve the chord BEFORE it decides to rebuild the
/// buffer: the assignment that applies the move lowers to
/// `mem::take(&mut page_editor)`, and a taken `editor` leaves a fresh
/// `Content::default()` behind — a cosmic-text buffer built under a write lock
/// on the process-global font system, per key press, for a chord that fires
/// once in a thousand.
pub fn page_history_shortcut(
    _logical: iced::keyboard::Key,
    physical: iced::keyboard::key::Physical,
    modifiers: iced::keyboard::Modifiers,
    ready: bool,
) -> String {
    use iced::keyboard::key::{Code, Physical};
    let is_z = matches!(physical, Physical::Code(Code::KeyZ));
    if !ready || !is_z || !modifiers.command() {
        return String::new();
    }
    match modifiers.shift() {
        false => "undo".to_owned(),
        true => "redo".to_owned(),
    }
}

/// Apply the move [`page_history_shortcut`] named. An empty verdict is the
/// identity, so the caller stays branch-free.
pub fn page_history_key(document: Content, action: String) -> Content {
    let restored = match action.as_str() {
        "undo" => history::undo(|| (document.text(), document.cursor())),
        "redo" => history::redo(|| (document.text(), document.cursor())),
        _ => return document,
    };
    restored.unwrap_or(document)
}

/// Apply one editor interaction to the buffer.
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
    history::record(|| (document.text(), document.cursor()));
    // A key that can MOVE a line owes the ordered runs a recount; typing a
    // character into one cannot, and must not pay for the walk.
    let structural = matches!(
        edit,
        Edit::Enter
            | Edit::Backspace
            | Edit::Delete
            | Edit::Indent
            | Edit::Unindent
            | Edit::Paste(_)
    );
    let handled = match edit {
        Edit::Enter => close_fence(&mut document) || continue_list(&mut document),
        Edit::Backspace => remove_list_marker(&mut document),
        Edit::Indent => shift_indent(&mut document, 1),
        Edit::Unindent => shift_indent(&mut document, -1),
        _ => false,
    };
    if !handled {
        document.perform(edit_action);
    }
    if structural {
        let landed = document.cursor().position.line;
        let from = list_start(&document, landed);
        renumber_below(&mut document, from);
    }
    document
}

/// The document's text, for the save tick's dirty check and its plan.
///
/// Owned, not borrowed: the extern boundary hands state fields by value.
/// VERBATIM: iced 0.14's `Content::text()` joins lines without inventing a
/// trailing newline, so a trailing newline here IS a final empty line — the
/// empty paragraph a page can end on. Trimming it made every such page dirty
/// on open, and the resulting plan REMOVED the block.
pub fn page_text(document: Content) -> String {
    document.text()
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

/// The first line of the list the edit landed in. Numbering is positional, so
/// a run can only be counted from its own start — seeding from "the number on
/// the line above" would propagate whatever number was already wrong.
fn list_start(document: &Content, line: usize) -> usize {
    let mut start = line.min(document.line_count().saturating_sub(1));
    while start > 0 {
        let Some(above) = document.line(start - 1) else {
            break;
        };
        if list_marker(&above.text).is_none() {
            break;
        }
        start -= 1;
    }
    start
}

/// Re-count every ordered run from `from` down, one counter per depth.
///
/// An ordered marker is POSITIONAL — the store keeps no number (see
/// [`sync::parse_document`]) — so any number the buffer shows that is not the
/// item's position is one the next reload silently corrects. Every structural
/// key routes through here rather than each one doing its own arithmetic.
///
/// A shallower line drops every deeper counter, so a nested list restarts at 1
/// under each parent; a line that is not an ordered item ends the run at its
/// own depth, and the next item there starts a NEW run at 1 — which is exactly
/// what backspacing a marker out of the middle of a list leaves behind.
///
/// It walks to the end of the document rather than stopping at the first
/// paragraph: a run below one can still be a run this edit split off.
// ponytail: O(lines) per structural key. A page is tens of lines and only a
// line whose number actually moved is rewritten; bound it to the edited run if
// documents ever get long enough to feel it.
fn renumber_below(document: &mut Content, from: usize) {
    let mut caret = document.cursor();
    let mut counts: Vec<u64> = Vec::new();
    let mut index = from;
    while let Some(line) = document.line(index) {
        let text = line.text.into_owned();
        let (depth, rest) = sync::split_indent(&text);
        counts.truncate(depth + 1);
        counts.resize(depth + 1, 0);
        let Some(digits) = sync::ordered_digits(rest) else {
            counts[depth] = 0;
            index += 1;
            continue;
        };
        counts[depth] += 1;
        let number = counts[depth].to_string();
        let column = text.len() - rest.len();
        if number.len() != digits {
            // The caret sits on this line and past the marker: widening or
            // narrowing the number carries the caret with it.
            let past_marker =
                index == caret.position.line && caret.position.column >= column + digits;
            if past_marker {
                caret.position.column = caret
                    .position
                    .column
                    .saturating_add_signed(number.len() as isize - digits as isize);
            }
        }
        replace_range(
            document,
            Position {
                line: index,
                column,
            },
            Position {
                line: index,
                column: column + digits,
            },
            &number,
        );
        index += 1;
    }
    document.move_to(caret);
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
///
/// An indent the TREE cannot hold is refused (consumed as a no-op): line 0 is
/// the title, the first body line has no sibling to move under, and any line
/// may only go one step past the line above it — the module's `MoveBlock`
/// rejects anything deeper, so allowing it here would strand the buffer at a
/// depth no save can persist.
fn shift_indent(document: &mut Content, steps: i32) -> bool {
    let cursor = document.cursor();
    let Some(line) = document.line(cursor.position.line) else {
        return false;
    };
    let text = line.text.into_owned();
    let indent = text.len() - text.trim_start_matches([' ', '\t']).len();
    if steps > 0 {
        if cursor.position.line <= 1 {
            return true;
        }
        let ceiling = document
            .line(cursor.position.line - 1)
            .map_or(0, |above| sync::split_indent(&above.text).0 + 1);
        let deep_enough = sync::split_indent(&text).0 + 1 > ceiling;
        if deep_enough {
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
                column: 0,
            },
            "  ",
        );
        // `replace_range` parks the caret after the pasted indent; the caret
        // belongs on the character it was on, two columns to the right.
        document.move_to(Cursor {
            position: Position {
                line: cursor.position.line,
                column: cursor.position.column + 2,
            },
            selection: None,
        });
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
    document.move_to(Cursor {
        position: Position {
            line: cursor.position.line,
            column: cursor.position.column.saturating_sub(removable),
        },
        selection: None,
    });
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
        let content = typed("1. one\n2. two", 1, 6);
        assert_eq!(press(content, Edit::Enter), "1. one\n2. two\n3. ");
    }

    #[test]
    fn enter_renumbers_the_ordered_run_below_the_new_item() {
        let content = typed("1. one\n2. two\n3. three", 0, 6);
        assert_eq!(press(content, Edit::Enter), "1. one\n2. \n3. two\n4. three");
    }

    #[test]
    fn renumbering_steps_over_children_and_stops_at_the_run_s_end() {
        // A deeper line belongs to the item above it and counts on its own;
        // the paragraph ends the run, so what follows is a NEW list at 1.
        let nested = typed("1. one\n  1. child\n2. two\nplain\n9. apart", 0, 6);
        assert_eq!(
            press(nested, Edit::Enter),
            "1. one\n2. \n  1. child\n3. two\nplain\n1. apart"
        );
    }

    #[test]
    fn a_bullet_run_below_is_left_alone() {
        let content = typed("- one\n- two", 0, 5);
        assert_eq!(press(content, Edit::Enter), "- one\n- \n- two");
    }

    #[test]
    fn backspacing_a_marker_out_restarts_the_run_below_it() {
        // "two" stops being an item, so what is left below it is a NEW list.
        let content = typed("1. one\n2. two\n3. three", 1, 3);
        assert_eq!(press(content, Edit::Backspace), "1. one\ntwo\n1. three");
    }

    #[test]
    fn tab_recounts_both_the_run_it_left_and_the_one_it_joined() {
        // Line 0 is the title, so the list starts on line 1.
        let content = typed("Title\n1. one\n2. two\n3. three", 2, 6);
        // "two" nests under "one" as its first child, and "three" takes the
        // number "two" gave up.
        assert_eq!(
            press(content, Edit::Indent),
            "Title\n1. one\n  1. two\n2. three"
        );
    }

    #[test]
    fn shift_tab_lifts_an_item_back_into_the_run_above_it() {
        let content = typed("1. one\n  1. two\n  2. three", 2, 9);
        assert_eq!(press(content, Edit::Unindent), "1. one\n  1. two\n2. three");
    }

    #[test]
    fn joining_two_items_recounts_what_is_left() {
        // Backspace at column 0 merges the line up — one item fewer.
        let content = typed("1. one\n2. two\n3. three", 1, 0);
        assert_eq!(press(content, Edit::Backspace), "1. one2. two\n2. three");
    }

    #[test]
    fn a_run_is_counted_from_its_own_start_not_from_the_number_above() {
        // Typing "5." does not buy a list that starts at five: the store keeps
        // no number, so the buffer has to show the position it will come back
        // as.
        let content = typed("5. one\n9. two", 0, 6);
        assert_eq!(press(content, Edit::Enter), "1. one\n2. \n3. two");
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
        let content = typed("Title\n- a\n- one", 2, 5);
        assert_eq!(press(content, Edit::Indent), "Title\n- a\n  - one");
    }

    #[test]
    fn tab_refuses_a_depth_the_tree_cannot_hold() {
        // The first body line has no sibling to move under…
        let first = typed("Title\n- one", 1, 5);
        assert_eq!(press(first, Edit::Indent), "Title\n- one");
        // …and no line may go more than one step past the line above it.
        let deep = typed("Title\n- a\n  - b", 2, 7);
        assert_eq!(press(deep, Edit::Indent), "Title\n- a\n  - b");
        let ladder = typed("Title\n- a\n- b", 2, 5);
        assert_eq!(press(ladder, Edit::Indent), "Title\n- a\n  - b");
    }

    #[test]
    fn shift_tab_lifts_one_step_and_stops_at_the_left_margin() {
        let nested = typed("    - one", 0, 9);
        assert_eq!(press(nested, Edit::Unindent), "  - one");
        let flat = typed("- one", 0, 5);
        assert_eq!(press(flat, Edit::Unindent), "- one");
    }

    #[test]
    fn tab_keeps_the_caret_on_its_character() {
        let indent = PageAction::Edit(Action::Edit(Edit::Indent));
        let nested = apply_page_action(typed("Title\n- a\n- one", 2, 5), indent);
        assert_eq!(nested.text(), "Title\n- a\n  - one");
        assert_eq!(nested.cursor().position.column, 7);
        let unindent = PageAction::Edit(Action::Edit(Edit::Unindent));
        let lifted = apply_page_action(typed("Title\n- a\n  - one", 2, 7), unindent);
        assert_eq!(lifted.text(), "Title\n- a\n- one");
        assert_eq!(lifted.cursor().position.column, 5);
        // A caret inside the indent being removed clamps to the margin.
        let unindent = PageAction::Edit(Action::Edit(Edit::Unindent));
        let clamped = apply_page_action(typed("Title\n- a\n  - one", 2, 1), unindent);
        assert_eq!(clamped.text(), "Title\n- a\n- one");
        assert_eq!(clamped.cursor().position.column, 0);
    }

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
        assert_eq!(
            commented_lines(blocks.clone(), vec!["a\nb".into()]),
            vec![2, 3, 4, 5]
        );
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

        assert!(page_opens_comments(PageEvent::OpenComments));
        assert!(!page_opens_comments(PageEvent::OpenLink("x".into())));
    }

    #[test]
    fn a_press_on_the_todo_box_ticks_and_a_press_on_a_link_opens() {
        use iced::widget::text_editor::Position;
        // The box, not the bullet and not the text: columns 2..5 of the
        // marker (`[`, tick, `]`).
        let on_box = Position { line: 4, column: 3 };
        assert!(matches!(
            line_press("- [ ] ship it", on_box),
            Some(PageEvent::ToggleTodo(4))
        ));
        let on_text = Position { line: 4, column: 8 };
        assert!(line_press("- [ ] ship it", on_text).is_none());
        assert!(line_press("- plain bullet", on_box).is_none());
        // A press inside a web link opens it; other schemes stay text.
        let line = "see https://duck.example/docs today";
        let inside = Position { line: 1, column: 8 };
        assert!(matches!(
            line_press(line, inside),
            Some(PageEvent::OpenLink(url)) if url == "https://duck.example/docs"
        ));
        let outside = Position { line: 1, column: 1 };
        assert!(line_press(line, outside).is_none());
    }

    #[test]
    fn a_todo_tick_is_one_buffer_edit_the_save_tick_can_read() {
        let content = typed("Title\n- [ ] ship", 0, 0);
        let ticked = apply_page_event(content, PageEvent::ToggleTodo(1));
        assert_eq!(ticked.text(), "Title\n- [x] ship");
        let unticked = apply_page_event(ticked, PageEvent::ToggleTodo(1));
        assert_eq!(unticked.text(), "Title\n- [ ] ship");
        // An open-link event never touches the buffer.
        let same = apply_page_event(
            typed("Title\nbody", 0, 0),
            PageEvent::OpenLink("https://x".into()),
        );
        assert_eq!(same.text(), "Title\nbody");
    }

    #[test]
    fn cmd_z_walks_the_history_and_shift_redoes() {
        use iced::keyboard::key::{Code, Physical};
        use iced::keyboard::{Key, Modifiers};
        history::reset();
        let typed_doc = apply_page_event(
            typed("Title\nbod", 1, 3),
            PageEvent::Action(PageAction::Edit(Action::Edit(Edit::Insert('y')))),
        );
        assert_eq!(typed_doc.text(), "Title\nbody");
        let undo = page_history_shortcut(
            Key::Unidentified,
            Physical::Code(Code::KeyZ),
            Modifiers::COMMAND,
            true,
        );
        assert_eq!(undo, "undo");
        let undone = page_history_key(typed_doc, undo);
        assert_eq!(undone.text(), "Title\nbod");
        // The caret returns to where the group STARTED, not to the origin.
        assert_eq!(
            undone.cursor().position,
            iced::widget::text_editor::Position { line: 1, column: 3 }
        );
        let redo = page_history_shortcut(
            Key::Unidentified,
            Physical::Code(Code::KeyZ),
            Modifiers::COMMAND | Modifiers::SHIFT,
            true,
        );
        assert_eq!(redo, "redo");
        let redone = page_history_key(undone, redo);
        assert_eq!(redone.text(), "Title\nbody");
        // …and redo puts it back where the caret sat when Cmd+Z was pressed.
        assert_eq!(
            redone.cursor().position,
            iced::widget::text_editor::Position { line: 1, column: 4 }
        );
        // Off the pages tab the chord names no move — which is what keeps
        // `global_key_pressed` from taking the buffer on an ordinary keystroke.
        let parked_move = page_history_shortcut(
            Key::Unidentified,
            Physical::Code(Code::KeyZ),
            Modifiers::COMMAND,
            false,
        );
        assert_eq!(parked_move, "");
        let parked = page_history_key(redone, parked_move);
        assert_eq!(parked.text(), "Title\nbody");
        history::reset();
    }

    #[test]
    fn page_text_keeps_a_final_empty_line_because_it_is_a_block() {
        // Trimming it made every page ending on an empty paragraph dirty on
        // open — and the resulting plan REMOVED that block unprompted.
        let content = Content::with_text("one\ntwo\n");
        assert_eq!(page_text(content), "one\ntwo\n");
        assert_eq!(page_text(Content::with_text("one")), "one");
    }
}
