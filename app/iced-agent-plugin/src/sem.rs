//! The `sem` tagging widget: a transparent wrapper that annotates its content
//! with a semantic role/name/value so the [`Collector`](crate::collect) can
//! rebuild an accessibility tree from an `Operation` walk.
//!
//! It delegates every `Widget` method to the inner element (it owns no state,
//! layout, or drawing of its own — same shape as `iced_widget::Themer`) except
//! `operate`, where it brackets the content with `custom` probes carrying the
//! semantic metadata.

use std::any::Any;

use iced::advanced::widget::{tree, Operation, Tree};
use iced::advanced::{layout, mouse, overlay, renderer, Clipboard, Layout, Shell, Widget};
use iced::{Element, Event, Length, Rectangle, Size, Vector};

use crate::protocol::Role;

/// Marker threaded through `Operation::custom` on entry/exit of a `sem` node.
/// The collector reads the `Enter` metadata and uses the balanced `Exit` to
/// pop the hierarchy.
pub enum SemProbe {
    Enter {
        role: Role,
        name: String,
        value: Option<String>,
        disabled: bool,
    },
    Exit,
}

/// A transparent semantic wrapper around a content element.
pub struct Sem<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    role: Role,
    name: String,
    value: Option<String>,
    disabled: bool,
    content: Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message, Theme, Renderer> Sem<'a, Message, Theme, Renderer> {
    /// Wraps `content`, tagging it with `role` and `name`.
    pub fn new(
        role: Role,
        name: impl Into<String>,
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        Self {
            role,
            name: name.into(),
            value: None,
            disabled: false,
            content: content.into(),
        }
    }

    /// Attaches a semantic value (e.g. the current text of an input).
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Marks the node disabled in the semantic tree.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// Convenience constructor returning an `Element` directly, for the common
/// case with no value/disabled. Use [`Sem::new`] + builders when those matter.
pub fn sem<'a, Message: 'a>(
    role: Role,
    name: impl Into<String>,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    Sem::new(role, name, content).into()
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Sem<'_, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    fn tag(&self) -> tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<Tree> {
        self.content.as_widget().children()
    }

    fn diff(&self, tree: &mut Tree) {
        self.content.as_widget().diff(tree);
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
        self.content.as_widget_mut().layout(tree, renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let mut enter = SemProbe::Enter {
            role: self.role,
            name: self.name.clone(),
            value: self.value.clone(),
            disabled: self.disabled,
        };
        operation.custom(None, layout.bounds(), &mut enter as &mut dyn Any);
        self.content
            .as_widget_mut()
            .operate(tree, layout, renderer, operation);
        let mut exit = SemProbe::Exit;
        operation.custom(None, layout.bounds(), &mut exit as &mut dyn Any);
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
            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
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
        self.content
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
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
        self.content
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content
            .as_widget_mut()
            .overlay(tree, layout, renderer, viewport, translation)
    }
}

impl<'a, Message, Theme, Renderer> From<Sem<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(widget: Sem<'a, Message, Theme, Renderer>) -> Self {
        Element::new(widget)
    }
}
