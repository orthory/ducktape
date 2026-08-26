use super::*;

#[test]
fn wrapped_rich_motion_keeps_visual_direction_and_boundaries() {
    let lines = [
        "one two three four five six".to_owned(),
        "seven eight nine".to_owned(),
    ];
    let mut document = DocumentLayout::default();
    document.update(
        TestDoc::new(&lines).lines(),
        &mut WholeLine::default(),
        &|_| Format::default(),
        test_layout_style(70.0),
        DocumentUpdate::text(DocumentChange::Discover),
        usize::MAX,
    );

    let position = Position {
        line: 0,
        column: lines[0].len() - 1,
    };
    let caret = document.caret(position);
    let center_y = caret.y + caret.height / 2.0;
    let viewport_height = caret.height * 2.0;
    let seeded_x = caret.x + 1.0;
    let expected = [
        (
            Motion::Up,
            document.hit(Point::new(caret.x, center_y - caret.height)),
            None,
        ),
        (
            Motion::Down,
            document.hit(Point::new(caret.x, center_y + caret.height)),
            None,
        ),
        (Motion::Home, document.hit(Point::new(0.0, center_y)), None),
        (Motion::End, document.hit(Point::new(70.0, center_y)), None),
        (
            Motion::PageUp,
            document.hit(Point::new(caret.x, center_y - viewport_height)),
            None,
        ),
        (
            Motion::PageDown,
            document.hit(Point::new(seeded_x, center_y + viewport_height)),
            Some(seeded_x),
        ),
    ];
    assert_eq!(expected[0].1.line, 0, "Up stays within the wrap");
    assert_eq!(expected[1].1.line, 1, "Down crosses the source line");
    assert_ne!(expected[0].1, expected[1].1, "vertical directions differ");
    assert_ne!(expected[0].1, expected[4].1, "PageUp skips a run");
    assert_ne!(expected[1].1, expected[5].1, "PageDown skips a run");
    assert_ne!(expected[2].1, expected[3].1, "Home and End differ");

    for (motion, expected, initial_x) in expected {
        let hit_x = initial_x.unwrap_or(caret.x);
        let mut preferred_x = initial_x;
        let moved = movement::move_cursor(
            &document,
            &mut preferred_x,
            viewport_height,
            Cursor {
                position,
                selection: None,
            },
            motion,
            false,
        );
        assert_eq!(moved.position, expected, "{motion:?}");
        assert_eq!(moved.selection, None, "{motion:?}");
        if matches!(motion, Motion::Home | Motion::End) {
            assert_eq!(preferred_x, None, "{motion:?}");
        } else {
            assert_eq!(preferred_x, Some(hit_x), "{motion:?}");
        }
    }
}

#[test]
fn overlapping_formats_keep_block_metrics_under_token_colors() {
    let block = Format {
        size: Some(Pixels(14.0)),
        line_height: Some(text::LineHeight::Absolute(Pixels(24.0))),
        line_padding: Padding::from([0.0, 12.0]),
        line_highlight: Some(text::Highlight {
            background: iced::Background::Color(Color::BLACK),
            border: iced::Border::default(),
        }),
        ..Format::default()
    };
    let token = Format {
        color: Some(Color::from_rgb(1.0, 0.0, 0.0)),
        ..Format::default()
    };

    let segments = compose_segments("let value", &[(0..9, block), (4..9, token)]);

    assert_eq!(segments.len(), 2);
    assert_eq!(segments[1].format.size, block.size);
    assert_eq!(segments[1].format.line_height, block.line_height);
    assert_eq!(segments[1].format.color, token.color);
    assert_eq!(segments[1].format.line_highlight, block.line_highlight);
    assert_eq!(segments[1].format.line_padding, block.line_padding);
}

#[test]
fn line_padding_insets_a_line_that_wears_no_highlight() {
    // A nesting indent is padding with nothing painted behind it. Reading the
    // padding off the highlighted run made that impossible to express.
    let source = Content::with_text("nested item");
    let mut document = DocumentLayout::default();
    document.update(
        TestDoc::new(&content_lines(&source)).lines(),
        &mut WholeLine::default(),
        &|_| Format {
            line_padding: Padding {
                left: 22.0,
                ..Padding::ZERO
            },
            ..Format::default()
        },
        test_layout_style(100.0),
        DocumentUpdate::text(DocumentChange::Discover),
        usize::MAX,
    );

    let line = &document.lines[0];
    assert_eq!(line.signature.line_padding.left, 22.0);
    assert!(line.signature.line_highlight.is_none());
    assert!((line.paragraph.bounds().width - 78.0).abs() < 0.01);
}

