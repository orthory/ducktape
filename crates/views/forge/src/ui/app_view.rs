use super::*;
impl ForgeView {
    pub fn view(&self) -> wire::Node {
        let viewport = || ducktape_view_guest::slots::handler::<(f32, f32), Message>(
            Box::new(|(width, height)| Some(Message::ViewportChanged(width.into(), height.into())))
        );
        let failed_without_content = !self.host_error.is_empty() && self.repos.is_empty();
        let content = if failed_without_content {
            self.unavailable("ForgeView/error".into())
        } else {
            self.render_forge("ForgeView/forge".into())
        };
        wire::Node::Sensor {
            key: "ForgeView/viewport".into(),
            reset: None,
            on_show: Some(viewport()),
            on_resize: Some(viewport()),
            on_hide: None,
            anticipate: None,
            delay: None,
            child: Box::new(native::sized(content, Some(wire::Length::Fill), Some(wire::Length::Fill))),
        }
    }
}
