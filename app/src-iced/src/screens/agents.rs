//! Native agent roster, auto-reply, and activity screens.
//!
//! State and rendering stay transport-free; the host executes [`Command`]s
//! and feeds results back as [`ServiceEvent`]s.

use std::{collections::BTreeMap, ops::Deref};

use iced::widget::{
    Button, Space, button, checkbox, column, container, row, scrollable, text, text_input,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Shadow, Vector};

use crate::icons::{self, Icon};
use crate::theme::{self, MONO, Palette, RADIUS_LG, RADIUS_MD, RADIUS_SM, SANS, SANS_SEMIBOLD};
use crate::view_api::{AppIntent, Route};

const HEADER_HEIGHT: f32 = 56.0;
const ROSTER_WIDTH: f32 = 286.0;
const LIBRARY_ROOT: &str = "/shared/skills";
const MAX_ACTIVITY_ENTRIES: usize = 120;
const MAX_RAW_ENTRY_CHARS: usize = 256_000;
const MAX_ACTIVITY_ROWS: usize = 500;
const MAX_FIELD_LINES: usize = 120;
const MAX_ROW_CHARS: usize = 4_000;
const MAX_FIELD_CHARS: usize = 128_000;
const FIELD_HEAD_LINES: usize = 20;
const FIELD_TAIL_LINES: usize = MAX_FIELD_LINES - FIELD_HEAD_LINES - 1;

#[derive(Debug, Clone, Copy)]
struct Colors {
    palette: Palette,
    accent: Color,
}

impl Deref for Colors {
    type Target = Palette;

