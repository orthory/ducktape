use super::*;
impl super::ForgeView {
    pub(crate) fn view(&self) -> ::ducktape_view_guest::wire::Node {
        let palette = self.palette();
        {
            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
            children.push(::ducktape_view_guest::wire::Node::Sensor {
                key: format!("{}/@sensor:655", "ForgeView"),
                reset: None,
                on_show: Some(
                    ::ducktape_view_guest::slots::handler::<(f32, f32), Message>(Box::new({
                        let route =
                            move |size: (f64, f64)| Message::ViewportChanged(size.0, size.1);
                        move |sent: (f32, f32)| Some(route((f64::from(sent.0), f64::from(sent.1))))
                    })),
                ),
                on_resize: Some(
                    ::ducktape_view_guest::slots::handler::<(f32, f32), Message>(Box::new({
                        let route =
                            move |size: (f64, f64)| Message::ViewportChanged(size.0, size.1);
                        move |sent: (f32, f32)| Some(route((f64::from(sent.0), f64::from(sent.1))))
                    })),
                ),
                on_hide: None,
                anticipate: None,
                delay: None,
                child: Box::new(::ducktape_view_guest::wire::Node::Space {
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: Some(::ducktape_view_guest::wire::Length::Fixed((0.0) as f32)),
                }),
            });
            if (!(self.host_error).is_empty()) && (self.repos).is_empty() {
                children.push(::ducktape_view_guest::wire::Node::Container {
                    shadow: ::ducktape_view_guest::wire::Shadow {
                        color: None,
                        x: None,
                        y: None,
                        blur: None,
                    },
                    max_width: None,
                    max_height: None,
                    clip: false,
                    key: format!("{}/@container:658", "ForgeView"),
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: Some(::ducktape_view_guest::wire::Length::Fill),
                    padding: Some(::ducktape_view_guest::wire::Edges {
                        top: (22.0) as f32,
                        right: (22.0) as f32,
                        bottom: (22.0) as f32,
                        left: (22.0) as f32,
                    }),
                    align_x: None,
                    align_y: None,
                    background: (None).map(::ducktape_view_guest::wire::Background::Color),
                    border: None,
                    snap: None,
                    content: Box::new(
                        self.render_empty_state_0(
                            palette,
                            format!("{}/EmptyState@4442", "ForgeView"),
                        ),
                    ),
                });
            }
            if (self.host_error).is_empty() || (!(self.repos).is_empty()) {
                children.push({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children.push({
                        let node_scope = format!("{}/forge", "ForgeView");
                        self.render_forge(palette, node_scope.clone())
                    });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:666", "ForgeView"),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: None,
                        padding: None,
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: Some(::ducktape_view_guest::wire::Length::Fill),
                        align: None,
                        background: None,
                        border: None,
                        children: children,
                    }
                });
            }
            ::ducktape_view_guest::wire::Node::Linear {
                max_width: None,
                clip: false,
                key: format!("{}/@layout:654", "ForgeView"),
                wrap: None,
                axis: ::ducktape_view_guest::wire::Axis::Column,
                spacing: None,
                padding: None,
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: Some(::ducktape_view_guest::wire::Length::Fill),
                align: None,
                background: None,
                border: None,
                children: children,
            }
        }
    }
}
