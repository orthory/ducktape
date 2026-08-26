use super::*;

#[test]
fn ime_stages_rebuild_only_the_changed_line_in_a_long_document() {
    let mut lines = (0..1_000)
        .map(|index| format!("stable line {index}"))
        .collect::<Vec<_>>();
    let mut highlighter = WholeLine::default();
    let mut document = DocumentLayout::default();
    let style = test_layout_style(700.0);

    assert_eq!(
        document
            .update(
                TestDoc::new(&lines).lines(),
                &mut highlighter,
                &|_| Format::default(),
                style,
                DocumentUpdate::text(DocumentChange::Discover),
                usize::MAX,
            )
            .rebuilt_lines,
        lines.len()
    );

    for stage in ["ㅇ", "으", "응"] {
        lines[500] = format!("stable line 500 {stage}");
        assert_eq!(
            document
                .update(
                    TestDoc::new(&lines).lines(),
                    &mut highlighter,
                    &|_| Format::default(),
                    style,
                    DocumentUpdate::text(DocumentChange::Discover),
                    usize::MAX,
                )
                .rebuilt_lines,
            1,
            "{stage:?} must not reshape unchanged paragraphs"
        );
    }
}

#[test]
fn line_insertions_reuse_the_unchanged_suffix() {
    let mut lines = vec!["first".to_owned(), "second".to_owned(), "third".to_owned()];
    let mut highlighter = WholeLine::default();
    let mut document = DocumentLayout::default();
    let style = test_layout_style(700.0);

    assert_eq!(
        document
            .update(
                TestDoc::new(&lines).lines(),
                &mut highlighter,
                &|_| Format::default(),
                style,
                DocumentUpdate::text(DocumentChange::Discover),
                usize::MAX,
            )
            .rebuilt_lines,
        3
    );

    lines.insert(1, "inserted".to_owned());
    assert_eq!(
        document
            .update(
                TestDoc::new(&lines).lines(),
                &mut highlighter,
                &|_| Format::default(),
                style,
                DocumentUpdate::text(DocumentChange::Discover),
                usize::MAX,
            )
            .rebuilt_lines,
        1
    );

    lines.remove(1);
    assert_eq!(
        document
            .update(
                TestDoc::new(&lines).lines(),
                &mut highlighter,
                &|_| Format::default(),
                style,
                DocumentUpdate::text(DocumentChange::Discover),
                usize::MAX,
            )
            .rebuilt_lines,
        0
    );
}

#[test]
fn change_hint_maps_replacements_insertions_undo_and_redo_without_line_diffing() {
    let style = test_layout_style(700.0);
    let mut highlighter = WholeLine::default();
    let mut document = DocumentLayout::default();
    let original = vec!["first".to_owned(), "second".to_owned(), "third".to_owned()];
    document.update(
        TestDoc::new(&original).lines(),
        &mut highlighter,
        &|_| Format::default(),
        style,
        DocumentUpdate::text(DocumentChange::Discover),
        usize::MAX,
    );
    let original_ids = document
        .lines
        .iter()
        .map(|line| line.identity)
        .collect::<Vec<_>>();

    let replaced = vec!["first".to_owned(), "SECOND".to_owned(), "third".to_owned()];
    let update = document.update(
        TestDoc::new(&replaced).lines(),
        &mut highlighter,
        &|_| Format::default(),
        style,
        DocumentUpdate::text(DocumentChange::Hint(test_change(1, 1, 1))),
        usize::MAX,
    );
    assert_eq!(update.mapping_line_comparisons, 0);
    assert_eq!(update.rebuilt_lines, 1);
    assert_eq!(update.shaped_paragraphs, 1);
    assert_eq!(update.highlighted_lines, 2);
    assert!(update.change_hint_used);
    assert_eq!(document.lines[0].identity, original_ids[0]);
    assert_eq!(document.lines[2].identity, original_ids[2]);
    let replaced_ids = document
        .lines
        .iter()
        .map(|line| line.identity)
        .collect::<Vec<_>>();

    let inserted = vec![
        "first".to_owned(),
        "inserted".to_owned(),
        "SECOND".to_owned(),
        "third".to_owned(),
    ];
    let update = document.update(
        TestDoc::new(&inserted).lines(),
        &mut highlighter,
        &|_| Format::default(),
        style,
        DocumentUpdate::text(DocumentChange::Hint(test_change(1, 0, 1))),
        usize::MAX,
    );
    assert_eq!(update.mapping_line_comparisons, 0);
    assert_eq!(update.rebuilt_lines, 1);
    assert!(update.change_hint_used);
    assert_eq!(document.lines[2].identity, replaced_ids[1]);
    assert_eq!(document.lines[3].identity, replaced_ids[2]);

    let update = document.update(
        TestDoc::new(&replaced).lines(),
        &mut highlighter,
        &|_| Format::default(),
        style,
        DocumentUpdate::text(DocumentChange::Hint(test_change(1, 1, 0))),
        usize::MAX,
    );
    assert_eq!(update.mapping_line_comparisons, 0);
    assert_eq!(update.rebuilt_lines, 0);
    assert_eq!(document.lines[1].identity, replaced_ids[1]);
    assert_eq!(document.lines[2].identity, replaced_ids[2]);

    let update = document.update(
        TestDoc::new(&inserted).lines(),
        &mut highlighter,
        &|_| Format::default(),
        style,
        DocumentUpdate::text(DocumentChange::Hint(test_change(1, 0, 1))),
        usize::MAX,
    );
    assert_eq!(update.mapping_line_comparisons, 0);
    assert_eq!(update.rebuilt_lines, 1);
    assert_eq!(document.lines[2].identity, replaced_ids[1]);
    assert_eq!(document.lines[3].identity, replaced_ids[2]);
}

