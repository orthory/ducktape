//! The page document's markdown highlighter.
//!
//! Modelled on `ducktape-ui/examples/markdown-editor`: one `RichTextEditor`
//! over the whole document, and the SYNTAX ITSELF carries the formatting. A
//! line that starts `## ` is a Heading 2 because those three bytes are there,
//! not because a menu was opened — and the three bytes vanish unless the caret
//! is on their line, which is what makes the surface read as WYSIWYG instead of
//! as a source view.
//!
//! TWO THINGS THE REFERENCE DOES THAT THIS DELIBERATELY DOES NOT:
//!   * `pulldown-cmark` for the inline grammar. Pages must agree with the CHAT
//!     renderer, not with CommonMark — a `_word_` that italicises in a message
//!     has to italicise here. [`crate::editor::inline_marks`] IS that grammar,
//!     already tested against `chat::client::inline_spans`, so it is the parser
//!     for both surfaces and no dependency is added.
//!   * `iced_highlighter` language tokens inside fences. `PageBlock` carries no
//!     language field, so there is nothing to colour BY; a fence body is one
//!     mono plate.
//!
//! The line metrics are the Pages design tokens from `components/pages.ice`
//! (H1 20/1.25, H2 16/1.3, H3 14/1.35, body 14/1.65, quote 14/1.6, code 12/1.6,
//! callout 13/1.6), so a saved document reads at exactly the size it was typed.

use iced::advanced::text::{Highlight as TextHighlight, Highlighter, LineHeight};
use iced::font::{Family, Style as FontStyle, Weight};
use iced::{Border, Color, Font, Padding, Pixels};
use std::ops::Range;
use ui_lang_runtime::rich_text_editor::Format;

use crate::editor::{Inline, inline_marks};

pub const BODY_SIZE: f32 = 14.0;
pub const BODY_LINE_HEIGHT: f32 = 1.65;
/// Line 0 is the page title. It reads a step above H1 so the document opens on
/// an obvious title, and it is the ONLY line whose shape is positional rather
/// than declared by a prefix.
const TITLE_SIZE: f32 = 22.0;
const TITLE_LINE_HEIGHT: f32 = 1.15;
const HEADING_SIZE: [f32; 3] = [20.0, 16.0, 14.0];
const HEADING_LINE_HEIGHT: [f32; 3] = [1.25, 1.3, 1.35];
const QUOTE_LINE_HEIGHT: f32 = 1.6;
const CALLOUT_SIZE: f32 = 13.0;
const CALLOUT_LINE_HEIGHT: f32 = 1.6;
const CODE_SIZE: f32 = 12.0;
const CODE_LINE_HEIGHT: f32 = 1.6;
const CODE_PLATE_PAD: f32 = 15.0;
/// A hidden marker cannot be `size 0` — a zero-metric span drops out of the
/// shaped run and takes the caret's column with it. The reference uses the
/// same hair-width value for the same reason.
const HIDDEN_SIZE: f32 = 0.01;

/// Where the caret is, and which palette is being painted. The whole point of
/// carrying it into the highlighter is the marker reveal: syntax on the caret's
/// own line stays legible so it can be edited, everywhere else it disappears.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Caret {
    pub line: usize,
    pub column: usize,
    pub dark: bool,
    /// Document lines inside a block carrying an unresolved comment thread —
    /// they wear a quiet wash so the anchor is visible in the document.
    pub commented: Vec<i64>,
}

/// One painted run. `Marker` is the markdown syntax itself; every other variant
/// is content wearing the shape that syntax declared.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mark {
    Title,
    Marker { hidden: bool, style: Style },
    Body(Style),
    ListMarker(Style),
    Fence { hidden: bool },
    CodeBody,
}

/// The shape a line's prefix declared, plus the inline marks inside it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub heading: Option<u8>,
    pub quote: bool,
    pub callout: bool,
    pub divider: bool,
    pub strong: bool,
    pub emphasis: bool,
    pub link: bool,
    pub commented: bool,
}

