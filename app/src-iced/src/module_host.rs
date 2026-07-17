//! Static composition boundary for package-ready user views.
//!
//! This deliberately uses concrete enums rather than a dynamic plugin ABI or
//! trait registry. Every effect and event retains its originating view until
//! that view's reducer consumes it.

use iced::Element;

use crate::screens::user;
use crate::theme;
use crate::view_api::{AppIntent, Route, ViewId};

/// Render a built-in native view through the same origin-tagged boundary used
/// by its reducer. A future packaged-view runtime can replace this one host
/// entry point without adding capabilities to the view modules themselves.
pub fn view(state: &user::State, view: ViewId, mode: theme::Mode) -> Element<'_, Message> {
    user::view(state, built_in_screen(view), mode)
        .map(move |message| from_built_in_message(view, message))
}

const fn built_in_screen(view: ViewId) -> user::Screen {
    match view {
        ViewId::Home => user::Screen::Home,
        ViewId::Chat => user::Screen::Chat,
        ViewId::Pages => user::Screen::Pages,
        ViewId::Files => user::Screen::Files,
    }
}

fn from_built_in_message(view: ViewId, message: user::Message) -> Message {
    match message {
        user::Message::Load(_) => Message::Load(view),
        user::Message::Home(message) => Message::Home(message),
        user::Message::Chat(message) => Message::Chat(message),
        user::Message::Pages(message) => Message::Pages(message),
        user::Message::Files(message) => Message::Files(message),
        user::Message::AccountTick | user::Message::Service(_) => {
            unreachable!("a built-in view emitted a host-only message")
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Load(ViewId),
    Home(user::HomeMessage),
    Chat(user::ChatMessageEvent),
    Pages(user::PagesMessage),
    Files(user::FilesMessage),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeEffect(user::Command);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatCommand(user::Command);

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum ChatEffect {
    Command(ChatCommand),
    Intent(AppIntent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagesEffect(user::Command);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesEffect(user::Command);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Home(HomeEffect),
    Chat(ChatEffect),
    Pages(PagesEffect),
    Files(FilesEffect),
}

impl Effect {
    #[allow(dead_code)]
    pub const fn view(&self) -> ViewId {
        match self {
            Self::Home(_) => ViewId::Home,
            Self::Chat(_) => ViewId::Chat,
            Self::Pages(_) => ViewId::Pages,
            Self::Files(_) => ViewId::Files,
        }
    }

    pub const fn command(&self) -> Option<&user::Command> {
        match self {
            Self::Home(effect) => Some(&effect.0),
            Self::Chat(ChatEffect::Command(effect)) => Some(&effect.0),
            Self::Chat(ChatEffect::Intent(_)) => None,
            Self::Pages(effect) => Some(&effect.0),
            Self::Files(effect) => Some(&effect.0),
        }
    }

    pub const fn intent(&self) -> Option<&AppIntent> {
        match self {
            Self::Chat(ChatEffect::Intent(intent)) => Some(intent),
            _ => None,
        }
    }
}

impl HomeEffect {
    fn new(command: user::Command) -> Option<Self> {
        is_home_command(&command).then_some(Self(command))
    }

    pub(crate) fn into_command(self) -> user::Command {
        self.0
    }
}

impl ChatCommand {
    fn new(command: user::Command) -> Option<Self> {
        is_chat_command(&command).then_some(Self(command))
    }

    pub(crate) fn into_command(self) -> user::Command {
        self.0
    }
}

impl PagesEffect {
    fn new(command: user::Command) -> Option<Self> {
        is_pages_command(&command).then_some(Self(command))
    }

    pub(crate) fn into_command(self) -> user::Command {
        self.0
    }
}

impl FilesEffect {
    fn new(command: user::Command) -> Option<Self> {
        is_files_command(&command).then_some(Self(command))
    }

    pub(crate) fn into_command(self) -> user::Command {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeEvent(user::ServiceEvent);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatEvent(user::ServiceEvent);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagesEvent(user::ServiceEvent);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesEvent(user::ServiceEvent);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Home(HomeEvent),
    Chat(ChatEvent),
    Pages(PagesEvent),
    Files(FilesEvent),
}

impl Event {
    pub(crate) fn home(event: user::ServiceEvent) -> Self {
        Self::Home(HomeEvent::new(event))
    }

    pub(crate) fn chat(event: user::ServiceEvent) -> Self {
        Self::Chat(ChatEvent::new(event))
    }

    pub(crate) fn pages(event: user::ServiceEvent) -> Self {
        Self::Pages(PagesEvent::new(event))
    }

    pub(crate) fn files(event: user::ServiceEvent) -> Self {
        Self::Files(FilesEvent::new(event))
    }
}

impl HomeEvent {
    fn new(event: user::ServiceEvent) -> Self {
        if is_home_event(&event) {
            Self(event)
        } else {
            Self(invalid_service_event(user::Screen::Home))
        }
    }
}

impl ChatEvent {
    fn new(event: user::ServiceEvent) -> Self {
        if is_chat_event(&event) {
            Self(event)
        } else {
            Self(invalid_service_event(user::Screen::Chat))
        }
    }
}

impl PagesEvent {
    fn new(event: user::ServiceEvent) -> Self {
        if is_pages_event(&event) {
            Self(event)
        } else {
            Self(invalid_service_event(user::Screen::Pages))
        }
    }
}

impl FilesEvent {
    fn new(event: user::ServiceEvent) -> Self {
        if is_files_event(&event) {
            Self(event)
        } else {
            Self(invalid_service_event(user::Screen::Files))
        }
    }
}

pub fn update(state: &mut user::State, message: Message) -> Option<Effect> {
    match message {
        Message::Load(view) => {
            let command = user::update(state, user::Message::Load(built_in_screen(view)))?;
            match view {
                ViewId::Home => HomeEffect::new(command).map(Effect::Home),
                ViewId::Chat => ChatCommand::new(command)
                    .map(ChatEffect::Command)
                    .map(Effect::Chat),
                ViewId::Pages => PagesEffect::new(command).map(Effect::Pages),
                ViewId::Files => FilesEffect::new(command).map(Effect::Files),
            }
        }
        Message::Home(message) => user::update(state, user::Message::Home(message))
            .and_then(HomeEffect::new)
            .map(Effect::Home),
        Message::Chat(message) => {
            let intent = chat_intent(&message);
            let command = user::update(state, user::Message::Chat(message));
            match (command, intent) {
                (Some(command), _) => ChatCommand::new(command)
                    .map(ChatEffect::Command)
                    .map(Effect::Chat),
                (None, Some(intent)) => Some(Effect::Chat(ChatEffect::Intent(intent))),
                (None, None) => None,
            }
        }
        Message::Pages(message) => user::update(state, user::Message::Pages(message))
            .and_then(PagesEffect::new)
            .map(Effect::Pages),
        Message::Files(message) => user::update(state, user::Message::Files(message))
            .and_then(FilesEffect::new)
            .map(Effect::Files),
    }
}

pub fn apply_event(state: &mut user::State, event: Event) -> Option<Effect> {
    match event {
        Event::Home(event) => user::update(state, user::Message::Service(event.0))
            .and_then(HomeEffect::new)
            .map(Effect::Home),
        Event::Chat(event) => user::update(state, user::Message::Service(event.0))
            .and_then(ChatCommand::new)
            .map(ChatEffect::Command)
            .map(Effect::Chat),
        Event::Pages(event) => user::update(state, user::Message::Service(event.0))
            .and_then(PagesEffect::new)
            .map(Effect::Pages),
        Event::Files(event) => user::update(state, user::Message::Service(event.0))
            .and_then(FilesEffect::new)
            .map(Effect::Files),
    }
}

fn chat_intent(message: &user::ChatMessageEvent) -> Option<AppIntent> {
    match message {
        user::ChatMessageEvent::OpenLink(user::ChatLink::Page(page)) => {
            Some(AppIntent::Navigate(Route::Page {
                page: page.clone(),
                block: None,
            }))
        }
        user::ChatMessageEvent::OpenLink(user::ChatLink::Forge { repository, number }) => {
            Some(AppIntent::Navigate(Route::Forge {
                repository: repository.clone(),
                item: *number,
            }))
        }
        user::ChatMessageEvent::OpenLink(user::ChatLink::User(key)) => {
            Some(AppIntent::Navigate(Route::Member {
                key: key.clone(),
                account: None,
            }))
        }
        user::ChatMessageEvent::OpenLink(user::ChatLink::Agent { id, .. }) => {
            Some(AppIntent::Navigate(Route::Agent { id: id.clone() }))
        }
        user::ChatMessageEvent::OpenLink(user::ChatLink::External(address)) => {
            Some(AppIntent::OpenExternal(address.clone()))
        }
        user::ChatMessageEvent::PopOutHuddle => Some(AppIntent::PopOutHuddle),
        _ => None,
    }
}

fn is_home_command(command: &user::Command) -> bool {
    matches!(
        command,
        user::Command::LoadHome
            | user::Command::SaveDisplayName(_)
            | user::Command::SetDuckName(_)
            | user::Command::ChooseAvatar
            | user::Command::SaveProfile { .. }
            | user::Command::CopyText(_)
            | user::Command::SwitchWorkspace(_)
            | user::Command::AddNetwork
            | user::Command::LinkDevice
            | user::Command::PollLink
            | user::Command::ApproveLink { .. }
            | user::Command::CancelLink
            | user::Command::ResolveLinkChallenge { .. }
            | user::Command::GenerateLinkResponse { .. }
            | user::Command::StartPhoneEnrollment
            | user::Command::PollPhoneEnrollment
            | user::Command::ApprovePhoneEnrollment { .. }
            | user::Command::CancelPhoneEnrollment
            | user::Command::RemoveMember(_)
            | user::Command::UnbindNode(_)
            | user::Command::SetNodeLabel { .. }
            | user::Command::EnrollTouchId(_)
            | user::Command::DisableTouchId
            | user::Command::LockAccount
            | user::Command::UnlockAccount
            | user::Command::SecureAccount
            | user::Command::RevealRecovery
    )
}

fn is_chat_command(command: &user::Command) -> bool {
    matches!(
        command,
        user::Command::LoadChat { .. }
            | user::Command::CreateChannel { .. }
            | user::Command::LoadChannel(_)
            | user::Command::SendMessage { .. }
            | user::Command::EditMessage { .. }
            | user::Command::DeleteMessage { .. }
            | user::Command::ChooseChatAttachment
            | user::Command::DownloadChatAttachment(_)
            | user::Command::RenameChannel { .. }
            | user::Command::SetChannelArchived { .. }
            | user::Command::LoadThread { .. }
            | user::Command::SetReaction { .. }
            | user::Command::SetChannelMembership { .. }
            | user::Command::LoadTags(_)
            | user::Command::FilterTag { .. }
            | user::Command::LoadMessageWindow { .. }
            | user::Command::SetHuddle { .. }
    )
}

fn is_pages_command(command: &user::Command) -> bool {
    matches!(
        command,
        user::Command::LoadPages { .. }
            | user::Command::CreatePage { .. }
            | user::Command::LoadPage(_)
            | user::Command::RenamePage { .. }
            | user::Command::SaveBlock { .. }
            | user::Command::SetBlockKind { .. }
            | user::Command::ApplySlash { .. }
            | user::Command::SetBlockChecked { .. }
            | user::Command::RemoveBlock(_)
            | user::Command::AddBlock { .. }
            | user::Command::SplitPageBlock { .. }
            | user::Command::MergePageBlock { .. }
            | user::Command::DeletePage(_)
            | user::Command::SetPageParent { .. }
            | user::Command::SetSpanMark { .. }
            | user::Command::MoveBlock { .. }
            | user::Command::PasteBlocks { .. }
            | user::Command::ReadPageClipboard(_)
            | user::Command::FocusPageBlock(_)
            | user::Command::CommitPageAfter { .. }
            | user::Command::AddPageComment { .. }
            | user::Command::ResolvePageComment { .. }
            | user::Command::DeletePageComment(_)
            | user::Command::EditPageComment { .. }
    )
}

fn is_files_command(command: &user::Command) -> bool {
    matches!(
        command,
        user::Command::LoadFiles { .. }
            | user::Command::LoadFile { .. }
            | user::Command::CreateFolder { .. }
            | user::Command::ChooseFiles { .. }
            | user::Command::ChooseFolder { .. }
            | user::Command::UploadDropped { .. }
            | user::Command::LoadSnapshot { .. }
            | user::Command::DownloadFile { .. }
            | user::Command::DeleteFile(_)
            | user::Command::LoadFileDiff { .. }
    )
}

fn is_home_event(event: &user::ServiceEvent) -> bool {
    matches!(
        event,
        user::ServiceEvent::HomeLoaded(_)
            | user::ServiceEvent::AvatarChosen(_)
            | user::ServiceEvent::HomeProfileFinished(_)
            | user::ServiceEvent::LinkStarted(_)
            | user::ServiceEvent::LinkPolled(_)
            | user::ServiceEvent::ResponderChallengeResolved(_)
            | user::ServiceEvent::ResponderResponseGenerated(_)
            | user::ServiceEvent::PhoneEnrollmentStarted(_)
            | user::ServiceEvent::PhoneEnrollmentPolled(_)
            | user::ServiceEvent::AccountActionFinished(_)
            | user::ServiceEvent::ActionFinished {
                screen: user::Screen::Home,
                ..
            }
    )
}

fn is_chat_event(event: &user::ServiceEvent) -> bool {
    matches!(
        event,
        user::ServiceEvent::ChatLoaded(_)
            | user::ServiceEvent::ChannelLoaded(_)
            | user::ServiceEvent::MessageWindowLoaded { .. }
            | user::ServiceEvent::ThreadLoaded(_)
            | user::ServiceEvent::ChatTagsLoaded(_)
            | user::ServiceEvent::ChatHitsLoaded(_)
            | user::ServiceEvent::ChatAttachmentUploaded(_)
            | user::ServiceEvent::ActionFinished {
                screen: user::Screen::Chat,
                ..
            }
    )
}

fn is_pages_event(event: &user::ServiceEvent) -> bool {
    matches!(
        event,
        user::ServiceEvent::PagesLoaded(_)
            | user::ServiceEvent::PageLoaded(_)
            | user::ServiceEvent::ActionFinished {
                screen: user::Screen::Pages,
                ..
            }
    )
}

fn is_files_event(event: &user::ServiceEvent) -> bool {
    matches!(
        event,
        user::ServiceEvent::FilesLoaded(_)
            | user::ServiceEvent::FileLoaded(_)
            | user::ServiceEvent::FileDiffLoaded(_)
            | user::ServiceEvent::ActionFinished {
                screen: user::Screen::Files,
                ..
            }
    )
}

fn invalid_service_event(screen: user::Screen) -> user::ServiceEvent {
    user::ServiceEvent::ActionFinished {
        screen,
        result: Err("host returned an event for the wrong view".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_static_views_render_through_the_native_boundary() {
        let state = user::State::default();
        for view_id in [ViewId::Home, ViewId::Chat, ViewId::Pages, ViewId::Files] {
            drop(view(&state, view_id, theme::Mode::Light));
        }
    }

    #[test]
    fn retry_loads_keep_their_view_origin() {
        for view_id in [ViewId::Home, ViewId::Chat, ViewId::Pages, ViewId::Files] {
            assert_eq!(
                from_built_in_message(view_id, user::Message::Load(user::Screen::Files)),
                Message::Load(view_id)
            );
        }
    }

    #[test]
    fn effects_keep_their_origin_and_reject_cross_view_commands() {
        let mut state = user::State::default();
        let home = update(&mut state, Message::Home(user::HomeMessage::ChooseAvatar)).unwrap();
        assert_eq!(home.view(), ViewId::Home);
        assert!(HomeEffect::new(user::Command::LoadFiles { path: "/".into() }).is_none());

        state.chat.active_channel = Some("general".into());
        let chat = update(&mut state, Message::Chat(user::ChatMessageEvent::LoadTags)).unwrap();
        assert_eq!(chat.view(), ViewId::Chat);
        assert!(ChatCommand::new(user::Command::DeletePage("page".into())).is_none());

        let pages = update(&mut state, Message::Pages(user::PagesMessage::Refresh)).unwrap();
        assert_eq!(pages.view(), ViewId::Pages);

        let files = update(&mut state, Message::Files(user::FilesMessage::Refresh)).unwrap();
        assert_eq!(files.view(), ViewId::Files);
    }

    #[test]
    fn cross_view_chat_actions_leave_the_reducer_as_typed_intents() {
        let mut state = user::State::default();
        let effect = update(
            &mut state,
            Message::Chat(user::ChatMessageEvent::OpenLink(user::ChatLink::Page(
                "page-7".into(),
            ))),
        )
        .unwrap();
        assert_eq!(
            effect.intent(),
            Some(&AppIntent::Navigate(Route::Page {
                page: "page-7".into(),
                block: None,
            }))
        );

        let effect = update(
            &mut state,
            Message::Chat(user::ChatMessageEvent::PopOutHuddle),
        )
        .unwrap();
        assert_eq!(effect.intent(), Some(&AppIntent::PopOutHuddle));
    }

    #[test]
    fn service_events_cannot_be_retagged_to_another_view() {
        let event = Event::home(user::ServiceEvent::FilesLoaded(Ok(None)));
        let Event::Home(HomeEvent(event)) = event else {
            panic!("event lost its Home origin");
        };
        assert!(matches!(
            event,
            user::ServiceEvent::ActionFinished {
                screen: user::Screen::Home,
                result: Err(_),
            }
        ));
    }
}