#[test]
fn invalid_change_hints_fall_back_to_exact_diffing() {
    let style = test_layout_style(700.0);
    for invalid in [
        test_change(4, 0, 0),
        test_change(1, 0, 1),
        test_change(usize::MAX, 1, 1),
    ] {
        let mut highlighter = WholeLine::default();
        let mut document = DocumentLayout::default();
        document.update(
            TestDoc::new(&["first".to_owned(), "second".to_owned(), "third".to_owned()]).lines(),
            &mut highlighter,
            &|_| Format::default(),
            style,
            DocumentUpdate::text(DocumentChange::Discover),
            usize::MAX,
        );
        let changed = ["first".to_owned(), "SECOND".to_owned(), "third".to_owned()];
        let update = document.update(
            TestDoc::new(&changed).lines(),
            &mut highlighter,
            &|_| Format::default(),
            style,
            DocumentUpdate::text(DocumentChange::Hint(invalid)),
            usize::MAX,
        );

        assert!(update.change_hint_rejected, "{invalid:?}");
        assert!(!update.change_hint_used, "{invalid:?}");
        assert!(update.mapping_line_comparisons > 0, "{invalid:?}");
        assert_eq!(update.rebuilt_lines, 1, "{invalid:?}");
        assert_eq!(document.lines[1].signature.text, "SECOND");
    }
}

#[test]
fn insertion_hint_keeps_an_identical_shifted_suffix_line_identity() {
    let style = test_layout_style(700.0);
    let mut highlighter = WholeLine::default();
    let mut document = DocumentLayout::default();
    let original = [
        "first".to_owned(),
        "duplicate".to_owned(),
        "last".to_owned(),
    ];
    document.update(
        TestDoc::new(&original).lines(),
        &mut highlighter,
        &|_| Format::default(),
        style,
        DocumentUpdate::text(DocumentChange::Discover),
        usize::MAX,
    );
    let shifted_identity = document.lines[1].identity;

    let inserted = [
        "first".to_owned(),
        "duplicate".to_owned(),
        "duplicate".to_owned(),
        "last".to_owned(),
    ];
    let update = document.update(
        TestDoc::new(&inserted).lines(),
        &mut highlighter,
        &|_| Format::default(),
        style,
        DocumentUpdate::text(DocumentChange::Hint(test_change(1, 0, 1))),
        usize::MAX,
    );

    assert_eq!(update.rebuilt_lines, 1);
    assert_ne!(document.lines[1].identity, shifted_identity);
    assert_eq!(document.lines[2].identity, shifted_identity);
}

#[test]
fn change_hint_restarts_stateful_highlighting_at_the_changed_line() {
    let style = test_layout_style(700.0);
    let mut highlighter = <ToggleHighlighter as text::Highlighter>::new(&());
    let mut document = DocumentLayout::default();
    document.update(
        TestDoc::new(&["before".to_owned(), "middle".to_owned(), "after".to_owned()]).lines(),
        &mut highlighter,
        &|inside| Format {
            color: inside.then_some(Color::BLACK),
            ..Format::default()
        },
        style,
        DocumentUpdate::text(DocumentChange::Discover),
        usize::MAX,
    );

    let changed = ["before".to_owned(), "toggle".to_owned(), "after".to_owned()];
    let update = document.update(
        TestDoc::new(&changed).lines(),
        &mut highlighter,
        &|inside| Format {
            color: inside.then_some(Color::BLACK),
            ..Format::default()
        },
        style,
        DocumentUpdate::text(DocumentChange::Hint(test_change(1, 1, 1))),
        usize::MAX,
    );

    assert_eq!(update.mapping_line_comparisons, 0);
    assert_eq!(update.highlighted_lines, 2);
    assert_eq!(update.rebuilt_lines, 2);
    assert_eq!(
        document.lines[2].signature.segments[0].format.color,
        Some(Color::BLACK)
    );
}

