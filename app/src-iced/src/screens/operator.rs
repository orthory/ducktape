//! Native node-operator surfaces: Node, Gateway, Modules, Sandbox, and Metrics.
//!
//! This module owns presentation state only. [`update`] emits typed [`Command`]s;
//! the host performs node I/O and returns a [`ServiceEvent`].

use iced::widget::{Space, button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Shadow, Vector};

use crate::icons::{self, Icon};
use crate::theme::{self, MONO, Palette, RADIUS_LG, RADIUS_MD, RADIUS_SM, SANS};

const PAGE_PAD: f32 = 22.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Node,
    Gateway,
    Modules,
    Sandbox,
    Metrics,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resource<T> {
    Loading,
    Empty,
    Error(String),
    Ready(T),
}

mod gateway;
mod metrics;
mod modules;
mod node;
mod sandbox;

pub use gateway::{
    GatewayData, GatewayDraft, GatewayMessage, GatewayRoute, GatewayState, RouteAudience,
    RouteHealth, RouteMethod, RouteTarget,
};
pub use metrics::{DataPlaneMetric, MetricsSnapshot, MetricsState, SyncPeerMetric};
pub use modules::{ModuleCategory, ModulesState};
pub use node::{ConnectionRow, LogLine, NodeMessage, NodeRole, NodeSnapshot, NodeState, NodeTab};
pub use sandbox::{
    CheckState, SandboxCheck, SandboxData, SandboxMessage, SandboxMode, SandboxState,
};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleRoot {
    pub id: String,
    pub root: String,
    pub category: ModuleCategory,
}

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub node: NodeState,
    pub gateway: GatewayState,
    pub modules: ModulesState,
    pub sandbox: SandboxState,
    pub metrics: MetricsState,
}

