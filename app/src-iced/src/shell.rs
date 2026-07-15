use std::collections::BTreeMap;

use iced::widget::{
    Space, button, column, container, image, mouse_area, pick_list, progress_bar, row, scrollable,
    stack, text,
};
use iced::{
    Alignment, Background, Border, Color, Element, Length, Size, Subscription, Task, window,
};

use crate::account_service;
use crate::adapters;
use crate::backend::{
    Backend, IdentityStatus, LinkAddress, LinkPending, LinkResponse, MemberKeyKind,
    PossessionRequest, Workspace, WorkspaceSnapshot, decode_link_challenge, encode_link_response,
};
#[cfg(feature = "cef-browser")]
use crate::browser::{
    Bounds as BrowserBounds, BrowserEvent, BrowserRuntime, ParentWindow, PermissionPrompt,
};
use crate::browser_chrome;
use crate::community_service;
use crate::desktop;
use crate::forge_agents_service;
use crate::huddle_media;
use crate::huddle_session;
use crate::icons::{self, Icon};
use crate::mac_tray;
use crate::module_host;
use crate::notifications;
use crate::onboarding;
use crate::operator_service::{self, DesktopPreferences, SettingsContext};
use crate::page_presence;
use crate::screens::agents as agents_screen;
use crate::screens::explorer as explorer_screen;
use crate::screens::forge as forge_screen;
use crate::screens::governance as governance_screen;
use crate::screens::members as members_screen;
use crate::screens::operator as operator_screens;
use crate::screens::settings as settings_screen;
use crate::screens::user as user_screens;
use crate::screens::workspace as workspace_screens;
use crate::search;
use crate::theme::{self, Mode};
use crate::transport::{NodeClient, ServerFrame};
use crate::view_api::{AppIntent, Route};
use crate::workspace_service;

const TITLEBAR_HEIGHT: f32 = 44.0;
const NETWORK_RAIL_WIDTH: f32 = 62.0;
const MODULE_RAIL_WIDTH: f32 = 74.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    User,
    Operator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Home,
    Chat,
    Pages,
    Files,
    Browser,
    Forge,
    Agents,
    Members,
    Governance,
    Explorer,
    Node,
    Gateway,
    Modules,
    Sandbox,
    Metrics,
    Settings,
}

