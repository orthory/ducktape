use super::*;
use ducktape_view_guest::slots;

fn span(
    content: String,
    link: Option<String>,
    weight: wire::Weight,
    italic: bool,
) -> wire::RichSpan {
    let decorated = weight != wire::Weight::Normal || italic;
    wire::RichSpan {
        content,
        underline: link.is_some(),
        link,
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
    }
}

impl ForgeView {
    pub(super) fn unavailable(&self, key: String) -> wire::Node {
        native::column(
            &key,
            [
                native::heading(format!("{key}/title"), "Unable to read Forge"),
                native::text(format!("{key}/error"), self.host_error.clone()),
            ],
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

    pub(super) fn loading_tracker(&self, key: String) -> wire::Node {
        native::text(key, "Loading repository tracker…")
    }
    pub(super) fn tracker_unavailable(&self, key: String) -> wire::Node {
        native::text(
            key,
            "Could not load this repository. Return to all repos and open it again to retry.",
        )
    }
    pub(super) fn empty_issues(&self, key: String) -> wire::Node {
        native::text(
            key,
            "No issues — this app reads the tracker but cannot open one yet.",
        )
    }
    pub(super) fn empty_pulls(&self, key: String) -> wire::Node {
        native::text(
            key,
            "No pull requests — an agent run opens one when it delivers its work.",
        )
    }
    pub(super) fn loading_item(&self, key: String) -> wire::Node {
        native::text(key, "Loading tracker item…")
    }
    pub(super) fn item_unavailable(&self, key: String) -> wire::Node {
        native::text(
            key,
            "Could not load this item. Go back and open it again to retry.",
        )
    }

    pub(super) fn finality(&self, key: String, height: i64) -> wire::Node {
        let label = if height > 0 {
            format!("✓ finalized · h {height}")
        } else {
            "✓ finalized".into()
        };
        native::text(key, label)
    }

    fn rich_line(
        &self,
        key: String,
        open: impl Fn(String) -> Message + Clone + 'static,
        block: &crate::host::ChatBlock,
    ) -> wire::Node {
        let mut spans = Vec::new();
        for part in &block.spans {
            for next in [
                span(
                    part.mention.clone(),
                    Some(part.mention_link.clone()),
                    wire::Weight::Medium,
                    false,
                ),
                span(
                    part.link_text.clone(),
                    Some(part.link.clone()),
                    wire::Weight::Medium,
                    false,
                ),
                span(part.bold_italic.clone(), None, wire::Weight::Bold, true),
                span(part.bold.clone(), None, wire::Weight::Bold, false),
                span(part.italic.clone(), None, wire::Weight::Normal, true),
                span(part.plain.clone(), None, wire::Weight::Normal, false),
            ] {
                if !next.content.is_empty() {
                    spans.push(next);
                }
            }
        }
        wire::Node::RichText {
            key,
            spans,
            options: wire::TextOptions {
                wrapping: Some(wire::Wrapping::WordOrGlyph),
                ..Default::default()
            },
            size: None,
            color: None,
            font: Default::default(),
            width: Some(wire::Length::Fill),
            align_x: None,
            on_link: Some(slots::handler::<String, Message>(Box::new(move |link| {
                Some(open(link))
            }))),
        }
    }

    pub(super) fn rich_body(
        &self,
        key: String,
        open: impl Fn(String) -> Message + Clone + 'static,
        blocks: Vec<crate::host::ChatBlock>,
    ) -> wire::Node {
        let mut children = Vec::new();
        for (index, block) in blocks.iter().enumerate() {
            let key = format!("{key}/block/{index}");
            match block.kind.as_str() {
                "divider" => children.push(wire::Node::Rule {
                    key,
                    axis: wire::Axis::Row,
                    thickness: 1.,
                    color: None,
                    weak: false,
                    radius: None,
                    snap: None,
                }),
                "code" => {
                    let code = native::text_options(
                        native::text(format!("{key}/code"), block.text.clone()),
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
                    );
                    children.push(native::column(
                        &key,
                        [
                            native::text(format!("{key}/language"), block.lang.clone()),
                            code,
                        ],
                    ));
                }
                "quote" | "paragraph" => {
                    let content = if block.rich {
                        self.rich_line(format!("{key}/text"), open.clone(), block)
                    } else {
                        native::text(format!("{key}/text"), block.text.clone())
                    };
                    let content = if block.kind == "quote" {
                        native::row(&key, [native::text(format!("{key}/quote"), "│"), content])
                    } else {
                        content
                    };
                    children.push(content);
                }
                _ => {}
            }
        }
        native::column(key, children)
    }
    pub(super) fn item_body(
        &self,
        key: String,
        open: impl Fn(String) -> Message + Clone + 'static,
    ) -> wire::Node {
        self.rich_body(key, open, self.forge_item_blocks.clone())
    }
}
