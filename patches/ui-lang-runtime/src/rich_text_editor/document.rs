use iced::advanced::graphics::text::{Paragraph as GraphicsParagraph, cosmic_text};
use iced::advanced::text::{self, Paragraph as _, Renderer as _, Span, Text};
use iced::alignment;
use iced::widget::text_editor::Position;
use iced::{Color, Font, Padding, Pixels, Point, Rectangle, Size, Vector};
use std::ops::Range;
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

use super::EditorChange;

/// Visual formatting for a highlighted source range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Format {
    /// Text color override.
    pub color: Option<Color>,
    /// Font override.
    pub font: Option<Font>,
    /// Font size override.
    pub size: Option<Pixels>,
    /// Line height override.
    pub line_height: Option<text::LineHeight>,
    /// Background drawn around the formatted span.
    pub highlight: Option<text::Highlight>,
    /// Background drawn across every visual line containing the range.
    pub line_highlight: Option<text::Highlight>,
    /// Layout padding inside [`Self::line_highlight`].
    pub line_padding: Padding,
    /// A full-width horizontal rule painted across the line's vertical center
    /// — what a markdown divider renders as when its `---` glyphs are hidden.
    pub line_rule: Option<Color>,
    /// Strikethrough color.
    pub strikethrough: Option<Color>,
    /// Extra paint-only padding around [`Self::highlight`].
    pub padding: Padding,
}

impl Default for Format {
    fn default() -> Self {
        Self {
            color: None,
            font: None,
            size: None,
            line_height: None,
            highlight: None,
            line_highlight: None,
            line_padding: Padding::ZERO,
            line_rule: None,
            strikethrough: None,
            padding: Padding::ZERO,
        }
    }
}

impl Format {
    fn overlay(self, overlay: Self) -> Self {
        Self {
            color: overlay.color.or(self.color),
            font: overlay.font.or(self.font),
            size: overlay.size.or(self.size),
            line_height: overlay.line_height.or(self.line_height),
            highlight: overlay.highlight.or(self.highlight),
            line_highlight: overlay.line_highlight.or(self.line_highlight),
            line_padding: if overlay.line_padding == Padding::ZERO {
                self.line_padding
            } else {
                overlay.line_padding
            },
            line_rule: overlay.line_rule.or(self.line_rule),
            strikethrough: overlay.strikethrough.or(self.strikethrough),
            padding: if overlay.padding == Padding::ZERO {
                self.padding
            } else {
                overlay.padding
            },
        }
    }
}

#[derive(Default)]
pub(super) struct DocumentLayout {
    pub(super) lines: Vec<DocumentLine>,
    pub(super) height: f32,
}

