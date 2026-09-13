use super::*;
use ducktape_view_guest::slots;

impl ChatView {
    pub(super) fn direct_message(
        &self,
        key: String,
        choose: impl Fn(String) -> Message + Clone + 'static,
        peer: crate::host::DmPeer,
        selected: bool,
        unread: bool,
    ) -> wire::Node {
        let agent = if peer.is_agent { "AI · " } else { "" };
        let badge = if unread { " · Unread" } else { "" };
        let label = format!("{agent}{}{badge}", peer.name);
        let action = if self.busy {
            None
        } else {
            Some(slots::message(choose(peer.key)))
        };
        let mut button = native::button(key, label, action, wire::ButtonPreset::Secondary);
        if let wire::Node::Button { checked, width, .. } = &mut button {
            *checked = Some(selected);
            *width = Some(wire::Length::Fill);
        }
        button
    }

    pub(super) fn direct_message_header(&self, key: String) -> wire::Node {
        native::row(
            &key,
            [
                self.active_dm_avatar(format!("{key}/avatar")),
                native::heading(format!("{key}/name"), self.active_dm.name.clone()),
            ],
        )
    }
}
