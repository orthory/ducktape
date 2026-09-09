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
//! that layout with every later call — a parent's own walk inside its
//! `update` included, before any relayout — so the tree `build` makes is
//! diffed only where a layout follows at once: the runtime's build
//! (`children` or `diff`, then `layout`) and `layout` itself. This
//! composer's tree has one shape whatever the words, so the paint here only
//! notes which revision of the document the tree was laid out with; the
//! reader's events and the app's reset ([`clear`]) move the document and
//! its revision, and `update` answers with a relayout. The shape is the one
//! `composer_surface`'s lint test holds for both host composers.

use std::sync::{Mutex, MutexGuard, OnceLock};

use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use iced::widget::text_editor::Content;
use iced::{Border, Element, Event, Length, Rectangle, Size, Vector, widget};
use ui_lang_runtime::view_tree::Surface;
use ui_lang_wire::SurfaceValue as Value;

use crate::editor::{ComposerEvent, apply_composer_event, rich_composer};

/// The one seat: the document every writer moves (and its revision), and
/// the revision the tree was last laid out with.
#[derive(Default)]
struct Slot {
    document: Content,
    rev: u64,
    painted_rev: u64,
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

/// Empties the draft: a new chat, or a workspace reset.
pub fn clear() {
    let mut slot = lock();
    slot.document = Content::new();
    slot.rev += 1;
}

/// The tree noted as laid out with the document as it stands; whether the
/// document moved since the last paint. Runs only where a layout follows
/// at once: `children`, `diff`, and `layout` before it lays the tree out.
fn paint(slot: &mut Slot) -> bool {
    if slot.painted_rev == slot.rev {
        return false;
    }
    slot.painted_rev = slot.rev;
    true
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
    /// The child tree diffed against the element `content` builds. iced
    /// does this on a rebuild of the app's view and never on a relayout, so
    /// `layout` does it itself whenever it paints, before laying the tree
    /// out.
    fn diff_shape(&self, tree: &mut Tree, content: &Content) {
        tree.diff_children(&[self.build(content).as_widget()]);
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

    /// The composer for this call, over the live words: the editor and the
    /// round send button — the composer row of screens/shell.ice before the
    /// port, shape for shape.
    fn build<'a>(&'a self, content: &'a Content) -> Element<'a, ComposerEvent> {
        let empty = content.text().trim().is_empty();
        let editor = rich_composer(
            content,
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
        paint(&mut slot);
        vec![Tree::new(self.build(&slot.document).as_widget())]
    }

    fn diff(&self, tree: &mut Tree) {
        let mut slot = lock();
        paint(&mut slot);
        self.diff_shape(tree, &slot.document);
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let mut slot = lock();
        let document_moved = paint(&mut slot);
        if document_moved {
            self.diff_shape(tree, &slot.document);
        }
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
        // The reader's events move the document, never the tree: a parent
        // may still walk this tree over the current layout before the
        // relayout asked for below.
        let reader_acted = !events.is_empty();
        for event in events {
            if let Some(submitted) = self.apply(&mut slot.document, event) {
                shell.publish(submitted);
            }
        }
        if reader_acted {
            slot.rev += 1;
        }
        let painted_is_behind = slot.rev != slot.painted_rev;
        if !painted_is_behind {
            return;
        }
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

    /// THE APP'S RESET LANDS BETWEEN FRAMES. It empties the words at once —
    /// the editor is a leaf, so the words are not shape — and every walk of
    /// the laid-out tree still meets the tree that layout was made for; the
    /// next update relayouts over the emptied box.
    #[test]
    fn a_clear_written_between_frames_leaves_the_laid_out_tree_walkable() {
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
        clear();
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

        assert_eq!(lock().document.text().trim_end(), "", "the box emptied");
        // every walk of the laid-out tree meets the tree it was made for
        ui.operate(&renderer, &mut Walk);
        ui.draw(
            &mut renderer,
            &iced::Theme::Dark,
            &renderer::Style::default(),
            cursor,
        );

        // the next update relayouts over the emptied box
        ui.update(
            &[Event::Window(iced::window::Event::RedrawRequested(
                std::time::Instant::now(),
            ))],
            cursor,
            &mut renderer,
            &mut clipboard,
            &mut published,
        );
        ui.draw(
            &mut renderer,
            &iced::Theme::Dark,
            &renderer::Style::default(),
            cursor,
        );
        assert!(published.is_empty(), "no send");
    }
}
