use super::*;
use iced::widget::{column, row};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckState {
    Ok,
    Failed,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxCheck {
    pub id: String,
    pub label: String,
    pub detail: String,
    pub state: CheckState,
    pub fixable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxMode {
    Off,
    Podman,
    Tart,
}

impl SandboxMode {
    const fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Podman => "Podman",
            Self::Tart => "Tart",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxData {
    pub can_control: bool,
    pub backend: String,
    pub os: String,
    pub current_mode: SandboxMode,
    pub available_modes: Vec<SandboxMode>,
    pub serving: bool,
    pub checks: Vec<SandboxCheck>,
    pub active_agents: Vec<(String, String)>,
    pub active_channel: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxState {
    pub data: Resource<SandboxData>,
    pub chosen: Option<SandboxMode>,
    pub applying: bool,
    /// A one-shot success flash after an apply+restart, cleared on the next
    /// mode choice / re-check so the confirmation beat survives the reload.
    pub applied: bool,
    pub setup_check: Option<String>,
    /// Which active agent runs the canned setup, chosen by the operator; falls
    /// back to the first active agent when unset.
    pub setup_agent: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxMessage {
    Recheck,
    Choose(SandboxMode),
    CancelApply,
    ConfirmApply,
    ChooseSetupAgent(String),
    SetUpWithAgent { check: String, agent: String },
}

pub(super) fn update(state: &mut SandboxState, message: SandboxMessage) -> Option<Command> {
    match message {
        SandboxMessage::Recheck => {
            state.data = Resource::Loading;
            state.applied = false;
            return Some(Command::CheckSandbox);
        }
        SandboxMessage::Choose(mode) => {
            state.chosen = Some(mode);
            state.applied = false;
            state.error = None;
        }
        SandboxMessage::CancelApply => state.chosen = None,
        SandboxMessage::ConfirmApply => {
            let mode = state.chosen.take()?;
            state.applying = true;
            state.applied = false;
            state.error = None;
            return Some(Command::ApplySandbox(mode));
        }
        SandboxMessage::ChooseSetupAgent(agent) => state.setup_agent = Some(agent),
        SandboxMessage::SetUpWithAgent { check, agent } => {
            state.setup_check = Some(check.clone());
            return Some(Command::StartSandboxSetup { check, agent });
        }
    }
    None
}

pub(super) fn view(state: &SandboxState, p: Palette) -> Element<'_, Message> {
    let Resource::Ready(data) = &state.data else {
        return resource_screen(
            &state.data,
            "Sandbox",
            "Sandbox host checks are unavailable.",
            Screen::Sandbox,
            Icon::Sandbox,
            p,
        );
    };
    let header = section_header("Sandbox", None, None, p);

    let mut body = column![
        text("Choose how this node executes agent work, verify the host, and apply changes with a guarded restart.")
            .font(SANS).size(BODY).color(p.muted_2),
        row![
            text("Sandbox serving").font(SANS_SEMIBOLD).size(BODY_LG).color(p.ink),
            pill(if data.serving { "Serving" } else { "Not serving" }, if data.serving { p.green } else { p.amber }, p),
            text(format!("mode {}", data.current_mode.label())).font(MONO).size(CAPTION).color(p.muted_2),
            Space::new().width(Length::Fill),
            outline_button("Re-check", Message::Sandbox(SandboxMessage::Recheck), data.can_control, p),
        ].align_y(Alignment::Center).spacing(10),
        text("Nodes serve agent work only when opted in. Turning it on announces this node's executors and metered capacity into the capability registry.")
            .font(SANS).size(BODY).color(p.muted_3),
    ].spacing(8);
    if !data.can_control {
        body = body.push(warning("This app isn't managing a local node, so these checks can't reach the node host. Run the preflight on the machine that runs the node.", p));
    }
    let mut checks = column![
        text(format!("{} · {}", data.backend, data.os))
            .font(MONO)
            .size(CAPTION)
            .color(p.muted_2)
    ]
    .spacing(0);
    for check in &data.checks {
        checks = checks.push(divider_soft(p));
        checks = checks.push(check_row(check, data, state, p));
    }
    body = body.push(section_label("DETECTION", p)).push(card(checks, p));
    // Agent picker: when more than one active agent can run the canned setup,
    // let the operator choose which one instead of silently taking the first.
    if data.checks.iter().any(|check| check.fixable) && data.active_agents.len() > 1 {
        let selected = setup_agent(data, state);
        let mut picker = row![
            text("Run setup as")
                .font(SANS)
                .size(BODY)
                .color(p.muted_3)
        ]
        .spacing(7)
        .align_y(Alignment::Center);
        for (id, name) in &data.active_agents {
            picker = picker.push(toggle_button(
                name.clone(),
                selected.as_deref() == Some(id.as_str()),
                Message::Sandbox(SandboxMessage::ChooseSetupAgent(id.clone())),
                data.active_channel.is_some(),
                p,
            ));
        }
        body = body.push(picker);
    }
    body = body.push(section_label("OPT-IN SWITCH", p));
    let mut choices = row![].spacing(7).align_y(Alignment::Center);
    for mode in &data.available_modes {
        let is_current = data.current_mode == *mode;
        let button = toggle_button(
            mode.label(),
            state.chosen == Some(*mode),
            Message::Sandbox(SandboxMessage::Choose(*mode)),
            data.can_control && !state.applying && !is_current,
            p,
        );
        if is_current {
            choices = choices.push(
                row![
                    button,
                    container(text("CURRENT").font(MONO).size(CAPTION).color(p.green))
                        .padding([2, 6])
                        .style(move |_| rounded_surface(p.sunken, p.border_soft, RADIUS_SM)),
                ]
                .spacing(5)
                .align_y(Alignment::Center),
            );
        } else {
            choices = choices.push(button);
        }
    }
    let status = if let Some(error) = &state.error {
        format!("Apply failed: {error}")
    } else if state.applying {
        "Applying config and restarting the node…".into()
    } else if state.applied {
        "Applied. The node restarted with the selected mode.".into()
    } else {
        "Choose a mode to review and apply it.".into()
    };
    let status_color = if state.error.is_some() {
        p.red
    } else if state.applied {
        p.green
    } else {
        p.muted_3
    };
    body = body.push(card(
        column![choices, text(status).font(SANS).size(BODY).color(status_color)].spacing(10),
        p,
    ));
    if let Some(chosen) = state.chosen {
        body = body.push(confirm_card(
            &format!("Apply {}?", chosen.label()),
            "This updates this workspace's node config and restarts the local node. If the new node fails to start, the previous config is restored.",
            "Apply and restart",
            Message::Sandbox(SandboxMessage::CancelApply),
            Message::Sandbox(SandboxMessage::ConfirmApply),
            p,
        ));
    }
    container(column![
        header,
        scrollable(container(body).padding(Padding {
            top: 18.0,
            right: 22.0,
            bottom: 22.0,
            left: 22.0,
        }))
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| surface(p.canvas))
    .into()
}

/// The agent that runs the canned setup: the operator's pick if it is still
/// active, otherwise the first active agent.
fn setup_agent(data: &SandboxData, state: &SandboxState) -> Option<String> {
    state
        .setup_agent
        .clone()
        .filter(|id| data.active_agents.iter().any(|(agent, _)| agent == id))
        .or_else(|| data.active_agents.first().map(|(id, _)| id.clone()))
}

fn check_row(
    check: &SandboxCheck,
    data: &SandboxData,
    state: &SandboxState,
    p: Palette,
) -> Element<'static, Message> {
    let (glyph, color) = match check.state {
        CheckState::Ok => ("✓", p.green),
        CheckState::Failed => ("✕", p.red),
        CheckState::Unknown => ("?", p.amber),
    };
    let mut line = row![
        container(text(glyph).font(SANS).size(CAPTION).color(color))
            .width(19)
            .height(19)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(move |_| rounded_surface(p.sunken, p.border_soft, 99.0)),
        column![
            text(check.label.clone())
                .font(SANS)
                .size(BODY)
                .color(p.ink_soft),
            text(check.detail.clone())
                .font(MONO)
                .size(CAPTION)
                .color(p.muted_2),
        ]
        .spacing(2),
        Space::new().width(Length::Fill),
    ]
    .spacing(11)
    .align_y(Alignment::Center);
    if check.fixable {
        let enabled = !data.active_agents.is_empty() && data.active_channel.is_some();
        let agent = setup_agent(data, state).unwrap_or_default();
        line = line.push(outline_button(
            if state.setup_check.as_deref() == Some(check.id.as_str()) {
                "Setup run requested →"
            } else {
                "Set up with an agent"
            },
            Message::Sandbox(SandboxMessage::SetUpWithAgent {
                check: check.id.clone(),
                agent,
            }),
            enabled,
            p,
        ));
    }
    container(line)
        .width(Length::Fill)
        .padding([11, 13])
        .into()
}
