impl PagesView {
    fn icon(
        &self,
        palette: Palette,
        scope: String,
        name: &str,
        size: f32,
    ) -> ducktape_view_guest::wire::Node {
        use ducktape_view_guest::wire::{Length, Node};
        let (hash, bytes) = ducktape_view_guest::slots::picture(crate::host::icon(name));
        Node::Svg {
            key: format!("{scope}/root"),
            hash,
            bytes,
            inherit_button_ink: false,
            label: None,
            color: Some(palette.colors[73]),
            hover: None,
            fit: None,
            rotation: None,
            opacity: None,
            width: Some(Length::Fixed(size)),
            height: Some(Length::Fixed(size)),
        }
    }
}