    fn deref(&self) -> &Self::Target {
        &self.palette
    }
}
const ACTIONS: [(&str, &str); 7] = [
    ("chat.post", "Reply in the thread it was mentioned in"),
    (
        "chat.post_message",
        "Start messages in any channel, on its own initiative",
    ),
    ("tasks.create", "Create tasks"),
    ("tasks.update_status", "Update task status"),
    ("pages.comment", "Comment on pages"),
    ("pages.set_checked", "Check off page todos"),
    ("duckfs.write_text", "Write files"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resource<T> {
    Loading,
    Empty,
    Error(String),
    Ready(T),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Agents,
    AutoReply,
    Activity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunFilter {
    All,
    Mine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatus {
    Active,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityStatus {
    Loading,
    Ready,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Owner {
    External(String),
    Module(String),
    System,
}

impl Owner {
    fn label(&self) -> String {
        match self {
            Self::External(key) => format!("external:{}", short(key)),
            Self::Module(module) => format!("module:{module}"),
            Self::System => "system".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadMode {
    Always,
    OnDemand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillRef {
    pub name: String,
    pub source_prefix: String,
    pub snapshot: Option<String>,
    pub load: LoadMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResourceCaps {
    pub forge_read: Vec<String>,
    pub forge_push: Vec<String>,
    pub duckfs_read: Vec<String>,
    pub duckfs_write: Vec<String>,
    pub tools: Vec<String>,
    pub secrets: Vec<String>,
    pub pages_write: Vec<String>,
    pub subagent_budget: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRecord {
    pub id: String,
    pub owner: Owner,
    pub display_name: String,
    pub capability: String,
    pub allowed_actions: Vec<String>,
    pub status: AgentStatus,
    pub created_at: String,
    pub updated_at: String,
    pub caps: ResourceCaps,
    pub skills: Vec<SkillRef>,
    pub pending: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Channel {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnPolicy {
    Mention,
    All,
    RoundRobin,
    Assigned(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watch {
    pub channel_id: String,
    pub policy: TurnPolicy,
    pub pending: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingRun {
    pub run_id: String,
    pub dispatch_id: String,
    pub agent_id: String,
    pub channel_id: String,
    pub anchor_sequence: u64,
    pub job_id: Option<String>,
    pub created_at: String,
    pub requested_by_me: bool,
    pub attempt: u32,
    pub lease_remaining: Option<u64>,
    pub pending: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunOutcome {
    Delivered,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRecord {
    pub run_id: String,
    pub dispatch_id: String,
    pub agent_id: String,
    pub channel_id: String,
    pub anchor_sequence: u64,
    pub outcome: RunOutcome,
    pub degraded: bool,
    pub created_at: u64,
    pub delivered_at: u64,
    pub executing_node: String,
    pub output_ref: Option<String>,
    pub pr_number: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunLogEntry {
    Line { stream: RunStream, text: String },
    Gap(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RunLog {
    pub entries: Vec<RunLogEntry>,
    pub dropped: u64,
    pub last_cursor: u64,
    pub unavailable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SemanticLogKind {
    Message,
    Command,
    Output,
    Status,
    Exit,
    File,
    Tool,
    Text,
    Gap,
    Blank,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SemanticLogRow {
    kind: SemanticLogKind,
    stream: Option<RunStream>,
    text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunLogEvent {
    Connected,
    Line {
        dispatch_id: String,
        cursor: u64,
        stream: RunStream,
        text: String,
    },
    Lagged {
        dispatch_id: String,
        cursor: u64,
    },
    Unavailable {
        dispatch_id: Option<String>,
        reason: String,
    },
    Disconnected(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Usage {
    pub requests: u64,
    pub failed: u64,
    pub duration_blocks: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

impl Eq for Usage {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentData {
    pub agents: Vec<AgentRecord>,
    pub capabilities: Vec<String>,
    pub capability_status: CapabilityStatus,
    pub channels: Vec<Channel>,
    pub watches: Vec<Watch>,
    pub pending_runs: Vec<PendingRun>,
    pub recent_runs: Vec<RunRecord>,
    pub recent_runs_error: Option<String>,
    pub usage: Option<Usage>,
    pub job_worker_pending: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentDraft {
    pub display_name: String,
    pub id_override: String,
    pub capability: String,
    pub allowed_actions: Vec<String>,
    pub forge_read: String,
    pub forge_push: String,
    pub duckfs_read: String,
    pub duckfs_write: String,
    pub tools: String,
    pub secrets: String,
    pub pages_write: String,
    pub subagent_budget: String,
    pub library_read: bool,
    pub skills: Vec<SkillRef>,
    pub skill_name: String,
    pub skill_prefix: String,
    pub skill_load: LoadMode,
    pub advanced: bool,
}

impl Default for AgentDraft {
    fn default() -> Self {
        Self {
            display_name: String::new(),
            id_override: String::new(),
            capability: String::new(),
            allowed_actions: vec!["chat.post".into()],
            forge_read: String::new(),
            forge_push: String::new(),
            duckfs_read: String::new(),
            duckfs_write: String::new(),
            tools: String::new(),
            secrets: String::new(),
            pages_write: String::new(),
            subagent_budget: "0".into(),
            library_read: true,
            skills: Vec::new(),
            skill_name: String::new(),
            skill_prefix: String::new(),
            skill_load: LoadMode::OnDemand,
            advanced: false,
        }
    }
}

impl AgentDraft {
    fn id(&self) -> String {
        slug(if self.id_override.trim().is_empty() {
            &self.display_name
        } else {
            &self.id_override
        })
    }

    fn ready(&self, data: &AgentData) -> bool {
        !self.display_name.trim().is_empty()
            && !self.id().is_empty()
            && data.capability_status == CapabilityStatus::Ready
            && data.capabilities.contains(&self.capability)
            && !self.allowed_actions.is_empty()
    }

    fn caps(&self) -> ResourceCaps {
        let mut caps = ResourceCaps {
            forge_read: canonical_words(&self.forge_read),
            forge_push: canonical_words(&self.forge_push),
            duckfs_read: canonical_words(&self.duckfs_read),
            duckfs_write: canonical_words(&self.duckfs_write),
            tools: canonical_words(&self.tools),
            secrets: canonical_words(&self.secrets),
            pages_write: canonical_words(&self.pages_write),
            subagent_budget: positive_u32(&self.subagent_budget),
        };
        caps.duckfs_read.retain(|prefix| prefix != LIBRARY_ROOT);
        if self.library_read {
            caps.duckfs_read.push(LIBRARY_ROOT.into());
            caps.duckfs_read.sort();
        }
        caps
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditDraft {
    pub agent_id: String,
    pub display_name: String,
    pub capability: String,
    pub allowed_actions: Vec<String>,
    pub forge_read: String,
    pub forge_push: String,
    pub duckfs_read: String,
    pub duckfs_write: String,
    pub tools: String,
    pub secrets: String,
    pub pages_write: String,
    pub subagent_budget: String,
    pub library_read: bool,
    pub skills: Vec<SkillRef>,
}

impl From<&AgentRecord> for EditDraft {
    fn from(agent: &AgentRecord) -> Self {
        Self {
            agent_id: agent.id.clone(),
            display_name: agent.display_name.clone(),
            capability: agent.capability.clone(),
            allowed_actions: agent.allowed_actions.clone(),
            forge_read: agent.caps.forge_read.join(", "),
            forge_push: agent.caps.forge_push.join(", "),
            duckfs_read: agent
                .caps
                .duckfs_read
                .iter()
                .filter(|prefix| prefix.as_str() != LIBRARY_ROOT)
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
            duckfs_write: agent.caps.duckfs_write.join(", "),
            tools: agent.caps.tools.join(", "),
            secrets: agent.caps.secrets.join(", "),
            pages_write: agent.caps.pages_write.join(", "),
            subagent_budget: agent.caps.subagent_budget.unwrap_or(0).to_string(),
            library_read: can_read_library(&agent.caps),
            skills: agent.skills.clone(),
        }
    }
}

impl EditDraft {
    fn caps(&self) -> ResourceCaps {
        let mut caps = ResourceCaps {
            forge_read: canonical_words(&self.forge_read),
            forge_push: canonical_words(&self.forge_push),
            duckfs_read: canonical_words(&self.duckfs_read),
            duckfs_write: canonical_words(&self.duckfs_write),
            tools: canonical_words(&self.tools),
            secrets: canonical_words(&self.secrets),
            pages_write: canonical_words(&self.pages_write),
            subagent_budget: positive_u32(&self.subagent_budget),
        };
        caps.duckfs_read.retain(|prefix| prefix != LIBRARY_ROOT);
        if self.library_read {
            caps.duckfs_read.push(LIBRARY_ROOT.into());
            caps.duckfs_read.sort();
        }
        caps
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchPolicyKind {
    Mention,
    All,
    RoundRobin,
    Assigned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchDraft {
    pub channel_id: String,
    pub policy: WatchPolicyKind,
    pub assigned_agent: String,
}

impl Default for WatchDraft {
    fn default() -> Self {
        Self {
            channel_id: String::new(),
            policy: WatchPolicyKind::Mention,
            assigned_agent: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub data: Resource<AgentData>,
    pub tab: Tab,
    pub run_filter: RunFilter,
    pub selected_agent_id: Option<String>,
    pub explicit_selection: bool,
    pub adding: bool,
    pub editing: Option<EditDraft>,
    pub register: AgentDraft,
    pub watch: WatchDraft,
    pub busy: bool,
    pub error: Option<String>,
    pub expanded_run_logs: Vec<String>,
    pub run_logs: BTreeMap<String, RunLog>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            data: Resource::Loading,
            tab: Tab::Agents,
            run_filter: RunFilter::All,
            selected_agent_id: None,
            explicit_selection: false,
            adding: false,
            editing: None,
            register: AgentDraft::default(),
            watch: WatchDraft::default(),
            busy: false,
            error: None,
            expanded_run_logs: Vec::new(),
            run_logs: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Load,
    SelectTab(Tab),
    SelectRunFilter(RunFilter),
    SelectAgent(String),
    SelectFocusedAgent(String),
    ClearExplicitSelection,
    StartAdding,
    CancelAdding,
    RegisterNameChanged(String),
    RegisterIdChanged(String),
    RegisterCapabilityChanged(String),
    ToggleRegisterAction(String),
    RegisterForgeReadChanged(String),
    RegisterForgePushChanged(String),
    RegisterDuckfsReadChanged(String),
    RegisterDuckfsWriteChanged(String),
    RegisterToolsChanged(String),
    RegisterSecretsChanged(String),
    RegisterPagesChanged(String),
    RegisterBudgetChanged(String),
    RegisterLibraryChanged(bool),
    ToggleRegisterAdvanced,
    SkillNameChanged(String),
    SkillPrefixChanged(String),
    SkillLoadChanged(LoadMode),
    AddSkill,
    RemoveSkill(usize),
    Register,
    StartEditing,
    CloseEditing,
    EditNameChanged(String),
    EditCapabilityChanged(String),
    ToggleEditAction(String),
    EditForgeReadChanged(String),
    EditForgePushChanged(String),
    EditDuckfsReadChanged(String),
    EditDuckfsWriteChanged(String),
    EditToolsChanged(String),
    EditSecretsChanged(String),
    EditPagesChanged(String),
    EditBudgetChanged(String),
    EditLibraryChanged(bool),
    SaveEdit,
    ToggleAgentStatus,
    WatchChannelChanged(String),
    WatchPolicyChanged(WatchPolicyKind),
    WatchAssignedChanged(String),
    AddWatch,
    RemoveWatch(String),
    SetJobWorker(bool),
    CancelRun(String),
    ReassignRun(String, u32),
    ToggleRunLog(String),
    OpenRunAnchor { channel_id: String, sequence: u64 },
    OpenRunPullRequest { channel_id: String, number: u64 },
    RunLog(RunLogEvent),
    RetryCapabilities,
    Service(ServiceEvent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Load,
    RefreshCapabilities,
    RegisterAgent {
        display_name: String,
        agent_id: String,
        capability: String,
        allowed_actions: Vec<String>,
        caps: ResourceCaps,
        skills: Vec<SkillRef>,
    },
    UpdateAgent {
        agent_id: String,
        display_name: String,
        capability: String,
        allowed_actions: Vec<String>,
        caps: ResourceCaps,
        skills: Vec<SkillRef>,
    },
    PauseAgent(String),
    ResumeAgent(String),
    WatchChannel {
        channel_id: String,
        policy: TurnPolicy,
    },
    UnwatchChannel(String),
    SetJobWorker(bool),
    CancelRun(String),
    ReassignRun {
        run_id: String,
        attempt: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Command(Command),
    Intent(AppIntent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceEvent {
    Loaded(Result<Option<AgentData>, String>),
    CapabilitiesLoaded(Result<Vec<String>, String>),
    WriteFinished(Result<(), String>),
}

pub fn reduce(state: &mut State, message: Message) -> Option<Effect> {
    match message {
        Message::OpenRunAnchor {
            channel_id,
            sequence,
        } => Some(Effect::Intent(
            if let Some((repository, number)) = forge_item_channel(&channel_id) {
                AppIntent::Navigate(Route::Forge {
                    repository: repository.to_owned(),
                    item: Some(number),
                })
            } else {
                AppIntent::Navigate(Route::Chat {
                    channel: Some(channel_id),
                    message: Some(sequence),
                })
            },
        )),
        Message::OpenRunPullRequest { channel_id, number } => {
            let (repository, _) = forge_item_channel(&channel_id)?;
            Some(Effect::Intent(AppIntent::Navigate(Route::Forge {
                repository: repository.to_owned(),
                item: Some(number),
            })))
        }
        message => update(state, message).map(Effect::Command),
    }
}

pub fn update(state: &mut State, message: Message) -> Option<Command> {
    match message {
        Message::Load => {
            *state = State::default();
            Some(Command::Load)
        }
        Message::SelectTab(tab) => {
            state.tab = tab;
            None
        }
        Message::SelectRunFilter(filter) => {
            state.run_filter = filter;
            None
        }
        Message::SelectAgent(id) => {
            state.selected_agent_id = Some(id);
            state.explicit_selection = true;
            state.adding = false;
            state.editing = None;
            None
        }
        Message::SelectFocusedAgent(id) => {
            state.tab = Tab::Agents;
            state.selected_agent_id = Some(id);
            state.explicit_selection = true;
            state.adding = false;
            state.editing = None;
            None
        }
        Message::ClearExplicitSelection => {
            state.selected_agent_id = None;
            state.explicit_selection = false;
            state.editing = None;
            None
        }
        Message::StartAdding => {
            state.tab = Tab::Agents;
            state.adding = true;
            state.editing = None;
            state.error = None;
            None
        }
        Message::CancelAdding => {
            state.adding = false;
            state.register = AgentDraft::default();
            state.error = None;
            None
        }
        Message::RegisterNameChanged(value) => {
            state.register.display_name = value;
            None
        }
        Message::RegisterIdChanged(value) => {
            state.register.id_override = value;
            None
        }
        Message::RegisterCapabilityChanged(value) => {
            state.register.capability = value;
            None
        }
        Message::ToggleRegisterAction(action) => {
            toggle(&mut state.register.allowed_actions, action);
            None
        }
        Message::RegisterForgeReadChanged(value) => {
            state.register.forge_read = value;
            None
        }
        Message::RegisterForgePushChanged(value) => {
            state.register.forge_push = value;
            None
        }
        Message::RegisterDuckfsReadChanged(value) => {
            state.register.duckfs_read = value;
            None
        }
        Message::RegisterDuckfsWriteChanged(value) => {
            state.register.duckfs_write = value;
            None
        }
        Message::RegisterToolsChanged(value) => {
            state.register.tools = value;
            None
        }
        Message::RegisterSecretsChanged(value) => {
            state.register.secrets = value;
            None
        }
        Message::RegisterPagesChanged(value) => {
            state.register.pages_write = value;
            None
        }
        Message::RegisterBudgetChanged(value) => {
            if value.is_empty() || value.parse::<u32>().is_ok() {
                state.register.subagent_budget = value;
            }
            None
        }
        Message::RegisterLibraryChanged(value) => {
            state.register.library_read = value;
            None
        }
        Message::ToggleRegisterAdvanced => {
            state.register.advanced = !state.register.advanced;
            None
        }
        Message::SkillNameChanged(value) => {
            state.register.skill_name = value;
            None
        }
        Message::SkillPrefixChanged(value) => {
            state.register.skill_prefix = value;
            None
        }
        Message::SkillLoadChanged(value) => {
            state.register.skill_load = value;
            None
        }
        Message::AddSkill => {
            let name = state.register.skill_name.trim();
            let prefix = state.register.skill_prefix.trim();
            if !name.is_empty() && prefix.starts_with('/') {
                state.register.skills.push(SkillRef {
                    name: name.into(),
                    source_prefix: prefix.trim_end_matches('/').into(),
                    snapshot: None,
                    load: state.register.skill_load,
                });
                state.register.skill_name.clear();
                state.register.skill_prefix.clear();
            }
            None
        }
        Message::RemoveSkill(index) => {
            if index < state.register.skills.len() {
                state.register.skills.remove(index);
            }
            None
        }
        Message::Register => {
            let Resource::Ready(data) = &state.data else {
                return None;
            };
            if !state.register.ready(data) || state.busy {
                return None;
            }
            state.busy = true;
            state.error = None;
            Some(Command::RegisterAgent {
                display_name: state.register.display_name.trim().into(),
                agent_id: state.register.id(),
                capability: state.register.capability.clone(),
                allowed_actions: state.register.allowed_actions.clone(),
                caps: state.register.caps(),
                skills: state.register.skills.clone(),
            })
        }
        Message::StartEditing => {
            let agent = selected_agent(state)?.clone();
            state.editing = Some(EditDraft::from(&agent));
            None
        }
        Message::CloseEditing => {
            state.editing = None;
            None
        }
        Message::EditNameChanged(value) => {
            state.editing.as_mut()?.display_name = value;
            None
        }
        Message::EditCapabilityChanged(value) => {
            state.editing.as_mut()?.capability = value;
            None
        }
        Message::ToggleEditAction(action) => {
            toggle(&mut state.editing.as_mut()?.allowed_actions, action);
            None
        }
        Message::EditForgeReadChanged(value) => {
            state.editing.as_mut()?.forge_read = value;
            None
        }
        Message::EditForgePushChanged(value) => {
            state.editing.as_mut()?.forge_push = value;
            None
        }
        Message::EditDuckfsReadChanged(value) => {
            state.editing.as_mut()?.duckfs_read = value;
            None
        }
        Message::EditDuckfsWriteChanged(value) => {
            state.editing.as_mut()?.duckfs_write = value;
            None
        }
        Message::EditToolsChanged(value) => {
            state.editing.as_mut()?.tools = value;
            None
        }
        Message::EditSecretsChanged(value) => {
            state.editing.as_mut()?.secrets = value;
            None
        }
        Message::EditPagesChanged(value) => {
            state.editing.as_mut()?.pages_write = value;
            None
        }
        Message::EditBudgetChanged(value) => {
            if value.is_empty() || value.parse::<u32>().is_ok() {
                state.editing.as_mut()?.subagent_budget = value;
            }
            None
        }
        Message::EditLibraryChanged(value) => {
            state.editing.as_mut()?.library_read = value;
            None
        }
        Message::SaveEdit => {
            let edit = state.editing.as_ref()?;
            if state.busy || edit.display_name.trim().is_empty() || edit.capability.is_empty() {
                return None;
            }
            state.busy = true;
            Some(Command::UpdateAgent {
                agent_id: edit.agent_id.clone(),
                display_name: edit.display_name.trim().into(),
                capability: edit.capability.clone(),
                allowed_actions: edit.allowed_actions.clone(),
                caps: edit.caps(),
                skills: edit.skills.clone(),
            })
        }
        Message::ToggleAgentStatus => {
            let (id, status, pending) = selected_agent(state)
                .map(|agent| (agent.id.clone(), agent.status, agent.pending))?;
            if pending || state.busy {
                return None;
            }
            state.busy = true;
            Some(match status {
                AgentStatus::Active => Command::PauseAgent(id),
                AgentStatus::Paused => Command::ResumeAgent(id),
            })
        }
        Message::WatchChannelChanged(value) => {
            state.watch.channel_id = value;
            None
        }
        Message::WatchPolicyChanged(value) => {
            state.watch.policy = value;
            None
        }
        Message::WatchAssignedChanged(value) => {
            state.watch.assigned_agent = value;
            None
        }
        Message::AddWatch => {
            if state.busy || state.watch.channel_id.is_empty() {
                return None;
            }
            let policy = match state.watch.policy {
                WatchPolicyKind::Mention => TurnPolicy::Mention,
                WatchPolicyKind::All => TurnPolicy::All,
                WatchPolicyKind::RoundRobin => TurnPolicy::RoundRobin,
                WatchPolicyKind::Assigned if !state.watch.assigned_agent.is_empty() => {
                    TurnPolicy::Assigned(state.watch.assigned_agent.clone())
                }
                WatchPolicyKind::Assigned => return None,
            };
            state.busy = true;
            Some(Command::WatchChannel {
                channel_id: state.watch.channel_id.clone(),
                policy,
            })
        }
        Message::RemoveWatch(channel) => {
            if state.busy {
                return None;
            }
            state.busy = true;
            Some(Command::UnwatchChannel(channel))
        }
        Message::SetJobWorker(enabled) => {
            let Resource::Ready(data) = &state.data else {
                return None;
            };
            if data.job_worker_pending || state.busy {
                return None;
            }
            state.busy = true;
            Some(Command::SetJobWorker(enabled))
        }
        Message::CancelRun(id) => {
            state.busy = true;
            Some(Command::CancelRun(id))
        }
        Message::ReassignRun(run_id, attempt) => {
            state.busy = true;
            Some(Command::ReassignRun { run_id, attempt })
        }
        Message::ToggleRunLog(dispatch_id) => {
            if let Some(index) = state
                .expanded_run_logs
                .iter()
                .position(|current| current == &dispatch_id)
            {
                state.expanded_run_logs.remove(index);
            } else {
                state.expanded_run_logs.push(dispatch_id.clone());
                state.run_logs.entry(dispatch_id).or_default();
            }
            None
        }
        Message::OpenRunAnchor { .. } | Message::OpenRunPullRequest { .. } => None,
        Message::RunLog(event) => {
            match event {
                RunLogEvent::Connected => {
                    for dispatch_id in &state.expanded_run_logs {
                        state
                            .run_logs
                            .entry(dispatch_id.clone())
                            .or_default()
                            .unavailable = false;
                    }
                }
                RunLogEvent::Line {
                    dispatch_id,
                    cursor,
                    stream,
                    text,
                } => {
                    let log = state.run_logs.entry(dispatch_id).or_default();
                    if cursor <= log.last_cursor {
                        return None;
                    }
                    log.last_cursor = cursor;
                    let entry = if text.len() > MAX_RAW_ENTRY_CHARS {
                        RunLogEntry::Gap(format!(
                            "provider event omitted: {} characters over limit",
                            text.len() - MAX_RAW_ENTRY_CHARS
                        ))
                    } else {
                        RunLogEntry::Line { stream, text }
                    };
                    append_run_log(log, entry);
                }
                RunLogEvent::Lagged {
                    dispatch_id,
                    cursor,
                } => {
                    let log = state.run_logs.entry(dispatch_id).or_default();
                    if cursor > log.last_cursor {
                        log.last_cursor = cursor;
                        append_run_log(
                            log,
                            RunLogEntry::Gap(format!(
                                "output gap: older lines were dropped before cursor {cursor}"
                            )),
                        );
                    }
                }
                RunLogEvent::Unavailable {
                    dispatch_id,
                    reason: _,
                } => match dispatch_id {
                    Some(dispatch_id) => {
                        state.run_logs.entry(dispatch_id).or_default().unavailable = true;
                    }
                    None => {
                        for dispatch_id in &state.expanded_run_logs {
                            state
                                .run_logs
                                .entry(dispatch_id.clone())
                                .or_default()
                                .unavailable = true;
                        }
                    }
                },
                RunLogEvent::Disconnected(_) => {}
            }
            None
        }
        Message::RetryCapabilities => Some(Command::RefreshCapabilities),
        Message::Service(event) => service(state, event),
    }
}

fn append_run_log(log: &mut RunLog, entry: RunLogEntry) {
    if log.entries.len() == MAX_ACTIVITY_ENTRIES {
        log.entries.remove(0);
        log.dropped = log.dropped.saturating_add(1);
    }
    log.entries.push(entry);
}

fn semantic_log_rows(entries: &[RunLogEntry]) -> Vec<SemanticLogRow> {
    let mut rows = Vec::new();
    let mut started = BTreeMap::<String, usize>::new();
    let first = entries.len().saturating_sub(MAX_ACTIVITY_ENTRIES);
    for entry in &entries[first..] {
        match entry {
            RunLogEntry::Gap(text) => push_log_row(
                &mut rows,
                SemanticLogRow {
                    kind: SemanticLogKind::Gap,
                    stream: None,
                    text: normalize_log_text(text),
                },
            ),
            RunLogEntry::Line { stream, text } => {
                if text.trim().is_empty() {
                    push_log_row(
                        &mut rows,
                        SemanticLogRow {
                            kind: SemanticLogKind::Blank,
                            stream: Some(*stream),
                            text: String::new(),
                        },
                    );
                } else if !parse_json_log_event(text, *stream, &mut rows, &mut started) {
                    push_log_text(&mut rows, SemanticLogKind::Text, *stream, text);
                }
            }
        }
    }
    if rows.len() <= MAX_ACTIVITY_ROWS {
        return rows;
    }
    let omitted = rows.len() - (MAX_ACTIVITY_ROWS - 1);
    let mut tail = rows.split_off(rows.len() - (MAX_ACTIVITY_ROWS - 1));
    tail.insert(
        0,
        SemanticLogRow {
            kind: SemanticLogKind::Gap,
            stream: None,
            text: format!("live log tail: {omitted} rendered rows omitted"),
        },
    );
    tail
}

fn parse_json_log_event(
    line: &str,
    stream: RunStream,
    rows: &mut Vec<SemanticLogRow>,
    started: &mut BTreeMap<String, usize>,
) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line.trim()) else {
        return false;
    };
    let Some(record) = value.as_object() else {
        if let Some(text) = value.as_str() {
            push_log_text(rows, SemanticLogKind::Text, stream, text);
        } else {
            push_log_status(rows, stream, "event: JSON value".into());
        }
        return true;
    };
    let event_type = json_string(record, "type");
    match event_type {
        Some("thread.started") => push_log_status(
            rows,
            stream,
            json_string(record, "thread_id").map_or_else(
                || "thread started".into(),
                |thread| format!("thread started: {thread}"),
            ),
        ),
        Some("turn.started") => push_log_status(rows, stream, "turn started".into()),
        Some("turn.completed") => push_log_status(rows, stream, "turn completed".into()),
        Some("item.started") | Some("item.completed") => {
            let completed = event_type == Some("item.completed");
            let Some(item) = record.get("item").and_then(serde_json::Value::as_object) else {
                push_log_status(
                    rows,
                    stream,
                    if completed {
                        "item completed".into()
                    } else {
                        "item started".into()
                    },
                );
                return true;
            };
            let key = log_item_key(item);
            let paired = if completed {
                key.as_ref().is_some_and(|key| take_started(started, key))
            } else {
                if let Some(key) = key {
                    *started.entry(key).or_default() += 1;
                }
                false
            };
            push_log_item(rows, stream, item, !completed || !paired, completed);
        }
        _ => {
            if let Some(message) = json_string(record, "message")
                .or_else(|| json_string(record, "text"))
                .or_else(|| json_string(record, "error"))
            {
                push_log_text(rows, SemanticLogKind::Text, stream, message);
            } else {
                push_log_status(
                    rows,
                    stream,
                    event_type.map_or_else(
                        || "event: JSON object".into(),
                        |kind| format!("event: {kind}"),
                    ),
                );
            }
        }
    }
    true
}

fn push_log_item(
    rows: &mut Vec<SemanticLogRow>,
    stream: RunStream,
    item: &serde_json::Map<String, serde_json::Value>,
    primary: bool,
    details: bool,
) {
    let item_type = json_string(item, "type");
    if primary {
        let label = match item_type {
            Some("agent_message") => {
                json_string(item, "text").map(|text| (SemanticLogKind::Message, text.to_owned()))
            }
            Some("command_execution") => json_string(item, "command")
                .map(|command| (SemanticLogKind::Command, command.to_owned())),
            Some("file_change") => {
                let files = changed_file_labels(item.get("changes"));
                Some((
                    SemanticLogKind::File,
                    if files == "file changes" {
                        files
                    } else {
                        format!("files: {files}")
                    },
                ))
            }
            Some("mcp_tool_call") => {
                let server = json_string(item, "server");
                let tool = json_string(item, "tool").or_else(|| json_string(item, "name"));
                let name = match (server, tool) {
                    (Some(server), Some(tool)) => format!("{server}/{tool}"),
                    (Some(name), None) | (None, Some(name)) => name.to_owned(),
                    (None, None) => "call".into(),
                };
                Some((SemanticLogKind::Tool, format!("MCP tool: {name}")))
            }
            Some(kind) => Some((SemanticLogKind::Text, format!("item: {kind}"))),
            None => None,
        };
        if let Some((kind, text)) = label {
            if matches!(kind, SemanticLogKind::Message | SemanticLogKind::Output) {
                push_log_text(rows, kind, stream, &text);
            } else {
                push_log_row(
                    rows,
                    SemanticLogRow {
                        kind,
                        stream: Some(stream),
                        text,
                    },
                );
            }
        }
    }
    if !details {
        return;
    }
    if item_type == Some("command_execution") {
        if let Some(output) =
            json_string(item, "aggregated_output").or_else(|| json_string(item, "output"))
        {
            push_log_text(rows, SemanticLogKind::Output, stream, output);
        }
    } else if item_type == Some("mcp_tool_call")
        && let Some(output) = json_string(item, "result").or_else(|| json_string(item, "error"))
    {
        push_log_text(rows, SemanticLogKind::Output, stream, output);
    }
    if let Some(status) = json_string(item, "status") {
        push_log_status(rows, stream, format!("status: {status}"));
    }
    if let Some(exit) = item.get("exit_code").and_then(|value| match value {
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::String(value) => Some(value.clone()),
        _ => None,
    }) {
        push_log_row(
            rows,
            SemanticLogRow {
                kind: SemanticLogKind::Exit,
                stream: Some(stream),
                text: format!("exit: {exit}"),
            },
        );
    }
}

fn log_item_key(item: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    let kind = json_string(item, "type")?;
    if let Some(id) = json_string(item, "id") {
        return Some(format!("{kind}:id:{id}"));
    }
    let primary = match kind {
        "agent_message" => json_string(item, "text")?.to_owned(),
        "command_execution" => json_string(item, "command")?.to_owned(),
        "file_change" => changed_file_labels(item.get("changes")),
        "mcp_tool_call" => {
            let server = json_string(item, "server");
            let tool = json_string(item, "tool").or_else(|| json_string(item, "name"));
            match (server, tool) {
                (Some(server), Some(tool)) => format!("{server}/{tool}"),
                (Some(name), None) | (None, Some(name)) => name.to_owned(),
                (None, None) => "call".into(),
            }
        }
        _ => kind.to_owned(),
    };
    Some(format!("{kind}:value:{primary}"))
}

fn take_started(started: &mut BTreeMap<String, usize>, key: &str) -> bool {
    let Some(count) = started.get_mut(key) else {
        return false;
    };
    *count -= 1;
    if *count == 0 {
        started.remove(key);
    }
    true
}

fn changed_file_labels(value: Option<&serde_json::Value>) -> String {
    let files = value
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .map(|change| {
            change.as_str().map(str::to_owned).unwrap_or_else(|| {
                change
                    .as_object()
                    .and_then(|change| {
                        json_string(change, "path")
                            .or_else(|| json_string(change, "file"))
                            .or_else(|| json_string(change, "filename"))
                    })
                    .unwrap_or("changed file")
                    .to_owned()
            })
        })
        .collect::<Vec<_>>();
    if files.is_empty() {
        "file changes".into()
    } else {
        files.join(", ")
    }
}

fn json_string<'a>(
    record: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<&'a str> {
    record.get(key).and_then(serde_json::Value::as_str)
}

fn push_log_status(rows: &mut Vec<SemanticLogRow>, stream: RunStream, text: String) {
    push_log_row(
        rows,
        SemanticLogRow {
            kind: SemanticLogKind::Status,
            stream: Some(stream),
            text,
        },
    );
}

fn push_log_text(
    rows: &mut Vec<SemanticLogRow>,
    kind: SemanticLogKind,
    stream: RunStream,
    text: &str,
) {
    let mut text = normalize_log_text(&clip_log_field(text));
    while text.ends_with('\n') {
        text.pop();
    }
    if text.is_empty() {
        return;
    }
    let lines = text.lines().map(str::to_owned).collect::<Vec<_>>();
    let visible = if lines.len() > MAX_FIELD_LINES {
        let omitted = lines.len() - FIELD_HEAD_LINES - FIELD_TAIL_LINES;
        let mut visible = lines[..FIELD_HEAD_LINES].to_vec();
        visible.push(format!("… {omitted} lines omitted …"));
        visible.extend_from_slice(&lines[lines.len() - FIELD_TAIL_LINES..]);
        visible
    } else {
        lines
    };
    for line in visible {
        let gap = line.starts_with('…') && line.ends_with("omitted …");
        push_log_row(
            rows,
            SemanticLogRow {
                kind: if gap { SemanticLogKind::Gap } else { kind },
                stream: Some(stream),
                text: line,
            },
        );
    }
}

fn push_log_row(rows: &mut Vec<SemanticLogRow>, mut row: SemanticLogRow) {
    row.text = clip_log_row(&row.text);
    if row.text.trim().is_empty() {
        row.kind = SemanticLogKind::Blank;
        row.text.clear();
    }
    if row.kind == SemanticLogKind::Blank
        && rows
            .last()
            .is_some_and(|row| row.kind == SemanticLogKind::Blank)
    {
        return;
    }
    rows.push(row);
}

fn normalize_log_text(text: &str) -> String {
    let normalized = text
        .replace("\\r\\n", "\n")
        .replace("\\n", "\n")
        .replace("\\r", "\r")
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let mut output = String::with_capacity(normalized.len());
    let mut newlines = 0;
    for character in normalized.chars() {
        if character == '\n' {
            newlines += 1;
            if newlines <= 2 {
                output.push(character);
            }
        } else {
            newlines = 0;
            output.push(character);
        }
    }
    output
}

fn clip_log_field(text: &str) -> String {
    clip_log_text(text, MAX_FIELD_CHARS)
}

fn clip_log_row(text: &str) -> String {
    clip_log_text(text, MAX_ROW_CHARS)
}

fn clip_log_text(text: &str, limit: usize) -> String {
    let count = text.chars().count();
    if count <= limit {
        return text.to_owned();
    }
    let omitted = count - limit;
    let marker = format!(" … {omitted} characters omitted … ");
    let available = limit.saturating_sub(marker.chars().count()).max(2);
    let head = available / 3;
    let tail = available - head;
    let start = text.chars().take(head).collect::<String>();
    let end = text
        .chars()
        .rev()
        .take(tail)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{start}{marker}{end}")
}

pub fn expanded_run_dispatches(state: &State) -> Vec<String> {
    state.expanded_run_logs.clone()
}

fn service(state: &mut State, event: ServiceEvent) -> Option<Command> {
    match event {
        ServiceEvent::Loaded(result) => {
            state.data = match result {
                Ok(Some(data)) => Resource::Ready(data),
                Ok(None) => Resource::Empty,
                Err(error) => Resource::Error(error),
            };
            if let Resource::Ready(data) = &state.data {
                if state.register.capability.is_empty() {
                    state.register.capability =
                        data.capabilities.first().cloned().unwrap_or_default();
                }
                if state.watch.channel_id.is_empty() {
                    state.watch.channel_id = data
                        .channels
                        .first()
                        .map_or_else(String::new, |channel| channel.id.clone());
                }
            }
        }
        ServiceEvent::CapabilitiesLoaded(result) => match (&mut state.data, result) {
            (Resource::Ready(data), Ok(capabilities)) => {
                data.capabilities = capabilities;
                data.capability_status = CapabilityStatus::Ready;
            }
            (Resource::Ready(data), Err(error)) => {
                data.capability_status = CapabilityStatus::Error;
                state.error = Some(error);
            }
            (_, Err(error)) => state.error = Some(error),
            _ => {}
        },
        ServiceEvent::WriteFinished(result) => {
            state.busy = false;
            match result {
                Ok(()) => {
                    state.error = None;
                    state.adding = false;
                    state.editing = None;
                    state.register = AgentDraft::default();
                    state.watch = WatchDraft::default();
                    return Some(Command::Load);
                }
                Err(error) => state.error = Some(error),
            }
        }
    }
    None
}

fn toggle(values: &mut Vec<String>, value: String) {
    if let Some(index) = values.iter().position(|current| current == &value) {
        values.remove(index);
    } else {
        values.push(value);
    }
}

pub fn view(state: &State, mode: theme::Mode, accent: Color) -> Element<'_, Message> {
    let p = Colors {
        palette: *theme::palette(mode),
        accent,
    };
    let data = match &state.data {
        Resource::Ready(data) => Some(data),
        _ => None,
    };
    let header = header(state, data, p);
    let body: Element<'_, Message> = match &state.data {
        Resource::Loading => center_state("Loading agents...", "", p),
        Resource::Empty => center_state("No agent data", "The workspace has not loaded yet.", p),
        Resource::Error(error) => center_state("Agents unavailable", error, p),
        Resource::Ready(data) => match state.tab {
            Tab::Agents => agents_tab(state, data, p),
            Tab::AutoReply => auto_reply_tab(state, data, p),
            Tab::Activity => activity_tab(state, data, p),
        },
    };
    column![header, body]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn header<'a>(state: &'a State, data: Option<&'a AgentData>, p: Colors) -> Element<'a, Message> {
    let agents = data.map_or(0, |data| data.agents.len());
    let watches = data.map_or(0, |data| data.watches.len());
    let runs = data.map_or(0, |data| data.pending_runs.len() + data.recent_runs.len());
    container(
        row![
            icon_tile(Icon::Agent, 30.0, p),
            text("Agents").font(SANS_SEMIBOLD).size(18).color(p.ink),
            Space::new().width(Length::Fill),
            container(
                row![
                    header_tab("Agents", agents, Tab::Agents, state.tab, p),
                    header_tab("Auto-reply", watches, Tab::AutoReply, state.tab, p),
                    header_tab("Activity", runs, Tab::Activity, state.tab, p)
                ]
                .spacing(4)
                .padding(3)
            )
            .style(move |_| rounded_surface(p.sidebar, p.border, RADIUS_MD)),
            primary_button("+ Add agent", Some(Message::StartAdding), p)
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .height(HEADER_HEIGHT)
    .padding([0, 22])
    .style(move |_| bottom_rule(p.paper, p.border_soft))
    .into()
}

fn agents_tab<'a>(state: &'a State, data: &'a AgentData, p: Colors) -> Element<'a, Message> {
    row![roster(state, data, p), detail_pane(state, data, p)]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn roster<'a>(state: &'a State, data: &'a AgentData, p: Colors) -> Element<'a, Message> {
    let selected = selected_agent(state).map(|agent| agent.id.as_str());
    let mut list = column![
        row![
            section_label("ROSTER", p),
            Space::new().width(Length::Fill),
            text(format!("{} total", data.agents.len()))
                .font(MONO)
                .size(10.5)
                .color(p.muted_2)
        ]
        .padding([14, 14])
        .align_y(Alignment::Center)
    ];
    if data.agents.is_empty() {
        list = list.push(empty_state(
            "No agents yet",
            "Add an agent to get started.",
            p,
        ));
    } else {
        for agent in &data.agents {
            list = list.push(roster_row(
                agent,
                !state.adding && selected == Some(agent.id.as_str()),
                p,
            ));
        }
    }
    container(scrollable(list))
        .width(ROSTER_WIDTH)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(p.sidebar)),
            border: Border {
                color: p.border_soft,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn roster_row(agent: &AgentRecord, selected: bool, p: Colors) -> Element<'static, Message> {
    let active = agent.status == AgentStatus::Active;
    button(
        row![
            avatar(&agent.display_name, 36.0, p.filled, p.on_filled, p),
            column![
                text(agent.display_name.clone())
                    .font(SANS_SEMIBOLD)
                    .size(13.5)
                    .color(if selected { p.accent } else { p.ink }),
                row![
                    status_dot(if active { p.green } else { p.amber }),
                    text(if active { "Active" } else { "Paused" })
                        .font(SANS)
                        .size(10.5)
                        .color(p.muted_3),
                    text("·").color(p.icon_idle),
                    text(capability_short(&agent.capability))
                        .font(MONO)
                        .size(10.5)
                        .color(p.muted_2)
                ]
                .spacing(6)
                .align_y(Alignment::Center)
            ]
            .spacing(3)
            .width(Length::Fill)
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([12, 14])
    .style(move |_, status| button::Style {
        background: (selected || matches!(status, button::Status::Hovered))
            .then_some(Background::Color(if selected { p.sunken } else { p.hover })),
        text_color: p.ink,
        border: Border {
            color: p.border_soft,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    })
    .on_press(Message::SelectAgent(agent.id.clone()))
    .into()
}

fn detail_pane<'a>(state: &'a State, data: &'a AgentData, p: Colors) -> Element<'a, Message> {
    let content: Element<'a, Message> = if state.adding {
        register_form(state, data, p)
    } else if let Some(agent) = selected_agent(state) {
        agent_detail(state, data, agent, p)
    } else if state.explicit_selection
        && state.selected_agent_id.is_some()
        && !data.agents.is_empty()
    {
        missing_agent(state.selected_agent_id.as_deref().unwrap_or_default(), p)
    } else {
        no_agents(p)
    };
    container(scrollable(
        container(content).width(Length::Fill).padding(22),
    ))
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn register_form<'a>(state: &'a State, data: &'a AgentData, p: Colors) -> Element<'a, Message> {
    let draft = &state.register;
    let ready = draft.ready(data) && !state.busy;
    let mut form = column![
        section_label("REGISTER AGENT", p),
        card(
            column![
                row![
                    avatar(
                        if draft.display_name.is_empty() { "AI" } else { &draft.display_name },
                        40.0,
                        p.filled,
                        p.on_filled,
                        p,
                    ),
                    column![
                        text("Add an agent").font(SANS_SEMIBOLD).size(13.5).color(p.ink),
                        text("Give it a name, pick what it runs on, and curate the documents it carries.")
                            .font(SANS)
                            .size(11.5)
                            .color(p.muted_2)
                    ]
                    .spacing(3)
                    .width(Length::Fill),
                    pill("AGENT", p.accent, p)
                ]
                .spacing(12)
                .align_y(Alignment::Start),
                row![
                    labeled_input(
                        "AGENT DISPLAY NAME",
                        "Triage Agent…",
                        &draft.display_name,
                        Message::RegisterNameChanged,
                        p,
                    ),
                    labeled_input(
                        "RUNS ON",
                        "codex_gpt-5_medium",
                        &draft.capability,
                        Message::RegisterCapabilityChanged,
                        p,
                    )
                ]
                .spacing(9),
                skill_editor(state, p),
                permission_grid(&draft.allowed_actions, false, p),
                section_label("RESOURCE CAPS", p),
                checkbox(draft.library_read)
                    .label("Can search the global skill library")
                    .on_toggle(Message::RegisterLibraryChanged)
                    .size(15)
                    .text_size(10.5),
                row![
                    labeled_mono_input(
                        "FORGE READ REPOSITORIES",
                        "repo names",
                        &draft.forge_read,
                        Message::RegisterForgeReadChanged,
                        p,
                    ),
                    labeled_mono_input(
                        "FORGE PUSH REPOSITORIES",
                        "repo names",
                        &draft.forge_push,
                        Message::RegisterForgePushChanged,
                        p,
                    )
                ]
                .spacing(9),
                row![
                    labeled_mono_input(
                        "ADDITIONAL DUCKFS READ PREFIXES",
                        "/shared/data /projects/demo",
                        &draft.duckfs_read,
                        Message::RegisterDuckfsReadChanged,
                        p,
                    ),
                    labeled_mono_input(
                        "DUCKFS WRITE PREFIXES",
                        "/shared/agents/my-agent",
                        &draft.duckfs_write,
                        Message::RegisterDuckfsWriteChanged,
                        p,
                    )
                ]
                .spacing(9),
                row![
                    labeled_mono_input(
                        "ALLOWED TOOL IDS",
                        "tool or MCP ids",
                        &draft.tools,
                        Message::RegisterToolsChanged,
                        p,
                    ),
                    labeled_mono_input(
                        "SECRET REFERENCES",
                        "opaque vault references",
                        &draft.secrets,
                        Message::RegisterSecretsChanged,
                        p,
                    )
                ]
                .spacing(9),
                row![
                    labeled_mono_input(
                        "PAGE WRITE ACCESS",
                        "page ids, comma separated, or *",
                        &draft.pages_write,
                        Message::RegisterPagesChanged,
                        p,
                    ),
                    labeled_mono_input(
                        "CONCURRENT PEER CALLS",
                        "0",
                        &draft.subagent_budget,
                        Message::RegisterBudgetChanged,
                        p,
                    )
                ]
                .spacing(9),
                text("Maximum live calls across the recursive call tree; completed calls release their slot. 0 disables calls and the runtime hard cap is 8.")
                    .font(SANS)
                    .size(10.5)
                    .color(p.muted_2),
                secondary_button(
                    if draft.advanced { "Hide advanced" } else { "Advanced" },
                    Some(Message::ToggleRegisterAdvanced),
                    p,
                ),
                if draft.advanced {
                    labeled_input(
                        "AGENT ID",
                        "derived from display name",
                        &draft.id_override,
                        Message::RegisterIdChanged,
                        p,
                    )
                } else {
                    text(format!("Agent id: {}", draft.id())).font(MONO).size(10.5).color(p.muted_2).into()
                },
                row![
                    Space::new().width(Length::Fill),
                    secondary_button("Cancel", Some(Message::CancelAdding), p),
                    primary_button(
                        if state.busy { "Registering..." } else { "Register agent" },
                        ready.then_some(Message::Register),
                        p,
                    )
                ]
                .spacing(8)
            ]
            .spacing(14)
            .padding(16),
            p,
        )
    ]
    .spacing(9);
    if data.capability_status == CapabilityStatus::Error {
        form = form.push(
            row![
                text("Could not load executor capabilities.")
                    .font(SANS)
                    .size(11.5)
                    .color(p.red),
                secondary_button("Retry", Some(Message::RetryCapabilities), p)
            ]
            .spacing(8),
        );
    }
    if let Some(error) = &state.error {
        form = form.push(error_banner(error, p));
    }
    form.into()
}

fn skill_editor(state: &State, p: Colors) -> Element<'_, Message> {
    let draft = &state.register;
    let mut skills = column![section_label("CURATED SKILLS", p)].spacing(7);
    for (index, skill) in draft.skills.iter().enumerate() {
        skills = skills.push(
            row![
                pill(
                    if skill.load == LoadMode::Always {
                        "ALWAYS"
                    } else {
                        "ON DEMAND"
                    },
                    if skill.load == LoadMode::Always {
                        p.accent
                    } else {
                        p.purple
                    },
                    p
                ),
                text(skill.name.clone())
                    .font(SANS_SEMIBOLD)
                    .size(12)
                    .color(p.ink),
                text(skill.source_prefix.clone())
                    .font(MONO)
                    .size(10.5)
                    .color(p.muted_2)
                    .width(Length::Fill),
                secondary_button("Remove", Some(Message::RemoveSkill(index)), p)
            ]
            .spacing(9)
            .align_y(Alignment::Center),
        );
    }
    skills = skills.push(
        row![
            text_input("Skill name", &draft.skill_name)
                .font(SANS)
                .size(11.5)
                .on_input(Message::SkillNameChanged),
            text_input("/skills/name", &draft.skill_prefix)
                .font(MONO)
                .size(11.5)
                .on_input(Message::SkillPrefixChanged),
            secondary_button(
                if draft.skill_load == LoadMode::Always {
                    "Always"
                } else {
                    "On demand"
                },
                Some(Message::SkillLoadChanged(
                    if draft.skill_load == LoadMode::Always {
                        LoadMode::OnDemand
                    } else {
                        LoadMode::Always
                    }
                )),
                p,
            ),
            secondary_button(
                "Add",
                (!draft.skill_name.trim().is_empty() && draft.skill_prefix.trim().starts_with('/'))
                    .then_some(Message::AddSkill),
                p,
            )
        ]
        .spacing(7),
    );
    skills.into()
}

fn agent_detail<'a>(
    state: &'a State,
    data: &'a AgentData,
    agent: &'a AgentRecord,
    p: Colors,
) -> Element<'a, Message> {
    let active = agent.status == AgentStatus::Active;
    let identity = container(
        row![
            avatar(&agent.display_name, 50.0, p.accent, Color::WHITE, p),
            column![
                text(agent.display_name.clone())
                    .font(SANS_SEMIBOLD)
                    .size(20)
                    .color(p.on_filled),
                row![
                    text(agent.id.clone()).font(MONO).size(11).color(mix(
                        p.filled,
                        p.on_filled,
                        0.7
                    )),
                    on_dark_pill(
                        if active { "ACTIVE" } else { "PAUSED" },
                        if active { p.green } else { p.amber },
                        p
                    )
                ]
                .spacing(9)
                .align_y(Alignment::Center)
            ]
            .spacing(6)
            .width(Length::Fill),
            on_dark_button(
                if state.editing.is_some() {
                    "Close edit"
                } else {
                    "Edit"
                },
                (!agent.pending && !state.busy).then_some(if state.editing.is_some() {
                    Message::CloseEditing
                } else {
                    Message::StartEditing
                }),
                p,
            ),
            on_dark_button(
                if active {
                    "Pause agent"
                } else {
                    "Resume agent"
                },
                (!agent.pending && !state.busy).then_some(Message::ToggleAgentStatus),
                p,
            )
        ]
        .spacing(14)
        .align_y(Alignment::Start),
    )
    .padding(Padding {
        top: 18.0,
        right: 18.0,
        bottom: 17.0,
        left: 18.0,
    })
    .style(move |_| surface(p.filled));

    let mut body = column![
        section_label("RUNS ON", p),
        container(capability_strip(&agent.capability, p))
            .padding([12, 14])
            .style(move |_| rounded_surface(p.sunken, p.border, RADIUS_MD)),
        section_label("CURATED SKILLS", p)
    ]
    .spacing(8)
    .padding(18);
    if agent.skills.is_empty() {
        body = body.push(
            text("No curated skills.")
                .font(SANS)
                .size(11.5)
                .color(p.muted_2),
        );
    } else {
        for skill in &agent.skills {
            body = body.push(skill_row(skill, p));
        }
    }
    body = body
        .push(section_label("IDENTITY", p))
        .push(info_row(
            "Agent address",
            &format!("{}@agents.duck", agent.id),
            p,
        ))
        .push(info_row("Owner", &agent.owner.label(), p))
        .push(info_row("Created", &agent.created_at, p))
        .push(info_row("Updated", &agent.updated_at, p))
        .push(section_label("PERMISSIONS", p));
    if agent.allowed_actions.is_empty() {
        body = body.push(
            text("Can't take any actions yet.")
                .font(SANS)
                .size(11.5)
                .color(p.muted_2),
        );
    } else {
        let mut permissions = row![].spacing(7);
        for action in &agent.allowed_actions {
            permissions = permissions.push(pill(action, p.accent, p));
        }
        body = body.push(permissions.wrap());
    }
    body = body
        .push(section_label("RESOURCE CAPS", p))
        .push(resource_caps_chips(&agent.caps, p));
    if let Some(edit) = &state.editing {
        body = body.push(edit_form(state, data, edit, p));
    }
    if let Some(error) = &state.error {
        body = body.push(error_banner(error, p));
    }
    container(column![identity, body])
        .width(Length::Fill)
        .style(move |_| card_style(p))
        .into()
}

fn resource_caps_chips(caps: &ResourceCaps, p: Colors) -> Element<'static, Message> {
    let mut grants = row![].spacing(7);
    for grant in resource_grant_labels(caps) {
        grants = grants.push(pill(&grant, p.muted_3, p));
    }
    grants.wrap().into()
}

fn resource_grant_labels(caps: &ResourceCaps) -> Vec<String> {
    let mut grants = Vec::new();
    for (label, values) in [
        ("Forge read", &caps.forge_read),
        ("Forge push", &caps.forge_push),
        ("DuckFS read", &caps.duckfs_read),
        ("DuckFS write", &caps.duckfs_write),
        ("Tool", &caps.tools),
        ("Secret", &caps.secrets),
        ("Page write", &caps.pages_write),
    ] {
        for value in values {
            grants.push(format!("{label}: {value}"));
        }
    }
    grants.push(format!(
        "Concurrent peer calls: {}",
        caps.subagent_budget.unwrap_or(0)
    ));
    grants
}

fn edit_form<'a>(
    state: &'a State,
    data: &'a AgentData,
    edit: &'a EditDraft,
    p: Colors,
) -> Element<'a, Message> {
    let valid = !edit.display_name.trim().is_empty()
        && data.capabilities.contains(&edit.capability)
        && !edit.allowed_actions.is_empty()
        && !state.busy;
    let form = column![
        horizontal_divider(p),
        section_label("EDIT AGENT", p),
        row![
            labeled_input(
                "DISPLAY NAME",
                "Name",
                &edit.display_name,
                Message::EditNameChanged,
                p
            ),
            labeled_input(
                "RUNS ON",
                "Capability",
                &edit.capability,
                Message::EditCapabilityChanged,
                p
            )
        ]
        .spacing(9),
        permission_grid(&edit.allowed_actions, true, p),
        section_label("RESOURCE CAPS", p),
        checkbox(edit.library_read)
            .label("Can search the global skill library")
            .on_toggle(Message::EditLibraryChanged)
            .size(15)
            .text_size(10.5),
        row![
            labeled_mono_input(
                "FORGE READ REPOSITORIES",
                "repo names",
                &edit.forge_read,
                Message::EditForgeReadChanged,
                p,
            ),
            labeled_mono_input(
                "FORGE PUSH REPOSITORIES",
                "repo names",
                &edit.forge_push,
                Message::EditForgePushChanged,
                p,
            )
        ]
        .spacing(9),
        row![
            labeled_mono_input(
                "ADDITIONAL DUCKFS READ PREFIXES",
                "/shared/data /projects/demo",
                &edit.duckfs_read,
                Message::EditDuckfsReadChanged,
                p,
            ),
            labeled_mono_input(
                "DUCKFS WRITE PREFIXES",
                "/shared/agents/my-agent",
                &edit.duckfs_write,
                Message::EditDuckfsWriteChanged,
                p,
            )
        ]
        .spacing(9),
        row![
            labeled_mono_input(
                "ALLOWED TOOL IDS",
                "tool or MCP ids",
                &edit.tools,
                Message::EditToolsChanged,
                p,
            ),
            labeled_mono_input(
                "SECRET REFERENCES",
                "opaque vault references",
                &edit.secrets,
                Message::EditSecretsChanged,
                p,
            )
        ]
        .spacing(9),
        row![
            labeled_mono_input(
                "PAGE WRITE ACCESS",
                "page ids, comma separated, or *",
                &edit.pages_write,
                Message::EditPagesChanged,
                p,
            ),
            labeled_mono_input(
                "CONCURRENT PEER CALLS",
                "0",
                &edit.subagent_budget,
                Message::EditBudgetChanged,
                p,
            )
        ]
        .spacing(9),
        text("Maximum live calls across the recursive call tree; completed calls release their slot. 0 disables calls and the runtime hard cap is 8.")
            .font(SANS)
            .size(10.5)
            .color(p.muted_2),
        row![
            Space::new().width(Length::Fill),
            secondary_button("Cancel", Some(Message::CloseEditing), p),
            primary_button(
                if state.busy {
                    "Saving..."
                } else {
                    "Save changes"
                },
                valid.then_some(Message::SaveEdit),
                p,
            )
        ]
        .spacing(8)
    ]
    .spacing(10)
    .padding([14, 0]);
    form.into()
}

fn permission_grid<'a>(selected: &'a [String], editing: bool, p: Colors) -> Element<'a, Message> {
    let mut grid = column![section_label("PERMISSIONS", p)].spacing(7);
    for (action, label) in ACTIONS {
        let action_owned = action.to_owned();
        let checked = selected.iter().any(|value| value == action);
        grid = grid.push(
            checkbox(checked)
                .label(label)
                .on_toggle(move |_| {
                    if editing {
                        Message::ToggleEditAction(action_owned.clone())
                    } else {
                        Message::ToggleRegisterAction(action_owned.clone())
                    }
                })
                .size(15)
                .text_size(10.5),
        );
    }
    grid.into()
}

fn skill_row(skill: &SkillRef, p: Colors) -> Element<'static, Message> {
    container(
        row![
            pill(
                if skill.load == LoadMode::Always {
                    "ALWAYS"
                } else {
                    "ON DEMAND"
                },
                if skill.load == LoadMode::Always {
                    p.accent
                } else {
                    p.purple
                },
                p
            ),
            text(skill.name.clone())
                .font(SANS_SEMIBOLD)
                .size(12)
                .color(p.ink),
            text(format!(
                "{}/SKILL.md",
                skill.source_prefix.trim_end_matches('/')
            ))
            .font(MONO)
            .size(10.5)
            .color(p.muted_2)
            .width(Length::Fill)
        ]
        .spacing(9)
        .align_y(Alignment::Center),
    )
    .padding([8, 10])
    .style(move |_| rounded_surface(p.paper, p.border, RADIUS_SM))
    .into()
}

fn missing_agent(id: &str, p: Colors) -> Element<'static, Message> {
    card(
        column![
            icon_tile(Icon::Agent, 46.0, p),
            text("Agent not found").font(SANS_SEMIBOLD).size(16).color(p.ink),
            text(format!("{id} isn’t in this workspace’s roster — it may have been removed since it was mentioned."))
                .font(SANS)
                .size(12)
                .color(p.muted_2),
            primary_button("Back to the roster", Some(Message::ClearExplicitSelection), p)
        ]
        .spacing(10)
        .padding([40, 24])
        .align_x(Alignment::Center),
        p,
    )
}

fn no_agents(p: Colors) -> Element<'static, Message> {
    card(
        column![
            icon_tile(Icon::Agent, 46.0, p),
            text("No agents yet")
                .font(SANS_SEMIBOLD)
                .size(16)
                .color(p.ink),
            text("Add your first agent to start automating chats and tasks.")
                .font(SANS)
                .size(12)
                .color(p.muted_2),
            primary_button("+ Add agent", Some(Message::StartAdding), p)
        ]
        .spacing(10)
        .padding([40, 24])
        .align_x(Alignment::Center),
        p,
    )
}

fn auto_reply_tab<'a>(state: &'a State, data: &'a AgentData, p: Colors) -> Element<'a, Message> {
    let mut rows = column![
        section_label("AUTO-REPLY", p),
        text("Choose which channels agents watch and when they answer.")
            .font(SANS)
            .size(11.5)
            .color(p.muted_2)
    ]
    .spacing(7)
    .padding(22);
    let mut watches = column![];
    if data.watches.is_empty() {
        watches = watches.push(empty_state(
            "No watched channels",
            "Add one below to let agents reply automatically.",
            p,
        ));
    } else {
        for watch in &data.watches {
            watches = watches.push(watch_row(watch, data, p));
        }
    }
    rows = rows.push(card(watches, p));
    rows = rows.push(watch_form(state, data, p));
    if let Some(error) = &state.error {
        rows = rows.push(error_banner(error, p));
    }
    scrollable(rows)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn watch_row(watch: &Watch, data: &AgentData, p: Colors) -> Element<'static, Message> {
    let label = data
        .channels
        .iter()
        .find(|channel| channel.id == watch.channel_id)
        .map_or(watch.channel_id.clone(), |channel| channel.name.clone());
    let policy = policy_text(&watch.policy, data);
    container(
        row![
            container(icons::view(Icon::Chat, 15.0, p.muted_2))
                .width(31)
                .height(31)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(move |_| rounded_surface(p.sunken, p.border, RADIUS_SM)),
            column![
                text(format!("# {label}")).font(MONO).size(12).color(p.ink),
                text(policy).font(SANS).size(11.5).color(p.muted_2)
            ]
            .spacing(2)
            .width(Length::Fill),
            secondary_button(
                "Turn off",
                (!watch.pending).then_some(Message::RemoveWatch(watch.channel_id.clone())),
                p,
            )
        ]
        .spacing(11)
        .align_y(Alignment::Center),
    )
    .padding([12, 14])
    .style(move |_| bottom_rule(p.paper, p.border_soft))
    .into()
}

fn watch_form<'a>(state: &'a State, data: &'a AgentData, p: Colors) -> Element<'a, Message> {
    let policy_ready =
        state.watch.policy != WatchPolicyKind::Assigned || !state.watch.assigned_agent.is_empty();
    let mut form = column![
        section_label("ADD A CHANNEL", p),
        labeled_input(
            "CHANNEL",
            data.channels
                .first()
                .map_or("general", |channel| channel.name.as_str()),
            &state.watch.channel_id,
            Message::WatchChannelChanged,
            p,
        ),
        row![
            segment_button(
                "When mentioned",
                WatchPolicyKind::Mention,
                state.watch.policy,
                p
            ),
            segment_button("Every message", WatchPolicyKind::All, state.watch.policy, p),
            segment_button(
                "Take turns",
                WatchPolicyKind::RoundRobin,
                state.watch.policy,
                p
            ),
            segment_button(
                "Only a chosen agent",
                WatchPolicyKind::Assigned,
                state.watch.policy,
                p
            )
        ]
        .spacing(6)
    ]
    .spacing(9)
    .padding(16);
    if state.watch.policy == WatchPolicyKind::Assigned {
        form = form.push(labeled_input(
            "AGENT",
            data.agents
                .first()
                .map_or("agent-id", |agent| agent.id.as_str()),
            &state.watch.assigned_agent,
            Message::WatchAssignedChanged,
            p,
        ));
    }
    form = form.push(row![
        Space::new().width(Length::Fill),
        primary_button(
            if state.busy {
                "Adding..."
            } else {
                "Add auto-reply"
            },
            (!state.busy && !state.watch.channel_id.is_empty() && policy_ready)
                .then_some(Message::AddWatch),
            p,
        )
    ]);
    card(form, p)
}

fn activity_tab<'a>(state: &'a State, data: &'a AgentData, p: Colors) -> Element<'a, Message> {
    let mut body = column![
        job_worker(data, p),
        usage_card(data.usage.as_ref(), p),
        row![
            filter_button("All", RunFilter::All, state.run_filter, p),
            filter_button("Requested by you", RunFilter::Mine, state.run_filter, p)
        ]
        .spacing(6)
    ]
    .spacing(12)
    .padding(22);
    let runs: Vec<&PendingRun> = data
        .pending_runs
        .iter()
        .filter(|run| state.run_filter == RunFilter::All || run.requested_by_me)
        .collect();
    if runs.is_empty() {
        body = body.push(card(
            empty_state("No active runs", "In-flight agent work appears here.", p),
            p,
        ));
    } else {
        body = body.push(section_label("IN PROGRESS", p));
        for run in runs {
            body = body.push(run_row(run, data, state, p));
        }
    }
    body = body.push(
        row![
            section_label("HISTORY", p),
            text(data.recent_runs.len())
                .font(MONO)
                .size(10)
                .color(p.muted_2)
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    );
    if let Some(error) = &data.recent_runs_error {
        body = body.push(error_banner(
            &format!("Run history unavailable: {error}"),
            p,
        ));
    }
    if data.recent_runs.is_empty() && data.recent_runs_error.is_none() {
        body = body.push(card(
            empty_state(
                "No delivered runs yet",
                "Finished runs land here; the node keeps the most recent 100.",
                p,
            ),
            p,
        ));
    } else {
        for run in &data.recent_runs {
            body = body.push(history_row(run, data, state, p));
        }
    }
    if let Some(error) = &state.error {
        body = body.push(error_banner(error, p));
    }
    scrollable(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn job_worker(data: &AgentData, p: Colors) -> Element<'static, Message> {
    card(
        row![
            icon_tile(Icon::Agent, 34.0, p),
            column![
                text("Jobs worker")
                    .font(SANS_SEMIBOLD)
                    .size(13.5)
                    .color(p.ink),
                text("Let active agents claim work from the jobs board.")
                    .font(SANS)
                    .size(11.5)
                    .color(p.muted_2),
                text("Current committed status is not readable on this network.")
                    .font(SANS)
                    .size(10.5)
                    .color(p.muted_2)
            ]
            .spacing(3)
            .width(Length::Fill),
            secondary_button(
                "Enable",
                (!data.job_worker_pending).then_some(Message::SetJobWorker(true)),
                p,
            ),
            secondary_button(
                "Disable",
                (!data.job_worker_pending).then_some(Message::SetJobWorker(false)),
                p,
            )
        ]
        .spacing(11)
        .padding([12, 14])
        .align_y(Alignment::Center),
        p,
    )
}

fn usage_card(usage: Option<&Usage>, p: Colors) -> Element<'static, Message> {
    let Some(usage) = usage else {
        return card(
            row![
                text("Usage").font(SANS_SEMIBOLD).size(13).color(p.ink),
                Space::new().width(Length::Fill),
                text("No usage yet").font(SANS).size(11.5).color(p.muted_2)
            ]
            .padding([12, 14]),
            p,
        );
    };
    card(
        row![
            stat("REQUESTS", usage.requests.to_string(), p),
            stat("INPUT TOKENS", usage.input_tokens.to_string(), p),
            stat("OUTPUT TOKENS", usage.output_tokens.to_string(), p),
            stat("FAILED", usage.failed.to_string(), p),
            stat("BLOCKS", usage.duration_blocks.to_string(), p)
        ]
        .spacing(12)
        .padding(14),
        p,
    )
}

fn run_row(
    run: &PendingRun,
    data: &AgentData,
    state: &State,
    p: Colors,
) -> Element<'static, Message> {
    let agent = data
        .agents
        .iter()
        .find(|agent| agent.id == run.agent_id)
        .map_or(run.agent_id.clone(), |agent| agent.display_name.clone());
    let channel = data
        .channels
        .iter()
        .find(|channel| channel.id == run.channel_id)
        .map_or(run.channel_id.clone(), |channel| channel.name.clone());
    let expanded = state.expanded_run_logs.contains(&run.dispatch_id);
    let mut content = column![
        row![
            avatar(&agent, 34.0, p.filled, p.on_filled, p),
            column![
                row![
                    text(agent).font(SANS_SEMIBOLD).size(13).color(p.ink),
                    pill("RUNNING", p.blue, p)
                ]
                .spacing(8),
                text(if let Some(job) = &run.job_id {
                    format!("job {job} · dispatch {}", short(&run.dispatch_id))
                } else {
                    format!(
                        "#{channel} · message {} · dispatch {}",
                        run.anchor_sequence,
                        short(&run.dispatch_id)
                    )
                })
                .font(MONO)
                .size(10.5)
                .color(p.muted_2),
                text(run.created_at.clone())
                    .font(SANS)
                    .size(10.5)
                    .color(p.muted_2)
            ]
            .spacing(4)
            .width(Length::Fill),
            secondary_button(
                if expanded { "Hide log" } else { "Live log" },
                Some(Message::ToggleRunLog(run.dispatch_id.clone())),
                p,
            ),
            secondary_button(
                "Reassign",
                (!run.pending).then_some(Message::ReassignRun(run.run_id.clone(), run.attempt + 1)),
                p,
            ),
            secondary_button(
                "Cancel",
                (!run.pending).then_some(Message::CancelRun(run.run_id.clone())),
                p,
            )
        ]
        .spacing(11)
        .padding([12, 14])
        .align_y(Alignment::Center)
    ];
    if expanded {
        content = content.push(run_log_pane(&run.dispatch_id, false, state, p));
    }
    card(content, p)
}

fn history_row(
    run: &RunRecord,
    data: &AgentData,
    state: &State,
    p: Colors,
) -> Element<'static, Message> {
    let agent = data
        .agents
        .iter()
        .find(|agent| agent.id == run.agent_id)
        .map_or(run.agent_id.clone(), |agent| agent.display_name.clone());
    let channel = data
        .channels
        .iter()
        .find(|channel| channel.id == run.channel_id)
        .map_or(run.channel_id.clone(), |channel| channel.name.clone());
    let target = if run.channel_id.is_empty() {
        "job".into()
    } else {
        format!("#{channel} · message {}", run.anchor_sequence)
    };
    let expanded = state.expanded_run_logs.contains(&run.dispatch_id);
    let tone = if run.outcome == RunOutcome::Delivered {
        p.green
    } else {
        p.red
    };
    let mut metadata = row![
        pill(
            if run.outcome == RunOutcome::Delivered {
                "DELIVERED"
            } else {
                "FAILED"
            },
            tone,
            p,
        ),
        pill(&run_duration(run), p.muted_3, p),
    ]
    .spacing(7)
    .align_y(Alignment::Center);
    if !run.channel_id.is_empty() && run.anchor_sequence > 0 {
        metadata = metadata.push(run_link_button(
            target,
            Message::OpenRunAnchor {
                channel_id: run.channel_id.clone(),
                sequence: run.anchor_sequence,
            },
            p.blue,
            p,
        ));
    } else {
        metadata = metadata.push(text(target).font(MONO).size(10.5).color(p.muted_2));
    }
    if run.degraded {
        metadata = metadata.push(pill("DEGRADED", p.amber, p));
    }
    if run.executing_node != "unknown" {
        metadata = metadata.push(pill(
            &format!("on {}", short(&run.executing_node)),
            p.purple,
            p,
        ));
    }
    if let Some(number) = run.pr_number {
        metadata = if forge_item_channel(&run.channel_id).is_some() {
            metadata.push(run_link_button(
                format!("PR #{number}"),
                Message::OpenRunPullRequest {
                    channel_id: run.channel_id.clone(),
                    number,
                },
                p.green,
                p,
            ))
        } else {
            metadata.push(pill(&format!("PR #{number}"), p.green, p))
        };
    }
    metadata = metadata
        .push(Space::new().width(Length::Fill))
        .push(secondary_button(
            if expanded { "Hide log" } else { "Log" },
            Some(Message::ToggleRunLog(run.dispatch_id.clone())),
            p,
        ));
    if let Some(reference) = &run.output_ref {
        metadata = metadata.push(text(short(reference)).font(MONO).size(9.5).color(p.muted_2));
    }
    let mut content = column![
        row![
            avatar(&agent, 30.0, p.filled, p.on_filled, p),
            text(agent).font(SANS_SEMIBOLD).size(12.5).color(p.ink)
        ]
        .spacing(9)
        .align_y(Alignment::Center),
        metadata,
    ]
    .spacing(8)
    .padding([10, 12]);
    if expanded {
        content = content.push(run_log_pane(&run.dispatch_id, true, state, p));
    }
    card(content, p)
}

fn run_log_pane(
    dispatch_id: &str,
    terminal: bool,
    state: &State,
    p: Colors,
) -> Element<'static, Message> {
    let log = state.run_logs.get(dispatch_id);
    let mut output = column![];
    if let Some(dropped) = log.map(|log| log.dropped).filter(|dropped| *dropped > 0) {
        output = output.push(
            text(format!("live log tail: {dropped} older events omitted"))
                .font(MONO)
                .size(10)
                .color(p.amber),
        );
    }
    if let Some(log) = log {
        for row in semantic_log_rows(&log.entries) {
            if row.kind == SemanticLogKind::Blank {
                output = output.push(Space::new().height(4));
                continue;
            }
            let label = match row.kind {
                SemanticLogKind::Message => "message",
                SemanticLogKind::Command => "command",
                SemanticLogKind::Output => "output",
                SemanticLogKind::Status => "status",
                SemanticLogKind::Exit => "exit",
                SemanticLogKind::File => "files",
                SemanticLogKind::Tool => "tool",
                SemanticLogKind::Text => match row.stream {
                    Some(RunStream::Stderr) => "stderr",
                    _ => "stdout",
                },
                SemanticLogKind::Gap => "gap",
                SemanticLogKind::Blank => "",
            };
            let color = match row.kind {
                SemanticLogKind::Command | SemanticLogKind::Tool => p.blue,
                SemanticLogKind::Message | SemanticLogKind::File => p.ink,
                SemanticLogKind::Status => p.muted_2,
                SemanticLogKind::Exit if row.text == "exit: 0" => p.green,
                SemanticLogKind::Gap => p.amber,
                _ if row.stream == Some(RunStream::Stderr) => p.red,
                _ => p.ink_soft,
            };
            output = output.push(
                row![
                    text(label).font(MONO).size(9).color(p.muted_2).width(52),
                    text(row.text).font(MONO).size(10.5).color(color),
                ]
                .spacing(8),
            );
        }
    }
    if log.is_none_or(|log| log.entries.is_empty()) {
        let unavailable = log.is_some_and(|log| log.unavailable);
        output = output.push(
            text(if unavailable {
                "Run output unavailable."
            } else if terminal {
                "No retained output received; older output may have been evicted."
            } else {
                "Waiting for retained output..."
            })
            .font(SANS)
            .size(11.5)
            .color(p.muted_2),
        );
    }
    container(scrollable(output.spacing(3)))
        .height(Length::Fixed(180.0))
        .padding([8, 10])
        .style(move |_| rounded_surface(p.canvas, p.border_soft, RADIUS_SM))
        .into()
}

fn run_duration(run: &RunRecord) -> String {
    const WALL_CLOCK_SECONDS_FLOOR: u64 = 978_307_200;
    const WALL_CLOCK_MILLIS_FLOOR: u64 = 978_307_200_000;
    let wall_seconds = |stamp: u64| {
        if stamp > WALL_CLOCK_MILLIS_FLOOR {
            Some(stamp / 1_000)
        } else if stamp > WALL_CLOCK_SECONDS_FLOOR {
            Some(stamp)
        } else {
            None
        }
    };
    if let (Some(start), Some(end)) = (wall_seconds(run.created_at), wall_seconds(run.delivered_at))
    {
        let seconds = end.saturating_sub(start);
        return match seconds {
            0 => "<1s".into(),
            1..=59 => format!("{seconds}s"),
            60..=3_599 => format!("{}m {}s", seconds / 60, seconds % 60),
            _ => format!("{}h {}m", seconds / 3_600, seconds % 3_600 / 60),
        };
    }
    let blocks = run.delivered_at.saturating_sub(run.created_at);
    if blocks == 1 {
        "1 block".into()
    } else {
        format!("{blocks} blocks")
    }
}

fn selected_agent(state: &State) -> Option<&AgentRecord> {
    let Resource::Ready(data) = &state.data else {
        return None;
    };
    match &state.selected_agent_id {
        Some(id) => data.agents.iter().find(|agent| &agent.id == id),
        None => data.agents.first(),
    }
}

fn header_tab(
    label: &'static str,
    count: usize,
    tab: Tab,
    active: Tab,
    p: Colors,
) -> Button<'static, Message> {
    button(
        row![
            text(label).font(SANS_SEMIBOLD).size(12),
            container(text(count).font(MONO).size(10).color(if tab == active {
                p.accent
            } else {
                p.muted_2
            }))
            .padding([0, 5])
            .style(move |_| rounded_surface(
                if tab == active {
                    mix(p.paper, p.accent, 0.09)
                } else {
                    p.sidebar
                },
                Color::TRANSPARENT,
                999.0
            ))
        ]
        .spacing(7)
        .align_y(Alignment::Center),
    )
    .padding([6, 12])
    .style(move |_, _| button::Style {
        background: (tab == active).then_some(Background::Color(p.paper)),
        text_color: if tab == active { p.accent } else { p.muted_2 },
        border: Border {
            radius: RADIUS_SM.into(),
            ..Default::default()
        },
        shadow: if tab == active {
            Shadow {
                color: Color::from_rgba8(0, 0, 0, 0.07),
                offset: Vector::new(0.0, 1.0),
                blur_radius: 3.0,
            }
        } else {
            Shadow::default()
        },
        ..Default::default()
    })
    .on_press(Message::SelectTab(tab))
}

fn segment_button(
    label: &'static str,
    policy: WatchPolicyKind,
    active: WatchPolicyKind,
    p: Colors,
) -> Button<'static, Message> {
    button(text(label).font(SANS_SEMIBOLD).size(10.5))
        .padding([6, 9])
        .style(move |_, _| button::Style {
            background: Some(Background::Color(if policy == active {
                mix(p.paper, p.accent, 0.09)
            } else {
                p.paper
            })),
            text_color: if policy == active {
                p.accent
            } else {
                p.muted_3
            },
            border: Border {
                color: if policy == active {
                    mix(p.paper, p.accent, 0.25)
                } else {
                    p.border
                },
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            ..Default::default()
        })
        .on_press(Message::WatchPolicyChanged(policy))
}

fn filter_button(
    label: &'static str,
    filter: RunFilter,
    active: RunFilter,
    p: Colors,
) -> Button<'static, Message> {
    button(text(label).font(SANS_SEMIBOLD).size(11))
        .padding([5, 10])
        .style(move |_, _| button::Style {
            background: Some(Background::Color(if filter == active {
                p.filled
            } else {
                p.paper
            })),
            text_color: if filter == active {
                p.on_filled
            } else {
                p.muted_2
            },
            border: Border {
                color: p.border_strong,
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            ..Default::default()
        })
        .on_press(Message::SelectRunFilter(filter))
}

fn capability_strip(capability: &str, p: Colors) -> Element<'static, Message> {
    let parts: Vec<&str> = capability.split('_').collect();
    let provider = title_case(parts.first().copied().unwrap_or(capability));
    let mut strip = row![
        text(provider)
            .font(SANS_SEMIBOLD)
            .size(12.5)
            .color(p.accent)
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    if let Some(model) = parts.get(1) {
        strip = strip.push(text("›").font(MONO).size(12).color(p.icon_idle));
        strip = strip.push(text((*model).to_owned()).font(MONO).size(11.5).color(p.ink));
    }
    if let Some(effort) = parts.get(2) {
        strip = strip.push(pill(&effort.to_uppercase(), p.accent, p));
    }
    strip.into()
}

fn capability_short(capability: &str) -> String {
    let mut parts = capability.split('_');
    let provider = title_case(parts.next().unwrap_or(capability));
    parts
        .next()
        .map_or(provider.clone(), |model| format!("{provider} · {model}"))
}

fn policy_text(policy: &TurnPolicy, data: &AgentData) -> String {
    match policy {
        TurnPolicy::Mention => "When mentioned".into(),
        TurnPolicy::All => "Every message".into(),
        TurnPolicy::RoundRobin => "Take turns".into(),
        TurnPolicy::Assigned(id) => format!(
            "Only {}",
            data.agents
                .iter()
                .find(|agent| &agent.id == id)
                .map_or(id.as_str(), |agent| agent.display_name.as_str())
        ),
    }
}

fn labeled_input<'a>(
    label: &'static str,
    placeholder: &'a str,
    value: &'a str,
    on_input: fn(String) -> Message,
    p: Colors,
) -> Element<'a, Message> {
    column![
        section_label(label, p),
        text_input(placeholder, value)
            .font(SANS)
            .size(12.5)
            .on_input(on_input)
    ]
    .spacing(5)
    .width(Length::Fill)
    .into()
}

fn labeled_mono_input<'a>(
    label: &'static str,
    placeholder: &'a str,
    value: &'a str,
    on_input: fn(String) -> Message,
    p: Colors,
) -> Element<'a, Message> {
    column![
        section_label(label, p),
        text_input(placeholder, value)
            .font(MONO)
            .size(12.5)
            .on_input(on_input)
    ]
    .spacing(5)
    .width(Length::Fill)
    .into()
}

fn info_row(label: &'static str, value: &str, p: Colors) -> Element<'static, Message> {
    container(
        row![
            text(label).font(MONO).size(11).color(p.muted_2),
            Space::new().width(Length::Fill),
            text(value.to_owned()).font(MONO).size(11).color(p.muted_3)
        ]
        .spacing(14),
    )
    .padding([9, 11])
    .style(move |_| rounded_surface(p.paper, p.border, RADIUS_SM))
    .into()
}

fn stat(label: &'static str, value: String, p: Colors) -> Element<'static, Message> {
    column![
        section_label(label, p),
        text(value).font(MONO).size(16).color(p.ink)
    ]
    .spacing(5)
    .width(Length::Fill)
    .into()
}

fn avatar(
    name: &str,
    size: f32,
    background: Color,
    foreground: Color,
    p: Colors,
) -> Element<'static, Message> {
    container(
        text(initials(name))
            .font(MONO)
            .size((size * 0.31).max(10.0))
            .color(foreground),
    )
    .width(size)
    .height(size)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .style(move |_| {
        rounded_surface(
            background,
            mix(background, p.paper, 0.16),
            (size * 0.24).max(7.0),
        )
    })
    .into()
}

fn icon_tile(icon: Icon, size: f32, p: Colors) -> Element<'static, Message> {
    container(icons::view(icon, size * 0.53, p.on_filled))
        .width(size)
        .height(size)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(move |_| rounded_surface(p.filled, Color::TRANSPARENT, RADIUS_SM))
        .into()
}

fn status_dot(color: Color) -> Element<'static, Message> {
    container(Space::new())
        .width(6)
        .height(6)
        .style(move |_| rounded_surface(color, Color::TRANSPARENT, 99.0))
        .into()
}

fn section_label(label: &str, p: Colors) -> Element<'static, Message> {
    text(label.to_owned())
        .font(MONO)
        .size(9)
        .color(p.muted_2)
        .into()
}

fn pill(label: &str, tone: Color, p: Colors) -> Element<'static, Message> {
    container(text(label.to_owned()).font(MONO).size(9).color(tone))
        .padding([3, 7])
        .style(move |_| rounded_surface(mix(p.paper, tone, 0.09), mix(p.paper, tone, 0.25), 5.0))
        .into()
}

fn on_dark_pill(label: &str, tone: Color, p: Colors) -> Element<'static, Message> {
    container(
        text(label.to_owned())
            .font(MONO)
            .size(9)
            .color(mix(p.on_filled, tone, 0.35)),
    )
    .padding([3, 7])
    .style(move |_| {
        rounded_surface(
            mix(p.filled, p.on_filled, 0.08),
            mix(p.filled, p.on_filled, 0.16),
            999.0,
        )
    })
    .into()
}

fn card<'a>(content: impl Into<Element<'a, Message>>, p: Colors) -> Element<'a, Message> {
    container(content)
        .width(Length::Fill)
        .style(move |_| card_style(p))
        .into()
}

fn card_style(p: Colors) -> container::Style {
    container::Style {
        background: Some(Background::Color(p.paper)),
        border: Border {
            color: p.border,
            width: 1.0,
            radius: RADIUS_LG.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba8(0, 0, 0, 0.06),
            offset: Vector::new(0.0, 1.0),
            blur_radius: 3.0,
        },
        ..Default::default()
    }
}

fn empty_state(title: &str, detail: &str, p: Colors) -> Element<'static, Message> {
    column![
        icon_tile(Icon::Agent, 36.0, p),
        text(title.to_owned())
            .font(SANS_SEMIBOLD)
            .size(14)
            .color(p.muted_3),
        text(detail.to_owned())
            .font(SANS)
            .size(11.5)
            .color(p.muted_2)
    ]
    .spacing(8)
    .padding([30, 18])
    .align_x(Alignment::Center)
    .into()
}

fn center_state<'a>(title: &str, detail: &str, p: Colors) -> Element<'a, Message> {
    let mut content = column![
        icon_tile(Icon::Agent, 46.0, p),
        text(title.to_owned())
            .font(SANS_SEMIBOLD)
            .size(16)
            .color(p.ink)
    ]
    .spacing(10)
    .align_x(Alignment::Center);
    if !detail.is_empty() {
        content = content.push(text(detail.to_owned()).font(SANS).size(12).color(p.muted_2));
    }
    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
}

fn error_banner(error: &str, p: Colors) -> Element<'static, Message> {
    container(text(error.to_owned()).font(SANS).size(11).color(p.red))
        .width(Length::Fill)
        .padding([7, 9])
        .style(move |_| {
            rounded_surface(
                mix(p.paper, p.red, 0.09),
                mix(p.paper, p.red, 0.25),
                RADIUS_SM,
            )
        })
        .into()
}

fn secondary_button(
    label: &'static str,
    message: Option<Message>,
    p: Colors,
) -> Button<'static, Message> {
    let enabled = message.is_some();
    button(text(label).font(SANS_SEMIBOLD).size(12))
        .padding([7, 12])
        .style(move |_, status| button::Style {
            background: Some(Background::Color(
                if matches!(status, button::Status::Hovered) && enabled {
                    p.sunken
                } else {
                    p.paper
                },
            )),
            text_color: if enabled { p.ink_soft } else { p.muted_2 },
            border: Border {
                color: p.border_strong,
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            ..Default::default()
        })
        .on_press_maybe(message)
}

fn run_link_button(
    label: String,
    message: Message,
    tone: Color,
    p: Colors,
) -> Button<'static, Message> {
    button(text(label).font(MONO).size(10.5))
        .padding([2, 7])
        .style(move |_, status| button::Style {
            background: Some(Background::Color(
                if matches!(status, button::Status::Hovered) {
                    p.sunken
                } else {
                    p.paper
                },
            )),
            text_color: tone,
            border: Border {
                color: p.border_strong,
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            ..Default::default()
        })
        .on_press(message)
}

pub fn forge_item_channel(channel_id: &str) -> Option<(&str, u64)> {
    let rest = channel_id.strip_prefix("forge:")?;
    let (repository, number) = rest.rsplit_once(':')?;
    let number = number.parse::<u64>().ok()?;
    (!repository.is_empty() && number > 0).then_some((repository, number))
}

fn primary_button(
    label: &'static str,
    message: Option<Message>,
    p: Colors,
) -> Button<'static, Message> {
    let enabled = message.is_some();
    button(text(label).font(SANS_SEMIBOLD).size(12))
        .padding([7, 12])
        .style(move |_, _| button::Style {
            background: Some(Background::Color(if enabled { p.accent } else { p.chip })),
            text_color: if enabled { Color::WHITE } else { p.muted_2 },
            border: Border {
                color: if enabled { p.accent } else { p.chip },
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            shadow: if enabled {
                Shadow {
                    color: Color::from_rgba8(160, 90, 60, 0.3),
                    offset: Vector::new(0.0, 1.0),
                    blur_radius: 2.0,
                }
            } else {
                Shadow::default()
            },
            ..Default::default()
        })
        .on_press_maybe(message)
}

fn on_dark_button(
    label: &'static str,
    message: Option<Message>,
    p: Colors,
) -> Button<'static, Message> {
    let enabled = message.is_some();
    button(text(label).font(SANS_SEMIBOLD).size(12))
        .padding([7, 12])
        .style(move |_, _| button::Style {
            background: Some(Background::Color(mix(p.filled, p.on_filled, 0.07))),
            text_color: if enabled {
                p.on_filled
            } else {
                mix(p.filled, p.on_filled, 0.45)
            },
            border: Border {
                color: mix(p.filled, p.on_filled, 0.22),
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            ..Default::default()
        })
        .on_press_maybe(message)
}

fn surface(color: Color) -> container::Style {
    container::Style {
        background: Some(Background::Color(color)),
        ..Default::default()
    }
}

fn rounded_surface(background: Color, border: Color, radius: f32) -> container::Style {
    container::Style {
        background: Some(Background::Color(background)),
        border: Border {
            color: border,
            width: (border != Color::TRANSPARENT) as u8 as f32,
            radius: radius.into(),
        },
        ..Default::default()
    }
}

fn bottom_rule(background: Color, border: Color) -> container::Style {
    container::Style {
        background: Some(Background::Color(background)),
        border: Border {
            color: border,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

fn horizontal_divider(p: Colors) -> Element<'static, Message> {
    container(Space::new())
        .height(1)
        .style(move |_| surface(p.border_soft))
        .into()
}

fn initials(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    match words.as_slice() {
        [] => "AI".into(),
        [one] => one.chars().take(2).collect::<String>().to_uppercase(),
        many => format!(
            "{}{}",
            many[0].chars().next().unwrap_or('A'),
            many.last()
                .and_then(|word| word.chars().next())
                .unwrap_or('I')
        )
        .to_uppercase(),
    }
}

fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    chars.next().map_or_else(String::new, |head| {
        format!("{}{}", head.to_uppercase(), chars.as_str())
    })
}

fn slug(value: &str) -> String {
    let mut result = String::new();
    let mut separator = false;
    for ch in value.trim().to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            if separator && !result.is_empty() {
                result.push('-');
            }
            result.push(ch);
            separator = false;
        } else {
            separator = true;
        }
        if result.len() == 63 {
            break;
        }
    }
    result.trim_end_matches('-').into()
}

fn canonical_words(value: &str) -> Vec<String> {
    let mut words = value
        .split(|ch: char| ch.is_whitespace() || ch == ',')
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    words.sort();
    words.dedup();
    words
}

fn positive_u32(value: &str) -> Option<u32> {
    value.trim().parse().ok().filter(|budget| *budget > 0)
}

fn can_read_library(caps: &ResourceCaps) -> bool {
    caps.duckfs_read
        .iter()
        .any(|prefix| prefix == LIBRARY_ROOT || LIBRARY_ROOT.starts_with(&format!("{prefix}/")))
}

fn short(value: &str) -> String {
    if value.chars().count() <= 18 {
        value.into()
    } else {
        format!(
            "{}…{}",
            value.chars().take(10).collect::<String>(),
            value
                .chars()
                .rev()
                .take(6)
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>()
        )
    }
}

fn mix(base: Color, tint: Color, amount: f32) -> Color {
    Color {
        r: base.r + (tint.r - base.r) * amount,
        g: base.g + (tint.g - base.g) * amount,
        b: base.b + (tint.b - base.b) * amount,
        a: 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> AgentData {
        AgentData {
            agents: vec![],
            capabilities: vec!["codex_gpt-5_medium".into()],
            capability_status: CapabilityStatus::Ready,
            channels: vec![Channel {
                id: "general".into(),
                name: "general".into(),
            }],
            watches: vec![],
            pending_runs: vec![],
            recent_runs: vec![],
            recent_runs_error: None,
            usage: None,
            job_worker_pending: false,
        }
    }

    #[test]
    fn slug_is_a_bounded_dns_label() {
        assert_eq!(slug("  Triage Agent!  "), "triage-agent");
        assert!(slug(&"a".repeat(80)).len() <= 63);
        assert_eq!(slug("!!!"), "");
    }

    #[test]
    fn library_read_follows_prefix_containment_not_text_prefixes() {
        let caps = |prefix: &str| ResourceCaps {
            duckfs_read: vec![prefix.into()],
            ..ResourceCaps::default()
        };
        assert!(can_read_library(&caps(LIBRARY_ROOT)));
        assert!(can_read_library(&caps("/shared")));
        assert!(!can_read_library(&caps("/shared/skill")));
        assert!(!can_read_library(&caps("/shared/skills-old")));
    }

    #[test]
    fn native_view_constructs_for_loading_and_populated_states() {
        let state = State::default();
        let _ = view(&state, theme::Mode::Light, theme::ACCENTS[0]);
        let state = State {
            data: Resource::Ready(data()),
            ..State::default()
        };
        let _ = view(&state, theme::Mode::Dark, theme::ACCENTS[1]);
    }

    #[test]
    fn registration_uses_derived_id_and_resource_caps() {
        let mut state = State {
            data: Resource::Ready(data()),
            ..State::default()
        };
        state.register.display_name = "Triage Agent".into();
        state.register.capability = "codex_gpt-5_medium".into();
        state.register.forge_read = "beta, alpha beta".into();
        state.register.forge_push = "beta".into();
        state.register.duckfs_read = "/shared/skills /shared/data /shared/data".into();
        state.register.duckfs_write = "/shared/output".into();
        state.register.tools = "browser.search".into();
        state.register.secrets = "vault/github".into();
        state.register.pages_write = "roadmap, notes roadmap".into();
        state.register.subagent_budget = "3".into();
        let command = update(&mut state, Message::Register);
        let Some(Command::RegisterAgent { agent_id, caps, .. }) = command else {
            panic!("registration should emit a command");
        };
        assert_eq!(agent_id, "triage-agent");
        assert_eq!(
            caps,
            ResourceCaps {
                forge_read: vec!["alpha".into(), "beta".into()],
                forge_push: vec!["beta".into()],
                duckfs_read: vec!["/shared/data".into(), LIBRARY_ROOT.into()],
                duckfs_write: vec!["/shared/output".into()],
                tools: vec!["browser.search".into()],
                secrets: vec!["vault/github".into()],
                pages_write: vec!["notes".into(), "roadmap".into()],
                subagent_budget: Some(3),
            }
        );
    }

    #[test]
    fn peer_call_budget_input_stays_within_u32() {
        let mut state = State::default();
        update(
            &mut state,
            Message::RegisterBudgetChanged("4294967296".into()),
        );
        assert_eq!(state.register.subagent_budget, "0");
        update(&mut state, Message::RegisterBudgetChanged("8".into()));
        assert_eq!(state.register.subagent_budget, "8");
    }

    #[test]
    fn edit_exposes_and_preserves_every_resource_cap() {
        let agent = AgentRecord {
            id: "triage".into(),
            owner: Owner::System,
            display_name: "Triage".into(),
            capability: "codex_gpt-5_medium".into(),
            allowed_actions: vec!["chat.post".into()],
            status: AgentStatus::Active,
            created_at: "now".into(),
            updated_at: "now".into(),
            caps: ResourceCaps {
                forge_read: vec!["core".into()],
                forge_push: vec!["core".into()],
                duckfs_read: vec![LIBRARY_ROOT.into()],
                duckfs_write: vec!["/shared/output".into()],
                tools: vec!["search".into()],
                secrets: vec!["vault/github".into()],
                pages_write: vec!["roadmap".into()],
                subagent_budget: Some(2),
            },
            skills: vec![],
            pending: false,
        };
        let mut state = State::default();
        let mut loaded = data();
        loaded.agents.push(agent);
        state.data = Resource::Ready(loaded);
        update(&mut state, Message::StartEditing);
        let command = update(&mut state, Message::SaveEdit);
        let Some(Command::UpdateAgent { caps, .. }) = command else {
            panic!("edit should emit a command");
        };
        assert_eq!(
            caps,
            ResourceCaps {
                forge_read: vec!["core".into()],
                forge_push: vec!["core".into()],
                duckfs_read: vec![LIBRARY_ROOT.into()],
                duckfs_write: vec!["/shared/output".into()],
                tools: vec!["search".into()],
                secrets: vec!["vault/github".into()],
                pages_write: vec!["roadmap".into()],
                subagent_budget: Some(2),
            }
        );
    }

    #[test]
    fn resource_detail_labels_include_every_grant_and_zero_peer_budget() {
        let caps = ResourceCaps {
            forge_read: vec!["core".into()],
            forge_push: vec!["release".into()],
            duckfs_read: vec!["/shared/data".into()],
            duckfs_write: vec!["/shared/output".into()],
            tools: vec!["browser.search".into()],
            secrets: vec!["vault/github".into()],
            pages_write: vec!["roadmap".into()],
            subagent_budget: None,
        };
        assert_eq!(
            resource_grant_labels(&caps),
            [
                "Forge read: core",
                "Forge push: release",
                "DuckFS read: /shared/data",
                "DuckFS write: /shared/output",
                "Tool: browser.search",
                "Secret: vault/github",
                "Page write: roadmap",
                "Concurrent peer calls: 0",
            ]
        );
    }

    #[test]
    fn explicit_missing_selection_does_not_fall_back_to_first_agent() {
        let mut state = State::default();
        let mut loaded = data();
        loaded.agents.push(AgentRecord {
            id: "real".into(),
            owner: Owner::System,
            display_name: "Real".into(),
            capability: "codex_gpt-5_medium".into(),
            allowed_actions: vec!["chat.post".into()],
            status: AgentStatus::Active,
            created_at: "now".into(),
            updated_at: "now".into(),
            caps: ResourceCaps::default(),
            skills: vec![],
            pending: false,
        });
        state.data = Resource::Ready(loaded);
        update(&mut state, Message::SelectFocusedAgent("gone".into()));
        assert!(selected_agent(&state).is_none());
        assert!(state.explicit_selection);
    }

    #[test]
    fn assigned_watch_requires_an_agent() {
        let mut state = State::default();
        state.data = Resource::Ready(data());
        state.watch.channel_id = "general".into();
        state.watch.policy = WatchPolicyKind::Assigned;
        assert_eq!(update(&mut state, Message::AddWatch), None);
        state.watch.assigned_agent = "triage".into();
        assert_eq!(
            update(&mut state, Message::AddWatch),
            Some(Command::WatchChannel {
                channel_id: "general".into(),
                policy: TurnPolicy::Assigned("triage".into()),
            })
        );
    }

    #[test]
    fn successful_mutation_reloads_committed_state() {
        let mut state = State::default();
        state.busy = true;
        assert_eq!(
            update(
                &mut state,
                Message::Service(ServiceEvent::WriteFinished(Ok(())))
            ),
            Some(Command::Load)
        );
        assert!(!state.busy);
    }

    #[test]
    fn live_log_deduplicates_cursors_and_keeps_an_honest_tail() {
        let mut state = State::default();
        update(&mut state, Message::ToggleRunLog("ab".repeat(32)));
        for cursor in 1..=150 {
            update(
                &mut state,
                Message::RunLog(RunLogEvent::Line {
                    dispatch_id: "ab".repeat(32),
                    cursor,
                    stream: RunStream::Stdout,
                    text: format!("line {cursor}"),
                }),
            );
        }
        update(
            &mut state,
            Message::RunLog(RunLogEvent::Line {
                dispatch_id: "ab".repeat(32),
                cursor: 150,
                stream: RunStream::Stderr,
                text: "duplicate".into(),
            }),
        );
        let log = &state.run_logs[&"ab".repeat(32)];
        assert_eq!(log.entries.len(), MAX_ACTIVITY_ENTRIES);
        assert_eq!(log.dropped, 30);
        assert_eq!(log.last_cursor, 150);
        assert!(
            !log.entries.iter().any(
                |entry| matches!(entry, RunLogEntry::Line { text, .. } if text == "duplicate")
            )
        );
    }

    #[test]
    fn live_log_prettifies_jsonl_and_pairs_started_items() {
        let json_line = |value: serde_json::Value| RunLogEntry::Line {
            stream: RunStream::Stdout,
            text: value.to_string(),
        };
        let rows = semantic_log_rows(&[
            json_line(serde_json::json!({
                "type": "thread.started",
                "thread_id": "thread-123"
            })),
            json_line(serde_json::json!({ "type": "turn.started" })),
            json_line(serde_json::json!({
                "type": "item.started",
                "item": { "type": "command_execution", "command": "cargo test -p app" }
            })),
            json_line(serde_json::json!({
                "type": "item.completed",
                "item": {
                    "type": "command_execution",
                    "command": "cargo test -p app",
                    "aggregated_output": "running tests\n\n\nfinished\n",
                    "exit_code": 0,
                    "status": "completed"
                }
            })),
            json_line(serde_json::json!({
                "type": "item.started",
                "item": { "type": "agent_message", "text": "all tests passed" }
            })),
            json_line(serde_json::json!({
                "type": "item.completed",
                "item": { "type": "agent_message", "text": "all tests passed" }
            })),
            json_line(serde_json::json!({ "type": "turn.completed" })),
        ]);
        let summary = rows
            .iter()
            .map(|row| (row.kind, row.text.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            summary,
            vec![
                (SemanticLogKind::Status, "thread started: thread-123"),
                (SemanticLogKind::Status, "turn started"),
                (SemanticLogKind::Command, "cargo test -p app"),
                (SemanticLogKind::Output, "running tests"),
                (SemanticLogKind::Blank, ""),
                (SemanticLogKind::Output, "finished"),
                (SemanticLogKind::Status, "status: completed"),
                (SemanticLogKind::Exit, "exit: 0"),
                (SemanticLogKind::Message, "all tests passed"),
                (SemanticLogKind::Status, "turn completed"),
            ]
        );
        assert_eq!(
            rows.iter()
                .filter(|row| row.kind == SemanticLogKind::Command)
                .count(),
            1
        );
        assert!(!rows.iter().any(|row| row.text.contains("item.completed")));
    }

    #[test]
    fn live_log_semantic_rows_bound_untrusted_multiline_output() {
        let output = (0..10_000)
            .map(|line| format!("output line {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let rows = semantic_log_rows(&[RunLogEntry::Line {
            stream: RunStream::Stdout,
            text: serde_json::json!({
                "type": "item.completed",
                "item": {
                    "id": "cmd-1",
                    "type": "command_execution",
                    "command": "generate output",
                    "aggregated_output": output,
                    "exit_code": 101,
                    "status": "failed"
                }
            })
            .to_string(),
        }]);
        assert!(rows.len() <= MAX_ACTIVITY_ROWS);
        assert!(
            rows.iter()
                .all(|row| row.text.chars().count() <= MAX_ROW_CHARS)
        );
        assert!(rows.iter().any(|row| row.text.contains("lines omitted")));
        assert!(rows.iter().any(|row| row.text == "output line 9999"));
        assert_eq!(rows.last().map(|row| row.text.as_str()), Some("exit: 101"));
    }

    #[test]
    fn forge_run_channels_keep_repository_colons_and_reject_invalid_numbers() {
        assert_eq!(
            forge_item_channel("forge:team:repo:42"),
            Some(("team:repo", 42))
        );
        assert_eq!(forge_item_channel("forge:repo:0"), None);
        assert_eq!(forge_item_channel("general"), None);
    }

    #[test]
    fn run_links_leave_the_reducer_as_typed_intents() {
        let mut state = State::default();
        assert_eq!(
            reduce(
                &mut state,
                Message::OpenRunAnchor {
                    channel_id: "general".into(),
                    sequence: 73,
                },
            ),
            Some(Effect::Intent(AppIntent::Navigate(Route::Chat {
                channel: Some("general".into()),
                message: Some(73),
            })))
        );
        assert_eq!(
            reduce(
                &mut state,
                Message::OpenRunPullRequest {
                    channel_id: "forge:team:repo:42".into(),
                    number: 91,
                },
            ),
            Some(Effect::Intent(AppIntent::Navigate(Route::Forge {
                repository: "team:repo".into(),
                item: Some(91),
            })))
        );
    }

    #[test]
    fn terminal_duration_distinguishes_clocks_from_block_counters() {
        let mut run = RunRecord {
            run_id: "run".into(),
            dispatch_id: "ab".repeat(32),
            agent_id: "triage".into(),
            channel_id: "general".into(),
            anchor_sequence: 1,
            outcome: RunOutcome::Delivered,
            degraded: false,
            created_at: 10,
            delivered_at: 13,
            executing_node: "unknown".into(),
            output_ref: None,
            pr_number: None,
        };
        assert_eq!(run_duration(&run), "3 blocks");
        run.created_at = 1_800_000_000_000;
        run.delivered_at = 1_800_000_065_000;
        assert_eq!(run_duration(&run), "1m 5s");
    }
}