#[test]
fn widget_change_hint_separates_materialization_diff_and_shaping_metrics() {
    let renderer = headless_renderer();
    let limits = layout::Limits::new(Size::ZERO, Size::new(400.0, 120.0));
    let content = Content::with_text("first\nsecond\nthird");
    let mut editor = RichTextEditor::<_, ()>::new(&content, ContentVersion::new(9, 0))
        .width(Length::Fixed(400.0))
        .height(Length::Fixed(120.0));
    let mut tree = widget::Tree::new(&editor as &dyn Widget<_, Theme, iced::Renderer>);
    editor.layout(&mut tree, &renderer, &limits);
    drop(editor);
    tree.state
        .downcast_mut::<State<text::highlighter::PlainText>>()
        .metrics = LayoutMetrics::default();

    let content = Content::with_text("first\nseXcond\nthird");
    let mut editor = RichTextEditor::<_, ()>::new(&content, ContentVersion::new(9, 1))
        .change_hint(EditorChange::new(
            ContentVersion::new(9, 0),
            ContentVersion::new(9, 1),
            1,
            1,
            1,
        ))
        .width(Length::Fixed(400.0))
        .height(Length::Fixed(120.0));
    editor.layout(&mut tree, &renderer, &limits);

    let state = tree
        .state
        .downcast_ref::<State<text::highlighter::PlainText>>();
    assert_eq!(state.metrics.full_text_materializations, 1);
    assert_eq!(
        state.metrics.materialized_source_bytes,
        "first\nseXcond\nthird".len()
    );
    assert_eq!(state.metrics.mapping_line_comparisons, 0);
    assert_eq!(state.metrics.styled_signature_comparisons, 2);
    assert_eq!(state.metrics.newly_owned_styled_texts, 1);
    assert_eq!(state.metrics.newly_owned_styled_text_bytes, "seXcond".len());
    // Editing a line without adding or removing one touches the lines where
    // they sit; no slot vector is prepared at all.
    assert_eq!(state.metrics.line_vector_slots_prepared, 0);
    assert_eq!(state.metrics.rebuilt_lines, 1);
    assert_eq!(state.metrics.shaped_paragraphs, 1);
    assert_eq!(state.metrics.highlighted_lines, 2);
    assert_eq!(state.metrics.accepted_change_hints, 1);
    assert_eq!(state.metrics.rejected_change_hints, 0);
    assert_eq!(state.document.lines[1].signature.text, "seXcond");
}

#[test]
fn stale_batched_and_cross_document_hints_fall_back_before_reusing_a_prefix() {
    let renderer = headless_renderer();
    let limits = layout::Limits::new(Size::ZERO, Size::new(400.0, 120.0));
    let initial = Content::with_text("old first\nold second\nthird");
    let initial_version = ContentVersion::new(40, 0);
    let mut editor = RichTextEditor::<_, ()>::new(&initial, initial_version)
        .width(Length::Fixed(400.0))
        .height(Length::Fixed(120.0));
    let mut tree = widget::Tree::new(&editor as &dyn Widget<_, Theme, iced::Renderer>);
    editor.layout(&mut tree, &renderer, &limits);
    drop(editor);
    tree.state
        .downcast_mut::<State<text::highlighter::PlainText>>()
        .metrics = LayoutMetrics::default();

    // Revision 1 changed line 0, but no layout happened before revision 2
    // changed line 1. The latest edit alone must not be applied to the layout
    // produced by revision 0.
    let batched = Content::with_text("new first\nnew second\nthird");
    let revision_1 = ContentVersion::new(40, 1);
    let revision_2 = ContentVersion::new(40, 2);
    let stale_latest_edit = EditorChange::new(revision_1, revision_2, 1, 1, 1);
    let mut editor = RichTextEditor::<_, ()>::new(&batched, revision_2)
        .change_hint(stale_latest_edit)
        .width(Length::Fixed(400.0))
        .height(Length::Fixed(120.0));
    editor.layout(&mut tree, &renderer, &limits);

    let state = tree
        .state
        .downcast_ref::<State<text::highlighter::PlainText>>();
    assert_eq!(state.document.lines[0].signature.text, "new first");
    assert_eq!(state.document.lines[1].signature.text, "new second");
    assert!(state.metrics.mapping_line_comparisons > 0);
    assert_eq!(state.metrics.accepted_change_hints, 0);
    assert_eq!(state.metrics.rejected_change_hints, 1);
    drop(editor);

    // Reusing the same builder hint at an unchanged version is inert.
    tree.state
        .downcast_mut::<State<text::highlighter::PlainText>>()
        .metrics = LayoutMetrics::default();
    let mut editor = RichTextEditor::<_, ()>::new(&batched, revision_2)
        .change_hint(stale_latest_edit)
        .width(Length::Fixed(400.0))
        .height(Length::Fixed(120.0));
    editor.layout(&mut tree, &renderer, &limits);
    assert_eq!(
        tree.state
            .downcast_ref::<State<text::highlighter::PlainText>>()
            .metrics,
        LayoutMetrics::default()
    );
    drop(editor);

    // Even an exact pair cannot authorize mapping across document identity.
    let replacement = Content::with_text("replacement");
    let replacement_version = ContentVersion::new(41, 0);
    let mut editor = RichTextEditor::<_, ()>::new(&replacement, replacement_version)
        .change_hint(EditorChange::new(revision_2, replacement_version, 0, 3, 1))
        .width(Length::Fixed(400.0))
        .height(Length::Fixed(120.0));
    editor.layout(&mut tree, &renderer, &limits);
    let state = tree
        .state
        .downcast_ref::<State<text::highlighter::PlainText>>();
    assert_eq!(state.document.lines[0].signature.text, "replacement");
    assert_eq!(state.metrics.accepted_change_hints, 0);
    assert_eq!(state.metrics.rejected_change_hints, 1);
}

