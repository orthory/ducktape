use super::*;
use ducktape_view_guest::slots;

impl ChatView {
    pub(super) fn principal_avatar(
        &self,
        key: String,
        initials: String,
        agent: bool,
    ) -> wire::Node {
        let label = if agent {
            format!("AI · {initials}")
        } else {
            initials
        };
        native::text(key, label)
    }

    pub(super) fn active_dm_avatar(&self, key: String) -> wire::Node {
        self.principal_avatar(
            key,
            self.active_dm.initials.clone(),
            self.active_dm.is_agent,
        )
    }

    pub(super) fn archived_badge(&self, key: String) -> wire::Node {
        native::text(key, "Archived")
    }
    pub(super) fn private_badge(&self, key: String) -> wire::Node {
        native::text(key, "Members only")
    }

    pub(super) fn huddle_controls(
        &self,
        key: String,
        leave: impl Fn() -> Message + Clone + 'static,
        show: impl Fn() -> Message + Clone + 'static,
    ) -> wire::Node {
        let elapsed = crate::host::mmss(self.huddle_now - self.huddle_joined_at);
        let mute = if self.call_muted { " · Muted" } else { "" };
        native::row(
            &key,
            [
                native::button(
                    format!("{key}/show"),
                    format!("LIVE · {elapsed}{mute}"),
                    Some(slots::message(show())),
                    wire::ButtonPreset::Secondary,
                ),
                native::button(
                    format!("{key}/leave"),
                    "Leave huddle",
                    Some(slots::message(leave())),
                    wire::ButtonPreset::Secondary,
                ),
            ],
        )
    }

    pub(super) fn start_huddle(
        &self,
        key: String,
        join: impl Fn() -> Message + Clone + 'static,
    ) -> wire::Node {
        native::button(
            key,
            "Start a huddle",
            Some(slots::message(join())),
            wire::ButtonPreset::Secondary,
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

    pub(super) fn empty_messages(&self, key: String) -> wire::Node {
        native::column(
            &key,
            [
                native::heading(format!("{key}/title"), "No messages yet"),
                native::text(
                    format!("{key}/detail"),
                    "Nobody has posted here. Send the first message below.",
                ),
            ],
        )
    }

    pub(super) fn archived_notice(&self, key: String) -> wire::Node {
        native::text(
            key,
            "This channel is archived. Unarchive it from Channel details to post here again.",
        )
    }

    pub(super) fn private_notice(&self, key: String) -> wire::Node {
        native::text(
            key,
            "This channel is members-only and your key is not on its roster. Ask a member to add your key from Channel details.",
        )
    }

    pub(super) fn name_label(&self, key: String) -> wire::Node {
        native::text(key, "Name")
    }
    pub(super) fn members_label(&self, key: String) -> wire::Node {
        native::text(key, "Members")
    }
}
