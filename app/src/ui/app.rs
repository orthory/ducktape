#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum AppTheme {
    App,
    AppDark,
}
#[derive(Default)]
struct __IceDerivedCache {
    dark: ::std::cell::OnceCell<bool>,
    app_background: ::std::cell::OnceCell<::std::string::String>,
    app_text: ::std::cell::OnceCell<::std::string::String>,
    has_error: ::std::cell::OnceCell<bool>,
    mutation_busy: ::std::cell::OnceCell<bool>,
    hub_busy: ::std::cell::OnceCell<bool>,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum LiveKind {
    Retry,
    Tip,
    Ready,
    Chat,
    Bell,
    Plane,
    Resync,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SearchPhase {
    Idle,
    Searching,
    Done,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Appearance {
    System,
    Light,
    Dark,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SubmitVerdict {
    Admitted,
    Refused,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ComposerKind {
    Message,
    Reply,
    Edit,
    ThreadEdit,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum MessageAction {
    Toolbar,
    More,
    Reactions,
    Editing,
    Delete,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ForgePhase {
    Idle,
    Loading,
    Ready,
    Failed,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum HubStep {
    Loading,
    Password,
    Phrase,
    Confirm,
    Wallets,
    Restore,
    Networks,
    Join,
    Provisioning,
    Live,
    Account,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum WalletDoor {
    Wallets,
    Password,
    Unreached,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum WindowSummon {
    Open,
    Raise,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum TrayOpen {
    Launch,
    Console,
    Raise,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ConsoleEntry {
    Idle,
    Entering,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum RowPlate {
    Plain,
    Selected,
    Ranged,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum CopySurface {
    Nowhere,
    Timeline,
    Thread,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum CommandChord {
    Ignored,
    Quit,
    CloseWindow,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum AccountProbe {
    Found,
    Missing,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum CeremonyPhase {
    Working,
    ShowQr,
    Done,
    Failed,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum WelcomeDoor {
    Create,
    Login,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DuckKind {
    Unknown,
    Web,
    ForeignNetwork,
    Page,
    Files,
    ForgeRepo,
    ForgeItem,
    ForgeBlob,
    Channel,
    ChannelMessage,
    Run,
    Account,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ShellTab {
    Chat,
    Pages,
    Forge,
    Agents,
    Files,
    Explorer,
    Node,
    Members,
    Governance,
    Settings,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ForgeIntent {
    OpenLink,
    Copy,
    Composer,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum AgentsIntent {
    Badge,
    Register,
    OpenRun,
    OpenLink,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SettingsIntent {
    Tab,
    Reconnect,
    SwitchNetwork,
    Unlock,
    Lock,
    Rename,
    Create,
    KeyAdd,
    Join,
    KeyRemove,
    Passkey,
    PasskeyDesktop,
    CeremonyCancel,
    Wallet,
    Login,
    Copy,
    Light,
    Dark,
    Notifications,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PagesIntent {
    OpenLink,
    Copy,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ChatIntent {
    OpenHit,
    ToggleCreate,
    ChooseChannel,
    ChooseDm,
    ShowHuddle,
    LeaveHuddle,
    JoinHuddle,
    Scrolled,
    OpenLink,
    Copy,
    CopyLink,
    BeginEdit,
    CancelRun,
    OpenRun,
    Composer,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum MutationPhase {
    Idle,
    Recovering,
    BlockComment,
    Channel,
    ChannelArchive,
    ChannelMember,
    ChannelRename,
    ChannelUnarchive,
    CommentResolve,
    Huddle,
    MessageDelete,
    MessageEdit,
    Onboarding,
    Page,
    PageDelete,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum CeremonyRetirement {
    Keep,
    Welcome,
    Account,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum BellTarget {
    Unavailable,
    Message,
    Page,
    Forge,
    Repo,
    Run,
}
#[allow(dead_code)]
pub struct Ducktape {
    pub(crate) __ice_run_lane_0_generation: u64,
    pub(crate) __ice_run_lane_0_handle: ::std::option::Option<::ducktape_view_guest::task::Handle>,
    pub(crate) __ice_run_lane_1_generation: u64,
    pub(crate) __ice_run_lane_1_handle: ::std::option::Option<::ducktape_view_guest::task::Handle>,
    pub(crate) __ice_run_lane_2_generation: u64,
    pub(crate) __ice_run_lane_2_handle: ::std::option::Option<::ducktape_view_guest::task::Handle>,
    pub(crate) __ice_run_lane_3_generation: u64,
    pub(crate) __ice_run_lane_3_handle: ::std::option::Option<::ducktape_view_guest::task::Handle>,
    pub(crate) __ice_run_lane_4_generation: u64,
    pub(crate) __ice_run_lane_4_handle: ::std::option::Option<::ducktape_view_guest::task::Handle>,
    pub(crate) __ice_run_lane_5_generation: u64,
    pub(crate) __ice_run_lane_5_handle: ::std::option::Option<::ducktape_view_guest::task::Handle>,
    pub(crate) __ice_run_lane_6_generation: u64,
    pub(crate) __ice_run_lane_6_handle: ::std::option::Option<::ducktape_view_guest::task::Handle>,
    pub(crate) __ice_run_lane_7_generation: u64,
    pub(crate) __ice_run_lane_7_handle: ::std::option::Option<::ducktape_view_guest::task::Handle>,
    pub(crate) __ice_run_lane_8_generation: u64,
    pub(crate) __ice_run_lane_8_handle: ::std::option::Option<::ducktape_view_guest::task::Handle>,
    pub(crate) __ice_run_lane_9_generation: u64,
    pub(crate) __ice_run_lane_9_handle: ::std::option::Option<::ducktape_view_guest::task::Handle>,
    pub(crate) __ice_run_lane_10_generation: u64,
    pub(crate) __ice_run_lane_10_handle: ::std::option::Option<::ducktape_view_guest::task::Handle>,
    pub(crate) __ice_run_lane_11_generation: u64,
    pub(crate) __ice_run_lane_11_handle: ::std::option::Option<::ducktape_view_guest::task::Handle>,
    pub(crate) __ice_run_lane_12_generation: u64,
    pub(crate) __ice_run_lane_12_handle: ::std::option::Option<::ducktape_view_guest::task::Handle>,
    pub(crate) __ice_run_lane_13_generation: u64,
    pub(crate) __ice_run_lane_13_handle: ::std::option::Option<::ducktape_view_guest::task::Handle>,
    pub(crate) __ice_run_lane_14_generation: u64,
    pub(crate) __ice_run_lane_14_handle: ::std::option::Option<::ducktape_view_guest::task::Handle>,
    pub(crate) __ice_run_lane_15_generation: u64,
    pub(crate) __ice_run_lane_15_handle: ::std::option::Option<::ducktape_view_guest::task::Handle>,
    pub(crate) __ice_run_lane_16_generation: u64,
    pub(crate) __ice_run_lane_16_handle: ::std::option::Option<::ducktape_view_guest::task::Handle>,
    pub(crate) __ice_run_lane_17_generation: u64,
    pub(crate) __ice_run_lane_17_handle: ::std::option::Option<::ducktape_view_guest::task::Handle>,
    pub(crate) __ice_run_lane_18_generation: u64,
    pub(crate) __ice_run_lane_18_handle: ::std::option::Option<::ducktape_view_guest::task::Handle>,
    pub(crate) __ice_run_lane_19_generation: u64,
    pub(crate) __ice_run_lane_19_handle: ::std::option::Option<::ducktape_view_guest::task::Handle>,
    pub(crate) __ice_run_lane_20_generation: u64,
    pub(crate) __ice_run_lane_20_handle: ::std::option::Option<::ducktape_view_guest::task::Handle>,
    pub(crate) __ice_run_lane_21_generation: u64,
    pub(crate) __ice_run_lane_21_handle: ::std::option::Option<::ducktape_view_guest::task::Handle>,
    pub(crate) __ice_run_lane_22_generation: u64,
    pub(crate) __ice_run_lane_22_handle: ::std::option::Option<::ducktape_view_guest::task::Handle>,
    pub(crate) __ice_run_lane_23_generation: u64,
    pub(crate) __ice_run_lane_23_handle: ::std::option::Option<::ducktape_view_guest::task::Handle>,
    pub(crate) __ice_run_lane_24_generation: u64,
    pub(crate) __ice_run_lane_24_handle: ::std::option::Option<::ducktape_view_guest::task::Handle>,
    pub(crate) __ice_run_lane_25_generation: u64,
    pub(crate) __ice_run_lane_25_handle: ::std::option::Option<::ducktape_view_guest::task::Handle>,
    pub(crate) __ice_run_lane_26_generation: u64,
    pub(crate) __ice_run_lane_26_handle: ::std::option::Option<::ducktape_view_guest::task::Handle>,
    pub(crate) app_palette: AppTheme,
    pub(crate) appearance: Appearance,
    pub(crate) desktop_notifications: bool,
    pub(crate) wall_now: i64,
    pub(crate) rpc: ::std::string::String,
    pub(crate) connected_rpc: ::std::string::String,
    pub(crate) password: ::std::string::String,
    pub(crate) status: ::std::string::String,
    pub(crate) connected: bool,
    pub(crate) loading: bool,
    pub(crate) views_live_serial: i64,
    pub(crate) cmd_held: bool,
    pub(crate) shift_held: bool,
    pub(crate) focused_win: ::std::option::Option<crate::shell::WindowKey>,
    pub(crate) block_height: i64,
    pub(crate) hydration_generation: i64,
    pub(crate) connect_generation: i64,
    pub(crate) signer_key: ::std::string::String,
    pub(crate) hydration_retry_attempt: i64,
    pub(crate) mutation_phase: MutationPhase,
    pub(crate) error: ::std::string::String,
    pub(crate) startup_duck_link: ::std::string::String,
    pub(crate) channels: ::std::vec::Vec<crate::backend::ChatChannel>,
    pub(crate) rooms: ::std::vec::Vec<crate::backend::ChatSidebarRow>,
    pub(crate) chat_generation: i64,
    pub(crate) channel_reads: ::std::vec::Vec<crate::backend::ChannelRead>,
    pub(crate) unread_boundary: i64,
    pub(crate) active_channel: ::std::string::String,
    pub(crate) active_channel_name: ::std::string::String,
    pub(crate) active_channel_archived: bool,
    pub(crate) active_channel_members_only: bool,
    pub(crate) channel_members: ::std::vec::Vec<crate::backend::ChatMember>,
    pub(crate) post_refusal: ::std::string::String,
    pub(crate) chat_land_seq: i64,
    pub(crate) chat_edit_seq: i64,
    pub(crate) chat_edit_rev: i64,
    pub(crate) composer_stashed: bool,
    pub(crate) composer_roster_set: bool,
    pub(crate) composer_seeded: bool,
    pub(crate) live_agents: ::std::vec::Vec<crate::backend::LiveAgentRow>,
    pub(crate) chat_sent_serial: i64,
    pub(crate) chat_pending_sends: ::std::vec::Vec<crate::backend::PendingSend>,
    pub(crate) chat_copy_chord_serial: i64,
    pub(crate) channel_draft: ::std::string::String,
    pub(crate) channel_create_open: bool,
    pub(crate) channel_create_members_only: bool,
    pub(crate) pending_channel: ::std::string::String,
    pub(crate) chat_at_tail: bool,
    pub(crate) history_view: bool,
    pub(crate) chat_chain_id: ::std::string::String,
    pub(crate) dm_peers: ::std::vec::Vec<crate::backend::DmPeer>,
    pub(crate) dm_rows: ::std::vec::Vec<crate::backend::DmSidebarRow>,
    pub(crate) dm_peers_generation: i64,
    pub(crate) active_dm_peer: ::std::string::String,
    pub(crate) active_dm: crate::backend::DmPeer,
    pub(crate) shell_tab: ShellTab,
    pub(crate) members_answered: bool,
    pub(crate) members_rows: ::std::vec::Vec<crate::backend::MemberRow>,
    pub(crate) members_generation: i64,
    pub(crate) gov_open: i64,
    pub(crate) agents_open_run: ::std::string::String,
    pub(crate) agents_opened: i64,
    pub(crate) agents_live: bool,
    pub(crate) forge_note_pending: ::std::string::String,
    pub(crate) forge_link: ::std::string::String,
    pub(crate) forge_link_tick: i64,
    pub(crate) node_key: ::std::string::String,
    pub(crate) node_data_dir: ::std::string::String,
    pub(crate) settings_key_path: ::std::string::String,
    pub(crate) settings_key_state: ::std::string::String,
    pub(crate) settings_user_key: ::std::string::String,
    pub(crate) settings_generation: i64,
    pub(crate) account_exists: bool,
    pub(crate) account_number: ::std::string::String,
    pub(crate) account_name: ::std::string::String,
    pub(crate) account_bio: ::std::string::String,
    pub(crate) account_generation: i64,
    pub(crate) account_busy: bool,
    pub(crate) account_ticket: ::std::string::String,
    pub(crate) account_banner_dismissed: bool,
    pub(crate) account_ceremony_phase: ::std::string::String,
    pub(crate) account_ceremony_qr: ::std::string::String,
    pub(crate) account_ceremony_detail: ::std::string::String,
    pub(crate) account_ceremony_left: ::std::string::String,
    pub(crate) node_version: ::std::string::String,
    pub(crate) node_root_hash: ::std::string::String,
    pub(crate) network_chain_id: ::std::string::String,
    pub(crate) node_last_finalized: i64,
    pub(crate) node_checkpoint: i64,
    pub(crate) node_height: i64,
    pub(crate) node_phase: ::std::string::String,
    pub(crate) node_phase_since: i64,
    pub(crate) node_sync_target: i64,
    pub(crate) node_sync_applied: i64,
    pub(crate) node_sync_retries: i64,
    pub(crate) node_sync_failures: i64,
    pub(crate) node_sync_last_error: ::std::string::String,
    pub(crate) node_view_label: ::std::string::String,
    pub(crate) node_quorum_label: ::std::string::String,
    pub(crate) node_reachable_label: ::std::string::String,
    pub(crate) fs_drop_dir: ::std::string::String,
    pub(crate) fs_dropping: bool,
    pub(crate) fs_route: ::std::string::String,
    pub(crate) fs_route_serial: i64,
    pub(crate) palette_open: bool,
    pub(crate) bell_open: bool,
    pub(crate) bell_unread: i64,
    pub(crate) bell_items: ::std::vec::Vec<crate::backend::BellItem>,
    pub(crate) bell_presentations: ::std::vec::Vec<crate::backend::BellPresentation>,
    pub(crate) bell_read_through: i64,
    pub(crate) bell_clear_through: i64,
    pub(crate) bell_marking: bool,
    pub(crate) bell_error: ::std::string::String,
    pub(crate) palette_draft: ::std::string::String,
    pub(crate) palette_search_phase: SearchPhase,
    pub(crate) palette_chat_hits: ::std::vec::Vec<crate::backend::ChatSearchHit>,
    pub(crate) palette_page_hits: ::std::vec::Vec<crate::backend::PageSearchHit>,
    pub(crate) toast: ::std::string::String,
    pub(crate) toast_age: i64,
    pub(crate) page_route: ::std::string::String,
    pub(crate) page_route_serial: i64,
    pub(crate) onboarding_win: ::std::option::Option<crate::shell::WindowKey>,
    pub(crate) console_win: ::std::option::Option<crate::shell::WindowKey>,
    pub(crate) huddle_win: ::std::option::Option<crate::shell::WindowKey>,
    pub(crate) network_name: ::std::string::String,
    pub(crate) hub_step: HubStep,
    pub(crate) hub_networks: ::std::vec::Vec<crate::backend::HubNetwork>,
    pub(crate) hub_selected: ::std::string::String,
    pub(crate) hub_wallets: ::std::vec::Vec<crate::backend::WalletInfo>,
    pub(crate) hub_wallet_selected: ::std::string::String,
    pub(crate) onboarding_name: ::std::string::String,
    pub(crate) onboarding_error: ::std::string::String,
    pub(crate) console_entry: ConsoleEntry,
    pub(crate) invite_link: ::std::string::String,
    pub(crate) provision_steps: ::std::vec::Vec<crate::backend::ProvisionStep>,
    pub(crate) provision_index: i64,
    pub(crate) hub_chain_id: ::std::string::String,
    pub(crate) welcome_name_draft: ::std::string::String,
    pub(crate) ceremony_phase: ::std::string::String,
    pub(crate) ceremony_qr: ::std::string::String,
    pub(crate) ceremony_detail: ::std::string::String,
    pub(crate) ceremony_left: ::std::string::String,
    pub(crate) huddle_joined: bool,
    pub(crate) huddle_channel: ::std::string::String,
    pub(crate) huddle_channel_name: ::std::string::String,
    pub(crate) huddle_joined_at: i64,
    pub(crate) huddle_now: i64,
    pub(crate) call_status: ::std::string::String,
    pub(crate) call_muted: bool,
    pub(crate) call_peers: ::std::vec::Vec<crate::call::CallEvent>,
    pub(crate) call_camera: bool,
    pub(crate) call_sharing: bool,
    pub(crate) call_video_live: bool,
    pub(crate) huddle_stage: ::std::string::String,
    pub(crate) huddle_roster: ::std::vec::Vec<crate::backend::HuddleParticipant>,
    pub(crate) huddle_rows: ::std::vec::Vec<crate::call::HuddleTileRow>,
    pub(crate) __ice_derived: __IceDerivedCache,
    pub(crate) __ice_secrets: crate::secret::SecretStore,
}
impl ::std::fmt::Debug for Ducktape {
    fn fmt(&self, __formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        __formatter.write_str("Ducktape")
    }
}
#[derive(Clone)]
pub(crate) enum __DucktapeMessage {
    __RequestLane0(u64, ::std::boxed::Box<__DucktapeMessage>),
    __RequestLane1(u64, ::std::boxed::Box<__DucktapeMessage>),
    __RequestLane2(u64, ::std::boxed::Box<__DucktapeMessage>),
    __RequestLane3(u64, ::std::boxed::Box<__DucktapeMessage>),
    __RequestLane4(u64, ::std::boxed::Box<__DucktapeMessage>),
    __RequestLane5(u64, ::std::boxed::Box<__DucktapeMessage>),
    __RequestLane6(u64, ::std::boxed::Box<__DucktapeMessage>),
    __RequestLane7(u64, ::std::boxed::Box<__DucktapeMessage>),
    __RequestLane8(u64, ::std::boxed::Box<__DucktapeMessage>),
    __RequestLane9(u64, ::std::boxed::Box<__DucktapeMessage>),
    __RequestLane10(
        u64,
        ::std::option::Option<::std::boxed::Box<__DucktapeMessage>>,
    ),
    __RequestLane11(u64, ::std::boxed::Box<__DucktapeMessage>),
    __RequestLane12(u64, ::std::boxed::Box<__DucktapeMessage>),
    __RequestLane13(u64, ::std::boxed::Box<__DucktapeMessage>),
    __RequestLane14(u64, ::std::boxed::Box<__DucktapeMessage>),
    __RequestLane15(u64, ::std::boxed::Box<__DucktapeMessage>),
    __RequestLane16(u64, ::std::boxed::Box<__DucktapeMessage>),
    __RequestLane17(u64, ::std::boxed::Box<__DucktapeMessage>),
    __RequestLane18(u64, ::std::boxed::Box<__DucktapeMessage>),
    __RequestLane19(u64, ::std::boxed::Box<__DucktapeMessage>),
    __RequestLane20(
        u64,
        ::std::option::Option<::std::boxed::Box<__DucktapeMessage>>,
    ),
    __RequestLane21(u64, ::std::boxed::Box<__DucktapeMessage>),
    __RequestLane22(u64, ::std::boxed::Box<__DucktapeMessage>),
    __RequestLane23(u64, ::std::boxed::Box<__DucktapeMessage>),
    __RequestLane24(
        u64,
        ::std::option::Option<::std::boxed::Box<__DucktapeMessage>>,
    ),
    __RequestLane25(u64, ::std::boxed::Box<__DucktapeMessage>),
    __RequestLane26(
        u64,
        ::std::option::Option<::std::boxed::Box<__DucktapeMessage>>,
    ),
    AppearanceLoaded(Appearance),
    SetAppearanceLight,
    SetAppearanceDark,
    AppearanceSaved(bool),
    DesktopNotificationsLoaded(bool),
    DesktopNotificationsSaved(bool),
    Reconnect,
    WorkspaceConnected(crate::backend::WorkspaceData),
    ConsoleEntryAnswered,
    LiveUpdated(crate::backend::LiveUpdate),
    LiveResynced(crate::backend::LiveRefresh),
    LiveResyncFailed(crate::backend::HydrationError),
    SelectShellTab(ShellTab),
    MembersLoadSelected(crate::backend::LoadRequest),
    SettingsLoadSelected(crate::backend::LoadRequest),
    AccountLoadSelected(crate::backend::LoadRequest),
    DmPeersLoadSelected(crate::backend::LoadRequest),
    NamesMovedSelected(crate::backend::LoadRequest),
    Tick,
    WallTick,
    WindowWasClosed(crate::shell::WindowKey),
    TrayOpen,
    TrayQuit,
    ModifierStateChanged(gpui_kit::Modifiers),
    DragLaunchWindow,
    CloseLaunchWindow,
    WindowFocused(crate::shell::WindowKey),
    WindowUnfocused(crate::shell::WindowKey),
    WindowFocusNoted,
    CommandChordPressed(crate::shell::KeyPress),
    TrayOpenBell,
    TrayGoChat,
    TrayGoPages,
    TrayGoNode,
    TrayGoSettings,
    TrayReconnect,
    TrayCopyNodeKey,
    MutationFailed(crate::backend::AppError),
    DismissError,
    ConnectFailed(crate::backend::HydrationError),
    ForgeViewEvent(crate::module_view::ModuleViewEvent),
    ForgeComposerEvent(::std::string::String, ::std::string::String),
    ForgeNoteSent(::std::string::String, crate::backend::SendReceipt),
    ForgeNoteFailed(
        ::std::string::String,
        ::std::string::String,
        crate::backend::OptimisticMutationError,
    ),
    FilesViewEvent(crate::module_view::ModuleViewEvent),
    FsFileDropped(::std::string::String),
    FsDropped(bool),
    FsDropFailed(crate::backend::AppError),
    AccountLoaded(crate::backend::AccountData),
    AccountFailed(crate::backend::HydrationError),
    AccountRenamed(bool),
    AccountRenameFailed(crate::backend::AppError),
    AccountTicketMinted(::std::string::String),
    AccountCeremonyStepped(crate::backend::CeremonyStep),
    AccountChanged(bool),
    AccountOpFailed(crate::backend::AppError),
    OpenRunPanel(::std::string::String),
    GovernanceViewEvent(crate::module_view::ModuleViewEvent),
    MembersViewEvent(crate::module_view::ModuleViewEvent),
    MembersLoaded(crate::backend::MembersData),
    MembersFailed(crate::backend::HydrationError),
    DmPeersLoaded(crate::backend::DmPeersData),
    DmPeersFailed(crate::backend::HydrationError),
    AgentsViewEvent(crate::module_view::ModuleViewEvent),
    AgentStatusSet(bool),
    NodeViewEvent(crate::module_view::ModuleViewEvent),
    NodeFactsLoaded(crate::backend::NodeFacts),
    NodeFactsFailed(crate::backend::AppError),
    NodeStatusPushed(crate::backend::NodeFacts),
    SettingsLoaded(crate::backend::SettingsFacts),
    SettingsFailed(crate::backend::HydrationError),
    SettingsViewEvent(crate::module_view::ModuleViewEvent),
    SettingsUnlocked(::std::string::String),
    SettingsUnlockFailed(crate::backend::AppError),
    CopyToClipboard(::std::string::String, ::std::string::String),
    DismissToast,
    ToastTick,
    ExplorerViewEvent(crate::module_view::ModuleViewEvent),
    ClosePalette,
    ToggleBell,
    ReloadBell,
    CloseBell,
    MarkBellReadSubmit,
    BellLoaded(i64, ::std::string::String, crate::backend::BellData),
    BellContextLoaded(
        i64,
        ::std::string::String,
        ::std::vec::Vec<crate::backend::BellPresentation>,
    ),
    BellFailed(i64, ::std::string::String, crate::backend::AppError),
    BellMarked(i64, ::std::string::String, crate::backend::BellDelta),
    BellMarkFailed(i64, ::std::string::String, crate::backend::AppError),
    BellOpenItem(i64, ::std::string::String, crate::backend::BellPresentation),
    GlobalKeyPressed(crate::shell::KeyPress),
    PaletteChanged(::std::string::String),
    PaletteResults(crate::backend::PaletteSearchData),
    PaletteSearchFailed(crate::backend::AppError),
    OpenChatSearchHit(::std::string::String, i64),
    ChooseChannel(::std::string::String),
    ChooseDm(::std::string::String),
    CreateChannelSubmit,
    ToggleChannelCreateMembersOnly,
    ToggleChannelCreate,
    JoinHuddleSubmit,
    HuddleJoinedAck(bool),
    ChatBeginEdit(::std::string::String, ::std::string::String, i64, i64),
    ComposerSubmitted(
        ComposerKind,
        ::std::string::String,
        ::std::string::String,
        ::std::string::String,
    ),
    EditMessageSubmit(::std::string::String),
    MessageSent(crate::backend::SendReceipt),
    MessageSendFailed(crate::backend::OptimisticMutationError),
    ThreadReplySendFailed(crate::backend::OptimisticMutationError),
    ThreadReplySent(crate::backend::SendReceipt),
    ChatUpdated(crate::backend::ChatData),
    ChatLoadFailed(crate::backend::HydrationError),
    ChannelCreated(crate::backend::ChatData),
    LiveAgentsEvent(crate::backend::LiveAgentNotice),
    LiveCancelAcked(bool),
    ChatAcked(bool),
    CopyMessageLink(::std::string::String),
    OpenMessageLink(::std::string::String),
    ChatScrolled(f64, f64, f64, f64),
    CopyChordPressed(crate::shell::KeyPress),
    ChatViewEvent(crate::module_view::ModuleViewEvent),
    PagesViewEvent(crate::module_view::ModuleViewEvent),
    OpenPageSearchHit(::std::string::String, ::std::string::String),
    ExternalUrlOpened(bool),
    ExternalUrlFailed(crate::backend::AppError),
    OnboardingOpened(crate::shell::WindowKey),
    HubBooted(crate::backend::HubState),
    HubRefreshed(crate::backend::HubState),
    NetworkProbed(crate::backend::HubProbe),
    PickWallet(::std::string::String),
    UnlockSubmit(::std::string::String),
    KeyUnlocked(::std::string::String),
    LoginSkip,
    PasswordSubmit(::std::string::String),
    DeviceKeyCreated(::std::string::String),
    PhraseWrittenDown,
    ShowPhraseAgain,
    ConfirmPhraseSubmit(::std::string::String),
    PhraseConfirmed(::std::string::String),
    PhraseConfirmFailed(crate::backend::AppError),
    GoRestore,
    GoLogin,
    RestoreSubmit(::std::string::String, ::std::string::String),
    KeyRestored(::std::string::String),
    LoginFailed(crate::backend::AppError),
    PickNetwork(::std::string::String),
    OpenNetworkSubmit,
    ConnectRemoteSubmit(::std::string::String),
    WalletsLoaded(crate::backend::WalletList),
    ChainNamed(::std::string::String),
    ChainProbeFailed(crate::backend::AppError),
    AccountProbed(crate::backend::AccountData),
    AccountProbeFailed(crate::backend::HydrationError),
    WelcomeSkip,
    WelcomeCancel,
    WelcomeCreateSubmit(::std::string::String),
    WelcomeLoginSubmit,
    WelcomeDesktop,
    WelcomeDesktopDone(bool),
    CeremonyStepped(crate::backend::CeremonyStep),
    WelcomeFailed(crate::backend::AppError),
    NetworkEntered,
    ConsoleOpened(crate::shell::WindowKey),
    ForgetNetworkSubmit(::std::string::String),
    NetworkForgotten(bool),
    GoJoin,
    GoNetworks,
    JoinNetworkSubmit,
    WorkspaceMaterialized(crate::backend::WorkspaceInit),
    ProvisionStepped(crate::backend::ProvisionStep),
    OnboardingInviteMinted(::std::string::String),
    CopyOnboardingInvite,
    EnterConsole,
    OnboardingFailed(crate::backend::AppError),
    SwitchNetwork,
    OnboardingReopened(crate::shell::WindowKey),
    DismissAccountBanner,
    OpenAccountWelcome,
    WelcomeReopened(crate::shell::WindowKey),
    CallEvent(crate::call::CallEvent),
    ToggleCallMute,
    ToggleCallCamera,
    ToggleCallScreen,
    ShowHuddle,
    HuddleOpened(crate::shell::WindowKey),
    HuddleGoChannel,
    LeaveHuddleHere,
    HuddleLeft(bool),
    __SecretTyped(::std::string::String, ::std::string::String),
    __BindWelcomeNameDraft(::std::string::String),
    __BindChannelDraft(::std::string::String),
    __BindPaletteDraft(::std::string::String),
    __ExternNoop,
}
impl ::std::fmt::Debug for __DucktapeMessage {
    fn fmt(&self, __formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        __formatter.write_str("__DucktapeMessage")
    }
}

impl Ducktape {
    fn __ice_derived_dark(&self) -> &bool {
        self.__ice_derived
            .dark
            .get_or_init(|| (self.appearance == Appearance::Dark))
    }
    fn __ice_derived_app_background(&self) -> &::std::string::String {
        self.__ice_derived.app_background.get_or_init(|| {
            crate::backend::keep_str(
                (self.appearance == Appearance::Dark),
                ::std::convert::AsRef::as_ref(&("#1b1a16")),
                ::std::convert::AsRef::as_ref(&("#fdfdfb")),
            )
        })
    }
    fn __ice_derived_app_text(&self) -> &::std::string::String {
        self.__ice_derived.app_text.get_or_init(|| {
            crate::backend::keep_str(
                (self.appearance == Appearance::Dark),
                ::std::convert::AsRef::as_ref(&("#e8e6df")),
                ::std::convert::AsRef::as_ref(&("#2c2b27")),
            )
        })
    }
    fn __ice_derived_has_error(&self) -> &bool {
        self.__ice_derived
            .has_error
            .get_or_init(|| (!(self.error).is_empty()))
    }
    fn __ice_derived_mutation_busy(&self) -> &bool {
        self.__ice_derived
            .mutation_busy
            .get_or_init(|| (self.mutation_phase != MutationPhase::Idle))
    }
    fn __ice_derived_hub_busy(&self) -> &bool {
        self.__ice_derived.hub_busy.get_or_init(|| {
            ((*self.__ice_derived_mutation_busy())
                || ((self.console_entry == ConsoleEntry::Entering)
                    && (self.onboarding_error).is_empty()))
        })
    }
    fn __window_0() -> crate::shell::WindowKind {
        crate::shell::WindowKind::Onboarding
    }
    fn __window_1() -> crate::shell::WindowKind {
        crate::shell::WindowKind::Console
    }
    fn __window_2() -> crate::shell::WindowKind {
        crate::shell::WindowKind::Huddle
    }
    fn __state() -> Self {
        Self {
            __ice_run_lane_0_generation: 0,
            __ice_run_lane_0_handle: ::std::option::Option::None,
            __ice_run_lane_1_generation: 0,
            __ice_run_lane_1_handle: ::std::option::Option::None,
            __ice_run_lane_2_generation: 0,
            __ice_run_lane_2_handle: ::std::option::Option::None,
            __ice_run_lane_3_generation: 0,
            __ice_run_lane_3_handle: ::std::option::Option::None,
            __ice_run_lane_4_generation: 0,
            __ice_run_lane_4_handle: ::std::option::Option::None,
            __ice_run_lane_5_generation: 0,
            __ice_run_lane_5_handle: ::std::option::Option::None,
            __ice_run_lane_6_generation: 0,
            __ice_run_lane_6_handle: ::std::option::Option::None,
            __ice_run_lane_7_generation: 0,
            __ice_run_lane_7_handle: ::std::option::Option::None,
            __ice_run_lane_8_generation: 0,
            __ice_run_lane_8_handle: ::std::option::Option::None,
            __ice_run_lane_9_generation: 0,
            __ice_run_lane_9_handle: ::std::option::Option::None,
            __ice_run_lane_10_generation: 0,
            __ice_run_lane_10_handle: ::std::option::Option::None,
            __ice_run_lane_11_generation: 0,
            __ice_run_lane_11_handle: ::std::option::Option::None,
            __ice_run_lane_12_generation: 0,
            __ice_run_lane_12_handle: ::std::option::Option::None,
            __ice_run_lane_13_generation: 0,
            __ice_run_lane_13_handle: ::std::option::Option::None,
            __ice_run_lane_14_generation: 0,
            __ice_run_lane_14_handle: ::std::option::Option::None,
            __ice_run_lane_15_generation: 0,
            __ice_run_lane_15_handle: ::std::option::Option::None,
            __ice_run_lane_16_generation: 0,
            __ice_run_lane_16_handle: ::std::option::Option::None,
            __ice_run_lane_17_generation: 0,
            __ice_run_lane_17_handle: ::std::option::Option::None,
            __ice_run_lane_18_generation: 0,
            __ice_run_lane_18_handle: ::std::option::Option::None,
            __ice_run_lane_19_generation: 0,
            __ice_run_lane_19_handle: ::std::option::Option::None,
            __ice_run_lane_20_generation: 0,
            __ice_run_lane_20_handle: ::std::option::Option::None,
            __ice_run_lane_21_generation: 0,
            __ice_run_lane_21_handle: ::std::option::Option::None,
            __ice_run_lane_22_generation: 0,
            __ice_run_lane_22_handle: ::std::option::Option::None,
            __ice_run_lane_23_generation: 0,
            __ice_run_lane_23_handle: ::std::option::Option::None,
            __ice_run_lane_24_generation: 0,
            __ice_run_lane_24_handle: ::std::option::Option::None,
            __ice_run_lane_25_generation: 0,
            __ice_run_lane_25_handle: ::std::option::Option::None,
            __ice_run_lane_26_generation: 0,
            __ice_run_lane_26_handle: ::std::option::Option::None,
            app_palette: AppTheme::App,
            appearance: Appearance::System,
            desktop_notifications: true,
            wall_now: ({ crate::backend::current_wall_seconds() }),
            rpc: "".to_owned(),
            connected_rpc: "".to_owned(),
            password: "".to_owned(),
            status: "Connecting…".to_owned(),
            connected: false,
            loading: false,
            views_live_serial: 0,
            cmd_held: false,
            shift_held: false,
            focused_win: ::std::option::Option::None,
            block_height: (-1),
            hydration_generation: 0,
            connect_generation: 0,
            signer_key: "".to_owned(),
            hydration_retry_attempt: 0,
            mutation_phase: MutationPhase::Idle,
            error: "".to_owned(),
            startup_duck_link: crate::backend::startup_duck_url(),
            channels: ::std::vec::Vec::new(),
            rooms: ::std::vec::Vec::new(),
            chat_generation: 0,
            channel_reads: ::std::vec::Vec::new(),
            unread_boundary: 0,
            active_channel: "".to_owned(),
            active_channel_name: "".to_owned(),
            active_channel_archived: false,
            active_channel_members_only: false,
            channel_members: ::std::vec::Vec::new(),
            post_refusal: "".to_owned(),
            chat_land_seq: 0,
            chat_edit_seq: 0,
            chat_edit_rev: 0,
            composer_stashed: false,
            composer_roster_set: false,
            composer_seeded: false,
            live_agents: ::std::vec::Vec::new(),
            chat_sent_serial: 0,
            chat_pending_sends: ::std::vec::Vec::new(),
            chat_copy_chord_serial: 0,
            channel_draft: "".to_owned(),
            channel_create_open: false,
            channel_create_members_only: false,
            pending_channel: "".to_owned(),
            chat_at_tail: true,
            history_view: false,
            chat_chain_id: "".to_owned(),
            dm_peers: ::std::vec::Vec::new(),
            dm_rows: ::std::vec::Vec::new(),
            dm_peers_generation: 0,
            active_dm_peer: "".to_owned(),
            active_dm: crate::backend::no_dm_peer(),
            shell_tab: ShellTab::Chat,
            members_answered: false,
            members_rows: ::std::vec::Vec::new(),
            members_generation: 0,
            gov_open: 0,
            agents_open_run: "".to_owned(),
            agents_opened: 0,
            agents_live: false,
            forge_note_pending: "".to_owned(),
            forge_link: "".to_owned(),
            forge_link_tick: 0,
            node_key: "".to_owned(),
            node_data_dir: "".to_owned(),
            settings_key_path: "".to_owned(),
            settings_key_state: "".to_owned(),
            settings_user_key: "".to_owned(),
            settings_generation: 0,
            account_exists: false,
            account_number: "".to_owned(),
            account_name: "".to_owned(),
            account_bio: "".to_owned(),
            account_generation: 0,
            account_busy: false,
            account_ticket: "".to_owned(),
            account_banner_dismissed: false,
            account_ceremony_phase: "".to_owned(),
            account_ceremony_qr: "".to_owned(),
            account_ceremony_detail: "".to_owned(),
            account_ceremony_left: "".to_owned(),
            node_version: "".to_owned(),
            node_root_hash: "".to_owned(),
            network_chain_id: "".to_owned(),
            node_last_finalized: (-1),
            node_checkpoint: (-1),
            node_height: (-1),
            node_phase: "".to_owned(),
            node_phase_since: (-1),
            node_sync_target: (-1),
            node_sync_applied: (-1),
            node_sync_retries: 0,
            node_sync_failures: 0,
            node_sync_last_error: "".to_owned(),
            node_view_label: "—".to_owned(),
            node_quorum_label: "—".to_owned(),
            node_reachable_label: "—".to_owned(),
            fs_drop_dir: "/shared".to_owned(),
            fs_dropping: false,
            fs_route: "".to_owned(),
            fs_route_serial: 0,
            palette_open: false,
            bell_open: false,
            bell_unread: 0,
            bell_items: ::std::vec::Vec::new(),
            bell_presentations: ::std::vec::Vec::new(),
            bell_read_through: 0,
            bell_clear_through: 0,
            bell_marking: false,
            bell_error: "".to_owned(),
            palette_draft: "".to_owned(),
            palette_search_phase: SearchPhase::Idle,
            palette_chat_hits: ::std::vec::Vec::new(),
            palette_page_hits: ::std::vec::Vec::new(),
            toast: "".to_owned(),
            toast_age: 0,
            page_route: "".to_owned(),
            page_route_serial: 0,
            onboarding_win: ::std::option::Option::None,
            console_win: ::std::option::Option::None,
            huddle_win: ::std::option::Option::None,
            network_name: "".to_owned(),
            hub_step: HubStep::Loading,
            hub_networks: ::std::vec::Vec::new(),
            hub_selected: "".to_owned(),
            hub_wallets: ::std::vec::Vec::new(),
            hub_wallet_selected: "".to_owned(),
            onboarding_name: "".to_owned(),
            onboarding_error: "".to_owned(),
            console_entry: ConsoleEntry::Idle,
            invite_link: "".to_owned(),
            provision_steps: ::std::vec::Vec::new(),
            provision_index: 0,
            hub_chain_id: "".to_owned(),
            welcome_name_draft: "".to_owned(),
            ceremony_phase: "".to_owned(),
            ceremony_qr: "".to_owned(),
            ceremony_detail: "".to_owned(),
            ceremony_left: "".to_owned(),
            huddle_joined: false,
            huddle_channel: "".to_owned(),
            huddle_channel_name: "".to_owned(),
            huddle_joined_at: 0,
            huddle_now: 0,
            call_status: "".to_owned(),
            call_muted: false,
            call_peers: ::std::vec::Vec::new(),
            call_camera: false,
            call_sharing: false,
            call_video_live: false,
            huddle_stage: "".to_owned(),
            huddle_roster: ::std::vec::Vec::new(),
            huddle_rows: ::std::vec::Vec::new(),
            __ice_derived: ::std::default::Default::default(),
            __ice_secrets: ::std::default::Default::default(),
        }
    }
    fn __boot_task(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
        let task = (|| {
            let (_, __task) = crate::shell::open(Self::__window_0());
            __task.map(move |value| __DucktapeMessage::OnboardingOpened(value))
        })();
        task
    }
    fn __preset_task_0(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
        let task = (|| {
            self.rpc = "".to_owned();
            self.status = "Offline".to_owned();
            self.connected = false;
            self.loading = false;
            self.mutation_phase = MutationPhase::Idle;
            self.__ice_derived.mutation_busy.take();
            self.__ice_derived.hub_busy.take();
            self.error = "".to_owned();
            self.__ice_derived.has_error.take();
            self.shell_tab = ShellTab::Chat;
            self.channel_draft = "".to_owned();
            self.channel_create_members_only = false;
            self.palette_open = false;
            self.palette_draft = "".to_owned();
            ::ducktape_view_guest::Task::none()
        })();
        task
    }
    fn __preset_task_1(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
        let task = (|| {
            self.status = "Offline".to_owned();
            self.connected = false;
            self.loading = false;
            self.mutation_phase = MutationPhase::Idle;
            self.__ice_derived.mutation_busy.take();
            self.__ice_derived.hub_busy.take();
            self.shell_tab = ShellTab::Chat;
            self.palette_open = true;
            self.palette_draft = "".to_owned();
            ::ducktape_view_guest::Task::none()
        })();
        task
    }
    fn __preset_task_2(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
        let task = (|| {
            self.rpc = "".to_owned();
            self.status = "Offline".to_owned();
            self.connected = false;
            self.loading = false;
            self.mutation_phase = MutationPhase::Idle;
            self.__ice_derived.mutation_busy.take();
            self.__ice_derived.hub_busy.take();
            self.error = "".to_owned();
            self.__ice_derived.has_error.take();
            self.shell_tab = ShellTab::Settings;
            ::ducktape_view_guest::Task::none()
        })();
        task
    }
    fn __preset_task_3(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
        let task = (|| {
            self.error = "Connection failed".to_owned();
            self.__ice_derived.has_error.take();
            ::ducktape_view_guest::Task::none()
        })();
        task
    }
    fn __preset_task_4(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
        let task = (|| {
            self.mutation_phase = MutationPhase::Idle;
            self.__ice_derived.mutation_busy.take();
            self.__ice_derived.hub_busy.take();
            self.onboarding_error = "".to_owned();
            self.__ice_derived.hub_busy.take();
            self.hub_step = HubStep::Networks;
            self.hub_networks = ::std::vec::Vec::new();
            self.hub_selected = "".to_owned();
            self.rpc = "http://127.0.0.1:1".to_owned();
            self.password = "hunter2-hunter2".to_owned();
            self.hub_wallets = ::std::vec![
                crate::backend::wallet_info(
                    "alice".to_owned(),
                    "aabbccddeeff00112233".to_owned(),
                    "encrypted".to_owned(),
                    false
                ),
                crate::backend::wallet_info(
                    "demo".to_owned(),
                    "eeff0011".to_owned(),
                    "encrypted".to_owned(),
                    true
                )
            ];
            self.hub_wallet_selected = "demo".to_owned();
            ::ducktape_view_guest::Task::none()
        })();
        task
    }
    fn __preset_task_5(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
        let task = (|| {
            self.mutation_phase = MutationPhase::Onboarding;
            self.__ice_derived.mutation_busy.take();
            self.__ice_derived.hub_busy.take();
            self.onboarding_error = "".to_owned();
            self.__ice_derived.hub_busy.take();
            self.hub_step = HubStep::Account;
            self.ceremony_phase = "show_qr".to_owned();
            self.ceremony_qr = "https://auth.ducktape.industries/#op=get&challenge=AQID".to_owned();
            self.ceremony_detail = "Your phone will confirm with the passkey.".to_owned();
            self.ceremony_left = "4:58".to_owned();
            ::ducktape_view_guest::Task::none()
        })();
        task
    }
    fn __preset_task_6(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
        let task = (|| {
            self.rpc = "http://127.0.0.1:1".to_owned();
            self.connected_rpc = "http://127.0.0.1:1".to_owned();
            self.password = "hunter2-hunter2".to_owned();
            self.status = "Connected".to_owned();
            self.connected = true;
            self.loading = false;
            self.mutation_phase = MutationPhase::Idle;
            self.__ice_derived.mutation_busy.take();
            self.__ice_derived.hub_busy.take();
            self.error = "".to_owned();
            self.__ice_derived.has_error.take();
            self.account_exists = false;
            self.account_banner_dismissed = false;
            self.shell_tab = ShellTab::Settings;
            ::ducktape_view_guest::Task::none()
        })();
        task
    }
    fn __preset_task_7(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
        let task = (|| {
            self.password = "hunter2-hunter2".to_owned();
            self.rpc = "http://127.0.0.1:1".to_owned();
            self.mutation_phase = MutationPhase::Onboarding;
            self.__ice_derived.mutation_busy.take();
            self.__ice_derived.hub_busy.take();
            self.hub_step = HubStep::Networks;
            ::ducktape_view_guest::Task::none()
        })();
        task
    }
    fn __preset_task_8(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
        let task = (|| {
            self.palette_open = true;
            self.palette_draft = "".to_owned();
            self.connected_rpc = "http://127.0.0.1:1".to_owned();
            ::ducktape_view_guest::Task::none()
        })();
        task
    }
    fn __preset_task_9(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
        let task = (|| {
            self.connected = true;
            self.console_win = ::std::option::Option::Some(
                ({ crate::backend::window_target(::std::option::Option::None) }),
            );
            self.connected_rpc = "http://127.0.0.1:1".to_owned();
            self.network_name = "demo".to_owned();
            self.bell_unread = 3;
            self.huddle_joined = true;
            self.huddle_channel_name = "general".to_owned();
            self.call_muted = false;
            self.appearance = Appearance::Dark;
            self.__ice_derived.dark.take();
            self.__ice_derived.app_background.take();
            self.__ice_derived.app_text.take();
            ::ducktape_view_guest::Task::none()
        })();
        task
    }
    fn __preset_task_10(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
        let task = (|| {
            self.connected = false;
            self.huddle_joined = true;
            self.huddle_channel_name = "eng".to_owned();
            self.call_status = "live".to_owned();
            self.call_muted = false;
            self.call_camera = false;
            self.call_sharing = true;
            self.call_video_live = true;
            self.huddle_stage = "you".to_owned();
            ::ducktape_view_guest::Task::none()
        })();
        task
    }
    fn __preset_task_11(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
        let task = (|| {
            self.connected = true;
            self.hub_step = HubStep::Live;
            ::ducktape_view_guest::Task::none()
        })();
        task
    }
    fn __preset_task_12(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
        let task = (|| {
            self.connected = true;
            self.connected_rpc = "http://127.0.0.1:8844".to_owned();
            self.network_chain_id = "testnet#abcd".to_owned();
            self.connect_generation = 7;
            self.signer_key = "aa11".to_owned();
            self.shell_tab = ShellTab::Chat;
            self.live_agents = ::std::vec![crate::backend::live_agent_row(
                "channel-a".to_owned(),
                2,
                "chat:2:agent-1".to_owned(),
                "Chief Duck".to_owned(),
                "Reading the repo".to_owned()
            )];
            ::ducktape_view_guest::Task::none()
        })();
        task
    }
    fn __preset_task_13(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
        let task = (|| {
            self.connected = true;
            self.connected_rpc = "http://127.0.0.1:8844".to_owned();
            self.network_chain_id = "testnet#abcd".to_owned();
            self.connect_generation = 7;
            self.signer_key = "aa11".to_owned();
            self.shell_tab = ShellTab::Settings;
            self.account_ceremony_phase = "qr".to_owned();
            self.account_ceremony_qr = "otpauth://totp/demo".to_owned();
            ::ducktape_view_guest::Task::none()
        })();
        task
    }
    fn __subscription(&self) -> ducktape_view_guest::Subscription<__DucktapeMessage> {
        use ducktape_view_guest::Subscription;
        let mut subscriptions = Vec::new();
        if self.connected {
            subscriptions.push(
                Subscription::run_with(self.connected_rpc.clone(), |rpc: &String| {
                    crate::backend::live_events(rpc.clone())
                })
                .map(__DucktapeMessage::LiveUpdated),
            );
            subscriptions.push(
                Subscription::run_with(self.connected_rpc.clone(), |rpc: &String| {
                    crate::backend::node_status_live(rpc.clone())
                })
                .map(__DucktapeMessage::NodeStatusPushed),
            );
            subscriptions.push(
                Subscription::run_with(
                    (
                        self.connected_rpc.clone(),
                        self.network_chain_id.clone(),
                        self.connect_generation,
                        self.signer_key.clone(),
                    ),
                    |data: &(String, String, i64, String)| {
                        crate::backend::chat_live_agents(
                            data.0.clone(),
                            data.1.clone(),
                            data.2,
                            data.3.clone(),
                        )
                    },
                )
                .map(__DucktapeMessage::LiveAgentsEvent),
            );
        }
        let active_call = self.connected && self.huddle_joined && !self.huddle_channel.is_empty();
        if active_call {
            subscriptions.push(
                Subscription::run_with(
                    (self.connected_rpc.clone(), self.huddle_channel.clone()),
                    |data: &(String, String)| {
                        crate::call::call_session(data.0.clone(), data.1.clone())
                    },
                )
                .map(__DucktapeMessage::CallEvent),
            );
        }
        if self.huddle_joined {
            subscriptions
                .push(Subscription::run(crate::shell::seconds).map(|()| __DucktapeMessage::Tick));
        }
        if self.console_win.is_some() {
            subscriptions.push(
                Subscription::run(crate::shell::seconds).map(|()| __DucktapeMessage::WallTick),
            );
        }
        Subscription::batch(subscriptions)
    }
    pub(crate) fn __boot() -> (Self, ducktape_view_guest::Task<__DucktapeMessage>) {
        let mut state = Self::__state();
        let task = state.__boot_task();
        (state, task)
    }
    #[cfg(test)]
    fn __preset_0() -> (Self, ducktape_view_guest::Task<__DucktapeMessage>) {
        let mut state = Self::__state();
        let task = state.__preset_task_0();
        (state, task)
    }
    #[cfg(test)]
    fn __preset_1() -> (Self, ducktape_view_guest::Task<__DucktapeMessage>) {
        let mut state = Self::__state();
        let task = state.__preset_task_1();
        (state, task)
    }
    #[cfg(test)]
    fn __preset_2() -> (Self, ducktape_view_guest::Task<__DucktapeMessage>) {
        let mut state = Self::__state();
        let task = state.__preset_task_2();
        (state, task)
    }
    #[cfg(test)]
    fn __preset_3() -> (Self, ducktape_view_guest::Task<__DucktapeMessage>) {
        let mut state = Self::__state();
        let task = state.__preset_task_3();
        (state, task)
    }
    #[cfg(test)]
    fn __preset_4() -> (Self, ducktape_view_guest::Task<__DucktapeMessage>) {
        let mut state = Self::__state();
        let task = state.__preset_task_4();
        (state, task)
    }
    #[cfg(test)]
    fn __preset_5() -> (Self, ducktape_view_guest::Task<__DucktapeMessage>) {
        let mut state = Self::__state();
        let task = state.__preset_task_5();
        (state, task)
    }
    #[cfg(test)]
    fn __preset_6() -> (Self, ducktape_view_guest::Task<__DucktapeMessage>) {
        let mut state = Self::__state();
        let task = state.__preset_task_6();
        (state, task)
    }
    #[cfg(test)]
    fn __preset_7() -> (Self, ducktape_view_guest::Task<__DucktapeMessage>) {
        let mut state = Self::__state();
        let task = state.__preset_task_7();
        (state, task)
    }
    #[cfg(test)]
    fn __preset_8() -> (Self, ducktape_view_guest::Task<__DucktapeMessage>) {
        let mut state = Self::__state();
        let task = state.__preset_task_8();
        (state, task)
    }
    #[cfg(test)]
    fn __preset_9() -> (Self, ducktape_view_guest::Task<__DucktapeMessage>) {
        let mut state = Self::__state();
        let task = state.__preset_task_9();
        (state, task)
    }
    #[cfg(test)]
    fn __preset_10() -> (Self, ducktape_view_guest::Task<__DucktapeMessage>) {
        let mut state = Self::__state();
        let task = state.__preset_task_10();
        (state, task)
    }
    #[cfg(test)]
    fn __preset_11() -> (Self, ducktape_view_guest::Task<__DucktapeMessage>) {
        let mut state = Self::__state();
        let task = state.__preset_task_11();
        (state, task)
    }
    #[cfg(test)]
    fn __preset_12() -> (Self, ducktape_view_guest::Task<__DucktapeMessage>) {
        let mut state = Self::__state();
        let task = state.__preset_task_12();
        (state, task)
    }
    #[cfg(test)]
    fn __preset_13() -> (Self, ducktape_view_guest::Task<__DucktapeMessage>) {
        let mut state = Self::__state();
        let task = state.__preset_task_13();
        (state, task)
    }
}
include!("app_update.rs");
#[cfg(test)]
mod state_tests {
    use super::*;
    use futures::{FutureExt, StreamExt};
    fn dispatch(state: &mut Ducktape, message: __DucktapeMessage) {
        let mut tasks = vec![state.__update(message).into_stream()];
        while let Some(mut stream) = tasks.pop() {
            while let Some(Some(message)) = stream.next().now_or_never() {
                tasks.push(state.__update(message).into_stream());
            }
        }
    }
    #[test]
    fn an_unlock_in_place_drops_the_previous_keys_live_rows() {
        let (mut state, _) = Ducktape::__preset_12();
        let __actual = !(state.live_agents).is_empty();
        assert!(__actual);
        let __message = __DucktapeMessage::SettingsUnlocked("bb22".to_owned());
        dispatch(&mut state, __message);
        let __left = state.signer_key.to_owned();
        let __right = "bb22".to_owned();
        assert_eq!(__left, __right);
        let __actual = (state.live_agents).is_empty();
        assert!(__actual);
    }
    #[test]
    fn locking_the_seat_takes_the_private_output_with_it() {
        let (mut state, _) = Ducktape::__preset_12();
        let __actual = !(state.live_agents).is_empty();
        assert!(__actual);
        let __message = __DucktapeMessage::SettingsViewEvent(crate::module_view::view_event(
            "lock".to_owned(),
            "".to_owned(),
        ));
        dispatch(&mut state, __message);
        let __actual = (state.signer_key).is_empty();
        assert!(__actual);
        let __actual = (state.password).is_empty();
        assert!(__actual);
        let __actual = (state.live_agents).is_empty();
        assert!(__actual);
    }
    #[tokio::test]
    async fn a_chat_address_opened_from_another_tab_lands_on_the_chat_tab() {
        let (mut state, _) = Ducktape::__preset_12();
        let __message = __DucktapeMessage::SelectShellTab(ShellTab::Agents);
        dispatch(&mut state, __message);
        let __left = state.shell_tab.clone();
        let __right = ShellTab::Agents;
        assert_eq!(__left, __right);
        let __message = __DucktapeMessage::OpenMessageLink("duck://channel/general".to_owned());
        dispatch(&mut state, __message);
        let __left = state.shell_tab.clone();
        let __right = ShellTab::Chat;
        assert_eq!(__left, __right);
    }
    #[test]
    fn a_run_opened_mid_ceremony_retires_the_ceremony_like_any_tab_move() {
        let (mut state, _) = Ducktape::__preset_13();
        let __actual = !(state.account_ceremony_phase).is_empty();
        assert!(__actual);
        let __message = __DucktapeMessage::OpenRunPanel("dispatch-1".to_owned());
        dispatch(&mut state, __message);
        let __left = state.shell_tab.clone();
        let __right = ShellTab::Agents;
        assert_eq!(__left, __right);
        let __left = state.agents_open_run.to_owned();
        let __right = "dispatch-1".to_owned();
        assert_eq!(__left, __right);
        let __actual = (state.account_ceremony_phase).is_empty();
        assert!(__actual);
        let __actual = (state.account_ceremony_qr).is_empty();
        assert!(__actual);
    }
}