/// The block shape a line's leading bytes declare. `Body` is the absence of a
/// prefix, which is why it has no marker range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Prefix {
    /// `# ` / `## ` / `### ` — hidden, and the body takes the heading metrics.
    Heading(u8),
    /// `- `, `1. `, `- [ ] ` — KEPT VISIBLE. The marker is the bullet; hiding
    /// it would leave a list looking like a stack of naked paragraphs.
    List,
    /// `> ` — hidden, body reads muted italic.
    Quote,
    /// `!> ` — hidden. Callout has no CommonMark spelling and the block kind
    /// predates this surface, so it gets an extension rather than being
    /// silently rewritten into a quote on the first save.
    Callout,
    /// `---` on its own line.
    Divider,
    Body,
}

/// The prefix a line declares, and the byte offset its content starts at.
fn prefix_of(line: &str) -> (Prefix, usize) {
    let trimmed = line.trim_start_matches([' ', '\t']);
    let indent = line.len() - trimmed.len();

    if trimmed.trim_end() == "---" {
        return (Prefix::Divider, line.len());
    }
    if let Some(rest) = trimmed.strip_prefix("!> ") {
        return (Prefix::Callout, line.len() - rest.len());
    }
    if let Some(rest) = trimmed.strip_prefix("> ") {
        return (Prefix::Quote, line.len() - rest.len());
    }
    let hashes = trimmed.bytes().take_while(|byte| *byte == b'#').count();
    let heading_marked = (1..=3).contains(&hashes) && trimmed.as_bytes().get(hashes) == Some(&b' ');
    if heading_marked {
        let content = indent + hashes + 1;
        return (Prefix::Heading(hashes as u8), content);
    }
    match list_content(trimmed) {
        Some(offset) => (Prefix::List, indent + offset),
        None => (Prefix::Body, indent),
    }
}

/// The byte offset a list line's CONTENT starts at, marker and tick included.
/// `- `, `* `, `+ `, `1. `, `1) `, and any of those followed by `[ ]`/`[x]`.
fn list_content(trimmed: &str) -> Option<usize> {
    let bytes = trimmed.as_bytes();
    let mut cursor = match *bytes.first()? {
        b'-' | b'+' | b'*' => 1,
        byte if byte.is_ascii_digit() => {
            let digits = bytes
                .iter()
                .take_while(|byte| byte.is_ascii_digit())
                .count();
            let delimited = matches!(bytes.get(digits), Some(b'.' | b')'));
            if !delimited {
                return None;
            }
            digits + 1
        }
        _ => return None,
    };
    if bytes.get(cursor) != Some(&b' ') {
        return None;
    }
    cursor += 1;
    let ticked = matches!(
        bytes.get(cursor..cursor + 4),
        Some(b"[ ] " | b"[x] " | b"[X] ")
    );
    if ticked {
        cursor += 4;
    }
    Some(cursor)
}

/// True when the line opens or closes a fenced code block.
fn is_fence(line: &str) -> bool {
    let trimmed = line.trim_start_matches([' ', '\t']);
    trimmed.starts_with("```")
}

/// The document highlighter. `fences` is the per-line "am I inside code?"
/// carry — the one piece of cross-line state, kept as a vector so the widget's
/// incremental relayout can resume at any line instead of rescanning.
#[derive(Debug)]
pub struct DocumentHighlighter {
    current_line: usize,
    fences: Vec<bool>,
    caret: Caret,
}

impl Highlighter for DocumentHighlighter {
    type Settings = Caret;
    type Highlight = Mark;
    type Iterator<'a> = std::vec::IntoIter<(Range<usize>, Mark)>;

    fn new(caret: &Self::Settings) -> Self {
        Self {
            current_line: 0,
            fences: vec![false],
            caret: caret.clone(),
        }
    }

    fn update(&mut self, caret: &Self::Settings) {
        // A palette flip restyles every line; a caret move only restyles the
        // line it left and the line it landed on.
        let restyle_everything =
            self.caret.dark != caret.dark || self.caret.commented != caret.commented;
        if restyle_everything {
            self.caret = caret.clone();
            self.fences.truncate(1);
            self.current_line = 0;
            return;
        }
        let earliest = self.caret.line.min(caret.line);
        self.caret = caret.clone();
        self.change_line(earliest);
    }

