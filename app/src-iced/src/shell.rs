use std::{collections::BTreeMap, time::Duration};

use iced::widget::{Space, button, container, mouse_area, scrollable, text};
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
use crate::browser::{BrowserRuntime, ParentWindow, PermissionPrompt};
use crate::browser_chrome;
use crate::community_service;
use crate::desktop;
use crate::forge_agents_service;
use crate::huddle_ui;
use crate::icons::{self, Icon};
use crate::mac_tray;
use crate::module_host;
#[cfg(feature = "cef-browser")]
use crate::network_content::LocalDocument;
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
use crate::screens::terminal as terminal_screen;
use crate::screens::user as user_screens;
use crate::screens::workspace as workspace_screens;
use crate::search;
use crate::terminal_service;
use crate::theme::{self, Mode};
use crate::transport::{NodeClient, ServerFrame};
use crate::view_api::{AppIntent, Route};
use crate::workspace_service;

#[cfg(all(feature = "agent", debug_assertions))]
pub(crate) mod agent_wire;
#[cfg(all(feature = "agent", debug_assertions))]
mod preset;
#[cfg(all(feature = "agent", debug_assertions, test))]
mod qa;
#[cfg(all(feature = "agent", debug_assertions, test))]
mod sim;
mod browser_session;
mod view;
#[cfg(feature = "cef-browser")]
use browser_session::bounds as browser_bounds;
use browser_session::{
    hide as hide_browser, reset_gateway as reset_browser_gateway,
    sync_visibility as sync_browser_visibility,
};
#[cfg(test)]
use view::{titlebar_connection, workspace_initials};

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
    Terminal,
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
    const OPERATOR: [Self; 6] = [
        Self::Node,
        Self::Gateway,
        Self::Modules,
        Self::Sandbox,
        Self::Terminal,
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
            Self::Terminal => "Terminal",
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
            Self::Terminal => Icon::Terminal,
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
    CloseOverlays,
    Search(search::Message),
    Onboarding(onboarding::Message),
    UserView(module_host::Message),
    UserScreen(user_screens::Message),
    UserModule(module_host::Event),
    Forge(forge_screen::Message),
    Agents(agents_screen::Message),
    Members(members_screen::Message),
    Governance(governance_screen::Message),
    Explorer(explorer_screen::Message),
    Operator(operator_screens::Message),
    Terminal(terminal_screen::Message),
    TerminalTick,
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
    ExternalUrlOpened(Result<(), String>),
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
    ExitReady,
    Huddle(huddle_ui::Message),
    HuddleOpened(window::Id),
    TrayTick,
    Tray(mac_tray::Event),
    TrayPositioned(window::Id, f32, f64, f64),
    TraySelect(Screen),
    ToggleNotifications,
    CloseNotifications,
    ToggleNotificationGroup(String),
    OpenNotification(usize),
    NativeNotificationActivated(Option<notifications::Target>),
    NotificationStream {
        origin: String,
        event: notifications::StreamEvent,
    },
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
    BrowserLocalDocumentLoaded {
        generation: u64,
        request_generation: u64,
        workspace_id: Option<String>,
        tab_index: usize,
        expected_url: String,
        result: Result<LocalDocument, String>,
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
    #[cfg(all(feature = "agent", debug_assertions))]
    AgentTick,
}

struct Shell {
    desktop: desktop::State,
    notifications: notifications::State,
    notification_matcher: notifications::Matcher,
    backend: Option<Backend>,
    active_workspace: Option<Workspace>,
    node_client: Option<NodeClient>,
    node_stream_connected: bool,
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
    terminal_screen: terminal_screen::State,
    terminal: Option<terminal_service::Handle>,
    terminal_closing: Option<terminal_service::Handle>,
    terminal_pending_start: Option<(u64, terminal_screen::SessionMode)>,
    settings: settings_screen::State,
    workspace: workspace_screens::State,
    workspace_overlay: bool,
    tray_selected: Screen,
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
    browser_local_pending: Option<browser_session::PendingLocalDocument>,
    #[cfg(feature = "cef-browser")]
    browser_local_generation: u64,
    #[cfg(feature = "cef-browser")]
    browser_permission: Option<PermissionPrompt>,
    pending_display_name: Option<String>,
    huddle: huddle_ui::State,
    page_presence: Option<PagePresenceRuntime>,
    quitting: bool,
    quit_services_ready: bool,
    quit_exit_ready: bool,
    quit_window_closing: bool,
    #[cfg(all(feature = "agent", debug_assertions))]
    agent: Option<agent_wire::Runtime>,
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
            node_stream_connected: false,
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
            terminal_screen: terminal_screen::State::default(),
            terminal: None,
            terminal_closing: None,
            terminal_pending_start: None,
            settings: settings_screen::State::default(),
            workspace: workspace_screens::State::default(),
            workspace_overlay: false,
            tray_selected: Screen::Node,
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
            browser_local_pending: None,
            #[cfg(feature = "cef-browser")]
            browser_local_generation: 0,
            #[cfg(feature = "cef-browser")]
            browser_permission: None,
            pending_display_name: None,
            huddle: huddle_ui::State::default(),
            page_presence: None,
            quitting: false,
            quit_services_ready: false,
            quit_exit_ready: false,
            quit_window_closing: false,
            #[cfg(all(feature = "agent", debug_assertions))]
            agent: None,
        }
    }
}