#[test]
fn caret_selection_and_viewport_resize_do_not_rediscover_line_changes() {
    let renderer = headless_renderer();
    let content = Content::with_text("first\nsecond\nthird");
    let version = ContentVersion::new(10, 0);
    let limits = layout::Limits::new(Size::ZERO, Size::new(400.0, 120.0));
    let mut editor = RichTextEditor::<_, ()>::new(&content, version)
        .width(Length::Fixed(400.0))
        .height(Length::Fixed(120.0));
    let mut tree = widget::Tree::new(&editor as &dyn Widget<_, Theme, iced::Renderer>);
    editor.layout(&mut tree, &renderer, &limits);
    drop(editor);
    tree.state
        .downcast_mut::<State<text::highlighter::PlainText>>()
        .metrics = LayoutMetrics::default();

    let mut content = content;
    content.move_to(Cursor {
        position: Position { line: 2, column: 3 },
        selection: Some(Position { line: 1, column: 1 }),
    });
    let mut editor = RichTextEditor::<_, ()>::new(&content, version)
        .width(Length::Fixed(400.0))
        .height(Length::Fixed(120.0));
    editor.layout(&mut tree, &renderer, &limits);
    assert_eq!(
        tree.state
            .downcast_ref::<State<text::highlighter::PlainText>>()
            .metrics,
        LayoutMetrics::default()
    );
    drop(editor);

    let resized_limits = layout::Limits::new(Size::ZERO, Size::new(320.0, 120.0));
    let mut editor = RichTextEditor::<_, ()>::new(&content, version)
        .width(Length::Fixed(320.0))
        .height(Length::Fixed(120.0));
    editor.layout(&mut tree, &renderer, &resized_limits);
    let state = tree
        .state
        .downcast_ref::<State<text::highlighter::PlainText>>();
    assert_eq!(state.metrics.full_text_materializations, 0);
    assert_eq!(state.metrics.mapping_line_comparisons, 0);
    assert_eq!(state.metrics.rebuilt_lines, 3);
    assert_eq!(state.metrics.shaped_paragraphs, 3);
}

#[test]
fn content_version_distinguishes_document_replacement_from_text_revision() {
    let renderer = headless_renderer();
    let limits = layout::Limits::new(Size::ZERO, Size::new(400.0, 120.0));
    let mut content = Content::with_text("first document");
    let mut editor = RichTextEditor::<_, ()>::new(&content, ContentVersion::new(7, 0))
        .width(Length::Fixed(400.0))
        .height(Length::Fixed(120.0));
    let mut tree = widget::Tree::new(&editor as &dyn Widget<_, Theme, iced::Renderer>);

    editor.layout(&mut tree, &renderer, &limits);
    drop(editor);
    tree.state
        .downcast_mut::<State<text::highlighter::PlainText>>()
        .metrics = LayoutMetrics::default();

    content = Content::with_text("second document");
    let mut editor = RichTextEditor::<_, ()>::new(&content, ContentVersion::new(8, 0))
        .width(Length::Fixed(400.0))
        .height(Length::Fixed(120.0));
    editor.layout(&mut tree, &renderer, &limits);

    let state = tree
        .state
        .downcast_ref::<State<text::highlighter::PlainText>>();
    assert_eq!(state.source, "second document");
    assert_eq!(state.metrics.full_text_materializations, 1);
    assert_eq!(state.metrics.rebuilt_lines, 1);
    assert_eq!(state.content_version, Some(ContentVersion::new(8, 0)));
}

