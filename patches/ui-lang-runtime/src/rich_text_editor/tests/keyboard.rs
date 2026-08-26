use super::*;

#[test]
fn a_key_binding_override_parts_plain_enter_from_shift_enter() {
    use iced::advanced::clipboard;
    use iced::advanced::renderer::Headless;
    use iced::keyboard::key::{Code, Physical};
    use iced::keyboard::{Key, Location, Modifiers};

    let content = Content::new();
    let mut editor = RichTextEditor::new(&content, ContentVersion::new(1, 0))
        .width(Length::Fixed(120.0))
        .height(Length::Fixed(80.0))
        .on_action(|action| action)
        .key_binding(|press| {
            let plain_enter =
                matches!(press.key, Key::Named(key::Named::Enter)) && !press.modifiers.shift();
            if plain_enter {
                // The application owns plain Enter (e.g. submit) — the key
                // bubbles instead of editing the document.
                None
            } else {
                default_key_binding(press)
            }
        });
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
    let mut messages: Vec<Action> = Vec::new();

    let enter_key = Key::Named(key::Named::Enter);
    let enter = |modifiers: Modifiers| {
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: enter_key.clone(),
            modified_key: enter_key.clone(),
            physical_key: Physical::Code(Code::Enter),
            location: Location::Standard,
            modifiers,
            text: None,
            repeat: false,
        })
    };

    let mut shell = Shell::new(&mut messages);
    editor.update(
        &mut tree,
        &enter(Modifiers::empty()),
        Layout::new(&node),
        mouse::Cursor::Unavailable,
        &renderer,
        &mut clipboard,
        &mut shell,
        &viewport,
    );
    assert!(
        messages.is_empty(),
        "plain Enter must bubble to the application, got {messages:?}"
    );

    let mut shell = Shell::new(&mut messages);
    editor.update(
        &mut tree,
        &enter(Modifiers::SHIFT),
        Layout::new(&node),
        mouse::Cursor::Unavailable,
        &renderer,
        &mut clipboard,
        &mut shell,
        &viewport,
    );
    assert_eq!(
        messages,
        [Action::Edit(text_editor::Action::Edit(Edit::Enter))],
        "shift+enter delegates to the stock newline binding"
    );
}

#[test]
fn command_up_and_down_move_or_select_to_the_document_edges() {
    use iced::keyboard::key::Named;

    // `Modifiers::macos_command` is false off macOS, so the decision is
    // driven directly and the stock binding is left to iced.
    assert!(
        matches!(
            document_edge_binding(Named::ArrowDown, true, false),
            Some(Binding::Move(Motion::DocumentEnd))
        ),
        "Cmd+Down moves to the end of the document"
    );
    assert!(
        matches!(
            document_edge_binding(Named::ArrowUp, true, false),
            Some(Binding::Move(Motion::DocumentStart))
        ),
        "Cmd+Up moves to the start of the document"
    );
    assert!(
        matches!(
            document_edge_binding(Named::ArrowDown, true, true),
            Some(Binding::Select(Motion::DocumentEnd))
        ),
        "Shift+Cmd+Down selects to the end of the document"
    );
    assert!(
        matches!(
            document_edge_binding(Named::ArrowUp, true, true),
            Some(Binding::Select(Motion::DocumentStart))
        ),
        "Shift+Cmd+Up selects to the start of the document"
    );
    assert!(
        document_edge_binding(Named::ArrowDown, false, false).is_none(),
        "a plain arrow keeps iced's line motion"
    );
    assert!(
        document_edge_binding(Named::ArrowLeft, true, false).is_none(),
        "Cmd+Left keeps iced's Home remap"
    );

    // The motion is a native one, so the content it is applied to lands on
    // the last line.
    let mut content: Content = Content::with_text("first\nsecond\nthird");
    content.perform(text_editor::Action::Move(Motion::DocumentEnd));
    assert_eq!(content.cursor().position, Position { line: 2, column: 5 });
}
