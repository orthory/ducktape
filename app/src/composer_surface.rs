//! The chat composers as HOST SURFACES. The Chat tab is a module-owned view
//! (`crates/views/chat`), and the tree wire carries no rich editor — so the
//! view leaves a `chat_composer` slot per room and per thread, and the app
//! paints the composer there: the plate, the marks row, Send, the
//! failed-send banner, over `crate::editor`'s `RichTextEditor`.
//!
//! THE WORDS NEVER CROSS THE WIRE. Each slot is keyed by the composer's
//! scope (`composer_scope` / `thread_scope`, spelled by the view exactly as
//! `backend/model.rs` spells them), and the document for a scope lives here
//! for the life of the process — the retained-instance rule of
//! ducktape-ui#697: a draft never rides a room switch, and closing the rail
//! does not discard a reply. A submit leaves as the `composer` intent
//! ([`intent`]) carrying the kind, the trimmed body and a fresh operation
//! id; a refused or failed body comes back through [`unsent`], into the
//! banner of the box it was written in.
//!
//! `text_editor` borrows its `Content` for the widget's lifetime and the
//! rendered tree is `'static`, so — like the runtime's own host editor — the
//! widget here holds the shared document and builds the real composer
//! afresh inside every `Widget` method, over a lock held for that call
//! alone. The `Tree` state is the built composer's own, so focus and caret
//! carry over between calls as they would for a widget built once.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use iced::widget::text_editor::Content;
use iced::{Border, Color, Element, Event, Font, Length, Rectangle, Size, Vector, widget};
use ui_lang_runtime::view_tree::Surface;
use ui_lang_wire::SurfaceValue as Value;

use crate::editor::{ComposerEvent, apply_composer_event, composer_toggle_mark, rich_composer};

/// One composer's state: its words, and the unsent body its last failed
/// send handed back (ducktape-ui#698).
#[derive(Default)]
struct Document {
    content: Content,
    failed: String,
}

type Shared = Arc<Mutex<Document>>;

thread_local! {
    // THE UI THREAD'S OWN. Every composer is painted and edited on the one
    // thread iced runs the app on, and a handler's `unsent` runs there too —
    // so the documents are that thread's, which also keeps every test
    // thread's rooms apart without a window in the key.
    //
    // ponytail: one document per scope for the life of the thread, never
    // evicted — a scope is a room or a thread the reader typed in, which is
    // bounded by how many she visits; add an LRU if a long session shows it.
    static DOCUMENTS: RefCell<HashMap<String, Shared>> = RefCell::default();
}

fn document(scope: &str) -> Shared {
    DOCUMENTS.with_borrow_mut(|documents| documents.entry(scope.to_owned()).or_default().clone())
}

fn lock(document: &Shared) -> MutexGuard<'_, Document> {
    // A panic while the lock was held leaves the document usable: the
    // widget only ever reads it here and applies whole interactions.
    document
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The app's share-out: what a room's or thread's send let go of, handed
/// back to the box it came from. A committed body is not unsent, so it
/// never stashes — `remember_failed_draft` decides, as it did in the view.
pub fn unsent(scope: &str, text: &str, committed: bool) {
    let document = document(scope);
    let mut document = lock(&document);
    document.failed = crate::backend::remember_failed_draft(
        std::mem::take(&mut document.failed),
        "stash".into(),
        text.into(),
        committed,
    );
}

/// The `chat_composer` surface: `(scope, kind, compact, hint, blocked,
/// restore_blocked, failed_note)`, as the view declares it.
pub fn provider() -> Surface {
    Arc::new(|_key: &str, args: &[Value]| {
        let [
            Value::Str(scope),
            Value::Str(kind),
            Value::Bool(compact),
            Value::Str(hint),
            Value::Bool(blocked),
            Value::Bool(restore_blocked),
            Value::Str(failed_note),
        ] = args
        else {
            return widget::text("invalid chat_composer arguments").into();
        };
        Element::new(Composer {
            document: document(scope),
            scope: scope.clone(),
            kind: kind.clone(),
            compact: *compact,
            hint: hint.clone(),
            blocked: *blocked,
            restore_blocked: *restore_blocked,
            failed_note: failed_note.clone(),
        })
    })
}