impl Default for State {
    fn default() -> Self {
        Self {
            node: NodeState {
                data: Resource::Loading,
                active_tab: NodeTab::Overview,
                copied: None,
                log_filter: String::new(),
                error: None,
            },
            gateway: GatewayState {
                data: Resource::Loading,
                draft: GatewayDraft::default(),
                selected: None,
                health: RouteHealth::Idle,
                busy: false,
                note: None,
            },
            modules: ModulesState {
                data: Resource::Loading,
                copied: None,
            },
            sandbox: SandboxState {
                data: Resource::Loading,
                chosen: None,
                applying: false,
                setup_check: None,
                error: None,
            },
            metrics: MetricsState {
                data: Resource::Loading,
                paused: false,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Load(Screen),
    Node(NodeMessage),
    Gateway(GatewayMessage),
    CopyModule { id: String, root: String },
    Sandbox(SandboxMessage),
    ToggleMetricsPause,
    Service(ServiceEvent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    LoadNode,
    LoadGateway,
    LoadGatewayRoute(String),
    LoadModules,
    LoadSandbox,
    LoadMetrics,
    StartNode,
    StopNode,
    CopyText(String),
    SaveGatewayRoute(GatewayDraft),
    RemoveGatewayRoute(String),
    CheckGatewayHealth(String),
    CreateGatewayStarter(GatewayDraft),
    CheckSandbox,
    ApplySandbox(SandboxMode),
    StartSandboxSetup { check: String, agent: String },
    PauseMetrics(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ServiceEvent {
    NodeLoaded(Result<Option<NodeSnapshot>, String>),
    GatewayLoaded(Result<Option<GatewayData>, String>),
    GatewayRouteLoaded(Result<GatewayDraft, String>),
    GatewayHealthChecked(Result<RouteHealth, String>),
    ModulesLoaded(Result<Option<Vec<ModuleRoot>>, String>),
    SandboxLoaded(Result<Option<SandboxData>, String>),
    MetricsLoaded(Result<Option<MetricsSnapshot>, String>),
    ActionFinished {
        screen: Screen,
        result: Result<(), String>,
    },
}

pub fn update(state: &mut State, message: Message) -> Option<Command> {
    match message {
        Message::Load(screen) => load(state, screen),
        Message::Node(message) => node::update(&mut state.node, message),
        Message::Gateway(message) => gateway::update(&mut state.gateway, message),
        Message::CopyModule { id, root } => modules::copy(&mut state.modules, id, root),
        Message::Sandbox(message) => sandbox::update(&mut state.sandbox, message),
        Message::ToggleMetricsPause => metrics::toggle_pause(&mut state.metrics),
        Message::Service(event) => service_event(state, event),
    }
}

fn load(state: &mut State, screen: Screen) -> Option<Command> {
    match screen {
        Screen::Node => {
            state.node.data = Resource::Loading;
            Some(Command::LoadNode)
        }
        Screen::Gateway => {
            state.gateway.data = Resource::Loading;
            Some(Command::LoadGateway)
        }
        Screen::Modules => {
            state.modules.data = Resource::Loading;
            Some(Command::LoadModules)
        }
        Screen::Sandbox => {
            state.sandbox.data = Resource::Loading;
            Some(Command::LoadSandbox)
        }
        Screen::Metrics => {
            state.metrics.data = Resource::Loading;
            Some(Command::LoadMetrics)
        }
    }
}

fn service_event(state: &mut State, event: ServiceEvent) -> Option<Command> {
    match event {
        ServiceEvent::NodeLoaded(result) => {
            state.node.data = resource(result);
            state.node.error = None;
        }
        ServiceEvent::GatewayLoaded(result) => state.gateway.data = resource(result),
        ServiceEvent::GatewayRouteLoaded(result) => match result {
            Ok(draft) => state.gateway.draft = draft,
            Err(error) => state.gateway.note = Some(error),
        },
        ServiceEvent::GatewayHealthChecked(result) => {
            state.gateway.health = result.unwrap_or(RouteHealth::Unavailable)
        }
        ServiceEvent::ModulesLoaded(result) => state.modules.data = resource(result),
        ServiceEvent::SandboxLoaded(result) => {
            state.sandbox.data = resource(result);
            state.sandbox.applying = false;
        }
        ServiceEvent::MetricsLoaded(result) => state.metrics.data = resource(result),
        ServiceEvent::ActionFinished { screen, result } => match (screen, result) {
            (Screen::Node, Ok(())) => return Some(Command::LoadNode),
            (Screen::Node, Err(error)) => state.node.error = Some(error),
            (Screen::Gateway, Ok(())) => {
                state.gateway.busy = false;
                state.gateway.note = Some("Route saved.".into());
                return Some(Command::LoadGateway);
            }
            (Screen::Gateway, Err(error)) => {
                state.gateway.busy = false;
                state.gateway.note = Some(error);
            }
            (Screen::Sandbox, Ok(())) => {
                state.sandbox.applying = false;
                return Some(Command::LoadSandbox);
            }
            (Screen::Sandbox, Err(error)) => {
                state.sandbox.applying = false;
                state.sandbox.error = Some(error);
            }
            _ => {}
        },
    }
    None
}

fn resource<T>(result: Result<Option<T>, String>) -> Resource<T> {
    match result {
        Ok(Some(value)) => Resource::Ready(value),
        Ok(None) => Resource::Empty,
        Err(error) => Resource::Error(error),
    }
}

pub fn view(state: &State, screen: Screen, mode: theme::Mode) -> Element<'_, Message> {
    let p = *theme::palette(mode);
    match screen {
        Screen::Node => node::view(&state.node, p),
        Screen::Gateway => gateway::view(&state.gateway, p),
        Screen::Modules => modules::view(&state.modules, p),
        Screen::Sandbox => sandbox::view(&state.sandbox, p),
        Screen::Metrics => metrics::view(&state.metrics, p),
    }
}

fn resource_screen<'a, T>(
    resource: &'a Resource<T>,
    title: &'static str,
    empty: &'static str,
    screen: Screen,
    icon: Icon,
    p: Palette,
) -> Element<'a, Message> {
    match resource {
        Resource::Loading => {
            center_state(&format!("Loading {title}"), "Reading node state…", icon, None, p)
        }
        // Empty is a resolved state, not a spinner: carry a Reload CTA so it is
        // visually and interactively distinct from Loading (which passes None).
        Resource::Empty => center_state(title, empty, icon, Some(screen), p),
        Resource::Error(error) => {
            error_state(&format!("{title} unavailable"), error, screen, icon, p)
        }
        Resource::Ready(_) => unreachable!("ready resources are rendered by their screen"),
    }
}

fn center_state<'a>(
    title: &str,
    detail: &str,
    icon: Icon,
    retry: Option<Screen>,
    p: Palette,
) -> Element<'a, Message> {
    let mut body = column![
        icon_tile(icon, 42.0, p),
        text(title.to_string()).font(SANS).size(14).color(p.muted_3),
        text(detail.to_string())
            .font(SANS)
            .size(11.5)
            .color(p.muted_2)
    ]
    .spacing(9)
    .align_x(Alignment::Center);
    if let Some(screen) = retry {
        body = body.push(outline_button("Reload", Message::Load(screen), true, p));
    }
    container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .padding(24)
        .into()
}

fn error_state<'a>(
    title: &str,
    detail: &'a str,
    screen: Screen,
    icon: Icon,
    p: Palette,
) -> Element<'a, Message> {
    container(
        column![
            icon_tile(icon, 42.0, p),
            text(title.to_string()).font(SANS).size(14).color(p.ink),
            text(detail).font(MONO).size(11.5).color(p.red),
            outline_button("Retry", Message::Load(screen), true, p)
        ]
        .spacing(9)
        .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .padding(24)
    .into()
}

