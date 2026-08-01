//! The composer surface: `ui_lang_runtime::RichTextEditor` behind the
//! `crate::editor` externs. The stock Ice `editor` widget compiles to iced's
//! plain `TextEditor`; this adapter is what carries the rich line layout and
//! the IME hardening (preedit metrics, incremental relayout, grapheme deletes)
//! that live only in the custom widget.
//!
//! The inline highlighter mirrors `chat::client::inline_spans` — the SAME
//! marker grammar the message renderer parses, so what lights up while typing
//! is exactly what the sent message will format. Marks do not nest; the first
//! matching delimiter wins; there are no word-boundary rules. Mentions are the
//! one renderer mark not previewed here: they need the member roster, and the
//! composer is not worth a roster prop while the plain-ink fallback reads fine.

use iced::advanced::text::{self, Highlighter};
use iced::font::{Style as FontStyle, Weight};
use iced::widget::text_editor::{self, Content};
use iced::{Border, Color, Element, Font};
use std::hash::{Hash as _, Hasher as _};
use std::ops::Range;
use ui_lang_runtime::rich_text_editor::{ContentVersion, Format, RichTextEditor};

pub use ui_lang_runtime::rich_text_editor::Action as RichAction;

const COMPOSER_SIZE: f32 = 13.5;
const COMPOSER_LINE_HEIGHT: f32 = 1.3;

// theme.ice mirrors: the adapter paints outside the Ice styling surface, so
// these carry the palette values it cannot reach. Keep them in sync with the
// palette block in `ui/theme.ice`.
const fn rgb8(r: u8, g: u8, b: u8) -> Color {
    Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
}
/// The composer's ink set, one per palette reading of `theme.ice`.
struct ComposerInk {
    ink: Color,
    muted: Color,
    hint: Color,
    ring: Color,
}

const LIGHT_INK: ComposerInk = ComposerInk {
    ink: rgb8(0x2c, 0x2b, 0x27),
    muted: rgb8(0x6b, 0x69, 0x62),
    hint: rgb8(0xb3, 0xb1, 0xa8),
    ring: rgb8(0x26, 0x25, 0x1f),
};

const DARK_INK: ComposerInk = ComposerInk {
    ink: rgb8(0xe8, 0xe6, 0xdf),
    muted: rgb8(0xa8, 0xa6, 0x9c),
    hint: rgb8(0x6b, 0x6a, 0x61),
    ring: rgb8(0xe8, 0xe6, 0xdf),
};

fn composer_ink(theme: &iced::Theme) -> &'static ComposerInk {
    if crate::backend::theme_is_dark(theme) {
        &DARK_INK
    } else {
        &LIGHT_INK
    }
}

// ponytail: the inline-mark formatter has no theme input (`format_key` is a
// static 0), so the marker/link inks are single values chosen to read on both
// palettes. Thread the appearance into the composer externs and bump
// `format_key` per palette if these ever need per-theme tuning.
const MARK_DIM: Color = rgb8(0x8a, 0x88, 0x7e);
const MARK_LINK: Color = rgb8(0x6f, 0x8a, 0xab);

/// One composer interaction, classified where the modifiers are still known.
/// `Submit` is plain Enter (and the Send button, via
/// [`composer_submit_event`]); everything else is an edit to apply.
#[derive(Clone, Debug, PartialEq)]
pub enum ComposerEvent {
    Submit,
    Apply(RichAction),
}

pub fn composer_submit_event() -> ComposerEvent {
    ComposerEvent::Submit
}

pub fn composer_submits(event: ComposerEvent) -> bool {
    matches!(event, ComposerEvent::Submit)
}

pub fn apply_composer_event(mut document: Content, event: ComposerEvent) -> Content {
    match event {
        ComposerEvent::Apply(RichAction::Edit(action)) => document.perform(action),
        ComposerEvent::Apply(RichAction::MoveTo(cursor)) => document.move_to(cursor),
        ComposerEvent::Submit => {}
    }
    document
}

pub fn rich_composer(
    document: &Content,
    hint: String,
    disabled: bool,
    shift: bool,
    min_h: f64,
    max_h: f64,
    pad: f64,
) -> Element<'_, ComposerEvent> {
    let editor = RichTextEditor::new(document, content_version(document))
        .placeholder(hint)
        .width(iced::Length::Fill)
        .min_height(min_h as f32)
        .max_height(max_h as f32)
        .font(composer_font(Weight::Normal, FontStyle::Normal))
        .size(COMPOSER_SIZE)
        .line_height(COMPOSER_LINE_HEIGHT)
        .wrapping(text::Wrapping::Word)
        .padding(pad as f32)
        // format_key 0: the format table is static — no theme or mode inputs.
        .highlight_with::<InlineMarkdownHighlighter>((), 0, inline_format)
        .style(composer_style);
    if disabled {
        return editor.into();
    }
    editor
        .on_action(move |action| classify(action, shift))
        .into()
}

