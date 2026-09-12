//! Measured native text inputs for the guest's logical lines. Source bytes,
//! block rules and history stay in the guest; this is only a layout projection.

use super::{EditorStore, Projection, offset, position};
use gpui_kit::base::input::{
    Editor, EditorState, InputEditorStyle, TextDecoration, TextDecorationCollection,
};
use gpui_kit::component::button::Button;
use gpui_kit::*;
use std::{ops::Range, sync::Arc};
use ui_lang_wire as wire;
use unicode_segmentation::UnicodeSegmentation;
use wire::editor_presentation::{EditorFormat, EditorInteraction};

/// Removing a hidden syntax span is a projection, never a document mutation.
/// Segments retain both byte spaces so a click or IME edit maps back exactly.
#[derive(Clone, Debug)]
struct Segment {
    source: Range<usize>,
    display: Range<usize>,
    format: Option<EditorFormat>,
}

#[derive(Clone, Debug)]
struct LineProjection {
    source: Range<usize>,
    display: String,
    segments: Vec<Segment>,
    size: f32,
    line_height: f32,
    padding: wire::Edges,
    background: Option<wire::Rgba>,
    border: Option<wire::Border>,
    rule: Option<wire::Rgba>,
    font: Option<wire::NamedFont>,
}

impl LineProjection {
    fn source_at(&self, display: usize) -> usize {
        for segment in &self.segments {
            if display < segment.display.end {
                return segment.source.start + display.saturating_sub(segment.display.start);
            }
        }
        self.segments.last().map_or(0, |segment| segment.source.end)
    }

    fn display_at(&self, source: usize) -> usize {
        for segment in &self.segments {
            if source <= segment.source.start {
                return segment.display.start;
            }
            if source <= segment.source.end {
                return segment.display.start + source - segment.source.start;
            }
        }
        self.display.len()
    }
}

struct LineInput {
    input: Entity<EditorState>,
    decorations: TextDecorationCollection,
    _observation: Subscription,
    projection: LineProjection,
    height: Pixels,
    inset: Pixels,
    lead: Point<Pixels>,
}

pub struct WireEditor {
    key: String,
    store: EditorStore,
    lines: Vec<LineInput>,
    preview: Arc<str>,
    cursor: wire::EditorCursor,
    reset: Option<u64>,
    projection: Option<Projection>,
    painted: Option<wire::EditorOptions>,
    scroll: ScrollHandle,
    focus_line: Option<usize>,
    drag_line: Option<u32>,
}

impl EventEmitter<()> for WireEditor {}

impl WireEditor {
    pub fn new(
        key: String,
        store: EditorStore,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut this = Self {
            key,
            store,
            lines: Vec::new(),
            preview: Arc::from(""),
            cursor: Default::default(),
            reset: None,
            projection: None,
            painted: None,
            scroll: ScrollHandle::new(),
            focus_line: None,
            drag_line: None,
        };
        this.sync(window, cx);
        this
    }

    fn focused_line(&self, window: &Window, cx: &App) -> Option<usize> {
        self.lines
            .iter()
            .position(|line| line.input.read(cx).focus_handle(cx).is_focused(window))
    }

    pub fn is_focused(&self, window: &Window, cx: &App) -> bool {
        self.focused_line(window, cx).is_some()
    }

