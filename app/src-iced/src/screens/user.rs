//! Home, Chat, Pages, and Files native surfaces.
//!
//! Views are intentionally transport-free. [`update`] emits a typed
//! [`Command`]; the host performs it and returns a [`ServiceEvent`].

use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Alignment, Background, Border, Color, Element, Length, Shadow, Vector};

use crate::icons::{self, Icon};
use crate::theme::{self, MONO, Palette, RADIUS_LG, RADIUS_MD, RADIUS_SM, SANS};
use crate::view_api::DropToken;
#[cfg(test)]
use crate::view_api::LinkResponse;
#[cfg(test)]
use crate::view_api::MemberKeyKind;
pub use crate::view_api::Resource;
#[cfg(test)]
use crate::view_api::encode_link_response;
#[cfg(test)]
use home::qr_svg;

#[cfg(test)]
use super::chat_composer;
use super::{chat, file_browser, home, pages};
pub use chat::{
    Channel, ChannelContent, ChatData, ChatHit, ChatLink, ChatMessage, ChatMessageEvent, ChatSpan,
    ChatState, ChatTag, ChatThread, HuddleMember, PostPolicy, Reaction,
};
pub use file_browser::{
    FileDiff, FileEntry, FileKind, FileListing, FilePreview, FilePreviewContent, FileSnapshot,
    Message as FilesMessage, State as FilesState,
};
#[allow(unused_imports)]
pub use home::{
    AccountActionsState, AccountKeyKind, AccountPanel, AccountProfile, AvatarDraft, AvatarEdit,
    Custody, CustodyStatus, DeviceNetworkGroup, DeviceRow, HomeData, HomeMessage, HomeState,
    LinkChallengeView, LinkReplyPreview, LinkResponderReply, LinkResponderSession, LinkSession,
    MemberKeyRow, PhoneCandidateView, PhoneEnrollmentView, SecretInput, Standing, WorkspaceRow,
};
pub use pages::{
    BlockKind, BlockMove, InlineMark, Message as PagesMessage, PageBlock, PageComment,
    PageCommentThread, PageDocument, PageMeta, PagePresence, PagesData, RelativeAnchor, SpanMark,
    State as PagesState, ThreadMove,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Home,
    Chat,
    Pages,
    Files,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct State {
    pub home: HomeState,
    pub chat: ChatState,
    pub pages: PagesState,
    pub files: FilesState,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
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
        Message::Home(message) => home::update(&mut state.home, message),
        Message::Chat(message) => chat::update(&mut state.chat, message),
        Message::Pages(message) => update_pages(&mut state.pages, message),
        Message::Files(message) => update_files(&mut state.files, message),
        Message::Service(event) => service_event(state, event),
    }
}

fn account_tick(state: &State) -> Option<Command> {
    home::account_tick(&state.home)
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

pub(super) fn nonempty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

pub fn view(state: &State, screen: Screen, mode: theme::Mode) -> Element<'_, Message> {
    let p = *theme::palette(mode);
    match screen {
        Screen::Home => home::view(&state.home, p),
        Screen::Chat => chat::view(
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

pub(super) fn danger_outline<'a>(
    label: impl ToString,
    message: Message,
    p: Palette,
) -> Element<'a, Message> {
    let label = label.to_string();
    let btn = button(text(label.clone()).font(SANS).size(12))
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
        .on_press(message);
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Button, label, btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn pages_view(state: &PagesState, p: Palette) -> Element<'_, Message> {
    pages::view(state, p).map(Message::Pages)
}

pub(super) fn center_state<'a>(
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

pub(super) fn error_state<'a>(
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

pub(super) fn card<'a>(
    content: impl Into<Element<'a, Message>>,
    p: Palette,
) -> Element<'a, Message> {
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

pub(super) fn surface(color: Color) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(color)),
        ..Default::default()
    }
}
pub(super) fn panel(color: Color, border: Color) -> iced::widget::container::Style {
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
pub(super) fn bottom_border(bg: Color, border: Color) -> iced::widget::container::Style {
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
pub(super) fn top_border(bg: Color, border: Color) -> iced::widget::container::Style {
    bottom_border(bg, border)
}

pub(super) fn field<'a>(
    placeholder: &str,
    value: &'a str,
    on_input: impl Fn(String) -> Message + 'a,
    p: Palette,
) -> iced::widget::TextInput<'a, Message> {
    field_enabled(placeholder, value, on_input, true, p)
}

pub(super) fn field_enabled<'a>(
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

pub(super) fn plain_input<'a>(
    placeholder: &str,
    value: &'a str,
    on_input: impl Fn(String) -> Message + 'a,
    p: Palette,
) -> iced::widget::TextInput<'a, Message> {
    plain_input_enabled(placeholder, value, on_input, true, p)
}

pub(super) fn plain_input_enabled<'a>(
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

pub(super) fn outline<'a>(
    label: impl ToString,
    message: Message,
    p: Palette,
) -> Element<'a, Message> {
    outline_enabled(label, message, true, p)
}
pub(super) fn outline_enabled<'a>(
    label: impl ToString,
    message: Message,
    enabled: bool,
    p: Palette,
) -> Element<'a, Message> {
    let label = label.to_string();
    let button = button(text(label.clone()).font(SANS).size(12))
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

pub(super) fn filled<'a>(
    label: impl ToString,
    message: Message,
    enabled: bool,
    p: Palette,
) -> Element<'a, Message> {
    let label = label.to_string();
    let button = button(text(label.clone()).font(SANS).size(12.5))
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

pub(super) fn section_label(label: &'static str, p: Palette) -> Element<'static, Message> {
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(
        iced_agent_plugin::Role::Heading,
        label,
        text(label).font(MONO).size(9).color(p.muted_2),
    );
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    text(label).font(MONO).size(9).color(p.muted_2).into()
}
pub(super) fn section_header(
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
pub(super) fn divider(p: Palette) -> Element<'static, Message> {
    container(Space::new().height(1))
        .width(Length::Fill)
        .style(move |_| surface(p.border))
        .into()
}
pub(super) fn notice<'a>(copy: &'a str, p: Palette) -> Element<'a, Message> {
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
pub(super) fn notice_owned(copy: String, p: Palette) -> Element<'static, Message> {
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
pub(super) fn error_banner<'a>(copy: &'a str, p: Palette) -> Element<'a, Message> {
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

pub(super) fn avatar(name: &str, size: f32, p: Palette) -> Element<'static, Message> {
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

pub(super) fn icon_tile(icon: Icon, size: f32, p: Palette) -> Element<'static, Message> {
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
pub fn page_block_input_id(block: &str) -> String {
    pages::page_block_input_id(block)
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