/// The widget's change-detection key: equal versions promise equal text
/// (cursor and selection moves keep the version, per the widget's contract).
/// The Ice `editor` state is a bare `Content` with no revision counter, so
/// the version is the text's own hash — the composer drafts are small, and
/// the widget skips its internal resync whenever the text is unchanged.
fn content_version(document: &Content) -> ContentVersion {
    let mut hasher = std::hash::DefaultHasher::new();
    document.text().hash(&mut hasher);
    ContentVersion::new(0, hasher.finish())
}

fn classify(action: RichAction, shift: bool) -> ComposerEvent {
    let plain_enter = matches!(
        action,
        RichAction::Edit(text_editor::Action::Edit(text_editor::Edit::Enter))
    );
    if plain_enter && !shift {
        return ComposerEvent::Submit;
    }
    ComposerEvent::Apply(action)
}

fn composer_style(theme: &iced::Theme, status: text_editor::Status) -> text_editor::Style {
    let ink = composer_ink(theme);
    let focused = matches!(status, text_editor::Status::Focused { .. });
    let disabled = matches!(status, text_editor::Status::Disabled);
    text_editor::Style {
        background: Color::TRANSPARENT.into(),
        border: if focused {
            Border {
                color: ink.ring,
                width: 1.0,
                // The composer mounts inside rounded plates (r=12 cards in
                // chat, thread and forge) — a square ring visibly pokes their
                // corners.
                radius: 9.0.into(),
            }
        } else {
            Border::default()
        },
        placeholder: ink.hint,
        value: if disabled { ink.muted } else { ink.ink },
        selection: Color { a: 0.18, ..ink.ink },
    }
}

// The generated app exposes the `default=true` font declaration, so the
// adapter follows `theme.ice` instead of duplicating the family name.
fn composer_font(weight: Weight, style: FontStyle) -> Font {
    Font {
        weight,
        style,
        ..crate::Ducktape::default_font()
    }
}

/// One structural key pressed in the pages block editor, carried through the
/// editor's checked route. `action` is one of "split", "delete", "indent",
/// "outdent", "escape" — classified in [`block_key_press`] where the modifiers
/// and the block's shape are still known.
#[derive(Clone, Debug, PartialEq)]
pub struct BlockKeyEvent {
    pub action: String,
}

/// The block editor's key binding: structural keys become [`BlockKeyEvent`]s,
/// everything else keeps its native editing binding. Enter splits the block
/// (Shift+Enter stays a newline); inside a Code block that inverts — Enter is
/// a newline and Shift+Enter leaves. Backspace only turns structural on an
/// EMPTY block with no children, because RemoveBlock takes the whole subtree.
pub fn block_key_press(
    press: text_editor::KeyPress,
    kind: String,
    empty: bool,
    has_children: bool,
) -> Option<text_editor::Binding<BlockKeyEvent>> {
    let focused = matches!(press.status, text_editor::Status::Focused { .. });
    if !focused {
        return text_editor::Binding::from_key_press(press);
    }
    let shift = press.modifiers.shift();
    let key = press.key.clone();
    let custom = |action: &str| {
        Some(text_editor::Binding::Custom(BlockKeyEvent {
            action: action.into(),
        }))
    };
    match key.as_ref() {
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter) => {
            let newline_is_default = kind == "Code";
            let splits = if newline_is_default { shift } else { !shift };
            if splits {
                custom("split")
            } else {
                text_editor::Binding::from_key_press(press)
            }
        }
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Backspace)
            if empty && !has_children =>
        {
            custom("delete")
        }
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Tab) if shift => custom("outdent"),
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Tab) => custom("indent"),
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) => custom("escape"),
        _ => text_editor::Binding::from_key_press(press),
    }
}

/// The block editor's highlighter seat: wraps the stock Ice `editor` widget
/// with the same inline-markdown highlighter the composers use, so marks
/// light up identically while typing a block and while typing a message.
pub fn page_inline_marks<'a, Message: Clone + 'a>(
    editor: text_editor::TextEditor<'a, text::highlighter::PlainText, Message>,
) -> impl Into<Element<'a, Message>> {
    editor.highlight_with::<InlineMarkdownHighlighter>((), stock_inline_format)
}

/// [`inline_format`] for the stock widget's format table, which carries a
/// theme reference the custom widget's static table does not.
fn stock_inline_format(kind: &Inline, _theme: &iced::Theme) -> text::highlighter::Format<Font> {
    match kind {
        Inline::Marker => text::highlighter::Format {
            color: Some(MARK_DIM),
            font: None,
        },
        Inline::Bold => text::highlighter::Format {
            color: None,
            font: Some(composer_font(Weight::Bold, FontStyle::Normal)),
        },
        Inline::Italic => text::highlighter::Format {
            color: None,
            font: Some(composer_font(Weight::Normal, FontStyle::Italic)),
        },
        Inline::Link => text::highlighter::Format {
            color: Some(MARK_LINK),
            font: None,
        },
    }
}

