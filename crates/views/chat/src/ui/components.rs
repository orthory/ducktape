use super::*;
use ducktape_view_guest::slots;

impl ChatView {
    pub(super) fn channel_button(
        &self,
        key: String,
        choose: impl Fn(String) -> Message + Clone + 'static,
        channel: crate::host::ChatChannel,
        selected: bool,
        unread: bool,
    ) -> wire::Node {
        let mut label = format!("# {}", channel.name);
        if channel.members_only {
            label.push_str(" · Members only");
        }
        if channel.archived {
            label.push_str(" · Archived");
        }
        if channel.huddle_count > 0 {
            label.push_str(&format!(" · Huddle {}", channel.huddle_count));
        }
        if unread {
            label.push_str(" · Unread");
        }
        let action = if self.busy {
            None
        } else {
            Some(slots::message(choose(channel.id)))
        };
        let mut button = native::button(key, label, action, wire::ButtonPreset::Secondary);
        if let wire::Node::Button { checked, width, label, .. } = &mut button {
            *checked = Some(selected);
            *width = Some(wire::Length::Fill);
            *label = Some(channel.name);
        }
        button
    }

    pub(super) fn loading_messages(&self, key: String) -> wire::Node {
        native::text(key, "Loading messages…")
    }

    pub(super) fn search_result(
        &self,
        key: String,
        open: impl Fn(String, i64, i64) -> Message + Clone + 'static,
        hit: crate::host::ChatSearchHit,
    ) -> wire::Node {
        let action = slots::message(open(hit.channel_id, hit.root_seq, hit.seq));
        let content = native::column(
            format!("{key}/content"),
            [
            native::row(format!("{key}/byline"), [
                native::text(format!("{key}/author"), hit.author),
                native::text(format!("{key}/meta"), hit.meta),
            ]),
                native::text(format!("{key}/text"), hit.text.clone()),
            ],
        );
        let mut button =
            native::button_child(key, content, Some(action), wire::ButtonPreset::Secondary);
        if let wire::Node::Button { label, width, .. } = &mut button {
            *label = Some(hit.text);
            *width = Some(wire::Length::Fill);
        }
        button
    }

    pub(super) fn composer_gate(&self, key: String) -> wire::Node {
        match self.post_refusal.as_str() {
            "channel_archived" => self.archived_notice(key),
            "members_only" => self.private_notice(key),
            _ => native::column(key, []),
        }
    }

    pub(super) fn member_row(
        &self,
        key: String,
        remove: impl Fn(String) -> Message + Clone + 'static,
        member: crate::host::ChatMember,
    ) -> wire::Node {
        let action = if self.busy {
            None
        } else {
            Some(slots::message(remove(member.key)))
        };
        let mut button = native::button(
            format!("{key}/remove"),
            "Remove member",
            action,
            wire::ButtonPreset::Secondary,
        );
        if let wire::Node::Button { description, .. } = &mut button {
            *description = Some(member.label.clone());
        }
        native::row(
            &key,
            [native::text(format!("{key}/name"), member.label), button],
        )
    }

    pub(super) fn live_run_card(
        &self,
        key: String,
        stop: impl Fn(String) -> Message + Clone + 'static,
        open: impl Fn(String) -> Message + Clone + 'static,
        run: crate::host::LiveRunHint,
    ) -> wire::Node {
        native::column(
            &key,
            [
                native::text(format!("{key}/agent"), format!("AI · {}", run.agent)),
                native::text(format!("{key}/status"), run.status),
                native::row(
                    format!("{key}/actions"),
                    [
                        native::button(
                            format!("{key}/open"),
                            "View run",
                            Some(slots::message(open(run.dispatch_id))),
                            wire::ButtonPreset::Secondary,
                        ),
                        native::button(
                            format!("{key}/stop"),
                            "Stop",
                            Some(slots::message(stop(run.run_id))),
                            wire::ButtonPreset::Secondary,
                        ),
                    ],
                ),
            ],
        )
    }
}
