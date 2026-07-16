//! Native agent roster, auto-reply, and activity screens.
//!
//! State and rendering stay transport-free; the host executes [`Command`]s
//! and feeds results back as [`ServiceEvent`]s.

mod run_log;
mod view;

use std::collections::BTreeMap;

use crate::view_api::{AppIntent, Route};

#[allow(unused_imports)] // Retains the existing screens::agents::RunLogEntry path.
pub use run_log::RunLogEntry;
pub use run_log::{RunLog, RunLogEvent, RunStream};
pub use view::view;

const LIBRARY_ROOT: &str = "/shared/skills";

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
    #[allow(dead_code)]
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
#[allow(clippy::large_enum_variant)]
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
            run_log::apply_event(&mut state.run_logs, &state.expanded_run_logs, event);
            None
        }
        Message::RetryCapabilities => Some(Command::RefreshCapabilities),
        Message::Service(event) => service(state, event),
    }
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

fn selected_agent(state: &State) -> Option<&AgentRecord> {
    let Resource::Ready(data) = &state.data else {
        return None;
    };
    match &state.selected_agent_id {
        Some(id) => data.agents.iter().find(|agent| &agent.id == id),
        None => data.agents.first(),
    }
}

pub fn forge_item_channel(channel_id: &str) -> Option<(&str, u64)> {
    let rest = channel_id.strip_prefix("forge:")?;
    let (repository, number) = rest.rsplit_once(':')?;
    let number = number.parse::<u64>().ok()?;
    (!repository.is_empty() && number > 0).then_some((repository, number))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme;

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
            view::resource_grant_labels(&caps),
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
        let mut state = State {
            data: Resource::Ready(data()),
            ..State::default()
        };
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
        let mut state = State {
            busy: true,
            ..State::default()
        };
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
        assert_eq!(view::run_duration(&run), "3 blocks");
        run.created_at = 1_800_000_000_000;
        run.delivered_at = 1_800_000_065_000;
        assert_eq!(view::run_duration(&run), "1m 5s");
    }
}
