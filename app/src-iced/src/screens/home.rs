//! Home account, workspace, device, and enrollment surface.

use iced::widget::{
    Column, Space, Svg, button, column, container, image, row, scrollable, svg, text,
};
use iced::{Alignment, Background, Border, Color, Element, Length};
use qrcode::{QrCode, types::Color as QrColor};
use zeroize::Zeroize as _;

use crate::icons::Icon;
use crate::theme::{BODY, CAPTION, LABEL, MONO, Palette, RADIUS_MD, RADIUS_SM, SANS};
use crate::view_api::{MemberKeyKind, Resource, decode_link_response, encode_link_response};

use super::user::{
    Command, Message, Screen, avatar, bottom_border, card, center_state, danger_outline, divider,
    error_banner, error_state, field, field_enabled, filled, icon_tile, nonempty, notice,
    notice_owned, outline, outline_enabled, plain_input, plain_input_enabled, section_header,
    section_label, surface,
};

const HOME_PAD: f32 = 22.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    Validator,
    Resident,
    NoSeat,
}

impl Standing {
    const fn label(self) -> &'static str {
        match self {
            Self::Validator => "Validator",
            Self::Resident => "Resident",
            Self::NoSeat => "No seat",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AccountProfile {
    pub display_name: String,
    pub account_id: String,
    pub duck_name: Option<String>,
    pub avatar: Option<String>,
    pub avatar_bytes: Option<Vec<u8>>,
    pub bio: Option<String>,
}

impl std::fmt::Debug for AccountProfile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AccountProfile")
            .field("display_name", &self.display_name)
            .field("account_id", &self.account_id)
            .field("duck_name", &self.duck_name)
            .field("avatar", &self.avatar)
            .field(
                "avatar_bytes",
                &self.avatar_bytes.as_ref().map(|bytes| bytes.len()),
            )
            .field("bio", &self.bio)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AvatarDraft {
    pub mime: String,
    pub bytes: Vec<u8>,
}

impl std::fmt::Debug for AvatarDraft {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AvatarDraft")
            .field("mime", &self.mime)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum AvatarEdit {
    #[default]
    Keep,
    Remove,
    Replace(AvatarDraft),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceRow {
    pub id: String,
    pub name: String,
    pub network_id: String,
    pub standing: Standing,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceRow {
    pub key: String,
    pub label: String,
    pub standing: Standing,
    pub this_device: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceNetworkGroup {
    pub network_id: String,
    pub name: String,
    pub active: bool,
    pub at_ms: u64,
    pub devices: Vec<DeviceRow>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountKeyKind {
    Ed25519,
    P256,
    WebauthnP256,
}

impl AccountKeyKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Ed25519 => "Seed key",
            Self::P256 => "Security key",
            Self::WebauthnP256 => "Passkey",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberKeyRow {
    pub key: String,
    pub kind: AccountKeyKind,
    pub label: Option<String>,
    pub this_device: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkChallengeView {
    pub chain_id: String,
    pub account_id: String,
    pub nonce: u64,
    pub name: Option<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct LinkSession {
    pub challenge: LinkChallengeView,
    pub challenge_code: String,
    pub relay_url: Option<String>,
}

impl std::fmt::Debug for LinkSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LinkSession")
            .field("challenge", &self.challenge)
            .field("challenge_code", &"[PUBLIC CEREMONY CODE]")
            .field("relay_url", &self.relay_url.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkReplyPreview {
    pub response_code: String,
    pub key: String,
    pub kind: AccountKeyKind,
    pub label: Option<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct LinkResponderSession {
    pub challenge: LinkChallengeView,
    pub challenge_code: String,
    pub relay_url: Option<String>,
}

impl std::fmt::Debug for LinkResponderSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LinkResponderSession")
            .field("challenge", &self.challenge)
            .field("challenge_code", &"[PUBLIC CEREMONY CODE]")
            .field("relay_url", &self.relay_url.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct LinkResponderReply {
    pub response_code: String,
    pub key: String,
    pub sent_automatically: bool,
}

impl std::fmt::Debug for LinkResponderReply {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LinkResponderReply")
            .field("response_code", &"[PUBLIC CEREMONY CODE]")
            .field("key", &self.key)
            .field("sent_automatically", &self.sent_automatically)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct PhoneEnrollmentView {
    pub url: String,
    pub chain_id: String,
    pub account_id: String,
    pub nonce: u64,
}

impl std::fmt::Debug for PhoneEnrollmentView {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PhoneEnrollmentView")
            .field("url", &"[REDACTED]")
            .field("chain_id", &self.chain_id)
            .field("account_id", &self.account_id)
            .field("nonce", &self.nonce)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhoneCandidateView {
    pub key: String,
    pub signature: String,
}

#[derive(Clone, PartialEq, Eq, Default)]
pub struct SecretInput(String);

impl SecretInput {
    pub(super) fn set(&mut self, value: String) {
        self.0.zeroize();
        self.0 = value;
    }

    pub(super) fn clear(&mut self) {
        self.0.zeroize();
    }
}

impl std::ops::Deref for SecretInput {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Debug for SecretInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SecretInput([REDACTED])")
    }
}

impl Drop for SecretInput {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountPanel {
    None,
    Link,
    LinkSelf,
    Phone,
    TouchId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountActionsState {
    pub panel: AccountPanel,
    pub link: Option<LinkSession>,
    pub link_response: String,
    pub link_preview: Option<LinkReplyPreview>,
    pub responder_input: SecretInput,
    pub responder_label: String,
    pub responder_session: Option<LinkResponderSession>,
    pub responder_reply: Option<LinkResponderReply>,
    pub phone: Option<PhoneEnrollmentView>,
    pub phone_candidate: Option<PhoneCandidateView>,
    pub phone_label: String,
    pub touch_id_available: bool,
    pub touch_id_enrolled: bool,
    pub touch_id_password: SecretInput,
    pub pending_remove: Option<String>,
    pub pending_unbind: Option<String>,
    pub editing_node: Option<String>,
    pub node_label_draft: String,
    pub pending_touch_id_disable: bool,
    pub busy: bool,
    pub error: Option<String>,
}

impl Default for AccountActionsState {
    fn default() -> Self {
        Self {
            panel: AccountPanel::None,
            link: None,
            link_response: String::new(),
            link_preview: None,
            responder_input: SecretInput::default(),
            responder_label: String::new(),
            responder_session: None,
            responder_reply: None,
            phone: None,
            phone_candidate: None,
            phone_label: String::new(),
            touch_id_available: false,
            touch_id_enrolled: false,
            touch_id_password: SecretInput::default(),
            pending_remove: None,
            pending_unbind: None,
            editing_node: None,
            node_label_draft: String::new(),
            pending_touch_id_disable: false,
            busy: false,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustodyStatus {
    Plaintext,
    Locked,
    Unlocked,
}

impl CustodyStatus {
    const fn label(self) -> &'static str {
        match self {
            Self::Plaintext => "Not password-protected",
            Self::Locked => "Locked",
            Self::Unlocked => "Unlocked",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Custody {
    pub public_key: String,
    pub status: CustodyStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeData {
    pub profile: Option<AccountProfile>,
    pub workspaces: Vec<WorkspaceRow>,
    pub devices: Vec<DeviceRow>,
    pub device_networks: Vec<DeviceNetworkGroup>,
    pub member_keys: Vec<MemberKeyRow>,
    pub custody: Option<Custody>,
    pub touch_id_available: bool,
    pub touch_id_enrolled: bool,
    pub disconnected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeState {
    pub data: Resource<HomeData>,
    pub display_name_draft: String,
    pub duck_name_draft: String,
    pub duck_name_error: Option<String>,
    pub bio_draft: String,
    pub avatar_edit: AvatarEdit,
    pub profile_busy: bool,
    pub error: Option<String>,
    pub account: AccountActionsState,
}

impl Default for HomeState {
    fn default() -> Self {
        Self {
            data: Resource::Loading,
            display_name_draft: String::new(),
            duck_name_draft: String::new(),
            duck_name_error: None,
            bio_draft: String::new(),
            avatar_edit: AvatarEdit::Keep,
            profile_busy: false,
            error: None,
            account: AccountActionsState::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HomeMessage {
    DisplayNameChanged(String),
    CommitDisplayName,
    DuckNameChanged(String),
    CommitDuckName,
    RemoveDuckName,
    BioChanged(String),
    ChooseAvatar,
    RemoveAvatar,
    SaveProfile,
    CopyAccountId(String),
    SwitchWorkspace(String),
    AddNetwork,
    LinkDevice,
    LinkThisDevice,
    ResponderInputChanged(String),
    ResolveLinkChallenge,
    ResponderLabelChanged(String),
    GenerateLinkResponse,
    LinkResponseChanged(String),
    PollLink,
    ApproveLink,
    PhoneEnrollment,
    PollPhone,
    PhoneLabelChanged(String),
    ApprovePhone,
    RemoveMember(String),
    ConfirmRemoveMember,
    UnbindNode(String),
    ConfirmUnbindNode,
    EditNodeLabel(String, String),
    NodeLabelChanged(String),
    CommitNodeLabel,
    CancelNodeLabel,
    EnableTouchId,
    TouchIdPasswordChanged(String),
    SubmitTouchId,
    DisableTouchId,
    ConfirmDisableTouchId,
    CancelAccountPanel,
    CancelConfirmation,
    LockAccount,
    UnlockAccount,
    SecureAccount,
    RevealRecovery,
}

pub(super) fn account_tick(state: &HomeState) -> Option<Command> {
    match state.account.panel {
        AccountPanel::Link
            if state.account.link.is_some() && state.account.link_response.trim().is_empty() =>
        {
            Some(Command::PollLink)
        }
        AccountPanel::Phone
            if state.account.phone.is_some() && state.account.phone_candidate.is_none() =>
        {
            Some(Command::PollPhoneEnrollment)
        }
        AccountPanel::LinkSelf if state.account.responder_reply.is_some() => {
            Some(Command::LoadHome)
        }
        _ => None,
    }
}

pub(super) fn update(state: &mut HomeState, message: HomeMessage) -> Option<Command> {
    state.error = None;
    match message {
        HomeMessage::DisplayNameChanged(value) => {
            state.display_name_draft = value;
            None
        }
        HomeMessage::CommitDisplayName => {
            let name = state.display_name_draft.trim().to_string();
            if name.len() > 64 {
                state.error = Some("display name must be 64 bytes or fewer".into());
                return None;
            }
            state.profile_busy = true;
            Some(Command::SaveDisplayName(name))
        }
        HomeMessage::DuckNameChanged(value) => {
            state.duck_name_draft = value;
            state.duck_name_error = None;
            None
        }
        HomeMessage::CommitDuckName => {
            let handle = state.duck_name_draft.trim().to_ascii_lowercase();
            state.duck_name_draft = handle.clone();
            if let Some(error) = duck_name_error(&handle) {
                state.duck_name_error = Some(error);
                return None;
            }
            state.profile_busy = true;
            Some(Command::SetDuckName(Some(handle)))
        }
        HomeMessage::RemoveDuckName => {
            state.profile_busy = true;
            Some(Command::SetDuckName(None))
        }
        HomeMessage::BioChanged(value) => {
            state.bio_draft = value.chars().take(280).collect();
            None
        }
        HomeMessage::ChooseAvatar => {
            state.profile_busy = true;
            Some(Command::ChooseAvatar)
        }
        HomeMessage::RemoveAvatar => {
            state.avatar_edit = AvatarEdit::Remove;
            None
        }
        HomeMessage::SaveProfile => {
            state.profile_busy = true;
            Some(Command::SaveProfile {
                bio: state.bio_draft.clone(),
                avatar: state.avatar_edit.clone(),
            })
        }
        HomeMessage::CopyAccountId(id) => Some(Command::CopyText(id)),
        HomeMessage::SwitchWorkspace(id) => Some(Command::SwitchWorkspace(id)),
        HomeMessage::AddNetwork => Some(Command::AddNetwork),
        HomeMessage::LinkDevice => {
            state.account.error = None;
            state.account.busy = true;
            state.account.panel = AccountPanel::Link;
            state.account.link = None;
            state.account.link_response.clear();
            state.account.link_preview = None;
            Some(Command::LinkDevice)
        }
        HomeMessage::LinkThisDevice => {
            state.account.error = None;
            state.account.panel = AccountPanel::LinkSelf;
            state.account.responder_session = None;
            state.account.responder_reply = None;
            None
        }
        HomeMessage::ResponderInputChanged(value) => {
            state.account.responder_input.set(value);
            state.account.responder_session = None;
            state.account.responder_reply = None;
            None
        }
        HomeMessage::ResolveLinkChallenge => {
            let input = nonempty(&state.account.responder_input)?;
            state.account.busy = true;
            Some(Command::ResolveLinkChallenge { input })
        }
        HomeMessage::ResponderLabelChanged(value) => {
            state.account.responder_label = value;
            None
        }
        HomeMessage::GenerateLinkResponse => {
            let session = state.account.responder_session.clone()?;
            let label = nonempty(&state.account.responder_label);
            state.account.busy = true;
            Some(Command::GenerateLinkResponse { session, label })
        }
        HomeMessage::LinkResponseChanged(value) => {
            state.account.link_preview = link_reply_preview(&value);
            state.account.link_response = value;
            None
        }
        HomeMessage::PollLink => Some(Command::PollLink),
        HomeMessage::ApproveLink => {
            let challenge = state.account.link.as_ref()?.challenge.clone();
            let response = nonempty(&state.account.link_response)?;
            state.account.busy = true;
            Some(Command::ApproveLink {
                challenge,
                response,
            })
        }
        HomeMessage::PhoneEnrollment => {
            state.account.error = None;
            state.account.busy = true;
            state.account.panel = AccountPanel::Phone;
            state.account.phone = None;
            state.account.phone_candidate = None;
            state.account.phone_label.clear();
            Some(Command::StartPhoneEnrollment)
        }
        HomeMessage::PollPhone => Some(Command::PollPhoneEnrollment),
        HomeMessage::PhoneLabelChanged(value) => {
            state.account.phone_label = value;
            None
        }
        HomeMessage::ApprovePhone => {
            let enrollment = state.account.phone.clone()?;
            let candidate = state.account.phone_candidate.clone()?;
            let label = nonempty(&state.account.phone_label);
            state.account.busy = true;
            Some(Command::ApprovePhoneEnrollment {
                enrollment,
                candidate,
                label,
            })
        }
        HomeMessage::RemoveMember(key) => {
            state.account.pending_remove = Some(key);
            None
        }
        HomeMessage::ConfirmRemoveMember => {
            state.account.busy = true;
            state
                .account
                .pending_remove
                .take()
                .map(Command::RemoveMember)
        }
        HomeMessage::UnbindNode(key) => {
            state.account.pending_unbind = Some(key);
            None
        }
        HomeMessage::ConfirmUnbindNode => {
            state.account.busy = true;
            state.account.pending_unbind.take().map(Command::UnbindNode)
        }
        HomeMessage::EditNodeLabel(key, label) => {
            state.account.editing_node = Some(key);
            state.account.node_label_draft = if label != "Device" {
                label
            } else {
                String::new()
            };
            None
        }
        HomeMessage::NodeLabelChanged(value) => {
            state.account.node_label_draft = value;
            None
        }
        HomeMessage::CommitNodeLabel => {
            let key = state.account.editing_node.take()?;
            let label = nonempty(&state.account.node_label_draft);
            state.account.busy = true;
            Some(Command::SetNodeLabel { key, label })
        }
        HomeMessage::CancelNodeLabel => {
            state.account.editing_node = None;
            state.account.node_label_draft.clear();
            None
        }
        HomeMessage::EnableTouchId => {
            state.account.error = None;
            state.account.panel = AccountPanel::TouchId;
            None
        }
        HomeMessage::TouchIdPasswordChanged(value) => {
            state.account.touch_id_password.set(value);
            None
        }
        HomeMessage::SubmitTouchId => {
            let password = nonempty(&state.account.touch_id_password)?;
            state.account.busy = true;
            Some(Command::EnrollTouchId(password))
        }
        HomeMessage::DisableTouchId => {
            state.account.pending_touch_id_disable = true;
            None
        }
        HomeMessage::ConfirmDisableTouchId => {
            state.account.pending_touch_id_disable = false;
            state.account.busy = true;
            Some(Command::DisableTouchId)
        }
        HomeMessage::CancelAccountPanel => {
            let command = match state.account.panel {
                AccountPanel::Link => Some(Command::CancelLink),
                AccountPanel::Phone => Some(Command::CancelPhoneEnrollment),
                _ => None,
            };
            state.account.panel = AccountPanel::None;
            state.account.link = None;
            state.account.phone = None;
            state.account.phone_candidate = None;
            state.account.responder_session = None;
            state.account.responder_reply = None;
            state.account.responder_input.clear();
            state.account.responder_label.clear();
            state.account.touch_id_password.clear();
            state.account.error = None;
            command
        }
        HomeMessage::CancelConfirmation => {
            state.account.pending_remove = None;
            state.account.pending_unbind = None;
            state.account.pending_touch_id_disable = false;
            None
        }
        HomeMessage::LockAccount => Some(Command::LockAccount),
        HomeMessage::UnlockAccount => Some(Command::UnlockAccount),
        HomeMessage::SecureAccount => Some(Command::SecureAccount),
        HomeMessage::RevealRecovery => Some(Command::RevealRecovery),
    }
}

fn duck_name_error(handle: &str) -> Option<String> {
    if handle.is_empty() {
        Some("Enter a name.".into())
    } else if handle.len() > 63 {
        Some("Use 63 characters or fewer.".into())
    } else if handle.starts_with('-') || handle.ends_with('-') {
        Some("A name cannot start or end with a hyphen.".into())
    } else if !handle
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        Some("Use lowercase letters, numbers, and hyphens.".into())
    } else if matches!(handle, "net" | "agents") {
        Some(format!("{handle}.duck is reserved."))
    } else {
        None
    }
}

fn link_reply_preview(value: &str) -> Option<LinkReplyPreview> {
    let response = decode_link_response(value).ok()?;
    Some(LinkReplyPreview {
        response_code: encode_link_response(&response).ok()?,
        key: response.pubkey,
        kind: match response.kind {
            MemberKeyKind::Ed25519 => AccountKeyKind::Ed25519,
            MemberKeyKind::P256 => AccountKeyKind::P256,
            MemberKeyKind::WebauthnP256 => AccountKeyKind::WebauthnP256,
        },
        label: response.label,
    })
}

pub(super) fn view(state: &HomeState, p: Palette) -> Element<'_, Message> {
    let content = match &state.data {
        Resource::Loading => center_state(
            "Loading Home…",
            "Reading account and network state.",
            Icon::Home,
            p,
        ),
        Resource::Empty => container(
            column![
                icon_tile(Icon::Home, 42.0, p),
                text("No networks yet").font(SANS).size(14).color(p.ink),
                text("Add a network to get started.")
                    .font(SANS)
                    .size(11.5)
                    .color(p.muted_2),
                outline("+ Add network", Message::Home(HomeMessage::AddNetwork), p),
            ]
            .spacing(9)
            .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into(),
        Resource::Error(error) => error_state("Couldn't load Home", error, Screen::Home, p),
        Resource::Ready(data) => home_content(state, data, p),
    };
    container(column![text("Home").font(SANS).size(16).color(p.filled), content].spacing(9))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(HOME_PAD)
        .style(move |_| surface(p.canvas))
        .into()
}

fn home_content<'a>(state: &'a HomeState, data: &'a HomeData, p: Palette) -> Element<'a, Message> {
    let profile = if let Some(profile) = &data.profile {
        let linked = !profile.account_id.is_empty();
        let account_line = if profile.account_id.is_empty() {
            "not linked to an account yet".into()
        } else {
            format!("{} · account id", short(&profile.account_id))
        };
        let registered = profile.duck_name.is_some();
        let mut duck = column![
            text("Duck name").font(SANS).size(12).color(p.ink),
            text("Optional account name — your identity works without one.")
                .font(SANS)
                .size(11)
                .color(p.muted),
            row![
                field_enabled(
                    "your-name",
                    &state.duck_name_draft,
                    |value| Message::Home(HomeMessage::DuckNameChanged(value)),
                    linked && !state.profile_busy,
                    p,
                )
                .on_submit_maybe(
                    (linked && !state.profile_busy)
                        .then_some(Message::Home(HomeMessage::CommitDuckName)),
                )
                .width(150),
                text(".duck").font(MONO).size(12).color(p.muted),
                outline_enabled(
                    if registered { "Update" } else { "Register" },
                    Message::Home(HomeMessage::CommitDuckName),
                    linked && !state.profile_busy && !state.duck_name_draft.trim().is_empty(),
                    p,
                ),
            ]
            .spacing(7)
            .align_y(Alignment::Center),
        ]
        .spacing(7);
        if registered {
            duck = duck.push(outline_enabled(
                "Remove Duck name",
                Message::Home(HomeMessage::RemoveDuckName),
                linked && !state.profile_busy,
                p,
            ));
        }
        if let Some(error) = state.duck_name_error.as_deref() {
            duck = duck.push(text(error).font(SANS).size(10.5).color(p.danger));
        }
        card(
            column![
                row![
                    profile_avatar(profile, &AvatarEdit::Keep, 40.0, p),
                    column![
                        plain_input_enabled(
                            "Display name",
                            &state.display_name_draft,
                            |value| Message::Home(HomeMessage::DisplayNameChanged(value)),
                            linked,
                            p
                        )
                        .on_submit_maybe(
                            linked.then_some(Message::Home(HomeMessage::CommitDisplayName)),
                        ),
                        text(account_line).font(MONO).size(10.5).color(p.muted),
                    ]
                    .spacing(3)
                    .width(Length::Fill),
                    outline_enabled(
                        "Copy id",
                        Message::Home(HomeMessage::CopyAccountId(profile.account_id.clone())),
                        linked,
                        p
                    ),
                ]
                .spacing(13)
                .align_y(Alignment::Center),
                divider(p),
                duck,
            ]
            .spacing(8),
            p,
        )
    } else {
        card(
            column![
                text("not linked to an account yet")
                    .font(MONO)
                    .size(11)
                    .color(p.muted)
            ],
            p,
        )
    };

    let profile_editor = data.profile.as_ref().map(|profile| {
        let linked = !profile.account_id.is_empty();
        let avatar_changed = !matches!(state.avatar_edit, AvatarEdit::Keep);
        let dirty = avatar_changed || state.bio_draft != profile.bio.as_deref().unwrap_or_default();
        let can_remove = !matches!(state.avatar_edit, AvatarEdit::Remove)
            && (profile.avatar.is_some() || matches!(state.avatar_edit, AvatarEdit::Replace(_)));
        let mut actions = row![outline_enabled(
            "Change avatar",
            Message::Home(HomeMessage::ChooseAvatar),
            linked && !state.profile_busy,
            p,
        )]
        .spacing(7);
        if can_remove {
            actions = actions.push(outline_enabled(
                "Remove",
                Message::Home(HomeMessage::RemoveAvatar),
                linked && !state.profile_busy,
                p,
            ));
        }
        let mut editor = column![
            text("Profile").font(SANS).size(12).color(p.ink),
            row![
                profile_avatar(profile, &state.avatar_edit, 56.0, p),
                column![
                    actions,
                    text("Global to your account — shown on every network you join.")
                        .font(SANS)
                        .size(11)
                        .color(p.muted),
                ]
                .spacing(6)
                .width(Length::Fill),
            ]
            .spacing(13)
            .align_y(Alignment::Start),
            text("Bio / status").font(SANS).size(11).color(p.muted),
            field_enabled(
                "A short line about you",
                &state.bio_draft,
                |value| Message::Home(HomeMessage::BioChanged(value)),
                linked,
                p,
            ),
            row![
                outline_enabled(
                    if state.profile_busy {
                        "Saving…"
                    } else {
                        "Save"
                    },
                    Message::Home(HomeMessage::SaveProfile),
                    linked && dirty && !state.profile_busy,
                    p,
                ),
                text(format!("{}/280", state.bio_draft.chars().count()))
                    .font(MONO)
                    .size(10.5)
                    .color(p.muted),
            ]
            .spacing(9)
            .align_y(Alignment::Center),
        ]
        .spacing(9);
        if !linked {
            editor = editor.push(
                text("Bind this node to an account first.")
                    .font(SANS)
                    .size(10.5)
                    .color(p.muted),
            );
        }
        if let Some(error) = state.error.as_deref() {
            editor = editor.push(text(error).font(SANS).size(10.5).color(p.danger));
        }
        card(editor, p)
    });

    let mut networks = Column::new().spacing(0);
    networks = networks.push(table_header(
        ["Network", "Network ID", "Your standing", "Active"],
        p,
    ));
    if data.workspaces.is_empty() {
        networks = networks.push(
            container(
                text("No networks yet — add one to get started.")
                    .font(SANS)
                    .size(12)
                    .color(p.muted),
            )
            .padding([9, 12]),
        );
    } else {
        for (index, workspace) in data.workspaces.iter().enumerate() {
            if index > 0 {
                networks = networks.push(divider(p));
            }
            networks = networks.push(workspace_row(workspace, p));
        }
    }

    let mut body = column![profile].spacing(9);
    if let Some(editor) = profile_editor {
        body = body.push(editor);
    }
    body = body
        .push(section_header(
            "YOUR NETWORKS",
            Some(("+ Add network", Message::Home(HomeMessage::AddNetwork))),
            p,
        ))
        .push(card(networks, p));

    if data.disconnected {
        body = body.push(notice("Account data lives on each network — enter a network from the rail to see this account's keys and standing there. Device custody below always works.", p));
    }

    body = body.push(section_label("YOUR DEVICES", p));
    if data.device_networks.is_empty() {
        body = body.push(card(
            column![info_row(
                "Devices",
                if data.member_keys.is_empty() {
                    "link this device to see account devices"
                } else {
                    "none bound yet"
                },
                p
            )],
            p,
        ));
    } else {
        for network in &data.device_networks {
            let mut devices = Column::new().spacing(0).push(account_item_row(
                network.name.clone(),
                if network.active {
                    "CONNECTED".into()
                } else {
                    format!("last seen {}", time_ago(network.at_ms))
                },
                None,
                p,
            ));
            if network.devices.is_empty() {
                devices = devices.push(info_row("Devices", "none bound", p));
            } else {
                for device in &network.devices {
                    devices =
                        devices.push(device_item_row(device, network.active, &state.account, p));
                }
            }
            body = body.push(card(devices, p));
            if !network.active {
                body = body.push(notice_owned(
                    format!(
                        "Switch to {} to rename or unbind its devices.",
                        network.name
                    ),
                    p,
                ));
            }
        }
    }
    if let Some(key) = state.account.pending_unbind.as_deref() {
        body = body.push(account_confirmation(
            "Unbind this device?",
            format!(
                "{} will stop belonging to this account on the active network. Its validator seat is separate.",
                short(key)
            ),
            "Unbind device",
            HomeMessage::ConfirmUnbindNode,
            p,
        ));
    }
    body = body.push(notice("Lost a device? Unbind it on the network it was on, then reveal your recovery phrase below to restore your account on the replacement.", p));

    if state.account.touch_id_available {
        body = body.push(section_label("THIS DEVICE", p));
        let touch_control = if state.account.touch_id_enrolled {
            control_row(
                "Touch ID",
                "Unlock this account on this Mac with Touch ID or the Mac login password fallback.",
                "Disable Touch ID",
                Message::Home(HomeMessage::DisableTouchId),
                p,
            )
        } else {
            control_row(
                "Touch ID",
                "Unlock this account on this Mac with Touch ID instead of typing its password.",
                "Enable Touch ID",
                Message::Home(HomeMessage::EnableTouchId),
                p,
            )
        };
        let mut touch = column![touch_control].spacing(9);
        if state.account.panel == AccountPanel::TouchId && !state.account.touch_id_enrolled {
            touch = touch.push(
                column![
                    field(
                        "Account password",
                        &state.account.touch_id_password,
                        |value| Message::Home(HomeMessage::TouchIdPasswordChanged(value)),
                        p
                    )
                    .secure(true),
                    row![
                        outline("Cancel", Message::Home(HomeMessage::CancelAccountPanel), p),
                        filled(
                            if state.account.busy {
                                "Enabling…"
                            } else {
                                "Enable Touch ID"
                            },
                            Message::Home(HomeMessage::SubmitTouchId),
                            !state.account.busy
                                && !state.account.touch_id_password.trim().is_empty(),
                            p
                        )
                    ]
                    .spacing(8)
                ]
                .spacing(8)
                .padding([4, 12]),
            );
        }
        body = body.push(card(touch, p));
        if state.account.pending_touch_id_disable {
            body = body.push(account_confirmation(
                "Disable Touch ID on this device?",
                "Your account is unaffected. You can keep unlocking with its password or recovery phrase.",
                "Disable Touch ID",
                HomeMessage::ConfirmDisableTouchId,
                p,
            ));
        }
    }

    body = body.push(section_label("DEVICES & KEYS", p));
    let mut keys = Column::new().spacing(0);
    if data.member_keys.is_empty() {
        keys = keys.push(info_row("Member keys", "not linked on this network", p));
    } else {
        for member in &data.member_keys {
            keys = keys.push(account_item_row(
                member
                    .label
                    .clone()
                    .unwrap_or_else(|| member.kind.label().into()),
                format!(
                    "{} · {}{}",
                    member.kind.label(),
                    short(&member.key),
                    if member.this_device {
                        " · this device"
                    } else {
                        ""
                    }
                ),
                (data.member_keys.len() > 1).then(|| {
                    (
                        "Remove",
                        HomeMessage::RemoveMember(member.key.clone()),
                        true,
                    )
                }),
                p,
            ));
        }
    }
    if data.member_keys.is_empty() {
        keys = keys.push(control_row(
            "Link this device",
            "Use the private LAN address or challenge code shown by a device already in your account.",
            if state.account.panel == AccountPanel::LinkSelf { "Cancel" } else { "Start" },
            Message::Home(if state.account.panel == AccountPanel::LinkSelf {
                HomeMessage::CancelAccountPanel
            } else {
                HomeMessage::LinkThisDevice
            }),
            p,
        ));
        if state.account.panel == AccountPanel::LinkSelf {
            keys = keys.push(link_responder_panel(&state.account, p));
        }
    } else {
        keys = keys.push(control_row(
            "Link a device",
            "Bring another machine into this account — it signs as you, with its own key.",
            if state.account.panel == AccountPanel::Link {
                "Cancel"
            } else {
                "Start"
            },
            Message::Home(if state.account.panel == AccountPanel::Link {
                HomeMessage::CancelAccountPanel
            } else {
                HomeMessage::LinkDevice
            }),
            p,
        ));
        if state.account.panel == AccountPanel::Link {
            keys = keys.push(link_inviter_panel(&state.account, p));
        }
        keys = keys.push(control_row(
            "Add a key from your phone",
            "Scan a QR over the LAN — the phone mints a security key you approve here.",
            if state.account.panel == AccountPanel::Phone {
                "Cancel"
            } else {
                "Show QR"
            },
            Message::Home(if state.account.panel == AccountPanel::Phone {
                HomeMessage::CancelAccountPanel
            } else {
                HomeMessage::PhoneEnrollment
            }),
            p,
        ));
        if state.account.panel == AccountPanel::Phone {
            keys = keys.push(phone_enrollment_panel(&state.account, p));
        }
    }
    body = body.push(card(keys, p));
    if let Some(key) = state.account.pending_remove.as_deref() {
        body = body.push(account_confirmation(
            "Remove this key from your account?",
            format!(
                "{} can no longer sign as this account. Other account keys keep working.",
                short(key)
            ),
            "Remove key",
            HomeMessage::ConfirmRemoveMember,
            p,
        ));
    }
    if let Some(error) = &state.account.error {
        body = body.push(error_banner(error, p));
    }
    if let Some(custody) = &data.custody {
        let action = match custody.status {
            CustodyStatus::Locked => (
                "Unlock account",
                "Verify your password to sign with this account for this session.",
                "Unlock",
                HomeMessage::UnlockAccount,
            ),
            CustodyStatus::Unlocked => (
                "Lock account",
                "Drop the cached password — the next signing action needs it again.",
                "Lock",
                HomeMessage::LockAccount,
            ),
            CustodyStatus::Plaintext => (
                "Set a password",
                "Encrypt this account key at rest so a stolen device can't sign as you.",
                "Set password",
                HomeMessage::SecureAccount,
            ),
        };
        body = body.push(section_label("RECOVERY & SECURITY", p));
        body = body.push(card(
            column![
                info_row("Account key (this device)", short(&custody.public_key), p),
                info_row("Password lock", custody.status.label(), p),
                control_row(action.0, action.1, action.2, Message::Home(action.3), p),
                control_row(
                    "Recovery phrase",
                    "View your 24-word backup phrase. Always requires your password.",
                    "Reveal recovery phrase",
                    Message::Home(HomeMessage::RevealRecovery),
                    p
                ),
            ],
            p,
        ));
    }
    if let Some(error) = &state.error {
        body = body.push(error_banner(error, p));
    }
    scrollable(body.push(Space::new().height(22)))
        .height(Length::Fill)
        .into()
}

fn account_item_row(
    label: impl ToString,
    value: impl ToString,
    action: Option<(&'static str, HomeMessage, bool)>,
    p: Palette,
) -> Element<'static, Message> {
    let mut value_row = row![text(value.to_string()).font(MONO).size(10.5).color(p.muted)]
        .spacing(8)
        .align_y(Alignment::Center);
    if let Some((label, message, danger)) = action {
        let button = if danger {
            danger_outline(label, Message::Home(message), p)
        } else {
            outline(label, Message::Home(message), p)
        };
        value_row = value_row.push(button);
    }
    container(
        row![
            text(label.to_string()).font(SANS).size(12).color(p.ink),
            Space::new().width(Length::Fill),
            value_row,
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([10, 12])
    .style(move |_| bottom_border(Color::TRANSPARENT, p.border_soft))
    .into()
}

fn device_item_row<'a>(
    device: &'a DeviceRow,
    active: bool,
    state: &'a AccountActionsState,
    p: Palette,
) -> Element<'a, Message> {
    let editing = active && state.editing_node.as_deref() == Some(device.key.as_str());
    let mut value = row![
        text(format!(
            "{}{}",
            short(&device.key),
            if device.this_device {
                " · this device"
            } else {
                ""
            }
        ))
        .font(MONO)
        .size(10.5)
        .color(p.muted),
        standing_chip(device.standing, p),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    if active {
        if editing {
            value = value
                .push(sem_input(
                    "Node label",
                    &state.node_label_draft,
                    plain_input(
                        "e.g. Kim's laptop",
                        &state.node_label_draft,
                        |value| Message::Home(HomeMessage::NodeLabelChanged(value)),
                        p,
                    )
                    .on_submit(Message::Home(HomeMessage::CommitNodeLabel)),
                ))
                .push(outline(
                    "Save",
                    Message::Home(HomeMessage::CommitNodeLabel),
                    p,
                ))
                .push(outline(
                    "Cancel",
                    Message::Home(HomeMessage::CancelNodeLabel),
                    p,
                ));
        } else {
            value = value
                .push(outline(
                    if device.label == "Device" {
                        "Label"
                    } else {
                        "Rename"
                    },
                    Message::Home(HomeMessage::EditNodeLabel(
                        device.key.clone(),
                        device.label.clone(),
                    )),
                    p,
                ))
                .push(danger_outline(
                    "Unbind",
                    Message::Home(HomeMessage::UnbindNode(device.key.clone())),
                    p,
                ));
        }
    }
    container(
        row![
            text(device.label.clone()).font(SANS).size(12).color(p.ink),
            Space::new().width(Length::Fill),
            value,
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([10, 12])
    .style(move |_| bottom_border(Color::TRANSPARENT, p.border_soft))
    .into()
}

fn standing_chip(standing: Standing, p: Palette) -> Element<'static, Message> {
    let (foreground, background, border) = match standing {
        Standing::Validator => (p.on_filled, p.filled, p.filled),
        Standing::Resident => (p.green, p.sunken, p.green),
        Standing::NoSeat => (p.muted_2, p.sunken, p.border),
    };
    container(
        text(standing.label().to_uppercase())
            .font(MONO)
            .size(9)
            .color(foreground),
    )
    .padding([2, 6])
    .style(move |_| iced::widget::container::Style {
        background: Some(Background::Color(background)),
        border: Border {
            color: border,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    })
    .into()
}

fn time_ago(at_ms: u64) -> String {
    let now: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX);
    let seconds = now.saturating_sub(at_ms) / 1_000;
    match seconds {
        0..=59 => "just now".into(),
        60..=3_599 => format!("{}m ago", seconds.div_ceil(60)),
        3_600..=86_399 => format!("{}h ago", (seconds / 60).div_ceil(60)),
        _ => format!("{}d ago", (seconds / 3_600).div_ceil(24)),
    }
}

fn link_responder_panel(state: &AccountActionsState, p: Palette) -> Element<'_, Message> {
    let mut panel = Column::new()
        .spacing(9)
        .padding([8, 12])
        .push(
            text("Paste the LAN address for automatic delivery, or the manual challenge code.")
                .font(SANS)
                .size(11)
                .color(p.muted),
        )
        .push(field(
            "http://192.168… or ducktape-link-challenge-v1:…",
            &state.responder_input,
            |value| Message::Home(HomeMessage::ResponderInputChanged(value)),
            p,
        ))
        .push(filled(
            if state.busy {
                "Checking…"
            } else {
                "Continue"
            },
            Message::Home(HomeMessage::ResolveLinkChallenge),
            !state.busy && !state.responder_input.trim().is_empty(),
            p,
        ));
    let Some(session) = state.responder_session.as_ref() else {
        return panel.into();
    };
    let account_name = session
        .challenge
        .name
        .clone()
        .unwrap_or_else(|| short(&session.challenge.account_id));
    panel = panel
        .push(divider(p))
        .push(info_row("Account", account_name, p))
        .push(info_row("Network", &session.challenge.chain_id, p))
        .push(info_row(
            "Account fingerprint",
            short(&session.challenge.account_id),
            p,
        ))
        .push(
            text("Confirm these details match the device that showed the code. This device will then prove possession of its own key.")
                .font(SANS)
                .size(11)
                .color(p.muted),
        )
        .push(field(
            "Device label (optional)",
            &state.responder_label,
            |value| Message::Home(HomeMessage::ResponderLabelChanged(value)),
            p,
        ))
        .push(filled(
            if state.busy { "Signing…" } else { "Link this device" },
            Message::Home(HomeMessage::GenerateLinkResponse),
            !state.busy && state.responder_reply.is_none(),
            p,
        ));
    if let Some(reply) = state.responder_reply.as_ref() {
        panel = panel
            .push(notice(
                if reply.sent_automatically {
                    "Response sent over the private LAN. Approve this key on your other device."
                } else {
                    "Copy this response to your other device and approve only after its fingerprint matches."
                },
                p,
            ))
            .push(info_row("This device", short(&reply.key), p))
            .push(code_box(&reply.response_code, p))
            .push(outline(
                "Copy response",
                Message::Home(HomeMessage::CopyAccountId(reply.response_code.clone())),
                p,
            ));
    }
    panel.into()
}

fn link_inviter_panel(state: &AccountActionsState, p: Palette) -> Element<'_, Message> {
    let mut panel = Column::new().spacing(9).padding([8, 12]);
    let Some(link) = state.link.as_ref() else {
        panel = panel.push(
            text(if state.busy {
                "Minting a fresh link code…"
            } else {
                "The link code could not be created."
            })
            .font(SANS)
            .size(11.5)
            .color(p.muted),
        );
        return panel.into();
    };
    panel = panel.push(
        text("1 · On the new device, choose Link device. Use the LAN address or paste the challenge code.")
            .font(SANS)
            .size(11)
            .color(p.muted),
    );
    if let Some(url) = link.relay_url.as_deref() {
        panel = panel.push(qr_image(url, 160.0, p)).push(code_box(url, p));
    }
    panel = panel
        .push(code_box(&link.challenge_code, p))
        .push(outline(
            "Copy challenge",
            Message::Home(HomeMessage::CopyAccountId(link.challenge_code.clone())),
            p,
        ))
        .push(
            text("2 · The LAN reply arrives automatically, or paste it here. Approve only after the key below matches the new device.")
                .font(SANS)
                .size(11)
                .color(p.muted),
        )
        .push(field(
            "ducktape-link-response-v1:…",
            &state.link_response,
            |value| Message::Home(HomeMessage::LinkResponseChanged(value)),
            p,
        ));
    let mut actions = row![outline(
        "Check for reply",
        Message::Home(HomeMessage::PollLink),
        p,
    )]
    .spacing(8);
    if let Some(reply) = state.link_preview.as_ref() {
        panel = panel.push(
            text(format!(
                "Approve only if the new device shows: {} · {}{}",
                reply.kind.label(),
                short(&reply.key),
                reply
                    .label
                    .as_deref()
                    .map(|label| format!(" · {label}"))
                    .unwrap_or_default()
            ))
            .font(MONO)
            .size(10.5)
            .color(p.ink),
        );
        actions = actions.push(outline_enabled(
            if state.busy {
                "Approving…"
            } else {
                "Approve link"
            },
            Message::Home(HomeMessage::ApproveLink),
            !state.busy,
            p,
        ));
    }
    panel.push(actions).into()
}

fn phone_enrollment_panel(state: &AccountActionsState, p: Palette) -> Element<'_, Message> {
    let mut panel = Column::new().spacing(9).padding([8, 12]);
    let Some(enrollment) = state.phone.as_ref() else {
        panel = panel.push(
            text(if state.busy {
                "Starting the private-LAN enrollment…"
            } else {
                "The phone enrollment could not be started."
            })
            .font(SANS)
            .size(11.5)
            .color(p.muted),
        );
        return panel.into();
    };
    if let Some(candidate) = state.phone_candidate.as_ref() {
        panel = panel
            .push(
                text("Your phone created a key. Approving signs the account authorizer on this desktop.")
                    .font(SANS)
                    .size(11)
                    .color(p.muted),
            )
            .push(
                text(format!("Security key · {}", short(&candidate.key)))
                    .font(MONO)
                    .size(10.5)
                    .color(p.ink),
            )
            .push(field(
                "Key label (optional, e.g. my phone)",
                &state.phone_label,
                |value| Message::Home(HomeMessage::PhoneLabelChanged(value)),
                p,
            ))
            .push(outline_enabled(
                if state.busy { "Approving…" } else { "Approve key" },
                Message::Home(HomeMessage::ApprovePhone),
                !state.busy,
                p,
            ));
    } else {
        panel = panel
            .push(
                text("Scan with your phone on the same Wi-Fi. The key is generated on the phone and nothing lands until you approve it here.")
                    .font(SANS)
                    .size(11)
                    .color(p.muted),
            )
            .push(qr_image(&enrollment.url, 190.0, p))
            .push(code_box(&enrollment.url, p))
            .push(outline(
                "Check for phone",
                Message::Home(HomeMessage::PollPhone),
                p,
            ));
    }
    panel.into()
}

fn account_confirmation(
    title: impl ToString,
    description: impl ToString,
    confirm: &'static str,
    message: HomeMessage,
    p: Palette,
) -> Element<'static, Message> {
    container(
        column![
            text(title.to_string())
                .font(SANS)
                .size(12.5)
                .color(p.danger),
            text(description.to_string())
                .font(SANS)
                .size(11)
                .color(p.muted),
            row![
                outline("Cancel", Message::Home(HomeMessage::CancelConfirmation), p),
                danger_outline(confirm, Message::Home(message), p),
            ]
            .spacing(8),
        ]
        .spacing(7),
    )
    .width(Length::Fill)
    .padding([11, 13])
    .style(move |_| iced::widget::container::Style {
        background: Some(Background::Color(p.danger_soft)),
        border: Border {
            color: p.danger_border,
            width: 1.0,
            radius: RADIUS_MD.into(),
        },
        ..Default::default()
    })
    .into()
}

fn code_box(value: &str, p: Palette) -> Element<'static, Message> {
    // Security-critical, human-verified strings (challenge/response codes, relay
    // and enrollment URLs). 8.5px mono was below legibility for fingerprint
    // matching; CAPTION mono, still glyph-wrapping to fit its container.
    container(
        text(value.to_string())
            .font(MONO)
            .size(CAPTION)
            .color(p.muted)
            .wrapping(iced::widget::text::Wrapping::Glyph),
    )
    .width(Length::Fill)
    .padding(8)
    .style(move |_| iced::widget::container::Style {
        background: Some(Background::Color(p.sunken)),
        border: Border {
            color: p.border,
            width: 1.0,
            radius: RADIUS_SM.into(),
        },
        ..Default::default()
    })
    .into()
}

fn qr_image(value: &str, size: f32, p: Palette) -> Element<'static, Message> {
    match qr_svg(value) {
        Ok(document) => container(
            Svg::new(svg::Handle::from_memory(document.into_bytes()))
                .width(size)
                .height(size),
        )
        .width(Length::Fill)
        .align_x(Alignment::Center)
        .into(),
        Err(_) => notice("QR unavailable — use the address or code shown below.", p),
    }
}

pub(super) fn qr_svg(value: &str) -> Result<String, String> {
    use std::fmt::Write as _;

    let code = QrCode::new(value.as_bytes()).map_err(|_| "could not encode QR".to_string())?;
    let width = code.width();
    let canvas = width + 8;
    let mut path = String::new();
    for y in 0..width {
        for x in 0..width {
            if code[(x, y)] == QrColor::Dark {
                write!(path, "M{} {}h1v1h-1z", x + 4, y + 4).expect("write QR path to String");
            }
        }
    }
    Ok(format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 {canvas} {canvas}' shape-rendering='crispEdges'><rect width='100%' height='100%' fill='white'/><path d='{path}' fill='black'/></svg>"
    ))
}

fn profile_avatar(
    profile: &AccountProfile,
    edit: &AvatarEdit,
    size: f32,
    p: Palette,
) -> Element<'static, Message> {
    let bytes = match edit {
        AvatarEdit::Replace(avatar) => Some(avatar.bytes.as_slice()),
        AvatarEdit::Keep => profile.avatar_bytes.as_deref(),
        AvatarEdit::Remove => None,
    };
    let Some(bytes) = bytes else {
        return avatar(&profile.display_name, size, p);
    };
    container(
        image(iced::widget::image::Handle::from_bytes(bytes.to_vec()))
            .content_fit(iced::ContentFit::Cover)
            .border_radius(99.0)
            .width(size)
            .height(size),
    )
    .width(size)
    .height(size)
    .clip(true)
    .style(move |_| iced::widget::container::Style {
        background: Some(Background::Color(p.sunken)),
        border: Border {
            color: p.border,
            width: 1.0,
            radius: 99.0.into(),
        },
        ..Default::default()
    })
    .into()
}
fn info_row(label: impl ToString, value: impl ToString, p: Palette) -> Element<'static, Message> {
    container(
        row![
            text(label.to_string()).font(SANS).size(12).color(p.ink),
            Space::new().width(Length::Fill),
            text(value.to_string()).font(MONO).size(10.5).color(p.muted)
        ]
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([10, 12])
    .style(move |_| bottom_border(Color::TRANSPARENT, p.border_soft))
    .into()
}
fn control_row<'a>(
    title: &'a str,
    description: &'a str,
    button_label: &'static str,
    message: Message,
    p: Palette,
) -> Element<'a, Message> {
    container(
        row![
            column![
                text(title).font(SANS).size(12).color(p.ink),
                text(description).font(SANS).size(10.5).color(p.muted)
            ]
            .spacing(3)
            .width(Length::Fill),
            outline(button_label, message, p)
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .padding([10, 12])
    .style(move |_| bottom_border(Color::TRANSPARENT, p.border_soft))
    .into()
}

fn table_header<const N: usize>(
    labels: [&'static str; N],
    p: Palette,
) -> Element<'static, Message> {
    let mut cells = row![].spacing(4);
    for label in labels {
        cells = cells.push(
            text(label)
                .font(MONO)
                .size(9.5)
                .color(p.muted)
                .width(Length::FillPortion(1)),
        );
    }
    container(cells)
        .padding([9, 12])
        .style(move |_| bottom_border(Color::TRANSPARENT, p.border_soft))
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

fn active_chip(p: Palette) -> Element<'static, Message> {
    container(text("ACTIVE").font(MONO).size(CAPTION).color(p.green))
        .padding([2, 6])
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(p.sunken)),
            border: Border {
                color: p.green,
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn workspace_row(workspace: &WorkspaceRow, p: Palette) -> Element<'static, Message> {
    // The whole name/id/standing span is the mouse path (row click → Enter); the
    // trailing cell is a real, interactive Enter button (the keyboard/AT path)
    // for inactive rows, or a non-interactive ACTIVE chip for the active one.
    let standing_cell: Element<'static, Message> = if workspace.active {
        container(standing_chip(workspace.standing, p))
            .width(Length::FillPortion(1))
            .into()
    } else {
        text("—")
            .font(MONO)
            .size(CAPTION)
            .color(p.muted_3)
            .width(Length::FillPortion(1))
            .into()
    };
    let select = button(
        row![
            text(workspace.name.clone())
                .font(SANS)
                .size(BODY)
                .color(p.ink)
                .width(Length::FillPortion(1)),
            text(workspace.network_id.clone())
                .font(MONO)
                .size(LABEL)
                .color(p.muted)
                .width(Length::FillPortion(1)),
            standing_cell,
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .width(Length::FillPortion(3))
    .padding([9, 12])
    .on_press(Message::Home(HomeMessage::SwitchWorkspace(
        workspace.id.clone(),
    )))
    .style(move |_, status| iced::widget::button::Style {
        background: matches!(status, iced::widget::button::Status::Hovered)
            .then_some(Background::Color(p.titlebar)),
        text_color: p.ink,
        border: Border::default(),
        ..Default::default()
    });
    #[cfg(all(feature = "agent", debug_assertions))]
    let select =
        iced_agent_plugin::sem(iced_agent_plugin::Role::ListItem, workspace.name.clone(), select);

    let action = if workspace.active {
        active_chip(p)
    } else {
        outline(
            "Enter",
            Message::Home(HomeMessage::SwitchWorkspace(workspace.id.clone())),
            p,
        )
    };

    row![
        select,
        container(action)
            .width(Length::FillPortion(1))
            .padding([9, 12])
            .align_x(Alignment::End),
    ]
    .spacing(4)
    .align_y(Alignment::Center)
    .into()
}

fn short(value: &str) -> String {
    if value.chars().count() <= 18 {
        value.to_owned()
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