impl Shell {
    fn boot() -> (Self, Task<Message>) {
        // Dev-only: DUCKTAPE_PRESET=<name> boots a named fixture state (e.g.
        // ui-demo skips onboarding for chrome-level agent QA).
        #[cfg(all(feature = "agent", debug_assertions))]
        if let Some(boot) = preset::from_env() {
            return boot;
        }
        let (main, open_main) = window::open(desktop::main_settings());
        let mut state = Self::default();
        state.desktop.main = Some(main);
        #[cfg(all(feature = "agent", debug_assertions))]
        {
            state.agent = Some(agent_wire::boot());
        }
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
        if self.screen() == Screen::Browser {
            hide_browser(self);
        }
        if self.screen() == Screen::Terminal && screen != Screen::Terminal {
            self.stop_terminal();
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

    fn replace_node_client(&mut self, client: Option<NodeClient>) {
        self.node_client = client;
        self.node_stream_connected = false;
    }

    fn normalize_tray_selection(&mut self) {
        if !tray_screen_available(self.tray_selected, self.active_workspace.is_some()) {
            self.tray_selected = Screen::Node;
        }
    }

    fn stop_terminal(&mut self) {
        let _ = terminal_screen::update(&mut self.terminal_screen, terminal_screen::Message::Stop);
        self.terminal_pending_start = None;
        self.begin_terminal_close();
    }

    fn begin_terminal_close(&mut self) {
        if self.terminal_closing.is_some() {
            debug_assert!(self.terminal.is_none());
            return;
        }
        let Some(handle) = self.terminal.take() else {
            return;
        };
        handle.request_stop();
        self.terminal_closing = Some(handle);
    }
}

fn tray_screen_available(screen: Screen, local_managed: bool) -> bool {
    screen == Screen::Node
        || Screen::USER.contains(&screen)
        || local_managed && Screen::OPERATOR.contains(&screen)
}

pub fn run() -> iced::Result {
    let daemon = iced::daemon(Shell::boot, update, view::view);
    // Dev-only: named boot presets for iced_test's Emulator lane.
    #[cfg(all(feature = "agent", debug_assertions))]
    let daemon = daemon.presets(preset::all());
    let result = daemon
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
        .run();
    #[cfg(feature = "cef-browser")]
    crate::browser::shutdown_after_event_loop();
    result
}

fn update(state: &mut Shell, message: Message) -> Task<Message> {
    match message {
        Message::MainOpened(id) => {
            state.desktop.main = Some(id);
            state.window_size = desktop::MAIN_SIZE;
            #[cfg(all(feature = "agent", debug_assertions))]
            agent_wire::window_opened(state, "main", id);
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
            if let Some(screen) = screen
                && (screen != Screen::Terminal || terminal_available(state))
            {
                state.navigate(screen);
            }
            let terminal = if state.screen() == Screen::Terminal && state.terminal.is_none() {
                start_terminal(state)
            } else {
                Task::none()
            };
            if let Some(id) = state.desktop.main {
                return Task::batch([
                    window::set_mode(id, window::Mode::Windowed),
                    window::minimize(id, false),
                    window::gain_focus(id),
                    sync_browser_visibility(state),
                    terminal,
                ]);
            }
            let (id, open) = window::open(desktop::main_settings());
            state.desktop.main = Some(id);
            return Task::batch([open.map(Message::MainOpened), terminal]);
        }
        Message::Quit => return quit(state),
        #[cfg(all(feature = "agent", debug_assertions))]
        Message::AgentTick => return agent_wire::tick(state),
        Message::QuitReady => {
            state.quit_services_ready = true;
            return finish_quit(state);
        }
        Message::ExitReady => {
            if !state.quit_window_closing
                && let Some(id) = state.desktop.main
            {
                state.quit_window_closing = true;
                return window::close(id);
            }
            tracing::debug!(target: "ducktape::shell", "desktop exit action requested");
            return iced::exit();
        }
        Message::TerminalTick => return poll_terminal(state),
        Message::Terminal(message) => {
            if let Some(effect) = terminal_screen::update(&mut state.terminal_screen, message) {
                return execute_terminal(state, effect);
            }
        }
        Message::Huddle(message) => {
            let local_node = state
                .active_workspace
                .as_ref()
                .map(|workspace| workspace.pubkey.as_str());
            let action = huddle_ui::update(
                &mut state.huddle,
                message,
                &state.user_screens.chat,
                local_node,
                state.node_client.as_ref(),
            );
            return execute_huddle_action(state, action);
        }
        Message::HuddleOpened(id) => {
            state.desktop.huddle = Some(id);
            #[cfg(all(feature = "agent", debug_assertions))]
            agent_wire::window_opened(state, "huddle", id);
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
                    #[cfg(all(feature = "agent", debug_assertions))]
                    agent_wire::window_opened(state, "tray", id);
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
        Message::TraySelect(screen)
            if tray_screen_available(screen, state.active_workspace.is_some()) =>
        {
            state.tray_selected = screen;
        }
        Message::TraySelect(_) => {}
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
        Message::NotificationStream { origin, event } => {
            if state
                .node_client
                .as_ref()
                .is_none_or(|client| client.origin() != origin)
            {
                return Task::none();
            }
            match event {
                notifications::StreamEvent::Connected => {
                    state.node_stream_connected = true;
                    return governance_stream_reload(state);
                }
                notifications::StreamEvent::Disconnected => {
                    state.node_stream_connected = false;
                }
                notifications::StreamEvent::Frame(frame) => {
                    state.node_stream_connected = true;
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
            }
        }
        Message::NotificationResolved(Some(item)) => return present_notification(state, item),
        Message::NotificationResolved(None) => {}
        Message::ExternalUrlOpened(Ok(())) => {}
        Message::ExternalUrlOpened(Err(_)) => {
            tracing::warn!(
                target: "ducktape::desktop",
                reason = "external_url_open_failed",
                "could not open an external link in the system browser"
            );
        }
        Message::BackendLoaded(Ok((backend, snapshot))) => {
            state.stop_terminal();
            let close_huddle = reset_huddle(state);
            state.replace_node_client(None);
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
            state.normalize_tray_selection();
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
        Message::Back => {
            let terminal_available = terminal_available(state);
            let Some(index) = (0..state.history_index)
                .rev()
                .find(|index| state.history[*index] != Screen::Terminal || terminal_available)
            else {
                return Task::none();
            };
            let previous = state.screen();
            state.history_index = index;
            state.section = if Screen::OPERATOR.contains(&state.screen()) {
                Section::Operator
            } else {
                Section::User
            };
            let terminal = sync_terminal_navigation(state, previous);
            return Task::batch([sync_browser_visibility(state), terminal]);
        }
        Message::Forward => {
            let terminal_available = terminal_available(state);
            let Some(index) = (state.history_index + 1..state.history.len())
                .find(|index| state.history[*index] != Screen::Terminal || terminal_available)
            else {
                return Task::none();
            };
            let previous = state.screen();
            state.history_index = index;
            state.section = if Screen::OPERATOR.contains(&state.screen()) {
                Section::Operator
            } else {
                Section::User
            };
            let terminal = sync_terminal_navigation(state, previous);
            return Task::batch([sync_browser_visibility(state), terminal]);
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
            if screen == Screen::Terminal && !terminal_available(state) {
                return Task::none();
            }
            state.navigate(screen);
            if screen == Screen::Terminal {
                return Task::batch([start_terminal(state), sync_browser_visibility(state)]);
            }
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
        Message::CloseOverlays => {
            if state.search.open {
                let _ = search::update(&mut state.search, search::Message::Close);
                return sync_browser_visibility(state);
            }
            if state.notifications.open {
                state.notifications.close();
                return Task::none();
            }
            if state.workspace_overlay {
                state.workspace_overlay = false;
                return sync_browser_visibility(state);
            }
        }
        Message::ToggleSearch => {
            if state.search.open {
                let _ = search::update(&mut state.search, search::Message::Close);
                return sync_browser_visibility(state);
            }
            if !state.onboarding.is_ready() || state.workspace_overlay {
                return Task::none();
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
        Message::UserView(message) => {
            return update_user_module(state, message);
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
            if state.screen() == Screen::Terminal {
                state.navigate(Screen::Node);
            } else {
                state.stop_terminal();
            }
            let close_huddle = reset_huddle(state);
            state.notification_matcher = notifications::Matcher::default();
            state.active_workspace = snapshot.active.clone();
            state.replace_node_client(
                snapshot
                    .active
                    .as_ref()
                    .and_then(|workspace| NodeClient::local(workspace.ports.http).ok()),
            );
            state.normalize_tray_selection();
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
        Message::Browser(message) => return browser_session::update_chrome(state, message),
        #[cfg(feature = "cef-browser")]
        Message::BrowserLocalDocumentLoaded {
            generation,
            request_generation,
            workspace_id,
            tab_index,
            expected_url,
            result,
        } => {
            return browser_session::local_document_loaded(
                state,
                generation,
                request_generation,
                workspace_id,
                tab_index,
                expected_url,
                result,
            );
        }
        #[cfg(feature = "cef-browser")]
        Message::BrowserGatewayLoaded {
            generation,
            workspace_id,
            result,
        } => return browser_session::gateway_loaded(state, generation, workspace_id, result),
        #[cfg(feature = "cef-browser")]
        Message::BrowserWindowReady(id) => return browser_session::window_ready(state, id),
        #[cfg(feature = "cef-browser")]
        Message::BrowserParentReady(result) => return browser_session::parent_ready(state, result),
        #[cfg(feature = "cef-browser")]
        Message::BrowserPump => return browser_session::pump(state),
        #[cfg(feature = "cef-browser")]
        Message::BrowserPermissionDecision { id, allow, session } => {
            return browser_session::decide_permission(state, id, allow, session);
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
        Message::WindowReady(_, None) => {}
    }
    Task::none()
}

fn subscription(state: &Shell) -> Subscription<Message> {
    if state.quit_exit_ready {
        return iced::time::every(Duration::from_millis(10)).map(|_| Message::ExitReady);
    }
    let mut subscriptions = vec![
        window::events().map(|(id, event)| Message::WindowEvent(id, event)),
        iced::event::listen_with(global_shortcut),
    ];
    #[cfg(all(feature = "agent", debug_assertions))]
    subscriptions.push(
        iced::time::every(Duration::from_millis(150)).map(|_| Message::AgentTick),
    );
    if state.screen() == Screen::Metrics && !state.operator.metrics.paused {
        subscriptions.push(
            iced::time::every(Duration::from_secs(2)).map(|_| Message::MetricsTick),
        );
    }
    if user_screens::account_polling(&state.user_screens) {
        subscriptions.push(
            iced::time::every(Duration::from_millis(1_200))
                .map(|_| Message::UserScreen(user_screens::Message::AccountTick)),
        );
    }
    if state.huddle.is_active() {
        subscriptions.push(
            iced::time::every(Duration::from_millis(33))
                .map(|_| Message::Huddle(huddle_ui::Message::Tick)),
        );
    }
    if state.terminal.is_some() || state.terminal_closing.is_some() {
        subscriptions.push(
            iced::time::every(Duration::from_millis(16)).map(|_| Message::TerminalTick),
        );
    }
    if state.page_presence.is_some() {
        subscriptions.push(
            iced::time::every(Duration::from_millis(250))
                .map(|_| Message::PagePresenceTick),
        );
    }
    if let Some(client) = &state.node_client {
        let origin = client.origin();
        subscriptions.push(
            notifications::subscription(origin.clone())
                .with(origin)
                .map(notification_stream_message),
        );
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
            iced::time::every(Duration::from_secs(5)).map(|_| Message::CommunityTick),
        ),
        Screen::Governance | Screen::Explorer => subscriptions.push(
            iced::time::every(Duration::from_secs(2)).map(|_| Message::CommunityTick),
        ),
        _ => {}
    }
    #[cfg(target_os = "macos")]
    subscriptions
        .push(iced::time::every(Duration::from_millis(100)).map(|_| Message::TrayTick));
    #[cfg(feature = "cef-browser")]
    if state.browser.is_some() {
        subscriptions.push(
            iced::time::every(Duration::from_millis(8)).map(|_| Message::BrowserPump),
        );
    }
    Subscription::batch(subscriptions)
}

fn notification_stream_message((origin, event): (String, notifications::StreamEvent)) -> Message {
    Message::NotificationStream { origin, event }
}

fn sync_huddle_runtime(state: &mut Shell) -> Task<Message> {
    let local_node = state
        .active_workspace
        .as_ref()
        .map(|workspace| workspace.pubkey.as_str());
    let action = huddle_ui::sync(
        &mut state.huddle,
        &state.user_screens.chat,
        local_node,
        state.node_client.as_ref(),
    );
    execute_huddle_action(state, action)
}

fn execute_huddle_action(state: &mut Shell, action: Option<huddle_ui::Action>) -> Task<Message> {
    match action {
        Some(huddle_ui::Action::PopOut) => {
            if let Some(id) = state.desktop.huddle {
                return window::gain_focus(id);
            }
            let (id, open) = window::open(desktop::huddle_settings());
            state.desktop.huddle = Some(id);
            open.map(Message::HuddleOpened)
        }
        Some(huddle_ui::Action::PopIn | huddle_ui::Action::ClosePopout) => {
            close_huddle_window(state)
        }
        Some(huddle_ui::Action::OpenChat) => Task::done(Message::OpenMain(Some(Screen::Chat))),
        Some(huddle_ui::Action::Leave(channel)) => Task::batch([
            execute_user_screen(
                state,
                user_screens::Command::SetHuddle {
                    channel,
                    joined: false,
                },
            ),
            close_huddle_window(state),
        ]),
        Some(huddle_ui::Action::HideBrowser) => {
            hide_browser(state);
            Task::none()
        }
        Some(huddle_ui::Action::SyncBrowser) => sync_browser_visibility(state),
        None => Task::none(),
    }
}

fn reset_huddle(state: &mut Shell) -> Task<Message> {
    state.huddle.reset();
    close_huddle_window(state)
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
        .retain(|_, (_, at)| at.elapsed() < Duration::from_secs(5));
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
        iced::keyboard::Key::Named(Named::Escape) => Some(Message::CloseOverlays),
        // Arrow keys walk the search results. Harmless when the palette is
        // closed (search::update guards MoveSelection on `open`), and the
        // focused input's `on_submit` handles Enter to activate the selection.
        iced::keyboard::Key::Named(Named::ArrowDown) => {
            Some(Message::Search(search::Message::MoveSelection(1)))
        }
        iced::keyboard::Key::Named(Named::ArrowUp) => {
            Some(Message::Search(search::Message::MoveSelection(-1)))
        }
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
    if state.quitting {
        return Task::none();
    }
    tracing::debug!(target: "ducktape::shell", "desktop quit requested");
    state.quitting = true;
    let channel = state.huddle.take_channel();
    state.terminal_pending_start = None;
    let terminals = [state.terminal.take(), state.terminal_closing.take()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let _ = terminal_screen::update(&mut state.terminal_screen, terminal_screen::Message::Stop);
    #[cfg(feature = "cef-browser")]
    {
        state.browser_permission = None;
    }
    if channel.is_none() && terminals.is_empty() {
        state.quit_services_ready = true;
        return finish_quit(state);
    }
    let backend = state.backend.clone();
    let workspace = state.active_workspace.clone();
    let client = state.node_client.clone();
    Task::perform(
        async move {
            for terminal in terminals {
                terminal.shutdown().await;
            }
            if let Some(channel) = channel {
                let leave = crate::screen_service::execute(
                    backend,
                    workspace,
                    client,
                    user_screens::Command::SetHuddle {
                        channel,
                        joined: false,
                    },
                );
                let _ = tokio::time::timeout(Duration::from_secs(2), leave).await;
            }
        },
        |_| Message::QuitReady,
    )
}

fn finish_quit(state: &mut Shell) -> Task<Message> {
    if !state.quit_services_ready || state.quit_exit_ready {
        return Task::none();
    }
    if let Some(backend) = &state.backend {
        backend.shutdown();
    }
    #[cfg(feature = "cef-browser")]
    if let Some(browser) = &mut state.browser {
        match browser.finish_shutdown() {
            Ok(false) => return Task::none(),
            Ok(true) => {}
            Err(error) => tracing::warn!(
                target: "ducktape::browser",
                reason = "close_failed",
                error = %error,
                "CEF browser did not close cleanly before its parent window closed"
            ),
        }
    }
    #[cfg(feature = "cef-browser")]
    if let Some(mut browser) = state.browser.take() {
        browser.defer_shutdown_after_event_loop();
        drop(browser);
    }
    state.quit_exit_ready = true;
    tracing::debug!(target: "ducktape::shell", "desktop services are ready to exit");
    Task::none()
}

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
                    tokio::time::sleep(Duration::from_millis(550)).await;
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

fn start_terminal(state: &mut Shell) -> Task<Message> {
    if state.terminal.is_some() || state.terminal_pending_start.is_some() {
        return Task::none();
    }
    terminal_screen::update(&mut state.terminal_screen, terminal_screen::Message::Start)
        .map_or_else(Task::none, |effect| execute_terminal(state, effect))
}

fn sync_terminal_navigation(state: &mut Shell, previous: Screen) -> Task<Message> {
    match (
        previous == Screen::Terminal,
        state.screen() == Screen::Terminal,
    ) {
        (true, false) => {
            state.stop_terminal();
            Task::none()
        }
        (false, true) => start_terminal(state),
        _ => Task::none(),
    }
}

fn execute_terminal(state: &mut Shell, effect: terminal_screen::Effect) -> Task<Message> {
    match effect {
        terminal_screen::Effect::Start { generation, mode } => {
            state.terminal_pending_start = Some((generation, mode));
            state.begin_terminal_close();
            if state.terminal_closing.is_none() {
                state.terminal_pending_start = None;
                begin_terminal(state, generation, mode);
            }
        }
        terminal_screen::Effect::Stop { generation } => {
            state.terminal_pending_start = None;
            if state
                .terminal
                .as_ref()
                .is_some_and(|handle| handle.generation() == generation)
            {
                state.begin_terminal_close();
            }
        }
        terminal_screen::Effect::Input { generation, bytes } => {
            let accepted = state
                .terminal
                .as_ref()
                .filter(|handle| handle.generation() == generation)
                .is_some_and(|handle| handle.send_input(bytes));
            if !accepted {
                terminal_failed(state, generation, "Terminal input queue is unavailable.");
            }
        }
        terminal_screen::Effect::Command { generation, text } => {
            let origin = terminal_author(state);
            let accepted = state
                .terminal
                .as_ref()
                .filter(|handle| handle.generation() == generation)
                .is_some_and(|handle| handle.send_command(text, origin));
            if !accepted {
                terminal_failed(state, generation, "Terminal command queue is unavailable.");
            }
        }
        terminal_screen::Effect::Copy(value) => {
            return iced::clipboard::write(value).map(|()| Message::ClipboardWritten);
        }
        terminal_screen::Effect::ReadClipboard { generation } => {
            return iced::clipboard::read().map(move |value| {
                Message::Terminal(terminal_screen::Message::Paste {
                    generation,
                    value: value.unwrap_or_default(),
                })
            });
        }
        terminal_screen::Effect::Resize {
            generation,
            cols,
            rows,
        } => {
            let accepted = state
                .terminal
                .as_ref()
                .filter(|handle| handle.generation() == generation)
                .is_some_and(|handle| handle.resize(cols, rows));
            if !accepted {
                terminal_failed(state, generation, "Terminal resize queue is unavailable.");
            }
        }
    }
    Task::none()
}

fn begin_terminal(state: &mut Shell, generation: u64, mode: terminal_screen::SessionMode) {
    if state.screen() != Screen::Terminal
        || state.terminal_screen.generation() != generation
        || state.terminal_screen.session_mode() != mode
    {
        return;
    }
    let Some(client) = state
        .node_client
        .clone()
        .filter(|_| terminal_available(state))
    else {
        terminal_failed(
            state,
            generation,
            "Terminal sessions require a connected local node.",
        );
        return;
    };
    state.terminal = Some(terminal_service::Handle::start(client, generation, mode));
}

fn terminal_author(state: &Shell) -> String {
    match &state.user_screens.home.data {
        user_screens::Resource::Ready(data) => data
            .profile
            .as_ref()
            .map(|profile| profile.display_name.trim())
            .filter(|name| !name.is_empty())
            .unwrap_or("operator")
            .to_owned(),
        _ => "operator".into(),
    }
}

fn terminal_failed(state: &mut Shell, generation: u64, detail: &str) {
    let _ = terminal_screen::update(
        &mut state.terminal_screen,
        terminal_screen::Message::Failed {
            generation,
            detail: detail.into(),
        },
    );
    state.terminal_pending_start = None;
    state.begin_terminal_close();
}

fn poll_terminal(state: &mut Shell) -> Task<Message> {
    if state
        .terminal_closing
        .as_ref()
        .is_some_and(terminal_service::Handle::is_stopped)
    {
        state.terminal_closing.take();
        if let Some((generation, mode)) = state.terminal_pending_start.take()
            && state.screen() == Screen::Terminal
            && state.terminal_screen.generation() == generation
            && state.terminal_screen.session_mode() == mode
        {
            begin_terminal(state, generation, mode);
        }
    }
    let events = state
        .terminal
        .as_ref()
        .map(terminal_service::Handle::take_events)
        .unwrap_or_default();
    let mut tasks = Vec::new();
    for event in events {
        let message = match event {
            terminal_service::Event::Connected { generation } => {
                terminal_screen::Message::Connected { generation }
            }
            terminal_service::Event::Reconnecting { generation, detail } => {
                terminal_screen::Message::Reconnecting { generation, detail }
            }
            terminal_service::Event::Output { generation, bytes } => {
                terminal_screen::Message::Output { generation, bytes }
            }
            terminal_service::Event::CommandLogged {
                generation,
                seq,
                origin,
                text,
            } => terminal_screen::Message::CommandLogged {
                generation,
                seq,
                origin,
                text,
            },
            terminal_service::Event::Failed { generation, detail } => {
                terminal_screen::Message::Failed { generation, detail }
            }
        };
        if let Some(effect) = terminal_screen::update(&mut state.terminal_screen, message) {
            tasks.push(execute_terminal(state, effect));
        }
    }
    Task::batch(tasks)
}

fn terminal_available(state: &Shell) -> bool {
    let (Some(workspace), Some(client)) = (&state.active_workspace, &state.node_client) else {
        return false;
    };
    NodeClient::local(workspace.ports.http).is_ok_and(|managed| managed.origin() == client.origin())
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
        AppIntent::OpenExternal(address) => Task::perform(
            crate::external_url::open(address),
            Message::ExternalUrlOpened,
        ),
        AppIntent::PopOutHuddle => Task::done(Message::Huddle(huddle_ui::Message::PopOut)),
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

fn execute_agents(state: &Shell, command: agents_screen::Command) -> Task<Message> {
    if let agents_screen::Command::CopyText(value) = command {
        return iced::clipboard::write(value).map(|()| Message::ClipboardWritten);
    }
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
                    tokio::time::sleep(Duration::from_millis(millis)).await;
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
                state.navigate(Screen::Chat);
                let close_huddle = reset_huddle(state);
                state.section = Section::User;
                state.active_workspace = None;
                state.replace_node_client(NodeClient::new(&url).ok());
                state.normalize_tray_selection();
                state.workspace_overlay = false;
                reset_browser_gateway(state);
                return Task::batch([close_huddle, load_current_user_screen(state)]);
            }
        },
        workspace_screens::Command::Dismiss => {
            // Dismiss always closes: trapping the user in the network picker
            // when they have no node was an inescapable modal. The empty-state
            // screens carry the "enter a network" path back.
            state.workspace_overlay = false;
            return sync_browser_visibility(state);
        }
        workspace_screens::Command::ActivateWorkspace { .. } => {
            state.stop_terminal();
            let close_huddle = reset_huddle(state);
            state.replace_node_client(None);
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

const fn user_screen(screen: Screen) -> Option<user_screens::Screen> {
    match screen {
        Screen::Home => Some(user_screens::Screen::Home),
        Screen::Chat => Some(user_screens::Screen::Chat),
        Screen::Pages => Some(user_screens::Screen::Pages),
        Screen::Files => Some(user_screens::Screen::Files),
        _ => None,
    }
}

const fn user_view(screen: Screen) -> Option<crate::view_api::ViewId> {
    match screen {
        Screen::Home => Some(crate::view_api::ViewId::Home),
        Screen::Chat => Some(crate::view_api::ViewId::Chat),
        Screen::Pages => Some(crate::view_api::ViewId::Pages),
        Screen::Files => Some(crate::view_api::ViewId::Files),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn local_workspace() -> Workspace {
        Workspace {
            id: "local".into(),
            name: "Local".into(),
            chain_id: "local-chain".into(),
            pubkey: "local-key".into(),
            founder: true,
            member: true,
            ports: crate::backend::WorkspacePorts {
                listen: 41_000,
                http: 41_001,
                rpc: 41_002,
                wireguard: None,
                invite: None,
            },
        }
    }

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
    fn search_stays_closed_without_app_content() {
        let mut shell = Shell::default();
        drop(update(&mut shell, Message::ToggleSearch));
        assert!(!shell.search.open);

        shell.onboarding.stage = onboarding::Stage::Ready;
        shell.workspace_overlay = true;
        drop(update(&mut shell, Message::ToggleSearch));
        assert!(!shell.search.open);

        shell.workspace_overlay = false;
        drop(update(&mut shell, Message::ToggleSearch));
        assert!(shell.search.open);
    }

    #[test]
    fn escape_closes_overlays_in_z_order() {
        let mut shell = Shell::default();
        shell.search.open = true;
        shell.notifications.open = true;
        shell.workspace_overlay = true;

        drop(update(&mut shell, Message::CloseOverlays));
        assert!(!shell.search.open && shell.notifications.open && shell.workspace_overlay);
        drop(update(&mut shell, Message::CloseOverlays));
        assert!(!shell.notifications.open && shell.workspace_overlay);
        drop(update(&mut shell, Message::CloseOverlays));
        assert!(!shell.workspace_overlay);
    }

    #[test]
    fn escape_dismisses_the_network_overlay_even_without_a_node() {
        // Regression: with no node client, Dismiss re-asserted the overlay, so
        // Close/X/Escape could never leave the network picker (an inescapable
        // modal). Escape must close it regardless of node state.
        let mut shell = Shell::default();
        shell.onboarding.stage = onboarding::Stage::Ready;
        shell.workspace_overlay = true;
        assert!(shell.node_client.is_none());

        drop(update(&mut shell, Message::CloseOverlays));
        assert!(!shell.workspace_overlay, "Escape must close the picker");

        // And the Close button's Dismiss command is unconditional too.
        shell.workspace_overlay = true;
        drop(execute_workspace(
            &mut shell,
            workspace_screens::Command::Dismiss,
        ));
        assert!(!shell.workspace_overlay, "Close must dismiss the picker");
    }

    #[test]
    fn history_navigation_keeps_the_module_section_in_sync() {
        let mut shell = Shell::default();
        shell.navigate(Screen::Node);
        assert_eq!(shell.section, Section::Operator);

        drop(update(&mut shell, Message::Back));
        assert_eq!(shell.screen(), Screen::Chat);
        assert_eq!(shell.section, Section::User);

        drop(update(&mut shell, Message::Forward));
        assert_eq!(shell.screen(), Screen::Node);
        assert_eq!(shell.section, Section::Operator);
    }

    #[test]
    fn history_navigation_skips_an_unavailable_terminal() {
        let mut shell = Shell {
            history: vec![Screen::Chat, Screen::Terminal, Screen::Node],
            history_index: 2,
            section: Section::Operator,
            ..Shell::default()
        };

        drop(update(&mut shell, Message::Back));
        assert_eq!(shell.screen(), Screen::Chat);
        assert_eq!(shell.section, Section::User);

        drop(update(&mut shell, Message::Forward));
        assert_eq!(shell.screen(), Screen::Node);
        assert_eq!(shell.section, Section::Operator);
    }

    #[cfg(feature = "cef-browser")]
    #[test]
    fn browser_parent_completion_is_ignored_after_start_or_quit() {
        assert!(browser_session::parent_request_is_current(false, false));
        assert!(!browser_session::parent_request_is_current(true, false));
        assert!(!browser_session::parent_request_is_current(false, true));
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
        let mut shell = Shell {
            browser_gateway_generation: 9,
            browser_gateway_loading: true,
            ..Shell::default()
        };
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
    fn stale_network_reload_result_is_ignored() {
        let mut shell = Shell {
            node_client: Some(NodeClient::new("https://node.example").unwrap()),
            ..Shell::default()
        };
        drop(update(
            &mut shell,
            Message::Browser(browser_chrome::Message::Open),
        ));
        assert_eq!(shell.browser_local_generation, 1);
        drop(update(
            &mut shell,
            Message::Browser(browser_chrome::Message::Reload),
        ));
        assert_eq!(shell.browser_local_generation, 2);

        drop(update(
            &mut shell,
            Message::BrowserLocalDocumentLoaded {
                generation: 0,
                request_generation: 1,
                workspace_id: None,
                tab_index: 0,
                expected_url: "duck://net.duck/".into(),
                result: Ok(LocalDocument {
                    url: "duck://net.duck/".into(),
                    bytes: std::sync::Arc::from(b"old".as_slice()),
                    snapshot: "old".into(),
                    title: "net.duck".into(),
                }),
            },
        ));

        assert!(shell.browser_chrome.loading);
        assert!(shell.browser_local_pending.is_none());
        assert!(shell.browser.is_none());
        assert!(shell.browser_chrome.error.is_none());
    }

    #[cfg(feature = "cef-browser")]
    #[test]
    fn fresh_tab_network_open_starts_local_load_without_gateway() {
        let mut shell = Shell {
            node_client: Some(NodeClient::new("https://node.example").unwrap()),
            ..Shell::default()
        };
        shell.navigate(Screen::Browser);
        drop(update(
            &mut shell,
            Message::Browser(browser_chrome::Message::NewTab),
        ));
        assert_eq!(shell.browser_chrome.active_tab, 1);
        assert_eq!(shell.browser_chrome.runtime_url(), browser_chrome::IDLE_URL);

        drop(update(
            &mut shell,
            Message::Browser(browser_chrome::Message::Open),
        ));

        assert_eq!(shell.browser_chrome.active_tab, 1);
        assert_eq!(shell.browser_chrome.runtime_url(), "duck://net.duck/");
        assert!(shell.browser_chrome.loading);
        assert_eq!(shell.browser_local_generation, 1);
        assert!(!shell.browser_gateway_loading);
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
        assert_eq!(shell.browser_local_generation, 1);
        assert!(shell.browser_gateway_base.is_none());
        assert!(!shell.browser_gateway_loading);
        assert_eq!(shell.browser_chrome, browser_chrome::State::default());
    }

    #[cfg(feature = "cef-browser")]
    #[test]
    fn gateway_result_is_discarded_after_switching_to_net_duck() {
        let mut shell = Shell {
            browser_gateway_loading: true,
            ..Shell::default()
        };
        browser_chrome::update(&mut shell.browser_chrome, browser_chrome::Message::Open);

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
        assert!(shell.browser_chrome.error.is_none());
    }

    #[test]
    fn workspace_labels_are_derived_from_workspace_names() {
        assert_eq!(workspace_initials("Duck Tape"), "DT");
        assert_eq!(workspace_initials("forge"), "F");
        assert_eq!(workspace_initials(""), "?");
    }

    #[test]
    fn idle_browser_does_not_start_gateway_or_replace_its_empty_state() {
        let mut shell = Shell::default();
        shell.navigate(Screen::Browser);

        drop(sync_browser_visibility(&mut shell));

        assert!(shell.browser_chrome.is_idle());
        assert!(!shell.browser_chrome.loading);
        assert!(shell.browser_chrome.error.is_none());
        #[cfg(feature = "cef-browser")]
        assert!(!shell.browser_gateway_loading);
    }

    #[test]
    fn external_intents_do_not_enter_the_embedded_browser() {
        let mut shell = Shell {
            backend_error: Some("existing backend failure".into()),
            ..Shell::default()
        };
        let chrome = shell.browser_chrome.clone();

        drop(open_app_intent(
            &mut shell,
            AppIntent::OpenExternal("https://example.com/docs".into()),
        ));

        assert_eq!(shell.screen(), Screen::Chat);
        assert_eq!(shell.browser_chrome, chrome);
        assert_eq!(
            shell.backend_error.as_deref(),
            Some("existing backend failure")
        );

        drop(update(
            &mut shell,
            Message::ExternalUrlOpened(Err("opener unavailable".into())),
        ));
        assert_eq!(
            shell.backend_error.as_deref(),
            Some("existing backend failure")
        );
    }

    #[test]
    fn titlebar_does_not_label_remote_or_disconnected_sessions_local() {
        let mut shell = Shell::default();
        let palette = theme::palette(shell.mode);
        assert_eq!(titlebar_connection(&shell, palette).0, "OFFLINE");
        shell.replace_node_client(Some(NodeClient::new("https://node.example").unwrap()));
        assert_eq!(
            titlebar_connection(&shell, palette).0,
            "REMOTE · RECONNECTING"
        );
        shell.node_stream_connected = true;
        assert_eq!(titlebar_connection(&shell, palette).0, "REMOTE · CONNECTED");
    }

    #[test]
    fn stale_stream_events_cannot_mark_the_current_client_connected() {
        let mut shell = Shell::default();
        shell.replace_node_client(Some(NodeClient::new("https://node.example").unwrap()));

        drop(update(
            &mut shell,
            Message::NotificationStream {
                origin: "https://old-node.example/".into(),
                event: notifications::StreamEvent::Connected,
            },
        ));
        assert!(!shell.node_stream_connected);

        let origin = shell.node_client.as_ref().unwrap().origin();
        drop(update(
            &mut shell,
            Message::NotificationStream {
                origin: origin.clone(),
                event: notifications::StreamEvent::Connected,
            },
        ));
        assert!(shell.node_stream_connected);

        drop(update(
            &mut shell,
            Message::NotificationStream {
                origin,
                event: notifications::StreamEvent::Disconnected,
            },
        ));
        assert!(!shell.node_stream_connected);
        assert_eq!(
            titlebar_connection(&shell, theme::palette(shell.mode)).0,
            "REMOTE · RECONNECTING"
        );
    }

    #[test]
    fn tray_keeps_module_selection_without_opening_the_main_window() {
        let mut shell = Shell::default();
        assert!(
            Screen::USER
                .into_iter()
                .all(|screen| tray_screen_available(screen, false))
        );
        assert!(!tray_screen_available(Screen::Gateway, false));
        assert!(
            Screen::OPERATOR
                .into_iter()
                .all(|screen| tray_screen_available(screen, true))
        );

        drop(update(&mut shell, Message::TraySelect(Screen::Forge)));
        assert_eq!(shell.tray_selected, Screen::Forge);
        assert_eq!(shell.screen(), Screen::Chat);

        shell.tray_selected = Screen::Gateway;
        shell.active_workspace = None;
        shell.normalize_tray_selection();
        assert_eq!(shell.tray_selected, Screen::Node);
    }

    #[test]
    fn terminal_is_operator_only_and_ordered_between_sandbox_and_metrics() {
        assert_eq!(
            Screen::OPERATOR,
            [
                Screen::Node,
                Screen::Gateway,
                Screen::Modules,
                Screen::Sandbox,
                Screen::Terminal,
                Screen::Metrics,
            ]
        );
        assert!(!Screen::USER.contains(&Screen::Terminal));
    }

    #[test]
    fn terminal_fails_closed_without_a_local_workspace_and_stops_on_navigation() {
        let mut shell = Shell {
            active_workspace: Some(local_workspace()),
            node_client: Some(NodeClient::new("https://node.example").unwrap()),
            ..Shell::default()
        };
        drop(update(&mut shell, Message::Navigate(Screen::Terminal)));
        assert_eq!(shell.screen(), Screen::Chat);
        assert_eq!(
            shell.terminal_screen.status(),
            terminal_screen::Status::Idle
        );
        assert!(shell.terminal.is_none());

        shell.node_client = Some(NodeClient::local(41_001).unwrap());
        assert!(terminal_available(&shell));
        shell.navigate(Screen::Terminal);
        let effect =
            terminal_screen::update(&mut shell.terminal_screen, terminal_screen::Message::Start);
        assert!(matches!(
            effect,
            Some(terminal_screen::Effect::Start { .. })
        ));
        let generation = shell.terminal_screen.generation();
        assert_eq!(
            shell.terminal_screen.status(),
            terminal_screen::Status::Starting
        );

        shell.navigate(Screen::Metrics);
        assert_eq!(
            shell.terminal_screen.status(),
            terminal_screen::Status::Idle
        );
        assert_ne!(shell.terminal_screen.generation(), generation);
    }

    #[test]
    fn workspace_switch_navigates_away_from_a_stopped_terminal() {
        let mut shell = Shell::default();
        shell.navigate(Screen::Terminal);
        let effect =
            terminal_screen::update(&mut shell.terminal_screen, terminal_screen::Message::Start);
        assert!(matches!(
            effect,
            Some(terminal_screen::Effect::Start { .. })
        ));

        drop(update(
            &mut shell,
            Message::WorkspaceSnapshotLoaded(Ok(WorkspaceSnapshot {
                workspaces: vec![local_workspace()],
                active: Some(local_workspace()),
            })),
        ));

        assert_eq!(shell.screen(), Screen::Node);
        assert_eq!(
            shell.terminal_screen.status(),
            terminal_screen::Status::Idle
        );
        assert!(shell.terminal.is_none());
        assert!(terminal_available(&shell));
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
