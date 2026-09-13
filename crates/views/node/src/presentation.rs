use crate::{Message, NodeTab, NodeView, host};
use ducktape_view_guest::{
    kit, slots,
    wire::{self, ButtonPreset, Length, Node},
};

impl NodeView {
    pub(crate) fn view(&self) -> Node {
        let tabs = [
            (NodeTab::Overview, "Overview", "Node overview"),
            (NodeTab::Permissions, "Permissions", "Node permissions"),
            (NodeTab::Activity, "Activity", "Node activity"),
            (NodeTab::Modules, "Modules", "Node modules"),
        ]
        .map(|(tab, title, accessible)| {
            let selected = self.node_tab == tab;
            let preset = if selected {
                ButtonPreset::Secondary
            } else {
                ButtonPreset::Subtle
            };
            let mut button = kit::button(
                format!("node/tab/{title}"),
                title,
                Some(slots::message(Message::SelectNodeTab(tab))),
                preset,
            );
            if let Node::Button { label, checked, .. } = &mut button {
                *label = Some(accessible.into());
                *checked = Some(selected);
            }
            button
        });
        let mut body = vec![
            kit::heading("node/title", "This node"),
            kit::text("node/connection", &self.status),
            kit::row("node/tabs", tabs),
        ];
        if !self.host_error.is_empty() {
            body.push(kit::text("node/error", &self.host_error));
        }
        let panel = match (self.connected, self.node_tab) {
            (false, _) => kit::text("node/disconnected", "Not connected"),
            (true, NodeTab::Overview) => self.overview(),
            (true, NodeTab::Permissions) => self.permissions(),
            (true, NodeTab::Activity) => self.activity(),
            (true, NodeTab::Modules) => self.modules(),
        };
        body.push(panel);
        kit::padded(
            kit::sized(
                kit::column("node/content", body),
                Some(Length::Fill),
                Some(Length::Fill),
            ),
            wire::Edges::all(24.),
        )
    }

    fn overview(&self) -> Node {
        let facts = &self.facts;
        let mut content = vec![
            Self::reading(
                "node/height",
                "Height",
                host::height_label_short(facts.node_height),
            ),
            Self::reading(
                "node/checkpoint",
                "Checkpoint",
                host::height_label_short(facts.node_checkpoint),
            ),
            Self::reading(
                "node/finalized",
                "Last finalized",
                host::relative_time(facts.node_last_finalized, self.wall_now),
            ),
            Self::reading(
                "node/quorum",
                "Reachable / quorum",
                host::reading_pair(&facts.node_reachable_label, &facts.node_quorum_label),
            ),
            Self::copyable("node/key", "Node key", &facts.node_key),
            Self::copyable("node/root", "Root hash", &facts.node_root_hash),
            Self::copyable("node/directory", "Data directory", &self.node_data_dir),
            Self::reading("node/version", "Version", &facts.node_version),
            Self::reading("node/sync", "Synchronization", &facts.sync_line),
            Self::reading(
                "node/phase-since",
                "Phase since",
                host::relative_time(facts.node_phase_since, self.wall_now),
            ),
            Self::reading(
                "node/retries",
                "Sync retries",
                facts.node_sync_retries.to_string(),
            ),
            Self::reading(
                "node/failures",
                "Sync failures",
                facts.node_sync_failures.to_string(),
            ),
            kit::button(
                "node/open-modules",
                "Installed modules",
                Some(slots::message(Message::OpenNodeModules)),
                ButtonPreset::Secondary,
            ),
            kit::heading("node/peers/title", "Peers"),
        ];
        if !facts.node_sync_last_error.is_empty() {
            content.push(kit::text("node/sync-error", &facts.node_sync_last_error));
        }
        for peer in &self.node_peers {
            let key = format!("node/peer/{}", peer.key);
            let status = if peer.live {
                "Connected"
            } else {
                "Disconnected"
            };
            content.push(kit::row(
                &key,
                [
                    kit::text(format!("{key}/key"), &peer.key),
                    kit::text(format!("{key}/role"), &peer.role),
                    kit::text(format!("{key}/status"), status),
                ],
            ));
        }
        if self.node_peers.is_empty() {
            content.push(kit::text("node/peers/empty", "No direct peers."));
        }
        kit::scroll("node/overview", kit::column("node/readings", content))
    }

