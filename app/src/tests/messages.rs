use super::*;

#[test]
fn the_message_timeline_virtualizes_under_an_end_anchored_scroll() {
    let chat = rust_tokens(super::connection::CHAT);
    assert!(chat.contains("Node::KeyedColumn") && chat.contains("virtual_row:"));
    assert!(
        chat.contains("virtual_row:Some(44.0f32)"),
        "the wire supplies a bounded row estimate"
    );
    assert!(chat.contains("anchor_y:") && chat.contains("ScrollAnchor::End"));
    assert!(
        chat.contains("message.view_key"),
        "prepend and confirmation must preserve row identity"
    );
    assert!(
        chat.contains("copy_anchor_seq")
            && chat.contains("copy_head_seq")
            && chat.contains("copy_surface")
    );
    let native = rust_tokens(include_str!("../view_tree.rs"));
    assert!(
        native.contains("gpui_kit::list("),
        "the host uses native viewport virtualization"
    );
    assert!(
        native.contains(".splice(") && native.contains(".remeasure_items("),
        "row identity updates preserve measurements"
    );
}

#[test]
fn the_message_line_is_one_rich_text_paragraph() {
    for source in [
        include_str!("../../../crates/views/chat/src/ui/chat.rs"),
        include_str!("../../../crates/views/forge/src/ui/kit.rs"),
    ] {
        let source = rust_tokens(source);
        assert!(source.contains("Node::RichText"));
        assert!(source.contains("mention_link"));
        assert!(source.contains("underline:true") || source.contains("underline:link.is_some()"));
        assert!(source.contains("on_link:"));
        assert!(
            source.contains("Wrapping::WordOrGlyph"),
            "a paragraph wraps as one text layout"
        );
    }
    let native = rust_tokens(include_str!("../view_tree.rs"));
    assert!(
        native.contains("StyledText::new("),
        "one native paragraph owns wrapping and links"
    );
    assert!(
        native.contains(".on_click("),
        "the paragraph's link indices are actionable"
    );
}

#[test]
fn the_mention_plate_leaves_space_before_and_after_the_token() {
    use gpui_kit::{FontWeight, TextRun, font, px};
    let cx = crate::frame_probe::headless_context();
    let blocks = chat::client::paragraph_blocks("before<@3>after");
    let spans = &blocks[0].spans;
    let mut text = String::new();
    let mut runs = Vec::new();
    let mut bounds = Vec::new();
    for span in spans {
        let content = if span.mention.is_empty() {
            &span.plain
        } else {
            &span.mention
        };
        let start = text.len();
        text.push_str(content);
        bounds.push(start..text.len());
        let mut face = font("Geist");
        if !span.mention.is_empty() {
            face.weight = FontWeight::MEDIUM;
        }
        runs.push(TextRun {
            len: content.len(),
            font: face,
            ..Default::default()
        });
    }
    let shaper = gpui_kit::WindowTextSystem::new(cx.text_system().clone());
    let line = shaper.shape_line(text.into(), px(13.5), &runs, None);
    let padding = px(1.);
    let leading = line.x_for_index(bounds[2].start) - padding - line.x_for_index(bounds[0].end);
    let trailing = line.x_for_index(bounds[4].start) - line.x_for_index(bounds[2].end) - padding;
    assert!(
        leading >= px(1.5),
        "leading whitespace remains visible: {leading:?}"
    );
    assert!(
        trailing >= px(1.5),
        "trailing whitespace remains visible: {trailing:?}"
    );
}

#[test]
fn the_edited_marker_reaches_every_row_it_annotates() {
    let components = super::connection::CHAT;
    let branches = super::connection::branches(components);
    let continuations: Vec<_> = branches
        .iter()
        .filter(|(guard, body, _)| {
            guard.contains("edited") && guard.contains("show_author") && body.contains("·edited")
        })
        .collect();
    assert!(
        !continuations.is_empty(),
        "continuation messages own an edited annotation"
    );
    assert!(
        branches
            .iter()
            .any(|(guard, body, _)| guard.contains("edited")
                && !guard.contains("show_author")
                && body.contains("·edited")),
        "thread roots own an edited annotation too"
    );
}
