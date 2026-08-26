//! A press observer that reports the cursor position of a left press.
//!
//! iced's [`iced::widget::MouseArea`] stops once its content captures an event,
//! so a press over an interactive child (a button, an input) never reaches the
//! area's own handlers — and its `on_press` carries no position anyway. The
//! only way to learn *where* a press landed used to be streaming `on_move`
//! into application state, which republishes on every cursor pixel and forces
//! a full view rebuild per move. This widget replaces that stream: it wraps one
//! child, forwards every event to it untouched, and after the child has run —
//! captured or not — publishes `on_press_at(position)` in the widget's own
//! local coordinates for a left press inside its bounds. One message per
//! press, zero per move.
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use iced::{Element, Event, Length, Point, Rectangle, Size, Vector};

/// A press-position observer wrapping a single child.
pub struct PressArea<
    'a,
    Message,
    Theme = iced::Theme,
    Renderer = iced::Renderer,
    OnPressAt = fn(Point) -> Message,
> {
    content: Element<'a, Message, Theme, Renderer>,
    on_press_at: Option<OnPressAt>,
}

/// Creates a [`PressArea`] around the given content.
pub fn press_area<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> PressArea<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    PressArea {
        content: content.into(),
        on_press_at: None,
    }
}

impl<'a, Message, Theme, Renderer, OnPressAt> PressArea<'a, Message, Theme, Renderer, OnPressAt> {
    /// Sets the callback receiving the local position of a left press.
    #[must_use]
    pub fn on_press_at<NewOnPressAt>(
        self,
        on_press_at: NewOnPressAt,
    ) -> PressArea<'a, Message, Theme, Renderer, NewOnPressAt>
    where
        NewOnPressAt: Fn(Point) -> Message + 'a,
    {
        PressArea {
            content: self.content,
            on_press_at: Some(on_press_at),
        }
    }
}

impl<Message, Theme, Renderer, OnPressAt> Widget<Message, Theme, Renderer>
    for PressArea<'_, Message, Theme, Renderer, OnPressAt>
where
    Message: Clone,
    OnPressAt: Fn(Point) -> Message,
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
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
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

        // Deliberately NOT gated on `shell.is_event_captured()`: the press
        // that matters most lands on an interactive child, which captures it.
        // Observing does not capture, so the child keeps its click.
        if let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event
            && let Some(on_press_at) = &self.on_press_at
            && let Some(position) = cursor.position_in(layout.bounds())
        {
            shell.publish(on_press_at(position));
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

impl<'a, Message, Theme, Renderer, OnPressAt>
    From<PressArea<'a, Message, Theme, Renderer, OnPressAt>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
    OnPressAt: Fn(Point) -> Message + 'a,
{
    fn from(area: PressArea<'a, Message, Theme, Renderer, OnPressAt>) -> Self {
        Self::new(area)
    }
}
