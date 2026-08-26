use super::*;

#[test]
fn pointer_coordinates_distinguish_relative_hits_from_global_drags() {
    let padding = Padding::from([3.0, 5.0]);
    let relative = Point::new(17.0, 19.0);
    assert_eq!(local_point(relative, padding, 11.0), Point::new(12.0, 27.0));

    let translated_bounds = Rectangle::new(Point::new(100.0, 200.0), Size::new(80.0, 40.0));
    assert_eq!(
        clamped_local_point(Point::new(117.0, 219.0), translated_bounds, padding, 11.0,),
        Point::new(12.0, 27.0)
    );
    assert_eq!(
        clamped_local_point(Point::new(250.0, 150.0), translated_bounds, padding, 11.0,),
        Point::new(75.0, 8.0)
    );
}

#[test]
fn clicks_in_editor_padding_focus_and_clear_selection() {
    use iced::advanced::clipboard;
    use iced::advanced::renderer::Headless;

    let mut content = Content::with_text("alpha beta");
    content.move_to(Cursor {
        position: Position { line: 0, column: 5 },
        selection: Some(Position { line: 0, column: 0 }),
    });
    let mut editor = RichTextEditor::new(&content, ContentVersion::new(1, 0))
        .width(Length::Fixed(160.0))
        .height(Length::Fixed(80.0))
        .padding(16.0)
        .on_action(|action| action);
    let renderer = iced_test::futures::futures::executor::block_on(
        <iced::Renderer as Headless>::new(Font::DEFAULT, Pixels(16.0), Some("tiny-skia")),
    )
    .expect("headless renderer");
    let mut tree = widget::Tree::new(&editor as &dyn Widget<_, Theme, iced::Renderer>);
    tree.state
        .downcast_mut::<State<text::highlighter::PlainText>>()
        .focus = Some(Focus::now());
    let limits = layout::Limits::new(Size::ZERO, Size::new(160.0, 80.0));
    let node = editor.layout(&mut tree, &renderer, &limits);
    let viewport = Rectangle::with_size(Size::new(160.0, 80.0));
    let mut clipboard = clipboard::Null;
    let mut messages = Vec::new();
    let mut shell = Shell::new(&mut messages);
    tree.state
        .downcast_mut::<State<text::highlighter::PlainText>>()
        .pending_ime_commit
        .on_commit("ㄹ ");

    editor.update(
        &mut tree,
        &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
        Layout::new(&node),
        mouse::Cursor::Available(Point::new(4.0, 20.0)),
        &renderer,
        &mut clipboard,
        &mut shell,
        &viewport,
    );

    assert!(shell.is_event_captured());
    assert_eq!(
        messages,
        [Action::MoveTo(Cursor {
            position: Position { line: 0, column: 0 },
            selection: None,
        })]
    );
    assert!(
        tree.state
            .downcast_ref::<State<text::highlighter::PlainText>>()
            .focus
            .is_some()
    );
    assert!(
        !tree
            .state
            .downcast_ref::<State<text::highlighter::PlainText>>()
            .pending_ime_commit
            .is_pending()
    );
    messages.clear();

    let release_was_captured = {
        let mut shell = Shell::new(&mut messages);
        editor.update(
            &mut tree,
            &Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            Layout::new(&node),
            mouse::Cursor::Available(Point::new(4.0, 20.0)),
            &renderer,
            &mut clipboard,
            &mut shell,
            &viewport,
        );
        shell.is_event_captured()
    };
    assert!(release_was_captured);
    assert!(messages.is_empty());

    editor = editor.focus_enabled(false);
    let mut shell = Shell::new(&mut messages);
    editor.update(
        &mut tree,
        &Event::Window(window::Event::RedrawRequested(Instant::now())),
        Layout::new(&node),
        mouse::Cursor::Unavailable,
        &renderer,
        &mut clipboard,
        &mut shell,
        &viewport,
    );
    let state = tree
        .state
        .downcast_ref::<State<text::highlighter::PlainText>>();
    assert!(state.focus.is_none());
    assert!(state.pointer.drag_anchor.is_none());
}

