use super::*;

#[derive(Default)]
struct WholeLine {
    current_line: usize,
}

impl text::Highlighter for WholeLine {
    type Settings = ();
    type Highlight = ();
    type Iterator<'a> = std::iter::Once<(Range<usize>, ())>;

    fn new(_settings: &Self::Settings) -> Self {
        Self::default()
    }

    fn update(&mut self, _new_settings: &Self::Settings) {}

    fn change_line(&mut self, line: usize) {
        self.current_line = line;
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        self.current_line += 1;
        std::iter::once((0..line.len(), ()))
    }

    fn current_line(&self) -> usize {
        self.current_line
    }
}

struct CaretSizedMarker {
    current_line: usize,
    expanded: bool,
}

#[derive(Default)]
struct ToggleHighlighter {
    current_line: usize,
    inside: bool,
    states: Vec<bool>,
}

impl text::Highlighter for ToggleHighlighter {
    type Settings = ();
    type Highlight = bool;
    type Iterator<'a> = std::option::IntoIter<(Range<usize>, bool)>;

    fn new(_settings: &Self::Settings) -> Self {
        Self {
            states: vec![false],
            ..Self::default()
        }
    }

    fn update(&mut self, _new_settings: &Self::Settings) {}

    fn change_line(&mut self, line: usize) {
        self.current_line = line;
        self.inside = self.states.get(line).copied().unwrap_or(false);
        self.states.truncate(line.saturating_add(1));
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        if line == "toggle" {
            self.inside = !self.inside;
        }
        self.current_line += 1;
        if self.states.len() == self.current_line {
            self.states.push(self.inside);
        } else if let Some(state) = self.states.get_mut(self.current_line) {
            *state = self.inside;
        }
        (!line.is_empty())
            .then_some((0..line.len(), self.inside))
            .into_iter()
    }

    fn current_line(&self) -> usize {
        self.current_line
    }
}

impl text::Highlighter for CaretSizedMarker {
    type Settings = bool;
    type Highlight = bool;
    type Iterator<'a> = std::vec::IntoIter<(Range<usize>, bool)>;

    fn new(expanded: &Self::Settings) -> Self {
        Self {
            current_line: 0,
            expanded: *expanded,
        }
    }

    fn update(&mut self, expanded: &Self::Settings) {
        self.expanded = *expanded;
    }

    fn change_line(&mut self, line: usize) {
        self.current_line = line;
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        self.current_line += 1;
        (!line.is_empty())
            .then_some((0..1, self.expanded))
            .into_iter()
            .collect::<Vec<_>>()
            .into_iter()
    }

    fn current_line(&self) -> usize {
        self.current_line
    }
}

fn test_layout_style(width: f32) -> LineLayoutStyle {
    LineLayoutStyle {
        width,
        font: Font::DEFAULT,
        text_size: Pixels(16.0),
        line_height: text::LineHeight::Relative(1.6),
        wrapping: text::Wrapping::Word,
    }
}

/// Lends owned test lines to the layout as a borrowed document, the way the
/// widget lends its own source text.
struct TestDoc {
    text: String,
    map: TextLines,
}

impl TestDoc {
    fn new(lines: &[String]) -> Self {
        let text = lines.join("\n");
        let map = TextLines::parse(&text);
        Self { text, map }
    }

    fn lines(&self) -> Lines<'_> {
        Lines::new(&self.text, &self.map)
    }
}

fn content_lines(content: &Content) -> Vec<String> {
    content.lines().map(|line| line.text.into_owned()).collect()
}

fn test_change(
    first_changed_line: usize,
    removed_lines: usize,
    inserted_lines: usize,
) -> EditorChange {
    EditorChange::new(
        ContentVersion::new(1, 0),
        ContentVersion::new(1, 1),
        first_changed_line,
        removed_lines,
        inserted_lines,
    )
}

fn headless_renderer() -> iced::Renderer {
    use iced::advanced::renderer::Headless;

    iced_test::futures::futures::executor::block_on(<iced::Renderer as Headless>::new(
        Font::DEFAULT,
        Pixels(16.0),
        Some("tiny-skia"),
    ))
    .expect("headless renderer")
}

#[path = "ime.rs"]
mod ime_tests;
#[path = "keyboard.rs"]
mod keyboard_tests;
#[path = "layout.rs"]
mod layout_tests;
#[path = "paint.rs"]
mod paint_tests;
#[path = "performance.rs"]
mod performance_tests;
#[path = "pointer.rs"]
mod pointer_tests;
#[path = "unicode.rs"]
mod unicode_tests;