impl Screen {
    const USER: [Self; 9] = [
        Self::Chat,
        Self::Pages,
        Self::Files,
        Self::Browser,
        Self::Forge,
        Self::Agents,
        Self::Members,
        Self::Governance,
        Self::Explorer,
    ];
    const OPERATOR: [Self; 5] = [
        Self::Node,
        Self::Gateway,
        Self::Modules,
        Self::Sandbox,
        Self::Metrics,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Chat => "Chat",
            Self::Pages => "Pages",
            Self::Files => "Files",
            Self::Browser => "Browser",
            Self::Forge => "Forge",
            Self::Agents => "Agents",
            Self::Members => "Members",
            Self::Governance => "Governance",
            Self::Explorer => "Explorer",
            Self::Node => "Node",
            Self::Gateway => "Gateway",
            Self::Modules => "Modules",
            Self::Sandbox => "Sandbox",
            Self::Metrics => "Metrics",
            Self::Settings => "Settings",
        }
    }

    const fn icon(self) -> Icon {
        match self {
            Self::Home => Icon::Home,
            Self::Chat => Icon::Chat,
            Self::Pages => Icon::Pages,
            Self::Files => Icon::Files,
            Self::Browser => Icon::Browser,
            Self::Forge => Icon::Forge,
            Self::Agents => Icon::Agent,
            Self::Members => Icon::Members,
            Self::Governance => Icon::Governance,
            Self::Explorer => Icon::Explorer,
            Self::Node => Icon::Node,
            Self::Gateway => Icon::Link,
            Self::Modules => Icon::Modules,
            Self::Sandbox => Icon::Sandbox,
            Self::Metrics => Icon::Metrics,
            Self::Settings => Icon::Settings,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum WindowAction {
    Drag,
    Minimize,
    Maximize,
    Close,
}

#[derive(Debug, Clone, Copy)]
enum PageShortcut {
    Move(user_screens::BlockMove),
    RemoveEmpty,
    Activate,
    Cycle(bool),
    NewPage,
    CloseTab,
}

#[derive(Debug, Clone)]
enum Message {
    BackendLoaded(Result<(Backend, WorkspaceSnapshot), String>),
    PreferencesLoaded(Result<DesktopPreferences, String>),
    Back,
    Forward,
    Navigate(Screen),
    Section(Section),
    ToggleTheme,
    ToggleSearch,
    Search(search::Message),
    Onboarding(onboarding::Message),
    UserScreen(user_screens::Message),
    UserModule(module_host::Event),
    Forge(forge_screen::Message),
    Agents(agents_screen::Message),
    Members(members_screen::Message),
    Governance(governance_screen::Message),
    Explorer(explorer_screen::Message),
    Operator(operator_screens::Message),
    MetricsTick,
    CommunityTick,
    PageHistory(bool),
    PageShortcut(PageShortcut),
    PageShortcutFocusChecked {
        shortcut: PageShortcut,
        block: String,
        is_focused: bool,
    },
    PagePresenceTick,
    ClipboardWritten,
    Settings(settings_screen::Message),
    Workspace(workspace_screens::Message),
    WorkspaceSnapshotLoaded(Result<WorkspaceSnapshot, String>),
    AutoBindFinished(Result<bool, String>),
    Browser(browser_chrome::Message),
    MainOpened(window::Id),
    WindowEvent(window::Id, window::Event),
    OpenMain(Option<Screen>),
    Quit,
    QuitReady,
    OpenHuddle,
    HuddleOpened(window::Id),
    CloseHuddle,
    HuddleTick,
    HuddleMute,
    HuddleCamera,
    HuddleShare,
    HuddleLeave,
    HuddleRetry,
    HuddleExpand,
    HuddleCollapse,
    HuddleToggleLayout,
    HuddleToggleDevices,
    HuddleMicrophone(String),
    HuddleCameraDevice(String),
    HuddleSpeaker(String),
    HuddleScreenSource(String),
    TrayTick,
    Tray(mac_tray::Event),
    TrayPositioned(window::Id, f32, f64, f64),
    ToggleNotifications,
    CloseNotifications,
    ToggleNotificationGroup(String),
    OpenNotification(usize),
    NativeNotificationActivated(Option<notifications::Target>),
    NotificationStream(notifications::StreamEvent),
    NotificationResolved(Option<notifications::Item>),
    #[cfg(feature = "cef-browser")]
    BrowserWindowReady(Option<window::Id>),
    #[cfg(feature = "cef-browser")]
    BrowserGatewayLoaded {
        generation: u64,
        workspace_id: Option<String>,
        result: Result<String, String>,
    },
    #[cfg(feature = "cef-browser")]
    BrowserParentReady(Result<ParentWindow, String>),
    #[cfg(feature = "cef-browser")]
    BrowserPump,
    #[cfg(feature = "cef-browser")]
    BrowserPermissionDecision {
        id: u64,
        allow: bool,
        session: bool,
    },
    Window(WindowAction),
    WindowReady(WindowAction, Option<window::Id>),
}

struct Shell {
    desktop: desktop::State,
    notifications: notifications::State,
    notification_matcher: notifications::Matcher,
    backend: Option<Backend>,
    active_workspace: Option<Workspace>,
    node_client: Option<NodeClient>,
    backend_error: Option<String>,
    mode: Mode,
    accent: Color,
    section: Section,
    history: Vec<Screen>,
    history_index: usize,
    onboarding: onboarding::State,
    search: search::State,
    user_screens: user_screens::State,
    dropped_files: adapters::DropRegistry,
    forge: forge_screen::State,
    agents: agents_screen::State,
    members: members_screen::State,
    governance: governance_screen::State,
    explorer: explorer_screen::State,
    operator: operator_screens::State,
    settings: settings_screen::State,
    workspace: workspace_screens::State,
    workspace_overlay: bool,
    browser_chrome: browser_chrome::State,
    browser_error: Option<String>,
    window_size: Size,
    #[cfg(feature = "cef-browser")]
    browser: Option<BrowserRuntime>,
    #[cfg(feature = "cef-browser")]
    browser_gateway_base: Option<String>,
    #[cfg(feature = "cef-browser")]
    browser_gateway_loading: bool,
    #[cfg(feature = "cef-browser")]
    browser_gateway_generation: u64,
    #[cfg(feature = "cef-browser")]
    browser_permission: Option<PermissionPrompt>,
    pending_display_name: Option<String>,
    huddle: Option<HuddleRuntime>,
    huddle_expanded: bool,
    huddle_spotlight: bool,
    huddle_device_prefs: HuddleDevicePrefs,
    page_presence: Option<PagePresenceRuntime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HuddleStatus {
    Connecting,
    Reconnecting,
    Live,
    Unavailable,
}

struct HuddleRuntime {
    channel: String,
    session: Option<huddle_session::Handle>,
    media: Option<huddle_media::Handle>,
    status: HuddleStatus,
    muted: bool,
    camera_on: bool,
    sharing: bool,
    error: Option<String>,
    local_frame: Option<image::Handle>,
    peer_frames: BTreeMap<String, image::Handle>,
    peers: BTreeMap<String, HuddlePeer>,
    devices: huddle_media::DeviceOptions,
    devices_open: bool,
    level: u8,
    speaking_until: Option<std::time::Instant>,
    recipients: Vec<String>,
    retry_pending: bool,
    last_reconnect: Option<std::time::Instant>,
}

struct HuddlePeer {
    muted: bool,
    camera_on: bool,
    sharing: bool,
    seen: std::time::Instant,
}

#[derive(Default, Clone, Copy)]
struct HuddleDevicePrefs {
    microphone: Option<usize>,
    camera: Option<usize>,
    speaker: Option<usize>,
}

struct PagePresenceRuntime {
    page: String,
    handle: page_presence::Handle,
    peers: BTreeMap<String, (user_screens::PagePresence, std::time::Instant)>,
}

impl Default for Shell {
    fn default() -> Self {
        Self {
            desktop: desktop::State::default(),
            notifications: notifications::State::load_default(),
            notification_matcher: notifications::Matcher::default(),
            backend: None,
            active_workspace: None,
            node_client: None,
            backend_error: None,
            mode: Mode::Light,
            accent: theme::ACCENTS[0],
            section: Section::User,
            history: vec![Screen::Chat],
            history_index: 0,
            onboarding: onboarding::State::default(),
            search: search::State::default(),
            user_screens: user_screens::State::default(),
            dropped_files: adapters::DropRegistry::default(),
            forge: forge_screen::State::default(),
            agents: agents_screen::State::default(),
            members: members_screen::State::default(),
            governance: governance_screen::State::default(),
            explorer: explorer_screen::State::default(),
            operator: operator_screens::State::default(),
            settings: settings_screen::State::default(),
            workspace: workspace_screens::State::default(),
            workspace_overlay: false,
            browser_chrome: browser_chrome::State::default(),
            browser_error: None,
            window_size: Size::new(1280.0, 800.0),
            #[cfg(feature = "cef-browser")]
            browser: None,
            #[cfg(feature = "cef-browser")]
            browser_gateway_base: None,
            #[cfg(feature = "cef-browser")]
            browser_gateway_loading: false,
            #[cfg(feature = "cef-browser")]
            browser_gateway_generation: 0,
            #[cfg(feature = "cef-browser")]
            browser_permission: None,
            pending_display_name: None,
            huddle: None,
            huddle_expanded: false,
            huddle_spotlight: false,
            huddle_device_prefs: HuddleDevicePrefs::default(),
            page_presence: None,
        }
    }
}

impl Shell {
    fn boot() -> (Self, Task<Message>) {
        let (main, open_main) = window::open(desktop::main_settings());
        let mut state = Self::default();
        state.desktop.main = Some(main);
        (
            state,
            Task::batch([
                open_main.map(Message::MainOpened),
                Task::perform(
                    async {
                        let backend = Backend::new().await?;
                        let snapshot = backend.workspace_snapshot().await?;
                        Ok((backend, snapshot))
                    },
                    Message::BackendLoaded,
                ),
                Task::perform(
                    async { operator_service::load_preferences() },
                    Message::PreferencesLoaded,
                ),
            ]),
        )
    }

    fn screen(&self) -> Screen {
        self.history[self.history_index]
    }

    fn navigate(&mut self, screen: Screen) {
        if self.screen() == screen {
            return;
        }
        self.history.truncate(self.history_index + 1);
        self.history.push(screen);
        self.history_index += 1;
        self.section = if Screen::OPERATOR.contains(&screen) {
            Section::Operator
        } else {
            Section::User
        };
    }
}

pub fn run() -> iced::Result {
    iced::daemon(Shell::boot, update, view)
        .title(|state: &Shell, id| desktop::title(&state.desktop, id, state.notifications.unread))
        .theme(|state: &Shell, _| theme::iced_theme(state.mode, state.accent))
        .default_font(theme::SANS)
        .font(theme::FONT_BYTES[0])
        .font(theme::FONT_BYTES[1])
        .font(theme::FONT_BYTES[2])
        .font(theme::FONT_BYTES[3])
        .font(theme::FONT_BYTES[4])
        .font(theme::FONT_BYTES[5])
        .antialiasing(true)
        .subscription(subscription)
        .run()
}

fn update(state: &mut Shell, message: Message) -> Task<Message> {
    match message {
        Message::MainOpened(id) => {
            state.desktop.main = Some(id);
            state.window_size = desktop::MAIN_SIZE;
            if let Err(error) = mac_tray::init() {
                tracing::warn!(
                    target: "ducktape::shell",
                    reason = "tray_init_failed",
                    error = %error,
                    "could not initialize the macOS menu bar item"
                );
            }
            mac_tray::set_unread(state.notifications.unread);
            return sync_browser_visibility(state);
        }
        Message::OpenMain(screen) => {
            if let Some(screen) = screen {
                state.navigate(screen);
            }
            if let Some(id) = state.desktop.main {
                return Task::batch([
                    window::set_mode(id, window::Mode::Windowed),
                    window::minimize(id, false),
                    window::gain_focus(id),
                    sync_browser_visibility(state),
                ]);
            }
            let (id, open) = window::open(desktop::main_settings());
            state.desktop.main = Some(id);
            return open.map(Message::MainOpened);
        }
        Message::Quit => return quit(state),
        Message::QuitReady => return iced::exit(),
        Message::OpenHuddle => {
            if state.huddle.is_none() {
                return Task::none();
            }
            state.huddle_expanded = false;
            if let Some(id) = state.desktop.huddle {
                return window::gain_focus(id);
            }
            let (id, open) = window::open(desktop::huddle_settings());
            state.desktop.huddle = Some(id);
            return open.map(Message::HuddleOpened);
        }
        Message::HuddleOpened(id) => {
            state.desktop.huddle = Some(id);
        }
        Message::CloseHuddle => {
            if let Some(id) = state.desktop.huddle {
                return window::close(id);
            }
        }
        Message::HuddleTick => return poll_huddle(state),
        Message::HuddleMute => {
            if let Some(huddle) = &mut state.huddle {
                huddle.muted = !huddle.muted;
                if let Some(media) = &huddle.media {
                    media.send(huddle_media::Command::SetMuted(huddle.muted));
                }
                send_huddle_beacon(huddle);
            }
        }
        Message::HuddleCamera => {
            if let Some(huddle) = &mut state.huddle {
                huddle.error = None;
                if let Some(media) = &huddle.media {
                    media.send(huddle_media::Command::SetCamera(!huddle.camera_on));
                }
            }
        }
        Message::HuddleShare => {
            if let Some(huddle) = &mut state.huddle {
                huddle.error = None;
                if let Some(media) = &huddle.media {
                    media.send(huddle_media::Command::SetScreenShare(!huddle.sharing));
                }
            }
        }
        Message::HuddleLeave => {
            let Some(channel) = state.huddle.as_ref().map(|huddle| huddle.channel.clone()) else {
                return Task::none();
            };
            state.huddle = None;
            state.huddle_expanded = false;
            let leave = execute_user_screen(
                state,
                user_screens::Command::SetHuddle {
                    channel,
                    joined: false,
                },
            );
            let close = close_huddle_window(state);
            return Task::batch([leave, close]);
        }
        Message::HuddleRetry => {
            state.huddle_expanded = false;
            let Some(huddle) = &mut state.huddle else {
                return Task::none();
            };
            huddle.status = HuddleStatus::Connecting;
            huddle.error = None;
            huddle.retry_pending = true;
            huddle.last_reconnect = None;
            huddle.session.take();
            if let Some(media) = &huddle.media {
                media.send(huddle_media::Command::Stop);
            }
            return restart_huddle_if_stopped(state);
        }
        Message::HuddleExpand => {
            state.huddle_expanded = true;
            hide_browser(state);
        }
        Message::HuddleCollapse => {
            state.huddle_expanded = false;
            return sync_browser_visibility(state);
        }
        Message::HuddleToggleLayout => state.huddle_spotlight = !state.huddle_spotlight,
        Message::HuddleToggleDevices => {
            if let Some(huddle) = &mut state.huddle {
                huddle.devices_open = !huddle.devices_open;
                if huddle.devices_open
                    && let Some(media) = &huddle.media
                {
                    media.send(huddle_media::Command::RefreshDevices);
                }
            }
        }
        Message::HuddleMicrophone(value) => {
            if let Some(huddle) = &state.huddle
                && let Some(media) = &huddle.media
            {
                media.send(huddle_media::Command::SetMicrophone(device_index(&value)));
            }
        }
        Message::HuddleCameraDevice(value) => {
            if let Some(huddle) = &state.huddle
                && let Some(media) = &huddle.media
            {
                media.send(huddle_media::Command::SetCameraDevice(device_index(&value)));
            }
        }
        Message::HuddleSpeaker(value) => {
            if let Some(huddle) = &state.huddle
                && let Some(media) = &huddle.media
            {
                media.send(huddle_media::Command::SetSpeaker(device_index(&value)));
            }
        }
        Message::HuddleScreenSource(value) => {
            if let Some(huddle) = &state.huddle
                && let Some(media) = &huddle.media
            {
                media.send(huddle_media::Command::SetScreenSource(device_index(&value)));
            }
        }
        Message::WindowEvent(id, event) => match event {
            window::Event::FileHovered(_)
                if state.desktop.main == Some(id) && state.screen() == Screen::Files =>
            {
                return update_user_module(
                    state,
                    module_host::Message::Files(user_screens::FilesMessage::DropHovered(true)),
                );
            }
            window::Event::FileDropped(path)
                if state.desktop.main == Some(id) && state.screen() == Screen::Files =>
            {
                let token = match state.dropped_files.mint(path) {
                    Ok(token) => token,
                    Err(error) => {
                        state.user_screens.files.drop_active = false;
                        state.user_screens.files.error = Some(error);
                        return Task::none();
                    }
                };
                let task = update_user_module(
                    state,
                    module_host::Message::Files(user_screens::FilesMessage::FileDropped(token)),
                );
                state.dropped_files.discard(token);
                return task;
            }
            window::Event::FilesHoveredLeft
                if state.desktop.main == Some(id) && state.screen() == Screen::Files =>
            {
                return update_user_module(
                    state,
                    module_host::Message::Files(user_screens::FilesMessage::DropHovered(false)),
                );
            }
            window::Event::Resized(size) if state.desktop.main == Some(id) => {
                state.window_size = size;
                #[cfg(feature = "cef-browser")]
                if let Some(browser) = &mut state.browser
                    && let Err(error) = browser.set_bounds(browser_bounds(size))
                {
                    state.browser_error = Some(error);
                }
            }
            window::Event::Focused if state.desktop.main == Some(id) => {
                state.desktop.main_focused = true;
            }
            window::Event::Unfocused if state.desktop.main == Some(id) => {
                state.desktop.main_focused = false;
            }
            window::Event::Unfocused if state.desktop.tray == Some(id) => {
                state.desktop.mark_tray_hidden();
                return window::close(id);
            }
            window::Event::CloseRequested => return close_window(state, id),
            window::Event::Closed => {
                state.desktop.closed(id);
            }
            _ => {}
        },
        Message::TrayTick => {
            #[cfg(all(feature = "cef-browser", target_os = "macos"))]
            if crate::browser::take_macos_terminate_request() {
                return Task::done(Message::Quit);
            }
            if let Some(event) = mac_tray::poll() {
                return Task::done(Message::Tray(event));
            }
        }
        Message::Tray(event) => {
            #[cfg(target_os = "macos")]
            match event {
                mac_tray::Event::Open => return Task::done(Message::OpenMain(None)),
                mac_tray::Event::Quit => return Task::done(Message::Quit),
                mac_tray::Event::Toggle { x, y } => {
                    if let Some(id) = state.desktop.tray {
                        state.desktop.mark_tray_hidden();
                        return window::close(id);
                    }
                    if state.desktop.tray_was_just_hidden() {
                        return Task::none();
                    }
                    let (id, open) = window::open(desktop::tray_settings(iced::Point::new(
                        (x - f64::from(desktop::TRAY_SIZE.width) / 2.0) as f32,
                        (y + 6.0) as f32,
                    )));
                    state.desktop.tray = Some(id);
                    return Task::batch([
                        open.discard(),
                        window::scale_factor(id)
                            .map(move |scale| Message::TrayPositioned(id, scale, x, y)),
                    ]);
                }
            }
            #[cfg(not(target_os = "macos"))]
            match event {}
        }
        Message::TrayPositioned(id, scale, x, y) => {
            if state.desktop.tray == Some(id) {
                let scale = f64::from(scale.max(1.0));
                let point = iced::Point::new(
                    (x / scale - f64::from(desktop::TRAY_SIZE.width) / 2.0).max(8.0) as f32,
                    (y / scale + 6.0) as f32,
                );
                return Task::batch([window::move_to(id, point), window::gain_focus(id)]);
            }
        }
        Message::ToggleNotifications => {
            state.notifications.toggle();
            mac_tray::set_unread(state.notifications.unread);
        }
        Message::CloseNotifications => state.notifications.close(),
        Message::ToggleNotificationGroup(key) => state.notifications.toggle_group(key),
        Message::OpenNotification(index) => {
            let Some(item) = state.notifications.recent.get(index).cloned() else {
                return Task::none();
            };
            state.notifications.close();
            return open_notification_target(state, item.target());
        }
        Message::NativeNotificationActivated(Some(target)) => {
            return Task::batch([
                Task::done(Message::OpenMain(None)),
                open_notification_target(state, target),
            ]);
        }
        Message::NativeNotificationActivated(None) => {}
        Message::NotificationStream(notifications::StreamEvent::Frame(frame)) => {
            let governance = governance_stream_frame(state, &frame);
            let config = notification_config(state);
            match state.notification_matcher.handle(frame, &config) {
                Some(notifications::Matched::Item(item)) => {
                    return Task::batch([present_notification(state, item), governance]);
                }
                Some(notifications::Matched::Reply(candidate)) => {
                    let Some(client) = state.node_client.clone() else {
                        return governance;
                    };
                    return Task::batch([
                        Task::perform(
                            notifications::resolve_reply(client, candidate),
                            Message::NotificationResolved,
                        ),
                        governance,
                    ]);
                }
                None => return governance,
            }
        }
        Message::NotificationStream(notifications::StreamEvent::Connected) => {
            return governance_stream_reload(state);
        }
        Message::NotificationStream(notifications::StreamEvent::Disconnected) => {}
        Message::NotificationResolved(Some(item)) => return present_notification(state, item),
        Message::NotificationResolved(None) => {}
        Message::BackendLoaded(Ok((backend, snapshot))) => {
            state.huddle = None;
            state.huddle_expanded = false;
            let close_huddle = close_huddle_window(state);
            state.node_client = None;
            state.workspace.workspaces = snapshot
                .workspaces
                .iter()
                .cloned()
                .map(workspace_for_screen)
                .collect();
            state.workspace.workspace = snapshot.active.clone().map(workspace_for_screen);
            state.workspace_overlay = snapshot
                .active
                .as_ref()
                .is_none_or(|workspace| !workspace.member);
            state.active_workspace = snapshot.active;
            state.backend = Some(backend.clone());
            state.backend_error = None;
            return Task::batch([
                close_huddle,
                execute_onboarding(Some(backend), onboarding::Command::LoadIdentity),
            ]);
        }
        Message::BackendLoaded(Err(error)) => {
            state.backend_error = Some(error.clone());
            let _ = onboarding::update(
                &mut state.onboarding,
                onboarding::Message::Service(onboarding::ServiceEvent::IdentityLoaded(Err(error))),
            );
        }
        Message::PreferencesLoaded(Ok(preferences)) => {
            state.mode = preferences.mode;
            state.accent = theme::ACCENTS[preferences.accent.min(theme::ACCENTS.len() - 1)];
            state.settings.mode = preferences.mode;
            state.settings.accent = preferences.accent.min(theme::ACCENTS.len() - 1);
            state.settings.notifications = preferences.notifications;
        }
        Message::PreferencesLoaded(Err(error)) => {
            tracing::warn!(
                target: "ducktape::settings",
                reason = "preferences_load_failed",
                error = %error,
                "desktop preferences could not be loaded"
            );
        }
        Message::Back if state.history_index > 0 => {
            state.history_index -= 1;
            return sync_browser_visibility(state);
        }
        Message::Forward if state.history_index + 1 < state.history.len() => {
            state.history_index += 1;
            return sync_browser_visibility(state);
        }
        Message::Navigate(screen)
            if state.screen() == Screen::Home
                && screen != Screen::Home
                && matches!(
                    state.user_screens.home.account.panel,
                    user_screens::AccountPanel::Link | user_screens::AccountPanel::Phone
                ) =>
        {
            let cancel = user_screens::update(
                &mut state.user_screens,
                user_screens::Message::Home(user_screens::HomeMessage::CancelAccountPanel),
            )
            .map_or_else(Task::none, |command| execute_user_screen(state, command));
            return Task::batch([cancel, Task::done(Message::Navigate(screen))]);
        }
        Message::Navigate(screen) => {
            state.navigate(screen);
            if screen == Screen::Forge
                && let Some(command) =
                    forge_screen::update(&mut state.forge, forge_screen::Message::Load)
            {
                return Task::batch([
                    execute_forge(state, command),
                    sync_browser_visibility(state),
                ]);
            }
            if screen == Screen::Agents
                && let Some(command) =
                    agents_screen::update(&mut state.agents, agents_screen::Message::Load)
            {
                return Task::batch([
                    execute_agents(state, command),
                    sync_browser_visibility(state),
                ]);
            }
            if screen == Screen::Members
                && let Some(command) =
                    members_screen::update(&mut state.members, members_screen::Message::Load)
            {
                return Task::batch([
                    execute_members(state, command),
                    sync_browser_visibility(state),
                ]);
            }
            if screen == Screen::Governance
                && let Some(command) = governance_screen::update(
                    &mut state.governance,
                    governance_screen::Message::Load,
                )
            {
                return Task::batch([
                    execute_governance(state, command),
                    sync_browser_visibility(state),
                ]);
            }
            if screen == Screen::Explorer
                && let Some(command) =
                    explorer_screen::update(&mut state.explorer, explorer_screen::Message::Load)
            {
                return Task::batch([
                    execute_explorer(state, command),
                    sync_browser_visibility(state),
                ]);
            }
            if let Some(screen) = user_screen(screen)
                && let Some(command) = user_screens::update(
                    &mut state.user_screens,
                    user_screens::Message::Load(screen),
                )
            {
                return execute_user_screen(state, command);
            }
            if let Some(screen) = operator_screen(screen)
                && let Some(command) = operator_screens::update(
                    &mut state.operator,
                    operator_screens::Message::Load(screen),
                )
            {
                return execute_operator(state, command);
            }
            if screen == Screen::Settings
                && let Some(command) =
                    settings_screen::update(&mut state.settings, settings_screen::Message::Load)
            {
                return execute_settings(state, command);
            }
            return sync_browser_visibility(state);
        }
        Message::Section(section) => {
            state.section = section;
            state.navigate(match section {
                Section::User => Screen::Chat,
                Section::Operator => Screen::Node,
            });
            return sync_browser_visibility(state);
        }
        Message::ToggleTheme => {
            state.mode = state.mode.toggled();
            state.settings.mode = state.mode;
            return execute_settings(state, settings_screen::Command::SetTheme(state.mode));
        }
        Message::ToggleSearch => {
            if state.search.open {
                let _ = search::update(&mut state.search, search::Message::Close);
                return sync_browser_visibility(state);
            }
            hide_browser(state);
            let catalog = search_catalog(state);
            let command = search::update(&mut state.search, search::Message::Open(catalog));
            let focus = command.map_or_else(Task::none, |command| execute_search(state, command));
            let files = Task::perform(search::load_files(state.node_client.clone()), |result| {
                Message::Search(search::Message::FilesLoaded(result))
            });
            let members = members_screen::update(&mut state.members, members_screen::Message::Load)
                .map_or_else(Task::none, |command| execute_members(state, command));
            return Task::batch([focus, files, members]);
        }
        Message::Search(message) => {
            let was_open = state.search.open;
            if let Some(command) = search::update(&mut state.search, message) {
                return execute_search(state, command);
            }
            if was_open && !state.search.open {
                return sync_browser_visibility(state);
            }
        }
        Message::Onboarding(message) => {
            let was_ready = state.onboarding.is_ready();
            if let Some(command) = onboarding::update(&mut state.onboarding, message) {
                match &command {
                    onboarding::Command::CreateIdentity { display_name, .. }
                    | onboarding::Command::CreateIdentityWithTouchId { display_name } => {
                        state.pending_display_name.clone_from(display_name);
                    }
                    _ => {}
                }
                if matches!(
                    command,
                    onboarding::Command::GateCompleted | onboarding::Command::GateSkipped
                ) {
                    return after_gate_ready(state);
                }
                return execute_onboarding(state.backend.clone(), command);
            }
            if !was_ready && state.onboarding.is_ready() {
                return after_gate_ready(state);
            }
        }
        Message::UserScreen(message) => {
            match message {
                user_screens::Message::Home(message) => {
                    return update_user_module(state, module_host::Message::Home(message));
                }
                user_screens::Message::Chat(message) => {
                    return update_user_module(state, module_host::Message::Chat(message));
                }
                user_screens::Message::Pages(message) => {
                    return update_user_module(state, module_host::Message::Pages(message));
                }
                user_screens::Message::Files(message) => {
                    return update_user_module(state, module_host::Message::Files(message));
                }
                message => {
                    let refresh = match &message {
                        user_screens::Message::Service(
                            user_screens::ServiceEvent::ActionFinished {
                                screen,
                                result: Ok(()),
                            },
                        ) => Some(*screen),
                        _ => None,
                    };
                    if let Some(command) = user_screens::update(&mut state.user_screens, message) {
                        let task = execute_user_screen(state, command);
                        let huddle = sync_huddle_runtime(state);
                        sync_page_presence(state);
                        publish_page_cursor(state);
                        return Task::batch([task, huddle]);
                    }
                    if let Some(screen) = refresh
                        && let Some(command) = user_screens::update(
                            &mut state.user_screens,
                            user_screens::Message::Load(screen),
                        )
                    {
                        let task = execute_user_screen(state, command);
                        sync_page_presence(state);
                        return task;
                    }
                }
            }
            let huddle = sync_huddle_runtime(state);
            sync_page_presence(state);
            publish_page_cursor(state);
            return huddle;
        }
        Message::UserModule(event) => {
            return apply_user_module_event(state, event);
        }
        Message::PageHistory(redo) => {
            if state.screen() == Screen::Pages
                && let Some(command) = user_screens::update(
                    &mut state.user_screens,
                    user_screens::Message::Pages(if redo {
                        user_screens::PagesMessage::Redo
                    } else {
                        user_screens::PagesMessage::Undo
                    }),
                )
            {
                let task = execute_user_screen(state, command);
                publish_page_cursor(state);
                return task;
            }
        }
        Message::PageShortcut(shortcut) => {
            if state.screen() != Screen::Pages {
                return Task::none();
            }
            if matches!(
                shortcut,
                PageShortcut::Move(_) | PageShortcut::RemoveEmpty | PageShortcut::Activate
            ) {
                let Some(block) = state.user_screens.pages.focused_block.clone() else {
                    return Task::none();
                };
                let id = iced::widget::Id::from(user_screens::page_block_input_id(&block));
                return iced::widget::operation::is_focused(id).map(move |is_focused| {
                    Message::PageShortcutFocusChecked {
                        shortcut,
                        block: block.clone(),
                        is_focused,
                    }
                });
            }
            return apply_page_shortcut(state, shortcut);
        }
        Message::PageShortcutFocusChecked {
            shortcut,
            block,
            is_focused,
        } => {
            if state.screen() == Screen::Pages
                && is_focused
                && state.user_screens.pages.focused_block.as_deref() == Some(block.as_str())
            {
                return apply_page_shortcut(state, shortcut);
            }
        }
        Message::PagePresenceTick => poll_page_presence(state),
        Message::Forge(message) => {
            if let Some(command) = forge_screen::update(&mut state.forge, message) {
                return execute_forge(state, command);
            }
        }
        Message::Agents(message) => {
            if let Some(effect) = agents_screen::reduce(&mut state.agents, message) {
                return match effect {
                    agents_screen::Effect::Command(command) => execute_agents(state, command),
                    agents_screen::Effect::Intent(intent) => Task::batch([
                        open_app_intent(state, intent),
                        sync_browser_visibility(state),
                    ]),
                };
            }
        }
        Message::Members(message) => {
            if let Some(command) = members_screen::update(&mut state.members, message) {
                refresh_search_members(state);
                return execute_members(state, command);
            }
            refresh_search_members(state);
        }
        Message::Governance(message) => {
            if let Some(command) = governance_screen::update(&mut state.governance, message) {
                return execute_governance(state, command);
            }
        }
        Message::Explorer(message) => {
            if let Some(command) = explorer_screen::update(&mut state.explorer, message) {
                return execute_explorer(state, command);
            }
        }
        Message::Operator(message) => {
            if let Some(command) = operator_screens::update(&mut state.operator, message) {
                return execute_operator(state, command);
            }
        }
        Message::MetricsTick => {
            if state.screen() == Screen::Metrics && !state.operator.metrics.paused {
                return execute_operator(state, operator_screens::Command::LoadMetrics);
            }
        }
        Message::CommunityTick => match state.screen() {
            Screen::Members if !state.members.busy => {
                if let Some(command) =
                    members_screen::update(&mut state.members, members_screen::Message::Refresh)
                {
                    return execute_members(state, command);
                }
            }
            Screen::Governance if !state.governance.busy => {
                if let Some(command) = governance_screen::update(
                    &mut state.governance,
                    governance_screen::Message::Refresh,
                ) {
                    return execute_governance(state, command);
                }
            }
            Screen::Explorer => {
                if let Some(command) =
                    explorer_screen::update(&mut state.explorer, explorer_screen::Message::Refresh)
                {
                    return execute_explorer(state, command);
                }
            }
            _ => {}
        },
        Message::ClipboardWritten => {}
        Message::Settings(message) => {
            let refresh_workspace = matches!(
                &message,
                settings_screen::Message::Service(settings_screen::ServiceEvent::DangerFinished(
                    Ok(())
                ))
            );
            if let Some(command) = settings_screen::update(&mut state.settings, message) {
                return execute_settings(state, command);
            }
            if refresh_workspace && let Some(backend) = state.backend.clone() {
                return Task::perform(
                    async move { backend.workspace_snapshot().await },
                    Message::WorkspaceSnapshotLoaded,
                );
            }
        }
        Message::Workspace(message) => {
            if matches!(&message, workspace_screens::Message::Open) {
                state.workspace_overlay = true;
                hide_browser(state);
            }
            if let Some(command) = workspace_screens::update(&mut state.workspace, message) {
                state.workspace_overlay = matches!(
                    state.workspace.stage,
                    workspace_screens::Stage::Joining | workspace_screens::Stage::Failed
                ) || state.workspace_overlay;
                return execute_workspace(state, command);
            }
        }
        Message::WorkspaceSnapshotLoaded(Ok(snapshot)) => {
            state.huddle = None;
            state.huddle_expanded = false;
            let close_huddle = close_huddle_window(state);
            state.notification_matcher = notifications::Matcher::default();
            state.active_workspace = snapshot.active.clone();
            state.node_client = snapshot
                .active
                .as_ref()
                .and_then(|workspace| NodeClient::local(workspace.ports.http).ok());
            state.workspace.workspaces = snapshot
                .workspaces
                .into_iter()
                .map(workspace_for_screen)
                .collect();
            state.workspace.workspace = snapshot.active.map(workspace_for_screen);
            state.workspace_overlay = false;
            reset_browser_gateway(state);
            if let (Some(backend), Some(workspace), Some(client)) = (
                state.backend.clone(),
                state.active_workspace.clone(),
                state.node_client.clone(),
            ) {
                return Task::batch([
                    close_huddle,
                    Task::perform(
                        account_service::auto_bind_on_connect(backend, workspace, client),
                        Message::AutoBindFinished,
                    ),
                ]);
            }
            return Task::batch([close_huddle, finish_workspace_connection(state)]);
        }
        Message::WorkspaceSnapshotLoaded(Err(error)) => {
            state.workspace.error = Some(error);
            state.workspace_overlay = true;
        }
        Message::AutoBindFinished(result) => {
            if let Err(error) = result {
                tracing::debug!(
                    target: "ducktape::account",
                    event = "account_auto_bind_failed",
                    reason = "connect_bind_failed",
                    detail = %error,
                    "account binding will retry on the next workspace connection"
                );
            }
            return finish_workspace_connection(state);
        }
        Message::Browser(message) => {
            #[cfg(feature = "cef-browser")]
            let chrome_before = state.browser_chrome.clone();
            #[cfg(feature = "cef-browser")]
            let action = message.clone();
            if let Some(url) = browser_chrome::update(&mut state.browser_chrome, message) {
                #[cfg(feature = "cef-browser")]
                if let Some(browser) = &mut state.browser {
                    let result = match &action {
                        browser_chrome::Message::NewTab => browser.new_tab(&url),
                        browser_chrome::Message::SelectTab(index) => browser.select_tab(*index),
                        browser_chrome::Message::CloseTab(index) => {
                            browser.close_tab(*index, state.browser_chrome.active_tab, &url)
                        }
                        _ => browser.navigate(&url),
                    };
                    match result {
                        Ok(()) => {
                            state.browser_chrome.loading = false;
                            state.browser_error = None;
                        }
                        Err(error) => {
                            if matches!(action, browser_chrome::Message::NewTab) {
                                state.browser_chrome = chrome_before;
                            }
                            state.browser_chrome.loading = false;
                            state.browser_chrome.error = Some(error.clone());
                            state.browser_error = Some(error);
                        }
                    }
                } else {
                    return sync_browser_visibility(state);
                }
                #[cfg(not(feature = "cef-browser"))]
                {
                    let _ = url;
                    state.browser_chrome.loading = false;
                    state.browser_chrome.error =
                        Some("This build does not include the embedded CEF browser.".into());
                }
            }
        }
        #[cfg(feature = "cef-browser")]
        Message::BrowserGatewayLoaded {
            generation,
            workspace_id,
            result: Ok(base),
        } => {
            if generation != state.browser_gateway_generation
                || workspace_id != browser_gateway_workspace(state)
            {
                return Task::none();
            }
            state.browser_gateway_loading = false;
            if let Err(error) = BrowserRuntime::validate_gateway_base(&base) {
                state.browser_chrome.loading = false;
                state.browser_chrome.error = Some(error.clone());
                state.browser_error = Some(error);
                return Task::none();
            }
            if let Some(browser) = &mut state.browser {
                if let Err(error) = browser.set_gateway_base(Some(base.clone())) {
                    state.browser_chrome.loading = false;
                    state.browser_chrome.error = Some(error.clone());
                    state.browser_error = Some(error);
                    return Task::none();
                }
                if let Ok(url) = state.browser_chrome.url() {
                    let result = if browser.has_surface() {
                        browser.navigate(&url)
                    } else {
                        browser.reopen(&url)
                    };
                    if let Err(error) = result {
                        state.browser_chrome.loading = false;
                        state.browser_chrome.error = Some(error.clone());
                        state.browser_error = Some(error);
                        return Task::none();
                    }
                }
                state.browser_gateway_base = Some(base);
                state.browser_chrome.loading = false;
                return sync_browser_visibility(state);
            }
            state.browser_gateway_base = Some(base);
            return Task::done(Message::BrowserWindowReady(state.desktop.main));
        }
        #[cfg(feature = "cef-browser")]
        Message::BrowserGatewayLoaded {
            generation,
            workspace_id,
            result: Err(error),
        } => {
            if generation != state.browser_gateway_generation
                || workspace_id != browser_gateway_workspace(state)
            {
                return Task::none();
            }
            state.browser_gateway_loading = false;
            state.browser_chrome.loading = false;
            state.browser_chrome.error = Some(error.clone());
            state.browser_error = Some(error);
        }
        #[cfg(feature = "cef-browser")]
        Message::BrowserWindowReady(Some(id)) => {
            return window::run(id, ParentWindow::from_iced).map(Message::BrowserParentReady);
        }
        #[cfg(feature = "cef-browser")]
        Message::BrowserWindowReady(None) => {
            state.browser_error = Some("iced did not expose its native window".into());
        }
        #[cfg(feature = "cef-browser")]
        Message::BrowserParentReady(Ok(parent)) => {
            let url = state
                .browser_chrome
                .url()
                .unwrap_or_else(|_| "duck://net.duck/".into());
            match BrowserRuntime::create_with_gateway(
                parent,
                browser_bounds(state.window_size),
                &url,
                state.browser_gateway_base.clone(),
            ) {
                Ok(browser) => {
                    state.browser = Some(browser);
                    state.browser_chrome.loading = false;
                    state.browser_error = None;
                }
                Err(error) => {
                    state.browser_chrome.loading = false;
                    state.browser_chrome.error = Some(error.clone());
                    state.browser_error = Some(error);
                }
            }
        }
        #[cfg(feature = "cef-browser")]
        Message::BrowserParentReady(Err(error)) => {
            state.browser_error = Some(error);
        }
        #[cfg(feature = "cef-browser")]
        Message::BrowserPump => {
            let page_visible =
                state.screen() == Screen::Browser && !state.workspace_overlay && !state.search.open;
            if let Some(browser) = &mut state.browser {
                browser.pump();
                for event in browser.take_events() {
                    match event {
                        BrowserEvent::NavigationCommitted { browser_id, url } => {
                            let Some(tab_index) = browser.tab_index(browser_id) else {
                                continue;
                            };
                            if let Err(error) = browser_chrome::commit_navigation(
                                &mut state.browser_chrome,
                                tab_index,
                                &url,
                            ) {
                                state.browser_chrome.error = Some(error);
                            }
                        }
                    }
                }
                let prompt = browser.permission_prompt();
                if prompt != state.browser_permission {
                    state.browser_permission = prompt;
                    let visible = state.browser_permission.is_none() && page_visible;
                    if let Err(error) = browser.set_visible(visible) {
                        state.browser_error = Some(error);
                    }
                }
            }
        }
        #[cfg(feature = "cef-browser")]
        Message::BrowserPermissionDecision { id, allow, session } => {
            let page_visible =
                state.screen() == Screen::Browser && !state.workspace_overlay && !state.search.open;
            if state.browser_permission.as_ref().map(|prompt| prompt.id) == Some(id)
                && let Some(browser) = &mut state.browser
            {
                match browser.decide_permission(id, allow, session) {
                    Ok(()) => {
                        state.browser_permission = None;
                        if let Err(error) = browser.set_visible(page_visible) {
                            state.browser_error = Some(error);
                        }
                    }
                    Err(error) => state.browser_error = Some(error),
                }
            }
        }
        Message::Window(action) => {
            return Task::done(Message::WindowReady(action, state.desktop.main));
        }
        Message::WindowReady(action, Some(id)) => {
            return match action {
                WindowAction::Drag => window::drag(id),
                WindowAction::Minimize => window::minimize(id, true),
                WindowAction::Maximize => window::toggle_maximize(id),
                WindowAction::Close => close_window(state, id),
            };
        }
        Message::Back | Message::Forward | Message::WindowReady(_, None) => {}
    }
    Task::none()
}

fn subscription(state: &Shell) -> Subscription<Message> {
    let mut subscriptions = vec![
        window::events().map(|(id, event)| Message::WindowEvent(id, event)),
        iced::event::listen_with(global_shortcut),
    ];
    if state.screen() == Screen::Metrics && !state.operator.metrics.paused {
        subscriptions.push(
            iced::time::every(std::time::Duration::from_secs(2)).map(|_| Message::MetricsTick),
        );
    }
    if user_screens::account_polling(&state.user_screens) {
        subscriptions.push(
            iced::time::every(std::time::Duration::from_millis(1_200))
                .map(|_| Message::UserScreen(user_screens::Message::AccountTick)),
        );
    }
    if state.huddle.is_some() {
        subscriptions.push(
            iced::time::every(std::time::Duration::from_millis(33)).map(|_| Message::HuddleTick),
        );
    }
    if state.page_presence.is_some() {
        subscriptions.push(
            iced::time::every(std::time::Duration::from_millis(250))
                .map(|_| Message::PagePresenceTick),
        );
    }
    if let Some(client) = &state.node_client {
        subscriptions
            .push(notifications::subscription(client.origin()).map(Message::NotificationStream));
        let dispatches = agents_screen::expanded_run_dispatches(&state.agents);
        if !dispatches.is_empty() {
            subscriptions.push(
                forge_agents_service::run_output_subscription(client.origin(), dispatches)
                    .map(|event| Message::Agents(agents_screen::Message::RunLog(event))),
            );
        }
    }
    match state.screen() {
        Screen::Members => subscriptions.push(
            iced::time::every(std::time::Duration::from_secs(5)).map(|_| Message::CommunityTick),
        ),
        Screen::Governance | Screen::Explorer => subscriptions.push(
            iced::time::every(std::time::Duration::from_secs(2)).map(|_| Message::CommunityTick),
        ),
        _ => {}
    }
    #[cfg(target_os = "macos")]
    subscriptions
        .push(iced::time::every(std::time::Duration::from_millis(100)).map(|_| Message::TrayTick));
    #[cfg(feature = "cef-browser")]
    if state.browser.is_some() {
        subscriptions.push(
            iced::time::every(std::time::Duration::from_millis(8)).map(|_| Message::BrowserPump),
        );
    }
    Subscription::batch(subscriptions)
}

fn joined_huddle(state: &Shell) -> Option<(String, Vec<String>)> {
    let user_screens::Resource::Ready(data) = &state.user_screens.chat.data else {
        return None;
    };
    let self_key = data.self_key.as_ref()?;
    let channel = data
        .channels
        .iter()
        .find(|channel| {
            channel.id.as_str()
                == state
                    .user_screens
                    .chat
                    .active_channel
                    .as_deref()
                    .unwrap_or("")
                && channel.huddle.iter().any(|member| &member.user == self_key)
        })
        .or_else(|| {
            data.channels
                .iter()
                .find(|channel| channel.huddle.iter().any(|member| &member.user == self_key))
        })?;
    let local_node = state
        .active_workspace
        .as_ref()
        .map(|workspace| workspace.pubkey.as_str());
    let mut recipients = channel
        .huddle
        .iter()
        .filter(|member| Some(member.node.as_str()) != local_node)
        .map(|member| member.node.clone())
        .collect::<Vec<_>>();
    recipients.sort();
    recipients.dedup();
    recipients.truncate(64);
    Some((channel.id.clone(), recipients))
}

fn sync_huddle_runtime(state: &mut Shell) -> Task<Message> {
    let Some((channel, recipients)) = joined_huddle(state) else {
        let stopped = state.huddle.take().is_some();
        state.huddle_expanded = false;
        return if stopped {
            close_huddle_window(state)
        } else {
            Task::none()
        };
    };
    if let Some(huddle) = &mut state.huddle
        && huddle.channel == channel
    {
        if huddle.recipients != recipients {
            huddle.recipients.clone_from(&recipients);
            if let Some(session) = &huddle.session {
                let _ = session
                    .control
                    .try_send(huddle_session::Control::Recipients(recipients));
            }
        }
        return Task::none();
    }

    state.huddle = Some(start_huddle_runtime(
        state,
        channel,
        recipients,
        true,
        HuddleStatus::Connecting,
        None,
    ));
    Task::none()
}

fn start_huddle_runtime(
    state: &Shell,
    channel: String,
    recipients: Vec<String>,
    muted: bool,
    status: HuddleStatus,
    last_reconnect: Option<std::time::Instant>,
) -> HuddleRuntime {
    let started = state
        .node_client
        .as_ref()
        .ok_or_else(|| "connect to the workspace before starting a huddle".to_string())
        .and_then(|client| huddle_session::Handle::start(client, &channel));
    let (session, media, error) = match started {
        Ok((session, port)) => {
            let _ = session
                .control
                .try_send(huddle_session::Control::Recipients(recipients.clone()));
            let _ = session.control.try_send(huddle_session::Control::Beacon {
                muted,
                camera_on: false,
                sharing: false,
            });
            let media = huddle_media::Handle::start(port);
            if !muted {
                media.send(huddle_media::Command::SetMuted(false));
            }
            if state.huddle_device_prefs.microphone.is_some() {
                media.send(huddle_media::Command::SetMicrophone(
                    state.huddle_device_prefs.microphone,
                ));
            }
            if state.huddle_device_prefs.camera.is_some() {
                media.send(huddle_media::Command::SetCameraDevice(
                    state.huddle_device_prefs.camera,
                ));
            }
            if state.huddle_device_prefs.speaker.is_some() {
                media.send(huddle_media::Command::SetSpeaker(
                    state.huddle_device_prefs.speaker,
                ));
            }
            (Some(session), Some(media), None)
        }
        Err(error) => (None, None, Some(error)),
    };
    HuddleRuntime {
        channel,
        session,
        media,
        status: if error.is_some() {
            HuddleStatus::Unavailable
        } else {
            status
        },
        muted,
        camera_on: false,
        sharing: false,
        error,
        local_frame: None,
        peer_frames: BTreeMap::new(),
        peers: BTreeMap::new(),
        devices: huddle_media::DeviceOptions::default(),
        devices_open: false,
        level: 0,
        speaking_until: None,
        recipients,
        retry_pending: false,
        last_reconnect,
    }
}

fn restart_huddle_if_stopped(state: &mut Shell) -> Task<Message> {
    let ready = state.huddle.as_ref().is_some_and(|huddle| {
        huddle.retry_pending
            && huddle
                .media
                .as_ref()
                .is_none_or(huddle_media::Handle::is_stopped)
    });
    if !ready {
        return Task::none();
    }
    let previous = state.huddle.take().expect("retry checked a huddle");
    let Some((channel, recipients)) = joined_huddle(state) else {
        state.huddle_expanded = false;
        return close_huddle_window(state);
    };
    state.huddle = Some(start_huddle_runtime(
        state,
        channel,
        recipients,
        previous.muted,
        previous.status,
        previous.last_reconnect,
    ));
    Task::none()
}

fn close_huddle_window(state: &Shell) -> Task<Message> {
    state.desktop.huddle.map_or_else(Task::none, window::close)
}

fn sync_page_presence(state: &mut Shell) {
    let page = if state.screen() == Screen::Pages {
        match &state.user_screens.pages.data {
            user_screens::Resource::Ready(data) => {
                data.document.as_ref().map(|document| document.id.clone())
            }
            _ => None,
        }
    } else {
        None
    };
    let Some(page) = page else {
        state.page_presence = None;
        return;
    };
    if state
        .page_presence
        .as_ref()
        .is_some_and(|presence| presence.page == page)
    {
        return;
    }
    state.page_presence = state
        .node_client
        .as_ref()
        .and_then(|client| page_presence::Handle::start(client, &page).ok())
        .map(|handle| PagePresenceRuntime {
            page,
            handle,
            peers: BTreeMap::new(),
        });
}

fn publish_page_cursor(state: &Shell) {
    let Some(presence) = &state.page_presence else {
        return;
    };
    if !matches!(
        &state.user_screens.pages.data,
        user_screens::Resource::Ready(data)
            if data.document.as_ref().is_some_and(|doc| doc.id == presence.page)
    ) {
        return;
    }
    let (block, anchor, head) = state.user_screens.pages.cursor_presence();
    let _ = presence
        .handle
        .control
        .try_send(page_presence::Control::Cursor {
            block,
            anchor,
            head,
        });
}

fn poll_page_presence(state: &mut Shell) {
    if state.screen() != Screen::Pages {
        state.page_presence = None;
        return;
    }
    let Some(presence) = &mut state.page_presence else {
        return;
    };
    for _ in 0..32 {
        match presence.handle.events.try_recv() {
            Ok(page_presence::Event::Peer(peer)) => {
                presence
                    .peers
                    .insert(peer.peer.clone(), (peer, std::time::Instant::now()));
            }
            Ok(page_presence::Event::Failed(reason)) => tracing::debug!(
                target: "ducktape::pages",
                event = "page_presence_unavailable",
                reason = "connection_failed",
                detail = %reason
            ),
            Ok(page_presence::Event::Closed) => break,
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
        }
    }
    presence
        .peers
        .retain(|_, (_, at)| at.elapsed() < std::time::Duration::from_secs(5));
    if let user_screens::Resource::Ready(data) = &mut state.user_screens.pages.data
        && let Some(document) = &mut data.document
        && document.id == presence.page
    {
        document.presence = presence
            .peers
            .values()
            .map(|(peer, _)| peer.clone())
            .collect();
    }
}

fn send_huddle_beacon(huddle: &HuddleRuntime) {
    if let Some(session) = &huddle.session {
        let _ = session.control.try_send(huddle_session::Control::Beacon {
            muted: huddle.muted,
            camera_on: huddle.camera_on,
            sharing: huddle.sharing,
        });
    }
}

fn poll_huddle(state: &mut Shell) -> Task<Message> {
    let Some(huddle) = &mut state.huddle else {
        return Task::none();
    };
    let mut terminal = None;
    if let Some(session) = &mut huddle.session {
        for _ in 0..16 {
            match session.events.try_recv() {
                Ok(huddle_session::Event::Connecting) => {
                    if huddle.status != HuddleStatus::Reconnecting {
                        huddle.status = HuddleStatus::Connecting;
                    }
                }
                Ok(huddle_session::Event::Live) => {
                    huddle.status = HuddleStatus::Live;
                    huddle.error = None;
                }
                Ok(huddle_session::Event::PeerBeacon {
                    peer,
                    muted,
                    camera_on,
                    sharing,
                }) => {
                    if !camera_on && !sharing {
                        huddle.peer_frames.remove(&peer);
                    }
                    huddle.peers.insert(
                        peer,
                        HuddlePeer {
                            muted,
                            camera_on,
                            sharing,
                            seen: std::time::Instant::now(),
                        },
                    );
                }
                Ok(huddle_session::Event::Closed) => {
                    terminal = Some(None);
                    break;
                }
                Ok(huddle_session::Event::Failed(error)) => {
                    terminal = Some(Some(error));
                    break;
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
            }
        }
    }
    if let Some(error) = terminal {
        let now = std::time::Instant::now();
        let reconnect = error.is_none()
            && huddle.status == HuddleStatus::Live
            && huddle
                .last_reconnect
                .is_none_or(|last| now.duration_since(last) > std::time::Duration::from_secs(30));
        huddle.session.take();
        if let Some(media) = &huddle.media {
            media.send(huddle_media::Command::Stop);
        }
        if reconnect {
            huddle.status = HuddleStatus::Reconnecting;
            huddle.error = None;
            huddle.retry_pending = true;
            huddle.last_reconnect = Some(now);
        } else {
            huddle.status = HuddleStatus::Unavailable;
            huddle.error =
                Some(error.unwrap_or_else(|| "The huddle connection closed".to_string()));
        }
    }
    if let Some(media) = &huddle.media {
        for _ in 0..16 {
            match media.events.try_recv() {
                Ok(huddle_media::Event::Ready) => {}
                Ok(huddle_media::Event::Devices(devices)) => {
                    state.huddle_device_prefs = HuddleDevicePrefs {
                        microphone: devices.microphone,
                        camera: devices.camera,
                        speaker: devices.speaker,
                    };
                    if !devices.screen_sources.is_empty() && devices.screen_source.is_none() {
                        huddle.devices_open = true;
                    }
                    huddle.devices = devices;
                }
                Ok(huddle_media::Event::VideoState { camera_on, sharing }) => {
                    huddle.camera_on = camera_on;
                    huddle.sharing = sharing;
                    if !camera_on && !sharing {
                        huddle.local_frame = None;
                    }
                    if sharing && !huddle.devices.screen_sources.is_empty() {
                        huddle.devices_open = false;
                    }
                    send_huddle_beacon(huddle);
                }
                Ok(huddle_media::Event::LocalFrame(frame)) => {
                    huddle.local_frame = Some(image::Handle::from_rgba(
                        frame.width,
                        frame.height,
                        frame.rgba.as_ref().to_vec(),
                    ));
                }
                Ok(huddle_media::Event::PeerFrame { peer, frame }) => {
                    if huddle.peer_frames.len() < 8 || huddle.peer_frames.contains_key(&peer) {
                        huddle.peer_frames.insert(
                            peer,
                            image::Handle::from_rgba(
                                frame.width,
                                frame.height,
                                frame.rgba.as_ref().to_vec(),
                            ),
                        );
                    }
                }
                Ok(huddle_media::Event::RequestKeyframe(peer)) => {
                    if let Some(session) = &huddle.session {
                        let _ = session
                            .control
                            .try_send(huddle_session::Control::RequestKeyframe(peer));
                    }
                }
                Ok(huddle_media::Event::Failed { kind, detail }) => {
                    huddle.error = Some(format!("{}: {detail}", kind.label()));
                    if matches!(
                        kind,
                        huddle_media::FailureKind::MicrophoneDenied
                            | huddle_media::FailureKind::MicrophoneUnavailable
                            | huddle_media::FailureKind::Unsupported
                    ) {
                        huddle.status = HuddleStatus::Unavailable;
                    }
                }
                Ok(huddle_media::Event::Stopped) => {
                    if !huddle.retry_pending {
                        huddle.status = HuddleStatus::Unavailable;
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
            }
        }
        huddle.level = media.level();
        if huddle.level >= 8 {
            huddle.speaking_until =
                Some(std::time::Instant::now() + std::time::Duration::from_millis(600));
        }
    }
    restart_huddle_if_stopped(state)
}

fn huddle_speaking(huddle: &HuddleRuntime) -> bool {
    huddle
        .speaking_until
        .is_some_and(|until| std::time::Instant::now() < until)
}

fn device_index(value: &str) -> Option<usize> {
    if value == "System default" {
        None
    } else {
        value
            .split_once(" · ")
            .and_then(|(index, _)| index.parse::<usize>().ok())
            .and_then(|index| index.checked_sub(1))
    }
}

fn device_labels(options: &[String]) -> Vec<String> {
    std::iter::once("System default".to_string())
        .chain(
            options
                .iter()
                .enumerate()
                .map(|(index, label)| format!("{} · {label}", index + 1)),
        )
        .collect()
}

fn selected_device(options: &[String], selection: Option<usize>) -> Option<String> {
    Some(selection.map_or_else(
        || "System default".to_string(),
        |index| {
            options.get(index).map_or_else(
                || "System default".to_string(),
                |label| format!("{} · {label}", index + 1),
            )
        },
    ))
}

fn apply_page_shortcut(state: &mut Shell, shortcut: PageShortcut) -> Task<Message> {
    let message = match shortcut {
        PageShortcut::Move(movement) => user_screens::PagesMessage::MoveFocusedBlock(movement),
        PageShortcut::RemoveEmpty => user_screens::PagesMessage::RemoveEmptyFocusedBlock,
        PageShortcut::Activate => user_screens::PagesMessage::ActivateFocusedBlock,
        PageShortcut::Cycle(next) => user_screens::PagesMessage::CycleTab(next),
        PageShortcut::NewPage => user_screens::PagesMessage::NewPage,
        PageShortcut::CloseTab => user_screens::PagesMessage::CloseActiveTab,
    };
    user_screens::update(
        &mut state.user_screens,
        user_screens::Message::Pages(message),
    )
    .map_or_else(Task::none, |command| execute_user_screen(state, command))
}

fn global_shortcut(
    event: iced::Event,
    status: iced::event::Status,
    _window: window::Id,
) -> Option<Message> {
    let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
        key,
        physical_key,
        modifiers,
        ..
    }) = event
    else {
        return None;
    };
    use iced::keyboard::key::{Code, Named};
    match key.as_ref() {
        iced::keyboard::Key::Character(value)
            if modifiers.command() && value.eq_ignore_ascii_case("k") =>
        {
            Some(Message::ToggleSearch)
        }
        iced::keyboard::Key::Character(value)
            if status == iced::event::Status::Ignored
                && modifiers.command()
                && value.eq_ignore_ascii_case("z") =>
        {
            Some(Message::PageHistory(modifiers.shift()))
        }
        iced::keyboard::Key::Character(value)
            if status == iced::event::Status::Ignored
                && modifiers.command()
                && !modifiers.shift()
                && matches!(value.to_ascii_lowercase().as_str(), "t" | "n") =>
        {
            Some(Message::PageShortcut(PageShortcut::NewPage))
        }
        iced::keyboard::Key::Character(value)
            if status == iced::event::Status::Ignored
                && modifiers.command()
                && !modifiers.shift()
                && value.eq_ignore_ascii_case("w") =>
        {
            Some(Message::PageShortcut(PageShortcut::CloseTab))
        }
        _ if status == iced::event::Status::Ignored
            && modifiers.command()
            && modifiers.shift()
            && physical_key == Code::BracketRight =>
        {
            Some(Message::PageShortcut(PageShortcut::Cycle(true)))
        }
        _ if status == iced::event::Status::Ignored
            && modifiers.command()
            && modifiers.shift()
            && physical_key == Code::BracketLeft =>
        {
            Some(Message::PageShortcut(PageShortcut::Cycle(false)))
        }
        iced::keyboard::Key::Named(Named::Tab) => Some(Message::PageShortcut(PageShortcut::Move(
            if modifiers.shift() {
                user_screens::BlockMove::Outdent
            } else {
                user_screens::BlockMove::Indent
            },
        ))),
        iced::keyboard::Key::Named(Named::Backspace) => {
            Some(Message::PageShortcut(PageShortcut::RemoveEmpty))
        }
        iced::keyboard::Key::Named(Named::Enter) if modifiers.command() => {
            Some(Message::PageShortcut(PageShortcut::Activate))
        }
        iced::keyboard::Key::Named(Named::Escape) => Some(Message::Search(search::Message::Close)),
        _ => None,
    }
}

fn close_window(state: &mut Shell, id: window::Id) -> Task<Message> {
    match state.desktop.kind(id) {
        desktop::Kind::Main => {
            state.desktop.main_focused = false;
            #[cfg(target_os = "macos")]
            {
                hide_browser(state);
                mac_tray::main_hidden();
                let cancel = cancel_account_ceremony(state);
                Task::batch([window::set_mode(id, window::Mode::Hidden), cancel])
            }
            #[cfg(not(target_os = "macos"))]
            {
                Task::done(Message::Quit)
            }
        }
        desktop::Kind::Huddle => window::close(id),
        desktop::Kind::Tray => {
            state.desktop.mark_tray_hidden();
            window::close(id)
        }
    }
}

fn quit(state: &mut Shell) -> Task<Message> {
    let channel = state.huddle.take().map(|huddle| huddle.channel);
    state.huddle_expanded = false;
    close_browser(state);
    let Some(channel) = channel else {
        return iced::exit();
    };
    let backend = state.backend.clone();
    let workspace = state.active_workspace.clone();
    let client = state.node_client.clone();
    Task::perform(
        async move {
            let leave = crate::screen_service::execute(
                backend,
                workspace,
                client,
                user_screens::Command::SetHuddle {
                    channel,
                    joined: false,
                },
            );
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), leave).await;
        },
        |_| Message::QuitReady,
    )
}

#[cfg(feature = "cef-browser")]
fn close_browser(state: &mut Shell) {
    if let Some(mut browser) = state.browser.take()
        && let Err(error) = browser.close()
    {
        tracing::warn!(
            target: "ducktape::browser",
            reason = "close_failed",
            error = %error,
            "CEF browser did not close cleanly before its parent window closed"
        );
    }
    state.browser_permission = None;
}

#[cfg(not(feature = "cef-browser"))]
fn close_browser(_state: &mut Shell) {}

fn open_notification_target(state: &mut Shell, target: notifications::Target) -> Task<Message> {
    if let Some((repository, number)) = target
        .channel_id
        .as_deref()
        .and_then(notifications::parse_forge_item_channel)
    {
        state.navigate(Screen::Forge);
        let repository = forge_screen::update(
            &mut state.forge,
            forge_screen::Message::SelectRepository(repository),
        )
        .map(|command| execute_forge(state, command));
        let item = forge_screen::update(&mut state.forge, forge_screen::Message::OpenItem(number))
            .map(|command| execute_forge(state, command));
        return Task::batch(repository.into_iter().chain(item));
    }
    let screen = match target.screen.as_str() {
        "chat" => Screen::Chat,
        "agents" | "agent" | "runs" => Screen::Agents,
        "forge" => Screen::Forge,
        "governance" => Screen::Governance,
        _ => Screen::Chat,
    };
    state.navigate(screen);
    if screen == Screen::Chat
        && let Some(channel) = target.channel_id
        && let Some(command) = user_screens::update(
            &mut state.user_screens,
            user_screens::Message::Chat(user_screens::ChatMessageEvent::SelectChannel(channel)),
        )
    {
        return execute_user_screen(state, command);
    }
    Task::done(Message::Navigate(screen))
}

fn cancel_account_ceremony(state: &mut Shell) -> Task<Message> {
    if !matches!(
        state.user_screens.home.account.panel,
        user_screens::AccountPanel::Link | user_screens::AccountPanel::Phone
    ) {
        return Task::none();
    }
    user_screens::update(
        &mut state.user_screens,
        user_screens::Message::Home(user_screens::HomeMessage::CancelAccountPanel),
    )
    .map_or_else(Task::none, |command| execute_user_screen(state, command))
}

fn notification_config(state: &Shell) -> notifications::Config {
    let preferences = &state.settings.notifications;
    let (self_user_key, self_node_keys) = match &state.user_screens.home.data {
        user_screens::Resource::Ready(data) => (
            data.custody
                .as_ref()
                .map(|custody| custody.public_key.clone()),
            data.devices
                .iter()
                .map(|device| device.key.clone())
                .collect(),
        ),
        _ => (None, Vec::new()),
    };
    let author_names = match &state.members.data {
        members_screen::Resource::Ready(data) => data
            .members
            .iter()
            .map(|member| (member.key.to_ascii_lowercase(), member.display_name.clone()))
            .collect(),
        _ => std::collections::BTreeMap::new(),
    };
    notifications::Config {
        enabled: preferences.enabled,
        mentions: preferences.mentions,
        replies: preferences.replies,
        huddles: preferences.huddles,
        runs: preferences.runs,
        forge: preferences.forge,
        governance: preferences.governance,
        muted_channels: preferences.muted_channels.clone(),
        self_user_key,
        self_node_keys,
        author_names,
        focused_channel: state.user_screens.chat.active_channel.clone(),
        main_focused: state.desktop.main_focused,
    }
}

fn governance_stream_frame(state: &mut Shell, frame: &ServerFrame) -> Task<Message> {
    let changed = matches!(
        frame,
        ServerFrame::Event { topic, .. } | ServerFrame::Lagged { topic, .. }
            if topic == "module:governance"
    );
    if changed {
        governance_stream_reload(state)
    } else {
        Task::none()
    }
}

fn governance_stream_reload(state: &mut Shell) -> Task<Message> {
    if state.screen() != Screen::Governance {
        return Task::none();
    }
    governance_screen::update(&mut state.governance, governance_screen::Message::Refresh)
        .map_or_else(Task::none, |command| execute_governance(state, command))
}

fn present_notification(state: &mut Shell, item: notifications::Item) -> Task<Message> {
    state.notifications.push(item.clone());
    mac_tray::set_unread(state.notifications.unread);
    Task::perform(
        notifications::present(item),
        Message::NativeNotificationActivated,
    )
}

#[cfg(feature = "cef-browser")]
fn sync_browser_visibility(state: &mut Shell) -> Task<Message> {
    let visible = state.screen() == Screen::Browser
        && !state.workspace_overlay
        && !state.search.open
        && state.browser_permission.is_none();
    if state
        .browser
        .as_ref()
        .is_some_and(BrowserRuntime::has_surface)
    {
        let browser = state.browser.as_mut().expect("browser presence checked");
        if let Err(error) = browser.set_visible(visible) {
            state.browser_error = Some(error);
        }
        Task::none()
    } else if visible {
        state.browser_chrome.loading = true;
        if state.browser_gateway_base.is_some() {
            if let Some(browser) = &mut state.browser {
                let result = state
                    .browser_chrome
                    .url()
                    .and_then(|url| browser.reopen(&url));
                match result {
                    Ok(()) => {
                        state.browser_chrome.loading = false;
                        if let Err(error) = browser.set_visible(true) {
                            state.browser_error = Some(error);
                        }
                    }
                    Err(error) => {
                        state.browser_chrome.loading = false;
                        state.browser_chrome.error = Some(error.clone());
                        state.browser_error = Some(error);
                    }
                }
                Task::none()
            } else {
                Task::done(Message::BrowserWindowReady(state.desktop.main))
            }
        } else if state.browser_gateway_loading {
            Task::none()
        } else if let Some(client) = state.node_client.clone() {
            state.browser_gateway_loading = true;
            let generation = state.browser_gateway_generation;
            let workspace_id = browser_gateway_workspace(state);
            Task::perform(
                async move {
                    client
                        .gateway_browser_base()
                        .await
                        .map_err(|error| error.to_string())
                },
                move |result| Message::BrowserGatewayLoaded {
                    generation,
                    workspace_id,
                    result,
                },
            )
        } else {
            state.browser_chrome.loading = false;
            state.browser_chrome.error = Some("Connect a workspace to browse .duck routes.".into());
            Task::none()
        }
    } else {
        Task::none()
    }
}

#[cfg(not(feature = "cef-browser"))]
fn sync_browser_visibility(state: &mut Shell) -> Task<Message> {
    if state.screen() == Screen::Browser && !state.workspace_overlay && !state.search.open {
        state.browser_chrome.error =
            Some("This build does not include the embedded CEF browser.".into());
    }
    Task::none()
}

#[cfg(feature = "cef-browser")]
fn browser_bounds(size: Size) -> BrowserBounds {
    BrowserBounds {
        x: (NETWORK_RAIL_WIDTH + MODULE_RAIL_WIDTH) as i32,
        y: (TITLEBAR_HEIGHT + 36.0 + 48.0) as i32,
        width: (size.width - NETWORK_RAIL_WIDTH - MODULE_RAIL_WIDTH)
            .max(1.0)
            .round() as i32,
        height: (size.height - TITLEBAR_HEIGHT - 36.0 - 48.0)
            .max(1.0)
            .round() as i32,
    }
}

fn execute_onboarding(backend: Option<Backend>, command: onboarding::Command) -> Task<Message> {
    if let onboarding::Command::CopyText(value) = command {
        return iced::clipboard::write(value.into_string()).map(|()| {
            Message::Onboarding(onboarding::Message::Service(
                onboarding::ServiceEvent::TextCopied(Ok(())),
            ))
        });
    }
    if matches!(
        command,
        onboarding::Command::GateCompleted | onboarding::Command::GateSkipped
    ) {
        return Task::none();
    }

    Task::perform(
        async move {
            let backend = backend.ok_or_else(|| "desktop backend is unavailable".to_string());
            match command {
                onboarding::Command::LoadIdentity => {
                    let result = match backend {
                        Ok(backend) => {
                            let (identity, touch_id) = tokio::join!(
                                backend.identity_state(),
                                backend.touch_id_available(),
                            );
                            identity.map(|report| onboarding::IdentityReport {
                                kind: match report.state {
                                    IdentityStatus::Absent => onboarding::IdentityKind::Absent,
                                    IdentityStatus::Plaintext => {
                                        onboarding::IdentityKind::Plaintext
                                    }
                                    IdentityStatus::Locked => onboarding::IdentityKind::Locked,
                                    IdentityStatus::Unlocked => onboarding::IdentityKind::Unlocked,
                                },
                                mnemonic_confirmed: report.mnemonic_confirmed,
                                touch_id_available: touch_id.unwrap_or(false),
                            })
                        }
                        Err(error) => Err(error),
                    };
                    onboarding::ServiceEvent::IdentityLoaded(result)
                }
                onboarding::Command::CreateIdentity {
                    password,
                    display_name: _,
                } => {
                    let result =
                        match backend {
                            Ok(backend) => backend
                                .create_identity(password.into_string())
                                .await
                                .map(|created| onboarding::CreatedIdentity {
                                    mnemonic: created.mnemonic.as_str().to_owned(),
                                }),
                            Err(error) => Err(error),
                        };
                    onboarding::ServiceEvent::IdentityCreated(result)
                }
                onboarding::Command::CreateIdentityWithTouchId { display_name: _ } => {
                    let result = match backend {
                        Ok(backend) => {
                            backend.create_identity_for_touch_id().await.map(|created| {
                                onboarding::CreatedIdentity {
                                    mnemonic: created.mnemonic.as_str().to_owned(),
                                }
                            })
                        }
                        Err(error) => Err(error),
                    };
                    onboarding::ServiceEvent::IdentityCreated(result)
                }
                onboarding::Command::ConfirmMnemonic => {
                    let result = match backend {
                        Ok(backend) => backend.confirm_recovery().await,
                        Err(error) => Err(error),
                    };
                    onboarding::ServiceEvent::MnemonicConfirmed(result)
                }
                onboarding::Command::RestoreIdentity { mnemonic, password } => {
                    let result = match backend {
                        Ok(backend) => backend
                            .restore_identity(mnemonic.into_string(), password.into_string())
                            .await
                            .map(|_| ()),
                        Err(error) => Err(error),
                    };
                    onboarding::ServiceEvent::IdentityRestored(result)
                }
                onboarding::Command::PrepareLinkIdentity { password } => {
                    let result = match backend {
                        Ok(backend) => {
                            match backend.create_identity(password.into_string()).await {
                                Ok(_) => backend.confirm_recovery().await,
                                Err(error) => Err(error),
                            }
                        }
                        Err(error) => Err(error),
                    };
                    onboarding::ServiceEvent::LinkIdentityPrepared(result)
                }
                onboarding::Command::GenerateLinkResponse {
                    challenge,
                    device_label,
                } => {
                    let result = match backend {
                        Ok(backend) => {
                            generate_link_reply(backend, challenge.into_string(), device_label)
                                .await
                        }
                        Err(error) => Err(error),
                    };
                    onboarding::ServiceEvent::LinkResponseGenerated(result)
                }
                onboarding::Command::UnlockIdentity { password } => {
                    let result = match backend {
                        Ok(backend) => backend
                            .unlock_identity(password.into_string())
                            .await
                            .map(|_| ()),
                        Err(error) => Err(error),
                    };
                    onboarding::ServiceEvent::IdentityUnlocked(result)
                }
                onboarding::Command::UnlockWithTouchId => {
                    let result = match backend {
                        Ok(backend) => backend.touch_id_unlock().await.map(|_| ()),
                        Err(error) => Err(error),
                    };
                    onboarding::ServiceEvent::TouchIdUnlocked(result)
                }
                onboarding::Command::EnrollTouchIdSession => {
                    let result = match backend {
                        Ok(backend) => backend.touch_id_enroll_session().await,
                        Err(error) => Err(error),
                    };
                    onboarding::ServiceEvent::TouchIdEnrolled(result)
                }
                onboarding::Command::EncryptLegacy { password } => {
                    let result = match backend {
                        Ok(backend) => backend
                            .encrypt_legacy_identity(password.into_string())
                            .await
                            .map(|_| ()),
                        Err(error) => Err(error),
                    };
                    onboarding::ServiceEvent::LegacyEncrypted(result)
                }
                onboarding::Command::RevealMnemonic { password } => {
                    let result = match backend {
                        Ok(backend) => backend
                            .reveal_identity(password.into_string())
                            .await
                            .map(|revealed| revealed.mnemonic.as_str().to_owned()),
                        Err(error) => Err(error),
                    };
                    onboarding::ServiceEvent::MnemonicRevealed(result)
                }
                onboarding::Command::CopyText(_)
                | onboarding::Command::GateCompleted
                | onboarding::Command::GateSkipped => unreachable!("handled before task"),
            }
        },
        |event| Message::Onboarding(onboarding::Message::Service(event)),
    )
}

async fn generate_link_reply(
    backend: Backend,
    input: String,
    device_label: Option<String>,
) -> Result<onboarding::LinkReply, String> {
    let input = input.trim();
    let address = input
        .starts_with("http://")
        .then(|| LinkAddress::parse(input.to_owned()))
        .transpose()?;
    let challenge = match address.clone() {
        Some(address) => backend.link_fetch_challenge(address).await?,
        None => decode_link_challenge(input)?,
    };
    let identity = backend.identity_state().await?;
    let pubkey = identity
        .pubkey
        .ok_or_else(|| "unlock this device identity before linking it".to_string())?;
    let possession = backend
        .sign_possession(PossessionRequest {
            chain_id: challenge.chain_id.clone(),
            account_id: challenge.account_id.clone(),
            nonce: challenge.nonce,
        })
        .await?;
    let response = LinkResponse {
        pubkey: pubkey.clone(),
        kind: MemberKeyKind::Ed25519,
        possession,
        label: device_label,
    };
    let encoded = encode_link_response(&response)?;
    backend
        .link_pending_mark(LinkPending {
            chain_id: challenge.chain_id,
            account_id: challenge.account_id,
            member_key: pubkey.clone(),
        })
        .await?;
    let sent_automatically = match address {
        Some(address) => backend.link_send_response(address, response).await.is_ok(),
        None => false,
    };
    Ok(onboarding::LinkReply {
        response: encoded,
        account_name: challenge.name,
        device_key: Some(pubkey),
        sent_automatically,
    })
}

fn load_current_user_screen(state: &mut Shell) -> Task<Message> {
    if state.screen() == Screen::Forge {
        return forge_screen::update(&mut state.forge, forge_screen::Message::Load)
            .map_or_else(Task::none, |command| execute_forge(state, command));
    }
    if state.screen() == Screen::Agents {
        return agents_screen::update(&mut state.agents, agents_screen::Message::Load)
            .map_or_else(Task::none, |command| execute_agents(state, command));
    }
    if state.screen() == Screen::Members {
        return members_screen::update(&mut state.members, members_screen::Message::Load)
            .map_or_else(Task::none, |command| execute_members(state, command));
    }
    if state.screen() == Screen::Governance {
        return governance_screen::update(&mut state.governance, governance_screen::Message::Load)
            .map_or_else(Task::none, |command| execute_governance(state, command));
    }
    if state.screen() == Screen::Explorer {
        return explorer_screen::update(&mut state.explorer, explorer_screen::Message::Load)
            .map_or_else(Task::none, |command| execute_explorer(state, command));
    }
    let Some(screen) = user_screen(state.screen()) else {
        return Task::none();
    };
    let Some(command) =
        user_screens::update(&mut state.user_screens, user_screens::Message::Load(screen))
    else {
        return Task::none();
    };
    execute_user_screen(state, command)
}

fn load_notification_context(state: &mut Shell) -> Task<Message> {
    let home = if state.screen() == Screen::Home {
        Task::none()
    } else {
        user_screens::update(
            &mut state.user_screens,
            user_screens::Message::Load(user_screens::Screen::Home),
        )
        .map_or_else(Task::none, |command| execute_user_screen(state, command))
    };
    let members = if state.screen() == Screen::Members {
        Task::none()
    } else {
        members_screen::update(&mut state.members, members_screen::Message::Load)
            .map_or_else(Task::none, |command| execute_members(state, command))
    };
    Task::batch([home, members])
}

fn finish_workspace_connection(state: &mut Shell) -> Task<Message> {
    Task::batch([
        load_current_user_screen(state),
        load_notification_context(state),
        sync_browser_visibility(state),
    ])
}

fn after_gate_ready(state: &mut Shell) -> Task<Message> {
    if let Some(active) = state.active_workspace.as_ref() {
        let message = workspace_screens::Message::SelectWorkspace(active.id.clone());
        if let Some(command) = workspace_screens::update(&mut state.workspace, message) {
            return execute_workspace(state, command);
        }
    } else {
        state.workspace_overlay = true;
        if let Some(command) =
            workspace_screens::update(&mut state.workspace, workspace_screens::Message::Open)
        {
            return execute_workspace(state, command);
        }
    }
    load_current_user_screen(state)
}

fn update_user_module(state: &mut Shell, message: module_host::Message) -> Task<Message> {
    let effect = module_host::update(&mut state.user_screens, message);
    execute_user_module_effect(state, effect)
}

fn apply_user_module_event(state: &mut Shell, event: module_host::Event) -> Task<Message> {
    let effect = module_host::apply_event(&mut state.user_screens, event);
    execute_user_module_effect(state, effect)
}

fn execute_user_module_effect(
    state: &mut Shell,
    effect: Option<module_host::Effect>,
) -> Task<Message> {
    let huddle = sync_huddle_runtime(state);
    sync_page_presence(state);
    publish_page_cursor(state);
    let Some(effect) = effect else {
        return huddle;
    };
    if let Some(intent) = effect.intent().cloned() {
        let browser = if matches!(intent, AppIntent::PopOutHuddle) {
            Task::none()
        } else {
            sync_browser_visibility(state)
        };
        return Task::batch([huddle, open_app_intent(state, intent), browser]);
    }
    if let Some(user_screens::Command::UploadDropped { target, token }) = effect.command() {
        let target = target.clone();
        let source = state.dropped_files.consume(*token);
        return Task::batch([
            huddle,
            Task::perform(
                adapters::execute_drop(
                    state.backend.clone(),
                    state.node_client.clone(),
                    target,
                    source,
                ),
                Message::UserModule,
            ),
        ]);
    }
    if user_effect_is_shell_owned(&effect) {
        return Task::batch([huddle, execute_shell_owned_user_effect(state, effect)]);
    }
    Task::batch([
        huddle,
        Task::perform(
            adapters::execute_module(
                state.backend.clone(),
                state.active_workspace.clone(),
                state.node_client.clone(),
                effect,
            ),
            Message::UserModule,
        ),
    ])
}

fn user_effect_is_shell_owned(effect: &module_host::Effect) -> bool {
    effect.command().is_some_and(|command| {
        matches!(
            command,
            user_screens::Command::SwitchWorkspace(_)
                | user_screens::Command::AddNetwork
                | user_screens::Command::UnlockAccount
                | user_screens::Command::SecureAccount
                | user_screens::Command::RevealRecovery
                | user_screens::Command::CommitPageAfter { .. }
                | user_screens::Command::CopyText(_)
                | user_screens::Command::ReadPageClipboard(_)
                | user_screens::Command::FocusPageBlock(_)
        )
    })
}

fn execute_shell_owned_user_effect(
    state: &mut Shell,
    effect: module_host::Effect,
) -> Task<Message> {
    let command = match effect {
        module_host::Effect::Home(effect) => effect.into_command(),
        module_host::Effect::Chat(module_host::ChatEffect::Command(effect)) => {
            effect.into_command()
        }
        module_host::Effect::Pages(effect) => effect.into_command(),
        module_host::Effect::Files(effect) => effect.into_command(),
        module_host::Effect::Chat(module_host::ChatEffect::Intent(intent)) => {
            return open_app_intent(state, intent);
        }
    };
    execute_user_screen(state, command)
}

fn execute_user_screen(state: &mut Shell, command: user_screens::Command) -> Task<Message> {
    match command {
        user_screens::Command::SwitchWorkspace(id) => {
            state.workspace_overlay = true;
            hide_browser(state);
            return workspace_screens::update(
                &mut state.workspace,
                workspace_screens::Message::SelectWorkspace(id),
            )
            .map_or_else(Task::none, |command| execute_workspace(state, command));
        }
        user_screens::Command::AddNetwork => {
            state.workspace_overlay = true;
            hide_browser(state);
            return workspace_screens::update(
                &mut state.workspace,
                workspace_screens::Message::Open,
            )
            .map_or_else(Task::none, |command| execute_workspace(state, command));
        }
        user_screens::Command::UnlockAccount => {
            hide_browser(state);
            onboarding::begin_account_action(
                &mut state.onboarding,
                onboarding::AccountAction::Unlock,
            );
            return Task::none();
        }
        user_screens::Command::SecureAccount => {
            hide_browser(state);
            onboarding::begin_account_action(
                &mut state.onboarding,
                onboarding::AccountAction::Secure,
            );
            return Task::none();
        }
        user_screens::Command::RevealRecovery => {
            hide_browser(state);
            onboarding::begin_account_action(
                &mut state.onboarding,
                onboarding::AccountAction::RevealRecovery,
            );
            return Task::none();
        }
        user_screens::Command::CommitPageAfter { block, generation } => {
            return Task::perform(
                async move {
                    tokio::time::sleep(std::time::Duration::from_millis(550)).await;
                    (block, generation)
                },
                |(block, generation)| {
                    Message::UserScreen(user_screens::Message::Pages(
                        user_screens::PagesMessage::CommitBlockIf { block, generation },
                    ))
                },
            );
        }
        _ => {}
    }
    if let user_screens::Command::CopyText(value) = command {
        return iced::clipboard::write(value).map(|()| {
            Message::UserScreen(user_screens::Message::Service(
                user_screens::ServiceEvent::ActionFinished {
                    screen: user_screens::Screen::Home,
                    result: Ok(()),
                },
            ))
        });
    }
    if let user_screens::Command::ReadPageClipboard(index) = command {
        return iced::clipboard::read().map(move |value| {
            Message::UserScreen(user_screens::Message::Pages(
                user_screens::PagesMessage::PasteBlocks(index, value.unwrap_or_default()),
            ))
        });
    }
    if let user_screens::Command::FocusPageBlock(block) = command {
        return iced::widget::operation::focus(iced::widget::Id::from(
            user_screens::page_block_input_id(&block),
        ));
    }
    Task::perform(
        adapters::execute_user(
            state.backend.clone(),
            state.active_workspace.clone(),
            state.node_client.clone(),
            command,
        ),
        |event| Message::UserScreen(user_screens::Message::Service(event)),
    )
}

fn execute_operator(state: &mut Shell, command: operator_screens::Command) -> Task<Message> {
    if let operator_screens::Command::CopyText(value) = command {
        return iced::clipboard::write(value).map(|()| {
            Message::Operator(operator_screens::Message::Service(
                operator_screens::ServiceEvent::ActionFinished {
                    screen: operator_screens::Screen::Node,
                    result: Ok(()),
                },
            ))
        });
    }
    Task::perform(
        operator_service::execute_operator(
            state.backend.clone(),
            state.node_client.clone(),
            state.active_workspace.clone(),
            command,
        ),
        |event| Message::Operator(operator_screens::Message::Service(event)),
    )
}

fn execute_forge(state: &Shell, command: forge_screen::Command) -> Task<Message> {
    Task::perform(
        forge_agents_service::execute_forge(
            state.backend.clone(),
            state.active_workspace.clone(),
            state.node_client.clone(),
            command,
        ),
        |event| Message::Forge(forge_screen::Message::Service(event)),
    )
}

fn execute_search(state: &mut Shell, command: search::Command) -> Task<Message> {
    match command {
        search::Command::Focus => search::focus(),
        search::Command::Search { generation, query } => Task::perform(
            search::search(state.node_client.clone(), query),
            move |result| Message::Search(search::Message::SearchFinished { generation, result }),
        ),
        search::Command::Selected(target) => open_search_target(state, target),
    }
}

fn open_search_target(state: &mut Shell, target: search::Target) -> Task<Message> {
    let intent = match target {
        search::Target::Chat {
            channel_id,
            sequence,
        } => AppIntent::Navigate(Route::Chat {
            channel: Some(channel_id),
            message: Some(sequence),
        }),
        search::Target::Page { page_id, block_id } => AppIntent::Navigate(Route::Page {
            page: page_id,
            block: Some(block_id),
        }),
        search::Target::Member { account_id, key } => AppIntent::Navigate(Route::Member {
            key,
            account: account_id,
        }),
        search::Target::File { path, directory } => {
            AppIntent::Navigate(Route::File { path, directory })
        }
    };
    Task::batch([
        open_app_intent(state, intent),
        sync_browser_visibility(state),
    ])
}

fn open_app_intent(state: &mut Shell, intent: AppIntent) -> Task<Message> {
    match intent {
        AppIntent::Navigate(Route::Home) => {
            state.navigate(Screen::Home);
            load_current_user_screen(state)
        }
        AppIntent::Navigate(Route::Chat { channel, message }) => {
            state.navigate(Screen::Chat);
            match channel {
                Some(id) => update_user_module(
                    state,
                    module_host::Message::Chat(user_screens::ChatMessageEvent::OpenLink(
                        user_screens::ChatLink::Channel {
                            id,
                            sequence: message,
                        },
                    )),
                ),
                None => load_current_user_screen(state),
            }
        }
        AppIntent::Navigate(Route::Page { page, block }) => {
            state.navigate(Screen::Pages);
            let message = match block {
                Some(block) => user_screens::PagesMessage::OpenPageAt { page, block },
                None => user_screens::PagesMessage::OpenPage(page),
            };
            update_user_module(state, module_host::Message::Pages(message))
        }
        AppIntent::Navigate(Route::File { path, directory }) => {
            state.navigate(Screen::Files);
            update_user_module(
                state,
                module_host::Message::Files(user_screens::FilesMessage::OpenEntry(
                    path,
                    if directory {
                        user_screens::FileKind::Directory
                    } else {
                        user_screens::FileKind::File
                    },
                )),
            )
        }
        AppIntent::Navigate(Route::Forge { repository, item }) => {
            state.navigate(Screen::Forge);
            let repository = forge_screen::update(
                &mut state.forge,
                forge_screen::Message::SelectRepository(repository),
            )
            .map_or_else(Task::none, |command| execute_forge(state, command));
            let item = item
                .and_then(|number| {
                    forge_screen::update(&mut state.forge, forge_screen::Message::OpenItem(number))
                })
                .map_or_else(Task::none, |command| execute_forge(state, command));
            Task::batch([repository, item])
        }
        AppIntent::Navigate(Route::Member { key, account }) => {
            state.navigate(Screen::Members);
            let message = account.map_or_else(
                || members_screen::Message::Select(key),
                members_screen::Message::FocusAccount,
            );
            members_screen::update(&mut state.members, message)
                .map_or_else(Task::none, |command| execute_members(state, command))
        }
        AppIntent::Navigate(Route::Agent { id }) => {
            state.navigate(Screen::Agents);
            let _ = agents_screen::update(
                &mut state.agents,
                agents_screen::Message::SelectFocusedAgent(id),
            );
            Task::none()
        }
        AppIntent::OpenExternal(address) => {
            state.navigate(Screen::Browser);
            state.browser_chrome.address = address;
            if let Some(_url) =
                browser_chrome::update(&mut state.browser_chrome, browser_chrome::Message::Open)
            {
                #[cfg(feature = "cef-browser")]
                if let Some(browser) = &state.browser {
                    if let Err(error) = browser.navigate(&_url) {
                        state.browser_chrome.loading = false;
                        state.browser_chrome.error = Some(error);
                    }
                }
            }
            sync_browser_visibility(state)
        }
        AppIntent::PopOutHuddle => Task::done(Message::OpenHuddle),
    }
}

fn search_catalog(state: &Shell) -> search::Catalog {
    let members = search_members(state);
    let files = match &state.user_screens.files.data {
        user_screens::Resource::Ready(listing) => listing
            .entries
            .iter()
            .map(|entry| search::FileHit {
                path: entry.path.clone(),
                name: entry.name.clone(),
                directory: entry.kind == user_screens::FileKind::Directory,
            })
            .collect(),
        _ => Vec::new(),
    };
    search::Catalog {
        members,
        files,
        client_mode: state.active_workspace.is_none() && state.node_client.is_some(),
    }
}

fn search_members(state: &Shell) -> Vec<search::MemberHit> {
    match &state.members.data {
        members_screen::Resource::Ready(data) => data
            .members
            .iter()
            .map(|member| search::MemberHit {
                account_id: member
                    .bound_account
                    .as_ref()
                    .map(|account| account.id.clone()),
                key: member.key.clone(),
                name: member.display_name.clone(),
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn refresh_search_members(state: &mut Shell) {
    if state.search.open {
        let members = search_members(state);
        let _ = search::update(&mut state.search, search::Message::MembersLoaded(members));
    }
}

fn notifications_overlay(state: &Shell) -> Element<'_, Message> {
    let p = theme::palette(state.mode);
    let dismiss = mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
        .on_press(Message::CloseNotifications);
    let mut items = column![].spacing(2);
    if state.notifications.recent.is_empty() {
        items = items.push(
            container(
                column![
                    text("No notifications").size(12).font(theme::SANS_SEMIBOLD),
                    text("Mentions, replies, huddles, runs, Forge, and governance updates appear here.")
                        .size(10.5)
                        .color(p.muted),
                ]
                .spacing(5),
            )
            .padding([14, 12]),
        );
    } else {
        let groups = notifications::groups(&state.notifications.recent, |id| {
            match &state.user_screens.chat.data {
                user_screens::Resource::Ready(data) => data
                    .channels
                    .iter()
                    .find(|channel| channel.id == id)
                    .map(|channel| channel.name.clone())
                    .unwrap_or_else(|| id.to_owned()),
                _ => id.to_owned(),
            }
        });
        for group in groups {
            if group.indices.len() == 1 {
                let index = group.indices[0];
                if let Some(item) = state.notifications.recent.get(index) {
                    items = items.push(notification_item_row(item, index, p));
                }
                continue;
            }
            let expanded = state.notifications.expanded.as_deref() == Some(&group.key);
            items = items.push(
                button(
                    column![
                        row![
                            text(if expanded { "▾" } else { "▸" })
                                .size(10.5)
                                .color(p.muted_2),
                            text(group.label.clone())
                                .size(11)
                                .font(theme::SANS_SEMIBOLD)
                                .width(Length::Fill),
                            text(notification_time(
                                state.notifications.recent[group.indices[0]].at
                            ))
                            .size(9.5)
                            .color(p.muted_2),
                        ]
                        .spacing(6)
                        .align_y(Alignment::Center),
                        text(group.summary(&state.notifications.recent))
                            .size(10.5)
                            .color(p.muted),
                    ]
                    .spacing(3),
                )
                .width(Length::Fill)
                .padding([7, 9])
                .on_press(Message::ToggleNotificationGroup(group.key.clone()))
                .style(move |_, status| notification_row_style(status, p)),
            );
            if expanded {
                let mut nested = column![].spacing(2).padding(iced::Padding {
                    top: 0.0,
                    right: 0.0,
                    bottom: 0.0,
                    left: 12.0,
                });
                for index in group.indices {
                    if let Some(item) = state.notifications.recent.get(index) {
                        nested = nested.push(notification_item_row(item, index, p));
                    }
                }
                items = items.push(nested);
            }
        }
    }
    let panel = container(
        column![
            row![
                text("Notifications")
                    .size(12.5)
                    .font(theme::SANS_SEMIBOLD)
                    .width(Length::Fill),
                text("Seen").size(9.5).font(theme::MONO).color(p.muted),
            ]
            .align_y(Alignment::Center),
            scrollable(items).height(Length::Shrink),
        ]
        .spacing(6),
    )
    .width(320)
    .max_height(400)
    .padding(4)
    .style(move |_| bordered(p.paper, p.border, 8.0));
    let anchored = container(panel)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::alignment::Horizontal::Right)
        .align_y(iced::alignment::Vertical::Top)
        .padding(iced::Padding {
            top: 48.0,
            right: 13.0,
            bottom: 0.0,
            left: 0.0,
        });
    stack![dismiss, anchored]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn notification_item_row<'a>(
    item: &'a notifications::Item,
    index: usize,
    p: &'a theme::Palette,
) -> Element<'a, Message> {
    button(
        column![
            row![
                text(&item.title)
                    .size(11.5)
                    .font(theme::SANS_SEMIBOLD)
                    .width(Length::Fill),
                text(notification_time(item.at)).size(9.5).color(p.muted_2),
            ]
            .spacing(8),
            text(if item.body.is_empty() {
                item.category.fallback_screen()
            } else {
                &item.body
            })
            .size(10.5)
            .color(p.muted),
        ]
        .spacing(4),
    )
    .width(Length::Fill)
    .padding([7, 9])
    .on_press(Message::OpenNotification(index))
    .style(move |_, status| notification_row_style(status, p))
    .into()
}

fn notification_row_style(status: button::Status, p: &theme::Palette) -> button::Style {
    button::Style {
        background: matches!(status, button::Status::Hovered).then_some(Background::Color(p.hover)),
        text_color: p.ink,
        border: Border {
            radius: 5.0.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

fn notification_time(at: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64);
    let seconds = now.saturating_sub(at) / 1_000;
    match seconds {
        0..=59 => "now".into(),
        60..=3_599 => format!("{}m", seconds / 60),
        3_600..=86_399 => format!("{}h", seconds / 3_600),
        _ => format!("{}d", seconds / 86_400),
    }
}

fn execute_agents(state: &Shell, command: agents_screen::Command) -> Task<Message> {
    Task::perform(
        forge_agents_service::execute_agents(
            state.backend.clone(),
            state.node_client.clone(),
            command,
        ),
        |event| Message::Agents(agents_screen::Message::Service(event)),
    )
}

fn execute_members(state: &mut Shell, command: members_screen::Command) -> Task<Message> {
    match command {
        members_screen::Command::CopyText(value) => {
            return iced::clipboard::write(value).map(|()| Message::ClipboardWritten);
        }
        members_screen::Command::ClearFocus => return Task::none(),
        _ => {}
    }
    Task::perform(
        community_service::execute_members(
            state.backend.clone(),
            state.node_client.clone(),
            state.active_workspace.clone(),
            command,
        ),
        |event| Message::Members(members_screen::Message::Service(event)),
    )
}

fn execute_governance(state: &Shell, command: governance_screen::Command) -> Task<Message> {
    Task::perform(
        community_service::execute_governance(
            state.backend.clone(),
            state.node_client.clone(),
            state.active_workspace.clone(),
            command,
        ),
        |event| Message::Governance(governance_screen::Message::Service(event)),
    )
}

fn execute_explorer(state: &Shell, command: explorer_screen::Command) -> Task<Message> {
    if command == explorer_screen::Command::ClearFocus {
        return Task::none();
    }
    Task::perform(
        community_service::execute_explorer(
            state.node_client.clone(),
            state.active_workspace.clone(),
            command,
        ),
        |event| Message::Explorer(explorer_screen::Message::Service(event)),
    )
}

fn execute_settings(state: &mut Shell, command: settings_screen::Command) -> Task<Message> {
    match command {
        settings_screen::Command::OpenAccount => {
            state.navigate(Screen::Home);
            return load_current_user_screen(state);
        }
        settings_screen::Command::OpenNetworks => {
            state.workspace_overlay = true;
            hide_browser(state);
            return workspace_screens::update(
                &mut state.workspace,
                workspace_screens::Message::Open,
            )
            .map_or_else(Task::none, |command| execute_workspace(state, command));
        }
        settings_screen::Command::OpenMembers => {
            state.navigate(Screen::Members);
            let load = members_screen::update(&mut state.members, members_screen::Message::Load)
                .map_or_else(Task::none, |command| execute_members(state, command));
            return Task::batch([load, sync_browser_visibility(state)]);
        }
        settings_screen::Command::OpenNode => {
            state.navigate(Screen::Node);
            if let Some(command) = operator_screens::update(
                &mut state.operator,
                operator_screens::Message::Load(operator_screens::Screen::Node),
            ) {
                return execute_operator(state, command);
            }
            return Task::none();
        }
        settings_screen::Command::SetTheme(mode) => {
            state.mode = mode;
            state.settings.mode = mode;
        }
        settings_screen::Command::SetAccent(index) => {
            if let Some(accent) = theme::ACCENTS.get(index) {
                state.accent = *accent;
                state.settings.accent = index;
            }
        }
        _ => {}
    }
    let context = SettingsContext {
        active_channel: state.user_screens.chat.active_channel.clone(),
        forget_needs_force: matches!(
            &state.settings.data,
            settings_screen::Resource::Ready(data) if data.forget_needs_force
        ),
    };
    Task::perform(
        operator_service::execute_settings(
            state.backend.clone(),
            state.node_client.clone(),
            state.active_workspace.clone(),
            context,
            command,
        ),
        |event| Message::Settings(settings_screen::Message::Service(event)),
    )
}

fn execute_workspace(state: &mut Shell, command: workspace_screens::Command) -> Task<Message> {
    match command {
        workspace_screens::Command::CopyText(value) => {
            return iced::clipboard::write(value).map(|()| {
                Message::Workspace(workspace_screens::Message::Service(
                    workspace_screens::ServiceEvent::TextCopied(Ok(())),
                ))
            });
        }
        workspace_screens::Command::ClearCopiedAfter { millis } => {
            return Task::perform(
                async move {
                    tokio::time::sleep(std::time::Duration::from_millis(millis)).await;
                },
                |()| Message::Workspace(workspace_screens::Message::ClearCopied),
            );
        }
        workspace_screens::Command::Connected(target) => match target {
            workspace_screens::ConnectionTarget::Workspace(_) => {
                state.workspace_overlay = false;
                let Some(backend) = state.backend.clone() else {
                    state.workspace.error = Some("desktop backend is unavailable".into());
                    state.workspace_overlay = true;
                    return Task::none();
                };
                return Task::perform(
                    async move { backend.workspace_snapshot().await },
                    Message::WorkspaceSnapshotLoaded,
                );
            }
            workspace_screens::ConnectionTarget::Remote(url) => {
                state.huddle = None;
                state.huddle_expanded = false;
                let close_huddle = close_huddle_window(state);
                state.section = Section::User;
                state.node_client = NodeClient::new(&url).ok();
                state.active_workspace = None;
                state.workspace_overlay = false;
                reset_browser_gateway(state);
                return Task::batch([close_huddle, load_current_user_screen(state)]);
            }
        },
        workspace_screens::Command::Dismiss => {
            state.workspace_overlay = state.node_client.is_none();
            return sync_browser_visibility(state);
        }
        workspace_screens::Command::ActivateWorkspace { .. } => {
            state.huddle = None;
            state.huddle_expanded = false;
            let close_huddle = close_huddle_window(state);
            state.node_client = None;
            reset_browser_gateway(state);
            return Task::batch([
                close_huddle,
                Task::perform(
                    workspace_service::execute(state.backend.clone(), command),
                    |event| Message::Workspace(workspace_screens::Message::Service(event)),
                ),
            ]);
        }
        _ => {}
    }
    Task::perform(
        workspace_service::execute(state.backend.clone(), command),
        |event| Message::Workspace(workspace_screens::Message::Service(event)),
    )
}

fn workspace_for_screen(workspace: Workspace) -> workspace_screens::Workspace {
    workspace_screens::Workspace {
        id: workspace.id,
        name: workspace.name,
        chain_id: workspace.chain_id,
        pubkey: workspace.pubkey,
        member: workspace.member,
    }
}

#[cfg(feature = "cef-browser")]
fn hide_browser(state: &mut Shell) {
    if let Some(browser) = &mut state.browser {
        let _ = browser.set_visible(false);
    }
}

#[cfg(not(feature = "cef-browser"))]
fn hide_browser(_state: &mut Shell) {}

#[cfg(feature = "cef-browser")]
fn reset_browser_gateway(state: &mut Shell) {
    state.browser_gateway_generation = state.browser_gateway_generation.wrapping_add(1);
    state.browser_gateway_base = None;
    state.browser_gateway_loading = false;
    state.browser_chrome = browser_chrome::State::default();
    state.browser_permission = None;
    if let Some(browser) = &mut state.browser
        && let Err(error) = browser.reset_workspace()
    {
        state.browser_error = Some(error);
    }
}

#[cfg(feature = "cef-browser")]
fn browser_gateway_workspace(state: &Shell) -> Option<String> {
    state
        .active_workspace
        .as_ref()
        .map(|workspace| workspace.id.clone())
}

#[cfg(not(feature = "cef-browser"))]
fn reset_browser_gateway(_state: &mut Shell) {}

const fn user_screen(screen: Screen) -> Option<user_screens::Screen> {
    match screen {
        Screen::Home => Some(user_screens::Screen::Home),
        Screen::Chat => Some(user_screens::Screen::Chat),
        Screen::Pages => Some(user_screens::Screen::Pages),
        Screen::Files => Some(user_screens::Screen::Files),
        _ => None,
    }
}

const fn operator_screen(screen: Screen) -> Option<operator_screens::Screen> {
    match screen {
        Screen::Node => Some(operator_screens::Screen::Node),
        Screen::Gateway => Some(operator_screens::Screen::Gateway),
        Screen::Modules => Some(operator_screens::Screen::Modules),
        Screen::Sandbox => Some(operator_screens::Screen::Sandbox),
        Screen::Metrics => Some(operator_screens::Screen::Metrics),
        _ => None,
    }
}

fn view(state: &Shell, id: window::Id) -> Element<'_, Message> {
    match state.desktop.kind(id) {
        desktop::Kind::Main => main_view(state),
        desktop::Kind::Huddle => huddle_view(state),
        desktop::Kind::Tray => tray_view(state),
    }
}

fn main_view(state: &Shell) -> Element<'_, Message> {
    let palette = theme::palette(state.mode);
    let body: Element<'_, Message> = if state.onboarding.is_ready() {
        let frame = app_frame(state);
        if state.workspace_overlay {
            stack![
                frame,
                workspace_screens::view(&state.workspace, state.mode).map(Message::Workspace)
            ]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        } else {
            frame
        }
    } else {
        onboarding::view(&state.onboarding, state.mode).map(Message::Onboarding)
    };
    let mut content = column![titlebar(state)].spacing(0);
    if let Some(error) = &state.backend_error {
        content = content.push(
            container(text(format!("Desktop backend unavailable: {error}")).size(11))
                .padding([7, 14])
                .width(Length::Fill)
                .style(move |_| {
                    bordered(palette.danger_soft, palette.danger_border, 0.0).color(palette.danger)
                }),
        );
    }
    content = content.push(body);
    let base: Element<'_, Message> = container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style::default().background(palette.paper))
        .into();
    #[cfg(feature = "cef-browser")]
    let layered: Element<'_, Message> = if let Some(prompt) = &state.browser_permission
        && !state.search.open
    {
        stack![base, permission_prompt_view(prompt, state.mode)]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else {
        base
    };
    #[cfg(not(feature = "cef-browser"))]
    let layered: Element<'_, Message> = base;
    let layered = if state.notifications.open && !state.search.open {
        stack![layered, notifications_overlay(state)]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else {
        layered
    };
    let layered: Element<'_, Message> = if state.search.open {
        stack![
            layered,
            search::view(&state.search, state.mode).map(Message::Search)
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    } else {
        layered
    };
    if state.huddle_expanded && state.huddle.is_some() && state.desktop.huddle.is_none() {
        return huddle_stage_view(state);
    }
    if state.huddle.is_some()
        && state.desktop.huddle.is_none()
        && !state.workspace_overlay
        && !state.search.open
    {
        let dock = container(huddle_dock_view(state))
            .width(320)
            .max_height(520)
            .align_x(iced::alignment::Horizontal::Left)
            .align_y(iced::alignment::Vertical::Bottom)
            .padding(iced::Padding {
                top: 0.0,
                right: 0.0,
                bottom: 12.0,
                left: NETWORK_RAIL_WIDTH + MODULE_RAIL_WIDTH + 12.0,
            });
        stack![layered, dock]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else {
        layered
    }
}

fn huddle_dock_view(state: &Shell) -> Element<'_, Message> {
    let p = theme::palette(state.mode);
    let huddle = state.huddle.as_ref().expect("dock requires a huddle");
    let (status, status_color) = huddle_status(huddle, p);
    let count = huddle_member_count(state, &huddle.channel);
    let header = row![
        container(Space::new())
            .width(8)
            .height(8)
            .style(move |_| rounded(status_color, 4.0)),
        column![
            text(format!("Huddle · #{}", huddle.channel))
                .size(12.5)
                .font(theme::SANS_SEMIBOLD),
            text(format!("{status} · {count} in call"))
                .size(10)
                .color(p.muted),
        ]
        .spacing(1)
        .width(Length::Fill),
        button(text("Expand").size(10))
            .on_press(Message::HuddleExpand)
            .padding([4, 7]),
        button(text("Pop out").size(10))
            .on_press(Message::OpenHuddle)
            .padding([4, 7]),
    ]
    .spacing(7)
    .align_y(Alignment::Center);

    let mut body = column![header].spacing(8);
    if let Some(error) = &huddle.error {
        body = body.push(
            container(text(error).size(10).color(p.danger))
                .width(Length::Fill)
                .padding([6, 8])
                .style(move |_| bordered(p.danger_soft, p.danger_border, 5.0)),
        );
    }
    body = body.push(huddle_compact_body(state, p));
    if huddle.devices_open {
        body = body.push(huddle_devices_view(huddle, p));
    }
    body = body.push(huddle_controls_view(state, false, p));
    container(body)
        .padding(10)
        .width(320)
        .style(move |_| {
            bordered(p.paper, p.border_strong, 9.0).shadow(iced::Shadow {
                color: Color::from_rgba8(0, 0, 0, 0.14),
                offset: iced::Vector::new(0.0, 5.0),
                blur_radius: 16.0,
            })
        })
        .into()
}

fn huddle_stage_view(state: &Shell) -> Element<'_, Message> {
    let p = theme::palette(state.mode);
    let huddle = state.huddle.as_ref().expect("stage requires a huddle");
    let (status, status_color) = huddle_status(huddle, p);
    let count = huddle_member_count(state, &huddle.channel);
    let header = row![
        container(Space::new())
            .width(9)
            .height(9)
            .style(move |_| rounded(status_color, 4.5)),
        text(format!("#{}", huddle.channel))
            .size(14)
            .font(theme::SANS_SEMIBOLD),
        text(format!("{status} · {count} in call"))
            .size(11)
            .color(p.muted),
        Space::new().width(Length::Fill),
        button(
            text(if state.huddle_spotlight {
                "Gallery"
            } else {
                "Spotlight"
            })
            .size(11)
        )
        .on_press(Message::HuddleToggleLayout)
        .padding([7, 11]),
        button(text("Pop out").size(11))
            .on_press(Message::OpenHuddle)
            .padding([7, 11]),
        button(text("Collapse").size(11))
            .on_press(Message::HuddleCollapse)
            .padding([7, 11]),
    ]
    .spacing(9)
    .align_y(Alignment::Center);
    let mut notices = column![].spacing(6);
    if let Some(error) = &huddle.error {
        notices = notices.push(text(error).size(10.5).color(p.danger));
    }
    if huddle.muted && huddle_speaking(huddle) {
        notices = notices.push(
            text("Your mic is muted, but it is picking you up.")
                .size(10.5)
                .color(p.danger),
        );
    }
    let center = if huddle_member_count(state, &huddle.channel) <= 1 {
        huddle_self_check(state, p)
    } else {
        huddle_gallery(state, p, state.huddle_spotlight)
    };
    let controls = column![
        if huddle.devices_open {
            huddle_devices_view(huddle, p)
        } else {
            Space::new().height(0).into()
        },
        huddle_controls_view(state, true, p),
    ]
    .spacing(8)
    .align_x(Alignment::Center);
    container(
        column![header, notices, center, controls]
            .spacing(10)
            .height(Length::Fill),
    )
    .padding([10, 14])
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| container::Style::default().background(p.paper).color(p.ink))
    .into()
}

fn huddle_status(huddle: &HuddleRuntime, p: &theme::Palette) -> (&'static str, Color) {
    match huddle.status {
        HuddleStatus::Live => ("Live", p.green),
        HuddleStatus::Connecting => ("Connecting", p.amber),
        HuddleStatus::Reconnecting => ("Reconnecting", p.amber),
        HuddleStatus::Unavailable => ("Unavailable", p.danger),
    }
}

fn huddle_channel<'a>(state: &'a Shell, channel: &str) -> Option<&'a user_screens::Channel> {
    let user_screens::Resource::Ready(chat) = &state.user_screens.chat.data else {
        return None;
    };
    chat.channels
        .iter()
        .find(|candidate| candidate.id == channel || candidate.name == channel)
}