#[test]
fn a_selection_drag_does_not_turn_the_next_click_into_a_double_click() {
    use iced::advanced::clipboard;
    use iced::advanced::renderer::Headless;

    let content = Content::with_text("alpha beta gamma");
    let padding = 8.0;
    let mut editor = RichTextEditor::new(&content, ContentVersion::new(1, 0))
        .width(Length::Fixed(220.0))
        .height(Length::Fixed(80.0))
        .padding(padding)
        .on_action(|action| action);
    let renderer = iced_test::futures::futures::executor::block_on(
        <iced::Renderer as Headless>::new(Font::DEFAULT, Pixels(16.0), Some("tiny-skia")),
    )
    .expect("headless renderer");
    let mut tree = widget::Tree::new(&editor as &dyn Widget<_, Theme, iced::Renderer>);
    let limits = layout::Limits::new(Size::ZERO, Size::new(220.0, 80.0));
    let node = editor.layout(&mut tree, &renderer, &limits);
    let viewport = Rectangle::with_size(Size::new(220.0, 80.0));
    let (start_position, start, outside) = {
        let state = tree
            .state
            .downcast_ref::<State<text::highlighter::PlainText>>();
        let start_position = Position { line: 0, column: 1 };
        let start = state.document.caret(start_position);
        let end = state.document.caret(Position {
            line: 0,
            column: 10,
        });
        (
            start_position,
            Point::new(padding + start.x, padding + start.y + start.height / 2.0),
            Point::new(260.0, padding + end.y + end.height / 2.0),
        )
    };
    let mut clipboard = clipboard::Null;
    let mut messages = Vec::new();

    {
        let mut shell = Shell::new(&mut messages);
        editor.update(
            &mut tree,
            &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Layout::new(&node),
            mouse::Cursor::Available(start),
            &renderer,
            &mut clipboard,
            &mut shell,
            &viewport,
        );
    }
    assert_eq!(
        messages,
        [Action::MoveTo(Cursor {
            position: start_position,
            selection: None,
        })]
    );
    messages.clear();

    {
        let mut shell = Shell::new(&mut messages);
        editor.update(
            &mut tree,
            &Event::Mouse(mouse::Event::CursorMoved { position: outside }),
            Layout::new(&node),
            mouse::Cursor::Available(outside),
            &renderer,
            &mut clipboard,
            &mut shell,
            &viewport,
        );
    }
    let [Action::MoveTo(dragged)] = messages.as_slice() else {
        panic!("drag must publish one rich selection: {messages:?}");
    };
    assert_eq!(dragged.selection, Some(start_position));
    assert_eq!(dragged.position.column, "alpha beta gamma".len());
    let state = tree
        .state
        .downcast_ref::<State<text::highlighter::PlainText>>();
    assert!(state.pointer.drag_moved);
    assert!(state.pointer.last_click.is_none());
    messages.clear();

    let release_was_captured = {
        let mut shell = Shell::new(&mut messages);
        editor.update(
            &mut tree,
            &Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            Layout::new(&node),
            mouse::Cursor::Available(outside),
            &renderer,
            &mut clipboard,
            &mut shell,
            &viewport,
        );
        shell.is_event_captured()
    };
    assert!(release_was_captured);
    assert!(messages.is_empty());

    {
        let mut shell = Shell::new(&mut messages);
        editor.update(
            &mut tree,
            &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Layout::new(&node),
            mouse::Cursor::Available(start),
            &renderer,
            &mut clipboard,
            &mut shell,
            &viewport,
        );
    }
    let [Action::MoveTo(clicked)] = messages.as_slice() else {
        panic!("post-drag click must publish one caret move: {messages:?}");
    };
    assert_eq!(clicked.position, start_position);
    assert_eq!(clicked.selection, None);
}

