//! The shell composer as a HOST SURFACE. The Shell tab is a module-owned
//! view (`crates/views/shell`), and the tree wire carries no rich editor —
//! so the view leaves a `shell_composer` slot and the app paints the editor
//! and its send button there, over `crate::editor`'s `RichTextEditor`.
//!
//! THE WORDS NEVER CROSS THE WIRE. The one document lives here for the life
//! of the process; a submit leaves as the `send` intent carrying the trimmed
//! body, and the app's own reset ([`clear`]) empties it. `text_editor`
//! borrows its `Content` for the widget's lifetime and the rendered tree is
//! `'static`, so — like the runtime's own host editor — the widget holds the
//! shared document and builds the real composer afresh inside every
//! `Widget` method, over a lock held for that call alone. The `Tree` state
//! is the built composer's own, so focus and caret carry over between calls.
//!
//! THE TREE THE LAYOUT WAS MADE FOR. iced lays a tree out once and walks
//! that layout with every later call, so the document `build` reads may
//! change only where a layout follows before any walk: the runtime's build
//! (`children` or `diff`, then `layout`) and the event walk (`update`, then
//! the relayout it asks for). The app's reset runs between frames, so it
//! never reaches the document: it queues in the slot's inbox and the widget
//! takes it in ([`take_inputs`]) at exactly those two points, the shape
//! `composer_surface`'s lint test holds for both host composers.

use std::sync::{Mutex, MutexGuard, OnceLock};

use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use iced::widget::text_editor::Content;
use iced::{Border, Element, Event, Length, Rectangle, Size, Vector, widget};
use ui_lang_runtime::view_tree::Surface;
use ui_lang_wire::SurfaceValue as Value;

use crate::editor::{ComposerEvent, apply_composer_event, rich_composer};

/// What the app hands the composer between frames, held in the inbox until
/// the widget takes it in where a layout follows.
enum Input {
    /// The draft emptied: a new chat, or a workspace reset.
    Clear,
}

/// The one seat: the document the painted composer reads and the inbox
/// the app writes.
#[derive(Default)]
struct Slot {
    document: Content,
    inbox: Vec<Input>,
}

fn slot() -> &'static Mutex<Slot> {
    static SLOT: OnceLock<Mutex<Slot>> = OnceLock::new();
    SLOT.get_or_init(Mutex::default)
}

fn lock() -> MutexGuard<'static, Slot> {
    // A panic while the lock was held leaves the slot usable: the widget
    // only ever reads it here and applies whole interactions.
    slot()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Empties the draft: a new chat, or a workspace reset. It waits in the
/// inbox until the composer next takes its inputs in.
pub fn clear() {
    lock().inbox.push(Input::Clear);
}

/// The app's inputs taken into the document, and whether it changed for
/// them. This is the ONLY writer of the document outside the reader's own
/// events, and it runs only where a layout follows before any walk of the
/// tree: `children`, `diff`, and `update` after its event walk.
fn take_inputs(slot: &mut Slot) -> bool {
    let inputs = std::mem::take(&mut slot.inbox);
    if inputs.is_empty() {
        return false;
    }
    let before = slot.document.text();
    for input in inputs {
        match input {
            Input::Clear => slot.document = Content::new(),
        }
    }
    slot.document.text() != before
}

/// The `shell_composer` surface: `(hint, disabled)`, as the view declares
/// it. A submit is published as the trimmed body.
pub fn provider() -> Surface {
    std::sync::Arc::new(|_key: &str, args: &[Value]| {
        let [Value::Str(hint), Value::Bool(disabled)] = args else {
            return widget::text("invalid shell_composer arguments").into();
        };
        Element::new(Composer {
            hint: hint.clone(),
            disabled: *disabled,
        })
    })
}

/// The `send` intent a surface value is, if it is one.
pub fn intent(value: &Value) -> Option<crate::module_view::ModuleViewEvent> {
    let Value::Str(body) = value else {
        return None;
    };
    Some(crate::module_view::ModuleViewEvent {
        kind: "send".into(),
        detail: serde_json::json!({ "body": body }).to_string(),
    })
}

