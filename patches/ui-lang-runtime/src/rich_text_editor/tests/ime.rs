use super::*;

#[test]
fn preedit_uses_the_same_wrapped_layout_as_committed_text() {
    fn geometry(lines: Lines<'_>) -> Vec<(usize, usize, f32, f32, f32)> {
        let mut document = DocumentLayout::default();
        document.update(
            lines,
            &mut WholeLine::default(),
            &|_| Format::default(),
            test_layout_style(70.0),
            DocumentUpdate::text(DocumentChange::Discover),
            usize::MAX,
        );
        document
            .lines
            .iter()
            .enumerate()
            .flat_map(|(line_index, line)| {
                line.paragraph.buffer().layout_runs().map(move |run| {
                    (
                        line_index,
                        run.glyphs.len(),
                        line.top + run.line_top,
                        run.line_height,
                        run.line_w,
                    )
                })
            })
            .collect()
    }

    let mut source: Content = Content::with_text("앞 뒤");
    source.move_to(Cursor {
        position: Position { line: 0, column: 4 },
        selection: None,
    });
    let source_text = source.text();
    let source_lines = TextLines::parse(&source_text);
    let composition = CompositionDocument::new(
        source.cursor(),
        &source_text,
        source_lines,
        &input_method::Preedit {
            content: "한글입력".into(),
            selection: Some(12..12),
            text_size: None,
        },
    )
    .expect("visible composition");
    let committed = Content::with_text("앞 한글입력뒤");

    assert_eq!(source.text(), "앞 뒤");
    let committed_doc = TestDoc::new(&content_lines(&committed));
    let composed = Lines::new(&composition.display, &composition.layout.display_lines);
    assert_eq!(
        (0..composed.len())
            .map(|i| composed.get(i))
            .collect::<Vec<_>>(),
        (0..committed_doc.lines().len())
            .map(|i| committed_doc.lines().get(i))
            .collect::<Vec<_>>()
    );
    assert_eq!(geometry(composed), geometry(committed_doc.lines()));
    assert_eq!(
        composition.layout.cursor,
        Position {
            line: 0,
            column: 16
        }
    );
    assert_eq!(
        composition.layout.display_to_source(Position {
            line: 0,
            column: 10
        }),
        Position { line: 0, column: 4 }
    );
}

#[test]
fn preedit_replaces_the_selected_source_without_committing_it() {
    let mut source: Content = Content::with_text("앞 OLD 뒤");
    source.move_to(Cursor {
        position: Position { line: 0, column: 7 },
        selection: Some(Position { line: 0, column: 4 }),
    });
    let source_text = source.text();
    let source_lines = TextLines::parse(&source_text);
    let composition = CompositionDocument::new(
        source.cursor(),
        &source_text,
        source_lines,
        &input_method::Preedit {
            content: "한글".into(),
            selection: Some(6..6),
            text_size: None,
        },
    )
    .expect("visible composition");

    assert_eq!(source.text(), "앞 OLD 뒤");
    let composed = Lines::new(&composition.display, &composition.layout.display_lines);
    assert_eq!(
        (0..composed.len())
            .map(|i| composed.get(i))
            .collect::<Vec<_>>(),
        ["앞 한글 뒤"]
    );
    assert_eq!(
        composition.layout.display_to_source(Position {
            line: 0,
            column: 10
        }),
        Position { line: 0, column: 7 }
    );
}