fn huddle_member_count(state: &Shell, channel: &str) -> usize {
    huddle_channel(state, channel).map_or(1, |channel| channel.huddle.len().max(1))
}

fn huddle_compact_body<'a>(state: &'a Shell, p: &'a theme::Palette) -> Element<'a, Message> {
    let huddle = state
        .huddle
        .as_ref()
        .expect("compact huddle requires runtime");
    let participants = huddle_participants(state, huddle);
    if participants.is_empty() {
        return container(
            column![
                text("Huddle is ready").size(11).font(theme::SANS_SEMIBOLD),
                text("Waiting for participants…").size(10).color(p.muted),
            ]
            .spacing(3),
        )
        .width(Length::Fill)
        .padding([8, 9])
        .style(move |_| bordered(p.canvas, p.border, 6.0))
        .into();
    }
    let overflow = participants.len().saturating_sub(4);
    let mut tiles = participants
        .into_iter()
        .take(4)
        .map(|participant| huddle_participant_tile(participant, p, 84.0));
    let mut grid = column![].spacing(6);
    loop {
        let Some(first) = tiles.next() else {
            break;
        };
        let mut line = row![first].spacing(6).width(Length::Fill);
        line = if let Some(second) = tiles.next() {
            line.push(second)
        } else {
            line.push(Space::new().width(Length::Fill))
        };
        grid = grid.push(line);
    }
    if overflow > 0 {
        grid = grid.push(
            text(format!("+{overflow} more not shown"))
                .size(9)
                .color(p.muted),
        );
    }
    grid.into()
}
fn huddle_devices_view<'a>(
    huddle: &'a HuddleRuntime,
    p: &'a theme::Palette,
) -> Element<'a, Message> {
    let microphones = device_labels(&huddle.devices.microphones);
    let cameras = device_labels(&huddle.devices.cameras);
    let speakers = device_labels(&huddle.devices.speakers);
    let mut devices = column![
        row![
            text("Devices")
                .size(11)
                .font(theme::SANS_SEMIBOLD)
                .width(Length::Fill),
            button(text("Close").size(9.5))
                .on_press(Message::HuddleToggleDevices)
                .padding([3, 6]),
        ]
        .align_y(Alignment::Center),
        text("Microphone").size(9.5).color(p.muted),
        pick_list(
            microphones,
            selected_device(&huddle.devices.microphones, huddle.devices.microphone),
            Message::HuddleMicrophone,
        )
        .text_size(10.5)
        .width(Length::Fill),
        text("Camera").size(9.5).color(p.muted),
        pick_list(
            cameras,
            selected_device(&huddle.devices.cameras, huddle.devices.camera),
            Message::HuddleCameraDevice,
        )
        .text_size(10.5)
        .width(Length::Fill),
        text("Speaker").size(9.5).color(p.muted),
        pick_list(
            speakers,
            selected_device(&huddle.devices.speakers, huddle.devices.speaker),
            Message::HuddleSpeaker,
        )
        .text_size(10.5)
        .width(Length::Fill),
    ]
    .spacing(5);
    if !huddle.devices.screen_sources.is_empty() {
        let screen_sources = huddle
            .devices
            .screen_sources
            .iter()
            .enumerate()
            .map(|(index, label)| format!("{} · {label}", index + 1))
            .collect::<Vec<_>>();
        let selected = huddle
            .devices
            .screen_source
            .and_then(|index| screen_sources.get(index).cloned());
        devices = devices.push(
            container(
                column![
                    text("Choose what to share")
                        .size(10)
                        .font(theme::SANS_SEMIBOLD),
                    text("Select a display or window to start sharing.")
                        .size(9.5)
                        .color(p.muted),
                    pick_list(screen_sources, selected, Message::HuddleScreenSource,)
                        .placeholder("Select a screen source")
                        .text_size(10.5)
                        .width(Length::Fill),
                ]
                .spacing(4),
            )
            .padding([5, 0]),
        );
    }
    container(devices)
        .padding(8)
        .width(Length::Fill)
        .style(move |_| bordered(p.canvas, p.border, 6.0))
        .into()
}