/// The intent a surface event is, if it is one: a submit crosses as a
/// `composer` record — with the scope it was written in, so the handler
/// that takes it later can tell a box the reader has since left from the
/// one on screen; an edit is the surface's own business and is not.
pub fn intent(value: &Value) -> Option<crate::module_view::ModuleViewEvent> {
    let Value::Record { name, fields } = value else {
        return None;
    };
    if name != "composer" {
        return None;
    }
    let field = |wanted: &str| {
        fields.iter().find_map(|(key, value)| match value {
            Value::Str(text) if key == wanted => Some(text.as_str()),
            _ => None,
        })
    };
    let detail = serde_json::json!({
        "scope": field("scope")?,
        "kind": field("kind")?,
        "body": field("body")?,
        "id": field("id")?,
    });
    Some(crate::module_view::ModuleViewEvent {
        kind: "composer".into(),
        detail: detail.to_string(),
    })
}

/// What the reader did in the composer: an editor event (a keystroke, a
/// paste, a click, a chord, the Send button's synthetic Submit), a mark
/// from the toolbar, or the banner's two buttons.
#[derive(Clone, Debug)]
pub(crate) enum Interaction {
    Editor(ComposerEvent),
    Mark(&'static str),
    Restore,
    Dismiss,
}

struct Composer {
    document: Shared,
    scope: String,
    kind: String,
    compact: bool,
    hint: String,
    blocked: bool,
    restore_blocked: bool,
    failed_note: String,
}

impl Composer {
    /// One interaction applied to the document; a submit that goes through
    /// is the value published to the guest tree.
    fn apply(&self, document: &mut Document, interaction: Interaction) -> Option<Value> {
        match interaction {
            Interaction::Editor(ComposerEvent::Submit) => {
                if self.blocked {
                    return None;
                }
                let body = document.content.text().trim().to_owned();
                if body.is_empty() {
                    return None;
                }
                document.content = Content::new();
                let prefix = if self.kind == "reply" {
                    "reply"
                } else {
                    "message"
                };
                Some(Value::Record {
                    name: "composer".into(),
                    fields: vec![
                        ("scope".into(), Value::Str(self.scope.clone())),
                        ("kind".into(), Value::Str(self.kind.clone())),
                        ("body".into(), Value::Str(body)),
                        (
                            "id".into(),
                            Value::Str(crate::backend::fresh_operation_id(prefix.into())),
                        ),
                    ],
                })
            }
            Interaction::Editor(event) => {
                let content = std::mem::take(&mut document.content);
                document.content = apply_composer_event(content, event);
                None
            }
            Interaction::Mark(glyph) => {
                if self.blocked {
                    return None;
                }
                let content = std::mem::take(&mut document.content);
                document.content = composer_toggle_mark(content, glyph.into());
                None
            }
            Interaction::Restore => {
                let empty = document.content.text().trim().is_empty();
                if self.restore_blocked || document.failed.is_empty() || !empty {
                    return None;
                }
                document.content = Content::with_text(&document.failed);
                document.failed.clear();
                None
            }
            Interaction::Dismiss => {
                document.failed.clear();
                None
            }
        }
    }