#[test]
fn hangul_ime_stages_relayout_before_the_next_key() {
    use iced::advanced::clipboard;
    use iced::advanced::renderer::Headless;

    let mut content = Content::with_text("앞 ");
    content.move_to(Cursor {
        position: Position { line: 0, column: 4 },
        selection: None,
    });
    let mut editor = RichTextEditor::new(&content, ContentVersion::new(1, 0))
        .change_hint(test_change(0, 1, 1))
        .width(Length::Fixed(120.0))
        .height(Length::Fixed(80.0))
        .on_action(|action| action);
    let renderer = iced_test::futures::futures::executor::block_on(
        <iced::Renderer as Headless>::new(Font::DEFAULT, Pixels(16.0), Some("tiny-skia")),
    )
    .expect("headless renderer");
    let mut tree = widget::Tree::new(&editor as &dyn Widget<_, Theme, iced::Renderer>);
    tree.state
        .downcast_mut::<State<text::highlighter::PlainText>>()
        .focus = Some(Focus::now());
    let limits = layout::Limits::new(Size::ZERO, Size::new(120.0, 80.0));
    let mut node = editor.layout(&mut tree, &renderer, &limits);
    tree.state
        .downcast_mut::<State<text::highlighter::PlainText>>()
        .metrics = LayoutMetrics::default();
    let viewport = Rectangle::with_size(Size::new(120.0, 80.0));
    let mut clipboard = clipboard::Null;

    for stage in ["ㅇ", "으", "응"] {
        let event = Event::InputMethod(input_method::Event::Preedit(stage.to_owned(), Some(3..3)));
        let mut messages = Vec::new();
        let mut shell = Shell::new(&mut messages);

        editor.update(
            &mut tree,
            &event,
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &renderer,
            &mut clipboard,
            &mut shell,
            &viewport,
        );

        assert!(
            shell.is_layout_invalid(),
            "{stage:?} must reshape in the same event cycle"
        );
        shell.revalidate_layout(|| {
            node = editor.layout(&mut tree, &renderer, &limits);
        });
        assert!(messages.is_empty());
        assert_eq!(
            tree.state
                .downcast_ref::<State<text::highlighter::PlainText>>()
                .shaped_preedit
                .as_ref()
                .map(|preedit| preedit.content.as_str()),
            Some(stage)
        );
    }
    let metrics = &tree
        .state
        .downcast_ref::<State<text::highlighter::PlainText>>()
        .metrics;
    assert_eq!(metrics.full_text_materializations, 0);
    assert_eq!(metrics.mapping_line_comparisons, 6);
    assert_eq!(metrics.rebuilt_lines, 3);
    assert_eq!(metrics.shaped_paragraphs, 3);
    assert_eq!(metrics.accepted_change_hints, 0);

    // winit clears preedit immediately before the assembled commit. These
    // two events belong to the same OS event cycle; no full string was
    // inserted during the three composition updates above.
    let mut messages = Vec::new();
    for event in [
        Event::InputMethod(input_method::Event::Preedit(String::new(), None)),
        Event::InputMethod(input_method::Event::Commit("응".to_owned())),
    ] {
        let mut shell = Shell::new(&mut messages);
        editor.update(
            &mut tree,
            &event,
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &renderer,
            &mut clipboard,
            &mut shell,
            &viewport,
        );
        shell.revalidate_layout(|| {
            node = editor.layout(&mut tree, &renderer, &limits);
        });
    }

    let [Action::Edit(text_editor::Action::Edit(Edit::Paste(committed)))] = messages.as_slice()
    else {
        panic!("IME commit must produce exactly one text edit: {messages:?}");
    };
    assert_eq!(committed.as_str(), "응");
}

#[test]
fn macos_ime_boundary_survives_the_trailing_empty_preedit() {
    use iced::keyboard::key::{Code, Physical};

    let period = keyboard::Key::Character(".".into());
    let no_modifiers = keyboard::Modifiers::empty();

    let mut pending = PendingImeCommit::default();
    pending.on_preedit("강");
    pending.on_preedit("");
    pending.on_commit("강");
    pending.on_preedit("");

    let character =
        ime_boundary_character(&period, &period, Physical::Code(Code::Period), no_modifiers);
    assert_eq!(pending.resolve(None), ImeBoundary::Unrelated);
    assert_eq!(pending.resolve(character), ImeBoundary::Missing('.'));
    assert_eq!(pending.resolve(character), ImeBoundary::Unrelated);
}

