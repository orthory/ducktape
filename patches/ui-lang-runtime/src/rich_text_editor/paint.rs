use super::composition::CompositionLayout;
use super::document::{DocumentLayout, ordered_positions};
use iced::advanced::graphics::text::cosmic_text;
use iced::advanced::text::{self, Paragraph as _};
use iced::advanced::{Renderer as _, renderer};
use iced::widget::text_editor::{Cursor, Position};
use iced::{Color, Padding, Point, Rectangle, Size, Vector};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct LineHighlightGroup {
    pub(super) top: f32,
    pub(super) height: f32,
    pub(super) highlight: text::Highlight,
}

pub(super) fn visit_line_highlight_groups(
    runs: impl IntoIterator<Item = (Option<text::Highlight>, f32, f32)>,
    mut visit: impl FnMut(LineHighlightGroup),
) {
    let mut current = None;

    for (highlight, top, height) in runs {
        let Some(highlight) = highlight else {
            if let Some(group) = current.take() {
                visit(group);
            }
            continue;
        };

        if let Some(group) = current.as_mut()
            && group.highlight == highlight
        {
            let bottom = (group.top + group.height).max(top + height);
            group.height = bottom - group.top;
            continue;
        }

        if let Some(group) = current.replace(LineHighlightGroup {
            top,
            height,
            highlight,
        }) {
            visit(group);
        }
    }

    if let Some(group) = current {
        visit(group);
    }
}

pub(super) fn draw_line_highlights(
    renderer: &mut iced::Renderer,
    document: &DocumentLayout,
    clip: Rectangle,
    origin: Point,
) {
    renderer.with_layer(clip, |renderer| {
        visit_line_highlight_groups(
            document.lines.iter().map(|line| {
                (
                    line.signature.line_highlight,
                    origin.y + line.top,
                    line.height,
                )
            }),
            |group| {
                let bounds = Rectangle::new(
                    Point::new(clip.x, group.top),
                    Size::new(clip.width, group.height),
                );
                if clip.intersection(&bounds).is_some() {
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds,
                            border: group.highlight.border,
                            ..renderer::Quad::default()
                        },
                        group.highlight.background,
                    );
                }
            },
        );
    });
}

/// The full-width rules of divider-style lines, one centered stripe per line
/// carrying a [`Format::line_rule`].
pub(super) fn draw_line_rules(
    renderer: &mut iced::Renderer,
    document: &DocumentLayout,
    clip: Rectangle,
    origin: Point,
) {
    for line in &document.lines {
        let Some(color) = line.signature.line_rule else {
            continue;
        };
        let rule = Rectangle::new(
            Point::new(clip.x, origin.y + line.top + line.height / 2.0 - 0.5),
            Size::new(clip.width, 1.0),
        );
        if let Some(rule) = clip.intersection(&rule) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: rule,
                    ..renderer::Quad::default()
                },
                color,
            );
        }
    }
}

pub(super) fn draw_span_highlights(
    renderer: &mut iced::Renderer,
    document: &DocumentLayout,
    clip: Rectangle,
    origin: Point,
) {
    for line in &document.lines {
        let top = origin.y + line.top;
        if top + line.height < clip.y || top > clip.y + clip.height {
            continue;
        }
        let Some(line_clip) = clip.intersection(&Rectangle::new(
            Point::new(clip.x, top),
            Size::new(clip.width, line.height),
        )) else {
            continue;
        };
        let translation = origin - Point::ORIGIN
            + Vector::new(
                line.signature.line_padding.left,
                line.top + line.signature.line_padding.top,
            );
        for (index, span) in line.spans.iter().enumerate() {
            let Some(highlight) = span.highlight else {
                continue;
            };
            for bounds in line.paragraph.span_bounds(index) {
                if let Some(bounds) =
                    span_highlight_bounds(bounds + translation, span.padding, line_clip)
                {
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds,
                            border: highlight.border,
                            ..renderer::Quad::default()
                        },
                        highlight.background,
                    );
                }
            }
        }
    }
}

pub(super) fn span_highlight_bounds(
    bounds: Rectangle,
    padding: Padding,
    line_clip: Rectangle,
) -> Option<Rectangle> {
    line_clip.intersection(&Rectangle::new(
        bounds.position() - Vector::new(padding.left, padding.top),
        bounds.size() + Size::new(padding.x(), padding.y()),
    ))
}

