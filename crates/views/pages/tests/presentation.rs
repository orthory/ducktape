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
            focused: true,
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
            presentation::build(state, editor_binding::initial_menu(), dark, vec![1], true)
                .unwrap();
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

#[test]
fn named_link_syntax_hides_on_blur_and_returns_without_changing_source_or_caret() {
    let line = "앞 [작업 보기](duck://agents/runs/abc) 뒤";
    let text = format!("Title\n{line}");
    let editor = Editor::new(&text);
    let state = ui_lang_guest::EditorStateView {
        cursor: wire::EditorCursor {
            position: wire::EditorPosition { line: 1, column: 8 },
            selection: None,
        },
        ..editor.state_view()
    };
    for focused in [true, false, true] {
        let paint = presentation::build(
            state,
            editor_binding::initial_menu(),
            false,
            vec![],
            focused,
        )
        .unwrap();
        let visible: String = paint
            .spans
            .iter()
            .filter(|span| span.line == 1)
            .filter(|span| paint.formats[span.format as usize].size.unwrap_or(14.0) > 1.0)
            .map(|span| &line[span.start as usize..span.end as usize])
            .collect();
        assert_eq!(
            visible,
            if focused {
                line
            } else {
                "앞 작업 보기 뒤"
            }
        );
        let hit = paint
            .affordances
            .hits
            .iter()
            .find(|hit| hit.tag == 2)
            .unwrap();
        assert_eq!(&line[hit.start as usize..hit.end as usize], "작업 보기");
        assert_eq!(
            pages_view::inline::document_link_at(line, hit.start as usize).as_deref(),
            Some("duck://agents/runs/abc")
        );
        assert_eq!(state.text, text);
        assert_eq!(state.cursor.position.column, 8);
    }
}

#[test]
fn named_links_use_markdown_destinations_and_leave_incomplete_syntax_editable() {
    use pages_view::inline::{Inline, document_link_at, document_marks};
    for (source, label, destination) in [
        (
            "[문서](https://example.com/a_(b))",
            "문서",
            "https://example.com/a_(b)",
        ),
        (
            "[문서](<https://example.com/a> \"설명\")",
            "문서",
            "https://example.com/a",
        ),
        (
            "[문서](https://example.com/?a=1&amp;b=2)",
            "문서",
            "https://example.com/?a=1&b=2",
        ),
    ] {
        let marks = document_marks(source);
        let (range, _) = marks
            .iter()
            .find(|(_, mark)| *mark == Inline::Link)
            .unwrap();
        assert_eq!(&source[range.clone()], label);
        assert_eq!(
            document_link_at(source, range.start).as_deref(),
            Some(destination)
        );
        assert_eq!(
            marks
                .iter()
                .filter(|(_, kind)| *kind == Inline::Marker)
                .count(),
            2
        );
    }
    for source in ["[문서](not finished", "\\[문서](https://example.com)"] {
        assert!(
            !document_marks(source)
                .iter()
                .any(|(_, mark)| *mark == Inline::Marker)
        );
    }
}