#[test]
fn drag_anchor_keeps_the_press_layout_until_release() {
    use iced::advanced::clipboard;
    use iced::advanced::renderer::Headless;

    let mut content = Content::with_text("xabcdef");
    let padding = 8.0;
    let marker_format = |expanded: &bool| Format {
        size: Some(Pixels(if *expanded { 64.0 } else { 0.01 })),
        ..Format::default()
    };
    let mut editor = RichTextEditor::new(&content, ContentVersion::new(1, 0))
        .width(Length::Fixed(220.0))
        .height(Length::Fixed(80.0))
        .padding(padding)
        .highlight_with::<CaretSizedMarker>(false, 0, marker_format)
        .on_action(|action| action);
    let renderer = iced_test::futures::futures::executor::block_on(
        <iced::Renderer as Headless>::new(Font::DEFAULT, Pixels(16.0), Some("tiny-skia")),
    )
    .expect("headless renderer");
    let mut tree = widget::Tree::new(&editor as &dyn Widget<_, Theme, iced::Renderer>);
    let limits = layout::Limits::new(Size::ZERO, Size::new(220.0, 80.0));
    let node = editor.layout(&mut tree, &renderer, &limits);
    let viewport = Rectangle::with_size(Size::new(220.0, 80.0));
    let anchor = Position { line: 0, column: 4 };
    let start = {
        let caret = tree
            .state
            .downcast_ref::<State<CaretSizedMarker>>()
            .document
            .caret(anchor);
        Point::new(padding + caret.x, padding + caret.y + caret.height / 2.0)
    };
    let mut clipboard = clipboard::Null;
    let mut messages = Vec::new();

    {
        let mut shell = Shell::new(&mut messages);
        editor.update(
            &mut tree,
            &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Layout::new(&node),
            mouse::Cursor::Available(start),
            &renderer,
            &mut clipboard,
            &mut shell,
            &viewport,
        );
    }
    let [Action::MoveTo(clicked)] = messages.as_slice() else {
        panic!("press must publish one caret move: {messages:?}");
    };
    assert_eq!(clicked.position, anchor);
    let clicked = *clicked;
    messages.clear();
    drop(editor);
    content.move_to(clicked);

    let mut editor = RichTextEditor::new(&content, ContentVersion::new(1, 0))
        .width(Length::Fixed(220.0))
        .height(Length::Fixed(80.0))
        .padding(padding)
        .highlight_with::<CaretSizedMarker>(true, 0, marker_format)
        .on_action(|action| action);
    let node = editor.layout(&mut tree, &renderer, &limits);
    assert!(
        !tree
            .state
            .downcast_ref::<State<CaretSizedMarker>>()
            .settings
    );

    {
        let mut shell = Shell::new(&mut messages);
        editor.update(
            &mut tree,
            &Event::Mouse(mouse::Event::CursorMoved { position: start }),
            Layout::new(&node),
            mouse::Cursor::Available(start),
            &renderer,
            &mut clipboard,
            &mut shell,
            &viewport,
        );
    }
    assert_eq!(
        messages,
        [Action::MoveTo(Cursor {
            position: anchor,
            selection: None,
        })],
        "returning to the physical press point must collapse the drag"
    );
    messages.clear();

    let release_invalidated_layout = {
        let mut shell = Shell::new(&mut messages);
        editor.update(
            &mut tree,
            &Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            Layout::new(&node),
            mouse::Cursor::Available(start),
            &renderer,
            &mut clipboard,
            &mut shell,
            &viewport,
        );
        shell.is_layout_invalid()
    };
    assert!(release_invalidated_layout);

    editor.layout(&mut tree, &renderer, &limits);
    assert!(
        tree.state
            .downcast_ref::<State<CaretSizedMarker>>()
            .settings
    );
}

#[test]
fn only_a_rendered_link_hit_can_reach_an_outer_release_handler() {
    use iced::advanced::clipboard;
    use iced::advanced::renderer::Headless;

    let content = Content::with_text("link text");
    let padding = 8.0;
    let mut editor = RichTextEditor::new(&content, ContentVersion::new(1, 0))
        .width(Length::Fixed(220.0))
        .height(Length::Fixed(100.0))
        .padding(padding)
        .mouse_interaction(|_, position| {
            if position.column < 4 {
                mouse::Interaction::Pointer
            } else {
                mouse::Interaction::Text
            }
        })
        .on_action(|action| action);
    let renderer = iced_test::futures::futures::executor::block_on(
        <iced::Renderer as Headless>::new(Font::DEFAULT, Pixels(16.0), Some("tiny-skia")),
    )
    .expect("headless renderer");
    let mut tree = widget::Tree::new(&editor as &dyn Widget<_, Theme, iced::Renderer>);
    let limits = layout::Limits::new(Size::ZERO, Size::new(220.0, 100.0));
    let node = editor.layout(&mut tree, &renderer, &limits);
    let viewport = Rectangle::with_size(Size::new(220.0, 100.0));
    let link = {
        let state = tree
            .state
            .downcast_ref::<State<text::highlighter::PlainText>>();
        let caret = state.document.caret(Position { line: 0, column: 2 });
        Point::new(padding + caret.x, padding + caret.y + caret.height / 2.0)
    };
    let blank = Point::new(link.x, 90.0);

    assert_eq!(
        Widget::mouse_interaction(
            &editor,
            &tree,
            Layout::new(&node),
            mouse::Cursor::Available(link),
            &viewport,
            &renderer,
        ),
        mouse::Interaction::Pointer
    );
    assert_eq!(
        Widget::mouse_interaction(
            &editor,
            &tree,
            Layout::new(&node),
            mouse::Cursor::Available(blank),
            &viewport,
            &renderer,
        ),
        mouse::Interaction::Text
    );

    let mut clipboard = clipboard::Null;
    let mut messages = Vec::new();
    for point in [blank, link] {
        {
            let mut shell = Shell::new(&mut messages);
            editor.update(
                &mut tree,
                &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                Layout::new(&node),
                mouse::Cursor::Available(point),
                &renderer,
                &mut clipboard,
                &mut shell,
                &viewport,
            );
        }
        messages.clear();
        let captured = {
            let mut shell = Shell::new(&mut messages);
            editor.update(
                &mut tree,
                &Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                Layout::new(&node),
                mouse::Cursor::Available(point),
                &renderer,
                &mut clipboard,
                &mut shell,
                &viewport,
            );
            shell.is_event_captured()
        };
        assert_eq!(captured, point == blank);
        assert!(messages.is_empty());
    }
}