fn huddle_controls_view<'a>(
    state: &'a Shell,
    comfortable: bool,
    p: &'a theme::Palette,
) -> Element<'a, Message> {
    let huddle = state.huddle.as_ref().expect("controls require a huddle");
    let live = huddle.status == HuddleStatus::Live;
    let video_allowed = live && huddle_member_count(state, &huddle.channel) <= 8;
    let padding = if comfortable { [7, 12] } else { [5, 7] };
    let mut controls = row![]
        .spacing(if comfortable { 8 } else { 4 })
        .align_y(Alignment::Center);
    controls = controls.push(huddle_control_button(
        if huddle.muted {
            "Unmute"
        } else if comfortable {
            "Mute"
        } else {
            "Mic"
        },
        live.then_some(Message::HuddleMute),
        huddle.muted,
        false,
        padding,
        p,
    ));
    controls = controls.push(huddle_control_button(
        if huddle.camera_on {
            "Camera off"
        } else if comfortable {
            "Camera"
        } else {
            "Cam"
        },
        video_allowed.then_some(Message::HuddleCamera),
        huddle.camera_on,
        false,
        padding,
        p,
    ));
    controls = controls.push(huddle_control_button(
        if huddle.sharing {
            "Stop share"
        } else {
            "Share"
        },
        video_allowed.then_some(Message::HuddleShare),
        huddle.sharing,
        false,
        padding,
        p,
    ));
    controls = controls.push(huddle_control_button(
        if comfortable { "Devices" } else { "Dev" },
        Some(Message::HuddleToggleDevices),
        huddle.devices_open,
        false,
        padding,
        p,
    ));
    if huddle.status == HuddleStatus::Unavailable {
        controls = controls.push(huddle_control_button(
            "Retry",
            Some(Message::HuddleRetry),
            false,
            false,
            padding,
            p,
        ));
    }
    controls = controls.push(huddle_control_button(
        "Leave",
        Some(Message::HuddleLeave),
        false,
        true,
        padding,
        p,
    ));
    controls.into()
}