#[test]
#[ignore = "large-document performance contract run explicitly in CI"]
fn performance_contract_100k_caret_and_one_char_insertion() {
    let source = large_source();
    let mut content = Content::with_text(&source);
    let renderer = headless_renderer();
    let limits = layout::Limits::new(Size::ZERO, Size::new(800.0, 600.0));
    let version = ContentVersion::new(11, 0);
    let mut editor = RichTextEditor::<_, ()>::new(&content, version)
        .width(Length::Fixed(800.0))
        .height(Length::Fixed(600.0))
        .wrapping(text::Wrapping::None);
    let mut tree = widget::Tree::new(&editor as &dyn Widget<_, Theme, iced::Renderer>);

    editor.layout(&mut tree, &renderer, &limits);
    drop(editor);
    tree.state
        .downcast_mut::<State<text::highlighter::PlainText>>()
        .metrics = LayoutMetrics::default();
    let mut budget_failures = Vec::new();

    let caret_started = Instant::now();
    for event in 0..1_000 {
        let line = event * 97 % 100_000;
        content.move_to(Cursor {
            position: Position {
                line,
                column: event % 8,
            },
            selection: None,
        });
        let mut editor = RichTextEditor::<_, ()>::new(&content, version)
            .width(Length::Fixed(800.0))
            .height(Length::Fixed(600.0))
            .wrapping(text::Wrapping::None);
        editor.layout(&mut tree, &renderer, &limits);
    }
    let caret_elapsed = caret_started.elapsed();

    let state = tree
        .state
        .downcast_ref::<State<text::highlighter::PlainText>>();
    assert_eq!(state.document.lines.len(), 100_001);
    assert_eq!(state.metrics, LayoutMetrics::default());
    budget_failures.extend(record_performance_metrics(
        "caret_1000",
        1_000,
        caret_elapsed,
        Duration::from_secs(5),
        &state.metrics,
    ));

    tree.state
        .downcast_mut::<State<text::highlighter::PlainText>>()
        .metrics = LayoutMetrics::default();
    content.perform(text_editor::Action::Move(Motion::Right));
    content.move_to(Cursor {
        position: Position {
            line: 50_000,
            column: 4,
        },
        selection: None,
    });
    assert_eq!(
        content.line(50_000).map(|line| line.text.into_owned()),
        Some("line 50000".to_owned())
    );
    assert_eq!(
        content.cursor(),
        Cursor {
            position: Position {
                line: 50_000,
                column: 4,
            },
            selection: None,
        }
    );
    let started = Instant::now();
    content.perform(text_editor::Action::Edit(Edit::Insert('x')));
    let mut editor = RichTextEditor::<_, ()>::new(&content, ContentVersion::new(11, 1))
        .change_hint(EditorChange::new(
            ContentVersion::new(11, 0),
            ContentVersion::new(11, 1),
            50_000,
            1,
            1,
        ))
        .width(Length::Fixed(800.0))
        .height(Length::Fixed(600.0))
        .wrapping(text::Wrapping::None);
    editor.layout(&mut tree, &renderer, &limits);
    let elapsed = started.elapsed();
    assert_eq!(
        content.line(50_000).map(|line| line.text.into_owned()),
        Some("linex 50000".to_owned())
    );

    let state = tree
        .state
        .downcast_ref::<State<text::highlighter::PlainText>>();
    eprintln!(
        "100k caret={caret_elapsed:?}, insertion={elapsed:?}, metrics={:?}",
        state.metrics
    );
    assert_eq!(state.metrics.full_text_materializations, 1);
    assert_eq!(state.metrics.materialized_source_bytes, source.len() + 1);
    assert_eq!(state.metrics.mapping_line_comparisons, 0);
    // The caret loop parked the viewport ~47k lines below the edit, but the
    // reveal in this same layout call snaps it back to the caret before
    // anything is drawn — so the pass covers the viewport the reveal
    // produces plus overscan, never the stale scroll's region: 29 visible
    // lines and 32 of overscan. The lines in between stay beyond the
    // validated mark, and the frame that scrolls back down re-opens a pass
    // for them, exactly as any scroll past the mark does.
    assert_eq!(state.metrics.styled_signature_comparisons, 61);
    assert_eq!(state.metrics.newly_owned_styled_texts, 1);
    assert_eq!(
        state.metrics.newly_owned_styled_text_bytes,
        "linex 50000".len()
    );
    // No slot vector is prepared: the line count is unchanged, so the pass
    // edits the lines where they sit instead of rebuilding the vector.
    assert_eq!(state.metrics.line_vector_slots_prepared, 0);
    assert_eq!(state.metrics.rebuilt_lines, 1);
    assert_eq!(state.metrics.shaped_paragraphs, 1);
    assert_eq!(state.metrics.highlighted_lines, 61);
    assert_eq!(state.metrics.accepted_change_hints, 1);
    assert_eq!(state.document.lines[50_000].signature.text, "linex 50000");
    budget_failures.extend(record_performance_metrics(
        "one_char_insertion",
        1,
        elapsed,
        Duration::from_millis(500),
        &state.metrics,
    ));
    assert_performance_budgets(&budget_failures);
}

#[test]
#[ignore = "large-document performance contract run explicitly in CI"]
fn performance_contract_100k_selection_drag_pointer_events() {
    use iced::advanced::clipboard;

    let source = large_source();
    let mut content = Content::with_text(&source);
    let renderer = headless_renderer();
    let limits = layout::Limits::new(Size::ZERO, Size::new(800.0, 600.0));
    let viewport = Rectangle::with_size(Size::new(800.0, 600.0));
    let version = ContentVersion::new(12, 0);
    let mut editor = RichTextEditor::new(&content, version)
        .width(Length::Fixed(800.0))
        .height(Length::Fixed(600.0))
        .wrapping(text::Wrapping::None)
        .on_action(|action| action);
    let mut tree = widget::Tree::new(&editor as &dyn Widget<_, Theme, iced::Renderer>);
    let mut node = editor.layout(&mut tree, &renderer, &limits);
    let anchor = {
        let caret = tree
            .state
            .downcast_ref::<State<text::highlighter::PlainText>>()
            .document
            .caret(Position { line: 0, column: 0 });
        Point::new(5.0 + caret.x, 5.0 + caret.y + caret.height / 2.0)
    };
    let mut clipboard = clipboard::Null;
    let mut messages = Vec::new();
    {
        let mut shell = Shell::new(&mut messages);
        editor.update(
            &mut tree,
            &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Layout::new(&node),
            mouse::Cursor::Available(anchor),
            &renderer,
            &mut clipboard,
            &mut shell,
            &viewport,
        );
    }
    let [Action::MoveTo(cursor)] = messages.as_slice() else {
        panic!("drag press must publish one caret: {messages:?}");
    };
    let cursor = *cursor;
    messages.clear();
    drop(editor);
    content.move_to(cursor);
    tree.state
        .downcast_mut::<State<text::highlighter::PlainText>>()
        .metrics = LayoutMetrics::default();

    let started = Instant::now();
    for event in 0..1_000 {
        let point = Point::new(
            20.0 + (event % 40) as f32 * 9.0,
            10.0 + (event % 20) as f32 * 25.0,
        );
        let mut editor = RichTextEditor::new(&content, version)
            .width(Length::Fixed(800.0))
            .height(Length::Fixed(600.0))
            .wrapping(text::Wrapping::None)
            .on_action(|action| action);
        node = editor.layout(&mut tree, &renderer, &limits);
        {
            let mut shell = Shell::new(&mut messages);
            editor.update(
                &mut tree,
                &Event::Mouse(mouse::Event::CursorMoved { position: point }),
                Layout::new(&node),
                mouse::Cursor::Available(point),
                &renderer,
                &mut clipboard,
                &mut shell,
                &viewport,
            );
        }
        let [Action::MoveTo(cursor)] = messages.as_slice() else {
            panic!("drag event {event} must publish one selection: {messages:?}");
        };
        let cursor = *cursor;
        assert!(cursor.selection.is_some());
        messages.clear();
        drop(editor);
        content.move_to(cursor);
    }
    let elapsed = started.elapsed();

    let mut editor = RichTextEditor::new(&content, version)
        .width(Length::Fixed(800.0))
        .height(Length::Fixed(600.0))
        .wrapping(text::Wrapping::None)
        .on_action(|action| action);
    node = editor.layout(&mut tree, &renderer, &limits);
    let mut shell = Shell::new(&mut messages);
    editor.update(
        &mut tree,
        &Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        Layout::new(&node),
        mouse::Cursor::Available(anchor),
        &renderer,
        &mut clipboard,
        &mut shell,
        &viewport,
    );
    assert!(shell.is_event_captured());
    assert!(messages.is_empty());

    let state = tree
        .state
        .downcast_ref::<State<text::highlighter::PlainText>>();
    assert_eq!(state.document.lines.len(), 100_001);
    assert_eq!(state.metrics, LayoutMetrics::default());
    let budget_failures = record_performance_metrics(
        "selection_drag_1000",
        1_000,
        elapsed,
        Duration::from_secs(10),
        &state.metrics,
    )
    .into_iter()
    .collect::<Vec<_>>();
    eprintln!(
        "100k selection drag={elapsed:?}, metrics={:?}",
        state.metrics
    );
    assert_performance_budgets(&budget_failures);
}