#[test]
fn ime_close_preserves_release_only_punctuation() {
    use iced::advanced::clipboard;
    use iced::advanced::renderer::Headless;
    use iced::keyboard::key::{Code, Physical};
    use iced::keyboard::{Key, Location, Modifiers};

    let content = Content::with_text("ㄹ");
    let mut editor = RichTextEditor::new(&content, ContentVersion::new(1, 0))
        .width(Length::Fixed(120.0))
        .height(Length::Fixed(80.0))
        .on_action(|action| action);
    let renderer = iced_test::futures::futures::executor::block_on(
        <iced::Renderer as Headless>::new(Font::DEFAULT, Pixels(16.0), Some("tiny-skia")),
    )
    .expect("headless renderer");
    let mut tree = widget::Tree::new(&editor as &dyn Widget<_, Theme, iced::Renderer>);
    let state = tree
        .state
        .downcast_mut::<State<text::highlighter::PlainText>>();
    state.focus = Some(Focus::now());
    let limits = layout::Limits::new(Size::ZERO, Size::new(120.0, 80.0));
    let node = editor.layout(&mut tree, &renderer, &limits);
    let viewport = Rectangle::with_size(Size::new(120.0, 80.0));
    let mut clipboard = clipboard::Null;
    let mut messages = Vec::new();

    for (character, code) in [(',', Code::Comma), ('.', Code::Period)] {
        tree.state
            .downcast_mut::<State<text::highlighter::PlainText>>()
            .pending_ime_commit
            .on_commit("ㄹ");
        let key = Key::Character(character.to_string().into());
        for event in [
            Event::InputMethod(input_method::Event::Closed),
            Event::Keyboard(keyboard::Event::KeyReleased {
                key: key.clone(),
                modified_key: key,
                physical_key: Physical::Code(code),
                location: Location::Standard,
                modifiers: Modifiers::empty(),
            }),
        ] {
            let mut shell = Shell::new(&mut messages);
            editor.update(
                &mut tree,
                &event,
                Layout::new(&node),
                mouse::Cursor::Unavailable,
                &renderer,
                &mut clipboard,
                &mut shell,
                &viewport,
            );
        }
    }

    assert_eq!(
        messages,
        [
            Action::Edit(text_editor::Action::Edit(Edit::Insert(','))),
            Action::Edit(text_editor::Action::Edit(Edit::Insert('.'))),
        ]
    );
}

#[test]
fn ime_boundary_press_and_release_produce_exactly_one_ascii_edit() {
    use iced::advanced::clipboard;
    use iced::advanced::renderer::Headless;
    use iced::keyboard::key::{Code, Physical};
    use iced::keyboard::{Key, Location, Modifiers};

    let content = Content::with_text("ㄹ");
    let mut editor = RichTextEditor::new(&content, ContentVersion::new(1, 0))
        .width(Length::Fixed(120.0))
        .height(Length::Fixed(80.0))
        .on_action(|action| action);
    let renderer = iced_test::futures::futures::executor::block_on(
        <iced::Renderer as Headless>::new(Font::DEFAULT, Pixels(16.0), Some("tiny-skia")),
    )
    .expect("headless renderer");
    let mut tree = widget::Tree::new(&editor as &dyn Widget<_, Theme, iced::Renderer>);
    tree.state
        .downcast_mut::<State<text::highlighter::PlainText>>()
        .focus = Some(Focus::now());
    let limits = layout::Limits::new(Size::ZERO, Size::new(120.0, 80.0));
    let node = editor.layout(&mut tree, &renderer, &limits);
    let viewport = Rectangle::with_size(Size::new(120.0, 80.0));
    let mut clipboard = clipboard::Null;
    let mut messages = Vec::new();
    let comma = Key::Character(",".into());

    tree.state
        .downcast_mut::<State<text::highlighter::PlainText>>()
        .pending_ime_commit
        .on_commit("ㄹ");
    for event in [
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: comma.clone(),
            modified_key: comma.clone(),
            physical_key: Physical::Code(Code::Comma),
            location: Location::Standard,
            modifiers: Modifiers::empty(),
            text: Some(",".into()),
            repeat: false,
        }),
        Event::Keyboard(keyboard::Event::KeyReleased {
            key: comma.clone(),
            modified_key: comma,
            physical_key: Physical::Code(Code::Comma),
            location: Location::Standard,
            modifiers: Modifiers::empty(),
        }),
    ] {
        let mut shell = Shell::new(&mut messages);
        editor.update(
            &mut tree,
            &event,
            Layout::new(&node),
            mouse::Cursor::Unavailable,
            &renderer,
            &mut clipboard,
            &mut shell,
            &viewport,
        );
    }
    assert_eq!(
        messages,
        [Action::Edit(text_editor::Action::Edit(Edit::Insert(',')))]
    );
    messages.clear();

    tree.state
        .downcast_mut::<State<text::highlighter::PlainText>>()
        .pending_ime_commit
        .on_commit("ㄹ ");
    let space = Key::Named(keyboard::key::Named::Space);
    let mut shell = Shell::new(&mut messages);
    editor.update(
        &mut tree,
        &Event::Keyboard(keyboard::Event::KeyPressed {
            key: space.clone(),
            modified_key: space,
            physical_key: Physical::Code(Code::Space),
            location: Location::Standard,
            modifiers: Modifiers::empty(),
            text: Some(" ".into()),
            repeat: false,
        }),
        Layout::new(&node),
        mouse::Cursor::Unavailable,
        &renderer,
        &mut clipboard,
        &mut shell,
        &viewport,
    );
    assert!(shell.is_event_captured());
    assert!(messages.is_empty());

    tree.state
        .downcast_mut::<State<text::highlighter::PlainText>>()
        .pending_ime_commit
        .on_commit("ㄹ ");
    let command = if cfg!(target_os = "macos") {
        Modifiers::LOGO
    } else {
        Modifiers::CTRL
    };
    let select_all = Key::Character("a".into());
    let mut shell = Shell::new(&mut messages);
    editor.update(
        &mut tree,
        &Event::Keyboard(keyboard::Event::KeyPressed {
            key: select_all.clone(),
            modified_key: select_all,
            physical_key: Physical::Code(Code::KeyA),
            location: Location::Standard,
            modifiers: command,
            text: Some("a".into()),
            repeat: false,
        }),
        Layout::new(&node),
        mouse::Cursor::Unavailable,
        &renderer,
        &mut clipboard,
        &mut shell,
        &viewport,
    );
    assert!(
        !tree
            .state
            .downcast_ref::<State<text::highlighter::PlainText>>()
            .pending_ime_commit
            .is_pending()
    );
}

