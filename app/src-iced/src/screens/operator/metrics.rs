use super::*;
use iced::widget::{column, row};
fn format_rate(value: f64) -> String {
    format!("{}/s", format_bytes(value.max(0.0) as u64))
}

fn format_bytes(value: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut size = value as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{value} {}", UNITS[unit])
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DataPlaneMetric {
    pub service: String,
    pub owner: String,
    pub age: String,
    pub tx_bytes_per_second: f64,
    pub rx_bytes_per_second: f64,
    pub total_bytes: u64,
    pub dropped: u64,
    pub halted: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SyncPeerMetric {
    pub peer: String,
    pub phase: String,
    pub age: String,
    pub progress: Option<f32>,
    pub blocks_left: Option<u64>,
    pub tx_bytes_per_second: f64,
    pub total_bytes: u64,
    pub frames: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MetricsSnapshot {
    pub block_height: u64,
    pub connected_peers: usize,
    pub blocks_per_second: f64,
    pub apply_p50_ms: f64,
    pub apply_p95_ms: f64,
    pub accepted: u64,
    pub rejected: u64,
    pub data_planes: Vec<DataPlaneMetric>,
    pub sync_peers: Vec<SyncPeerMetric>,
    pub sampled_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MetricsState {
    pub data: Resource<MetricsSnapshot>,
    pub paused: bool,
}

pub(super) fn toggle_pause(state: &mut MetricsState) -> Option<Command> {
    state.paused = !state.paused;
    Some(Command::PauseMetrics(state.paused))
}

pub(super) fn view(state: &MetricsState, p: Palette) -> Element<'_, Message> {
    let Resource::Ready(snapshot) = &state.data else {
        return resource_screen(
            &state.data,
            "Metrics",
            "No metrics samples yet.",
            Screen::Metrics,
            Icon::Metrics,
            p,
        );
    };
    let actions = row![
        text(if state.paused { "Paused" } else { "Live" })
            .font(MONO)
            .size(CAPTION)
            .color(if state.paused { p.muted_2 } else { p.green }),
        outline_button(
            if state.paused { "Resume" } else { "Pause" },
            Message::ToggleMetricsPause,
            true,
            p
        ),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    let header = section_header("Metrics", None, Some(actions.into()), p);
    let intro = text(format!(
        "Live node telemetry · sampled {}",
        snapshot.sampled_at
    ))
    .font(SANS)
    .size(BODY)
    .color(p.muted_2);
    let summary = row![
        stat_card(
            "BLOCK HEIGHT",
            snapshot.block_height.to_string(),
            "committed",
            p
        ),
        stat_card(
            "PEERS",
            snapshot.connected_peers.to_string(),
            "connected",
            p
        ),
        stat_card(
            "BLOCK RATE",
            format!("{:.2}/s", snapshot.blocks_per_second),
            "recent",
            p
        ),
        stat_card(
            "APPLY P95",
            format!("{:.1} ms", snapshot.apply_p95_ms),
            "p50 shown below",
            p
        ),
    ]
    .spacing(9);
    let mut planes = column![section_panel_header(
        "DATA PLANES",
        Some(snapshot.data_planes.len().to_string()),
        p
    )]
    .spacing(8);
    if snapshot.data_planes.is_empty() {
        planes = planes.push(notice(
            "No open data planes — this node is not carrying overlay traffic.",
            p,
        ));
    }
    for plane in &snapshot.data_planes {
        planes = planes.push(metric_plane_row(plane, p));
    }
    let mut sync = column![section_panel_header(
        "STATE SYNC",
        (!snapshot.sync_peers.is_empty()).then(|| format!("serving {}", snapshot.sync_peers.len())),
        p
    )]
    .spacing(8);
    if snapshot.sync_peers.is_empty() {
        sync = sync.push(notice(
            "No peer is pulling state from this node — the state-sync lane is idle.",
            p,
        ));
    }
    for peer in &snapshot.sync_peers {
        sync = sync.push(metric_sync_row(peer, p));
    }
    let body = column![
        intro,
        summary,
        card(
            row![
                column![
                    text("ACCEPTED").font(MONO).size(CAPTION).color(p.muted_2),
                    text(snapshot.accepted.to_string())
                        .font(MONO)
                        .size(HEADING)
                        .color(p.green)
                ]
                .spacing(4),
                Space::new().width(Length::Fill),
                column![
                    text("REJECTED").font(MONO).size(CAPTION).color(p.muted_2),
                    text(snapshot.rejected.to_string())
                        .font(MONO)
                        .size(HEADING)
                        .color(p.red)
                ]
                .spacing(4),
                Space::new().width(Length::Fill),
                column![
                    text("APPLY P50").font(MONO).size(CAPTION).color(p.muted_2),
                    text(format!("{:.1} ms", snapshot.apply_p50_ms))
                        .font(MONO)
                        .size(HEADING)
                        .color(p.ink)
                ]
                .spacing(4),
            ],
            p
        ),
        card(planes, p),
        card(sync, p),
    ]
    .spacing(12);
    container(column![
        header,
        scrollable(container(body).padding(PAGE_PAD))
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| surface(p.canvas))
    .into()
}

fn metric_plane_row(plane: &DataPlaneMetric, p: Palette) -> Element<'_, Message> {
    container(
        row![
            column![
                text(if plane.halted {
                    format!("{} · HALTED", plane.service)
                } else {
                    plane.service.clone()
                })
                .font(MONO)
                .size(LABEL)
                .color(if plane.halted { p.red } else { p.ink }),
                text(format!("by {} · open {}", short(&plane.owner, 10, 6), plane.age))
                    .font(MONO)
                    .size(CAPTION)
                    .color(p.muted_2),
            ]
            .spacing(2)
            .width(180),
            Space::new().width(Length::Fill),
            column![
                text(format!("↑ {}", format_rate(plane.tx_bytes_per_second)))
                    .font(MONO)
                    .size(CAPTION)
                    .color(p.ink),
                text(format!("↓ {}", format_rate(plane.rx_bytes_per_second)))
                    .font(MONO)
                    .size(CAPTION)
                    .color(p.ink),
            ]
            .spacing(2)
            .width(110),
            column![
                text(format!("{} total", format_bytes(plane.total_bytes)))
                    .font(MONO)
                    .size(CAPTION)
                    .color(p.muted),
                text(format!("{} dropped", plane.dropped))
                    .font(MONO)
                    .size(CAPTION)
                    .color(if plane.dropped > 0 {
                        p.amber
                    } else {
                        p.muted_2
                    }),
            ]
            .spacing(2)
            .align_x(Alignment::End)
            .width(130),
        ]
        .align_y(Alignment::Center)
        .spacing(12),
    )
    .padding([8, 0])
    .into()
}

fn metric_sync_row(peer: &SyncPeerMetric, p: Palette) -> Element<'_, Message> {
    let progress = peer
        .progress
        .map(|value| format!("{:.0}%", value * 100.0))
        .unwrap_or_else(|| "no progression".into());
    let left = peer
        .blocks_left
        .map(|value| {
            if value == 0 {
                "synced".into()
            } else {
                format!("{value} blocks left")
            }
        })
        .unwrap_or_default();
    container(
        row![
            column![
                text(short(&peer.peer, 12, 8))
                    .font(MONO)
                    .size(LABEL)
                    .color(p.ink),
                text(format!("{} · {}", peer.phase, peer.age))
                    .font(MONO)
                    .size(CAPTION)
                    .color(p.muted_2),
            ]
            .spacing(2)
            .width(170),
            column![
                text(progress).font(MONO).size(CAPTION).color(p.ink),
                text(left).font(MONO).size(CAPTION).color(p.muted),
            ]
            .spacing(2)
            .width(150),
            Space::new().width(Length::Fill),
            column![
                text(format!("↑ {}", format_rate(peer.tx_bytes_per_second)))
                    .font(MONO)
                    .size(CAPTION)
                    .color(p.ink),
                text(format!(
                    "{} · {} frames",
                    format_bytes(peer.total_bytes),
                    peer.frames
                ))
                .font(MONO)
                .size(CAPTION)
                .color(p.muted),
            ]
            .spacing(2)
            .align_x(Alignment::End),
        ]
        .align_y(Alignment::Center)
        .spacing(12),
    )
    .padding([8, 0])
    .into()
}

// --- compact native widget vocabulary ----------------------------------
