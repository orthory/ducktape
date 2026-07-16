//! Native governance surface. Proposal arithmetic mirrors the committed
//! governance interface; I/O remains behind typed [`Command`]s.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use iced::widget::text::Wrapping;
use iced::widget::{
    Space, button, column, container, row, scrollable, text, text_editor, text_input, tooltip,
};
use iced::{Alignment, Background, Border, Color, Element, Length};

use crate::icons::{self, Icon};
use crate::theme::{
    self, BODY, CAPTION, HEADING, LABEL, MONO, Palette, RADIUS_LG, RADIUS_SM, SANS, SANS_SEMIBOLD,
    TITLE,
};
use crate::view_api::SubmitReceipt;

const MAX_SHARE_ACCOUNTS: usize = 256;
const MAX_SAFE_SHARES: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resource<T> {
    Loading,
    Empty,
    Error(String),
    Ready(T),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    All,
    Open,
    Settled,
}

impl Filter {
    const ALL: [(Self, &'static str); 3] = [
        (Self::All, "All"),
        (Self::Open, "Open"),
        (Self::Settled, "Settled"),
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalStatus {
    Open,
    Passed,
    Rejected,
}

impl ProposalStatus {
    const fn label(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Passed => "passed",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoterKind {
    ValidatorNode,
    Account,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VotingRule {
    DynamicValidatorMajority,
    Threshold { required_yes: u64 },
    ParticipatingMajority { quorum: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    AddValidator(String),
    RemoveValidator(String),
    Signal(String),
    AddResident(String),
    RemoveResident(String),
    ScheduleUpgrade {
        name: String,
        activation_height: u64,
        to_version: u32,
    },
    CancelUpgrade(String),
    UpdateModule {
        name: String,
        module_id: String,
        activation_height: u64,
        code_hash: String,
    },
    CancelModuleUpdate {
        name: String,
        module_id: String,
    },
    AdoptShares(Vec<ShareAllocation>),
    SetShares {
        account_id: String,
        shares: u64,
    },
    SetShareMode(bool),
}

impl Action {
    fn label(&self) -> &'static str {
        match self {
            Self::AddValidator(_) => "Add validator",
            Self::RemoveValidator(_) => "Remove validator",
            Self::Signal(_) => "Signal",
            Self::AddResident(_) => "Add resident",
            Self::RemoveResident(_) => "Remove resident",
            Self::ScheduleUpgrade { .. } => "Schedule upgrade",
            Self::CancelUpgrade(_) => "Cancel upgrade",
            Self::UpdateModule { .. } => "Update module",
            Self::CancelModuleUpdate { .. } => "Cancel module update",
            Self::AdoptShares(_) => "Adopt governance shares",
            Self::SetShares { .. } => "Set account shares",
            Self::SetShareMode(true) => "Use account shares",
            Self::SetShareMode(false) => "Use validator votes",
        }
    }

    fn detail(&self) -> String {
        match self {
            Self::AddValidator(key)
            | Self::RemoveValidator(key)
            | Self::AddResident(key)
            | Self::RemoveResident(key) => short_key(key),
            Self::Signal(copy) => copy.clone(),
            Self::ScheduleUpgrade {
                name,
                activation_height,
                to_version,
            } => format!("{name} · v{to_version} at #{activation_height}"),
            Self::CancelUpgrade(name) => name.clone(),
            Self::UpdateModule {
                name,
                module_id,
                activation_height,
                code_hash,
            } => format!(
                "{name} · {module_id} at #{activation_height} · {}",
                short_key(code_hash)
            ),
            Self::CancelModuleUpdate { name, module_id } => {
                format!("{name} · {module_id}")
            }
            Self::AdoptShares(allocations) => {
                format!("{} account allocations", allocations.len())
            }
            Self::SetShares { account_id, shares } => {
                format!("{} · {shares}", short_key(account_id))
            }
            Self::SetShareMode(enabled) => if *enabled {
                "account shares"
            } else {
                "validator ballots"
            }
            .into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ballot {
    pub principal: String,
    pub approve: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VotingPower {
    pub principal: String,
    pub power: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    pub id: String,
    pub action: Action,
    pub proposer: String,
    pub created_at: u64,
    pub deadline: u64,
    pub status: ProposalStatus,
    pub votes: Vec<Ballot>,
    pub voter_kind: VoterKind,
    pub electorate: Vec<VotingPower>,
    pub voting_rule: VotingRule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareAllocation {
    pub account_id: String,
    pub shares: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shares {
    pub active: bool,
    pub allocations: Vec<ShareAllocation>,
    pub total: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledUpgrade {
    pub name: String,
    pub to_version: u32,
    pub activation_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradeMember {
    pub key: String,
    pub display_name: String,
    pub ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradeStatus {
    pub current_version: u32,
    pub pending: Option<ScheduledUpgrade>,
    pub armed: bool,
    pub members: Vec<UpgradeMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceData {
    pub proposals: Vec<Proposal>,
    pub shares: Shares,
    pub local_nodes: Vec<String>,
    pub local_account: Option<String>,
    pub member_count: usize,
    pub legacy_can_vote: bool,
    pub known_accounts: Vec<String>,
    pub current_height: u64,
    pub upgrade: Resource<UpgradeStatus>,
}

#[derive(Debug, Clone)]
pub struct State {
    pub data: Resource<GovernanceData>,
    pub filter: Filter,
    pub signal_text: String,
    pub allocation_text: text_editor::Content,
    pub share_account: String,
    pub share_value: String,
    pub upgrade_name: String,
    pub upgrade_version: String,
    pub upgrade_height: String,
    pub busy: bool,
    pub loading: bool,
    pub reload_pending: bool,
    pub error: Option<String>,
    /// A validation error owned by a specific form, rendered inline beneath that
    /// form's controls instead of the off-screen bottom banner (B2).
    pub form_error: Option<(FormSlot, String)>,
    pub reload_error: Option<String>,
    pub operations: BTreeMap<String, OperationPhase>,
}

/// Which form a [`State::form_error`] belongs under, so the reason lands where
/// the user is looking rather than at the bottom of the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormSlot {
    ShareSetup,
    ShareChange,
    Schedule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationPhase {
    Pending,
    Receipt {
        height: u64,
        op_hash: Option<String>,
    },
    Finalized {
        height: u64,
        op_hash: Option<String>,
    },
    Rejected,
}

impl Default for State {
    fn default() -> Self {
        Self {
            data: Resource::Loading,
            filter: Filter::All,
            signal_text: String::new(),
            allocation_text: text_editor::Content::new(),
            share_account: String::new(),
            share_value: String::new(),
            upgrade_name: String::new(),
            upgrade_version: String::new(),
            upgrade_height: String::new(),
            busy: false,
            loading: false,
            reload_pending: false,
            error: None,
            form_error: None,
            reload_error: None,
            operations: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum Message {
    Load,
    Refresh,
    SetFilter(Filter),
    SignalChanged(String),
    ProposeSignal,
    AllocationEdited(text_editor::Action),
    ProposeAdoptShares,
    ShareAccountChanged(String),
    ShareValueChanged(String),
    ProposeSetShares,
    ProposeSetShareMode(bool),
    UpgradeNameChanged(String),
    UpgradeVersionChanged(String),
    UpgradeHeightChanged(String),
    ProposeScheduleUpgrade,
    ProposeCancelUpgrade(String),
    Vote { proposal_id: String, approve: bool },
    Execute(String),
    Service(ServiceEvent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Load,
    Propose { proposal_id: String, action: Action },
    Vote { proposal_id: String, approve: bool },
    Execute(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceEvent {
    Loaded(Result<Option<GovernanceData>, String>),
    ActionFinished {
        proposal_id: String,
        result: Result<SubmitReceipt, String>,
    },
}

pub fn update(state: &mut State, message: Message) -> Option<Command> {
    match message {
        Message::Load => {
            if !matches!(state.data, Resource::Ready(_)) {
                state.data = Resource::Loading;
            }
            request_reload(state)
        }
        Message::Refresh => request_reload(state),
        Message::SetFilter(filter) => {
            state.filter = filter;
            None
        }
        Message::SignalChanged(value) => {
            state.signal_text = value;
            None
        }
        Message::ProposeSignal => {
            if !can_propose(state) || state.busy {
                return None;
            }
            let copy = state.signal_text.trim().to_string();
            if copy.is_empty() {
                return None;
            }
            let command = start_proposal(state, Action::Signal(copy));
            if command.is_some() {
                state.signal_text.clear();
            }
            command
        }
        Message::AllocationEdited(action) => {
            state.allocation_text.perform(action);
            None
        }
        Message::ProposeAdoptShares => {
            if !can_propose(state) || state.busy {
                return None;
            }
            let allocations = match parse_share_allocations(&state.allocation_text.text()) {
                Ok(rows) => rows,
                Err(error) => {
                    state.form_error = Some((FormSlot::ShareSetup, error));
                    return None;
                }
            };
            start_proposal(state, Action::AdoptShares(allocations))
        }
        Message::ShareAccountChanged(value) => {
            state.share_account = value;
            None
        }
        Message::ShareValueChanged(value) => {
            state.share_value = value;
            None
        }
        Message::ProposeSetShares => {
            if !can_propose(state) || state.busy {
                return None;
            }
            let account_id = match valid_hex(&state.share_account) {
                Some(value) => value,
                None => {
                    state.form_error = Some((
                        FormSlot::ShareChange,
                        "Enter an account hex id and a non-negative integer share value.".into(),
                    ));
                    return None;
                }
            };
            let Ok(shares) = state.share_value.trim().parse::<u64>() else {
                state.form_error = Some((
                    FormSlot::ShareChange,
                    "Enter an account hex id and a non-negative integer share value.".into(),
                ));
                return None;
            };
            if shares > MAX_SAFE_SHARES {
                state.form_error = Some((
                    FormSlot::ShareChange,
                    format!("Shares must be at most {MAX_SAFE_SHARES}."),
                ));
                return None;
            }
            start_proposal(state, Action::SetShares { account_id, shares })
        }
        Message::ProposeSetShareMode(enabled) => {
            if can_propose(state) && !state.busy {
                start_proposal(state, Action::SetShareMode(enabled))
            } else {
                None
            }
        }
        Message::UpgradeNameChanged(value) => {
            state.upgrade_name = value;
            None
        }
        Message::UpgradeVersionChanged(value) => {
            state.upgrade_version = value;
            None
        }
        Message::UpgradeHeightChanged(value) => {
            state.upgrade_height = value;
            None
        }
        Message::ProposeScheduleUpgrade => {
            if !can_propose(state) || state.busy {
                return None;
            }
            let Resource::Ready(data) = &state.data else {
                return None;
            };
            let Resource::Ready(upgrade) = &data.upgrade else {
                return None;
            };
            let draft = ScheduledUpgrade {
                name: state.upgrade_name.trim().into(),
                to_version: state.upgrade_version.trim().parse().unwrap_or(0),
                activation_height: state.upgrade_height.trim().parse().unwrap_or(0),
            };
            if let Err(error) =
                validate_schedule(&draft, upgrade.current_version, data.current_height)
            {
                state.form_error = Some((FormSlot::Schedule, error));
                return None;
            }
            let command = start_proposal(
                state,
                Action::ScheduleUpgrade {
                    name: draft.name,
                    activation_height: draft.activation_height,
                    to_version: draft.to_version,
                },
            );
            if command.is_some() {
                state.upgrade_name.clear();
                state.upgrade_version.clear();
                state.upgrade_height.clear();
            }
            command
        }
        Message::ProposeCancelUpgrade(name) => {
            if can_propose(state) && !state.busy && !name.trim().is_empty() {
                start_proposal(state, Action::CancelUpgrade(name))
            } else {
                None
            }
        }
        Message::Vote {
            proposal_id,
            approve,
        } => {
            let eligible = proposal_by_id(state, &proposal_id).is_some_and(|proposal| {
                proposal.status == ProposalStatus::Open
                    && proposal_eligible(state, proposal)
                    && local_vote(state, proposal) != Some(approve)
            });
            if !eligible || state.busy || operation_in_flight(state, &proposal_id) {
                return None;
            }
            start(
                state,
                Command::Vote {
                    proposal_id,
                    approve,
                },
            )
        }
        Message::Execute(proposal_id) => {
            let open = proposal_by_id(state, &proposal_id)
                .is_some_and(|proposal| proposal.status == ProposalStatus::Open);
            if !open || state.busy || operation_in_flight(state, &proposal_id) {
                None
            } else {
                start(state, Command::Execute(proposal_id))
            }
        }
        Message::Service(ServiceEvent::Loaded(result)) => {
            state.loading = false;
            state.busy = false;
            match result {
                Ok(Some(data)) => {
                    settle_receipts(state, data.current_height);
                    state.operations.retain(|id, phase| {
                        data.proposals.iter().any(|proposal| proposal.id == *id)
                            || matches!(
                                phase,
                                OperationPhase::Pending | OperationPhase::Receipt { .. }
                            )
                    });
                    state.data = Resource::Ready(data);
                    state.reload_error = None;
                }
                Ok(None) => {
                    state.data = Resource::Empty;
                    state.reload_error = None;
                }
                Err(error) => {
                    if matches!(state.data, Resource::Ready(_)) {
                        state.reload_error = Some(error);
                    } else {
                        state.data = Resource::Error(error);
                    }
                }
            }
            if state.reload_pending {
                state.reload_pending = false;
                request_reload(state)
            } else {
                None
            }
        }
        Message::Service(ServiceEvent::ActionFinished {
            proposal_id,
            result,
        }) => {
            match result {
                Ok(receipt) => {
                    state.operations.insert(
                        proposal_id,
                        OperationPhase::Receipt {
                            height: receipt.height,
                            op_hash: receipt.op_hash,
                        },
                    );
                }
                Err(error) => {
                    state
                        .operations
                        .insert(proposal_id, OperationPhase::Rejected);
                    state.error = Some(error);
                }
            }
            state.busy = false;
            state.reload_pending = false;
            let command = request_reload(state);
            state.busy = true;
            command
        }
    }
}

fn start(state: &mut State, command: Command) -> Option<Command> {
    state.busy = true;
    state.error = None;
    state.form_error = None;
    if let Some(proposal_id) = command.proposal_id() {
        state
            .operations
            .insert(proposal_id.to_owned(), OperationPhase::Pending);
    }
    Some(command)
}

impl Command {
    fn proposal_id(&self) -> Option<&str> {
        match self {
            Self::Load => None,
            Self::Propose { proposal_id, .. } | Self::Vote { proposal_id, .. } => Some(proposal_id),
            Self::Execute(proposal_id) => Some(proposal_id),
        }
    }
}

fn start_proposal(state: &mut State, action: Action) -> Option<Command> {
    match fresh_proposal_id() {
        Ok(proposal_id) => start(
            state,
            Command::Propose {
                proposal_id,
                action,
            },
        ),
        Err(error) => {
            state.error = Some(error);
            None
        }
    }
}

fn request_reload(state: &mut State) -> Option<Command> {
    if state.loading || state.busy {
        state.reload_pending = true;
        None
    } else {
        state.loading = true;
        Some(Command::Load)
    }
}

fn settle_receipts(state: &mut State, current_height: u64) {
    for phase in state.operations.values_mut() {
        if let OperationPhase::Receipt { height, op_hash } = phase
            && current_height >= *height
        {
            *phase = OperationPhase::Finalized {
                height: *height,
                op_hash: op_hash.take(),
            };
        }
    }
}

fn operation_in_flight(state: &State, proposal_id: &str) -> bool {
    matches!(
        state.operations.get(proposal_id),
        Some(OperationPhase::Pending | OperationPhase::Receipt { .. })
    )
}

fn fresh_proposal_id() -> Result<String, String> {
    let mut bytes = [0_u8; 16];
    getrandom::getrandom(&mut bytes).map_err(|error| format!("proposal id randomness: {error}"))?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let mut hex = String::with_capacity(32);
    for byte in bytes {
        write!(&mut hex, "{byte:02x}").expect("writing to a string cannot fail");
    }
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    ))
}

fn proposal_by_id<'a>(state: &'a State, id: &str) -> Option<&'a Proposal> {
    let Resource::Ready(data) = &state.data else {
        return None;
    };
    data.proposals.iter().find(|proposal| proposal.id == id)
}

fn principal_is_local(data: &GovernanceData, proposal: &Proposal, principal: &str) -> bool {
    match proposal.voter_kind {
        VoterKind::ValidatorNode => data
            .local_nodes
            .iter()
            .any(|node| same_key(node, principal)),
        VoterKind::Account => data
            .local_account
            .as_deref()
            .is_some_and(|account| same_key(account, principal)),
    }
}

fn local_vote(state: &State, proposal: &Proposal) -> Option<bool> {
    let Resource::Ready(data) = &state.data else {
        return None;
    };
    proposal
        .votes
        .iter()
        .find(|ballot| principal_is_local(data, proposal, &ballot.principal))
        .map(|ballot| ballot.approve)
}

fn proposal_eligible(state: &State, proposal: &Proposal) -> bool {
    let Resource::Ready(data) = &state.data else {
        return false;
    };
    if proposal.electorate.is_empty() {
        proposal.voter_kind == VoterKind::ValidatorNode && data.legacy_can_vote
    } else {
        proposal
            .electorate
            .iter()
            .any(|entry| principal_is_local(data, proposal, &entry.principal))
    }
}

fn can_propose(state: &State) -> bool {
    let Resource::Ready(data) = &state.data else {
        return false;
    };
    if data.shares.active {
        let Some(account) = data.local_account.as_deref() else {
            return false;
        };
        data.shares
            .allocations
            .iter()
            .any(|allocation| same_key(&allocation.account_id, account))
    } else {
        data.legacy_can_vote
    }
}

pub fn tally(proposal: &Proposal) -> (u64, u64) {
    let mut yes = 0u64;
    let mut no = 0u64;
    for ballot in &proposal.votes {
        let power = if proposal.electorate.is_empty() {
            1
        } else {
            proposal
                .electorate
                .iter()
                .find(|entry| same_key(&entry.principal, &ballot.principal))
                .map_or(0, |entry| entry.power)
        };
        if ballot.approve {
            yes = yes.saturating_add(power);
        } else {
            no = no.saturating_add(power);
        }
    }
    (yes, no)
}

pub fn decision_threshold(proposal: &Proposal, member_count: usize) -> u64 {
    match proposal.voting_rule {
        VotingRule::DynamicValidatorMajority => member_count as u64 / 2 + 1,
        VotingRule::Threshold { required_yes } => required_yes,
        VotingRule::ParticipatingMajority { quorum } => quorum,
    }
}

pub fn can_settle_early(proposal: &Proposal, member_count: usize) -> bool {
    let (yes, no) = tally(proposal);
    match proposal.voting_rule {
        VotingRule::DynamicValidatorMajority => yes > member_count as u64 / 2,
        VotingRule::Threshold { required_yes } => yes >= required_yes,
        VotingRule::ParticipatingMajority { quorum } => {
            let total_power = if proposal.electorate.is_empty() {
                member_count as u64
            } else {
                proposal.electorate.iter().map(|entry| entry.power).sum()
            };
            yes.saturating_add(no) >= quorum && yes > total_power.saturating_sub(yes)
        }
    }
}

pub fn parse_share_allocations(text: &str) -> Result<Vec<ShareAllocation>, String> {
    let mut seen = BTreeSet::new();
    let mut allocations = Vec::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let mut fields = line.split_whitespace();
        let Some(raw_account) = fields.next() else {
            continue;
        };
        let Some(raw_shares) = fields.next() else {
            return Err("Use one unique ‘account-hex shares’ row per account.".into());
        };
        if fields.next().is_some() {
            return Err("Use one unique ‘account-hex shares’ row per account.".into());
        }
        let Some(account_id) = valid_hex(raw_account) else {
            return Err("Use one unique ‘account-hex shares’ row per account.".into());
        };
        let Ok(shares) = raw_shares.parse::<u64>() else {
            return Err("Use one unique ‘account-hex shares’ row per account.".into());
        };
        if shares == 0
            || shares > MAX_SAFE_SHARES
            || allocations.len() == MAX_SHARE_ACCOUNTS
            || !seen.insert(account_id.clone())
        {
            return Err("Use one unique ‘account-hex shares’ row per account.".into());
        }
        allocations.push(ShareAllocation { account_id, shares });
    }
    let total = allocations.iter().try_fold(0_u64, |total, allocation| {
        total.checked_add(allocation.shares)
    });
    if allocations.is_empty() || total.is_none_or(|total| total > MAX_SAFE_SHARES) {
        Err("Use one unique ‘account-hex shares’ row per account.".into())
    } else {
        Ok(allocations)
    }
}

pub fn format_share_percent(shares: u64, total: u64) -> String {
    if total == 0 {
        return "0%".into();
    }
    let hundredths = ((shares as u128 * 10_000 + total as u128 / 2) / total as u128) as u64;
    let whole = hundredths / 100;
    match hundredths % 100 {
        0 => format!("{whole}%"),
        fraction if fraction % 10 == 0 => format!("{whole}.{}%", fraction / 10),
        fraction => format!("{whole}.{fraction:02}%"),
    }
}

pub fn validate_schedule(
    draft: &ScheduledUpgrade,
    current_version: u32,
    current_height: u64,
) -> Result<(), String> {
    if draft.name.trim().is_empty() {
        return Err("Name the upgrade.".into());
    }
    if draft.to_version <= current_version {
        return Err(format!(
            "Target version must be greater than the current version ({current_version})."
        ));
    }
    if draft.activation_height <= current_height {
        return Err(format!(
            "Activation height must be past the current height ({current_height})."
        ));
    }
    Ok(())
}

pub fn view(state: &State, mode: theme::Mode) -> Element<'_, Message> {
    let p = *theme::palette(mode);
    let Resource::Ready(data) = &state.data else {
        return resource_view(&state.data, p);
    };
    let mut content = column![
        header(state, data, p),
        filter_bar(state, p),
        shares_panel(state, data, p),
        proposal_form(state, data, p),
        upgrade_panel(state, data, p),
    ]
    .width(Length::Fill)
    .height(Length::Fill);
    if let Some(error) = &state.error {
        content = content.push(
            container(selectable_error(error, p))
                .padding([8, 22])
                .width(Length::Fill),
        );
    }
    if let Some(error) = &state.reload_error {
        content = content.push(
            container(
                text(format!("Live refresh unavailable: {error}"))
                    .font(SANS)
                    .size(LABEL)
                    .color(p.amber),
            )
            .padding([8, 22])
            .width(Length::Fill),
        );
    }
    content = content.push(proposal_list(state, data, p));
    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| surface(p.canvas))
        .into()
}

fn resource_view(resource: &Resource<GovernanceData>, p: Palette) -> Element<'_, Message> {
    let (title, detail, retry) = match resource {
        Resource::Loading => (
            "Loading governance",
            "Reading proposals and the frozen voting rule…",
            false,
        ),
        Resource::Empty => (
            "Governance unavailable",
            "The connected node exposes no governance surface.",
            true,
        ),
        Resource::Error(error) => ("Governance unavailable", error.as_str(), true),
        Resource::Ready(_) => unreachable!(),
    };
    let mut body = column![
        icons::view(Icon::Governance, 26.0, p.icon_idle),
        text(title).font(SANS_SEMIBOLD).size(TITLE).color(p.ink),
        text(detail).font(SANS).size(BODY).color(p.muted_2),
    ]
    .spacing(7)
    .align_x(Alignment::Center);
    if retry {
        body = body.push(outline_button("Retry", Message::Load, true, p));
    }
    container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .center(Length::Fill)
        .style(move |_| surface(p.canvas))
        .into()
}

fn header(state: &State, data: &GovernanceData, p: Palette) -> Element<'static, Message> {
    let open = data
        .proposals
        .iter()
        .filter(|proposal| proposal.status == ProposalStatus::Open)
        .count();
    let (role, tone) = if data.shares.active {
        let allocation = data.local_account.as_deref().and_then(|account| {
            data.shares
                .allocations
                .iter()
                .find(|allocation| same_key(&allocation.account_id, account))
        });
        allocation.map_or(("Read only".into(), p.amber), |allocation| {
            (
                format!(
                    "Shareholder {}/{} ({})",
                    allocation.shares,
                    data.shares.total,
                    format_share_percent(allocation.shares, data.shares.total)
                ),
                p.green,
            )
        })
    } else if data.legacy_can_vote {
        ("Voting member".into(), p.green)
    } else {
        ("Read only".into(), p.amber)
    };
    container(
        row![
            text("Governance")
                .font(SANS_SEMIBOLD)
                .size(HEADING)
                .color(p.ink),
            text(format!("{open} open · {}", data.proposals.len()))
                .font(MONO)
                .size(LABEL)
                .color(p.muted_2),
            text(format!("height #{}", group_digits(data.current_height)))
                .font(MONO)
                .size(CAPTION)
                .color(p.muted_2),
            Space::new().width(Length::Fill),
            if state.loading {
                pill("refreshing", p.amber, p)
            } else {
                Space::new().width(0).into()
            },
            pill(role, tone, p),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .height(56)
    .padding([0, 22])
    .align_y(Alignment::Center)
    .style(move |_| ruled_surface(p.paper, p.border_soft))
    .into()
}

fn filter_bar(state: &State, p: Palette) -> Element<'_, Message> {
    let mut filters = row![].spacing(7);
    for (filter, label) in Filter::ALL {
        filters = filters.push(segment_button(
            label,
            state.filter == filter,
            Message::SetFilter(filter),
            p,
        ));
    }
    container(filters)
        .width(Length::Fill)
        .padding([12, 22])
        .style(move |_| ruled_surface(p.paper, p.border_soft))
        .into()
}

fn shares_panel<'a>(
    state: &'a State,
    data: &'a GovernanceData,
    p: Palette,
) -> Element<'a, Message> {
    let can = can_propose(state) && !state.busy;
    let mode = if data.shares.active {
        format!("{} account shares", data.shares.total)
    } else {
        "validator ballots · default".into()
    };
    let mut body = column![
        row![
            section_label("VOTING MODE", p),
            pill(mode, if data.shares.active { p.green } else { p.amber }, p),
        ]
        .spacing(7)
        .align_y(Alignment::Center)
    ]
    .spacing(9);
    if data.shares.allocations.is_empty() {
        let accounts = if data.known_accounts.is_empty() {
            "none bound yet".into()
        } else {
            data.known_accounts
                .iter()
                .map(|account| short_key(account))
                .collect::<Vec<_>>()
                .join(", ")
        };
        body = body
            .push(
                text(format!("Validator ballots are the default. Initial share setup also enables share mode. Existing Identity accounts: {accounts}."))
                    .font(SANS)
                    .size(LABEL)
                    .color(p.muted_2),
            )
            .push(
                row![
                    text_editor(&state.allocation_text)
                        .placeholder("account-hex 100\none-account-per-line 40")
                        .on_action(Message::AllocationEdited)
                        .font(MONO)
                        .size(LABEL)
                        .padding([7, 8])
                        .min_height(58)
                        .max_height(92),
                    filled_button("Propose share setup", Message::ProposeAdoptShares, can, p),
                ]
                .spacing(8),
            );
        body = push_form_error(body, state, FormSlot::ShareSetup, p);
    } else {
        body = body
            .push(
                row![
                    text(if data.shares.active {
                        "New proposals use frozen account-share power."
                    } else {
                        "New proposals use one ballot per validator; configured shares are retained."
                    })
                    .font(SANS)
                    .size(LABEL)
                    .color(p.muted_2),
                    Space::new().width(Length::Fill),
                    filled_button(
                        if data.shares.active { "Propose validator mode" } else { "Propose share mode" },
                        Message::ProposeSetShareMode(!data.shares.active),
                        can,
                        p,
                    ),
                ]
                .align_y(Alignment::Center),
            )
            .push(allocation_chips(&data.shares, p))
            .push(
                row![
                    sem_input(
                        "Account hex",
                        &state.share_account,
                        text_input("Account hex", &state.share_account)
                            .on_input(Message::ShareAccountChanged)
                            .font(MONO)
                            .size(LABEL)
                            .padding([7, 8])
                            .width(Length::Fill),
                    ),
                    sem_input(
                        "Shares (0 removes)",
                        &state.share_value,
                        text_input("Shares (0 removes)", &state.share_value)
                            .on_input(Message::ShareValueChanged)
                            .font(MONO)
                            .size(LABEL)
                            .padding([7, 8])
                            .width(145),
                    ),
                    filled_button("Propose change", Message::ProposeSetShares, can, p),
                ]
                .spacing(8),
            );
        body = push_form_error(body, state, FormSlot::ShareChange, p);
    }
    section(body, p)
}

/// Configured allocations as a wrapping set of per-item chips, each carrying its
/// full account hex on hover (M2) — never a run-on mono blob.
fn allocation_chips(shares: &Shares, p: Palette) -> Element<'static, Message> {
    let mut chips = row![].spacing(6).align_y(Alignment::Center);
    for allocation in &shares.allocations {
        let chip = pill(
            format!(
                "{} · {} · {}",
                short_key(&allocation.account_id),
                allocation.shares,
                format_share_percent(allocation.shares, shares.total)
            ),
            p.ink_soft,
            p,
        );
        chips = chips.push(tip(chip, allocation.account_id.clone(), p));
    }
    chips.wrap().into()
}

/// Renders a form's own validation error inline beneath its controls (B2).
fn push_form_error<'a>(
    body: iced::widget::Column<'a, Message>,
    state: &State,
    slot: FormSlot,
    p: Palette,
) -> iced::widget::Column<'a, Message> {
    match &state.form_error {
        Some((owner, message)) if *owner == slot => body.push(selectable_error(message, p)),
        _ => body,
    }
}

fn proposal_form<'a>(
    state: &'a State,
    data: &'a GovernanceData,
    p: Palette,
) -> Element<'a, Message> {
    let can = can_propose(state) && !state.busy;
    let body = if can_propose(state) {
        card(
            column![
                text("Signal proposal")
                    .font(SANS_SEMIBOLD)
                    .size(TITLE)
                    .color(p.ink_soft),
                text(format!(
                    "Put a question to the {}. Passing binds the signal; it has no on-chain effect of its own.",
                    if data.shares.active { "shareholders" } else { "validator set" }
                ))
                .font(SANS)
                .size(LABEL)
                .color(p.muted_2),
                row![
                    sem_input(
                        "Signal",
                        &state.signal_text,
                        text_input("Describe what the set should signal…", &state.signal_text)
                            .on_input(Message::SignalChanged)
                            .on_submit(Message::ProposeSignal)
                            .font(SANS)
                            .size(BODY)
                            .padding([8, 9])
                            .width(Length::Fill),
                    ),
                    filled_button("Propose", Message::ProposeSignal, can, p),
                ]
                .spacing(8),
            ]
            .spacing(8),
            p,
        )
    } else {
        notice(
            if data.shares.active {
                "Only an eligible shareholder account can open or vote on proposals."
            } else {
                "Only an eligible validator can open or vote on proposals."
            },
            p,
        )
    };
    section(
        column![
            row![
                icons::view(Icon::Governance, 13.0, p.muted_2),
                section_label("OPEN A PROPOSAL", p),
            ]
            .spacing(7),
            body,
        ]
        .spacing(9),
        p,
    )
}

fn upgrade_panel<'a>(
    state: &'a State,
    data: &'a GovernanceData,
    p: Palette,
) -> Element<'a, Message> {
    let mut body = column![
        row![
            icons::view(Icon::Governance, 13.0, p.muted_2),
            section_label("NODE UPGRADE", p),
        ]
        .spacing(7)
        .align_y(Alignment::Center)
    ]
    .spacing(9);
    match &data.upgrade {
        Resource::Loading => body = body.push(muted("Loading upgrade status…", p)),
        Resource::Empty => body = body.push(muted("Upgrade status unavailable.", p)),
        Resource::Error(error) => {
            body = body.push(muted(&format!("Upgrade status unavailable: {error}"), p))
        }
        Resource::Ready(upgrade) => {
            body = body.push(pill(
                format!("node v{}", upgrade.current_version),
                p.green,
                p,
            ));
            if let Some(pending) = &upgrade.pending {
                let ready = upgrade.members.iter().filter(|member| member.ready).count();
                let mut card_body = column![
                    row![
                        text(pending.name.clone())
                            .font(SANS_SEMIBOLD)
                            .size(TITLE)
                            .color(p.ink),
                        pill(
                            if upgrade.armed {
                                "armed"
                            } else {
                                "awaiting readiness"
                            },
                            if upgrade.armed { p.green } else { p.amber },
                            p
                        ),
                    ]
                    .spacing(9),
                    text(format!(
                        "v{} → v{}    activates at #{}    ready {}/{}",
                        upgrade.current_version,
                        pending.to_version,
                        group_digits(pending.activation_height),
                        ready,
                        upgrade.members.len()
                    ))
                    .font(MONO)
                    .size(LABEL)
                    .color(p.ink_soft),
                ]
                .spacing(8);
                for member in &upgrade.members {
                    card_body = card_body.push(
                        row![
                            text(if member.ready { "✓" } else { "○" })
                                .font(SANS)
                                .size(BODY)
                                .color(if member.ready { p.green } else { p.muted_2 }),
                            text(member.display_name.clone())
                                .font(SANS)
                                .size(LABEL)
                                .color(p.ink_soft),
                            Space::new().width(Length::Fill),
                            text(if member.ready { "ready" } else { "arming" })
                                .font(MONO)
                                .size(CAPTION)
                                .color(p.muted_2),
                        ]
                        .spacing(8),
                    );
                }
                card_body = card_body.push(row![
                    Space::new().width(Length::Fill),
                    outline_button(
                        "Propose cancel",
                        Message::ProposeCancelUpgrade(pending.name.clone()),
                        can_propose(state) && !state.busy,
                        p,
                    ),
                ]);
                body = body.push(card(card_body, p));
            } else if can_propose(state) {
                let mut form = column![
                    text("Schedule upgrade")
                        .font(SANS_SEMIBOLD)
                        .size(TITLE)
                        .color(p.ink_soft),
                    text(format!(
                        "On v{}, current height #{}. Governance authorizes; the upgrade arms once every validator signals ready.",
                        upgrade.current_version, group_digits(data.current_height)
                    ))
                    .font(SANS)
                    .size(LABEL)
                    .color(p.muted_2),
                    row![
                        sem_input(
                            "Upgrade name",
                            &state.upgrade_name,
                            text_input("Upgrade name", &state.upgrade_name)
                                .on_input(Message::UpgradeNameChanged)
                                .font(SANS)
                                .size(BODY)
                                .padding([8, 9])
                                .width(Length::Fill),
                        ),
                        sem_input(
                            "Target version",
                            &state.upgrade_version,
                            text_input(
                                &format!("Target version (> {})", upgrade.current_version),
                                &state.upgrade_version,
                            )
                            .on_input(Message::UpgradeVersionChanged)
                            .font(MONO)
                            .size(LABEL)
                            .padding([8, 9])
                            .width(165),
                        ),
                        sem_input(
                            "Activation height",
                            &state.upgrade_height,
                            text_input(
                                &format!("Activation height (> {})", group_digits(data.current_height)),
                                &state.upgrade_height,
                            )
                            .on_input(Message::UpgradeHeightChanged)
                            .font(MONO)
                            .size(LABEL)
                            .padding([8, 9])
                            .width(185),
                        ),
                        filled_button(
                            "Propose",
                            Message::ProposeScheduleUpgrade,
                            !state.busy,
                            p,
                        ),
                    ]
                    .spacing(8),
                ]
                .spacing(8);
                form = push_form_error(form, state, FormSlot::Schedule, p);
                body = body.push(card(form, p));
            } else {
                body = body.push(muted(
                    "No upgrade scheduled. Only an eligible validator can propose one.",
                    p,
                ));
            }
        }
    }
    section(body, p)
}

fn proposal_list<'a>(
    state: &'a State,
    data: &'a GovernanceData,
    p: Palette,
) -> Element<'a, Message> {
    let mut proposals: Vec<&Proposal> = data
        .proposals
        .iter()
        .filter(|proposal| match state.filter {
            Filter::All => true,
            Filter::Open => proposal.status == ProposalStatus::Open,
            Filter::Settled => proposal.status != ProposalStatus::Open,
        })
        .collect();
    proposals.sort_by(|left, right| {
        let left_open = left.status == ProposalStatus::Open;
        let right_open = right.status == ProposalStatus::Open;
        right_open
            .cmp(&left_open)
            .then_with(|| left.id.cmp(&right.id))
    });
    if proposals.is_empty() {
        return container(
            column![
                icons::view(Icon::Governance, 26.0, p.icon_idle),
                text(match state.filter {
                    Filter::All => "No proposals to show.",
                    Filter::Open => "No open proposals to show.",
                    Filter::Settled => "No settled proposals to show.",
                })
                .font(SANS)
                .size(TITLE)
                .color(p.muted_2),
                text("Proposals appear here once an eligible voter opens one.")
                    .font(SANS)
                    .size(LABEL)
                    .color(p.muted_2),
            ]
            .spacing(6)
            .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center(Length::Fill)
        .into();
    }
    let mut list = column![].spacing(10);
    for proposal in proposals {
        list = list.push(proposal_card(state, data, proposal, p));
    }
    scrollable(container(list).padding(12).width(Length::Fill))
        .height(Length::Fill)
        .into()
}

fn proposal_card<'a>(
    state: &'a State,
    data: &'a GovernanceData,
    proposal: &'a Proposal,
    p: Palette,
) -> Element<'a, Message> {
    let (yes, no) = tally(proposal);
    let total = yes.saturating_add(no);
    let threshold = decision_threshold(proposal, data.member_count);
    let my_vote = proposal
        .votes
        .iter()
        .find(|ballot| principal_is_local(data, proposal, &ballot.principal))
        .map(|ballot| ballot.approve);
    let tone = match proposal.status {
        ProposalStatus::Open => p.amber,
        ProposalStatus::Passed => p.green,
        ProposalStatus::Rejected => p.danger,
    };
    let proposer_local = principal_is_local(data, proposal, &proposal.proposer);
    let operation = state.operations.get(&proposal.id);
    let (operation_copy, operation_tone) = operation.map_or((None, p.muted_2), |phase| {
        let copy = match phase {
            OperationPhase::Pending => "✓ submitted".into(),
            OperationPhase::Receipt {
                height, op_hash, ..
            } => format!(
                "✓ confirming #{}{}",
                height,
                op_hash
                    .as_deref()
                    .map(|hash| format!(" · {}", short_key(hash)))
                    .unwrap_or_default()
            ),
            OperationPhase::Finalized {
                height, op_hash, ..
            } => format!(
                "✓✓ finalized #{}{}",
                height,
                op_hash
                    .as_deref()
                    .map(|hash| format!(" · {}", short_key(hash)))
                    .unwrap_or_default()
            ),
            OperationPhase::Rejected => "× rejected".into(),
        };
        let tone = match phase {
            OperationPhase::Pending | OperationPhase::Receipt { .. } => p.amber,
            OperationPhase::Finalized { .. } => p.green,
            OperationPhase::Rejected => p.danger,
        };
        (Some(copy), tone)
    });
    // Meta line: proposer (+ full hex tooltip), "this node" when local, and the
    // proposal id (+ full id tooltip) — the row truncates each, so hover reveals
    // the copy-worthy full value (P1).
    let mut meta = row![
        tip(
            text(format!("by {}", short_key(&proposal.proposer)))
                .font(MONO)
                .size(LABEL)
                .color(p.muted_2),
            proposal.proposer.clone(),
            p,
        ),
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    if proposer_local {
        meta = meta.push(
            text("· this node")
                .font(MONO)
                .size(LABEL)
                .color(p.muted_2),
        );
    }
    meta = meta.push(tip(
        text(format!("· {}", short_key(&proposal.id)))
            .font(MONO)
            .size(LABEL)
            .color(p.muted_2),
        proposal.id.clone(),
        p,
    ));
    let mut body = column![
        row![
            text(proposal.action.label())
                .font(SANS_SEMIBOLD)
                .size(TITLE)
                .color(p.ink),
            Space::new().width(Length::Fill),
            pill(proposal.status.label(), tone, p),
        ]
        .align_y(Alignment::Center),
        text(proposal.action.detail())
            .font(SANS)
            .size(BODY)
            .color(p.ink_soft)
            .wrapping(Wrapping::WordOrGlyph)
            .width(Length::Fill),
        meta.wrap(),
        row![
            text(format!(
                "consensus {} → deadline {}",
                proposal.created_at, proposal.deadline
            ))
            .font(MONO)
            .size(CAPTION)
            .color(p.muted_2),
            if let Some(copy) = operation_copy {
                text(format!("· {copy}"))
                    .font(MONO)
                    .size(CAPTION)
                    .color(operation_tone)
            } else {
                text("").font(MONO).size(CAPTION).color(p.muted_2)
            },
        ]
        .spacing(8),
        tally_bar(yes, no, p),
        row![
            text(format!("approve {yes}"))
                .font(MONO)
                .size(LABEL)
                .color(p.green),
            text(format!("reject {no}"))
                .font(MONO)
                .size(LABEL)
                .color(p.danger),
            text(format!("· {total} cast"))
                .font(MONO)
                .size(LABEL)
                .color(p.muted_3),
            Space::new().width(Length::Fill),
            text(match proposal.voting_rule {
                VotingRule::ParticipatingMajority { .. } => {
                    format!("quorum {threshold} + majority")
                }
                _ => format!("needs {threshold} approve"),
            })
            .font(MONO)
            .size(LABEL)
            .color(p.muted_3),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    ]
    .spacing(8);
    if proposal.status == ProposalStatus::Open {
        // Scope disability to THIS proposal's in-flight op — an unrelated write
        // no longer greys every card, and Settle stays live on open proposals
        // that have no op of their own (M1).
        let operation_in_flight = operation_in_flight(state, &proposal.id);
        let eligible = proposal_eligible(state, proposal) && !operation_in_flight;
        body = body.push(
            row![
                vote_button(
                    "✓  Approve",
                    my_vote == Some(true),
                    Message::Vote {
                        proposal_id: proposal.id.clone(),
                        approve: true,
                    },
                    eligible && my_vote != Some(true),
                    p.green,
                    p,
                ),
                vote_button(
                    "×  Reject",
                    my_vote == Some(false),
                    Message::Vote {
                        proposal_id: proposal.id.clone(),
                        approve: false,
                    },
                    eligible && my_vote != Some(false),
                    p.danger,
                    p,
                ),
                Space::new().width(Length::Fill),
                filled_button(
                    if can_settle_early(proposal, data.member_count) {
                        "Settle · ready"
                    } else {
                        "Settle"
                    },
                    Message::Execute(proposal.id.clone()),
                    !operation_in_flight,
                    p,
                ),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );
    }
    container(body)
        .width(Length::Fill)
        .padding([14, 15])
        .style(move |_| rounded_surface(p.paper, p.border, RADIUS_LG))
        .into()
}

fn tally_bar(yes: u64, no: u64, p: Palette) -> Element<'static, Message> {
    let total = yes.saturating_add(no).max(1);
    let yes_portion = (yes.saturating_mul(100) / total).min(100) as u16;
    let yes_width = if yes_portion == 0 {
        Length::Fixed(0.0)
    } else {
        Length::FillPortion(yes_portion)
    };
    let no_width = if yes_portion == 100 {
        Length::Fixed(0.0)
    } else {
        Length::FillPortion(100 - yes_portion)
    };
    container(
        row![
            container(Space::new().height(6))
                .width(yes_width)
                .style(move |_| surface(p.green)),
            container(Space::new().height(6))
                .width(no_width)
                .style(move |_| surface(p.red)),
        ]
        .height(6),
    )
    .width(Length::Fill)
    .style(move |_| rounded_surface(p.sunken, p.border_soft, 3.0))
    .into()
}

fn section<'a>(body: impl Into<Element<'a, Message>>, p: Palette) -> Element<'a, Message> {
    container(body)
        .width(Length::Fill)
        .padding([12, 22])
        .style(move |_| ruled_surface(p.paper, p.border_soft))
        .into()
}

fn card<'a>(body: impl Into<Element<'a, Message>>, p: Palette) -> Element<'a, Message> {
    container(body)
        .width(Length::Fill)
        .padding([12, 13])
        .style(move |_| rounded_surface(p.paper, p.border, RADIUS_LG))
        .into()
}

