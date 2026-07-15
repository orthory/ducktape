//! Home, Chat, Pages, and Files native surfaces.
//!
//! Views are intentionally transport-free. [`update`] emits a typed
//! [`Command`]; the host performs it and returns a [`ServiceEvent`].

use iced::widget::{
    Button, Column, Space, Svg, button, column, container, image, rich_text, row, scrollable, span,
    svg, text, text_input,
};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Shadow, Vector, font};

use crate::icons::{self, Icon};
use crate::theme::{self, MONO, Palette, RADIUS_LG, RADIUS_MD, RADIUS_SM, SANS};
#[cfg(test)]
use crate::view_api::LinkResponse;
pub use crate::view_api::Resource;
use crate::view_api::{DropToken, MemberKeyKind, decode_link_response, encode_link_response};
use qrcode::{QrCode, types::Color as QrColor};
use zeroize::Zeroize as _;

use super::{chat_composer, file_browser, pages};
pub use file_browser::{
    FileDiff, FileEntry, FileKind, FileListing, FilePreview, FilePreviewContent, FileSnapshot,
    Message as FilesMessage, State as FilesState,
};
pub use pages::{
    BlockKind, BlockMove, InlineMark, Message as PagesMessage, PageBlock, PageComment,
    PageCommentThread, PageDocument, PageMeta, PagePresence, PagesData, RelativeAnchor, SpanMark,
    State as PagesState, ThreadMove,
};

