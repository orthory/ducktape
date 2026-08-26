//! A text editor whose rendering and input geometry share one rich-text layout.

use iced::advanced::input_method;
use iced::advanced::text::{self, Renderer as _, Text};
use iced::advanced::widget::operation;
use iced::advanced::widget::{self, tree};
use iced::advanced::{
    Clipboard, InputMethod, Layout, Renderer as _, Shell, Widget, layout, mouse, renderer,
};
use iced::alignment;
use iced::keyboard;
use iced::widget::text_editor::{self, Binding, Content, Cursor, Edit, Position};
use iced::{
    Element, Event, Font, Length, Padding, Pixels, Point, Rectangle, Size, Theme, Vector, window,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(test)]
use iced::Color;
#[cfg(test)]
use iced::advanced::graphics::text::Paragraph as GraphicsParagraph;
#[cfg(test)]
use iced::advanced::text::Paragraph as _;
#[cfg(test)]
use iced::keyboard::key;
#[cfg(test)]
use iced::widget::text_editor::Motion;
#[cfg(test)]
use std::ops::Range;

#[path = "rich_text_editor/affordance.rs"]
mod affordance;
use affordance::{
    DRAG_THRESHOLD, GutterDrag, MenuColors, draw_drop_indicator, draw_gutter, draw_margin_mark,
    draw_margin_tip, draw_menu, gutter_buttons, margin_mark_bounds, margin_tip_bounds, menu_panel,
    menu_row_at, snap_boundary,
};
pub use affordance::{
    EditorMenu, GUTTER_WIDTH, GutterButton, MARGIN_WIDTH, MenuAnchor, MenuEvent, MenuItem,
};
#[path = "rich_text_editor/keyboard.rs"]
mod keyboard_input;
pub use keyboard_input::default_key_binding;
use keyboard_input::*;
#[path = "rich_text_editor/composition.rs"]
mod composition;
use composition::*;
#[path = "rich_text_editor/document.rs"]
mod document;
pub use document::Format;
use document::*;
#[path = "rich_text_editor/movement.rs"]
mod movement;
#[path = "rich_text_editor/paint.rs"]
mod paint;
use paint::*;
#[path = "rich_text_editor/pointer.rs"]
mod pointer;
use pointer::*;

type FormatFn<'a, H> = dyn Fn(&<H as text::Highlighter>::Highlight) -> Format + 'a;
type StyleFn<'a> = dyn Fn(&Theme, text_editor::Status) -> text_editor::Style + 'a;

/// Caller-owned identity for the text stored in an editor [`Content`].
///
/// Two equal versions must always produce the same [`Content::text`]. Change
/// `document` when replacing the document and change `revision` after every
/// successful text mutation. Cursor and selection changes keep the same
/// version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContentVersion {
    document: u64,
    revision: u64,
}

/// The logical-line span replaced by one content revision.
///
/// The span is bound to an exact [`ContentVersion`] transition. The editor
/// only uses it when `from` is the version that produced the cached layout and
/// `to` is the version passed to [`RichTextEditor::new`]. A
/// character edit within one line replaces one line with one line; splitting
/// one line replaces one line with two; joining two lines replaces two lines
/// with one.
///
/// A skipped render, stale transition, document replacement, overflowing
/// bound, or inconsistent resulting line count falls back to exact line
/// diffing. The caller must still ensure that lines outside an accepted span
/// are unchanged between `from` and `to`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EditorChange {
    from: ContentVersion,
    to: ContentVersion,
    /// The first logical line affected by the edit.
    pub first_changed_line: usize,
    /// The number of lines replaced in the previous revision.
    pub removed_lines: usize,
    /// The number of lines present in the new revision at the same position.
    pub inserted_lines: usize,
}

impl EditorChange {
    /// Creates a logical-line replacement hint.
    pub const fn new(
        from: ContentVersion,
        to: ContentVersion,
        first_changed_line: usize,
        removed_lines: usize,
        inserted_lines: usize,
    ) -> Self {
        Self {
            from,
            to,
            first_changed_line,
            removed_lines,
            inserted_lines,
        }
    }

    /// Returns the content version before the edit.
    pub const fn from(self) -> ContentVersion {
        self.from
    }

    /// Returns the content version after the edit.
    pub const fn to(self) -> ContentVersion {
        self.to
    }
}

impl ContentVersion {
    /// Creates a document-scoped content version.
    pub const fn new(document: u64, revision: u64) -> Self {
        Self { document, revision }
    }

    /// Returns the identity of the containing document.
    pub const fn document(self) -> u64 {
        self.document
    }

    /// Returns the revision of the document text.
    pub const fn revision(self) -> u64 {
        self.revision
    }
}

/// An edit produced by a [`RichTextEditor`].
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Apply a regular Iced text editor action.
    Edit(text_editor::Action),
    /// Move the content cursor to a position measured in the rich layout.
    MoveTo(Cursor),
}

/// A press interceptor over a rich-layout source position — `Some` consumes
/// the press.
type LinePressFn<'a, Message> = dyn Fn(&str, Position) -> Option<Message> + 'a;

/// The key-binding override — decides every press; `None` leaves the key
/// unhandled so it bubbles to the application.
type KeyBindingFn<'a> = dyn Fn(&text_editor::KeyPress) -> Option<Binding<Edit>> + 'a;

/// The bubbling-chord route — consulted when a key press resolves to no
/// binding; a message CAPTURES the press instead of letting it bubble.
type ChordFn<'a, Message> = dyn Fn(&text_editor::KeyPress) -> Option<Message> + 'a;

/// The hover-gutter route — `None` hides the gutter for that line.
type GutterFn<'a, Message> = dyn Fn(usize, GutterButton) -> Option<Message> + 'a;

/// The handle-drag drop route: (grabbed line, boundary line to land before).
type GutterDropFn<'a, Message> = dyn Fn(usize, usize) -> Option<Message> + 'a;

/// The anchored-menu route.
type MenuFn<'a, Message> = dyn Fn(MenuEvent) -> Message + 'a;

/// The right-margin mark route — the source line whose mark was pressed.
type MarginPressFn<'a, Message> = dyn Fn(usize) -> Message + 'a;

/// An editable rich-text surface.
///
/// Unlike [`iced::widget::TextEditor`], this widget shapes each highlighted
/// logical line once and uses the same cached line paragraphs for painting,
/// hit testing, selections, vertical movement, and IME placement.
pub struct RichTextEditor<'a, Highlighter, Message>
where
    Highlighter: text::Highlighter,
{
    id: Option<widget::Id>,
    content: &'a Content,
    content_version: ContentVersion,
    change_hint: Option<EditorChange>,
    placeholder: Option<String>,
    font: Option<Font>,
    text_size: Option<Pixels>,
    line_height: text::LineHeight,
    width: Length,
    height: Length,
    min_height: f32,
    max_height: f32,
    padding: Padding,
    end_padding: f32,
    wrapping: text::Wrapping,
    on_action: Option<Box<dyn Fn(Action) -> Message + 'a>>,
    key_binding: Option<Box<KeyBindingFn<'a>>>,
    on_chord: Option<Box<ChordFn<'a, Message>>>,
    on_line_press: Option<Box<LinePressFn<'a, Message>>>,
    on_gutter: Option<Box<GutterFn<'a, Message>>>,
    drop_boundaries: Vec<usize>,
    on_gutter_drop: Option<Box<GutterDropFn<'a, Message>>>,
    /// `(line, thread count)` — the count the chip spells. See
    /// [`margin_mark_caption`].
    margin_marks: Vec<(usize, usize)>,
    margin_label: String,
    on_margin_press: Option<Box<MarginPressFn<'a, Message>>>,
    menu: Option<EditorMenu>,
    on_menu: Option<Box<MenuFn<'a, Message>>>,
    focus_enabled: bool,
    highlighter_settings: Highlighter::Settings,
    format: Box<FormatFn<'a, Highlighter>>,
    format_key: u64,
    mouse_interaction: Option<Box<InteractionFn<'a>>>,
    style: Box<StyleFn<'a>>,
}