    /// The composer for this call, over the locked document: the
    /// failed-send banner (an empty slot when there is none, so the editor
    /// under it keeps its tree position and its caret), then the plate
    /// with the editor, the marks and Send — `ChatComposer` in
    /// components/chat.ice before the port, shape for shape.
    fn build<'a>(&'a self, document: &'a Document) -> Element<'a, Interaction> {
        let empty = document.content.text().trim().is_empty();
        let banner: Element<'a, Interaction> = if document.failed.is_empty() {
            widget::Space::new().into()
        } else {
            let restore = widget::button(widget::text("Restore").size(12.5))
                .padding([5, 9])
                .style(secondary_button)
                .on_press_maybe((!self.restore_blocked && empty).then_some(Interaction::Restore));
            let dismiss = widget::button(widget::text("×").size(14.0).center())
                .width(28)
                .height(28)
                .padding(0)
                .style(ghost_button)
                .on_press(Interaction::Dismiss);
            widget::container(
                widget::row![
                    widget::container(widget::Space::new())
                        .width(6)
                        .height(6)
                        .style(|theme| {
                            let tokens = crate::backend::app_tokens(theme);
                            widget::container::Style {
                                background: Some(tokens.palette.destructive_dot.into()),
                                border: Border::default().rounded(3.0),
                                ..Default::default()
                            }
                        }),
                    widget::text(self.failed_note.as_str())
                        .size(12.5)
                        .line_height(1.45)
                        .width(Length::Fill)
                        .style(|theme| widget::text::Style {
                            color: Some(crate::backend::app_tokens(theme).palette.destructive),
                        }),
                    restore,
                    dismiss,
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .width(Length::Fill),
            )
            .padding([11, 13])
            .width(Length::Fill)
            .style(|theme| {
                let tokens = crate::backend::app_tokens(theme);
                widget::container::Style {
                    background: Some(tokens.palette.destructive_background.into()),
                    border: Border {
                        color: tokens.palette.destructive_line,
                        width: 1.0,
                        radius: 9.0.into(),
                    },
                    ..Default::default()
                }
            })
            .into()
        };
        let editor = rich_composer(
            &document.content,
            self.hint.clone(),
            self.blocked,
            44.0,
            150.0,
            10.0,
        )
        .map(Interaction::Editor);
        let mark = |label: Element<'a, Interaction>, glyph: &'static str| {
            widget::button(widget::container(label).center(Length::Fill))
                .width(26)
                .height(24)
                .padding(0)
                .style(ghost_button)
                .on_press_maybe((!self.blocked).then_some(Interaction::Mark(glyph)))
                .into()
        };
        let marks: Vec<Element<'a, Interaction>> = vec![
            mark(
                widget::text("B")
                    .size(12.5)
                    .font(Font {
                        weight: iced::font::Weight::Bold,
                        ..crate::Ducktape::default_font()
                    })
                    .into(),
                "bold",
            ),
            mark(
                widget::text("I")
                    .size(12.5)
                    .font(Font {
                        style: iced::font::Style::Italic,
                        ..crate::Ducktape::default_font()
                    })
                    .into(),
                "italic",
            ),
            widget::container(widget::Space::new())
                .width(1)
                .height(14)
                .style(|theme| widget::container::Style {
                    background: Some(crate::backend::app_tokens(theme).palette.border.into()),
                    ..Default::default()
                })
                .into(),
            mark(glyph("code-brackets"), "code"),
            mark(glyph("quote"), "quote"),
        ];
        let send = widget::button(widget::text("Send").size(12.5))
            .padding(if self.compact { [6, 11] } else { [7, 12] })
            .height(if self.compact { 28 } else { 29 })
            .style(primary_button)
            .on_press_maybe(
                (!self.blocked && !empty).then_some(Interaction::Editor(ComposerEvent::Submit)),
            );
        let mut tail = widget::row(marks)
            .spacing(2)
            .align_y(iced::Alignment::Center)
            .push(widget::Space::new().width(Length::Fill));
        if !self.compact {
            tail = tail
                .push(
                    widget::text("↵ send · ⇧↵ newline")
                        .size(10.5)
                        .font(Font {
                            family: iced::font::Family::Name("Geist Mono"),
                            weight: iced::font::Weight::Medium,
                            ..Font::DEFAULT
                        })
                        .style(|theme| widget::text::Style {
                            color: Some(crate::backend::app_tokens(theme).palette.muted_foreground),
                        }),
                )
                .push(widget::Space::new().width(8));
        }
        let tail = tail.push(send);
        let plate = widget::container(
            widget::column![
                editor,
                widget::container(tail.width(Length::Fill)).padding(iced::Padding {
                    top: 0.0,
                    right: 8.0,
                    bottom: 8.0,
                    left: 8.0,
                }),
            ]
            .width(Length::Fill),
        )
        .width(Length::Fill)
        .clip(true)
        .style(|theme| {
            let tokens = crate::backend::app_tokens(theme);
            widget::container::Style {
                background: Some(tokens.palette.card.into()),
                border: Border {
                    color: tokens.palette.control_line,
                    width: 1.0,
                    radius: 12.0.into(),
                },
                shadow: tokens.elevation.popover,
                ..Default::default()
            }
        });
        widget::column![banner, plate]
            .spacing(10)
            .width(Length::Fill)
            .into()
    }
}

