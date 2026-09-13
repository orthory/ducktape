use ducktape_view_guest::{
    kit, slots,
    wire::{self, ButtonPreset, Length, Node},
};

use crate::{ExplorerView, Message, host};

impl ExplorerView {
    pub(crate) fn view(&self) -> Node {
        let search = kit::input(
            "explorer/search",
            "Search messages, pages, issues, files, runs…",
            &self.query,
            slots::handler(Box::new(|value: String| Some(Message::BindQuery(value)))),
            Some(slots::message(Message::SearchSubmit)),
        );
        let can_refresh = self.connected && !self.loading;
        let mut body = vec![
            kit::heading("explorer/title", "Explorer"),
            kit::row(
                "explorer/status",
                [
                    kit::text("explorer/height", host::height_label(self.head)),
                    kit::text("explorer/sync", &self.sync_line),
                ],
            ),
            kit::row(
                "explorer/toolbar",
                [
                    search,
                    kit::button(
                        "explorer/refresh",
                        "Refresh",
                        can_refresh.then(|| slots::message(Message::Refresh)),
                        ButtonPreset::Secondary,
                    ),
                ],
            ),
        ];
        if !self.host_error.is_empty() {
            body.push(kit::text("explorer/error", &self.host_error));
        }
        let panel = match (self.connected, self.searching, self.sent_query.is_empty()) {
            (false, _, _) => kit::text("explorer/disconnected", "Not connected"),
            (true, true, _) => kit::text("explorer/loading-search", "Searching…"),
            (true, false, false) => self.search_results(),
            (true, false, true) => self.ledger(),
        };
        body.push(panel);
        let viewport = slots::handler(Box::new(|(width, height): (f32, f32)| {
            Some(Message::ViewportChanged(
                f64::from(width),
                f64::from(height),
            ))
        }));
        Node::Sensor {
            key: "explorer/viewport".into(),
            reset: None,
            on_show: Some(viewport),
            on_resize: Some(viewport),
            on_hide: None,
            anticipate: None,
            delay: None,
            child: Box::new(kit::padded(
                kit::sized(
                    kit::column("explorer/content", body),
                    Some(Length::Fill),
                    Some(Length::Fill),
                ),
                wire::Edges::all(24.),
            )),
        }
    }

    fn ledger(&self) -> Node {
        let mut rows = Vec::new();
        for block in &self.blocks {
            let key = format!("explorer/block/{}", block.height);
            let summary = kit::column(
                format!("{key}/summary"),
                [
                    kit::row(
                        format!("{key}/metadata"),
                        [
                            kit::text(format!("{key}/height"), block.height.to_string()),
                            kit::text(
                                format!("{key}/count"),
                                host::plural(block.op_count, "op", "ops"),
                            ),
                        ],
                    ),
                    kit::sized(
                        kit::text(format!("{key}/hash"), host::hex(&block.hash)),
                        Some(Length::Fill),
                        None,
                    ),
                ],
            );
            let selected = block.height == self.selected;
            let preset = if selected {
                ButtonPreset::Secondary
            } else {
                ButtonPreset::Subtle
            };
            let mut button = kit::sized(
                kit::button_child(
                    &key,
                    summary,
                    Some(slots::message(Message::SelectExplorerBlock(block.height))),
                    preset,
                ),
                Some(Length::Fill),
                None,
            );
            if let Node::Button { checked, label, .. } = &mut button {
                *checked = Some(selected);
                *label = Some("Inspect block".into());
            }
            rows.push(button);
        }
        if rows.is_empty() {
            let message = if self.loading {
                "Loading blocks…"
            } else {
                "No blocks carrying operations yet."
            };
            rows.push(kit::text("explorer/empty-ledger", message));
        }
        let list = kit::sized(
            kit::container(
                "explorer/ledger-pane",
                kit::scroll("explorer/blocks", kit::column("explorer/block-list", rows)),
            ),
            Some(Length::Fixed(self.ledger_width as f32)),
            Some(Length::Fill),
        );
        let divider = Node::ResizeHandle {
            key: "explorer/ledger-resize".into(),
            on_press: None,
            on_release: None,
            on_drag: Some(slots::handler(Box::new(|(x, y): (f64, f64)| {
                Some(Message::LedgerResized(x, y))
            }))),
            cursor: Some(wire::mouse::Cursor::ResizingHorizontally),
            content: Box::new(Node::Space {
                width: Some(Length::Fixed(8.)),
                height: Some(Length::Fill),
            }),
        };
        kit::sized(
            kit::row("explorer/ledger", [list, divider, self.block_details()]),
            Some(Length::Fill),
            Some(Length::Fill),
        )
    }