impl<'a, Message> RichTextEditor<'a, text::highlighter::PlainText, Message> {
    /// Creates a plain rich editor backed by `content` at `content_version`.
    pub fn new(content: &'a Content, content_version: ContentVersion) -> Self {
        Self {
            id: None,
            content,
            content_version,
            change_hint: None,
            placeholder: None,
            font: None,
            text_size: None,
            line_height: text::LineHeight::default(),
            width: Length::Fill,
            height: Length::Shrink,
            min_height: 0.0,
            max_height: f32::INFINITY,
            padding: Padding::new(5.0),
            end_padding: 0.0,
            wrapping: text::Wrapping::default(),
            on_action: None,
            key_binding: None,
            on_chord: None,
            focus_enabled: true,
            highlighter_settings: (),
            format: Box::new(|_| Format::default()),
            format_key: 0,
            mouse_interaction: None,
            on_line_press: None,
            on_gutter: None,
            drop_boundaries: Vec::new(),
            on_gutter_drop: None,
            margin_marks: Vec::new(),
            margin_label: String::new(),
            on_margin_press: None,
            menu: None,
            on_menu: None,
            style: Box::new(text_editor::default),
        }
    }
}

impl<'a, Highlighter, Message> RichTextEditor<'a, Highlighter, Message>
where
    Highlighter: text::Highlighter,
{
    /// Sets the widget identity used by focus operations.
    pub fn id(mut self, id: impl Into<widget::Id>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Sets the placeholder shown for an empty document.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Sets the width.
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height.
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the minimum height.
    pub fn min_height(mut self, height: impl Into<Pixels>) -> Self {
        self.min_height = height.into().0;
        self
    }

    /// Lets the document scroll past its last line by this much, so the end
    /// of a long note can sit well above the bottom edge while the clip still
    /// runs to that edge. Unlike bottom `padding`, it never shortens the
    /// visible text area.
    pub fn end_padding(mut self, padding: impl Into<Pixels>) -> Self {
        self.end_padding = padding.into().0.max(0.0);
        self
    }

    /// Sets the maximum height.
    pub fn max_height(mut self, height: impl Into<Pixels>) -> Self {
        self.max_height = height.into().0;
        self
    }

    /// Sets the default font.
    pub fn font(mut self, font: impl Into<Font>) -> Self {
        self.font = Some(font.into());
        self
    }

    /// Sets the default text size.
    pub fn size(mut self, size: impl Into<Pixels>) -> Self {
        self.text_size = Some(size.into());
        self
    }

    /// Sets the default line height.
    pub fn line_height(mut self, line_height: impl Into<text::LineHeight>) -> Self {
        self.line_height = line_height.into();
        self
    }

    /// Sets the inner padding.
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the wrapping strategy.
    pub fn wrapping(mut self, wrapping: text::Wrapping) -> Self {
        self.wrapping = wrapping;
        self
    }

    /// Enables editing and maps editor actions to application messages.
    pub fn on_action(mut self, on_action: impl Fn(Action) -> Message + 'a) -> Self {
        self.on_action = Some(Box::new(on_action));
        self
    }

    /// Overrides how key presses map to editor bindings, the way iced's stock
    /// `text_editor` allows.
    ///
    /// The closure fully decides each press: return `None` to leave the key
    /// unhandled (it bubbles to the application), or delegate to
    /// [`default_key_binding`] for the editor's stock behavior. The closure
    /// sees the press itself — including its live modifiers — so decisions
    /// like parting plain Enter from Shift+Enter happen at the widget, not
    /// against a lagged modifier mirror.
    ///
    /// ```
    /// use iced::keyboard::{Key, key};
    /// use iced::widget::text_editor::Content;
    /// use ui_lang_runtime::rich_text_editor::default_key_binding;
    /// use ui_lang_runtime::{ContentVersion, RichTextEditor};
    ///
    /// let content = Content::new();
    /// let editor: RichTextEditor<'_, _, ()> =
    ///     RichTextEditor::new(&content, ContentVersion::new(1, 0)).key_binding(|press| {
    ///         let plain_enter = matches!(press.key, Key::Named(key::Named::Enter))
    ///             && !press.modifiers.shift();
    ///         if plain_enter {
    ///             None // the application decides what plain Enter means
    ///         } else {
    ///             default_key_binding(press)
    ///         }
    ///     });
    /// ```
    pub fn key_binding(
        mut self,
        key_binding: impl Fn(&text_editor::KeyPress) -> Option<Binding<Edit>> + 'a,
    ) -> Self {
        self.key_binding = Some(Box::new(key_binding));
        self
    }

    /// Routes a key press that resolved to NO binding — the presses
    /// [`default_key_binding`] deliberately lets bubble, application command
    /// chords first among them. Returning a message consumes the press and
    /// publishes it like an action, so a composer can catch its own
    /// formatting chords (Cmd/Ctrl+B and friends) without giving up the
    /// bubble contract for everything else; `None` bubbles as before.
    pub fn on_chord(
        mut self,
        on_chord: impl Fn(&text_editor::KeyPress) -> Option<Message> + 'a,
    ) -> Self {
        self.on_chord = Some(Box::new(on_chord));
        self
    }

    /// Keeps the editor's internal focus and drag state aligned with the
    /// surrounding view focus.
    pub fn focus_enabled(mut self, enabled: bool) -> Self {
        self.focus_enabled = enabled;
        self
    }

    /// Describes the logical lines replaced since the previous content
    /// version rendered by this widget.
    ///
    /// This optional optimization avoids discovering the unchanged line
    /// prefix and suffix after a text mutation. Its `from` and `to` versions
    /// must exactly match the cached and current [`ContentVersion`]. Initial
    /// layout, skipped revisions, document replacement, unchanged versions,
    /// active IME composition, and structurally invalid spans use exact
    /// diffing instead.
    pub fn change_hint(mut self, change: EditorChange) -> Self {
        self.change_hint = Some(change);
        self
    }

    /// Uses a custom highlighter and rich formatting function.
    ///
    /// `format_key` must change whenever captured values that affect formatting
    /// change. It lets the widget reuse its shaped line paragraphs otherwise.
    pub fn highlight_with<H>(
        self,
        settings: H::Settings,
        format_key: u64,
        format: impl Fn(&H::Highlight) -> Format + 'a,
    ) -> RichTextEditor<'a, H, Message>
    where
        H: text::Highlighter,
    {
        RichTextEditor {
            id: self.id,
            content: self.content,
            content_version: self.content_version,
            change_hint: self.change_hint,
            placeholder: self.placeholder,
            font: self.font,
            text_size: self.text_size,
            line_height: self.line_height,
            width: self.width,
            height: self.height,
            min_height: self.min_height,
            max_height: self.max_height,
            padding: self.padding,
            end_padding: self.end_padding,
            wrapping: self.wrapping,
            on_action: self.on_action,
            key_binding: self.key_binding,
            on_chord: self.on_chord,
            focus_enabled: self.focus_enabled,
            highlighter_settings: settings,
            format: Box::new(format),
            format_key,
            mouse_interaction: self.mouse_interaction,
            on_line_press: self.on_line_press,
            on_gutter: self.on_gutter,
            drop_boundaries: self.drop_boundaries,
            on_gutter_drop: self.on_gutter_drop,
            margin_marks: self.margin_marks,
            margin_label: self.margin_label,
            on_margin_press: self.on_margin_press,
            menu: self.menu,
            on_menu: self.on_menu,
            style: self.style,
        }
    }

    /// Intercepts a left press on a rich-layout source position. Returning a
    /// message CONSUMES the press — no caret move, no focus change — which is
    /// what a checkbox tick or a link open wants; `None` falls through to the
    /// ordinary click.
    pub fn on_line_press(mut self, press: impl Fn(&str, Position) -> Option<Message> + 'a) -> Self {
        self.on_line_press = Some(Box::new(press));
        self
    }

    /// Shows the hover gutter — "+" and the dots handle beside the hovered
    /// line. The closure decides per line: `None` hides the gutter there
    /// (a title line), `Some` is the message a press publishes. A gutter
    /// press consumes the press like a line press — no caret move, no focus
    /// steal. The host must reserve [`GUTTER_WIDTH`] of left padding.
    pub fn on_gutter(
        mut self,
        gutter: impl Fn(usize, GutterButton) -> Option<Message> + 'a,
    ) -> Self {
        self.on_gutter = Some(Box::new(gutter));
        self
    }

    /// Enables drag-to-reorder on the handle: `boundaries` are the source
    /// lines a dragged block may land BEFORE (append the line count for the
    /// end of the document), and the closure maps (grabbed line, boundary) to
    /// the drop message. A handle press becomes a drag once the pointer
    /// moves past a threshold; released without movement it is the ordinary
    /// handle press.
    pub fn on_gutter_drop(
        mut self,
        boundaries: Vec<usize>,
        on_drop: impl Fn(usize, usize) -> Option<Message> + 'a,
    ) -> Self {
        self.drop_boundaries = boundaries;
        self.on_gutter_drop = Some(Box::new(on_drop));
        self
    }

    /// Marks source lines with a right-margin chip (a comment badge) and maps
    /// a press on one to a message. A mark press is its own gesture — no caret
    /// move, no focus steal. The host must reserve [`MARGIN_WIDTH`] of right
    /// padding for the mark column.
    pub fn margin_marks(
        mut self,
        marks: Vec<(usize, usize)>,
        press: impl Fn(usize) -> Message + 'a,
    ) -> Self {
        self.margin_marks = marks;
        self.on_margin_press = Some(Box::new(press));
        self
    }

    /// Names the margin chip: the tip that appears beside a hovered mark.
    ///
    /// A chip drawn INSIDE this widget cannot carry a host tooltip, so without
    /// this the only thing telling a reader what it does is the pointer
    /// cursor. Empty (the default) draws no tip.
    pub fn margin_label(mut self, label: impl Into<String>) -> Self {
        self.margin_label = label.into();
        self
    }

    /// Shows an anchored dropdown described by the application each frame.
    /// While one is up (and [`Self::on_menu`] is set) the editor intercepts
    /// Up/Down/Enter/Tab/Escape for it instead of editing.
    pub fn menu(mut self, menu: Option<EditorMenu>) -> Self {
        self.menu = menu.filter(|menu| !menu.items.is_empty());
        self
    }

    /// Maps anchored-menu events to application messages.
    pub fn on_menu(mut self, on_menu: impl Fn(MenuEvent) -> Message + 'a) -> Self {
        self.on_menu = Some(Box::new(on_menu));
        self
    }

    /// Selects the pointer shown over a rich-layout source position.
    pub fn mouse_interaction(
        mut self,
        interaction: impl Fn(&str, Position) -> mouse::Interaction + 'a,
    ) -> Self {
        self.mouse_interaction = Some(Box::new(interaction));
        self
    }

    /// Sets the surface style.
    pub fn style(
        mut self,
        style: impl Fn(&Theme, text_editor::Status) -> text_editor::Style + 'a,
    ) -> Self {
        self.style = Box::new(style);
        self
    }

    fn status(&self, state: &State<Highlighter>, is_hovered: bool) -> text_editor::Status {
        if self.on_action.is_none() {
            text_editor::Status::Disabled
        } else if state.focus.is_some() {
            text_editor::Status::Focused { is_hovered }
        } else if is_hovered {
            text_editor::Status::Hovered
        } else {
            text_editor::Status::Active
        }
    }

    fn interaction_at(&self, state: &State<Highlighter>, point: Point) -> mouse::Interaction {
        interaction_at(
            self.content,
            &state.document,
            state.composition.as_ref(),
            self.mouse_interaction.as_deref(),
            point,
        )
    }

    fn input_method<'b>(
        &self,
        state: &'b State<Highlighter>,
        layout: Layout<'_>,
    ) -> InputMethod<&'b str> {
        let Some(Focus {
            is_window_focused: true,
            ..
        }) = state.focus.as_ref()
        else {
            return InputMethod::Disabled;
        };

        let text_bounds = layout.bounds().shrink(self.padding);
        let position = state
            .composition
            .as_ref()
            .map_or(self.content.cursor().position, |composition| {
                composition.cursor
            });
        let caret = state.document.caret(position);
        let translation = text_bounds.position() - Point::ORIGIN - Vector::new(0.0, state.scroll);
        let cursor = caret + translation;

        InputMethod::Enabled {
            cursor,
            purpose: input_method::Purpose::Normal,
            // The preedit is already shaped into the editor document. Passing it
            // to iced_winit as well would draw a second overlay with the default
            // font and a different baseline.
            preedit: None,
        }
    }

    /// The anchored menu's absolute panel rectangle, when one is up. The
    /// menu waits out an IME composition — its keys belong to the IME.
    fn menu_panel_in(
        &self,
        state: &State<Highlighter>,
        text_bounds: Rectangle,
    ) -> Option<Rectangle> {
        let menu = self.menu.as_ref()?;
        if state.composition.is_some() || self.on_menu.is_none() {
            return None;
        }
        let position = match menu.anchor {
            MenuAnchor::Caret => self.content.cursor().position,
            MenuAnchor::Line(line) => Position { line, column: 0 },
        };
        let anchor = state.document.caret(position)
            + (text_bounds.position() - Point::ORIGIN - Vector::new(0.0, state.scroll));
        Some(menu_panel(anchor, menu.items.len(), text_bounds))
    }

    /// The line the hover gutter rides.
    ///
    /// AN OPEN LINE-ANCHORED MENU OWNS IT. The "⋮⋮" that opened the menu is
    /// the menu's anchor, so it cannot slide onto whatever line the pointer
    /// wandered to next — that leaves the panel hanging off nothing and the
    /// handle marking a block the menu will not act on. Only when no menu is
    /// up does the pointer decide. The caret palette is keyboard-driven and
    /// anchors on the caret, which draws itself.
    fn gutter_line(&self, state: &State<Highlighter>) -> Option<usize> {
        self.menu_line().or(state.hover_line)
    }

    /// The line an open menu is anchored to, if it is anchored to one.
    fn menu_line(&self) -> Option<usize> {
        let menu = self.menu.as_ref().filter(|_| self.on_menu.is_some())?;
        match menu.anchor {
            MenuAnchor::Line(line) => Some(line),
            MenuAnchor::Caret => None,
        }
    }

    /// The absolute column-0 caret row of `line` — what the gutter centers on.
    fn gutter_row(
        &self,
        state: &State<Highlighter>,
        text_bounds: Rectangle,
        line: usize,
    ) -> Rectangle {
        state.document.caret(Position { line, column: 0 })
            + (text_bounds.position() - Point::ORIGIN - Vector::new(0.0, state.scroll))
    }

    /// The absolute y of a drop boundary — the top of that line, or the
    /// bottom of the document for the past-the-end boundary.
    fn boundary_y(
        &self,
        state: &State<Highlighter>,
        boundary: usize,
        text_bounds: Rectangle,
    ) -> f32 {
        let document_y = state
            .document
            .lines
            .get(boundary)
            .map_or(state.document.height, |line| line.top);
        text_bounds.y - state.scroll + document_y
    }

    /// Every drop boundary as a `(boundary, absolute y)` snap candidate.
    fn drop_candidates<'b>(
        &'b self,
        state: &'b State<Highlighter>,
        text_bounds: Rectangle,
    ) -> impl Iterator<Item = (usize, f32)> + 'b {
        self.drop_boundaries
            .iter()
            .map(move |&boundary| (boundary, self.boundary_y(state, boundary, text_bounds)))
    }
}