#[test]
fn line_padding_changes_wrapping_caret_and_hit_geometry() {
    let source = Content::with_text("code that wraps");
    let padding = Padding {
        top: 4.0,
        right: 12.0,
        bottom: 6.0,
        left: 12.0,
    };
    let mut document = DocumentLayout::default();
    document.update(
        TestDoc::new(&content_lines(&source)).lines(),
        &mut WholeLine::default(),
        &|_| Format {
            line_highlight: Some(text::Highlight {
                background: iced::Background::Color(Color::BLACK),
                border: iced::Border::default(),
            }),
            line_padding: padding,
            ..Format::default()
        },
        test_layout_style(100.0),
        DocumentUpdate::text(DocumentChange::Discover),
        usize::MAX,
    );

    let line = &document.lines[0];
    assert!((line.paragraph.bounds().width - 76.0).abs() < 0.01);
    assert!(
        (line.height
            - paragraph_height(
                &line.paragraph,
                Pixels(16.0),
                text::LineHeight::Relative(1.6),
            )
            - padding.y())
        .abs()
            < 0.01
    );

    let start = document.caret(Position { line: 0, column: 0 });
    assert!((start.x - padding.left).abs() < 0.01);
    assert!((start.y - padding.top).abs() < 0.01);
    assert_eq!(
        document.hit(Point::new(start.x, start.y + start.height / 2.0)),
        Position { line: 0, column: 0 }
    );
    assert_eq!(
        document.hit_test(Point::new(start.x, start.y + start.height / 2.0)),
        Some(Position { line: 0, column: 0 })
    );
    assert_eq!(
        document.hit_test(Point::new(99.0, start.y + start.height / 2.0)),
        None
    );
}

#[test]
fn inline_highlight_padding_cannot_bleed_into_adjacent_lines() {
    let bounds = Rectangle::new(Point::new(20.0, 10.0), Size::new(30.0, 20.0));
    let line = Rectangle::new(Point::new(0.0, 10.0), Size::new(100.0, 20.0));
    let padded = span_highlight_bounds(
        bounds,
        Padding {
            top: 5.0,
            right: 6.0,
            bottom: 5.0,
            left: 6.0,
        },
        line,
    )
    .expect("visible highlight");

    assert_eq!(
        padded,
        Rectangle::new(Point::new(14.0, 10.0), Size::new(42.0, 20.0))
    );
}

#[test]
fn hidden_markers_and_heading_text_share_one_hit_test_layout() {
    let spans = vec![
        to_span(
            "# ".to_owned(),
            Format {
                size: Some(Pixels(0.01)),
                color: Some(Color::TRANSPARENT),
                ..Format::default()
            },
        ),
        to_span(
            "Heading".to_owned(),
            Format {
                size: Some(Pixels(30.0)),
                line_height: Some(text::LineHeight::Absolute(Pixels(42.0))),
                ..Format::default()
            },
        ),
    ];
    let paragraph = GraphicsParagraph::with_spans(Text {
        content: spans.as_slice(),
        bounds: Size::new(500.0, 500.0),
        size: Pixels(16.0),
        line_height: text::LineHeight::Relative(1.6),
        font: Font::DEFAULT,
        align_x: text::Alignment::Default,
        align_y: alignment::Vertical::Top,
        shaping: text::Shaping::Advanced,
        wrapping: text::Wrapping::Word,
    });

    let caret = caret_rectangle(paragraph.buffer(), Position { line: 0, column: 2 });
    let hit = hit_position(paragraph.buffer(), Point::new(caret.x, caret.y + 1.0));

    assert_eq!(hit.line, 0);
    assert_eq!(hit.column, 2);
    assert!(caret.height >= 42.0);
}

