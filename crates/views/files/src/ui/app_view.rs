use super::*;
impl super::FilesView {
    pub(crate) fn view(&self) -> wire::Node {
        let mut children: Vec<wire::Node> = Vec::new();
        if *self.derived_draft_parked() {
            children.push({
                let mut children: Vec<wire::Node> = vec![
                    native::text(
                        format!("{}/@text:384", "FilesView"),
                        "Unsaved changes to:".to_owned().to_string(),
                    ),
                    native::text(
                        format!("{}/@text:385", "FilesView"),
                        self.draft_path.to_owned().to_string(),
                    ),
                    native::text(
                        format!("{}/@text:386", "FilesView"),
                        "Return to this file to continue editing."
                            .to_owned()
                            .to_string(),
                    ),
                    native::button(
                        format!("{}/@button:387", "FilesView"),
                        String::from("Discard unsaved changes"),
                        Some(::ducktape_view_guest::slots::message(
                            Message::DiscardDraft(self.draft_id),
                        )),
                        wire::ButtonPreset::Secondary,
                    ),
                ];
                native::spaced(
                    native::sized(
                        native::column(format!("{}/@layout:383", "FilesView"), children),
                        Some(wire::Length::Fill),
                        None,
                    ),
                    4.0f32,
                )
            });
        }
        if !self.notice.is_empty() {
            children.push(native::text(
                format!("{}/@text:389", "FilesView"),
                self.notice.to_owned().to_string(),
            ));
        }
        if self.omitted > 0 {
            children.push({
                let mut children: Vec<wire::Node> = vec![
                    {
                        let node_scope = format!("{}/display-omitted", "FilesView");
                        native::text(node_scope.clone(), self.omitted.to_string())
                    },
                    native::text(
                        format!("{}/@text:393", "FilesView"),
                        "rows are not shown.".to_owned().to_string(),
                    ),
                ];
                native::spaced(
                    native::row(format!("{}/@layout:391", "FilesView"), children),
                    4.0f32,
                )
            });
        }
        children.push(wire::Node::Sensor {
            key: format!("{}/@sensor:394", "FilesView"),
            reset: None,
            on_show: Some(
                ::ducktape_view_guest::slots::handler::<(f32, f32), Message>(Box::new({
                    let route = move |size: (f64, f64)| Message::ViewportChanged(size.0, size.1);
                    move |sent: (f32, f32)| Some(route((f64::from(sent.0), f64::from(sent.1))))
                })),
            ),
            on_resize: Some(
                ::ducktape_view_guest::slots::handler::<(f32, f32), Message>(Box::new({
                    let route = move |size: (f64, f64)| Message::ViewportChanged(size.0, size.1);
                    move |sent: (f32, f32)| Some(route((f64::from(sent.0), f64::from(sent.1))))
                })),
            ),
            on_hide: None,
            anticipate: None,
            delay: None,
            child: Box::new({
                let node_scope = format!("{}/root", "FilesView");
                native::sized(
                    native::container(
                        node_scope.clone(),
                        self.files_screen(format!("{}/FilesScreen@2164", node_scope)),
                    ),
                    Some(wire::Length::Fill),
                    Some(wire::Length::Fill),
                )
            }),
        });
        native::sized(
            native::column(format!("{}/@layout:381", "FilesView"), children),
            Some(wire::Length::Fill),
            Some(wire::Length::Fill),
        )
    }
}
