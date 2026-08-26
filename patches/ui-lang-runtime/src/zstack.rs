//! A z-stack that sizes to the bounding box of all its layers.
//!
//! iced's own [`iced::widget::Stack`] takes its intrinsic size from the first
//! (base) layer alone and constrains every other layer to that size. That is a
//! footgun for overlay menus: a tiny helper layer placed first (e.g. a 1px
//! hidden focus `input`) collapses the whole stack — and the real popover with
//! it — down to 1px. This widget instead lays every layer out against the full
//! limits and takes the union (max) of their sizes, which is the intuitive
//! z-stack behaviour (SwiftUI `ZStack`, CSS positioned stacks). Layers are still
//! drawn in order, first at the bottom.
use iced::advanced::widget::{Operation, Tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use iced::{Element, Event, Length, Rectangle, Size, Vector};

/// A container that displays children on top of each other, sized to contain
/// all of them.
pub struct ZStack<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    width: Length,
    height: Length,
    children: Vec<Element<'a, Message, Theme, Renderer>>,
    clip: bool,
}

/// Creates a [`ZStack`] from the given layers, ordered bottom to top.
pub fn zstack<'a, Message, Theme, Renderer>(
    children: impl IntoIterator<Item = Element<'a, Message, Theme, Renderer>>,
) -> ZStack<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    let children: Vec<_> = children.into_iter().collect();
    // The stack's reported length per axis comes from the layers that DON'T
    // fill it: a fill layer is an overlay over the content, not a sizing
    // vote — reporting Fill for it made every parent column treat the stack
    // as fluid and hand it the leftover space, which in a shrink column is
    // ZERO (a full-height bar beside a paragraph collapsed the whole stack).
    // Only when every layer fills an axis does the stack itself fill it, so
    // a stack of fill layers still stretches like a popover backdrop.
    let mut width = Length::Shrink;
    let mut height = Length::Shrink;
    let mut sized_width = false;
    let mut sized_height = false;
    let mut any_layer = false;
    for child in &children {
        let hint = child.as_widget().size_hint();
        if hint.is_void() {
            continue;
        }
        any_layer = true;
        if !hint.width.is_fill() {
            width = width.enclose(hint.width);
            sized_width = true;
        }
        if !hint.height.is_fill() {
            height = height.enclose(hint.height);
            sized_height = true;
        }
    }
    if !sized_width && any_layer {
        width = Length::Fill;
    }
    if !sized_height && any_layer {
        height = Length::Fill;
    }
    ZStack {
        width,
        height,
        children,
        clip: false,
    }
}

impl<'a, Message, Theme, Renderer> ZStack<'a, Message, Theme, Renderer> {
    /// Sets the width of the [`ZStack`].
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the [`ZStack`].
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets whether the [`ZStack`] should clip overflowing content.
    pub fn clip(mut self, clip: bool) -> Self {
        self.clip = clip;
        self
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for ZStack<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    fn children(&self) -> Vec<Tree> {
        self.children.iter().map(Tree::new).collect()
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&self.children);
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let limits = limits.width(self.width).height(self.height);

        if self.children.is_empty() {
            return layout::Node::new(limits.resolve(self.width, self.height, Size::ZERO));
        }

        // Lay out every layer against the full limits and union their sizes, so
        // the smallest layer can never shrink-wrap the tallest one away.
        //
        // EXCEPT layers that fill an axis the limits leave UNBOUNDED (a stack
        // inside a vertical scroll has infinite max height): resolving those
        // against the raw limits yields an infinite node that degenerates the
        // whole stack. Such layers are deferred and laid out against the size
        // the CONTENT layers resolved — a full-height bar beside a paragraph
        // fills the paragraph, never the void.
        let max = limits.max();
        let is_deferred = |child: &Element<'a, Message, Theme, Renderer>| {
            let hint = child.as_widget().size();
            (max.width.is_infinite() && hint.width.is_fill())
                || (max.height.is_infinite() && hint.height.is_fill())
        };
        let resolved_size = |intrinsic| {
            // The same unbounded-axis guard for the stack's OWN length: a fill
            // layer makes the constructor report Fill, and resolving Fill against
            // an infinite max is the degenerate node this method just avoided.
            let width = if max.width.is_infinite() && self.width.is_fill() {
                Length::Shrink
            } else {
                self.width
            };
            let height = if max.height.is_infinite() && self.height.is_fill() {
                Length::Shrink
            } else {
                self.height
            };
            limits.resolve(width, height, intrinsic)
        };