/// A renderer-parity inline mark, plus the marker glyphs themselves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Inline {
    Marker,
    Bold,
    Italic,
    Link,
}

fn inline_format(kind: &Inline) -> Format {
    match kind {
        Inline::Marker => Format {
            color: Some(MARK_DIM),
            ..Format::default()
        },
        Inline::Bold => Format {
            font: Some(composer_font(Weight::Bold, FontStyle::Normal)),
            ..Format::default()
        },
        Inline::Italic => Format {
            font: Some(composer_font(Weight::Normal, FontStyle::Italic)),
            ..Format::default()
        },
        Inline::Link => Format {
            color: Some(MARK_LINK),
            ..Format::default()
        },
    }
}

struct InlineMarkdownHighlighter {
    current_line: usize,
}

impl Highlighter for InlineMarkdownHighlighter {
    type Settings = ();
    type Highlight = Inline;
    type Iterator<'a> = std::vec::IntoIter<(Range<usize>, Inline)>;

    fn new(_settings: &Self::Settings) -> Self {
        Self { current_line: 0 }
    }

    fn update(&mut self, _settings: &Self::Settings) {
        self.current_line = 0;
    }

    fn change_line(&mut self, line: usize) {
        self.current_line = line;
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        self.current_line += 1;
        inline_marks(line).into_iter()
    }

    fn current_line(&self) -> usize {
        self.current_line
    }
}

/// Byte-ranged mirror of `chat::client::inline_spans`, minus mentions: bare
/// `http(s)://` runs, then `**`/`__` bold, then `*`/`_` italic; unmatched or
/// empty fences stay plain. Ranges land on char boundaries by construction —
/// the scanner only advances through `char_indices`.
fn inline_marks(line: &str) -> Vec<(Range<usize>, Inline)> {
    let mut marks = Vec::new();
    let mut at = 0;
    while at < line.len() {
        let rest = &line[at..];
        if let Some(len) = url_len(rest) {
            marks.push((at..at + len, Inline::Link));
            at += len;
            continue;
        }
        let fence = ["**", "__", "*", "_"]
            .iter()
            .find_map(|marker| fenced(rest, marker));
        let Some((marker_len, inner_len)) = fence else {
            at += rest.chars().next().map_or(1, char::len_utf8);
            continue;
        };
        let bold = marker_len == 2;
        let kind = if bold { Inline::Bold } else { Inline::Italic };
        let body = at + marker_len..at + marker_len + inner_len;
        marks.push((at..body.start, Inline::Marker));
        marks.push((body.clone(), kind));
        marks.push((body.end..body.end + marker_len, Inline::Marker));
        at = body.end + marker_len;
    }
    marks
}

/// If `rest` opens with `marker` and a later closing `marker` encloses a
/// non-empty body, `(marker byte length, body byte length)`.
fn fenced(rest: &str, marker: &str) -> Option<(usize, usize)> {
    let body = rest.strip_prefix(marker)?;
    let close = body.find(marker)?;
    if close == 0 {
        return None;
    }
    Some((marker.len(), close))
}

