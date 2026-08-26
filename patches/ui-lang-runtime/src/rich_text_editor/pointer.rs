use super::composition::CompositionLayout;
use super::document::DocumentLayout;
use iced::advanced::mouse;
use iced::widget::text_editor::{Content, Cursor, Position};
use iced::{Padding, Point, Rectangle, Vector};
use unicode_segmentation::UnicodeSegmentation;

pub(super) type InteractionFn<'a> = dyn Fn(&str, Position) -> mouse::Interaction + 'a;

#[derive(Debug, Default)]
pub(super) struct PointerState {
    pub(super) last_click: Option<mouse::Click>,
    pub(super) drag_anchor: Option<Position>,
    pub(super) drag_moved: bool,
    release_bubbles: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Release {
    pub(super) capture: bool,
    pub(super) relayout: bool,
}

impl PointerState {
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(super) fn is_dragging(&self) -> bool {
        self.drag_anchor.is_some()
    }

    pub(super) fn press(
        &mut self,
        content: &Content,
        document: &DocumentLayout,
        composition: Option<&CompositionLayout>,
        local: Point,
        over_link: bool,
    ) -> Cursor {
        let click = mouse::Click::new(local, mouse::Button::Left, self.last_click);
        let position = hit_test(document, composition, local);
        let cursor = match click.kind() {
            mouse::click::Kind::Single => {
                self.drag_anchor = Some(position);
                Cursor {
                    position,
                    selection: None,
                }
            }
            mouse::click::Kind::Double => {
                self.drag_anchor = None;
                select_word(content, position)
            }
            mouse::click::Kind::Triple => {
                self.drag_anchor = None;
                select_line(content, position)
            }
        };

        self.drag_moved = false;
        self.release_bubbles = Some(click.kind() == mouse::click::Kind::Single && over_link);
        self.last_click = Some(click);
        cursor
    }

    pub(super) fn drag(
        &mut self,
        document: &DocumentLayout,
        composition: Option<&CompositionLayout>,
        local: Point,
    ) -> Option<Cursor> {
        let anchor = self.drag_anchor?;
        let position = hit_test(document, composition, local);
        if position != anchor {
            self.drag_moved = true;
            // A drag is not the first click of a later double-click.
            self.last_click = None;
        }
        Some(Cursor {
            position,
            selection: (position != anchor).then_some(anchor),
        })
    }

    pub(super) fn release(&mut self, release_over_link: bool) -> Release {
        let relayout = self.drag_anchor.take().is_some();
        let dragged = relayout && self.drag_moved;
        self.drag_moved = false;
        let capture = self
            .release_bubbles
            .take()
            .is_some_and(|bubble| dragged || !bubble || !release_over_link);
        Release { capture, relayout }
    }
}

pub(super) fn local_point(point: Point, padding: Padding, scroll: f32) -> Point {
    point - Vector::new(padding.left, padding.top) + Vector::new(0.0, scroll)
}

pub(super) fn clamped_local_point(
    point: Point,
    bounds: Rectangle,
    padding: Padding,
    scroll: f32,
) -> Point {
    let relative = point - Vector::new(bounds.x, bounds.y);
    let point = Point::new(
        relative.x.clamp(0.0, bounds.width),
        relative.y.clamp(0.0, bounds.height),
    );
    local_point(point, padding, scroll)
}

/// The SOURCE line and position under `point`, for press interception —
/// the same resolve `interaction_at` performs for the pointer shape.
pub(super) fn source_line_at(
    content: &Content,
    document: &DocumentLayout,
    composition: Option<&CompositionLayout>,
    point: Point,
) -> Option<(String, Position)> {
    let position = document.hit_test(point)?;
    let position = display_to_source(composition, position);
    let line = content.line(position.line)?;
    Some((line.text.into_owned(), position))
}

pub(super) fn interaction_at(
    content: &Content,
    document: &DocumentLayout,
    composition: Option<&CompositionLayout>,
    interaction: Option<&InteractionFn<'_>>,
    point: Point,
) -> mouse::Interaction {
    let Some(position) = document.hit_test(point) else {
        return mouse::Interaction::Text;
    };
    let position = display_to_source(composition, position);
    let Some(line) = content.line(position.line) else {
        return mouse::Interaction::Text;
    };

    interaction.map_or(mouse::Interaction::Text, |interaction| {
        interaction(&line.text, position)
    })
}

fn hit_test(
    document: &DocumentLayout,
    composition: Option<&CompositionLayout>,
    point: Point,
) -> Position {
    display_to_source(composition, document.hit(point))
}

fn display_to_source(composition: Option<&CompositionLayout>, position: Position) -> Position {
    composition.map_or(position, |composition| {
        composition.display_to_source(position)
    })
}

fn select_word(content: &Content, position: Position) -> Cursor {
    let Some(line) = content.line(position.line) else {
        return Cursor {
            position,
            selection: None,
        };
    };
    let selected = line
        .text
        .split_word_bound_indices()
        .find_map(|(start, word)| {
            let end = start + word.len();
            (start <= position.column && position.column < end
                || position.column == line.text.len() && end == line.text.len())
            .then_some(start..end)
        });
    let Some(range) = selected else {
        return Cursor {
            position,
            selection: None,
        };
    };
    Cursor {
        position: Position {
            line: position.line,
            column: range.end,
        },
        selection: Some(Position {
            line: position.line,
            column: range.start,
        }),
    }
}

fn select_line(content: &Content, position: Position) -> Cursor {
    let end = content
        .line(position.line)
        .map_or(0, |line| line.text.len());
    Cursor {
        position: Position {
            line: position.line,
            column: end,
        },
        selection: (end > 0).then_some(Position {
            line: position.line,
            column: 0,
        }),
    }
}