/// Native hover tooltip carrying a full value the row truncates (P1/M2).
fn tip<'a>(
    control: impl Into<Element<'a, Message>>,
    label: impl ToString,
    p: Palette,
) -> Element<'a, Message> {
    tooltip(
        control,
        container(
            text(label.to_string())
                .font(MONO)
                .size(CAPTION)
                .color(p.ink),
        )
        .padding([4, 8])
        .style(move |_| rounded_surface(p.paper, p.border, 6.0)),
        tooltip::Position::Bottom,
    )
    .gap(6)
    .into()
}

/// A read-only `text_input` — focusable and selectable so a voter can copy the
/// reason, but never editable. Mirrors the `workspace.rs` `selectable_error`.
fn selectable_error(message: &str, p: Palette) -> Element<'static, Message> {
    text_input("", message)
        .font(SANS)
        .size(LABEL)
        .padding(0)
        .style(move |_, _| iced::widget::text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            icon: p.danger,
            placeholder: p.danger,
            value: p.danger,
            selection: theme::ACCENTS[0],
        })
        .into()
}

fn section_label(label: &'static str, p: Palette) -> Element<'static, Message> {
    text(label)
        .font(SANS_SEMIBOLD)
        .size(CAPTION)
        .color(p.muted_2)
        .into()
}

fn muted(copy: &str, p: Palette) -> Element<'static, Message> {
    text(copy.to_string())
        .font(SANS)
        .size(LABEL)
        .color(p.muted_2)
        .into()
}

