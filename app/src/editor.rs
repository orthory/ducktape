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
use iced::keyboard::{Key, key::Named};
use iced::widget::text_editor::{self, Binding, Content, Cursor, Edit, KeyPress, Motion};
use iced::{Border, Color, Element, Font};
use std::hash::{Hash as _, Hasher as _};
use std::ops::Range;
use ui_lang_runtime::rich_text_editor::{
    ContentVersion, Format, RichTextEditor, default_key_binding,
};

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
}

const LIGHT_INK: ComposerInk = ComposerInk {
    ink: rgb8(0x2c, 0x2b, 0x27),
    muted: rgb8(0x6b, 0x69, 0x62),
    hint: rgb8(0xb3, 0xb1, 0xa8),
};

const DARK_INK: ComposerInk = ComposerInk {
    ink: rgb8(0xe8, 0xe6, 0xdf),
    muted: rgb8(0xa8, 0xa6, 0x9c),
    hint: rgb8(0x6b, 0x6a, 0x61),
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

/// One composer interaction, classified at the widget's own key binding.
/// `Submit` is plain Enter (and the Send button, via
/// [`composer_submit_event`]); everything else is an edit to apply.
#[derive(Clone, Debug, PartialEq)]
pub enum ComposerEvent {
    Submit,
    Apply(RichAction),
    /// A formatting chord the widget claimed via its `on_chord` route —
    /// Cmd/Ctrl+B and friends land HERE, in the composer that has the
    /// caret, instead of bubbling to the app's one keyboard subscription
    /// (which cannot see widget focus, let alone a component instance).
    Mark(String),
}

pub fn composer_submit_event() -> ComposerEvent {
    ComposerEvent::Submit
}

pub fn composer_submits(event: ComposerEvent) -> bool {
    matches!(event, ComposerEvent::Submit)
}

/// Put the caret at `cursor`. iced 0.14's `Content::move_to` sets the caret
/// and touches the selection only when the cursor CARRIES one, so the plain
/// press after a drag left the old anchor standing — and the widget's next
/// pointer move read as a drag from it. `Move` is the one `Content` action
/// that drops a selection; the exact caret follows it.
pub fn move_to(document: &mut Content, cursor: Cursor) {
    let drops_selection = cursor.selection.is_none() && document.cursor().selection.is_some();
    if drops_selection {
        document.perform(text_editor::Action::Move(Motion::Left));
    }
    document.move_to(cursor);
}

pub fn apply_composer_event(document: Content, event: ComposerEvent) -> Content {
    let mut document = document;
    match event {
        ComposerEvent::Apply(RichAction::Edit(action)) => document.perform(action),
        ComposerEvent::Apply(RichAction::MoveTo(cursor)) => move_to(&mut document, cursor),
        ComposerEvent::Mark(kind) => return composer_toggle_mark(document, kind),
        ComposerEvent::Submit => {}
    }
    document
}

/// The composer toolbar's insertion: wrap the selection in `kind`'s markers,
/// or insert an empty marker pair and park the cursor inside it. The markers
/// are the SAME grammar `inline_marks` previews and the renderer parses, so
/// the button and the typed fence produce identical messages.
pub fn composer_toggle_mark(mut document: Content, kind: String) -> Content {
    let (open, close) = match kind.as_str() {
        "bold" => ("**", "**"),
        "italic" => ("_", "_"),
        "code" => ("```\n", "\n```"),
        "quote" => ("> ", ""),
        _ => return document,
    };
    let selected = document.selection().unwrap_or_default();
    let marked = format!("{open}{selected}{close}");
    document.perform(text_editor::Action::Edit(text_editor::Edit::Paste(
        std::sync::Arc::new(marked),
    )));
    if selected.is_empty() {
        for _ in 0..close.chars().count() {
            document.perform(text_editor::Action::Move(text_editor::Motion::Left));
        }
    }
    document
}

/// The composer's formatting shortcuts, classified AT THE WIDGET via its
/// `on_chord` route: Cmd/Ctrl+B bold, Cmd/Ctrl+I italic, Cmd/Ctrl+Shift+C
/// code block, Cmd/Ctrl+Shift+9 quote — Slack's own table. These presses
/// resolve to no binding (the bubble contract for application chords), so
/// the route sees exactly them; a claimed chord becomes a
/// [`ComposerEvent::Mark`] on the composer that has the caret, and every
/// other chord bubbles on to the app subscription untouched.
fn composer_chord(press: &KeyPress) -> Option<ComposerEvent> {
    use iced::keyboard::key::{Code, Physical};
    if !press.modifiers.command() {
        return None;
    }
    let shifted = press.modifiers.shift();
    let mark = match press.physical_key {
        Physical::Code(Code::KeyB) if !shifted => "bold",
        Physical::Code(Code::KeyI) if !shifted => "italic",
        Physical::Code(Code::KeyC) if shifted => "code",
        Physical::Code(Code::Digit9) if shifted => "quote",
        _ => return None,
    };
    Some(ComposerEvent::Mark(mark.to_owned()))
}

pub fn rich_composer(
    document: &Content,
    hint: String,
    disabled: bool,
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
        .style(composer_style)
        .key_binding(composer_key_binding);
    if disabled {
        return editor.into();
    }
    editor.on_action(classify).on_chord(composer_chord).into()
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

/// Enter or newline, decided AT THE PRESS from its live modifiers — the
/// widget's `key_binding` seam (ducktape-ui#601), which retired the lagged
/// `shift_held` mirror and the `keyboard modifiers` subscription that fed it.
/// ⇧↵ becomes a newline paste HERE, so the only press that can reach
/// [`classify`] as `Edit::Enter` is a plain Enter — the submit.
fn composer_key_binding(press: &KeyPress) -> Option<Binding<Edit>> {
    let enter = matches!(press.key, Key::Named(Named::Enter));
    if !enter {
        return default_key_binding(press);
    }
    if press.modifiers.shift() {
        return Some(Binding::Custom(Edit::Paste(std::sync::Arc::new(
            "\n".to_owned(),
        ))));
    }
    Some(Binding::Enter)
}

/// Plain Enter is the submit; every other action is an edit to apply.
/// Sound without a modifier argument because [`composer_key_binding`] already
/// rewrote ⇧↵ into a newline paste at the press.
fn classify(action: RichAction) -> ComposerEvent {
    let plain_enter = matches!(
        action,
        RichAction::Edit(text_editor::Action::Edit(text_editor::Edit::Enter))
    );
    if plain_enter {
        return ComposerEvent::Submit;
    }
    ComposerEvent::Apply(action)
}

fn composer_style(theme: &iced::Theme, status: text_editor::Status) -> text_editor::Style {
    let ink = composer_ink(theme);
    let disabled = matches!(status, text_editor::Status::Disabled);
    text_editor::Style {
        background: Color::TRANSPARENT.into(),
        // NO focus ring of its own: the editor mounts inside a bordered plate
        // (the r=12 composer card), and an inner rectangle on focus made the
        // input read as a separate component floating in its card.
        border: Border::default(),
        // THE PLACEHOLDER IS THE ONLY INK AN EMPTY COMPOSER HAS, so it is the
        // only place a disabled one can say so: `disabled` here is the absence
        // of `on_action`, the plate around it is unconditional, and an empty
        // document has no `value` to dim — so a dead composer was pixel-for-
        // pixel a ready one, and the hover cursor was the whole tell. `muted`
        // is also the ink the composer's own `⇧↵` hint reads at.
        placeholder: if disabled { ink.hint } else { ink.muted },
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

/// A renderer-parity inline mark, plus the marker glyphs themselves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Inline {
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
pub fn inline_marks(line: &str) -> Vec<(Range<usize>, Inline)> {
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
    use text_editor::Position;

    /// A press after a drag is a fresh caret, not a drag from the old anchor:
    /// the widget sends `MoveTo { selection: None }`, and iced's own `move_to`
    /// would have kept the standing selection.
    #[test]
    fn a_caret_move_without_a_selection_drops_the_standing_one() {
        let at = |column| Position { line: 0, column };
        let mut document = Content::with_text("one two three");
        document.move_to(Cursor {
            position: at(7),
            selection: Some(at(0)),
        });
        assert_eq!(document.cursor().selection, Some(at(0)));

        move_to(
            &mut document,
            Cursor {
                position: at(4),
                selection: None,
            },
        );
        assert_eq!(
            document.cursor(),
            Cursor {
                position: at(4),
                selection: None,
            }
        );
    }

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

    /// A DEAD COMPOSER HAS TO LOOK DEAD, AND ON AN EMPTY ONE THE PLACEHOLDER IS
    /// THE ONLY INK THERE IS. `disabled` here is just the absence of
    /// `on_action`; the plate, the border and the shadow around it are
    /// unconditional and there is no `value` to dim — so a composer refusing
    /// every keystroke (mid channel switch, disconnected, no channel, any post
    /// refusal) was pixel-identical to a ready one, and the hover cursor was
    /// the whole tell.
    #[test]
    fn a_disabled_composer_reads_dimmer_than_a_ready_one() {
        for theme in [iced::Theme::Light, iced::Theme::Dark] {
            let ready = composer_style(&theme, text_editor::Status::Active);
            let dead = composer_style(&theme, text_editor::Status::Disabled);
            assert_ne!(
                ready.placeholder, dead.placeholder,
                "the invitation must not read the same in both states"
            );
            let ink = composer_ink(&theme);
            assert_eq!(ready.placeholder, ink.muted);
            assert_eq!(dead.placeholder, ink.hint);
        }
    }

    /// THE ENTER DECISION READS THE PRESS, NOT A MIRROR. `shift_held` lagged
    /// the key by a full event-loop turn, so a ⇧↵ chord whose two downs landed
    /// in one drain classified as Submit and POSTED the half-written message.
    /// `composer_key_binding` sees `press.modifiers` on the press itself:
    /// plain Enter stays the stock newline binding (which [`classify`] reads
    /// as Submit), ⇧↵ is rewritten into a newline paste at the widget, and
    /// every other key delegates to the editor's stock table.
    #[test]
    fn plain_enter_submits_and_shift_enter_edits() {
        let enter_press = |modifiers: iced::keyboard::Modifiers| KeyPress {
            key: Key::Named(Named::Enter),
            modified_key: Key::Named(Named::Enter),
            physical_key: iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::Enter),
            modifiers,
            text: None,
            status: text_editor::Status::Focused { is_hovered: false },
        };

        // Plain Enter keeps the stock binding, and classify reads it as the
        // submit — the widget publishes `Edit::Enter` for no other press.
        let plain = composer_key_binding(&enter_press(iced::keyboard::Modifiers::empty()));
        assert!(matches!(plain, Some(Binding::Enter)));
        let enter = RichAction::Edit(text_editor::Action::Edit(text_editor::Edit::Enter));
        assert_eq!(classify(enter), ComposerEvent::Submit);

        // ⇧↵ becomes a newline PASTE at the press, so it reaches classify as
        // an ordinary edit and breaks the line instead of posting.
        let shifted = composer_key_binding(&enter_press(iced::keyboard::Modifiers::SHIFT))
            .expect("shift+enter binds");
        let Binding::Custom(edit) = shifted else {
            panic!("shift+enter must rewrite into a custom edit, got {shifted:?}");
        };
        let newline = RichAction::Edit(text_editor::Action::Edit(edit));
        let event = classify(newline);
        let ComposerEvent::Apply(_) = &event else {
            panic!("shift+enter must apply, not submit");
        };
        let mut document = Content::with_text("draft");
        document.perform(text_editor::Action::Move(text_editor::Motion::End));
        let document = apply_composer_event(document, event);
        assert_eq!(document.line_count(), 2, "⇧↵ breaks the line");

        // Any other key falls through to the editor's stock table.
        let typed = RichAction::Edit(text_editor::Action::Edit(text_editor::Edit::Insert('x')));
        assert_eq!(classify(typed.clone()), ComposerEvent::Apply(typed));
    }

    #[test]
    fn toolbar_marks_wrap_the_selection_or_park_the_cursor_inside() {
        // A selection is wrapped in place.
        let mut document = Content::with_text("ship it");
        document.perform(text_editor::Action::SelectAll);
        let document = composer_toggle_mark(document, "bold".into());
        assert_eq!(document.text().trim_end(), "**ship it**");

        // No selection: an empty pair is inserted and the cursor parks inside
        // it, so typing lands between the markers.
        let mut document = composer_toggle_mark(Content::new(), "italic".into());
        document.perform(text_editor::Action::Edit(text_editor::Edit::Insert('x')));
        assert_eq!(document.text().trim_end(), "_x_");

        // The block marks are the renderer's own fences.
        let mut document = Content::with_text("let x = 1;");
        document.perform(text_editor::Action::SelectAll);
        let document = composer_toggle_mark(document, "code".into());
        assert_eq!(document.text().trim_end(), "```\nlet x = 1;\n```");

        // An unknown kind changes nothing.
        let document = composer_toggle_mark(Content::with_text("keep"), "sparkle".into());
        assert_eq!(document.text().trim_end(), "keep");
    }

    #[test]
    fn mark_chords_follow_slacks_table_at_the_widget() {
        use iced::keyboard::key::{Code, Physical};
        use iced::keyboard::{Key, Modifiers};
        // The route the widget offers a press that resolved to no binding
        // (ducktape-ui#711). No gate argument any more: the chord reaches the
        // composer that HAS the caret, so there is nothing left to guess.
        let chord = |code, modifiers| {
            composer_chord(&KeyPress {
                key: Key::Unidentified,
                modified_key: Key::Unidentified,
                physical_key: Physical::Code(code),
                modifiers,
                text: None,
                status: text_editor::Status::Focused { is_hovered: false },
            })
        };
        let mark = |code, modifiers| match chord(code, modifiers) {
            Some(ComposerEvent::Mark(kind)) => kind,
            _ => String::new(),
        };
        assert_eq!(mark(Code::KeyB, Modifiers::COMMAND), "bold");
        assert_eq!(mark(Code::KeyI, Modifiers::COMMAND), "italic");
        assert_eq!(
            mark(Code::KeyC, Modifiers::COMMAND | Modifiers::SHIFT),
            "code"
        );
        assert_eq!(
            mark(Code::Digit9, Modifiers::COMMAND | Modifiers::SHIFT),
            "quote"
        );
        // Plain Cmd+C is copy, not code; plain typing is an ordinary edit and
        // never reaches this route at all. An unclaimed chord bubbles on.
        assert!(chord(Code::KeyC, Modifiers::COMMAND).is_none());
        assert!(chord(Code::KeyB, Modifiers::default()).is_none());
    }

    #[test]
    fn a_marked_chord_applies_like_any_other_composer_event() {
        // The claimed chord rides the ordinary event route into the
        // instance's local handler, where it wraps the selection.
        let document = Content::with_text("draft");
        let document = apply_composer_event(document, ComposerEvent::Mark("bold".into()));
        assert_eq!(document.text().trim_end(), "****draft");
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
}