fn screen_header(
    label: &'static str,
    count: Option<usize>,
    p: Palette,
) -> Element<'static, Message> {
    let mut content = row![text(label).font(SANS).size(16).color(p.ink)]
        .spacing(10)
        .align_y(Alignment::Center);
    if let Some(count) = count {
        content = content.push(text(count.to_string()).font(MONO).size(13).color(p.muted_2));
    }
    container(content)
        .width(Length::Fill)
        .height(56)
        .padding([0, 22])
        .align_y(Alignment::Center)
        .style(move |_| bottom_border(p.paper, p.border_soft))
        .into()
}

fn card<'a>(content: impl Into<Element<'a, Message>>, p: Palette) -> Element<'a, Message> {
    container(content)
        .width(Length::Fill)
        .padding(14)
        .style(move |_| card_style(p))
        .into()
}

fn card_style(p: Palette) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(p.paper)),
        border: Border {
            color: p.border,
            width: 1.0,
            radius: RADIUS_LG.into(),
        },
        shadow: Shadow {
            color: Color {
                a: 0.05,
                ..Color::from_rgb8(40, 38, 34)
            },
            offset: Vector::new(0.0, 1.0),
            blur_radius: 2.0,
        },
        ..Default::default()
    }
}

fn surface(color: Color) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(color)),
        ..Default::default()
    }
}

fn rounded_surface(color: Color, border: Color, radius: f32) -> iced::widget::container::Style {
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

fn bottom_border(bg: Color, border: Color) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(bg)),
        border: Border {
            color: border,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

fn top_border(bg: Color, border: Color) -> iced::widget::container::Style {
    bottom_border(bg, border)
}

fn section_label(label: &'static str, p: Palette) -> Element<'static, Message> {
    text(label).font(MONO).size(9.5).color(p.muted_2).into()
}

fn divider(p: Palette) -> Element<'static, Message> {
    container(Space::new().height(1))
        .width(Length::Fill)
        .style(move |_| surface(p.border))
        .into()
}

fn notice<'a>(copy: &'a str, p: Palette) -> Element<'a, Message> {
    container(text(copy).font(SANS).size(11.5).color(p.muted))
        .width(Length::Fill)
        .padding([10, 13])
        .style(move |_| rounded_surface(p.sunken, p.border, RADIUS_MD))
        .into()
}

fn warning<'a>(copy: &'a str, p: Palette) -> Element<'a, Message> {
    container(text(copy).font(SANS).size(11.5).color(p.amber))
        .width(Length::Fill)
        .padding([10, 13])
        .style(move |_| rounded_surface(p.danger_soft, p.danger_border, RADIUS_MD))
        .into()
}

fn error_banner<'a>(copy: &'a str, p: Palette) -> Element<'a, Message> {
    container(text(copy).font(SANS).size(12).color(p.danger))
        .width(Length::Fill)
        .padding([10, 13])
        .style(move |_| rounded_surface(p.danger_soft, p.danger_border, RADIUS_MD))
        .into()
}

fn icon_tile(icon: Icon, size: f32, p: Palette) -> Element<'static, Message> {
    container(icons::view(icon, size.min(22.0), p.muted_2))
        .width(size)
        .height(size)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(move |_| rounded_surface(p.sunken, p.border, 10.0))
        .into()
}