    pub fn widget_command(
        &mut self,
        command: &wire::WidgetCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let at = |index: u32| {
            position(
                &self.preview,
                self.preview
                    .grapheme_indices(true)
                    .nth(index as usize)
                    .map_or(self.preview.len(), |(offset, _)| offset),
            )
        };
        let cursor = match command {
            wire::WidgetCommand::Focus { .. } => self.cursor,
            wire::WidgetCommand::CursorFront { .. } => wire::EditorCursor::default(),
            wire::WidgetCommand::CursorEnd { .. } => wire::EditorCursor {
                position: position(&self.preview, self.preview.len()),
                selection: None,
            },
            wire::WidgetCommand::Cursor {
                position: index, ..
            } => wire::EditorCursor {
                position: at(*index),
                selection: None,
            },
            wire::WidgetCommand::SelectAll { .. } => wire::EditorCursor {
                position: position(&self.preview, self.preview.len()),
                selection: Some(Default::default()),
            },
            wire::WidgetCommand::Select { start, end, .. } => wire::EditorCursor {
                position: at(*end),
                selection: (*start != *end).then(|| at(*start)),
            },
            _ => return false,
        };
        if cursor != self.cursor {
            self.move_cursor(cursor, cx);
        }
        self.focus_line = Some(cursor.position.line as usize);
        self.sync(window, cx);
        true
    }