fn huddle_control_button<'a>(
    label: &'a str,
    message: Option<Message>,
    active: bool,
    danger: bool,
    padding: [u16; 2],
    p: &'a theme::Palette,
) -> Element<'a, Message> {
    button(text(label).size(10.5))
        .on_press_maybe(message)
        .padding(padding)
        .style(move |_, status| button::Style {
            background: Some(Background::Color(if danger {
                p.danger_soft
            } else if active {
                p.filled
            } else if matches!(status, button::Status::Hovered) {
                p.hover
            } else {
                p.paper
            })),
            text_color: if danger {
                p.danger
            } else if active {
                p.on_filled
            } else {
                p.ink
            },
            border: Border {
                color: if danger { p.danger_border } else { p.border },
                width: 1.0,
                radius: 6.0.into(),
            },
            ..button::Style::default()
        })
        .into()
}

fn huddle_self_check<'a>(state: &'a Shell, p: &'a theme::Palette) -> Element<'a, Message> {
    let huddle = state.huddle.as_ref().expect("self-check requires a huddle");
    let speaking = huddle_speaking(huddle);
    let preview: Element<'a, Message> = if let Some(handle) = &huddle.local_frame {
        huddle_video_tile_sized(
            handle,
            "You",
            p,
            300.0,
            speaking && !huddle.muted,
            huddle.sharing,
        )
    } else {
        container(
            column![
                text("Camera is off").size(14).font(theme::SANS_SEMIBOLD),
                button(text("Turn on camera").size(11))
                    .on_press_maybe(
                        (huddle.status == HuddleStatus::Live).then_some(Message::HuddleCamera)
                    )
                    .padding([7, 11]),
            ]
            .spacing(9)
            .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .height(300)
        .center_x(Length::Fill)
        .center_y(300)
        .style(move |_| bordered(p.canvas, p.border, 8.0))
        .into()
    };
    container(
        column![
            text("You're the only one here")
                .size(16)
                .font(theme::SANS_SEMIBOLD),
            text("Check your camera and microphone while others join.")
                .size(11)
                .color(p.muted),
            preview,
            row![
                text(if huddle.muted {
                    "Mic muted"
                } else {
                    "Microphone"
                })
                .size(10)
                .width(82),
                progress_bar(0.0..=100.0, f32::from(huddle.level)),
                text(format!("{}%", huddle.level)).size(9.5).color(p.muted),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(8)
        .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .max_width(720)
    .center_x(Length::Fill)
    .into()
}

struct HuddleParticipantView<'a> {
    label: &'a str,
    frame: Option<&'a image::Handle>,
    muted: bool,
    sharing: bool,
    speaking: bool,
    stale: bool,
}

fn huddle_participants<'a>(
    state: &'a Shell,
    huddle: &'a HuddleRuntime,
) -> Vec<HuddleParticipantView<'a>> {
    let self_key = match &state.user_screens.chat.data {
        user_screens::Resource::Ready(chat) => chat.self_key.as_deref(),
        _ => None,
    };
    let Some(channel) = huddle_channel(state, &huddle.channel) else {
        return Vec::new();
    };
    channel
        .huddle
        .iter()
        .map(|member| {
            let is_self = self_key.is_some_and(|key| key == member.user || key == member.node);
            let peer = (!is_self).then(|| huddle.peers.get(&member.node)).flatten();
            let sharing = if is_self {
                huddle.sharing
            } else {
                peer.is_some_and(|peer| peer.sharing)
            };
            let video_on = if is_self {
                huddle.camera_on || huddle.sharing
            } else {
                peer.is_some_and(|peer| peer.camera_on || peer.sharing)
            };
            let frame = if !video_on {
                None
            } else if is_self {
                huddle.local_frame.as_ref()
            } else {
                huddle.peer_frames.get(&member.node)
            };
            HuddleParticipantView {
                label: if is_self {
                    "You"
                } else {
                    huddle_member_label(state, &member.user)
                },
                frame,
                muted: if is_self {
                    huddle.muted
                } else {
                    peer.is_some_and(|peer| peer.muted)
                },
                sharing,
                speaking: is_self && huddle_speaking(huddle) && !huddle.muted,
                stale: peer.is_some_and(|peer| peer.seen.elapsed().as_secs() > 10),
            }
        })
        .collect()
}