    fn block_details(&self) -> Node {
        let block = self
            .blocks
            .iter()
            .find(|block| block.height == self.selected);
        let Some(block) = block else {
            return kit::container(
                "explorer/selection",
                kit::text(
                    "explorer/select-block",
                    "Select a block to inspect its operations and dispatch trace.",
                ),
            );
        };
        let mut content = vec![
            kit::heading("explorer/block-title", format!("Block {}", block.height)),
            Self::digest("explorer/block-hash", "Block hash", &block.hash),
            Self::digest("explorer/commit", "Commit", &block.commit),
        ];
        for (index, op) in host::explorer_ops_at(&self.ops, self.selected)
            .iter()
            .enumerate()
        {
            let key = format!("explorer/operation/{index}");
            content.push(kit::column(
                &key,
                [
                    kit::heading(format!("{key}/title"), &op.target),
                    kit::text(format!("{key}/status"), &op.disposition),
                    Self::digest(&format!("{key}/proposer"), "Proposer", &op.proposer),
                    Self::digest(&format!("{key}/hash"), "Op hash", &op.op_hash),
                    kit::text(format!("{key}/payload"), &op.payload),
                    kit::text(format!("{key}/trace"), &op.trace),
                ],
            ));
        }
        kit::scroll(
            "explorer/details",
            kit::column("explorer/detail-content", content),
        )
    }

    fn digest(key: &str, label: &str, value: &str) -> Node {
        kit::column(
            key,
            [
                kit::row(
                    format!("{key}/header"),
                    [
                        kit::text(format!("{key}/label"), label),
                        kit::button(
                            format!("{key}/copy"),
                            format!("Copy {}", label.to_lowercase()),
                            Some(slots::message(Message::CopyToClipboard(
                                value.into(),
                                format!("{label} copied"),
                            ))),
                            ButtonPreset::Subtle,
                        ),
                    ],
                ),
                kit::sized(
                    kit::text(format!("{key}/value"), host::hex(value)),
                    Some(Length::Fill),
                    None,
                ),
            ],
        )
    }

    fn search_results(&self) -> Node {
        let mut filters = vec![kit::button(
            "explorer/filter/all",
            "All",
            Some(slots::message(Message::PickExplorerKind("all".into()))),
            ButtonPreset::Secondary,
        )];
        for kind in &self.kinds {
            let selected = kind.kind == self.kind;
            let preset = if selected {
                ButtonPreset::Primary
            } else {
                ButtonPreset::Subtle
            };
            filters.push(kit::button(
                format!("explorer/filter/{}", kind.kind),
                format!("{} ({})", kind.label, kind.count),
                Some(slots::message(Message::PickExplorerKind(kind.kind.clone()))),
                preset,
            ));
        }
        filters.push(kit::button(
            "explorer/clear",
            "Clear workspace search",
            Some(slots::message(Message::ClearExplorerSearch)),
            ButtonPreset::Subtle,
        ));
        let mut content = vec![kit::row("explorer/filters", filters)];
        if !self.partial.is_empty() {
            content.push(kit::text("explorer/partial", &self.partial));
        }
        let matches_kind = |hit: &&host::ExplorerHit| self.kind == "all" || hit.kind == self.kind;
        let hits: Vec<_> = self.hits.iter().filter(matches_kind).collect();
        let empty_answer = hits.is_empty()
            && host::search_answer_stands(&self.sent_query, &self.query, self.searching);
        if empty_answer {
            content.push(kit::text("explorer/no-results", "No matching results."));
        }
        for (index, hit) in hits.iter().enumerate() {
            let key = format!("explorer/result/{index}");
            content.push(kit::column(
                &key,
                [
                    kit::heading(format!("{key}/title"), &hit.title),
                    kit::text(format!("{key}/kind"), &hit.meta),
                    kit::text(format!("{key}/snippet"), &hit.snippet),
                    Self::digest(&format!("{key}/target"), "Reference", &hit.target),
                ],
            ));
        }
        kit::scroll(
            "explorer/results",
            kit::column("explorer/result-list", content),
        )
    }
}
