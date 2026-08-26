//! A press-drag handle that grabs the pointer for the whole gesture.
//!
//! iced's [`iced::widget::MouseArea`] only reports `on_move` while the cursor is
//! *inside* the widget's bounds and drops `on_release` once the cursor leaves
//! them. That makes it useless for dragging a thin divider: the moment a fast
//! drag outruns a 6px splitter the move events stop and the release is lost, so
//! the drag wedges "stuck on". This widget instead grabs the pointer on left
//! press and keeps reporting `(dx, dy)` logical-pixel deltas — and the final
//! release — from the *global* cursor stream until the button comes up, no
//! matter where the cursor wanders. It wraps one child (the visible divider)
//! and delegates all layout/draw to it, so it composes anywhere an element does
//! — including inside a component, which `pane_grid` cannot.
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use iced::{Element, Event, Length, Point, Rectangle, Size, Vector};

/// A drag-to-resize handle wrapping a single divider child.
pub struct ResizeHandle<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    on_press: Option<Message>,
    on_release: Option<Message>,
    on_drag: Option<Box<dyn Fn(f64, f64) -> Message + 'a>>,
    interaction: Option<mouse::Interaction>,
}

/// Creates a [`ResizeHandle`] around the given divider content.
pub fn resize_handle<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> ResizeHandle<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    ResizeHandle {
        content: content.into(),
        on_press: None,
        on_release: None,
        on_drag: None,
        interaction: None,
    }
}

impl<'a, Message, Theme, Renderer> ResizeHandle<'a, Message, Theme, Renderer> {
    /// Sets the message emitted when the drag begins.
    #[must_use]
    pub fn on_press(mut self, message: Message) -> Self {
        self.on_press = Some(message);
        self
    }

    /// Sets the message emitted when the drag ends.
    #[must_use]
    pub fn on_release(mut self, message: Message) -> Self {
        self.on_release = Some(message);
        self
    }

    /// Sets the callback receiving `(dx, dy)` logical-pixel deltas per move.
    #[must_use]
    pub fn on_drag(mut self, on_drag: impl Fn(f64, f64) -> Message + 'a) -> Self {
        self.on_drag = Some(Box::new(on_drag));
        self
    }

    /// Sets the cursor shown while hovering or dragging the handle.
    #[must_use]
    pub fn interaction(mut self, interaction: mouse::Interaction) -> Self {
        self.interaction = Some(interaction);
        self
    }
}

/// Local drag state: whether the pointer is grabbed and where it last was.
#[derive(Default)]
struct DragState {
    dragging: bool,
    last: Point,
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for ResizeHandle<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: iced::advanced::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<DragState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(DragState::default())
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

        if shell.is_event_captured() {
            return;
        }

        let state: &mut DragState = tree.state.downcast_mut();
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let grabbing = cursor
                    .position()
                    .filter(|_| cursor.is_over(layout.bounds()));
                let Some(position) = grabbing else {
                    return;
                };
                state.dragging = true;
                state.last = position;
                if let Some(message) = &self.on_press {
                    shell.publish(message.clone());
                }
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) if state.dragging => {
                let dx = f64::from(position.x - state.last.x);
                let dy = f64::from(position.y - state.last.y);
                state.last = *position;
                if let Some(on_drag) = &self.on_drag {
                    shell.publish(on_drag(dx, dy));
                }
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if state.dragging => {
                state.dragging = false;
                if let Some(message) = &self.on_release {
                    shell.publish(message.clone());
                }
                shell.capture_event();
            }
            _ => {}
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
        let state: &DragState = tree.state.downcast_ref();
        let hovered = cursor.is_over(layout.bounds());
        if let Some(interaction) = self.interaction
            && (state.dragging || hovered)
        {
            return interaction;
        }
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

impl<'a, Message, Theme, Renderer> From<ResizeHandle<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(handle: ResizeHandle<'a, Message, Theme, Renderer>) -> Self {
        Self::new(handle)
    }
}
