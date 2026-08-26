//! Mouse-selectable plain text.

use iced::advanced::text::{Paragraph, Renderer as TextRenderer, Span};
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{
    Clipboard, Layout, Shell, Widget, clipboard, layout, mouse, overlay, renderer,
};
use iced::keyboard;
use iced::widget::{Text, text};
use iced::{Element, Event, Length, Point, Rectangle, Size, Vector};
use std::ops::Range;

use crate::selection;

/// Wraps plain text with native drag selection and clipboard shortcuts.
pub fn selectable_text<'a, Theme, Renderer>(
    content: Text<'a, Theme, Renderer>,
) -> SelectableText<'a, Theme, Renderer>
where
    Theme: text::Catalog,
    Renderer: TextRenderer,
{
    SelectableText { content }
}

/// Plain text that can be selected across wrapped lines.
pub struct SelectableText<'a, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Theme: text::Catalog,
    Renderer: TextRenderer,
{
    content: Text<'a, Theme, Renderer>,
}

#[derive(Default)]
struct State {
    token: u64,
    anchor: usize,
    cursor: usize,
    dragging: bool,
}

impl State {
    fn is_active(&self) -> bool {
        selection::holds(self.token)
    }

    fn range(&self, content: &str) -> Option<Range<usize>> {
        if !self.is_active() {
            return None;
        }

        let start = self.anchor.min(self.cursor);
        let end = self.anchor.max(self.cursor);
        (start != end && content.get(start..end).is_some()).then_some(start..end)
    }

    fn selected<'a>(&self, content: &'a str) -> Option<&'a str> {
        content.get(self.range(content)?)
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for SelectableText<'_, Theme, Renderer>
where
    Theme: text::Catalog,
    Renderer: TextRenderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(
            &self.content as &dyn Widget<Message, Theme, Renderer>,
        )]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.children[0].diff(&self.content as &dyn Widget<Message, Theme, Renderer>);
    }

    fn size(&self) -> Size<Length> {
        Widget::<Message, Theme, Renderer>::size(&self.content)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        Widget::<Message, Theme, Renderer>::layout(
            &mut self.content,
            &mut tree.children[0],
            renderer,
            limits,
        )
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        Widget::<Message, Theme, Renderer>::operate(
            &mut self.content,
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
        _renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let paragraph = tree.children[0]
            .state
            .downcast_ref::<text::State<Renderer::Paragraph>>();
        let content = paragraph.content();
        let state: &mut State = tree.state.downcast_mut();

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(position) = cursor.position_in(layout.bounds()) else {
                    return;
                };
                let Some(offset) = hit(paragraph.raw(), position, false) else {
                    return;
                };

                state.token = selection::claim();
                state.anchor = offset;
                state.cursor = offset;
                state.dragging = true;
                shell.request_redraw();
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) if state.dragging => {
                let Some(position) = cursor.position_from(layout.position()) else {
                    return;
                };
                if let Some(offset) = hit(paragraph.raw(), position, true)
                    && state.cursor != offset
                {
                    state.cursor = offset;
                    shell.request_redraw();
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if state.dragging => {
                state.dragging = false;
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                physical_key,
                modifiers,
                ..
            }) if state.is_active() && modifiers.command() => match key.to_latin(*physical_key) {
                Some('a') => {
                    state.anchor = 0;
                    state.cursor = content.len();
                    shell.capture_event();
                    shell.request_redraw();
                }
                Some('c') => {
                    if let Some(selected) = state.selected(content) {
                        clipboard.write(clipboard::Kind::Standard, selected.to_owned());
                        shell.capture_event();
                    }
                }
                _ => {}
            },
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            }) if state.is_active() => {
                selection::clear();
                state.dragging = false;
                shell.request_redraw();
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Text
        } else {
            mouse::Interaction::default()
        }
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
        let paragraph = tree.children[0]
            .state
            .downcast_ref::<text::State<Renderer::Paragraph>>();
        let state: &State = tree.state.downcast_ref();

        if let Some(range) = state.range(paragraph.content()) {
            let content = paragraph.content();
            let spans: [Span<'_, (), Renderer::Font>; 3] = [
                Span::new(&content[..range.start]),
                Span::new(&content[range.clone()]),
                Span::new(&content[range.end..]),
            ];
            let selected =
                Renderer::Paragraph::with_spans(paragraph.as_text().with_content(spans.as_slice()));
            let translation = layout.position() - Point::ORIGIN;
            let color = style.text_color.scale_alpha(0.28);

            for bounds in selected.span_bounds(1) {
                let bounds = bounds + translation;
                if bounds.intersects(viewport) {
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds,
                            ..Default::default()
                        },
                        color,
                    );
                }
            }
        }

        Widget::<Message, Theme, Renderer>::draw(
            &self.content,
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        Widget::<Message, Theme, Renderer>::overlay(
            &mut self.content,
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

fn hit<P: Paragraph>(paragraph: &P, mut point: Point, clamp: bool) -> Option<usize> {
    if clamp {
        let bounds = paragraph.bounds();
        point.x = point.x.clamp(0.0, bounds.width.max(0.0));
        point.y = point.y.clamp(0.0, bounds.height.max(0.0));
    }
    paragraph
        .hit_test(point)
        .map(iced::advanced::text::Hit::cursor)
}

impl<'a, Message, Theme, Renderer> From<SelectableText<'a, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: text::Catalog + 'a,
    Renderer: TextRenderer + 'a,
{
    fn from(text: SelectableText<'a, Theme, Renderer>) -> Self {
        Self::new(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_can_cross_lines_in_either_direction() {
        let token = selection::claim();
        let content = "first\nsecond";
        let mut state = State {
            token,
            anchor: 2,
            cursor: 9,
            dragging: false,
        };

        assert_eq!(state.selected(content), Some("rst\nsec"));
        (state.anchor, state.cursor) = (9, 2);
        assert_eq!(state.selected(content), Some("rst\nsec"));
    }
}