struct State<Highlighter>
where
    Highlighter: text::Highlighter,
{
    focus: Option<Focus>,
    preedit: Option<input_method::Preedit>,
    shaped_preedit: Option<input_method::Preedit>,
    composition: Option<CompositionLayout>,
    document: DocumentLayout,
    pending_ime_commit: PendingImeCommit,
    pointer: PointerState,
    hover_line: Option<usize>,
    gutter_drag: Option<GutterDrag>,
    highlighter: Highlighter,
    settings: Highlighter::Settings,
    source: String,
    source_line_map: TextLines,
    /// Lines at or past this index still carry the highlighting of an earlier
    /// pass. Scrolling one into view has to re-open a shaping pass.
    highlight_valid_until: usize,
    /// Lines below this index deferred a draw-only format delta — stale
    /// colour above the viewport, never stale geometry. Scrolling up to one
    /// re-opens a pass, mirroring `highlight_valid_until` downward.
    format_stale_before: usize,
    content_version: Option<ContentVersion>,
    width: f32,
    font: Font,
    text_size: Pixels,
    line_height: text::LineHeight,
    wrapping: text::Wrapping,
    format_key: u64,
    content_height: f32,
    end_padding: f32,
    viewport_height: f32,
    scroll: f32,
    preferred_x: Option<f32>,
    last_cursor: Cursor,
    #[cfg(test)]
    metrics: LayoutMetrics,
}