    pub fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(projection) = self.store.projection(&self.key) else {
            return;
        };
        let reset = self.reset != Some(projection.reference.reset);
        let settled = !projection.pending;
        let canonical = projection.text.clone().unwrap_or_else(|| Arc::from(""));
        let install = reset
            || (settled
                && (canonical != self.preview || projection.reference.cursor != self.cursor));
        let focused = self.focused_line(window, cx);
        self.store.set_focused(&self.key, focused.is_some());
        if install {
            self.preview = canonical;
            self.cursor = projection.reference.cursor;
            self.reset = Some(projection.reference.reset);
            if focused.is_some() {
                self.focus_line = Some(self.cursor.position.line as usize);
            }
        }
        let repaint = install
            || self.painted.as_ref() != Some(&projection.options)
            || self.lines.len() != wire::editor_lines(&self.preview).count();
        if repaint {
            let display = line_projections(&self.preview, &projection.options);
            self.lines.truncate(display.len());
            for (index, line) in display.into_iter().enumerate() {
                if index == self.lines.len() {
                    let input = cx.new(|cx| {
                        EditorState::new(window, cx)
                            .line_number(false)
                            .folding(false)
                            .indent_guides(false)
                            .searchable(false)
                            .scroll_beyond_last_line(Some(0))
                            .smart_indent(false)
                            .soft_wrap(true)
                            .context_menu(false)
                    });
                    let decorations = input.update(cx, |input, cx| {
                        input.set_auto_close(false, window, cx);
                        input.create_decorations_collection(Vec::new(), cx)
                    });
                    let observation = cx.observe_in(&input, window, move |this, _, window, cx| {
                        this.observed(index, window, cx)
                    });
                    self.lines.push(LineInput {
                        input,
                        decorations,
                        _observation: observation,
                        height: px(line.line_height + 2.),
                        inset: px(2.),
                        lead: point(px(0.), px(0.)),
                        projection: line.clone(),
                    });
                }
                let row = &mut self.lines[index];
                let style_changed = row.projection.size != line.size
                    || row.projection.line_height != line.line_height;
                if style_changed {
                    row.inset = px(2.);
                    row.height = px(line.line_height) + row.inset;
                }
                row.projection = line;
                let text = &row.projection.display;
                let local_cursor = local_cursor(&row.projection, self.cursor, index);
                row.input.update(cx, |input, cx| {
                    if input.value().as_ref() != text {
                        input.set_value(text.clone(), window, cx);
                    }
                    let normalized = local_cursor.start.min(local_cursor.end)
                        ..local_cursor.start.max(local_cursor.end);
                    if input.selected_range() != normalized || input.cursor() != local_cursor.end {
                        input.set_selected_range(local_cursor, cx);
                    }
                    input.set_soft_wrap(
                        !matches!(projection.options.wrapping, Some(wire::Wrapping::None)),
                        window,
                        cx,
                    );
                    input.set_editor_paddings(Edges::all(px(0.)));
                    let face = &projection.options.style.active;
                    input.set_editor_style(InputEditorStyle {
                        foreground: face.value.map(color).unwrap_or_default(),
                        muted_foreground: face.placeholder.map(color).unwrap_or_default(),
                        background: face.background.map(color).unwrap_or_default(),
                        selection: face.selection.map(color).unwrap_or_default(),
                        ..Default::default()
                    });
                });
                let decorations = row
                    .projection
                    .segments
                    .iter()
                    .filter_map(|segment| {
                        let format = segment.format.as_ref()?;
                        Some(TextDecoration::new(
                            segment.display.clone(),
                            HighlightStyle {
                                color: format.color.map(color),
                                background_color: format.background.map(color),
                                font_weight: format
                                    .font
                                    .as_ref()
                                    .map(|font| font_weight(font.weight)),
                                font_style: format.font.as_ref().map(|font| match font.style {
                                    wire::FontStyle::Normal => gpui_kit::FontStyle::Normal,
                                    wire::FontStyle::Italic | wire::FontStyle::Oblique => {
                                        gpui_kit::FontStyle::Italic
                                    }
                                }),
                                strikethrough: format.strikethrough.map(|ink| StrikethroughStyle {
                                    thickness: px(1.),
                                    color: Some(color(ink)),
                                }),
                                ..Default::default()
                            },
                        ))
                    })
                    .collect();
                row.decorations.set(decorations, cx);
            }
            self.painted = Some(projection.options.clone());
        }
        let editable =
            projection.editable && projection.fault.is_none() && projection.text.is_some();
        for row in &self.lines {
            row.input.update(cx, |input, cx| {
                if input.is_editable() != editable {
                    input.set_readonly(!editable, cx);
                }
            });
        }
        if let Some(index) = self
            .focus_line
            .take()
            .filter(|index| *index < self.lines.len())
        {
            self.lines[index]
                .input
                .read(cx)
                .focus_handle(cx)
                .focus(window);
        }
        self.projection = Some(projection);
    }

    fn composing(&self, index: usize, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.lines.get(index).is_some_and(|row| {
            row.input.update(cx, |input, cx| {
                input.marked_text_range(window, cx).is_some()
            })
        })
    }

    fn observed(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let focused = self.focused_line(window, cx) == Some(index);
        self.store
            .set_focused(&self.key, self.focused_line(window, cx).is_some());
        if !focused || self.composing(index, window, cx) {
            return;
        }
        let Some(row) = self.lines.get(index) else {
            return;
        };
        let state = row.input.read(cx);
        let text = state.value();
        let caret = state.cursor();
        let selection = state.selected_range();
        let patch = match wire::editor_document::editor_changed_span(&row.projection.display, &text)
        {
            Ok(patches) => patches.into_iter().next(),
            Err(_) => return,
        };
        let global_selection = self
            .cursor
            .selection
            .is_some_and(|anchor| anchor.line != self.cursor.position.line);
        let (after, next) = if let Some(patch) = patch {
            let mut start =
                row.projection.source.start + row.projection.source_at(patch.start_byte as usize);
            let mut end =
                row.projection.source.start + row.projection.source_at(patch.end_byte as usize);
            if global_selection {
                let active = offset(&self.preview, self.cursor.position);
                let anchor = offset(
                    &self.preview,
                    self.cursor.selection.expect("selection checked"),
                );
                start = active.min(anchor);
                end = active.max(anchor);
            }
            let mut after = self.preview[..start].to_owned();
            after.push_str(&patch.replacement);
            after.push_str(&self.preview[end..]);
            let next = wire::EditorCursor {
                position: position(
                    &after,
                    start + caret.saturating_sub(patch.start_byte as usize),
                ),
                selection: None,
            };
            (after, next)
        } else {
            let start = row.projection.source.start;
            let next = wire::EditorCursor {
                position: position(&self.preview, start + row.projection.source_at(caret)),
                selection: (selection.start != selection.end).then(|| {
                    position(
                        &self.preview,
                        start
                            + row.projection.source_at(if caret == selection.start {
                                selection.end
                            } else {
                                selection.start
                            }),
                    )
                }),
            };
            if global_selection && next.position == self.cursor.position {
                return;
            }
            if next == self.cursor {
                return;
            }
            (self.preview.to_string(), next)
        };
        let changed_text = after != self.preview.as_ref();
        let kind = if !changed_text {
            wire::EditorEditKind::Cursor
        } else if after.len() < self.preview.len() {
            wire::EditorEditKind::Backspace
        } else {
            wire::EditorEditKind::Insert
        };
        self.store
            .native(&self.key, &self.preview, self.cursor, &after, next, kind);
        self.preview = Arc::from(after);
        self.cursor = next;
        self.painted = None;
        cx.emit(());
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.focused_line(window, cx) else {
            return;
        };
        if self.composing(index, window, cx) {
            return;
        }
        let Some(projection) = &self.projection else {
            return;
        };
        let key = key_state(&event.keystroke);
        if let Some(menu) = projection
            .options
            .presentation
            .as_ref()
            .and_then(|p| p.affordances.menu.as_ref())
        {
            let action = match &key.key {
                wire::keyboard::Key::Named(wire::keyboard::Named::ArrowUp) => {
                    Some(EditorInteraction::MenuSelect {
                        index: menu.selected.saturating_sub(1),
                    })
                }
                wire::keyboard::Key::Named(wire::keyboard::Named::ArrowDown) => {
                    Some(EditorInteraction::MenuSelect {
                        index: (menu.selected + 1).min(menu.items.len().saturating_sub(1) as u32),
                    })
                }
                wire::keyboard::Key::Named(wire::keyboard::Named::Enter) => menu
                    .items
                    .get(menu.selected as usize)
                    .map(|item| EditorInteraction::MenuPick {
                        tag: item.tag.clone(),
                    }),
                wire::keyboard::Key::Named(wire::keyboard::Named::Escape) => {
                    Some(EditorInteraction::MenuDismiss)
                }
                _ => None,
            };
            if let Some(action) = action {
                self.interaction(action, cx);
                cx.stop_propagation();
                return;
            }
        }
        let command = if cfg!(target_os = "macos") {
            key.modifiers.logo
        } else {
            key.modifiers.control
        };
        if command && self.document_command(&key.key, cx) {
            cx.stop_propagation();
            return;
        }
        if self.cross_line_key(index, &key, cx) {
            cx.stop_propagation();
            return;
        }
        let claimed = self
            .projection
            .as_ref()
            .and_then(|p| p.options.binding.as_ref())
            .is_some_and(|binding| {
                binding
                    .claims
                    .iter()
                    .any(|claim| claim.matches(&key, cfg!(target_os = "macos")))
            });
        if !claimed {
            return;
        }
        self.store.request(
            &self.key,
            wire::EditorRequestInput::Key {
                key,
                repeat: event.is_held,
            },
        );
        cx.stop_propagation();
        cx.emit(());
    }

    fn document_command(&mut self, key: &wire::keyboard::Key, cx: &mut Context<Self>) -> bool {
        let wire::keyboard::Key::Character(key) = key else {
            return false;
        };
        match key.as_str() {
            "a" => {
                let cursor = wire::EditorCursor {
                    position: position(&self.preview, self.preview.len()),
                    selection: Some(wire::EditorPosition::default()),
                };
                self.move_cursor(cursor, cx);
                true
            }
            "c" | "x" => {
                let Some(anchor) = self.cursor.selection else {
                    return false;
                };
                let active = offset(&self.preview, self.cursor.position);
                let anchor = offset(&self.preview, anchor);
                let range = active.min(anchor)..active.max(anchor);
                cx.write_to_clipboard(ClipboardItem::new_string(
                    self.preview[range.clone()].to_owned(),
                ));
                if key == "x" {
                    let mut next = self.preview.to_string();
                    next.replace_range(range.clone(), "");
                    let cursor = wire::EditorCursor {
                        position: position(&next, range.start),
                        selection: None,
                    };
                    self.store.native(
                        &self.key,
                        &self.preview,
                        self.cursor,
                        &next,
                        cursor,
                        wire::EditorEditKind::Cut,
                    );
                    self.preview = Arc::from(next);
                    self.cursor = cursor;
                    self.painted = None;
                    cx.emit(());
                    cx.notify();
                }
                true
            }
            _ => false,
        }
    }

    fn cross_line_key(
        &mut self,
        index: usize,
        key: &wire::keyboard::KeyState,
        cx: &mut Context<Self>,
    ) -> bool {
        use wire::keyboard::{Key, Named};
        let row = &self.lines[index];
        let input = row.input.read(cx);
        let caret = input.cursor();
        let range = input.cursor_layout().map(|(b, _)| b);
        let first = input.range_to_bounds(&(0..0));
        let end = row.projection.display.len();
        let last = input.range_to_bounds(&(end..end));
        let at_top = range
            .zip(first)
            .is_some_and(|(a, b)| a.origin.y <= b.origin.y);
        let at_bottom = range
            .zip(last)
            .is_some_and(|(a, b)| a.origin.y >= b.origin.y);
        let target = match key.key {
            Key::Named(Named::ArrowUp) if at_top && index > 0 => {
                Some((index - 1, self.cursor.position.column))
            }
            Key::Named(Named::ArrowDown) if at_bottom && index + 1 < self.lines.len() => {
                Some((index + 1, self.cursor.position.column))
            }
            Key::Named(Named::ArrowLeft) if caret == 0 && index > 0 => Some((index - 1, u32::MAX)),
            Key::Named(Named::ArrowRight) if caret == end && index + 1 < self.lines.len() => {
                Some((index + 1, 0))
            }
            _ => None,
        };
        let Some((line, column)) = target else {
            return false;
        };
        let mut cursor = wire::EditorCursor {
            position: wire::EditorPosition {
                line: line as u32,
                column,
            },
            selection: if key.modifiers.shift {
                Some(self.cursor.selection.unwrap_or(self.cursor.position))
            } else {
                None
            },
        };
        cursor.clamp(&self.preview);
        self.focus_line = Some(line);
        self.move_cursor(cursor, cx);
        true
    }

    fn move_cursor(&mut self, cursor: wire::EditorCursor, cx: &mut Context<Self>) {
        self.store.native(
            &self.key,
            &self.preview,
            self.cursor,
            &self.preview,
            cursor,
            wire::EditorEditKind::Cursor,
        );
        self.cursor = cursor;
        self.painted = None;
        cx.emit(());
        cx.notify();
    }

    fn interaction(&mut self, action: EditorInteraction, cx: &mut Context<Self>) {
        self.store
            .request(&self.key, wire::EditorRequestInput::Interaction { action });
        cx.emit(());
    }

    fn measure(&mut self, cx: &mut Context<Self>) {
        let mut changed = false;
        for row in &mut self.lines {
            let input = row.input.read(cx);
            let Some(area) = input.text_bounds() else {
                continue;
            };
            let line_height = input
                .line_height()
                .unwrap_or(px(row.projection.line_height));
            let measured = input
                .range_to_bounds(&(0..row.projection.display.len()))
                .map_or(line_height, |b| b.size.height)
                .max(line_height);
            let inset = (input.input_bounds().size.height - area.size.height).max(px(0.));
            let needed = measured + inset + px(1.);
            let lead = input
                .range_to_bounds(&(0..0))
                .map_or(point(px(0.), px(0.)), |glyph| {
                    glyph.origin - input.input_bounds().origin
                });
            let different =
                (row.height - needed).abs() > px(0.5) || (row.lead.x - lead.x).abs() > px(0.5);
            if different {
                row.height = needed;
                row.inset = inset;
                row.lead = lead;
                changed = true;
            }
            let offset = input.scroll_offset();
            if offset.y != px(0.) && area.size.height + px(0.1) >= measured {
                row.input.update(cx, |input, cx| {
                    input.set_scroll_offset(point(offset.x, px(0.)), cx)
                });
            }
        }
        if changed {
            cx.notify();
        }
    }
}

