use super::*;
use ducktape_view_guest::slots;

impl FilesView {
    pub(crate) fn view(&self) -> wire::Node {
        let mut children = Vec::new();
        if *self.derived_draft_parked() {
            children.push(native::column(
                "FilesView/parked-draft",
                [
                    native::text("FilesView/draft-label", "Unsaved changes to:"),
                    native::text("FilesView/draft-path", self.draft_path.clone()),
                    native::text(
                        "FilesView/draft-hint",
                        "Return to this file to continue editing.",
                    ),
                    native::button(
                        "FilesView/discard-draft",
                        "Discard unsaved changes",
                        Some(slots::message(Message::DiscardDraft(self.draft_id))),
                        wire::ButtonPreset::Secondary,
                    ),
                ],
            ));
        }
        if !self.notice.is_empty() {
            children.push(native::text("FilesView/notice", self.notice.clone()));
        }
        if self.connected && self.omitted > 0 {
            children.push(native::row(
                "FilesView/omissions",
                [
                    native::text("FilesView/display-omitted", self.omitted.to_string()),
                    native::text("FilesView/omission-label", "rows are not shown."),
                ],
            ));
        }
        let viewport = || {
            slots::handler::<(f32, f32), Message>(Box::new(|(width, height)| {
                Some(Message::ViewportChanged(width.into(), height.into()))
            }))
        };
        children.push(wire::Node::Sensor {
            key: "FilesView/viewport".into(),
            reset: None,
            on_show: Some(viewport()),
            on_resize: Some(viewport()),
            on_hide: None,
            anticipate: None,
            delay: None,
            child: Box::new(self.files_screen("FilesView/root/FilesScreen@2164".into())),
        });
        native::sized(
            native::column("FilesView/root", children),
            Some(wire::Length::Fill),
            Some(wire::Length::Fill),
        )
    }
}