#[cfg(test)]
#[derive(Debug, Default, PartialEq, Eq)]
struct LayoutMetrics {
    full_text_materializations: usize,
    materialized_source_bytes: usize,
    composition_display_strings: usize,
    composition_display_bytes: usize,
    mapping_line_comparisons: usize,
    styled_signature_comparisons: usize,
    newly_owned_styled_texts: usize,
    newly_owned_styled_text_bytes: usize,
    line_vector_slots_prepared: usize,
    rebuilt_lines: usize,
    shaped_paragraphs: usize,
    highlighted_lines: usize,
    accepted_change_hints: usize,
    rejected_change_hints: usize,
}

#[derive(Debug, Clone)]
struct Focus {
    updated_at: Instant,
    now: Instant,
    is_window_focused: bool,
}

impl Focus {
    const BLINK_INTERVAL_MILLIS: u128 = 500;

    fn now() -> Self {
        let now = Instant::now();
        Self {
            updated_at: now,
            now,
            is_window_focused: true,
        }
    }

    fn is_cursor_visible(&self) -> bool {
        self.is_window_focused
            && ((self.now - self.updated_at).as_millis() / Self::BLINK_INTERVAL_MILLIS)
                .is_multiple_of(2)
    }
}

impl<H> operation::Focusable for State<H>
where
    H: text::Highlighter,
{
    fn is_focused(&self) -> bool {
        self.focus.is_some()
    }

    fn focus(&mut self) {
        self.focus = Some(Focus::now());
    }

    fn unfocus(&mut self) {
        self.focus = None;
        self.preedit = None;
        self.pending_ime_commit.clear();
        self.pointer.clear();
        self.gutter_drag = None;
    }
}

impl<Highlighter, Message> Widget<Message, Theme, iced::Renderer>
    for RichTextEditor<'_, Highlighter, Message>
