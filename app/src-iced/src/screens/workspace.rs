//! Native workspace connect, join-progress, and managed-node failure screens.
//!
//! The view owns presentation only. [`update`] emits a typed [`Command`]; the
//! host performs backend/transport work and returns a [`ServiceEvent`].

use iced::widget::{
    Column, Space, button, column, container, row, scrollable, text, text_input,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Shadow, Vector};

use crate::theme::{
    self, MONO, Palette, RADIUS_LG, RADIUS_MD, RADIUS_SM, SANS, SANS_MEDIUM, SANS_SEMIBOLD,
};

const CARD_WIDTH: f32 = 440.0;
const JOIN_WIDTH: f32 = 430.0;
const FAILED_WIDTH: f32 = 520.0;
const JOIN_POLL_MS: u64 = 1_500;
const COPY_FEEDBACK_MS: u64 = 1_200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Stage {
    #[default]
    Connect,
    Joining,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnectMode {
    #[default]
    Create,
    Join,
    Remote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub chain_id: String,
    pub pubkey: String,
    pub member: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Starting,
    Parked,
    Admitted,
    Synced,
    Promoted,
    Fatal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseReport {
    pub phase: Phase,
    pub detail: Option<String>,
}

impl PhaseReport {
    fn starting() -> Self {
        Self {
            phase: Phase::Starting,
            detail: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootErrorKind {
    StartupFailure,
    IncompatibleWorkspace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootFailure {
    pub kind: BootErrorKind,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootError {
    pub kind: BootErrorKind,
    pub workspace_id: String,
    pub reason: String,
    pub log_path: Option<String>,
    pub log_tail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogTail {
    pub path: String,
    pub tail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AccountLink {
    #[default]
    Hidden,
    Locked,
    PendingApproval,
    WillLink,
}

impl AccountLink {
    const fn hint(self) -> Option<&'static str> {
        match self {
            Self::Hidden => None,
            Self::Locked => Some(
                "Your account is locked — unlock it in the Account view to link this node to you.",
            ),
            Self::PendingApproval => {
                Some("Waiting for your other device to approve this device's link to your account.")
            }
            Self::WillLink => Some("This node will be linked to your account when it's admitted."),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgetRequest {
    pub workspace_id: String,
    pub force: bool,
}

#[derive(Debug, Default)]
pub struct State {
    pub stage: Stage,
    pub mode: ConnectMode,
    pub name: String,
    pub invite: String,
    pub remote_url: String,
    pub pending_remote: Option<String>,
    pub workspaces: Vec<Workspace>,
    pub workspace: Option<Workspace>,
    pub join_code: Option<String>,
    pub busy: bool,
    pub error: Option<String>,
    pub delete_needs_force: Option<String>,
    pub pending_forget: Option<ForgetRequest>,
    pub phase: Option<PhaseReport>,
    pub boot_error: Option<BootError>,
    pub show_log: bool,
    pub copied_node_key: bool,
    pub account_link: AccountLink,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Open,
    SelectMode(ConnectMode),
    NameChanged(String),
    InviteChanged(String),
    RemoteUrlChanged(String),
    Submit,
    Dismiss,
    SelectWorkspace(String),
    CopyJoinCode,
    CopyNodeKey,
    ClearCopied,
    RequestForget { workspace_id: String, force: bool },
    CancelForget,
    ConfirmForget,
    Cancel,
    Retry,
    ToggleLog,
    Service(ServiceEvent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionTarget {
    Workspace(String),
    Remote(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    LoadWorkspaces,
    LoadJoinCode,
    CreateWorkspace {
        name: String,
    },
    JoinWorkspace {
        name: String,
        invite: String,
    },
    ConnectRemote {
        url: String,
    },
    ActivateWorkspace {
        workspace_id: String,
    },
    /// A zero delay is the first probe. Repeat probes preserve the waiting
    /// room's 1.5-second cadence instead of busy-looping the backend.
    PollPhase {
        workspace_id: String,
        delay_ms: u64,
    },
    /// Report ready only after the expected node identity answers and the node
    /// has committed resident or validator standing.
    CheckJoinReady {
        workspace_id: String,
    },
    LoadLog {
        workspace_id: String,
    },
    RetryWorkspace {
        workspace_id: String,
    },
    CancelJoin {
        workspace_id: String,
    },
    ForgetWorkspace {
        workspace_id: String,
        force: bool,
    },
    CopyText(String),
    ClearCopiedAfter {
        millis: u64,
    },
    Connected(ConnectionTarget),
    Dismiss,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceEvent {
    WorkspacesLoaded(Result<Vec<Workspace>, String>),
    JoinCodeLoaded(Result<String, String>),
    WorkspaceCreated(Result<Workspace, String>),
    WorkspaceJoined(Result<Workspace, String>),
    RemoteConnected(Result<(), String>),
    WorkspaceActivated {
        workspace_id: String,
        result: Result<(), BootFailure>,
    },
    PhaseLoaded {
        workspace_id: String,
        result: Result<PhaseReport, String>,
    },
    JoinReady {
        workspace_id: String,
        ready: bool,
    },
    LogLoaded {
        workspace_id: String,
        result: Result<LogTail, String>,
    },
    WorkspaceForgot {
        workspace_id: String,
        force: bool,
        result: Result<Option<Workspace>, String>,
    },
    TextCopied(Result<(), String>),
}

pub fn update(state: &mut State, message: Message) -> Option<Command> {
    match message {
        Message::Open => {
            state.stage = Stage::Connect;
            state.error = None;
            state.boot_error = None;
            Some(Command::LoadWorkspaces)
        }
        Message::SelectMode(mode) => {
            state.mode = mode;
            state.error = None;
            if mode == ConnectMode::Join && state.join_code.is_none() {
                Some(Command::LoadJoinCode)
            } else {
                None
            }
        }
        Message::NameChanged(value) => {
            state.name = value;
            None
        }
        Message::InviteChanged(value) => {
            state.invite = sanitize_invite(&value);
            None
        }
        Message::RemoteUrlChanged(value) => {
            state.remote_url = value;
            None
        }
        Message::Submit => submit(state),
        Message::Dismiss => Some(Command::Dismiss),
        Message::SelectWorkspace(id) => select_workspace(state, &id),
        Message::CopyJoinCode => state.join_code.clone().map(Command::CopyText),
        Message::CopyNodeKey => {
            let key = state.workspace.as_ref()?.pubkey.clone();
            if key.is_empty() {
                return None;
            }
            state.copied_node_key = true;
            Some(Command::CopyText(key))
        }
        Message::ClearCopied => {
            state.copied_node_key = false;
            None
        }
        Message::RequestForget {
            workspace_id,
            force,
        } => {
            if state
                .workspaces
                .iter()
                .any(|workspace| workspace.id == workspace_id)
            {
                state.pending_forget = Some(ForgetRequest {
                    workspace_id,
                    force,
                });
            }
            None
        }
        Message::CancelForget => {
            state.pending_forget = None;
            None
        }
        Message::ConfirmForget => {
            let request = state.pending_forget.take()?;
            state.busy = true;
            state.error = None;
            state.delete_needs_force = None;
            Some(Command::ForgetWorkspace {
                workspace_id: request.workspace_id,
                force: request.force,
            })
        }
        Message::Cancel => cancel(state),
        Message::Retry => retry(state),
        Message::ToggleLog => {
            state.show_log = !state.show_log;
            None
        }
        Message::Service(event) => service_event(state, event),
    }
}

fn submit(state: &mut State) -> Option<Command> {
    if state.busy {
        return None;
    }
    let command = match state.mode {
        ConnectMode::Create => Command::CreateWorkspace {
            name: nonempty(&state.name)?,
        },
        ConnectMode::Join => Command::JoinWorkspace {
            name: nonempty(&state.name)?,
            invite: nonempty(&state.invite)?,
        },
        ConnectMode::Remote => Command::ConnectRemote {
            url: {
                let url = nonempty(&state.remote_url)?;
                state.pending_remote = Some(url.clone());
                url
            },
        },
    };
    state.busy = true;
    state.error = None;
    Some(command)
}

fn select_workspace(state: &mut State, id: &str) -> Option<Command> {
    let workspace = state
        .workspaces
        .iter()
        .find(|workspace| workspace.id == id)?
        .clone();
    state.busy = true;
    state.error = None;
    state.boot_error = None;
    if workspace.member {
        state.stage = Stage::Connect;
        state.phase = None;
    } else {
        state.stage = Stage::Joining;
        state.phase = Some(PhaseReport::starting());
    }
    state.workspace = Some(workspace);
    Some(Command::ActivateWorkspace {
        workspace_id: id.to_string(),
    })
}

fn cancel(state: &mut State) -> Option<Command> {
    match state.stage {
        Stage::Connect => Some(Command::Dismiss),
        Stage::Joining => {
            let workspace_id = state
                .workspace
                .as_ref()
                .map(|workspace| workspace.id.clone());
            state.stage = Stage::Connect;
            state.phase = None;
            state.busy = false;
            workspace_id.map(|workspace_id| Command::CancelJoin { workspace_id })
        }
        Stage::Failed => {
            state.stage = Stage::Connect;
            state.boot_error = None;
            state.show_log = false;
            state.busy = false;
            None
        }
    }
}

fn retry(state: &mut State) -> Option<Command> {
    let boot = state.boot_error.as_ref()?;
    if boot.kind == BootErrorKind::IncompatibleWorkspace || state.busy {
        return None;
    }
    state.busy = true;
    Some(Command::RetryWorkspace {
        workspace_id: boot.workspace_id.clone(),
    })
}

fn service_event(state: &mut State, event: ServiceEvent) -> Option<Command> {
    match event {
        ServiceEvent::WorkspacesLoaded(result) => {
            state.busy = false;
            match result {
                Ok(workspaces) => state.workspaces = workspaces,
                Err(error) => state.error = Some(error),
            }
            None
        }
        ServiceEvent::JoinCodeLoaded(result) => {
            match result {
                Ok(code) => state.join_code = Some(code),
                Err(error) => state.error = Some(error),
            }
            None
        }
        ServiceEvent::WorkspaceCreated(result) | ServiceEvent::WorkspaceJoined(result) => {
            workspace_materialized(state, result)
        }
        ServiceEvent::RemoteConnected(result) => {
            state.busy = false;
            match result {
                Ok(()) => Some(Command::Connected(ConnectionTarget::Remote(
                    state
                        .pending_remote
                        .take()
                        .unwrap_or_else(|| state.remote_url.trim().to_string()),
                ))),
                Err(error) => {
                    state.pending_remote = None;
                    state.error = Some(error);
                    None
                }
            }
        }
        ServiceEvent::WorkspaceActivated {
            workspace_id,
            result,
        } => workspace_activated(state, &workspace_id, result),
        ServiceEvent::PhaseLoaded {
            workspace_id,
            result,
        } => phase_loaded(state, &workspace_id, result),
        ServiceEvent::JoinReady {
            workspace_id,
            ready,
        } => {
            if !is_current_workspace(state, &workspace_id) || state.stage != Stage::Joining {
                return None;
            }
            if ready {
                state.busy = false;
                Some(Command::Connected(ConnectionTarget::Workspace(
                    workspace_id,
                )))
            } else {
                Some(Command::PollPhase {
                    workspace_id,
                    delay_ms: JOIN_POLL_MS,
                })
            }
        }
        ServiceEvent::LogLoaded {
            workspace_id,
            result,
        } => {
            if let Some(boot) = state
                .boot_error
                .as_mut()
                .filter(|boot| boot.workspace_id == workspace_id)
                && let Ok(log) = result
            {
                boot.log_path = Some(log.path);
                boot.log_tail = log.tail;
            }
            None
        }
        ServiceEvent::WorkspaceForgot {
            workspace_id,
            force,
            result,
        } => workspace_forgot(state, workspace_id, force, result),
        ServiceEvent::TextCopied(result) => {
            if result.is_err() {
                state.copied_node_key = false;
                None
            } else if state.copied_node_key {
                Some(Command::ClearCopiedAfter {
                    millis: COPY_FEEDBACK_MS,
                })
            } else {
                None
            }
        }
    }
}

fn workspace_materialized(state: &mut State, result: Result<Workspace, String>) -> Option<Command> {
    match result {
        Ok(workspace) => {
            upsert_workspace(&mut state.workspaces, &workspace);
            state.workspace = Some(workspace.clone());
            state.busy = true;
            state.error = None;
            if !workspace.member {
                state.stage = Stage::Joining;
                state.phase = Some(PhaseReport::starting());
            } else {
                state.stage = Stage::Connect;
                state.phase = None;
            }
            Some(Command::ActivateWorkspace {
                workspace_id: workspace.id,
            })
        }
        Err(error) => {
            state.busy = false;
            state.error = Some(error);
            None
        }
    }
}

fn workspace_activated(
    state: &mut State,
    workspace_id: &str,
    result: Result<(), BootFailure>,
) -> Option<Command> {
    if !is_current_workspace(state, workspace_id) {
        return None;
    }
    let member = state
        .workspace
        .as_ref()
        .is_some_and(|workspace| workspace.member);
    match result {
        Ok(()) if member => {
            state.busy = false;
            Some(Command::Connected(ConnectionTarget::Workspace(
                workspace_id.to_string(),
            )))
        }
        Ok(()) => {
            state.stage = Stage::Joining;
            state.busy = false;
            Some(Command::PollPhase {
                workspace_id: workspace_id.to_string(),
                delay_ms: 0,
            })
        }
        Err(failure) if member => {
            state.stage = Stage::Failed;
            state.busy = false;
            state.show_log = false;
            state.boot_error = Some(BootError {
                kind: failure.kind,
                workspace_id: workspace_id.to_string(),
                reason: failure.reason,
                log_path: None,
                log_tail: String::new(),
            });
            Some(Command::LoadLog {
                workspace_id: workspace_id.to_string(),
            })
        }
        Err(failure) => {
            state.stage = Stage::Joining;
            state.busy = false;
            state.error = Some(failure.reason.clone());
            state.phase = Some(PhaseReport {
                phase: Phase::Fatal,
                detail: Some(failure.reason),
            });
            None
        }
    }
}

fn phase_loaded(
    state: &mut State,
    workspace_id: &str,
    result: Result<PhaseReport, String>,
) -> Option<Command> {
    if !is_current_workspace(state, workspace_id) || state.stage != Stage::Joining {
        return None;
    }
    match result {
        Ok(report) => {
            let fatal = report.phase == Phase::Fatal;
            state.phase = Some(report);
            if fatal {
                state.busy = false;
                None
            } else {
                Some(Command::CheckJoinReady {
                    workspace_id: workspace_id.to_string(),
                })
            }
        }
        Err(error) => {
            state.busy = false;
            state.error = Some(error.clone());
            state.phase = Some(PhaseReport {
                phase: Phase::Fatal,
                detail: Some(error),
            });
            None
        }
    }
}

fn workspace_forgot(
    state: &mut State,
    workspace_id: String,
    force: bool,
    result: Result<Option<Workspace>, String>,
) -> Option<Command> {
    state.busy = false;
    let was_active = state.workspace.as_ref().map(|workspace| &workspace.id) == Some(&workspace_id);
    match result {
        Ok(next) => {
            state
                .workspaces
                .retain(|workspace| workspace.id != workspace_id);
            state.delete_needs_force = None;
            if was_active {
                state.workspace.clone_from(&next);
                next.map(|workspace| {
                    state.busy = true;
                    if workspace.member {
                        state.stage = Stage::Connect;
                        state.phase = None;
                    } else {
                        state.stage = Stage::Joining;
                        state.phase = Some(PhaseReport::starting());
                    }
                    Command::ActivateWorkspace {
                        workspace_id: workspace.id,
                    }
                })
            } else {
                None
            }
        }
        Err(error) => {
            if !force {
                state.delete_needs_force = Some(workspace_id);
            }
            state.error = Some(error);
            None
        }
    }
}

fn is_current_workspace(state: &State, id: &str) -> bool {
    state
        .workspace
        .as_ref()
        .map(|workspace| workspace.id.as_str())
        == Some(id)
}

fn upsert_workspace(workspaces: &mut Vec<Workspace>, workspace: &Workspace) {
    match workspaces
        .iter_mut()
        .find(|candidate| candidate.id == workspace.id)
    {
        Some(existing) => existing.clone_from(workspace),
        None => workspaces.push(workspace.clone()),
    }
}

fn sanitize_invite(raw: &str) -> String {
    raw.chars()
        .filter(|character| {
            !character.is_whitespace()
                && !matches!(*character, '\u{200b}'..='\u{200d}' | '\u{2060}')
        })
        .collect()
}

fn nonempty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

pub fn view(state: &State, mode: theme::Mode) -> Element<'_, Message> {
    let palette = *theme::palette(mode);
    if state.pending_forget.is_some() {
        return forget_confirmation(state, palette);
    }
    match state.stage {
        Stage::Connect => connect_view(state, palette),
        Stage::Joining => join_progress(state, palette),
        Stage::Failed => node_failed(state, palette),
    }
}

fn connect_view(state: &State, p: Palette) -> Element<'_, Message> {
    let (title, subtitle) = match state.mode {
        ConnectMode::Create => (
            "Name your network",
            "Found a new network — your account becomes its first member; this device runs its first node.",
        ),
        ConnectMode::Join => (
            "Join a network",
            "Paste an invite from a member — this device joins their network with a fresh node key, owned by your account.",
        ),
        ConnectMode::Remote => (
            "Connect to a remote node",
            "Enter the http address of a node running on another device. It stays running there — this app just connects to it.",
        ),
    };

    let header = row![
        column![
            text(title).font(SANS_SEMIBOLD).size(16).color(p.ink),
            text(subtitle).font(SANS_MEDIUM).size(12).color(p.muted),
        ]
        .spacing(5)
        .width(Length::Fill),
        icon_close(p),
    ]
    .spacing(10)
    .align_y(Alignment::Start);

    let tabs = container(
        row![
            tab(
                "Create",
                state.mode == ConnectMode::Create,
                ConnectMode::Create,
                p
            ),
            tab(
                "Join",
                state.mode == ConnectMode::Join,
                ConnectMode::Join,
                p
            ),
            tab(
                "Remote",
                state.mode == ConnectMode::Remote,
                ConnectMode::Remote,
                p
            ),
        ]
        .spacing(4),
    )
    .padding(4)
    .style(move |_| rounded(p.panel, RADIUS_MD));

    let fields: Element<'_, Message> = match state.mode {
        ConnectMode::Remote => {
            let input = field(
                "http://192.168.1.50:8844",
                &state.remote_url,
                Message::RemoteUrlChanged,
                MONO,
                p,
            )
            .on_submit(Message::Submit);
            container(sem_input("Node address", &state.remote_url, input)).into()
        }
        ConnectMode::Create => {
            let input = field("Network name", &state.name, Message::NameChanged, SANS, p)
                .on_submit(Message::Submit);
            container(sem_input("Network name", &state.name, input)).into()
        }
        ConnectMode::Join => {
            let invite = field(
                "Paste invite blob (🦆…)",
                &state.invite,
                Message::InviteChanged,
                MONO,
                p,
            )
            .padding([18, 10]);
            let name = field("Network name", &state.name, Message::NameChanged, SANS, p);
            column![
                sem_input("Network name", &state.name, name),
                join_code_card(state, p),
                sem_input("Invite blob", &state.invite, invite),
            ]
            .spacing(10)
            .into()
        }
    };
    let mut content = column![header, tabs, fields].spacing(16);

    if let Some(error) = &state.error {
        content = content.push(text(error).font(MONO).size(11.5).color(p.red));
    }
    let enabled = !state.busy && can_submit(state);
    let submit_label = if state.busy {
        "Setting up…"
    } else {
        match state.mode {
            ConnectMode::Create => "Create network",
            ConnectMode::Join => "Join network",
            ConnectMode::Remote => "Connect",
        }
    };
    content = content.push(primary(submit_label, enabled.then_some(Message::Submit), p));
    if !state.workspaces.is_empty() {
        content = content.push(networks(state, p));
    }

    modal_root(card(content, CARD_WIDTH, p), p)
}

fn can_submit(state: &State) -> bool {
    match state.mode {
        ConnectMode::Create => !state.name.trim().is_empty(),
        ConnectMode::Join => !state.name.trim().is_empty() && !state.invite.trim().is_empty(),
        ConnectMode::Remote => !state.remote_url.trim().is_empty(),
    }
}

fn join_code_card(state: &State, p: Palette) -> Element<'_, Message> {
    let code = state.join_code.as_deref().unwrap_or("generating…");
    container(
        column![
            text("YOUR JOIN CODE")
                .font(SANS_SEMIBOLD)
                .size(10.5)
                .color(p.muted_2),
            text("Send this code to whoever is inviting you — invites are locked to it.")
                .font(SANS_MEDIUM)
                .size(11)
                .color(p.muted),
            row![
                text(code)
                    .font(MONO)
                    .size(10.5)
                    .color(p.ink_soft)
                    .width(Length::Fill),
                outline_enabled("Copy", Message::CopyJoinCode, state.join_code.is_some(), p,),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(6),
    )
    .padding([10, 11])
    .style(move |_| bordered(p.paper, p.border, RADIUS_SM))
    .into()
}

fn networks(state: &State, p: Palette) -> Element<'_, Message> {
    let mut list = Column::new().spacing(6).push(
        text("YOUR NETWORKS")
            .font(SANS_SEMIBOLD)
            .size(10.5)
            .color(p.muted_2),
    );
    for workspace in &state.workspaces {
        let select = button(
            row![
                text(workspace.name.clone()).font(SANS_SEMIBOLD).size(12),
                Space::new().width(Length::Fill),
                text(workspace.chain_id.clone())
                    .font(MONO)
                    .size(10)
                    .color(p.muted_2),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .padding([9, 11])
        .on_press(Message::SelectWorkspace(workspace.id.clone()))
        .style(move |_, status| outline_style(p, true, status));
        #[cfg(all(feature = "agent", debug_assertions))]
        let select = iced_agent_plugin::sem(
            iced_agent_plugin::Role::ListItem,
            workspace.name.clone(),
            select,
        );
        let force = state.delete_needs_force.as_deref() == Some(&workspace.id);
        let label = if force { "Force delete" } else { "Delete" };
        let remove = danger_button(
            label,
            Message::RequestForget {
                workspace_id: workspace.id.clone(),
                force,
            },
            force,
            p,
        );
        list = list.push(row![select, remove].spacing(6).align_y(Alignment::Center));
    }
    column![
        container(Space::new().height(1))
            .width(Length::Fill)
            .style(move |_| rounded(p.border, 0.0)),
        Space::new().height(13),
        list,
    ]
    .spacing(0)
    .into()
}

fn join_progress(state: &State, p: Palette) -> Element<'_, Message> {
    let report = state.phase.as_ref();
    let phase = report.map(|report| report.phase).unwrap_or(Phase::Starting);
    let phase_detail = report.and_then(|report| report.detail.clone());
    let current = phase_step(phase);
    let fatal = phase == Phase::Fatal;
    let name = state
        .workspace
        .as_ref()
        .map(|workspace| workspace.name.as_str())
        .unwrap_or("workspace");
    let pubkey = state
        .workspace
        .as_ref()
        .map(|workspace| workspace.pubkey.clone())
        .unwrap_or_default();

    let mut content = Column::new().spacing(0);
    if state.workspaces.len() <= 1 {
        content = content.push(step_rail(p));
    }
    content = content
        .push(Space::new().height(13))
        .push(
            text(if fatal {
                "Join needs attention".to_string()
            } else {
                format!("Joining {name}")
            })
            .font(SANS_SEMIBOLD)
            .size(20)
            .color(p.filled),
        )
        .push(Space::new().height(5))
        .push(
            text("Parked nodes wait for admission, then sync finalized history and promote.")
                .font(SANS)
                .size(13)
                .color(p.muted),
        )
        .push(Space::new().height(18))
        .push(progress_bar(
            if fatal {
                12
            } else {
                ((current.max(0) + 1) * 25) as u16
            },
            fatal,
            p,
        ))
        .push(Space::new().height(20))
        .push(text("THIS NODE'S KEY").font(MONO).size(10).color(p.muted_2))
        .push(Space::new().height(8))
        .push(node_key(pubkey, state.copied_node_key, p));
    if let Some(hint) = state.account_link.hint() {
        content = content
            .push(Space::new().height(8))
            .push(text(hint).font(SANS).size(10.5).color(p.muted_2));
    }
    content = content.push(Space::new().height(22));
    for (index, (label, detail)) in JOIN_STEPS.iter().enumerate() {
        let index = index as i8;
        let visual = if fatal && index == 0 {
            StepVisual::Failed
        } else if !fatal && index < current {
            StepVisual::Done
        } else if !fatal && index == current {
            StepVisual::Running
        } else {
            StepVisual::Pending
        };
        let copy = if visual == StepVisual::Failed {
            phase_detail
                .clone()
                .unwrap_or_else(|| "the node failed to join".to_string())
        } else if visual == StepVisual::Running {
            phase_detail.clone().unwrap_or_else(|| detail.to_string())
        } else {
            detail.to_string()
        };
        content = content.push(join_step(label, copy, visual, p));
        if index < JOIN_STEPS.len() as i8 - 1 {
            content = content.push(Space::new().height(14));
        }
    }
    if fatal {
        content = content.push(Space::new().height(18)).push(fatal_line(
            phase_detail.unwrap_or_else(|| "the node failed to join".to_string()),
            p,
        ));
    }
    content =
        content
            .push(Space::new().height(28))
            .push(link_button("workspaces", Message::Cancel, p));

    container(scrollable(
        container(content).max_width(JOIN_WIDTH).width(Length::Fill),
    ))
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(30)
    .center_x(Length::Fill)
    .style(move |_| rounded(p.sunken, 0.0))
    .into()
}

const JOIN_STEPS: [(&str, &str); 4] = [
    (
        "Joining the network",
        "Tunnel and announce are up — the invite redeems automatically",
    ),
    (
        "Invite redeemed",
        "Full-node standing is recorded in the network",
    ),
    ("Finalized history synced", "Projection catches up locally"),
    ("Running as a full node", "The console opens automatically"),
];

fn phase_step(phase: Phase) -> i8 {
    match phase {
        Phase::Starting | Phase::Parked => 0,
        Phase::Admitted => 1,
        Phase::Synced => 2,
        Phase::Promoted => 3,
        Phase::Fatal => -1,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StepVisual {
    Done,
    Running,
    Pending,
    Failed,
}

fn join_step(
    label: &'static str,
    detail: String,
    visual: StepVisual,
    p: Palette,
) -> Element<'static, Message> {
    let active = visual != StepVisual::Pending;
    let detail_color = match visual {
        StepVisual::Failed => p.red,
        StepVisual::Running => p.muted_3,
        StepVisual::Done | StepVisual::Pending => p.muted_2,
    };
    row![
        step_icon(visual, p),
        column![
            text(label)
                .font(SANS)
                .size(13.5)
                .color(if active { p.ink_soft } else { p.muted_2 }),
            text(detail).font(MONO).size(11).color(detail_color),
        ]
        .spacing(2),
    ]
    .spacing(12)
    .align_y(Alignment::Start)
    .into()
}

fn step_icon(visual: StepVisual, p: Palette) -> Element<'static, Message> {
    let (label, background, border, color) = match visual {
        StepVisual::Done => ("✓", green_tint(p), p.green, p.green),
        StepVisual::Running => ("•", Color::TRANSPARENT, p.amber, p.amber),
        StepVisual::Pending => ("", Color::TRANSPARENT, p.border_strong, p.muted_2),
        StepVisual::Failed => ("!", p.danger_soft, p.danger_border, p.red),
    };
    container(text(label).font(MONO).size(11).color(color))
        .width(19)
        .height(19)
        .center_x(19)
        .center_y(19)
        .style(move |_| bordered(background, border, 10.0))
        .into()
}

fn progress_bar(progress: u16, fatal: bool, p: Palette) -> Element<'static, Message> {
    let progress = progress.clamp(1, 100);
    let filled = container(Space::new())
        .width(Length::FillPortion(progress))
        .height(5)
        .style(move |_| rounded(if fatal { p.red } else { p.filled }, 3.0));
    let empty = Space::new().width(Length::FillPortion(100 - progress));
    container(row![filled, empty].spacing(0))
        .height(5)
        .style(move |_| rounded(p.hover, 3.0))
        .into()
}

fn node_key(pubkey: String, copied: bool, p: Palette) -> Element<'static, Message> {
    let empty = pubkey.is_empty();
    let label = if empty {
        "waiting for identity".to_string()
    } else {
        pubkey
    };
    let btn = button(
        row![
            text(label)
                .font(MONO)
                .size(11)
                .color(p.muted_3)
                .width(Length::Fill),
            text(if copied { "copied" } else { "copy" })
                .font(SANS_SEMIBOLD)
                .size(11)
                .color(if copied { p.green } else { theme::ACCENTS[0] }),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([10, 12])
    .on_press_maybe((!empty).then_some(Message::CopyNodeKey))
    .style(move |_, _| iced::widget::button::Style {
        background: Some(Background::Color(p.sunken)),
        text_color: p.ink,
        border: Border {
            color: p.border,
            width: 1.0,
            radius: RADIUS_MD.into(),
        },
        ..Default::default()
    });
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::Sem::new(iced_agent_plugin::Role::Button, "Copy node key", btn)
        .disabled(empty)
        .into();
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn fatal_line(copy: String, p: Palette) -> Element<'static, Message> {
    container(text(copy).font(MONO).size(11).color(p.red))
        .width(Length::Fill)
        .padding([8, 10])
        .style(move |_| bordered(p.danger_soft, p.danger_border, RADIUS_SM))
        .into()
}

fn node_failed(state: &State, p: Palette) -> Element<'_, Message> {
    let Some(boot) = &state.boot_error else {
        return modal_root(Space::new(), p);
    };
    let name = state
        .workspace
        .as_ref()
        .map(|workspace| workspace.name.as_str())
        .unwrap_or("this workspace");
    let incompatible = boot.kind == BootErrorKind::IncompatibleWorkspace;
    let title = if incompatible {
        format!("Workspace update required for “{name}”")
    } else {
        format!("The node for “{name}” failed to start")
    };
    let reason = if incompatible {
        "This workspace was created with an incompatible state schema. Its data has not been changed. Archive the workspace directory before creating a fresh workspace; keep the existing workspace, node identity, and Ducktape account key so they can be recovered or exported later."
    } else {
        boot.reason.as_str()
    };

    let mut actions = row![].spacing(8).align_y(Alignment::Center);
    if incompatible {
        actions = actions.push(danger_primary("Create fresh workspace", Message::Cancel, p));
    } else {
        actions = actions
            .push(danger_primary("Retry", Message::Retry, p))
            .push(ghost("Choose another workspace", Message::Cancel, p));
    }
    if !boot.log_tail.trim().is_empty() {
        actions = actions.push(ghost(
            if state.show_log {
                "Hide daemon.log"
            } else {
                "Open daemon.log"
            },
            Message::ToggleLog,
            p,
        ));
    }

    let mut content = column![
        text(title).font(SANS_SEMIBOLD).size(13).color(p.danger),
        Space::new().height(6),
        text(reason).font(MONO).size(11.5).color(p.ink),
        Space::new().height(14),
        actions,
    ]
    .spacing(0);
    if state.show_log && !boot.log_tail.trim().is_empty() {
        content = content.push(Space::new().height(14)).push(
            container(scrollable(
                text(&boot.log_tail).font(MONO).size(10.5).color(p.ink_soft),
            ))
            .height(260)
            .padding(10)
            .style(move |_| bordered(p.paper, p.border, RADIUS_MD)),
        );
    }
    if let Some(path) = &boot.log_path {
        content = content
            .push(Space::new().height(10))
            .push(text(path).font(MONO).size(10).color(p.muted));
    }

    let failure = container(content)
        .width(Length::Fill)
        .max_width(FAILED_WIDTH)
        .padding(20)
        .style(move |_| bordered(p.danger_soft, p.danger_border, RADIUS_LG));
    container(failure)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(24)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(move |_| rounded(p.paper, 0.0))
        .into()
}

fn forget_confirmation(state: &State, p: Palette) -> Element<'_, Message> {
    let request = state.pending_forget.as_ref().expect("checked by view");
    let workspace = state
        .workspaces
        .iter()
        .find(|workspace| workspace.id == request.workspace_id);
    let name = workspace
        .map(|workspace| workspace.name.as_str())
        .unwrap_or("this network");
    let title = if request.force {
        format!("Force-delete {name}?")
    } else {
        format!("Delete {name}?")
    };
    let copy = if request.force {
        "Its node could not confirm it has left its validator set. Forcing deletes the network without that confirmation: its directory, node key, and registry entry are removed for good. Only do this for a solo or defunct network."
    } else {
        "This stops its node and deletes the network locally: directory, node key, and registry entry. It is refused while its node is still a current validator of a network with other members."
    };
    let confirm = if request.force {
        "Force delete"
    } else {
        "Delete network"
    };
    let content = column![
        text(title).font(SANS_SEMIBOLD).size(15).color(p.filled),
        Space::new().height(8),
        text(copy).font(SANS).size(12).color(p.muted_3),
        Space::new().height(16),
        row![
            Space::new().width(Length::Fill),
            dialog_button("Cancel", Message::CancelForget, false, p),
            dialog_button(confirm, Message::ConfirmForget, true, p),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    ]
    .spacing(0);
    let confirmation = container(content)
        .width(Length::Fill)
        .max_width(390)
        .padding(16)
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(p.paper)),
            border: Border {
                color: p.danger_border,
                width: 1.0,
                radius: RADIUS_LG.into(),
            },
            shadow: pop_shadow(),
            ..Default::default()
        });
    modal_root(confirmation, p)
}

fn icon_close(p: Palette) -> Element<'static, Message> {
    let btn = button(text("✕").font(SANS_MEDIUM).size(16))
        .width(26)
        .height(26)
        .padding(0)
        .on_press(Message::Dismiss)
        .style(move |_, status| iced::widget::button::Style {
            background: matches!(status, iced::widget::button::Status::Hovered)
                .then_some(Background::Color(p.hover)),
            text_color: p.muted,
            border: Border {
                radius: RADIUS_SM.into(),
                ..Default::default()
            },
            ..Default::default()
        });
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Button, "Close", btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn field<'a>(
    placeholder: &str,
    value: &'a str,
    on_input: impl Fn(String) -> Message + 'a,
    font: iced::Font,
    p: Palette,
) -> iced::widget::TextInput<'a, Message> {
    text_input(placeholder, value)
        .on_input(on_input)
        .padding([9, 11])
        .size(12.5)
        .font(font)
        .style(move |_, status| iced::widget::text_input::Style {
            background: Background::Color(p.sunken),
            border: Border {
                color: if matches!(status, iced::widget::text_input::Status::Focused { .. }) {
                    theme::ACCENTS[0]
                } else {
                    p.border_strong
                },
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            icon: p.muted,
            placeholder: p.muted_2,
            value: p.ink,
            selection: theme::ACCENTS[0],
        })
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

fn tab(
    label: &'static str,
    active: bool,
    mode: ConnectMode,
    p: Palette,
) -> Element<'static, Message> {
    let btn = button(
        container(text(label).font(SANS_SEMIBOLD).size(12))
            .width(Length::Fill)
            .center_x(Length::Fill),
    )
    .width(Length::FillPortion(1))
    .padding([8, 0])
    .on_press(Message::SelectMode(mode))
    .style(move |_, status| iced::widget::button::Style {
        background: (active || matches!(status, iced::widget::button::Status::Hovered))
            .then_some(Background::Color(if active { p.paper } else { p.hover })),
        text_color: if active { p.ink } else { p.muted },
        border: Border {
            radius: RADIUS_SM.into(),
            ..Default::default()
        },
        shadow: if active {
            Shadow {
                color: Color {
                    a: 0.05,
                    ..Color::from_rgb8(40, 38, 34)
                },
                offset: Vector::new(0.0, 1.0),
                blur_radius: 2.0,
            }
        } else {
            Shadow::default()
        },
        ..Default::default()
    });
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Tab, label, btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn primary<'a>(label: &'a str, message: Option<Message>, p: Palette) -> Element<'a, Message> {
    let enabled = message.is_some();
    let btn = button(
        container(text(label).font(SANS_SEMIBOLD).size(12.5))
            .width(Length::Fill)
            .center_x(Length::Fill),
    )
    .width(Length::Fill)
    .padding([10, 0])
    .on_press_maybe(message)
    .style(move |_, status| iced::widget::button::Style {
        background: Some(Background::Color(if !enabled {
            p.chip
        } else if matches!(status, iced::widget::button::Status::Hovered) {
            p.ink_soft
        } else {
            p.filled
        })),
        text_color: if enabled { p.on_filled } else { p.muted_3 },
        border: Border {
            radius: RADIUS_MD.into(),
            ..Default::default()
        },
        ..Default::default()
    });
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::Sem::new(iced_agent_plugin::Role::Button, label, btn)
        .disabled(!enabled)
        .into();
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn outline_enabled(
    label: &'static str,
    message: Message,
    enabled: bool,
    p: Palette,
) -> Element<'static, Message> {
    let btn = button(text(label).font(SANS_SEMIBOLD).size(10.5))
        .padding([5, 10])
        .on_press_maybe(enabled.then_some(message))
        .style(move |_, status| outline_style(p, enabled, status));
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::Sem::new(iced_agent_plugin::Role::Button, label, btn)
        .disabled(!enabled)
        .into();
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn ghost(label: &'static str, message: Message, p: Palette) -> Element<'static, Message> {
    let btn = button(text(label).font(SANS_SEMIBOLD).size(11))
        .padding([6, 12])
        .on_press(message)
        .style(move |_, status| outline_style(p, true, status));
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Button, label, btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn danger_button(
    label: &'static str,
    message: Message,
    filled: bool,
    p: Palette,
) -> Element<'static, Message> {
    let btn = button(text(label).font(SANS_SEMIBOLD).size(10.5))
        .padding([8, 10])
        .on_press(message)
        .style(move |_, status| iced::widget::button::Style {
            background: Some(Background::Color(if filled {
                p.red
            } else if matches!(status, iced::widget::button::Status::Hovered) {
                p.hover
            } else {
                p.paper
            })),
            text_color: if filled { p.on_filled } else { p.red },
            border: Border {
                color: if filled { p.red } else { p.border },
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            ..Default::default()
        });
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Button, label, btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn danger_primary(label: &'static str, message: Message, p: Palette) -> Element<'static, Message> {
    let btn = button(text(label).font(SANS_SEMIBOLD).size(11))
        .padding([7, 14])
        .on_press(message)
        .style(move |_, status| iced::widget::button::Style {
            background: Some(Background::Color(
                if matches!(status, iced::widget::button::Status::Hovered) {
                    p.red
                } else {
                    p.danger
                },
            )),
            text_color: Color::WHITE,
            border: Border {
                radius: RADIUS_MD.into(),
                ..Default::default()
            },
            ..Default::default()
        });
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Button, label, btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn dialog_button(
    label: &'static str,
    message: Message,
    danger: bool,
    p: Palette,
) -> Element<'static, Message> {
    let btn = button(text(label).font(SANS_SEMIBOLD).size(12))
        .height(32)
        .padding([0, 12])
        .on_press(message)
        .style(move |_, status| iced::widget::button::Style {
            background: Some(Background::Color(if danger {
                p.red
            } else if matches!(status, iced::widget::button::Status::Hovered) {
                p.hover
            } else {
                p.paper
            })),
            text_color: if danger { p.on_filled } else { p.ink_soft },
            border: Border {
                color: if danger { p.red } else { p.border_strong },
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            ..Default::default()
        });
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Button, label, btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn link_button(label: &'static str, message: Message, p: Palette) -> Element<'static, Message> {
    let btn = button(
        container(text(label).font(SANS_SEMIBOLD).size(11).color(p.muted))
            .width(Length::Fill)
            .center_x(Length::Fill),
    )
    .width(Length::Fill)
    .padding(4)
    .on_press(message)
    .style(|_, _| iced::widget::button::Style::default());
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Link, label, btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn step_rail(p: Palette) -> Element<'static, Message> {
    let mut rail = row![].spacing(9).align_y(Alignment::Center);
    for (index, label) in ["Account", "Workspace", "Connect"].into_iter().enumerate() {
        let step = index + 1;
        let done = step < 3;
        let current = step == 3;
        let marker = container(
            text(if done { "✓".into() } else { step.to_string() })
                .font(MONO)
                .size(9)
                .color(if current {
                    p.on_filled
                } else if done {
                    p.green
                } else {
                    p.muted_2
                }),
        )
        .width(17)
        .height(17)
        .center_x(17)
        .center_y(17)
        .style(move |_| {
            bordered(
                if current {
                    p.filled
                } else if done {
                    green_tint(p)
                } else {
                    Color::TRANSPARENT
                },
                if current {
                    p.filled
                } else if done {
                    p.green
                } else {
                    p.border_strong
                },
                9.0,
            )
        });
        rail = rail.push(marker).push(
            text(label)
                .font(SANS_SEMIBOLD)
                .size(10.5)
                .color(if current { p.ink } else { p.muted_2 }),
        );
        if step < 3 {
            rail = rail.push(
                container(Space::new().width(22).height(1))
                    .style(move |_| rounded(p.border_strong, 0.0)),
            );
        }
    }
    rail.into()
}

fn card<'a>(
    content: impl Into<Element<'a, Message>>,
    width: f32,
    p: Palette,
) -> Element<'a, Message> {
    container(content)
        .width(Length::Fill)
        .max_width(width)
        .padding(24)
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(p.sidebar)),
            border: Border {
                color: p.border,
                width: 1.0,
                radius: RADIUS_LG.into(),
            },
            shadow: pop_shadow(),
            ..Default::default()
        })
        .into()
}

fn pop_shadow() -> Shadow {
    Shadow {
        color: Color {
            a: 0.13,
            ..Color::BLACK
        },
        offset: Vector::new(0.0, 8.0),
        blur_radius: 28.0,
    }
}

fn modal_root<'a>(content: impl Into<Element<'a, Message>>, p: Palette) -> Element<'a, Message> {
    container(scrollable(
        container(content)
            .width(Length::Fill)
            .center_x(Length::Fill),
    ))
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(24)
    .center_y(Length::Fill)
    .style(move |_| {
        rounded(
            Color {
                a: 0.18,
                ..p.filled
            },
            0.0,
        )
    })
    .into()
}

fn outline_style(
    p: Palette,
    enabled: bool,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    iced::widget::button::Style {
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
    }
}

fn rounded(background: Color, radius: f32) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(background)),
        border: Border {
            radius: radius.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn bordered(background: Color, border: Color, radius: f32) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(background)),
        border: Border {
            color: border,
            width: 1.0,
            radius: radius.into(),
        },
        ..Default::default()
    }
}

fn green_tint(p: Palette) -> Color {
    Color { a: 0.12, ..p.green }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace(id: &str, member: bool) -> Workspace {
        Workspace {
            id: id.into(),
            name: if id == "team" { "Team" } else { "Other" }.into(),
            chain_id: format!("{id}#abcd"),
            pubkey: "ab12".into(),
            member,
        }
    }

    #[test]
    fn create_join_and_remote_submit_only_when_their_fields_are_ready() {
        let mut state = State::default();
        assert_eq!(update(&mut state, Message::Submit), None);
        update(&mut state, Message::NameChanged("  Team  ".into()));
        assert_eq!(
            update(&mut state, Message::Submit),
            Some(Command::CreateWorkspace {
                name: "Team".into()
            })
        );

        state.busy = false;
        assert_eq!(
            update(&mut state, Message::SelectMode(ConnectMode::Join)),
            Some(Command::LoadJoinCode)
        );
        update(
            &mut state,
            Message::InviteChanged("🦆abc\n\u{200b}def \u{2060}".into()),
        );
        assert_eq!(state.invite, "🦆abcdef");
        assert_eq!(
            update(&mut state, Message::Submit),
            Some(Command::JoinWorkspace {
                name: "Team".into(),
                invite: "🦆abcdef".into(),
            })
        );

        state.busy = false;
        update(&mut state, Message::SelectMode(ConnectMode::Remote));
        update(
            &mut state,
            Message::RemoteUrlChanged(" http://192.168.1.50:8844 ".into()),
        );
        assert_eq!(
            update(&mut state, Message::Submit),
            Some(Command::ConnectRemote {
                url: "http://192.168.1.50:8844".into()
            })
        );
        update(
            &mut state,
            Message::RemoteUrlChanged("http://changed.example:8844".into()),
        );
        assert_eq!(
            update(
                &mut state,
                Message::Service(ServiceEvent::RemoteConnected(Ok(())))
            ),
            Some(Command::Connected(ConnectionTarget::Remote(
                "http://192.168.1.50:8844".into()
            )))
        );
    }

    #[test]
    fn joined_workspace_walks_phase_and_readiness_effects_without_transport_logic() {
        let mut state = State::default();
        assert_eq!(
            update(
                &mut state,
                Message::Service(ServiceEvent::WorkspaceJoined(Ok(workspace("team", false))))
            ),
            Some(Command::ActivateWorkspace {
                workspace_id: "team".into()
            })
        );
        assert_eq!(state.stage, Stage::Joining);
        assert_eq!(state.phase.as_ref().unwrap().phase, Phase::Starting);
        assert_eq!(
            update(
                &mut state,
                Message::Service(ServiceEvent::WorkspaceActivated {
                    workspace_id: "team".into(),
                    result: Ok(()),
                })
            ),
            Some(Command::PollPhase {
                workspace_id: "team".into(),
                delay_ms: 0,
            })
        );
        assert_eq!(
            update(
                &mut state,
                Message::Service(ServiceEvent::PhaseLoaded {
                    workspace_id: "team".into(),
                    result: Ok(PhaseReport {
                        phase: Phase::Admitted,
                        detail: Some("standing granted".into()),
                    }),
                })
            ),
            Some(Command::CheckJoinReady {
                workspace_id: "team".into()
            })
        );
        assert_eq!(
            update(
                &mut state,
                Message::Service(ServiceEvent::JoinReady {
                    workspace_id: "team".into(),
                    ready: false,
                })
            ),
            Some(Command::PollPhase {
                workspace_id: "team".into(),
                delay_ms: JOIN_POLL_MS,
            })
        );
        assert_eq!(
            update(
                &mut state,
                Message::Service(ServiceEvent::JoinReady {
                    workspace_id: "team".into(),
                    ready: true,
                })
            ),
            Some(Command::Connected(ConnectionTarget::Workspace(
                "team".into()
            )))
        );
    }

    #[test]
    fn fatal_phase_stops_polling_and_workspaces_cancels_the_join() {
        let mut state = State {
            stage: Stage::Joining,
            workspace: Some(workspace("team", false)),
            phase: Some(PhaseReport::starting()),
            ..State::default()
        };
        assert_eq!(
            update(
                &mut state,
                Message::Service(ServiceEvent::PhaseLoaded {
                    workspace_id: "team".into(),
                    result: Ok(PhaseReport {
                        phase: Phase::Fatal,
                        detail: Some("address in use".into()),
                    }),
                })
            ),
            None
        );
        assert_eq!(state.phase.as_ref().unwrap().phase, Phase::Fatal);
        assert_eq!(
            update(&mut state, Message::Cancel),
            Some(Command::CancelJoin {
                workspace_id: "team".into()
            })
        );
        assert_eq!(state.stage, Stage::Connect);
    }

    #[test]
    fn member_boot_failure_loads_log_and_retries_the_same_workspace() {
        let mut state = State {
            workspace: Some(workspace("team", true)),
            ..State::default()
        };
        assert_eq!(
            update(
                &mut state,
                Message::Service(ServiceEvent::WorkspaceActivated {
                    workspace_id: "team".into(),
                    result: Err(BootFailure {
                        kind: BootErrorKind::StartupFailure,
                        reason: "address already in use".into(),
                    }),
                })
            ),
            Some(Command::LoadLog {
                workspace_id: "team".into()
            })
        );
        assert_eq!(state.stage, Stage::Failed);
        update(
            &mut state,
            Message::Service(ServiceEvent::LogLoaded {
                workspace_id: "team".into(),
                result: Ok(LogTail {
                    path: "/home/x/daemon.log".into(),
                    tail: "FATAL bind".into(),
                }),
            }),
        );
        assert_eq!(state.boot_error.as_ref().unwrap().log_tail, "FATAL bind");
        assert_eq!(
            update(&mut state, Message::Retry),
            Some(Command::RetryWorkspace {
                workspace_id: "team".into()
            })
        );
    }

    #[test]
    fn incompatible_workspace_never_retries() {
        let mut state = State {
            stage: Stage::Failed,
            workspace: Some(workspace("team", true)),
            boot_error: Some(BootError {
                kind: BootErrorKind::IncompatibleWorkspace,
                workspace_id: "team".into(),
                reason: "schema".into(),
                log_path: None,
                log_tail: String::new(),
            }),
            ..State::default()
        };
        assert_eq!(update(&mut state, Message::Retry), None);
        assert_eq!(update(&mut state, Message::Cancel), None);
        assert_eq!(state.stage, Stage::Connect);
    }

    #[test]
    fn forget_failure_reveals_force_only_for_that_workspace() {
        let mut state = State {
            workspaces: vec![workspace("team", true), workspace("other", true)],
            workspace: Some(workspace("team", true)),
            ..State::default()
        };
        update(
            &mut state,
            Message::RequestForget {
                workspace_id: "team".into(),
                force: false,
            },
        );
        assert_eq!(
            update(&mut state, Message::ConfirmForget),
            Some(Command::ForgetWorkspace {
                workspace_id: "team".into(),
                force: false,
            })
        );
        update(
            &mut state,
            Message::Service(ServiceEvent::WorkspaceForgot {
                workspace_id: "team".into(),
                force: false,
                result: Err("membership unconfirmed".into()),
            }),
        );
        assert_eq!(state.delete_needs_force.as_deref(), Some("team"));

        update(
            &mut state,
            Message::RequestForget {
                workspace_id: "team".into(),
                force: true,
            },
        );
        assert_eq!(
            update(&mut state, Message::ConfirmForget),
            Some(Command::ForgetWorkspace {
                workspace_id: "team".into(),
                force: true,
            })
        );
        assert_eq!(
            update(
                &mut state,
                Message::Service(ServiceEvent::WorkspaceForgot {
                    workspace_id: "team".into(),
                    force: true,
                    result: Ok(Some(workspace("other", true))),
                })
            ),
            Some(Command::ActivateWorkspace {
                workspace_id: "other".into()
            })
        );
        assert!(
            state
                .workspaces
                .iter()
                .all(|workspace| workspace.id != "team")
        );
    }

    #[test]
    fn forgetting_an_inactive_workspace_does_not_switch_the_active_workspace() {
        let mut state = State {
            workspaces: vec![workspace("team", true), workspace("other", true)],
            workspace: Some(workspace("team", true)),
            ..State::default()
        };
        assert_eq!(
            update(
                &mut state,
                Message::Service(ServiceEvent::WorkspaceForgot {
                    workspace_id: "other".into(),
                    force: false,
                    result: Ok(Some(workspace("team", true))),
                })
            ),
            None
        );
        assert_eq!(state.workspace.as_ref().unwrap().id, "team");
    }

    #[test]
    fn node_key_copy_feedback_clears_after_the_original_delay() {
        let mut state = State {
            workspace: Some(workspace("team", false)),
            ..State::default()
        };
        assert_eq!(
            update(&mut state, Message::CopyNodeKey),
            Some(Command::CopyText("ab12".into()))
        );
        assert!(state.copied_node_key);
        assert_eq!(
            update(
                &mut state,
                Message::Service(ServiceEvent::TextCopied(Ok(())))
            ),
            Some(Command::ClearCopiedAfter {
                millis: COPY_FEEDBACK_MS
            })
        );
        update(&mut state, Message::ClearCopied);
        assert!(!state.copied_node_key);
    }

    #[test]
    fn stale_phase_and_log_results_cannot_replace_the_current_workspace() {
        let mut state = State {
            stage: Stage::Joining,
            workspace: Some(workspace("team", false)),
            phase: Some(PhaseReport::starting()),
            ..State::default()
        };
        assert_eq!(
            update(
                &mut state,
                Message::Service(ServiceEvent::PhaseLoaded {
                    workspace_id: "old".into(),
                    result: Ok(PhaseReport {
                        phase: Phase::Fatal,
                        detail: Some("stale".into()),
                    }),
                })
            ),
            None
        );
        assert_eq!(state.phase.as_ref().unwrap().phase, Phase::Starting);
    }
}
