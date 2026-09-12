impl PagesView {
    pub(crate) fn view(&self) -> wire::Node {
        let palette = self.palette();
        wire::Node::Sensor {
            key: format!("{}/@sensor:721", "PagesView"),
            reset: None,
            on_show: Some(
                ::ducktape_view_guest::slots::handler::<(f32, f32), Message>(Box::new({
                    let route =
                        move |size: (f64, f64)| Message::PagesViewportChanged(size.0, size.1);
                    move |sent: (f32, f32)| Some(route((f64::from(sent.0), f64::from(sent.1))))
                })),
            ),
            on_resize: Some(
                ::ducktape_view_guest::slots::handler::<(f32, f32), Message>(Box::new({
                    let route =
                        move |size: (f64, f64)| Message::PagesViewportChanged(size.0, size.1);
                    move |sent: (f32, f32)| Some(route((f64::from(sent.0), f64::from(sent.1))))
                })),
            ),
            on_hide: None,
            anticipate: None,
            delay: None,
            child: Box::new({
                let node_scope = format!("{}/root", "PagesView");
                wire::Node::Container {
                    shadow: wire::Shadow {
                        color: None,
                        x: None,
                        y: None,
                        blur: None,
                    },
                    max_width: None,
                    max_height: None,
                    clip: false,
                    key: node_scope.clone(),
                    width: Some(wire::Length::Fill),
                    height: Some(wire::Length::Fill),
                    padding: None,
                    align_x: None,
                    align_y: None,
                    background: (Some(palette.colors[2])).map(wire::Background::Color),
                    border: None,
                    snap: None,
                    content: Box::new({
                        let node_scope = format!("{}/pages", node_scope);
                        self.pages(palette, node_scope.clone())
                    }),
                }
            }),
        }
    }
}