fn pill(label: impl ToString, tone: Color, p: Palette) -> Element<'static, Message> {
    container(
        row![
            container(Space::new().width(6).height(6))
                .style(move |_| rounded_surface(tone, tone, 99.0)),
            text(label.to_string()).font(MONO).size(10).color(tone)
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding([3, 9])
    .style(move |_| rounded_surface(p.sunken, p.border_soft, RADIUS_SM))
    .into()
}

fn stat_card(
    label: &'static str,
    value: String,
    hint: &'static str,
    p: Palette,
) -> Element<'static, Message> {
    container(
        column![
            text(label).font(MONO).size(8.5).color(p.muted_2),
            text(value).font(MONO).size(20).color(p.ink),
            text(hint).font(SANS).size(11).color(p.muted_2)
        ]
        .spacing(4),
    )
    .width(Length::Fill)
    .padding([12, 14])
    .style(move |_| rounded_surface(p.paper, p.border, RADIUS_LG))
    .into()
}

fn copy_value(
    label: &str,
    value: &str,
    copied: bool,
    key: &str,
    p: Palette,
) -> Element<'static, Message> {
    #[cfg(all(feature = "agent", debug_assertions))]
    let name = label.to_string();
    let btn = button(
        row![
            text(label.to_string())
                .font(MONO)
                .size(9)
                .color(p.muted_2)
                .width(130),
            text(short(value, 20, 12))
                .font(MONO)
                .size(11.5)
                .color(p.ink_soft),
            Space::new().width(Length::Fill),
            text(if copied { "COPIED" } else { "COPY" })
                .font(MONO)
                .size(9)
                .color(if copied { p.green } else { p.muted_2 }),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([10, 13])
    .style(move |_, _| iced::widget::button::Style {
        background: Some(Background::Color(if copied {
            p.danger_soft
        } else {
            p.sunken
        })),
        text_color: p.ink,
        border: Border {
            color: p.border,
            width: 1.0,
            radius: RADIUS_MD.into(),
        },
        ..Default::default()
    })
    .on_press(Message::Node(NodeMessage::Copy {
        key: key.to_string(),
        value: value.to_string(),
    }));
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Button, name, btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn segment_button<'a>(
    label: &'static str,
    active: bool,
    message: Message,
    p: Palette,
) -> Element<'a, Message> {
    let btn = button(text(label).font(SANS).size(11.5))
        .padding([6, 17])
        .style(move |_, _| iced::widget::button::Style {
            background: active.then_some(Background::Color(p.paper)),
            text_color: if active { p.ink } else { p.muted_2 },
            border: Border {
                color: if active {
                    p.border_strong
                } else {
                    Color::TRANSPARENT
                },
                width: 1.0,
                radius: RADIUS_MD.into(),
            },
            ..Default::default()
        })
        .on_press(message);
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Tab, label, btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn outline_button<'a>(
    label: impl ToString,
    message: Message,
    enabled: bool,
    p: Palette,
) -> Element<'a, Message> {
    let label = label.to_string();
    let button = button(text(label.clone()).font(SANS).size(11.5))
        .padding([7, 13])
        .style(move |_, status| iced::widget::button::Style {
            background: Some(Background::Color(
                if enabled && matches!(status, iced::widget::button::Status::Hovered) {
                    p.hover
                } else {
                    p.paper
                },
            )),
            text_color: if enabled { p.ink_soft } else { p.muted_2 },
            border: Border {
                color: if enabled {
                    p.border_strong
                } else {
                    p.border_soft
                },
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            ..Default::default()
        });
    let button = if enabled {
        button.on_press(message)
    } else {
        button
    };
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::Sem::new(iced_agent_plugin::Role::Button, label, button)
        .disabled(!enabled)
        .into();
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    button.into()
}

fn filled_button<'a>(
    label: impl ToString,
    message: Message,
    enabled: bool,
    p: Palette,
) -> Element<'a, Message> {
    let label = label.to_string();
    let button = button(text(label.clone()).font(SANS).size(11.5))
        .width(Length::Fill)
        .padding([8, 14])
        .style(move |_, _| iced::widget::button::Style {
            background: Some(Background::Color(if enabled {
                p.filled
            } else {
                p.border_soft
            })),
            text_color: if enabled { p.on_filled } else { p.muted_2 },
            border: Border {
                radius: RADIUS_SM.into(),
                ..Default::default()
            },
            ..Default::default()
        });
    let button = if enabled {
        button.on_press(message)
    } else {
        button
    };
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::Sem::new(iced_agent_plugin::Role::Button, label, button)
        .disabled(!enabled)
        .into();
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    button.into()
}