#[test]
fn macos_ime_boundary_deduplicates_committed_keys_and_recovers_ascii() {
    use iced::keyboard::key::{Code, Physical};

    let hangul = keyboard::Key::Character("ㄹ".into());
    let no_modifiers = keyboard::Modifiers::empty();
    let shifted = keyboard::Modifiers::SHIFT;
    let boundary = |key: &keyboard::Key, code, modifiers| {
        ime_boundary_character(key, key, Physical::Code(code), modifiers)
    };
    let resolve = |committed: &str, character| {
        let mut pending = PendingImeCommit::default();
        pending.on_commit(committed);
        pending.on_preedit("");
        pending.resolve(character)
    };

    assert_eq!(
        resolve("ㄹ", boundary(&hangul, Code::Comma, no_modifiers)),
        ImeBoundary::Missing(',')
    );
    let one = keyboard::Key::Character("1".into());
    assert_eq!(
        resolve("강", boundary(&one, Code::Digit1, no_modifiers)),
        ImeBoundary::Missing('1')
    );
    let bang = keyboard::Key::Character("!".into());
    assert_eq!(
        resolve("강", boundary(&bang, Code::Digit1, shifted)),
        ImeBoundary::Missing('!')
    );
    let question = keyboard::Key::Character("?".into());
    assert_eq!(
        resolve("강", boundary(&question, Code::Slash, shifted)),
        ImeBoundary::Missing('?')
    );
    let space = keyboard::Key::Named(key::Named::Space);
    assert_eq!(
        resolve("강 ", boundary(&space, Code::Space, no_modifiers)),
        ImeBoundary::Duplicate
    );

    let mut duplicate_space = PendingImeCommit::default();
    duplicate_space.on_preedit(" ");
    duplicate_space.on_preedit("");
    duplicate_space.on_commit(" ");
    duplicate_space.on_preedit("");
    assert_eq!(
        duplicate_space.resolve(boundary(&space, Code::Space, no_modifiers)),
        ImeBoundary::Duplicate
    );
    assert_eq!(
        boundary(&hangul, Code::Comma, keyboard::Modifiers::CTRL),
        None
    );
    assert_eq!(
        boundary(&hangul, Code::Comma, keyboard::Modifiers::ALT),
        None
    );

    let mut pending = PendingImeCommit::default();
    pending.on_commit("강");
    pending.on_preedit("ㄴ");
    assert_eq!(
        pending.resolve(boundary(&hangul, Code::Period, no_modifiers)),
        ImeBoundary::Unrelated
    );
}

