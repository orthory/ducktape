//! Draw-time hover: a base child, an optional hover tint, and a reveal child
//! that exists while the cursor is over the widget — or while the application
//! declares it HELD OPEN — with no application state involved in the hover
//! itself.
//!
//! The state-driven alternative (an `enter=`/`exit=` route flipping a
//! `hovered_x` field) republishes on every row crossing and rebuilds the
//! whole view each time; sweep a 150-row chat stream and the hover highlight
//! trails the cursor by hundreds of milliseconds while the queue drains.
//! This widget moves the decision to the passes that already know the cursor:
//! `draw` paints the tint and the reveal only under the cursor, `update` and
//! `mouse_interaction` forward to the reveal only while it is visible. A
//! cached `lazy` row can therefore keep its hover toolbar — hovering changes
//! no dependency, dispatches nothing, and reveals in the same frame.
//!
//! [`HoverReveal::open`] is the one thing the application does own, and it
//! exists because a reveal is often a TRIGGER: press the toolbar's emoji
//! button and a popover opens somewhere else, anchored on a toolbar that the
//! next mouse move erases — a card floating over nothing. `open` is the
//! popover's own openness handed back, so the trigger outlives the pointer
//! and dies with the thing it opened.
//!
//! Layout note: BOTH children are laid out every pass (the reveal must have
//! bounds the moment the cursor arrives, and layout may not read the cursor),
//! stacked over the same bounds like `zstack` — the base sizes the widget,
//! the reveal is positioned within it by its own alignment/padding markup.

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Shell, Widget, mouse, overlay, renderer};
use iced::{Color, Element, Event, Length, Rectangle, Size, Vector};

/// A two-child hover container: `base` always, `reveal` under the cursor.
pub struct HoverReveal<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    base: Element<'a, Message, Theme, Renderer>,
    reveal: Element<'a, Message, Theme, Renderer>,
    /// Painted over the widget's bounds while revealed, under both children.
    tint: Option<Color>,
    radius: f32,
    /// Holds the reveal open whatever the cursor is doing.
    open: bool,
}

/// Creates a [`HoverReveal`] over the given base and reveal children.
pub fn hover_reveal<'a, Message, Theme, Renderer>(
    base: impl Into<Element<'a, Message, Theme, Renderer>>,
    reveal: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> HoverReveal<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    HoverReveal {
        base: base.into(),
        reveal: reveal.into(),
        tint: None,
        radius: 0.0,
        open: false,
    }
}

impl<'a, Message, Theme, Renderer> HoverReveal<'a, Message, Theme, Renderer> {
    /// Sets the hover tint painted under the children while revealed.
    #[must_use]
    pub fn tint(mut self, tint: Color) -> Self {
        self.tint = Some(tint);
        self
    }

    /// Sets the tint's corner radius.
    #[must_use]
    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    /// Holds the reveal open regardless of the cursor — pass the openness of
    /// whatever the reveal's controls opened, so a trigger cannot vanish out
    /// from under its own popover.
    #[must_use]
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }
}

/// Whether the reveal was up on the LAST update pass — draw and interaction
/// read the live cursor themselves; this only lets `update` request a redraw
/// exactly when the reveal edge flips.
#[derive(Default)]
struct HoverState {
    revealed: bool,
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for HoverReveal<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: iced::advanced::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<HoverState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(HoverState::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.base), Tree::new(&self.reveal)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&self.base, &self.reveal]);
    }

    fn size(&self) -> Size<Length> {
        self.base.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.base.as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let base = self
            .base
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        let size = base.size();
        let reveal = self.reveal.as_widget_mut().layout(
            &mut tree.children[1],
            renderer,
            &layout::Limits::new(Size::ZERO, size),
        );
        layout::Node::with_children(size, vec![base, reveal])
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let mut children = layout.children();
        let base_layout = children.next().expect("hover base layout");
        let reveal_layout = children.next().expect("hover reveal layout");
        self.base
            .as_widget_mut()
            .operate(&mut tree.children[0], base_layout, renderer, operation);
        self.reveal.as_widget_mut().operate(
            &mut tree.children[1],
            reveal_layout,
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
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let revealed = self.open || cursor.is_over(bounds);
        let mut children = layout.children();
        let base_layout = children.next().expect("hover base layout");
        let reveal_layout = children.next().expect("hover reveal layout");

        // The reveal sees events first while visible — it floats OVER the
        // base, so its buttons must win the press.
        if revealed {
            self.reveal.as_widget_mut().update(
                &mut tree.children[1],
                event,
                reveal_layout,
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            );
        }
        if !shell.is_event_captured() {
            self.base.as_widget_mut().update(
                &mut tree.children[0],
                event,
                base_layout,
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            );
        }

        // Repaint exactly on the reveal edge — the passes that draw the tint
        // and the reveal read the cursor themselves but run only per frame.
        let state = tree.state.downcast_mut::<HoverState>();
        if state.revealed != revealed {
            state.revealed = revealed;
            shell.request_redraw();
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
        let bounds = layout.bounds();
        let mut children = layout.children();
        let base_layout = children.next().expect("hover base layout");
        let reveal_layout = children.next().expect("hover reveal layout");
        if self.open || cursor.is_over(bounds) {
            let reveal = self.reveal.as_widget().mouse_interaction(
                &tree.children[1],
                reveal_layout,
                cursor,
                viewport,
                renderer,
            );
            if reveal != mouse::Interaction::None {
                return reveal;
            }
        }
        self.base.as_widget().mouse_interaction(
            &tree.children[0],
            base_layout,
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
        let bounds = layout.bounds();
        let revealed = self.open || cursor.is_over(bounds);
        let mut children = layout.children();
        let base_layout = children.next().expect("hover base layout");
        let reveal_layout = children.next().expect("hover reveal layout");

        if revealed && let Some(tint) = self.tint {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: iced::Border {
                        radius: self.radius.into(),
                        ..iced::Border::default()
                    },
                    shadow: iced::Shadow::default(),
                    snap: false,
                },
                tint,
            );
        }
        self.base.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            base_layout,
            cursor,
            viewport,
        );
        // THE REVEAL GETS ITS OWN LAYER, exactly like `iced::widget::hover`.
        // Within one renderer layer both backends batch by primitive KIND and
        // not by call order, so an opaque reveal plate drawn after the base
        // still loses to the base's GLYPHS: a chat row's hover toolbar came up
        // with the message text painted straight through its card. Pushing a
        // layer is what makes "drawn later" mean "drawn above".
        if revealed {
            renderer.with_layer(bounds, |renderer| {
                self.reveal.as_widget().draw(
                    &tree.children[1],
                    renderer,
                    theme,
                    style,
                    reveal_layout,
                    cursor,
                    viewport,
                );
            });
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        // Only the base contributes overlays: a reveal holds a strip of
        // direct controls, never a widget that hangs one. The popover a
        // reveal's button OPENS belongs to the application, which hands its
        // openness back through `open`.
        let mut children = layout.children();
        let base_layout = children.next().expect("hover base layout");
        self.base.as_widget_mut().overlay(
            &mut tree.children[0],
            base_layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message, Theme, Renderer> From<HoverReveal<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(hover: HoverReveal<'a, Message, Theme, Renderer>) -> Self {
        Self::new(hover)
    }
}
