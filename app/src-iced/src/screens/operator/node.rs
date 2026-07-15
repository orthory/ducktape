use super::*;
use iced::widget::{column, row};
fn level_color(level: &str, p: Palette) -> Color {
    match level.to_ascii_lowercase().as_str() {
        "error" => p.red,
        "warn" => p.amber,
        "debug" | "trace" => p.muted_2,
        _ => p.green,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeTab {
    Overview,
    Connections,
    Permissions,
    Logs,
}

impl NodeTab {
    const ALL: [(Self, &'static str); 4] = [
        (Self::Overview, "Overview"),
        (Self::Connections, "Connections"),
        (Self::Permissions, "Permissions"),
        (Self::Logs, "Logs"),
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeRole {
    GenesisValidator,
    MemberValidator,
    RemoteUser,
    Guest,
}

impl NodeRole {
    const fn pill(self) -> &'static str {
        match self {
            Self::GenesisValidator => "GENESIS · VALIDATOR",
            Self::MemberValidator => "MEMBER · VALIDATOR",
            Self::RemoteUser => "USER · NODE",
            Self::Guest => "GUEST",
        }
    }

    const fn title(self) -> &'static str {
        match self {
            Self::GenesisValidator => "Genesis validator",
            Self::MemberValidator => "Member validator",
            Self::RemoteUser => "Remote user",
            Self::Guest => "Guest",
        }
    }

    const fn detail(self) -> &'static str {
        match self {
            Self::GenesisValidator => {
                "This node created the network at genesis. It validates committed state as an equal member and holds no special governance authority."
            }
            Self::MemberValidator => {
                "This workspace is admitted as a member and runs a validator for the network."
            }
            Self::RemoteUser => {
                "This user can inspect the connected node's committed state and metrics without node-operator controls."
            }
            Self::Guest => {
                "No desktop workspace validator identity is loaded. Committed node state remains readable."
            }
        }
    }

    const fn validator(self) -> bool {
        matches!(self, Self::GenesisValidator | Self::MemberValidator)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionRow {
    pub peer: String,
    pub direction: String,
    pub state: String,
    pub age: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    pub timestamp: String,
    pub level: String,
    pub target: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeSnapshot {
    pub connected: bool,
    pub managed: bool,
    pub workspace_name: String,
    pub role: NodeRole,
    pub peer: String,
    pub version: String,
    pub height: u64,
    pub app_hash: String,
    pub modules: Vec<ModuleRoot>,
    pub validator_count: usize,
    pub connections: Vec<ConnectionRow>,
    pub logs: Vec<LogLine>,
    pub blocks_per_second: Option<f64>,
    pub apply_p95_ms: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeState {
    pub data: Resource<NodeSnapshot>,
    pub active_tab: NodeTab,
    pub copied: Option<String>,
    pub log_filter: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeMessage {
    SelectTab(NodeTab),
    Start,
    Stop,
    Copy { key: String, value: String },
    LogFilterChanged(String),
}

pub(super) fn update(state: &mut NodeState, message: NodeMessage) -> Option<Command> {
    match message {
        NodeMessage::SelectTab(tab) => state.active_tab = tab,
        NodeMessage::Start => return Some(Command::StartNode),
        NodeMessage::Stop => return Some(Command::StopNode),
        NodeMessage::Copy { key, value } => {
            state.copied = Some(key);
            return Some(Command::CopyText(value));
        }
        NodeMessage::LogFilterChanged(value) => state.log_filter = value,
    }
    None
}

pub(super) fn view(state: &NodeState, p: Palette) -> Element<'_, Message> {
    match &state.data {
        Resource::Loading => center_state(
            "Loading node",
            "Reading committed node state…",
            Icon::Node,
            p,
        ),
        Resource::Empty => center_state(
            "No node state",
            "Connect a workspace or remote node.",
            Icon::Node,
            p,
        ),
        Resource::Error(error) => {
            error_state("Node unavailable", error, Screen::Node, Icon::Node, p)
        }
        Resource::Ready(snapshot) => {
            let status = if snapshot.connected {
                ("Synced", p.green)
            } else if snapshot.managed {
                ("Stopped", p.red)
            } else {
                ("Offline", p.amber)
            };
            let mut top = row![
                text("This node").font(SANS).size(16).color(p.ink),
                pill(status.0, status.1, p),
                pill(
                    snapshot.role.pill(),
                    if snapshot.role.validator() {
                        p.ink
                    } else {
                        p.amber
                    },
                    p
                ),
                Space::new().width(Length::Fill),
            ]
            .spacing(10)
            .align_y(Alignment::Center);
            if snapshot.managed {
                top = top.push(if snapshot.connected {
                    danger_button("Stop", Message::Node(NodeMessage::Stop), true, p)
                } else {
                    filled_button("Start", Message::Node(NodeMessage::Start), true, p)
                });
            }

            let mut tabs = row![].spacing(3).align_y(Alignment::Center);
            for (tab, label) in NodeTab::ALL {
                tabs = tabs.push(segment_button(
                    label,
                    state.active_tab == tab,
                    Message::Node(NodeMessage::SelectTab(tab)),
                    p,
                ));
            }

            let body = match state.active_tab {
                NodeTab::Overview => node_overview(snapshot, state, p),
                NodeTab::Connections => connections_view(snapshot, p),
                NodeTab::Permissions => permissions_view(snapshot, p),
                NodeTab::Logs => logs_view(snapshot, state, p),
            };
            let mut content = column![
                top,
                text(format!(
                    "peer {} · ducktape-node v{}",
                    short(&snapshot.peer, 12, 8),
                    snapshot.version
                ))
                .font(MONO)
                .size(10.5)
                .color(p.muted_2),
                container(tabs).padding(3).style(move |_| rounded_surface(
                    p.titlebar,
                    p.border_soft,
                    RADIUS_LG
                )),
                body,
            ]
            .spacing(12);
            if let Some(error) = &state.error {
                content = content.push(error_banner(error, p));
            }
            container(scrollable(container(content).padding(PAGE_PAD)))
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_| surface(p.canvas))
                .into()
        }
    }
}

fn node_overview<'a>(
    snapshot: &'a NodeSnapshot,
    state: &'a NodeState,
    p: Palette,
) -> Element<'a, Message> {
    let access = card(
        column![
            row![
                icon_tile(Icon::Node, 36.0, p),
                column![
                    text(snapshot.role.title()).font(SANS).size(14).color(p.ink),
                    text(format!(
                        "{} · peer {}",
                        snapshot.workspace_name,
                        short(&snapshot.peer, 14, 8)
                    ))
                    .font(MONO)
                    .size(10.5)
                    .color(p.muted_3),
                ]
                .spacing(3),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
            text(snapshot.role.detail())
                .font(SANS)
                .size(12)
                .color(p.ink_softer),
        ]
        .spacing(13),
        p,
    );

    let stats = row![
        stat_card("HEIGHT", snapshot.height.to_string(), "committed block", p),
        stat_card(
            "BLOCK RATE",
            snapshot
                .blocks_per_second
                .map(|v| format!("{v:.2}/s"))
                .unwrap_or_else(|| "—".into()),
            "live cadence",
            p
        ),
        stat_card(
            "APPLY P95",
            snapshot
                .apply_p95_ms
                .map(|v| format!("{v:.1} ms"))
                .unwrap_or_else(|| "—".into()),
            "commit latency",
            p
        ),
        stat_card(
            "VALIDATORS",
            snapshot.validator_count.to_string(),
            "current set",
            p
        ),
    ]
    .spacing(9);

    let mut roots = column![copy_value(
        "APP HASH",
        &snapshot.app_hash,
        state.copied.as_deref() == Some("app-hash"),
        "app-hash",
        p,
    )]
    .spacing(7);
    for module in &snapshot.modules {
        roots = roots.push(copy_value(
            &module.id,
            &module.root,
            state.copied.as_deref() == Some(module.id.as_str()),
            &module.id,
            p,
        ));
    }
    column![
        access,
        stats,
        section_label("STATE COMMITMENT", p),
        card(roots, p),
    ]
    .spacing(12)
    .into()
}

fn connections_view(snapshot: &NodeSnapshot, p: Palette) -> Element<'_, Message> {
    let mut rows = column![section_label("CONNECTIONS", p)].spacing(8);
    if snapshot.connections.is_empty() {
        rows = rows.push(notice("No active peer connections.", p));
    } else {
        for peer in &snapshot.connections {
            rows = rows.push(card(
                row![
                    column![
                        text(short(&peer.peer, 16, 10))
                            .font(MONO)
                            .size(12)
                            .color(p.ink),
                        text(format!("{} · {}", peer.direction, peer.age))
                            .font(MONO)
                            .size(10.5)
                            .color(p.muted_2),
                    ]
                    .spacing(3),
                    Space::new().width(Length::Fill),
                    pill(&peer.state, p.green, p),
                ]
                .align_y(Alignment::Center),
                p,
            ));
        }
    }
    rows.into()
}

fn permissions_view(snapshot: &NodeSnapshot, p: Palette) -> Element<'_, Message> {
    let validator = snapshot.role.validator();
    let rows = [
        ("Read node status", true, true),
        ("Verify committed roots", true, true),
        ("Submit module messages", true, true),
        ("Validate blocks", true, false),
        ("Admit waiting workspaces", true, false),
        ("Local daemon controls", snapshot.managed, false),
    ];
    let mut matrix = column![
        row![
            text("capability").font(SANS).size(11).color(p.muted),
            Space::new().width(Length::Fill),
            text(if validator {
                "VALIDATOR"
            } else {
                "REMOTE / GUEST"
            })
            .font(MONO)
            .size(9)
            .color(p.muted_2),
        ]
        .padding([10, 14])
    ];
    for (label, for_validator, for_guest) in rows {
        let allowed = if validator { for_validator } else { for_guest };
        matrix = matrix.push(
            container(row![
                text(label).font(SANS).size(12).color(p.ink_soft),
                Space::new().width(Length::Fill),
                text(if allowed { "✓" } else { "—" })
                    .font(MONO)
                    .size(12)
                    .color(if allowed { p.green } else { p.muted_2 }),
            ])
            .padding([11, 14])
            .style(move |_| top_border(Color::TRANSPARENT, p.border_soft)),
        );
    }
    column![
        text("Node permissions").font(SANS).size(18).color(p.ink),
        text("The role comes from committed membership; daemon controls also require a locally managed workspace.")
            .font(SANS).size(12).color(p.muted),
        card(matrix, p),
    ].spacing(9).into()
}

fn logs_view<'a>(
    snapshot: &'a NodeSnapshot,
    state: &'a NodeState,
    p: Palette,
) -> Element<'a, Message> {
    let filter = state.log_filter.to_ascii_lowercase();
    let mut lines = column![
        text_input("Filter logs", &state.log_filter)
            .on_input(|value| Message::Node(NodeMessage::LogFilterChanged(value)))
            .padding([8, 10])
            .font(MONO)
            .size(11.5)
    ]
    .spacing(8);
    let mut count = 0;
    for line in &snapshot.logs {
        let haystack =
            format!("{} {} {}", line.level, line.target, line.message).to_ascii_lowercase();
        if !filter.is_empty() && !haystack.contains(&filter) {
            continue;
        }
        count += 1;
        lines = lines.push(
            container(
                row![
                    text(&line.timestamp)
                        .font(MONO)
                        .size(10)
                        .color(p.muted_2)
                        .width(90),
                    text(&line.level)
                        .font(MONO)
                        .size(10)
                        .color(level_color(&line.level, p))
                        .width(52),
                    text(&line.target)
                        .font(MONO)
                        .size(10)
                        .color(p.muted)
                        .width(140),
                    text(&line.message).font(MONO).size(10.5).color(p.ink_soft),
                ]
                .spacing(8),
            )
            .padding([7, 9])
            .style(move |_| top_border(Color::TRANSPARENT, p.border_soft)),
        );
    }
    if count == 0 {
        lines = lines.push(notice("No log lines match this filter.", p));
    }
    card(lines, p)
}
