use super::*;
use ducktape_view_guest::slots;

impl ChatView {
    pub(super) fn message_body(
        key: String,
        blocks: &[crate::host::ChatBlock],
        on_link: Option<u32>,
    ) -> wire::Node {
        let mut children = Vec::new();
        for (index, block) in blocks.iter().enumerate() {
            let scope = format!("{key}/block/{index}");
            let content = match block.kind.as_str() {
                "divider" => wire::Node::Rule {
                    key: scope,
                    axis: wire::Axis::Row,
                    thickness: 1.,
                    color: None,
                    weak: false,
                    radius: None,
                    snap: None,
                },
                "code" => {
                    let mut children = Vec::new();
                    if !block.lang.is_empty() {
                        children.push(native::text(format!("{scope}/language"), &block.lang));
                    }
                    children.push(native::text_options(
                        native::text(format!("{scope}/code"), &block.text),
                        wire::TextOptions {
                            wrapping: Some(wire::Wrapping::WordOrGlyph),
                            font: Some(wire::NamedFont {
                                family: wire::FontFamily::Monospace,
                                weight: wire::Weight::Normal,
                                stretch: wire::FontStretch::Normal,
                                style: wire::FontStyle::Normal,
                            }),
                            ..Default::default()
                        },
                    ));
                    native::column(scope, children)
                }
                "quote" | "paragraph" => {
                    let text = if block.rich {
                        Self::rich_line(format!("{scope}/text"), block, on_link)
                    } else {
                        native::text(format!("{scope}/text"), &block.text)
                    };
                    if block.kind == "quote" {
                        native::row(
                            scope.clone(),
                            [native::text(format!("{scope}/quote"), "│"), text],
                        )
                    } else {
                        text
                    }
                }
                _ => continue,
            };
            children.push(content);
        }
        native::column(key, children)
    }
    pub(super) fn rich_line(
        key: String,
        block: &crate::host::ChatBlock,
        on_link: Option<u32>,
    ) -> wire::Node {
        let mut spans = Vec::new();
        for part in &block.spans {
            for (content, link, weight, italic) in [
                (
                    &part.mention,
                    Some(&part.mention_link),
                    wire::Weight::Medium,
                    false,
                ),
                (
                    &part.link_text,
                    Some(&part.link),
                    wire::Weight::Medium,
                    false,
                ),
                (&part.bold_italic, None, wire::Weight::Bold, true),
                (&part.bold, None, wire::Weight::Bold, false),
                (&part.italic, None, wire::Weight::Normal, true),
                (&part.plain, None, wire::Weight::Normal, false),
            ] {
                if content.is_empty() {
                    continue;
                }
                let decorated = weight != wire::Weight::Normal || italic;
                spans.push(wire::RichSpan {
                    content: content.clone(),
                    link: link.cloned(),
                    underline: link.is_some(),
                    font: decorated.then_some(wire::NamedFont {
                        family: wire::FontFamily::SansSerif,
                        weight,
                        stretch: wire::FontStretch::Normal,
                        style: if italic {
                            wire::FontStyle::Italic
                        } else {
                            wire::FontStyle::Normal
                        },
                    }),
                    ..Default::default()
                });
            }
        }
        wire::Node::RichText {
            key,
            spans,
            on_link,
            options: wire::TextOptions {
                wrapping: Some(wire::Wrapping::WordOrGlyph),
                ..Default::default()
            },
            size: None,
            color: None,
            font: Default::default(),
            width: Some(wire::Length::Fill),
            align_x: None,
        }
    }
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
