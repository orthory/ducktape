use super::*;
use ducktape_view_guest::slots;

fn object_cells(key: &str, name: String, size: String, object: String) -> wire::Node {
    let cell = |field: &str, value: String, width| {
        native::sized(
            native::text_options(
                native::text(format!("{key}/{field}"), value),
                wire::TextOptions {
                    wrapping: Some(wire::Wrapping::None),
                    ..Default::default()
                },
            ),
            Some(width),
            None,
        )
    };
    native::row(
        key,
        [
            cell("name", name, wire::Length::Fill),
            cell("size", size, wire::Length::Fixed(72.)),
            cell("object", object, wire::Length::Fixed(92.)),
        ],
    )
}

impl FilesView {
    pub(super) fn breadcrumb(
        &self,
        key: String,
        open: impl Fn(String) -> Message + Clone + 'static,
    ) -> wire::Node {
        let mut root = native::button(
            format!("{key}/root"),
            "duckfs",
            Some(slots::message(open("/".into()))),
            wire::ButtonPreset::Text,
        );
        if let wire::Node::Button { label, .. } = &mut root {
            *label = Some("Go to the duckfs root".into());
        }
        native::row(
            &key,
            [
                root,
                native::text(format!("{key}/path"), self.path.clone()),
                native::text(
                    format!("{key}/count"),
                    crate::host::fs_counts_summary(self.connected, self.listed, &self.entries),
                ),
            ],
        )
    }

    pub(super) fn folder_row(
        &self,
        key: String,
        open: impl Fn(String) -> Message + Clone + 'static,
        entry: crate::host::FsEntry,
    ) -> wire::Node {
        let mut button = native::button(
            key,
            format!("{}/", entry.name),
            Some(slots::message(open(entry.path))),
            wire::ButtonPreset::Text,
        );
        if let wire::Node::Button { label, width, .. } = &mut button {
            *label = Some("Open directory".into());
            *width = Some(wire::Length::Fill);
        }
        button
    }

    pub(super) fn object_table_header(&self, key: String) -> wire::Node {
        object_cells(&key, "Name".into(), "Size".into(), "Object".into())
    }

    pub(super) fn object_row(
        &self,
        key: String,
        open_directory: impl Fn(String) -> Message + Clone + 'static,
        open_file: impl Fn(String) -> Message + Clone + 'static,
        entry: crate::host::FsEntry,
        selected: bool,
    ) -> wire::Node {
        let directory = entry.kind == "dir";
        let size = if directory {
            "—".into()
        } else {
            crate::host::size_label(entry.size)
        };
        let object = if entry.object.is_empty() {
            "—".into()
        } else {
            entry.object
        };
        let content = object_cells(&format!("{key}/cells"), entry.name, size, object);
        let action = if directory {
            open_directory(entry.path)
        } else {
            open_file(entry.path)
        };
        let mut button = native::button_child(
            key,
            content,
            Some(slots::message(action)),
            wire::ButtonPreset::Text,
        );
        if let wire::Node::Button {
            label,
            checked,
            width,
            ..
        } = &mut button
        {
            *label = Some(
                if directory {
                    "Open directory"
                } else {
                    "Show object"
                }
                .into(),
            );
            *checked = Some(selected && !directory);
            *width = Some(wire::Length::Fill);
        }
        button
    }

    pub(super) fn object_panel(&self, key: String) -> wire::Node {
        let entry = &self.preview_entry;
        let directory = entry.kind == "dir";
        let object = if entry.object.is_empty() {
            "—"
        } else {
            &entry.object
        };
        let size = if directory {
            "—".into()
        } else {
            crate::host::size_label(entry.size)
        };
        let content = native::column(
            format!("{key}/facts"),
            [
                native::heading(format!("{key}/title"), "Object"),
                native::text(
                    format!("{key}/kind"),
                    if directory { "DIR" } else { "FILE" },
                ),
                native::text(format!("{key}/name"), entry.name.clone()),
                native::text(format!("{key}/path"), entry.path.clone()),
                native::text(format!("{key}/id-label"), "object id"),
                native::text(format!("{key}/id"), object),
                native::text(format!("{key}/size-label"), "size"),
                native::text(format!("{key}/size"), size),
            ],
        );
        native::sized(
            native::scroll(key, content),
            Some(wire::Length::Fixed(self.object_width as f32)),
            Some(wire::Length::Fill),
        )
    }
}
