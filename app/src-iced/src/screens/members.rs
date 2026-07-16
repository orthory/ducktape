//! Native membership directory over the committed validator and resident sets.
//! Presentation stays transport-free and emits typed host commands.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use iced::widget::text::Wrapping;
use iced::widget::{
    Space, button, column, container, image, row, scrollable, text, text_input, tooltip,
};
use iced::{Alignment, Background, Border, Color, Element, Length};

use crate::icons::{self, Icon};
use crate::theme::{
    self, BODY, BODY_LG, CAPTION, HEADING, LABEL, MONO, Palette, RADIUS_LG, RADIUS_MD, RADIUS_SM,
    SANS, SANS_SEMIBOLD, TITLE,
};

const MAX_DISPLAY_NAME_LEN: usize = 64;

/// Genesis-provenance policy copy. Genesis marks the founding cohort; it is a
/// historical fact, not a live governance grant.
const GENESIS_TOOLTIP: &str =
    "Genesis marks a founding member. It records provenance only and confers no governance authority.";

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
    Validators,
    Genesis,
    Local,
}

impl Filter {
    const ALL: [(Self, &'static str); 4] = [
        (Self::All, "All"),
        (Self::Validators, "Validators"),
        (Self::Genesis, "Genesis"),
        (Self::Local, "This Node"),
    ];

    const fn empty_label(self) -> &'static str {
        match self {
            Self::All | Self::Validators => "validators",
            Self::Genesis => "genesis",
            Self::Local => "this node",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    Validator,
    Resident,
}

impl Tier {
    const fn pill(self) -> &'static str {
        match self {
            Self::Validator => "Validator",
            Self::Resident => "Resident",
        }
    }

    const fn status(self) -> &'static str {
        match self {
            Self::Validator => "in validator set",
            Self::Resident => "resident standing",
        }
    }