struct Composer {
    hint: String,
    disabled: bool,
}

impl Composer {
    /// The child tree diffed against the element `document` builds. iced
    /// does this on a rebuild of the app's view and never on a relayout, so
    /// a document change inside `update` does it itself, before the layout
    /// that reuses the tree.
    fn diff_document(&self, tree: &mut Tree, document: &Content) {
        tree.diff_children(&[self.build(document).as_widget()]);
    }

    /// One editor event applied to the document; a submit that goes
    /// through is the body published to the guest tree.
    fn apply(&self, document: &mut Content, event: ComposerEvent) -> Option<Value> {
        if let ComposerEvent::Submit = event {
            let body = document.text().trim().to_owned();
            if self.disabled || body.is_empty() {
                return None;
            }
            *document = Content::new();
            return Some(Value::Str(body));
        }
        let content = std::mem::take(document);
        *document = apply_composer_event(content, event);
        None
    }

    /// The composer for this call, over the locked document: the editor
    /// and the round send button — the composer row of screens/shell.ice
    /// before the port, shape for shape.
    fn build<'a>(&'a self, document: &'a Content) -> Element<'a, ComposerEvent> {
        let empty = document.text().trim().is_empty();
        let editor = rich_composer(
            document,
            self.hint.clone(),
            self.disabled,
            false,
            40.0,
            150.0,
            8.0,
        );
        // Regular weight, deliberately — see the note on the message toolbar
        // in components/chat.ice: a semibold string label sends every
        // non-ASCII glyph down cosmic-text's walk-every-face fallback path.
        let send = widget::button(widget::text("↑").size(12.5).center())
            .width(32)
            .height(32)
            .padding(0)
            .style(send_button)
            .on_press_maybe((!self.disabled && !empty).then_some(ComposerEvent::Submit));
        widget::row![editor, send]
            .spacing(6)
            .align_y(iced::Alignment::Center)
            .width(Length::Fill)
            .into()
    }
}

