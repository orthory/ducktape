use super::*;
impl super::ForgeView {
    pub(crate) fn view(&self) -> wire::Node {
        let mut children: Vec<wire::Node> = vec![
            wire::Node::Sensor { key : format!("{}/@sensor:655", "ForgeView"), reset :
            None, on_show : Some(::ducktape_view_guest::slots::handler:: < (f32, f32),
            Message, > (Box::new({ let route = move | size : (f64, f64) |
            Message::ViewportChanged(size.0, size.1,); move | sent : (f32, f32) |
            Some(route((f64::from(sent.0), f64::from(sent.1))),) }),),), on_resize :
            Some(::ducktape_view_guest::slots::handler:: < (f32, f32), Message, >
            (Box::new({ let route = move | size : (f64, f64) |
            Message::ViewportChanged(size.0, size.1,); move | sent : (f32, f32) |
            Some(route((f64::from(sent.0), f64::from(sent.1))),) }),),), on_hide : None,
            anticipate : None, delay : None, child : Box::new(wire::Node::Space { width :
            Some(wire::Length::Fill), height : Some(wire::Length::Fixed(0.0f32)), }), }
        ];
        if !self.host_error.is_empty() && self.repos.is_empty() {
            children
                .push(
                    native::padded(
                        native::sized(
                            native::container(
                                format!("{}/@container:658", "ForgeView"),
                                self
                                    .unavailable(
                                        format!("{}/EmptyState@4442", "ForgeView"),
                                    ),
                            ),
                            Some(wire::Length::Fill),
                            Some(wire::Length::Fill),
                        ),
                        wire::Edges {
                            top: 22.0f32,
                            right: 22.0f32,
                            bottom: 22.0f32,
                            left: 22.0f32,
                        },
                    ),
                );
        }
        if self.host_error.is_empty() || !self.repos.is_empty() {
            children
                .push({
                    let mut children: Vec<wire::Node> = vec![
                        { let node_scope = format!("{}/forge", "ForgeView"); self
                        .render_forge(node_scope.clone()) }
                    ];
                    native::sized(
                        native::column(format!("{}/@layout:666", "ForgeView"), children),
                        Some(wire::Length::Fill),
                        Some(wire::Length::Fill),
                    )
                });
        }
        native::sized(
            native::column(format!("{}/@layout:654", "ForgeView"), children),
            Some(wire::Length::Fill),
            Some(wire::Length::Fill),
        )
    }
}
