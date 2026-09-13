//! WASM composition for the host's native gpui-kit controls.
//! Guests supply content, identity and actions. The native theme owns appearance.

use crate::wire::{self, Axis, ButtonContent, ButtonPreset, Length, Node};

pub fn text(key: impl Into<String>, content: impl Into<String>) -> Node {
    Node::Text {
        key: key.into(),
        content: content.into(),
        options: Default::default(),
        size: None,
        color: None,
        font: Default::default(),
        width: None,
        align_x: None,
    }
}

pub fn heading(key: impl Into<String>, content: impl Into<String>) -> Node {
    text_size(text(key, content), 20.)
}

pub fn text_size(mut node: Node, size: f32) -> Node {
    let Node::Text { size: value, .. } = &mut node else {
        panic!("text_size requires text")
    };
    *value = Some(size);
    node
}

pub fn text_options(mut node: Node, options: wire::TextOptions) -> Node {
    let Node::Text { options: value, .. } = &mut node else {
        panic!("text_options requires text")
    };
    *value = options;
    node
}

pub fn row(key: impl Into<String>, children: impl IntoIterator<Item = Node>) -> Node {
    linear(key.into(), Axis::Row, children.into_iter().collect())
}

pub fn column(key: impl Into<String>, children: impl IntoIterator<Item = Node>) -> Node {
    linear(key.into(), Axis::Column, children.into_iter().collect())
}

fn linear(key: String, axis: Axis, children: Vec<Node>) -> Node {
    Node::Linear {
        key,
        axis,
        children,
        spacing: Some(8.),
        padding: None,
        width: Some(Length::Fill),
        height: None,
        max_width: None,
        clip: false,
        wrap: None,
        align: None,
        background: None,
        border: None,
    }
}

pub fn container(key: impl Into<String>, child: Node) -> Node {
    Node::Container {
        key: key.into(),
        content: Box::new(child),
        shadow: Default::default(),
        max_width: None,
        max_height: None,
        clip: false,
        width: Some(Length::Fill),
        height: None,
        padding: None,
        align_x: None,
        align_y: None,
        background: None,
        border: None,
        snap: None,
    }
}

pub fn button(
    key: impl Into<String>,
    label: impl Into<String>,
    on_press: Option<u32>,
    preset: ButtonPreset,
) -> Node {
    control(
        key.into(),
        ButtonContent::Label(label.into()),
        on_press,
        preset,
    )
}

pub fn button_child(
    key: impl Into<String>,
    child: Node,
    on_press: Option<u32>,
    preset: ButtonPreset,
) -> Node {
    control(
        key.into(),
        ButtonContent::Child(Box::new(child)),
        on_press,
        preset,
    )
}

fn control(
    key: String,
    content: ButtonContent,
    on_press: Option<u32>,
    preset: ButtonPreset,
) -> Node {
    let label = match &content {
        ButtonContent::Label(label) => Some(label.clone()),
        ButtonContent::Child(_) => None,
    };
    Node::Button {
        key,
        content,
        on_press,
        label,
        checked: None,
        expanded: None,
        description: None,
        width: None,
        height: None,
        padding: None,
        style: wire::ButtonStyle {
            preset,
            ..Default::default()
        },
    }
}

pub fn input(
    key: impl Into<String>,
    placeholder: impl Into<String>,
    value: impl Into<String>,
    on_input: u32,
    on_submit: Option<u32>,
) -> Node {
    let placeholder = placeholder.into();
    Node::Input {
        key: key.into(),
        options: wire::InputOptions {
            label: placeholder.clone(),
            ..Default::default()
        },
        placeholder,
        value: value.into(),
        on_input,
        on_submit,
        width: Some(Length::Fill),
        secure: false,
        style: Default::default(),
    }
}

pub fn scroll(key: impl Into<String>, child: Node) -> Node {
    Node::Scroll {
        key: key.into(),
        content: Box::new(child),
        direction: wire::ScrollDirection::Vertical,
        width: Some(Length::Fill),
        height: Some(Length::Fill),
        on_scroll: None,
        virtual_rows: false,
        bar_hidden: false,
        bar_width: None,
        bar_margin: None,
        scroller_width: None,
        bar_spacing: None,
        anchor_x: Default::default(),
        anchor_y: Default::default(),
        auto_scroll: false,
        background: None,
        border: None,
    }
}

/// Apply layout dimensions to a composition node; input height remains native.
pub fn sized(mut node: Node, width: Option<Length>, height: Option<Length>) -> Node {
    match &mut node {
        Node::Linear {
            width: w,
            height: h,
            ..
        }
        | Node::Container {
            width: w,
            height: h,
            ..
        }
        | Node::Button {
            width: w,
            height: h,
            ..
        }
        | Node::Scroll {
            width: w,
            height: h,
            ..
        }
        | Node::Svg {
            width: w,
            height: h,
            ..
        }
        | Node::Image {
            width: w,
            height: h,
            ..
        }
        | Node::Space {
            width: w,
            height: h,
        } => {
            *w = width;
            *h = height;
        }
        Node::Text {
            width: w, options, ..
        } => {
            *w = width;
            options.height = height;
        }
        Node::Input { width: w, .. } => {
            assert!(
                height.is_none(),
                "input height belongs to the native control"
            );
            *w = width;
        }
        _ => panic!("sized requires a layout, text, button, image or input"),
    }
    node
}

pub fn padded(mut node: Node, padding: wire::Edges) -> Node {
    match &mut node {
        Node::Linear { padding: value, .. }
        | Node::Container { padding: value, .. }
        | Node::Button { padding: value, .. } => *value = Some(padding),
        Node::Input { options, .. } => options.padding = Some(padding),
        _ => panic!("padded requires a layout, button or input"),
    }
    node
}

pub fn spaced(mut node: Node, spacing: f32) -> Node {
    let Node::Linear { spacing: value, .. } = &mut node else {
        panic!("spaced requires a row or column")
    };
    *value = Some(spacing);
    node
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controls_defer_appearance_to_the_native_kit_and_keep_routes() {
        let node = button("save", "Save", Some(7), ButtonPreset::Primary);
        let Node::Button {
            style, on_press, ..
        } = node
        else {
            panic!("button")
        };
        assert_eq!(on_press, Some(7));
        assert_eq!(style, wire::ButtonStyle::default());
        let Node::Input {
            options,
            style,
            on_input,
            ..
        } = input("name", "Name", "draft", 9, None)
        else {
            panic!("input")
        };
        assert_eq!(on_input, 9);
        assert_eq!(options.label, "Name");
        assert_eq!(*style, wire::InputStyle::default());
    }
}