/// If `rest` opens a bare link, its byte length: the renderer's rule — an
/// `http(s)://` prefix, then everything up to whitespace.
fn url_len(rest: &str) -> Option<usize> {
    let starts_link = rest.starts_with("http://") || rest.starts_with("https://");
    if !starts_link {
        return None;
    }
    let len = rest.find(char::is_whitespace).unwrap_or(rest.len());
    Some(len)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mark_texts(line: &str) -> Vec<(&str, Inline)> {
        inline_marks(line)
            .into_iter()
            .map(|(range, kind)| (&line[range], kind))
            .collect()
    }

    #[test]
    fn marks_mirror_the_renderer_grammar() {
        assert_eq!(
            mark_texts("say **hi** to _all_ at https://duck.example/x"),
            vec![
                ("**", Inline::Marker),
                ("hi", Inline::Bold),
                ("**", Inline::Marker),
                ("_", Inline::Marker),
                ("all", Inline::Italic),
                ("_", Inline::Marker),
                ("https://duck.example/x", Inline::Link),
            ]
        );
    }

    #[test]
    fn intra_word_underscores_italicize_like_the_renderer() {
        // `chat::client::fenced` has no word-boundary rule; the preview must
        // agree with the renderer, surprising or not.
        assert_eq!(
            mark_texts("op_hash_receipts"),
            vec![
                ("_", Inline::Marker),
                ("hash", Inline::Italic),
                ("_", Inline::Marker),
            ]
        );
    }

    #[test]
    fn unmatched_and_empty_fences_stay_plain() {
        assert!(mark_texts("2 * 3 = six").is_empty());
        assert!(mark_texts("****").is_empty());
        assert!(mark_texts("*never closed").is_empty());
    }

    #[test]
    fn multibyte_text_keeps_char_boundaries() {
        assert_eq!(
            mark_texts("한글 **굵게** 그리고 _기울임_"),
            vec![
                ("**", Inline::Marker),
                ("굵게", Inline::Bold),
                ("**", Inline::Marker),
                ("_", Inline::Marker),
                ("기울임", Inline::Italic),
                ("_", Inline::Marker),
            ]
        );
    }

    #[test]
    fn plain_enter_submits_and_shift_enter_edits() {
        let enter = RichAction::Edit(text_editor::Action::Edit(text_editor::Edit::Enter));
        assert_eq!(classify(enter.clone(), false), ComposerEvent::Submit);
        assert_eq!(classify(enter.clone(), true), ComposerEvent::Apply(enter));
        let typed = RichAction::Edit(text_editor::Action::Edit(text_editor::Edit::Insert('x')));
        assert_eq!(classify(typed.clone(), false), ComposerEvent::Apply(typed));
    }

    #[test]
    fn apply_performs_edits_and_ignores_submit() {
        let document = Content::with_text("draft");
        let document = apply_composer_event(document, ComposerEvent::Submit);
        assert_eq!(document.text().trim_end(), "draft");
        let enter = ComposerEvent::Apply(RichAction::Edit(text_editor::Action::Edit(
            text_editor::Edit::Enter,
        )));
        let document = apply_composer_event(document, enter);
        assert_eq!(document.line_count(), 2);
    }

    fn key_press(named: iced::keyboard::key::Named, shift: bool) -> text_editor::KeyPress {
        let key = iced::keyboard::Key::Named(named);
        text_editor::KeyPress {
            key: key.clone(),
            modified_key: key,
            physical_key: iced::keyboard::key::Physical::Unidentified(
                iced::keyboard::key::NativeCode::Unidentified,
            ),
            modifiers: if shift {
                iced::keyboard::Modifiers::SHIFT
            } else {
                iced::keyboard::Modifiers::default()
            },
            text: None,
            status: text_editor::Status::Focused { is_hovered: false },
        }
    }

    fn action_of(binding: Option<text_editor::Binding<BlockKeyEvent>>) -> String {
        let Some(text_editor::Binding::Custom(event)) = binding else {
            return String::new();
        };
        event.action
    }

    #[test]
    fn block_enter_splits_except_inside_code_where_it_inverts() {
        use iced::keyboard::key::Named;
        let press = |shift| key_press(Named::Enter, shift);
        assert_eq!(
            action_of(block_key_press(press(false), "Text".into(), false, false)),
            "split"
        );
        // Shift+Enter stays the native newline binding.
        assert!(matches!(
            block_key_press(press(true), "Text".into(), false, false),
            Some(text_editor::Binding::Enter)
        ));
        assert!(matches!(
            block_key_press(press(false), "Code".into(), false, false),
            Some(text_editor::Binding::Enter)
        ));
        assert_eq!(
            action_of(block_key_press(press(true), "Code".into(), false, false)),
            "split"
        );
    }

    #[test]
    fn block_backspace_deletes_only_an_empty_childless_block() {
        use iced::keyboard::key::Named;
        let press = || key_press(Named::Backspace, false);
        assert_eq!(
            action_of(block_key_press(press(), "Text".into(), true, false)),
            "delete"
        );
        assert!(matches!(
            block_key_press(press(), "Text".into(), false, false),
            Some(text_editor::Binding::Backspace)
        ));
        assert!(matches!(
            block_key_press(press(), "Text".into(), true, true),
            Some(text_editor::Binding::Backspace)
        ));
    }

    #[test]
    fn block_tab_indents_and_escape_leaves() {
        use iced::keyboard::key::Named;
        assert_eq!(
            action_of(block_key_press(
                key_press(Named::Tab, false),
                "Text".into(),
                false,
                false
            )),
            "indent"
        );
        assert_eq!(
            action_of(block_key_press(
                key_press(Named::Tab, true),
                "Text".into(),
                false,
                false
            )),
            "outdent"
        );
        assert_eq!(
            action_of(block_key_press(
                key_press(Named::Escape, false),
                "Text".into(),
                false,
                false
            )),
            "escape"
        );
    }

    #[test]
    fn block_keys_stay_native_when_the_editor_is_not_focused() {
        use iced::keyboard::key::Named;
        let mut press = key_press(Named::Enter, false);
        press.status = text_editor::Status::Active;
        // The default table also refuses unfocused presses, so nothing fires.
        assert!(block_key_press(press, "Text".into(), false, false).is_none());
    }
}