pub(super) struct DocumentLine {
    pub(super) signature: StyledLine,
    pub(super) paragraph: GraphicsParagraph,
    pub(super) spans: Vec<Span<'static, (), Font>>,
    pub(super) strikethroughs: Vec<Option<Color>>,
    pub(super) top: f32,
    pub(super) height: f32,
    #[cfg(test)]
    pub(super) identity: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct StyledLine {
    pub(super) text: String,
    pub(super) segments: Vec<Segment>,
    pub(super) empty_format: Format,
    pub(super) line_highlight: Option<text::Highlight>,
    pub(super) line_padding: Padding,
    pub(super) line_rule: Option<Color>,
}

/// Scratch buffers for per-line format construction. `update` walks up to the
/// whole document comparing freshly highlighted formats against cached line
/// signatures; these buffers make a compare-and-discard line allocation-free
/// instead of paying a highlight vector, a boundary vector, and a segment
/// vector per line walked.
#[derive(Default)]
struct FormatScratch {
    highlights: Vec<(Range<usize>, Format)>,
    boundaries: Vec<usize>,
    segments: Vec<Segment>,
}

/// The per-line format facts that ride beside [`FormatScratch::segments`].
#[derive(Debug, Clone, Copy)]
struct LineMeta {
    empty_format: Format,
    line_highlight: Option<text::Highlight>,
    line_padding: Padding,
    line_rule: Option<Color>,
}

impl LineMeta {
    fn styled(self, text: String, segments: Vec<Segment>) -> StyledLine {
        StyledLine {
            text,
            segments,
            empty_format: self.empty_format,
            line_highlight: self.line_highlight,
            line_padding: self.line_padding,
            line_rule: self.line_rule,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct LineLayoutStyle {
    pub(super) width: f32,
    pub(super) font: Font,
    pub(super) text_size: Pixels,
    pub(super) line_height: text::LineHeight,
    pub(super) wrapping: text::Wrapping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct LayoutUpdate {
    pub(super) mapping_line_comparisons: usize,
    pub(super) styled_signature_comparisons: usize,
    pub(super) newly_owned_styled_texts: usize,
    pub(super) newly_owned_styled_text_bytes: usize,
    pub(super) line_vector_slots_prepared: usize,
    pub(super) rebuilt_lines: usize,
    pub(super) shaped_paragraphs: usize,
    pub(super) highlighted_lines: usize,
    pub(super) change_hint_used: bool,
    pub(super) change_hint_rejected: bool,
    /// Lines below this index kept their cached highlighting. The caller must
    /// re-open a pass before showing any of them.
    pub(super) highlight_valid_until: usize,
    /// Lines below this index may hold a deferred draw-only format delta —
    /// stale colour, never stale geometry. The caller must re-open a pass
    /// before the viewport reaches above this mark.
    pub(super) format_stale_before: usize,
}

#[derive(Debug, Clone, Copy)]
struct LineMapping {
    common_prefix: usize,
    common_suffix: usize,
    changed_overlap: usize,
    mapping_line_comparisons: usize,
    change_hint_used: bool,
    change_hint_rejected: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum DocumentChange {
    Unchanged,
    Discover,
    Hint(EditorChange),
}

#[derive(Debug, Clone, Copy)]
pub(super) struct DocumentUpdate {
    pub(super) change: DocumentChange,
    pub(super) geometry_changed: bool,
    pub(super) format_changed: bool,
    /// First line of the revealed viewport window (minus overscan). A line
    /// above it whose format delta cannot move a glyph may defer its rebuild
    /// to the pass that scrolls it back into view.
    pub(super) viewport_start: usize,
    /// The stale-prefix mark carried from the previous pass: lines below it
    /// may hold a deferred draw-only format delta.
    pub(super) stale_before: usize,
}

impl DocumentUpdate {
    #[cfg(test)]
    pub(super) const fn text(change: DocumentChange) -> Self {
        Self {
            change,
            geometry_changed: false,
            format_changed: false,
            viewport_start: 0,
            stale_before: 0,
        }
    }
}

pub(super) fn ordered_positions(left: Position, right: Position) -> (Position, Position) {
    if (left.line, left.column) <= (right.line, right.column) {
        (left, right)
    } else {
        (right, left)
    }
}

impl DocumentLayout {
    pub(super) fn update<H>(
        &mut self,
        texts: Lines<'_>,
        highlighter: &mut H,
        format: &dyn Fn(&H::Highlight) -> Format,
        style: LineLayoutStyle,
        update: DocumentUpdate,
        highlight_until: usize,
    ) -> LayoutUpdate
    where
        H: text::Highlighter,
    {
        let old_len = self.lines.len();
        let new_len = texts.len();
        let DocumentUpdate {
            change,
            geometry_changed,
            format_changed,
            viewport_start,
            stale_before,
        } = update;
        let LineMapping {
            common_prefix,
            common_suffix,
            changed_overlap,
            mapping_line_comparisons,
            change_hint_used,
            change_hint_rejected,
        } = match change {
            DocumentChange::Unchanged => LineMapping {
                common_prefix: old_len.min(new_len),
                common_suffix: 0,
                changed_overlap: 0,
                mapping_line_comparisons: 0,
                change_hint_used: false,
                change_hint_rejected: false,
            },
            DocumentChange::Discover => discover_mapping(&self.lines, texts),
            DocumentChange::Hint(change) => hinted_mapping(change, old_len, new_len)
                .unwrap_or_else(|| {
                    let mut mapping = discover_mapping(&self.lines, texts);
                    mapping.change_hint_rejected = true;
                    mapping
                }),
        };
        let text_changed = common_prefix < old_len || common_prefix < new_len;

        // A text edit above the stale prefix shifts its lines; the reveal that
        // follows every edit re-validates from the edit's viewport, so the
        // prefix conservatively shrinks to the edit point.
        let mut stale_before = stale_before;
        if text_changed {
            stale_before = stale_before.min(common_prefix);
        }

        let mut scan_start = highlighter.current_line().min(new_len);
        if text_changed {
            scan_start = scan_start.min(common_prefix);
        }
        if format_changed {
            scan_start = 0;
        }
        // The viewport has climbed above the stale prefix's floor: re-open
        // from the window's first line so the stale lines it shows are
        // re-highlighted, exactly as scrolling below `highlight_valid_until`
        // re-opens downward.
        if stale_before > viewport_start {
            scan_start = scan_start.min(viewport_start);
        }
        if scan_start < new_len {
            highlighter.change_line(scan_start);
        }

        let new_suffix_start = new_len.saturating_sub(common_suffix);
        // Highlighting is sequential, so it can only ever be cut off as a
        // suffix: skip a line and every later line's highlighter state is
        // wrong. The cut must also clear the changed middle region — those
        // lines hold stale TEXT, not merely stale colour, so reusing them
        // would render the document as it was before the edit. When nothing
        // changed textually that region is empty and the cut is free to sit
        // wherever the viewport ends.
        let text_region_end = if new_suffix_start > common_prefix {
            new_suffix_start
        } else {
            0
        };
        let truncate_at = highlight_until.max(text_region_end).min(new_len);

        // When the line count is unchanged every new index maps to the same
        // old index, so there is nothing to re-thread: the pass can edit the
        // lines where they sit. The general path below drains all of
        // `self.lines` into a fresh vector and moves every DocumentLine twice
        // even when one character changed — O(document) memcpy per keystroke,
        // per preedit, and per format key, which are exactly the equal-length
        // cases. A geometry change still rebuilds every line, so it keeps to
        // the general path.
        let mut scratch = FormatScratch::default();
        if !geometry_changed && old_len == new_len {
            let mut rebuilt = 0;
            let mut highlighted = 0;
            let mut styled_signature_comparisons = 0;
            let mut newly_owned_styled_texts = 0;
            let mut newly_owned_styled_text_bytes = 0;
            let mut deferred = false;

            for index in scan_start..truncate_at {
                let text = texts.get(index);
                highlighted += 1;
                let meta = styled_line_format(&mut scratch, text, highlighter, format);
                styled_signature_comparisons += 1;
                if self.lines[index]
                    .signature
                    .matches(text, &scratch.segments, &meta)
                {
                    continue;
                }
                // Above the revealed viewport, a delta that cannot move a
                // glyph — colour, a highlight plate, a strikethrough — keeps
                // its shaped paragraph and its exact height, and is rebuilt by
                // the pass that scrolls it back into view. Deferring a delta
                // that CAN move a glyph would let line tops drift under the
                // reader, so those stay eager wherever they sit.
                if index < viewport_start
                    && self.lines[index]
                        .signature
                        .layout_matches(text, &scratch.segments, &meta)
                {
                    deferred = true;
                    continue;
                }
                rebuilt += 1;
                let owned = if self.lines[index].signature.text == text {
                    std::mem::take(&mut self.lines[index].signature.text)
                } else {
                    newly_owned_styled_texts += 1;
                    newly_owned_styled_text_bytes += text.len();
                    text.to_owned()
                };
                let segments = std::mem::replace(
                    &mut scratch.segments,
                    std::mem::take(&mut self.lines[index].signature.segments),
                );
                self.lines[index] = DocumentLine::new(meta.styled(owned, segments), style);
            }

            let mut top = 0.0;
            for line in &mut self.lines {
                line.top = top;
                top += line.height;
            }
            self.height = top.max(style.line_height.to_absolute(style.text_size).0);

            let format_stale_before = if deferred {
                viewport_start
            } else if scan_start <= stale_before {
                // The walk re-validated everything from `scan_start` on and
                // deferred nothing, so any surviving staleness sits above it.
                stale_before.min(scan_start)
            } else {
                stale_before
            };

            return LayoutUpdate {
                mapping_line_comparisons,
                styled_signature_comparisons,
                newly_owned_styled_texts,
                newly_owned_styled_text_bytes,
                line_vector_slots_prepared: 0,
                rebuilt_lines: rebuilt,
                shaped_paragraphs: rebuilt,
                highlighted_lines: highlighted,
                change_hint_used,
                change_hint_rejected,
                highlight_valid_until: truncate_at,
                format_stale_before,
            };
        }

        let mut old = std::mem::take(&mut self.lines)
            .into_iter()
            .map(Some)
            .collect::<Vec<_>>();
        let old_suffix_start = old_len.saturating_sub(common_suffix);
        let mut lines = Vec::with_capacity(new_len);
        let mut rebuilt = 0;
        let mut highlighted = 0;
        let mut styled_signature_comparisons = 0;
        let mut newly_owned_styled_texts = 0;
        let mut newly_owned_styled_text_bytes = 0;

        for index in 0..new_len {
            let text = texts.get(index);
            let candidate = if index < common_prefix {
                Some(index)
            } else if index >= new_suffix_start {
                Some(old_suffix_start + index - new_suffix_start)
            } else if index < old_len
                && (!change_hint_used || index < common_prefix + changed_overlap)
            {
                Some(index)
            } else {
                None
            };

            if index < scan_start || index >= truncate_at {
                let mut line = candidate
                    .and_then(|candidate| old.get_mut(candidate))
                    .and_then(Option::take)
                    .expect("unchanged rich line");
                if geometry_changed {
                    line = DocumentLine::new(line.into_signature(), style);
                    rebuilt += 1;
                }
                lines.push(line);
                continue;
            }

            highlighted += 1;
            let meta = styled_line_format(&mut scratch, text, highlighter, format);
            let reusable = candidate
                .and_then(|candidate| old.get_mut(candidate))
                .and_then(|line| {
                    if !geometry_changed
                        && line.as_ref().is_some_and(|line| {
                            styled_signature_comparisons += 1;
                            line.signature.matches(text, &scratch.segments, &meta)
                        })
                    {
                        line.take()
                    } else {
                        None
                    }
                });
            let line = reusable.unwrap_or_else(|| {
                rebuilt += 1;
                let reused_text = candidate
                    .and_then(|candidate| old.get_mut(candidate))
                    .and_then(|line| {
                        if line
                            .as_ref()
                            .is_some_and(|line| line.signature.text == text)
                        {
                            line.take().map(DocumentLine::into_text)
                        } else {
                            None
                        }
                    });
                let text = reused_text.unwrap_or_else(|| {
                    newly_owned_styled_texts += 1;
                    newly_owned_styled_text_bytes += text.len();
                    text.to_owned()
                });
                DocumentLine::new(meta.styled(text, scratch.segments.clone()), style)
            });
            lines.push(line);
        }

        let mut top = 0.0;
        for line in &mut lines {
            line.top = top;
            top += line.height;
        }
        self.lines = lines;
        self.height = top.max(style.line_height.to_absolute(style.text_size).0);
        LayoutUpdate {
            mapping_line_comparisons,
            styled_signature_comparisons,
            newly_owned_styled_texts,
            newly_owned_styled_text_bytes,
            line_vector_slots_prepared: old_len.saturating_add(new_len),
            rebuilt_lines: rebuilt,
            shaped_paragraphs: rebuilt,
            highlighted_lines: highlighted,
            change_hint_used,
            change_hint_rejected,
            highlight_valid_until: truncate_at,
            // The general path never defers; whatever it walked is clean.
            format_stale_before: if scan_start <= stale_before {
                stale_before.min(scan_start)
            } else {
                stale_before
            },
        }
    }

    pub(super) fn caret(&self, position: Position) -> Rectangle {
        let Some(line) = self.line(position.line) else {
            return Rectangle::new(Point::ORIGIN, Size::new(1.0, 0.0));
        };
        let caret = caret_rectangle(
            line.paragraph.buffer(),
            Position {
                line: 0,
                column: position.column.min(line.signature.text.len()),
            },
        );
        caret
            + Vector::new(
                line.signature.line_padding.left,
                line.top + line.signature.line_padding.top,
            )
    }

    pub(super) fn hit(&self, point: Point) -> Position {
        let Some(last) = self.lines.len().checked_sub(1) else {
            return Position { line: 0, column: 0 };
        };
        let line_index = self
            .lines
            .partition_point(|line| line.top + line.height <= point.y)
            .min(last);
        let line = &self.lines[line_index];
        let local = hit_position(
            line.paragraph.buffer(),
            Point::new(
                point.x - line.signature.line_padding.left,
                point.y - line.top - line.signature.line_padding.top,
            ),
        );
        Position {
            line: line_index,
            column: local.column.min(line.signature.text.len()),
        }
    }

    /// The display line whose vertical band contains `y`, if any.
    pub(super) fn line_at_y(&self, y: f32) -> Option<usize> {
        if y < 0.0 || y >= self.height {
            return None;
        }
        let index = self
            .lines
            .partition_point(|line| line.top + line.height <= y);
        (index < self.lines.len()).then_some(index)
    }

    pub(super) fn hit_test(&self, point: Point) -> Option<Position> {
        if point.y < 0.0 || point.y >= self.height {
            return None;
        }
        let line_index = self
            .lines
            .partition_point(|line| line.top + line.height <= point.y);
        let line = self.lines.get(line_index)?;
        let local = Point::new(
            point.x - line.signature.line_padding.left,
            point.y - line.top - line.signature.line_padding.top,
        );
        if local.x < 0.0 || local.y < 0.0 {
            return None;
        }

        let run = line
            .paragraph
            .buffer()
            .layout_runs()
            .find(|run| run.line_top <= local.y && local.y < run.line_top + run.line_height)?;
        let mut glyphs = run.glyphs.iter();
        let first = glyphs.next()?;
        let (left, right) = glyphs.fold((first.x, first.x + first.w), |(left, right), glyph| {
            (left.min(glyph.x), right.max(glyph.x + glyph.w))
        });
        if local.x < left || local.x > right {
            return None;
        }

        let position = hit_position(line.paragraph.buffer(), local);
        Some(Position {
            line: line_index,
            column: position.column.min(line.signature.text.len()),
        })
    }

    pub(super) fn draw_text(
        &self,
        renderer: &mut iced::Renderer,
        origin: Point,
        color: Color,
        clip: Rectangle,
    ) {
        for line in &self.lines {
            let top = origin.y + line.top;
            if top + line.height < clip.y || top > clip.y + clip.height {
                continue;
            }
            renderer.fill_paragraph(
                &line.paragraph,
                origin
                    + Vector::new(
                        line.signature.line_padding.left,
                        line.top + line.signature.line_padding.top,
                    ),
                color,
                clip,
            );
        }
    }

    /// The number of lines whose top edge sits above `y`, from the tops the
    /// previous pass produced. Used to bound highlighting to the viewport;
    /// an empty document reports 0, which makes the first pass build in full.
    pub(super) fn lines_above(&self, y: f32) -> usize {
        self.lines.partition_point(|line| line.top < y)
    }

    pub(super) fn line(&self, index: usize) -> Option<&DocumentLine> {
        let index = index.min(self.lines.len().checked_sub(1)?);
        self.lines.get(index)
    }
}

fn hinted_mapping(change: EditorChange, old_len: usize, new_len: usize) -> Option<LineMapping> {
    let old_suffix_start = change
        .first_changed_line
        .checked_add(change.removed_lines)?;
    let new_suffix_start = change
        .first_changed_line
        .checked_add(change.inserted_lines)?;
    if old_suffix_start > old_len
        || new_suffix_start > new_len
        || old_len - old_suffix_start != new_len - new_suffix_start
    {
        return None;
    }

    Some(LineMapping {
        common_prefix: change.first_changed_line,
        common_suffix: old_len - old_suffix_start,
        changed_overlap: change.removed_lines.min(change.inserted_lines),
        mapping_line_comparisons: 0,
        change_hint_used: true,
        change_hint_rejected: false,
    })
}

fn discover_mapping(lines: &[DocumentLine], texts: Lines<'_>) -> LineMapping {
    let old_len = lines.len();
    let new_len = texts.len();
    let shared_len = old_len.min(new_len);
    let mut mapping_line_comparisons = 0;
    let mut common_prefix = 0;

    while common_prefix < shared_len {
        mapping_line_comparisons += 1;
        if lines[common_prefix].signature.text != texts.get(common_prefix) {
            break;
        }
        common_prefix += 1;
    }

    let mut common_suffix = 0;
    while common_suffix < shared_len.saturating_sub(common_prefix) {
        mapping_line_comparisons += 1;
        if lines[old_len - common_suffix - 1].signature.text
            != texts.get(new_len - common_suffix - 1)
        {
            break;
        }
        common_suffix += 1;
    }

    LineMapping {
        common_prefix,
        common_suffix,
        changed_overlap: 0,
        mapping_line_comparisons,
        change_hint_used: false,
        change_hint_rejected: false,
    }
}

impl DocumentLine {
    pub(super) fn new(signature: StyledLine, style: LineLayoutStyle) -> Self {
        #[cfg(test)]
        static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
        let mut spans = Vec::new();
        let mut strikethroughs = Vec::new();
        if signature.segments.is_empty() {
            push_span(
                &mut spans,
                &mut strikethroughs,
                String::new(),
                signature.empty_format,
            );
        } else {
            for segment in &signature.segments {
                push_span(
                    &mut spans,
                    &mut strikethroughs,
                    signature.text[segment.range.clone()].to_owned(),
                    segment.format,
                );
            }
        }

        // cosmic-text sizes a visual line by the largest line-height metric
        // among its glyphs, and iced only attaches that metric to spans that
        // set a size or line height. A body span left at the defaults would
        // not vote, so a wrapped line holding only a hidden 0.01 px marker
        // collapsed to nothing. Every span votes with the paragraph defaults.
        for span in &mut spans {
            span.size.get_or_insert(style.text_size);
            span.line_height.get_or_insert(style.line_height);
        }

        let paragraph = GraphicsParagraph::with_spans(Text {
            content: spans.as_slice(),
            bounds: Size::new(
                (style.width - signature.line_padding.x()).max(1.0),
                i32::MAX as f32,
            ),
            size: style.text_size,
            line_height: style.line_height,
            font: style.font,
            align_x: text::Alignment::Default,
            align_y: alignment::Vertical::Top,
            shaping: text::Shaping::Advanced,
            wrapping: style.wrapping,
        });
        let height = paragraph_height(&paragraph, style.text_size, style.line_height)
            + signature.line_padding.y();

        Self {
            signature,
            paragraph,
            spans,
            strikethroughs,
            top: 0.0,
            height,
            #[cfg(test)]
            identity: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        }
    }

    fn into_text(self) -> String {
        self.signature.text
    }

    fn into_signature(self) -> StyledLine {
        self.signature
    }
}

impl StyledLine {
    fn matches(&self, text: &str, segments: &[Segment], meta: &LineMeta) -> bool {
        self.text == text
            && self.segments == segments
            && self.empty_format == meta.empty_format
            && self.line_highlight == meta.line_highlight
            && self.line_padding == meta.line_padding
            && self.line_rule == meta.line_rule
    }

    /// Whether the new format agrees with this signature on everything that
    /// can move a glyph or a line top — font, size, line height, padding —
    /// leaving only paint: colour, highlight plates, rules, strikethrough.
    /// Equal-height is what makes deferring such a delta safe.
    fn layout_matches(&self, text: &str, segments: &[Segment], meta: &LineMeta) -> bool {
        fn paint_only(a: &Format, b: &Format) -> bool {
            a.font == b.font
                && a.size == b.size
                && a.line_height == b.line_height
                && a.line_padding == b.line_padding
        }
        self.text == text
            && self.line_padding == meta.line_padding
            && paint_only(&self.empty_format, &meta.empty_format)
            && self.segments.len() == segments.len()
            && self
                .segments
                .iter()
                .zip(segments)
                .all(|(old, new)| old.range == new.range && paint_only(&old.format, &new.format))
    }
}

/// A document's lines, borrowed from the text they live in. Replaces passing
/// `&[String]` around, which forced every caller to own a second copy of the
/// whole document just to hand its lines over.
#[derive(Debug, Clone, Copy)]
pub(super) struct Lines<'a> {
    source: &'a str,
    map: &'a TextLines,
}

impl<'a> Lines<'a> {
    pub(super) const fn new(source: &'a str, map: &'a TextLines) -> Self {
        Self { source, map }
    }

    pub(super) fn len(self) -> usize {
        self.map.len()
    }

    pub(super) fn get(self, index: usize) -> &'a str {
        self.map.line(self.source, index)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TextLines {
    starts: Vec<usize>,
    lengths: Vec<usize>,
}

impl TextLines {
    pub(super) fn empty() -> Self {
        Self {
            starts: vec![0],
            lengths: vec![0],
        }
    }

    /// Records where every line sits without copying any of them out. The
    /// ranges below ARE the lines; materializing them into owned strings
    /// duplicates the whole document for nothing.
    pub(super) fn parse(source: &str) -> Self {
        let bytes = source.as_bytes();
        let mut starts = vec![0];
        let mut lengths = Vec::new();
        let mut line_start = 0;
        let mut index = 0;

        while index < bytes.len() {
            let ending_len = match bytes[index] {
                b'\r' if bytes.get(index + 1) == Some(&b'\n') => 2,
                b'\n' if bytes.get(index + 1) == Some(&b'\r') => 2,
                b'\r' | b'\n' => 1,
                _ => {
                    index += 1;
                    continue;
                }
            };

            lengths.push(index - line_start);
            index += ending_len;
            line_start = index;
            starts.push(index);
        }

        lengths.push(source.len() - line_start);

        Self { starts, lengths }
    }

    pub(super) fn len(&self) -> usize {
        self.lengths.len()
    }

    /// The text of one line, borrowed straight out of the source it maps.
    pub(super) fn line<'a>(&self, source: &'a str, index: usize) -> &'a str {
        let start = self.starts.get(index).copied().unwrap_or_default();
        let length = self.lengths.get(index).copied().unwrap_or_default();
        source.get(start..start + length).unwrap_or_default()
    }

    pub(super) fn offset(&self, position: Position) -> usize {
        let line = position.line.min(self.starts.len().saturating_sub(1));
        self.starts.get(line).copied().unwrap_or_default()
            + position
                .column
                .min(self.lengths.get(line).copied().unwrap_or_default())
    }

    pub(super) fn position(&self, offset: usize) -> Position {
        let line = self
            .starts
            .partition_point(|start| *start <= offset)
            .saturating_sub(1);
        Position {
            line,
            column: offset
                .saturating_sub(self.starts.get(line).copied().unwrap_or_default())
                .min(self.lengths.get(line).copied().unwrap_or_default()),
        }
    }
}

fn styled_line_format<H>(
    scratch: &mut FormatScratch,
    source: &str,
    highlighter: &mut H,
    format: &dyn Fn(&H::Highlight) -> Format,
) -> LineMeta
where
    H: text::Highlighter,
{
    scratch.highlights.clear();
    scratch.highlights.extend(
        highlighter
            .highlight_line(source)
            .map(|(range, highlight)| (range, format(&highlight))),
    );
    compose_segments_into(
        &mut scratch.segments,
        &mut scratch.boundaries,
        source,
        &scratch.highlights,
    );
    let highlights = &scratch.highlights;
    let empty_format = highlights
        .iter()
        .fold(Format::default(), |base, (_, next)| base.overlay(*next));
    let line_highlight = highlights
        .iter()
        .filter_map(|(_, format)| format.line_highlight)
        .next_back();
    // Padding is LAYOUT and a highlight is PAINT: reading the padding off the
    // highlighted run tied them together, so a line could not be inset without
    // also wearing a plate — which is what a nesting indent needs to do.
    let line_padding = highlights
        .iter()
        .map(|(_, format)| format.line_padding)
        .rfind(|padding| *padding != Padding::ZERO)
        .unwrap_or(Padding::ZERO);
    let line_rule = highlights
        .iter()
        .filter_map(|(_, format)| format.line_rule)
        .next_back();

    LineMeta {
        empty_format,
        line_highlight,
        line_padding,
        line_rule,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Segment {
    pub(super) range: Range<usize>,
    pub(super) format: Format,
}

#[cfg(test)]
pub(super) fn compose_segments(line: &str, highlights: &[(Range<usize>, Format)]) -> Vec<Segment> {
    let mut segments = Vec::new();
    compose_segments_into(&mut segments, &mut Vec::new(), line, highlights);
    segments
}

fn compose_segments_into(
    segments: &mut Vec<Segment>,
    boundaries: &mut Vec<usize>,
    line: &str,
    highlights: &[(Range<usize>, Format)],
) {
    segments.clear();
    if line.is_empty() {
        return;
    }

    boundaries.clear();
    boundaries.push(0);
    boundaries.push(line.len());
    for (range, _) in highlights {
        let start = range.start.min(line.len());
        let end = range.end.min(line.len());
        if start <= end && line.is_char_boundary(start) && line.is_char_boundary(end) {
            boundaries.push(start);
            boundaries.push(end);
        }
    }
    boundaries.sort_unstable();
    boundaries.dedup();

    for pair in boundaries.windows(2) {
        let range = pair[0]..pair[1];
        if range.is_empty()
            || !line.is_char_boundary(range.start)
            || !line.is_char_boundary(range.end)
        {
            continue;
        }
        let format = highlights
            .iter()
            .filter(|(highlight, _)| highlight.start < range.end && range.start < highlight.end)
            .fold(Format::default(), |base, (_, next)| base.overlay(*next));

        if let Some(previous) = segments.last_mut()
            && previous.range.end == range.start
            && previous.format == format
        {
            previous.range.end = range.end;
        } else {
            segments.push(Segment { range, format });
        }
    }
}

pub(super) fn to_span(source: String, format: Format) -> Span<'static, (), Font> {
    let mut span = Span::new(source);
    span.color = format.color;
    span.font = format.font;
    span.size = format.size;
    span.line_height = format.line_height;
    span.highlight = format.highlight;
    span.padding = format.padding;
    span.strikethrough = format.strikethrough.is_some();
    span
}

pub(super) fn push_span(
    spans: &mut Vec<Span<'static, (), Font>>,
    strikethroughs: &mut Vec<Option<Color>>,
    source: String,
    format: Format,
) {
    strikethroughs.push(format.strikethrough);
    spans.push(to_span(source, format));
}

pub(super) fn paragraph_height(
    paragraph: &GraphicsParagraph,
    size: Pixels,
    line_height: text::LineHeight,
) -> f32 {
    paragraph
        .buffer()
        .layout_runs()
        .map(|run| run.line_top + run.line_height)
        .reduce(f32::max)
        .unwrap_or_else(|| line_height.to_absolute(size).0)
}

pub(super) fn hit_position(buffer: &cosmic_text::Buffer, point: Point) -> Position {
    let height = buffer
        .layout_runs()
        .map(|run| run.line_top + run.line_height)
        .fold(0.0, f32::max);
    let point = Point::new(
        point.x.max(0.0),
        point.y.clamp(0.0, (height - 0.5).max(0.0)),
    );
    let cursor = buffer.hit(point.x, point.y).unwrap_or_else(|| {
        let line = buffer.lines.len().saturating_sub(1);
        cosmic_text::Cursor::new(
            line,
            buffer.lines.get(line).map_or(0, |line| line.text().len()),
        )
    });
    Position {
        line: cursor.line,
        column: cursor.index,
    }
}

pub(super) fn caret_rectangle(buffer: &cosmic_text::Buffer, position: Position) -> Rectangle {
    let mut previous = None;
    for run in buffer
        .layout_runs()
        .filter(|run| run.line_i == position.line)
    {
        let start = run.glyphs.first().map_or(0, |glyph| glyph.start);
        let end = run.glyphs.last().map_or(start, |glyph| glyph.end);

        if start > position.column {
            return previous.unwrap_or_else(|| {
                Rectangle::new(
                    Point::new(0.0, run.line_top),
                    Size::new(1.0, run.line_height),
                )
            });
        }

        let cursor = cosmic_text::Cursor::new(position.line, position.column);
        if position.column <= end {
            let x = run
                .highlight(cursor, cursor)
                .map_or_else(|| caret_x(run.glyphs, position.column), |(x, _)| x);
            return Rectangle::new(Point::new(x, run.line_top), Size::new(1.0, run.line_height));
        }

        previous = Some(Rectangle::new(
            Point::new(run.line_w, run.line_top),
            Size::new(1.0, run.line_height),
        ));
    }

    previous.unwrap_or_else(|| {
        let metrics = buffer.metrics();
        Rectangle::new(
            Point::new(0.0, position.line as f32 * metrics.line_height),
            Size::new(1.0, metrics.line_height),
        )
    })
}

fn caret_x(glyphs: &[cosmic_text::LayoutGlyph], index: usize) -> f32 {
    glyphs
        .iter()
        .find(|glyph| index <= glyph.start)
        .map_or_else(
            || glyphs.last().map_or(0.0, |glyph| glyph.x + glyph.w),
            |glyph| glyph.x,
        )
}