pub(super) fn draw_selection(
    renderer: &mut iced::Renderer,
    document: &DocumentLayout,
    cursor: Cursor,
    clip: Rectangle,
    origin: Point,
    color: Color,
) {
    let Some(anchor) = cursor.selection else {
        return;
    };
    let (start, end) = ordered_positions(cursor.position, anchor);

    for line_index in start.line..=end.line {
        let Some(line) = document.line(line_index) else {
            continue;
        };
        let from = if line_index == start.line {
            start.column.min(line.signature.text.len())
        } else {
            0
        };
        let to = if line_index == end.line {
            end.column.min(line.signature.text.len())
        } else {
            line.signature.text.len()
        };
        let from = cosmic_text::Cursor::new(0, from);
        let to = cosmic_text::Cursor::new(0, to);

        for run in line.paragraph.buffer().layout_runs() {
            let Some((x, width)) = run.highlight(from, to) else {
                continue;
            };
            let bounds = Rectangle::new(
                Point::new(
                    origin.x + line.signature.line_padding.left + x,
                    origin.y + line.top + line.signature.line_padding.top + run.line_top,
                ),
                Size::new(width.max(1.0), run.line_height),
            );
            if let Some(bounds) = clip.intersection(&bounds) {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds,
                        ..renderer::Quad::default()
                    },
                    color,
                );
            }
        }
    }
}

pub(super) fn draw_strikethroughs(
    renderer: &mut iced::Renderer,
    document: &DocumentLayout,
    clip: Rectangle,
    origin: Point,
) {
    for document_line in &document.lines {
        let top = origin.y + document_line.top;
        if top + document_line.height < clip.y || top > clip.y + clip.height {
            continue;
        }
        let translation = origin - Point::ORIGIN
            + Vector::new(
                document_line.signature.line_padding.left,
                document_line.top + document_line.signature.line_padding.top,
            );
        for (index, color) in document_line
            .strikethroughs
            .iter()
            .enumerate()
            .filter_map(|(index, color)| color.map(|color| (index, color)))
        {
            for bounds in document_line.paragraph.span_bounds(index) {
                let line = Rectangle::new(
                    Point::new(bounds.x, bounds.y + bounds.height * 0.55) + translation,
                    Size::new(bounds.width, 1.0),
                );
                if let Some(line) = clip.intersection(&line) {
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: line,
                            ..renderer::Quad::default()
                        },
                        color,
                    );
                }
            }
        }
    }
}

pub(super) fn draw_composition(
    renderer: &mut iced::Renderer,
    document: &DocumentLayout,
    composition: &CompositionLayout,
    clip: Rectangle,
    origin: Point,
    color: Color,
    cursor_visible: bool,
) {
    draw_range_underline(
        renderer,
        document,
        composition.range,
        clip,
        origin,
        color,
        1.0,
    );

    if let Some((start, end)) = composition.selection
        && start != end
    {
        draw_range_underline(renderer, document, (start, end), clip, origin, color, 2.0);
    }

    if cursor_visible && composition.cursor_visible {
        let caret = document.caret(composition.cursor) + (origin - Point::ORIGIN);
        if let Some(caret) = clip.intersection(&caret) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: caret,
                    ..renderer::Quad::default()
                },
                color,
            );
        }
    }
}

fn draw_range_underline(
    renderer: &mut iced::Renderer,
    document: &DocumentLayout,
    range: (Position, Position),
    clip: Rectangle,
    origin: Point,
    color: Color,
    thickness: f32,
) {
    let (start, end) = ordered_positions(range.0, range.1);
    for line_index in start.line..=end.line {
        let Some(line) = document.line(line_index) else {
            continue;
        };
        let from = if line_index == start.line {
            start.column.min(line.signature.text.len())
        } else {
            0
        };
        let to = if line_index == end.line {
            end.column.min(line.signature.text.len())
        } else {
            line.signature.text.len()
        };
        let from = cosmic_text::Cursor::new(0, from);
        let to = cosmic_text::Cursor::new(0, to);
        for run in line.paragraph.buffer().layout_runs() {
            let Some((x, width)) = run.highlight(from, to) else {
                continue;
            };
            let underline = Rectangle::new(
                Point::new(
                    origin.x + line.signature.line_padding.left + x,
                    origin.y
                        + line.top
                        + line.signature.line_padding.top
                        + run.line_top
                        + run.line_height
                        - thickness,
                ),
                Size::new(width.max(1.0), thickness),
            );
            if let Some(underline) = clip.intersection(&underline) {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: underline,
                        ..renderer::Quad::default()
                    },
                    color,
                );
            }
        }
    }
}