fn notice(copy: &'static str, p: Palette) -> Element<'static, Message> {
    container(
        row![
            icons::view(Icon::Node, 15.0, p.amber),
            text(copy).font(SANS).size(BODY).color(p.amber),
        ]
        .spacing(9)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([11, 13])
    .style(move |_| rounded_surface(p.sunken, p.amber, RADIUS_LG))
    .into()
}

fn pill(label: impl ToString, tone: Color, p: Palette) -> Element<'static, Message> {
    container(text(label.to_string()).font(SANS).size(CAPTION).color(tone))
        .padding([3, 8])
        .style(move |_| rounded_surface(p.paper, tone, RADIUS_SM))
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

fn segment_button<'a>(
    label: &'a str,
    active: bool,
    message: Message,
    p: Palette,
) -> Element<'a, Message> {
    let btn = button(text(label).font(SANS).size(LABEL))
        .padding([5, 11])
        .style(move |_, _| iced::widget::button::Style {
            background: Some(Background::Color(if active { p.chip } else { p.paper })),
            text_color: if active { p.ink } else { p.muted_2 },
            border: Border {
                radius: RADIUS_SM.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .on_press(message);
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Button, label, btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn outline_button<'a>(
    label: &'a str,
    message: Message,
    enabled: bool,
    p: Palette,
) -> Element<'a, Message> {
    let control = button(text(label).font(SANS).size(BODY))
        .padding([7, 12])
        .style(move |_, status| iced::widget::button::Style {
            background: Some(Background::Color(
                if enabled && matches!(status, iced::widget::button::Status::Hovered) {
                    p.titlebar
                } else {
                    p.paper
                },
            )),
            text_color: if enabled { p.ink_soft } else { p.muted_2 },
            border: Border {
                color: if enabled { p.border_strong } else { p.border_soft },
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            ..Default::default()
        });
    let control = if enabled {
        control.on_press(message)
    } else {
        control
    };
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::Sem::new(iced_agent_plugin::Role::Button, label, control)
        .disabled(!enabled)
        .into();
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    control.into()
}

fn filled_button<'a>(
    label: &'a str,
    message: Message,
    enabled: bool,
    p: Palette,
) -> Element<'a, Message> {
    let control = button(text(label).font(SANS).size(BODY))
        .padding([7, 12])
        .style(move |_, _| iced::widget::button::Style {
            background: Some(Background::Color(if enabled { p.filled } else { p.sunken })),
            text_color: if enabled { p.on_filled } else { p.muted_2 },
            border: Border {
                color: if enabled { p.filled } else { p.border_soft },
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            ..Default::default()
        });
    let control = if enabled {
        control.on_press(message)
    } else {
        control
    };
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::Sem::new(iced_agent_plugin::Role::Button, label, control)
        .disabled(!enabled)
        .into();
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    control.into()
}

fn vote_button<'a>(
    label: &'a str,
    active: bool,
    message: Message,
    enabled: bool,
    tone: Color,
    p: Palette,
) -> Element<'a, Message> {
    let control = button(text(label).font(SANS).size(BODY))
        .padding([7, 12])
        .style(move |_, _| iced::widget::button::Style {
            background: Some(Background::Color(if active { p.sunken } else { p.paper })),
            text_color: if active || enabled { tone } else { p.muted_2 },
            border: Border {
                color: if active || enabled {
                    tone
                } else {
                    p.border_soft
                },
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            ..Default::default()
        });
    let control = if enabled {
        control.on_press(message)
    } else {
        control
    };
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::Sem::new(iced_agent_plugin::Role::Button, label, control)
        .disabled(!enabled)
        .into();
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    control.into()
}

fn valid_hex(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase();
    (!normalized.is_empty()
        && normalized.len().is_multiple_of(2)
        && normalized.bytes().all(|byte| byte.is_ascii_hexdigit()))
    .then_some(normalized)
}

fn same_key(left: &str, right: &str) -> bool {
    left.trim().eq_ignore_ascii_case(right.trim())
}

/// Groups a block height into thousands (`1234567` → `1,234,567`) so long
/// activation heights stay legible (P7).
fn group_digits(value: u64) -> String {
    let digits = value.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

fn short_key(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        "—".into()
    } else if value.len() <= 16 {
        value.into()
    } else {
        // Char-based slicing: proposal ids arrive from the wire unvalidated for
        // UTF-8 boundaries, so a byte slice at 8 would panic mid-codepoint.
        let head: String = value.chars().take(8).collect();
        let count = value.chars().count();
        let tail: String = value.chars().skip(count.saturating_sub(8)).collect();
        format!("{head}…{tail}")
    }
}

fn surface(color: Color) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(color)),
        ..Default::default()
    }
}