#[test]
fn application_command_shortcuts_are_not_inserted_as_text() {
    use iced::keyboard::key::{Code, Physical};
    use iced::keyboard::{Key, Modifiers};
    use iced::widget::text_editor::Status;

    let command = if cfg!(target_os = "macos") {
        Modifiers::LOGO
    } else {
        Modifiers::CTRL
    };
    let press = |key: Key, code: Code, text: &str| text_editor::KeyPress {
        key,
        modified_key: Key::Character(text.into()),
        physical_key: Physical::Code(code),
        modifiers: command,
        text: Some(text.into()),
        status: Status::Focused { is_hovered: true },
    };

    assert_eq!(
        default_key_binding(&press(Key::Character("z".into()), Code::KeyZ, "z")),
        None
    );
    assert_eq!(
        default_key_binding(&press(Key::Character("ㅋ".into()), Code::KeyZ, "z")),
        None
    );
    assert_eq!(
        default_key_binding(&press(Key::Character("c".into()), Code::KeyC, "c")),
        Some(Binding::Copy)
    );
    assert_eq!(
        default_key_binding(&press(Key::Character("x".into()), Code::KeyX, "x")),
        Some(Binding::Cut)
    );
    assert_eq!(
        default_key_binding(&press(Key::Character("v".into()), Code::KeyV, "v")),
        Some(Binding::Paste)
    );
    assert_eq!(
        default_key_binding(&press(Key::Character("a".into()), Code::KeyA, "a")),
        Some(Binding::SelectAll)
    );
}

#[test]
fn editor_specific_shortcuts_use_stock_edit_actions() {
    use iced::keyboard::key::{Code, Named, Physical};
    use iced::keyboard::{Key, Modifiers};
    use iced::widget::text_editor::Status;

    let press = |named, code, modifiers| {
        let key = Key::Named(named);
        text_editor::KeyPress {
            key: key.clone(),
            modified_key: key,
            physical_key: Physical::Code(code),
            modifiers,
            text: None,
            status: Status::Focused { is_hovered: true },
        }
    };

    assert_eq!(
        rich_binding(&press(Named::Tab, Code::Tab, Modifiers::empty())),
        Some(Binding::Custom(Edit::Indent))
    );
    assert_eq!(
        rich_binding(&press(Named::Tab, Code::Tab, Modifiers::SHIFT)),
        Some(Binding::Custom(Edit::Unindent))
    );

    let jump = if cfg!(target_os = "macos") {
        Modifiers::ALT
    } else {
        Modifiers::CTRL
    };
    assert_eq!(
        rich_binding(&press(Named::Backspace, Code::Backspace, jump)),
        Some(Binding::Sequence(vec![
            Binding::Select(Motion::WordLeft),
            Binding::Backspace,
        ]))
    );
}

#[cfg(target_os = "macos")]
#[test]
fn macos_command_deletes_to_visual_line_boundaries() {
    use iced::keyboard::key::{Code, Named, Physical};
    use iced::keyboard::{Key, Modifiers};
    use iced::widget::text_editor::Status;

    let press = |named, code| {
        let key = Key::Named(named);
        text_editor::KeyPress {
            key: key.clone(),
            modified_key: key,
            physical_key: Physical::Code(code),
            modifiers: Modifiers::LOGO,
            text: None,
            status: Status::Focused { is_hovered: true },
        }
    };

    assert_eq!(
        rich_binding(&press(Named::Backspace, Code::Backspace)),
        Some(Binding::Sequence(vec![
            Binding::Select(Motion::Home),
            Binding::Backspace,
        ]))
    );
    assert_eq!(
        rich_binding(&press(Named::Delete, Code::Delete)),
        Some(Binding::Sequence(vec![
            Binding::Select(Motion::End),
            Binding::Delete,
        ]))
    );
}
