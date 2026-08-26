use super::document::{DocumentLayout, ordered_positions};
use iced::Point;
use iced::widget::text_editor::{Cursor, Motion, Position};

pub(super) fn uses_rich_geometry(motion: Motion) -> bool {
    matches!(
        motion,
        Motion::Up | Motion::Down | Motion::Home | Motion::End | Motion::PageUp | Motion::PageDown
    )
}

pub(super) fn move_cursor(
    document: &DocumentLayout,
    preferred_x: &mut Option<f32>,
    viewport_height: f32,
    cursor: Cursor,
    motion: Motion,
    select: bool,
) -> Cursor {
    let anchor = select.then(|| cursor.selection.unwrap_or(cursor.position));
    let position = if let Some(selection) = cursor.selection
        && !select
        && matches!(
            motion,
            Motion::Up | Motion::Down | Motion::PageUp | Motion::PageDown
        ) {
        let (start, end) = ordered_positions(cursor.position, selection);
        if matches!(motion, Motion::Up | Motion::PageUp) {
            start
        } else {
            end
        }
    } else {
        rich_motion(
            document,
            preferred_x,
            viewport_height,
            cursor.position,
            motion,
        )
    };
    Cursor {
        position,
        selection: anchor.filter(|anchor| *anchor != position),
    }
}

fn rich_motion(
    document: &DocumentLayout,
    preferred_x: &mut Option<f32>,
    viewport_height: f32,
    position: Position,
    motion: Motion,
) -> Position {
    #[derive(Clone, Copy)]
    struct VisualRun {
        line: usize,
        top: f32,
        height: f32,
        start: usize,
        end: usize,
    }

    let caret = document.caret(position);
    let preferred_x_value = *preferred_x.get_or_insert(caret.x);
    let caret_center = caret.y + caret.height / 2.0;
    let distance = |run: &VisualRun| {
        if caret_center < run.top {
            run.top - caret_center
        } else if caret_center > run.top + run.height {
            caret_center - run.top - run.height
        } else {
            0.0
        }
    };
    let (last, previous, current, next, page_up, page_down) = document
        .lines
        .iter()
        .enumerate()
        .flat_map(|(line_index, line)| {
            line.paragraph
                .buffer()
                .layout_runs()
                .map(move |run| VisualRun {
                    line: line_index,
                    top: line.top + line.signature.line_padding.top + run.line_top,
                    height: run.line_height,
                    start: run.glyphs.first().map_or(0, |glyph| glyph.start),
                    end: run
                        .glyphs
                        .last()
                        .map_or(line.signature.text.len(), |glyph| glyph.end),
                })
        })
        .fold(
            (None, None, None, None, None, None),
            |(last, previous, current, next, page_up, page_down), run| {
                let (previous, current, next) =
                    if current.is_none_or(|current| distance(&run) < distance(&current)) {
                        (last, Some(run), None)
                    } else {
                        (previous, current, next.or(Some(run)))
                    };
                (
                    Some(run),
                    previous,
                    current,
                    next,
                    if page_up.is_none() || run.top <= caret.y - viewport_height {
                        Some(run)
                    } else {
                        page_up
                    },
                    page_down.or_else(|| (run.top >= caret.y + viewport_height).then_some(run)),
                )
            },
        );
    let target = match motion {
        Motion::Up => previous.or(current),
        Motion::Down => next.or(current),
        Motion::PageUp => page_up,
        Motion::PageDown => page_down.or(last),
        Motion::Home | Motion::End => current,
        _ => return position,
    };

    if matches!(motion, Motion::Home | Motion::End) {
        *preferred_x = None;
    }
    let Some(run) = target else {
        return position;
    };
    match motion {
        Motion::Home => Position {
            line: run.line,
            column: run.start,
        },
        Motion::End => Position {
            line: run.line,
            column: run.end,
        },
        _ => document.hit(Point::new(preferred_x_value, run.top + run.height / 2.0)),
    }
}