impl Render for WireEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync(window, cx);
        let options = self
            .projection
            .as_ref()
            .map(|p| p.options.clone())
            .unwrap_or_default();
        let paint = options.presentation.as_deref();
        let pad = paint
            .and_then(|p| p.padding)
            .unwrap_or_else(|| wire::Edges::all(options.padding.unwrap_or(8.)));
        let mut content = div()
            .relative()
            .flex()
            .flex_col()
            .w_full()
            .pt(px(pad.top))
            .pb(px(pad.bottom));
        for (index, row) in self.lines.iter().enumerate() {
            let line = index as u32;
            let layout = &row.projection;
            let mut body = div()
                .id(("line", index))
                .relative()
                .w_full()
                .pl(px(pad.left + layout.padding.left))
                .pr(px(pad.right + layout.padding.right))
                .pt(px(layout.padding.top))
                .pb(px(layout.padding.bottom));
            if let Some(ink) = layout.background {
                body = body.bg(color(ink));
            }
            if let Some(border) = layout.border {
                if let Some(ink) = border.color {
                    body = body.border_color(color(ink));
                }
                body = body
                    .border(px(border.width.unwrap_or(0.)))
                    .rounded(px(border.radius.unwrap_or_default()[0]));
            }
            if let Some(ink) = layout.rule {
                body = body.border_b_1().border_color(color(ink));
            }
            let mut editor = div()
                .w_full()
                .h(row.height)
                .ml(-row.lead.x)
                .mr(-row.lead.x)
                .text_size(px(layout.size))
                .line_height(px(layout.line_height))
                .child(Editor::new(&row.input));
            if let Some(font) = layout.font.as_ref().or(options.font.as_ref()) {
                let family = match &font.family {
                    wire::FontFamily::Named(name) => name.as_str(),
                    wire::FontFamily::Monospace => "monospace",
                    _ => "Geist",
                };
                editor = editor.font_family(SharedString::from(family.to_owned()));
            }
            body = body.child(editor);
            if let Some(paint) = paint {
                if let Some(gutter) = paint.affordances.gutters.iter().find(|g| g.line == line) {
                    let mut gutter_view = div()
                        .absolute()
                        .left(px(0.))
                        .top(px(layout.padding.top))
                        .flex()
                        .gap(px(1.));
                    if gutter.plus {
                        gutter_view =
                            gutter_view.child(Button::new(("plus", index)).label("+").on_click(
                                cx.listener(move |this, _, _, cx| {
                                    this.interaction(
                                        EditorInteraction::Gutter {
                                            line,
                                            button:
                                                wire::editor_presentation::EditorGutterButton::Plus,
                                        },
                                        cx,
                                    )
                                }),
                            ));
                    }
                    if gutter.handle {
                        gutter_view = gutter_view.child(div()
                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, _| this.drag_line = Some(line)))
                        .child(Button::new(("block", index)).label("⋮").on_click(cx.listener(move |this, _, _, cx|
                            this.interaction(EditorInteraction::Gutter { line, button: wire::editor_presentation::EditorGutterButton::Handle }, cx)))));
                    }
                    body = body.child(gutter_view);
                }
                if let Some(margin) = paint.affordances.margins.iter().find(|m| m.line == line) {
                    body = body.child(
                        div()
                            .absolute()
                            .right(px(0.))
                            .top(px(layout.padding.top))
                            .child(
                                Button::new(("comments", index))
                                    .label(margin.count.to_string())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.interaction(EditorInteraction::Margin { line }, cx)
                                    })),
                            ),
                    );
                }
                let boundary = paint.affordances.drop_boundaries.contains(&line);
                body = body.on_mouse_up(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        if let Some(from) = this.drag_line.take() {
                            if boundary && from != line {
                                this.interaction(
                                    EditorInteraction::GutterDrop {
                                        from,
                                        boundary: line,
                                    },
                                    cx,
                                );
                            }
                            return;
                        }
                        let action = this
                            .projection
                            .as_ref()
                            .and_then(|p| p.options.presentation.as_ref())
                            .and_then(|p| p.affordances.hit(this.cursor.position));
                        if let Some(action) = action {
                            this.interaction(action, cx);
                        }
                    }),
                );
                if let Some(menu) =
                    paint
                        .affordances
                        .menu
                        .as_ref()
                        .filter(|menu| match menu.anchor {
                            wire::editor_presentation::EditorMenuAnchor::Line(anchor) => {
                                anchor == line
                            }
                            wire::editor_presentation::EditorMenuAnchor::Caret => {
                                self.cursor.position.line == line
                            }
                        })
                {
                    let mut menu_view = div()
                        .absolute()
                        .left(px(pad.left))
                        .top(row.height + px(layout.padding.top))
                        .flex()
                        .flex_col()
                        .p(px(4.))
                        .bg(gpui_kit::rgb(0x27272a))
                        .rounded(px(6.));
                    for (item_index, item) in menu.items.iter().enumerate() {
                        let tag = item.tag.clone();
                        menu_view = menu_view.child(
                            Button::new(("menu", item_index))
                                .label(item.label.clone())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.interaction(
                                        EditorInteraction::MenuPick { tag: tag.clone() },
                                        cx,
                                    )
                                })),
                        );
                    }
                    body = body.child(menu_view);
                }
            }
            content = content.child(body);
        }
        let weak = cx.entity().downgrade();
        window.on_next_frame(move |_, cx| {
            let _ = weak.update(cx, |this, cx| this.measure(cx));
        });
        let mut root = div()
            .id(SharedString::from(self.key.clone()))
            .relative()
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .capture_key_down(cx.listener(Self::key_down))
            .child(content);
        if let Some(error) = self.projection.as_ref().and_then(|p| p.fault.clone()) {
            root = root.child(div().text_color(gpui_kit::rgb(0xc04040)).child(error));
        }
        root
    }
}

