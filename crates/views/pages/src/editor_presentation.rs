//! The existing Pages Markdown policy copied into a bounded wire presentation.
//! The guest computes styles and hit ranges; the host alone lays out and paints.
use crate::{editor_binding, editor_binding::MenuState, markdown};
use iced::advanced::text::Highlighter;
use std::collections::HashMap;
use ui_lang_guest::{EditorStateView, wire};
use wire::editor_presentation::{
    EditorFormat, EditorGutter, EditorHit, EditorPresentation, EditorSpan, PresentationError,
};

pub fn paint(
    state: EditorStateView<'_>,
    menu: MenuState,
    dark: bool,
    commented: Vec<i64>,
) -> EditorPresentation {
    build(state, menu, dark, commented).unwrap_or_default()
}

pub fn build(
    state: EditorStateView<'_>,
    menu: MenuState,
    dark: bool,
    commented: Vec<i64>,
) -> Result<EditorPresentation, PresentationError> {
    // Keep one paint pass inside the desktop tick budget. The canonical editor
    // remains complete when rich presentation is too dense to publish at once.
    if state.text.len() > 512 * 1024 || work_bound(state.text) > 8192 {
        return Err(PresentationError::Limit);
    }
    let mut result = editor_binding::menu_paint(state, menu);
    // Existing Pages geometry: 46px gutter + 2px breathing room, 28px
    // comment margin + 2px, and no vertical editor padding.
    result.padding = Some(wire::Edges {
        left: 48.0,
        right: 30.0,
        top: 0.0,
        bottom: 0.0,
    });
    let caret = markdown::Caret {
        line: state.cursor.position.line as usize,
        column: state.cursor.position.column as usize,
        dark,
        commented,
    };
    let mut highlighter = markdown::DocumentHighlighter::new(&caret);
    let mut formats = HashMap::new();
    for (line, text) in wire::editor_lines(state.text).enumerate() {
        // Native highlighting emits a whole body run followed by inline
        // overrides. Wire runs are disjoint: split that body's remaining tail.
        let mut runs: Vec<(std::ops::Range<usize>, markdown::Mark)> = Vec::new();
        for (range, mark) in highlighter.highlight_line(text) {
            if let Some((tail, _)) = runs.last()
                && range.start < tail.end
            {
                let (tail, body) = runs.pop().expect("last run");
                if range.start < tail.start || range.end > tail.end {
                    return Err(PresentationError::Range);
                }
                if tail.start < range.start {
                    runs.push((tail.start..range.start, body));
                }
                runs.push((range.clone(), mark));
                if range.end < tail.end {
                    runs.push((range.end..tail.end, body));
                }
            } else {
                runs.push((range, mark));
            }
            if runs.len() + result.spans.len() > wire::editor_presentation::MAX_EDITOR_SPANS {
                return Err(PresentationError::Limit);
            }
        }
        let mut links = Vec::new();
        for (range, mark) in runs {
            if matches!(mark, markdown::Mark::Body(style) if style.link) {
                links.push(EditorHit {
                    line: line as u32,
                    start: range.start as u32,
                    end: range.end as u32,
                    tag: 2,
                });
            }
            let format = match formats.get(&mark) {
                Some(index) => *index,
                None => {
                    if result.formats.len() == wire::editor_presentation::MAX_EDITOR_FORMATS {
                        return Err(PresentationError::Limit);
                    }
                    let index = result.formats.len() as u16;
                    result.formats.push(convert(markdown::format(&mark, dark))?);
                    formats.insert(mark, index);
                    index
                }
            };
            if result.spans.len() == wire::editor_presentation::MAX_EDITOR_SPANS {
                return Err(PresentationError::Limit);
            }
            result.spans.push(EditorSpan {
                line: line as u32,
                start: range.start as u32,
                end: range.end as u32,
                format,
            });
        }
        if line > 0 {
            result.affordances.gutters.push(EditorGutter {
                line: line as u32,
                plus: true,
                handle: true,
            });
        }
        let trimmed = text.trim_start_matches([' ', '\t']);
        let todo = ["- [ ] ", "- [x] ", "- [X] "]
            .iter()
            .any(|prefix| trimmed.starts_with(prefix));
        if todo {
            let start = (text.len() - trimmed.len() + 2) as u32;
            result.affordances.hits.push(EditorHit {
                line: line as u32,
                start,
                end: start + 3,
                tag: 1,
            });
        }
        result.affordances.hits.extend(links);
        // Interactive maps have a separate aggregate cap. Reject while building
        // instead of allocating an arbitrarily large map and dropping its tail.
        let entries = result.affordances.gutters.len() + result.affordances.hits.len();
        if entries > wire::editor_presentation::MAX_EDITOR_SPANS {
            return Err(PresentationError::Limit);
        }
    }
    result.affordances.drop_boundaries = crate::editor_menu::drop_boundaries_text(state.text)
        .into_iter()
        .map(|line| line as u32)
        .collect();
    result.validate(state.text)?;
    Ok(result)
}

