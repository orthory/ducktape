use super::*;
use ducktape_view_guest::slots;

impl FilesView {
    pub(super) fn confirm_delete(&self, key: String) -> wire::Node {
        let loading = *self.derived_loading();
        let cancel = if loading {
            None
        } else {
            Some(slots::message(Message::DisarmDeleteNow))
        };
        let delete = if loading {
            None
        } else {
            Some(slots::message(Message::DeleteSubmit))
        };
        native::sized(
            native::padded(
                native::column(
                    &key,
                    [
                        native::heading(format!("{key}/title"), "Delete this object"),
                        native::text(format!("{key}/target"), self.delete_target.clone()),
                        native::text(
                            format!("{key}/warning"),
                            "The committed object is removed from duckfs for every member. Earlier snapshots keep their copies.",
                        ),
                        native::row(
                            format!("{key}/actions"),
                            [
                                native::button(
                                    format!("{key}/cancel"),
                                    "Cancel",
                                    cancel,
                                    wire::ButtonPreset::Secondary,
                                ),
                                native::button(
                                    format!("{key}/delete"),
                                    "Delete object",
                                    delete,
                                    wire::ButtonPreset::Danger,
                                ),
                            ],
                        ),
                    ],
                ),
                wire::Edges::all(20.),
            ),
            Some(wire::Length::Fixed(418.)),
            None,
        )
    }

    pub(super) fn disconnected(&self, key: String) -> wire::Node {
        native::column(
            &key,
            [
                native::heading(format!("{key}/title"), "Not connected"),
                native::text(
                    format!("{key}/detail"),
                    "Choose a network from the workspace header to reconnect.",
                ),
            ],
        )
    }

    pub(super) fn changes_heading(&self, key: String) -> wire::Node {
        native::heading(key, "Changes vs HEAD")
    }
    pub(super) fn snapshots_heading(&self, key: String) -> wire::Node {
        native::heading(key, "Snapshots")
    }
    pub(super) fn empty_directory(&self, key: String) -> wire::Node {
        native::text(
            key,
            "Empty directory — nothing is committed under this path.",
        )
    }
}