/// The toolbar glyphs, as the design crate ships them; the button's ink is
/// the svg's.
fn glyph<'a>(name: &str) -> Element<'a, Interaction> {
    widget::svg(widget::svg::Handle::from_memory(crate::backend::icon(name)))
        .width(13)
        .height(13)
        .style(|theme, _| widget::svg::Style {
            color: Some(crate::backend::app_tokens(theme).palette.muted_foreground),
        })
        .into()
}

fn primary_button(theme: &iced::Theme, status: widget::button::Status) -> widget::button::Style {
    let tokens = crate::backend::app_tokens(theme);
    let (background, text) = match status {
        widget::button::Status::Disabled => {
            (tokens.palette.disabled, tokens.palette.disabled_foreground)
        }
        widget::button::Status::Hovered | widget::button::Status::Pressed => (
            tokens.palette.primary_hover,
            tokens.palette.primary_foreground,
        ),
        widget::button::Status::Active => {
            (tokens.palette.primary, tokens.palette.primary_foreground)
        }
    };
    widget::button::Style {
        background: Some(background.into()),
        text_color: text,
        border: Border::default().rounded(tokens.radius.button),
        ..Default::default()
    }
}

fn secondary_button(theme: &iced::Theme, status: widget::button::Status) -> widget::button::Style {
    let tokens = crate::backend::app_tokens(theme);
    let wash = |alpha: f32| Color {
        a: alpha,
        ..tokens.palette.foreground
    };
    let background = match status {
        widget::button::Status::Disabled => wash(0.05),
        widget::button::Status::Hovered => wash(0.14),
        widget::button::Status::Pressed => wash(0.18),
        widget::button::Status::Active => wash(0.09),
    };
    widget::button::Style {
        background: Some(background.into()),
        text_color: match status {
            widget::button::Status::Disabled => tokens.palette.disabled_foreground,
            _ => tokens.palette.foreground,
        },
        border: Border {
            color: wash(0.11),
            width: 1.0,
            radius: 7.0.into(),
        },
        ..Default::default()
    }
}

fn ghost_button(theme: &iced::Theme, status: widget::button::Status) -> widget::button::Style {
    let tokens = crate::backend::app_tokens(theme);
    let wash = |alpha: f32| Color {
        a: alpha,
        ..tokens.palette.foreground
    };
    let (background, text) = match status {
        widget::button::Status::Disabled => {
            (Color::TRANSPARENT, tokens.palette.disabled_foreground)
        }
        widget::button::Status::Hovered => (wash(0.08), tokens.palette.foreground),
        widget::button::Status::Pressed => (wash(0.12), tokens.palette.foreground),
        widget::button::Status::Active => (Color::TRANSPARENT, tokens.palette.muted_foreground),
    };
    widget::button::Style {
        background: Some(background.into()),
        text_color: text,
        border: Border::default().rounded(6.0),
        ..Default::default()
    }
}

