use super::*;

#[test]
fn malformed_highlight_boundaries_never_drop_unicode_text() {
    let segments = compose_segments(
        "é",
        &[(
            1..2,
            Format {
                color: Some(Color::BLACK),
                ..Format::default()
            },
        )],
    );

    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].range, 0.."é".len());
}

#[test]
fn lightweight_composition_parser_matches_iced_line_boundaries() {
    for source in [
        "",
        "\n",
        "\r",
        "\r\n",
        "\n\r",
        "첫째\n둘째",
        "첫째\r\n둘째\n",
        "첫째\n\r둘째\r",
    ] {
        let content = Content::with_text(source);
        let normalized = content.text();
        let parsed = TextLines::parse(&normalized);
        let lines = Lines::new(&normalized, &parsed);

        assert_eq!(
            (0..lines.len()).map(|i| lines.get(i)).collect::<Vec<_>>(),
            content_lines(&content),
            "{source:?}"
        );
        for line in 0..lines.len() {
            let text = lines.get(line);
            for column in [0, text.len()] {
                let position = Position { line, column };
                assert_eq!(
                    parsed.position(parsed.offset(position)),
                    position,
                    "{source:?}"
                );
            }
        }
    }
}