#[test]
#[ignore = "large-document performance contract run explicitly in CI"]
fn performance_contract_100k_hangul_ime_sequence() {
    use iced::advanced::clipboard;

    let source = large_source();
    let mut content = Content::with_text(&source);
    content.move_to(Cursor {
        position: Position {
            line: 50_000,
            column: 4,
        },
        selection: None,
    });
    let renderer = headless_renderer();
    let limits = layout::Limits::new(Size::ZERO, Size::new(800.0, 600.0));
    let viewport = Rectangle::with_size(Size::new(800.0, 600.0));
    let version = ContentVersion::new(13, 0);
    let mut editor = RichTextEditor::new(&content, version)
        .width(Length::Fixed(800.0))
        .height(Length::Fixed(600.0))
        .wrapping(text::Wrapping::None)
        .on_action(|action| action);
    let mut tree = widget::Tree::new(&editor as &dyn Widget<_, Theme, iced::Renderer>);
    tree.state
        .downcast_mut::<State<text::highlighter::PlainText>>()
        .focus = Some(Focus::now());
    let mut node = editor.layout(&mut tree, &renderer, &limits);
    tree.state
        .downcast_mut::<State<text::highlighter::PlainText>>()
        .metrics = LayoutMetrics::default();
    let mut clipboard = clipboard::Null;

    let started = Instant::now();
    for stage in ["ㅇ", "으", "응"] {
        let mut messages = Vec::new();
        let mut shell = Shell::new(&mut messages);
        editor.update(
            &mut tree,
            &Event::InputMethod(input_method::Event::Preedit(stage.into(), Some(3..3))),
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &renderer,
            &mut clipboard,
            &mut shell,
            &viewport,
        );
        assert!(shell.is_layout_invalid());
        shell.revalidate_layout(|| {
            node = editor.layout(&mut tree, &renderer, &limits);
        });
        assert!(messages.is_empty());
    }
    let elapsed = started.elapsed();

    let state = tree
        .state
        .downcast_ref::<State<text::highlighter::PlainText>>();
    eprintln!("100k IME={elapsed:?}, metrics={:?}", state.metrics);
    assert_eq!(state.metrics.full_text_materializations, 0);
    assert_eq!(state.metrics.composition_display_strings, 3);
    assert!(state.metrics.mapping_line_comparisons > 0);
    assert_eq!(state.metrics.styled_signature_comparisons, 99);
    assert_eq!(state.metrics.newly_owned_styled_texts, 3);
    assert_eq!(state.metrics.line_vector_slots_prepared, 0);
    assert_eq!(state.metrics.rebuilt_lines, 3);
    assert_eq!(state.metrics.shaped_paragraphs, 3);
    assert_eq!(state.metrics.highlighted_lines, 99);
    assert_eq!(state.metrics.accepted_change_hints, 0);
    let budget_failures = record_performance_metrics(
        "hangul_ime_sequence",
        3,
        elapsed,
        Duration::from_secs(1),
        &state.metrics,
    )
    .into_iter()
    .collect::<Vec<_>>();
    assert_performance_budgets(&budget_failures);
}