fn danger_button<'a>(
    label: impl ToString,
    message: Message,
    enabled: bool,
    p: Palette,
) -> Element<'a, Message> {
    let label = label.to_string();
    let button = button(text(label.clone()).font(SANS).size(11.5))
        .width(Length::Fill)
        .padding([8, 14])
        .style(move |_, _| iced::widget::button::Style {
            background: Some(Background::Color(if enabled {
                p.red
            } else {
                p.border_soft
            })),
            text_color: if enabled { p.paper } else { p.muted_2 },
            border: Border {
                radius: RADIUS_SM.into(),
                ..Default::default()
            },
            ..Default::default()
        });
    let button = if enabled {
        button.on_press(message)
    } else {
        button
    };
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::Sem::new(iced_agent_plugin::Role::Button, label, button)
        .disabled(!enabled)
        .into();
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    button.into()
}

fn toggle_button<'a>(
    label: impl ToString,
    active: bool,
    message: Message,
    enabled: bool,
    p: Palette,
) -> Element<'a, Message> {
    let label = label.to_string();
    let button = button(text(label.clone()).font(SANS).size(10.5))
        .padding([6, 10])
        .style(move |_, _| iced::widget::button::Style {
            background: Some(Background::Color(if active { p.filled } else { p.paper })),
            text_color: if active {
                p.on_filled
            } else if enabled {
                p.ink_soft
            } else {
                p.muted_2
            },
            border: Border {
                color: if active { p.filled } else { p.border_strong },
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            ..Default::default()
        });
    let button = if enabled {
        button.on_press(message)
    } else {
        button
    };
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::Sem::new(iced_agent_plugin::Role::Button, label, button)
        .disabled(!enabled)
        .into();
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    button.into()
}

fn labeled_input<'a>(
    label: &'static str,
    placeholder: &'static str,
    value: &'a str,
    on_input: impl Fn(String) -> Message + 'a,
    p: Palette,
) -> Element<'a, Message> {
    column![
        text(label).font(SANS).size(10).color(p.muted_3),
        sem_input(
            label,
            value,
            text_input(placeholder, value)
                .on_input(on_input)
                .padding([7, 8])
                .font(MONO)
                .size(11)
        )
    ]
    .spacing(5)
    .into()
}

/// Dev-only text-input tagging: wraps `input` in a `TextInput` semantic node
/// carrying `value`. Compiled out entirely unless the agent bridge is built.
#[cfg(all(feature = "agent", debug_assertions))]
fn sem_input<'a>(
    name: &'static str,
    value: &str,
    input: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    iced_agent_plugin::Sem::new(iced_agent_plugin::Role::TextInput, name, input)
        .value(value.to_string())
        .into()
}
#[cfg(not(all(feature = "agent", debug_assertions)))]
fn sem_input<'a>(
    _name: &'static str,
    _value: &str,
    input: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    input.into()
}

fn confirm_card(
    title: &str,
    detail: &str,
    confirm: &str,
    cancel: Message,
    accept: Message,
    p: Palette,
) -> Element<'static, Message> {
    container(
        column![
            text(title.to_string()).font(SANS).size(14).color(p.ink),
            text(detail.to_string())
                .font(SANS)
                .size(11.5)
                .color(p.muted_3),
            row![
                outline_button("Cancel", cancel, true, p),
                filled_button(confirm, accept, true, p)
            ]
            .spacing(7),
        ]
        .spacing(10),
    )
    .width(Length::Fill)
    .padding(15)
    .style(move |_| rounded_surface(p.paper, p.border_strong, RADIUS_LG))
    .into()
}

fn section_panel_header(
    label: &'static str,
    right: Option<String>,
    p: Palette,
) -> Element<'static, Message> {
    let mut line = row![
        text(label).font(MONO).size(9.5).color(p.muted_2),
        Space::new().width(Length::Fill)
    ]
    .align_y(Alignment::Center);
    if let Some(right) = right {
        line = line.push(text(right).font(MONO).size(10).color(p.muted_2));
    }
    line.into()
}