fn ruled_surface(color: Color, border: Color) -> iced::widget::container::Style {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn proposal(rule: VotingRule) -> Proposal {
        Proposal {
            id: "signal:1".into(),
            action: Action::Signal("Ship it".into()),
            proposer: "aa".repeat(32),
            created_at: 10,
            deadline: 20,
            status: ProposalStatus::Open,
            votes: Vec::new(),
            voter_kind: VoterKind::ValidatorNode,
            electorate: Vec::new(),
            voting_rule: rule,
        }
    }

    fn ready() -> State {
        State {
            data: Resource::Ready(GovernanceData {
                proposals: vec![proposal(VotingRule::DynamicValidatorMajority)],
                shares: Shares {
                    active: false,
                    allocations: Vec::new(),
                    total: 0,
                },
                local_nodes: vec!["aa".repeat(32)],
                local_account: Some("11".repeat(32)),
                member_count: 3,
                legacy_can_vote: true,
                known_accounts: vec!["11".repeat(32)],
                current_height: 100,
                upgrade: Resource::Ready(UpgradeStatus {
                    current_version: 3,
                    pending: None,
                    armed: false,
                    members: Vec::new(),
                }),
            }),
            ..State::default()
        }
    }

    #[test]
    fn service_preserves_loading_empty_error_and_populated_states() {
        let mut state = State::default();
        update(&mut state, Message::Service(ServiceEvent::Loaded(Ok(None))));
        assert_eq!(state.data, Resource::Empty);
        update(
            &mut state,
            Message::Service(ServiceEvent::Loaded(Err("offline".into()))),
        );
        assert_eq!(state.data, Resource::Error("offline".into()));
        let Resource::Ready(data) = ready().data else {
            unreachable!()
        };
        update(
            &mut state,
            Message::Service(ServiceEvent::Loaded(Ok(Some(data)))),
        );
        assert!(matches!(state.data, Resource::Ready(_)));
    }

    #[test]
    fn share_rows_require_unique_even_length_hex_and_positive_integers() {
        assert_eq!(
            parse_share_allocations("aabb 60\nccdd 40").unwrap(),
            vec![
                ShareAllocation {
                    account_id: "aabb".into(),
                    shares: 60
                },
                ShareAllocation {
                    account_id: "ccdd".into(),
                    shares: 40
                },
            ]
        );
        assert!(parse_share_allocations("aabb 60\naabb 40").is_err());
        assert!(parse_share_allocations("not-hex 10").is_err());
        assert!(parse_share_allocations("aabb 0").is_err());
    }

    #[test]
    fn frozen_voting_power_drives_tallies_and_early_settlement() {
        let mut weighted = proposal(VotingRule::ParticipatingMajority { quorum: 4 });
        weighted.electorate = vec![
            VotingPower {
                principal: "aa".into(),
                power: 3,
            },
            VotingPower {
                principal: "bb".into(),
                power: 2,
            },
        ];
        weighted.votes = vec![
            Ballot {
                principal: "aa".into(),
                approve: true,
            },
            Ballot {
                principal: "bb".into(),
                approve: false,
            },
        ];
        assert_eq!(tally(&weighted), (3, 2));
        assert!(can_settle_early(&weighted, 99));
    }

    #[test]
    fn votes_are_gated_by_the_frozen_electorate() {
        let mut state = ready();
        let Resource::Ready(data) = &mut state.data else {
            unreachable!()
        };
        data.proposals[0].electorate = vec![VotingPower {
            principal: "bb".repeat(32),
            power: 1,
        }];
        assert_eq!(
            update(
                &mut state,
                Message::Vote {
                    proposal_id: "signal:1".into(),
                    approve: true
                }
            ),
            None
        );
        let Resource::Ready(data) = &mut state.data else {
            unreachable!()
        };
        data.proposals[0].electorate = vec![VotingPower {
            principal: "aa".repeat(32),
            power: 1,
        }];
        assert_eq!(
            update(
                &mut state,
                Message::Vote {
                    proposal_id: "signal:1".into(),
                    approve: true
                }
            ),
            Some(Command::Vote {
                proposal_id: "signal:1".into(),
                approve: true
            })
        );
    }

    #[test]
    fn upgrade_schedule_requires_a_name_higher_version_and_future_height() {
        assert!(
            validate_schedule(
                &ScheduledUpgrade {
                    name: "forge-v2".into(),
                    to_version: 4,
                    activation_height: 200
                },
                3,
                100
            )
            .is_ok()
        );
        assert!(
            validate_schedule(
                &ScheduledUpgrade {
                    name: "".into(),
                    to_version: 4,
                    activation_height: 200
                },
                3,
                100
            )
            .is_err()
        );
        assert!(
            validate_schedule(
                &ScheduledUpgrade {
                    name: "x".into(),
                    to_version: 3,
                    activation_height: 200
                },
                3,
                100
            )
            .is_err()
        );
        assert!(
            validate_schedule(
                &ScheduledUpgrade {
                    name: "x".into(),
                    to_version: 4,
                    activation_height: 100
                },
                3,
                100
            )
            .is_err()
        );
    }

    #[test]
    fn displayed_share_percentages_are_derived() {
        assert_eq!(format_share_percent(1, 3), "33.33%");
        assert_eq!(format_share_percent(2, 3), "66.67%");
        assert_eq!(format_share_percent(60, 100), "60%");
        assert_eq!(format_share_percent(0, 0), "0%");
    }

    #[test]
    fn proposal_ids_are_uuid_v4_shape() {
        let id = fresh_proposal_id().unwrap();
        assert_eq!(id.len(), 36);
        assert_eq!(&id[14..15], "4");
        assert!(matches!(&id[19..20], "8" | "9" | "a" | "b"));
    }

    #[test]
    fn reloads_coalesce_to_one_follow_up() {
        let mut state = ready();
        assert_eq!(update(&mut state, Message::Refresh), Some(Command::Load));
        assert!(state.loading);
        assert_eq!(update(&mut state, Message::Refresh), None);
        assert!(state.reload_pending);
        let Resource::Ready(data) = ready().data else {
            unreachable!()
        };
        assert_eq!(
            update(
                &mut state,
                Message::Service(ServiceEvent::Loaded(Ok(Some(data))))
            ),
            Some(Command::Load)
        );
        assert!(state.loading);
        assert!(!state.reload_pending);
    }

    #[test]
    fn receipt_finalizes_only_after_the_authoritative_height_catches_up() {
        let mut state = ready();
        assert!(matches!(
            update(
                &mut state,
                Message::Vote {
                    proposal_id: "signal:1".into(),
                    approve: true,
                }
            ),
            Some(Command::Vote { .. })
        ));
        assert_eq!(
            update(
                &mut state,
                Message::Service(ServiceEvent::ActionFinished {
                    proposal_id: "signal:1".into(),
                    result: Ok(SubmitReceipt {
                        height: 101,
                        app_hash: "aa".into(),
                        op_hash: Some("bb".repeat(32)),
                    }),
                })
            ),
            Some(Command::Load)
        );
        assert!(matches!(
            state.operations.get("signal:1"),
            Some(OperationPhase::Receipt { height: 101, .. })
        ));

        let Resource::Ready(mut behind) = ready().data else {
            unreachable!()
        };
        behind.current_height = 100;
        update(
            &mut state,
            Message::Service(ServiceEvent::Loaded(Ok(Some(behind)))),
        );
        assert!(matches!(
            state.operations.get("signal:1"),
            Some(OperationPhase::Receipt { .. })
        ));
        assert_eq!(
            update(
                &mut state,
                Message::Vote {
                    proposal_id: "signal:1".into(),
                    approve: false,
                }
            ),
            None
        );

        assert_eq!(update(&mut state, Message::Refresh), Some(Command::Load));
        let Resource::Ready(mut caught_up) = ready().data else {
            unreachable!()
        };
        caught_up.current_height = 101;
        update(
            &mut state,
            Message::Service(ServiceEvent::Loaded(Ok(Some(caught_up)))),
        );
        assert!(matches!(
            state.operations.get("signal:1"),
            Some(OperationPhase::Finalized { height: 101, .. })
        ));
    }

    #[test]
    fn rejection_reloads_for_rollback_and_keeps_the_row_marked() {
        let mut state = ready();
        update(
            &mut state,
            Message::Vote {
                proposal_id: "signal:1".into(),
                approve: true,
            },
        );
        assert_eq!(
            update(
                &mut state,
                Message::Service(ServiceEvent::ActionFinished {
                    proposal_id: "signal:1".into(),
                    result: Err("not in the frozen electorate".into()),
                })
            ),
            Some(Command::Load)
        );
        assert!(state.busy);
        assert!(matches!(
            state.operations.get("signal:1"),
            Some(OperationPhase::Rejected)
        ));
        let Resource::Ready(data) = ready().data else {
            unreachable!()
        };
        update(
            &mut state,
            Message::Service(ServiceEvent::Loaded(Ok(Some(data)))),
        );
        assert!(!state.busy);
        assert_eq!(state.error.as_deref(), Some("not in the frozen electorate"));
    }

    #[test]
    fn an_identical_ballot_is_deduped_but_a_changed_ballot_is_allowed() {
        let mut state = ready();
        let Resource::Ready(data) = &mut state.data else {
            unreachable!()
        };
        data.proposals[0].votes.push(Ballot {
            principal: "aa".repeat(32),
            approve: true,
        });
        assert_eq!(
            update(
                &mut state,
                Message::Vote {
                    proposal_id: "signal:1".into(),
                    approve: true,
                }
            ),
            None
        );
        assert!(matches!(
            update(
                &mut state,
                Message::Vote {
                    proposal_id: "signal:1".into(),
                    approve: false,
                }
            ),
            Some(Command::Vote { approve: false, .. })
        ));
    }

    #[test]
    fn any_bound_local_node_can_satisfy_a_frozen_validator_electorate() {
        let mut state = ready();
        let Resource::Ready(data) = &mut state.data else {
            unreachable!()
        };
        data.local_nodes.push("bb".repeat(32));
        data.proposals[0].electorate = vec![VotingPower {
            principal: "bb".repeat(32),
            power: 1,
        }];
        assert!(matches!(
            update(
                &mut state,
                Message::Vote {
                    proposal_id: "signal:1".into(),
                    approve: true,
                }
            ),
            Some(Command::Vote { .. })
        ));
    }

    #[test]
    fn share_parser_enforces_the_module_registry_and_safe_integer_caps() {
        assert!(
            parse_share_allocations(&format!("{} {}", "aa".repeat(32), MAX_SAFE_SHARES + 1))
                .is_err()
        );
        let too_many = (0..=MAX_SHARE_ACCOUNTS)
            .map(|index| format!("{index:064x} 1"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(parse_share_allocations(&too_many).is_err());
    }
}