    const fn kind(self) -> &'static str {
        match self {
            Self::Validator => "validator key",
            Self::Resident => "resident key",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundAccount {
    pub id: String,
    pub name: Option<String>,
    pub device_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provider {
    pub label: String,
    pub models: Vec<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Member {
    pub key: String,
    pub display_name: String,
    pub profile_name: Option<String>,
    pub initials: String,
    pub avatar_bytes: Option<Vec<u8>>,
    pub tier: Tier,
    pub role: String,
    pub is_founder: bool,
    pub is_local: bool,
    pub bound_account: Option<BoundAccount>,
    pub providers: Vec<Provider>,
}

impl fmt::Debug for Member {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Member")
            .field("key", &self.key)
            .field("display_name", &self.display_name)
            .field("profile_name", &self.profile_name)
            .field("initials", &self.initials)
            .field("avatar_bytes", &self.avatar_bytes.as_ref().map(Vec::len))
            .field("tier", &self.tier)
            .field("role", &self.role)
            .field("is_founder", &self.is_founder)
            .field("is_local", &self.is_local)
            .field("bound_account", &self.bound_account)
            .field("providers", &self.providers)
            .finish()
    }
}

impl Member {
    fn normalized_key(&self) -> String {
        normalize_key(&self.key)
    }

    fn matches(&self, filter: Filter, query: &str) -> bool {
        let role = match filter {
            Filter::All => true,
            Filter::Validators => self.tier == Tier::Validator,
            Filter::Genesis => self.is_founder,
            Filter::Local => self.is_local,
        };
        role && (query.is_empty()
            || self.display_name.to_lowercase().contains(query)
            || self.key.to_ascii_lowercase().contains(query)
            || self.role.to_lowercase().contains(query))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinRequest {
    pub joiner: String,
    pub issuer: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MembersData {
    pub members: Vec<Member>,
    pub can_admin: bool,
    pub workspace_role: String,
    pub invite_blob: Option<String>,
    pub invite_short: Option<String>,
    pub pending_joins: Vec<JoinRequest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberAction {
    Demote,
    Promote,
    Revoke,
}

impl MemberAction {
    const fn title(self) -> &'static str {
        match self {
            Self::Demote => "Remove",
            Self::Promote => "Promote",
            Self::Revoke => "Revoke",
        }
    }

    const fn confirm(self) -> &'static str {
        match self {
            Self::Demote => "Remove from validators",
            Self::Promote => "Promote to validator",
            Self::Revoke => "Revoke standing",
        }
    }

    const fn detail(self) -> &'static str {
        match self {
            Self::Demote => {
                "This opens a removal proposal and casts this node's yes ballot. It only takes effect once a strict majority approves."
            }
            Self::Promote => {
                "This opens an AddValidator proposal and casts this node's yes ballot. The pre-synced resident joins quorum at the next epoch cutover after majority approval."
            }
            Self::Revoke => {
                "This opens a RemoveResident proposal and casts this node's yes ballot. After majority approval, the key drops off the mesh at the next epoch cutover and its node parks again."
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingAction {
    pub action: MemberAction,
    pub key: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub data: Resource<MembersData>,
    pub filter: Filter,
    pub query: String,
    pub selected_key: Option<String>,
    pub pending_focus_account: Option<String>,
    pub invitee_code: String,
    pub joiner_key: String,
    pub rename_key: Option<String>,
    pub rename_draft: String,
    pub pending: Option<PendingAction>,
    pub busy: bool,
    /// Which control has a write in flight — a normalized member key for row
    /// actions, or the sentinels `"invite"` / `"admit"` for the admin cards.
    /// Scopes the working state to the acting row instead of the whole surface.
    pub in_flight: Option<String>,
    pub invite_expanded: bool,
    pub error: Option<String>,
    pub copied: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            data: Resource::Loading,
            filter: Filter::All,
            query: String::new(),
            selected_key: None,
            pending_focus_account: None,
            invitee_code: String::new(),
            joiner_key: String::new(),
            rename_key: None,
            rename_draft: String::new(),
            pending: None,
            busy: false,
            in_flight: None,
            invite_expanded: false,
            error: None,
            copied: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Load,
    Refresh,
    SetFilter(Filter),
    SearchChanged(String),
    Select(String),
    CloseDetail,
    FocusAccount(String),
    InviteeCodeChanged(String),
    JoinerKeyChanged(String),
    RevealInvite,
    ToggleInviteFull,
    Copy { id: String, value: String },
    AdmitJoiner,
    ApproveJoin(String),
    BeginRename(String),
    RenameChanged(String),
    CommitRename,
    CancelRename,
    AskAction(MemberAction, String),
    CancelAction,
    ConfirmAction,
    Service(ServiceEvent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Load,
    RevealInvite(String),
    CopyText(String),
    ClearFocus,
    AdmitMember(String),
    SetDisplayName(String),
    DemoteMember(String),
    PromoteMember(String),
    RemoveResident(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceEvent {
    Loaded(Result<Option<MembersData>, String>),
    InviteRevealed(Result<(String, Option<String>), String>),
    ActionFinished(Result<(), String>),
}

pub fn update(state: &mut State, message: Message) -> Option<Command> {
    match message {
        Message::Load => {
            state.data = Resource::Loading;
            Some(Command::Load)
        }
        Message::Refresh => Some(Command::Load),
        Message::SetFilter(filter) => {
            state.filter = filter;
            None
        }
        Message::SearchChanged(value) => {
            state.query = value;
            None
        }
        Message::Select(key) => {
            if member_by_key(state, &key).is_some() {
                state.selected_key = Some(normalize_key(&key));
            }
            None
        }
        Message::CloseDetail => {
            state.selected_key = None;
            None
        }
        Message::FocusAccount(account) => {
            state.pending_focus_account = Some(normalize_key(&account));
            consume_focus(state)
        }
        Message::InviteeCodeChanged(value) => {
            state.invitee_code = value;
            None
        }
        Message::JoinerKeyChanged(value) => {
            state.joiner_key = value;
            None
        }
        Message::RevealInvite => {
            if !can_admin(state) || state.busy {
                return None;
            }
            let key = valid_public_key(&state.invitee_code)?;
            state.busy = true;
            state.in_flight = Some("invite".into());
            state.error = None;
            Some(Command::RevealInvite(key))
        }
        Message::ToggleInviteFull => {
            state.invite_expanded = !state.invite_expanded;
            None
        }
        Message::Copy { id, value } => {
            if value.is_empty() {
                return None;
            }
            state.copied = Some(id);
            Some(Command::CopyText(value))
        }
        Message::AdmitJoiner => {
            if !can_admin(state) || state.busy {
                return None;
            }
            let key = valid_public_key(&state.joiner_key)?;
            state.joiner_key.clear();
            state.busy = true;
            state.in_flight = Some("admit".into());
            state.error = None;
            Some(Command::AdmitMember(key))
        }
        Message::ApproveJoin(key) => {
            if !can_admin(state) || state.busy {
                return None;
            }
            let key = valid_public_key(&key)?;
            state.busy = true;
            state.in_flight = Some(key.clone());
            state.error = None;
            Some(Command::AdmitMember(key))
        }
        Message::BeginRename(key) => {
            let member = member_by_key(state, &key)?;
            if !member.is_local || member.bound_account.is_none() || state.busy {
                return None;
            }
            let draft = member.profile_name.clone().unwrap_or_default();
            let key = member.normalized_key();
            state.rename_draft = draft;
            state.rename_key = Some(key);
            None
        }
        Message::RenameChanged(value) => {
            state.rename_draft = value.chars().take(MAX_DISPLAY_NAME_LEN).collect();
            None
        }
        Message::CommitRename => {
            let key = state.rename_key.clone()?;
            let next = state.rename_draft.trim().to_string();
            let current = member_by_key(state, &key)?;
            if !current.is_local
                || current.bound_account.is_none()
                || next.is_empty()
                || next.chars().count() > MAX_DISPLAY_NAME_LEN
                || current.profile_name.as_deref() == Some(next.as_str())
            {
                state.rename_key = None;
                return None;
            }
            state.rename_key = None;
            state.busy = true;
            state.in_flight = Some(key);
            state.error = None;
            Some(Command::SetDisplayName(next))
        }
        Message::CancelRename => {
            state.rename_key = None;
            state.rename_draft.clear();
            None
        }
        Message::AskAction(action, key) => {
            let member = member_by_key(state, &key)?;
            if action_allowed(state, action, member) {
                state.pending = Some(PendingAction {
                    action,
                    key: member.normalized_key(),
                    display_name: member.display_name.clone(),
                });
            }
            None
        }
        Message::CancelAction => {
            state.pending = None;
            None
        }
        Message::ConfirmAction => {
            let pending = state.pending.take()?;
            let member = member_by_key(state, &pending.key)?;
            if !action_allowed(state, pending.action, member) {
                return None;
            }
            let key = member.key.clone();
            if matches!(pending.action, MemberAction::Demote | MemberAction::Revoke)
                && state.selected_key.as_deref() == Some(pending.key.as_str())
            {
                state.selected_key = None;
            }
            state.busy = true;
            state.in_flight = Some(pending.key.clone());
            state.error = None;
            Some(match pending.action {
                MemberAction::Demote => Command::DemoteMember(key),
                MemberAction::Promote => Command::PromoteMember(key),
                MemberAction::Revoke => Command::RemoveResident(key),
            })
        }
        Message::Service(event) => service_event(state, event),
    }
}

fn service_event(state: &mut State, event: ServiceEvent) -> Option<Command> {
    match event {
        ServiceEvent::Loaded(result) => {
            let revealed = match &state.data {
                Resource::Ready(data) => {
                    Some((data.invite_blob.clone(), data.invite_short.clone()))
                }
                _ => None,
            };
            state.data = match result {
                Ok(Some(mut data)) => {
                    if let Some((invite_blob, invite_short)) = revealed
                        && data.invite_blob.is_none()
                    {
                        data.invite_blob = invite_blob;
                        data.invite_short = invite_short;
                    }
                    Resource::Ready(data)
                }
                Ok(None) => Resource::Empty,
                Err(error) => Resource::Error(error),
            };
            state.busy = false;
            state.in_flight = None;
            consume_focus(state)
        }
        ServiceEvent::InviteRevealed(result) => {
            state.busy = false;
            state.in_flight = None;
            match result {
                Ok((blob, short)) => {
                    if let Resource::Ready(data) = &mut state.data {
                        data.invite_blob = Some(blob);
                        data.invite_short = short;
                    }
                    state.error = None;
                }
                Err(error) => state.error = Some(error),
            }
            None
        }
        ServiceEvent::ActionFinished(result) => {
            state.busy = false;
            state.in_flight = None;
            match result {
                Ok(()) => Some(Command::Load),
                Err(error) => {
                    state.error = Some(error);
                    None
                }
            }
        }
    }
}

fn consume_focus(state: &mut State) -> Option<Command> {
    let account = state.pending_focus_account.as_deref()?;
    let Resource::Ready(data) = &state.data else {
        return None;
    };
    // Identity hydration is independent from the roster. Hold the hand-off
    // until at least one binding exists, just like the original screen.
    if !data
        .members
        .iter()
        .any(|member| member.bound_account.is_some())
    {
        return None;
    }
    let selected = data.members.iter().find_map(|member| {
        member
            .bound_account
            .as_ref()
            .filter(|bound| same_key(&bound.id, account))
            .map(|_| member.normalized_key())
    });
    state.selected_key = selected;
    state.filter = Filter::All;
    state.query.clear();
    state.pending_focus_account = None;
    Some(Command::ClearFocus)
}

fn same_key(left: &str, right: &str) -> bool {
    left.trim().eq_ignore_ascii_case(right.trim())
}

fn member_by_key<'a>(state: &'a State, key: &str) -> Option<&'a Member> {
    let Resource::Ready(data) = &state.data else {
        return None;
    };
    let key = normalize_key(key);
    data.members
        .iter()
        .find(|member| member.normalized_key() == key)
}

fn can_admin(state: &State) -> bool {
    matches!(&state.data, Resource::Ready(data) if data.can_admin)
}

fn action_allowed(state: &State, action: MemberAction, member: &Member) -> bool {
    if !can_admin(state) || state.busy {
        return false;
    }
    match action {
        MemberAction::Demote => member.tier == Tier::Validator && !member.is_local,
        MemberAction::Promote | MemberAction::Revoke => member.tier == Tier::Resident,
    }
}

pub fn view(state: &State, mode: theme::Mode) -> Element<'_, Message> {
    let p = *theme::palette(mode);
    let Resource::Ready(data) = &state.data else {
        return resource_view(&state.data, p);
    };
    let selected = state.selected_key.as_deref().and_then(|key| {
        data.members
            .iter()
            .find(|member| member.normalized_key() == key)
    });
    let main = column![
        header(data, p),
        filter_bar(state, p),
        admin_actions(state, data, p),
        if let Some(pending) = &state.pending {
            confirm_card(pending, p)
        } else {
            Space::new().height(0).into()
        },
        member_list(state, data, p),
    ]
    .width(Length::Fill)
    .height(Length::Fill);
    let mut layout = row![main].height(Length::Fill);
    if let Some(member) = selected {
        layout = layout.push(member_detail(member, state, p));
    }
    container(layout)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| surface(p.canvas))
        .into()
}

fn resource_view(resource: &Resource<MembersData>, p: Palette) -> Element<'_, Message> {
    let (title, detail, retry) = match resource {
        Resource::Loading => (
            "Loading members",
            "Reading the committed validator set…",
            false,
        ),
        Resource::Empty => (
            "No validators to show",
            "This view only lists keys reported by the valset module.",
            true,
        ),
        Resource::Error(error) => ("Members unavailable", error.as_str(), true),
        Resource::Ready(_) => unreachable!(),
    };
    let mut body = column![
        icons::view(Icon::Members, 26.0, p.icon_idle),
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

fn header(data: &MembersData, p: Palette) -> Element<'static, Message> {
    let role_pill = pill(
        data.workspace_role.clone(),
        if data.can_admin { p.green } else { p.amber },
        p,
    );
    let role_pill = if data.workspace_role.eq_ignore_ascii_case("genesis") {
        tip(role_pill, GENESIS_TOOLTIP, p)
    } else {
        role_pill
    };
    container(
        row![
            text("Members")
                .font(SANS_SEMIBOLD)
                .size(HEADING)
                .color(p.ink),
            text(data.members.len())
                .font(MONO)
                .size(LABEL)
                .color(p.muted_2),
            Space::new().width(Length::Fill),
            role_pill,
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
    let mut filters = row![].spacing(7).align_y(Alignment::Center);
    for (filter, label) in Filter::ALL {
        filters = filters.push(segment_button(
            label,
            state.filter == filter,
            Message::SetFilter(filter),
            p,
        ));
    }
    container(
        row![
            filters,
            Space::new().width(Length::Fill),
            container(sem_input(
                "Search",
                &state.query,
                text_input("Search name or key…", &state.query)
                    .on_input(Message::SearchChanged)
                    .font(SANS)
                    .size(BODY)
                    .padding([7, 10])
                    .width(Length::Fill),
            ))
            .width(Length::Fill)
            .max_width(260),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .padding([12, 22])
    .style(move |_| ruled_surface(p.paper, p.border_soft))
    .into()
}

fn admin_actions<'a>(state: &'a State, data: &'a MembersData, p: Palette) -> Element<'a, Message> {
    let mut body = column![
        row![
            icons::view(Icon::Members, 13.0, p.muted_2),
            text("ADMIN ACTIONS")
                .font(SANS_SEMIBOLD)
                .size(CAPTION)
                .color(p.muted_2),
        ]
        .spacing(7)
        .align_y(Alignment::Center)
    ]
    .spacing(9);
    if !data.can_admin {
        body = body.push(notice(
            "Invite and admission controls require an admitted workspace.",
            p,
        ));
    } else {
        if !data.pending_joins.is_empty() {
            body = body.push(pending_joins(data, state.in_flight.as_deref(), p));
        }
        body = body.push(
            row![invite_card(state, data, p), admit_card(state, p)]
                .spacing(10)
                .align_y(Alignment::Start),
        );
    }
    if let Some(error) = &state.error {
        body = body.push(selectable_error(error, p));
    }
    container(body)
        .width(Length::Fill)
        .padding([13, 22])
        .style(move |_| ruled_surface(p.paper, p.border_soft))
        .into()
}

fn pending_joins(
    data: &MembersData,
    in_flight: Option<&str>,
    p: Palette,
) -> Element<'static, Message> {
    let mut rows = column![
        row![
            text("Joining Nodes")
                .font(SANS_SEMIBOLD)
                .size(TITLE)
                .color(p.ink_soft),
            text(data.pending_joins.len())
                .font(MONO)
                .size(LABEL)
                .color(p.muted_2),
            Space::new().width(Length::Fill),
            text("Invites redeem automatically; Approve forces the ballot manually.")
                .font(SANS)
                .size(LABEL)
                .color(p.muted_2),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
    ];
    for request in &data.pending_joins {
        let working = in_flight == Some(normalize_key(&request.joiner).as_str());
        let action: Element<'static, Message> = if working {
            working_pill("Approving…", p)
        } else {
            filled_button(
                "Approve",
                Message::ApproveJoin(request.joiner.clone()),
                in_flight.is_none(),
                p,
            )
        };
        rows = rows.push(
            container(
                row![
                    column![
                        text(short_key(&request.joiner))
                            .font(MONO)
                            .size(LABEL)
                            .color(p.ink),
                        text(format!("invited by {}", short_key(&request.issuer)))
                            .font(SANS)
                            .size(CAPTION)
                            .color(p.muted_2),
                    ]
                    .spacing(1),
                    Space::new().width(Length::Fill),
                    action,
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            )
            .padding([9, 13])
            .style(move |_| ruled_surface(p.paper, p.border_soft)),
        );
    }
    container(rows)
        .width(Length::Fill)
        .style(move |_| rounded_surface(p.paper, p.border_strong, RADIUS_LG))
        .into()
}

fn invite_card<'a>(state: &'a State, data: &'a MembersData, p: Palette) -> Element<'a, Message> {
    let valid = valid_public_key(&state.invitee_code).is_some();
    let revealing = state.in_flight.as_deref() == Some("invite");
    let reveal: Element<'a, Message> = if revealing {
        working_pill("Revealing…", p)
    } else {
        filled_button(
            if data.invite_blob.is_some() {
                "Refresh invite"
            } else {
                "Reveal invite"
            },
            Message::RevealInvite,
            valid && !state.busy,
            p,
        )
    };
    let mut body = column![
        text("Invite a member")
            .font(SANS_SEMIBOLD)
            .size(TITLE)
            .color(p.ink_soft),
        text("Paste the invitee's 64-hex join code, then reveal a fresh invite locked to that person.")
            .font(SANS)
            .size(LABEL)
            .color(p.muted_2),
        row![
            sem_input(
                "invitee join code (64 hex)",
                &state.invitee_code,
                text_input("invitee join code (64 hex)", &state.invitee_code)
                    .on_input(Message::InviteeCodeChanged)
                    .font(MONO)
                    .size(LABEL)
                    .padding([8, 9])
                    .width(Length::Fill),
            ),
            reveal,
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    ]
    .spacing(9);
    if !state.invitee_code.trim().is_empty() && !valid {
        body = body.push(
            text("A join code is 64 hex characters.")
                .font(SANS)
                .size(CAPTION)
                .color(p.muted_2),
        );
    }
    if let Some(blob) = &data.invite_blob {
        // Short link (when the coordinator minted one) is the primary, quiet
        // value; the full blob lives behind a disclosure that works offline.
        let primary = data.invite_short.as_deref().unwrap_or(blob.as_str());
        let has_short = data.invite_short.is_some();
        let mut primary_actions = row![Space::new().width(Length::Fill)]
            .spacing(7)
            .align_y(Alignment::Center);
        if state.copied.as_deref() == Some("invite") {
            primary_actions = primary_actions
                .push(text("Copied").font(SANS).size(CAPTION).color(p.green));
        }
        primary_actions = primary_actions.push(outline_button(
            if has_short { "Copy link" } else { "Copy invite" },
            Message::Copy {
                id: "invite".into(),
                value: primary.to_string(),
            },
            true,
            p,
        ));
        let mut block = column![
            text(primary.to_string())
                .font(MONO)
                .size(LABEL)
                .color(p.ink_soft)
                .wrapping(Wrapping::WordOrGlyph)
                .width(Length::Fill),
            text(if has_short {
                "One person, expires in 7 days. The full invite below works even if the coordinator restarts."
            } else {
                "Coordinator-free workspace invite."
            })
            .font(SANS)
            .size(CAPTION)
            .color(p.muted_2),
            primary_actions,
        ]
        .spacing(7);
        if has_short {
            block = block.push(
                bare_button(
                    if state.invite_expanded {
                        "Hide full invite"
                    } else {
                        "Full invite — works without the coordinator"
                    },
                    Message::ToggleInviteFull,
                    p,
                ),
            );
            if state.invite_expanded {
                let mut full_actions = row![Space::new().width(Length::Fill)]
                    .spacing(7)
                    .align_y(Alignment::Center);
                if state.copied.as_deref() == Some("invite-full") {
                    full_actions = full_actions
                        .push(text("Copied").font(SANS).size(CAPTION).color(p.green));
                }
                full_actions = full_actions.push(outline_button(
                    "Copy full invite",
                    Message::Copy {
                        id: "invite-full".into(),
                        value: blob.clone(),
                    },
                    true,
                    p,
                ));
                block = block.push(
                    container(
                        column![
                            text(blob.clone())
                                .font(MONO)
                                .size(LABEL)
                                .color(p.ink_soft)
                                .wrapping(Wrapping::WordOrGlyph)
                                .width(Length::Fill),
                            full_actions,
                        ]
                        .spacing(7),
                    )
                    .padding([8, 10])
                    .style(move |_| rounded_surface(p.sunken, p.border_soft, RADIUS_SM)),
                );
            }
        }
        body = body.push(
            container(block)
                .padding([10, 12])
                .style(move |_| rounded_surface(p.paper, p.border_soft, RADIUS_SM)),
        );
    }
    card(body, p)
}

fn admit_card(state: &State, p: Palette) -> Element<'_, Message> {
    let valid = valid_public_key(&state.joiner_key).is_some();
    let admitting = state.in_flight.as_deref() == Some("admit");
    let admit: Element<'_, Message> = if admitting {
        working_pill("Admitting…", p)
    } else {
        outline_button("Admit", Message::AdmitJoiner, valid && !state.busy, p)
    };
    card(
        column![
            text("Admit a joiner")
                .font(SANS_SEMIBOLD)
                .size(TITLE)
                .color(p.ink_soft),
            text("Promote a parked workspace by its public key.")
                .font(SANS)
                .size(LABEL)
                .color(p.muted_2),
            row![
                sem_input(
                    "Paste joiner public key…",
                    &state.joiner_key,
                    text_input("Paste joiner public key…", &state.joiner_key)
                        .on_input(Message::JoinerKeyChanged)
                        .font(MONO)
                        .size(LABEL)
                        .padding([8, 9])
                        .width(Length::Fill),
                ),
                admit,
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(9),
        p,
    )
}

fn member_list<'a>(state: &'a State, data: &'a MembersData, p: Palette) -> Element<'a, Message> {
    let query = state.query.trim().to_lowercase();
    let visible: Vec<&Member> = data
        .members
        .iter()
        .filter(|member| member.matches(state.filter, &query))
        .collect();
    if visible.is_empty() {
        return container(
            column![
                icons::view(Icon::Members, 26.0, p.icon_idle),
                text(format!("No {} to show.", state.filter.empty_label()))
                    .font(SANS)
                    .size(TITLE)
                    .color(p.muted_2),
                text("This view only lists keys reported by the valset module.")
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

    let mut list = column![].spacing(0);
    for item in group_members(&visible) {
        match item {
            Grouped::Single(member) => list = list.push(member_row(state, member, false, p)),
            Grouped::Group { name, members } => {
                list = list.push(
                    container(
                        row![
                            text(name).font(SANS_SEMIBOLD).size(LABEL).color(p.ink_soft),
                            text(format!(
                                "{} device{}",
                                members.len(),
                                if members.len() == 1 { "" } else { "s" }
                            ))
                            .font(MONO)
                            .size(CAPTION)
                            .color(p.muted_2),
                        ]
                        .spacing(8),
                    )
                    .padding([6, 14]),
                );
                for member in members {
                    list =
                        list.push(container(member_row(state, member, true, p)).padding([0, 14]));
                }
            }
        }
    }
    scrollable(
        container(
            container(list).style(move |_| rounded_surface(p.paper, p.border_soft, RADIUS_MD)),
        )
        .padding([6, 12])
        .width(Length::Fill),
    )
    .height(Length::Fill)
    .into()
}

enum Grouped<'a> {
    Single(&'a Member),
    Group {
        name: String,
        members: Vec<&'a Member>,
    },
}

fn group_members<'a>(members: &[&'a Member]) -> Vec<Grouped<'a>> {
    let mut counts = BTreeMap::<(Tier, String), usize>::new();
    for member in members {
        if let Some(account) = &member.bound_account {
            *counts
                .entry((member.tier, account.id.to_ascii_lowercase()))
                .or_default() += 1;
        }
    }
    let mut output = Vec::new();
    let mut emitted = BTreeSet::new();
    for member in members {
        let Some(account) = &member.bound_account else {
            output.push(Grouped::Single(member));
            continue;
        };
        let key = (member.tier, account.id.to_ascii_lowercase());
        if counts.get(&key).copied().unwrap_or_default() < 2 {
            output.push(Grouped::Single(member));
        } else if emitted.insert(key.clone()) {
            output.push(Grouped::Group {
                name: account
                    .name
                    .clone()
                    .unwrap_or_else(|| short_key(&account.id)),
                members: members
                    .iter()
                    .copied()
                    .filter(|candidate| {
                        candidate.tier == key.0
                            && candidate
                                .bound_account
                                .as_ref()
                                .is_some_and(|bound| bound.id.eq_ignore_ascii_case(&key.1))
                    })
                    .collect(),
            });
        }
    }
    output
}

fn member_row<'a>(
    state: &'a State,
    member: &'a Member,
    device_row: bool,
    p: Palette,
) -> Element<'a, Message> {
    let key = member.normalized_key();
    if state.rename_key.as_deref() == Some(key.as_str()) {
        let rename_placeholder = short_key(&member.key);
        return container(
            row![
                avatar(member, 32.0, p),
                sem_input(
                    "device key",
                    &state.rename_draft,
                    text_input(&rename_placeholder, &state.rename_draft)
                        .on_input(Message::RenameChanged)
                        .on_submit(Message::CommitRename)
                        .font(SANS)
                        .size(BODY)
                        .padding([7, 10])
                        .width(Length::Fill),
                ),
                filled_button("Save", Message::CommitRename, true, p),
                bare_button("×", Message::CancelRename, p),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .padding([10, 14])
        .style(move |_| ruled_surface(p.sunken, p.border_soft))
        .into();
    }

    let selected = state.selected_key.as_deref() == Some(key.as_str());
    let mut labels = row![].spacing(7).align_y(Alignment::Center);
    if member.is_founder {
        labels = labels.push(tip(pill("Genesis", p.ink, p), GENESIS_TOOLTIP, p));
    }
    labels = labels.push(pill(
        member.tier.pill(),
        if member.tier == Tier::Validator {
            p.green
        } else {
            p.amber
        },
        p,
    ));
    let mut name_line = row![
        text(if device_row {
            short_key(&member.key)
        } else {
            member.display_name.clone()
        })
        .font(SANS_SEMIBOLD)
        .size(BODY_LG)
        .color(p.ink),
    ]
    .spacing(7)
    .align_y(Alignment::Center);
    if member.is_local {
        name_line = name_line.push(pill("this node", p.blue, p));
    }
    let open = button(
        row![
            avatar(member, 32.0, p),
            column![
                name_line,
                text(format!(
                    "{} · {}",
                    short_key(&member.key),
                    member.tier.status()
                ))
                .font(MONO)
                .size(LABEL)
                .color(p.muted_2),
                provider_chips(member, p),
            ]
            .spacing(3)
            .width(Length::Fill),
            labels,
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([12, 14])
    .style(move |_, status| iced::widget::button::Style {
        background: Some(Background::Color(
            if matches!(status, iced::widget::button::Status::Hovered) {
                p.sidebar
            } else if selected {
                p.sunken
            } else {
                p.paper
            },
        )),
        text_color: p.ink,
        border: Border::default(),
        ..Default::default()
    })
    .on_press(Message::Select(member.key.clone()));
    #[cfg(all(feature = "agent", debug_assertions))]
    let open = iced_agent_plugin::sem(
        iced_agent_plugin::Role::ListItem,
        member.display_name.clone(),
        open,
    );

    let mut line = row![open].align_y(Alignment::Center);
    if state.in_flight.as_deref() == Some(key.as_str()) {
        // Only the acting row shows a working state; the rest stay live (M1).
        line = line.push(container(working_pill("Working…", p)).padding([0, 12]));
    } else {
        if member.is_local && member.bound_account.is_some() {
            line = line.push(
                container(bare_button(
                    "Rename",
                    Message::BeginRename(member.key.clone()),
                    p,
                ))
                .padding([0, 12]),
            );
        }
        // AskAction only opens the confirm card, so it stays enabled regardless
        // of an unrelated in-flight write; the submit itself is still serialized.
        if can_admin(state) && member.tier == Tier::Resident {
            line = line
                .push(filled_button(
                    "Promote",
                    Message::AskAction(MemberAction::Promote, member.key.clone()),
                    true,
                    p,
                ))
                .push(
                    container(outline_button(
                        "Revoke",
                        Message::AskAction(MemberAction::Revoke, member.key.clone()),
                        true,
                        p,
                    ))
                    .padding([0, 12]),
                );
        } else if can_admin(state) && member.tier == Tier::Validator && !member.is_local {
            line = line.push(
                container(outline_button(
                    "Remove",
                    Message::AskAction(MemberAction::Demote, member.key.clone()),
                    true,
                    p,
                ))
                .padding([0, 12]),
            );
        }
    }
    container(line)
        .width(Length::Fill)
        .style(move |_| ruled_surface(p.paper, p.border_soft))
        .into()
}

fn provider_chips(member: &Member, p: Palette) -> Element<'static, Message> {
    if member.providers.is_empty() {
        return Space::new().height(0).into();
    }
    let mut chips = row![].spacing(6).align_y(Alignment::Center);
    for provider in &member.providers {
        let mut chip = row![
            text(provider.label.clone())
                .font(SANS)
                .size(CAPTION)
                .color(p.ink_soft),
        ]
        .spacing(5)
        .align_y(Alignment::Center);
        if provider.models.len() > 1 {
            chip = chip.push(
                container(
                    text(provider.models.len().to_string())
                        .font(MONO)
                        .size(CAPTION)
                        .color(p.muted_2),
                )
                .padding([0, 4])
                .style(move |_| rounded_surface(p.sunken, p.border_soft, RADIUS_SM)),
            );
        }
        let chip = container(chip)
            .padding([2, 7])
            .style(move |_| rounded_surface(p.paper, p.border_soft, RADIUS_SM));
        let models = if provider.models.is_empty() {
            "default executor".to_string()
        } else {
            provider.models.join("\n")
        };
        chips = chips.push(tip(chip, models, p));
    }
    chips.wrap().into()
}

fn member_detail(member: &Member, state: &State, p: Palette) -> Element<'static, Message> {
    let header = container(
        row![
            text("Member")
                .font(SANS_SEMIBOLD)
                .size(TITLE)
                .color(p.ink),
            Space::new().width(Length::Fill),
            bare_button("×", Message::CloseDetail, p),
        ]
        .align_y(Alignment::Center),
    )
    .height(56)
    .padding([0, 16])
    .align_y(Alignment::Center)
    .style(move |_| ruled_surface(p.sidebar, p.border_soft));

    // 1. Identity — the only centered block (avatar, name, standing pills).
    let mut status = row![pill(
        if member.tier == Tier::Resident {
            "Resident standing"
        } else {
            "In validator set"
        },
        if member.tier == Tier::Resident {
            p.amber
        } else {
            p.green
        },
        p,
    )]
    .spacing(6);
    if member.is_founder {
        status = status.push(tip(pill("Genesis", p.ink, p), GENESIS_TOOLTIP, p));
    }
    let identity = column![
        avatar(member, 54.0, p),
        text(member.display_name.clone())
            .font(SANS_SEMIBOLD)
            .size(HEADING)
            .color(p.ink)
            .wrapping(Wrapping::WordOrGlyph),
        status,
    ]
    .spacing(8)
    .align_x(Alignment::Center);

    // 2. Info grid — left-aligned; long hex values wrap inside the pane (B1).
    let info = column![
        info_row(
            "profile",
            member.profile_name.as_deref().unwrap_or("not available"),
            None,
            false,
            p
        ),
        info_row(
            "public key",
            &member.key,
            Some(Message::Copy {
                id: "public-key".into(),
                value: member.key.clone()
            }),
            state.copied.as_deref() == Some("public-key"),
            p
        ),
        info_row("short key", &short_key(&member.key), None, false, p),
        info_row("role", &member.role, None, false, p),
        info_row("kind", member.tier.kind(), None, false, p),
        info_row("status", member.tier.status(), None, false, p),
        info_row(
            "genesis",
            if member.is_founder { "yes" } else { "no" },
            None,
            false,
            p
        ),
        info_row(
            "this node",
            if member.is_local { "yes" } else { "no" },
            None,
            false,
            p
        ),
    ]
    .spacing(8);

    // 3. Runs-on — left-aligned executor list.
    let mut runs_on = column![text("RUNS ON")
        .font(SANS_SEMIBOLD)
        .size(CAPTION)
        .color(p.muted_2)]
    .spacing(8);
    if member.providers.is_empty() {
        runs_on = runs_on.push(
            text("No executors announced by this node.")
                .font(SANS)
                .size(LABEL)
                .color(p.muted_2),
        );
    } else {
        for provider in &member.providers {
            runs_on = runs_on.push(
                column![
                    text(provider.label.clone())
                        .font(SANS)
                        .size(BODY)
                        .color(p.ink_soft),
                    text(if provider.models.is_empty() {
                        "default executor".into()
                    } else {
                        provider.models.join(" · ")
                    })
                    .font(MONO)
                    .size(LABEL)
                    .color(p.muted_3)
                    .wrapping(Wrapping::WordOrGlyph)
                    .width(Length::Fill),
                ]
                .spacing(4),
            );
        }
    }

    let body = column![identity, info, runs_on].spacing(18);
    container(column![
        header,
        scrollable(container(body).padding([18, 16]))
    ])
    .width(332)
    .height(Length::Fill)
    .style(move |_| ruled_surface(p.sidebar, p.border_soft))
    .into()
}

fn confirm_card(pending: &PendingAction, p: Palette) -> Element<'static, Message> {
    container(
        row![
            column![
                text(format!(
                    "{} {}?",
                    pending.action.title(),
                    pending.display_name
                ))
                .font(SANS_SEMIBOLD)
                .size(BODY_LG)
                .color(p.ink),
                text(pending.action.detail())
                    .font(SANS)
                    .size(BODY)
                    .color(p.muted_3),
            ]
            .spacing(5)
            .width(Length::Fill),
            outline_button("Cancel", Message::CancelAction, true, p),
            if pending.action == MemberAction::Promote {
                filled_button(pending.action.confirm(), Message::ConfirmAction, true, p)
            } else {
                danger_button(pending.action.confirm(), Message::ConfirmAction, true, p)
            },
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([12, 22])
    .style(move |_| ruled_surface(p.paper, p.border_soft))
    .into()
}

fn info_row(
    label: &str,
    value: &str,
    action: Option<Message>,
    copied: bool,
    p: Palette,
) -> Element<'static, Message> {
    let mut value_row = row![
        text(value.to_string())
            .font(MONO)
            .size(LABEL)
            .color(p.ink_soft)
            .wrapping(Wrapping::WordOrGlyph)
            .width(Length::Fill),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    if copied {
        value_row = value_row.push(text("Copied").font(SANS).size(CAPTION).color(p.green));
    }
    if let Some(message) = action {
        value_row = value_row.push(outline_button("Copy", message, true, p));
    }
    container(
        row![
            text(label.to_string())
                .font(MONO)
                .size(CAPTION)
                .color(p.muted_2)
                .width(82),
            value_row,
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([9, 11])
    .style(move |_| rounded_surface(p.paper, p.border, RADIUS_SM))
    .into()
}

fn avatar(member: &Member, size: f32, p: Palette) -> Element<'static, Message> {
    if let Some(bytes) = &member.avatar_bytes {
        return container(
            image(iced::widget::image::Handle::from_bytes(bytes.clone()))
                .content_fit(iced::ContentFit::Cover)
                .border_radius(999.0)
                .width(size)
                .height(size),
        )
        .width(size)
        .height(size)
        .clip(true)
        .style(move |_| rounded_surface(p.sunken, p.border, 999.0))
        .into();
    }
    // Founder → filled graphite; local node → accent tint; else a quiet chip.
    let (bg, fg) = if member.is_founder {
        (p.filled, p.on_filled)
    } else if member.is_local {
        (p.chip, p.ink)
    } else {
        (p.sunken, p.ink_soft)
    };
    container(
        text(if member.initials.is_empty() {
            "?".into()
        } else {
            member.initials.clone()
        })
        .font(SANS_SEMIBOLD)
        .size(size * 0.34)
        .color(fg),
    )
    .width(size)
    .height(size)
    .center(Length::Fill)
    .style(move |_| rounded_surface(bg, p.border, 999.0))
    .into()
}

fn pill(label: impl ToString, tone: Color, p: Palette) -> Element<'static, Message> {
    container(text(label.to_string()).font(SANS).size(CAPTION).color(tone))
        .padding([3, 8])
        .style(move |_| rounded_surface(p.paper, tone, RADIUS_SM))
        .into()
}

/// In-flight marker shown in place of a row's action button while its own write
/// is pending — scopes the "working" state to the acting row (M1).
fn working_pill(label: &'static str, p: Palette) -> Element<'static, Message> {
    container(text(label).font(SANS).size(LABEL).color(p.amber))
        .padding([7, 12])
        .style(move |_| rounded_surface(p.sunken, p.border_soft, RADIUS_SM))
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

fn card<'a>(body: impl Into<Element<'a, Message>>, p: Palette) -> Element<'a, Message> {
    container(body)
        .width(Length::Fill)
        .padding([12, 13])
        .style(move |_| rounded_surface(p.sunken, p.border, RADIUS_LG))
        .into()
}

/// Native hover tooltip carrying policy copy or a full value the row truncates.
fn tip<'a>(
    control: impl Into<Element<'a, Message>>,
    label: impl ToString,
    p: Palette,
) -> Element<'a, Message> {
    tooltip(
        control,
        container(
            text(label.to_string())
                .font(SANS)
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

/// A read-only `text_input` — focusable and selectable so a member can copy an
/// error, but never editable. Mirrors the `workspace.rs` `selectable_error`.
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

fn bare_button<'a>(label: &'a str, message: Message, p: Palette) -> Element<'a, Message> {
    let btn = button(text(label).font(SANS).size(LABEL).color(p.muted_3))
        .padding(6)
        .style(|_, _| iced::widget::button::Style::default())
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

fn danger_button<'a>(
    label: &'a str,
    message: Message,
    enabled: bool,
    p: Palette,
) -> Element<'a, Message> {
    let control = button(text(label).font(SANS).size(BODY))
        .padding([7, 12])
        .style(move |_, _| iced::widget::button::Style {
            background: Some(Background::Color(if enabled { p.danger } else { p.sunken })),
            text_color: if enabled { p.paper } else { p.muted_2 },
            border: Border {
                radius: RADIUS_SM.into(),
                ..Default::default()
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

fn valid_public_key(value: &str) -> Option<String> {
    let normalized = normalize_key(value);
    (normalized.len() == 64 && normalized.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then_some(normalized)
}

fn normalize_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn short_key(value: &str) -> String {
    let value = value.trim();
    if value.len() <= 16 {
        value.into()
    } else {
        format!("{}…{}", &value[..8], &value[value.len() - 8..])
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

    const LOCAL: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const PEER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const RESIDENT: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";

    fn member(key: &str, tier: Tier, local: bool) -> Member {
        Member {
            key: key.into(),
            display_name: if local { "Founder Rae" } else { "Peer" }.into(),
            profile_name: Some(if local { "Founder Rae" } else { "Peer" }.into()),
            initials: if local { "FR" } else { "PE" }.into(),
            avatar_bytes: None,
            tier,
            role: if local {
                "genesis validator"
            } else {
                tier.status()
            }
            .into(),
            is_founder: local,
            is_local: local,
            bound_account: local.then(|| BoundAccount {
                id: "11".repeat(32),
                name: Some("Rae".into()),
                device_label: None,
            }),
            providers: Vec::new(),
        }
    }

    fn ready() -> State {
        State {
            data: Resource::Ready(MembersData {
                members: vec![
                    member(LOCAL, Tier::Validator, true),
                    member(PEER, Tier::Validator, false),
                    member(RESIDENT, Tier::Resident, false),
                ],
                can_admin: true,
                workspace_role: "Genesis validator".into(),
                invite_blob: None,
                invite_short: None,
                pending_joins: Vec::new(),
            }),
            ..State::default()
        }
    }

    #[test]
    fn service_results_preserve_loading_empty_error_and_populated_states() {
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

        let Resource::Ready(mut data) = ready().data else {
            unreachable!()
        };
        data.members.clear();
        update(
            &mut state,
            Message::Service(ServiceEvent::Loaded(Ok(Some(data)))),
        );
        assert!(matches!(state.data, Resource::Ready(_)));
    }

    #[test]
    fn background_refresh_keeps_the_revealed_targeted_invite() {
        let mut state = ready();
        update(
            &mut state,
            Message::Service(ServiceEvent::InviteRevealed(Ok((
                "full".into(),
                Some("duck://short".into()),
            )))),
        );
        let Resource::Ready(data) = ready().data else {
            unreachable!()
        };
        update(
            &mut state,
            Message::Service(ServiceEvent::Loaded(Ok(Some(data)))),
        );
        let Resource::Ready(data) = state.data else {
            unreachable!()
        };
        assert_eq!(data.invite_blob.as_deref(), Some("full"));
        assert_eq!(data.invite_short.as_deref(), Some("duck://short"));
    }

    #[test]
    fn invites_and_admission_require_an_admin_and_exact_public_key() {
        let mut state = ready();
        update(&mut state, Message::InviteeCodeChanged("bad".into()));
        assert_eq!(update(&mut state, Message::RevealInvite), None);
        update(&mut state, Message::InviteeCodeChanged(format!(" {PEER} ")));
        assert_eq!(
            update(&mut state, Message::RevealInvite),
            Some(Command::RevealInvite(PEER.into()))
        );

        state.busy = false;
        let Resource::Ready(data) = &mut state.data else {
            unreachable!()
        };
        data.can_admin = false;
        update(&mut state, Message::JoinerKeyChanged(RESIDENT.into()));
        assert_eq!(update(&mut state, Message::AdmitJoiner), None);
    }

    #[test]
    fn destructive_changes_require_confirmation_and_never_target_self() {
        let mut state = ready();
        update(
            &mut state,
            Message::AskAction(MemberAction::Demote, LOCAL.into()),
        );
        assert_eq!(state.pending, None);
        update(
            &mut state,
            Message::AskAction(MemberAction::Demote, PEER.into()),
        );
        assert_eq!(
            state.pending.as_ref().map(|item| item.action),
            Some(MemberAction::Demote)
        );
        update(&mut state, Message::CancelAction);
        assert_eq!(state.pending, None);
        update(
            &mut state,
            Message::AskAction(MemberAction::Demote, PEER.into()),
        );
        assert_eq!(
            update(&mut state, Message::ConfirmAction),
            Some(Command::DemoteMember(PEER.into()))
        );
    }

    #[test]
    fn resident_promote_and_revoke_are_distinct() {
        let mut state = ready();
        update(
            &mut state,
            Message::AskAction(MemberAction::Promote, RESIDENT.into()),
        );
        assert_eq!(
            update(&mut state, Message::ConfirmAction),
            Some(Command::PromoteMember(RESIDENT.into()))
        );
        state.busy = false;
        update(
            &mut state,
            Message::AskAction(MemberAction::Revoke, RESIDENT.into()),
        );
        assert_eq!(
            update(&mut state, Message::ConfirmAction),
            Some(Command::RemoveResident(RESIDENT.into()))
        );
    }

    #[test]
    fn only_the_bound_local_row_can_rename_and_names_are_trimmed() {
        let mut state = ready();
        update(&mut state, Message::BeginRename(PEER.into()));
        assert_eq!(state.rename_key, None);
        update(&mut state, Message::BeginRename(LOCAL.into()));
        update(
            &mut state,
            Message::RenameChanged("  Rae the Founder  ".into()),
        );
        assert_eq!(
            update(&mut state, Message::CommitRename),
            Some(Command::SetDisplayName("Rae the Founder".into()))
        );
    }

    #[test]
    fn filtering_searches_names_keys_and_roles() {
        let Resource::Ready(data) = ready().data else {
            unreachable!()
        };
        assert!(data.members[0].matches(Filter::Genesis, "founder"));
        assert!(!data.members[2].matches(Filter::Validators, ""));
        assert!(data.members[2].matches(Filter::All, "dddd"));
    }

    #[test]
    fn account_focus_waits_for_bindings_then_selects_a_node_and_clears() {
        let mut state = State::default();
        assert_eq!(
            update(&mut state, Message::FocusAccount("11".repeat(32))),
            None
        );
        assert!(state.pending_focus_account.is_some());
        let Resource::Ready(data) = ready().data else {
            unreachable!()
        };
        assert_eq!(
            update(
                &mut state,
                Message::Service(ServiceEvent::Loaded(Ok(Some(data))))
            ),
            Some(Command::ClearFocus)
        );
        assert_eq!(state.selected_key.as_deref(), Some(LOCAL));
        assert_eq!(state.pending_focus_account, None);
    }
}