        if !self.children.iter().any(is_deferred) {
            let nodes = self
                .children
                .iter_mut()
                .zip(tree.children.iter_mut())
                .map(|(child, state)| child.as_widget_mut().layout(state, renderer, &limits))
                .collect::<Vec<_>>();
            let intrinsic = nodes.iter().fold(Size::ZERO, |acc, node| {
                let size = node.size();
                Size::new(acc.width.max(size.width), acc.height.max(size.height))
            });
            return layout::Node::with_children(resolved_size(intrinsic), nodes);
        }

        let mut nodes = self
            .children
            .iter_mut()
            .zip(tree.children.iter_mut())
            .map(|(child, state)| {
                if is_deferred(child) {
                    layout::Node::default()
                } else {
                    child.as_widget_mut().layout(state, renderer, &limits)
                }
            })
            .collect::<Vec<_>>();

        let intrinsic = nodes.iter().fold(Size::ZERO, |acc, node| {
            let size = node.size();
            Size::new(acc.width.max(size.width), acc.height.max(size.height))
        });

        let size = resolved_size(intrinsic);

        let bounded = layout::Limits::new(Size::ZERO, size);
        for ((child, state), node) in self
            .children
            .iter_mut()
            .zip(tree.children.iter_mut())
            .zip(nodes.iter_mut())
        {
            if is_deferred(child) {
                *node = child.as_widget_mut().layout(state, renderer, &bounded);
            }
        }

