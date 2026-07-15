//! Native node-operator surfaces: Node, Gateway, Modules, Sandbox, and Metrics.
//!
//! This module owns presentation state only. [`update`] emits typed [`Command`]s;
//! the host performs node I/O and returns a [`ServiceEvent`].

use iced::widget::{Button, Space, button, column, container, row, scrollable, text, text_input};
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
pub struct ModuleRoot {
    pub id: String,
    pub root: String,
    pub category: ModuleCategory,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteTarget {
    DuckFs,
    LoopbackHttp,
}

impl RouteTarget {
    const fn label(self) -> &'static str {
        match self {
            Self::DuckFs => "DuckFS content",
            Self::LoopbackHttp => "Local HTTP service",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteAudience {
    Network,
    Owner,
    Accounts,
}

impl RouteAudience {
    const fn label(self) -> &'static str {
        match self {
            Self::Network => "All identified network members",
            Self::Owner => "Owning account only",
            Self::Accounts => "Specific accounts",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RouteMethod {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
}

impl RouteMethod {
    const ALL: [Self; 6] = [
        Self::Get,
        Self::Head,
        Self::Post,
        Self::Put,
        Self::Patch,
        Self::Delete,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Head => "HEAD",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteHealth {
    Idle,
    Checking,
    Serving(u16),
    Reachable(u16),
    Failing(u16),
    Disabled,
    Unavailable,
}

impl RouteHealth {
    fn label(self) -> String {
        match self {
            Self::Idle => "Not checked".into(),
            Self::Checking => "Checking…".into(),
            Self::Serving(status) => format!("Serving · HTTP {status}"),
            Self::Reachable(status) => format!("Reachable · HTTP {status}"),
            Self::Failing(status) => format!("Failing · HTTP {status}"),
            Self::Disabled => "Health check needs HEAD".into(),
            Self::Unavailable => "Gateway unavailable".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayRoute {
    pub key: String,
    pub label: String,
    pub address: String,
    pub target: RouteTarget,
    pub revision: u64,
    pub this_node: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayDraft {
    pub label: String,
    pub address: String,
    pub target: RouteTarget,
    pub audience: RouteAudience,
    pub audience_accounts: Vec<String>,
    pub default_path: String,
    pub port: String,
    pub methods: Vec<RouteMethod>,
    pub request_kib: String,
    pub response_kib: String,
    pub allow_authorization: bool,
    pub allow_upgrade: bool,
    pub revision: Option<u64>,
}

impl Default for GatewayDraft {
    fn default() -> Self {
        Self {
            label: String::new(),
            address: "Account ID route".into(),
            target: RouteTarget::DuckFs,
            audience: RouteAudience::Network,
            audience_accounts: Vec::new(),
            default_path: "index.html".into(),
            port: "3000".into(),
            methods: vec![RouteMethod::Get, RouteMethod::Head],
            request_kib: "256".into(),
            response_kib: "4096".into(),
            allow_authorization: false,
            allow_upgrade: false,
            revision: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayData {
    pub routes: Vec<GatewayRoute>,
    pub handle: Option<String>,
    pub account_bound: bool,
    pub desktop_signer: bool,
    pub managed_workspace: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GatewayState {
    pub data: Resource<GatewayData>,
    pub draft: GatewayDraft,
    pub selected: Option<String>,
    pub health: RouteHealth,
    pub busy: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleCategory {
    Workspace,
    Developer,
    Automation,
    System,
}

impl ModuleCategory {
    const ALL: [Self; 4] = [
        Self::Workspace,
        Self::Developer,
        Self::Automation,
        Self::System,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::Workspace => "WORKSPACE",
            Self::Developer => "DEVELOPER",
            Self::Automation => "AUTOMATION",
            Self::System => "SYSTEM",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModulesState {
    pub data: Resource<Vec<ModuleRoot>>,
    pub copied: Option<String>,
}

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
    pub setup_check: Option<String>,
    pub error: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeMessage {
    SelectTab(NodeTab),
    Start,
    Stop,
    Copy { key: String, value: String },
    LogFilterChanged(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayMessage {
    SelectRoute(String),
    NewRoute,
    LabelChanged(String),
    SetTarget(RouteTarget),
    SetAudience(RouteAudience),
    DefaultPathChanged(String),
    PortChanged(String),
    ToggleMethod(RouteMethod),
    RequestKibChanged(String),
    ResponseKibChanged(String),
    ToggleAuthorization,
    ToggleUpgrade,
    CheckHealth,
    CreateStarter,
    Save,
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxMessage {
    Recheck,
    Choose(SandboxMode),
    CancelApply,
    ConfirmApply,
    SetUpWithAgent { check: String, agent: String },
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
        Message::Node(message) => update_node(&mut state.node, message),
        Message::Gateway(message) => update_gateway(&mut state.gateway, message),
        Message::CopyModule { id, root } => {
            state.modules.copied = Some(id);
            Some(Command::CopyText(root))
        }
        Message::Sandbox(message) => update_sandbox(&mut state.sandbox, message),
        Message::ToggleMetricsPause => {
            state.metrics.paused = !state.metrics.paused;
            Some(Command::PauseMetrics(state.metrics.paused))
        }
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

fn update_node(state: &mut NodeState, message: NodeMessage) -> Option<Command> {
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

fn update_gateway(state: &mut GatewayState, message: GatewayMessage) -> Option<Command> {
    match message {
        GatewayMessage::SelectRoute(key) => {
            state.selected = Some(key.clone());
            state.health = RouteHealth::Idle;
            state.note = None;
            return Some(Command::LoadGatewayRoute(key));
        }
        GatewayMessage::NewRoute => {
            state.selected = None;
            state.draft = GatewayDraft::default();
            state.health = RouteHealth::Idle;
            state.note = None;
        }
        GatewayMessage::LabelChanged(value) => {
            state.draft.label = value.to_ascii_lowercase();
        }
        GatewayMessage::SetTarget(target) => {
            state.draft.target = target;
            if target == RouteTarget::DuckFs {
                state.draft.methods = vec![RouteMethod::Get, RouteMethod::Head];
                state.draft.allow_authorization = false;
                state.draft.allow_upgrade = false;
            }
        }
        GatewayMessage::SetAudience(audience) => state.draft.audience = audience,
        GatewayMessage::DefaultPathChanged(value) => state.draft.default_path = value,
        GatewayMessage::PortChanged(value) => state.draft.port = value,
        GatewayMessage::ToggleMethod(method) => {
            if let Some(index) = state.draft.methods.iter().position(|item| *item == method) {
                state.draft.methods.remove(index);
            } else {
                state.draft.methods.push(method);
                state.draft.methods.sort();
            }
        }
        GatewayMessage::RequestKibChanged(value) => state.draft.request_kib = value,
        GatewayMessage::ResponseKibChanged(value) => state.draft.response_kib = value,
        GatewayMessage::ToggleAuthorization => {
            state.draft.allow_authorization = !state.draft.allow_authorization
        }
        GatewayMessage::ToggleUpgrade => state.draft.allow_upgrade = !state.draft.allow_upgrade,
        GatewayMessage::CheckHealth => {
            let key = state.selected.clone()?;
            state.health = RouteHealth::Checking;
            return Some(Command::CheckGatewayHealth(key));
        }
        GatewayMessage::CreateStarter => {
            if let Err(error) = validate_gateway_draft(&state.draft) {
                state.note = Some(error);
                return None;
            }
            return Some(Command::CreateGatewayStarter(state.draft.clone()));
        }
        GatewayMessage::Save => {
            if let Err(error) = validate_gateway_draft(&state.draft) {
                state.note = Some(error);
                return None;
            }
            state.busy = true;
            state.note = None;
            return Some(Command::SaveGatewayRoute(state.draft.clone()));
        }
        GatewayMessage::Remove => {
            let key = state.selected.clone()?;
            state.busy = true;
            state.note = None;
            return Some(Command::RemoveGatewayRoute(key));
        }
    }
    None
}

fn validate_gateway_draft(draft: &GatewayDraft) -> Result<(), String> {
    if !draft.label.is_empty()
        && (draft.label.len() > 63
            || draft.label.starts_with('-')
            || draft.label.ends_with('-')
            || !draft
                .label
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'))
    {
        return Err("Use lowercase letters, numbers, and hyphens for the route label.".into());
    }
    let response = draft
        .response_kib
        .parse::<u64>()
        .map_err(|_| "Response cap must be a whole number of KiB.".to_string())?;
    if response > 4096 {
        return Err("Response cap must be 0..4096 KiB.".into());
    }
    if draft.target == RouteTarget::LoopbackHttp {
        let port = draft
            .port
            .parse::<u16>()
            .map_err(|_| "Loopback port must be 1..65535.".to_string())?;
        if port == 0 {
            return Err("Loopback port must be 1..65535.".into());
        }
        let request = draft
            .request_kib
            .parse::<u64>()
            .map_err(|_| "Request cap must be a whole number of KiB.".to_string())?;
        if request > 1024 {
            return Err("Request cap must be 0..1024 KiB.".into());
        }
        if draft.methods.is_empty() {
            return Err("Choose at least one allowed method.".into());
        }
    }
    if draft.audience == RouteAudience::Accounts && draft.audience_accounts.is_empty() {
        return Err("Choose at least one account for this audience.".into());
    }
    Ok(())
}

fn update_sandbox(state: &mut SandboxState, message: SandboxMessage) -> Option<Command> {
    match message {
        SandboxMessage::Recheck => {
            state.data = Resource::Loading;
            return Some(Command::CheckSandbox);
        }
        SandboxMessage::Choose(mode) => {
            state.chosen = Some(mode);
            state.error = None;
        }
        SandboxMessage::CancelApply => state.chosen = None,
        SandboxMessage::ConfirmApply => {
            let mode = state.chosen.take()?;
            state.applying = true;
            state.error = None;
            return Some(Command::ApplySandbox(mode));
        }
        SandboxMessage::SetUpWithAgent { check, agent } => {
            state.setup_check = Some(check.clone());
            return Some(Command::StartSandboxSetup { check, agent });
        }
    }
    None
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
        Screen::Node => node_view(&state.node, p),
        Screen::Gateway => gateway_view(&state.gateway, p),
        Screen::Modules => modules_view(&state.modules, p),
        Screen::Sandbox => sandbox_view(&state.sandbox, p),
        Screen::Metrics => metrics_view(&state.metrics, p),
    }
}

fn node_view(state: &NodeState, p: Palette) -> Element<'_, Message> {
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

fn gateway_view(state: &GatewayState, p: Palette) -> Element<'_, Message> {
    let Resource::Ready(data) = &state.data else {
        return resource_screen(
            &state.data,
            "Gateway",
            "No gateway routes are available.",
            Screen::Gateway,
            Icon::Browser,
            p,
        );
    };

    let mut route_rows = column![section_label("PUBLISHED ROUTES", p)].spacing(6);
    if data.routes.is_empty() {
        route_rows = route_rows.push(notice("No routes published.", p));
    }
    for route in &data.routes {
        let selected = state.selected.as_deref() == Some(route.key.as_str());
        route_rows = route_rows.push(
            button(row![
                column![
                    text(&route.address).font(MONO).size(10.5).color(p.ink),
                    text(if selected {
                        state.health.label()
                    } else {
                        "Published".into()
                    })
                    .font(SANS)
                    .size(9.5)
                    .color(if selected {
                        health_color(state.health, p)
                    } else {
                        p.muted
                    }),
                ]
                .spacing(3),
                Space::new().width(Length::Fill),
                column![
                    text(format!(
                        "{} · {}",
                        route.target.label(),
                        if route.this_node {
                            "this node"
                        } else {
                            "remote"
                        }
                    ))
                    .font(SANS)
                    .size(9.5)
                    .color(p.muted_3),
                    text(format!("r{}", route.revision))
                        .font(MONO)
                        .size(9)
                        .color(p.muted),
                ]
                .spacing(3)
                .align_x(Alignment::End),
            ])
            .width(Length::Fill)
            .padding([8, 9])
            .style(move |_, _| iced::widget::button::Style {
                background: selected.then_some(Background::Color(p.paper)),
                text_color: p.ink,
                border: Border {
                    color: if selected { p.border_strong } else { p.border },
                    width: 1.0,
                    radius: RADIUS_SM.into(),
                },
                ..Default::default()
            })
            .on_press(Message::Gateway(GatewayMessage::SelectRoute(
                route.key.clone(),
            ))),
        );
    }

    let can_mutate =
        data.desktop_signer && data.account_bound && data.managed_workspace && !state.busy;
    let editor = gateway_editor(state, can_mutate, p);
    let header = screen_header("Gateway", Some(data.routes.len()), p);
    let intro = text("Connect one account address to exact DuckFS content or a local HTTP service. The address, reverse proxy, and signed access policy are saved together.")
        .font(SANS).size(11).color(p.muted_3);
    let mut body = column![intro, route_rows, divider(p), editor].spacing(14);
    if data.handle.is_none() && data.account_bound {
        body = body.push(notice("Routes can exist by Account ID. Register a Duck name in Account to make them browsable as .duck addresses.", p));
    }
    if !data.desktop_signer {
        body = body.push(notice(
            "Saving routes requires the desktop user-key signer.",
            p,
        ));
    } else if !data.account_bound {
        body = body.push(notice(
            "Bind this node to your Identity account before saving routes.",
            p,
        ));
    }
    if let Some(note) = &state.note {
        body = body.push(notice(note, p));
    }
    container(column![
        header,
        scrollable(container(body).padding(Padding {
            top: 22.0,
            right: 20.0,
            bottom: 40.0,
            left: 20.0,
        }))
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| surface(p.sidebar))
    .into()
}

fn gateway_editor(state: &GatewayState, can_mutate: bool, p: Palette) -> Element<'_, Message> {
    let draft = &state.draft;
    let title = if draft.revision.is_some() {
        "Edit route"
    } else {
        "New route"
    };
    let label_error = validate_route_label(&draft.label).err();
    let mut methods = row![].spacing(5);
    for method in RouteMethod::ALL {
        methods = methods.push(toggle_button(
            method.label(),
            draft.methods.contains(&method),
            Message::Gateway(GatewayMessage::ToggleMethod(method)),
            draft.target == RouteTarget::LoopbackHttp,
            p,
        ));
    }
    let targets = row![
        toggle_button(
            "DuckFS content",
            draft.target == RouteTarget::DuckFs,
            Message::Gateway(GatewayMessage::SetTarget(RouteTarget::DuckFs)),
            true,
            p
        ),
        toggle_button(
            "Local HTTP service",
            draft.target == RouteTarget::LoopbackHttp,
            Message::Gateway(GatewayMessage::SetTarget(RouteTarget::LoopbackHttp)),
            true,
            p
        ),
    ]
    .spacing(6);
    let audiences = row![
        toggle_button(
            "Network",
            draft.audience == RouteAudience::Network,
            Message::Gateway(GatewayMessage::SetAudience(RouteAudience::Network)),
            true,
            p
        ),
        toggle_button(
            "Owner",
            draft.audience == RouteAudience::Owner,
            Message::Gateway(GatewayMessage::SetAudience(RouteAudience::Owner)),
            true,
            p
        ),
        toggle_button(
            "Accounts",
            draft.audience == RouteAudience::Accounts,
            Message::Gateway(GatewayMessage::SetAudience(RouteAudience::Accounts)),
            true,
            p
        ),
    ]
    .spacing(6);

    let mut fields = column![
        row![
            text(title).font(SANS).size(13).color(p.ink),
            Space::new().width(Length::Fill),
            outline_button(
                "New route",
                Message::Gateway(GatewayMessage::NewRoute),
                true,
                p
            ),
        ]
        .align_y(Alignment::Center),
        text(format!(
            "revision {}",
            draft.revision.map_or_else(|| "—".into(), |v| v.to_string())
        ))
        .font(MONO)
        .size(9)
        .color(p.muted),
        text(&draft.address).font(MONO).size(10.5).color(p.ink_soft),
        labeled_input(
            "Route label · blank = account apex",
            "api",
            &draft.label,
            |value| Message::Gateway(GatewayMessage::LabelChanged(value)),
            p
        ),
    ]
    .spacing(9);
    if let Some(error) = label_error {
        fields = fields.push(text(error).font(SANS).size(9.5).color(p.danger));
    }
    fields = fields
        .push(text("SOURCE").font(MONO).size(9).color(p.muted_2))
        .push(targets)
        .push(text("AUDIENCE").font(MONO).size(9).color(p.muted_2))
        .push(audiences)
        .push(
            text(draft.audience.label())
                .font(SANS)
                .size(10.5)
                .color(p.muted_3),
        );

    if draft.target == RouteTarget::DuckFs {
        fields = fields
            .push(labeled_input(
                "Default path",
                "index.html",
                &draft.default_path,
                |value| Message::Gateway(GatewayMessage::DefaultPathChanged(value)),
                p,
            ))
            .push(outline_button(
                "Create starter file",
                Message::Gateway(GatewayMessage::CreateStarter),
                can_mutate && label_error.is_none(),
                p,
            ));
    } else {
        fields = fields
            .push(labeled_input(
                "Loopback port",
                "3000",
                &draft.port,
                |value| Message::Gateway(GatewayMessage::PortChanged(value)),
                p,
            ))
            .push(text("ALLOWED METHODS").font(MONO).size(9).color(p.muted_2))
            .push(methods)
            .push(toggle_button(
                "Allow explicit Authorization forwarding",
                draft.allow_authorization,
                Message::Gateway(GatewayMessage::ToggleAuthorization),
                true,
                p,
            ))
            .push(toggle_button(
                "Allow WebSocket upgrade",
                draft.allow_upgrade,
                Message::Gateway(GatewayMessage::ToggleUpgrade),
                true,
                p,
            ))
            .push(labeled_input(
                "Request KiB",
                "256",
                &draft.request_kib,
                |value| Message::Gateway(GatewayMessage::RequestKibChanged(value)),
                p,
            ));
    }
    fields = fields.push(labeled_input(
        "Response KiB",
        "4096",
        &draft.response_kib,
        |value| Message::Gateway(GatewayMessage::ResponseKibChanged(value)),
        p,
    ));
    if state.selected.is_some() {
        fields = fields.push(
            row![
                text(state.health.label())
                    .font(SANS)
                    .size(10)
                    .color(health_color(state.health, p)),
                Space::new().width(Length::Fill),
                outline_button(
                    "Check",
                    Message::Gateway(GatewayMessage::CheckHealth),
                    state.health != RouteHealth::Checking,
                    p
                ),
            ]
            .align_y(Alignment::Center),
        );
    }
    fields = fields.push(filled_button(
        "Save route",
        Message::Gateway(GatewayMessage::Save),
        can_mutate && label_error.is_none(),
        p,
    ));
    if state.selected.is_some() {
        fields = fields.push(danger_button(
            "Remove route",
            Message::Gateway(GatewayMessage::Remove),
            can_mutate,
            p,
        ));
    }
    fields.into()
}

fn modules_view(state: &ModulesState, p: Palette) -> Element<'_, Message> {
    let Resource::Ready(modules) = &state.data else {
        return resource_screen(
            &state.data,
            "Modules",
            "Waiting for module roots from the node.",
            Screen::Modules,
            Icon::Modules,
            p,
        );
    };
    let header = screen_header("Modules", Some(modules.len()), p);
    let intro = row![
        icon_tile(Icon::Modules, 36.0, p),
        column![
            text("Node module set").font(SANS).size(19).color(p.ink),
            text("These are the genesis modules this node runs, with each module's committed Merkle root.")
                .font(SANS).size(13).color(p.muted),
        ].spacing(3),
    ].spacing(11).align_y(Alignment::Start);
    let mut body = column![intro].spacing(18);
    for category in ModuleCategory::ALL {
        let rows: Vec<_> = modules
            .iter()
            .filter(|module| module.category == category)
            .collect();
        if rows.is_empty() {
            continue;
        }
        let mut group = column![row![
            text(category.label())
                .font(MONO)
                .size(10)
                .color(category_color(category, p)),
            Space::new().width(Length::Fill),
            text(rows.len().to_string())
                .font(MONO)
                .size(11)
                .color(p.muted_2),
        ]]
        .spacing(10);
        for module in rows {
            group = group.push(module_card(
                module,
                state.copied.as_deref() == Some(module.id.as_str()),
                p,
            ));
        }
        body = body.push(group);
    }
    container(column![
        header,
        scrollable(container(body).padding([22, 26]))
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| surface(p.canvas))
    .into()
}

fn module_card(module: &ModuleRoot, copied: bool, p: Palette) -> Element<'static, Message> {
    let (label, detail) = module_info(&module.id);
    card(
        row![
            container(
                text(
                    module
                        .id
                        .chars()
                        .take(2)
                        .collect::<String>()
                        .to_ascii_uppercase()
                )
                .font(MONO)
                .size(13)
                .color(p.on_filled)
            )
            .width(40)
            .height(40)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(move |_| rounded_surface(p.filled, p.filled, 10.0)),
            column![
                row![
                    text(label).font(SANS).size(13.5).color(p.ink),
                    text(module.id.clone()).font(MONO).size(11).color(p.muted_2),
                ]
                .spacing(7),
                text(detail).font(SANS).size(12).color(p.muted),
                button(
                    text(if copied {
                        "copied".into()
                    } else {
                        short(&module.root, 10, 8)
                    })
                    .font(MONO)
                    .size(11)
                )
                .padding([4, 8])
                .style(move |_, _| iced::widget::button::Style {
                    background: Some(Background::Color(if copied {
                        p.danger_soft
                    } else {
                        p.sunken
                    })),
                    text_color: if copied { p.green } else { p.muted_3 },
                    border: Border {
                        color: p.border_soft,
                        width: 1.0,
                        radius: RADIUS_SM.into()
                    },
                    ..Default::default()
                })
                .on_press(Message::CopyModule {
                    id: module.id.clone(),
                    root: module.root.clone()
                }),
            ]
            .spacing(5),
        ]
        .spacing(13)
        .align_y(Alignment::Start),
        p,
    )
}

fn sandbox_view(state: &SandboxState, p: Palette) -> Element<'_, Message> {
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
    let header = container(column![
        text("Sandbox").font(SANS).size(20).color(p.ink),
        text("Choose how this node executes agent work, verify the host, and apply changes with a guarded restart.")
            .font(SANS).size(11.5).color(p.muted_2),
    ].spacing(5)).width(Length::Fill).padding(Padding {
        top: 20.0,
        right: 22.0,
        bottom: 16.0,
        left: 22.0,
    }).style(move |_| bottom_border(p.canvas, p.border_soft));

    let mut body = column![
        row![
            text("Sandbox serving").font(SANS).size(15).color(p.ink),
            pill(if data.serving { "Serving" } else { "Not serving" }, if data.serving { p.green } else { p.amber }, p),
            text(format!("mode {}", data.current_mode.label())).font(MONO).size(11).color(p.muted_2),
            Space::new().width(Length::Fill),
            outline_button("Re-check", Message::Sandbox(SandboxMessage::Recheck), data.can_control, p),
        ].align_y(Alignment::Center).spacing(10),
        text("Nodes serve agent work only when opted in. Turning it on announces this node's executors and metered capacity into the capability registry.")
            .font(SANS).size(11).color(p.muted_3),
    ].spacing(8);
    if !data.can_control {
        body = body.push(warning("This app isn't managing a local node, so these checks can't reach the node host. Run the preflight on the machine that runs the node.", p));
    }
    let mut checks = column![
        text(format!("{} · {}", data.backend, data.os))
            .font(MONO)
            .size(10.5)
            .color(p.muted_2)
    ]
    .spacing(0);
    for check in &data.checks {
        checks = checks.push(check_row(check, data, state, p));
    }
    body = body
        .push(section_label("DETECTION", p))
        .push(card(checks, p))
        .push(section_label("OPT-IN SWITCH", p));
    let mut choices = row![].spacing(7);
    for mode in &data.available_modes {
        choices = choices.push(toggle_button(
            mode.label(),
            state.chosen == Some(*mode),
            Message::Sandbox(SandboxMessage::Choose(*mode)),
            data.can_control && !state.applying && data.current_mode != *mode,
            p,
        ));
    }
    let status = if let Some(error) = &state.error {
        format!("Apply failed: {error}")
    } else if state.applying {
        "Applying config and restarting the node…".into()
    } else {
        "Choose a mode to review and apply it.".into()
    };
    body = body.push(card(
        column![
            choices,
            text(status)
                .font(SANS)
                .size(11)
                .color(if state.error.is_some() {
                    p.red
                } else {
                    p.muted_3
                })
        ]
        .spacing(10),
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
        container(text(glyph).font(SANS).size(11).color(color))
            .width(19)
            .height(19)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(move |_| rounded_surface(p.sunken, p.border_soft, 99.0)),
        column![
            text(check.label.clone())
                .font(SANS)
                .size(12)
                .color(p.ink_soft),
            text(check.detail.clone())
                .font(MONO)
                .size(10.5)
                .color(p.muted_2),
        ]
        .spacing(2),
        Space::new().width(Length::Fill),
    ]
    .spacing(11)
    .align_y(Alignment::Center);
    if check.fixable {
        let enabled = !data.active_agents.is_empty() && data.active_channel.is_some();
        let agent = data
            .active_agents
            .first()
            .map(|(id, _)| id.clone())
            .unwrap_or_default();
        line = line.push(outline_button(
            if state.setup_check.as_deref() == Some(check.id.as_str()) {
                "setup run requested →"
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
        .style(move |_| top_border(Color::TRANSPARENT, p.border_soft))
        .into()
}

fn metrics_view(state: &MetricsState, p: Palette) -> Element<'_, Message> {
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
    let header = container(
        row![
            column![
                text("Metrics").font(SANS).size(20).color(p.ink),
                text(format!(
                    "Live node telemetry · sampled {}",
                    snapshot.sampled_at
                ))
                .font(SANS)
                .size(11.5)
                .color(p.muted_2),
            ]
            .spacing(5),
            Space::new().width(Length::Fill),
            outline_button(
                if state.paused { "Resume" } else { "Pause" },
                Message::ToggleMetricsPause,
                true,
                p
            ),
        ]
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding(Padding {
        top: 20.0,
        right: 22.0,
        bottom: 16.0,
        left: 22.0,
    })
    .style(move |_| bottom_border(p.canvas, p.border_soft));
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
        summary,
        card(
            row![
                column![
                    text("ACCEPTED").font(MONO).size(9).color(p.muted_2),
                    text(snapshot.accepted.to_string())
                        .font(MONO)
                        .size(18)
                        .color(p.green)
                ]
                .spacing(4),
                Space::new().width(Length::Fill),
                column![
                    text("REJECTED").font(MONO).size(9).color(p.muted_2),
                    text(snapshot.rejected.to_string())
                        .font(MONO)
                        .size(18)
                        .color(p.red)
                ]
                .spacing(4),
                Space::new().width(Length::Fill),
                column![
                    text("APPLY P50").font(MONO).size(9).color(p.muted_2),
                    text(format!("{:.1} ms", snapshot.apply_p50_ms))
                        .font(MONO)
                        .size(18)
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
                .size(12)
                .color(if plane.halted { p.red } else { p.ink }),
                text(format!("by {} · open {}", plane.owner, plane.age))
                    .font(MONO)
                    .size(10.5)
                    .color(p.muted_2),
            ]
            .spacing(2)
            .width(180),
            Space::new().width(Length::Fill),
            column![
                text(format!("↑ {}", format_rate(plane.tx_bytes_per_second)))
                    .font(MONO)
                    .size(11)
                    .color(p.ink),
                text(format!("↓ {}", format_rate(plane.rx_bytes_per_second)))
                    .font(MONO)
                    .size(11)
                    .color(p.ink),
            ]
            .spacing(2)
            .width(110),
            column![
                text(format!("{} total", format_bytes(plane.total_bytes)))
                    .font(MONO)
                    .size(10.5)
                    .color(p.muted),
                text(format!("{} dropped", plane.dropped))
                    .font(MONO)
                    .size(10.5)
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
                    .size(12)
                    .color(p.ink),
                text(format!("{} · {}", peer.phase, peer.age))
                    .font(MONO)
                    .size(10.5)
                    .color(p.muted_2),
            ]
            .spacing(2)
            .width(170),
            column![
                text(progress).font(MONO).size(11).color(p.ink),
                text(left).font(MONO).size(10.5).color(p.muted),
            ]
            .spacing(2)
            .width(150),
            Space::new().width(Length::Fill),
            column![
                text(format!("↑ {}", format_rate(peer.tx_bytes_per_second)))
                    .font(MONO)
                    .size(11)
                    .color(p.ink),
                text(format!(
                    "{} · {} frames",
                    format_bytes(peer.total_bytes),
                    peer.frames
                ))
                .font(MONO)
                .size(10.5)
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
            center_state(&format!("Loading {title}"), "Reading node state…", icon, p)
        }
        Resource::Empty => center_state(title, empty, icon, p),
        Resource::Error(error) => {
            error_state(&format!("{title} unavailable"), error, screen, icon, p)
        }
        Resource::Ready(_) => unreachable!("ready resources are rendered by their screen"),
    }
}

fn center_state<'a>(title: &str, detail: &str, icon: Icon, p: Palette) -> Element<'a, Message> {
    container(
        column![
            icon_tile(icon, 42.0, p),
            text(title.to_string()).font(SANS).size(14).color(p.muted_3),
            text(detail.to_string())
                .font(SANS)
                .size(11.5)
                .color(p.muted_2)
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
    button(
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
    }))
    .into()
}

fn segment_button<'a>(
    label: &'static str,
    active: bool,
    message: Message,
    p: Palette,
) -> Button<'a, Message> {
    button(text(label).font(SANS).size(11.5))
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
        .on_press(message)
}

fn outline_button<'a>(
    label: impl ToString,
    message: Message,
    enabled: bool,
    p: Palette,
) -> Button<'a, Message> {
    let button = button(text(label.to_string()).font(SANS).size(11.5))
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
    if enabled {
        button.on_press(message)
    } else {
        button
    }
}

fn filled_button<'a>(
    label: impl ToString,
    message: Message,
    enabled: bool,
    p: Palette,
) -> Button<'a, Message> {
    let button = button(text(label.to_string()).font(SANS).size(11.5))
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
    if enabled {
        button.on_press(message)
    } else {
        button
    }
}

fn danger_button<'a>(
    label: impl ToString,
    message: Message,
    enabled: bool,
    p: Palette,
) -> Button<'a, Message> {
    let button = button(text(label.to_string()).font(SANS).size(11.5))
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
    if enabled {
        button.on_press(message)
    } else {
        button
    }
}

fn toggle_button<'a>(
    label: impl ToString,
    active: bool,
    message: Message,
    enabled: bool,
    p: Palette,
) -> Button<'a, Message> {
    let button = button(text(label.to_string()).font(SANS).size(10.5))
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
    if enabled {
        button.on_press(message)
    } else {
        button
    }
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
        text_input(placeholder, value)
            .on_input(on_input)
            .padding([7, 8])
            .font(MONO)
            .size(11)
    ]
    .spacing(5)
    .into()
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

fn health_color(health: RouteHealth, p: Palette) -> Color {
    match health {
        RouteHealth::Serving(_) => p.green,
        RouteHealth::Failing(_) | RouteHealth::Unavailable => p.red,
        RouteHealth::Checking | RouteHealth::Reachable(_) => p.amber,
        RouteHealth::Idle | RouteHealth::Disabled => p.muted_3,
    }
}

fn category_color(category: ModuleCategory, p: Palette) -> Color {
    match category {
        ModuleCategory::Workspace => p.blue,
        ModuleCategory::Developer => p.purple,
        ModuleCategory::Automation => p.amber,
        ModuleCategory::System => p.muted_3,
    }
}

fn level_color(level: &str, p: Palette) -> Color {
    match level.to_ascii_lowercase().as_str() {
        "error" => p.red,
        "warn" => p.amber,
        "debug" | "trace" => p.muted_2,
        _ => p.green,
    }
}

fn validate_route_label(label: &str) -> Result<(), &'static str> {
    if label.is_empty() {
        return Ok(());
    }
    if label.len() > 63
        || label.starts_with('-')
        || label.ends_with('-')
        || !label
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err("Use lowercase letters, numbers, and hyphens.");
    }
    Ok(())
}

fn short(value: &str, start: usize, end: usize) -> String {
    if value.is_empty() {
        return "—".into();
    }
    if value.len() <= start + end + 1 {
        return value.into();
    }
    format!("{}…{}", &value[..start], &value[value.len() - end..])
}

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

fn module_info(id: &str) -> (&'static str, &'static str) {
    match id {
        "chat" => ("Chat", "Channels, messages, threads, and reactions."),
        "tasks" => ("Tasks", "A shared, ordered task list."),
        "forge" => ("Forge", "A git-backed repository, one commit per block."),
        "agent" => ("Agents", "The agent collaboration loop and run ledger."),
        "governance" => ("Governance", "Validator-set proposals and quorum voting."),
        "vaults" => ("Vaults", "Encrypted team secrets with an owner/reader ACL."),
        "inbox" => ("Inbox", "Per-member notification queues."),
        "automations" => ("Automations", "Event-triggered rules over module events."),
        "files" => ("Files", "A copy-on-write, content-addressed filesystem."),
        "identity" => ("Identity", "Accounts, member keys, and node bindings."),
        "duckdns" => (
            "DuckDNS",
            "Optional global .duck handles resolved to accounts.",
        ),
        "gateway" => ("Gateway", "Signed account routes to DuckFS or local HTTP."),
        _ => ("Module", "A registered genesis module."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
