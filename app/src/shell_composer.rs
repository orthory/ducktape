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

use std::sync::{Mutex, MutexGuard, OnceLock};

use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use iced::widget::text_editor::Content;
use iced::{Border, Element, Event, Length, Rectangle, Size, Vector, widget};
use ui_lang_runtime::view_tree::Surface;
use ui_lang_wire::SurfaceValue as Value;

use crate::editor::{ComposerEvent, apply_composer_event, rich_composer};

fn document() -> &'static Mutex<Content> {
    static DOCUMENT: OnceLock<Mutex<Content>> = OnceLock::new();
    DOCUMENT.get_or_init(Mutex::default)
}

fn lock() -> MutexGuard<'static, Content> {
    // A panic while the lock was held leaves the document usable: the
    // widget only ever reads it here and applies whole interactions.
    document()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Empties the draft: a new chat, or a workspace reset.
pub fn clear() {
    *lock() = Content::new();
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
        let document = lock();
        vec![Tree::new(self.build(&document).as_widget())]
    }

    fn diff(&self, tree: &mut Tree) {
        let document = lock();
        tree.diff_children(&[self.build(&document).as_widget()]);
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let document = lock();
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
        let document = lock();
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
        let document = lock();
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
        let mut document = lock();
        let mut events = Vec::new();
        let mut local = Shell::new(&mut events);
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
        for event in events {
            if let Some(submitted) = self.apply(&mut document, event) {
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
        let document = lock();
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
}
