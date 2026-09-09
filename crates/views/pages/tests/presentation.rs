//! Compare the flattened wire paint with the existing native Markdown policy.
use iced::advanced::text::Highlighter;
use pages_view::{editor_binding, editor_menu, markdown};
use ui_lang_guest::{Editor, wire};
#[path = "../src/editor_presentation.rs"]
pub mod presentation;

#[test]
fn flattened_runs_preserve_native_body_gaps_and_inline_precedence() {
    let text = "Title\n한글 ordinary **bold** gap _italic_ and https://example.com/end.\n- [ ] todo 👍🏽\n```\nlet n = 1;\n```";
    let editor = Editor::new(text);
    for dark in [false, true] {
        let caret = markdown::Caret {
            line: 2,
            column: 0,
            dark,
            commented: vec![1],
        };
        let state = ui_lang_guest::EditorStateView {
            cursor: wire::EditorCursor {
                position: wire::EditorPosition { line: 2, column: 0 },
                selection: None,
            },
            ..editor.state_view()
        };
        let actual =
            presentation::build(state, editor_binding::initial_menu(), dark, vec![1]).unwrap();
        let mut native = markdown::DocumentHighlighter::new(&caret);
        for (line, source) in wire::editor_lines(text).enumerate() {
            let expected: Vec<_> = native.highlight_line(source).collect();
            for (byte, _) in source.char_indices() {
                let expected = expected
                    .iter()
                    .rev()
                    .find(|(range, _)| range.contains(&byte))
                    .map(|(_, mark)| presentation::convert(markdown::format(mark, dark)).unwrap());
                let actual = actual
                    .spans
                    .iter()
                    .find(|span| {
                        span.line as usize == line
                            && span.start as usize <= byte
                            && byte < span.end as usize
                    })
                    .map(|span| actual.formats[span.format as usize].clone());
                assert_eq!(
                    actual, expected,
                    "line {line}, UTF-8 byte {byte}, dark {dark}"
                );
            }
        }
    }
}