fn color(value: iced::Color) -> wire::Rgba {
    wire::Rgba([value.r, value.g, value.b, value.a])
}
fn edges(value: iced::Padding) -> wire::Edges {
    wire::Edges {
        top: value.top,
        right: value.right,
        bottom: value.bottom,
        left: value.left,
    }
}
fn border(value: iced::Border) -> wire::Border {
    wire::Border {
        color: Some(color(value.color)),
        width: Some(value.width),
        radius: Some([
            value.radius.top_left,
            value.radius.top_right,
            value.radius.bottom_right,
            value.radius.bottom_left,
        ]),
    }
}
fn background(value: iced::Background) -> Result<wire::Rgba, PresentationError> {
    match value {
        iced::Background::Color(value) => Ok(color(value)),
        iced::Background::Gradient(_) => Err(PresentationError::Format),
    }
}
fn font(value: iced::Font) -> wire::NamedFont {
    use iced::font::{Family, Stretch, Style, Weight};
    wire::NamedFont {
        family: match value.family {
            Family::Name(name) => wire::FontFamily::Named(name.into()),
            Family::Serif => wire::FontFamily::Serif,
            Family::SansSerif => wire::FontFamily::SansSerif,
            Family::Cursive => wire::FontFamily::Cursive,
            Family::Fantasy => wire::FontFamily::Fantasy,
            Family::Monospace => wire::FontFamily::Monospace,
        },
        weight: match value.weight {
            Weight::Thin => wire::Weight::Thin,
            Weight::ExtraLight => wire::Weight::ExtraLight,
            Weight::Light => wire::Weight::Light,
            Weight::Normal => wire::Weight::Normal,
            Weight::Medium => wire::Weight::Medium,
            Weight::Semibold => wire::Weight::Semibold,
            Weight::Bold => wire::Weight::Bold,
            Weight::ExtraBold => wire::Weight::ExtraBold,
            Weight::Black => wire::Weight::Black,
        },
        stretch: match value.stretch {
            Stretch::UltraCondensed => wire::FontStretch::UltraCondensed,
            Stretch::ExtraCondensed => wire::FontStretch::ExtraCondensed,
            Stretch::Condensed => wire::FontStretch::Condensed,
            Stretch::SemiCondensed => wire::FontStretch::SemiCondensed,
            Stretch::Normal => wire::FontStretch::Normal,
            Stretch::SemiExpanded => wire::FontStretch::SemiExpanded,
            Stretch::Expanded => wire::FontStretch::Expanded,
            Stretch::ExtraExpanded => wire::FontStretch::ExtraExpanded,
            Stretch::UltraExpanded => wire::FontStretch::UltraExpanded,
        },
        style: match value.style {
            Style::Normal => wire::FontStyle::Normal,
            Style::Italic => wire::FontStyle::Italic,
            Style::Oblique => wire::FontStyle::Oblique,
        },
    }
}
pub(crate) fn convert(
    value: ui_lang_runtime::editor_format::Format,
) -> Result<EditorFormat, PresentationError> {
    Ok(EditorFormat {
        color: value.color.map(color),
        font: value.font.map(font),
        size: value.size.map(|pixels| pixels.0),
        line_height: value.line_height.map(|height| match height {
            iced::advanced::text::LineHeight::Relative(value) => wire::LineHeight::Relative(value),
            iced::advanced::text::LineHeight::Absolute(value) => {
                wire::LineHeight::Absolute(value.0)
            }
        }),
        background: value
            .highlight
            .map(|highlight| background(highlight.background))
            .transpose()?,
        border: value.highlight.map(|highlight| border(highlight.border)),
        line_background: value
            .line_highlight
            .map(|highlight| background(highlight.background))
            .transpose()?,
        line_border: value
            .line_highlight
            .map(|highlight| border(highlight.border)),
        line_padding: edges(value.line_padding),
        line_rule: value.line_rule.map(color),
        strikethrough: value.strikethrough.map(color),
        padding: edges(value.padding),
    })
}

fn work_bound(text: &str) -> usize {
    text.bytes()
        .fold(3usize, |cost, byte| {
            cost.saturating_add(match byte {
                b'\n' => 3,
                b'*' | b'_' => 2,
                _ => 0,
            })
        })
        .saturating_add(text.matches("http").count().saturating_mul(2))
}

/// No full document copy: callers already borrow the installed/accepted text.
/// This conservative work bound deliberately switches dense source to plain
/// editing before expensive formatting can exhaust a guest tick.
pub fn format_notice(text: &str) -> String {
    if text.len() > 512 * 1024 || work_bound(text) > 8192 {
        "Formatting is unavailable for this document. Your text and undo history are preserved."
            .into()
    } else {
        String::new()
    }
}