fn huddle_member_label<'a>(state: &'a Shell, key: &'a str) -> &'a str {
    if let members_screen::Resource::Ready(data) = &state.members.data
        && let Some(member) = data
            .members
            .iter()
            .find(|member| member.key.eq_ignore_ascii_case(key))
    {
        return &member.display_name;
    }
    key.get(..8).unwrap_or(key)
}

fn huddle_participant_tile<'a>(
    participant: HuddleParticipantView<'a>,
    p: &'a theme::Palette,
    height: f32,
) -> Element<'a, Message> {
    let media: Element<'a, Message> = participant.frame.map_or_else(
        || {
            let initials = participant
                .label
                .chars()
                .take(2)
                .flat_map(char::to_uppercase)
                .collect::<String>();
            container(
                text(initials)
                    .size(if height > 300.0 { 28 } else { 15 })
                    .color(p.muted_3),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(move |_| rounded(p.panel, 7.0))
            .into()
        },
        |handle| {
            image(handle.clone())
                .width(Length::Fill)
                .height(Length::Fill)
                .content_fit(if participant.sharing {
                    iced::ContentFit::Contain
                } else {
                    iced::ContentFit::Cover
                })
                .into()
        },
    );
    let mut name = row![text(participant.label).size(10).color(p.on_filled)]
        .spacing(5)
        .align_y(Alignment::Center);
    if participant.sharing {
        name = name.push(text("screen").size(8.5).color(p.on_filled));
    }
    if participant.muted {
        name = name.push(text("muted").size(8.5).color(p.on_filled));
    }
    let name = container(name)
        .padding([4, 7])
        .style(move |_| rounded(Color::from_rgba8(0, 0, 0, 0.68), 5.0));
    let top: Element<'a, Message> = if participant.stale {
        container(text("no signal").size(8.5).color(Color::WHITE))
            .padding([3, 6])
            .style(move |_| rounded(p.danger, 5.0))
            .into()
    } else {
        Space::new().height(0).into()
    };
    let overlay = column![
        row![Space::new().width(Length::Fill), top],
        Space::new().height(Length::Fill),
        row![name, Space::new().width(Length::Fill)],
    ]
    .padding(6);
    container(stack![media, overlay])
        .height(height)
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(p.panel)),
            border: Border {
                color: if participant.speaking {
                    p.green
                } else {
                    p.border
                },
                width: if participant.speaking { 2.0 } else { 1.0 },
                radius: 7.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

fn huddle_gallery<'a>(
    state: &'a Shell,
    p: &'a theme::Palette,
    spotlight: bool,
) -> Element<'a, Message> {
    let huddle = state.huddle.as_ref().expect("gallery requires a huddle");
    let mut participants = huddle_participants(state, huddle);
    if participants.is_empty() {
        return huddle_empty_stage(p);
    }
    if spotlight {
        let selected = participants
            .iter()
            .position(|participant| participant.speaking)
            .unwrap_or(0);
        let selected = participants.remove(selected);
        let has_filmstrip = !participants.is_empty();
        let mut filmstrip = row![].spacing(8);
        for participant in participants {
            filmstrip = filmstrip.push(
                container(huddle_participant_tile(participant, p, 96.0))
                    .width(150)
                    .height(96),
            );
        }
        let mut stage = column![huddle_participant_tile(selected, p, 390.0)]
            .spacing(9)
            .height(Length::Fill);
        if has_filmstrip {
            stage = stage.push(
                scrollable(filmstrip)
                    .direction(iced::widget::scrollable::Direction::Horizontal(
                        iced::widget::scrollable::Scrollbar::new(),
                    ))
                    .height(100),
            );
        }
        return container(stage)
            .max_width(920)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .into();
    }

    let columns = match participants.len() {
        0 | 1 => 1,
        2..=4 => 2,
        _ => 3,
    };
    let mut tiles = participants
        .into_iter()
        .map(|participant| huddle_participant_tile(participant, p, 220.0));
    let mut grid = column![].spacing(8);
    loop {
        let Some(first) = tiles.next() else {
            break;
        };
        let mut line = row![first].spacing(8).width(Length::Fill);
        for _ in 1..columns {
            line = if let Some(tile) = tiles.next() {
                line.push(tile)
            } else {
                line.push(Space::new().width(Length::Fill))
            };
        }
        grid = grid.push(line);
    }
    container(scrollable(grid).height(Length::Fill))
        .max_width(980)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .into()
}
fn huddle_empty_stage(p: &theme::Palette) -> Element<'_, Message> {
    container(
        column![
            text("Waiting for video")
                .size(15)
                .font(theme::SANS_SEMIBOLD),
            text("Participant video and screen shares appear here.")
                .size(10.5)
                .color(p.muted),
        ]
        .spacing(6)
        .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .style(move |_| bordered(p.canvas, p.border, 8.0))
    .into()
}

