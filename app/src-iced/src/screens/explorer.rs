//! Native block explorer. Presentation state is transport-free: the host
//! loads the finalized block ring and returns it through [`ServiceEvent`].

use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Alignment, Background, Border, Element, Length};

use crate::icons::{self, Icon};
use crate::theme::{self, MONO, Palette, RADIUS_MD, RADIUS_SM, SANS};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resource<T> {
    Loading,
    Empty,
    Error(String),
    Ready(T),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchInfo {
    pub module: String,
    pub origin: String,
    pub emitted_messages: u64,
    pub emitted_events: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    Applied,
    Rejected,
}

impl Disposition {
    const fn label(self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootOp {
    pub proposer: String,
    pub proposer_name: Option<String>,
    pub disposition: Disposition,
    pub target: String,
    pub operations: Vec<DispatchInfo>,
    pub payload: String,
    pub op_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockRecord {
    pub height: u64,
    pub hash: String,
    pub commit_hash: String,
    pub ops: Vec<RootOp>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub blocks: Resource<Vec<BlockRecord>>,
    /// Holds the immutable record, not merely its height, so a detail remains
    /// stable when the bounded ring evicts it during inspection.
    pub open: Option<BlockRecord>,
    pub pending_focus: Option<u64>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            blocks: Resource::Loading,
            open: None,
            pending_focus: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Load,
    Refresh,
    Open(u64),
    Back,
    #[allow(dead_code)]
    Focus(u64),
    Service(ServiceEvent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Load,
    ClearFocus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceEvent {
    Loaded(Result<Option<Vec<BlockRecord>>, String>),
}

pub fn update(state: &mut State, message: Message) -> Option<Command> {
    match message {
        Message::Load => {
            state.blocks = Resource::Loading;
            Some(Command::Load)
        }
        Message::Refresh => Some(Command::Load),
        Message::Open(height) => {
            let Resource::Ready(blocks) = &state.blocks else {
                return None;
            };
            state.open = blocks.iter().find(|block| block.height == height).cloned();
            None
        }
        Message::Back => {
            state.open = None;
            None
        }
        Message::Focus(height) => {
            state.pending_focus = Some(height);
            consume_focus(state)
        }
        Message::Service(ServiceEvent::Loaded(result)) => {
            state.blocks = match result {
                Ok(Some(blocks)) if blocks.is_empty() => Resource::Empty,
                Ok(Some(blocks)) => Resource::Ready(blocks),
                Ok(None) => Resource::Empty,
                Err(error) => Resource::Error(error),
            };
            consume_focus(state)
        }
    }
}

fn consume_focus(state: &mut State) -> Option<Command> {
    let height = state.pending_focus?;
    let Resource::Ready(blocks) = &state.blocks else {
        return None;
    };
    state.open = blocks.iter().find(|block| block.height == height).cloned();
    state.pending_focus = None;
    Some(Command::ClearFocus)
}

pub fn view(state: &State, mode: theme::Mode) -> Element<'_, Message> {
    let p = *theme::palette(mode);
    let count = match &state.blocks {
        Resource::Ready(blocks) => format!("{} blocks", blocks.len()),
        _ => "—".into(),
    };
    let header = container(
        row![
            text("Explorer").font(SANS).size(13).color(p.ink),
            Space::new().width(Length::Fill),
            text(count).font(MONO).size(11).color(p.muted),
        ]
        .align_y(Alignment::Center),
    )
    .height(56)
    .padding([0, 17])
    .align_y(Alignment::Center)
    .style(move |_| bottom_rule(p.paper, p.border_soft));

    let body = if let Some(block) = &state.open {
        block_detail(block, p)
    } else {
        blocks_view(&state.blocks, p)
    };
    container(column![header, body].height(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| surface(p.canvas))
        .into()
}

fn blocks_view(resource: &Resource<Vec<BlockRecord>>, p: Palette) -> Element<'_, Message> {
    match resource {
        Resource::Loading => state_view(
            "Loading blocks…",
            "Reading the finalized block ring.",
            p,
        ),
        Resource::Empty => state_view(
            "No blocks yet",
            "Empty heartbeat blocks are skipped, so rows appear once real ops commit.",
            p,
        ),
        Resource::Error(error) => {
            let body = column![
                text("Block explorer unavailable")
                    .font(SANS)
                    .size(13)
                    .color(p.ink),
                text(error).font(SANS).size(11.5).color(p.danger),
                outline_button("Retry", Message::Load, p),
            ]
            .spacing(8);
            container(body).padding(17).width(Length::Fill).into()
        }
        Resource::Ready(blocks) => {
            let headers = row![
                header_cell("HEIGHT", 72.0, p),
                fill_header("HASH", p),
                fill_header("COMMIT", p),
                fill_header("PROPOSER", p),
                header_cell("OPS", 52.0, p),
            ]
            .spacing(12)
            .padding(iced::Padding {
                top: 0.0,
                right: 13.0,
                bottom: 5.0,
                left: 13.0,
            });
            let mut rows = column![headers].spacing(7);
            for block in blocks.iter().rev() {
                rows = rows.push(block_row(block, p));
            }
            scrollable(container(rows).padding(17).width(Length::Fill))
                .height(Length::Fill)
                .into()
        }
    }
}

fn block_row(block: &BlockRecord, p: Palette) -> Element<'static, Message> {
    let proposers: Vec<&str> =
        block
            .ops
            .iter()
            .map(|op| op.proposer.as_str())
            .fold(Vec::new(), |mut values, proposer| {
                if !values.contains(&proposer) {
                    values.push(proposer);
                }
                values
            });
    let primary = block.ops.first();
    let proposer = primary
        .map(|op| {
            let label = op
                .proposer_name
                .clone()
                .unwrap_or_else(|| short_hex(&op.proposer));
            if proposers.len() > 1 {
                format!("{label} +{}", proposers.len() - 1)
            } else {
                label
            }
        })
        .unwrap_or_else(|| "—".into());
    let rejected = block
        .ops
        .iter()
        .any(|op| op.disposition == Disposition::Rejected);
    let op_count = if block.ops.is_empty() {
        "nop".into()
    } else {
        block.ops.len().to_string()
    };

    let btn = button(
        row![
            fixed_cell(format!("#{}", block.height), 72.0, p.ink, true),
            fill_cell(short_hex(&block.hash), p.ink_softer, true),
            fill_cell(short_hex(&block.commit_hash), p.muted_3, true),
            fill_cell(proposer, p.muted_3, false),
            fixed_cell(
                op_count,
                52.0,
                if block.ops.is_empty() {
                    p.muted_2
                } else if rejected {
                    p.red
                } else {
                    theme::ACCENTS[0]
                },
                true,
            ),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([9, 13])
    .style(move |_, status| iced::widget::button::Style {
        background: Some(Background::Color(
            if matches!(status, iced::widget::button::Status::Hovered) {
                p.sidebar
            } else {
                p.paper
            },
        )),
        text_color: p.ink,
        border: Border {
            color: p.border,
            width: 1.0,
            radius: RADIUS_MD.into(),
        },
        ..Default::default()
    })
    .on_press(Message::Open(block.height));
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(
        iced_agent_plugin::Role::ListItem,
        format!("#{}", block.height),
        btn,
    );
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn block_detail(block: &BlockRecord, p: Palette) -> Element<'static, Message> {
    let empty = block.ops.is_empty();
    let rejected = block
        .ops
        .iter()
        .filter(|op| op.disposition == Disposition::Rejected)
        .count();
    let status = if empty {
        "idle".into()
    } else {
        format!(
            "{} op{}{}",
            block.ops.len(),
            if block.ops.len() == 1 { "" } else { "s" },
            if rejected == 0 {
                String::new()
            } else {
                format!(" · {rejected} rejected")
            }
        )
    };
    let top = row![
        bare_button("← Blocks", Message::Back, p),
        text(format!("#{}", block.height))
            .font(MONO)
            .size(14)
            .color(p.ink),
        Space::new().width(Length::Fill),
        text(status)
            .font(SANS)
            .size(10.5)
            .color(if rejected > 0 { p.red } else { p.green }),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    let digests = card(
        column![
            digest_line("HASH", &block.hash, p),
            digest_line("COMMIT", &block.commit_hash, p),
        ]
        .spacing(7),
        p,
    );
    let mut content = column![top, digests].spacing(13);
    if empty {
        content = content.push(
            text("Idle block — no ops committed in this window (a heartbeat nop).")
                .font(SANS)
                .size(12)
                .color(p.muted_2),
        );
    } else {
        content = content.push(
            text(format!("OPS ({})", block.ops.len()))
                .font(SANS)
                .size(10.5)
                .color(p.muted),
        );
        for (index, op) in block.ops.iter().enumerate() {
            content = content.push(op_section(op, index, p));
        }
    }
    scrollable(container(content).padding(17).width(Length::Fill))
        .height(Length::Fill)
        .into()
}

fn op_section(op: &RootOp, index: usize, p: Palette) -> Element<'static, Message> {
    let proposer_label = op
        .proposer_name
        .as_ref()
        .map(|name| format!("{name} · {}", op.proposer))
        .unwrap_or_else(|| op.proposer.clone());
    let header = row![
        text(format!("OP {index}"))
            .font(MONO)
            .size(10.5)
            .color(p.muted_2),
        text(op.target.clone()).font(SANS).size(12.5).color(p.ink),
        text(op.disposition.label()).font(SANS).size(10.5).color(
            if op.disposition == Disposition::Rejected {
                p.red
            } else {
                p.green
            }
        ),
        Space::new().width(Length::Fill),
        text(
            op.proposer_name
                .clone()
                .unwrap_or_else(|| short_hex(&op.proposer)),
        )
        .font(SANS)
        .size(11)
        .color(p.muted_3),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    let mut body = column![
        header,
        digest_line("PROPOSER", &proposer_label, p),
        digest_line("OP HASH", &op.op_hash, p),
        text(format!("TRANSACTIONS ({})", op.operations.len()))
            .font(SANS)
            .size(10.5)
            .color(p.muted),
    ]
    .spacing(8);
    if op.operations.is_empty() {
        body = body.push(
            text(if op.disposition == Disposition::Rejected {
                "The op finalized but was rejected — a deterministic no-op, so no dispatches ran."
            } else {
                "No dispatches recorded."
            })
            .font(SANS)
            .size(12)
            .color(p.muted_2),
        );
    } else {
        for (dispatch_index, dispatch) in op.operations.iter().enumerate() {
            body = body.push(dispatch_row(dispatch, dispatch_index, p));
        }
    }
    if !op.payload.is_empty() {
        body = body
            .push(text("PAYLOAD").font(SANS).size(10.5).color(p.muted))
            .push(
                container(
                    text(op.payload.clone())
                        .font(MONO)
                        .size(11)
                        .color(p.ink_softer),
                )
                .width(Length::Fill)
                .padding([9, 11])
                .style(move |_| rounded_surface(p.sunken, p.border, RADIUS_MD)),
            );
    }
    card(body, p)
}

fn dispatch_row(dispatch: &DispatchInfo, index: usize, p: Palette) -> Element<'static, Message> {
    let mut fanout = Vec::new();
    if dispatch.emitted_messages > 0 {
        fanout.push(format!("▸{} msgs", dispatch.emitted_messages));
    }
    if dispatch.emitted_events > 0 {
        fanout.push(format!("◆{} events", dispatch.emitted_events));
    }
    container(
        row![
            text(index).font(MONO).size(11).color(p.muted_2),
            text(dispatch.module.clone())
                .font(SANS)
                .size(12)
                .color(p.ink),
            text(dispatch.origin.clone())
                .font(MONO)
                .size(11)
                .color(p.muted_2),
            Space::new().width(Length::Fill),
            text(fanout.join("  "))
                .font(MONO)
                .size(10.5)
                .color(p.muted_3),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([8, 11])
    .style(move |_| rounded_surface(p.paper, p.border, RADIUS_SM))
    .into()
}

fn digest_line(label: &'static str, value: &str, p: Palette) -> Element<'static, Message> {
    row![
        text(label).font(SANS).size(10.5).color(p.muted).width(72),
        text(if value.is_empty() {
            "—".to_string()
        } else {
            value.to_string()
        })
        .font(MONO)
        .size(11.5)
        .color(p.ink_softer),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .into()
}

fn state_view(title: &'static str, detail: &'static str, p: Palette) -> Element<'static, Message> {
    let badge = container(icons::view(Icon::Explorer, 23.0, p.ink_soft))
        .width(42)
        .height(42)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(move |_| rounded_surface(p.hover, p.border_soft, RADIUS_SM));
    container(
        column![
            badge,
            text(title).font(SANS).size(14).color(p.muted_3),
            text(detail).font(SANS).size(11.5).color(p.muted_2),
        ]
        .spacing(9)
        .align_x(Alignment::Center)
        .max_width(420),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .padding(24)
    .into()
}

fn header_cell(label: &'static str, width: f32, p: Palette) -> Element<'static, Message> {
    text(label)
        .font(SANS)
        .size(10.5)
        .color(p.muted)
        .width(width)
        .into()
}

fn fill_header(label: &'static str, p: Palette) -> Element<'static, Message> {
    text(label)
        .font(SANS)
        .size(10.5)
        .color(p.muted)
        .width(Length::Fill)
        .into()
}

fn fixed_cell(
    value: String,
    width: f32,
    color: iced::Color,
    mono: bool,
) -> Element<'static, Message> {
    text(value)
        .font(if mono { MONO } else { SANS })
        .size(if mono { 11.5 } else { 11.0 })
        .color(color)
        .width(width)
        .into()
}

fn fill_cell(value: String, color: iced::Color, mono: bool) -> Element<'static, Message> {
    text(value)
        .font(if mono { MONO } else { SANS })
        .size(11.5)
        .color(color)
        .width(Length::Fill)
        .into()
}

fn card<'a>(body: impl Into<Element<'a, Message>>, p: Palette) -> Element<'a, Message> {
    container(body)
        .width(Length::Fill)
        .padding([11, 13])
        .style(move |_| rounded_surface(p.paper, p.border, RADIUS_MD))
        .into()
}

fn bare_button<'a>(label: &'a str, message: Message, p: Palette) -> Element<'a, Message> {
    let btn = button(text(label).font(SANS).size(11.5).color(p.muted_3))
        .padding(0)
        .style(|_, _| iced::widget::button::Style::default())
        .on_press(message);
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Button, label, btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn outline_button<'a>(label: &'a str, message: Message, p: Palette) -> Element<'a, Message> {
    let btn = button(text(label).font(SANS).size(11.5))
        .padding([7, 12])
        .style(move |_, _| iced::widget::button::Style {
            background: Some(Background::Color(p.paper)),
            text_color: p.ink_soft,
            border: Border {
                color: p.border_strong,
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            ..Default::default()
        })
        .on_press(message);
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Button, label, btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn short_hex(value: &str) -> String {
    if value.is_empty() {
        "—".into()
    } else if value.len() > 10 {
        format!("{}…", &value[..10])
    } else {
        value.into()
    }
}

fn surface(color: iced::Color) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(color)),
        ..Default::default()
    }
}

fn bottom_rule(color: iced::Color, border: iced::Color) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(color)),
        border: Border {
            color: border,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

fn rounded_surface(
    color: iced::Color,
    border: iced::Color,
    radius: f32,
) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(color)),
        border: Border {
            color: border,
            width: 1.0,
            radius: radius.into(),
        },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(height: u64, ops: Vec<RootOp>) -> BlockRecord {
        BlockRecord {
            height,
            hash: "aa".repeat(32),
            commit_hash: "bb".repeat(32),
            ops,
        }
    }

    fn op(disposition: Disposition) -> RootOp {
        RootOp {
            proposer: "cc".repeat(32),
            proposer_name: Some("Founder Rae".into()),
            disposition,
            target: "chat".into(),
            operations: Vec::new(),
            payload: "{\"Post\":{}}".into(),
            op_hash: "dd".repeat(32),
        }
    }

    #[test]
    fn explorer_preserves_loading_empty_error_and_populated_states() {
        let mut state = State::default();
        update(&mut state, Message::Service(ServiceEvent::Loaded(Ok(None))));
        assert_eq!(state.blocks, Resource::Empty);
        update(
            &mut state,
            Message::Service(ServiceEvent::Loaded(Err("offline".into()))),
        );
        assert_eq!(state.blocks, Resource::Error("offline".into()));
        update(
            &mut state,
            Message::Service(ServiceEvent::Loaded(Ok(Some(vec![block(
                7,
                vec![op(Disposition::Applied)],
            )])))),
        );
        assert!(matches!(state.blocks, Resource::Ready(_)));
    }

    #[test]
    fn detail_holds_an_immutable_block_snapshot() {
        let original = block(7, vec![op(Disposition::Applied)]);
        let mut state = State {
            blocks: Resource::Ready(vec![original.clone()]),
            ..State::default()
        };
        update(&mut state, Message::Open(7));
        assert_eq!(state.open, Some(original));
        state.blocks = Resource::Ready(vec![]);
        assert_eq!(state.open.as_ref().map(|block| block.height), Some(7));
    }

    #[test]
    fn focus_waits_for_data_then_clears_even_when_evicted() {
        let mut state = State::default();
        assert_eq!(update(&mut state, Message::Focus(9)), None);
        assert_eq!(state.pending_focus, Some(9));
        assert_eq!(
            update(
                &mut state,
                Message::Service(ServiceEvent::Loaded(Ok(Some(vec![block(7, vec![])]))))
            ),
            Some(Command::ClearFocus)
        );
        assert_eq!(state.pending_focus, None);
        assert_eq!(state.open, None);
    }

    #[test]
    fn idle_and_rejected_blocks_remain_distinct() {
        assert!(block(1, vec![]).ops.is_empty());
        assert_eq!(
            block(2, vec![op(Disposition::Rejected)]).ops[0].disposition,
            Disposition::Rejected
        );
    }
}