fn send_button(theme: &iced::Theme, status: widget::button::Status) -> widget::button::Style {
    let palette = crate::backend::app_tokens(theme).palette;
    let (background, text) = match status {
        widget::button::Status::Disabled => (palette.disabled, palette.disabled_foreground),
        widget::button::Status::Hovered | widget::button::Status::Pressed => {
            (palette.primary_hover, palette.primary_foreground)
        }
        widget::button::Status::Active => (palette.primary, palette.primary_foreground),
    };
    widget::button::Style {
        background: Some(background.into()),
        text_color: text,
        border: Border::default().rounded(16.0),
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
        let mut slot = lock();
        take_inputs(&mut slot);
        vec![Tree::new(self.build(&slot.document).as_widget())]
    }

    fn diff(&self, tree: &mut Tree) {
        let mut slot = lock();
        take_inputs(&mut slot);
        self.diff_document(tree, &slot.document);
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let slot = lock();
        self.build(&slot.document)
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
        let slot = lock();
        self.build(&slot.document).as_widget().draw(
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
        let slot = lock();
        self.build(&slot.document).as_widget_mut().operate(
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
        let mut slot = lock();
        let mut events = Vec::new();
        let mut local = Shell::new(&mut events);
        self.build(&slot.document).as_widget_mut().update(
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
        // The reader acted on the tree she saw, so her events land before
        // the app's inputs: a body she submitted leaves before a reset
        // that arrived meanwhile empties the box.
        let reader_acted = !events.is_empty();
        for event in events {
            if let Some(submitted) = self.apply(&mut slot.document, event) {
                shell.publish(submitted);
            }
        }
        let inputs_taken = take_inputs(&mut slot);
        let document_changed = reader_acted || inputs_taken;
        if !document_changed {
            return;
        }
        self.diff_document(tree, &slot.document);
        shell.invalidate_layout();
        shell.request_redraw();
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let slot = lock();
        self.build(&slot.document).as_widget().mouse_interaction(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_submit_leaves_as_the_trimmed_body_and_empties_the_draft() {
        let composer = Composer {
            hint: String::new(),
            disabled: false,
        };
        let mut document = Content::with_text("  ship it  ");
        assert_eq!(
            composer.apply(&mut document, ComposerEvent::Submit),
            Some(Value::Str("ship it".into()))
        );
        assert_eq!(document.text().trim(), "");
        assert!(
            composer
                .apply(&mut document, ComposerEvent::Submit)
                .is_none()
        );
        let held = Composer {
            hint: String::new(),
            disabled: true,
        };
        let mut document = Content::with_text("later");
        assert!(held.apply(&mut document, ComposerEvent::Submit).is_none());
        assert_eq!(document.text().trim(), "later");
    }

    #[test]
    fn a_body_is_the_send_intent_and_a_unit_is_not() {
        let event = intent(&Value::Str("hi".into())).expect("a send");
        assert_eq!(event.kind, "send");
        assert_eq!(event.detail, r#"{"body":"hi"}"#);
        assert!(intent(&Value::Unit).is_none());
    }

    /// THE APP'S RESET LANDS BETWEEN FRAMES. The words the window laid the
    /// editor out over stay its words for every walk of that layout — the
    /// operation, the draw — and the box empties at the next update, under
    /// the relayout that update runs.
    #[test]
    fn a_clear_written_between_frames_waits_for_the_next_update() {
        use iced::advanced::clipboard;
        use iced::keyboard;
        use iced_test::runtime::user_interface::{self, UserInterface};

        struct Walk;
        impl Operation for Walk {
            fn traverse(&mut self, visit: &mut dyn FnMut(&mut dyn Operation)) {
                visit(self);
            }
        }
        fn key(character: &str, code: keyboard::key::Code) -> Event {
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Character(character.into()),
                modified_key: keyboard::Key::Character(character.into()),
                physical_key: keyboard::key::Physical::Code(code),
                location: keyboard::Location::Standard,
                modifiers: keyboard::Modifiers::empty(),
                text: Some(character.into()),
                repeat: false,
            })
        }

        // the one document is the process's, so the box starts as the
        // reset leaves it
        take_inputs(&mut lock());
        lock().document = Content::new();
        let composer = Element::<Value>::new(Composer {
            hint: String::new(),
            disabled: false,
        });
        let mut renderer = crate::frame_probe::headless_renderer();
        let size = Size::new(600.0, 200.0);
        let mut clipboard = clipboard::Null;
        let mut published: Vec<Value> = Vec::new();
        let mut ui =
            UserInterface::build(composer, size, user_interface::Cache::new(), &mut renderer);
        let position = iced::Point::new(100.0, 20.0);
        let cursor = mouse::Cursor::Available(position);
        ui.update(
            &[
                Event::Mouse(mouse::Event::CursorMoved { position }),
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                key("h", keyboard::key::Code::KeyH),
                key("i", keyboard::key::Code::KeyI),
            ],
            cursor,
            &mut renderer,
            &mut clipboard,
            &mut published,
        );
        assert_eq!(
            lock().document.text().trim_end(),
            "hi",
            "the press focused, the keys typed"
        );

        // between frames: the app resets the draft
        clear();

        // every walk of the laid-out tree is over the words it was laid
        // out for
        ui.operate(&renderer, &mut Walk);
        ui.draw(
            &mut renderer,
            &iced::Theme::Dark,
            &renderer::Style::default(),
            cursor,
        );
        assert_eq!(
            lock().document.text().trim_end(),
            "hi",
            "the reset waits for the next update"
        );

        // the next update takes the reset in and relayouts over it
        ui.update(
            &[Event::Window(iced::window::Event::RedrawRequested(
                std::time::Instant::now(),
            ))],
            cursor,
            &mut renderer,
            &mut clipboard,
            &mut published,
        );
        assert_eq!(lock().document.text().trim_end(), "", "the box emptied");
        ui.draw(
            &mut renderer,
            &iced::Theme::Dark,
            &renderer::Style::default(),
            cursor,
        );
        assert!(published.is_empty(), "no send");
    }
}
