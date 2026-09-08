//! The page document as a HOST SURFACE behind the pages view's slot. The
//! view (`crates/views/pages`) is a pure tree; the editor buffer, its history
//! and its save tick are the app's, so the app paints `page_document` into
//! the `page_document` slot the view leaves, from what the tab was last
//! drawn with. What the reader does in it is queued here; the guest hears
//! only that something happened (an `edited` intent), and the handler takes
//! the events back one at a time through [`page_document_take`].

use super::{PageEvent, page_document};
use iced::advanced::widget::{Operation, Tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use iced::widget::text_editor::Content;
use iced::{Element, Event, Length, Rectangle, Size, Vector, widget};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};
use ui_lang_runtime::view_tree::Surface;
use ui_lang_wire::SurfaceValue;

/// What the document was last drawn with: a copy of the buffer and the
/// readings the editor wears (the theme, the disabled state, the blocks and
/// the commented targets for the margin badges).
#[derive(Clone)]
struct Shown {
    document: Content,
    dark: bool,
    disabled: bool,
    blocks: Vec<crate::backend::PageBlock>,
    hits: Vec<String>,
}

#[derive(Default)]
struct Cell {
    shown: Option<Shown>,
    events: VecDeque<PageEvent>,
}

fn cell() -> &'static Mutex<Cell> {
    static CELL: OnceLock<Mutex<Cell>> = OnceLock::new();
    CELL.get_or_init(Mutex::default)
}

/// Stash what the pages tab is being drawn with, for the surface to paint.
// ponytail: the buffer is cloned once per render (the Ice state owns it and
// the surface needs its own); an Arc'd buffer is the upgrade if a page ever
// grows long enough for that copy to show in a frame.
pub(crate) fn show(
    document: &Content,
    dark: bool,
    disabled: bool,
    blocks: &[crate::backend::PageBlock],
    hits: &[String],
) {
    cell().lock().expect("page surface").shown = Some(Shown {
        document: copy(document),
        dark,
        disabled,
        blocks: blocks.to_vec(),
        hits: hits.to_vec(),
    });
}

/// The buffer for the surface to paint: `Content::clone` is `with_text`,
/// so the copy starts with its caret on the title and no selection — and
/// painted as-is it showed every reader a caret that never moved, a current
/// line that never changed, and no slash menu (which reads the caret too).
fn copy(document: &Content) -> Content {
    let mut copy = document.clone();
    copy.move_to(document.cursor());
    copy
}

/// The oldest event the surface queued, for the handler an `edited` intent
/// runs; one intent is delivered per event, so the queue never runs dry —
/// an empty link (which every reader treats as "not my turn") if it does.
pub fn page_document_take() -> PageEvent {
    cell()
        .lock()
        .expect("page surface")
        .events
        .pop_front()
        .unwrap_or_else(|| PageEvent::OpenLink(String::new()))
}

/// The provider the host registers under `page_document`: the editor over
/// what [`show`] last stashed, its events queued for [`page_document_take`].
pub(crate) fn provider() -> Surface {
    Arc::new(|_key: &str, _args: &[SurfaceValue]| {
        let shown = cell().lock().expect("page surface").shown.clone();
        match shown {
            Some(shown) => Element::new(PageSurface(shown)),
            None => widget::Space::new().into(),
        }
    })
}

/// The editor as a `'static` element: the widget owns the buffer copy and
/// builds `page_document` over it inside every call, so the borrowed editor
/// never has to outlive a frame. The editor's own state lives in the one
/// child tree.
struct PageSurface(Shown);

impl PageSurface {
    fn build(&self) -> Element<'_, PageEvent> {
        let shown = &self.0;
        page_document(
            &shown.document,
            shown.dark,
            shown.disabled,
            &shown.blocks,
            &shown.hits,
        )
    }
}

impl Widget<SurfaceValue, iced::Theme, iced::Renderer> for PageSurface {
    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Fill,
        }
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(self.build())]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&self.build()]);
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.build()
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
        self.build().as_widget().draw(
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
        self.build()
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, SurfaceValue>,
        viewport: &Rectangle,
    ) {
        let mut events = Vec::new();
        let mut local = Shell::new(&mut events);
        self.build().as_widget_mut().update(
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
        if events.is_empty() {
            return;
        }
        // One published unit per event: the host turns each into one
        // `edited` intent, and the handler takes one event per intent.
        let mut queue = cell().lock().expect("page surface");
        for event in events {
            queue.events.push_back(event);
            shell.publish(SurfaceValue::Unit);
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
        self.build().as_widget().mouse_interaction(
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
    ) -> Option<overlay::Element<'a, SurfaceValue, iced::Theme, iced::Renderer>> {
        // The editor draws its menu inside its own bounds; it has no overlay,
        // and one could not outlive the editor built for this call.
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::{PageAction, menu};
    use iced::widget::text_editor::{Action, Edit, Motion, Position};

    fn at_end(text: &str) -> Content {
        let mut document = Content::with_text(text);
        document.perform(Action::Move(Motion::DocumentEnd));
        document
    }

    /// The caret (and a selection) the reader has is the one the surface paints.
    #[test]
    fn the_copy_the_surface_paints_keeps_the_caret_and_the_selection() {
        let document = at_end("Title\nhello");
        assert_eq!(document.cursor().position, Position { line: 1, column: 5 });
        assert_eq!(copy(&document).cursor(), document.cursor());

        let mut selecting = at_end("Title\nhello");
        selecting.perform(Action::Select(Motion::WordLeft));
        let cursor = selecting.cursor();
        assert_eq!(
            cursor.selection,
            Some(Position { line: 1, column: 5 }),
            "{cursor:?}"
        );
        assert_eq!(copy(&selecting).cursor(), cursor);
    }

    /// A `/` typed into the page opens the palette; the surface, painting
    /// its copy, must still see it up — the palette reads the caret.
    #[test]
    fn a_slash_typed_into_the_page_is_up_on_the_surface() {
        let mut document = at_end("Title\nhello ");
        let action = PageAction::Edit(Action::Edit(Edit::Insert('/')));
        document.perform(Action::Edit(Edit::Insert('/')));
        menu::after_action(&document, &action);
        assert!(menu::current(&document).is_some(), "the palette is up");
        let painted = copy(&document);
        let shown = menu::current(&painted).expect("the surface shows the palette");
        assert!(
            shown.items.iter().any(|item| item.tag == "bullet"),
            "{:?}",
            shown.items.iter().map(|item| &item.tag).collect::<Vec<_>>()
        );
        menu::close();
    }
}
