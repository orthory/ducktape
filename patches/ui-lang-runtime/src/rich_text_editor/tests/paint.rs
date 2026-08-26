use super::*;

fn paint(content: &Content, focus: Option<Focus>) -> Vec<u8> {
    use iced::advanced::renderer::Headless as _;

    let size = Size::new(160.0, 80.0);
    let mut editor = RichTextEditor::<_, ()>::new(content, ContentVersion::new(1, 0))
        .width(Length::Fixed(size.width))
        .height(Length::Fixed(size.height))
        .style(|_, _| text_editor::Style {
            background: Color::WHITE.into(),
            border: iced::Border::default(),
            placeholder: Color::WHITE,
            value: Color::WHITE,
            selection: Color::from_rgb(1.0, 0.0, 0.0),
        });
    let mut renderer = headless_renderer();
    let mut tree = widget::Tree::new(&editor as &dyn Widget<_, Theme, iced::Renderer>);
    tree.state
        .downcast_mut::<State<text::highlighter::PlainText>>()
        .focus = focus;
    let node = editor.layout(&mut tree, &renderer, &layout::Limits::new(Size::ZERO, size));
    editor.draw(
        &tree,
        &mut renderer,
        &Theme::Light,
        &renderer::Style::default(),
        Layout::new(&node),
        mouse::Cursor::Unavailable,
        &Rectangle::with_size(size),
    );
    renderer.screenshot(
        Size::new(size.width as u32, size.height as u32),
        1.0,
        Color::WHITE,
    )
}

fn red_pixels(pixels: &[u8]) -> usize {
    pixels
        .chunks_exact(4)
        .filter(|px| px[0] == 255 && px[1] == 0 && px[2] == 0)
        .count()
}

/// The Find bar moves keyboard focus to its input and hands the match to the
/// editor as its selection. Every mainstream editor keeps painting that
/// selection while another control owns focus; a selection nobody can see
/// makes Find look like it did nothing.
#[test]
fn an_unfocused_editor_keeps_painting_its_selection() {
    let mut content = Content::with_text("alpha beta");
    content.move_to(Cursor {
        position: Position { line: 0, column: 5 },
        selection: Some(Position { line: 0, column: 0 }),
    });

    let focused = red_pixels(&paint(&content, Some(Focus::now())));
    assert!(focused > 0, "a focused selection has to paint");

    let unfocused = red_pixels(&paint(&content, None));
    assert_eq!(
        unfocused, focused,
        "losing focus must not hide the selection"
    );
}