#[test]
fn line_paragraphs_preserve_whole_document_caret_geometry() {
    let heading = Format {
        size: Some(Pixels(30.0)),
        line_height: Some(text::LineHeight::Absolute(Pixels(42.0))),
        ..Format::default()
    };
    let hidden = Format {
        size: Some(Pixels(0.01)),
        color: Some(Color::TRANSPARENT),
        ..Format::default()
    };
    let code = Format {
        size: Some(Pixels(14.0)),
        line_height: Some(text::LineHeight::Absolute(Pixels(24.0))),
        ..Format::default()
    };
    let signatures = [
        StyledLine {
            text: "# 제목".to_owned(),
            segments: vec![
                Segment {
                    range: 0..2,
                    format: hidden,
                },
                Segment {
                    range: 2.."# 제목".len(),
                    format: heading,
                },
            ],
            empty_format: Format::default(),
            line_highlight: None,
            line_padding: Padding::ZERO,
            line_rule: None,
        },
        StyledLine {
            text: "a body line long enough to wrap".to_owned(),
            segments: vec![Segment {
                range: 0.."a body line long enough to wrap".len(),
                format: Format::default(),
            }],
            empty_format: Format::default(),
            line_highlight: None,
            line_padding: Padding::ZERO,
            line_rule: None,
        },
        StyledLine {
            text: String::new(),
            segments: Vec::new(),
            empty_format: code,
            line_highlight: None,
            line_padding: Padding::ZERO,
            line_rule: None,
        },
        StyledLine {
            text: "let value = 1;".to_owned(),
            segments: vec![Segment {
                range: 0.."let value = 1;".len(),
                format: code,
            }],
            empty_format: Format::default(),
            line_highlight: None,
            line_padding: Padding::ZERO,
            line_rule: None,
        },
    ];
    let style = test_layout_style(120.0);

    let mut document = DocumentLayout::default();
    for signature in signatures.iter().cloned() {
        let mut line = DocumentLine::new(signature, style);
        line.top = document.height;
        document.height += line.height;
        document.lines.push(line);
    }

    let mut reference_spans = Vec::new();
    for (line_index, signature) in signatures.iter().enumerate() {
        let ending = (line_index + 1 < signatures.len()).then_some("\n");
        if signature.segments.is_empty() {
            reference_spans.push(to_span(
                ending.unwrap_or_default().to_owned(),
                signature.empty_format,
            ));
            continue;
        }
        for (segment_index, segment) in signature.segments.iter().enumerate() {
            let mut text = signature.text[segment.range.clone()].to_owned();
            if segment_index + 1 == signature.segments.len()
                && let Some(ending) = ending
            {
                text.push_str(ending);
            }
            reference_spans.push(to_span(text, segment.format));
        }
    }
    let reference = GraphicsParagraph::with_spans(Text {
        content: reference_spans.as_slice(),
        bounds: Size::new(style.width, i32::MAX as f32),
        size: style.text_size,
        line_height: style.line_height,
        font: style.font,
        align_x: text::Alignment::Default,
        align_y: alignment::Vertical::Top,
        shaping: text::Shaping::Advanced,
        wrapping: style.wrapping,
    });

    let reference_height = paragraph_height(&reference, style.text_size, style.line_height);
    assert!(
        (document.height - reference_height).abs() < 0.01,
        "document height {} != reference height {reference_height}",
        document.height
    );
    for (line, signature) in signatures.iter().enumerate() {
        for column in [0, signature.text.len()] {
            let expected = caret_rectangle(reference.buffer(), Position { line, column });
            let actual = document.caret(Position { line, column });
            assert!(
                (actual.x - expected.x).abs() < 0.01
                    && (actual.y - expected.y).abs() < 0.01
                    && (actual.height - expected.height).abs() < 0.01,
                "caret mismatch at {line}:{column}: {actual:?} != {expected:?}"
            );
            let point = Point::new(expected.x, expected.y + expected.height / 2.0);
            assert_eq!(document.hit(point), hit_position(reference.buffer(), point));
        }
    }
}

#[test]
fn empty_formatted_lines_keep_their_rich_metrics() {
    let content = Content::with_text("\n");
    let format = Format {
        size: Some(Pixels(14.0)),
        line_height: Some(text::LineHeight::Absolute(Pixels(23.0))),
        ..Format::default()
    };
    let mut document = DocumentLayout::default();
    let rebuilt = document.update(
        TestDoc::new(&content_lines(&content)).lines(),
        &mut WholeLine::default(),
        &|_| format,
        test_layout_style(500.0),
        DocumentUpdate::text(DocumentChange::Discover),
        usize::MAX,
    );

    assert_eq!(rebuilt.rebuilt_lines, content.line_count());
    assert_eq!(document.lines.len(), content.line_count());
    assert!(
        document
            .lines
            .iter()
            .all(|line| line.spans.len() == 1 && line.spans[0].size == format.size)
    );
    assert!(
        document
            .lines
            .iter()
            .all(|line| line.strikethroughs == [None] && line.height >= 23.0)
    );
}

#[test]
fn strikethrough_keeps_its_explicit_color() {
    let color = Color::from_rgb8(0x12, 0x34, 0x56);
    let mut spans = Vec::new();
    let mut strikethroughs = Vec::new();
    push_span(
        &mut spans,
        &mut strikethroughs,
        "old".to_owned(),
        Format {
            color: Some(Color::WHITE),
            strikethrough: Some(color),
            ..Format::default()
        },
    );

    assert_eq!(strikethroughs, vec![Some(color)]);
    assert!(spans[0].strikethrough);
}

