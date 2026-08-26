//! Keeps a scrollable's visible rows still when content lands *above* them.
//!
//! A live list that puts the newest row on top — a trade tape, a fills list —
//! grows at the end a reader is not looking at. iced stores a scroll offset as
//! an absolute distance from the top of the content (`Anchor::Start`) and
//! never revises it when the content changes: `diff` touches the child tree and
//! nothing else. So a beat that prepends four rows leaves the offset where it
//! was and moves every row under the reader down by four rows' worth of pixels.
//! Measured on the trading terminal, a reader 120px into the recent fills had
//! the row they were on move from y=1024 to y=1128 on one beat.
//!
//! `Anchor::End` is iced's answer to this and it is the wrong one here: it
//! stores the offset as a distance from the *bottom*, which does hold the rows
//! still, but it also makes offset zero mean the bottom — so a list resting
//! where it is supposed to rest, on the newest row, would open on its oldest.
//! A list wants the start anchor's resting place and the end anchor's
//! correction, which is what this widget is.
//!
//! It wraps one scrollable, watches its content height across layout passes,
//! and when the content has grown while the reader is scrolled away from the
//! top it scrolls by exactly the growth. A reader sitting at the top is left
//! alone: offset zero already means "the newest row", and that is the one place
//! where following the content is what a reader wants.
//!
//! What it reads is *growth*, which is the whole of what a wrapper around a
//! scrollable can know: the widget below it is a box of pixels with a height,
//! not a list with row identities. A list that has reached a cap and is now
//! sliding rows off its far end has a constant height and no growth to read, so
//! a reader scrolled into one still watches it slide — correcting that needs
//! the row model, which is where `virtual_list` already keeps its own anchor
//! (`RowsMeasured`, the `anchor`/`anchor_gap` pair). Fixing it here would mean
//! carrying keys through the scroll boundary; reach for the virtualized list
//! instead when a capped live list has to be read while it moves.

use iced::advanced::widget::operation::scrollable::AbsoluteOffset;
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use iced::{Element, Event, Length, Rectangle, Size, Vector};

/// A scroll-anchoring wrapper around a single scrollable.
pub struct ScrollAnchor<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
}

/// Wraps `content` — a scrollable — so that content arriving above the
/// viewport does not move what the reader is looking at.
pub fn scroll_anchor<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> ScrollAnchor<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    ScrollAnchor {
        content: content.into(),
    }
}

/// What the last layout pass saw. Both halves matter: the correction is only
/// sound while the *viewport* holds still, because a narrower viewport reflows
/// rows and grows the content without anything having been inserted.
#[derive(Default)]
struct AnchorState {
    content_height: Option<f32>,
    viewport_height: Option<f32>,
}

/// Reads the wrapped scrollable's geometry and applies the correction in the
/// same walk — `Operation::scrollable` hands over the content bounds, the
/// current translation and the scroll state together, so nothing has to be
/// carried between two passes.
struct Anchor {
    previous_content: Option<f32>,
    previous_viewport: Option<f32>,
    seen: Option<(f32, f32)>,
}

impl Operation for Anchor {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<()>)) {
        operate(self);
    }

    fn scrollable(
        &mut self,
        _id: Option<&iced::advanced::widget::Id>,
        bounds: Rectangle,
        content_bounds: Rectangle,
        translation: Vector,
        state: &mut dyn iced::advanced::widget::operation::Scrollable,
    ) {
        // The wrapped scrollable is the first this walk reaches — a container
        // operates on itself before its children — so a nested scrollable
        // inside the list keeps its own offset.
        if self.seen.is_some() {
            return;
        }
        self.seen = Some((content_bounds.height, bounds.height));

        let (Some(previous_content), Some(previous_viewport)) =
            (self.previous_content, self.previous_viewport)
        else {
            // The first pass has nothing to compare against.
            return;
        };
        // Half a logical pixel: layout arithmetic on fills and fractional
        // scales does not land on the same float twice.
        if (bounds.height - previous_viewport).abs() > 0.5 {
            return;
        }
        let grown = content_bounds.height - previous_content;
        if grown <= 0.5 || translation.y <= 0.5 {
            return;
        }
        state.scroll_by(AbsoluteOffset { x: 0.0, y: grown }, bounds, content_bounds);
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for ScrollAnchor<'_, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<AnchorState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(AnchorState::default())
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
        let node = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        // Layout is where the growth becomes visible and the only place it can
        // be corrected before the frame is drawn — the offset is applied as a
        // translation at draw time, so a correction landing here still shows
        // this frame. `virtual_list` reaches its own scrollable the same way.
        let state = tree.state.downcast_ref::<AnchorState>();
        let mut anchor = Anchor {
            previous_content: state.content_height,
            previous_viewport: state.viewport_height,
            seen: None,
        };
        self.content.as_widget_mut().operate(
            &mut tree.children[0],
            Layout::new(&node),
            renderer,
            &mut anchor,
        );
        if let Some((content_height, viewport_height)) = anchor.seen {
            let state = tree.state.downcast_mut::<AnchorState>();
            state.content_height = Some(content_height);
            state.viewport_height = Some(viewport_height);
        }
        node
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
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

impl<'a, Message, Theme, Renderer> From<ScrollAnchor<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(anchor: ScrollAnchor<'a, Message, Theme, Renderer>) -> Self {
        Self::new(anchor)
    }
}