fn huddle_view(state: &Shell) -> Element<'_, Message> {
    let p = theme::palette(state.mode);
    let Some(huddle) = &state.huddle else {
        return container(text("Not in a huddle").size(12).color(p.muted))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(move |_| {
                container::Style::default()
                    .background(p.canvas)
                    .color(p.ink)
            })
            .into();
    };
    let (status, status_color) = huddle_status(huddle, p);
    let count = huddle_member_count(state, &huddle.channel);
    let header = row![
        container(Space::new())
            .width(8)
            .height(8)
            .style(move |_| rounded(status_color, 4.0)),
        column![
            text(format!("Huddle · #{}", huddle.channel))
                .size(12.5)
                .font(theme::SANS_SEMIBOLD),
            text(format!("{status} · {count} in call"))
                .size(9.5)
                .color(p.muted),
        ]
        .spacing(1)
        .width(Length::Fill),
        button(text("Chat").size(10))
            .on_press(Message::OpenMain(Some(Screen::Chat)))
            .padding([4, 7]),
        button(text("Pop in").size(10))
            .on_press(Message::CloseHuddle)
            .padding([4, 7]),
    ]
    .spacing(7)
    .align_y(Alignment::Center);
    let mut content = column![header].spacing(7);
    if let Some(error) = &huddle.error {
        content = content.push(text(error).size(9.5).color(p.danger));
    }
    content = content.push(huddle_compact_body(state, p));
    content = content.push(
        row![
            text(if huddle.muted {
                "Mic muted"
            } else {
                "Microphone"
            })
            .size(9.5)
            .width(75),
            progress_bar(0.0..=100.0, f32::from(huddle.level)),
        ]
        .spacing(7)
        .align_y(Alignment::Center),
    );
    if huddle.devices_open {
        content = content.push(huddle_devices_view(huddle, p));
    }
    content = content.push(huddle_controls_view(state, false, p));
    container(scrollable(content).height(Length::Fill))
        .padding(10)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| {
            container::Style::default()
                .background(p.canvas)
                .color(p.ink)
        })
        .into()
}

fn huddle_video_tile_sized<'a>(
    handle: &'a image::Handle,
    label: &'a str,
    p: &'a theme::Palette,
    height: f32,
    speaking: bool,
    contain: bool,
) -> Element<'a, Message> {
    container(stack![
        image(handle.clone())
            .width(Length::Fill)
            .height(Length::Fill)
            .content_fit(if contain {
                iced::ContentFit::Contain
            } else {
                iced::ContentFit::Cover
            }),
        container(text(label).size(10).color(p.on_filled))
            .padding([4, 7])
            .style(move |_| rounded(Color::from_rgba8(0, 0, 0, 0.58), 5.0)),
    ])
    .height(height)
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(Background::Color(p.panel)),
        border: Border {
            color: if speaking { p.green } else { p.border },
            width: if speaking { 2.0 } else { 1.0 },
            radius: 7.0.into(),
        },
        ..container::Style::default()
    })
    .into()
}