#[test]
fn consecutive_line_highlights_share_one_surface() {
    let code = text::Highlight {
        background: iced::Background::Color(Color::BLACK),
        border: iced::Border {
            radius: 3.0.into(),
            width: 1.0,
            color: Color::WHITE,
        },
    };
    let quote = text::Highlight {
        background: iced::Background::Color(Color::WHITE),
        border: iced::Border::default(),
    };
    let runs = [
        (Some(code), 0.0, 12.0),
        (Some(code), 12.0, 12.0),
        (Some(code), 24.0, 12.0),
        (None, 36.0, 12.0),
        (Some(code), 48.0, 12.0),
        (Some(quote), 60.0, 12.0),
    ];
    let mut groups = Vec::new();

    visit_line_highlight_groups(runs, |group| groups.push(group));

    assert_eq!(
        groups,
        vec![
            LineHighlightGroup {
                top: 0.0,
                height: 36.0,
                highlight: code,
            },
            LineHighlightGroup {
                top: 48.0,
                height: 12.0,
                highlight: code,
            },
            LineHighlightGroup {
                top: 60.0,
                height: 12.0,
                highlight: quote,
            },
        ]
    );

    let highlights = vec![Some(code); 256];
    let runs = highlights
        .iter()
        .copied()
        .enumerate()
        .map(|(line, highlight)| (highlight, line as f32 * 12.0, 12.0));
    groups.clear();

    visit_line_highlight_groups(runs, |group| groups.push(group));

    assert_eq!(
        groups,
        vec![LineHighlightGroup {
            top: 0.0,
            height: highlights.len() as f32 * 12.0,
            highlight: code,
        }]
    );
}

#[test]
fn a_draw_only_format_delta_above_the_viewport_defers_and_cleans_on_scroll_up() {
    let doc_lines: Vec<String> = (0..100).map(|index| format!("line {index}")).collect();
    let doc = TestDoc::new(&doc_lines);
    let mut document = DocumentLayout::default();
    let mut highlighter = WholeLine::default();
    let style = test_layout_style(400.0);

    let built = document.update(
        doc.lines(),
        &mut highlighter,
        &|_| Format::default(),
        style,
        DocumentUpdate::text(DocumentChange::Discover),
        usize::MAX,
    );
    assert_eq!(built.rebuilt_lines, 100);

    // A colour-only format key flip while the viewport shows [80, 100): the
    // walk classifies every line, rebuilds only the window, and marks the
    // prefix stale instead of shaping 80 paragraphs nobody can see.
    let recolor = |_: &()| Format {
        color: Some(Color::BLACK),
        ..Format::default()
    };
    let flipped = document.update(
        doc.lines(),
        &mut highlighter,
        &recolor,
        style,
        DocumentUpdate {
            change: DocumentChange::Unchanged,
            geometry_changed: false,
            format_changed: true,
            viewport_start: 80,
            stale_before: 0,
        },
        100,
    );
    assert_eq!(flipped.highlighted_lines, 100);
    assert_eq!(flipped.rebuilt_lines, 20);
    assert_eq!(flipped.format_stale_before, 80);
    assert_eq!(
        document.lines[90].signature.segments[0].format.color,
        Some(Color::BLACK)
    );
    assert_eq!(document.lines[10].signature.segments[0].format.color, None);
    // Deferring cannot move a line: the tops the reader scrolls by are exact.
    assert!((document.lines[10].top - 10.0 * document.lines[0].height).abs() < 0.01);

    // Scrolling up to [40, 60) re-opens a pass that cleans exactly the
    // revealed window and lowers the mark to its first line; what is left
    // below the window falls beyond `highlight_valid_until` and is cleaned
    // by the ordinary scroll-down pass when it is shown again.
    let cleaned = document.update(
        doc.lines(),
        &mut highlighter,
        &recolor,
        style,
        DocumentUpdate {
            change: DocumentChange::Unchanged,
            geometry_changed: false,
            format_changed: false,
            viewport_start: 40,
            stale_before: 80,
        },
        60,
    );
    assert_eq!(cleaned.rebuilt_lines, 20);
    assert_eq!(cleaned.format_stale_before, 40);
    assert_eq!(cleaned.highlight_valid_until, 60);
    assert_eq!(
        document.lines[50].signature.segments[0].format.color,
        Some(Color::BLACK)
    );
    assert_eq!(document.lines[10].signature.segments[0].format.color, None);
}