fn local_cursor(line: &LineProjection, cursor: wire::EditorCursor, index: usize) -> Range<usize> {
    let Some(anchor) = cursor.selection else {
        let position = if cursor.position.line as usize == index {
            line.display_at(cursor.position.column as usize)
        } else {
            0
        };
        return position..position;
    };
    let (start, end) =
        if (anchor.line, anchor.column) < (cursor.position.line, cursor.position.column) {
            (anchor, cursor.position)
        } else {
            (cursor.position, anchor)
        };
    if index < start.line as usize || index > end.line as usize {
        return 0..0;
    }
    let a = if index == start.line as usize {
        line.display_at(start.column as usize)
    } else {
        0
    };
    let b = if index == end.line as usize {
        line.display_at(end.column as usize)
    } else {
        line.display.len()
    };
    if cursor.position.line <= anchor.line {
        b..a
    } else {
        a..b
    }
}

fn line_projections(text: &str, options: &wire::EditorOptions) -> Vec<LineProjection> {
    let paint = options
        .presentation
        .as_deref()
        .filter(|paint| paint.validate(text).is_ok());
    wire::editor_lines(text)
        .enumerate()
        .map(|(index, source)| {
            let start = source.as_ptr() as usize - text.as_ptr() as usize;
            let mut line = LineProjection {
                source: start..start + source.len(),
                display: String::new(),
                segments: Vec::new(),
                size: options.size.unwrap_or(14.),
                line_height: 0.,
                padding: wire::Edges::default(),
                background: None,
                border: None,
                rule: None,
                font: options.font.clone(),
            };
            let spans = paint
                .map(|p| {
                    p.spans
                        .iter()
                        .filter(|span| span.line == index as u32)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let mut cursor = 0usize;
            for span in spans {
                let Some(format) = paint.and_then(|p| p.formats.get(span.format as usize)) else {
                    continue;
                };
                if cursor < span.start as usize {
                    append_segment(&mut line, source, cursor..span.start as usize, None);
                }
                if format.size.is_none_or(|size| size >= 1.) {
                    append_segment(
                        &mut line,
                        source,
                        span.start as usize..span.end as usize,
                        Some(format.clone()),
                    );
                }
                if let Some(size) = format.size.filter(|size| *size >= 1.) {
                    line.size = size;
                }
                if let Some(height) = format.line_height {
                    line.line_height = match height {
                        wire::LineHeight::Absolute(value) => value,
                        wire::LineHeight::Relative(ratio) => ratio * line.size,
                    };
                }
                if format.line_padding != wire::Edges::default() {
                    line.padding = format.line_padding;
                }
                if format.line_background.is_some() {
                    line.background = format.line_background;
                }
                if format.line_border.is_some() {
                    line.border = format.line_border;
                }
                if format.line_rule.is_some() {
                    line.rule = format.line_rule;
                }
                if format.font.is_some() {
                    line.font = format.font.clone();
                }
                cursor = span.end as usize;
            }
            if cursor < source.len() {
                append_segment(&mut line, source, cursor..source.len(), None);
            }
            if line.line_height == 0. {
                line.line_height = match options.line_height {
                    Some(wire::LineHeight::Absolute(h)) => h,
                    Some(wire::LineHeight::Relative(r)) => r * line.size,
                    None => line.size * 1.4,
                };
            }
            line
        })
        .collect()
}

fn append_segment(
    line: &mut LineProjection,
    text: &str,
    source: Range<usize>,
    format: Option<EditorFormat>,
) {
    let start = line.display.len();
    line.display.push_str(&text[source.clone()]);
    line.segments.push(Segment {
        source,
        display: start..line.display.len(),
        format,
    });
}

fn color(ink: wire::Rgba) -> Hsla {
    let [r, g, b, a] = ink.0;
    gpui_kit::Rgba { r, g, b, a }.into()
}

fn font_weight(weight: wire::Weight) -> FontWeight {
    match weight {
        wire::Weight::Thin => FontWeight::THIN,
        wire::Weight::ExtraLight => FontWeight::EXTRA_LIGHT,
        wire::Weight::Light => FontWeight::LIGHT,
        wire::Weight::Normal => FontWeight::NORMAL,
        wire::Weight::Medium => FontWeight::MEDIUM,
        wire::Weight::Semibold => FontWeight::SEMIBOLD,
        wire::Weight::Bold => FontWeight::BOLD,
        wire::Weight::ExtraBold => FontWeight::EXTRA_BOLD,
        wire::Weight::Black => FontWeight::BLACK,
    }
}

fn key_state(key: &Keystroke) -> wire::keyboard::KeyState {
    use wire::keyboard::{Key, Named};
    let logical = match key.key.as_str() {
        "enter" => Key::Named(Named::Enter),
        "tab" => Key::Named(Named::Tab),
        "backspace" => Key::Named(Named::Backspace),
        "delete" => Key::Named(Named::Delete),
        "escape" => Key::Named(Named::Escape),
        "up" => Key::Named(Named::ArrowUp),
        "down" => Key::Named(Named::ArrowDown),
        "left" => Key::Named(Named::ArrowLeft),
        "right" => Key::Named(Named::ArrowRight),
        _ => Key::Character(key.key.clone()),
    };
    wire::keyboard::KeyState {
        key: logical.clone(),
        modified_key: logical,
        physical_key: wire::keyboard::Physical::Unidentified(
            wire::keyboard::NativeCode::Unidentified,
        ),
        location: wire::keyboard::Location::Standard,
        modifiers: wire::keyboard::Modifiers {
            shift: key.modifiers.shift,
            control: key.modifiers.control,
            alt: key.modifiers.alt,
            logo: key.modifiers.platform,
        },
    }
}