const HOME_PAD: f32 = 22.0;
const CHAT_RAIL_WIDTH: f32 = 200.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Home,
    Chat,
    Pages,
    Files,
}

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
    fn set(&mut self, value: String) {
        self.0.zeroize();
        self.0 = value;
    }

    fn clear(&mut self) {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostPolicy {
    Open,
    MembersOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub archived: bool,
    pub policy: PostPolicy,
    pub owner: Option<String>,
    pub huddle: Vec<HuddleMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HuddleMember {
    pub user: String,
    pub node: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reaction {
    pub emoji: String,
    pub count: usize,
    pub self_reacted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatLink {
    Tag(String),
    Page(String),
    File {
        path: String,
        name: String,
    },
    Channel {
        id: String,
        sequence: Option<u64>,
    },
    Forge {
        repository: String,
        number: Option<u64>,
    },
    External(String),
    User(String),
    Agent {
        module: String,
        id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatSpan {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub link: Option<ChatLink>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub sequence: u64,
    pub message_id: String,
    pub revision: u64,
    pub author: String,
    pub body: String,
    pub time: String,
    pub day: Option<String>,
    pub replies: usize,
    pub reactions: Vec<Reaction>,
    pub author_key: Option<String>,
    pub edited: bool,
    pub rich: Vec<ChatSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatThread {
    pub root: ChatMessage,
    pub replies: Vec<ChatMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatTag {
    pub label: String,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatHit {
    pub channel: String,
    pub sequence: u64,
    pub author: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatData {
    pub channels: Vec<Channel>,
    pub messages: Vec<ChatMessage>,
    pub thread: Option<ChatThread>,
    pub members: Vec<String>,
    pub tags: Vec<ChatTag>,
    pub hits: Vec<ChatHit>,
    pub history_window: Option<u64>,
    pub self_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelContent {
    pub messages: Vec<ChatMessage>,
    pub members: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatState {
    pub data: Resource<ChatData>,
    pub active_channel: Option<String>,
    pub draft: chat_composer::State,
    pub creating_channel: bool,
    pub channel_draft: String,
    pub channel_policy: PostPolicy,
    pub reply_draft: chat_composer::State,
    pub editing: Option<(u64, u64)>,
    pub edit_draft: String,
    pub pending_delete: Option<u64>,
    pub attachment_busy: bool,
    pub attachment_for_thread: bool,
    pub page_picker_for_thread: Option<bool>,
    pub rename_draft: String,
    pub member_key_draft: String,
    pub tag_filter: Option<String>,
    pub show_channel_details: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub home: HomeState,
    pub chat: ChatState,
    pub pages: PagesState,
    pub files: FilesState,
}

impl Default for State {
    fn default() -> Self {
        Self {
            home: HomeState {
                data: Resource::Loading,
                display_name_draft: String::new(),
                duck_name_draft: String::new(),
                duck_name_error: None,
                bio_draft: String::new(),
                avatar_edit: AvatarEdit::Keep,
                profile_busy: false,
                error: None,
                account: AccountActionsState::default(),
            },
            chat: ChatState {
                data: Resource::Loading,
                active_channel: None,
                draft: chat_composer::State::new(),
                creating_channel: false,
                channel_draft: String::new(),
                channel_policy: PostPolicy::Open,
                reply_draft: chat_composer::State::new(),
                editing: None,
                edit_draft: String::new(),
                pending_delete: None,
                attachment_busy: false,
                attachment_for_thread: false,
                page_picker_for_thread: None,
                rename_draft: String::new(),
                member_key_draft: String::new(),
                tag_filter: None,
                show_channel_details: false,
                error: None,
            },
            pages: PagesState::default(),
            files: FilesState::default(),
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

#[derive(Debug, Clone, PartialEq)]
pub enum ChatMessageEvent {
    SelectChannel(String),
    ToggleNewChannel,
    ChannelNameChanged(String),
    SetPolicy(PostPolicy),
    CreateChannel,
    Composer {
        thread: bool,
        message: chat_composer::Message,
    },
    ToggleDetails,
    RenameChanged(String),
    Rename,
    SetArchived(bool),
    OpenThread(u64),
    CloseThread,
    BeginEdit(u64, u64, String),
    EditChanged(String),
    CommitEdit,
    CancelEdit,
    RequestDeleteMessage(u64),
    ConfirmDeleteMessage,
    CancelDeleteMessage,
    OpenLink(ChatLink),
    React {
        sequence: u64,
        emoji: String,
        remove: bool,
    },
    MemberKeyChanged(String),
    SetMembership(bool),
    LoadTags,
    FilterTag(String),
    ClearTag,
    FocusMessage(u64),
    JoinHuddle,
    LeaveHuddle,
    PopOutHuddle,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Load(Screen),
    AccountTick,
    Home(HomeMessage),
    Chat(ChatMessageEvent),
    Pages(PagesMessage),
    Files(FilesMessage),
    Service(ServiceEvent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    LoadHome,
    LoadChat {
        active: Option<String>,
    },
    LoadPages {
        active: Option<String>,
        open_tabs: Vec<String>,
    },
    LoadFiles {
        path: String,
    },
    SaveDisplayName(String),
    SetDuckName(Option<String>),
    ChooseAvatar,
    SaveProfile {
        bio: String,
        avatar: AvatarEdit,
    },
    CopyText(String),
    SwitchWorkspace(String),
    AddNetwork,
    LinkDevice,
    PollLink,
    ApproveLink {
        challenge: LinkChallengeView,
        response: String,
    },
    CancelLink,
    ResolveLinkChallenge {
        input: String,
    },
    GenerateLinkResponse {
        session: LinkResponderSession,
        label: Option<String>,
    },
    StartPhoneEnrollment,
    PollPhoneEnrollment,
    ApprovePhoneEnrollment {
        enrollment: PhoneEnrollmentView,
        candidate: PhoneCandidateView,
        label: Option<String>,
    },
    CancelPhoneEnrollment,
    RemoveMember(String),
    UnbindNode(String),
    SetNodeLabel {
        key: String,
        label: Option<String>,
    },
    EnrollTouchId(String),
    DisableTouchId,
    LockAccount,
    UnlockAccount,
    SecureAccount,
    RevealRecovery,
    CreateChannel {
        name: String,
        policy: PostPolicy,
    },
    LoadChannel(String),
    SendMessage {
        channel: String,
        body: String,
        thread: Option<u64>,
    },
    EditMessage {
        channel: String,
        sequence: u64,
        base_revision: u64,
        body: String,
    },
    DeleteMessage {
        channel: String,
        sequence: u64,
    },
    ChooseChatAttachment,
    DownloadChatAttachment(String),
    RenameChannel {
        channel: String,
        name: String,
    },
    SetChannelArchived {
        channel: String,
        archived: bool,
    },
    LoadThread {
        channel: String,
        root: u64,
    },
    SetReaction {
        channel: String,
        sequence: u64,
        emoji: String,
        remove: bool,
    },
    SetChannelMembership {
        channel: String,
        key: String,
        member: bool,
    },
    LoadTags(String),
    FilterTag {
        channel: String,
        tag: String,
    },
    LoadMessageWindow {
        channel: String,
        sequence: u64,
    },
    SetHuddle {
        channel: String,
        joined: bool,
    },
    CreatePage {
        parent: Option<String>,
    },
    LoadPage(String),
    RenamePage {
        page: String,
        title: String,
    },
    SaveBlock {
        page: String,
        block: PageBlock,
    },
    SetBlockKind {
        block: String,
        kind: BlockKind,
    },
    ApplySlash {
        block: String,
        kind: BlockKind,
        text: String,
    },
    SetBlockChecked {
        block: String,
        checked: bool,
    },
    RemoveBlock(String),
    AddBlock {
        page: String,
        kind: BlockKind,
    },
    SplitPageBlock {
        page: String,
        left: PageBlock,
        right: PageBlock,
        thread_moves: Vec<ThreadMove>,
    },
    MergePageBlock {
        page: String,
        destination: PageBlock,
        source: PageBlock,
        thread_moves: Vec<ThreadMove>,
    },
    DeletePage(String),
    SetPageParent {
        page: String,
        parent: Option<String>,
    },
    SetSpanMark {
        block: String,
        start: usize,
        end: usize,
        kind: InlineMark,
        active: bool,
    },
    MoveBlock {
        block: String,
        parent: String,
        after: Option<String>,
    },
    PasteBlocks {
        parent: String,
        after: Option<String>,
        blocks: Vec<(BlockKind, String, bool)>,
    },
    ReadPageClipboard(usize),
    FocusPageBlock(String),
    CommitPageAfter {
        block: String,
        generation: u64,
    },
    AddPageComment {
        thread: String,
        comment: String,
        target: String,
        anchor: Option<RelativeAnchor>,
        text: String,
    },
    ResolvePageComment {
        thread: String,
        resolved: bool,
    },
    DeletePageComment(String),
    EditPageComment {
        comment: String,
        text: String,
    },
    LoadFile {
        path: String,
        snapshot: Option<String>,
    },
    CreateFolder {
        parent: String,
        name: String,
    },
    ChooseFiles {
        target: String,
    },
    ChooseFolder {
        target: String,
    },
    UploadDropped {
        target: String,
        token: DropToken,
    },
    LoadSnapshot {
        id: Option<String>,
        path: String,
    },
    DownloadFile {
        path: String,
        size: u64,
        snapshot: Option<String>,
    },
    BeginFileDragOut {
        path: String,
        size: u64,
        snapshot: Option<String>,
    },
    DeleteFile(String),
    LoadFileDiff {
        from: String,
        to: String,
        prefix: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceEvent {
    HomeLoaded(Result<Option<HomeData>, String>),
    AvatarChosen(Result<Option<AvatarDraft>, String>),
    HomeProfileFinished(Result<(), String>),
    ChatLoaded(Result<Option<ChatData>, String>),
    ChannelLoaded(Result<ChannelContent, String>),
    MessageWindowLoaded {
        sequence: u64,
        result: Result<Vec<ChatMessage>, String>,
    },
    ThreadLoaded(Result<ChatThread, String>),
    ChannelMembersLoaded(Result<Vec<String>, String>),
    ChatTagsLoaded(Result<Vec<ChatTag>, String>),
    ChatHitsLoaded(Result<Vec<ChatHit>, String>),
    ChatAttachmentUploaded(Result<String, String>),
    PagesLoaded(Result<Option<PagesData>, String>),
    PageLoaded(Result<PageDocument, String>),
    FilesLoaded(Result<Option<FileListing>, String>),
    FileLoaded(Result<FilePreview, String>),
    FileDiffLoaded(Result<Vec<FileDiff>, String>),
    FileDragOutUnavailable(String),
    LinkStarted(Result<LinkSession, String>),
    LinkPolled(Result<Option<LinkReplyPreview>, String>),
    ResponderChallengeResolved(Result<LinkResponderSession, String>),
    ResponderResponseGenerated(Result<LinkResponderReply, String>),
    PhoneEnrollmentStarted(Result<PhoneEnrollmentView, String>),
    PhoneEnrollmentPolled(Result<Option<PhoneCandidateView>, String>),
    AccountActionFinished(Result<(), String>),
    ActionFinished {
        screen: Screen,
        result: Result<(), String>,
    },
}

pub fn update(state: &mut State, message: Message) -> Option<Command> {
    match message {
        Message::Load(screen) => load(state, screen),
        Message::AccountTick => account_tick(state),
        Message::Home(message) => update_home(&mut state.home, message),
        Message::Chat(message) => update_chat(&mut state.chat, message),
        Message::Pages(message) => update_pages(&mut state.pages, message),
        Message::Files(message) => update_files(&mut state.files, message),
        Message::Service(event) => service_event(state, event),
    }
}

fn account_tick(state: &State) -> Option<Command> {
    match state.home.account.panel {
        AccountPanel::Link
            if state.home.account.link.is_some()
                && state.home.account.link_response.trim().is_empty() =>
        {
            Some(Command::PollLink)
        }
        AccountPanel::Phone
            if state.home.account.phone.is_some()
                && state.home.account.phone_candidate.is_none() =>
        {
            Some(Command::PollPhoneEnrollment)
        }
        AccountPanel::LinkSelf if state.home.account.responder_reply.is_some() => {
            Some(Command::LoadHome)
        }
        _ => None,
    }
}

pub fn account_polling(state: &State) -> bool {
    account_tick(state).is_some()
}

fn load(state: &mut State, screen: Screen) -> Option<Command> {
    match screen {
        Screen::Home => {
            state.home.data = Resource::Loading;
            Some(Command::LoadHome)
        }
        Screen::Chat => {
            state.chat.data = Resource::Loading;
            Some(Command::LoadChat {
                active: state.chat.active_channel.clone(),
            })
        }
        Screen::Pages => {
            let (active, open_tabs) = match &state.pages.data {
                Resource::Ready(data) => (
                    data.document.as_ref().map(|document| document.id.clone()),
                    data.open_tabs.clone(),
                ),
                _ => (None, Vec::new()),
            };
            state.pages.data = Resource::Loading;
            Some(Command::LoadPages { active, open_tabs })
        }
        Screen::Files => {
            let path = listing_path(&state.files).to_owned();
            state.files.data = Resource::Loading;
            Some(Command::LoadFiles { path })
        }
    }
}

fn update_home(state: &mut HomeState, message: HomeMessage) -> Option<Command> {
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
            state.account.node_label_draft =
                (label != "Device").then_some(label).unwrap_or_default();
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

fn update_chat(state: &mut ChatState, message: ChatMessageEvent) -> Option<Command> {
    state.error = None;
    match message {
        ChatMessageEvent::SelectChannel(id) => {
            state.active_channel = Some(id.clone());
            state.tag_filter = None;
            state.reply_draft.clear();
            state.editing = None;
            state.edit_draft.clear();
            state.rename_draft = match &state.data {
                Resource::Ready(data) => data
                    .channels
                    .iter()
                    .find(|channel| channel.id == id)
                    .map(|channel| channel.name.clone())
                    .unwrap_or_default(),
                _ => String::new(),
            };
            if let Resource::Ready(data) = &mut state.data {
                data.thread = None;
                data.hits.clear();
                data.history_window = None;
            }
            Some(Command::LoadChannel(id))
        }
        ChatMessageEvent::ToggleNewChannel => {
            state.creating_channel = !state.creating_channel;
            None
        }
        ChatMessageEvent::ChannelNameChanged(value) => {
            state.channel_draft = value;
            None
        }
        ChatMessageEvent::SetPolicy(policy) => {
            state.channel_policy = policy;
            None
        }
        ChatMessageEvent::CreateChannel => {
            let name = nonempty(&state.channel_draft)?;
            state.channel_draft.clear();
            state.creating_channel = false;
            Some(Command::CreateChannel {
                name,
                policy: state.channel_policy,
            })
        }
        ChatMessageEvent::Composer { thread, message } => {
            let destination = if matches!(&message, chat_composer::Message::Submit) {
                let channel = state.active_channel.clone()?;
                let root = if thread {
                    match &state.data {
                        Resource::Ready(data) => Some(data.thread.as_ref()?.root.sequence),
                        _ => return None,
                    }
                } else {
                    None
                };
                Some((channel, root))
            } else {
                None
            };
            let output = chat_composer::update(
                if thread {
                    &mut state.reply_draft
                } else {
                    &mut state.draft
                },
                message,
            )?;
            match output {
                chat_composer::Output::Submit(body) => {
                    let (channel, thread) = destination?;
                    Some(Command::SendMessage {
                        channel,
                        body,
                        thread,
                    })
                }
                chat_composer::Output::ChooseAttachment => {
                    if state.attachment_busy {
                        return None;
                    }
                    state.attachment_busy = true;
                    state.attachment_for_thread = thread;
                    Some(Command::ChooseChatAttachment)
                }
                chat_composer::Output::TogglePagePicker => {
                    state.page_picker_for_thread =
                        (state.page_picker_for_thread != Some(thread)).then_some(thread);
                    None
                }
                chat_composer::Output::ReferenceInserted => {
                    state.page_picker_for_thread = None;
                    None
                }
            }
        }
        ChatMessageEvent::ToggleDetails => {
            state.show_channel_details = !state.show_channel_details;
            None
        }
        ChatMessageEvent::RenameChanged(value) => {
            state.rename_draft = value;
            None
        }
        ChatMessageEvent::Rename => Some(Command::RenameChannel {
            channel: state.active_channel.clone()?,
            name: nonempty(&state.rename_draft)?,
        }),
        ChatMessageEvent::SetArchived(archived) => Some(Command::SetChannelArchived {
            channel: state.active_channel.clone()?,
            archived,
        }),
        ChatMessageEvent::OpenThread(root) => Some(Command::LoadThread {
            channel: state.active_channel.clone()?,
            root,
        }),
        ChatMessageEvent::CloseThread => {
            if let Resource::Ready(data) = &mut state.data {
                data.thread = None;
            }
            state.reply_draft.clear();
            None
        }
        ChatMessageEvent::BeginEdit(sequence, revision, body) => {
            state.editing = Some((sequence, revision));
            state.edit_draft = body;
            None
        }
        ChatMessageEvent::EditChanged(value) => {
            state.edit_draft = value;
            None
        }
        ChatMessageEvent::CommitEdit => {
            let channel = state.active_channel.clone()?;
            let (sequence, base_revision) = state.editing.take()?;
            let body = nonempty(&state.edit_draft)?;
            state.edit_draft.clear();
            Some(Command::EditMessage {
                channel,
                sequence,
                base_revision,
                body,
            })
        }
        ChatMessageEvent::CancelEdit => {
            state.editing = None;
            state.edit_draft.clear();
            None
        }
        ChatMessageEvent::RequestDeleteMessage(sequence) => {
            state.pending_delete = Some(sequence);
            None
        }
        ChatMessageEvent::ConfirmDeleteMessage => Some(Command::DeleteMessage {
            channel: state.active_channel.clone()?,
            sequence: state.pending_delete.take()?,
        }),
        ChatMessageEvent::CancelDeleteMessage => {
            state.pending_delete = None;
            None
        }
        ChatMessageEvent::OpenLink(ChatLink::File { path, .. }) => {
            Some(Command::DownloadChatAttachment(path))
        }
        ChatMessageEvent::OpenLink(ChatLink::Tag(tag)) => {
            state.tag_filter = Some(tag.clone());
            Some(Command::FilterTag {
                channel: state.active_channel.clone()?,
                tag,
            })
        }
        ChatMessageEvent::OpenLink(ChatLink::Channel { id, sequence }) => {
            state.active_channel = Some(id.clone());
            state.tag_filter = None;
            if let Resource::Ready(data) = &mut state.data {
                data.thread = None;
                data.hits.clear();
            }
            match sequence {
                Some(sequence) => Some(Command::LoadMessageWindow {
                    channel: id,
                    sequence,
                }),
                None => Some(Command::LoadChannel(id)),
            }
        }
        ChatMessageEvent::OpenLink(_) => None,
        ChatMessageEvent::React {
            sequence,
            emoji,
            remove,
        } => Some(Command::SetReaction {
            channel: state.active_channel.clone()?,
            sequence,
            emoji,
            remove,
        }),
        ChatMessageEvent::MemberKeyChanged(value) => {
            state.member_key_draft = value;
            None
        }
        ChatMessageEvent::SetMembership(member) => Some(Command::SetChannelMembership {
            channel: state.active_channel.clone()?,
            key: nonempty(&state.member_key_draft)?,
            member,
        }),
        ChatMessageEvent::LoadTags => Some(Command::LoadTags(state.active_channel.clone()?)),
        ChatMessageEvent::FilterTag(tag) => {
            state.tag_filter = Some(tag.clone());
            Some(Command::FilterTag {
                channel: state.active_channel.clone()?,
                tag,
            })
        }
        ChatMessageEvent::ClearTag => {
            state.tag_filter = None;
            if let Resource::Ready(data) = &mut state.data {
                data.hits.clear();
            }
            Some(Command::LoadChannel(state.active_channel.clone()?))
        }
        ChatMessageEvent::FocusMessage(sequence) => Some(Command::LoadMessageWindow {
            channel: state.active_channel.clone()?,
            sequence,
        }),
        ChatMessageEvent::JoinHuddle => Some(Command::SetHuddle {
            channel: state.active_channel.clone()?,
            joined: true,
        }),
        ChatMessageEvent::LeaveHuddle => Some(Command::SetHuddle {
            channel: state.active_channel.clone()?,
            joined: false,
        }),
        ChatMessageEvent::PopOutHuddle => None,
    }
}

fn update_pages(state: &mut PagesState, message: PagesMessage) -> Option<Command> {
    pages::update(state, message).map(page_effect)
}

fn page_effect(effect: pages::Effect) -> Command {
    match effect {
        pages::Effect::CreatePage { parent } => Command::CreatePage { parent },
        pages::Effect::LoadPages { active, open_tabs } => Command::LoadPages { active, open_tabs },
        pages::Effect::LoadPage(page) => Command::LoadPage(page),
        pages::Effect::RenamePage { page, title } => Command::RenamePage { page, title },
        pages::Effect::SaveBlock { page, block } => Command::SaveBlock { page, block },
        pages::Effect::SetBlockKind { block, kind } => Command::SetBlockKind { block, kind },
        pages::Effect::ApplySlash { block, kind, text } => {
            Command::ApplySlash { block, kind, text }
        }
        pages::Effect::SetBlockChecked { block, checked } => {
            Command::SetBlockChecked { block, checked }
        }
        pages::Effect::RemoveBlock(block) => Command::RemoveBlock(block),
        pages::Effect::AddBlock { page, kind } => Command::AddBlock { page, kind },
        pages::Effect::SplitBlock {
            page,
            left,
            right,
            thread_moves,
        } => Command::SplitPageBlock {
            page,
            left,
            right,
            thread_moves,
        },
        pages::Effect::MergeBlock {
            page,
            destination,
            source,
            thread_moves,
        } => Command::MergePageBlock {
            page,
            destination,
            source,
            thread_moves,
        },
        pages::Effect::DeletePage(page) => Command::DeletePage(page),
        pages::Effect::SetPageParent { page, parent } => Command::SetPageParent { page, parent },
        pages::Effect::SetSpanMark {
            block,
            start,
            end,
            kind,
            active,
        } => Command::SetSpanMark {
            block,
            start,
            end,
            kind,
            active,
        },
        pages::Effect::MoveBlock {
            block,
            parent,
            after,
        } => Command::MoveBlock {
            block,
            parent,
            after,
        },
        pages::Effect::PasteBlocks {
            parent,
            after,
            blocks,
        } => Command::PasteBlocks {
            parent,
            after,
            blocks,
        },
        pages::Effect::ReadClipboard(index) => Command::ReadPageClipboard(index),
        pages::Effect::FocusBlock(block) => Command::FocusPageBlock(block),
        pages::Effect::CommitAfter { block, generation } => {
            Command::CommitPageAfter { block, generation }
        }
        pages::Effect::AddComment {
            thread,
            comment,
            target,
            anchor,
            text,
        } => Command::AddPageComment {
            thread,
            comment,
            target,
            anchor,
            text,
        },
        pages::Effect::ResolveComment { thread, resolved } => {
            Command::ResolvePageComment { thread, resolved }
        }
        pages::Effect::DeleteComment(comment) => Command::DeletePageComment(comment),
        pages::Effect::EditComment { comment, text } => Command::EditPageComment { comment, text },
    }
}

fn update_files(state: &mut FilesState, message: FilesMessage) -> Option<Command> {
    file_browser::update(state, message).map(file_effect)
}

fn file_effect(effect: file_browser::Effect) -> Command {
    match effect {
        file_browser::Effect::LoadDirectory { path } => Command::LoadFiles { path },
        file_browser::Effect::LoadFile { path, snapshot } => Command::LoadFile { path, snapshot },
        file_browser::Effect::CreateFolder { parent, name } => {
            Command::CreateFolder { parent, name }
        }
        file_browser::Effect::ChooseFiles { target } => Command::ChooseFiles { target },
        file_browser::Effect::ChooseFolder { target } => Command::ChooseFolder { target },
        file_browser::Effect::UploadDropped { target, token } => {
            Command::UploadDropped { target, token }
        }
        file_browser::Effect::LoadSnapshot { id, path } => Command::LoadSnapshot { id, path },
        file_browser::Effect::Download {
            path,
            size,
            snapshot,
        } => Command::DownloadFile {
            path,
            size,
            snapshot,
        },
        file_browser::Effect::BeginDragOut {
            path,
            size,
            snapshot,
        } => Command::BeginFileDragOut {
            path,
            size,
            snapshot,
        },
        file_browser::Effect::Delete(path) => Command::DeleteFile(path),
        file_browser::Effect::CompareSnapshot { from, to, prefix } => {
            Command::LoadFileDiff { from, to, prefix }
        }
    }
}

fn service_event(state: &mut State, event: ServiceEvent) -> Option<Command> {
    match event {
        ServiceEvent::HomeLoaded(result) => match result {
            Ok(data) => {
                state.home.data = data.map(Resource::Ready).unwrap_or(Resource::Empty);
                state.home.display_name_draft = match &state.home.data {
                    Resource::Ready(data) => data
                        .profile
                        .as_ref()
                        .map(|profile| profile.display_name.clone())
                        .unwrap_or_default(),
                    _ => String::new(),
                };
                state.home.duck_name_draft = match &state.home.data {
                    Resource::Ready(data) => data
                        .profile
                        .as_ref()
                        .and_then(|profile| profile.duck_name.clone())
                        .unwrap_or_default(),
                    _ => String::new(),
                };
                state.home.bio_draft = match &state.home.data {
                    Resource::Ready(data) => data
                        .profile
                        .as_ref()
                        .and_then(|profile| profile.bio.clone())
                        .unwrap_or_default(),
                    _ => String::new(),
                };
                state.home.duck_name_error = None;
                state.home.avatar_edit = AvatarEdit::Keep;
                state.home.profile_busy = false;
                if let Resource::Ready(data) = &state.home.data {
                    state.home.account.touch_id_available = data.touch_id_available;
                    state.home.account.touch_id_enrolled = data.touch_id_enrolled;
                    if !data.member_keys.is_empty()
                        && state.home.account.panel == AccountPanel::LinkSelf
                    {
                        state.home.account.panel = AccountPanel::None;
                        state.home.account.responder_input.clear();
                        state.home.account.responder_label.clear();
                        state.home.account.responder_session = None;
                        state.home.account.responder_reply = None;
                    }
                }
            }
            Err(error) => state.home.data = Resource::Error(error),
        },
        ServiceEvent::AvatarChosen(result) => {
            state.home.profile_busy = false;
            match result {
                Ok(Some(avatar)) => state.home.avatar_edit = AvatarEdit::Replace(avatar),
                Ok(None) => {}
                Err(error) => state.home.error = Some(error),
            }
        }
        ServiceEvent::HomeProfileFinished(result) => {
            state.home.profile_busy = false;
            match result {
                Ok(()) => {
                    state.home.avatar_edit = AvatarEdit::Keep;
                    return Some(Command::LoadHome);
                }
                Err(error) => state.home.error = Some(error),
            }
        }
        ServiceEvent::ChatLoaded(result) => match result {
            Ok(data) => {
                state.chat.data = data.map(Resource::Ready).unwrap_or(Resource::Empty);
                state.chat.active_channel = match &state.chat.data {
                    Resource::Ready(data) => state
                        .chat
                        .active_channel
                        .as_ref()
                        .filter(|active| data.channels.iter().any(|channel| &channel.id == *active))
                        .cloned()
                        .or_else(|| {
                            data.channels
                                .iter()
                                .find(|channel| !channel.archived)
                                .or_else(|| data.channels.first())
                                .map(|channel| channel.id.clone())
                        }),
                    _ => None,
                };
            }
            Err(error) => state.chat.data = Resource::Error(error),
        },
        ServiceEvent::ChannelLoaded(result) => match result {
            Ok(content) => {
                if let Resource::Ready(data) = &mut state.chat.data {
                    data.messages = content.messages;
                    data.members = content.members;
                    data.history_window = None;
                    data.thread = None;
                }
            }
            Err(error) => state.chat.error = Some(error),
        },
        ServiceEvent::MessageWindowLoaded { sequence, result } => match result {
            Ok(messages) => {
                if let Resource::Ready(data) = &mut state.chat.data {
                    data.messages = messages;
                    data.history_window = Some(sequence);
                }
            }
            Err(error) => state.chat.error = Some(error),
        },
        ServiceEvent::ThreadLoaded(result) => match result {
            Ok(thread) => {
                if let Resource::Ready(data) = &mut state.chat.data {
                    data.thread = Some(thread);
                }
            }
            Err(error) => state.chat.error = Some(error),
        },
        ServiceEvent::ChannelMembersLoaded(result) => match result {
            Ok(members) => {
                if let Resource::Ready(data) = &mut state.chat.data {
                    data.members = members;
                }
            }
            Err(error) => state.chat.error = Some(error),
        },
        ServiceEvent::ChatTagsLoaded(result) => match result {
            Ok(tags) => {
                if let Resource::Ready(data) = &mut state.chat.data {
                    data.tags = tags;
                }
            }
            Err(error) => state.chat.error = Some(error),
        },
        ServiceEvent::ChatHitsLoaded(result) => match result {
            Ok(hits) => {
                if let Resource::Ready(data) = &mut state.chat.data {
                    data.hits = hits;
                }
            }
            Err(error) => state.chat.error = Some(error),
        },
        ServiceEvent::ChatAttachmentUploaded(result) => {
            state.chat.attachment_busy = false;
            match result {
                Ok(reference) => {
                    if state.chat.attachment_for_thread {
                        state.chat.reply_draft.append_reference(&reference);
                    } else {
                        state.chat.draft.append_reference(&reference);
                    }
                }
                Err(error) => state.chat.error = Some(error),
            }
        }
        ServiceEvent::PagesLoaded(result) => state.pages.loaded(result),
        ServiceEvent::PageLoaded(result) => {
            if let Some(effect) = state.pages.document_loaded(result) {
                return Some(page_effect(effect));
            }
        }
        ServiceEvent::FilesLoaded(result) => file_browser::loaded(&mut state.files, result),
        ServiceEvent::FileLoaded(result) => file_browser::preview_loaded(&mut state.files, result),
        ServiceEvent::FileDiffLoaded(result) => file_browser::diff_loaded(&mut state.files, result),
        ServiceEvent::FileDragOutUnavailable(reason) => {
            file_browser::drag_out_unavailable(&mut state.files, reason)
        }
        ServiceEvent::LinkStarted(result) => {
            state.home.account.busy = false;
            match result {
                Ok(session) => state.home.account.link = Some(session),
                Err(error) => state.home.account.error = Some(error),
            }
        }
        ServiceEvent::LinkPolled(result) => match result {
            Ok(Some(reply)) => {
                state.home.account.link_response = reply.response_code.clone();
                state.home.account.link_preview = Some(reply);
            }
            Ok(None) => {}
            Err(error) => state.home.account.error = Some(error),
        },
        ServiceEvent::ResponderChallengeResolved(result) => {
            state.home.account.busy = false;
            match result {
                Ok(session) => state.home.account.responder_session = Some(session),
                Err(error) => state.home.account.error = Some(error),
            }
        }
        ServiceEvent::ResponderResponseGenerated(result) => {
            state.home.account.busy = false;
            match result {
                Ok(reply) => state.home.account.responder_reply = Some(reply),
                Err(error) => state.home.account.error = Some(error),
            }
        }
        ServiceEvent::PhoneEnrollmentStarted(result) => {
            state.home.account.busy = false;
            match result {
                Ok(enrollment) => state.home.account.phone = Some(enrollment),
                Err(error) => state.home.account.error = Some(error),
            }
        }
        ServiceEvent::PhoneEnrollmentPolled(result) => match result {
            Ok(Some(candidate)) => state.home.account.phone_candidate = Some(candidate),
            Ok(None) => {}
            Err(error) => state.home.account.error = Some(error),
        },
        ServiceEvent::AccountActionFinished(result) => {
            state.home.account.busy = false;
            match result {
                Ok(()) => {
                    state.home.account.panel = AccountPanel::None;
                    state.home.account.link = None;
                    state.home.account.phone = None;
                    state.home.account.phone_candidate = None;
                    state.home.account.responder_session = None;
                    state.home.account.responder_reply = None;
                    state.home.account.editing_node = None;
                    state.home.account.touch_id_password.clear();
                    state.home.account.error = None;
                    return Some(Command::LoadHome);
                }
                Err(error) => state.home.account.error = Some(error),
            }
        }
        ServiceEvent::ActionFinished { screen, result } => {
            let error = result.err();
            match screen {
                Screen::Home => state.home.error = error,
                Screen::Chat => {
                    state.chat.error = error;
                    if state.chat.error.is_none() {
                        return Some(Command::LoadChat {
                            active: state.chat.active_channel.clone(),
                        });
                    }
                }
                Screen::Pages => {
                    state.pages.error = error;
                    if state.pages.error.is_none() {
                        let (active, open_tabs) = match &state.pages.data {
                            Resource::Ready(data) => (
                                data.document.as_ref().map(|document| document.id.clone()),
                                data.open_tabs.clone(),
                            ),
                            _ => (None, Vec::new()),
                        };
                        return Some(Command::LoadPages { active, open_tabs });
                    }
                }
                Screen::Files => {
                    state.files.error = error;
                    if state.files.error.is_none() {
                        return match &state.files.data {
                            Resource::Ready(listing) => Some(Command::LoadSnapshot {
                                id: listing.snapshot.clone(),
                                path: listing.path.clone(),
                            }),
                            _ => Some(Command::LoadFiles {
                                path: "/shared".into(),
                            }),
                        };
                    }
                }
            }
        }
    }
    None
}

fn listing_path(state: &FilesState) -> &str {
    file_browser::listing_path(state)
}

fn nonempty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
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

pub fn view(state: &State, screen: Screen, mode: theme::Mode) -> Element<'_, Message> {
    let p = *theme::palette(mode);
    match screen {
        Screen::Home => home_view(&state.home, p),
        Screen::Chat => chat_view(
            &state.chat,
            match &state.pages.data {
                Resource::Ready(data) => &data.pages,
                _ => &[],
            },
            p,
        ),
        Screen::Pages => pages_view(&state.pages, p),
        Screen::Files => file_browser::view(&state.files, p).map(Message::Files),
    }
}

fn home_view(state: &HomeState, p: Palette) -> Element<'_, Message> {
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
        for workspace in &data.workspaces {
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
            &format!(
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
            &format!(
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
                .push(
                    plain_input(
                        "e.g. Kim's laptop",
                        &state.node_label_draft,
                        |value| Message::Home(HomeMessage::NodeLabelChanged(value)),
                        p,
                    )
                    .on_submit(Message::Home(HomeMessage::CommitNodeLabel)),
                )
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

fn destructive_confirmation(
    description: String,
    confirm: Message,
    cancel: Message,
    p: Palette,
) -> Element<'static, Message> {
    container(
        column![
            text("Confirm deletion")
                .font(SANS)
                .size(12.5)
                .color(p.danger),
            text(description).font(SANS).size(11).color(p.muted),
            row![
                outline("Cancel", cancel, p),
                danger_outline("Delete", confirm, p),
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

fn danger_outline<'a>(label: impl ToString, message: Message, p: Palette) -> Button<'a, Message> {
    button(text(label.to_string()).font(SANS).size(12))
        .padding([7, 10])
        .style(move |_, status| iced::widget::button::Style {
            background: Some(Background::Color(
                if matches!(status, iced::widget::button::Status::Hovered) {
                    p.danger_soft
                } else {
                    p.paper
                },
            )),
            text_color: p.danger,
            border: Border {
                color: p.danger_border,
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            ..Default::default()
        })
        .on_press(message)
}

fn code_box(value: &str, p: Palette) -> Element<'static, Message> {
    container(
        text(value.to_string())
            .font(MONO)
            .size(8.5)
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

fn qr_svg(value: &str) -> Result<String, String> {
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

fn chat_view<'a>(state: &'a ChatState, pages: &'a [PageMeta], p: Palette) -> Element<'a, Message> {
    let data = match &state.data {
        Resource::Loading => {
            return center_state(
                "Loading chat…",
                "Reading channels and messages.",
                Icon::Chat,
                p,
            );
        }
        Resource::Empty => return chat_empty_shell(state, None, pages, p),
        Resource::Error(error) => return error_state("Couldn't load Chat", error, Screen::Chat, p),
        Resource::Ready(data) => data,
    };
    chat_empty_shell(state, Some(data), pages, p)
}

fn chat_empty_shell<'a>(
    state: &'a ChatState,
    data: Option<&'a ChatData>,
    pages: &'a [PageMeta],
    p: Palette,
) -> Element<'a, Message> {
    let channels = data
        .map(|data| data.channels.as_slice())
        .unwrap_or_default();
    let active = channels
        .iter()
        .find(|channel| Some(&channel.id) == state.active_channel.as_ref());
    let mut rail = column![
        row![
            text("CHANNELS").font(SANS).size(11).color(p.muted),
            Space::new().width(Length::Fill),
            outline("+", Message::Chat(ChatMessageEvent::ToggleNewChannel), p)
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(7)
    .padding([13, 8]);
    if state.creating_channel {
        rail = rail.push(
            column![
                field(
                    "channel name",
                    &state.channel_draft,
                    |value| Message::Chat(ChatMessageEvent::ChannelNameChanged(value)),
                    p
                ),
                policy_toggle(state.channel_policy, p),
                filled(
                    "Create channel",
                    Message::Chat(ChatMessageEvent::CreateChannel),
                    !state.channel_draft.trim().is_empty(),
                    p
                ),
            ]
            .spacing(6),
        );
    }
    let visible: Vec<_> = channels
        .iter()
        .filter(|channel| !channel.archived)
        .collect();
    if visible.is_empty() {
        rail = rail.push(
            text("No channels yet — create one.")
                .font(SANS)
                .size(11.5)
                .color(p.muted_2),
        );
    } else {
        for channel in visible {
            rail = rail.push(channel_button(
                channel,
                Some(&channel.id) == state.active_channel.as_ref(),
                p,
            ));
        }
    }
    let archived: Vec<_> = channels.iter().filter(|channel| channel.archived).collect();
    if !archived.is_empty() {
        rail = rail.push(
            text(format!("ARCHIVED · {}", archived.len()))
                .font(SANS)
                .size(11)
                .color(p.muted),
        );
        for channel in archived {
            rail = rail.push(channel_button(
                channel,
                Some(&channel.id) == state.active_channel.as_ref(),
                p,
            ));
        }
    }
    let rail = container(scrollable(rail))
        .width(CHAT_RAIL_WIDTH)
        .height(Length::Fill)
        .style(move |_| panel(p.sidebar, p.border_soft));

    let lane: Element<'a, Message> = if let Some(channel) = active {
        let messages = data
            .map(|data| data.messages.as_slice())
            .unwrap_or_default();
        let roots = messages.len();
        let self_in_huddle = data
            .and_then(|data| data.self_key.as_deref())
            .is_some_and(|key| {
                channel
                    .huddle
                    .iter()
                    .any(|member| member.user.eq_ignore_ascii_case(key))
            });
        let mut header_row = row![
            text("#").font(SANS).size(15).color(p.muted),
            text(&channel.name).font(SANS).size(15).color(p.ink),
            text(format!(
                "· {roots} {}",
                if roots == 1 { "message" } else { "messages" }
            ))
            .font(SANS)
            .size(12)
            .color(p.muted_2),
            Space::new().width(Length::Fill),
            outline("# Tags", Message::Chat(ChatMessageEvent::LoadTags), p),
            outline(
                if self_in_huddle {
                    format!("Leave huddle · {}", channel.huddle.len())
                } else if channel.huddle.is_empty() {
                    "Start huddle".into()
                } else {
                    format!("Join huddle · {}", channel.huddle.len())
                },
                Message::Chat(if self_in_huddle {
                    ChatMessageEvent::LeaveHuddle
                } else {
                    ChatMessageEvent::JoinHuddle
                }),
                p,
            ),
            outline("…", Message::Chat(ChatMessageEvent::ToggleDetails), p),
        ]
        .spacing(9)
        .align_y(Alignment::Center);
        if self_in_huddle {
            header_row = header_row.push(outline(
                "Pop out",
                Message::Chat(ChatMessageEvent::PopOutHuddle),
                p,
            ));
        }
        let header = container(header_row)
            .height(50)
            .padding([0, 18])
            .align_y(Alignment::Center)
            .style(move |_| bottom_border(p.paper, p.border_soft));
        let details: Element<'a, Message> = if state.show_channel_details {
            let member_summary = data
                .map(|data| {
                    format!(
                        "{} admitted member{}",
                        data.members.len(),
                        if data.members.len() == 1 { "" } else { "s" }
                    )
                })
                .unwrap_or_default();
            container(
                column![
                    row![
                        field(
                            "Channel name",
                            &state.rename_draft,
                            |value| Message::Chat(ChatMessageEvent::RenameChanged(value)),
                            p
                        ),
                        outline("Rename", Message::Chat(ChatMessageEvent::Rename), p),
                        outline(
                            if channel.archived {
                                "Unarchive"
                            } else {
                                "Archive"
                            },
                            Message::Chat(ChatMessageEvent::SetArchived(!channel.archived)),
                            p,
                        )
                    ]
                    .spacing(8),
                    row![
                        field(
                            "32-byte member key (hex)",
                            &state.member_key_draft,
                            |value| Message::Chat(ChatMessageEvent::MemberKeyChanged(value)),
                            p
                        ),
                        outline(
                            "Add",
                            Message::Chat(ChatMessageEvent::SetMembership(true)),
                            p
                        ),
                        outline(
                            "Remove",
                            Message::Chat(ChatMessageEvent::SetMembership(false)),
                            p
                        ),
                    ]
                    .spacing(8),
                    text(member_summary).font(MONO).size(10.5).color(p.muted_2),
                ]
                .spacing(8),
            )
            .padding([10, 18])
            .style(move |_| bottom_border(p.sunken, p.border_soft))
            .into()
        } else {
            Space::new().height(0).into()
        };
        let mut stream = column![channel_intro(&channel.name, messages.is_empty(), p)]
            .spacing(1)
            .padding([14, 18]);
        if let Some(tag) = &state.tag_filter {
            stream = stream.push(
                row![
                    text(format!(
                        "#{tag} · {} messages",
                        data.map(|data| data.hits.len()).unwrap_or_default()
                    ))
                    .font(SANS)
                    .size(12.5)
                    .color(theme::ACCENTS[0]),
                    Space::new().width(Length::Fill),
                    outline("Clear", Message::Chat(ChatMessageEvent::ClearTag), p),
                ]
                .align_y(Alignment::Center),
            );
            if let Some(data) = data {
                for hit in &data.hits {
                    stream = stream.push(
                        button(
                            column![
                                text(format!("{} · #{}", hit.author, hit.sequence))
                                    .font(MONO)
                                    .size(10.5)
                                    .color(p.muted_2),
                                text(hit.text.clone())
                                    .font(SANS)
                                    .size(13.5)
                                    .color(p.ink_soft),
                            ]
                            .spacing(3),
                        )
                        .width(Length::Fill)
                        .padding([8, 10])
                        .style(move |_, status| iced::widget::button::Style {
                            background: matches!(status, iced::widget::button::Status::Hovered)
                                .then_some(Background::Color(p.hover)),
                            text_color: p.ink,
                            border: Border {
                                radius: RADIUS_SM.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        })
                        .on_press(Message::Chat(ChatMessageEvent::FocusMessage(hit.sequence))),
                    );
                }
            }
        } else {
            if let Some(sequence) = data.and_then(|data| data.history_window) {
                stream = stream.push(notice_owned(format!("Older history around message #{sequence}. Select this channel again to jump to latest."), p));
            }
            if let Some(data) = data {
                if !data.tags.is_empty() {
                    let mut tags = row![].spacing(5);
                    for tag in &data.tags {
                        tags = tags.push(outline(
                            format!("#{} · {}", tag.label, tag.count),
                            Message::Chat(ChatMessageEvent::FilterTag(tag.label.clone())),
                            p,
                        ));
                    }
                    stream = stream.push(tags);
                }
            }
            for message in messages {
                if let Some(day) = &message.day {
                    stream = stream.push(day_divider(day, p));
                }
                stream = stream.push(message_row(
                    message,
                    channel.archived,
                    true,
                    data.and_then(|data| data.self_key.as_deref()),
                    state.editing,
                    &state.edit_draft,
                    data.and_then(|data| data.history_window) == Some(message.sequence),
                    p,
                ));
            }
        }
        let bottom: Element<'a, Message> = if channel.archived {
            container(
                row![
                    text("This channel is archived — posting and reactions are disabled.")
                        .font(SANS)
                        .size(12)
                        .color(p.muted),
                    Space::new().width(Length::Fill),
                    outline(
                        "Unarchive",
                        Message::Chat(ChatMessageEvent::SetArchived(false)),
                        p
                    ),
                ]
                .align_y(Alignment::Center),
            )
            .padding([10, 18])
            .style(move |_| top_border(p.sunken, p.border_soft))
            .into()
        } else {
            composer(state, channel, pages, false, p)
        };
        let mut main =
            column![header, details, scrollable(stream).height(Length::Fill)].width(Length::Fill);
        if let Some(sequence) = state.pending_delete {
            main = main.push(destructive_confirmation(
                format!("Delete message #{sequence}? This cannot be undone."),
                Message::Chat(ChatMessageEvent::ConfirmDeleteMessage),
                Message::Chat(ChatMessageEvent::CancelDeleteMessage),
                p,
            ));
        }
        let main = main.push(bottom);
        if let Some(thread) = data.and_then(|data| data.thread.as_ref()) {
            row![
                main,
                thread_panel(
                    state,
                    thread,
                    channel,
                    data.and_then(|data| data.self_key.as_deref()),
                    pages,
                    p,
                )
            ]
            .into()
        } else {
            main.into()
        }
    } else {
        let has_channels = !channels.is_empty();
        let center = column![
            icon_tile(Icon::Chat, 48.0, p),
            text(if has_channels {
                "Pick a channel"
            } else {
                "No channels yet"
            })
            .font(SANS)
            .size(14)
            .color(p.ink),
            text(if has_channels {
                "Choose a channel from the list to start reading and posting."
            } else {
                "Create the first channel to start the conversation."
            })
            .font(SANS)
            .size(12.5)
            .color(p.muted_2),
        ]
        .spacing(10)
        .align_x(Alignment::Center)
        .max_width(260);
        container(center)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .into()
    };
    row![
        rail,
        container(lane)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| surface(p.paper))
    ]
    .into()
}

fn pages_view(state: &PagesState, p: Palette) -> Element<'_, Message> {
    pages::view(state, p).map(Message::Pages)
}

fn center_state<'a>(
    title: &'a str,
    detail: &'a str,
    icon: Icon,
    p: Palette,
) -> Element<'a, Message> {
    container(
        column![
            icon_tile(icon, 42.0, p),
            text(title).font(SANS).size(14).color(p.muted_3),
            text(detail).font(SANS).size(11.5).color(p.muted_2)
        ]
        .spacing(9)
        .align_x(Alignment::Center)
        .max_width(360),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .padding(24)
    .into()
}

fn error_state<'a>(
    title: &'a str,
    detail: &'a str,
    screen: Screen,
    p: Palette,
) -> Element<'a, Message> {
    container(
        column![
            icon_tile(Icon::Settings, 42.0, p),
            text(title).font(SANS).size(14).color(p.ink),
            text(detail).font(MONO).size(11.5).color(p.red),
            outline("Retry", Message::Load(screen), p)
        ]
        .spacing(9)
        .align_x(Alignment::Center)
        .max_width(360),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .padding(24)
    .into()
}

fn card<'a>(content: impl Into<Element<'a, Message>>, p: Palette) -> Element<'a, Message> {
    container(content)
        .width(Length::Fill)
        .padding(15)
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
fn panel(color: Color, border: Color) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(color)),
        border: Border {
            color: border,
            width: 0.0,
            radius: 0.0.into(),
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

fn field<'a>(
    placeholder: &str,
    value: &'a str,
    on_input: impl Fn(String) -> Message + 'a,
    p: Palette,
) -> iced::widget::TextInput<'a, Message> {
    field_enabled(placeholder, value, on_input, true, p)
}

fn field_enabled<'a>(
    placeholder: &str,
    value: &'a str,
    on_input: impl Fn(String) -> Message + 'a,
    enabled: bool,
    p: Palette,
) -> iced::widget::TextInput<'a, Message> {
    text_input(placeholder, value)
        .on_input_maybe(enabled.then_some(on_input))
        .padding([8, 10])
        .size(12.5)
        .font(SANS)
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

fn plain_input<'a>(
    placeholder: &str,
    value: &'a str,
    on_input: impl Fn(String) -> Message + 'a,
    p: Palette,
) -> iced::widget::TextInput<'a, Message> {
    plain_input_enabled(placeholder, value, on_input, true, p)
}

fn plain_input_enabled<'a>(
    placeholder: &str,
    value: &'a str,
    on_input: impl Fn(String) -> Message + 'a,
    enabled: bool,
    p: Palette,
) -> iced::widget::TextInput<'a, Message> {
    text_input(placeholder, value)
        .on_input_maybe(enabled.then_some(on_input))
        .padding(0)
        .size(13.5)
        .font(SANS)
        .style(move |_, _| iced::widget::text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            icon: p.muted,
            placeholder: p.muted,
            value: p.ink,
            selection: theme::ACCENTS[0],
        })
}

fn outline<'a>(label: impl ToString, message: Message, p: Palette) -> Button<'a, Message> {
    outline_enabled(label, message, true, p)
}
fn outline_enabled<'a>(
    label: impl ToString,
    message: Message,
    enabled: bool,
    p: Palette,
) -> Button<'a, Message> {
    let button = button(text(label.to_string()).font(SANS).size(12))
        .padding([7, 10])
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

fn filled<'a>(
    label: impl ToString,
    message: Message,
    enabled: bool,
    p: Palette,
) -> Button<'a, Message> {
    let button = button(text(label.to_string()).font(SANS).size(12.5))
        .width(Length::Fill)
        .padding([8, 13])
        .style(move |_, status| iced::widget::button::Style {
            background: Some(Background::Color(if enabled {
                if matches!(status, iced::widget::button::Status::Hovered) {
                    p.ink_soft
                } else {
                    p.filled
                }
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

fn section_label(label: &'static str, p: Palette) -> Element<'static, Message> {
    text(label).font(MONO).size(9).color(p.muted_2).into()
}
fn section_header(
    label: &'static str,
    action: Option<(&'static str, Message)>,
    p: Palette,
) -> Element<'static, Message> {
    let mut content = row![
        text(label).font(MONO).size(9).color(p.muted_2),
        Space::new().width(Length::Fill)
    ]
    .align_y(Alignment::Center);
    if let Some((label, message)) = action {
        content = content.push(outline(label, message, p));
    }
    content.into()
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
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(p.sunken)),
            border: Border {
                color: p.border,
                width: 1.0,
                radius: RADIUS_MD.into(),
            },
            ..Default::default()
        })
        .into()
}
fn notice_owned(copy: String, p: Palette) -> Element<'static, Message> {
    container(text(copy).font(SANS).size(11.5).color(p.muted))
        .width(Length::Fill)
        .padding([10, 13])
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(p.sunken)),
            border: Border {
                color: p.border,
                width: 1.0,
                radius: RADIUS_MD.into(),
            },
            ..Default::default()
        })
        .into()
}
fn error_banner<'a>(copy: &'a str, p: Palette) -> Element<'a, Message> {
    container(text(copy).font(SANS).size(12).color(p.danger))
        .width(Length::Fill)
        .padding([10, 16])
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(p.danger_soft)),
            border: Border {
                color: p.danger_border,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn avatar(name: &str, size: f32, p: Palette) -> Element<'static, Message> {
    container(
        text(
            name.chars()
                .next()
                .unwrap_or('D')
                .to_uppercase()
                .to_string(),
        )
        .font(SANS)
        .size(14)
        .color(p.on_filled),
    )
    .width(size)
    .height(size)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .style(move |_| iced::widget::container::Style {
        background: Some(Background::Color(p.filled)),
        border: Border {
            radius: 99.0.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
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
fn icon_tile(icon: Icon, size: f32, p: Palette) -> Element<'static, Message> {
    container(icons::view(icon, size.min(22.0), p.muted_2))
        .width(size)
        .height(size)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(p.sunken)),
            border: Border {
                color: p.border,
                width: 1.0,
                radius: RADIUS_LG.into(),
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
fn workspace_row(workspace: &WorkspaceRow, p: Palette) -> Element<'static, Message> {
    button(
        row![
            text(workspace.name.clone())
                .font(SANS)
                .size(12)
                .color(p.ink)
                .width(Length::FillPortion(1)),
            text(workspace.network_id.clone())
                .font(MONO)
                .size(11)
                .color(p.muted)
                .width(Length::FillPortion(1)),
            text(if workspace.active {
                workspace.standing.label()
            } else {
                "—"
            })
            .font(MONO)
            .size(9)
            .color(if workspace.active { p.ink } else { p.muted_3 })
            .width(Length::FillPortion(1)),
            text(if workspace.active { "ACTIVE" } else { "Enter" })
                .font(MONO)
                .size(9)
                .color(if workspace.active { p.green } else { p.muted_3 })
                .width(Length::FillPortion(1))
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([9, 12])
    .style(move |_, status| iced::widget::button::Style {
        background: matches!(status, iced::widget::button::Status::Hovered)
            .then_some(Background::Color(p.titlebar)),
        text_color: p.ink,
        border: Border {
            color: p.border_soft,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    })
    .on_press(Message::Home(HomeMessage::SwitchWorkspace(
        workspace.id.clone(),
    )))
    .into()
}

fn policy_toggle(active: PostPolicy, p: Palette) -> Element<'static, Message> {
    row![
        policy_button("Open", PostPolicy::Open, active, p),
        policy_button("Members", PostPolicy::MembersOnly, active, p)
    ]
    .spacing(3)
    .into()
}
fn policy_button(
    label: &'static str,
    policy: PostPolicy,
    active: PostPolicy,
    p: Palette,
) -> Button<'static, Message> {
    button(text(label).font(SANS).size(11))
        .width(Length::FillPortion(1))
        .padding([4, 8])
        .style(move |_, _| iced::widget::button::Style {
            background: (policy == active).then_some(Background::Color(p.paper)),
            text_color: if policy == active { p.ink } else { p.muted_2 },
            border: Border {
                radius: 5.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .on_press(Message::Chat(ChatMessageEvent::SetPolicy(policy)))
}
fn channel_button(channel: &Channel, active: bool, p: Palette) -> Element<'static, Message> {
    button(
        row![
            text("#").font(SANS).size(13).color(p.muted_2),
            text(channel.name.clone())
                .font(SANS)
                .size(13.5)
                .color(if active {
                    p.ink
                } else if channel.archived {
                    p.muted_2
                } else {
                    p.muted_3
                })
        ]
        .spacing(7),
    )
    .width(Length::Fill)
    .padding([6, 9])
    .style(move |_, status| iced::widget::button::Style {
        background: (active || matches!(status, iced::widget::button::Status::Hovered))
            .then_some(Background::Color(p.hover)),
        text_color: p.ink,
        border: Border {
            radius: RADIUS_SM.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .on_press(Message::Chat(ChatMessageEvent::SelectChannel(
        channel.id.clone(),
    )))
    .into()
}
fn channel_intro<'a>(name: &'a str, empty: bool, p: Palette) -> Element<'a, Message> {
    column![
        icon_tile(Icon::Chat, 46.0, p),
        text(format!("#{name}")).font(SANS).size(18).color(p.ink),
        text(format!(
            "This is the very beginning of the #{name} channel.{}",
            if empty {
                " Send the first message to start the conversation."
            } else {
                ""
            }
        ))
        .font(SANS)
        .size(13.5)
        .color(p.muted_2)
    ]
    .spacing(9)
    .padding([6, 2])
    .into()
}
fn day_divider<'a>(day: &'a str, p: Palette) -> Element<'a, Message> {
    row![
        divider(p),
        text(day).font(MONO).size(11).color(p.muted_2),
        divider(p)
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .into()
}
fn message_row<'a>(
    message: &'a ChatMessage,
    archived: bool,
    thread_action: bool,
    self_key: Option<&'a str>,
    editing: Option<(u64, u64)>,
    edit_draft: &'a str,
    focused: bool,
    p: Palette,
) -> Element<'a, Message> {
    let mut actions = row![].spacing(5).align_y(Alignment::Center);
    if thread_action && message.replies > 0 {
        actions = actions.push(outline(
            format!(
                "{} {}",
                message.replies,
                if message.replies == 1 {
                    "reply"
                } else {
                    "replies"
                }
            ),
            Message::Chat(ChatMessageEvent::OpenThread(message.sequence)),
            p,
        ));
    } else if thread_action && !archived {
        actions = actions.push(outline(
            "Reply",
            Message::Chat(ChatMessageEvent::OpenThread(message.sequence)),
            p,
        ));
    }
    for reaction in &message.reactions {
        if !archived {
            actions = actions.push(outline(
                format!("{} {}", reaction.emoji, reaction.count),
                Message::Chat(ChatMessageEvent::React {
                    sequence: message.sequence,
                    emoji: reaction.emoji.clone(),
                    remove: reaction.self_reacted,
                }),
                p,
            ));
        }
    }
    if !archived {
        actions = actions.push(outline(
            "+ 👍",
            Message::Chat(ChatMessageEvent::React {
                sequence: message.sequence,
                emoji: "👍".into(),
                remove: false,
            }),
            p,
        ));
    }
    let is_self = self_key.is_some_and(|self_key| {
        message
            .author_key
            .as_deref()
            .is_some_and(|author| author.eq_ignore_ascii_case(self_key))
    });
    if is_self && !archived {
        actions = actions.push(outline(
            "Edit",
            Message::Chat(ChatMessageEvent::BeginEdit(
                message.sequence,
                message.revision,
                message.body.clone(),
            )),
            p,
        ));
        actions = actions.push(danger_outline(
            "Delete",
            Message::Chat(ChatMessageEvent::RequestDeleteMessage(message.sequence)),
            p,
        ));
    }
    let body: Element<'a, Message> =
        if editing.is_some_and(|(sequence, _)| sequence == message.sequence) {
            column![
                field(
                    "Edit message",
                    edit_draft,
                    |value| Message::Chat(ChatMessageEvent::EditChanged(value)),
                    p,
                )
                .on_submit(Message::Chat(ChatMessageEvent::CommitEdit)),
                row![
                    outline("Cancel", Message::Chat(ChatMessageEvent::CancelEdit), p),
                    outline_enabled(
                        "Save",
                        Message::Chat(ChatMessageEvent::CommitEdit),
                        !edit_draft.trim().is_empty(),
                        p,
                    )
                ]
                .spacing(5)
            ]
            .spacing(5)
            .into()
        } else {
            chat_message_body(message, p)
        };
    container(
        row![
            avatar(&message.author, 32.0, p),
            column![
                row![
                    text(message.author.clone())
                        .font(SANS)
                        .size(13)
                        .color(p.ink),
                    text(message.time.clone())
                        .font(MONO)
                        .size(10)
                        .color(p.muted_2),
                    text(if message.edited { "edited" } else { "" })
                        .font(MONO)
                        .size(9)
                        .color(p.muted_3)
                ]
                .spacing(7),
                body,
                actions
            ]
            .spacing(3)
            .width(Length::Fill)
        ]
        .spacing(10),
    )
    .padding([6, 8])
    .style(move |_| iced::widget::container::Style {
        background: focused.then_some(Background::Color(p.chip)),
        border: Border {
            color: if focused {
                p.border_strong
            } else {
                Color::TRANSPARENT
            },
            width: if focused { 1.0 } else { 0.0 },
            radius: RADIUS_SM.into(),
        },
        ..Default::default()
    })
    .into()
}

fn chat_message_body(message: &ChatMessage, p: Palette) -> Element<'static, Message> {
    if message.rich.is_empty() {
        return text(message.body.clone())
            .font(SANS)
            .size(13.5)
            .color(p.ink_soft)
            .into();
    }
    let mut spans = Vec::with_capacity(message.rich.len());
    for run in &message.rich {
        let mut item = span(run.text.clone()).color(p.ink_soft);
        if run.bold || run.italic {
            item = item.font(Font {
                weight: if run.bold {
                    font::Weight::Semibold
                } else {
                    font::Weight::Normal
                },
                style: if run.italic {
                    font::Style::Italic
                } else {
                    font::Style::Normal
                },
                ..SANS
            });
        }
        if let Some(link) = run.link.clone() {
            item = item.link(link).color(theme::ACCENTS[0]).underline(true);
        }
        spans.push(item);
    }
    rich_text(spans)
        .font(SANS)
        .size(13.5)
        .on_link_click(|link| Message::Chat(ChatMessageEvent::OpenLink(link)))
        .into()
}

fn thread_panel<'a>(
    state: &'a ChatState,
    thread: &'a ChatThread,
    channel: &'a Channel,
    self_key: Option<&'a str>,
    pages: &'a [PageMeta],
    p: Palette,
) -> Element<'a, Message> {
    let mut replies = column![
        message_row(
            &thread.root,
            channel.archived,
            false,
            self_key,
            state.editing,
            &state.edit_draft,
            false,
            p,
        ),
        divider(p)
    ]
    .spacing(5)
    .padding([10, 14]);
    for reply in &thread.replies {
        replies = replies.push(message_row(
            reply,
            channel.archived,
            false,
            self_key,
            state.editing,
            &state.edit_draft,
            false,
            p,
        ));
    }
    let composer: Element<'a, Message> = if channel.archived {
        notice("This channel is archived — thread replies are disabled.", p)
    } else {
        composer(state, channel, pages, true, p)
    };
    container(column![
        container(row![
            text("Thread").font(SANS).size(14).color(p.ink),
            Space::new().width(Length::Fill),
            outline("Close", Message::Chat(ChatMessageEvent::CloseThread), p),
        ])
        .padding([10, 14])
        .style(move |_| bottom_border(p.sidebar, p.border_soft)),
        scrollable(replies).height(Length::Fill),
        composer,
    ])
    .width(328)
    .height(Length::Fill)
    .style(move |_| panel(p.sidebar, p.border_soft))
    .into()
}
fn composer<'a>(
    state: &'a ChatState,
    channel: &'a Channel,
    pages: &'a [PageMeta],
    thread: bool,
    p: Palette,
) -> Element<'a, Message> {
    let draft = if thread {
        &state.reply_draft
    } else {
        &state.draft
    };
    let placeholder = if thread {
        "Reply in thread".to_string()
    } else {
        format!("Message #{}", channel.name)
    };
    chat_composer::view(
        draft,
        placeholder,
        if thread { "Reply" } else { "Send" },
        state.attachment_busy,
        state.page_picker_for_thread == Some(thread),
        pages
            .iter()
            .map(|page| (page.id.as_str(), page.title.as_str())),
        p,
    )
    .map(move |message| Message::Chat(ChatMessageEvent::Composer { thread, message }))
}

pub fn page_block_input_id(block: &str) -> String {
    pages::page_block_input_id(block)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_are_explicit_commands() {
        let mut state = State::default();
        assert_eq!(
            update(&mut state, Message::Load(Screen::Files)),
            Some(Command::LoadFiles {
                path: "/shared".into()
            })
        );
        assert_eq!(state.files.data, Resource::Loading);
    }

    #[test]
    fn service_results_preserve_loading_empty_error_ready_contract() {
        let mut state = State::default();
        update(
            &mut state,
            Message::Service(ServiceEvent::ChatLoaded(Ok(None))),
        );
        assert_eq!(state.chat.data, Resource::Empty);
        update(
            &mut state,
            Message::Service(ServiceEvent::ChatLoaded(Err("offline".into()))),
        );
        assert_eq!(state.chat.data, Resource::Error("offline".into()));
    }

    #[test]
    fn send_trims_and_clears_the_draft() {
        let mut state = State::default();
        state.chat.active_channel = Some("general".into());
        state.chat.draft = "  hello ducks  ".into();
        assert_eq!(
            update(
                &mut state,
                Message::Chat(ChatMessageEvent::Composer {
                    thread: false,
                    message: chat_composer::Message::Submit,
                }),
            ),
            Some(Command::SendMessage {
                channel: "general".into(),
                body: "hello ducks".into(),
                thread: None,
            })
        );
        assert!(state.chat.draft.is_empty());
    }

    #[test]
    fn thread_reply_has_an_independent_draft_and_root() {
        let mut state = State::default();
        state.chat.active_channel = Some("general".into());
        state.chat.draft = "main composer".into();
        state.chat.reply_draft = "  thread reply  ".into();
        state.chat.data = Resource::Ready(ChatData {
            channels: vec![],
            messages: vec![],
            thread: Some(ChatThread {
                root: chat_message(41),
                replies: vec![],
            }),
            members: vec![],
            tags: vec![],
            hits: vec![],
            history_window: None,
            self_key: None,
        });
        assert_eq!(
            update(
                &mut state,
                Message::Chat(ChatMessageEvent::Composer {
                    thread: true,
                    message: chat_composer::Message::Submit,
                }),
            ),
            Some(Command::SendMessage {
                channel: "general".into(),
                body: "thread reply".into(),
                thread: Some(41),
            })
        );
        assert_eq!(state.chat.draft, "main composer");
        assert!(state.chat.reply_draft.is_empty());
    }

    #[test]
    fn composer_attachment_and_page_picker_keep_their_target() {
        let mut state = State::default();
        assert_eq!(
            update(
                &mut state,
                Message::Chat(ChatMessageEvent::Composer {
                    thread: true,
                    message: chat_composer::Message::ChooseAttachment,
                }),
            ),
            Some(Command::ChooseChatAttachment)
        );
        update(
            &mut state,
            Message::Service(ServiceEvent::ChatAttachmentUploaded(Ok(
                "[notes.txt](duck://file/notes.txt)".into(),
            ))),
        );
        assert_eq!(
            state.chat.reply_draft.text(),
            "[notes.txt](duck://file/notes.txt)"
        );
        assert!(state.chat.draft.is_empty());

        update(
            &mut state,
            Message::Chat(ChatMessageEvent::Composer {
                thread: false,
                message: chat_composer::Message::TogglePagePicker,
            }),
        );
        assert_eq!(state.chat.page_picker_for_thread, Some(false));
        update(
            &mut state,
            Message::Chat(ChatMessageEvent::Composer {
                thread: false,
                message: chat_composer::Message::InsertPageRef {
                    id: "page-1".into(),
                    title: "Plan".into(),
                },
            }),
        );
        assert_eq!(state.chat.draft.text(), "[Plan](duck://page/page-1)");
        assert_eq!(state.chat.page_picker_for_thread, None);
    }

    #[test]
    fn chat_focus_and_membership_are_channel_scoped() {
        let mut state = State::default();
        state.chat.active_channel = Some("private".into());
        state.chat.member_key_draft = "ab".repeat(32);
        assert_eq!(
            update(
                &mut state,
                Message::Chat(ChatMessageEvent::FocusMessage(73))
            ),
            Some(Command::LoadMessageWindow {
                channel: "private".into(),
                sequence: 73,
            })
        );
        assert_eq!(
            update(
                &mut state,
                Message::Chat(ChatMessageEvent::SetMembership(true))
            ),
            Some(Command::SetChannelMembership {
                channel: "private".into(),
                key: "ab".repeat(32),
                member: true,
            })
        );
    }

    #[test]
    fn chat_delete_requires_explicit_confirmation() {
        let mut state = State::default();
        state.chat.active_channel = Some("general".into());
        assert_eq!(
            update(
                &mut state,
                Message::Chat(ChatMessageEvent::RequestDeleteMessage(73)),
            ),
            None
        );
        assert_eq!(state.chat.pending_delete, Some(73));

        assert_eq!(
            update(
                &mut state,
                Message::Chat(ChatMessageEvent::CancelDeleteMessage),
            ),
            None
        );
        assert_eq!(state.chat.pending_delete, None);

        update(
            &mut state,
            Message::Chat(ChatMessageEvent::RequestDeleteMessage(73)),
        );
        assert_eq!(
            update(
                &mut state,
                Message::Chat(ChatMessageEvent::ConfirmDeleteMessage),
            ),
            Some(Command::DeleteMessage {
                channel: "general".into(),
                sequence: 73,
            })
        );
        assert_eq!(state.chat.pending_delete, None);
    }

    #[test]
    fn page_edits_emit_only_at_commit_boundary() {
        let mut state = State::default();
        state.pages.loaded(Ok(Some(PagesData {
            pages: vec![],
            open_tabs: vec!["p".into()],
            document: Some(PageDocument {
                id: "p".into(),
                title: "First".into(),
                ancestry: vec![],
                blocks: vec![PageBlock {
                    id: "b".into(),
                    kind: BlockKind::Paragraph,
                    text: "old".into(),
                    depth: 0,
                    checked: false,
                    parent: "p".into(),
                    children: vec![],
                    marks: vec![],
                }],
                page_comments: 0,
                comment_threads: vec![],
                presence: vec![],
                self_key: None,
            }),
        })));
        assert_eq!(
            update(
                &mut state,
                Message::Pages(PagesMessage::BlockAction(
                    0,
                    iced::widget::text_editor::Action::SelectAll,
                )),
            ),
            None
        );
        assert_eq!(
            update(
                &mut state,
                Message::Pages(PagesMessage::BlockAction(
                    0,
                    iced::widget::text_editor::Action::Edit(
                        iced::widget::text_editor::Edit::Paste(std::sync::Arc::new("draft".into())),
                    ),
                ))
            ),
            Some(Command::CommitPageAfter {
                block: "b".into(),
                generation: 1,
            })
        );
        let mut stale = state.pages.document().unwrap().clone();
        stale.blocks[0].text = "stale server copy".into();
        update(
            &mut state,
            Message::Service(ServiceEvent::PageLoaded(Ok(stale))),
        );
        assert_eq!(state.pages.document().unwrap().blocks[0].text, "draft");
        assert_eq!(state.pages.dirty_block, Some(("b".into(), 1)));
        assert!(matches!(
            update(
                &mut state,
                Message::Pages(PagesMessage::CommitBlockIf {
                    block: "b".into(),
                    generation: 1,
                })
            ),
            Some(Command::SaveBlock { block, .. }) if block.text == "draft"
        ));
        assert!(
            matches!(update(&mut state, Message::Pages(PagesMessage::Undo)), Some(Command::SaveBlock { block, .. }) if block.text == "old")
        );
        assert!(
            matches!(update(&mut state, Message::Pages(PagesMessage::Redo)), Some(Command::SaveBlock { block, .. }) if block.text == "draft")
        );
        update(
            &mut state,
            Message::Pages(PagesMessage::BlockAction(
                0,
                iced::widget::text_editor::Action::SelectAll,
            )),
        );
        assert_eq!(
            update(
                &mut state,
                Message::Pages(PagesMessage::BlockAction(
                    0,
                    iced::widget::text_editor::Action::Edit(
                        iced::widget::text_editor::Edit::Paste(std::sync::Arc::new(
                            "/heading".into()
                        )),
                    ),
                ))
            ),
            None
        );
        assert_eq!(
            update(
                &mut state,
                Message::Pages(PagesMessage::ApplySlash(0, BlockKind::Heading1))
            ),
            Some(Command::ApplySlash {
                block: "b".into(),
                kind: BlockKind::Heading1,
                text: String::new(),
            })
        );
    }
    #[test]
    fn file_navigation_uses_kind_to_choose_effect() {
        let mut state = State::default();
        assert_eq!(
            update(
                &mut state,
                Message::Files(FilesMessage::OpenEntry(
                    "/shared/docs".into(),
                    FileKind::Directory
                ))
            ),
            Some(Command::LoadFiles {
                path: "/shared/docs".into()
            })
        );
        assert_eq!(
            update(
                &mut state,
                Message::Files(FilesMessage::OpenEntry(
                    "/shared/readme.md".into(),
                    FileKind::File
                ))
            ),
            Some(Command::LoadFile {
                path: "/shared/readme.md".into(),
                snapshot: None,
            })
        );
    }

    #[test]
    fn successful_refreshes_preserve_user_location() {
        let mut state = State::default();
        state.chat.active_channel = Some("design".into());
        assert_eq!(
            update(
                &mut state,
                Message::Service(ServiceEvent::ActionFinished {
                    screen: Screen::Chat,
                    result: Ok(()),
                })
            ),
            Some(Command::LoadChat {
                active: Some("design".into()),
            })
        );

        state.pages.data = Resource::Ready(PagesData {
            pages: vec![],
            open_tabs: vec!["p1".into(), "p2".into()],
            document: Some(page_document("p2")),
        });
        assert_eq!(
            update(
                &mut state,
                Message::Service(ServiceEvent::ActionFinished {
                    screen: Screen::Pages,
                    result: Ok(()),
                })
            ),
            Some(Command::LoadPages {
                active: Some("p2".into()),
                open_tabs: vec!["p1".into(), "p2".into()],
            })
        );

        state.files.data = Resource::Ready(file_listing(true));
        assert_eq!(
            update(
                &mut state,
                Message::Service(ServiceEvent::ActionFinished {
                    screen: Screen::Files,
                    result: Ok(()),
                })
            ),
            Some(Command::LoadSnapshot {
                id: Some("snapshot-7".into()),
                path: "/shared/design".into(),
            })
        );
    }

    #[test]
    fn snapshot_mode_is_read_only_but_downloadable() {
        let mut state = State::default();
        state.files.data = Resource::Ready(file_listing(true));
        let entry = FileEntry {
            path: "/shared/design/logo.svg".into(),
            name: "logo.svg".into(),
            kind: FileKind::File,
            size: 42,
            executable: false,
        };
        assert_eq!(
            update(
                &mut state,
                Message::Files(FilesMessage::RequestDelete(entry.clone()))
            ),
            None
        );
        assert!(state.files.pending_delete.is_none());
        assert_eq!(
            update(
                &mut state,
                Message::Files(FilesMessage::Download(entry.path.clone(), entry.size))
            ),
            Some(Command::DownloadFile {
                path: entry.path,
                size: 42,
                snapshot: Some("snapshot-7".into()),
            })
        );
    }

    #[test]
    fn page_deletions_require_explicit_confirmation() {
        let mut state = State::default();
        let mut document = page_document("p");
        document.blocks.push(PageBlock {
            id: "b".into(),
            kind: BlockKind::Toggle,
            text: "parent".into(),
            depth: 0,
            checked: false,
            parent: "p".into(),
            children: vec!["child".into()],
            marks: vec![],
        });
        document.blocks.push(PageBlock {
            id: "child".into(),
            kind: BlockKind::Paragraph,
            text: "nested".into(),
            depth: 1,
            checked: false,
            parent: "b".into(),
            children: vec![],
            marks: vec![],
        });
        state.pages.data = Resource::Ready(PagesData {
            pages: vec![],
            open_tabs: vec!["p".into()],
            document: Some(document),
        });
        assert_eq!(
            update(
                &mut state,
                Message::Pages(PagesMessage::RequestRemoveBlock(0))
            ),
            None
        );
        assert_eq!(state.pages.pending_block_delete.as_deref(), Some("b"));
        assert_eq!(
            update(&mut state, Message::Pages(PagesMessage::ConfirmRemoveBlock)),
            Some(Command::RemoveBlock("b".into()))
        );
        assert_eq!(
            update(&mut state, Message::Pages(PagesMessage::RequestDeletePage)),
            None
        );
        assert!(state.pages.pending_page_delete);
        assert_eq!(
            update(&mut state, Message::Pages(PagesMessage::ConfirmDeletePage)),
            Some(Command::DeletePage("p".into()))
        );
    }

    fn chat_message(sequence: u64) -> ChatMessage {
        ChatMessage {
            sequence,
            message_id: format!("m-{sequence}"),
            revision: 0,
            author: "eddy".into(),
            body: "hello".into(),
            time: "00:00".into(),
            day: None,
            replies: 0,
            reactions: vec![],
            author_key: None,
            edited: false,
            rich: vec![],
        }
    }

    fn page_document(id: &str) -> PageDocument {
        PageDocument {
            id: id.into(),
            title: "Page".into(),
            ancestry: vec![],
            blocks: vec![],
            page_comments: 0,
            comment_threads: vec![],
            presence: vec![],
            self_key: None,
        }
    }

    fn file_listing(snapshot: bool) -> FileListing {
        FileListing {
            path: "/shared/design".into(),
            entries: vec![],
            preview: None,
            read_only: snapshot,
            refreshing: false,
            head: Some("head-9".into()),
            snapshot: snapshot.then(|| "snapshot-7".into()),
            history: vec![],
            diff: vec![],
        }
    }

    #[test]
    fn link_approval_requires_a_strictly_decoded_fingerprint() {
        let mut state = State::default();
        let reply = LinkResponse {
            pubkey: "22".repeat(32),
            kind: MemberKeyKind::Ed25519,
            possession: r#"{"signature":{"sig":[1,2,3]}}"#.into(),
            label: Some("Laptop".into()),
        };
        let encoded = encode_link_response(&reply).unwrap();
        update(
            &mut state,
            Message::Home(HomeMessage::LinkResponseChanged(encoded.clone())),
        );
        assert_eq!(
            state
                .home
                .account
                .link_preview
                .as_ref()
                .map(|view| &view.key),
            Some(&reply.pubkey)
        );
        update(
            &mut state,
            Message::Home(HomeMessage::LinkResponseChanged("malformed".into())),
        );
        assert!(state.home.account.link_preview.is_none());
        assert_eq!(
            update(&mut state, Message::Home(HomeMessage::ApproveLink)),
            None
        );
    }

    #[test]
    fn account_tick_polls_only_an_active_ceremony() {
        let mut state = State::default();
        state.home.account.panel = AccountPanel::Phone;
        state.home.account.phone = Some(PhoneEnrollmentView {
            url: "[test]".into(),
            chain_id: "chain".into(),
            account_id: "11".repeat(32),
            nonce: 1,
        });
        assert!(account_polling(&state));
        assert_eq!(
            update(&mut state, Message::AccountTick),
            Some(Command::PollPhoneEnrollment)
        );
        state.home.account.phone_candidate = Some(PhoneCandidateView {
            key: format!("02{}", "11".repeat(32)),
            signature: "22".repeat(64),
        });
        assert!(!account_polling(&state));
        state.home.account.panel = AccountPanel::LinkSelf;
        state.home.account.responder_reply = Some(LinkResponderReply {
            response_code: "response".into(),
            key: "11".repeat(32),
            sent_automatically: false,
        });
        assert_eq!(
            update(&mut state, Message::AccountTick),
            Some(Command::LoadHome)
        );
    }

    #[test]
    fn successful_account_action_clears_secret_and_reloads_home() {
        let mut state = State::default();
        state
            .home
            .account
            .touch_id_password
            .set("correct horse battery staple".into());
        assert_eq!(
            update(
                &mut state,
                Message::Service(ServiceEvent::AccountActionFinished(Ok(())))
            ),
            Some(Command::LoadHome)
        );
        assert!(state.home.account.touch_id_password.is_empty());
        assert_eq!(state.home.account.panel, AccountPanel::None);
    }

    #[test]
    fn qr_renderer_includes_a_quiet_zone_and_dark_modules() {
        let svg = qr_svg("http://192.168.1.2:49152/enroll#0123456789abcdef").unwrap();
        assert!(svg.contains("fill='white'"));
        assert!(svg.contains("fill='black'"));
        assert!(svg.contains("M4 "));
    }

    #[test]
    fn unlinked_responder_confirms_challenge_before_signing() {
        let mut state = State::default();
        assert_eq!(
            update(&mut state, Message::Home(HomeMessage::LinkThisDevice)),
            None
        );
        assert_eq!(state.home.account.panel, AccountPanel::LinkSelf);
        update(
            &mut state,
            Message::Home(HomeMessage::ResponderInputChanged("challenge".into())),
        );
        assert_eq!(
            update(&mut state, Message::Home(HomeMessage::ResolveLinkChallenge)),
            Some(Command::ResolveLinkChallenge {
                input: "challenge".into()
            })
        );
        assert_eq!(
            update(&mut state, Message::Home(HomeMessage::GenerateLinkResponse)),
            None
        );
    }

    #[test]
    fn node_label_edit_trims_and_empty_clears() {
        let mut state = State::default();
        update(
            &mut state,
            Message::Home(HomeMessage::EditNodeLabel("11".repeat(32), "Laptop".into())),
        );
        update(
            &mut state,
            Message::Home(HomeMessage::NodeLabelChanged("   ".into())),
        );
        assert_eq!(
            update(&mut state, Message::Home(HomeMessage::CommitNodeLabel)),
            Some(Command::SetNodeLabel {
                key: "11".repeat(32),
                label: None,
            })
        );
    }
}