#[test]
#[ignore = "large-document performance contract run explicitly in CI"]
fn performance_contract_100k_format_key_only_layout() {
    let source = large_source();
    let content = Content::with_text(&source);
    let renderer = headless_renderer();
    let limits = layout::Limits::new(Size::ZERO, Size::new(800.0, 600.0));
    let version = ContentVersion::new(15, 0);
    let mut editor = RichTextEditor::<_, ()>::new(&content, version)
        .width(Length::Fixed(800.0))
        .height(Length::Fixed(600.0))
        .wrapping(text::Wrapping::None)
        .highlight_with::<WholeLine>((), 0, |_| Format::default());
    let mut tree = widget::Tree::new(&editor as &dyn Widget<_, Theme, iced::Renderer>);
    editor.layout(&mut tree, &renderer, &limits);
    drop(editor);
    tree.state.downcast_mut::<State<WholeLine>>().metrics = LayoutMetrics::default();

    let started = Instant::now();
    let mut editor = RichTextEditor::<_, ()>::new(&content, version)
        .width(Length::Fixed(800.0))
        .height(Length::Fixed(600.0))
        .wrapping(text::Wrapping::None)
        .highlight_with::<WholeLine>((), 1, |_| Format {
            color: Some(Color::BLACK),
            ..Format::default()
        });
    editor.layout(&mut tree, &renderer, &limits);
    let elapsed = started.elapsed();

    let state = tree.state.downcast_ref::<State<WholeLine>>();
    eprintln!("100k format key={elapsed:?}, metrics={:?}", state.metrics);
    assert_eq!(state.metrics.full_text_materializations, 0);
    assert_eq!(state.metrics.materialized_source_bytes, 0);
    assert_eq!(state.metrics.mapping_line_comparisons, 0);
    // A format key changes how every line looks, but only the lines on
    // screen have to be re-highlighted and re-shaped now; the rest keep the
    // formatting of the previous pass until they scroll into view. This used
    // to touch all 100_001 lines for ~10.7s.
    assert_eq!(state.metrics.styled_signature_comparisons, 61);
    assert_eq!(state.metrics.newly_owned_styled_texts, 0);
    assert_eq!(state.metrics.newly_owned_styled_text_bytes, 0);
    assert_eq!(state.metrics.line_vector_slots_prepared, 0);
    assert_eq!(state.metrics.rebuilt_lines, 61);
    assert_eq!(state.metrics.shaped_paragraphs, 61);
    assert_eq!(state.metrics.highlighted_lines, 61);
    let budget_failures = record_performance_metrics(
        "format_key_only",
        1,
        elapsed,
        Duration::from_millis(200),
        &state.metrics,
    )
    .into_iter()
    .collect::<Vec<_>>();
    assert_performance_budgets(&budget_failures);
}

#[test]
#[ignore = "large-document performance contract run explicitly in CI"]
fn performance_contract_100k_viewport_resize() {
    let source = large_source();
    let content = Content::with_text(&source);
    let renderer = headless_renderer();
    let initial_limits = layout::Limits::new(Size::ZERO, Size::new(800.0, 600.0));
    let version = ContentVersion::new(14, 0);
    let mut editor = RichTextEditor::<_, ()>::new(&content, version)
        .width(Length::Fixed(800.0))
        .height(Length::Fixed(600.0))
        .wrapping(text::Wrapping::None);
    let mut tree = widget::Tree::new(&editor as &dyn Widget<_, Theme, iced::Renderer>);
    editor.layout(&mut tree, &renderer, &initial_limits);
    drop(editor);
    tree.state
        .downcast_mut::<State<text::highlighter::PlainText>>()
        .metrics = LayoutMetrics::default();

    let resized_limits = layout::Limits::new(Size::ZERO, Size::new(640.0, 600.0));
    let started = Instant::now();
    let mut editor = RichTextEditor::<_, ()>::new(&content, version)
        .width(Length::Fixed(640.0))
        .height(Length::Fixed(600.0))
        .wrapping(text::Wrapping::None);
    editor.layout(&mut tree, &renderer, &resized_limits);
    let elapsed = started.elapsed();

    let state = tree
        .state
        .downcast_ref::<State<text::highlighter::PlainText>>();
    eprintln!("100k resize={elapsed:?}, metrics={:?}", state.metrics);
    assert_eq!(state.metrics.full_text_materializations, 0);
    assert_eq!(state.metrics.mapping_line_comparisons, 0);
    assert_eq!(state.metrics.styled_signature_comparisons, 0);
    assert_eq!(state.metrics.newly_owned_styled_texts, 0);
    // This editor has wrapping off, so narrowing it cannot reflow a single
    // line: nothing needs re-shaping, and no shaping pass should open at all.
    // This used to re-shape all 100_001 paragraphs — ~8.7s of work with no
    // observable difference — and the contract asserted that count, pinning
    // the waste in place.
    assert_eq!(state.metrics.line_vector_slots_prepared, 0);
    assert_eq!(state.metrics.rebuilt_lines, 0);
    assert_eq!(state.metrics.shaped_paragraphs, 0);
    assert_eq!(state.metrics.highlighted_lines, 0);
    let budget_failures = record_performance_metrics(
        "viewport_resize",
        1,
        elapsed,
        Duration::from_millis(100),
        &state.metrics,
    )
    .into_iter()
    .collect::<Vec<_>>();
    assert_performance_budgets(&budget_failures);
}

