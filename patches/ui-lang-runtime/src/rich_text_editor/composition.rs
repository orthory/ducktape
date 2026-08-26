use super::document::{TextLines, ordered_positions};
use iced::advanced::input_method;
use iced::widget::text_editor::{Cursor, Position};
use std::ops::Range;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct CompositionLayout {
    pub(super) source_lines: TextLines,
    pub(super) display_lines: TextLines,
    pub(super) source_range: Range<usize>,
    pub(super) display_range: Range<usize>,
    pub(super) range: (Position, Position),
    pub(super) selection: Option<(Position, Position)>,
    pub(super) cursor: Position,
    pub(super) cursor_visible: bool,
}

impl CompositionLayout {
    pub(super) fn display_to_source(&self, position: Position) -> Position {
        let offset = self.display_lines.offset(position);
        let source_offset = if offset <= self.display_range.start {
            offset
        } else if offset < self.display_range.end {
            self.source_range.start
        } else {
            offset
                .saturating_sub(self.display_range.len())
                .saturating_add(self.source_range.len())
        };
        self.source_lines.position(source_offset)
    }
}

pub(super) struct CompositionDocument {
    /// The document as the preedit makes it read. Its lines are borrowed out
    /// of this string rather than copied into a parallel vector.
    pub(super) display: String,
    pub(super) layout: CompositionLayout,
    #[cfg(test)]
    pub(super) display_bytes: usize,
}

impl CompositionDocument {
    pub(super) fn new(
        cursor: Cursor,
        source: &str,
        source_lines: TextLines,
        preedit: &input_method::Preedit,
    ) -> Option<Self> {
        if preedit.content.is_empty() {
            return None;
        }

        let (start, end) = cursor
            .selection
            .map_or((cursor.position, cursor.position), |anchor| {
                ordered_positions(cursor.position, anchor)
            });
        let source_range = source_lines.offset(start)..source_lines.offset(end);
        let mut display =
            String::with_capacity(source.len() - source_range.len() + preedit.content.len());
        display.push_str(&source[..source_range.start]);
        display.push_str(&preedit.content);
        display.push_str(&source[source_range.end..]);

        let display_lines = TextLines::parse(&display);
        let display_range =
            source_range.start..source_range.start.saturating_add(preedit.content.len());
        let selection = preedit.selection.as_ref().map(|selection| {
            let start = char_boundary_at_or_before(&preedit.content, selection.start);
            let end = char_boundary_at_or_before(&preedit.content, selection.end.max(start));
            display_lines.position(display_range.start + start)
                ..display_lines.position(display_range.start + end)
        });
        let cursor_offset = selection.as_ref().map_or(display_range.end, |selection| {
            display_lines.offset(selection.end)
        });
        let range = (
            display_lines.position(display_range.start),
            display_lines.position(display_range.end),
        );
        let cursor = display_lines.position(cursor_offset);
        let layout = CompositionLayout {
            source_lines,
            display_lines,
            source_range,
            display_range,
            range,
            selection: selection.map(|selection| (selection.start, selection.end)),
            cursor,
            cursor_visible: preedit.selection.is_some(),
        };

        Some(Self {
            #[cfg(test)]
            display_bytes: display.len(),
            display,
            layout,
        })
    }
}

fn char_boundary_at_or_before(source: &str, index: usize) -> usize {
    let mut index = index.min(source.len());
    while !source.is_char_boundary(index) {
        index -= 1;
    }
    index
}