    fn change_line(&mut self, line: usize) {
        if line >= self.fences.len() {
            self.fences.truncate(1);
            self.current_line = 0;
            return;
        }
        self.fences.truncate(line + 1);
        self.current_line = line;
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        let index = self.current_line;
        let inside_code = self.fences[index];
        let on_caret_line = index == self.caret.line;
        let commented = self.caret.commented.contains(&(index as i64));
        let (marks, next_inside) = match index == 0 {
            true => (vec![(0..line.len(), Mark::Title)], false),
            false => highlight(line, inside_code, on_caret_line, commented),
        };

        self.current_line += 1;
        match self.fences.len() == self.current_line {
            true => self.fences.push(next_inside),
            false => self.fences[self.current_line] = next_inside,
        }
        marks.into_iter()
    }

    fn current_line(&self) -> usize {
        self.current_line
    }
}

/// One line's runs, and whether the NEXT line is inside a code fence.
fn highlight(
    line: &str,
    inside_code: bool,
    on_caret_line: bool,
    commented: bool,
) -> (Vec<(Range<usize>, Mark)>, bool) {
    if is_fence(line) {
        let fence = vec![(
            0..line.len(),
            Mark::Fence {
                hidden: !on_caret_line,
            },
        )];
        return (fence, !inside_code);
    }
    if inside_code {
        return (vec![(0..line.len(), Mark::CodeBody)], true);
    }

    let (prefix, content) = prefix_of(line);
    let style = Style {
        heading: match prefix {
            Prefix::Heading(level) => Some(level),
            _ => None,
        },
        quote: prefix == Prefix::Quote,
        callout: prefix == Prefix::Callout,
        divider: prefix == Prefix::Divider,
        commented,
        ..Style::default()
    };

    let mut marks = Vec::new();
    // The marker run. A list keeps its own — it IS the bullet the reader sees.
    match prefix {
        Prefix::Body => {}
        Prefix::List => marks.push((0..content, Mark::ListMarker(style))),
        Prefix::Divider => marks.push((
            0..line.len(),
            Mark::Marker {
                hidden: false,
                style,
            },
        )),
        Prefix::Heading(_) | Prefix::Quote | Prefix::Callout => marks.push((
            0..content,
            Mark::Marker {
                hidden: !on_caret_line,
                style,
            },
        )),
    }

    if prefix == Prefix::Divider {
        return (marks, false);
    }

    // The line's own body carries the block style; the inline scanner then
    // overlays bold/italic/link on top of it, offset past the prefix.
    marks.push((content..line.len(), Mark::Body(style)));
    for (range, inline) in inline_marks(&line[content..]) {
        let shifted = content + range.start..content + range.end;
        let inline_style = match inline {
            Inline::Marker => {
                marks.push((
                    shifted,
                    Mark::Marker {
                        hidden: !on_caret_line,
                        style,
                    },
                ));
                continue;
            }
            Inline::Bold => Style {
                strong: true,
                ..style
            },
            Inline::Italic => Style {
                emphasis: true,
                ..style
            },
            Inline::Link => Style {
                link: true,
                ..style
            },
        };
        marks.push((shifted, Mark::Body(inline_style)));
    }
    (marks, false)
}

/// The document ink, one set per palette reading of `theme.ice`.
struct Ink {
    body: Color,
    strong: Color,
    muted: Color,
    marker: Color,
    link: Color,
    code_ink: Color,
    code_plate: Color,
    code_line: Color,
    callout_plate: Color,
    callout_line: Color,
    comment_wash: Color,
}

const fn rgb8(r: u8, g: u8, b: u8) -> Color {
    Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
}

/// A plate color MUST be translucent: the runtime draws an opaque
/// `line_highlight` above the glyphs (ducktape-ui bug, reproduced in the
/// pinned `markdown-editor` example by making its wash opaque), so every
/// plate here is a wash over the page instead of a solid.
const fn wash(r: u8, g: u8, b: u8, a: f32) -> Color {
    Color { a, ..rgb8(r, g, b) }
}

