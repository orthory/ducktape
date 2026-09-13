impl super::FilesView {
    pub(super) fn icon(
        &self,
        scope: String,
        name: &str,
        size: f32,
        media: &str,
    ) -> ducktape_view_guest::wire::Node {
        use ducktape_view_guest::wire::{Axis, Length, Node};
        let (hash, bytes) = ducktape_view_guest::slots::picture(crate::host::icon(name));
        Node::Linear {
            key: format!("{scope}/root"),
            axis: Axis::Column,
            spacing: None,
            padding: None,
            width: None,
            height: None,
            align: None,
            background: None,
            border: None,
            max_width: None,
            clip: false,
            wrap: None,
            children: vec![Node::Svg {
                key: format!("{scope}/{media}"),
                hash,
                bytes,
                inherit_button_ink: true,
                label: None,
                color: None,
                hover: None,
                fit: None,
                rotation: None,
                opacity: None,
                width: Some(Length::Fixed(size)),
                height: Some(Length::Fixed(size)),
            }],
        }
    }
}