/// The dismissal contract of the anchored menu: a press outside the widget or
/// a window blur publishes `Dismiss` — and MOVING THE POINTER NEVER DOES.
/// A menu a click opened outlives the pointer, exactly like the anchor it
/// hangs off (see `gutter_line`).
mod menu_isolation {
    use super::*;
    use iced::advanced::clipboard;
    use iced::advanced::renderer::Headless;

    #[derive(Debug, Clone, PartialEq)]
    enum Msg {
        Act(Action),
        Menu(MenuEvent),
    }

    fn menu(anchor: MenuAnchor) -> EditorMenu {
        EditorMenu {
            anchor,
            items: vec![
                MenuItem {
                    tag: "one".into(),
                    label: "One".into(),
                },
                MenuItem {
                    tag: "two".into(),
                    label: "Two".into(),
                },
            ],
            selected: 0,
        }
    }

    fn drive(anchor: MenuAnchor, event: Event, cursor: mouse::Cursor) -> Vec<Msg> {
        let content = Content::with_text("alpha\nbeta\ngamma");
        let mut editor = RichTextEditor::new(&content, ContentVersion::new(1, 0))
            .width(Length::Fixed(400.0))
            .height(Length::Fixed(300.0))
            .padding(16.0)
            .on_action(Msg::Act)
            .menu(Some(menu(anchor)))
            .on_menu(Msg::Menu);
        let renderer = iced_test::futures::futures::executor::block_on(
            <iced::Renderer as Headless>::new(Font::DEFAULT, Pixels(16.0), Some("tiny-skia")),
        )
        .expect("headless renderer");
        let mut tree = widget::Tree::new(&editor as &dyn Widget<_, Theme, iced::Renderer>);
        let limits = layout::Limits::new(Size::ZERO, Size::new(400.0, 300.0));
        let node = editor.layout(&mut tree, &renderer, &limits);
        let viewport = Rectangle::with_size(Size::new(400.0, 300.0));
        let mut clipboard = clipboard::Null;
        let mut messages = Vec::new();
        let mut shell = Shell::new(&mut messages);
        editor.update(
            &mut tree,
            &event,
            Layout::new(&node),
            cursor,
            &renderer,
            &mut clipboard,
            &mut shell,
            &viewport,
        );
        messages
    }

    #[test]
    fn a_press_outside_the_widget_dismisses_the_menu() {
        let messages = drive(
            MenuAnchor::Line(1),
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            mouse::Cursor::Available(Point::new(-30.0, -30.0)),
        );
        assert_eq!(messages, [Msg::Menu(MenuEvent::Dismiss)]);
    }

    #[test]
    fn a_window_blur_dismisses_the_menu() {
        let messages = drive(
            MenuAnchor::Line(1),
            Event::Window(window::Event::Unfocused),
            mouse::Cursor::Unavailable,
        );
        assert_eq!(messages, [Msg::Menu(MenuEvent::Dismiss)]);
    }

    #[test]
    fn a_pointer_straying_from_a_line_menu_leaves_it_open() {
        // The whole reason the menu is up is that the reader clicked for it.
        // Reading the panel means moving off the handle, and moving off the
        // handle used to take the panel away with it.
        let strayed = drive(
            MenuAnchor::Line(1),
            Event::Mouse(mouse::Event::CursorMoved {
                position: Point::new(390.0, 295.0),
            }),
            mouse::Cursor::Available(Point::new(390.0, 295.0)),
        );
        assert!(
            !strayed.contains(&Msg::Menu(MenuEvent::Dismiss)),
            "{strayed:?}"
        );
    }