where
    Highlighter: text::Highlighter,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State<Highlighter>>()
    }

    fn state(&self) -> tree::State {
        let font = Font::DEFAULT;
        let text_size = Pixels(16.0);
        tree::State::new(State {
            focus: None,
            preedit: None,
            shaped_preedit: None,
            composition: None,
            document: DocumentLayout::default(),
            pending_ime_commit: PendingImeCommit::default(),
            pointer: PointerState::default(),
            hover_line: None,
            gutter_drag: None,
            highlighter: Highlighter::new(&self.highlighter_settings),
            settings: self.highlighter_settings.clone(),
            source: String::new(),
            source_line_map: TextLines::empty(),
            highlight_valid_until: 0,
            format_stale_before: 0,
            content_version: None,
            width: 0.0,
            font,
            text_size,
            line_height: self.line_height,
            wrapping: self.wrapping,
            format_key: u64::MAX,
            content_height: 0.0,
            end_padding: 0.0,
            viewport_height: 0.0,
            scroll: 0.0,
            preferred_x: None,
            last_cursor: self.content.cursor(),
            #[cfg(test)]
            metrics: LayoutMetrics::default(),
        })
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let limits = limits
            .width(self.width)
            .height(self.height)
            .min_height(self.min_height)
            .max_height(self.max_height);
        let maximum = limits.max();
        let inner_width = (maximum.width - self.padding.x()).max(1.0);
        let viewport_height = (maximum.height - self.padding.y()).max(0.0);
        let font = self.font.unwrap_or_else(|| renderer.default_font());
        let text_size = self.text_size.unwrap_or_else(|| renderer.default_size());
        let cursor = self.content.cursor();
        let state = tree.state.downcast_mut::<State<Highlighter>>();

        let previous_content_version = state.content_version;
        let version_matches = previous_content_version == Some(self.content_version);
        let materialized_source = if version_matches {
            None
        } else {
            #[cfg(test)]
            {
                state.metrics.full_text_materializations += 1;
            }
            Some(self.content.text())
        };
        let source_changed = materialized_source.is_some();
        state.content_version = Some(self.content_version);
        if source_changed {
            let source = materialized_source.expect("changed content was materialized");
            let source_line_map = TextLines::parse(&source);
            #[cfg(test)]
            {
                state.metrics.materialized_source_bytes += source.len();
            }
            state.source = source;
            state.source_line_map = source_line_map;
        }

        // Caret-aware highlighters may reveal hidden syntax and change glyph
        // widths. Keep the exact layout that produced the press position for
        // the whole pointer gesture, otherwise the source anchor moves under
        // a stationary mouse as soon as the first caret action is applied.
        let settings_changed = state.settings != self.highlighter_settings;
        let settings_updated = settings_changed && !state.pointer.is_dragging();
        if settings_updated {
            state.highlighter.update(&self.highlighter_settings);
            state.settings.clone_from(&self.highlighter_settings);
        }

        let preedit_changed = state.shaped_preedit != state.preedit;
        // A line's shaped glyph run consults the available width only through
        // wrapping: every line is built with `align_x: Default`, so with
        // wrapping off, bounds width cannot move a glyph. A pure width change
        // then invalidates nothing — not the shaping, and not the line vector
        // either, so it must not even open a shaping pass. Turning wrapping
        // itself on or off is caught by the `wrapping` term below.
        let width_reflows = self.wrapping != text::Wrapping::None;
        let width_changed = width_reflows && state.width != inner_width;
        // Highlighting stops at the bottom of the viewport, so scrolling past
        // the validated region has to re-open a pass — nothing else would,
        // once the content and geometry have settled.
        const HIGHLIGHT_OVERSCAN_LINES: usize = 32;
        // An edit is followed by `reveal` below, which snaps the viewport to
        // the caret before anything is drawn — so the pass must cover the
        // viewport the reveal produces, not the one the previous frame left.
        // A scroll parked far from the caret would otherwise extend the pass
        // over every line between the edit and a region this frame never
        // shows. Predicted from pre-pass line tops, which any equal-line-count
        // edit preserves; the overscan absorbs the residual error, and a line
        // left beyond the mark re-opens a pass on the frame that shows it.
        // Composition is display-only, so the prediction skips preedit frames
        // and leaves their reveal target to the shaped composition.
        let scroll_for_highlight = if source_changed && state.preedit.is_none() {
            let caret = state.document.caret(cursor.position);
            let mut scroll = state.scroll;
            if caret.y < scroll {
                scroll = caret.y;
            } else if caret.y + caret.height > scroll + viewport_height {
                scroll = caret.y + caret.height - viewport_height;
            }
            scroll.clamp(
                0.0,
                (state.content_height + self.end_padding - viewport_height).max(0.0),
            )
        } else {
            state.scroll
        };
        // Clamped to the document: the validated mark can never exceed the
        // line count, so an unclamped window would read as "past validated"
        // forever on any document shorter than the overscan.
        let highlight_until = state
            .document
            .lines_above(scroll_for_highlight + viewport_height)
            .saturating_add(HIGHLIGHT_OVERSCAN_LINES)
            .min(state.source_line_map.len());
        // The window's first line, with the overscan mirrored upward: lines
        // above it may hold a deferred draw-only format delta, and scrolling
        // up to them re-opens a pass the same way `highlight_until` passing
        // the validated mark re-opens one downward.
        let viewport_start = state
            .document
            .lines_above(scroll_for_highlight)
            .saturating_sub(HIGHLIGHT_OVERSCAN_LINES);
        let needs_shape = source_changed
            || preedit_changed
            || settings_updated
            || highlight_until > state.highlight_valid_until
            || viewport_start < state.format_stale_before
            || width_changed
            || state.font != font
            || state.text_size != text_size
            || state.line_height != self.line_height
            || state.wrapping != self.wrapping
            || state.format_key != self.format_key;

        if needs_shape {
            let composition = state.preedit.as_ref().and_then(|preedit| {
                CompositionDocument::new(
                    cursor,
                    &state.source,
                    state.source_line_map.clone(),
                    preedit,
                )
            });
            #[cfg(test)]
            if let Some(composition) = composition.as_ref() {
                state.metrics.composition_display_strings += 1;
                state.metrics.composition_display_bytes += composition.display_bytes;
            }
            let shaped_lines = composition.as_ref().map_or_else(
                || Lines::new(&state.source, &state.source_line_map),
                |composition| Lines::new(&composition.display, &composition.layout.display_lines),
            );
            let geometry_changed = width_changed
                || state.font != font
                || state.text_size != text_size
                || state.line_height != self.line_height
                || state.wrapping != self.wrapping;
            let format_changed = state.format_key != self.format_key;

            let supplied_change = source_changed.then_some(self.change_hint).flatten();
            let accepted_change = supplied_change.filter(|change| {
                previous_content_version == Some(change.from())
                    && self.content_version == change.to()
                    && change.from().document() == change.to().document()
                    && state.shaped_preedit.is_none()
                    && state.preedit.is_none()
            });
            #[cfg(test)]
            let transition_hint_rejected = supplied_change.is_some() && accepted_change.is_none();
            let document_change = if let Some(change) = accepted_change {
                DocumentChange::Hint(change)
            } else if source_changed || preedit_changed {
                DocumentChange::Discover
            } else {
                DocumentChange::Unchanged
            };
            let update = state.document.update(
                shaped_lines,
                &mut state.highlighter,
                self.format.as_ref(),
                LineLayoutStyle {
                    width: inner_width,
                    font,
                    text_size,
                    line_height: self.line_height,
                    wrapping: self.wrapping,
                },
                DocumentUpdate {
                    change: document_change,
                    geometry_changed,
                    format_changed,
                    viewport_start,
                    stale_before: state.format_stale_before,
                },
                highlight_until,
            );
            state.highlight_valid_until = update.highlight_valid_until;
            state.format_stale_before = update.format_stale_before;
            #[cfg(test)]
            {
                state.metrics.mapping_line_comparisons += update.mapping_line_comparisons;
                state.metrics.styled_signature_comparisons += update.styled_signature_comparisons;
                state.metrics.newly_owned_styled_texts += update.newly_owned_styled_texts;
                state.metrics.newly_owned_styled_text_bytes += update.newly_owned_styled_text_bytes;
                state.metrics.line_vector_slots_prepared += update.line_vector_slots_prepared;
                state.metrics.rebuilt_lines += update.rebuilt_lines;
                state.metrics.shaped_paragraphs += update.shaped_paragraphs;
                state.metrics.highlighted_lines += update.highlighted_lines;
                state.metrics.accepted_change_hints += usize::from(update.change_hint_used);
                state.metrics.rejected_change_hints +=
                    usize::from(transition_hint_rejected || update.change_hint_rejected);
            }
            #[cfg(not(test))]
            let _ = update;
            state.content_height = state.document.height;
            state.composition = composition.map(|composition| composition.layout);
            state.shaped_preedit = state.preedit.clone();
            state.width = inner_width;
            state.font = font;
            state.text_size = text_size;
            state.line_height = self.line_height;
            state.wrapping = self.wrapping;
            state.format_key = self.format_key;
        }

        state.viewport_height = viewport_height;
        state.end_padding = self.end_padding;
        state.scroll = state.scroll.clamp(0.0, state.max_scroll());

        if source_changed || preedit_changed || settings_updated || cursor != state.last_cursor {
            let position = state
                .composition
                .as_ref()
                .map_or(cursor.position, |composition| composition.cursor);
            state.reveal(position);
            state.last_cursor = cursor;
        }

        let intrinsic_height = state.content_height + self.padding.y();
        let size = match self.height {
            Length::Shrink => limits.height(intrinsic_height).max(),
            _ => maximum,
        };

        layout::Node::new(size)
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State<Highlighter>>();
        let bounds = layout.bounds();

        if !self.focus_enabled && state.focus.is_some() {
            operation::Focusable::unfocus(state);
            shell.request_redraw();
        }

        if let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event
            && cursor.is_over(bounds)
        {
            let pixels = match delta {
                mouse::ScrollDelta::Lines { y, .. } => -y * state.text_size.0 * 3.0,
                mouse::ScrollDelta::Pixels { y, .. } => -y,
            };
            let next = (state.scroll + pixels).clamp(0.0, state.max_scroll());
            if next != state.scroll {
                state.scroll = next;
                shell.capture_event();
                // Highlighting only covers down to the old viewport bottom, so
                // revealing new lines needs a layout pass, not just a repaint.
                // That pass is bounded by the viewport, which is what makes
                // paying it per scroll step affordable.
                shell.invalidate_layout();
                shell.request_redraw();
            }
            return;
        }

        let Some(on_action) = self.on_action.as_ref() else {
            return;
        };

        match event {
            Event::Window(window::Event::Unfocused) => {
                if let Some(focus) = state.focus.as_mut() {
                    focus.is_window_focused = false;
                }
                // Losing the window closes any anchored menu — Escape and
                // click-away are gone with the focus, and the menu must not
                // sit stranded until the window returns.
                if self
                    .menu_panel_in(state, bounds.shrink(self.padding))
                    .is_some()
                {
                    let on_menu = self.on_menu.as_deref().expect("panel implies route");
                    shell.publish(on_menu(MenuEvent::Dismiss));
                    shell.request_redraw();
                }
            }
            Event::Window(window::Event::Focused) => {
                if let Some(focus) = state.focus.as_mut() {
                    focus.is_window_focused = true;
                    focus.updated_at = Instant::now();
                    shell.request_redraw();
                }
            }
            Event::Window(window::Event::RedrawRequested(now)) => {
                if let Some(focus) = state.focus.as_mut()
                    && focus.is_window_focused
                {
                    focus.now = *now;
                    let wait = Focus::BLINK_INTERVAL_MILLIS
                        - (focus.now - focus.updated_at).as_millis() % Focus::BLINK_INTERVAL_MILLIS;
                    shell.request_redraw_at(focus.now + Duration::from_millis(wait as u64));
                }
                shell.request_input_method(&self.input_method(state, layout));
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                state.pending_ime_commit.clear();
                if let Some(point) = cursor.position_in(bounds) {
                    let absolute = Point::new(bounds.x + point.x, bounds.y + point.y);
                    let text_bounds = bounds.shrink(self.padding);
                    // The anchored menu owns presses over its panel; a press
                    // anywhere else dismisses it and then lands normally.
                    if let Some(panel) = self.menu_panel_in(state, text_bounds) {
                        let menu = self.menu.as_ref().expect("panel implies menu");
                        let on_menu = self.on_menu.as_deref().expect("panel implies route");
                        if panel.contains(absolute) {
                            if let Some(row) = menu_row_at(panel, menu.items.len(), absolute)
                                && let Some(item) = menu.items.get(row)
                            {
                                shell.publish(on_menu(MenuEvent::Pick(item.tag.clone())));
                            }
                            // Typing continues where the pick landed.
                            state.focus = Some(Focus::now());
                            shell.capture_event();
                            shell.request_redraw();
                            return;
                        }
                        shell.publish(on_menu(MenuEvent::Dismiss));
                        shell.request_redraw();
                    }
                    // A gutter press is its own gesture — no caret move — but
                    // unlike a line press it FOCUSES the editor: "+" leaves a
                    // fresh caret to type into, the handle opens a menu whose
                    // keys the (focus-gated) keyboard route must reach.
                    if let Some(on_gutter) = self.on_gutter.as_deref()
                        && state.composition.is_none()
                        && let Some(line) = self.gutter_line(state)
                        && point.x < self.padding.left
                    {
                        let row = self.gutter_row(state, text_bounds, line);
                        let buttons = gutter_buttons(text_bounds, row);
                        if let Some((button, _)) = buttons
                            .iter()
                            .find(|(_, button_bounds)| button_bounds.contains(absolute))
                            && let Some(message) = on_gutter(line, *button)
                        {
                            // With drop wired, a handle press might be the
                            // start of a drag — its click message waits for
                            // the release to prove it stayed a click.
                            let handle_may_drag =
                                *button == GutterButton::Handle && self.on_gutter_drop.is_some();
                            if handle_may_drag {
                                state.gutter_drag = Some(GutterDrag {
                                    from: line,
                                    boundary: None,
                                    moved: false,
                                    grab_y: absolute.y,
                                });
                            } else {
                                shell.publish(message);
                            }
                            state.focus = Some(Focus::now());
                            shell.capture_event();
                            shell.request_redraw();
                            return;
                        }
                    }
                    // A margin-mark press is navigation, not editing: it
                    // consumes the press without a caret move or focus steal.
                    if let Some(on_margin) = self.on_margin_press.as_deref()
                        && state.composition.is_none()
                        && point.x > self.padding.left + text_bounds.width
                    {
                        let pressed =
                            self.margin_marks
                                .iter()
                                .map(|&(line, _)| line)
                                .find(|&line| {
                                    let row = self.gutter_row(state, text_bounds, line);
                                    margin_mark_bounds(text_bounds, row).contains(absolute)
                                });
                        if let Some(line) = pressed {
                            shell.publish(on_margin(line));
                            shell.capture_event();
                            shell.request_redraw();
                            return;
                        }
                    }
                    let local = local_point(point, self.padding, state.scroll);
                    // A consumed line press is its own gesture: no caret move,
                    // no focus steal — a checkbox tick must not also relocate
                    // the cursor into the line it ticked.
                    if let Some(on_line_press) = self.on_line_press.as_deref()
                        && let Some((line, position)) = pointer::source_line_at(
                            self.content,
                            &state.document,
                            state.composition.as_ref(),
                            local,
                        )
                        && let Some(message) = on_line_press(&line, position)
                    {
                        shell.publish(message);
                        shell.capture_event();
                        shell.request_redraw();
                        return;
                    }
                    let over_link =
                        self.interaction_at(state, local) == mouse::Interaction::Pointer;
                    let next = state.pointer.press(
                        self.content,
                        &state.document,
                        state.composition.as_ref(),
                        local,
                        over_link,
                    );

                    state.focus = Some(Focus::now());
                    state.preferred_x = None;
                    shell.publish(on_action(Action::MoveTo(next)));
                    shell.capture_event();
                    shell.request_redraw();
                } else {
                    // The press landed outside the editor: whatever it acted
                    // on, the anchored menu must not be left floating here —
                    // this is the click-away an overlay backdrop would catch.
                    if self
                        .menu_panel_in(state, bounds.shrink(self.padding))
                        .is_some()
                    {
                        let on_menu = self.on_menu.as_deref().expect("panel implies route");
                        shell.publish(on_menu(MenuEvent::Dismiss));
                        shell.request_redraw();
                    }
                    if state.focus.is_some() {
                        state.focus = None;
                        if state.replace_preedit(None) {
                            shell.invalidate_layout();
                        }
                        state.pending_ime_commit.clear();
                        state.pointer.clear();
                        shell.request_redraw();
                    }
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some(mut drag) = state.gutter_drag {
                    if let Some(point) = cursor.position() {
                        let text_bounds = bounds.shrink(self.padding);
                        if !drag.moved && (point.y - drag.grab_y).abs() > DRAG_THRESHOLD {
                            drag.moved = true;
                        }
                        if drag.moved {
                            let snapped =
                                snap_boundary(self.drop_candidates(state, text_bounds), point.y);
                            if snapped != drag.boundary {
                                drag.boundary = snapped;
                                shell.request_redraw();
                            }
                        }
                        state.gutter_drag = Some(drag);
                        shell.capture_event();
                    }
                    return;
                }
                if state.pointer.is_dragging() {
                    if let Some(point) = cursor.position() {
                        let local = clamped_local_point(point, bounds, self.padding, state.scroll);
                        if let Some(cursor) =
                            state
                                .pointer
                                .drag(&state.document, state.composition.as_ref(), local)
                        {
                            shell.publish(on_action(Action::MoveTo(cursor)));
                            shell.capture_event();
                        }
                    }
                    return;
                }
                // The hovered source line the gutter rides on.
                let hovered = self
                    .on_gutter
                    .as_ref()
                    .filter(|_| state.composition.is_none())
                    .and_then(|_| cursor.position_in(bounds))
                    .and_then(|point| {
                        let local = local_point(point, self.padding, state.scroll);
                        state.document.line_at_y(local.y)
                    });
                if hovered != state.hover_line {
                    state.hover_line = hovered;
                    shell.request_redraw();
                }
                // Hovering a menu row highlights it. THE POINTER DOES NOT END
                // THE MENU: a menu the mouse OPENED is closed the way every
                // other popover here is — a press outside it, Escape, a
                // window blur, or a pick. Dismissing it because the pointer
                // wandered gave a click-made thing a hover lifetime, and cost
                // the reader the panel they were still reading.
                if let Some(point) = cursor.position() {
                    let text_bounds = bounds.shrink(self.padding);
                    if let Some(panel) = self.menu_panel_in(state, text_bounds) {
                        let menu = self.menu.as_ref().expect("panel implies menu");
                        let on_menu = self.on_menu.as_deref().expect("panel implies route");
                        if let Some(row) = menu_row_at(panel, menu.items.len(), point)
                            && row != menu.selected
                        {
                            shell.publish(on_menu(MenuEvent::Select(row)));
                        }
                    }
                }
            }
            Event::Mouse(mouse::Event::CursorLeft) => {
                // The pointer left the window: the hover gutter has no line to
                // ride any more.
                if state.hover_line.is_some() {
                    state.hover_line = None;
                    shell.request_redraw();
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if let Some(drag) = state.gutter_drag.take() {
                    match drag.moved {
                        // It stayed a click — the deferred handle press.
                        false => {
                            if let Some(on_gutter) = self.on_gutter.as_deref()
                                && let Some(message) = on_gutter(drag.from, GutterButton::Handle)
                            {
                                shell.publish(message);
                            }
                        }
                        true => {
                            if let Some(on_drop) = self.on_gutter_drop.as_deref()
                                && let Some(boundary) = drag.boundary
                                && let Some(message) = on_drop(drag.from, boundary)
                            {
                                shell.publish(message);
                            }
                        }
                    }
                    shell.capture_event();
                    shell.request_redraw();
                    return;
                }
                let release_over_pointer = cursor.position_in(bounds).is_some_and(|point| {
                    let local = local_point(point, self.padding, state.scroll);
                    self.interaction_at(state, local) == mouse::Interaction::Pointer
                });
                let release = state.pointer.release(release_over_pointer);
                if release.capture {
                    // Only an actual rendered link click may reach an outer
                    // release handler.
                    shell.capture_event();
                }
                if release.relayout {
                    // Apply any caret-aware highlighter setting that was held
                    // back to keep the drag geometry stable.
                    shell.invalidate_layout();
                    shell.request_redraw();
                }
            }
            Event::InputMethod(input_method::Event::Opened) => {
                if state.replace_preedit(Some(input_method::Preedit::new())) {
                    shell.invalidate_layout();
                }
                state.pending_ime_commit.clear();
                shell.request_redraw();
            }
            Event::InputMethod(input_method::Event::Closed) => {
                if state.replace_preedit(None) {
                    shell.invalidate_layout();
                }
                // AppKit may close the composition before winit reports the
                // release-only ASCII key that ended it. Keep the boundary
                // commit until a printable keyboard event resolves it.
                shell.request_redraw();
            }
            Event::InputMethod(input_method::Event::Preedit(content, selection))
                if state.focus.is_some() =>
            {
                if state.replace_preedit(Some(input_method::Preedit {
                    content: content.clone(),
                    selection: selection.clone(),
                    text_size: None,
                })) {
                    // Rich composition is part of the shaped document, so a
                    // redraw alone cannot expose the new IME stage.
                    shell.invalidate_layout();
                }
                state.pending_ime_commit.on_preedit(content);
                shell.request_redraw();
            }
            Event::InputMethod(input_method::Event::Commit(content)) if state.focus.is_some() => {
                shell.publish(on_action(Action::Edit(text_editor::Action::Edit(
                    Edit::Paste(Arc::new(content.clone())),
                ))));
                if state.replace_preedit(None) {
                    shell.invalidate_layout();
                }
                if cfg!(target_os = "macos") {
                    state.pending_ime_commit.on_commit(content);
                } else {
                    state.pending_ime_commit.clear();
                }
                state.preferred_x = None;
                shell.capture_event();
            }
            Event::Keyboard(keyboard::Event::KeyReleased {
                key,
                modified_key,
                physical_key,
                modifiers,
                ..
            }) if state.focus.is_some() && state.pending_ime_commit.is_pending() => {
                // The built-in macOS Korean IME can commit the composition,
                // clear preedit again, and report only the released ASCII
                // boundary key. Recover that key when the commit omitted it.
                if let ImeBoundary::Missing(character) = state.pending_ime_commit.resolve(
                    ime_boundary_character(key, modified_key, *physical_key, *modifiers),
                ) {
                    shell.publish(on_action(Action::Edit(text_editor::Action::Edit(
                        Edit::Insert(character),
                    ))));
                    state.preferred_x = None;
                    shell.capture_event();
                    shell.request_redraw();
                }
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                modified_key,
                physical_key,
                modifiers,
                text,
                ..
            }) if state.focus.is_some() => {
                if state
                    .preedit
                    .as_ref()
                    .is_some_and(|preedit| !preedit.content.is_empty())
                {
                    state.pending_ime_commit.clear();
                    if modifiers.command() {
                        if state.replace_preedit(None) {
                            shell.invalidate_layout();
                        }
                        shell.request_redraw();
                    } else {
                        shell.capture_event();
                        return;
                    }
                }

                if state.pending_ime_commit.is_pending() {
                    if modifiers.control() || modifiers.alt() || modifiers.logo() {
                        state.pending_ime_commit.clear();
                    }
                    let text_character = text.as_deref().and_then(single_printable_ascii);
                    let event_character = text_character.or_else(|| {
                        ime_boundary_character(key, modified_key, *physical_key, *modifiers)
                    });

                    match state.pending_ime_commit.resolve(event_character) {
                        ImeBoundary::Duplicate => {
                            // Some IME paths report both a commit and the
                            // printable key press (notably Space). Keep one.
                            shell.capture_event();
                            return;
                        }
                        ImeBoundary::Missing(character) if text_character.is_none() => {
                            // The press survived without usable ASCII text.
                            // Insert the same boundary recovered on release.
                            shell.publish(on_action(Action::Edit(text_editor::Action::Edit(
                                Edit::Insert(character),
                            ))));
                            state.preferred_x = None;
                            shell.capture_event();
                            shell.request_redraw();
                            return;
                        }
                        ImeBoundary::Missing(_) | ImeBoundary::Unrelated => {}
                    }
                }

                // While a menu is up its navigation keys belong to the menu,
                // not the buffer — Enter picks instead of splitting the line.
                if let Some(menu) = self.menu.as_ref()
                    && state.composition.is_none()
                    && let Some(on_menu) = self.on_menu.as_deref()
                    && let Some(menu_event) = menu_key(key, menu)
                {
                    shell.publish(on_menu(menu_event));
                    shell.capture_event();
                    shell.request_redraw();
                    return;
                }

                let status = self.status(state, cursor.is_over(bounds));
                let key_press = text_editor::KeyPress {
                    key: key.clone(),
                    modified_key: modified_key.clone(),
                    physical_key: *physical_key,
                    modifiers: *modifiers,
                    text: text.clone(),
                    status,
                };
                let binding = match self.key_binding.as_deref() {
                    Some(key_binding) => key_binding(&key_press),
                    None => default_key_binding(&key_press),
                };

                let Some(binding) = binding else {
                    if let Some(on_chord) = self.on_chord.as_deref()
                        && let Some(message) = on_chord(&key_press)
                    {
                        state.pending_ime_commit.clear();
                        shell.publish(message);
                        shell.capture_event();
                        shell.request_redraw();
                    }
                    return;
                };
                {
                    state.pending_ime_commit.clear();
                    let capture = !matches!(binding, Binding::Unfocus);
                    let mut binding_context = BindingContext::new(
                        &state.document,
                        &mut state.preferred_x,
                        state.viewport_height,
                    );
                    let unfocus = apply_binding(
                        binding,
                        self.content,
                        &mut binding_context,
                        on_action.as_ref(),
                        clipboard,
                        shell,
                    );
                    if unfocus {
                        state.focus = None;
                        state.pointer.clear();
                    }
                    if capture {
                        shell.capture_event();
                    }
                    if let Some(focus) = state.focus.as_mut() {
                        focus.updated_at = Instant::now();
                    }
                    shell.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        _defaults: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let text_bounds = bounds.shrink(self.padding);
        let state = tree.state.downcast_ref::<State<Highlighter>>();
        let style = (self.style)(theme, self.status(state, cursor.is_over(bounds)));

        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: style.border,
                ..renderer::Quad::default()
            },
            style.background,
        );

        let origin = text_bounds.position() - Vector::new(0.0, state.scroll);

        draw_line_highlights(renderer, &state.document, text_bounds, origin);
        draw_line_rules(renderer, &state.document, text_bounds, origin);
        draw_span_highlights(renderer, &state.document, text_bounds, origin);

        // The selection outlives focus: a Find bar that owns the keyboard
        // still has to show the match it selected in here. Only the caret
        // is a focus affordance.
        if state.composition.is_none() {
            draw_selection(
                renderer,
                &state.document,
                self.content.cursor(),
                text_bounds,
                origin,
                style.selection,
            );
        }

        // THE GLYPH PASS LIVES IN ITS OWN LAYER, above every plate. Within one
        // renderer layer the backends batch by primitive kind, not by call
        // order, so an OPAQUE `line_highlight` quad could land over the very
        // glyphs it highlights (translucent washes only ever dimmed them —
        // which is how the bug hid). A pushed layer renders strictly after
        // the current one, restoring the order the draw code always meant.
        renderer.with_layer(bounds, |renderer| {
            if state.source.is_empty() && state.composition.is_none() {
                if let Some(placeholder) = self.placeholder.as_ref() {
                    renderer.fill_text(
                        Text {
                            content: placeholder.clone(),
                            bounds: text_bounds.size(),
                            size: state.text_size,
                            line_height: state.line_height,
                            font: state.font,
                            align_x: text::Alignment::Default,
                            align_y: alignment::Vertical::Top,
                            shaping: text::Shaping::Advanced,
                            wrapping: state.wrapping,
                        },
                        text_bounds.position(),
                        style.placeholder,
                        text_bounds,
                    );
                }
            } else {
                state
                    .document
                    .draw_text(renderer, origin, style.value, text_bounds);
            }

            draw_strikethroughs(renderer, &state.document, text_bounds, origin);

            if let Some(focus) = state.focus.as_ref() {
                if let Some(composition) = state.composition.as_ref() {
                    draw_composition(
                        renderer,
                        &state.document,
                        composition,
                        text_bounds,
                        origin,
                        style.value,
                        focus.is_cursor_visible(),
                    );
                    return;
                }

                let cursor_position = self.content.cursor().position;
                let caret = state.document.caret(cursor_position) + (origin - Point::ORIGIN);

                if focus.is_cursor_visible()
                    && let Some(caret) = text_bounds.intersection(&caret)
                {
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: caret,
                            ..renderer::Quad::default()
                        },
                        style.value,
                    );
                }
            }
        });

        if self.on_action.is_none() || state.composition.is_some() {
            return;
        }

        // The hover gutter rides the hovered line — or the line an open menu
        // anchored it to; the closure's verdict on Plus doubles as its
        // visibility switch for the line.
        if let Some(on_gutter) = self.on_gutter.as_deref()
            && !state.pointer.is_dragging()
            && state.gutter_drag.is_none()
            && let Some(line) = self.gutter_line(state)
            && on_gutter(line, GutterButton::Plus).is_some()
        {
            let row = self.gutter_row(state, text_bounds, line);
            let buttons = gutter_buttons(text_bounds, row);
            renderer.with_layer(bounds, |renderer| {
                draw_gutter(renderer, &buttons, style.placeholder);
            });
        }

        // The margin marks ride their lines whenever the host declared them —
        // persistent indicators, not hover affordances. The TIP is the hover
        // affordance: a chip drawn in here can carry no host tooltip, so
        // without it the pointer cursor is the only thing naming the gesture.
        if self.on_margin_press.is_some() && !self.margin_marks.is_empty() {
            let accent = theme.extended_palette().primary.base.color;
            let pointer = cursor
                .position_in(bounds)
                .map(|point| Point::new(bounds.x + point.x, bounds.y + point.y));
            let mut tip = None;
            renderer.with_layer(bounds, |renderer| {
                for &(line, count) in &self.margin_marks {
                    let row = self.gutter_row(state, text_bounds, line);
                    let mark = margin_mark_bounds(text_bounds, row);
                    if bounds.intersection(&mark).is_none() {
                        continue;
                    }
                    draw_margin_mark(renderer, mark, accent, count, state.font);
                    if pointer.is_some_and(|point| mark.contains(point)) {
                        tip = Some(mark);
                    }
                }
            });
            // Its own layer, expanded past `bounds`: the tip hangs back over
            // the text and must paint above every plate drawn under it.
            if let Some(mark) = tip.filter(|_| !self.margin_label.is_empty()) {
                let palette = theme.extended_palette();
                let colors = MenuColors {
                    panel: palette.background.base.color,
                    outline: palette.background.strong.color,
                    selected: palette.background.weak.color,
                    label: palette.background.base.text,
                };
                let panel = margin_tip_bounds(mark, &self.margin_label, text_bounds);
                renderer.with_layer(panel.expand(24.0), |renderer| {
                    draw_margin_tip(renderer, panel, &self.margin_label, state.font, &colors);
                });
            }
        }

        // Mid-drag, the accent line marks where the grabbed block would land.
        if let Some(drag) = state.gutter_drag.as_ref()
            && drag.moved
            && let Some(boundary) = drag.boundary
        {
            let y = self.boundary_y(state, boundary, text_bounds);
            let accent = theme.extended_palette().primary.base.color;
            renderer.with_layer(bounds, |renderer| {
                draw_drop_indicator(renderer, text_bounds, y, accent);
            });
        }

        // The anchored menu draws last, above everything, in its own layer —
        // expanded so its shadow is not clipped at the panel edge.
        if let Some(panel) = self.menu_panel_in(state, text_bounds) {
            let menu = self.menu.as_ref().expect("panel implies menu");
            let palette = theme.extended_palette();
            let colors = MenuColors {
                panel: palette.background.base.color,
                outline: palette.background.strong.color,
                selected: palette.background.weak.color,
                label: palette.background.base.text,
            };
            let selected = menu.selected.min(menu.items.len().saturating_sub(1));
            renderer.with_layer(panel.expand(24.0), |renderer| {
                draw_menu(renderer, panel, &menu.items, selected, state.font, &colors);
            });
        }
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let bounds = layout.bounds();
        if self.on_action.is_none() && cursor.is_over(bounds) {
            return mouse::Interaction::NotAllowed;
        }

        let dragging_handle = tree
            .state
            .downcast_ref::<State<Highlighter>>()
            .gutter_drag
            .is_some();
        if dragging_handle {
            return mouse::Interaction::Grabbing;
        }

        if let Some(point) = cursor.position_in(bounds) {
            let state = tree.state.downcast_ref::<State<Highlighter>>();
            let absolute = Point::new(bounds.x + point.x, bounds.y + point.y);
            let text_bounds = bounds.shrink(self.padding);
            if let Some(panel) = self.menu_panel_in(state, text_bounds)
                && panel.contains(absolute)
            {
                return mouse::Interaction::Pointer;
            }
            if self.on_gutter.is_some()
                && state.composition.is_none()
                && let Some(line) = self.gutter_line(state)
                && point.x < self.padding.left
            {
                let row = self.gutter_row(state, text_bounds, line);
                let over_button = gutter_buttons(text_bounds, row)
                    .iter()
                    .any(|(_, button_bounds)| button_bounds.contains(absolute));
                if over_button {
                    return mouse::Interaction::Pointer;
                }
            }
            if self.on_margin_press.is_some()
                && state.composition.is_none()
                && point.x > self.padding.left + text_bounds.width
            {
                let over_mark = self.margin_marks.iter().any(|&(line, _)| {
                    let row = self.gutter_row(state, text_bounds, line);
                    margin_mark_bounds(text_bounds, row).contains(absolute)
                });
                if over_mark {
                    return mouse::Interaction::Pointer;
                }
            }
            let point = point - Vector::new(self.padding.left, self.padding.top)
                + Vector::new(0.0, state.scroll);
            return self.interaction_at(state, point);
        }

        mouse::Interaction::default()
    }

    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        _renderer: &iced::Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        operation.focusable(
            self.id.as_ref(),
            layout.bounds(),
            tree.state.downcast_mut::<State<Highlighter>>(),
        );
    }
}