const LIGHT: Ink = Ink {
    body: rgb8(0x3a, 0x38, 0x33),
    strong: rgb8(0x26, 0x25, 0x1f),
    muted: rgb8(0x6b, 0x69, 0x62),
    marker: rgb8(0xb3, 0xb1, 0xa8),
    link: rgb8(0x5f, 0x7a, 0x9e),
    // The chat message block landed here first (#927): a quiet wash with a
    // hairline, never a dark slab — "black/26 was invisible in dark", and a
    // dark slab swallowed its own ink here too.
    code_ink: rgb8(0x3a, 0x38, 0x33),
    code_plate: wash(0x3a, 0x38, 0x33, 0.05),
    code_line: wash(0x3a, 0x38, 0x33, 0.10),
    callout_plate: wash(0xa0, 0x5a, 0x3c, 0.06),
    callout_line: wash(0xa0, 0x5a, 0x3c, 0.14),
    comment_wash: wash(0xa0, 0x5a, 0x3c, 0.09),
};

const DARK: Ink = Ink {
    body: rgb8(0xd4, 0xd2, 0xca),
    strong: rgb8(0xe8, 0xe6, 0xdf),
    muted: rgb8(0xa8, 0xa6, 0x9c),
    marker: rgb8(0x6b, 0x6a, 0x61),
    link: rgb8(0x8f, 0xa9, 0xc9),
    code_ink: rgb8(0xd4, 0xd2, 0xca),
    code_plate: wash(0xd4, 0xd2, 0xca, 0.07),
    code_line: wash(0xd4, 0xd2, 0xca, 0.12),
    callout_plate: wash(0xc9, 0x8a, 0x63, 0.10),
    callout_line: wash(0xc9, 0x8a, 0x63, 0.20),
    comment_wash: wash(0xc9, 0x8a, 0x63, 0.13),
};

fn ink(dark: bool) -> &'static Ink {
    match dark {
        true => &DARK,
        false => &LIGHT,
    }
}

fn body_font(weight: Weight, style: FontStyle) -> Font {
    Font {
        weight,
        style,
        ..crate::Ducktape::default_font()
    }
}

fn code_font() -> Font {
    Font {
        family: Family::Name(design::fonts::FAMILY_MONO),
        ..crate::Ducktape::default_font()
    }
}

/// The callout tile — the `card_wash` plate the block widget used to draw,
/// painted by the highlighter now that the callout is a line of the document.
fn callout_plate(ink: &Ink) -> TextHighlight {
    TextHighlight {
        background: ink.callout_plate.into(),
        border: Border {
            color: ink.callout_line,
            width: 1.0,
            radius: 11.0.into(),
        },
    }
}

/// The code plate: one continuous background across every visual line of the
/// fence, which is what `line_highlight` (as opposed to `highlight`) buys.
fn code_plate(ink: &Ink) -> TextHighlight {
    TextHighlight {
        background: ink.code_plate.into(),
        border: Border {
            color: ink.code_line,
            width: 1.0,
            radius: 10.0.into(),
        },
    }
}

/// The paint for one run. This is the whole visual contract of the surface.
pub fn format(mark: &Mark, dark: bool) -> Format {
    let ink = ink(dark);
    match *mark {
        Mark::Title => Format {
            color: Some(ink.strong),
            font: Some(body_font(Weight::Semibold, FontStyle::Normal)),
            size: Some(Pixels(TITLE_SIZE)),
            line_height: Some(LineHeight::Absolute(Pixels(TITLE_SIZE * TITLE_LINE_HEIGHT))),
            ..Format::default()
        },
        Mark::Marker { hidden, style } => {
            let mut format = body_format(style, ink);
            format.color = Some(match hidden {
                true => Color::TRANSPARENT,
                false => ink.marker,
            });
            if hidden {
                // Collapse the glyphs without collapsing the LINE: the body run
                // beside them still carries the real line height.
                format.size = Some(Pixels(HIDDEN_SIZE));
                format.line_height = None;
            }
            format
        }
        Mark::ListMarker(style) => Format {
            color: Some(ink.muted),
            ..body_format(style, ink)
        },
        Mark::Body(style) => body_format(style, ink),
        Mark::Fence { hidden } => Format {
            color: Some(match hidden {
                true => Color::TRANSPARENT,
                false => ink.marker,
            }),
            font: Some(code_font()),
            size: Some(Pixels(match hidden {
                true => HIDDEN_SIZE,
                false => CODE_SIZE,
            })),
            line_height: Some(LineHeight::Absolute(Pixels(match hidden {
                // The hidden fence row is the plate's own vertical inset.
                true => CODE_PLATE_PAD,
                false => CODE_SIZE * CODE_LINE_HEIGHT,
            }))),
            line_highlight: Some(code_plate(ink)),
            line_padding: Padding::from([0.0, CODE_PLATE_PAD]),
            ..Format::default()
        },
        Mark::CodeBody => Format {
            color: Some(ink.code_ink),
            font: Some(code_font()),
            size: Some(Pixels(CODE_SIZE)),
            line_height: Some(LineHeight::Absolute(Pixels(CODE_SIZE * CODE_LINE_HEIGHT))),
            line_highlight: Some(code_plate(ink)),
            line_padding: Padding::from([0.0, CODE_PLATE_PAD]),
            ..Format::default()
        },
    }
}