        layout::Node::with_children(size, nodes)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds());
        operation.traverse(&mut |operation| {
            self.children
                .iter_mut()
                .zip(&mut tree.children)
                .zip(layout.children())
                .for_each(|((child, state), layout)| {
                    child
                        .as_widget_mut()
                        .operate(state, layout, renderer, operation);
                });
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        mut cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if self.children.is_empty() {
            return;
        }

        let is_over = cursor.is_over(layout.bounds());
        let end = self.children.len() - 1;

        for (i, ((child, tree), layout)) in self
            .children
            .iter_mut()
            .rev()
            .zip(tree.children.iter_mut().rev())
            .zip(layout.children().rev())
            .enumerate()
        {
            child.as_widget_mut().update(
                tree, event, layout, cursor, renderer, clipboard, shell, viewport,
            );

            if shell.is_event_captured() {
                return;
            }

            if i < end && is_over && !cursor.is_levitating() {
                let interaction = child
                    .as_widget()
                    .mouse_interaction(tree, layout, cursor, viewport, renderer);

                if interaction != mouse::Interaction::None {
                    cursor = cursor.levitate();
                }
            }
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
        self.children
            .iter()
            .rev()
            .zip(tree.children.iter().rev())
            .zip(layout.children().rev())
            .map(|((child, tree), layout)| {
                child
                    .as_widget()
                    .mouse_interaction(tree, layout, cursor, viewport, renderer)
            })
            .find(|&interaction| interaction != mouse::Interaction::None)
            .unwrap_or_default()
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
        if let Some(clipped_viewport) = layout.bounds().intersection(viewport) {
            let viewport = if self.clip {
                &clipped_viewport
            } else {
                viewport
            };

            let layers_under = if cursor.is_over(layout.bounds()) {
                self.children
                    .iter()
                    .rev()
                    .zip(tree.children.iter().rev())
                    .zip(layout.children().rev())
                    .position(|((layer, tree), layout)| {
                        let interaction = layer
                            .as_widget()
                            .mouse_interaction(tree, layout, cursor, viewport, renderer);

                        interaction != mouse::Interaction::None
                    })
                    .map(|i| self.children.len() - i - 1)
                    .unwrap_or_default()
            } else {
                0
            };

            let mut layers = self
                .children
                .iter()
                .zip(&tree.children)
                .zip(layout.children())
                .enumerate();

            let layers = layers.by_ref();

            let mut draw_layer =
                |i, layer: &Element<'a, Message, Theme, Renderer>, tree, layout, cursor| {
                    if i > 0 {
                        renderer.with_layer(*viewport, |renderer| {
                            layer
                                .as_widget()
                                .draw(tree, renderer, theme, style, layout, cursor, viewport);
                        });
                    } else {
                        layer
                            .as_widget()
                            .draw(tree, renderer, theme, style, layout, cursor, viewport);
                    }
                };

            for (i, ((layer, tree), layout)) in layers.take(layers_under) {
                draw_layer(i, layer, tree, layout, mouse::Cursor::Unavailable);
            }

            for (i, ((layer, tree), layout)) in layers {
                draw_layer(i, layer, tree, layout, cursor);
            }
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
        overlay::from_children(
            &mut self.children,
            tree,
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message, Theme, Renderer> From<ZStack<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(stack: ZStack<'a, Message, Theme, Renderer>) -> Self {
        Self::new(stack)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::Theme;
    use iced::advanced::renderer::Headless;
    use iced::advanced::widget::Tree;
    use iced::widget::{Space, container};

    type TestRenderer = iced_test::renderer::Renderer;

    fn renderer() -> TestRenderer {
        iced_test::futures::futures::executor::block_on(<TestRenderer as Headless>::new(
            iced::Font::DEFAULT,
            iced::Pixels(14.0),
            None,
        ))
        .expect("headless renderer")
    }

    /// A fill-height layer inside an unbounded-height context (a vertical
    /// scroll) must fill the CONTENT layers' resolved height, not resolve
    /// against infinity and degenerate the whole stack.
    #[test]
    fn fill_layer_fills_the_content_height_under_unbounded_limits() {
        let renderer = renderer();
        let content: Element<'_, (), Theme, TestRenderer> =
            container(Space::new().width(100.0).height(40.0)).into();
        let bar: Element<'_, (), Theme, TestRenderer> = container(Space::new())
            .width(3.0)
            .height(Length::Fill)
            .into();
        let mut stack: Element<'_, (), Theme, TestRenderer> =
            zstack([content, bar]).width(Length::Fill).into();
        let mut tree = Tree::new(&stack);
        let limits = layout::Limits::new(Size::ZERO, Size::new(500.0, f32::INFINITY));

        let node = stack.as_widget_mut().layout(&mut tree, &renderer, &limits);

        assert!(node.size().height.is_finite(), "stack degenerated");
        assert!(
            (node.size().height - 40.0).abs() < 0.01,
            "{:?}",
            node.size()
        );
        let bar_height = node.children()[1].size().height;
        assert!((bar_height - 40.0).abs() < 0.01, "bar: {bar_height}");
    }

    /// The stack must not REPORT Fill just because an overlay layer fills it:
    /// a shrink parent column treats a Fill child as fluid and hands it the
    /// leftover space — zero — collapsing the stack. The content layer sizes
    /// the stack; the bar is an overlay.
    #[test]
    fn overlay_fill_layer_does_not_make_the_stack_fluid() {
        let renderer = renderer();
        let content: Element<'_, (), Theme, TestRenderer> =
            container(Space::new().width(100.0).height(40.0)).into();
        let bar: Element<'_, (), Theme, TestRenderer> = container(Space::new())
            .width(3.0)
            .height(Length::Fill)
            .into();
        let stack = zstack([content, bar]).width(Length::Fill);
        assert!(
            !Widget::<(), Theme, TestRenderer>::size(&stack)
                .height
                .is_fill(),
            "stack must stay content-sized on the unfilled axis"
        );

        // Through a shrink column — the real chat-stream nesting.
        let mut column: Element<'_, (), Theme, TestRenderer> =
            iced::widget::column![Element::from(stack)].into();
        let mut tree = Tree::new(&column);
        let limits = layout::Limits::new(Size::ZERO, Size::new(500.0, f32::INFINITY));
        let node = column.as_widget_mut().layout(&mut tree, &renderer, &limits);
        let stack_height = node.children()[0].size().height;
        assert!(
            (stack_height - 40.0).abs() < 0.01,
            "stack in column: {stack_height}"
        );
    }
}