impl Widget<Value, iced::Theme, iced::Renderer> for Composer {
    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Shrink,
        }
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<()>()
    }

    fn state(&self) -> tree::State {
        tree::State::None
    }

    fn children(&self) -> Vec<Tree> {
        let document = lock(&self.document);
        vec![Tree::new(self.build(&document).as_widget())]
    }

    fn diff(&self, tree: &mut Tree) {
        let document = lock(&self.document);
        tree.diff_children(&[self.build(&document).as_widget()]);
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let document = lock(&self.document);
        self.build(&document)
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let document = lock(&self.document);
        self.build(&document).as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        let document = lock(&self.document);
        self.build(&document).as_widget_mut().operate(
            &mut tree.children[0],
            layout,
            renderer,
            operation,
        );
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Value>,
        viewport: &Rectangle,
    ) {
        let mut document = lock(&self.document);
        let mut interactions = Vec::new();
        let mut local = Shell::new(&mut interactions);
        self.build(&document).as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            &mut local,
            viewport,
        );
        if local.is_event_captured() {
            shell.capture_event();
        }
        if local.is_layout_invalid() {
            shell.invalidate_layout();
        }
        if local.are_widgets_invalid() {
            shell.invalidate_widgets();
        }
        match local.redraw_request() {
            iced::window::RedrawRequest::NextFrame => shell.request_redraw(),
            iced::window::RedrawRequest::At(at) => shell.request_redraw_at(at),
            iced::window::RedrawRequest::Wait => {}
        }
        shell.input_method_mut().merge(local.input_method());
        for interaction in interactions {
            if let Some(submitted) = self.apply(&mut document, interaction) {
                shell.publish(submitted);
            }
            shell.invalidate_layout();
            shell.request_redraw();
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let document = lock(&self.document);
        self.build(&document).as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'a>(
        &'a mut self,
        _tree: &'a mut Tree,
        _layout: Layout<'a>,
        _renderer: &iced::Renderer,
        _viewport: &Rectangle,
        _translation: Vector,
    ) -> Option<overlay::Element<'a, Value, iced::Theme, iced::Renderer>> {
        // The composer has no overlay, and one could not outlive the lock.
        None
    }
}

/// The app suite's seat at the composer: one interaction on a scope's
/// document, exactly as the painted composer applies it, and the words and
/// the stash as the reader sees them.
#[cfg(test)]
pub(crate) mod testing {
    pub(crate) use super::Interaction;
    use super::*;

    /// A submit that goes through is the value the guest tree publishes.
    pub(crate) fn interact(
        scope: &str,
        kind: &str,
        blocked: bool,
        restore_blocked: bool,
        interaction: Interaction,
    ) -> Option<Value> {
        let composer = Composer {
            document: document(scope),
            scope: scope.into(),
            kind: kind.into(),
            compact: false,
            hint: String::new(),
            blocked,
            restore_blocked,
            failed_note: String::new(),
        };
        let document = composer.document.clone();
        let mut document = lock(&document);
        composer.apply(&mut document, interaction)
    }

    pub(crate) fn text(scope: &str) -> String {
        let document = document(scope);
        lock(&document).content.text()
    }

    pub(crate) fn failed(scope: &str) -> String {
        let document = document(scope);
        lock(&document).failed.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_send_stashes_into_the_box_it_came_from_and_a_committed_one_does_not() {
        unsent("net\u{1f}room-a", "hello", false);
        unsent("net\u{1f}room-a", "again", false);
        unsent("net\u{1f}room-b", "landed", true);
        assert_eq!(lock(&document("net\u{1f}room-a")).failed, "hello\nagain");
        assert_eq!(lock(&document("net\u{1f}room-b")).failed, "");
    }

    #[test]
    fn a_submit_is_the_composer_intent_and_an_edit_is_not() {
        let value = Value::Record {
            name: "composer".into(),
            fields: vec![
                ("scope".into(), Value::Str("net\u{1f}room-a#4".into())),
                ("kind".into(), Value::Str("reply".into())),
                ("body".into(), Value::Str("hi".into())),
                ("id".into(), Value::Str("reply-1".into())),
            ],
        };
        let event = intent(&value).expect("a submit");
        assert_eq!(event.kind, "composer");
        let detail: serde_json::Value = serde_json::from_str(&event.detail).expect("json");
        assert_eq!(detail["scope"], "net\u{1f}room-a#4");
        assert_eq!(detail["kind"], "reply");
        assert_eq!(detail["body"], "hi");
        assert_eq!(detail["id"], "reply-1");
        assert!(intent(&Value::Unit).is_none());
    }
}