fn short(value: &str, start: usize, end: usize) -> String {
    if value.is_empty() {
        return "—".into();
    }
    // Node-status strings (peer keys, app_hash, module roots) come from the
    // connected node, which is untrusted in Remote mode; slice on char
    // boundaries so a multibyte value cannot panic the render loop.
    let count = value.chars().count();
    if count <= start + end + 1 {
        return value.into();
    }
    let head: String = value.chars().take(start).collect();
    let tail: String = value.chars().skip(count - end).collect();
    format!("{head}…{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_never_panics_on_multibyte_node_values() {
        // A multibyte codepoint straddling the byte offsets used to slice.
        let value = "aaaaaaaaaaaa\u{1F600}bbbbbbbbbb";
        let out = short(value, 12, 8);
        assert!(out.contains('…'));
        // Short and empty inputs are returned whole / dashed, not sliced.
        assert_eq!(short("", 12, 8), "—");
        assert_eq!(short("\u{20AC}\u{20AC}", 12, 8), "\u{20AC}\u{20AC}");
    }

    #[test]
    fn every_operator_screen_loads_through_a_typed_command() {
        let mut state = State::default();
        let cases = [
            (Screen::Node, Command::LoadNode),
            (Screen::Gateway, Command::LoadGateway),
            (Screen::Modules, Command::LoadModules),
            (Screen::Sandbox, Command::LoadSandbox),
            (Screen::Metrics, Command::LoadMetrics),
        ];
        for (screen, expected) in cases {
            assert_eq!(update(&mut state, Message::Load(screen)), Some(expected));
        }
    }

    #[test]
    fn invalid_gateway_drafts_never_cross_the_service_boundary() {
        let mut state = State::default();
        state.gateway.draft.label = "Bad Label".into();
        assert_eq!(
            update(&mut state, Message::Gateway(GatewayMessage::Save)),
            None
        );
        assert!(state.gateway.note.as_deref().unwrap().contains("lowercase"));

        state.gateway.draft.label = "api".into();
        state.gateway.draft.target = RouteTarget::LoopbackHttp;
        state.gateway.draft.port = "0".into();
        assert_eq!(
            update(&mut state, Message::Gateway(GatewayMessage::Save)),
            None
        );
        assert!(state.gateway.note.as_deref().unwrap().contains("port"));
    }

    #[test]
    fn duckfs_target_closes_loopback_only_options() {
        let mut state = State::default();
        state.gateway.draft.target = RouteTarget::LoopbackHttp;
        state.gateway.draft.allow_authorization = true;
        state.gateway.draft.allow_upgrade = true;
        state.gateway.draft.methods = vec![RouteMethod::Post];
        update(
            &mut state,
            Message::Gateway(GatewayMessage::SetTarget(RouteTarget::DuckFs)),
        );
        assert_eq!(
            state.gateway.draft.methods,
            vec![RouteMethod::Get, RouteMethod::Head]
        );
        assert!(!state.gateway.draft.allow_authorization);
        assert!(!state.gateway.draft.allow_upgrade);
    }

    #[test]
    fn sandbox_apply_requires_the_confirmation_step() {
        let mut state = State::default();
        assert_eq!(
            update(
                &mut state,
                Message::Sandbox(SandboxMessage::Choose(SandboxMode::Podman))
            ),
            None
        );
        assert_eq!(state.sandbox.chosen, Some(SandboxMode::Podman));
        assert_eq!(
            update(&mut state, Message::Sandbox(SandboxMessage::ConfirmApply)),
            Some(Command::ApplySandbox(SandboxMode::Podman))
        );
        assert!(state.sandbox.applying);
        assert_eq!(state.sandbox.chosen, None);
    }

    #[test]
    fn service_results_preserve_loading_empty_error_and_ready_states() {
        let mut state = State::default();
        update(
            &mut state,
            Message::Service(ServiceEvent::ModulesLoaded(Ok(None))),
        );
        assert_eq!(state.modules.data, Resource::Empty);
        update(
            &mut state,
            Message::Service(ServiceEvent::ModulesLoaded(Err("offline".into()))),
        );
        assert_eq!(state.modules.data, Resource::Error("offline".into()));
        update(
            &mut state,
            Message::Service(ServiceEvent::ModulesLoaded(Ok(Some(Vec::new())))),
        );
        assert_eq!(state.modules.data, Resource::Ready(Vec::new()));
    }
}