#[test]
fn a_format_delta_that_can_move_a_glyph_rebuilds_eagerly_everywhere() {
    let doc_lines: Vec<String> = (0..50).map(|index| format!("line {index}")).collect();
    let doc = TestDoc::new(&doc_lines);
    let mut document = DocumentLayout::default();
    let mut highlighter = WholeLine::default();
    let style = test_layout_style(400.0);

    document.update(
        doc.lines(),
        &mut highlighter,
        &|_| Format::default(),
        style,
        DocumentUpdate::text(DocumentChange::Discover),
        usize::MAX,
    );

    // A size change moves glyphs and therefore line tops: deferring it would
    // let the document shift under a reader scrolling back. Every mismatch
    // rebuilds, viewport or not, and nothing is marked stale.
    let resized = document.update(
        doc.lines(),
        &mut highlighter,
        &|_: &()| Format {
            size: Some(Pixels(20.0)),
            ..Format::default()
        },
        style,
        DocumentUpdate {
            change: DocumentChange::Unchanged,
            geometry_changed: false,
            format_changed: true,
            viewport_start: 40,
            stale_before: 0,
        },
        50,
    );
    assert_eq!(resized.rebuilt_lines, 50);
    assert_eq!(resized.format_stale_before, 0);
}

/// `end_padding` lets the last line scroll up from the bottom edge without
/// shortening the text area the way bottom `padding` would.
#[test]
fn end_padding_extends_the_scroll_range_but_not_the_clip() {
    use iced::advanced::renderer::Headless;

    let content = Content::with_text(&"line\n".repeat(40));
    let renderer = iced_test::futures::futures::executor::block_on(
        <iced::Renderer as Headless>::new(Font::DEFAULT, Pixels(16.0), Some("tiny-skia")),
    )
    .expect("headless renderer");
    let limits = layout::Limits::new(Size::ZERO, Size::new(400.0, 300.0));

    let scroll_range = |end_padding: f32| {
        let mut editor = RichTextEditor::new(&content, ContentVersion::new(1, 0))
            .width(Length::Fixed(400.0))
            .height(Length::Fixed(300.0))
            .padding(0.0)
            .end_padding(end_padding);
        let mut tree = widget::Tree::new(&editor as &dyn Widget<Action, Theme, iced::Renderer>);
        let node = editor.layout(&mut tree, &renderer, &limits);
        let state = tree
            .state
            .downcast_ref::<State<text::highlighter::PlainText>>();
        (
            state.max_scroll(),
            state.viewport_height,
            node.size().height,
        )
    };

    let (plain_max, plain_viewport, plain_height) = scroll_range(0.0);
    let (padded_max, padded_viewport, padded_height) = scroll_range(120.0);
    assert!(plain_max > 0.0, "forty lines overflow a 300px editor");
    assert_eq!(
        padded_max,
        plain_max + 120.0,
        "the last line can scroll 120px above the bottom edge"
    );
    assert_eq!(
        padded_viewport, plain_viewport,
        "the visible text area is unchanged"
    );
    assert_eq!(padded_height, plain_height, "the widget keeps its size");
}

/// iced only hands cosmic-text a line-height metric for spans that set a
/// size or line height, and cosmic-text sizes a visual line by the maximum
/// metric it was handed. A wrapped line whose only sized span was a hidden
/// 0.01 px marker therefore collapsed to ~0 px and the next line was drawn
/// on top of it. Every span now carries the paragraph defaults.
#[test]
fn a_wrapped_line_with_only_tiny_sized_spans_keeps_the_body_line_height() {
    let text = "one two three four five six seven [eight](https://x) nine ten";
    let marker = Format {
        size: Some(Pixels(0.01)),
        ..Format::default()
    };
    let open = text.find('[').expect("opening marker");
    let close = text.find("](").expect("closing marker");
    let highlights = vec![
        (0..text.len(), Format::default()),
        (open..open + 1, marker),
        (close..text.len() - " nine ten".len(), marker),
    ];
    let line = DocumentLine::new(
        StyledLine {
            text: text.to_owned(),
            segments: compose_segments(text, &highlights),
            empty_format: Format::default(),
            line_highlight: None,
            line_padding: Padding::ZERO,
            line_rule: None,
        },
        test_layout_style(180.0),
    );
    let heights = line
        .paragraph
        .buffer()
        .layout_runs()
        .map(|run| run.line_height)
        .collect::<Vec<_>>();
    assert!(heights.len() >= 2, "the line wraps: {heights:?}");
    for height in &heights {
        assert!(
            (height - 16.0 * 1.6).abs() < 0.01,
            "every visual line keeps the 25.6px body line height: {heights:?}"
        );
    }
}