    #[test]
    fn a_pointer_near_the_panel_keeps_the_menu_and_hover_selects() {
        // Just under the anchor line, inside the panel: the move selects a
        // row rather than dismissing.
        let messages = drive(
            MenuAnchor::Line(1),
            Event::Mouse(mouse::Event::CursorMoved {
                position: Point::new(60.0, 90.0),
            }),
            mouse::Cursor::Available(Point::new(60.0, 90.0)),
        );
        assert!(
            !messages.contains(&Msg::Menu(MenuEvent::Dismiss)),
            "{messages:?}"
        );
    }

    #[test]
    fn the_caret_palette_never_follows_the_mouse() {
        let messages = drive(
            MenuAnchor::Caret,
            Event::Mouse(mouse::Event::CursorMoved {
                position: Point::new(390.0, 295.0),
            }),
            mouse::Cursor::Available(Point::new(390.0, 295.0)),
        );
        assert_eq!(messages, []);
    }
}

/// An open line-anchored menu OWNS the gutter: the "⋮⋮" that opened it stays
/// beside its own block instead of sliding to whatever line the pointer
/// drifted onto, which left the panel hanging off nothing.
mod gutter_anchoring {
    use super::*;
    use iced::advanced::clipboard;

    #[derive(Debug, Clone, PartialEq)]
    enum Msg {
        Act(Action),
        Menu(MenuEvent),
        Gutter(usize, GutterButton),
    }

    /// Presses the gutter handle beside LINE 0 while the pointer hovers line
    /// 2, and reports what the editor published.
    fn press_line_zero_handle(menu: Option<EditorMenu>) -> Vec<Msg> {
        let content = Content::with_text("alpha\nbeta\ngamma");
        // The padding must clear GUTTER_WIDTH or the buttons fall outside.
        let padding = 60.0;
        let mut editor = RichTextEditor::new(&content, ContentVersion::new(1, 0))
            .width(Length::Fixed(400.0))
            .height(Length::Fixed(300.0))
            .padding(padding)
            .on_action(Msg::Act)
            .on_gutter(|line, button| Some(Msg::Gutter(line, button)))
            .menu(menu)
            .on_menu(Msg::Menu);
        let renderer = headless_renderer();
        let mut tree = widget::Tree::new(&editor as &dyn Widget<_, Theme, iced::Renderer>);
        let limits = layout::Limits::new(Size::ZERO, Size::new(400.0, 300.0));
        let node = editor.layout(&mut tree, &renderer, &limits);
        let bounds = Layout::new(&node).bounds();
        let text_bounds = bounds.shrink(padding);

        // The pointer is parked on a DIFFERENT block than the menu's.
        {
            let state = tree
                .state
                .downcast_mut::<State<text::highlighter::PlainText>>();
            state.hover_line = Some(2);
        }
        let state = tree
            .state
            .downcast_ref::<State<text::highlighter::PlainText>>();
        let row = editor.gutter_row(state, text_bounds, 0);
        let [_, (_, handle)] = gutter_buttons(text_bounds, row);
        let press = handle.center();

        let mut clipboard = clipboard::Null;
        let mut messages = Vec::new();
        let mut shell = Shell::new(&mut messages);
        editor.update(
            &mut tree,
            &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Layout::new(&node),
            mouse::Cursor::Available(press),
            &renderer,
            &mut clipboard,
            &mut shell,
            &Rectangle::with_size(Size::new(400.0, 300.0)),
        );
        messages
    }

    #[test]
    fn an_open_line_menu_keeps_the_gutter_on_its_own_line() {
        let menu = EditorMenu {
            anchor: MenuAnchor::Line(0),
            items: vec![MenuItem {
                tag: "delete".into(),
                label: "Delete".into(),
            }],
            selected: 0,
        };
        let messages = press_line_zero_handle(Some(menu));
        assert!(
            messages.contains(&Msg::Gutter(0, GutterButton::Handle)),
            "{messages:?}"
        );
    }

    #[test]
    fn with_no_menu_up_the_gutter_follows_the_pointer_instead() {
        // The contrast that proves the test above measures the anchoring and
        // not the geometry: hovering line 2 puts no button beside line 0.
        let messages = press_line_zero_handle(None);
        assert!(
            !messages
                .iter()
                .any(|message| matches!(message, Msg::Gutter(..))),
            "{messages:?}"
        );
    }
}
