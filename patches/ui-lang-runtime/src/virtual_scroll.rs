//! Keeps `virtual-row` content mounted over the viewport its scrollable
//! actually shows.
//!
//! A virtual column seeds its window from the viewport it REMEMBERS, and two
//! moments leave that memory wrong with nothing else re-opening layout:
//!
//! - **The first frame, and every children replacement.** Before any event
//!   lands the column has no viewport at all, and the scrollable can be
//!   showing either end of the strip — an end-anchored one (a chat timeline, a
//!   transcript) shows the BOTTOM. So the column mounts nothing on that pass
//!   rather than guess, and this wrapper re-reads the scrollable's real
//!   translation inside `layout` and lays out again against it — as many
//!   times as it takes to settle, because measuring rows moves the content
//!   height and an end-anchored translation is read off that height. Rows are
//!   shaped in `layout`, so the pass that draws is the only one that ever
//!   shapes a row: a guessed window would be a whole screenful of shaping
//!   thrown away on every room switch.
//! - **A rapid wheel transaction.** Iced deliberately stops forwarding
//!   consecutive wheel events to the scrollable's descendants, so a fast
//!   trackpad burst can translate every mounted row out of the viewport.
//!   The wrapper synchronizes after the scrollable consumes each wheel event
//!   and requests layout only when the mounted overscan no longer covers it.

use iced::advanced::widget::{Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use iced::{Element, Event, Length, Rectangle, Size, Vector};

use crate::virtual_children::sync_virtual_columns;

/// How many times one layout call will re-aim a window that escaped the
/// viewport before leaving the rest to the next frame.
///
/// A turn only repeats while it is measuring rows nobody had measured yet, so
/// the walk is finite on its own; this caps what a single frame will spend
/// crossing a long stretch of guessed heights — a wheel flick over scrollback
/// nobody has read. Whatever is left over resolves over the following frames,
/// which is what every frame used to do.
const RE_AIMS_PER_FRAME: usize = 8;

pub struct VirtualScroll<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
}

pub fn virtual_scroll<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> VirtualScroll<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    VirtualScroll {
        content: content.into(),
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for VirtualScroll<'_, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::stateless()
    }

    fn state(&self) -> tree::State {
        tree::State::None
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let mut node = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        // The columns below seeded their windows from remembered viewports;
        // the scrollable's translation in THIS layout is where they really
        // are. Re-aim on the frames where a window escaped it — the first
        // frame and children replacements — so every drawn frame is aligned
        // without waiting on an invalidation nothing else raises.
        //
        // Re-aiming can escape AGAIN, which is why this loops rather than
        // taking one extra pass. Measuring a row replaces its estimate, that
        // moves the content height, and an end-anchored scrollable reads its
        // translation off the content height — so the viewport the second pass
        // mounted against is no longer where the frame will draw. Settling
        // that here is what makes the steady state steady: each turn measures
        // rows the last one had only guessed at, so the walk shortens and
        // stops, and the alternative is paying one turn of it per frame
        // forever while the reader watches the content slide.
        for _ in 0..RE_AIMS_PER_FRAME {
            if !sync_virtual_columns(
                &mut self.content,
                &mut tree.children[0],
                Layout::new(&node),
                renderer,
            ) {
                break;
            }
            // An escaped window re-aims below memoized layouts whose keys
            // cannot see the move; drop them so the next pass really
            // recomputes.
            let mut bust = crate::virtual_children::BustMemoLayouts;
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                Layout::new(&node),
                renderer,
                &mut bust,
            );
            node = self
                .content
                .as_widget_mut()
                .layout(&mut tree.children[0], renderer, limits);
        }
        node
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        if matches!(event, Event::Mouse(mouse::Event::WheelScrolled { .. }))
            && shell.is_event_captured()
            && sync_virtual_columns(&mut self.content, &mut tree.children[0], layout, renderer)
        {
            shell.invalidate_layout();
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message, Theme, Renderer> From<VirtualScroll<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(scroll: VirtualScroll<'a, Message, Theme, Renderer>) -> Self {
        Self::new(scroll)
    }
}