impl<'a, Highlighter, Message> From<RichTextEditor<'a, Highlighter, Message>>
    for Element<'a, Message>
where
    Highlighter: text::Highlighter,
    Message: 'a,
{
    fn from(editor: RichTextEditor<'a, Highlighter, Message>) -> Self {
        Self::new(editor)
    }
}

/// The menu's verdict on a key press, if the key is one it owns.
fn menu_key(key: &keyboard::Key, menu: &EditorMenu) -> Option<MenuEvent> {
    use keyboard::key::Named;
    let count = menu.items.len();
    let selected = menu.selected.min(count.saturating_sub(1));
    let keyboard::Key::Named(named) = key else {
        return None;
    };
    match named {
        Named::ArrowUp => Some(MenuEvent::Select(
            selected.checked_sub(1).unwrap_or(count.saturating_sub(1)),
        )),
        Named::ArrowDown => Some(MenuEvent::Select((selected + 1) % count.max(1))),
        Named::Enter | Named::Tab => Some(
            menu.items
                .get(selected)
                .map_or(MenuEvent::Dismiss, |item| MenuEvent::Pick(item.tag.clone())),
        ),
        Named::Escape => Some(MenuEvent::Dismiss),
        _ => None,
    }
}

impl<H> State<H>
where
    H: text::Highlighter,
{
    fn replace_preedit(&mut self, preedit: Option<input_method::Preedit>) -> bool {
        if self.preedit == preedit {
            return false;
        }

        self.preedit = preedit;
        true
    }

    fn max_scroll(&self) -> f32 {
        (self.content_height + self.end_padding - self.viewport_height).max(0.0)
    }

    fn reveal(&mut self, position: Position) {
        let caret = self.document.caret(position);
        if caret.y < self.scroll {
            self.scroll = caret.y;
        } else if caret.y + caret.height > self.scroll + self.viewport_height {
            self.scroll = caret.y + caret.height - self.viewport_height;
        }
        self.scroll = self.scroll.clamp(0.0, self.max_scroll());
    }
}

#[cfg(test)]
#[path = "rich_text_editor/tests/mod.rs"]
mod tests;