fn tray_view(state: &Shell) -> Element<'_, Message> {
    let canvas = Color::from_rgba8(20, 20, 23, 0.92);
    let panel = Color::from_rgba8(0, 0, 0, 0.18);
    let hairline = Color::from_rgba8(255, 255, 255, 0.12);
    let text_color = Color::from_rgba8(255, 255, 255, 0.94);
    let dim = Color::from_rgba8(255, 255, 255, 0.55);
    let connected = state.node_client.is_some();
    let workspace = state
        .active_workspace
        .as_ref()
        .map(|workspace| workspace.name.as_str())
        .unwrap_or("No network");
    let rail = column![
        tray_nav("Node", Icon::Node, Screen::Node, text_color, dim),
        tray_nav("Chat", Icon::Chat, Screen::Chat, text_color, dim),
        tray_nav("Pages", Icon::Pages, Screen::Pages, text_color, dim),
        tray_nav("Files", Icon::Files, Screen::Files, text_color, dim),
        tray_nav("Browser", Icon::Browser, Screen::Browser, text_color, dim),
        tray_nav("Forge", Icon::Forge, Screen::Forge, text_color, dim),
        tray_nav("Agents", Icon::Agent, Screen::Agents, text_color, dim),
        tray_nav("Members", Icon::Members, Screen::Members, text_color, dim),
        Space::new().height(Length::Fill),
        tray_nav(
            "Settings",
            Icon::Settings,
            Screen::Settings,
            text_color,
            dim
        ),
        button(
            row![
                icons::view(Icon::Close, 15.0, dim),
                text("Quit").size(11.5).color(text_color)
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .padding([7, 10])
        .style(move |_, status| tray_button_style(status, text_color))
        .on_press(Message::Quit),
    ]
    .spacing(2);
    let detail = column![
        text("Node").size(15).font(theme::SANS_SEMIBOLD),
        tray_field("Network", workspace, text_color, dim),
        tray_field(
            "Status",
            if connected { "Synced" } else { "Stopped" },
            text_color,
            dim,
        ),
        tray_field(
            "Role",
            state
                .active_workspace
                .as_ref()
                .map_or("—", |workspace| if workspace.founder {
                    "genesis · validator"
                } else if workspace.member {
                    "member · validator"
                } else {
                    "guest"
                }),
            text_color,
            dim,
        ),
        Space::new().height(8),
        text("SOFTWARE").size(10).color(dim),
        tray_field("Version", env!("CARGO_PKG_VERSION"), text_color, dim),
        Space::new().height(Length::Fill),
        button(text("Open in console").size(11.5))
            .width(Length::Fill)
            .padding([7, 10])
            .style(move |_, status| tray_button_style(status, text_color))
            .on_press(Message::OpenMain(Some(Screen::Node))),
    ]
    .spacing(10);
    let header = row![
        container(text("D").size(11).color(Color::WHITE))
            .center_x(24)
            .center_y(24)
            .style(move |_| rounded(theme::ACCENTS[0], 6.0)),
        column![
            text("Ducktape").size(12.5).font(theme::SANS_SEMIBOLD),
            text(if connected {
                "●  Synced"
            } else {
                "●  Stopped"
            })
            .size(10)
            .font(theme::MONO)
            .color(if connected {
                Color::from_rgb8(92, 180, 95)
            } else {
                Color::from_rgb8(207, 106, 94)
            }),
        ]
        .spacing(2),
    ]
    .spacing(9)
    .align_y(Alignment::Center);
    container(column![
        container(header)
            .padding([10, 12])
            .width(Length::Fill)
            .style(move |_| bordered(panel, hairline, 0.0)),
        row![
            container(rail)
                .width(150)
                .height(Length::Fill)
                .padding([8, 7])
                .style(move |_| bordered(panel, hairline, 0.0)),
            container(detail)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding([12, 13]),
        ]
        .height(Length::Fill),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| {
        container::Style::default()
            .background(canvas)
            .color(text_color)
    })
    .into()
}

fn tray_nav(
    label: &'static str,
    icon: Icon,
    screen: Screen,
    text_color: Color,
    dim: Color,
) -> Element<'static, Message> {
    button(
        row![
            icons::view(icon, 15.0, dim),
            text(label).size(11.5).color(text_color)
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([7, 10])
    .style(move |_, status| tray_button_style(status, text_color))
    .on_press(Message::OpenMain(Some(screen)))
    .into()
}

fn tray_button_style(status: button::Status, text_color: Color) -> button::Style {
    button::Style {
        background: matches!(status, button::Status::Hovered)
            .then(|| Background::Color(Color::from_rgba8(255, 255, 255, 0.08))),
        text_color,
        border: Border {
            radius: 5.0.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

fn tray_field(
    label: &'static str,
    value: &str,
    text_color: Color,
    dim: Color,
) -> Element<'static, Message> {
    row![
        text(label).size(10.5).color(dim).width(70),
        text(value.to_owned()).size(11).color(text_color),
    ]
    .align_y(Alignment::Center)
    .into()
}

#[cfg(feature = "cef-browser")]
fn permission_prompt_view(prompt: &PermissionPrompt, mode: Mode) -> Element<'_, Message> {
    let p = *theme::palette(mode);
    let site = prompt
        .origin
        .strip_prefix("duck://")
        .unwrap_or(prompt.origin.as_str());
    let mut permissions = column![].spacing(5);
    for permission in &prompt.permissions {
        let label = match permission.label() {
            "microphone" => "Your microphone",
            "camera" => "Your camera",
            "screen-capture" => "Your screen",
            other => other,
        };
        permissions = permissions.push(text(format!("• {label}")).size(12.5));
    }
    let card = container(
        column![
            container(text("Remote content").size(10.5).color(p.danger))
                .padding([2, 7])
                .style(move |_| bordered(p.danger_soft, p.danger_border, 4.0)),
            column![
                text(site).size(14).font(theme::SANS_SEMIBOLD),
                text(&prompt.origin)
                    .size(10.5)
                    .font(theme::MONO)
                    .color(p.muted),
            ]
            .spacing(3),
            column![text("wants to use:").size(12).color(p.muted), permissions].spacing(4),
            text(
                "This is a page published on your Ducktape network. It is not part of Ducktape and it is not asking for these on Ducktape's behalf."
            )
            .size(11.5)
            .color(p.muted),
            Space::new().height(Length::Fill),
            permission_button(
                "Allow while this page is open",
                Message::BrowserPermissionDecision {
                    id: prompt.id,
                    allow: true,
                    session: true,
                },
                true,
                p,
            ),
            row![
                permission_button(
                    "Allow once",
                    Message::BrowserPermissionDecision {
                        id: prompt.id,
                        allow: true,
                        session: false,
                    },
                    false,
                    p,
                ),
                permission_button(
                    "Don't allow",
                    Message::BrowserPermissionDecision {
                        id: prompt.id,
                        allow: false,
                        session: true,
                    },
                    false,
                    p,
                ),
            ]
            .spacing(7),
        ]
        .spacing(13),
    )
    .width(460)
    .height(360)
    .padding([18, 20])
    .style(move |_| bordered(p.paper, p.border_strong, 9.0));
    container(card)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(move |_| {
            rounded(
                Color {
                    a: 0.28,
                    ..p.filled
                },
                0.0,
            )
        })
        .into()
}

#[cfg(feature = "cef-browser")]
fn permission_button(
    label: &'static str,
    message: Message,
    primary: bool,
    p: theme::Palette,
) -> Element<'static, Message> {
    button(container(text(label).size(12)).center_x(Length::Fill))
        .width(Length::Fill)
        .padding([7, 10])
        .on_press(message)
        .style(move |_, status| button::Style {
            background: Some(Background::Color(if primary {
                if matches!(status, button::Status::Hovered) {
                    p.ink_soft
                } else {
                    p.filled
                }
            } else if matches!(status, button::Status::Hovered) {
                p.hover
            } else {
                p.paper
            })),
            text_color: if primary { p.on_filled } else { p.ink },
            border: Border {
                color: if primary { p.filled } else { p.border },
                width: 1.0,
                radius: 6.0.into(),
            },
            ..button::Style::default()
        })
        .into()
}

fn titlebar(state: &Shell) -> Element<'_, Message> {
    let p = theme::palette(state.mode);
    let (connection_label, connection_color) = titlebar_connection(state, p);
    let back = icon_button(
        Icon::ChevronLeft,
        Message::Back,
        state.history_index > 0,
        state,
    );
    let forward = icon_button(
        Icon::ChevronRight,
        Message::Forward,
        state.history_index + 1 < state.history.len(),
        state,
    );
    let identity = mouse_area(
        container(
            row![
                container(text("D").size(11).color(p.on_filled))
                    .center_x(22)
                    .center_y(22)
                    .style(move |_| rounded(p.filled, 5.0)),
                text(
                    state
                        .active_workspace
                        .as_ref()
                        .map(|workspace| workspace.name.as_str())
                        .unwrap_or_else(|| {
                            if state.node_client.is_some() {
                                "Remote node"
                            } else {
                                "Ducktape"
                            }
                        }),
                )
                .size(12),
                text(connection_label).size(9).color(connection_color),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_y(Alignment::Center),
    )
    .on_press(Message::Window(WindowAction::Drag));
    let search = button(
        row![
            icons::view(Icon::Search, 14.0, p.muted_2),
            text("Search").size(12).color(p.muted),
            Space::new().width(Length::Fill),
            text(if cfg!(target_os = "macos") {
                "⌘ K"
            } else {
                "Ctrl K"
            })
            .size(10)
            .color(p.muted_2),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([0, 12])
    .width(340)
    .height(28)
    .on_press(Message::ToggleSearch)
    .style(move |_, status| button::Style {
        background: Some(Background::Color(
            if matches!(status, button::Status::Hovered) {
                p.hover
            } else {
                p.paper
            },
        )),
        text_color: p.ink,
        border: Border {
            color: p.border,
            width: 1.0,
            radius: 8.0.into(),
        },
        ..button::Style::default()
    });
    let mut bell_content = row![icons::view(
        Icon::Bell,
        15.0,
        if state.notifications.unread > 0 {
            p.ink
        } else {
            p.muted_2
        },
    )]
    .spacing(4)
    .align_y(Alignment::Center);
    if state.notifications.unread > 0 {
        bell_content = bell_content.push(
            container(
                text(if state.notifications.unread > 99 {
                    "99+".into()
                } else {
                    state.notifications.unread.to_string()
                })
                .size(8.5)
                .font(theme::MONO)
                .color(Color::WHITE),
            )
            .padding([1, 5])
            .style(move |_| rounded(state.accent, 8.0)),
        );
    }
    let bell = button(bell_content)
        .padding([3, 5])
        .on_press(Message::ToggleNotifications)
        .style(move |_, status| button::Style {
            background: matches!(status, button::Status::Hovered)
                .then_some(Background::Color(p.hover)),
            text_color: p.ink,
            border: Border {
                radius: 5.0.into(),
                ..Border::default()
            },
            ..button::Style::default()
        });
    let mut controls = row![bell].spacing(1).align_y(Alignment::Center);
    if !cfg!(target_os = "macos") {
        controls = controls
            .push(icon_button(
                Icon::Minimize,
                Message::Window(WindowAction::Minimize),
                true,
                state,
            ))
            .push(icon_button(
                Icon::Maximize,
                Message::Window(WindowAction::Maximize),
                true,
                state,
            ))
            .push(icon_button(
                Icon::Close,
                Message::Window(WindowAction::Close),
                true,
                state,
            ));
    }
    let native_titlebar_inset = Space::new().width(if cfg!(target_os = "macos") { 68 } else { 0 });
    container(
        row![
            row![native_titlebar_inset, back, forward, identity]
                .spacing(2)
                .align_y(Alignment::Center)
                .width(Length::FillPortion(1)),
            search,
            container(controls)
                .width(Length::FillPortion(1))
                .align_x(iced::alignment::Horizontal::Right),
        ]
        .padding([0, 10])
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .height(TITLEBAR_HEIGHT)
    .style(move |_| container::Style {
        background: Some(Background::Color(p.titlebar)),
        border: Border {
            color: p.window_border,
            width: 0.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    })
    .into()
}

fn titlebar_connection(state: &Shell, p: &theme::Palette) -> (String, Color) {
    if state.node_client.is_none() {
        return ("OFFLINE".into(), p.muted_2);
    }
    let mode = if state.active_workspace.is_some() {
        "LOCAL"
    } else {
        "REMOTE"
    };
    match &state.operator.node.data {
        operator_screens::Resource::Ready(snapshot) if snapshot.connected => {
            (format!("{mode} · #{}", snapshot.height), p.green)
        }
        operator_screens::Resource::Error(_) => (format!("{mode} · RECONNECTING"), p.amber),
        _ => (mode.into(), state.accent),
    }
}

fn app_frame(state: &Shell) -> Element<'_, Message> {
    row![network_rail(state), module_rail(state), screen_view(state)]
        .spacing(0)
        .height(Length::Fill)
        .into()
}

fn workspace_initials(name: &str) -> String {
    let mut words = name
        .split_whitespace()
        .filter_map(|word| word.chars().next());
    match (words.next(), words.next()) {
        (Some(first), Some(second)) => [first, second]
            .into_iter()
            .flat_map(char::to_uppercase)
            .collect(),
        (Some(first), None) => first.to_uppercase().take(2).collect(),
        (None, _) => "?".into(),
    }
}

fn network_rail(state: &Shell) -> Element<'_, Message> {
    let p = theme::palette(state.mode);
    let item = |letter: String, active: bool, message| {
        button(container(text(letter).size(13)).center_x(34).center_y(34))
            .padding(0)
            .on_press(message)
            .style(move |_, status| rail_circle(p, state.accent, active, status))
    };
    let mut networks = column![item(
        "⌂".into(),
        state.screen() == Screen::Home,
        Message::Navigate(Screen::Home),
    )]
    .spacing(10)
    .align_x(Alignment::Center);
    for workspace in &state.workspace.workspaces {
        let active = state
            .active_workspace
            .as_ref()
            .is_some_and(|current| current.id == workspace.id);
        networks = networks.push(item(
            workspace_initials(&workspace.name),
            active,
            Message::Workspace(workspace_screens::Message::SelectWorkspace(
                workspace.id.clone(),
            )),
        ));
    }
    if state.active_workspace.is_none() && state.node_client.is_some() {
        networks = networks.push(item("R".into(), true, Message::Navigate(Screen::Home)));
    }
    networks = networks
        .push(item(
            "+".into(),
            false,
            Message::Workspace(workspace_screens::Message::Open),
        ))
        .push(Space::new().height(Length::Fill));
    container(networks.padding([10, 0]))
        .width(NETWORK_RAIL_WIDTH)
        .height(Length::Fill)
        .style(move |_| right_border(p.sidebar, p.border))
        .into()
}

fn module_rail(state: &Shell) -> Element<'_, Message> {
    let p = theme::palette(state.mode);
    let local_managed = state.active_workspace.is_some();
    let tabs = if local_managed {
        row![
            section_button("USER", Section::User, state),
            section_button("NODE", Section::Operator, state),
        ]
        .spacing(2)
    } else {
        row![section_button("USER", Section::User, state)].spacing(2)
    };
    let screens = match (state.section, local_managed) {
        (Section::User, _) | (Section::Operator, false) => &Screen::USER[..],
        (Section::Operator, true) => &Screen::OPERATOR[..],
    };
    let mut modules = column![tabs].spacing(4).align_x(Alignment::Center);
    for &screen in screens {
        modules = modules.push(module_button(screen, state));
    }
    modules = modules
        .push(Space::new().height(Length::Fill))
        .push(module_button(Screen::Settings, state))
        .push(icon_button(
            match state.mode {
                Mode::Light => Icon::Sun,
                Mode::Dark => Icon::Moon,
            },
            Message::ToggleTheme,
            true,
            state,
        ));
    container(modules.padding([8, 4]))
        .width(MODULE_RAIL_WIDTH)
        .height(Length::Fill)
        .style(move |_| right_border(p.sidebar, p.border))
        .into()
}

fn screen_view(state: &Shell) -> Element<'_, Message> {
    if let Some(screen) = user_screen(state.screen()) {
        return user_screens::view(&state.user_screens, screen, state.mode)
            .map(Message::UserScreen);
    }
    if state.screen() == Screen::Browser {
        #[cfg(feature = "cef-browser")]
        let cef_ready = state.browser.is_some();
        #[cfg(not(feature = "cef-browser"))]
        let cef_ready = false;
        return browser_chrome::view(&state.browser_chrome, state.mode, cef_ready)
            .map(Message::Browser);
    }
    if state.screen() == Screen::Forge {
        return forge_screen::view(&state.forge, state.mode).map(Message::Forge);
    }
    if state.screen() == Screen::Agents {
        return agents_screen::view(&state.agents, state.mode, state.accent).map(Message::Agents);
    }
    if state.screen() == Screen::Members {
        return members_screen::view(&state.members, state.mode).map(Message::Members);
    }
    if state.screen() == Screen::Governance {
        return governance_screen::view(&state.governance, state.mode).map(Message::Governance);
    }
    if state.screen() == Screen::Explorer {
        return explorer_screen::view(&state.explorer, state.mode).map(Message::Explorer);
    }
    if let Some(screen) = operator_screen(state.screen()) {
        return operator_screens::view(&state.operator, screen, state.mode).map(Message::Operator);
    }
    if state.screen() == Screen::Settings {
        return settings_screen::view(&state.settings).map(Message::Settings);
    }
    let p = theme::palette(state.mode);
    let screen = state.screen();
    container(
        column![
            container(text(screen.label()).size(16))
                .height(50)
                .width(Length::Fill)
                .align_y(Alignment::Center)
                .padding([0, 20])
                .style(move |_| container::Style {
                    border: Border {
                        color: p.border,
                        width: 0.0,
                        radius: 0.0.into(),
                    },
                    ..container::Style::default()
                }),
            container(
                column![
                    icons::view(screen.icon(), 30.0, p.icon_idle),
                    text(screen.label()).size(16),
                    text("Native iced surface").size(12).color(p.muted),
                ]
                .spacing(10)
                .align_x(Alignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
        ]
        .spacing(0),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| container::Style::default().background(p.paper))
    .into()
}

fn icon_button<'a>(
    icon: Icon,
    message: Message,
    enabled: bool,
    state: &Shell,
) -> Element<'a, Message> {
    let p = theme::palette(state.mode);
    button(icons::view(
        icon,
        15.0,
        if enabled { p.muted_3 } else { p.icon_idle },
    ))
    .width(32)
    .height(32)
    .padding(8)
    .on_press_maybe(enabled.then_some(message))
    .style(move |_, status| transparent_button(p, status))
    .into()
}

fn section_button<'a>(
    label: &'static str,
    section: Section,
    state: &Shell,
) -> Element<'a, Message> {
    let p = theme::palette(state.mode);
    let active = state.section == section;
    button(text(label).size(8))
        .height(28)
        .padding([0, 5])
        .on_press(Message::Section(section))
        .style(move |_, status| tab_style(p, active, status))
        .into()
}

fn module_button<'a>(screen: Screen, state: &Shell) -> Element<'a, Message> {
    let p = theme::palette(state.mode);
    let active = state.screen() == screen;
    button(
        column![
            icons::view(
                screen.icon(),
                18.0,
                if active { p.ink } else { p.icon_idle }
            ),
            text(screen.label())
                .size(9)
                .color(if active { p.ink } else { p.muted }),
        ]
        .spacing(5)
        .align_x(Alignment::Center),
    )
    .width(66)
    .height(54)
    .padding([7, 2])
    .on_press(Message::Navigate(screen))
    .style(move |_, status| tab_style(p, active, status))
    .into()
}

fn rounded(background: Color, radius: f32) -> container::Style {
    container::Style::default()
        .background(background)
        .border(Border {
            radius: radius.into(),
            ..Border::default()
        })
}

fn bordered(background: Color, border: Color, radius: f32) -> container::Style {
    container::Style::default()
        .background(background)
        .border(Border {
            color: border,
            width: 1.0,
            radius: radius.into(),
        })
}

fn right_border(background: Color, border: Color) -> container::Style {
    container::Style {
        background: Some(Background::Color(background)),
        border: Border {
            color: border,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

fn transparent_button(p: &theme::Palette, status: button::Status) -> button::Style {
    button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then_some(Background::Color(p.hover)),
        text_color: p.ink,
        border: Border {
            radius: theme::RADIUS_SM.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

fn tab_style(p: &theme::Palette, active: bool, status: button::Status) -> button::Style {
    let background = if active {
        Some(Background::Color(p.paper))
    } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
        Some(Background::Color(p.hover))
    } else {
        None
    };
    button::Style {
        background,
        text_color: if active { p.ink } else { p.muted },
        border: Border {
            radius: theme::RADIUS_SM.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

fn rail_circle(
    p: &theme::Palette,
    accent: Color,
    active: bool,
    status: button::Status,
) -> button::Style {
    let background = if active {
        accent
    } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
        p.hover
    } else {
        p.panel
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: if active { Color::WHITE } else { p.ink },
        border: Border {
            color: if active { p.paper } else { p.border },
            width: if active { 2.0 } else { 1.0 },
            radius: 17.0.into(),
        },
        ..button::Style::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key_press(
        key: iced::keyboard::Key,
        physical_key: iced::keyboard::key::Code,
        modifiers: iced::keyboard::Modifiers,
    ) -> iced::Event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            modified_key: key.clone(),
            key,
            physical_key: iced::keyboard::key::Physical::Code(physical_key),
            location: iced::keyboard::Location::Standard,
            modifiers,
            text: None,
            repeat: false,
        })
    }

    #[test]
    fn history_drops_forward_branch() {
        let mut shell = Shell::default();
        shell.navigate(Screen::Pages);
        shell.navigate(Screen::Files);
        shell.history_index -= 1;
        shell.navigate(Screen::Forge);
        assert_eq!(
            shell.history,
            vec![Screen::Chat, Screen::Pages, Screen::Forge]
        );
        assert_eq!(shell.screen(), Screen::Forge);
    }

    #[test]
    fn captured_editor_undo_is_not_promoted_to_page_undo() {
        let event = key_press(
            iced::keyboard::Key::Character("z".into()),
            iced::keyboard::key::Code::KeyZ,
            iced::keyboard::Modifiers::COMMAND,
        );
        assert!(
            global_shortcut(
                event.clone(),
                iced::event::Status::Captured,
                window::Id::unique(),
            )
            .is_none()
        );
        assert!(matches!(
            global_shortcut(event, iced::event::Status::Ignored, window::Id::unique(),),
            Some(Message::PageHistory(false))
        ));
    }

    #[test]
    fn captured_block_edit_shortcut_is_deferred_for_focus_validation() {
        let event = key_press(
            iced::keyboard::Key::Named(iced::keyboard::key::Named::Backspace),
            iced::keyboard::key::Code::Backspace,
            iced::keyboard::Modifiers::NONE,
        );
        assert!(matches!(
            global_shortcut(event, iced::event::Status::Captured, window::Id::unique(),),
            Some(Message::PageShortcut(PageShortcut::RemoveEmpty))
        ));
    }

    #[cfg(feature = "cef-browser")]
    #[test]
    fn stale_browser_gateway_result_is_ignored() {
        let mut shell = Shell::default();
        shell.browser_gateway_generation = 9;
        shell.browser_gateway_loading = true;
        drop(update(
            &mut shell,
            Message::BrowserGatewayLoaded {
                generation: 8,
                workspace_id: None,
                result: Ok("http://127.0.0.1:1/.duck/browser/".into()),
            },
        ));
        assert!(shell.browser_gateway_loading);
        assert!(shell.browser_gateway_base.is_none());
    }

    #[cfg(feature = "cef-browser")]
    #[test]
    fn workspace_browser_reset_discards_gateway_and_tab_state() {
        let mut shell = Shell {
            browser_gateway_generation: 9,
            browser_gateway_base: Some("http://127.0.0.1:49152".into()),
            browser_gateway_loading: true,
            ..Shell::default()
        };
        browser_chrome::update(&mut shell.browser_chrome, browser_chrome::Message::NewTab);

        reset_browser_gateway(&mut shell);

        assert_eq!(shell.browser_gateway_generation, 10);
        assert!(shell.browser_gateway_base.is_none());
        assert!(!shell.browser_gateway_loading);
        assert_eq!(shell.browser_chrome, browser_chrome::State::default());
    }

    #[cfg(feature = "cef-browser")]
    #[test]
    fn invalid_browser_gateway_is_not_persisted_before_cef_exists() {
        let mut shell = Shell {
            browser_gateway_loading: true,
            ..Shell::default()
        };

        drop(update(
            &mut shell,
            Message::BrowserGatewayLoaded {
                generation: 0,
                workspace_id: None,
                result: Ok("http://example.com:49152".into()),
            },
        ));

        assert!(!shell.browser_gateway_loading);
        assert!(shell.browser_gateway_base.is_none());
        assert!(shell.browser_chrome.error.is_some());
    }

    #[test]
    fn workspace_labels_are_derived_from_workspace_names() {
        assert_eq!(workspace_initials("Duck Tape"), "DT");
        assert_eq!(workspace_initials("forge"), "F");
        assert_eq!(workspace_initials(""), "?");
    }

    #[test]
    fn titlebar_does_not_label_remote_or_disconnected_sessions_local() {
        let mut shell = Shell::default();
        let palette = theme::palette(shell.mode);
        assert_eq!(titlebar_connection(&shell, palette).0, "OFFLINE");
        shell.node_client = Some(NodeClient::new("https://node.example").unwrap());
        assert_eq!(titlebar_connection(&shell, palette).0, "REMOTE");
    }

    #[test]
    fn agent_run_anchor_routes_to_chat_history_or_its_forge_item() {
        let mut shell = Shell::default();
        drop(update(
            &mut shell,
            Message::Agents(agents_screen::Message::OpenRunAnchor {
                channel_id: "general".into(),
                sequence: 73,
            }),
        ));
        assert_eq!(shell.screen(), Screen::Chat);
        assert_eq!(
            shell.user_screens.chat.active_channel.as_deref(),
            Some("general")
        );

        drop(update(
            &mut shell,
            Message::Agents(agents_screen::Message::OpenRunAnchor {
                channel_id: "forge:team:repo:42".into(),
                sequence: 73,
            }),
        ));
        assert_eq!(shell.screen(), Screen::Forge);
        assert_eq!(shell.forge.selected_repo.as_deref(), Some("team:repo"));
        assert_eq!(shell.forge.selected_item, Some(42));
    }

    #[test]
    fn agent_run_pull_request_uses_the_parsed_repo_and_provided_number() {
        let mut shell = Shell::default();
        drop(update(
            &mut shell,
            Message::Agents(agents_screen::Message::OpenRunPullRequest {
                channel_id: "forge:team:repo:42".into(),
                number: 91,
            }),
        ));
        assert_eq!(shell.screen(), Screen::Forge);
        assert_eq!(shell.forge.selected_repo.as_deref(), Some("team:repo"));
        assert_eq!(shell.forge.selected_item, Some(91));
    }
}