    fn permissions(&self) -> Node {
        let description = match self.tier.as_str() {
            "validator" => "Signs quorum, finalizes rounds and stores full history.",
            "resident" => "Stores full history without signing quorum.",
            "guest" => "Reads and verifies finalized headers.",
            _ => "Node standing has not been reported.",
        };
        let admin = if self.admin {
            "Available to this seat"
        } else {
            "Not available to this seat"
        };
        kit::scroll(
            "node/permissions",
            kit::column(
                "node/standing",
                [
                    Self::reading("node/tier", "Standing", &self.tier),
                    kit::text("node/standing-description", description),
                    Self::reading("node/admin", "Node administration", admin),
                    kit::text(
                        "node/quorum-note",
                        "Quorum standing is granted and revoked by quorum, not by this device.",
                    ),
                    kit::row(
                        "node/permissions/header",
                        [
                            kit::text("node/permissions/capability", "Capability"),
                            kit::text("node/permissions/tiers", "Validator · Resident · Guest"),
                        ],
                    ),
                    Self::reading(
                        "node/permissions/read",
                        "Read and verify finality",
                        "Yes · Yes · Yes",
                    ),
                    Self::reading(
                        "node/permissions/propose",
                        "Propose modules and members",
                        "Yes · Yes · No",
                    ),
                    Self::reading(
                        "node/permissions/sign",
                        "Sign quorum and finalize",
                        "Yes · No · No",
                    ),
                ],
            ),
        )
    }

    fn modules(&self) -> Node {
        let mut content = Vec::new();
        for module in &self.module_rows {
            let key = format!("node/module/{}", module.id);
            let mut values = vec![
                kit::heading(format!("{key}/title"), &module.id),
                kit::text(format!("{key}/category"), &module.category),
                Self::copyable(&format!("{key}/root"), "State root", &module.root),
                Self::copyable(&format!("{key}/code"), "Active code", &module.code_hash),
            ];
            if !module.pending_hash.is_empty() {
                values.extend([
                    Self::copyable(
                        &format!("{key}/pending"),
                        "Pending code",
                        &module.pending_hash,
                    ),
                    Self::reading(
                        &format!("{key}/activation"),
                        "Activation height",
                        module.activation_height.to_string(),
                    ),
                    Self::reading(
                        &format!("{key}/readiness"),
                        "Readiness",
                        module.readiness.to_string(),
                    ),
                    kit::text(
                        format!("{key}/ready"),
                        if module.ready {
                            "Ready"
                        } else {
                            "Waiting for readiness"
                        },
                    ),
                ]);
            }
            content.push(kit::column(&key, values));
        }
        if content.is_empty() {
            content.push(kit::text(
                "node/modules/empty",
                "No installed modules reported.",
            ));
        }
        kit::scroll("node/modules", kit::column("node/module-list", content))
    }

    fn activity(&self) -> Node {
        let filter = kit::input(
            "node/log-filter",
            "filter logs…",
            &self.node_log_filter,
            slots::handler(Box::new(|value: String| {
                Some(Message::NodeLogFilterChanged(value))
            })),
            None,
        );
        let live = kit::input(
            "node/live-filter",
            "info,ducktape::join=debug",
            &self.live_log_filter,
            slots::handler(Box::new(|value: String| {
                Some(Message::LiveLogFilterChanged(value))
            })),
            Some(slots::message(Message::ApplyLiveLogFilter)),
        );
        let can_retune = self.admin && !self.live_log_filter.trim().is_empty();
        let mut content = vec![
            filter,
            kit::row(
                "node/retune",
                [
                    live,
                    kit::button(
                        "node/retune/apply",
                        "Retune",
                        can_retune.then(|| slots::message(Message::ApplyLiveLogFilter)),
                        ButtonPreset::Secondary,
                    ),
                ],
            ),
            kit::text("node/filter-note", &self.live_filter_note),
        ];
        let visible = host::visible_log(&self.log_lines, &self.node_log_filter);
        let note = host::log_note(self.log_lines.len() as i64, visible.len() as i64);
        if !note.is_empty() {
            content.push(kit::text("node/log-note", note));
        }
        let rows = visible.iter().map(|line| {
            let key = format!("node/log/{}", line.cursor);
            kit::row(
                &key,
                [
                    kit::text(format!("{key}/time"), &line.time),
                    kit::text(format!("{key}/level"), &line.level),
                    kit::sized(
                        kit::text(format!("{key}/message"), &line.message),
                        Some(Length::Fill),
                        None,
                    ),
                ],
            )
        });
        content.push(kit::scroll(
            "node/logs",
            kit::column("node/log-lines", rows),
        ));
        kit::sized(
            kit::column("node/activity", content),
            Some(Length::Fill),
            Some(Length::Fill),
        )
    }

    fn reading(key: &str, label: &str, value: impl Into<String>) -> Node {
        kit::row(
            key,
            [
                kit::sized(
                    kit::text(format!("{key}/label"), label),
                    Some(Length::Fixed(180.)),
                    None,
                ),
                kit::sized(
                    kit::text(format!("{key}/value"), value),
                    Some(Length::Fill),
                    None,
                ),
            ],
        )
    }

    fn copyable(key: &str, label: &str, value: &str) -> Node {
        kit::column(
            key,
            [
                Self::reading(&format!("{key}/reading"), label, value),
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
        )
    }
}