fn body_format(style: Style, ink: &Ink) -> Format {
    let weight = match style.strong || style.heading.is_some() {
        true => Weight::Semibold,
        false => Weight::Normal,
    };
    let italic = match style.emphasis || style.quote {
        true => FontStyle::Italic,
        false => FontStyle::Normal,
    };
    let color = if style.link {
        ink.link
    } else if style.quote || style.divider {
        ink.muted
    } else if style.heading.is_some() {
        ink.strong
    } else {
        ink.body
    };
    let mut format = Format {
        color: Some(color),
        font: Some(body_font(weight, italic)),
        ..Format::default()
    };
    // A commented block wears a quiet brand wash across its lines — the
    // in-document anchor for the rail's threads. The callout keeps its own
    // tile (the stronger plate already marks the line).
    if style.commented && !style.callout {
        format.line_highlight = Some(TextHighlight {
            background: ink.comment_wash.into(),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 6.0.into(),
            },
        });
    }
    if let Some(level) = style.heading {
        let step = usize::from(level).saturating_sub(1).min(2);
        let size = HEADING_SIZE[step];
        format.size = Some(Pixels(size));
        format.line_height = Some(LineHeight::Absolute(Pixels(
            size * HEADING_LINE_HEIGHT[step],
        )));
        return format;
    }
    if style.callout {
        format.size = Some(Pixels(CALLOUT_SIZE));
        format.line_height = Some(LineHeight::Absolute(Pixels(
            CALLOUT_SIZE * CALLOUT_LINE_HEIGHT,
        )));
        format.line_highlight = Some(callout_plate(ink));
        format.line_padding = Padding::from([9.0, 14.0]);
        return format;
    }
    if style.quote {
        format.line_height = Some(LineHeight::Absolute(Pixels(BODY_SIZE * QUOTE_LINE_HEIGHT)));
    }
    format
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shapes(line: &str) -> (Prefix, usize) {
        prefix_of(line)
    }

    #[test]
    fn prefixes_map_to_the_block_kinds_the_module_stores() {
        assert_eq!(shapes("# Title").0, Prefix::Heading(1));
        assert_eq!(shapes("### Small").0, Prefix::Heading(3));
        assert_eq!(shapes("- item").0, Prefix::List);
        assert_eq!(shapes("1. item").0, Prefix::List);
        assert_eq!(shapes("- [ ] todo").0, Prefix::List);
        assert_eq!(shapes("> quoted").0, Prefix::Quote);
        assert_eq!(shapes("!> noted").0, Prefix::Callout);
        assert_eq!(shapes("---").0, Prefix::Divider);
        assert_eq!(shapes("plain").0, Prefix::Body);
    }

    #[test]
    fn a_fourth_hash_is_prose_because_the_module_stops_at_heading_3() {
        assert_eq!(shapes("#### four").0, Prefix::Body);
        // ...and a bare `#` with no space is a tag, not a heading.
        assert_eq!(shapes("#tag").0, Prefix::Body);
    }

    #[test]
    fn content_offsets_skip_the_marker_but_keep_list_bullets() {
        assert_eq!(shapes("## Two"), (Prefix::Heading(2), 3));
        assert_eq!(shapes("> q"), (Prefix::Quote, 2));
        assert_eq!(shapes("!> c"), (Prefix::Callout, 3));
        // The list content starts PAST the tick, so `[x] ` never reads as prose.
        assert_eq!(shapes("- [x] done"), (Prefix::List, 6));
        assert_eq!(shapes("12) twelve"), (Prefix::List, 4));
    }

    #[test]
    fn indented_lines_keep_their_indent_in_the_content_offset() {
        assert_eq!(shapes("    - nested"), (Prefix::List, 6));
        assert_eq!(shapes("  ## deep"), (Prefix::Heading(2), 5));
    }

    #[test]
    fn a_fence_toggles_the_carry_and_its_body_is_never_reparsed() {
        let (marks, inside) = highlight("```", false, false, false);
        assert!(inside);
        assert!(matches!(marks[0].1, Mark::Fence { hidden: true }));
        // `# ` inside a fence is code, not a heading.
        let (body, still_inside) = highlight("# not a heading", true, false, false);
        assert!(still_inside);
        assert_eq!(body, vec![(0..15, Mark::CodeBody)]);
        let (_, closed) = highlight("```", true, false, false);
        assert!(!closed);
    }

    #[test]
    fn the_caret_line_reveals_its_markers_and_other_lines_hide_them() {
        let (away, _) = highlight("## Heading", false, false, false);
        assert!(matches!(away[0].1, Mark::Marker { hidden: true, .. }));
        let (under, _) = highlight("## Heading", false, true, false);
        assert!(matches!(under[0].1, Mark::Marker { hidden: false, .. }));
    }

    #[test]
    fn inline_marks_ride_on_top_of_the_block_style_at_the_right_offsets() {
        let (marks, _) = highlight("## say **hi**", false, false, false);
        let bold = marks
            .iter()
            .find(|(_, mark)| matches!(mark, Mark::Body(style) if style.strong))
            .expect("a bold run");
        // "## say **hi**" — the bold body is `hi` at bytes 9..11.
        assert_eq!(bold.0, 9..11);
        let Mark::Body(style) = bold.1 else {
            unreachable!("matched a body run")
        };
        // ...and it is STILL a heading, so it keeps the heading metrics.
        assert_eq!(style.heading, Some(2));
    }

    #[test]
    fn line_zero_is_the_title_and_nothing_in_it_is_markdown() {
        let mut highlighter = <DocumentHighlighter as Highlighter>::new(&Caret {
            line: 0,
            column: 0,
            dark: false,
            commented: Vec::new(),
        });
        let title: Vec<_> = highlighter.highlight_line("# still the title").collect();
        assert_eq!(title, vec![(0..17, Mark::Title)]);
        // ...and line 1 parses normally, so the title costs the body nothing.
        let body: Vec<_> = highlighter.highlight_line("# a real heading").collect();
        assert!(matches!(body[0].1, Mark::Marker { .. }));
    }

    #[test]
    fn a_hidden_marker_keeps_a_measurable_size() {
        let hidden = format(
            &Mark::Marker {
                hidden: true,
                style: Style::default(),
            },
            false,
        );
        assert_eq!(hidden.color, Some(Color::TRANSPARENT));
        assert!(hidden.size.expect("a size").0 > 0.0);
    }
}

#[cfg(test)]
mod plate_probe {
    use super::*;

    #[test]
    fn the_code_body_line_keeps_full_size_ink() {
        let mut hl = <DocumentHighlighter as Highlighter>::new(&Caret {
            line: 0,
            column: 0,
            dark: false,
            commented: Vec::new(),
        });
        let lines = [
            "Team Runbook v2",
            "How we ship: branch off dev, PR, review, merge.",
            "## Rules",
            "- ship small",
            "- test everything",
            "- [ ] wire QA",
            "```",
            "cargo test -p ducktape-app",
            "```",
        ];
        let mut all = Vec::new();
        for line in lines {
            all.push(hl.highlight_line(line).collect::<Vec<_>>());
        }
        let cargo = &all[7];
        assert_eq!(cargo.len(), 1, "{cargo:?}");
        assert!(matches!(cargo[0].1, Mark::CodeBody), "{cargo:?}");
        let f = format(&cargo[0].1, false);
        assert_eq!(f.size.map(|s| s.0), Some(CODE_SIZE));
        assert!(f.color.expect("ink").a > 0.9, "opaque ink");
        assert!(f.line_highlight.is_some());
    }
}