fn large_source() -> String {
    (0..100_000)
        .map(|index| format!("line {index}\n"))
        .collect()
}

fn record_performance_metrics(
    scenario: &str,
    iterations: usize,
    elapsed: Duration,
    budget: Duration,
    metrics: &LayoutMetrics,
) -> Option<String> {
    let path = std::env::var_os("ICE_EDITOR_PERF_JSONL").map(std::path::PathBuf::from);
    let injected = std::env::var_os("ICE_EDITOR_PERF_INJECT_WALL_FAILURE");
    let budget = wall_time_budget(scenario, budget, injected.as_deref());
    record_performance_metrics_to(
        path.as_deref(),
        scenario,
        iterations,
        elapsed,
        budget,
        metrics,
    )
}

fn wall_time_budget(
    scenario: &str,
    budget: Duration,
    injected_failure: Option<&std::ffi::OsStr>,
) -> Duration {
    if injected_failure == Some(std::ffi::OsStr::new(scenario)) {
        Duration::from_nanos(1)
    } else {
        budget
    }
}

fn record_performance_metrics_to(
    path: Option<&std::path::Path>,
    scenario: &str,
    iterations: usize,
    elapsed: Duration,
    budget: Duration,
    metrics: &LayoutMetrics,
) -> Option<String> {
    let value = serde_json::json!({
        "schema": "ice.rich-text-editor.performance.v1",
        "kind": "operation",
        "scenario": scenario,
        "document_lines": 100_001,
        "iterations": iterations,
        "elapsed_ns": u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX),
        "wall_time_budget_ns": u64::try_from(budget.as_nanos()).unwrap_or(u64::MAX),
        "metrics": {
            "full_text_materializations": metrics.full_text_materializations,
            "materialized_source_bytes": metrics.materialized_source_bytes,
            "composition_display_strings": metrics.composition_display_strings,
            "composition_display_bytes": metrics.composition_display_bytes,
            "mapping_line_comparisons": metrics.mapping_line_comparisons,
            "styled_signature_comparisons": metrics.styled_signature_comparisons,
            "newly_owned_styled_texts": metrics.newly_owned_styled_texts,
            "newly_owned_styled_text_bytes": metrics.newly_owned_styled_text_bytes,
            "line_vector_slots_prepared": metrics.line_vector_slots_prepared,
            "rebuilt_lines": metrics.rebuilt_lines,
            "shaped_paragraphs": metrics.shaped_paragraphs,
            "highlighted_lines": metrics.highlighted_lines,
            "accepted_change_hints": metrics.accepted_change_hints,
            "rejected_change_hints": metrics.rejected_change_hints,
        },
    });
    if let Some(path) = path {
        append_performance_record(path, &value);
    }

    (elapsed >= budget).then(|| {
        format!(
            "{scenario} took {}ns; budget is {}ns",
            elapsed.as_nanos(),
            budget.as_nanos()
        )
    })
}

fn append_performance_record(path: &std::path::Path, value: &serde_json::Value) {
    use std::fs::OpenOptions;
    use std::io::Write as _;

    let mut output = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap_or_else(|error| panic!("failed to open {}: {error}", path.display()));
    writeln!(output, "{value}")
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", path.display()));
    output
        .flush()
        .unwrap_or_else(|error| panic!("failed to flush {}: {error}", path.display()));
    output
        .sync_all()
        .unwrap_or_else(|error| panic!("failed to sync {}: {error}", path.display()));
}

fn assert_performance_budgets(failures: &[String]) {
    if let Err(message) = performance_budget_gate(failures) {
        panic!("{message}");
    }
}

fn performance_budget_gate(failures: &[String]) -> Result<(), String> {
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "RichTextEditor wall-time budgets failed:\n{}",
            failures.join("\n")
        ))
    }
}

#[test]
fn performance_evidence_failure_injection_preserves_wall_record() {
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_PATH: AtomicU64 = AtomicU64::new(0);

    let path = std::env::temp_dir().join(format!(
        "ice-editor-wall-evidence-{}-{}.jsonl",
        std::process::id(),
        NEXT_PATH.fetch_add(1, Ordering::Relaxed)
    ));
    let metrics = LayoutMetrics::default();
    let budget = wall_time_budget(
        "caret_1000",
        Duration::from_secs(5),
        Some(std::ffi::OsStr::new("caret_1000")),
    );
    let failure = record_performance_metrics_to(
        Some(&path),
        "caret_1000",
        1_000,
        Duration::from_nanos(2),
        budget,
        &metrics,
    )
    .expect("injected wall-time excess must fail the gate");

    let raw = std::fs::read_to_string(&path).expect("failure evidence must be readable");
    std::fs::remove_file(&path).expect("temporary failure evidence must be removable");
    let lines = raw.lines().collect::<Vec<_>>();
    let [line] = lines.as_slice() else {
        panic!("failure evidence must contain exactly one record: {raw:?}");
    };
    let record: serde_json::Value = serde_json::from_str(line).expect("valid failure evidence");
    assert_eq!(record["scenario"], "caret_1000");
    assert_eq!(record["elapsed_ns"], 2);
    assert_eq!(record["wall_time_budget_ns"], 1);
    assert!(failure.contains("took 2ns; budget is 1ns"));
    assert!(performance_budget_gate(&[failure]).is_err());
}
