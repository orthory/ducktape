macro_rules! __ice_generated_items_2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f6170702e696365 { ($($item:item)*) => { $(#[allow(warnings, clippy::all)] $item)* }; }
__ice_generated_items_2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f6170702e696365! {
type __IceRenderer = ::iced::Renderer; type __IceElement<'a, Message, Theme = ::iced::Theme> = ::iced::Element<'a, Message, Theme, __IceRenderer>;
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct __IceKeyPress {
    key: ::iced::keyboard::Key,
    modified_key: ::iced::keyboard::Key,
    physical_key: ::iced::keyboard::key::Physical,
    location: ::iced::keyboard::Location,
    modifiers: ::iced::keyboard::Modifiers,
    text: ::std::option::Option<::std::string::String>,
    repeat: bool,
}
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct __IceKeyRelease {
    key: ::iced::keyboard::Key,
    modified_key: ::iced::keyboard::Key,
    physical_key: ::iced::keyboard::key::Physical,
    location: ::iced::keyboard::Location,
    modifiers: ::iced::keyboard::Modifiers,
}
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct __IceSystemInfo {
    system_name: ::std::option::Option<::std::string::String>,
    system_kernel: ::std::option::Option<::std::string::String>,
    system_version: ::std::option::Option<::std::string::String>,
    system_short_version: ::std::option::Option<::std::string::String>,
    cpu_brand: ::std::string::String,
    cpu_cores: ::std::option::Option<i64>,
    memory_total: i64,
    memory_used: ::std::option::Option<i64>,
    graphics_backend: ::std::string::String,
    graphics_adapter: ::std::string::String,
}
fn __ice_system_theme(value: ::iced::theme::Mode) -> ::std::string::String {
    match value {
        ::iced::theme::Mode::None => "none",
        ::iced::theme::Mode::Light => "light",
        ::iced::theme::Mode::Dark => "dark",
    }.to_owned()
}
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct __IceWidgetTarget {
    kind: ::std::string::String,
    id: ::std::option::Option<::iced::widget::Id>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    visible_x: ::std::option::Option<f64>,
    visible_y: ::std::option::Option<f64>,
    visible_width: ::std::option::Option<f64>,
    visible_height: ::std::option::Option<f64>,
    content: ::std::option::Option<::std::string::String>,
    content_x: ::std::option::Option<f64>,
    content_y: ::std::option::Option<f64>,
    content_width: ::std::option::Option<f64>,
    content_height: ::std::option::Option<f64>,
    translation_x: ::std::option::Option<f64>,
    translation_y: ::std::option::Option<f64>,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum AppTheme {
App,
AppDark,
}
#[derive(Clone, Copy)]
struct __IcePalette { name: &'static str, colors: [::iced::Color; 128] }
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
pub(crate) struct __IceWalletRowState {
pw: ::std::string::String,
__ice_rev: [u64; 1],
}
impl ::std::default::Default for __IceWalletRowState {
fn default() -> Self { Self {
pw: "".to_owned(),
__ice_rev: [::ui_lang_runtime::rev::seed(); 1],
} }
}
#[allow(dead_code)]
pub(crate) struct __IcePasswordScreenState {
pw: ::std::string::String,
pw2: ::std::string::String,
__ice_rev: [u64; 2],
}
impl ::std::default::Default for __IcePasswordScreenState {
fn default() -> Self { Self {
pw: "".to_owned(),
pw2: "".to_owned(),
__ice_rev: [::ui_lang_runtime::rev::seed(); 2],
} }
}
#[allow(dead_code)]
pub(crate) struct __IceConfirmPhraseScreenState {
answer: ::std::string::String,
__ice_rev: [u64; 1],
}
impl ::std::default::Default for __IceConfirmPhraseScreenState {
fn default() -> Self { Self {
answer: "".to_owned(),
__ice_rev: [::ui_lang_runtime::rev::seed(); 1],
} }
}
#[allow(dead_code)]
pub(crate) struct __IceRestoreScreenState {
name_draft: ::std::string::String,
pw: ::std::string::String,
__ice_rev: [u64; 2],
}
impl ::std::default::Default for __IceRestoreScreenState {
fn default() -> Self { Self {
name_draft: "restored".to_owned(),
pw: "".to_owned(),
__ice_rev: [::ui_lang_runtime::rev::seed(); 2],
} }
}
#[allow(dead_code)]
pub(crate) struct __IceNetworksScreenState {
remote: ::std::string::String,
__ice_rev: [u64; 1],
}
impl ::std::default::Default for __IceNetworksScreenState {
fn default() -> Self { Self {
remote: "".to_owned(),
__ice_rev: [::ui_lang_runtime::rev::seed(); 1],
} }
}
#[cfg(test)]
#[allow(non_camel_case_types, dead_code)]
#[derive(Clone)]
pub(crate) struct __IceTestState_wallet_row {
pub(crate) pw: ::std::string::String,
}
#[cfg(test)]
#[allow(dead_code)]
impl Ducktape {
pub(crate) fn __ice_test_state_wallet_row(&self, scope: &str) -> ::std::option::Option<__IceTestState_wallet_row> { let __ice_view = |__state: &__IceWalletRowState| __IceTestState_wallet_row { pw: __state.pw.clone(), }; let __ice_stored = self.__ice_component_057616c6c6574526f77.get(scope).map(__ice_view); __ice_stored.or_else(|| self.__ice_test_scopes_wallet_row().iter().any(|__ice_scope| __ice_scope == scope).then(|| __ice_view(&<__IceWalletRowState>::default()))) }
pub(crate) fn __ice_test_scopes_wallet_row(&self) -> ::std::vec::Vec<::std::string::String> { let mut __scopes: ::std::vec::Vec<::std::string::String> = self.__ice_component_057616c6c6574526f77.keys().cloned().collect(); for __scope in ::ui_lang_runtime::testing::component_sightings("WalletRow") { if !__scopes.contains(&__scope) { __scopes.push(__scope); } } __scopes }
}
#[cfg(test)]
#[allow(non_camel_case_types, dead_code)]
#[derive(Clone)]
pub(crate) struct __IceTestState_password_screen {
pub(crate) pw: ::std::string::String,
pub(crate) pw2: ::std::string::String,
}
#[cfg(test)]
#[allow(dead_code)]
impl Ducktape {
pub(crate) fn __ice_test_state_password_screen(&self, scope: &str) -> ::std::option::Option<__IceTestState_password_screen> { let __ice_view = |__state: &__IcePasswordScreenState| __IceTestState_password_screen { pw: __state.pw.clone(),pw2: __state.pw2.clone(), }; let __ice_stored = self.__ice_component_050617373776f726453637265656e.get(scope).map(__ice_view); __ice_stored.or_else(|| self.__ice_test_scopes_password_screen().iter().any(|__ice_scope| __ice_scope == scope).then(|| __ice_view(&<__IcePasswordScreenState>::default()))) }
pub(crate) fn __ice_test_scopes_password_screen(&self) -> ::std::vec::Vec<::std::string::String> { let mut __scopes: ::std::vec::Vec<::std::string::String> = self.__ice_component_050617373776f726453637265656e.keys().cloned().collect(); for __scope in ::ui_lang_runtime::testing::component_sightings("PasswordScreen") { if !__scopes.contains(&__scope) { __scopes.push(__scope); } } __scopes }
}
#[cfg(test)]
#[allow(non_camel_case_types, dead_code)]
#[derive(Clone)]
pub(crate) struct __IceTestState_confirm_phrase_screen {
pub(crate) answer: ::std::string::String,
}
#[cfg(test)]
#[allow(dead_code)]
impl Ducktape {
pub(crate) fn __ice_test_state_confirm_phrase_screen(&self, scope: &str) -> ::std::option::Option<__IceTestState_confirm_phrase_screen> { let __ice_view = |__state: &__IceConfirmPhraseScreenState| __IceTestState_confirm_phrase_screen { answer: __state.answer.clone(), }; let __ice_stored = self.__ice_component_0436f6e6669726d50687261736553637265656e.get(scope).map(__ice_view); __ice_stored.or_else(|| self.__ice_test_scopes_confirm_phrase_screen().iter().any(|__ice_scope| __ice_scope == scope).then(|| __ice_view(&<__IceConfirmPhraseScreenState>::default()))) }
pub(crate) fn __ice_test_scopes_confirm_phrase_screen(&self) -> ::std::vec::Vec<::std::string::String> { let mut __scopes: ::std::vec::Vec<::std::string::String> = self.__ice_component_0436f6e6669726d50687261736553637265656e.keys().cloned().collect(); for __scope in ::ui_lang_runtime::testing::component_sightings("ConfirmPhraseScreen") { if !__scopes.contains(&__scope) { __scopes.push(__scope); } } __scopes }
}
#[cfg(test)]
#[allow(non_camel_case_types, dead_code)]
#[derive(Clone)]
pub(crate) struct __IceTestState_restore_screen {
pub(crate) name_draft: ::std::string::String,
pub(crate) pw: ::std::string::String,
}
#[cfg(test)]
#[allow(dead_code)]
impl Ducktape {
pub(crate) fn __ice_test_state_restore_screen(&self, scope: &str) -> ::std::option::Option<__IceTestState_restore_screen> { let __ice_view = |__state: &__IceRestoreScreenState| __IceTestState_restore_screen { name_draft: __state.name_draft.clone(),pw: __state.pw.clone(), }; let __ice_stored = self.__ice_component_0526573746f726553637265656e.get(scope).map(__ice_view); __ice_stored.or_else(|| self.__ice_test_scopes_restore_screen().iter().any(|__ice_scope| __ice_scope == scope).then(|| __ice_view(&<__IceRestoreScreenState>::default()))) }
pub(crate) fn __ice_test_scopes_restore_screen(&self) -> ::std::vec::Vec<::std::string::String> { let mut __scopes: ::std::vec::Vec<::std::string::String> = self.__ice_component_0526573746f726553637265656e.keys().cloned().collect(); for __scope in ::ui_lang_runtime::testing::component_sightings("RestoreScreen") { if !__scopes.contains(&__scope) { __scopes.push(__scope); } } __scopes }
}
#[cfg(test)]
#[allow(non_camel_case_types, dead_code)]
#[derive(Clone)]
pub(crate) struct __IceTestState_networks_screen {
pub(crate) remote: ::std::string::String,
}
#[cfg(test)]
#[allow(dead_code)]
impl Ducktape {
pub(crate) fn __ice_test_state_networks_screen(&self, scope: &str) -> ::std::option::Option<__IceTestState_networks_screen> { let __ice_view = |__state: &__IceNetworksScreenState| __IceTestState_networks_screen { remote: __state.remote.clone(), }; let __ice_stored = self.__ice_component_04e6574776f726b7353637265656e.get(scope).map(__ice_view); __ice_stored.or_else(|| self.__ice_test_scopes_networks_screen().iter().any(|__ice_scope| __ice_scope == scope).then(|| __ice_view(&<__IceNetworksScreenState>::default()))) }
pub(crate) fn __ice_test_scopes_networks_screen(&self) -> ::std::vec::Vec<::std::string::String> { let mut __scopes: ::std::vec::Vec<::std::string::String> = self.__ice_component_04e6574776f726b7353637265656e.keys().cloned().collect(); for __scope in ::ui_lang_runtime::testing::component_sightings("NetworksScreen") { if !__scopes.contains(&__scope) { __scopes.push(__scope); } } __scopes }
}
#[allow(dead_code)]
pub struct Ducktape {
pub(crate) __ice_accessibility: ::ui_lang_runtime::Bridge<__DucktapeMessage>,
#[cfg(all(target_os = "macos", not(test)))]
pub(crate) __ice_accessibility_windows: ::ui_lang_runtime::WindowBridges<__DucktapeMessage>,
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
pub(crate) focused_win: ::std::option::Option<::crate::shell::WindowKey>,
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
pub(crate) onboarding_win: ::std::option::Option<::crate::shell::WindowKey>,
pub(crate) console_win: ::std::option::Option<::crate::shell::WindowKey>,
pub(crate) huddle_win: ::std::option::Option<::crate::shell::WindowKey>,
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
pub(crate) __ice_rev: [u64; 156],
pub(crate) __ice_component_057616c6c6574526f77: ::std::collections::HashMap<::std::string::String, __IceWalletRowState>,
pub(crate) __ice_component_057616c6c6574526f77_initial: __IceWalletRowState,
pub(crate) __ice_component_050617373776f726453637265656e: ::std::collections::HashMap<::std::string::String, __IcePasswordScreenState>,
pub(crate) __ice_component_050617373776f726453637265656e_initial: __IcePasswordScreenState,
pub(crate) __ice_component_0436f6e6669726d50687261736553637265656e: ::std::collections::HashMap<::std::string::String, __IceConfirmPhraseScreenState>,
pub(crate) __ice_component_0436f6e6669726d50687261736553637265656e_initial: __IceConfirmPhraseScreenState,
pub(crate) __ice_component_0526573746f726553637265656e: ::std::collections::HashMap<::std::string::String, __IceRestoreScreenState>,
pub(crate) __ice_component_0526573746f726553637265656e_initial: __IceRestoreScreenState,
pub(crate) __ice_component_04e6574776f726b7353637265656e: ::std::collections::HashMap<::std::string::String, __IceNetworksScreenState>,
pub(crate) __ice_component_04e6574776f726b7353637265656e_initial: __IceNetworksScreenState,
pub(crate) __ice_secrets: ::ui_lang_runtime::SecretStore,
}
impl ::std::fmt::Debug for Ducktape { fn fmt(&self, __formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result { __formatter.write_str("Ducktape") } }
#[derive(Clone)]
pub(crate) enum __DucktapeMessage {
__AccessibilitySnapshot(::std::boxed::Box<::ui_lang_runtime::Snapshot<__DucktapeMessage>>),
__AccessibilityAction(::ui_lang_runtime::ActionRequest),
__AccessibilityWindow(::crate::shell::WindowKey, ::iced::window::Event),
#[cfg(all(any(target_os = "windows", target_os = "macos"), not(test)))]
__AccessibilityNativeWindow(::ui_lang_runtime::NativeWindow),
__AccessibilityFocusNext(::std::option::Option<::crate::shell::WindowKey>),
__AccessibilityFocusPrevious(::std::option::Option<::crate::shell::WindowKey>),
__TemplateChanged,
#[cfg(all(target_os = "macos", not(test)))]
__AccessibilityWindowNative(::ui_lang_runtime::NativeWindow),
#[cfg(all(target_os = "macos", not(test)))]
__AccessibilityWindowSnapshot(::crate::shell::WindowKey, ::std::boxed::Box<::ui_lang_runtime::Snapshot<__DucktapeMessage>>),
#[cfg(all(target_os = "macos", not(test)))]
__AccessibilityWindowAction(::crate::shell::WindowKey, ::ui_lang_runtime::ActionRequest),
#[cfg(all(target_os = "macos", not(test)))]
__AccessibilityWindowEvent(::crate::shell::WindowKey, ::iced::window::Event),
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
__RequestLane10(u64, ::std::option::Option<::std::boxed::Box<__DucktapeMessage>>),
__RequestLane11(u64, ::std::boxed::Box<__DucktapeMessage>),
__RequestLane12(u64, ::std::boxed::Box<__DucktapeMessage>),
__RequestLane13(u64, ::std::boxed::Box<__DucktapeMessage>),
__RequestLane14(u64, ::std::boxed::Box<__DucktapeMessage>),
__RequestLane15(u64, ::std::boxed::Box<__DucktapeMessage>),
__RequestLane16(u64, ::std::boxed::Box<__DucktapeMessage>),
__RequestLane17(u64, ::std::boxed::Box<__DucktapeMessage>),
__RequestLane18(u64, ::std::boxed::Box<__DucktapeMessage>),
__RequestLane19(u64, ::std::boxed::Box<__DucktapeMessage>),
__RequestLane20(u64, ::std::option::Option<::std::boxed::Box<__DucktapeMessage>>),
__RequestLane21(u64, ::std::boxed::Box<__DucktapeMessage>),
__RequestLane22(u64, ::std::boxed::Box<__DucktapeMessage>),
__RequestLane23(u64, ::std::boxed::Box<__DucktapeMessage>),
__RequestLane24(u64, ::std::option::Option<::std::boxed::Box<__DucktapeMessage>>),
__RequestLane25(u64, ::std::boxed::Box<__DucktapeMessage>),
__RequestLane26(u64, ::std::option::Option<::std::boxed::Box<__DucktapeMessage>>),
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
WindowWasClosed(::crate::shell::WindowKey),
TrayOpen,
TrayQuit,
ModifierStateChanged(gpui_kit::Modifiers),
DragLaunchWindow,
CloseLaunchWindow,
WindowFocused(::crate::shell::WindowKey),
WindowUnfocused(::crate::shell::WindowKey),
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
ForgeNoteFailed(::std::string::String, ::std::string::String, crate::backend::OptimisticMutationError),
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
BellContextLoaded(i64, ::std::string::String, ::std::vec::Vec<crate::backend::BellPresentation>),
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
ComposerSubmitted(ComposerKind, ::std::string::String, ::std::string::String, ::std::string::String),
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
OnboardingOpened(::crate::shell::WindowKey),
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
ConsoleOpened(::crate::shell::WindowKey),
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
OnboardingReopened(::crate::shell::WindowKey),
DismissAccountBanner,
OpenAccountWelcome,
WelcomeReopened(::crate::shell::WindowKey),
CallEvent(crate::call::CallEvent),
ToggleCallMute,
ToggleCallCamera,
ToggleCallScreen,
ShowHuddle,
HuddleOpened(::crate::shell::WindowKey),
HuddleGoChannel,
LeaveHuddleHere,
HuddleLeft(bool),
__0C57616c6c6574526f77B7077(::std::string::String, ::std::string::String),
__0C50617373776f726453637265656eB7077(::std::string::String, ::std::string::String),
__0C50617373776f726453637265656eB707732(::std::string::String, ::std::string::String),
__0C436f6e6669726d50687261736553637265656eB616e73776572(::std::string::String, ::std::string::String),
__0C526573746f726553637265656eB6e616d655f6472616674(::std::string::String, ::std::string::String),
__0C526573746f726553637265656eB7077(::std::string::String, ::std::string::String),
__0C4e6574776f726b7353637265656eB72656d6f7465(::std::string::String, ::std::string::String),
__SecretTyped(::std::string::String, ::std::string::String),
__BindWelcomeNameDraft(::std::string::String),
__BindChannelDraft(::std::string::String),
__BindPaletteDraft(::std::string::String),
__ExternNoop,
}
impl ::std::fmt::Debug for __DucktapeMessage { fn fmt(&self, __formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result { __formatter.write_str("__DucktapeMessage") } }
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_LoadRequest(_value: &crate::backend::LoadRequest) {
let _: &::std::string::String = &_value.rpc;
let _: &::std::string::String = &_value.key;
let _: &i64 = &_value.generation;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_ChatChannel(_value: &crate::backend::ChatChannel) {
let _: &::std::string::String = &_value.id;
let _: &::std::string::String = &_value.name;
let _: &bool = &_value.archived;
let _: &bool = &_value.members_only;
let _: &i64 = &_value.huddle_count;
let _: &i64 = &_value.head_seq;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_ChatReaction(_value: &crate::backend::ChatReaction) {
let _: &::std::string::String = &_value.emoji;
let _: &i64 = &_value.count;
let _: &bool = &_value.reacted_by_me;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_ChatMember(_value: &crate::backend::ChatMember) {
let _: &::std::string::String = &_value.key;
let _: &::std::string::String = &_value.label;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_ChannelRead(_value: &crate::backend::ChannelRead) {
let _: &::std::string::String = &_value.channel;
let _: &i64 = &_value.seq;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_ChannelSwitchFacts(_value: &crate::backend::ChannelSwitchFacts) {
let _: &i64 = &_value.unread_boundary;
let _: &::std::string::String = &_value.name;
let _: &bool = &_value.archived;
let _: &bool = &_value.members_only;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_ChatSpan(_value: &crate::backend::ChatSpan) {
let _: &::std::string::String = &_value.mention;
let _: &::std::string::String = &_value.mention_link;
let _: &::std::string::String = &_value.link_text;
let _: &::std::string::String = &_value.link;
let _: &::std::string::String = &_value.bold_italic;
let _: &::std::string::String = &_value.bold;
let _: &::std::string::String = &_value.italic;
let _: &::std::string::String = &_value.plain;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_ChatBlock(_value: &crate::backend::ChatBlock) {
let _: &::std::string::String = &_value.kind;
let _: &::std::string::String = &_value.text;
let _: &::std::string::String = &_value.lang;
let _: &bool = &_value.rich;
let _: &::std::vec::Vec<crate::backend::ChatSpan> = &_value.spans;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_ChatMessage(_value: &crate::backend::ChatMessage) {
let _: &::std::string::String = &_value.id;
let _: &i64 = &_value.view_key;
let _: &i64 = &_value.seq;
let _: &::std::string::String = &_value.author;
let _: &::std::string::String = &_value.meta;
let _: &::std::string::String = &_value.body;
let _: &::std::string::String = &_value.edit_body;
let _: &::std::vec::Vec<crate::backend::ChatBlock> = &_value.blocks;
let _: &bool = &_value.pending;
let _: &i64 = &_value.rev;
let _: &bool = &_value.edited;
let _: &bool = &_value.deleted;
let _: &i64 = &_value.reply_count;
let _: &i64 = &_value.thread_seq;
let _: &bool = &_value.show_author;
let _: &::std::string::String = &_value.initial;
let _: &::std::string::String = &_value.avatar_kind;
let _: &i64 = &_value.height;
let _: &i64 = &_value.time;
let _: &::std::vec::Vec<crate::backend::ChatReaction> = &_value.reactions;
let _: &i64 = &_value.render_rev;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_HuddleParticipant(_value: &crate::backend::HuddleParticipant) {
let _: &::std::string::String = &_value.key;
let _: &::std::string::String = &_value.label;
let _: &::std::string::String = &_value.initials;
let _: &bool = &_value.is_agent;
let _: &bool = &_value.is_you;
let _: &i64 = &_value.joined_at;
let _: &::std::string::String = &_value.node;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_ChatData(_value: &crate::backend::ChatData) {
let _: &i64 = &_value.generation;
let _: &::std::vec::Vec<crate::backend::ChatChannel> = &_value.channels;
let _: &::std::string::String = &_value.active_channel;
let _: &::std::string::String = &_value.active_channel_name;
let _: &bool = &_value.active_channel_archived;
let _: &bool = &_value.active_channel_members_only;
let _: &::std::vec::Vec<crate::backend::HuddleParticipant> = &_value.huddle_roster;
let _: &::std::vec::Vec<crate::backend::ChatMember> = &_value.channel_members;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_PendingSend(_value: &crate::backend::PendingSend) {
let _: &::std::string::String = &_value.id;
let _: &::std::string::String = &_value.body;
let _: &i64 = &_value.thread_seq;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_SendReceipt(_value: &crate::backend::SendReceipt) {
let _: &::std::string::String = &_value.operation_id;
let _: &::std::string::String = &_value.channel_id;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_ChatDelta(_value: &crate::backend::ChatDelta) {
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_LiveRefresh(_value: &crate::backend::LiveRefresh) {
let _: &i64 = &_value.generation;
let _: &bool = &_value.chat_loaded;
let _: &::std::vec::Vec<crate::backend::ChatChannel> = &_value.channels;
let _: &::std::string::String = &_value.active_channel;
let _: &::std::string::String = &_value.active_channel_name;
let _: &bool = &_value.active_channel_archived;
let _: &bool = &_value.active_channel_members_only;
let _: &::std::vec::Vec<crate::backend::HuddleParticipant> = &_value.huddle_roster;
let _: &::std::vec::Vec<crate::backend::ChatMember> = &_value.channel_members;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_ChatSearchHit(_value: &crate::backend::ChatSearchHit) {
let _: &::std::string::String = &_value.channel_id;
let _: &i64 = &_value.seq;
let _: &i64 = &_value.root_seq;
let _: &::std::string::String = &_value.author;
let _: &::std::string::String = &_value.text;
let _: &::std::string::String = &_value.meta;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_PageSearchHit(_value: &crate::backend::PageSearchHit) {
let _: &::std::string::String = &_value.page_id;
let _: &::std::string::String = &_value.page_title;
let _: &::std::string::String = &_value.block_id;
let _: &::std::string::String = &_value.kind;
let _: &::std::string::String = &_value.text;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_PageSearchData(_value: &crate::backend::PageSearchData) {
let _: &::std::vec::Vec<crate::backend::PageSearchHit> = &_value.hits;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_PaletteSearchData(_value: &crate::backend::PaletteSearchData) {
let _: &::std::vec::Vec<crate::backend::ChatSearchHit> = &_value.chat_hits;
let _: &::std::vec::Vec<crate::backend::PageSearchHit> = &_value.page_hits;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_WorkspaceData(_value: &crate::backend::WorkspaceData) {
let _: &i64 = &_value.generation;
let _: &::std::string::String = &_value.rpc;
let _: &::std::string::String = &_value.status;
let _: &i64 = &_value.height;
let _: &::std::vec::Vec<crate::backend::ChatChannel> = &_value.channels;
let _: &::std::string::String = &_value.active_channel;
let _: &::std::string::String = &_value.active_channel_name;
let _: &bool = &_value.active_channel_archived;
let _: &bool = &_value.active_channel_members_only;
let _: &::std::vec::Vec<crate::backend::HuddleParticipant> = &_value.huddle_roster;
let _: &::std::vec::Vec<crate::backend::ChatMember> = &_value.channel_members;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_BellItem(_value: &crate::backend::BellItem) {
let _: &i64 = &_value.seq;
let _: &i64 = &_value.change_seq;
let _: &::std::string::String = &_value.source;
let _: &::std::string::String = &_value.reason;
let _: &::std::string::String = &_value.kind;
let _: &::std::string::String = &_value.actor;
let _: &i64 = &_value.height;
let _: &bool = &_value.read;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_BellDelta(_value: &crate::backend::BellDelta) {
let _: &::std::string::String = &_value.kind;
let _: &crate::backend::BellItem = &_value.item;
let _: &i64 = &_value.up_to_seq;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_BellPresentation(_value: &crate::backend::BellPresentation) {
let _: &i64 = &_value.seq;
let _: &::std::string::String = &_value.title;
let _: &::std::string::String = &_value.detail;
let _: &BellTarget = &_value.target;
let _: &::std::string::String = &_value.object;
let _: &i64 = &_value.number;
let _: &::std::string::String = &_value.anchor;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_BellData(_value: &crate::backend::BellData) {
let _: &i64 = &_value.unread;
let _: &::std::vec::Vec<crate::backend::BellItem> = &_value.items;
let _: &::std::vec::Vec<crate::backend::BellPresentation> = &_value.presentations;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_LiveUpdate(_value: &crate::backend::LiveUpdate) {
let _: &LiveKind = &_value.kind;
let _: &::std::string::String = &_value.status;
let _: &i64 = &_value.height;
let _: &::std::string::String = &_value.module;
let _: &bool = &_value.load_chat;
let _: &bool = &_value.debounce;
let _: &::std::vec::Vec<crate::backend::ChatDelta> = &_value.chat;
let _: &crate::backend::BellDelta = &_value.bell;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_ChatLiveFold(_value: &crate::backend::ChatLiveFold) {
let _: &::std::vec::Vec<crate::backend::ChatChannel> = &_value.channels;
let _: &::std::vec::Vec<crate::backend::ChatMember> = &_value.channel_members;
let _: &::std::vec::Vec<crate::backend::ChannelRead> = &_value.channel_reads;
let _: &::std::vec::Vec<crate::backend::ChatSidebarRow> = &_value.rooms;
let _: &::std::vec::Vec<crate::backend::DmSidebarRow> = &_value.dm_rows;
let _: &::std::string::String = &_value.active_channel_name;
let _: &bool = &_value.active_channel_archived;
let _: &bool = &_value.active_channel_members_only;
let _: &::std::string::String = &_value.post_refusal;
let _: &bool = &_value.refresh_chat;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_AppError(_value: &crate::backend::AppError) {
let _: &::std::string::String = &_value.message;
let _: &bool = &_value.committed;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_LiveActivity(_value: &crate::backend::LiveActivity) {
let _: &::std::string::String = &_value.label;
let _: &bool = &_value.done;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_LiveAgentRow(_value: &crate::backend::LiveAgentRow) {
let _: &::std::string::String = &_value.channel_id;
let _: &i64 = &_value.anchor_seq;
let _: &i64 = &_value.thread_root;
let _: &::std::string::String = &_value.run_id;
let _: &::std::string::String = &_value.dispatch_id;
let _: &::std::string::String = &_value.agent;
let _: &::std::string::String = &_value.status;
let _: &::std::vec::Vec<crate::backend::LiveActivity> = &_value.activity;
let _: &::std::string::String = &_value.answer_preview;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_LiveAgentNotice(_value: &crate::backend::LiveAgentNotice) {
let _: &::std::string::String = &_value.rpc;
let _: &::std::string::String = &_value.chain_id;
let _: &i64 = &_value.generation;
let _: &::std::string::String = &_value.signer_key;
let _: &::std::vec::Vec<crate::backend::LiveAgentRow> = &_value.rows;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_OptimisticMutationError(_value: &crate::backend::OptimisticMutationError) {
let _: &::std::string::String = &_value.message;
let _: &bool = &_value.committed;
let _: &::std::string::String = &_value.operation_id;
let _: &::std::string::String = &_value.scope_id;
let _: &i64 = &_value.thread_seq;
let _: &::std::string::String = &_value.body;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_HydrationError(_value: &crate::backend::HydrationError) {
let _: &i64 = &_value.generation;
let _: &::std::string::String = &_value.message;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_WorkspaceInit(_value: &crate::backend::WorkspaceInit) {
let _: &::std::string::String = &_value.chain_id;
let _: &::std::string::String = &_value.workspace;
let _: &::std::string::String = &_value.rpc;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_ProvisionStep(_value: &crate::backend::ProvisionStep) {
let _: &i64 = &_value.index;
let _: &::std::string::String = &_value.label;
let _: &::std::string::String = &_value.state;
let _: &bool = &_value.settled;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_HubNetwork(_value: &crate::backend::HubNetwork) {
let _: &::std::string::String = &_value.id;
let _: &::std::string::String = &_value.chain_id;
let _: &::std::string::String = &_value.name;
let _: &::std::string::String = &_value.endpoint;
let _: &::std::string::String = &_value.kind;
let _: &i64 = &_value.last_used;
let _: &bool = &_value.probed;
let _: &bool = &_value.live;
let _: &i64 = &_value.height;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_HubProbe(_value: &crate::backend::HubProbe) {
let _: &::std::string::String = &_value.id;
let _: &bool = &_value.live;
let _: &i64 = &_value.height;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_WalletInfo(_value: &crate::backend::WalletInfo) {
let _: &::std::string::String = &_value.name;
let _: &::std::string::String = &_value.pubkey;
let _: &::std::string::String = &_value.state;
let _: &bool = &_value.active;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_HubState(_value: &crate::backend::HubState) {
let _: &::std::vec::Vec<crate::backend::HubNetwork> = &_value.networks;
let _: &::std::string::String = &_value.preselect;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_WalletList(_value: &crate::backend::WalletList) {
let _: &::std::vec::Vec<crate::backend::WalletInfo> = &_value.wallets;
let _: &::std::string::String = &_value.error;
let _: &bool = &_value.keystore;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_PhraseRow(_value: &crate::backend::PhraseRow) {
let _: &::std::string::String = &_value.left_number;
let _: &::std::string::String = &_value.left_word;
let _: &::std::string::String = &_value.right_number;
let _: &::std::string::String = &_value.right_word;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_NavItem(_value: &crate::backend::NavItem) {
let _: &ShellTab = &_value.id;
let _: &::std::string::String = &_value.title;
let _: &::std::string::String = &_value.icon;
let _: &i64 = &_value.badge;
let _: &bool = &_value.active;
let _: &bool = &_value.live;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_DuckLink(_value: &crate::backend::DuckLink) {
let _: &DuckKind = &_value.kind;
let _: &::std::string::String = &_value.repo;
let _: &i64 = &_value.number;
let _: &i64 = &_value.seq;
let _: &::std::string::String = &_value.page;
let _: &::std::string::String = &_value.block;
let _: &::std::string::String = &_value.dispatch;
let _: &::std::string::String = &_value.channel;
let _: &::std::string::String = &_value.path;
let _: &::std::string::String = &_value.rev;
let _: &::std::string::String = &_value.account;
let _: &::std::string::String = &_value.net;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_NodeFacts(_value: &crate::backend::NodeFacts) {
let _: &::std::string::String = &_value.public_key;
let _: &::std::string::String = &_value.version;
let _: &::std::string::String = &_value.root_hash;
let _: &::std::string::String = &_value.chain_id;
let _: &::std::option::Option<i64> = &_value.view;
let _: &::std::option::Option<i64> = &_value.quorum;
let _: &::std::option::Option<i64> = &_value.reachable_validators;
let _: &i64 = &_value.last_finalized_at;
let _: &i64 = &_value.checkpoint_height;
let _: &i64 = &_value.height;
let _: &::std::string::String = &_value.phase;
let _: &i64 = &_value.phase_since;
let _: &i64 = &_value.sync_target;
let _: &i64 = &_value.sync_applied;
let _: &i64 = &_value.sync_retries;
let _: &i64 = &_value.sync_failures;
let _: &::std::string::String = &_value.sync_last_error;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_AccountData(_value: &crate::backend::AccountData) {
let _: &i64 = &_value.generation;
let _: &bool = &_value.exists;
let _: &::std::string::String = &_value.number;
let _: &::std::string::String = &_value.name;
let _: &::std::string::String = &_value.bio;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_CeremonyStep(_value: &crate::backend::CeremonyStep) {
let _: &::std::string::String = &_value.phase;
let _: &::std::string::String = &_value.qr;
let _: &::std::string::String = &_value.detail;
let _: &::std::string::String = &_value.left;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_SettingsFacts(_value: &crate::backend::SettingsFacts) {
let _: &i64 = &_value.generation;
let _: &::std::string::String = &_value.key_path;
let _: &::std::string::String = &_value.key_state;
let _: &::std::string::String = &_value.data_dir;
let _: &::std::string::String = &_value.user_key;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_MemberRow(_value: &crate::backend::MemberRow) {
let _: &::std::string::String = &_value.key;
let _: &::std::string::String = &_value.label;
let _: &::std::string::String = &_value.role;
let _: &bool = &_value.is_this_node;
let _: &bool = &_value.is_agent;
let _: &::std::string::String = &_value.model;
let _: &bool = &_value.live;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_MembersData(_value: &crate::backend::MembersData) {
let _: &i64 = &_value.generation;
let _: &::std::vec::Vec<crate::backend::MemberRow> = &_value.members;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_ChatSidebarRow(_value: &crate::backend::ChatSidebarRow) {
let _: &crate::backend::ChatChannel = &_value.channel;
let _: &bool = &_value.unread;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_DmSidebarRow(_value: &crate::backend::DmSidebarRow) {
let _: &crate::backend::DmPeer = &_value.peer;
let _: &bool = &_value.unread;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_HuddleAfterLoad(_value: &crate::backend::HuddleAfterLoad) {
let _: &bool = &_value.joined;
let _: &::std::vec::Vec<crate::backend::HuddleParticipant> = &_value.roster;
let _: &::std::string::String = &_value.channel;
let _: &::std::string::String = &_value.channel_name;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_DmPeer(_value: &crate::backend::DmPeer) {
let _: &::std::string::String = &_value.key;
let _: &::std::string::String = &_value.name;
let _: &::std::string::String = &_value.initials;
let _: &bool = &_value.is_agent;
let _: &::std::string::String = &_value.channel_id;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_DmPeersData(_value: &crate::backend::DmPeersData) {
let _: &i64 = &_value.generation;
let _: &::std::vec::Vec<crate::backend::DmPeer> = &_value.peers;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_CallEvent(_value: &crate::call::CallEvent) {
let _: &::std::string::String = &_value.kind;
let _: &::std::string::String = &_value.message;
let _: &::std::string::String = &_value.peer;
let _: &bool = &_value.muted;
let _: &bool = &_value.camera_on;
let _: &bool = &_value.sharing;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_HuddleTileRow(_value: &crate::call::HuddleTileRow) {
let _: &crate::backend::HuddleParticipant = &_value.person;
let _: &bool = &_value.muted;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_VideoSource(_value: &crate::video::VideoSource) {
let _: &bool = &_value.camera;
let _: &bool = &_value.sharing;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_ModuleViewEvent(_value: &crate::module_view::ModuleViewEvent) {
let _: &::std::string::String = &_value.kind;
let _: &::std::string::String = &_value.detail;
}
#[allow(dead_code)] fn __ui_lang_check_pure_load_request(arg0: bool, arg1: ::std::string::String, arg2: ::std::string::String, arg3: i64) { let _: ::std::option::Option<crate::backend::LoadRequest> = crate::backend::load_request(arg0, arg1, arg2, arg3); }
#[allow(dead_code)] fn __ui_lang_check_pure_send_pending(arg0: ::std::vec::Vec<crate::backend::PendingSend>, arg1: ::std::string::String, arg2: ::std::string::String, arg3: i64) { let _: ::std::vec::Vec<crate::backend::PendingSend> = crate::backend::send_pending(arg0, arg1, arg2, arg3); }
#[allow(dead_code)] fn __ui_lang_check_pure_send_settled<'a>(arg0: ::std::vec::Vec<crate::backend::PendingSend>, arg1: &'a str) { let _: ::std::vec::Vec<crate::backend::PendingSend> = crate::backend::send_settled(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_send_failed<'a>(arg0: ::std::vec::Vec<crate::backend::PendingSend>, arg1: &'a str, arg2: bool) { let _: ::std::vec::Vec<crate::backend::PendingSend> = crate::backend::send_failed(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_apply_bell(arg0: ::std::vec::Vec<crate::backend::BellItem>, arg1: crate::backend::BellDelta) { let _: ::std::vec::Vec<crate::backend::BellItem> = crate::backend::apply_bell(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_bell_account_items<'a>(arg0: ::std::vec::Vec<crate::backend::BellItem>, arg1: &'a str, arg2: &'a str) { let _: ::std::vec::Vec<crate::backend::BellItem> = crate::backend::bell_account_items(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_bell_unread_count<'a>(arg0: &'a [crate::backend::BellItem], arg1: &'a str, arg2: &'a str) { let _: i64 = crate::backend::bell_unread_count(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_bell_visible_items<'a>(arg0: &'a [crate::backend::BellItem], arg1: &'a str, arg2: &'a str) { let _: ::std::vec::Vec<crate::backend::BellItem> = crate::backend::bell_visible_items(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_merge_bell_loaded(arg0: ::std::vec::Vec<crate::backend::BellItem>, arg1: ::std::vec::Vec<crate::backend::BellItem>, arg2: i64, arg3: i64) { let _: ::std::vec::Vec<crate::backend::BellItem> = crate::backend::merge_bell_loaded(arg0, arg1, arg2, arg3); }
#[allow(dead_code)] fn __ui_lang_check_pure_merge_bell_presentations(arg0: ::std::vec::Vec<crate::backend::BellItem>, arg1: ::std::vec::Vec<crate::backend::BellPresentation>, arg2: ::std::vec::Vec<crate::backend::BellPresentation>) { let _: ::std::vec::Vec<crate::backend::BellPresentation> = crate::backend::merge_bell_presentations(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_bell_link<'a>(arg0: &'a crate::backend::BellPresentation, arg1: ::std::string::String) { let _: ::std::string::String = crate::backend::bell_link(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_bell_missing_items<'a>(arg0: ::std::vec::Vec<crate::backend::BellItem>, arg1: &'a [crate::backend::BellPresentation]) { let _: ::std::vec::Vec<crate::backend::BellItem> = crate::backend::bell_missing_items(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_bell_label<'a>(arg0: &'a crate::backend::BellItem, arg1: &'a [crate::backend::BellPresentation]) { let _: ::std::string::String = crate::backend::bell_label(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_bell_openable<'a>(arg0: &'a crate::backend::BellItem, arg1: &'a [crate::backend::BellPresentation]) { let _: bool = crate::backend::bell_openable(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_bell_presentation<'a>(arg0: &'a crate::backend::BellItem, arg1: &'a [crate::backend::BellPresentation]) { let _: crate::backend::BellPresentation = crate::backend::bell_presentation(arg0, arg1); }
#[allow(dead_code)] async fn __ui_lang_check_future_load_bell_presentations(arg0: ::std::string::String, arg1: ::std::vec::Vec<crate::backend::BellItem>) { let _: ::std::result::Result<::std::vec::Vec<crate::backend::BellPresentation>, crate::backend::AppError> = crate::backend::load_bell_presentations(arg0, arg1).await; }
#[allow(dead_code)] fn __ui_lang_check_pure_bell_head(arg0: ::std::vec::Vec<crate::backend::BellItem>) { let _: i64 = crate::backend::bell_head(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_bell_severity<'a>(arg0: &'a str) { let _: ::std::string::String = crate::backend::bell_severity(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_bell_title<'a>(arg0: &'a str) { let _: ::std::string::String = crate::backend::bell_title(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_bell_worst_severity<'a>(arg0: &'a [crate::backend::BellItem]) { let _: ::std::string::String = crate::backend::bell_worst_severity(arg0); }
#[allow(dead_code)] async fn __ui_lang_check_future_load_bell(arg0: ::std::string::String, arg1: ::std::string::String) { let _: ::std::result::Result<crate::backend::BellData, crate::backend::AppError> = crate::backend::load_bell(arg0, arg1).await; }
#[allow(dead_code)] async fn __ui_lang_check_future_mark_bell_read(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::std::string::String, arg3: i64) { let _: ::std::result::Result<crate::backend::BellDelta, crate::backend::AppError> = crate::backend::mark_bell_read(arg0, arg1, arg2, arg3).await; }
#[allow(dead_code)] fn __ui_lang_check_task_note_window_focus(arg0: bool) { let _: ::ducktape_view_guest::Task<()> = crate::backend::note_window_focus(arg0); }
#[allow(dead_code)] fn __ui_lang_check_container_style_raised_style(theme: &::iced::Theme) { let _: ::iced::widget::container::Style = crate::backend::raised_style(theme); }
#[allow(dead_code)] fn __ui_lang_check_pure_icon<'a>(arg0: &'a str) { let _: ::std::vec::Vec<u8> = crate::backend::icon(arg0); }
#[allow(dead_code)] async fn __ui_lang_check_future_connect(arg0: ::std::string::String, arg1: i64, arg2: i64) { let _: ::std::result::Result<crate::backend::WorkspaceData, crate::backend::HydrationError> = crate::backend::connect(arg0, arg1, arg2).await; }
#[allow(dead_code)] fn __ui_lang_check_stream_live_events(arg0: ::std::string::String) { let _: ::ducktape_view_guest::Task<crate::backend::LiveUpdate> = ::ducktape_view_guest::Task::run(crate::backend::live_events(arg0), |value| value); }
#[allow(dead_code)] fn __ui_lang_check_pure_fold_live_chat(arg0: ::std::vec::Vec<crate::backend::ChatDelta>, arg1: ::std::vec::Vec<crate::backend::ChatChannel>, arg2: ::std::vec::Vec<crate::backend::ChatMember>, arg3: ::std::vec::Vec<crate::backend::ChannelRead>, arg4: ::std::vec::Vec<crate::backend::DmPeer>, arg5: ::std::string::String, arg6: ::std::string::String, arg7: bool, arg8: bool, arg9: ::std::string::String, arg10: bool, arg11: bool) { let _: crate::backend::ChatLiveFold = crate::backend::fold_live_chat(arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10, arg11); }
#[allow(dead_code)] async fn __ui_lang_check_future_live_resync_load(arg0: ::std::string::String, arg1: ::std::string::String, arg2: bool, arg3: bool, arg4: i64, arg5: i64) { let _: ::std::result::Result<crate::backend::LiveRefresh, crate::backend::HydrationError> = crate::backend::live_resync_load(arg0, arg1, arg2, arg3, arg4, arg5).await; }
#[allow(dead_code)] fn __ui_lang_check_sync_fresh_operation_id(arg0: ::std::string::String) { let _: ::std::string::String = crate::backend::fresh_operation_id(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_restore_draft(arg0: ::std::string::String, arg1: ::std::string::String, arg2: bool) { let _: ::std::string::String = crate::backend::restore_draft(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_block_action_menu_y(arg0: f64, arg1: f64) { let _: f64 = crate::backend::block_action_menu_y(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_remember_failed_draft(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::std::string::String, arg3: bool) { let _: ::std::string::String = crate::backend::remember_failed_draft(arg0, arg1, arg2, arg3); }
#[allow(dead_code)] fn __ui_lang_check_sync_canonical_endpoint(arg0: ::std::string::String) { let _: ::std::string::String = crate::backend::canonical_endpoint(arg0); }
#[allow(dead_code)] async fn __ui_lang_check_future_join_network(arg0: ::ui_lang_runtime::Secret) { let _: ::std::result::Result<crate::backend::WorkspaceInit, crate::backend::AppError> = crate::backend::join_network(arg0).await; }
#[allow(dead_code)] async fn __ui_lang_check_future_mint_invite(arg0: ::std::string::String) { let _: ::std::result::Result<::std::string::String, crate::backend::AppError> = crate::backend::mint_invite(arg0).await; }
#[allow(dead_code)] fn __ui_lang_check_stream_provision_progress(arg0: ::std::string::String, arg1: ::std::string::String) { let _: ::ducktape_view_guest::Task<crate::backend::ProvisionStep> = ::ducktape_view_guest::Task::run(crate::backend::provision_progress(arg0, arg1), |value| value); }
#[allow(dead_code)] async fn __ui_lang_check_future_hub_state() { let _: crate::backend::HubState = crate::backend::hub_state().await; }
#[allow(dead_code)] async fn __ui_lang_check_future_load_wallets(arg0: ::std::string::String) { let _: crate::backend::WalletList = crate::backend::load_wallets(arg0).await; }
#[allow(dead_code)] fn __ui_lang_check_pure_wallet_door<'a>(arg0: &'a crate::backend::WalletList) { let _: WalletDoor = crate::backend::wallet_door(arg0); }
#[allow(dead_code)] fn __ui_lang_check_stream_probe_known_networks() { let _: ::ducktape_view_guest::Task<crate::backend::HubProbe> = ::ducktape_view_guest::Task::run(crate::backend::probe_known_networks(), |value| value); }
#[allow(dead_code)] fn __ui_lang_check_pure_apply_network_probe(arg0: ::std::vec::Vec<crate::backend::HubNetwork>, arg1: crate::backend::HubProbe) { let _: ::std::vec::Vec<crate::backend::HubNetwork> = crate::backend::apply_network_probe(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_network_run_hint<'a>(arg0: &'a crate::backend::HubNetwork) { let _: ::std::string::String = crate::backend::network_run_hint(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_hub_entry_step(arg0: ::std::vec::Vec<crate::backend::WalletInfo>) { let _: HubStep = crate::backend::hub_entry_step(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_preselect_wallet(arg0: ::std::vec::Vec<crate::backend::WalletInfo>) { let _: ::std::string::String = crate::backend::preselect_wallet(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_short_pubkey<'a>(arg0: &'a str) { let _: ::std::string::String = crate::backend::short_pubkey(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_wallet_caption<'a>(arg0: &'a str) { let _: ::std::string::String = crate::backend::wallet_caption(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_password_caption<'a>(arg0: &'a str) { let _: ::std::string::String = crate::backend::password_caption(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_wallet_info(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::std::string::String, arg3: bool) { let _: crate::backend::WalletInfo = crate::backend::wallet_info(arg0, arg1, arg2, arg3); }
#[allow(dead_code)] fn __ui_lang_check_pure_wallet_list(arg0: ::std::vec::Vec<crate::backend::WalletInfo>, arg1: ::std::string::String, arg2: bool) { let _: crate::backend::WalletList = crate::backend::wallet_list(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_selected_network_endpoint(arg0: ::std::vec::Vec<crate::backend::HubNetwork>, arg1: ::std::string::String) { let _: ::std::string::String = crate::backend::selected_network_endpoint(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_selected_network_name(arg0: ::std::vec::Vec<crate::backend::HubNetwork>, arg1: ::std::string::String) { let _: ::std::string::String = crate::backend::selected_network_name(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_refreshed_hub_selection(arg0: ::std::vec::Vec<crate::backend::HubNetwork>, arg1: ::std::string::String, arg2: ::std::string::String) { let _: ::std::string::String = crate::backend::refreshed_hub_selection(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_password_problem<'a>(arg0: &'a str, arg1: &'a str) { let _: ::std::string::String = crate::backend::password_problem(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_without_window(arg0: ::std::option::Option<::crate::shell::WindowKey>, arg1: ::crate::shell::WindowKey) { let _: ::std::option::Option<::crate::shell::WindowKey> = crate::backend::without_window(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_ceremony_retirement(arg0: bool, arg1: bool) { let _: CeremonyRetirement = crate::backend::ceremony_retirement(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_last_window_closed_exits(arg0: ::std::option::Option<::crate::shell::WindowKey>, arg1: ::std::option::Option<::crate::shell::WindowKey>) { let _: bool = crate::backend::last_window_closed_exits(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_tray_open_action(arg0: bool, arg1: bool) { let _: TrayOpen = crate::backend::tray_open_action(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_huddle_summon(arg0: ::std::option::Option<::crate::shell::WindowKey>) { let _: WindowSummon = crate::backend::huddle_summon(arg0); }
#[allow(dead_code)] fn __ui_lang_check_sync_window_target(arg0: ::std::option::Option<::crate::shell::WindowKey>) { let _: ::crate::shell::WindowKey = crate::backend::window_target(arg0); }
#[allow(dead_code)] fn __ui_lang_check_sync_window_target_unless(arg0: bool, arg1: ::std::option::Option<::crate::shell::WindowKey>) { let _: ::crate::shell::WindowKey = crate::backend::window_target_unless(arg0, arg1); }
#[allow(dead_code)] async fn __ui_lang_check_future_create_device_key(arg0: ::std::string::String, arg1: ::std::string::String) { let _: ::std::result::Result<::std::string::String, crate::backend::AppError> = crate::backend::create_device_key(arg0, arg1).await; }
#[allow(dead_code)] fn __ui_lang_check_pure_phrase_rows() { let _: ::std::vec::Vec<crate::backend::PhraseRow> = crate::backend::phrase_rows(); }
#[allow(dead_code)] fn __ui_lang_check_pure_phrase_rows_of<'a>(arg0: &'a str) { let _: ::std::vec::Vec<crate::backend::PhraseRow> = crate::backend::phrase_rows_of(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_recovery_prompt() { let _: ::std::string::String = crate::backend::recovery_prompt(); }
#[allow(dead_code)] async fn __ui_lang_check_future_confirm_recovery_phrase(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::std::string::String) { let _: ::std::result::Result<::std::string::String, crate::backend::AppError> = crate::backend::confirm_recovery_phrase(arg0, arg1, arg2).await; }
#[allow(dead_code)] async fn __ui_lang_check_future_restore_user_key(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::ui_lang_runtime::Secret, arg3: ::std::string::String) { let _: ::std::result::Result<::std::string::String, crate::backend::AppError> = crate::backend::restore_user_key(arg0, arg1, arg2, arg3).await; }
#[allow(dead_code)] async fn __ui_lang_check_future_unlock_wallet(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::std::string::String) { let _: ::std::result::Result<::std::string::String, crate::backend::AppError> = crate::backend::unlock_wallet(arg0, arg1, arg2).await; }
#[allow(dead_code)] async fn __ui_lang_check_future_unlock_user_key(arg0: ::std::string::String, arg1: ::std::string::String) { let _: ::std::result::Result<::std::string::String, crate::backend::AppError> = crate::backend::unlock_user_key(arg0, arg1).await; }
#[allow(dead_code)] async fn __ui_lang_check_future_lock_signer() { let _: bool = crate::backend::lock_signer().await; }
#[allow(dead_code)] async fn __ui_lang_check_future_remember_network(arg0: ::std::string::String) { let _: bool = crate::backend::remember_network(arg0).await; }
#[allow(dead_code)] async fn __ui_lang_check_future_forget_network(arg0: ::std::string::String) { let _: bool = crate::backend::forget_network(arg0).await; }
#[allow(dead_code)] fn __ui_lang_check_pure_connection_degraded<'a>(arg0: &'a str) { let _: bool = crate::backend::connection_degraded(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_titlebar_inset() { let _: f64 = crate::backend::titlebar_inset(); }
#[allow(dead_code)] fn __ui_lang_check_pure_palette_key_action(arg0: ::iced::keyboard::Key, arg1: ::iced::keyboard::key::Physical, arg2: ::iced::keyboard::Modifiers, arg3: bool) { let _: ::std::string::String = crate::backend::palette_key_action(arg0, arg1, arg2, arg3); }
#[allow(dead_code)] fn __ui_lang_check_pure_topmost_overlay(arg0: bool, arg1: bool, arg2: bool) { let _: ::std::string::String = crate::backend::topmost_overlay(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_escape_target(arg0: ::iced::keyboard::Key, arg1: bool, arg2: bool, arg3: bool) { let _: ::std::string::String = crate::backend::escape_target(arg0, arg1, arg2, arg3); }
#[allow(dead_code)] fn __ui_lang_check_pure_command_held(arg0: ::iced::keyboard::Modifiers) { let _: bool = crate::backend::command_held(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_shift_held(arg0: ::iced::keyboard::Modifiers) { let _: bool = crate::backend::shift_held(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_is_copy_chord(arg0: ::iced::keyboard::Key, arg1: ::iced::keyboard::key::Physical, arg2: ::iced::keyboard::Modifiers) { let _: bool = crate::backend::is_copy_chord(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_command_chord(arg0: ::iced::keyboard::Key, arg1: ::iced::keyboard::key::Physical, arg2: ::iced::keyboard::Modifiers) { let _: CommandChord = crate::backend::command_chord(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_resolve_duck_link(arg0: ::std::string::String, arg1: ::std::string::String) { let _: crate::backend::DuckLink = crate::backend::resolve_duck_link(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_foreign_network_error(arg0: ::std::string::String, arg1: ::std::string::String) { let _: ::std::string::String = crate::backend::foreign_network_error(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_duck_page_link(arg0: ::std::string::String, arg1: ::std::string::String) { let _: ::std::string::String = crate::backend::duck_page_link(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_duck_channel_link(arg0: ::std::string::String, arg1: ::std::string::String) { let _: ::std::string::String = crate::backend::duck_channel_link(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_duck_channel_message_link(arg0: ::std::string::String, arg1: i64, arg2: ::std::string::String) { let _: ::std::string::String = crate::backend::duck_channel_message_link(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_startup_duck_url() { let _: ::std::string::String = crate::backend::startup_duck_url(); }
#[allow(dead_code)] fn __ui_lang_check_pure_scope_channel<'a>(arg0: &'a str, arg1: &'a str) { let _: ::std::string::String = crate::backend::scope_channel(arg0, arg1); }
#[allow(dead_code)] async fn __ui_lang_check_future_duck_echo_str(arg0: ::std::string::String) { let _: ::std::result::Result<::std::string::String, crate::backend::AppError> = crate::backend::duck_echo_str(arg0).await; }
#[allow(dead_code)] async fn __ui_lang_check_future_duck_echo_i64(arg0: i64) { let _: ::std::result::Result<i64, crate::backend::AppError> = crate::backend::duck_echo_i64(arg0).await; }
#[allow(dead_code)] async fn __ui_lang_check_future_duck_echo_f64(arg0: f64) { let _: ::std::result::Result<f64, crate::backend::AppError> = crate::backend::duck_echo_f64(arg0).await; }
#[allow(dead_code)] fn __ui_lang_check_pure_files_write_gate(arg0: ::std::string::String, arg1: ::std::string::String) { let _: ::std::string::String = crate::backend::files_write_gate(arg0, arg1); }
#[allow(dead_code)] async fn __ui_lang_check_future_files_upload(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::std::string::String, arg3: ::std::string::String) { let _: ::std::result::Result<bool, crate::backend::AppError> = crate::backend::files_upload(arg0, arg1, arg2, arg3).await; }
#[allow(dead_code)] fn __ui_lang_check_pure_shell_nav(arg0: ShellTab, arg1: i64, arg2: bool) { let _: ::std::vec::Vec<crate::backend::NavItem> = crate::backend::shell_nav(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_reading_pair<'a>(arg0: &'a str, arg1: &'a str) { let _: ::std::string::String = crate::backend::reading_pair(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_expires_in_blocks(arg0: i64, arg1: i64, arg2: i64) { let _: ::std::string::String = crate::backend::expires_in_blocks(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_relative_time(arg0: i64, arg1: i64) { let _: ::std::string::String = crate::backend::relative_time(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_sync_current_wall_seconds() { let _: i64 = crate::backend::current_wall_seconds(); }
#[allow(dead_code)] fn __ui_lang_check_pure_mmss(arg0: i64) { let _: ::std::string::String = crate::backend::mmss(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_network_label(arg0: ::std::string::String, arg1: ::std::string::String) { let _: ::std::string::String = crate::backend::network_label(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_tray_badge(arg0: i64) { let _: ::std::string::String = crate::backend::tray_badge(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_tray_tooltip(arg0: ::std::string::String, arg1: ::std::string::String) { let _: ::std::string::String = crate::backend::tray_tooltip(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_tray_bell_row(arg0: i64) { let _: ::std::string::String = crate::backend::tray_bell_row(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_tray_huddle_row(arg0: bool, arg1: ::std::string::String) { let _: ::std::string::String = crate::backend::tray_huddle_row(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_tray_choice_row(arg0: ::std::string::String, arg1: bool) { let _: ::std::string::String = crate::backend::tray_choice_row(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_height_label(arg0: i64) { let _: ::std::string::String = crate::backend::height_label(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_height_label_short(arg0: i64) { let _: ::std::string::String = crate::backend::height_label_short(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_height_ago(arg0: i64, arg1: i64, arg2: i64) { let _: ::std::string::String = crate::backend::height_ago(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_initial_of<'a>(arg0: &'a str) { let _: ::std::string::String = crate::backend::initial_of(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_initials_of<'a>(arg0: &'a str) { let _: ::std::string::String = crate::backend::initials_of(arg0); }
#[allow(dead_code)] async fn __ui_lang_check_future_load_node_facts(arg0: ::std::string::String) { let _: ::std::result::Result<crate::backend::NodeFacts, crate::backend::AppError> = crate::backend::load_node_facts(arg0).await; }
#[allow(dead_code)] fn __ui_lang_check_pure_optional_number(arg0: ::std::option::Option<i64>) { let _: ::std::string::String = crate::backend::optional_number(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_sync_label<'a>(arg0: &'a str, arg1: i64, arg2: i64) { let _: ::std::string::String = crate::backend::sync_label(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_stream_node_status_live(arg0: ::std::string::String) { let _: ::ducktape_view_guest::Task<crate::backend::NodeFacts> = ::ducktape_view_guest::Task::run(crate::backend::node_status_live(arg0), |value| value); }
#[allow(dead_code)] async fn __ui_lang_check_future_load_account(arg0: ::std::string::String, arg1: i64) { let _: ::std::result::Result<crate::backend::AccountData, crate::backend::HydrationError> = crate::backend::load_account(arg0, arg1).await; }
#[allow(dead_code)] async fn __ui_lang_check_future_chain_id_of(arg0: ::std::string::String) { let _: ::std::result::Result<::std::string::String, crate::backend::AppError> = crate::backend::chain_id_of(arg0).await; }
#[allow(dead_code)] fn __ui_lang_check_pure_account_data_none(arg0: i64) { let _: crate::backend::AccountData = crate::backend::account_data_none(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_account_probe(arg0: bool) { let _: AccountProbe = crate::backend::account_probe(arg0); }
#[allow(dead_code)] async fn __ui_lang_check_future_set_account_name(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::std::string::String) { let _: ::std::result::Result<bool, crate::backend::AppError> = crate::backend::set_account_name(arg0, arg1, arg2).await; }
#[allow(dead_code)] async fn __ui_lang_check_future_create_account(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::std::string::String) { let _: ::std::result::Result<bool, crate::backend::AppError> = crate::backend::create_account(arg0, arg1, arg2).await; }
#[allow(dead_code)] async fn __ui_lang_check_future_mint_key_ticket(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::std::string::String, arg3: ::std::string::String, arg4: ::std::string::String) { let _: ::std::result::Result<::std::string::String, crate::backend::AppError> = crate::backend::mint_key_ticket(arg0, arg1, arg2, arg3, arg4).await; }
#[allow(dead_code)] async fn __ui_lang_check_future_join_with_ticket(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::std::string::String) { let _: ::std::result::Result<bool, crate::backend::AppError> = crate::backend::join_with_ticket(arg0, arg1, arg2).await; }
#[allow(dead_code)] async fn __ui_lang_check_future_remove_account_key(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::std::string::String) { let _: ::std::result::Result<bool, crate::backend::AppError> = crate::backend::remove_account_key(arg0, arg1, arg2).await; }
#[allow(dead_code)] async fn __ui_lang_check_future_register_passkey(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::std::string::String, arg3: ::std::string::String) { let _: ::std::result::Result<bool, crate::backend::AppError> = crate::backend::register_passkey(arg0, arg1, arg2, arg3).await; }
#[allow(dead_code)] async fn __ui_lang_check_future_link_wallet(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::std::string::String, arg3: ::std::string::String) { let _: ::std::result::Result<bool, crate::backend::AppError> = crate::backend::link_wallet(arg0, arg1, arg2, arg3).await; }
#[allow(dead_code)] async fn __ui_lang_check_future_login_with_passkey(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::std::string::String, arg3: ::std::string::String) { let _: ::std::result::Result<bool, crate::backend::AppError> = crate::backend::login_with_passkey(arg0, arg1, arg2, arg3).await; }
#[allow(dead_code)] fn __ui_lang_check_stream_create_account_by_qr(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::std::string::String, arg3: ::std::string::String) { let _: ::ducktape_view_guest::Task<crate::backend::CeremonyStep> = ::ducktape_view_guest::Task::run(crate::backend::create_account_by_qr(arg0, arg1, arg2, arg3), |value| value); }
#[allow(dead_code)] fn __ui_lang_check_stream_login_by_qr(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::std::string::String) { let _: ::ducktape_view_guest::Task<crate::backend::CeremonyStep> = ::ducktape_view_guest::Task::run(crate::backend::login_by_qr(arg0, arg1, arg2), |value| value); }
#[allow(dead_code)] fn __ui_lang_check_stream_add_passkey_by_qr(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::std::string::String, arg3: ::std::string::String) { let _: ::ducktape_view_guest::Task<crate::backend::CeremonyStep> = ::ducktape_view_guest::Task::run(crate::backend::add_passkey_by_qr(arg0, arg1, arg2, arg3), |value| value); }
#[allow(dead_code)] fn __ui_lang_check_pure_ceremony_step(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::std::string::String) { let _: crate::backend::CeremonyStep = crate::backend::ceremony_step(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_ceremony_phase<'a>(arg0: &'a crate::backend::CeremonyStep) { let _: CeremonyPhase = crate::backend::ceremony_phase(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_welcome_door<'a>(arg0: &'a str) { let _: WelcomeDoor = crate::backend::welcome_door(arg0); }
#[allow(dead_code)] async fn __ui_lang_check_future_load_settings_facts(arg0: ::std::string::String, arg1: i64) { let _: ::std::result::Result<crate::backend::SettingsFacts, crate::backend::HydrationError> = crate::backend::load_settings_facts(arg0, arg1).await; }
#[allow(dead_code)] fn __ui_lang_check_pure_picture_path(arg0: ::std::string::String) { let _: bool = crate::backend::picture_path(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_picture_caption(arg0: i64, arg1: i64) { let _: ::std::string::String = crate::backend::picture_caption(arg0, arg1); }
#[allow(dead_code)] async fn __ui_lang_check_future_set_agent_status(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::std::string::String, arg3: bool) { let _: ::std::result::Result<bool, crate::backend::AppError> = crate::backend::set_agent_status(arg0, arg1, arg2, arg3).await; }
#[allow(dead_code)] async fn __ui_lang_check_future_register_agent(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::std::string::String, arg3: ::std::string::String) { let _: ::std::result::Result<bool, crate::backend::AppError> = crate::backend::register_agent(arg0, arg1, arg2, arg3).await; }
#[allow(dead_code)] async fn __ui_lang_check_future_load_members(arg0: ::std::string::String, arg1: i64) { let _: ::std::result::Result<crate::backend::MembersData, crate::backend::HydrationError> = crate::backend::load_members(arg0, arg1).await; }
#[allow(dead_code)] fn __ui_lang_check_pure_members_is_admin<'a>(arg0: &'a [crate::backend::MemberRow]) { let _: bool = crate::backend::members_is_admin(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_member_tier<'a>(arg0: &'a [crate::backend::MemberRow]) { let _: ::std::string::String = crate::backend::member_tier(arg0); }
#[allow(dead_code)] async fn __ui_lang_check_future_load_appearance() { let _: Appearance = crate::backend::load_appearance().await; }
#[allow(dead_code)] async fn __ui_lang_check_future_save_appearance(arg0: Appearance) { let _: bool = crate::backend::save_appearance(arg0).await; }
#[allow(dead_code)] async fn __ui_lang_check_future_load_desktop_notifications() { let _: bool = crate::backend::load_desktop_notifications().await; }
#[allow(dead_code)] async fn __ui_lang_check_future_save_desktop_notifications(arg0: bool) { let _: bool = crate::backend::save_desktop_notifications(arg0).await; }
#[allow(dead_code)] fn __ui_lang_check_pure_mutation_failure_phase(arg0: bool) { let _: MutationPhase = crate::backend::mutation_failure_phase(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_mutation_phase_after_recovery(arg0: MutationPhase) { let _: MutationPhase = crate::backend::mutation_phase_after_recovery(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_message_seq_after_failure(arg0: i64, arg1: MutationPhase, arg2: bool) { let _: i64 = crate::backend::message_seq_after_failure(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_channel_last_read(arg0: ::std::vec::Vec<crate::backend::ChannelRead>, arg1: ::std::string::String) { let _: i64 = crate::backend::channel_last_read(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_channel_head_seq(arg0: ::std::vec::Vec<crate::backend::ChatChannel>, arg1: ::std::string::String) { let _: i64 = crate::backend::channel_head_seq(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_mark_channel_read(arg0: ::std::vec::Vec<crate::backend::ChannelRead>, arg1: ::std::string::String, arg2: i64) { let _: ::std::vec::Vec<crate::backend::ChannelRead> = crate::backend::mark_channel_read(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_chat_sidebar_rooms(arg0: ::std::vec::Vec<crate::backend::ChatChannel>, arg1: ::std::vec::Vec<crate::backend::DmPeer>, arg2: ::std::vec::Vec<crate::backend::ChannelRead>) { let _: ::std::vec::Vec<crate::backend::ChatSidebarRow> = crate::backend::chat_sidebar_rooms(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_chat_sidebar_dms(arg0: ::std::vec::Vec<crate::backend::ChatChannel>, arg1: ::std::vec::Vec<crate::backend::DmPeer>, arg2: ::std::vec::Vec<crate::backend::ChannelRead>) { let _: ::std::vec::Vec<crate::backend::DmSidebarRow> = crate::backend::chat_sidebar_dms(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_channel_switch_facts(arg0: ::std::vec::Vec<crate::backend::ChannelRead>, arg1: ::std::vec::Vec<crate::backend::ChatChannel>, arg2: ::std::string::String, arg3: ::std::string::String, arg4: i64, arg5: ::std::string::String) { let _: crate::backend::ChannelSwitchFacts = crate::backend::channel_switch_facts(arg0, arg1, arg2, arg3, arg4, arg5); }
#[allow(dead_code)] fn __ui_lang_check_pure_upsert_channel_rows(arg0: ::std::vec::Vec<crate::backend::ChatChannel>, arg1: ::std::vec::Vec<crate::backend::ChatChannel>) { let _: ::std::vec::Vec<crate::backend::ChatChannel> = crate::backend::upsert_channel_rows(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_near_scroll_top(arg0: f64) { let _: bool = crate::backend::near_scroll_top(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_near_scroll_tail(arg0: f64) { let _: bool = crate::backend::near_scroll_tail(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_submit_verdict(arg0: bool, arg1: bool, arg2: ::std::string::String, arg3: ::std::string::String, arg4: bool, arg5: ::std::string::String, arg6: ::std::string::String) { let _: SubmitVerdict = crate::backend::submit_verdict(arg0, arg1, arg2, arg3, arg4, arg5, arg6); }
#[allow(dead_code)] fn __ui_lang_check_pure_composer_op_prefix(arg0: ComposerKind) { let _: ::std::string::String = crate::backend::composer_op_prefix(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_composer_scope<'a>(arg0: &'a str, arg1: &'a str) { let _: ::std::string::String = crate::backend::composer_scope(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_thread_scope<'a>(arg0: &'a str, arg1: &'a str, arg2: i64) { let _: ::std::string::String = crate::backend::thread_scope(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_edit_scope<'a>(arg0: &'a str, arg1: &'a str, arg2: i64) { let _: ::std::string::String = crate::backend::edit_scope(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_scope_thread_seq<'a>(arg0: &'a str) { let _: i64 = crate::backend::scope_thread_seq(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_keep_channels(arg0: bool, arg1: bool, arg2: ::std::vec::Vec<crate::backend::ChatChannel>, arg3: ::std::vec::Vec<crate::backend::ChatChannel>) { let _: ::std::vec::Vec<crate::backend::ChatChannel> = crate::backend::keep_channels(arg0, arg1, arg2, arg3); }
#[allow(dead_code)] fn __ui_lang_check_pure_chain_moved(arg0: ::std::string::String, arg1: ::std::string::String) { let _: bool = crate::backend::chain_moved(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_keep_members(arg0: bool, arg1: ::std::vec::Vec<crate::backend::ChatMember>, arg2: ::std::vec::Vec<crate::backend::ChatMember>) { let _: ::std::vec::Vec<crate::backend::ChatMember> = crate::backend::keep_members(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_search_answer_stands<'a>(arg0: &'a str, arg1: &'a str, arg2: bool) { let _: bool = crate::backend::search_answer_stands(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_plane_live_hit(arg0: LiveKind, arg1: ::std::string::String, arg2: ::std::string::String) { let _: bool = crate::backend::plane_live_hit(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_tab_reads_plane(arg0: ShellTab, arg1: ::std::string::String) { let _: bool = crate::backend::tab_reads_plane(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_keep_str<'a>(arg0: bool, arg1: &'a str, arg2: &'a str) { let _: ::std::string::String = crate::backend::keep_str(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_keep_bool(arg0: bool, arg1: bool, arg2: bool) { let _: bool = crate::backend::keep_bool(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_keep_i64(arg0: bool, arg1: i64, arg2: i64) { let _: i64 = crate::backend::keep_i64(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_initial_channel_reads(arg0: ::std::vec::Vec<crate::backend::ChatChannel>, arg1: ::std::vec::Vec<crate::backend::ChannelRead>) { let _: ::std::vec::Vec<crate::backend::ChannelRead> = crate::backend::initial_channel_reads(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_frozen_unread_boundary(arg0: ::std::vec::Vec<crate::backend::ChannelRead>, arg1: ::std::vec::Vec<crate::backend::ChatChannel>, arg2: ::std::string::String, arg3: ::std::string::String, arg4: i64) { let _: i64 = crate::backend::frozen_unread_boundary(arg0, arg1, arg2, arg3, arg4); }
#[allow(dead_code)] async fn __ui_lang_check_future_load_channel_window(arg0: ::std::string::String, arg1: ::std::string::String, arg2: i64) { let _: ::std::result::Result<crate::backend::ChatData, crate::backend::HydrationError> = crate::backend::load_channel_window(arg0, arg1, arg2).await; }
#[allow(dead_code)] async fn __ui_lang_check_future_create_channel(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::std::string::String, arg3: bool, arg4: i64) { let _: ::std::result::Result<crate::backend::ChatData, crate::backend::AppError> = crate::backend::create_channel(arg0, arg1, arg2, arg3, arg4).await; }
#[allow(dead_code)] async fn __ui_lang_check_future_join_huddle(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::std::string::String) { let _: ::std::result::Result<bool, crate::backend::AppError> = crate::backend::join_huddle(arg0, arg1, arg2).await; }
#[allow(dead_code)] async fn __ui_lang_check_future_leave_huddle(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::std::string::String) { let _: ::std::result::Result<bool, crate::backend::AppError> = crate::backend::leave_huddle(arg0, arg1, arg2).await; }
#[allow(dead_code)] fn __ui_lang_check_pure_huddle_after_load(arg0: bool, arg1: bool, arg2: ::std::string::String, arg3: ::std::string::String, arg4: ::std::vec::Vec<crate::backend::HuddleParticipant>, arg5: ::std::string::String, arg6: ::std::string::String, arg7: ::std::vec::Vec<crate::backend::HuddleParticipant>) { let _: crate::backend::HuddleAfterLoad = crate::backend::huddle_after_load(arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7); }
#[allow(dead_code)] async fn __ui_lang_check_future_load_dm_peers(arg0: ::std::string::String, arg1: i64) { let _: ::std::result::Result<crate::backend::DmPeersData, crate::backend::HydrationError> = crate::backend::load_dm_peers(arg0, arg1).await; }
#[allow(dead_code)] fn __ui_lang_check_pure_dm_room_of_peer(arg0: ::std::vec::Vec<crate::backend::DmPeer>, arg1: ::std::string::String) { let _: ::std::string::String = crate::backend::dm_room_of_peer(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_dm_peer_of_channel(arg0: ::std::string::String, arg1: ::std::vec::Vec<crate::backend::DmPeer>, arg2: ::std::string::String) { let _: ::std::string::String = crate::backend::dm_peer_of_channel(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_dm_peer_named(arg0: ::std::vec::Vec<crate::backend::DmPeer>, arg1: ::std::string::String) { let _: crate::backend::DmPeer = crate::backend::dm_peer_named(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_no_dm_peer() { let _: crate::backend::DmPeer = crate::backend::no_dm_peer(); }
#[allow(dead_code)] async fn __ui_lang_check_future_open_dm(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::std::string::String, arg3: i64) { let _: ::std::result::Result<crate::backend::ChatData, crate::backend::HydrationError> = crate::backend::open_dm(arg0, arg1, arg2, arg3).await; }
#[allow(dead_code)] fn __ui_lang_check_pure_post_gate(arg0: bool, arg1: bool, arg2: ::std::vec::Vec<crate::backend::ChatMember>, arg3: ::std::string::String) { let _: ::std::string::String = crate::backend::post_gate(arg0, arg1, arg2, arg3); }
#[allow(dead_code)] async fn __ui_lang_check_future_send_message(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::std::string::String, arg3: ::std::string::String, arg4: ::std::string::String) { let _: ::std::result::Result<crate::backend::SendReceipt, crate::backend::OptimisticMutationError> = crate::backend::send_message(arg0, arg1, arg2, arg3, arg4).await; }
#[allow(dead_code)] async fn __ui_lang_check_future_send_reply(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::std::string::String, arg3: i64, arg4: ::std::string::String, arg5: ::std::string::String) { let _: ::std::result::Result<crate::backend::SendReceipt, crate::backend::OptimisticMutationError> = crate::backend::send_reply(arg0, arg1, arg2, arg3, arg4, arg5).await; }
#[allow(dead_code)] async fn __ui_lang_check_future_edit_message(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::std::string::String, arg3: i64, arg4: i64, arg5: ::std::string::String) { let _: ::std::result::Result<bool, crate::backend::AppError> = crate::backend::edit_message(arg0, arg1, arg2, arg3, arg4, arg5).await; }
#[allow(dead_code)] async fn __ui_lang_check_future_cancel_agent_run(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::std::string::String) { let _: ::std::result::Result<bool, crate::backend::AppError> = crate::backend::cancel_agent_run(arg0, arg1, arg2).await; }
#[allow(dead_code)] fn __ui_lang_check_stream_chat_live_agents(arg0: ::std::string::String, arg1: ::std::string::String, arg2: i64, arg3: ::std::string::String) { let _: ::ducktape_view_guest::Task<crate::backend::LiveAgentNotice> = ::ducktape_view_guest::Task::run(crate::backend::chat_live_agents(arg0, arg1, arg2, arg3), |value| value); }
#[allow(dead_code)] fn __ui_lang_check_pure_live_agents_stale<'a>(arg0: &'a crate::backend::LiveAgentNotice, arg1: &'a str, arg2: &'a str, arg3: i64, arg4: &'a str) { let _: bool = crate::backend::live_agents_stale(arg0, arg1, arg2, arg3, arg4); }
#[allow(dead_code)] fn __ui_lang_check_pure_live_agent_row(arg0: ::std::string::String, arg1: i64, arg2: ::std::string::String, arg3: ::std::string::String, arg4: ::std::string::String) { let _: crate::backend::LiveAgentRow = crate::backend::live_agent_row(arg0, arg1, arg2, arg3, arg4); }
#[allow(dead_code)] async fn __ui_lang_check_future_open_external_url(arg0: ::std::string::String) { let _: ::std::result::Result<bool, crate::backend::AppError> = crate::backend::open_external_url(arg0).await; }
#[allow(dead_code)] fn __ui_lang_check_pure_count_label(arg0: i64) { let _: ::std::string::String = crate::backend::count_label(arg0); }
#[allow(dead_code)] async fn __ui_lang_check_future_search_pages(arg0: ::std::string::String, arg1: ::std::string::String, arg2: ::std::string::String) { let _: ::std::result::Result<crate::backend::PageSearchData, crate::backend::AppError> = crate::backend::search_pages(arg0, arg1, arg2).await; }
#[allow(dead_code)] async fn __ui_lang_check_future_palette_search(arg0: ::std::string::String, arg1: ::std::string::String) { let _: ::std::result::Result<crate::backend::PaletteSearchData, crate::backend::AppError> = crate::backend::palette_search(arg0, arg1).await; }
#[allow(dead_code)] fn __ui_lang_check_stream_call_session(arg0: ::std::string::String, arg1: ::std::string::String) { let _: ::ducktape_view_guest::Task<crate::call::CallEvent> = ::ducktape_view_guest::Task::run(crate::call::call_session(arg0, arg1), |value| value); }
#[allow(dead_code)] fn __ui_lang_check_sync_call_set_muted(arg0: bool) { let _: bool = crate::call::call_set_muted(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_call_status_after(arg0: ::std::string::String, arg1: crate::call::CallEvent) { let _: ::std::string::String = crate::call::call_status_after(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_apply_call_peer(arg0: ::std::vec::Vec<crate::call::CallEvent>, arg1: crate::call::CallEvent) { let _: ::std::vec::Vec<crate::call::CallEvent> = crate::call::apply_call_peer(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_huddle_tile_rows(arg0: ::std::vec::Vec<crate::backend::HuddleParticipant>, arg1: ::std::vec::Vec<crate::call::CallEvent>, arg2: bool) { let _: ::std::vec::Vec<crate::call::HuddleTileRow> = crate::call::huddle_tile_rows(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_call_video_live_after(arg0: ::std::vec::Vec<crate::call::CallEvent>, arg1: bool, arg2: bool) { let _: bool = crate::call::call_video_live_after(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_huddle_stage_peer(arg0: ::std::vec::Vec<crate::call::CallEvent>, arg1: bool) { let _: ::std::string::String = crate::call::huddle_stage_peer(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_sync_call_use_camera(arg0: bool) { let _: crate::video::VideoSource = crate::video::call_use_camera(arg0); }
#[allow(dead_code)] fn __ui_lang_check_sync_call_use_screen(arg0: bool) { let _: crate::video::VideoSource = crate::video::call_use_screen(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_view_event(arg0: ::std::string::String, arg1: ::std::string::String) { let _: crate::module_view::ModuleViewEvent = crate::module_view::view_event(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_sync_view_live_hit<'a>(arg0: &'a str, arg1: i64) { let _: i64 = crate::module_view::view_live_hit(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_sync_view_block_hit(arg0: i64, arg1: i64) { let _: i64 = crate::module_view::view_block_hit(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_agents_intent<'a>(arg0: &'a crate::module_view::ModuleViewEvent) { let _: AgentsIntent = crate::module_view::agents_intent(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_event_text<'a>(arg0: &'a crate::module_view::ModuleViewEvent, arg1: &'a str) { let _: ::std::string::String = crate::module_view::event_text(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_event_flag<'a>(arg0: &'a crate::module_view::ModuleViewEvent, arg1: &'a str) { let _: bool = crate::module_view::event_flag(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_settings_intent<'a>(arg0: &'a crate::module_view::ModuleViewEvent) { let _: SettingsIntent = crate::module_view::settings_intent(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_settings_event_tab<'a>(arg0: &'a crate::module_view::ModuleViewEvent) { let _: ShellTab = crate::module_view::settings_event_tab(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_pages_intent<'a>(arg0: &'a crate::module_view::ModuleViewEvent) { let _: PagesIntent = crate::module_view::pages_intent(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_forge_intent<'a>(arg0: &'a crate::module_view::ModuleViewEvent) { let _: ForgeIntent = crate::module_view::forge_intent(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_event_number<'a>(arg0: &'a crate::module_view::ModuleViewEvent, arg1: &'a str) { let _: i64 = crate::module_view::event_number(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_chat_intent<'a>(arg0: &'a crate::module_view::ModuleViewEvent) { let _: ChatIntent = crate::module_view::chat_intent(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_chat_event_kind<'a>(arg0: &'a crate::module_view::ModuleViewEvent) { let _: ComposerKind = crate::module_view::chat_event_kind(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_event_int<'a>(arg0: &'a crate::module_view::ModuleViewEvent, arg1: &'a str) { let _: i64 = crate::module_view::event_int(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_event_num<'a>(arg0: &'a crate::module_view::ModuleViewEvent, arg1: &'a str) { let _: f64 = crate::module_view::event_num(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_sync_chat_composer_unsent<'a>(arg0: &'a str, arg1: &'a str, arg2: bool) { let _: bool = crate::module_view::chat_composer_unsent(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_sync_chat_composer_seed<'a>(arg0: &'a str, arg1: &'a str) { let _: bool = crate::module_view::chat_composer_seed(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_sync_chat_composer_roster<'a>(arg0: &'a str, arg1: &'a [crate::backend::ChatMember]) { let _: bool = crate::module_view::chat_composer_roster(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_component_forge_markdown(arg0: ::std::string::String, arg1: ::std::string::String, arg2: bool) { let _: __IceElement<'static, ::std::string::String> = crate::backend::forge_markdown(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_component_forge_code(arg0: ::std::string::String, arg1: ::std::string::String, arg2: bool) { let _: __IceElement<'static, ()> = crate::backend::forge_code(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_component_picture(arg0: ::std::string::String, arg1: ::std::string::String) { let _: __IceElement<'static, ()> = crate::backend::picture(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_component_call_video_tiles<'a>(arg0: &'a str) { let _: __IceElement<'a, ()> = crate::video::call_video_tiles(arg0); }
#[allow(dead_code)] fn __ui_lang_check_component_call_video_stage<'a>(arg0: &'a str) { let _: __IceElement<'a, ()> = crate::video::call_video_stage(arg0); }
#[allow(dead_code)] fn __ui_lang_check_component_governance_view(arg0: bool, arg1: bool, arg2: bool) { let _: __IceElement<'static, crate::module_view::ModuleViewEvent> = crate::module_view::governance_view(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_component_members_view(arg0: bool, arg1: bool, arg2: bool) { let _: __IceElement<'static, crate::module_view::ModuleViewEvent> = crate::module_view::members_view(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_component_agents_view<'a>(arg0: bool, arg1: bool, arg2: &'a str, arg3: &'a str, arg4: i64) { let _: __IceElement<'a, crate::module_view::ModuleViewEvent> = crate::module_view::agents_view(arg0, arg1, arg2, arg3, arg4); }
#[allow(dead_code)] fn __ui_lang_check_component_node_view<'a>(arg0: bool, arg1: bool, arg2: bool, arg3: &'a str, arg4: &'a str, arg5: &'a str, arg6: i64) { let _: __IceElement<'a, crate::module_view::ModuleViewEvent> = crate::module_view::node_view(arg0, arg1, arg2, arg3, arg4, arg5, arg6); }
#[allow(dead_code)] fn __ui_lang_check_component_explorer_view<'a>(arg0: bool, arg1: bool, arg2: i64, arg3: &'a str) { let _: __IceElement<'a, crate::module_view::ModuleViewEvent> = crate::module_view::explorer_view(arg0, arg1, arg2, arg3); }
#[allow(dead_code)] fn __ui_lang_check_component_settings_view<'a>(arg0: bool, arg1: bool, arg2: bool, arg3: &'a str, arg4: MutationPhase, arg5: Appearance, arg6: bool, arg7: &'a str, arg8: &'a str, arg9: &'a str, arg10: &'a str, arg11: &'a str, arg12: &'a str, arg13: &'a str, arg14: &'a str, arg15: &'a str, arg16: &'a str, arg17: &'a str, arg18: &'a str, arg19: bool, arg20: bool, arg21: &'a str) { let _: __IceElement<'a, crate::module_view::ModuleViewEvent> = crate::module_view::settings_view(arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10, arg11, arg12, arg13, arg14, arg15, arg16, arg17, arg18, arg19, arg20, arg21); }
#[allow(dead_code)] fn __ui_lang_check_component_files_view<'a>(arg0: bool, arg1: bool, arg2: &'a str, arg3: &'a str, arg4: i64) { let _: __IceElement<'a, crate::module_view::ModuleViewEvent> = crate::module_view::files_view(arg0, arg1, arg2, arg3, arg4); }
#[allow(dead_code)] fn __ui_lang_check_component_pages_view<'a>(arg0: bool, arg1: bool, arg2: &'a str, arg3: &'a str, arg4: i64) { let _: __IceElement<'a, crate::module_view::ModuleViewEvent> = crate::module_view::pages_view(arg0, arg1, arg2, arg3, arg4); }
#[allow(dead_code)] fn __ui_lang_check_component_forge_view<'a>(arg0: bool, arg1: bool, arg2: &'a str, arg3: &'a str, arg4: &'a str, arg5: &'a str, arg6: &'a str, arg7: &'a str, arg8: i64) { let _: __IceElement<'a, crate::module_view::ModuleViewEvent> = crate::module_view::forge_view(arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8); }
#[allow(dead_code)] fn __ui_lang_check_component_chat_view<'a>(arg0: bool, arg1: bool, arg2: &'a str, arg3: &'a str, arg4: &'a str, arg5: &'a str, arg6: i64, arg7: &'a str, arg8: &'a str, arg9: i64, arg10: &'a [crate::backend::ChatSidebarRow], arg11: &'a [crate::backend::DmSidebarRow], arg12: bool, arg13: &'a str, arg14: &'a str, arg15: &'a crate::backend::DmPeer, arg16: i64, arg17: i64, arg18: MutationPhase, arg19: bool, arg20: bool, arg21: &'a str, arg22: &'a str, arg23: i64, arg24: i64, arg25: bool, arg26: bool, arg27: i64, arg28: i64, arg29: &'a [crate::backend::PendingSend], arg30: &'a [crate::backend::LiveAgentRow]) { let _: __IceElement<'a, crate::module_view::ModuleViewEvent> = crate::module_view::chat_view(arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10, arg11, arg12, arg13, arg14, arg15, arg16, arg17, arg18, arg19, arg20, arg21, arg22, arg23, arg24, arg25, arg26, arg27, arg28, arg29, arg30); }
}
__ice_generated_items_2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f6170702e696365! {
#[allow(unused_parens)]
impl Ducktape {
#[must_use]
pub fn default_font() -> ::iced::Font { ::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Normal, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal } }
fn __ice_derived_dark(&self) -> &bool { self.__ice_derived.dark.get_or_init(|| (self.appearance == Appearance::Dark)) }
fn __ice_derived_app_background(&self) -> &::std::string::String { self.__ice_derived.app_background.get_or_init(|| crate::backend::keep_str((self.appearance == Appearance::Dark), ::std::convert::AsRef::as_ref(&("#1b1a16")), ::std::convert::AsRef::as_ref(&("#fdfdfb")))) }
fn __ice_derived_app_text(&self) -> &::std::string::String { self.__ice_derived.app_text.get_or_init(|| crate::backend::keep_str((self.appearance == Appearance::Dark), ::std::convert::AsRef::as_ref(&("#e8e6df")), ::std::convert::AsRef::as_ref(&("#2c2b27")))) }
fn __ice_derived_has_error(&self) -> &bool { self.__ice_derived.has_error.get_or_init(|| (!(self.error).is_empty())) }
fn __ice_derived_mutation_busy(&self) -> &bool { self.__ice_derived.mutation_busy.get_or_init(|| (self.mutation_phase != MutationPhase::Idle)) }
fn __ice_derived_hub_busy(&self) -> &bool { self.__ice_derived.hub_busy.get_or_init(|| ((*self.__ice_derived_mutation_busy()) || ((self.console_entry == ConsoleEntry::Entering) && (self.onboarding_error).is_empty()))) }
fn __window_0() -> crate::shell::WindowKind { crate::shell::WindowKind::Onboarding }
fn __window_1() -> crate::shell::WindowKind { crate::shell::WindowKind::Console }
fn __window_2() -> crate::shell::WindowKind { crate::shell::WindowKind::Huddle }


}
}
__ice_generated_items_2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f6170702e696365! {
#[allow(unused_parens)]
impl Ducktape {
fn __palette(&self, window: ::crate::shell::WindowKey) -> __IcePalette {
let _ = &window;
match self.app_palette.clone() {
AppTheme::App => __IcePalette { name: "app", colors: [::iced::Color::from_rgba8(58, 56, 51, 1.000000), ::iced::Color::from_rgba8(212, 210, 202, 1.000000), ::iced::Color::from_rgba8(253, 253, 251, 1.000000), ::iced::Color::from_rgba8(255, 255, 255, 1.000000), ::iced::Color::from_rgba8(44, 43, 39, 1.000000), ::iced::Color::from_rgba8(107, 105, 98, 1.000000), ::iced::Color::from_rgba8(246, 245, 242, 1.000000), ::iced::Color::from_rgba8(38, 37, 31, 1.000000), ::iced::Color::from_rgba8(50, 47, 40, 1.000000), ::iced::Color::from_rgba8(255, 255, 255, 1.000000), ::iced::Color::from_rgba8(236, 235, 230, 1.000000), ::iced::Color::from_rgba8(179, 177, 168, 1.000000), ::iced::Color::from_rgba8(255, 255, 255, 1.000000), ::iced::Color::from_rgba8(94, 92, 85, 1.000000), ::iced::Color::from_rgba8(243, 242, 239, 1.000000), ::iced::Color::from_rgba8(63, 62, 57, 1.000000), ::iced::Color::from_rgba8(160, 90, 60, 1.000000), ::iced::Color::from_rgba8(255, 255, 255, 1.000000), ::iced::Color::from_rgba8(249, 241, 234, 1.000000), ::iced::Color::from_rgba8(231, 210, 196, 1.000000), ::iced::Color::from_rgba8(184, 84, 76, 1.000000), ::iced::Color::from_rgba8(255, 255, 255, 1.000000), ::iced::Color::from_rgba8(253, 244, 243, 1.000000), ::iced::Color::from_rgba8(239, 214, 211, 1.000000), ::iced::Color::from_rgba8(224, 101, 92, 1.000000), ::iced::Color::from_rgba8(95, 158, 116, 1.000000), ::iced::Color::from_rgba8(21, 20, 16, 1.000000), ::iced::Color::from_rgba8(238, 245, 240, 1.000000), ::iced::Color::from_rgba8(207, 227, 215, 1.000000), ::iced::Color::from_rgba8(92, 180, 95, 1.000000), ::iced::Color::from_rgba8(160, 123, 50, 1.000000), ::iced::Color::from_rgba8(21, 20, 16, 1.000000), ::iced::Color::from_rgba8(251, 244, 230, 1.000000), ::iced::Color::from_rgba8(236, 220, 174, 1.000000), ::iced::Color::from_rgba8(227, 180, 67, 1.000000), ::iced::Color::from_rgba8(210, 208, 199, 1.000000), ::iced::Color::from_rgba8(79, 77, 71, 1.000000), ::iced::Color::from_rgba8(38, 37, 31, 1.000000), ::iced::Color::from_rgba8(243, 241, 234, 1.000000), ::iced::Color::from_rgba8(231, 230, 226, 1.000000), ::iced::Color::from_rgba8(224, 223, 215, 1.000000), ::iced::Color::from_rgba8(138, 137, 131, 1.000000), ::iced::Color::from_rgba8(38, 37, 31, 1.000000), ::iced::Color::from_rgba8(253, 252, 250, 0.501961), ::iced::Color::from_rgba8(253, 252, 250, 0.619608), ::iced::Color::from_rgba8(253, 252, 250, 0.858824), ::iced::Color::from_rgba8(40, 38, 34, 0.129412), ::iced::Color::from_rgba8(40, 38, 34, 0.219608), ::iced::Color::from_rgba8(40, 38, 34, 0.301961), ::iced::Color::from_rgba8(40, 38, 34, 0.219608), ::iced::Color::from_rgba8(40, 38, 34, 0.101961), ::iced::Color::from_rgba8(227, 225, 217, 1.000000), ::iced::Color::from_rgba8(236, 234, 227, 1.000000), ::iced::Color::from_rgba8(250, 250, 248, 1.000000), ::iced::Color::from_rgba8(251, 251, 249, 1.000000), ::iced::Color::from_rgba8(243, 242, 239, 1.000000), ::iced::Color::from_rgba8(236, 235, 230, 1.000000), ::iced::Color::from_rgba8(248, 247, 243, 1.000000), ::iced::Color::from_rgba8(240, 239, 234, 1.000000), ::iced::Color::from_rgba8(214, 212, 204, 1.000000), ::iced::Color::from_rgba8(239, 238, 233, 1.000000), ::iced::Color::from_rgba8(9, 11, 14, 1.000000), ::iced::Color::from_rgba8(36, 42, 51, 1.000000), ::iced::Color::from_rgba8(236, 233, 225, 1.000000), ::iced::Color::from_rgba8(236, 214, 208, 1.000000), ::iced::Color::from_rgba8(253, 246, 244, 1.000000), ::iced::Color::from_rgba8(163, 82, 72, 1.000000), ::iced::Color::from_rgba8(143, 70, 61, 1.000000), ::iced::Color::from_rgba8(50, 47, 40, 1.000000), ::iced::Color::from_rgba8(58, 57, 52, 1.000000), ::iced::Color::from_rgba8(154, 152, 143, 1.000000), ::iced::Color::from_rgba8(167, 165, 155, 1.000000), ::iced::Color::from_rgba8(179, 177, 168, 1.000000), ::iced::Color::from_rgba8(189, 187, 177, 1.000000), ::iced::Color::from_rgba8(203, 201, 191, 1.000000), ::iced::Color::from_rgba8(123, 167, 140, 1.000000), ::iced::Color::from_rgba8(95, 122, 158, 1.000000), ::iced::Color::from_rgba8(238, 242, 247, 1.000000), ::iced::Color::from_rgba8(218, 226, 236, 1.000000), ::iced::Color::from_rgba8(127, 154, 184, 1.000000), ::iced::Color::from_rgba8(163, 82, 72, 1.000000), ::iced::Color::from_rgba8(251, 236, 234, 1.000000), ::iced::Color::from_rgba8(236, 207, 201, 1.000000), ::iced::Color::from_rgba8(207, 106, 94, 1.000000), ::iced::Color::from_rgba8(251, 248, 240, 1.000000), ::iced::Color::from_rgba8(40, 38, 34, 0.341176), ::iced::Color::from_rgba8(247, 246, 242, 1.000000), ::iced::Color::from_rgba8(250, 249, 246, 1.000000), ::iced::Color::from_rgba8(252, 251, 249, 1.000000), ::iced::Color::from_rgba8(251, 250, 247, 1.000000), ::iced::Color::from_rgba8(253, 248, 243, 1.000000), ::iced::Color::from_rgba8(240, 236, 225, 1.000000), ::iced::Color::from_rgba8(244, 231, 200, 1.000000), ::iced::Color::from_rgba8(217, 216, 208, 1.000000), ::iced::Color::from_rgba8(213, 211, 202, 1.000000), ::iced::Color::from_rgba8(182, 180, 168, 1.000000), ::iced::Color::from_rgba8(200, 198, 188, 1.000000), ::iced::Color::from_rgba8(194, 192, 182, 1.000000), ::iced::Color::from_rgba8(208, 206, 196, 1.000000), ::iced::Color::from_rgba8(220, 219, 212, 1.000000), ::iced::Color::from_rgba8(122, 120, 114, 1.000000), ::iced::Color::from_rgba8(126, 158, 136, 1.000000), ::iced::Color::from_rgba8(102, 100, 94, 1.000000), ::iced::Color::from_rgba8(122, 111, 158, 1.000000), ::iced::Color::from_rgba8(241, 237, 245, 1.000000), ::iced::Color::from_rgba8(221, 210, 230, 1.000000), ::iced::Color::from_rgba8(240, 245, 241, 1.000000), ::iced::Color::from_rgba8(220, 235, 224, 1.000000), ::iced::Color::from_rgba8(238, 246, 239, 1.000000), ::iced::Color::from_rgba8(225, 239, 227, 1.000000), ::iced::Color::from_rgba8(47, 107, 65, 1.000000), ::iced::Color::from_rgba8(251, 238, 236, 1.000000), ::iced::Color::from_rgba8(244, 221, 216, 1.000000), ::iced::Color::from_rgba8(161, 67, 56, 1.000000), ::iced::Color::from_rgba8(246, 243, 249, 1.000000), ::iced::Color::from_rgba8(74, 72, 67, 1.000000), ::iced::Color::from_rgba8(224, 145, 138, 1.000000), ::iced::Color::from_rgba8(160, 138, 90, 1.000000), ::iced::Color::from_rgba8(95, 138, 114, 1.000000), ::iced::Color::from_rgba8(237, 244, 239, 1.000000), ::iced::Color::from_rgba8(122, 111, 158, 1.000000), ::iced::Color::from_rgba8(241, 239, 247, 1.000000), ::iced::Color::from_rgba8(74, 72, 67, 1.000000), ::iced::Color::from_rgba8(242, 241, 237, 1.000000), ::iced::Color::from_rgba8(185, 113, 78, 1.000000), ::iced::Color::from_rgba8(250, 240, 233, 1.000000), ::iced::Color::from_rgba8(192, 138, 62, 1.000000), ::iced::Color::from_rgba8(250, 243, 230, 1.000000)] },
AppTheme::AppDark => __IcePalette { name: "app_dark", colors: [::iced::Color::from_rgba8(212, 210, 202, 1.000000), ::iced::Color::from_rgba8(69, 68, 60, 1.000000), ::iced::Color::from_rgba8(27, 26, 22, 1.000000), ::iced::Color::from_rgba8(34, 33, 29, 1.000000), ::iced::Color::from_rgba8(232, 230, 223, 1.000000), ::iced::Color::from_rgba8(168, 166, 156, 1.000000), ::iced::Color::from_rgba8(38, 37, 31, 1.000000), ::iced::Color::from_rgba8(232, 230, 223, 1.000000), ::iced::Color::from_rgba8(244, 242, 234, 1.000000), ::iced::Color::from_rgba8(27, 26, 22, 1.000000), ::iced::Color::from_rgba8(51, 50, 44, 1.000000), ::iced::Color::from_rgba8(107, 106, 97, 1.000000), ::iced::Color::from_rgba8(42, 41, 37, 1.000000), ::iced::Color::from_rgba8(181, 179, 169, 1.000000), ::iced::Color::from_rgba8(46, 45, 39, 1.000000), ::iced::Color::from_rgba8(207, 205, 196, 1.000000), ::iced::Color::from_rgba8(201, 138, 99, 1.000000), ::iced::Color::from_rgba8(27, 26, 22, 1.000000), ::iced::Color::from_rgba8(51, 38, 29, 1.000000), ::iced::Color::from_rgba8(74, 56, 43, 1.000000), ::iced::Color::from_rgba8(217, 123, 114, 1.000000), ::iced::Color::from_rgba8(27, 26, 22, 1.000000), ::iced::Color::from_rgba8(51, 33, 31, 1.000000), ::iced::Color::from_rgba8(77, 47, 44, 1.000000), ::iced::Color::from_rgba8(224, 101, 92, 1.000000), ::iced::Color::from_rgba8(127, 184, 148, 1.000000), ::iced::Color::from_rgba8(21, 20, 16, 1.000000), ::iced::Color::from_rgba8(30, 42, 34, 1.000000), ::iced::Color::from_rgba8(50, 71, 58, 1.000000), ::iced::Color::from_rgba8(92, 180, 95, 1.000000), ::iced::Color::from_rgba8(212, 169, 78, 1.000000), ::iced::Color::from_rgba8(21, 20, 16, 1.000000), ::iced::Color::from_rgba8(46, 39, 23, 1.000000), ::iced::Color::from_rgba8(77, 63, 34, 1.000000), ::iced::Color::from_rgba8(227, 180, 67, 1.000000), ::iced::Color::from_rgba8(58, 57, 49, 1.000000), ::iced::Color::from_rgba8(207, 205, 196, 1.000000), ::iced::Color::from_rgba8(243, 241, 234, 1.000000), ::iced::Color::from_rgba8(38, 37, 31, 1.000000), ::iced::Color::from_rgba8(53, 52, 46, 1.000000), ::iced::Color::from_rgba8(59, 58, 51, 1.000000), ::iced::Color::from_rgba8(133, 131, 123, 1.000000), ::iced::Color::from_rgba8(232, 230, 223, 1.000000), ::iced::Color::from_rgba8(27, 26, 22, 0.501961), ::iced::Color::from_rgba8(27, 26, 22, 0.619608), ::iced::Color::from_rgba8(27, 26, 22, 0.858824), ::iced::Color::from_rgba8(0, 0, 0, 0.250980), ::iced::Color::from_rgba8(0, 0, 0, 0.349020), ::iced::Color::from_rgba8(0, 0, 0, 0.450980), ::iced::Color::from_rgba8(0, 0, 0, 0.349020), ::iced::Color::from_rgba8(0, 0, 0, 0.149020), ::iced::Color::from_rgba8(18, 17, 16, 1.000000), ::iced::Color::from_rgba8(25, 24, 21, 1.000000), ::iced::Color::from_rgba8(32, 31, 27, 1.000000), ::iced::Color::from_rgba8(30, 29, 25, 1.000000), ::iced::Color::from_rgba8(42, 41, 37, 1.000000), ::iced::Color::from_rgba8(49, 48, 43, 1.000000), ::iced::Color::from_rgba8(36, 35, 30, 1.000000), ::iced::Color::from_rgba8(40, 39, 34, 1.000000), ::iced::Color::from_rgba8(14, 13, 11, 1.000000), ::iced::Color::from_rgba8(44, 43, 38, 1.000000), ::iced::Color::from_rgba8(9, 11, 14, 1.000000), ::iced::Color::from_rgba8(36, 42, 51, 1.000000), ::iced::Color::from_rgba8(48, 47, 41, 1.000000), ::iced::Color::from_rgba8(77, 47, 44, 1.000000), ::iced::Color::from_rgba8(42, 29, 27, 1.000000), ::iced::Color::from_rgba8(194, 90, 79, 1.000000), ::iced::Color::from_rgba8(211, 104, 92, 1.000000), ::iced::Color::from_rgba8(244, 242, 234, 1.000000), ::iced::Color::from_rgba8(220, 218, 210, 1.000000), ::iced::Color::from_rgba8(143, 141, 132, 1.000000), ::iced::Color::from_rgba8(124, 122, 113, 1.000000), ::iced::Color::from_rgba8(107, 106, 97, 1.000000), ::iced::Color::from_rgba8(96, 95, 86, 1.000000), ::iced::Color::from_rgba8(85, 84, 76, 1.000000), ::iced::Color::from_rgba8(123, 167, 140, 1.000000), ::iced::Color::from_rgba8(127, 154, 184, 1.000000), ::iced::Color::from_rgba8(30, 37, 48, 1.000000), ::iced::Color::from_rgba8(48, 62, 82, 1.000000), ::iced::Color::from_rgba8(127, 154, 184, 1.000000), ::iced::Color::from_rgba8(211, 104, 92, 1.000000), ::iced::Color::from_rgba8(48, 31, 28, 1.000000), ::iced::Color::from_rgba8(77, 47, 44, 1.000000), ::iced::Color::from_rgba8(207, 106, 94, 1.000000), ::iced::Color::from_rgba8(42, 37, 23, 1.000000), ::iced::Color::from_rgba8(0, 0, 0, 0.501961), ::iced::Color::from_rgba8(32, 31, 26, 1.000000), ::iced::Color::from_rgba8(35, 34, 29, 1.000000), ::iced::Color::from_rgba8(38, 37, 32, 1.000000), ::iced::Color::from_rgba8(38, 36, 24, 1.000000), ::iced::Color::from_rgba8(42, 34, 27, 1.000000), ::iced::Color::from_rgba8(53, 50, 42, 1.000000), ::iced::Color::from_rgba8(69, 58, 30, 1.000000), ::iced::Color::from_rgba8(63, 62, 54, 1.000000), ::iced::Color::from_rgba8(69, 68, 60, 1.000000), ::iced::Color::from_rgba8(110, 109, 99, 1.000000), ::iced::Color::from_rgba8(91, 90, 82, 1.000000), ::iced::Color::from_rgba8(98, 97, 90, 1.000000), ::iced::Color::from_rgba8(74, 73, 65, 1.000000), ::iced::Color::from_rgba8(51, 50, 44, 1.000000), ::iced::Color::from_rgba8(163, 161, 152, 1.000000), ::iced::Color::from_rgba8(126, 158, 136, 1.000000), ::iced::Color::from_rgba8(157, 155, 146, 1.000000), ::iced::Color::from_rgba8(168, 154, 201, 1.000000), ::iced::Color::from_rgba8(42, 38, 51, 1.000000), ::iced::Color::from_rgba8(68, 60, 87, 1.000000), ::iced::Color::from_rgba8(30, 42, 34, 1.000000), ::iced::Color::from_rgba8(50, 71, 58, 1.000000), ::iced::Color::from_rgba8(29, 42, 32, 1.000000), ::iced::Color::from_rgba8(36, 53, 42, 1.000000), ::iced::Color::from_rgba8(143, 201, 162, 1.000000), ::iced::Color::from_rgba8(47, 31, 28, 1.000000), ::iced::Color::from_rgba8(61, 39, 35, 1.000000), ::iced::Color::from_rgba8(222, 139, 127, 1.000000), ::iced::Color::from_rgba8(38, 35, 48, 1.000000), ::iced::Color::from_rgba8(46, 45, 40, 1.000000), ::iced::Color::from_rgba8(160, 92, 85, 1.000000), ::iced::Color::from_rgba8(192, 168, 110, 1.000000), ::iced::Color::from_rgba8(127, 184, 148, 1.000000), ::iced::Color::from_rgba8(30, 42, 34, 1.000000), ::iced::Color::from_rgba8(168, 154, 201, 1.000000), ::iced::Color::from_rgba8(42, 38, 51, 1.000000), ::iced::Color::from_rgba8(207, 205, 196, 1.000000), ::iced::Color::from_rgba8(46, 45, 40, 1.000000), ::iced::Color::from_rgba8(208, 144, 104, 1.000000), ::iced::Color::from_rgba8(51, 38, 29, 1.000000), ::iced::Color::from_rgba8(212, 169, 78, 1.000000), ::iced::Color::from_rgba8(46, 39, 23, 1.000000)] },
}
}
fn __app_theme(__ice_palette: __IcePalette) -> ::iced::Theme {
         ::std::thread_local! { static __ICE_APP_THEME: ::std::cell::RefCell<::std::option::Option<(__IcePalette, ::iced::Theme)>> = const { ::std::cell::RefCell::new(::std::option::Option::None) }; }
         __ICE_APP_THEME.with(|__cached| {
         if let ::std::option::Option::Some((__cached_palette, __cached_theme)) = &*__cached.borrow() {
         if __cached_palette.name == __ice_palette.name && __cached_palette.colors == __ice_palette.colors { return __cached_theme.clone(); }
         }
         let __theme =
::iced::Theme::custom(::std::format!("Ducktape/{}", __ice_palette.name), ::iced::theme::Palette {
background: __ice_palette.colors[2],
text: __ice_palette.colors[4],
primary: __ice_palette.colors[7],
success: __ice_palette.colors[7],
warning: __ice_palette.colors[20],
danger: __ice_palette.colors[20],
});
*__cached.borrow_mut() = ::std::option::Option::Some((__ice_palette, __theme.clone()));
__theme })
}
pub(crate) fn __theme(&self, window: ::crate::shell::WindowKey) -> ::iced::Theme {
Self::__app_theme(self.__palette(window))
}
fn __title(&self, window: ::crate::shell::WindowKey) -> ::std::string::String { "Ducktape".to_owned() }
fn __style(&self, __theme: &::iced::Theme) -> ::iced::theme::Style { let mut __style = ::iced::theme::Base::base(__theme);
__style.background_color = ((*self.__ice_derived_app_background()).to_owned()).parse::<::iced::Color>().unwrap_or(__style.background_color);
__style.text_color = ((*self.__ice_derived_app_text()).to_owned()).parse::<::iced::Color>().unwrap_or(__style.text_color);
__style }
}
}
__ice_generated_items_2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f6170702e696365! {
#[allow(unused_parens)]
impl Ducktape {
fn __state() -> Self {
Self {
__ice_accessibility: ::ui_lang_runtime::Bridge::without_native_adapter(),
#[cfg(all(target_os = "macos", not(test)))]
__ice_accessibility_windows: ::ui_lang_runtime::WindowBridges::new(),
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
__ice_rev: [::ui_lang_runtime::rev::seed(); 156],
__ice_secrets: ::std::default::Default::default(),
__ice_component_057616c6c6574526f77: ::std::collections::HashMap::new(),
__ice_component_057616c6c6574526f77_initial: ::std::default::Default::default(),
__ice_component_050617373776f726453637265656e: ::std::collections::HashMap::new(),
__ice_component_050617373776f726453637265656e_initial: ::std::default::Default::default(),
__ice_component_0436f6e6669726d50687261736553637265656e: ::std::collections::HashMap::new(),
__ice_component_0436f6e6669726d50687261736553637265656e_initial: ::std::default::Default::default(),
__ice_component_0526573746f726553637265656e: ::std::collections::HashMap::new(),
__ice_component_0526573746f726553637265656e_initial: ::std::default::Default::default(),
__ice_component_04e6574776f726b7353637265656e: ::std::collections::HashMap::new(),
__ice_component_04e6574776f726b7353637265656e_initial: ::std::default::Default::default(),
}
}
fn __boot_task(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
let task = (|| {
{ let (_, __task) = ::crate::shell::open(Self::__window_0()); __task.map(move |value| __DucktapeMessage::OnboardingOpened(value)) }
})();
task
}
pub(crate) fn __boot() -> (Self, ::ducktape_view_guest::Task<__DucktapeMessage>) {
let mut state = Self::__state();
{
const __ICE_TRAY_ICONS: &[::ui_lang_runtime::tray::TrayIcon] = &[::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-offline.rgba", rgba: { const __ICE_TRAY_RGBA_0: &[u8] = include_bytes!("../../assets/tray-offline.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_0.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_0 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-unread.rgba", rgba: { const __ICE_TRAY_RGBA_1: &[u8] = include_bytes!("../../assets/tray-unread.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_1.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_1 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray.rgba", rgba: { const __ICE_TRAY_RGBA_2: &[u8] = include_bytes!("../../assets/tray.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_2.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_2 }, width: 128u32, height: 128u32 },];
const __ICE_TRAY_ROWS: &[::ui_lang_runtime::tray::TrayRow] = &[::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 4usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },];
::ui_lang_runtime::tray::init(::ui_lang_runtime::tray::TrayConfig { icons: __ICE_TRAY_ICONS, rows: __ICE_TRAY_ROWS, icon_template: false, });
::ui_lang_runtime::tray::set_item(3usize, "Open Ducktape");
::ui_lang_runtime::tray::set_item(5usize, "Go to");
::ui_lang_runtime::tray::set_item(6usize, "Chat");
::ui_lang_runtime::tray::set_item(7usize, "Pages");
::ui_lang_runtime::tray::set_item(8usize, "Node");
::ui_lang_runtime::tray::set_item(9usize, "Settings");
::ui_lang_runtime::tray::set_item(13usize, "Leave huddle");
::ui_lang_runtime::tray::set_item(14usize, "Appearance");
::ui_lang_runtime::tray::set_item(18usize, "Copy node key");
::ui_lang_runtime::tray::set_item(19usize, "Reconnect");
::ui_lang_runtime::tray::set_item(21usize, "Quit Ducktape");
}
let task = state.__boot_task();
state.__tray_sync();
(state, task)
}
fn __tray_row(__row: usize) -> ::std::option::Option<__DucktapeMessage> { match __row { 3usize => ::std::option::Option::Some(__DucktapeMessage::TrayOpen),4usize => ::std::option::Option::Some(__DucktapeMessage::TrayOpenBell),6usize => ::std::option::Option::Some(__DucktapeMessage::TrayGoChat),7usize => ::std::option::Option::Some(__DucktapeMessage::TrayGoPages),8usize => ::std::option::Option::Some(__DucktapeMessage::TrayGoNode),9usize => ::std::option::Option::Some(__DucktapeMessage::TrayGoSettings),12usize => ::std::option::Option::Some(__DucktapeMessage::ToggleCallMute),13usize => ::std::option::Option::Some(__DucktapeMessage::LeaveHuddleHere),15usize => ::std::option::Option::Some(__DucktapeMessage::SetAppearanceLight),16usize => ::std::option::Option::Some(__DucktapeMessage::SetAppearanceDark),18usize => ::std::option::Option::Some(__DucktapeMessage::TrayCopyNodeKey),19usize => ::std::option::Option::Some(__DucktapeMessage::TrayReconnect),21usize => ::std::option::Option::Some(__DucktapeMessage::TrayQuit), _ => ::std::option::Option::None } }
fn __tray_sync(&self) {
::ui_lang_runtime::tray::select_icon(&[(!self.connected), (self.bell_unread > 0)]);
::ui_lang_runtime::tray::set_label(&(crate::backend::tray_badge(self.bell_unread)));
::ui_lang_runtime::tray::set_tooltip(&(crate::backend::tray_tooltip(self.network_name.to_owned(), self.status.to_owned())));
::ui_lang_runtime::tray::set_item(0usize, &(crate::backend::keep_str((self.network_name).is_empty(), ::std::convert::AsRef::as_ref(&("No network")), ::std::convert::AsRef::as_ref(&(self.network_name)))));
::ui_lang_runtime::tray::set_item(1usize, &(self.status.to_owned()));
::ui_lang_runtime::tray::set_item(4usize, &(crate::backend::tray_bell_row(self.bell_unread)));
::ui_lang_runtime::tray::set_item(11usize, &(crate::backend::tray_huddle_row(self.huddle_joined, self.huddle_channel_name.to_owned())));
::ui_lang_runtime::tray::set_item(12usize, &(crate::backend::keep_str(self.call_muted, ::std::convert::AsRef::as_ref(&("Unmute")), ::std::convert::AsRef::as_ref(&("Mute")))));
::ui_lang_runtime::tray::set_item(15usize, &(crate::backend::tray_choice_row("Light".to_owned(), (self.appearance == Appearance::Light))));
::ui_lang_runtime::tray::set_item(16usize, &(crate::backend::tray_choice_row("Dark".to_owned(), (self.appearance == Appearance::Dark))));
::ui_lang_runtime::tray::set_visible(4usize, (self.console_win != ::std::option::Option::None));
::ui_lang_runtime::tray::set_visible(5usize, (self.console_win != ::std::option::Option::None));
::ui_lang_runtime::tray::set_visible(11usize, self.huddle_joined);
::ui_lang_runtime::tray::set_visible(18usize, (self.console_win != ::std::option::Option::None));
::ui_lang_runtime::tray::set_visible(19usize, (self.console_win != ::std::option::Option::None));
}
}
}
__ice_generated_items_2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f6170702e696365! {
#[allow(unused_parens)]
impl Ducktape {
fn __preset_task_0(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
let task = (|| {
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.rpc, __ice_next) { self.rpc = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = "Offline".to_owned(); if ::ui_lang_runtime::state_changed!(self.status, __ice_next) { self.status = __ice_next; self.__ice_rev[7] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.connected, __ice_next) { self.connected = __ice_next; self.__ice_rev[8] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.loading, __ice_next) { self.loading = __ice_next; self.__ice_rev[9] += 1; } }
{ let __ice_next = MutationPhase::Idle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
{ let __ice_next = ShellTab::Chat; if ::ui_lang_runtime::state_changed!(self.shell_tab, __ice_next) { self.shell_tab = __ice_next; self.__ice_rev[55] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.channel_draft, __ice_next) { self.channel_draft = __ice_next; self.__ice_rev[43] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.channel_create_members_only, __ice_next) { self.channel_create_members_only = __ice_next; self.__ice_rev[45] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.palette_open, __ice_next) { self.palette_open = __ice_next; self.__ice_rev[104] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.palette_draft, __ice_next) { self.palette_draft = __ice_next; self.__ice_rev[113] += 1; } }
::ducktape_view_guest::Task::none()
})();
task
}
fn __preset_0() -> (Self, ::ducktape_view_guest::Task<__DucktapeMessage>) {
let mut state = Self::__state();
{
const __ICE_TRAY_ICONS: &[::ui_lang_runtime::tray::TrayIcon] = &[::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-offline.rgba", rgba: { const __ICE_TRAY_RGBA_0: &[u8] = include_bytes!("../../assets/tray-offline.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_0.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_0 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-unread.rgba", rgba: { const __ICE_TRAY_RGBA_1: &[u8] = include_bytes!("../../assets/tray-unread.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_1.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_1 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray.rgba", rgba: { const __ICE_TRAY_RGBA_2: &[u8] = include_bytes!("../../assets/tray.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_2.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_2 }, width: 128u32, height: 128u32 },];
const __ICE_TRAY_ROWS: &[::ui_lang_runtime::tray::TrayRow] = &[::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 4usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },];
::ui_lang_runtime::tray::init(::ui_lang_runtime::tray::TrayConfig { icons: __ICE_TRAY_ICONS, rows: __ICE_TRAY_ROWS, icon_template: false, });
::ui_lang_runtime::tray::set_item(3usize, "Open Ducktape");
::ui_lang_runtime::tray::set_item(5usize, "Go to");
::ui_lang_runtime::tray::set_item(6usize, "Chat");
::ui_lang_runtime::tray::set_item(7usize, "Pages");
::ui_lang_runtime::tray::set_item(8usize, "Node");
::ui_lang_runtime::tray::set_item(9usize, "Settings");
::ui_lang_runtime::tray::set_item(13usize, "Leave huddle");
::ui_lang_runtime::tray::set_item(14usize, "Appearance");
::ui_lang_runtime::tray::set_item(18usize, "Copy node key");
::ui_lang_runtime::tray::set_item(19usize, "Reconnect");
::ui_lang_runtime::tray::set_item(21usize, "Quit Ducktape");
}

let task = state.__preset_task_0();
state.__tray_sync();
(state, task)
}
fn __preset_task_1(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
let task = (|| {
{ let __ice_next = "Offline".to_owned(); if ::ui_lang_runtime::state_changed!(self.status, __ice_next) { self.status = __ice_next; self.__ice_rev[7] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.connected, __ice_next) { self.connected = __ice_next; self.__ice_rev[8] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.loading, __ice_next) { self.loading = __ice_next; self.__ice_rev[9] += 1; } }
{ let __ice_next = MutationPhase::Idle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = ShellTab::Chat; if ::ui_lang_runtime::state_changed!(self.shell_tab, __ice_next) { self.shell_tab = __ice_next; self.__ice_rev[55] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.palette_open, __ice_next) { self.palette_open = __ice_next; self.__ice_rev[104] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.palette_draft, __ice_next) { self.palette_draft = __ice_next; self.__ice_rev[113] += 1; } }
::ducktape_view_guest::Task::none()
})();
task
}
fn __preset_1() -> (Self, ::ducktape_view_guest::Task<__DucktapeMessage>) {
let mut state = Self::__state();
{
const __ICE_TRAY_ICONS: &[::ui_lang_runtime::tray::TrayIcon] = &[::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-offline.rgba", rgba: { const __ICE_TRAY_RGBA_0: &[u8] = include_bytes!("../../assets/tray-offline.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_0.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_0 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-unread.rgba", rgba: { const __ICE_TRAY_RGBA_1: &[u8] = include_bytes!("../../assets/tray-unread.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_1.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_1 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray.rgba", rgba: { const __ICE_TRAY_RGBA_2: &[u8] = include_bytes!("../../assets/tray.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_2.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_2 }, width: 128u32, height: 128u32 },];
const __ICE_TRAY_ROWS: &[::ui_lang_runtime::tray::TrayRow] = &[::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 4usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },];
::ui_lang_runtime::tray::init(::ui_lang_runtime::tray::TrayConfig { icons: __ICE_TRAY_ICONS, rows: __ICE_TRAY_ROWS, icon_template: false, });
::ui_lang_runtime::tray::set_item(3usize, "Open Ducktape");
::ui_lang_runtime::tray::set_item(5usize, "Go to");
::ui_lang_runtime::tray::set_item(6usize, "Chat");
::ui_lang_runtime::tray::set_item(7usize, "Pages");
::ui_lang_runtime::tray::set_item(8usize, "Node");
::ui_lang_runtime::tray::set_item(9usize, "Settings");
::ui_lang_runtime::tray::set_item(13usize, "Leave huddle");
::ui_lang_runtime::tray::set_item(14usize, "Appearance");
::ui_lang_runtime::tray::set_item(18usize, "Copy node key");
::ui_lang_runtime::tray::set_item(19usize, "Reconnect");
::ui_lang_runtime::tray::set_item(21usize, "Quit Ducktape");
}

let task = state.__preset_task_1();
state.__tray_sync();
(state, task)
}
fn __preset_task_2(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
let task = (|| {
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.rpc, __ice_next) { self.rpc = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = "Offline".to_owned(); if ::ui_lang_runtime::state_changed!(self.status, __ice_next) { self.status = __ice_next; self.__ice_rev[7] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.connected, __ice_next) { self.connected = __ice_next; self.__ice_rev[8] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.loading, __ice_next) { self.loading = __ice_next; self.__ice_rev[9] += 1; } }
{ let __ice_next = MutationPhase::Idle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
{ let __ice_next = ShellTab::Settings; if ::ui_lang_runtime::state_changed!(self.shell_tab, __ice_next) { self.shell_tab = __ice_next; self.__ice_rev[55] += 1; } }
::ducktape_view_guest::Task::none()
})();
task
}
fn __preset_2() -> (Self, ::ducktape_view_guest::Task<__DucktapeMessage>) {
let mut state = Self::__state();
{
const __ICE_TRAY_ICONS: &[::ui_lang_runtime::tray::TrayIcon] = &[::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-offline.rgba", rgba: { const __ICE_TRAY_RGBA_0: &[u8] = include_bytes!("../../assets/tray-offline.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_0.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_0 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-unread.rgba", rgba: { const __ICE_TRAY_RGBA_1: &[u8] = include_bytes!("../../assets/tray-unread.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_1.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_1 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray.rgba", rgba: { const __ICE_TRAY_RGBA_2: &[u8] = include_bytes!("../../assets/tray.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_2.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_2 }, width: 128u32, height: 128u32 },];
const __ICE_TRAY_ROWS: &[::ui_lang_runtime::tray::TrayRow] = &[::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 4usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },];
::ui_lang_runtime::tray::init(::ui_lang_runtime::tray::TrayConfig { icons: __ICE_TRAY_ICONS, rows: __ICE_TRAY_ROWS, icon_template: false, });
::ui_lang_runtime::tray::set_item(3usize, "Open Ducktape");
::ui_lang_runtime::tray::set_item(5usize, "Go to");
::ui_lang_runtime::tray::set_item(6usize, "Chat");
::ui_lang_runtime::tray::set_item(7usize, "Pages");
::ui_lang_runtime::tray::set_item(8usize, "Node");
::ui_lang_runtime::tray::set_item(9usize, "Settings");
::ui_lang_runtime::tray::set_item(13usize, "Leave huddle");
::ui_lang_runtime::tray::set_item(14usize, "Appearance");
::ui_lang_runtime::tray::set_item(18usize, "Copy node key");
::ui_lang_runtime::tray::set_item(19usize, "Reconnect");
::ui_lang_runtime::tray::set_item(21usize, "Quit Ducktape");
}

let task = state.__preset_task_2();
state.__tray_sync();
(state, task)
}
fn __preset_task_3(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
let task = (|| {
{ let __ice_next = "Connection failed".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
::ducktape_view_guest::Task::none()
})();
task
}
fn __preset_3() -> (Self, ::ducktape_view_guest::Task<__DucktapeMessage>) {
let mut state = Self::__state();
{
const __ICE_TRAY_ICONS: &[::ui_lang_runtime::tray::TrayIcon] = &[::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-offline.rgba", rgba: { const __ICE_TRAY_RGBA_0: &[u8] = include_bytes!("../../assets/tray-offline.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_0.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_0 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-unread.rgba", rgba: { const __ICE_TRAY_RGBA_1: &[u8] = include_bytes!("../../assets/tray-unread.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_1.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_1 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray.rgba", rgba: { const __ICE_TRAY_RGBA_2: &[u8] = include_bytes!("../../assets/tray.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_2.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_2 }, width: 128u32, height: 128u32 },];
const __ICE_TRAY_ROWS: &[::ui_lang_runtime::tray::TrayRow] = &[::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 4usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },];
::ui_lang_runtime::tray::init(::ui_lang_runtime::tray::TrayConfig { icons: __ICE_TRAY_ICONS, rows: __ICE_TRAY_ROWS, icon_template: false, });
::ui_lang_runtime::tray::set_item(3usize, "Open Ducktape");
::ui_lang_runtime::tray::set_item(5usize, "Go to");
::ui_lang_runtime::tray::set_item(6usize, "Chat");
::ui_lang_runtime::tray::set_item(7usize, "Pages");
::ui_lang_runtime::tray::set_item(8usize, "Node");
::ui_lang_runtime::tray::set_item(9usize, "Settings");
::ui_lang_runtime::tray::set_item(13usize, "Leave huddle");
::ui_lang_runtime::tray::set_item(14usize, "Appearance");
::ui_lang_runtime::tray::set_item(18usize, "Copy node key");
::ui_lang_runtime::tray::set_item(19usize, "Reconnect");
::ui_lang_runtime::tray::set_item(21usize, "Quit Ducktape");
}

let task = state.__preset_task_3();
state.__tray_sync();
(state, task)
}
fn __preset_task_4(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
let task = (|| {
{ let __ice_next = MutationPhase::Idle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = HubStep::Networks; if ::ui_lang_runtime::state_changed!(self.hub_step, __ice_next) { self.hub_step = __ice_next; self.__ice_rev[125] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.hub_networks, __ice_next) { self.hub_networks = __ice_next; self.__ice_rev[126] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.hub_selected, __ice_next) { self.hub_selected = __ice_next; self.__ice_rev[127] += 1; } }
{ let __ice_next = "http://127.0.0.1:1".to_owned(); if ::ui_lang_runtime::state_changed!(self.rpc, __ice_next) { self.rpc = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = "hunter2-hunter2".to_owned(); if ::ui_lang_runtime::state_changed!(self.password, __ice_next) { self.password = __ice_next; self.__ice_rev[6] += 1; } }
{ let __ice_next = ::std::vec![crate::backend::wallet_info("alice".to_owned(), "aabbccddeeff00112233".to_owned(), "encrypted".to_owned(), false), crate::backend::wallet_info("demo".to_owned(), "eeff0011".to_owned(), "encrypted".to_owned(), true)]; if ::ui_lang_runtime::state_changed!(self.hub_wallets, __ice_next) { self.hub_wallets = __ice_next; self.__ice_rev[128] += 1; } }
{ let __ice_next = "demo".to_owned(); if ::ui_lang_runtime::state_changed!(self.hub_wallet_selected, __ice_next) { self.hub_wallet_selected = __ice_next; self.__ice_rev[129] += 1; } }
::ducktape_view_guest::Task::none()
})();
task
}
fn __preset_4() -> (Self, ::ducktape_view_guest::Task<__DucktapeMessage>) {
let mut state = Self::__state();
{
const __ICE_TRAY_ICONS: &[::ui_lang_runtime::tray::TrayIcon] = &[::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-offline.rgba", rgba: { const __ICE_TRAY_RGBA_0: &[u8] = include_bytes!("../../assets/tray-offline.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_0.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_0 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-unread.rgba", rgba: { const __ICE_TRAY_RGBA_1: &[u8] = include_bytes!("../../assets/tray-unread.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_1.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_1 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray.rgba", rgba: { const __ICE_TRAY_RGBA_2: &[u8] = include_bytes!("../../assets/tray.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_2.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_2 }, width: 128u32, height: 128u32 },];
const __ICE_TRAY_ROWS: &[::ui_lang_runtime::tray::TrayRow] = &[::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 4usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },];
::ui_lang_runtime::tray::init(::ui_lang_runtime::tray::TrayConfig { icons: __ICE_TRAY_ICONS, rows: __ICE_TRAY_ROWS, icon_template: false, });
::ui_lang_runtime::tray::set_item(3usize, "Open Ducktape");
::ui_lang_runtime::tray::set_item(5usize, "Go to");
::ui_lang_runtime::tray::set_item(6usize, "Chat");
::ui_lang_runtime::tray::set_item(7usize, "Pages");
::ui_lang_runtime::tray::set_item(8usize, "Node");
::ui_lang_runtime::tray::set_item(9usize, "Settings");
::ui_lang_runtime::tray::set_item(13usize, "Leave huddle");
::ui_lang_runtime::tray::set_item(14usize, "Appearance");
::ui_lang_runtime::tray::set_item(18usize, "Copy node key");
::ui_lang_runtime::tray::set_item(19usize, "Reconnect");
::ui_lang_runtime::tray::set_item(21usize, "Quit Ducktape");
}

let task = state.__preset_task_4();
state.__tray_sync();
(state, task)
}
fn __preset_task_5(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
let task = (|| {
{ let __ice_next = MutationPhase::Onboarding; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = HubStep::Account; if ::ui_lang_runtime::state_changed!(self.hub_step, __ice_next) { self.hub_step = __ice_next; self.__ice_rev[125] += 1; } }
{ let __ice_next = "show_qr".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_phase, __ice_next) { self.ceremony_phase = __ice_next; self.__ice_rev[138] += 1; } }
{ let __ice_next = "https://auth.ducktape.industries/#op=get&challenge=AQID".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_qr, __ice_next) { self.ceremony_qr = __ice_next; self.__ice_rev[139] += 1; } }
{ let __ice_next = "Your phone will confirm with the passkey.".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_detail, __ice_next) { self.ceremony_detail = __ice_next; self.__ice_rev[140] += 1; } }
{ let __ice_next = "4:58".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_left, __ice_next) { self.ceremony_left = __ice_next; self.__ice_rev[141] += 1; } }
::ducktape_view_guest::Task::none()
})();
task
}
fn __preset_5() -> (Self, ::ducktape_view_guest::Task<__DucktapeMessage>) {
let mut state = Self::__state();
{
const __ICE_TRAY_ICONS: &[::ui_lang_runtime::tray::TrayIcon] = &[::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-offline.rgba", rgba: { const __ICE_TRAY_RGBA_0: &[u8] = include_bytes!("../../assets/tray-offline.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_0.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_0 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-unread.rgba", rgba: { const __ICE_TRAY_RGBA_1: &[u8] = include_bytes!("../../assets/tray-unread.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_1.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_1 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray.rgba", rgba: { const __ICE_TRAY_RGBA_2: &[u8] = include_bytes!("../../assets/tray.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_2.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_2 }, width: 128u32, height: 128u32 },];
const __ICE_TRAY_ROWS: &[::ui_lang_runtime::tray::TrayRow] = &[::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 4usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },];
::ui_lang_runtime::tray::init(::ui_lang_runtime::tray::TrayConfig { icons: __ICE_TRAY_ICONS, rows: __ICE_TRAY_ROWS, icon_template: false, });
::ui_lang_runtime::tray::set_item(3usize, "Open Ducktape");
::ui_lang_runtime::tray::set_item(5usize, "Go to");
::ui_lang_runtime::tray::set_item(6usize, "Chat");
::ui_lang_runtime::tray::set_item(7usize, "Pages");
::ui_lang_runtime::tray::set_item(8usize, "Node");
::ui_lang_runtime::tray::set_item(9usize, "Settings");
::ui_lang_runtime::tray::set_item(13usize, "Leave huddle");
::ui_lang_runtime::tray::set_item(14usize, "Appearance");
::ui_lang_runtime::tray::set_item(18usize, "Copy node key");
::ui_lang_runtime::tray::set_item(19usize, "Reconnect");
::ui_lang_runtime::tray::set_item(21usize, "Quit Ducktape");
}

let task = state.__preset_task_5();
state.__tray_sync();
(state, task)
}
fn __preset_task_6(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
let task = (|| {
{ let __ice_next = "http://127.0.0.1:1".to_owned(); if ::ui_lang_runtime::state_changed!(self.rpc, __ice_next) { self.rpc = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = "http://127.0.0.1:1".to_owned(); if ::ui_lang_runtime::state_changed!(self.connected_rpc, __ice_next) { self.connected_rpc = __ice_next; self.__ice_rev[5] += 1; } }
{ let __ice_next = "hunter2-hunter2".to_owned(); if ::ui_lang_runtime::state_changed!(self.password, __ice_next) { self.password = __ice_next; self.__ice_rev[6] += 1; } }
{ let __ice_next = "Connected".to_owned(); if ::ui_lang_runtime::state_changed!(self.status, __ice_next) { self.status = __ice_next; self.__ice_rev[7] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.connected, __ice_next) { self.connected = __ice_next; self.__ice_rev[8] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.loading, __ice_next) { self.loading = __ice_next; self.__ice_rev[9] += 1; } }
{ let __ice_next = MutationPhase::Idle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.account_exists, __ice_next) { self.account_exists = __ice_next; self.__ice_rev[72] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.account_banner_dismissed, __ice_next) { self.account_banner_dismissed = __ice_next; self.__ice_rev[79] += 1; } }
{ let __ice_next = ShellTab::Settings; if ::ui_lang_runtime::state_changed!(self.shell_tab, __ice_next) { self.shell_tab = __ice_next; self.__ice_rev[55] += 1; } }
::ducktape_view_guest::Task::none()
})();
task
}
fn __preset_6() -> (Self, ::ducktape_view_guest::Task<__DucktapeMessage>) {
let mut state = Self::__state();
{
const __ICE_TRAY_ICONS: &[::ui_lang_runtime::tray::TrayIcon] = &[::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-offline.rgba", rgba: { const __ICE_TRAY_RGBA_0: &[u8] = include_bytes!("../../assets/tray-offline.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_0.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_0 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-unread.rgba", rgba: { const __ICE_TRAY_RGBA_1: &[u8] = include_bytes!("../../assets/tray-unread.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_1.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_1 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray.rgba", rgba: { const __ICE_TRAY_RGBA_2: &[u8] = include_bytes!("../../assets/tray.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_2.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_2 }, width: 128u32, height: 128u32 },];
const __ICE_TRAY_ROWS: &[::ui_lang_runtime::tray::TrayRow] = &[::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 4usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },];
::ui_lang_runtime::tray::init(::ui_lang_runtime::tray::TrayConfig { icons: __ICE_TRAY_ICONS, rows: __ICE_TRAY_ROWS, icon_template: false, });
::ui_lang_runtime::tray::set_item(3usize, "Open Ducktape");
::ui_lang_runtime::tray::set_item(5usize, "Go to");
::ui_lang_runtime::tray::set_item(6usize, "Chat");
::ui_lang_runtime::tray::set_item(7usize, "Pages");
::ui_lang_runtime::tray::set_item(8usize, "Node");
::ui_lang_runtime::tray::set_item(9usize, "Settings");
::ui_lang_runtime::tray::set_item(13usize, "Leave huddle");
::ui_lang_runtime::tray::set_item(14usize, "Appearance");
::ui_lang_runtime::tray::set_item(18usize, "Copy node key");
::ui_lang_runtime::tray::set_item(19usize, "Reconnect");
::ui_lang_runtime::tray::set_item(21usize, "Quit Ducktape");
}

let task = state.__preset_task_6();
state.__tray_sync();
(state, task)
}
fn __preset_task_7(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
let task = (|| {
{ let __ice_next = "hunter2-hunter2".to_owned(); if ::ui_lang_runtime::state_changed!(self.password, __ice_next) { self.password = __ice_next; self.__ice_rev[6] += 1; } }
{ let __ice_next = "http://127.0.0.1:1".to_owned(); if ::ui_lang_runtime::state_changed!(self.rpc, __ice_next) { self.rpc = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = MutationPhase::Onboarding; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = HubStep::Networks; if ::ui_lang_runtime::state_changed!(self.hub_step, __ice_next) { self.hub_step = __ice_next; self.__ice_rev[125] += 1; } }
::ducktape_view_guest::Task::none()
})();
task
}
fn __preset_7() -> (Self, ::ducktape_view_guest::Task<__DucktapeMessage>) {
let mut state = Self::__state();
{
const __ICE_TRAY_ICONS: &[::ui_lang_runtime::tray::TrayIcon] = &[::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-offline.rgba", rgba: { const __ICE_TRAY_RGBA_0: &[u8] = include_bytes!("../../assets/tray-offline.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_0.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_0 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-unread.rgba", rgba: { const __ICE_TRAY_RGBA_1: &[u8] = include_bytes!("../../assets/tray-unread.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_1.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_1 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray.rgba", rgba: { const __ICE_TRAY_RGBA_2: &[u8] = include_bytes!("../../assets/tray.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_2.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_2 }, width: 128u32, height: 128u32 },];
const __ICE_TRAY_ROWS: &[::ui_lang_runtime::tray::TrayRow] = &[::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 4usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },];
::ui_lang_runtime::tray::init(::ui_lang_runtime::tray::TrayConfig { icons: __ICE_TRAY_ICONS, rows: __ICE_TRAY_ROWS, icon_template: false, });
::ui_lang_runtime::tray::set_item(3usize, "Open Ducktape");
::ui_lang_runtime::tray::set_item(5usize, "Go to");
::ui_lang_runtime::tray::set_item(6usize, "Chat");
::ui_lang_runtime::tray::set_item(7usize, "Pages");
::ui_lang_runtime::tray::set_item(8usize, "Node");
::ui_lang_runtime::tray::set_item(9usize, "Settings");
::ui_lang_runtime::tray::set_item(13usize, "Leave huddle");
::ui_lang_runtime::tray::set_item(14usize, "Appearance");
::ui_lang_runtime::tray::set_item(18usize, "Copy node key");
::ui_lang_runtime::tray::set_item(19usize, "Reconnect");
::ui_lang_runtime::tray::set_item(21usize, "Quit Ducktape");
}

let task = state.__preset_task_7();
state.__tray_sync();
(state, task)
}
fn __preset_task_8(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
let task = (|| {
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.palette_open, __ice_next) { self.palette_open = __ice_next; self.__ice_rev[104] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.palette_draft, __ice_next) { self.palette_draft = __ice_next; self.__ice_rev[113] += 1; } }
{ let __ice_next = "http://127.0.0.1:1".to_owned(); if ::ui_lang_runtime::state_changed!(self.connected_rpc, __ice_next) { self.connected_rpc = __ice_next; self.__ice_rev[5] += 1; } }
::ducktape_view_guest::Task::none()
})();
task
}
fn __preset_8() -> (Self, ::ducktape_view_guest::Task<__DucktapeMessage>) {
let mut state = Self::__state();
{
const __ICE_TRAY_ICONS: &[::ui_lang_runtime::tray::TrayIcon] = &[::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-offline.rgba", rgba: { const __ICE_TRAY_RGBA_0: &[u8] = include_bytes!("../../assets/tray-offline.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_0.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_0 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-unread.rgba", rgba: { const __ICE_TRAY_RGBA_1: &[u8] = include_bytes!("../../assets/tray-unread.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_1.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_1 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray.rgba", rgba: { const __ICE_TRAY_RGBA_2: &[u8] = include_bytes!("../../assets/tray.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_2.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_2 }, width: 128u32, height: 128u32 },];
const __ICE_TRAY_ROWS: &[::ui_lang_runtime::tray::TrayRow] = &[::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 4usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },];
::ui_lang_runtime::tray::init(::ui_lang_runtime::tray::TrayConfig { icons: __ICE_TRAY_ICONS, rows: __ICE_TRAY_ROWS, icon_template: false, });
::ui_lang_runtime::tray::set_item(3usize, "Open Ducktape");
::ui_lang_runtime::tray::set_item(5usize, "Go to");
::ui_lang_runtime::tray::set_item(6usize, "Chat");
::ui_lang_runtime::tray::set_item(7usize, "Pages");
::ui_lang_runtime::tray::set_item(8usize, "Node");
::ui_lang_runtime::tray::set_item(9usize, "Settings");
::ui_lang_runtime::tray::set_item(13usize, "Leave huddle");
::ui_lang_runtime::tray::set_item(14usize, "Appearance");
::ui_lang_runtime::tray::set_item(18usize, "Copy node key");
::ui_lang_runtime::tray::set_item(19usize, "Reconnect");
::ui_lang_runtime::tray::set_item(21usize, "Quit Ducktape");
}

let task = state.__preset_task_8();
state.__tray_sync();
(state, task)
}
fn __preset_task_9(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
let task = (|| {
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.connected, __ice_next) { self.connected = __ice_next; self.__ice_rev[8] += 1; } }
{ let __ice_next = ::std::option::Option::Some(({ crate::backend::window_target(::std::option::Option::None) })); if ::ui_lang_runtime::state_changed!(self.console_win, __ice_next) { self.console_win = __ice_next; self.__ice_rev[122] += 1; } }
{ let __ice_next = "http://127.0.0.1:1".to_owned(); if ::ui_lang_runtime::state_changed!(self.connected_rpc, __ice_next) { self.connected_rpc = __ice_next; self.__ice_rev[5] += 1; } }
{ let __ice_next = "demo".to_owned(); if ::ui_lang_runtime::state_changed!(self.network_name, __ice_next) { self.network_name = __ice_next; self.__ice_rev[124] += 1; } }
{ let __ice_next = 3; if ::ui_lang_runtime::state_changed!(self.bell_unread, __ice_next) { self.bell_unread = __ice_next; self.__ice_rev[106] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.huddle_joined, __ice_next) { self.huddle_joined = __ice_next; self.__ice_rev[142] += 1; } }
{ let __ice_next = "general".to_owned(); if ::ui_lang_runtime::state_changed!(self.huddle_channel_name, __ice_next) { self.huddle_channel_name = __ice_next; self.__ice_rev[144] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.call_muted, __ice_next) { self.call_muted = __ice_next; self.__ice_rev[148] += 1; } }
{ let __ice_next = Appearance::Dark; if ::ui_lang_runtime::state_changed!(self.appearance, __ice_next) { self.appearance = __ice_next; self.__ice_rev[1] += 1; self.__ice_derived.dark.take(); self.__ice_derived.app_background.take(); self.__ice_derived.app_text.take(); } }
::ducktape_view_guest::Task::none()
})();
task
}
fn __preset_9() -> (Self, ::ducktape_view_guest::Task<__DucktapeMessage>) {
let mut state = Self::__state();
{
const __ICE_TRAY_ICONS: &[::ui_lang_runtime::tray::TrayIcon] = &[::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-offline.rgba", rgba: { const __ICE_TRAY_RGBA_0: &[u8] = include_bytes!("../../assets/tray-offline.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_0.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_0 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-unread.rgba", rgba: { const __ICE_TRAY_RGBA_1: &[u8] = include_bytes!("../../assets/tray-unread.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_1.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_1 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray.rgba", rgba: { const __ICE_TRAY_RGBA_2: &[u8] = include_bytes!("../../assets/tray.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_2.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_2 }, width: 128u32, height: 128u32 },];
const __ICE_TRAY_ROWS: &[::ui_lang_runtime::tray::TrayRow] = &[::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 4usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },];
::ui_lang_runtime::tray::init(::ui_lang_runtime::tray::TrayConfig { icons: __ICE_TRAY_ICONS, rows: __ICE_TRAY_ROWS, icon_template: false, });
::ui_lang_runtime::tray::set_item(3usize, "Open Ducktape");
::ui_lang_runtime::tray::set_item(5usize, "Go to");
::ui_lang_runtime::tray::set_item(6usize, "Chat");
::ui_lang_runtime::tray::set_item(7usize, "Pages");
::ui_lang_runtime::tray::set_item(8usize, "Node");
::ui_lang_runtime::tray::set_item(9usize, "Settings");
::ui_lang_runtime::tray::set_item(13usize, "Leave huddle");
::ui_lang_runtime::tray::set_item(14usize, "Appearance");
::ui_lang_runtime::tray::set_item(18usize, "Copy node key");
::ui_lang_runtime::tray::set_item(19usize, "Reconnect");
::ui_lang_runtime::tray::set_item(21usize, "Quit Ducktape");
}

let task = state.__preset_task_9();
state.__tray_sync();
(state, task)
}
fn __preset_task_10(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
let task = (|| {
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.connected, __ice_next) { self.connected = __ice_next; self.__ice_rev[8] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.huddle_joined, __ice_next) { self.huddle_joined = __ice_next; self.__ice_rev[142] += 1; } }
{ let __ice_next = "eng".to_owned(); if ::ui_lang_runtime::state_changed!(self.huddle_channel_name, __ice_next) { self.huddle_channel_name = __ice_next; self.__ice_rev[144] += 1; } }
{ let __ice_next = "live".to_owned(); if ::ui_lang_runtime::state_changed!(self.call_status, __ice_next) { self.call_status = __ice_next; self.__ice_rev[147] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.call_muted, __ice_next) { self.call_muted = __ice_next; self.__ice_rev[148] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.call_camera, __ice_next) { self.call_camera = __ice_next; self.__ice_rev[150] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.call_sharing, __ice_next) { self.call_sharing = __ice_next; self.__ice_rev[151] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.call_video_live, __ice_next) { self.call_video_live = __ice_next; self.__ice_rev[152] += 1; } }
{ let __ice_next = "you".to_owned(); if ::ui_lang_runtime::state_changed!(self.huddle_stage, __ice_next) { self.huddle_stage = __ice_next; self.__ice_rev[153] += 1; } }
::ducktape_view_guest::Task::none()
})();
task
}
fn __preset_10() -> (Self, ::ducktape_view_guest::Task<__DucktapeMessage>) {
let mut state = Self::__state();
{
const __ICE_TRAY_ICONS: &[::ui_lang_runtime::tray::TrayIcon] = &[::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-offline.rgba", rgba: { const __ICE_TRAY_RGBA_0: &[u8] = include_bytes!("../../assets/tray-offline.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_0.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_0 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-unread.rgba", rgba: { const __ICE_TRAY_RGBA_1: &[u8] = include_bytes!("../../assets/tray-unread.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_1.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_1 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray.rgba", rgba: { const __ICE_TRAY_RGBA_2: &[u8] = include_bytes!("../../assets/tray.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_2.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_2 }, width: 128u32, height: 128u32 },];
const __ICE_TRAY_ROWS: &[::ui_lang_runtime::tray::TrayRow] = &[::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 4usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },];
::ui_lang_runtime::tray::init(::ui_lang_runtime::tray::TrayConfig { icons: __ICE_TRAY_ICONS, rows: __ICE_TRAY_ROWS, icon_template: false, });
::ui_lang_runtime::tray::set_item(3usize, "Open Ducktape");
::ui_lang_runtime::tray::set_item(5usize, "Go to");
::ui_lang_runtime::tray::set_item(6usize, "Chat");
::ui_lang_runtime::tray::set_item(7usize, "Pages");
::ui_lang_runtime::tray::set_item(8usize, "Node");
::ui_lang_runtime::tray::set_item(9usize, "Settings");
::ui_lang_runtime::tray::set_item(13usize, "Leave huddle");
::ui_lang_runtime::tray::set_item(14usize, "Appearance");
::ui_lang_runtime::tray::set_item(18usize, "Copy node key");
::ui_lang_runtime::tray::set_item(19usize, "Reconnect");
::ui_lang_runtime::tray::set_item(21usize, "Quit Ducktape");
}

let task = state.__preset_task_10();
state.__tray_sync();
(state, task)
}
fn __preset_task_11(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
let task = (|| {
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.connected, __ice_next) { self.connected = __ice_next; self.__ice_rev[8] += 1; } }
{ let __ice_next = HubStep::Live; if ::ui_lang_runtime::state_changed!(self.hub_step, __ice_next) { self.hub_step = __ice_next; self.__ice_rev[125] += 1; } }
::ducktape_view_guest::Task::none()
})();
task
}
fn __preset_11() -> (Self, ::ducktape_view_guest::Task<__DucktapeMessage>) {
let mut state = Self::__state();
{
const __ICE_TRAY_ICONS: &[::ui_lang_runtime::tray::TrayIcon] = &[::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-offline.rgba", rgba: { const __ICE_TRAY_RGBA_0: &[u8] = include_bytes!("../../assets/tray-offline.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_0.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_0 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-unread.rgba", rgba: { const __ICE_TRAY_RGBA_1: &[u8] = include_bytes!("../../assets/tray-unread.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_1.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_1 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray.rgba", rgba: { const __ICE_TRAY_RGBA_2: &[u8] = include_bytes!("../../assets/tray.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_2.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_2 }, width: 128u32, height: 128u32 },];
const __ICE_TRAY_ROWS: &[::ui_lang_runtime::tray::TrayRow] = &[::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 4usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },];
::ui_lang_runtime::tray::init(::ui_lang_runtime::tray::TrayConfig { icons: __ICE_TRAY_ICONS, rows: __ICE_TRAY_ROWS, icon_template: false, });
::ui_lang_runtime::tray::set_item(3usize, "Open Ducktape");
::ui_lang_runtime::tray::set_item(5usize, "Go to");
::ui_lang_runtime::tray::set_item(6usize, "Chat");
::ui_lang_runtime::tray::set_item(7usize, "Pages");
::ui_lang_runtime::tray::set_item(8usize, "Node");
::ui_lang_runtime::tray::set_item(9usize, "Settings");
::ui_lang_runtime::tray::set_item(13usize, "Leave huddle");
::ui_lang_runtime::tray::set_item(14usize, "Appearance");
::ui_lang_runtime::tray::set_item(18usize, "Copy node key");
::ui_lang_runtime::tray::set_item(19usize, "Reconnect");
::ui_lang_runtime::tray::set_item(21usize, "Quit Ducktape");
}

let task = state.__preset_task_11();
state.__tray_sync();
(state, task)
}
fn __preset_task_12(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
let task = (|| {
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.connected, __ice_next) { self.connected = __ice_next; self.__ice_rev[8] += 1; } }
{ let __ice_next = "http://127.0.0.1:8844".to_owned(); if ::ui_lang_runtime::state_changed!(self.connected_rpc, __ice_next) { self.connected_rpc = __ice_next; self.__ice_rev[5] += 1; } }
{ let __ice_next = "testnet#abcd".to_owned(); if ::ui_lang_runtime::state_changed!(self.network_chain_id, __ice_next) { self.network_chain_id = __ice_next; self.__ice_rev[86] += 1; } }
{ let __ice_next = 7; if ::ui_lang_runtime::state_changed!(self.connect_generation, __ice_next) { self.connect_generation = __ice_next; self.__ice_rev[16] += 1; } }
{ let __ice_next = "aa11".to_owned(); if ::ui_lang_runtime::state_changed!(self.signer_key, __ice_next) { self.signer_key = __ice_next; self.__ice_rev[17] += 1; } }
{ let __ice_next = ShellTab::Chat; if ::ui_lang_runtime::state_changed!(self.shell_tab, __ice_next) { self.shell_tab = __ice_next; self.__ice_rev[55] += 1; } }
{ let __ice_next = ::std::vec![crate::backend::live_agent_row("channel-a".to_owned(), 2, "chat:2:agent-1".to_owned(), "Chief Duck".to_owned(), "Reading the repo".to_owned())]; if ::ui_lang_runtime::state_changed!(self.live_agents, __ice_next) { self.live_agents = __ice_next; self.__ice_rev[39] += 1; } }
::ducktape_view_guest::Task::none()
})();
task
}
fn __preset_12() -> (Self, ::ducktape_view_guest::Task<__DucktapeMessage>) {
let mut state = Self::__state();
{
const __ICE_TRAY_ICONS: &[::ui_lang_runtime::tray::TrayIcon] = &[::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-offline.rgba", rgba: { const __ICE_TRAY_RGBA_0: &[u8] = include_bytes!("../../assets/tray-offline.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_0.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_0 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-unread.rgba", rgba: { const __ICE_TRAY_RGBA_1: &[u8] = include_bytes!("../../assets/tray-unread.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_1.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_1 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray.rgba", rgba: { const __ICE_TRAY_RGBA_2: &[u8] = include_bytes!("../../assets/tray.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_2.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_2 }, width: 128u32, height: 128u32 },];
const __ICE_TRAY_ROWS: &[::ui_lang_runtime::tray::TrayRow] = &[::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 4usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },];
::ui_lang_runtime::tray::init(::ui_lang_runtime::tray::TrayConfig { icons: __ICE_TRAY_ICONS, rows: __ICE_TRAY_ROWS, icon_template: false, });
::ui_lang_runtime::tray::set_item(3usize, "Open Ducktape");
::ui_lang_runtime::tray::set_item(5usize, "Go to");
::ui_lang_runtime::tray::set_item(6usize, "Chat");
::ui_lang_runtime::tray::set_item(7usize, "Pages");
::ui_lang_runtime::tray::set_item(8usize, "Node");
::ui_lang_runtime::tray::set_item(9usize, "Settings");
::ui_lang_runtime::tray::set_item(13usize, "Leave huddle");
::ui_lang_runtime::tray::set_item(14usize, "Appearance");
::ui_lang_runtime::tray::set_item(18usize, "Copy node key");
::ui_lang_runtime::tray::set_item(19usize, "Reconnect");
::ui_lang_runtime::tray::set_item(21usize, "Quit Ducktape");
}

let task = state.__preset_task_12();
state.__tray_sync();
(state, task)
}
fn __preset_task_13(&mut self) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
let task = (|| {
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.connected, __ice_next) { self.connected = __ice_next; self.__ice_rev[8] += 1; } }
{ let __ice_next = "http://127.0.0.1:8844".to_owned(); if ::ui_lang_runtime::state_changed!(self.connected_rpc, __ice_next) { self.connected_rpc = __ice_next; self.__ice_rev[5] += 1; } }
{ let __ice_next = "testnet#abcd".to_owned(); if ::ui_lang_runtime::state_changed!(self.network_chain_id, __ice_next) { self.network_chain_id = __ice_next; self.__ice_rev[86] += 1; } }
{ let __ice_next = 7; if ::ui_lang_runtime::state_changed!(self.connect_generation, __ice_next) { self.connect_generation = __ice_next; self.__ice_rev[16] += 1; } }
{ let __ice_next = "aa11".to_owned(); if ::ui_lang_runtime::state_changed!(self.signer_key, __ice_next) { self.signer_key = __ice_next; self.__ice_rev[17] += 1; } }
{ let __ice_next = ShellTab::Settings; if ::ui_lang_runtime::state_changed!(self.shell_tab, __ice_next) { self.shell_tab = __ice_next; self.__ice_rev[55] += 1; } }
{ let __ice_next = "qr".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_phase, __ice_next) { self.account_ceremony_phase = __ice_next; self.__ice_rev[80] += 1; } }
{ let __ice_next = "otpauth://totp/demo".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_qr, __ice_next) { self.account_ceremony_qr = __ice_next; self.__ice_rev[81] += 1; } }
::ducktape_view_guest::Task::none()
})();
task
}
fn __preset_13() -> (Self, ::ducktape_view_guest::Task<__DucktapeMessage>) {
let mut state = Self::__state();
{
const __ICE_TRAY_ICONS: &[::ui_lang_runtime::tray::TrayIcon] = &[::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-offline.rgba", rgba: { const __ICE_TRAY_RGBA_0: &[u8] = include_bytes!("../../assets/tray-offline.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_0.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_0 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray-unread.rgba", rgba: { const __ICE_TRAY_RGBA_1: &[u8] = include_bytes!("../../assets/tray-unread.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_1.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_1 }, width: 128u32, height: 128u32 },::ui_lang_runtime::tray::TrayIcon { path: "../../assets/tray.rgba", rgba: { const __ICE_TRAY_RGBA_2: &[u8] = include_bytes!("../../assets/tray.rgba"); const _: () = ::std::assert!(__ICE_TRAY_RGBA_2.len() == 65536, "tray icon RGBA byte length does not match width × height × 4"); __ICE_TRAY_RGBA_2 }, width: 128u32, height: 128u32 },];
const __ICE_TRAY_ROWS: &[::ui_lang_runtime::tray::TrayRow] = &[::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 4usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: false, nested: 2usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },::ui_lang_runtime::tray::TrayRow::Separator,::ui_lang_runtime::tray::TrayRow::Item { command: true, nested: 0usize },];
::ui_lang_runtime::tray::init(::ui_lang_runtime::tray::TrayConfig { icons: __ICE_TRAY_ICONS, rows: __ICE_TRAY_ROWS, icon_template: false, });
::ui_lang_runtime::tray::set_item(3usize, "Open Ducktape");
::ui_lang_runtime::tray::set_item(5usize, "Go to");
::ui_lang_runtime::tray::set_item(6usize, "Chat");
::ui_lang_runtime::tray::set_item(7usize, "Pages");
::ui_lang_runtime::tray::set_item(8usize, "Node");
::ui_lang_runtime::tray::set_item(9usize, "Settings");
::ui_lang_runtime::tray::set_item(13usize, "Leave huddle");
::ui_lang_runtime::tray::set_item(14usize, "Appearance");
::ui_lang_runtime::tray::set_item(18usize, "Copy node key");
::ui_lang_runtime::tray::set_item(19usize, "Reconnect");
::ui_lang_runtime::tray::set_item(21usize, "Quit Ducktape");
}

let task = state.__preset_task_13();
state.__tray_sync();
(state, task)
}
}
}
__ice_generated_items_2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f6170702e696365! {
#[allow(unused_parens)]
impl Ducktape {
fn __subscription(&self) -> ducktape_view_guest::Subscription<__DucktapeMessage> {
use ducktape_view_guest::Subscription;
let mut subscriptions = Vec::new();
if self.connected {
subscriptions.push(Subscription::run_with(self.connected_rpc.clone(), |rpc: &String| crate::backend::live_events(rpc.clone())).map(__DucktapeMessage::LiveUpdated));
subscriptions.push(Subscription::run_with(self.connected_rpc.clone(), |rpc: &String| crate::backend::node_status_live(rpc.clone())).map(__DucktapeMessage::NodeStatusPushed));
subscriptions.push(Subscription::run_with((self.connected_rpc.clone(), self.network_chain_id.clone(), self.connect_generation, self.signer_key.clone()), |data: &(String, String, i64, String)| crate::backend::chat_live_agents(data.0.clone(), data.1.clone(), data.2, data.3.clone())).map(__DucktapeMessage::LiveAgentsEvent));
}
let active_call = self.connected && self.huddle_joined && !self.huddle_channel.is_empty();
if active_call {
subscriptions.push(Subscription::run_with((self.connected_rpc.clone(), self.huddle_channel.clone()), |data: &(String, String)| crate::call::call_session(data.0.clone(), data.1.clone())).map(__DucktapeMessage::CallEvent));
}
if self.huddle_joined { subscriptions.push(Subscription::run(crate::shell::seconds).map(|()| __DucktapeMessage::Tick)); }
if self.console_win.is_some() { subscriptions.push(Subscription::run(crate::shell::seconds).map(|()| __DucktapeMessage::WallTick)); }
Subscription::batch(subscriptions)
}

}
}
__ice_generated_items_2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f6170702e696365! {
#[allow(unused_parens)]
impl Ducktape {
#[cfg(test)]
fn __ice_test_mount_0(&self, window: ::crate::shell::WindowKey) -> __IceElement<'_, __DucktapeMessage> { let __ice_palette = self.__palette(window); let __ice_app_theme = Self::__app_theme(__ice_palette);  let __ice_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 43, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/workspace-tabs", "Ducktape"); ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_118(__ice_palette, __ice_node_scope.clone())) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
};    ::ui_lang_runtime::navigation(__ice_content, __DucktapeMessage::__AccessibilityFocusNext(::std::option::Option::Some(window)), __DucktapeMessage::__AccessibilityFocusPrevious(::std::option::Option::Some(window))).in_window(window).into() }
#[cfg(test)]
fn __ice_test_program_0() -> ::iced::Daemon<impl ::iced::Program<State = Self, Message = __DucktapeMessage, Theme = ::iced::Theme>> { ::iced::daemon(Self::__boot, Self::__update, Self::__ice_test_mount_0).title(Self::__title).subscription(Self::__subscription).theme(Self::__theme).style(Self::__style).settings(::iced::Settings { 
id: ::std::option::Option::Some("dev.ducktape.app".to_owned()),
default_text_size: ::iced::Pixels(13.5 as f32),
antialiasing: true,
 ..::std::default::Default::default() }).default_font(Self::default_font())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/Geist[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/GeistMono[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/NotoColorEmoji.ttf").as_slice())
.presets([::iced::Preset::new("ui_offline", Self::__preset_0), ::iced::Preset::new("ui_palette_open", Self::__preset_1), ::iced::Preset::new("ui_settings", Self::__preset_2), ::iced::Preset::new("ui_component_error", Self::__preset_3), ::iced::Preset::new("ui_launch", Self::__preset_4), ::iced::Preset::new("ui_welcome_qr", Self::__preset_5), ::iced::Preset::new("ui_console_no_account", Self::__preset_6), ::iced::Preset::new("ui_pick_probe", Self::__preset_7), ::iced::Preset::new("ui_palette_overlay", Self::__preset_8), ::iced::Preset::new("ui_tray_live", Self::__preset_9), ::iced::Preset::new("ui_huddle_sharing", Self::__preset_10), ::iced::Preset::new("ui_tray_reconnect", Self::__preset_11), ::iced::Preset::new("ui_live_run_seated", Self::__preset_12), ::iced::Preset::new("ui_ceremony_on_settings", Self::__preset_13)]) }
#[cfg(test)]
fn __ice_test_mount_1(&self, window: ::crate::shell::WindowKey) -> __IceElement<'_, __DucktapeMessage> { let __ice_palette = self.__palette(window); let __ice_app_theme = Self::__app_theme(__ice_palette);  let __ice_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 114, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/channel-field", "Ducktape"); { let __ice_component_field_scope_10728 = __ice_node_scope.clone(); ::ui_lang_runtime::rev_memo(1177u64, (self.__ice_rev[43], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_119(__ice_palette, __ice_component_field_scope_10728))).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
};    ::ui_lang_runtime::navigation(__ice_content, __DucktapeMessage::__AccessibilityFocusNext(::std::option::Option::Some(window)), __DucktapeMessage::__AccessibilityFocusPrevious(::std::option::Option::Some(window))).in_window(window).into() }
#[cfg(test)]
fn __ice_test_program_1() -> ::iced::Daemon<impl ::iced::Program<State = Self, Message = __DucktapeMessage, Theme = ::iced::Theme>> { ::iced::daemon(Self::__boot, Self::__update, Self::__ice_test_mount_1).title(Self::__title).subscription(Self::__subscription).theme(Self::__theme).style(Self::__style).settings(::iced::Settings { 
id: ::std::option::Option::Some("dev.ducktape.app".to_owned()),
default_text_size: ::iced::Pixels(13.5 as f32),
antialiasing: true,
 ..::std::default::Default::default() }).default_font(Self::default_font())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/Geist[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/GeistMono[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/NotoColorEmoji.ttf").as_slice())
.presets([::iced::Preset::new("ui_offline", Self::__preset_0), ::iced::Preset::new("ui_palette_open", Self::__preset_1), ::iced::Preset::new("ui_settings", Self::__preset_2), ::iced::Preset::new("ui_component_error", Self::__preset_3), ::iced::Preset::new("ui_launch", Self::__preset_4), ::iced::Preset::new("ui_welcome_qr", Self::__preset_5), ::iced::Preset::new("ui_console_no_account", Self::__preset_6), ::iced::Preset::new("ui_pick_probe", Self::__preset_7), ::iced::Preset::new("ui_palette_overlay", Self::__preset_8), ::iced::Preset::new("ui_tray_live", Self::__preset_9), ::iced::Preset::new("ui_huddle_sharing", Self::__preset_10), ::iced::Preset::new("ui_tray_reconnect", Self::__preset_11), ::iced::Preset::new("ui_live_run_seated", Self::__preset_12), ::iced::Preset::new("ui_ceremony_on_settings", Self::__preset_13)]) }
#[cfg(test)]
fn __ice_test_mount_2(&self, window: ::crate::shell::WindowKey) -> __IceElement<'_, __DucktapeMessage> { let __ice_palette = self.__palette(window); let __ice_app_theme = Self::__app_theme(__ice_palette);  let __ice_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 136, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/surface", "Ducktape"); { let __a11y_key = __ice_node_scope.as_str(); let __container_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 137, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/library", __ice_node_scope); { let __ice_component_panel_scope_10751 = __ice_node_scope.clone(); ::ui_lang_runtime::rev_memo(1180u64, (self.__ice_rev[20], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_123(__ice_palette, __ice_component_panel_scope_10751))).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __container = {  ::iced::widget::container(__container_content).id(::iced::widget::Id::from(__ice_node_scope.clone())).width(::iced::Fill) }; ::ui_lang_runtime::accessible(__container, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
};    ::ui_lang_runtime::navigation(__ice_content, __DucktapeMessage::__AccessibilityFocusNext(::std::option::Option::Some(window)), __DucktapeMessage::__AccessibilityFocusPrevious(::std::option::Option::Some(window))).in_window(window).into() }
#[cfg(test)]
fn __ice_test_program_2() -> ::iced::Daemon<impl ::iced::Program<State = Self, Message = __DucktapeMessage, Theme = ::iced::Theme>> { ::iced::daemon(Self::__boot, Self::__update, Self::__ice_test_mount_2).title(Self::__title).subscription(Self::__subscription).theme(Self::__theme).style(Self::__style).settings(::iced::Settings { 
id: ::std::option::Option::Some("dev.ducktape.app".to_owned()),
default_text_size: ::iced::Pixels(13.5 as f32),
antialiasing: true,
 ..::std::default::Default::default() }).default_font(Self::default_font())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/Geist[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/GeistMono[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/NotoColorEmoji.ttf").as_slice())
.presets([::iced::Preset::new("ui_offline", Self::__preset_0), ::iced::Preset::new("ui_palette_open", Self::__preset_1), ::iced::Preset::new("ui_settings", Self::__preset_2), ::iced::Preset::new("ui_component_error", Self::__preset_3), ::iced::Preset::new("ui_launch", Self::__preset_4), ::iced::Preset::new("ui_welcome_qr", Self::__preset_5), ::iced::Preset::new("ui_console_no_account", Self::__preset_6), ::iced::Preset::new("ui_pick_probe", Self::__preset_7), ::iced::Preset::new("ui_palette_overlay", Self::__preset_8), ::iced::Preset::new("ui_tray_live", Self::__preset_9), ::iced::Preset::new("ui_huddle_sharing", Self::__preset_10), ::iced::Preset::new("ui_tray_reconnect", Self::__preset_11), ::iced::Preset::new("ui_live_run_seated", Self::__preset_12), ::iced::Preset::new("ui_ceremony_on_settings", Self::__preset_13)]) }
#[cfg(test)]
fn __ice_test_mount_3(&self, window: ::crate::shell::WindowKey) -> __IceElement<'_, __DucktapeMessage> { let __ice_palette = self.__palette(window); let __ice_app_theme = Self::__app_theme(__ice_palette);  let __ice_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 173, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/workspace-tabs", "Ducktape"); ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_124(__ice_palette, __ice_node_scope.clone())) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
};    ::ui_lang_runtime::navigation(__ice_content, __DucktapeMessage::__AccessibilityFocusNext(::std::option::Option::Some(window)), __DucktapeMessage::__AccessibilityFocusPrevious(::std::option::Option::Some(window))).in_window(window).into() }
#[cfg(test)]
fn __ice_test_program_3() -> ::iced::Daemon<impl ::iced::Program<State = Self, Message = __DucktapeMessage, Theme = ::iced::Theme>> { ::iced::daemon(Self::__boot, Self::__update, Self::__ice_test_mount_3).title(Self::__title).subscription(Self::__subscription).theme(Self::__theme).style(Self::__style).settings(::iced::Settings { 
id: ::std::option::Option::Some("dev.ducktape.app".to_owned()),
default_text_size: ::iced::Pixels(13.5 as f32),
antialiasing: true,
 ..::std::default::Default::default() }).default_font(Self::default_font())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/Geist[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/GeistMono[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/NotoColorEmoji.ttf").as_slice())
.presets([::iced::Preset::new("ui_offline", Self::__preset_0), ::iced::Preset::new("ui_palette_open", Self::__preset_1), ::iced::Preset::new("ui_settings", Self::__preset_2), ::iced::Preset::new("ui_component_error", Self::__preset_3), ::iced::Preset::new("ui_launch", Self::__preset_4), ::iced::Preset::new("ui_welcome_qr", Self::__preset_5), ::iced::Preset::new("ui_console_no_account", Self::__preset_6), ::iced::Preset::new("ui_pick_probe", Self::__preset_7), ::iced::Preset::new("ui_palette_overlay", Self::__preset_8), ::iced::Preset::new("ui_tray_live", Self::__preset_9), ::iced::Preset::new("ui_huddle_sharing", Self::__preset_10), ::iced::Preset::new("ui_tray_reconnect", Self::__preset_11), ::iced::Preset::new("ui_live_run_seated", Self::__preset_12), ::iced::Preset::new("ui_ceremony_on_settings", Self::__preset_13)]) }
#[cfg(test)]
fn __ice_test_mount_4(&self, window: ::crate::shell::WindowKey) -> __IceElement<'_, __DucktapeMessage> { let __ice_palette = self.__palette(window); let __ice_app_theme = Self::__app_theme(__ice_palette);  let __ice_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 263, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/hub", "Ducktape"); ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_169(__ice_palette, __ice_node_scope.clone())) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
};    ::ui_lang_runtime::navigation(__ice_content, __DucktapeMessage::__AccessibilityFocusNext(::std::option::Option::Some(window)), __DucktapeMessage::__AccessibilityFocusPrevious(::std::option::Option::Some(window))).in_window(window).into() }
#[cfg(test)]
fn __ice_test_program_4() -> ::iced::Daemon<impl ::iced::Program<State = Self, Message = __DucktapeMessage, Theme = ::iced::Theme>> { ::iced::daemon(Self::__boot, Self::__update, Self::__ice_test_mount_4).title(Self::__title).subscription(Self::__subscription).theme(Self::__theme).style(Self::__style).settings(::iced::Settings { 
id: ::std::option::Option::Some("dev.ducktape.app".to_owned()),
default_text_size: ::iced::Pixels(13.5 as f32),
antialiasing: true,
 ..::std::default::Default::default() }).default_font(Self::default_font())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/Geist[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/GeistMono[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/NotoColorEmoji.ttf").as_slice())
.presets([::iced::Preset::new("ui_offline", Self::__preset_0), ::iced::Preset::new("ui_palette_open", Self::__preset_1), ::iced::Preset::new("ui_settings", Self::__preset_2), ::iced::Preset::new("ui_component_error", Self::__preset_3), ::iced::Preset::new("ui_launch", Self::__preset_4), ::iced::Preset::new("ui_welcome_qr", Self::__preset_5), ::iced::Preset::new("ui_console_no_account", Self::__preset_6), ::iced::Preset::new("ui_pick_probe", Self::__preset_7), ::iced::Preset::new("ui_palette_overlay", Self::__preset_8), ::iced::Preset::new("ui_tray_live", Self::__preset_9), ::iced::Preset::new("ui_huddle_sharing", Self::__preset_10), ::iced::Preset::new("ui_tray_reconnect", Self::__preset_11), ::iced::Preset::new("ui_live_run_seated", Self::__preset_12), ::iced::Preset::new("ui_ceremony_on_settings", Self::__preset_13)]) }
#[cfg(test)]
fn __ice_test_mount_5(&self, window: ::crate::shell::WindowKey) -> __IceElement<'_, __DucktapeMessage> { let __ice_palette = self.__palette(window); let __ice_app_theme = Self::__app_theme(__ice_palette);  let __ice_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 326, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/wallets", "Ducktape"); ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_171(__ice_palette, __ice_node_scope.clone())) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
};    ::ui_lang_runtime::navigation(__ice_content, __DucktapeMessage::__AccessibilityFocusNext(::std::option::Option::Some(window)), __DucktapeMessage::__AccessibilityFocusPrevious(::std::option::Option::Some(window))).in_window(window).into() }
#[cfg(test)]
fn __ice_test_program_5() -> ::iced::Daemon<impl ::iced::Program<State = Self, Message = __DucktapeMessage, Theme = ::iced::Theme>> { ::iced::daemon(Self::__boot, Self::__update, Self::__ice_test_mount_5).title(Self::__title).subscription(Self::__subscription).theme(Self::__theme).style(Self::__style).settings(::iced::Settings { 
id: ::std::option::Option::Some("dev.ducktape.app".to_owned()),
default_text_size: ::iced::Pixels(13.5 as f32),
antialiasing: true,
 ..::std::default::Default::default() }).default_font(Self::default_font())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/Geist[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/GeistMono[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/NotoColorEmoji.ttf").as_slice())
.presets([::iced::Preset::new("ui_offline", Self::__preset_0), ::iced::Preset::new("ui_palette_open", Self::__preset_1), ::iced::Preset::new("ui_settings", Self::__preset_2), ::iced::Preset::new("ui_component_error", Self::__preset_3), ::iced::Preset::new("ui_launch", Self::__preset_4), ::iced::Preset::new("ui_welcome_qr", Self::__preset_5), ::iced::Preset::new("ui_console_no_account", Self::__preset_6), ::iced::Preset::new("ui_pick_probe", Self::__preset_7), ::iced::Preset::new("ui_palette_overlay", Self::__preset_8), ::iced::Preset::new("ui_tray_live", Self::__preset_9), ::iced::Preset::new("ui_huddle_sharing", Self::__preset_10), ::iced::Preset::new("ui_tray_reconnect", Self::__preset_11), ::iced::Preset::new("ui_live_run_seated", Self::__preset_12), ::iced::Preset::new("ui_ceremony_on_settings", Self::__preset_13)]) }
#[cfg(test)]
fn __ice_test_mount_6(&self, window: ::crate::shell::WindowKey) -> __IceElement<'_, __DucktapeMessage> { let __ice_palette = self.__palette(window); let __ice_app_theme = Self::__app_theme(__ice_palette);  let __ice_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 372, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/pw", "Ducktape"); { let __ice_component_050617373776f726453637265656e_scope_10986 = __ice_node_scope.clone(); ::ui_lang_runtime::rev_memo(1203u64, (self.__ice_component_050617373776f726453637265656e.get(&__ice_component_050617373776f726453637265656e_scope_10986).map_or(0, |__state| __state.__ice_rev[0]), self.__ice_component_050617373776f726453637265656e.get(&__ice_component_050617373776f726453637265656e_scope_10986).map_or(0, |__state| __state.__ice_rev[1]), __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_175(__ice_palette, __ice_component_050617373776f726453637265656e_scope_10986))).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
};    ::ui_lang_runtime::navigation(__ice_content, __DucktapeMessage::__AccessibilityFocusNext(::std::option::Option::Some(window)), __DucktapeMessage::__AccessibilityFocusPrevious(::std::option::Option::Some(window))).in_window(window).into() }
#[cfg(test)]
fn __ice_test_program_6() -> ::iced::Daemon<impl ::iced::Program<State = Self, Message = __DucktapeMessage, Theme = ::iced::Theme>> { ::iced::daemon(Self::__boot, Self::__update, Self::__ice_test_mount_6).title(Self::__title).subscription(Self::__subscription).theme(Self::__theme).style(Self::__style).settings(::iced::Settings { 
id: ::std::option::Option::Some("dev.ducktape.app".to_owned()),
default_text_size: ::iced::Pixels(13.5 as f32),
antialiasing: true,
 ..::std::default::Default::default() }).default_font(Self::default_font())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/Geist[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/GeistMono[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/NotoColorEmoji.ttf").as_slice())
.presets([::iced::Preset::new("ui_offline", Self::__preset_0), ::iced::Preset::new("ui_palette_open", Self::__preset_1), ::iced::Preset::new("ui_settings", Self::__preset_2), ::iced::Preset::new("ui_component_error", Self::__preset_3), ::iced::Preset::new("ui_launch", Self::__preset_4), ::iced::Preset::new("ui_welcome_qr", Self::__preset_5), ::iced::Preset::new("ui_console_no_account", Self::__preset_6), ::iced::Preset::new("ui_pick_probe", Self::__preset_7), ::iced::Preset::new("ui_palette_overlay", Self::__preset_8), ::iced::Preset::new("ui_tray_live", Self::__preset_9), ::iced::Preset::new("ui_huddle_sharing", Self::__preset_10), ::iced::Preset::new("ui_tray_reconnect", Self::__preset_11), ::iced::Preset::new("ui_live_run_seated", Self::__preset_12), ::iced::Preset::new("ui_ceremony_on_settings", Self::__preset_13)]) }
#[cfg(test)]
fn __ice_test_mount_8(&self, window: ::crate::shell::WindowKey) -> __IceElement<'_, __DucktapeMessage> { let __ice_palette = self.__palette(window); let __ice_app_theme = Self::__app_theme(__ice_palette);  let __ice_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 418, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/phrase", "Ducktape"); { let __ice_component_050687261736553637265656e_scope_11032 = __ice_node_scope.clone(); ::ui_lang_runtime::rev_memo(1204u64, (__ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_176(__ice_palette, __ice_component_050687261736553637265656e_scope_11032))).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
};    ::ui_lang_runtime::navigation(__ice_content, __DucktapeMessage::__AccessibilityFocusNext(::std::option::Option::Some(window)), __DucktapeMessage::__AccessibilityFocusPrevious(::std::option::Option::Some(window))).in_window(window).into() }
#[cfg(test)]
fn __ice_test_program_8() -> ::iced::Daemon<impl ::iced::Program<State = Self, Message = __DucktapeMessage, Theme = ::iced::Theme>> { ::iced::daemon(Self::__boot, Self::__update, Self::__ice_test_mount_8).title(Self::__title).subscription(Self::__subscription).theme(Self::__theme).style(Self::__style).settings(::iced::Settings { 
id: ::std::option::Option::Some("dev.ducktape.app".to_owned()),
default_text_size: ::iced::Pixels(13.5 as f32),
antialiasing: true,
 ..::std::default::Default::default() }).default_font(Self::default_font())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/Geist[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/GeistMono[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/NotoColorEmoji.ttf").as_slice())
.presets([::iced::Preset::new("ui_offline", Self::__preset_0), ::iced::Preset::new("ui_palette_open", Self::__preset_1), ::iced::Preset::new("ui_settings", Self::__preset_2), ::iced::Preset::new("ui_component_error", Self::__preset_3), ::iced::Preset::new("ui_launch", Self::__preset_4), ::iced::Preset::new("ui_welcome_qr", Self::__preset_5), ::iced::Preset::new("ui_console_no_account", Self::__preset_6), ::iced::Preset::new("ui_pick_probe", Self::__preset_7), ::iced::Preset::new("ui_palette_overlay", Self::__preset_8), ::iced::Preset::new("ui_tray_live", Self::__preset_9), ::iced::Preset::new("ui_huddle_sharing", Self::__preset_10), ::iced::Preset::new("ui_tray_reconnect", Self::__preset_11), ::iced::Preset::new("ui_live_run_seated", Self::__preset_12), ::iced::Preset::new("ui_ceremony_on_settings", Self::__preset_13)]) }
#[cfg(test)]
fn __ice_test_mount_9(&self, window: ::crate::shell::WindowKey) -> __IceElement<'_, __DucktapeMessage> { let __ice_palette = self.__palette(window); let __ice_app_theme = Self::__app_theme(__ice_palette);  let __ice_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 442, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/confirm", "Ducktape"); { let __ice_component_0436f6e6669726d50687261736553637265656e_scope_11056 = __ice_node_scope.clone(); ::ui_lang_runtime::rev_memo(1205u64, (self.__ice_component_0436f6e6669726d50687261736553637265656e.get(&__ice_component_0436f6e6669726d50687261736553637265656e_scope_11056).map_or(0, |__state| __state.__ice_rev[0]), __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_177(__ice_palette, __ice_component_0436f6e6669726d50687261736553637265656e_scope_11056))).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
};    ::ui_lang_runtime::navigation(__ice_content, __DucktapeMessage::__AccessibilityFocusNext(::std::option::Option::Some(window)), __DucktapeMessage::__AccessibilityFocusPrevious(::std::option::Option::Some(window))).in_window(window).into() }
#[cfg(test)]
fn __ice_test_program_9() -> ::iced::Daemon<impl ::iced::Program<State = Self, Message = __DucktapeMessage, Theme = ::iced::Theme>> { ::iced::daemon(Self::__boot, Self::__update, Self::__ice_test_mount_9).title(Self::__title).subscription(Self::__subscription).theme(Self::__theme).style(Self::__style).settings(::iced::Settings { 
id: ::std::option::Option::Some("dev.ducktape.app".to_owned()),
default_text_size: ::iced::Pixels(13.5 as f32),
antialiasing: true,
 ..::std::default::Default::default() }).default_font(Self::default_font())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/Geist[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/GeistMono[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/NotoColorEmoji.ttf").as_slice())
.presets([::iced::Preset::new("ui_offline", Self::__preset_0), ::iced::Preset::new("ui_palette_open", Self::__preset_1), ::iced::Preset::new("ui_settings", Self::__preset_2), ::iced::Preset::new("ui_component_error", Self::__preset_3), ::iced::Preset::new("ui_launch", Self::__preset_4), ::iced::Preset::new("ui_welcome_qr", Self::__preset_5), ::iced::Preset::new("ui_console_no_account", Self::__preset_6), ::iced::Preset::new("ui_pick_probe", Self::__preset_7), ::iced::Preset::new("ui_palette_overlay", Self::__preset_8), ::iced::Preset::new("ui_tray_live", Self::__preset_9), ::iced::Preset::new("ui_huddle_sharing", Self::__preset_10), ::iced::Preset::new("ui_tray_reconnect", Self::__preset_11), ::iced::Preset::new("ui_live_run_seated", Self::__preset_12), ::iced::Preset::new("ui_ceremony_on_settings", Self::__preset_13)]) }
#[cfg(test)]
fn __ice_test_mount_10(&self, window: ::crate::shell::WindowKey) -> __IceElement<'_, __DucktapeMessage> { let __ice_palette = self.__palette(window); let __ice_app_theme = Self::__app_theme(__ice_palette);  let __ice_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 463, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/hub", "Ducktape"); ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_178(__ice_palette, __ice_node_scope.clone())) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
};    ::ui_lang_runtime::navigation(__ice_content, __DucktapeMessage::__AccessibilityFocusNext(::std::option::Option::Some(window)), __DucktapeMessage::__AccessibilityFocusPrevious(::std::option::Option::Some(window))).in_window(window).into() }
#[cfg(test)]
fn __ice_test_program_10() -> ::iced::Daemon<impl ::iced::Program<State = Self, Message = __DucktapeMessage, Theme = ::iced::Theme>> { ::iced::daemon(Self::__boot, Self::__update, Self::__ice_test_mount_10).title(Self::__title).subscription(Self::__subscription).theme(Self::__theme).style(Self::__style).settings(::iced::Settings { 
id: ::std::option::Option::Some("dev.ducktape.app".to_owned()),
default_text_size: ::iced::Pixels(13.5 as f32),
antialiasing: true,
 ..::std::default::Default::default() }).default_font(Self::default_font())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/Geist[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/GeistMono[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/NotoColorEmoji.ttf").as_slice())
.presets([::iced::Preset::new("ui_offline", Self::__preset_0), ::iced::Preset::new("ui_palette_open", Self::__preset_1), ::iced::Preset::new("ui_settings", Self::__preset_2), ::iced::Preset::new("ui_component_error", Self::__preset_3), ::iced::Preset::new("ui_launch", Self::__preset_4), ::iced::Preset::new("ui_welcome_qr", Self::__preset_5), ::iced::Preset::new("ui_console_no_account", Self::__preset_6), ::iced::Preset::new("ui_pick_probe", Self::__preset_7), ::iced::Preset::new("ui_palette_overlay", Self::__preset_8), ::iced::Preset::new("ui_tray_live", Self::__preset_9), ::iced::Preset::new("ui_huddle_sharing", Self::__preset_10), ::iced::Preset::new("ui_tray_reconnect", Self::__preset_11), ::iced::Preset::new("ui_live_run_seated", Self::__preset_12), ::iced::Preset::new("ui_ceremony_on_settings", Self::__preset_13)]) }
#[cfg(test)]
fn __ice_test_mount_11(&self, window: ::crate::shell::WindowKey) -> __IceElement<'_, __DucktapeMessage> { let __ice_palette = self.__palette(window); let __ice_app_theme = Self::__app_theme(__ice_palette);  let __ice_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 560, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/welcome", "Ducktape"); { let __ice_component_057656c636f6d6553637265656e_scope_11174 = __ice_node_scope.clone(); ::ui_lang_runtime::rev_memo(1207u64, (self.__ice_rev[137], self.__ice_rev[138], self.__ice_rev[139], self.__ice_rev[140], self.__ice_rev[141], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_180(__ice_palette, __ice_component_057656c636f6d6553637265656e_scope_11174))).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
};    ::ui_lang_runtime::navigation(__ice_content, __DucktapeMessage::__AccessibilityFocusNext(::std::option::Option::Some(window)), __DucktapeMessage::__AccessibilityFocusPrevious(::std::option::Option::Some(window))).in_window(window).into() }
#[cfg(test)]
fn __ice_test_program_11() -> ::iced::Daemon<impl ::iced::Program<State = Self, Message = __DucktapeMessage, Theme = ::iced::Theme>> { ::iced::daemon(Self::__boot, Self::__update, Self::__ice_test_mount_11).title(Self::__title).subscription(Self::__subscription).theme(Self::__theme).style(Self::__style).settings(::iced::Settings { 
id: ::std::option::Option::Some("dev.ducktape.app".to_owned()),
default_text_size: ::iced::Pixels(13.5 as f32),
antialiasing: true,
 ..::std::default::Default::default() }).default_font(Self::default_font())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/Geist[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/GeistMono[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/NotoColorEmoji.ttf").as_slice())
.presets([::iced::Preset::new("ui_offline", Self::__preset_0), ::iced::Preset::new("ui_palette_open", Self::__preset_1), ::iced::Preset::new("ui_settings", Self::__preset_2), ::iced::Preset::new("ui_component_error", Self::__preset_3), ::iced::Preset::new("ui_launch", Self::__preset_4), ::iced::Preset::new("ui_welcome_qr", Self::__preset_5), ::iced::Preset::new("ui_console_no_account", Self::__preset_6), ::iced::Preset::new("ui_pick_probe", Self::__preset_7), ::iced::Preset::new("ui_palette_overlay", Self::__preset_8), ::iced::Preset::new("ui_tray_live", Self::__preset_9), ::iced::Preset::new("ui_huddle_sharing", Self::__preset_10), ::iced::Preset::new("ui_tray_reconnect", Self::__preset_11), ::iced::Preset::new("ui_live_run_seated", Self::__preset_12), ::iced::Preset::new("ui_ceremony_on_settings", Self::__preset_13)]) }
#[cfg(test)]
fn __ice_test_mount_13(&self, window: ::crate::shell::WindowKey) -> __IceElement<'_, __DucktapeMessage> { let __ice_palette = self.__palette(window); let __ice_app_theme = Self::__app_theme(__ice_palette);  let __ice_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 610, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/account-banner", "Ducktape"); { let __ice_component_04163636f756e7442616e6e6572_scope_11224 = __ice_node_scope.clone(); ::ui_lang_runtime::rev_memo(1208u64, (self.__ice_rev[6], self.__ice_rev[72], self.__ice_rev[79], self.__ice_rev[8], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_181(__ice_palette, __ice_component_04163636f756e7442616e6e6572_scope_11224))).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
};    ::ui_lang_runtime::navigation(__ice_content, __DucktapeMessage::__AccessibilityFocusNext(::std::option::Option::Some(window)), __DucktapeMessage::__AccessibilityFocusPrevious(::std::option::Option::Some(window))).in_window(window).into() }
#[cfg(test)]
fn __ice_test_program_13() -> ::iced::Daemon<impl ::iced::Program<State = Self, Message = __DucktapeMessage, Theme = ::iced::Theme>> { ::iced::daemon(Self::__boot, Self::__update, Self::__ice_test_mount_13).title(Self::__title).subscription(Self::__subscription).theme(Self::__theme).style(Self::__style).settings(::iced::Settings { 
id: ::std::option::Option::Some("dev.ducktape.app".to_owned()),
default_text_size: ::iced::Pixels(13.5 as f32),
antialiasing: true,
 ..::std::default::Default::default() }).default_font(Self::default_font())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/Geist[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/GeistMono[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/NotoColorEmoji.ttf").as_slice())
.presets([::iced::Preset::new("ui_offline", Self::__preset_0), ::iced::Preset::new("ui_palette_open", Self::__preset_1), ::iced::Preset::new("ui_settings", Self::__preset_2), ::iced::Preset::new("ui_component_error", Self::__preset_3), ::iced::Preset::new("ui_launch", Self::__preset_4), ::iced::Preset::new("ui_welcome_qr", Self::__preset_5), ::iced::Preset::new("ui_console_no_account", Self::__preset_6), ::iced::Preset::new("ui_pick_probe", Self::__preset_7), ::iced::Preset::new("ui_palette_overlay", Self::__preset_8), ::iced::Preset::new("ui_tray_live", Self::__preset_9), ::iced::Preset::new("ui_huddle_sharing", Self::__preset_10), ::iced::Preset::new("ui_tray_reconnect", Self::__preset_11), ::iced::Preset::new("ui_live_run_seated", Self::__preset_12), ::iced::Preset::new("ui_ceremony_on_settings", Self::__preset_13)]) }
#[cfg(test)]
fn __ice_test_mount_15(&self, window: ::crate::shell::WindowKey) -> __IceElement<'_, __DucktapeMessage> { let __ice_palette = self.__palette(window); let __ice_app_theme = Self::__app_theme(__ice_palette);  let __ice_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 657, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/overlays", "Ducktape"); { let __ice_component_04f7665726c61794c61796572_scope_11271 = __ice_node_scope.clone(); ::ui_lang_runtime::rev_memo(1209u64, (self.__ice_rev[104], self.__ice_rev[113], self.__ice_rev[114], self.__ice_rev[115], self.__ice_rev[116], self.__ice_rev[43], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_188(__ice_palette, __ice_component_04f7665726c61794c61796572_scope_11271))).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
};    ::ui_lang_runtime::navigation(__ice_content, __DucktapeMessage::__AccessibilityFocusNext(::std::option::Option::Some(window)), __DucktapeMessage::__AccessibilityFocusPrevious(::std::option::Option::Some(window))).in_window(window).into() }
#[cfg(test)]
fn __ice_test_program_15() -> ::iced::Daemon<impl ::iced::Program<State = Self, Message = __DucktapeMessage, Theme = ::iced::Theme>> { ::iced::daemon(Self::__boot, Self::__update, Self::__ice_test_mount_15).title(Self::__title).subscription(Self::__subscription).theme(Self::__theme).style(Self::__style).settings(::iced::Settings { 
id: ::std::option::Option::Some("dev.ducktape.app".to_owned()),
default_text_size: ::iced::Pixels(13.5 as f32),
antialiasing: true,
 ..::std::default::Default::default() }).default_font(Self::default_font())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/Geist[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/GeistMono[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/NotoColorEmoji.ttf").as_slice())
.presets([::iced::Preset::new("ui_offline", Self::__preset_0), ::iced::Preset::new("ui_palette_open", Self::__preset_1), ::iced::Preset::new("ui_settings", Self::__preset_2), ::iced::Preset::new("ui_component_error", Self::__preset_3), ::iced::Preset::new("ui_launch", Self::__preset_4), ::iced::Preset::new("ui_welcome_qr", Self::__preset_5), ::iced::Preset::new("ui_console_no_account", Self::__preset_6), ::iced::Preset::new("ui_pick_probe", Self::__preset_7), ::iced::Preset::new("ui_palette_overlay", Self::__preset_8), ::iced::Preset::new("ui_tray_live", Self::__preset_9), ::iced::Preset::new("ui_huddle_sharing", Self::__preset_10), ::iced::Preset::new("ui_tray_reconnect", Self::__preset_11), ::iced::Preset::new("ui_live_run_seated", Self::__preset_12), ::iced::Preset::new("ui_ceremony_on_settings", Self::__preset_13)]) }
#[cfg(test)]
fn __ice_test_mount_16(&self, window: ::crate::shell::WindowKey) -> __IceElement<'_, __DucktapeMessage> { let __ice_palette = self.__palette(window); let __ice_app_theme = Self::__app_theme(__ice_palette);  let __ice_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 700, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __DucktapeMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 701, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/beneath", "Ducktape"); { let __a11y_key = __ice_node_scope.clone(); let __a11y_id = ::ui_lang_runtime::StableId::new(&__a11y_key); let __disabled = false; let __activate = __DucktapeMessage::ToggleChannelCreate; let __button_content: __IceElement<'_, __DucktapeMessage> = ::iced::widget::text("Create a channel").size(12.5).font(::iced::Font { weight: ::iced::font::Weight::Semibold, ..Self::default_font() }).into(); let __button = ::iced::widget::button(__button_content).padding(::iced::Padding { top: 11.0, right: 16.0, bottom: 11.0, left: 16.0 }).on_press_maybe(if __disabled { None } else { Some(__activate.clone()) }).style(move |__theme, __status| { let mut __style = ::iced::widget::button::primary(__theme, __status); let __background: Option<::iced::Color> = match __status { ::iced::widget::button::Status::Hovered => Some(__ice_palette.colors[8]), ::iced::widget::button::Status::Pressed => Some({ let mut __color = __ice_palette.colors[7]; __color.a = 0.800000; __color }), ::iced::widget::button::Status::Disabled => Some(__ice_palette.colors[7]), _ => Some(__ice_palette.colors[7]) }; if let Some(__background) = __background { __style.background = Some(::iced::Background::Color(__background)); } __style.text_color = __ice_palette.colors[9]; __style.border.radius = 9.0.into(); if matches!(__status, ::iced::widget::button::Status::Disabled) { __style.background = Some(__ice_palette.colors[10].into()); __style.text_color = __ice_palette.colors[11]; } __style }); ::ui_lang_runtime::accessible(__button, __a11y_id, ::ui_lang_runtime::Role::Button).logical_id_maybe(::core::cfg!(test).then_some(&*__a11y_key)).focus_id(::iced::widget::Id::from(__a11y_key)).label("Create a channel").disabled(__disabled).on_activate_maybe(if __disabled { None } else { Some(__activate) }).focus_ring(__ice_palette.colors[42], 9.0).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 702, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/overlays", "Ducktape"); { let __ice_component_04f7665726c61794c61796572_scope_11316 = __ice_node_scope.clone(); ::ui_lang_runtime::rev_memo(1212u64, (self.__ice_rev[104], self.__ice_rev[113], self.__ice_rev[114], self.__ice_rev[115], self.__ice_rev[116], self.__ice_rev[43], __ice_palette.name), ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_188(__ice_palette, __ice_component_04f7665726c61794c61796572_scope_11316))).into() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); let __layout = ::ui_lang_runtime::zstack(__children).width(::iced::Fill).height(::iced::Fill); let __content = ::iced::widget::container(__layout); let __layout_content: __IceElement<'_, __DucktapeMessage> = __content.into(); let __a11y_key = format!("{}/@layout:700", "Ducktape"); ::ui_lang_runtime::accessible(__layout_content, ::ui_lang_runtime::StableId::new(&__a11y_key), ::ui_lang_runtime::Role::GenericContainer).logical_id_maybe(::core::cfg!(test).then_some(__a11y_key)).into() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
};    ::ui_lang_runtime::navigation(__ice_content, __DucktapeMessage::__AccessibilityFocusNext(::std::option::Option::Some(window)), __DucktapeMessage::__AccessibilityFocusPrevious(::std::option::Option::Some(window))).in_window(window).into() }
#[cfg(test)]
fn __ice_test_program_16() -> ::iced::Daemon<impl ::iced::Program<State = Self, Message = __DucktapeMessage, Theme = ::iced::Theme>> { ::iced::daemon(Self::__boot, Self::__update, Self::__ice_test_mount_16).title(Self::__title).subscription(Self::__subscription).theme(Self::__theme).style(Self::__style).settings(::iced::Settings { 
id: ::std::option::Option::Some("dev.ducktape.app".to_owned()),
default_text_size: ::iced::Pixels(13.5 as f32),
antialiasing: true,
 ..::std::default::Default::default() }).default_font(Self::default_font())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/Geist[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/GeistMono[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/NotoColorEmoji.ttf").as_slice())
.presets([::iced::Preset::new("ui_offline", Self::__preset_0), ::iced::Preset::new("ui_palette_open", Self::__preset_1), ::iced::Preset::new("ui_settings", Self::__preset_2), ::iced::Preset::new("ui_component_error", Self::__preset_3), ::iced::Preset::new("ui_launch", Self::__preset_4), ::iced::Preset::new("ui_welcome_qr", Self::__preset_5), ::iced::Preset::new("ui_console_no_account", Self::__preset_6), ::iced::Preset::new("ui_pick_probe", Self::__preset_7), ::iced::Preset::new("ui_palette_overlay", Self::__preset_8), ::iced::Preset::new("ui_tray_live", Self::__preset_9), ::iced::Preset::new("ui_huddle_sharing", Self::__preset_10), ::iced::Preset::new("ui_tray_reconnect", Self::__preset_11), ::iced::Preset::new("ui_live_run_seated", Self::__preset_12), ::iced::Preset::new("ui_ceremony_on_settings", Self::__preset_13)]) }
#[cfg(test)]
fn __ice_test_mount_19(&self, window: ::crate::shell::WindowKey) -> __IceElement<'_, __DucktapeMessage> { let __ice_palette = self.__palette(window); let __ice_app_theme = Self::__app_theme(__ice_palette);  let __ice_content: __IceElement<'_, __DucktapeMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("tests/app.ice", 819, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __DucktapeMessage> = { let __ice_node_scope = format!("{}/huddle", "Ducktape"); ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_208(__ice_palette, __ice_node_scope.clone())) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
};    ::ui_lang_runtime::navigation(__ice_content, __DucktapeMessage::__AccessibilityFocusNext(::std::option::Option::Some(window)), __DucktapeMessage::__AccessibilityFocusPrevious(::std::option::Option::Some(window))).in_window(window).into() }
#[cfg(test)]
fn __ice_test_program_19() -> ::iced::Daemon<impl ::iced::Program<State = Self, Message = __DucktapeMessage, Theme = ::iced::Theme>> { ::iced::daemon(Self::__boot, Self::__update, Self::__ice_test_mount_19).title(Self::__title).subscription(Self::__subscription).theme(Self::__theme).style(Self::__style).settings(::iced::Settings { 
id: ::std::option::Option::Some("dev.ducktape.app".to_owned()),
default_text_size: ::iced::Pixels(13.5 as f32),
antialiasing: true,
 ..::std::default::Default::default() }).default_font(Self::default_font())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/Geist[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/GeistMono[wght].ttf").as_slice())
.font(include_bytes!("../../../crates/views/support/design/assets/fonts/NotoColorEmoji.ttf").as_slice())
.presets([::iced::Preset::new("ui_offline", Self::__preset_0), ::iced::Preset::new("ui_palette_open", Self::__preset_1), ::iced::Preset::new("ui_settings", Self::__preset_2), ::iced::Preset::new("ui_component_error", Self::__preset_3), ::iced::Preset::new("ui_launch", Self::__preset_4), ::iced::Preset::new("ui_welcome_qr", Self::__preset_5), ::iced::Preset::new("ui_console_no_account", Self::__preset_6), ::iced::Preset::new("ui_pick_probe", Self::__preset_7), ::iced::Preset::new("ui_palette_overlay", Self::__preset_8), ::iced::Preset::new("ui_tray_live", Self::__preset_9), ::iced::Preset::new("ui_huddle_sharing", Self::__preset_10), ::iced::Preset::new("ui_tray_reconnect", Self::__preset_11), ::iced::Preset::new("ui_live_run_seated", Self::__preset_12), ::iced::Preset::new("ui_ceremony_on_settings", Self::__preset_13)]) }
}
#[cfg(test)]
mod __ice_tests {
use super::*;
#[test]
fn __ice_agent_inspect() { ::ui_lang_runtime::testing::agent_inspect(|| Ducktape::__program(), "app.ice"); }
#[test]
fn __ice_view_fits_default_stack() {
::std::thread::Builder::new().stack_size(4 * 1024 * 1024).spawn(|| {
let (__app, _) = Ducktape::__boot();
let _ = __app.__view(::crate::shell::WindowKey::unique());
let (__app, _) = Ducktape::__preset_0();
let _ = __app.__view(::crate::shell::WindowKey::unique());
let (__app, _) = Ducktape::__preset_1();
let _ = __app.__view(::crate::shell::WindowKey::unique());
let (__app, _) = Ducktape::__preset_2();
let _ = __app.__view(::crate::shell::WindowKey::unique());
let (__app, _) = Ducktape::__preset_3();
let _ = __app.__view(::crate::shell::WindowKey::unique());
let (__app, _) = Ducktape::__preset_4();
let _ = __app.__view(::crate::shell::WindowKey::unique());
let (__app, _) = Ducktape::__preset_5();
let _ = __app.__view(::crate::shell::WindowKey::unique());
let (__app, _) = Ducktape::__preset_6();
let _ = __app.__view(::crate::shell::WindowKey::unique());
let (__app, _) = Ducktape::__preset_7();
let _ = __app.__view(::crate::shell::WindowKey::unique());
let (__app, _) = Ducktape::__preset_8();
let _ = __app.__view(::crate::shell::WindowKey::unique());
let (__app, _) = Ducktape::__preset_9();
let _ = __app.__view(::crate::shell::WindowKey::unique());
let (__app, _) = Ducktape::__preset_10();
let _ = __app.__view(::crate::shell::WindowKey::unique());
let (__app, _) = Ducktape::__preset_11();
let _ = __app.__view(::crate::shell::WindowKey::unique());
let (__app, _) = Ducktape::__preset_12();
let _ = __app.__view(::crate::shell::WindowKey::unique());
let (__app, _) = Ducktape::__preset_13();
let _ = __app.__view(::crate::shell::WindowKey::unique());
}).unwrap().join().unwrap();
}
#[test]
fn palette_escape_contract() {
let __config = ::ui_lang_runtime::testing::Config::new("palette_escape_contract").source(::ui_lang_runtime::testing::Location::new("tests/app.ice", 39, 1, "test palette_escape_contract")).viewport(1120.0f32, 720.0f32).preset("ui_palette_open");
let mut __test = ::ui_lang_runtime::testing::Driver::new(Ducktape::__ice_test_program_0(), __config);
::ui_lang_runtime::testing::step("palette_escape_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 103, 3, "expect palette_open"), || {
let __actual = __test.state().palette_open; __test.check(__actual, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 103, 3, "expect palette_open"));
});
::ui_lang_runtime::testing::step("palette_escape_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 104, 3, "expect exists palette"), || {
let __target = "Ducktape".to_owned() + "/workspace-tabs/palette-input"; __test.check_exists(&__target, true, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 104, 3, "expect exists palette"));
});
::ui_lang_runtime::testing::step("palette_escape_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 105, 3, "click palette"), || {
let __target = "Ducktape".to_owned() + "/workspace-tabs/palette-input"; let _ = __test.perform_action(::ui_lang_runtime::testing::Action::Click { target: __target.to_owned(), button: ::ui_lang_runtime::testing::MouseButton::Left, count: 1 }, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 105, 3, "click palette"));
});
::ui_lang_runtime::testing::step("palette_escape_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 106, 3, "key escape"), || {
let _ = __test.perform_action(::ui_lang_runtime::testing::Action::Key(::ui_lang_runtime::testing::Key::named(::iced::keyboard::key::Named::Escape)), ::ui_lang_runtime::testing::Location::new("tests/app.ice", 106, 3, "key escape"));
});
::ui_lang_runtime::testing::step("palette_escape_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 107, 3, "expect !palette_open"), || {
let __actual = !__test.state().palette_open; __test.check(__actual, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 107, 3, "expect !palette_open"));
});
::ui_lang_runtime::testing::step("palette_escape_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 108, 3, "expect missing palette"), || {
let __target = "Ducktape".to_owned() + "/workspace-tabs/palette-input"; __test.check_exists(&__target, false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 108, 3, "expect missing palette"));
});
}
#[test]
fn channel_draft_contract() {
let __config = ::ui_lang_runtime::testing::Config::new("channel_draft_contract").source(::ui_lang_runtime::testing::Location::new("tests/app.ice", 110, 1, "test channel_draft_contract")).viewport(480.0f32, 240.0f32).preset("ui_offline");
let mut __test = ::ui_lang_runtime::testing::Driver::new(Ducktape::__ice_test_program_1(), __config);
::ui_lang_runtime::testing::step("channel_draft_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 121, 3, "expect (channel_draft == \"\")"), || {
let __left = __test.state().channel_draft.to_owned(); let __right = "".to_owned(); __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 121, 3, "expect (channel_draft == \"\")"));
});
::ui_lang_runtime::testing::step("channel_draft_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 122, 3, "click draft"), || {
let __target = "Ducktape".to_owned() + "/channel-field/root/draft"; let _ = __test.perform_action(::ui_lang_runtime::testing::Action::Click { target: __target.to_owned(), button: ::ui_lang_runtime::testing::MouseButton::Left, count: 1 }, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 122, 3, "click draft"));
});
::ui_lang_runtime::testing::step("channel_draft_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 123, 3, "type \"general\""), || {
let __value = "general".to_owned(); let _ = __test.perform_action(::ui_lang_runtime::testing::Action::Type(__value), ::ui_lang_runtime::testing::Location::new("tests/app.ice", 123, 3, "type \"general\""));
});
::ui_lang_runtime::testing::step("channel_draft_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 124, 3, "expect (draft.value == \"general\")"), || {
let __left = ({ let __target_path = "Ducktape".to_owned() + "/channel-field/root/draft"; __test.target(&__target_path, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 124, 3, "expect (draft.value == \"general\")")) }).value(); let __right = "general".to_owned(); __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 124, 3, "expect (draft.value == \"general\")"));
});
::ui_lang_runtime::testing::step("channel_draft_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 125, 3, "expect (channel_draft == \"general\")"), || {
let __left = __test.state().channel_draft.to_owned(); let __right = "general".to_owned(); __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 125, 3, "expect (channel_draft == \"general\")"));
});
::ui_lang_runtime::testing::step("channel_draft_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 126, 3, "key backspace"), || {
let _ = __test.perform_action(::ui_lang_runtime::testing::Action::Key(::ui_lang_runtime::testing::Key::named(::iced::keyboard::key::Named::Backspace)), ::ui_lang_runtime::testing::Location::new("tests/app.ice", 126, 3, "key backspace"));
});
::ui_lang_runtime::testing::step("channel_draft_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 127, 3, "expect (draft.value == \"genera\")"), || {
let __left = ({ let __target_path = "Ducktape".to_owned() + "/channel-field/root/draft"; __test.target(&__target_path, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 127, 3, "expect (draft.value == \"genera\")")) }).value(); let __right = "genera".to_owned(); __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 127, 3, "expect (draft.value == \"genera\")"));
});
::ui_lang_runtime::testing::step("channel_draft_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 128, 3, "expect (channel_draft == \"genera\")"), || {
let __left = __test.state().channel_draft.to_owned(); let __right = "genera".to_owned(); __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 128, 3, "expect (channel_draft == \"genera\")"));
});
::ui_lang_runtime::testing::step("channel_draft_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 129, 3, "dispatch toggle_channel_create_members_only()"), || {
let __message = __DucktapeMessage::ToggleChannelCreateMembersOnly; __test.dispatch(__message, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 129, 3, "dispatch toggle_channel_create_members_only()"));
});
::ui_lang_runtime::testing::step("channel_draft_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 130, 3, "expect channel_create_members_only"), || {
let __actual = __test.state().channel_create_members_only; __test.check(__actual, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 130, 3, "expect channel_create_members_only"));
});
}
#[test]
fn shared_components_contract() {
let __config = ::ui_lang_runtime::testing::Config::new("shared_components_contract").source(::ui_lang_runtime::testing::Location::new("tests/app.ice", 132, 1, "test shared_components_contract")).viewport(560.0f32, 360.0f32).preset("ui_component_error");
let mut __test = ::ui_lang_runtime::testing::Driver::new(Ducktape::__ice_test_program_2(), __config);
::ui_lang_runtime::testing::step("shared_components_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 156, 3, "expect text \"Shared components\" within library"), || {
let __value = "Shared components".to_owned(); let __within = "Ducktape".to_owned() + "/surface"; __test.check_text(&__value, ::std::option::Option::Some(&__within), false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 156, 3, "expect text \"Shared components\" within library"));
});
::ui_lang_runtime::testing::step("shared_components_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 157, 3, "expect text \"Connection failed\" within alert"), || {
let __value = "Connection failed".to_owned(); let __within = "Ducktape".to_owned() + "/surface/library/root/alert/root"; __test.check_text(&__value, ::std::option::Option::Some(&__within), false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 157, 3, "expect text \"Connection failed\" within alert"));
});
::ui_lang_runtime::testing::step("shared_components_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 158, 3, "expect text \"Ready\" within badge"), || {
let __value = "Ready".to_owned(); let __within = "Ducktape".to_owned() + "/surface/library/root/badge/root"; __test.check_text(&__value, ::std::option::Option::Some(&__within), false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 158, 3, "expect text \"Ready\" within badge"));
});
::ui_lang_runtime::testing::step("shared_components_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 159, 3, "expect text \"Esc\" within kbd"), || {
let __value = "Esc".to_owned(); let __within = "Ducktape".to_owned() + "/surface/library/root/kbd/root"; __test.check_text(&__value, ::std::option::Option::Some(&__within), false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 159, 3, "expect text \"Esc\" within kbd"));
});
::ui_lang_runtime::testing::step("shared_components_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 160, 3, "expect alert.width ~= (library.width - 40.0)"), || {
let __left = (({ let __target_path = "Ducktape".to_owned() + "/surface/library/root/alert/root"; __test.target(&__target_path, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 160, 3, "expect alert.width ~= (library.width - 40.0)")) }).width()) as f64; let __right = ((({ let __target_path = "Ducktape".to_owned() + "/surface"; __test.target(&__target_path, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 160, 3, "expect alert.width ~= (library.width - 40.0)")) }).width() - 40.0)) as f64; __test.check_approx(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 160, 3, "expect alert.width ~= (library.width - 40.0)"));
});
::ui_lang_runtime::testing::step("shared_components_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 161, 3, "expect (alert.border.color == color.rgb8(239, 214, 211))"), || {
let __left = (({ let __target_path = "Ducktape".to_owned() + "/surface/library/root/alert/root"; __test.target(&__target_path, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 161, 3, "expect (alert.border.color == color.rgb8(239, 214, 211))")) }).border()).color; let __right = ::iced::Color::from_rgb8(239u8, 214u8, 211u8); __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 161, 3, "expect (alert.border.color == color.rgb8(239, 214, 211))"));
});
::ui_lang_runtime::testing::step("shared_components_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 162, 3, "expect alert.border.width ~= 1.0"), || {
let __left = ((({ let __target_path = "Ducktape".to_owned() + "/surface/library/root/alert/root"; __test.target(&__target_path, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 162, 3, "expect alert.border.width ~= 1.0")) }).border()).width as f64) as f64; let __right = (1.0) as f64; __test.check_approx(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 162, 3, "expect alert.border.width ~= 1.0"));
});
::ui_lang_runtime::testing::step("shared_components_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 163, 3, "expect (alert.border.radius == radius(11.0))"), || {
let __left = (({ let __target_path = "Ducktape".to_owned() + "/surface/library/root/alert/root"; __test.target(&__target_path, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 163, 3, "expect (alert.border.radius == radius(11.0))")) }).border()).radius; let __right = ::iced::border::radius((11.0) as f32); __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 163, 3, "expect (alert.border.radius == radius(11.0))"));
});
::ui_lang_runtime::testing::step("shared_components_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 164, 3, "expect (dismiss.background == background.color(color.rgb8(38, 37, 31)))"), || {
let __left = ({ let __target_path = "Ducktape".to_owned() + "/surface/library/root/dismiss"; __test.target(&__target_path, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 164, 3, "expect (dismiss.background == background.color(color.rgb8(38, 37, 31)))")) }).background(); let __right = ::iced::Background::Color(::iced::Color::from_rgb8(38u8, 37u8, 31u8)); __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 164, 3, "expect (dismiss.background == background.color(color.rgb8(38, 37, 31)))"));
});
::ui_lang_runtime::testing::step("shared_components_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 165, 3, "expect (dismiss.border.radius == radius(9.0))"), || {
let __left = (({ let __target_path = "Ducktape".to_owned() + "/surface/library/root/dismiss"; __test.target(&__target_path, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 165, 3, "expect (dismiss.border.radius == radius(9.0))")) }).border()).radius; let __right = ::iced::border::radius((9.0) as f32); __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 165, 3, "expect (dismiss.border.radius == radius(9.0))"));
});
::ui_lang_runtime::testing::step("shared_components_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 166, 3, "click dismiss"), || {
let __target = "Ducktape".to_owned() + "/surface/library/root/dismiss"; let _ = __test.perform_action(::ui_lang_runtime::testing::Action::Click { target: __target.to_owned(), button: ::ui_lang_runtime::testing::MouseButton::Left, count: 1 }, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 166, 3, "click dismiss"));
});
::ui_lang_runtime::testing::step("shared_components_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 167, 3, "expect (error == \"\")"), || {
let __left = __test.state().error.to_owned(); let __right = "".to_owned(); __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 167, 3, "expect (error == \"\")"));
});
}
#[test]
fn minimum_window_layout_contract() {
let __config = ::ui_lang_runtime::testing::Config::new("minimum_window_layout_contract").source(::ui_lang_runtime::testing::Location::new("tests/app.ice", 169, 1, "test minimum_window_layout_contract")).viewport(1280.0f32, 800.0f32).preset("ui_offline");
let mut __test = ::ui_lang_runtime::testing::Driver::new(Ducktape::__ice_test_program_3(), __config);
::ui_lang_runtime::testing::step("minimum_window_layout_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 228, 3, "expect titlebar.height ~= 40.0"), || {
let __left = (({ let __target_path = "Ducktape".to_owned() + "/workspace-tabs/titlebar/root"; __test.target(&__target_path, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 228, 3, "expect titlebar.height ~= 40.0")) }).height()) as f64; let __right = (40.0) as f64; __test.check_approx(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 228, 3, "expect titlebar.height ~= 40.0"));
});
::ui_lang_runtime::testing::step("minimum_window_layout_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 229, 3, "expect rail.width ~= 74.0"), || {
let __left = (({ let __target_path = "Ducktape".to_owned() + "/workspace-tabs/rail/root"; __test.target(&__target_path, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 229, 3, "expect rail.width ~= 74.0")) }).width()) as f64; let __right = (74.0) as f64; __test.check_approx(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 229, 3, "expect rail.width ~= 74.0"));
});
::ui_lang_runtime::testing::step("minimum_window_layout_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 230, 3, "expect rail.y ~= titlebar.bottom"), || {
let __left = (({ let __target_path = "Ducktape".to_owned() + "/workspace-tabs/rail/root"; __test.target(&__target_path, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 230, 3, "expect rail.y ~= titlebar.bottom")) }).y()) as f64; let __right = (({ let __target_path = "Ducktape".to_owned() + "/workspace-tabs/titlebar/root"; __test.target(&__target_path, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 230, 3, "expect rail.y ~= titlebar.bottom")) }).bottom()) as f64; __test.check_approx(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 230, 3, "expect rail.y ~= titlebar.bottom"));
});
::ui_lang_runtime::testing::step("minimum_window_layout_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 231, 3, "expect content.x ~= (rail.right + 1.0)"), || {
let __left = (({ let __target_path = "Ducktape".to_owned() + "/workspace-tabs/content"; __test.target(&__target_path, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 231, 3, "expect content.x ~= (rail.right + 1.0)")) }).x()) as f64; let __right = ((({ let __target_path = "Ducktape".to_owned() + "/workspace-tabs/rail/root"; __test.target(&__target_path, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 231, 3, "expect content.x ~= (rail.right + 1.0)")) }).right() + 1.0)) as f64; __test.check_approx(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 231, 3, "expect content.x ~= (rail.right + 1.0)"));
});
::ui_lang_runtime::testing::step("minimum_window_layout_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 232, 3, "expect (content.width > 1180.0)"), || {
let __actual = ({ let __target_path = "Ducktape".to_owned() + "/workspace-tabs/content"; __test.target(&__target_path, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 232, 3, "expect (content.width > 1180.0)")) }).width() > 1180.0; __test.check(__actual, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 232, 3, "expect (content.width > 1180.0)"));
});
::ui_lang_runtime::testing::step("minimum_window_layout_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 233, 3, "expect (rail.background == background.color(color.rgb8(250, 250, 248)))"), || {
let __left = ({ let __target_path = "Ducktape".to_owned() + "/workspace-tabs/rail/root"; __test.target(&__target_path, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 233, 3, "expect (rail.background == background.color(color.rgb8(250, 250, 248)))")) }).background(); let __right = ::iced::Background::Color(::iced::Color::from_rgb8(250u8, 250u8, 248u8)); __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 233, 3, "expect (rail.background == background.color(color.rgb8(250, 250, 248)))"));
});
::ui_lang_runtime::testing::step("minimum_window_layout_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 234, 3, "expect (content.background == background.color(color.rgb8(253, 253, 251)))"), || {
let __left = ({ let __target_path = "Ducktape".to_owned() + "/workspace-tabs/content"; __test.target(&__target_path, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 234, 3, "expect (content.background == background.color(color.rgb8(253, 253, 251)))")) }).background(); let __right = ::iced::Background::Color(::iced::Color::from_rgb8(253u8, 253u8, 251u8)); __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 234, 3, "expect (content.background == background.color(color.rgb8(253, 253, 251)))"));
});
::ui_lang_runtime::testing::step("minimum_window_layout_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 235, 3, "window resize 820 540"), || {
let __width = (820) as f32; let __height = (540) as f32; let _ = __test.perform_action(::ui_lang_runtime::testing::Action::Resize(::iced::Size::new(__width, __height)), ::ui_lang_runtime::testing::Location::new("tests/app.ice", 235, 3, "window resize 820 540"));
});
::ui_lang_runtime::testing::step("minimum_window_layout_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 236, 3, "expect rail.width ~= 74.0"), || {
let __left = (({ let __target_path = "Ducktape".to_owned() + "/workspace-tabs/rail/root"; __test.target(&__target_path, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 236, 3, "expect rail.width ~= 74.0")) }).width()) as f64; let __right = (74.0) as f64; __test.check_approx(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 236, 3, "expect rail.width ~= 74.0"));
});
::ui_lang_runtime::testing::step("minimum_window_layout_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 237, 3, "expect content.x ~= (rail.right + 1.0)"), || {
let __left = (({ let __target_path = "Ducktape".to_owned() + "/workspace-tabs/content"; __test.target(&__target_path, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 237, 3, "expect content.x ~= (rail.right + 1.0)")) }).x()) as f64; let __right = ((({ let __target_path = "Ducktape".to_owned() + "/workspace-tabs/rail/root"; __test.target(&__target_path, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 237, 3, "expect content.x ~= (rail.right + 1.0)")) }).right() + 1.0)) as f64; __test.check_approx(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 237, 3, "expect content.x ~= (rail.right + 1.0)"));
});
::ui_lang_runtime::testing::step("minimum_window_layout_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 238, 3, "expect (content.width > 730.0)"), || {
let __actual = ({ let __target_path = "Ducktape".to_owned() + "/workspace-tabs/content"; __test.target(&__target_path, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 238, 3, "expect (content.width > 730.0)")) }).width() > 730.0; __test.check(__actual, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 238, 3, "expect (content.width > 730.0)"));
});
}
#[test]
fn launch_wallets_contract() {
let __config = ::ui_lang_runtime::testing::Config::new("launch_wallets_contract").source(::ui_lang_runtime::testing::Location::new("tests/app.ice", 259, 1, "test launch_wallets_contract")).viewport(480.0f32, 680.0f32).preset("ui_launch");
let mut __test = ::ui_lang_runtime::testing::Driver::new(Ducktape::__ice_test_program_4(), __config);
::ui_lang_runtime::testing::step("launch_wallets_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 313, 3, "expect exists pw"), || {
let __target = format!("{}/wallet-password", format!("{}/root", format!("{}/wallet-row({})", format!("{}/root", format!("{}/wallets", format!("{}/root", format!("{}/hub", "Ducktape".to_owned())))), "demo"))); __test.check_exists(&__target, true, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 313, 3, "expect exists pw"));
});
::ui_lang_runtime::testing::step("launch_wallets_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 314, 3, "dispatch go_restore()"), || {
let __message = __DucktapeMessage::GoRestore; __test.dispatch(__message, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 314, 3, "dispatch go_restore()"));
});
::ui_lang_runtime::testing::step("launch_wallets_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 315, 3, "expect (hub_step == HubStep.restore)"), || {
let __left = __test.state().hub_step.clone(); let __right = HubStep::Restore; __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 315, 3, "expect (hub_step == HubStep.restore)"));
});
::ui_lang_runtime::testing::step("launch_wallets_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 316, 3, "dispatch go_login()"), || {
let __message = __DucktapeMessage::GoLogin; __test.dispatch(__message, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 316, 3, "dispatch go_login()"));
});
::ui_lang_runtime::testing::step("launch_wallets_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 317, 3, "expect (hub_step == HubStep.wallets)"), || {
let __left = __test.state().hub_step.clone(); let __right = HubStep::Wallets; __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 317, 3, "expect (hub_step == HubStep.wallets)"));
});
}
#[test]
fn wallet_list_contract() {
let __config = ::ui_lang_runtime::testing::Config::new("wallet_list_contract").source(::ui_lang_runtime::testing::Location::new("tests/app.ice", 322, 1, "test wallet_list_contract")).viewport(520.0f32, 680.0f32).preset("ui_launch");
let mut __test = ::ui_lang_runtime::testing::Driver::new(Ducktape::__ice_test_program_5(), __config);
::ui_lang_runtime::testing::step("wallet_list_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 347, 3, "expect exists demo_pw"), || {
let __target = format!("{}/wallet-password", format!("{}/root", format!("{}/wallet-row({})", format!("{}/root", format!("{}/wallets", "Ducktape".to_owned())), "demo"))); __test.check_exists(&__target, true, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 347, 3, "expect exists demo_pw"));
});
::ui_lang_runtime::testing::step("wallet_list_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 348, 3, "expect missing alice_pw"), || {
let __target = format!("{}/wallet-password", format!("{}/root", format!("{}/wallet-row({})", format!("{}/root", format!("{}/wallets", "Ducktape".to_owned())), "alice"))); __test.check_exists(&__target, false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 348, 3, "expect missing alice_pw"));
});
::ui_lang_runtime::testing::step("wallet_list_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 349, 3, "expect text \"demo\" within list"), || {
let __value = "demo".to_owned(); let __within = "Ducktape".to_owned() + "/wallets/root"; __test.check_text(&__value, ::std::option::Option::Some(&__within), false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 349, 3, "expect text \"demo\" within list"));
});
::ui_lang_runtime::testing::step("wallet_list_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 350, 3, "expect text \"alice\" within list"), || {
let __value = "alice".to_owned(); let __within = "Ducktape".to_owned() + "/wallets/root"; __test.check_text(&__value, ::std::option::Option::Some(&__within), false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 350, 3, "expect text \"alice\" within list"));
});
::ui_lang_runtime::testing::step("wallet_list_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 352, 3, "expect text \"aabbccddeeff0011…\" within list"), || {
let __value = "aabbccddeeff0011…".to_owned(); let __within = "Ducktape".to_owned() + "/wallets/root"; __test.check_text(&__value, ::std::option::Option::Some(&__within), false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 352, 3, "expect text \"aabbccddeeff0011…\" within list"));
});
::ui_lang_runtime::testing::step("wallet_list_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 354, 3, "click alice_row"), || {
let __target = format!("{}/wallet-pick", format!("{}/root", format!("{}/wallet-row({})", format!("{}/root", format!("{}/wallets", "Ducktape".to_owned())), "alice"))); let _ = __test.perform_action(::ui_lang_runtime::testing::Action::Click { target: __target.to_owned(), button: ::ui_lang_runtime::testing::MouseButton::Left, count: 1 }, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 354, 3, "click alice_row"));
});
::ui_lang_runtime::testing::step("wallet_list_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 355, 3, "expect (hub_wallet_selected == \"alice\")"), || {
let __left = __test.state().hub_wallet_selected.to_owned(); let __right = "alice".to_owned(); __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 355, 3, "expect (hub_wallet_selected == \"alice\")"));
});
::ui_lang_runtime::testing::step("wallet_list_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 356, 3, "expect exists alice_pw"), || {
let __target = format!("{}/wallet-password", format!("{}/root", format!("{}/wallet-row({})", format!("{}/root", format!("{}/wallets", "Ducktape".to_owned())), "alice"))); __test.check_exists(&__target, true, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 356, 3, "expect exists alice_pw"));
});
::ui_lang_runtime::testing::step("wallet_list_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 357, 3, "expect missing demo_pw"), || {
let __target = format!("{}/wallet-password", format!("{}/root", format!("{}/wallet-row({})", format!("{}/root", format!("{}/wallets", "Ducktape".to_owned())), "demo"))); __test.check_exists(&__target, false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 357, 3, "expect missing demo_pw"));
});
::ui_lang_runtime::testing::step("wallet_list_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 358, 3, "click alice_pw"), || {
let __target = format!("{}/wallet-password", format!("{}/root", format!("{}/wallet-row({})", format!("{}/root", format!("{}/wallets", "Ducktape".to_owned())), "alice"))); let _ = __test.perform_action(::ui_lang_runtime::testing::Action::Click { target: __target.to_owned(), button: ::ui_lang_runtime::testing::MouseButton::Left, count: 1 }, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 358, 3, "click alice_pw"));
});
::ui_lang_runtime::testing::step("wallet_list_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 359, 3, "type \"hunter2-hunter2\""), || {
let __value = "hunter2-hunter2".to_owned(); let _ = __test.perform_action(::ui_lang_runtime::testing::Action::Type(__value), ::ui_lang_runtime::testing::Location::new("tests/app.ice", 359, 3, "type \"hunter2-hunter2\""));
});
::ui_lang_runtime::testing::step("wallet_list_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 360, 3, "expect component alice.pw == \"hunter2-hunter2\""), || {
let __scope = format!("{}/wallet-row({})", format!("{}/root", format!("{}/wallets", "Ducktape".to_owned())), "alice"); let __expected = "hunter2-hunter2".to_owned(); let __actual = __test.state().__ice_test_state_wallet_row(&__scope).map(|__instance| __instance.pw); let __live = __test.state().__ice_test_scopes_wallet_row(); __test.check_component_state(&__scope, __actual, __expected, false, &__live, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 360, 3, "expect component alice.pw == \"hunter2-hunter2\""));
});
}
#[test]
fn password_screen_read_only_escape_contract() {
let __config = ::ui_lang_runtime::testing::Config::new("password_screen_read_only_escape_contract").source(::ui_lang_runtime::testing::Location::new("tests/app.ice", 368, 1, "test password_screen_read_only_escape_contract")).viewport(480.0f32, 680.0f32).preset("ui_launch");
let mut __test = ::ui_lang_runtime::testing::Driver::new(Ducktape::__ice_test_program_6(), __config);
::ui_lang_runtime::testing::step("password_screen_read_only_escape_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 382, 3, "expect exists field"), || {
let __target = "Ducktape".to_owned() + "/pw/root/device-password"; __test.check_exists(&__target, true, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 382, 3, "expect exists field"));
});
::ui_lang_runtime::testing::step("password_screen_read_only_escape_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 383, 3, "expect exists go"), || {
let __target = "Ducktape".to_owned() + "/pw/root/password-submit"; __test.check_exists(&__target, true, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 383, 3, "expect exists go"));
});
::ui_lang_runtime::testing::step("password_screen_read_only_escape_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 384, 3, "expect text \"the keystore listing is unreadable\" within screen"), || {
let __value = "the keystore listing is unreadable".to_owned(); let __within = "Ducktape".to_owned() + "/pw/root"; __test.check_text(&__value, ::std::option::Option::Some(&__within), false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 384, 3, "expect text \"the keystore listing is unreadable\" within screen"));
});
::ui_lang_runtime::testing::step("password_screen_read_only_escape_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 385, 3, "expect exists skip"), || {
let __target = "Ducktape".to_owned() + "/pw/root/password-skip"; __test.check_exists(&__target, true, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 385, 3, "expect exists skip"));
});
}
#[test]
fn a_network_pick_opens_the_door_its_keystore_names() {
let __config = ::ui_lang_runtime::testing::Config::new("a_network_pick_opens_the_door_its_keystore_names").source(::ui_lang_runtime::testing::Location::new("tests/app.ice", 393, 1, "test a_network_pick_opens_the_door_its_keystore_names")).preset("ui_pick_probe");
let mut __test = ::ui_lang_runtime::testing::Driver::new(Ducktape::__program(), __config);
::ui_lang_runtime::testing::step("a_network_pick_opens_the_door_its_keystore_names", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 395, 3, "dispatch wallets_loaded(wallet_list([wallet_info(\"alice\", \"aabbccddeeff00112233\", \"encrypted\", false), wallet_info(\"demo\", \"eeff0011\", \"encrypted\", true)], \"\", true))"), || {
let __message = __DucktapeMessage::WalletsLoaded(crate::backend::wallet_list(::std::vec![crate::backend::wallet_info("alice".to_owned(), "aabbccddeeff00112233".to_owned(), "encrypted".to_owned(), false), crate::backend::wallet_info("demo".to_owned(), "eeff0011".to_owned(), "encrypted".to_owned(), true)], "".to_owned(), true)); __test.dispatch(__message, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 395, 3, "dispatch wallets_loaded(wallet_list([wallet_info(\"alice\", \"aabbccddeeff00112233\", \"encrypted\", false), wallet_info(\"demo\", \"eeff0011\", \"encrypted\", true)], \"\", true))"));
});
::ui_lang_runtime::testing::step("a_network_pick_opens_the_door_its_keystore_names", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 396, 3, "expect (hub_step == HubStep.wallets)"), || {
let __left = __test.state().hub_step.clone(); let __right = HubStep::Wallets; __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 396, 3, "expect (hub_step == HubStep.wallets)"));
});
::ui_lang_runtime::testing::step("a_network_pick_opens_the_door_its_keystore_names", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 397, 3, "expect (hub_wallet_selected == \"demo\")"), || {
let __left = __test.state().hub_wallet_selected.to_owned(); let __right = "demo".to_owned(); __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 397, 3, "expect (hub_wallet_selected == \"demo\")"));
});
::ui_lang_runtime::testing::step("a_network_pick_opens_the_door_its_keystore_names", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 398, 3, "expect (mutation_phase == MutationPhase.idle)"), || {
let __left = __test.state().mutation_phase.clone(); let __right = MutationPhase::Idle; __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 398, 3, "expect (mutation_phase == MutationPhase.idle)"));
});
::ui_lang_runtime::testing::step("a_network_pick_opens_the_door_its_keystore_names", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 399, 3, "dispatch wallets_loaded(wallet_list([], \"the keystore listing is unreadable\", true))"), || {
let __message = __DucktapeMessage::WalletsLoaded(crate::backend::wallet_list(::std::vec::Vec::new(), "the keystore listing is unreadable".to_owned(), true)); __test.dispatch(__message, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 399, 3, "dispatch wallets_loaded(wallet_list([], \"the keystore listing is unreadable\", true))"));
});
::ui_lang_runtime::testing::step("a_network_pick_opens_the_door_its_keystore_names", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 400, 3, "expect (hub_step == HubStep.password)"), || {
let __left = __test.state().hub_step.clone(); let __right = HubStep::Password; __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 400, 3, "expect (hub_step == HubStep.password)"));
});
::ui_lang_runtime::testing::step("a_network_pick_opens_the_door_its_keystore_names", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 401, 3, "expect (hub_wallet_selected == \"\")"), || {
let __left = __test.state().hub_wallet_selected.to_owned(); let __right = "".to_owned(); __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 401, 3, "expect (hub_wallet_selected == \"\")"));
});
::ui_lang_runtime::testing::step("a_network_pick_opens_the_door_its_keystore_names", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 402, 3, "expect (onboarding_error == \"the keystore listing is unreadable\")"), || {
let __left = __test.state().onboarding_error.to_owned(); let __right = "the keystore listing is unreadable".to_owned(); __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 402, 3, "expect (onboarding_error == \"the keystore listing is unreadable\")"));
});
::ui_lang_runtime::testing::step("a_network_pick_opens_the_door_its_keystore_names", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 403, 3, "dispatch pick_network(\"remote\")"), || {
let __message = __DucktapeMessage::PickNetwork("remote".to_owned()); __test.dispatch(__message, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 403, 3, "dispatch pick_network(\"remote\")"));
});
::ui_lang_runtime::testing::step("a_network_pick_opens_the_door_its_keystore_names", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 404, 3, "dispatch wallets_loaded(wallet_list([], \"this node could not be reached\", false))"), || {
let __message = __DucktapeMessage::WalletsLoaded(crate::backend::wallet_list(::std::vec::Vec::new(), "this node could not be reached".to_owned(), false)); __test.dispatch(__message, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 404, 3, "dispatch wallets_loaded(wallet_list([], \"this node could not be reached\", false))"));
});
::ui_lang_runtime::testing::step("a_network_pick_opens_the_door_its_keystore_names", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 405, 3, "expect (hub_step == HubStep.password)"), || {
let __left = __test.state().hub_step.clone(); let __right = HubStep::Password; __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 405, 3, "expect (hub_step == HubStep.password)"));
});
::ui_lang_runtime::testing::step("a_network_pick_opens_the_door_its_keystore_names", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 406, 3, "expect (onboarding_error == \"this node could not be reached\")"), || {
let __left = __test.state().onboarding_error.to_owned(); let __right = "this node could not be reached".to_owned(); __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 406, 3, "expect (onboarding_error == \"this node could not be reached\")"));
});
::ui_lang_runtime::testing::step("a_network_pick_opens_the_door_its_keystore_names", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 407, 3, "expect (mutation_phase == MutationPhase.idle)"), || {
let __left = __test.state().mutation_phase.clone(); let __right = MutationPhase::Idle; __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 407, 3, "expect (mutation_phase == MutationPhase.idle)"));
});
}
#[test]
fn phrase_screen_shows_all_24_words() {
let __config = ::ui_lang_runtime::testing::Config::new("phrase_screen_shows_all_24_words").source(::ui_lang_runtime::testing::Location::new("tests/app.ice", 414, 1, "test phrase_screen_shows_all_24_words")).viewport(480.0f32, 680.0f32).preset("ui_launch");
let mut __test = ::ui_lang_runtime::testing::Driver::new(Ducktape::__ice_test_program_8(), __config);
::ui_lang_runtime::testing::step("phrase_screen_shows_all_24_words", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 426, 3, "expect exists go"), || {
let __target = "Ducktape".to_owned() + "/phrase/root/phrase-continue"; __test.check_exists(&__target, true, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 426, 3, "expect exists go"));
});
::ui_lang_runtime::testing::step("phrase_screen_shows_all_24_words", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 427, 3, "expect text \"abandon\" within screen"), || {
let __value = "abandon".to_owned(); let __within = "Ducktape".to_owned() + "/phrase/root"; __test.check_text(&__value, ::std::option::Option::Some(&__within), false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 427, 3, "expect text \"abandon\" within screen"));
});
::ui_lang_runtime::testing::step("phrase_screen_shows_all_24_words", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 428, 3, "expect text \"absurd\" within screen"), || {
let __value = "absurd".to_owned(); let __within = "Ducktape".to_owned() + "/phrase/root"; __test.check_text(&__value, ::std::option::Option::Some(&__within), false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 428, 3, "expect text \"absurd\" within screen"));
});
::ui_lang_runtime::testing::step("phrase_screen_shows_all_24_words", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 429, 3, "expect text \"unaware\" within screen"), || {
let __value = "unaware".to_owned(); let __within = "Ducktape".to_owned() + "/phrase/root"; __test.check_text(&__value, ::std::option::Option::Some(&__within), false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 429, 3, "expect text \"unaware\" within screen"));
});
::ui_lang_runtime::testing::step("phrase_screen_shows_all_24_words", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 430, 3, "expect text \"after this ceremony, the app never shows it again\" within screen"), || {
let __value = "after this ceremony, the app never shows it again".to_owned(); let __within = "Ducktape".to_owned() + "/phrase/root"; __test.check_text(&__value, ::std::option::Option::Some(&__within), false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 430, 3, "expect text \"after this ceremony, the app never shows it again\" within screen"));
});
::ui_lang_runtime::testing::step("phrase_screen_shows_all_24_words", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 431, 3, "capture launch_phrase_light"), || {
let _ = __test.perform_action(::ui_lang_runtime::testing::Action::Capture("launch_phrase_light".to_owned()), ::ui_lang_runtime::testing::Location::new("tests/app.ice", 431, 3, "capture launch_phrase_light"));
});
::ui_lang_runtime::testing::step("phrase_screen_shows_all_24_words", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 432, 3, "click go"), || {
let __target = "Ducktape".to_owned() + "/phrase/root/phrase-continue"; let _ = __test.perform_action(::ui_lang_runtime::testing::Action::Click { target: __target.to_owned(), button: ::ui_lang_runtime::testing::MouseButton::Left, count: 1 }, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 432, 3, "click go"));
});
::ui_lang_runtime::testing::step("phrase_screen_shows_all_24_words", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 433, 3, "expect (hub_step == HubStep.confirm)"), || {
let __left = __test.state().hub_step.clone(); let __right = HubStep::Confirm; __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 433, 3, "expect (hub_step == HubStep.confirm)"));
});
}
#[test]
fn confirm_screen_asks_three_words_back() {
let __config = ::ui_lang_runtime::testing::Config::new("confirm_screen_asks_three_words_back").source(::ui_lang_runtime::testing::Location::new("tests/app.ice", 438, 1, "test confirm_screen_asks_three_words_back")).viewport(480.0f32, 680.0f32).preset("ui_launch");
let mut __test = ::ui_lang_runtime::testing::Driver::new(Ducktape::__ice_test_program_9(), __config);
::ui_lang_runtime::testing::step("confirm_screen_asks_three_words_back", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 453, 3, "expect exists field"), || {
let __target = "Ducktape".to_owned() + "/confirm/root/confirm-words"; __test.check_exists(&__target, true, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 453, 3, "expect exists field"));
});
::ui_lang_runtime::testing::step("confirm_screen_asks_three_words_back", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 454, 3, "expect text \"Type words 5, 12 and 20 — in that order, separated by spaces.\" within screen"), || {
let __value = "Type words 5, 12 and 20 — in that order, separated by spaces.".to_owned(); let __within = "Ducktape".to_owned() + "/confirm/root"; __test.check_text(&__value, ::std::option::Option::Some(&__within), false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 454, 3, "expect text \"Type words 5, 12 and 20 — in that order, separated by spaces.\" within screen"));
});
::ui_lang_runtime::testing::step("confirm_screen_asks_three_words_back", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 455, 3, "capture launch_confirm_light"), || {
let _ = __test.perform_action(::ui_lang_runtime::testing::Action::Capture("launch_confirm_light".to_owned()), ::ui_lang_runtime::testing::Location::new("tests/app.ice", 455, 3, "capture launch_confirm_light"));
});
::ui_lang_runtime::testing::step("confirm_screen_asks_three_words_back", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 456, 3, "click back"), || {
let __target = "Ducktape".to_owned() + "/confirm/root/confirm-back"; let _ = __test.perform_action(::ui_lang_runtime::testing::Action::Click { target: __target.to_owned(), button: ::ui_lang_runtime::testing::MouseButton::Left, count: 1 }, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 456, 3, "click back"));
});
::ui_lang_runtime::testing::step("confirm_screen_asks_three_words_back", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 457, 3, "expect (hub_step == HubStep.phrase)"), || {
let __left = __test.state().hub_step.clone(); let __right = HubStep::Phrase; __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 457, 3, "expect (hub_step == HubStep.phrase)"));
});
}
#[test]
fn launch_networks_empty_contract() {
let __config = ::ui_lang_runtime::testing::Config::new("launch_networks_empty_contract").source(::ui_lang_runtime::testing::Location::new("tests/app.ice", 459, 1, "test launch_networks_empty_contract")).viewport(480.0f32, 680.0f32).preset("ui_launch");
let mut __test = ::ui_lang_runtime::testing::Driver::new(Ducktape::__ice_test_program_10(), __config);
::ui_lang_runtime::testing::step("launch_networks_empty_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 514, 3, "expect exists cta"), || {
let __target = "Ducktape".to_owned() + "/hub/root/networks/root/join-cta"; __test.check_exists(&__target, true, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 514, 3, "expect exists cta"));
});
::ui_lang_runtime::testing::step("launch_networks_empty_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 515, 3, "expect exists remote_field"), || {
let __target = "Ducktape".to_owned() + "/hub/root/networks/root/remote-endpoint"; __test.check_exists(&__target, true, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 515, 3, "expect exists remote_field"));
});
::ui_lang_runtime::testing::step("launch_networks_empty_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 516, 3, "click cta"), || {
let __target = "Ducktape".to_owned() + "/hub/root/networks/root/join-cta"; let _ = __test.perform_action(::ui_lang_runtime::testing::Action::Click { target: __target.to_owned(), button: ::ui_lang_runtime::testing::MouseButton::Left, count: 1 }, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 516, 3, "click cta"));
});
::ui_lang_runtime::testing::step("launch_networks_empty_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 517, 3, "expect (hub_step == HubStep.join)"), || {
let __left = __test.state().hub_step.clone(); let __right = HubStep::Join; __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 517, 3, "expect (hub_step == HubStep.join)"));
});
}
#[test]
fn welcome_screen_contract() {
let __config = ::ui_lang_runtime::testing::Config::new("welcome_screen_contract").source(::ui_lang_runtime::testing::Location::new("tests/app.ice", 556, 1, "test welcome_screen_contract")).viewport(480.0f32, 680.0f32).preset("ui_welcome_qr");
let mut __test = ::ui_lang_runtime::testing::Driver::new(Ducktape::__ice_test_program_11(), __config);
::ui_lang_runtime::testing::step("welcome_screen_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 580, 3, "expect exists qr"), || {
let __target = "Ducktape".to_owned() + "/welcome/root/welcome-qr"; __test.check_exists(&__target, true, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 580, 3, "expect exists qr"));
});
::ui_lang_runtime::testing::step("welcome_screen_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 581, 3, "expect exists left"), || {
let __target = "Ducktape".to_owned() + "/welcome/root/welcome-left"; __test.check_exists(&__target, true, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 581, 3, "expect exists left"));
});
::ui_lang_runtime::testing::step("welcome_screen_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 582, 3, "expect missing create"), || {
let __target = "Ducktape".to_owned() + "/welcome/root/welcome-create"; __test.check_exists(&__target, false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 582, 3, "expect missing create"));
});
::ui_lang_runtime::testing::step("welcome_screen_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 583, 3, "expect text \"demo\" within screen"), || {
let __value = "demo".to_owned(); let __within = "Ducktape".to_owned() + "/welcome/root"; __test.check_text(&__value, ::std::option::Option::Some(&__within), false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 583, 3, "expect text \"demo\" within screen"));
});
::ui_lang_runtime::testing::step("welcome_screen_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 584, 3, "click cancel"), || {
let __target = "Ducktape".to_owned() + "/welcome/root/welcome-cancel"; let _ = __test.perform_action(::ui_lang_runtime::testing::Action::Click { target: __target.to_owned(), button: ::ui_lang_runtime::testing::MouseButton::Left, count: 1 }, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 584, 3, "click cancel"));
});
::ui_lang_runtime::testing::step("welcome_screen_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 585, 3, "expect (ceremony_qr == \"\")"), || {
let __left = __test.state().ceremony_qr.to_owned(); let __right = "".to_owned(); __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 585, 3, "expect (ceremony_qr == \"\")"));
});
::ui_lang_runtime::testing::step("welcome_screen_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 586, 3, "expect (ceremony_phase == \"\")"), || {
let __left = __test.state().ceremony_phase.to_owned(); let __right = "".to_owned(); __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 586, 3, "expect (ceremony_phase == \"\")"));
});
::ui_lang_runtime::testing::step("welcome_screen_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 587, 3, "expect (hub_step == HubStep.account)"), || {
let __left = __test.state().hub_step.clone(); let __right = HubStep::Account; __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 587, 3, "expect (hub_step == HubStep.account)"));
});
::ui_lang_runtime::testing::step("welcome_screen_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 589, 3, "expect exists create"), || {
let __target = "Ducktape".to_owned() + "/welcome/root/welcome-create"; __test.check_exists(&__target, true, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 589, 3, "expect exists create"));
});
::ui_lang_runtime::testing::step("welcome_screen_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 590, 3, "expect missing qr"), || {
let __target = "Ducktape".to_owned() + "/welcome/root/welcome-qr"; __test.check_exists(&__target, false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 590, 3, "expect missing qr"));
});
}
#[test]
fn network_pick_lands_on_the_welcome_step_without_an_account() {
let __config = ::ui_lang_runtime::testing::Config::new("network_pick_lands_on_the_welcome_step_without_an_account").source(::ui_lang_runtime::testing::Location::new("tests/app.ice", 596, 1, "test network_pick_lands_on_the_welcome_step_without_an_account")).preset("ui_pick_probe");
let mut __test = ::ui_lang_runtime::testing::Driver::new(Ducktape::__program(), __config);
::ui_lang_runtime::testing::step("network_pick_lands_on_the_welcome_step_without_an_account", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 598, 3, "dispatch account_probed(account_data_none(7))"), || {
let __message = __DucktapeMessage::AccountProbed(crate::backend::account_data_none(7)); __test.dispatch(__message, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 598, 3, "dispatch account_probed(account_data_none(7))"));
});
::ui_lang_runtime::testing::step("network_pick_lands_on_the_welcome_step_without_an_account", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 599, 3, "expect (hub_step == HubStep.account)"), || {
let __left = __test.state().hub_step.clone(); let __right = HubStep::Account; __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 599, 3, "expect (hub_step == HubStep.account)"));
});
::ui_lang_runtime::testing::step("network_pick_lands_on_the_welcome_step_without_an_account", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 600, 3, "expect (mutation_phase == MutationPhase.idle)"), || {
let __left = __test.state().mutation_phase.clone(); let __right = MutationPhase::Idle; __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 600, 3, "expect (mutation_phase == MutationPhase.idle)"));
});
::ui_lang_runtime::testing::step("network_pick_lands_on_the_welcome_step_without_an_account", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 601, 3, "expect (ceremony_phase == \"\")"), || {
let __left = __test.state().ceremony_phase.to_owned(); let __right = "".to_owned(); __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 601, 3, "expect (ceremony_phase == \"\")"));
});
}
#[test]
fn console_banner_names_the_missing_account() {
let __config = ::ui_lang_runtime::testing::Config::new("console_banner_names_the_missing_account").source(::ui_lang_runtime::testing::Location::new("tests/app.ice", 606, 1, "test console_banner_names_the_missing_account")).viewport(900.0f32, 300.0f32).preset("ui_console_no_account");
let mut __test = ::ui_lang_runtime::testing::Driver::new(Ducktape::__ice_test_program_13(), __config);
::ui_lang_runtime::testing::step("console_banner_names_the_missing_account", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 621, 3, "expect exists banner"), || {
let __target = "Ducktape".to_owned() + "/account-banner/root/banner"; __test.check_exists(&__target, true, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 621, 3, "expect exists banner"));
});
::ui_lang_runtime::testing::step("console_banner_names_the_missing_account", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 622, 3, "click dismiss"), || {
let __target = "Ducktape".to_owned() + "/account-banner/root/banner/dismiss"; let _ = __test.perform_action(::ui_lang_runtime::testing::Action::Click { target: __target.to_owned(), button: ::ui_lang_runtime::testing::MouseButton::Left, count: 1 }, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 622, 3, "click dismiss"));
});
::ui_lang_runtime::testing::step("console_banner_names_the_missing_account", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 623, 3, "expect (account_banner_dismissed == true)"), || {
let __left = __test.state().account_banner_dismissed; let __right = true; __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 623, 3, "expect (account_banner_dismissed == true)"));
});
::ui_lang_runtime::testing::step("console_banner_names_the_missing_account", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 624, 3, "expect missing banner"), || {
let __target = "Ducktape".to_owned() + "/account-banner/root/banner"; __test.check_exists(&__target, false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 624, 3, "expect missing banner"));
});
}
#[test]
fn ceremony_steps_drive_the_welcome() {
let __config = ::ui_lang_runtime::testing::Config::new("ceremony_steps_drive_the_welcome").source(::ui_lang_runtime::testing::Location::new("tests/app.ice", 628, 1, "test ceremony_steps_drive_the_welcome")).preset("ui_welcome_qr");
let mut __test = ::ui_lang_runtime::testing::Driver::new(Ducktape::__program(), __config);
::ui_lang_runtime::testing::step("ceremony_steps_drive_the_welcome", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 630, 3, "dispatch ceremony_stepped(ceremony_step(\"working\", \"\", \"Consenting to the new key…\"))"), || {
let __message = __DucktapeMessage::CeremonyStepped(crate::backend::ceremony_step("working".to_owned(), "".to_owned(), "Consenting to the new key…".to_owned())); __test.dispatch(__message, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 630, 3, "dispatch ceremony_stepped(ceremony_step(\"working\", \"\", \"Consenting to the new key…\"))"));
});
::ui_lang_runtime::testing::step("ceremony_steps_drive_the_welcome", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 631, 3, "expect (ceremony_phase == \"working\")"), || {
let __left = __test.state().ceremony_phase.to_owned(); let __right = "working".to_owned(); __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 631, 3, "expect (ceremony_phase == \"working\")"));
});
::ui_lang_runtime::testing::step("ceremony_steps_drive_the_welcome", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 632, 3, "expect (ceremony_qr == \"\")"), || {
let __left = __test.state().ceremony_qr.to_owned(); let __right = "".to_owned(); __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 632, 3, "expect (ceremony_qr == \"\")"));
});
::ui_lang_runtime::testing::step("ceremony_steps_drive_the_welcome", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 633, 3, "expect (ceremony_detail == \"Consenting to the new key…\")"), || {
let __left = __test.state().ceremony_detail.to_owned(); let __right = "Consenting to the new key…".to_owned(); __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 633, 3, "expect (ceremony_detail == \"Consenting to the new key…\")"));
});
::ui_lang_runtime::testing::step("ceremony_steps_drive_the_welcome", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 634, 3, "expect (mutation_phase == MutationPhase.onboarding)"), || {
let __left = __test.state().mutation_phase.clone(); let __right = MutationPhase::Onboarding; __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 634, 3, "expect (mutation_phase == MutationPhase.onboarding)"));
});
::ui_lang_runtime::testing::step("ceremony_steps_drive_the_welcome", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 635, 3, "dispatch ceremony_stepped(ceremony_step(\"failed\", \"\", \"the phone did not answer in time\"))"), || {
let __message = __DucktapeMessage::CeremonyStepped(crate::backend::ceremony_step("failed".to_owned(), "".to_owned(), "the phone did not answer in time".to_owned())); __test.dispatch(__message, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 635, 3, "dispatch ceremony_stepped(ceremony_step(\"failed\", \"\", \"the phone did not answer in time\"))"));
});
::ui_lang_runtime::testing::step("ceremony_steps_drive_the_welcome", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 636, 3, "expect (ceremony_phase == \"\")"), || {
let __left = __test.state().ceremony_phase.to_owned(); let __right = "".to_owned(); __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 636, 3, "expect (ceremony_phase == \"\")"));
});
::ui_lang_runtime::testing::step("ceremony_steps_drive_the_welcome", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 637, 3, "expect (onboarding_error == \"the phone did not answer in time\")"), || {
let __left = __test.state().onboarding_error.to_owned(); let __right = "the phone did not answer in time".to_owned(); __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 637, 3, "expect (onboarding_error == \"the phone did not answer in time\")"));
});
::ui_lang_runtime::testing::step("ceremony_steps_drive_the_welcome", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 638, 3, "expect (mutation_phase == MutationPhase.idle)"), || {
let __left = __test.state().mutation_phase.clone(); let __right = MutationPhase::Idle; __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 638, 3, "expect (mutation_phase == MutationPhase.idle)"));
});
}
#[test]
fn palette_overlay_contract() {
let __config = ::ui_lang_runtime::testing::Config::new("palette_overlay_contract").source(::ui_lang_runtime::testing::Location::new("tests/app.ice", 653, 1, "test palette_overlay_contract")).viewport(1120.0f32, 720.0f32).preset("ui_palette_overlay");
let mut __test = ::ui_lang_runtime::testing::Driver::new(Ducktape::__ice_test_program_15(), __config);
::ui_lang_runtime::testing::step("palette_overlay_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 680, 3, "expect exists field"), || {
let __target = "Ducktape".to_owned() + "/overlays/palette-input"; __test.check_exists(&__target, true, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 680, 3, "expect exists field"));
});
::ui_lang_runtime::testing::step("palette_overlay_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 681, 3, "click field"), || {
let __target = "Ducktape".to_owned() + "/overlays/palette-input"; let _ = __test.perform_action(::ui_lang_runtime::testing::Action::Click { target: __target.to_owned(), button: ::ui_lang_runtime::testing::MouseButton::Left, count: 1 }, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 681, 3, "click field"));
});
::ui_lang_runtime::testing::step("palette_overlay_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 682, 3, "type \"duck\""), || {
let __value = "duck".to_owned(); let _ = __test.perform_action(::ui_lang_runtime::testing::Action::Type(__value), ::ui_lang_runtime::testing::Location::new("tests/app.ice", 682, 3, "type \"duck\""));
});
::ui_lang_runtime::testing::step("palette_overlay_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 683, 3, "expect (palette_draft == \"duck\")"), || {
let __left = __test.state().palette_draft.to_owned(); let __right = "duck".to_owned(); __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 683, 3, "expect (palette_draft == \"duck\")"));
});
::ui_lang_runtime::testing::step("palette_overlay_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 684, 3, "key escape"), || {
let _ = __test.perform_action(::ui_lang_runtime::testing::Action::Key(::ui_lang_runtime::testing::Key::named(::iced::keyboard::key::Named::Escape)), ::ui_lang_runtime::testing::Location::new("tests/app.ice", 684, 3, "key escape"));
});
::ui_lang_runtime::testing::step("palette_overlay_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 685, 3, "expect !palette_open"), || {
let __actual = !__test.state().palette_open; __test.check(__actual, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 685, 3, "expect !palette_open"));
});
}
#[test]
fn palette_backdrop_takes_the_pointer() {
let __config = ::ui_lang_runtime::testing::Config::new("palette_backdrop_takes_the_pointer").source(::ui_lang_runtime::testing::Location::new("tests/app.ice", 696, 1, "test palette_backdrop_takes_the_pointer")).viewport(1120.0f32, 720.0f32).preset("ui_palette_overlay");
let mut __test = ::ui_lang_runtime::testing::Driver::new(Ducktape::__ice_test_program_16(), __config);
::ui_lang_runtime::testing::step("palette_backdrop_takes_the_pointer", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 726, 3, "expect palette_open"), || {
let __actual = __test.state().palette_open; __test.check(__actual, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 726, 3, "expect palette_open"));
});
::ui_lang_runtime::testing::step("palette_backdrop_takes_the_pointer", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 727, 3, "expect !channel_create_open"), || {
let __actual = !__test.state().channel_create_open; __test.check(__actual, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 727, 3, "expect !channel_create_open"));
});
::ui_lang_runtime::testing::step("palette_backdrop_takes_the_pointer", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 728, 3, "expect exists field"), || {
let __target = "Ducktape".to_owned() + "/overlays/palette-input"; __test.check_exists(&__target, true, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 728, 3, "expect exists field"));
});
::ui_lang_runtime::testing::step("palette_backdrop_takes_the_pointer", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 729, 3, "click beneath"), || {
let __target = "Ducktape".to_owned() + "/beneath"; let _ = __test.perform_action(::ui_lang_runtime::testing::Action::Click { target: __target.to_owned(), button: ::ui_lang_runtime::testing::MouseButton::Left, count: 1 }, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 729, 3, "click beneath"));
});
::ui_lang_runtime::testing::step("palette_backdrop_takes_the_pointer", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 730, 3, "expect !channel_create_open"), || {
let __actual = !__test.state().channel_create_open; __test.check(__actual, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 730, 3, "expect !channel_create_open"));
});
::ui_lang_runtime::testing::step("palette_backdrop_takes_the_pointer", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 731, 3, "expect !palette_open"), || {
let __actual = !__test.state().palette_open; __test.check(__actual, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 731, 3, "expect !palette_open"));
});
::ui_lang_runtime::testing::step("palette_backdrop_takes_the_pointer", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 732, 3, "expect missing field"), || {
let __target = "Ducktape".to_owned() + "/overlays/palette-input"; __test.check_exists(&__target, false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 732, 3, "expect missing field"));
});
}
#[test]
fn tray_menu_contract() {
let __config = ::ui_lang_runtime::testing::Config::new("tray_menu_contract").source(::ui_lang_runtime::testing::Location::new("tests/app.ice", 740, 1, "test tray_menu_contract")).preset("ui_offline");
let mut __test = ::ui_lang_runtime::testing::Driver::new(Ducktape::__program(), __config);
::ui_lang_runtime::testing::step("tray_menu_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 742, 3, "expect tray icon \"../../assets/tray-offline.rgba\""), || {
let __value = "../../assets/tray-offline.rgba".to_owned(); __test.check_tray(::ui_lang_runtime::testing::TrayField::Icon, &__value, false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 742, 3, "expect tray icon \"../../assets/tray-offline.rgba\""));
});
::ui_lang_runtime::testing::step("tray_menu_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 743, 3, "expect tray item \"No network\""), || {
let __value = "No network".to_owned(); __test.check_tray(::ui_lang_runtime::testing::TrayField::Item, &__value, false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 743, 3, "expect tray item \"No network\""));
});
::ui_lang_runtime::testing::step("tray_menu_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 744, 3, "expect tray item \"Offline\""), || {
let __value = "Offline".to_owned(); __test.check_tray(::ui_lang_runtime::testing::TrayField::Item, &__value, false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 744, 3, "expect tray item \"Offline\""));
});
::ui_lang_runtime::testing::step("tray_menu_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 745, 3, "expect no tray command \"Offline\""), || {
let __value = "Offline".to_owned(); __test.check_tray(::ui_lang_runtime::testing::TrayField::Command, &__value, true, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 745, 3, "expect no tray command \"Offline\""));
});
::ui_lang_runtime::testing::step("tray_menu_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 746, 3, "expect tray command \"Open Ducktape\""), || {
let __value = "Open Ducktape".to_owned(); __test.check_tray(::ui_lang_runtime::testing::TrayField::Command, &__value, false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 746, 3, "expect tray command \"Open Ducktape\""));
});
::ui_lang_runtime::testing::step("tray_menu_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 747, 3, "expect tray command \"Quit Ducktape\""), || {
let __value = "Quit Ducktape".to_owned(); __test.check_tray(::ui_lang_runtime::testing::TrayField::Command, &__value, false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 747, 3, "expect tray command \"Quit Ducktape\""));
});
::ui_lang_runtime::testing::step("tray_menu_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 748, 3, "expect tray item \"Appearance\""), || {
let __value = "Appearance".to_owned(); __test.check_tray(::ui_lang_runtime::testing::TrayField::Item, &__value, false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 748, 3, "expect tray item \"Appearance\""));
});
::ui_lang_runtime::testing::step("tray_menu_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 749, 3, "expect no tray item \"Notifications\""), || {
let __value = "Notifications".to_owned(); __test.check_tray(::ui_lang_runtime::testing::TrayField::Item, &__value, true, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 749, 3, "expect no tray item \"Notifications\""));
});
::ui_lang_runtime::testing::step("tray_menu_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 750, 3, "expect no tray item \"Go to\""), || {
let __value = "Go to".to_owned(); __test.check_tray(::ui_lang_runtime::testing::TrayField::Item, &__value, true, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 750, 3, "expect no tray item \"Go to\""));
});
::ui_lang_runtime::testing::step("tray_menu_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 751, 3, "expect no tray item \"Huddle\""), || {
let __value = "Huddle".to_owned(); __test.check_tray(::ui_lang_runtime::testing::TrayField::Item, &__value, true, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 751, 3, "expect no tray item \"Huddle\""));
});
::ui_lang_runtime::testing::step("tray_menu_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 752, 3, "expect no tray item \"Leave huddle\""), || {
let __value = "Leave huddle".to_owned(); __test.check_tray(::ui_lang_runtime::testing::TrayField::Item, &__value, true, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 752, 3, "expect no tray item \"Leave huddle\""));
});
::ui_lang_runtime::testing::step("tray_menu_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 753, 3, "expect no tray item \"Reconnect\""), || {
let __value = "Reconnect".to_owned(); __test.check_tray(::ui_lang_runtime::testing::TrayField::Item, &__value, true, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 753, 3, "expect no tray item \"Reconnect\""));
});
::ui_lang_runtime::testing::step("tray_menu_contract", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 754, 3, "tray choose \"Open Ducktape\""), || {
let __value = "Open Ducktape".to_owned();
let __row = __test.tray_command_row(&__value, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 754, 3, "tray choose \"Open Ducktape\""));
let __message = Ducktape::__tray_row(__row).expect("the row-to-handler table has no entry for a row declared with a route");
__test.dispatch(__message, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 754, 3, "tray choose \"Open Ducktape\""));
});
}
#[test]
fn tray_menu_reads_the_state() {
let __config = ::ui_lang_runtime::testing::Config::new("tray_menu_reads_the_state").source(::ui_lang_runtime::testing::Location::new("tests/app.ice", 776, 1, "test tray_menu_reads_the_state")).preset("ui_tray_live");
let mut __test = ::ui_lang_runtime::testing::Driver::new(Ducktape::__program(), __config);
::ui_lang_runtime::testing::step("tray_menu_reads_the_state", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 778, 3, "expect tray icon \"../../assets/tray-unread.rgba\""), || {
let __value = "../../assets/tray-unread.rgba".to_owned(); __test.check_tray(::ui_lang_runtime::testing::TrayField::Icon, &__value, false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 778, 3, "expect tray icon \"../../assets/tray-unread.rgba\""));
});
::ui_lang_runtime::testing::step("tray_menu_reads_the_state", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 779, 3, "expect tray label \"3\""), || {
let __value = "3".to_owned(); __test.check_tray(::ui_lang_runtime::testing::TrayField::Label, &__value, false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 779, 3, "expect tray label \"3\""));
});
::ui_lang_runtime::testing::step("tray_menu_reads_the_state", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 780, 3, "expect tray item \"demo\""), || {
let __value = "demo".to_owned(); __test.check_tray(::ui_lang_runtime::testing::TrayField::Item, &__value, false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 780, 3, "expect tray item \"demo\""));
});
::ui_lang_runtime::testing::step("tray_menu_reads_the_state", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 781, 3, "expect tray item \"Notifications · 3 unread\""), || {
let __value = "Notifications · 3 unread".to_owned(); __test.check_tray(::ui_lang_runtime::testing::TrayField::Item, &__value, false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 781, 3, "expect tray item \"Notifications · 3 unread\""));
});
::ui_lang_runtime::testing::step("tray_menu_reads_the_state", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 782, 3, "expect tray item \"Huddle · #general\""), || {
let __value = "Huddle · #general".to_owned(); __test.check_tray(::ui_lang_runtime::testing::TrayField::Item, &__value, false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 782, 3, "expect tray item \"Huddle · #general\""));
});
::ui_lang_runtime::testing::step("tray_menu_reads_the_state", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 783, 3, "expect tray item \"✓ Dark\""), || {
let __value = "✓ Dark".to_owned(); __test.check_tray(::ui_lang_runtime::testing::TrayField::Item, &__value, false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 783, 3, "expect tray item \"✓ Dark\""));
});
::ui_lang_runtime::testing::step("tray_menu_reads_the_state", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 784, 3, "expect no tray item \"✓ Light\""), || {
let __value = "✓ Light".to_owned(); __test.check_tray(::ui_lang_runtime::testing::TrayField::Item, &__value, true, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 784, 3, "expect no tray item \"✓ Light\""));
});
::ui_lang_runtime::testing::step("tray_menu_reads_the_state", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 785, 3, "expect tray command \"Mute\""), || {
let __value = "Mute".to_owned(); __test.check_tray(::ui_lang_runtime::testing::TrayField::Command, &__value, false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 785, 3, "expect tray command \"Mute\""));
});
::ui_lang_runtime::testing::step("tray_menu_reads_the_state", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 788, 3, "expect tray command \"Copy node key\""), || {
let __value = "Copy node key".to_owned(); __test.check_tray(::ui_lang_runtime::testing::TrayField::Command, &__value, false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 788, 3, "expect tray command \"Copy node key\""));
});
::ui_lang_runtime::testing::step("tray_menu_reads_the_state", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 789, 3, "expect tray item \"Go to\""), || {
let __value = "Go to".to_owned(); __test.check_tray(::ui_lang_runtime::testing::TrayField::Item, &__value, false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 789, 3, "expect tray item \"Go to\""));
});
::ui_lang_runtime::testing::step("tray_menu_reads_the_state", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 790, 3, "tray choose \"Mute\""), || {
let __value = "Mute".to_owned();
let __row = __test.tray_command_row(&__value, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 790, 3, "tray choose \"Mute\""));
let __message = Ducktape::__tray_row(__row).expect("the row-to-handler table has no entry for a row declared with a route");
__test.dispatch(__message, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 790, 3, "tray choose \"Mute\""));
});
::ui_lang_runtime::testing::step("tray_menu_reads_the_state", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 791, 3, "expect tray item \"Unmute\""), || {
let __value = "Unmute".to_owned(); __test.check_tray(::ui_lang_runtime::testing::TrayField::Item, &__value, false, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 791, 3, "expect tray item \"Unmute\""));
});
::ui_lang_runtime::testing::step("tray_menu_reads_the_state", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 792, 3, "expect call_muted"), || {
let __actual = __test.state().call_muted; __test.check(__actual, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 792, 3, "expect call_muted"));
});
}
#[test]
fn the_huddle_controls_survive_the_narrowest_panel() {
let __config = ::ui_lang_runtime::testing::Config::new("the_huddle_controls_survive_the_narrowest_panel").source(::ui_lang_runtime::testing::Location::new("tests/app.ice", 815, 1, "test the_huddle_controls_survive_the_narrowest_panel")).viewport(320.0f32, 640.0f32).preset("ui_huddle_sharing");
let mut __test = ::ui_lang_runtime::testing::Driver::new(Ducktape::__ice_test_program_19(), __config);
::ui_lang_runtime::testing::step("the_huddle_controls_survive_the_narrowest_panel", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 839, 3, "expect call_sharing"), || {
let __actual = __test.state().call_sharing; __test.check(__actual, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 839, 3, "expect call_sharing"));
});
::ui_lang_runtime::testing::step("the_huddle_controls_survive_the_narrowest_panel", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 840, 3, "expect ((leave.x + leave.width) <= (panel.x + panel.width))"), || {
let __actual = (({ let __target_path = "Ducktape".to_owned() + "/huddle/root/controls/root/leave"; __test.target(&__target_path, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 840, 3, "expect ((leave.x + leave.width) <= (panel.x + panel.width))")) }).x() + ({ let __target_path = "Ducktape".to_owned() + "/huddle/root/controls/root/leave"; __test.target(&__target_path, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 840, 3, "expect ((leave.x + leave.width) <= (panel.x + panel.width))")) }).width()) <= (({ let __target_path = "Ducktape".to_owned() + "/huddle/root"; __test.target(&__target_path, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 840, 3, "expect ((leave.x + leave.width) <= (panel.x + panel.width))")) }).x() + ({ let __target_path = "Ducktape".to_owned() + "/huddle/root"; __test.target(&__target_path, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 840, 3, "expect ((leave.x + leave.width) <= (panel.x + panel.width))")) }).width()); __test.check(__actual, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 840, 3, "expect ((leave.x + leave.width) <= (panel.x + panel.width))"));
});
::ui_lang_runtime::testing::step("the_huddle_controls_survive_the_narrowest_panel", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 841, 3, "expect share.width ~= 32.0"), || {
let __left = (({ let __target_path = "Ducktape".to_owned() + "/huddle/root/controls/root/share-stop"; __test.target(&__target_path, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 841, 3, "expect share.width ~= 32.0")) }).width()) as f64; let __right = (32.0) as f64; __test.check_approx(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 841, 3, "expect share.width ~= 32.0"));
});
::ui_lang_runtime::testing::step("the_huddle_controls_survive_the_narrowest_panel", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 842, 3, "capture huddle_sharing_light"), || {
let _ = __test.perform_action(::ui_lang_runtime::testing::Action::Capture("huddle_sharing_light".to_owned()), ::ui_lang_runtime::testing::Location::new("tests/app.ice", 842, 3, "capture huddle_sharing_light"));
});
}
#[test]
fn closing_the_last_window_only_unregisters_it() {
let __config = ::ui_lang_runtime::testing::Config::new("closing_the_last_window_only_unregisters_it").source(::ui_lang_runtime::testing::Location::new("tests/app.ice", 852, 1, "test closing_the_last_window_only_unregisters_it")).preset("ui_offline");
let mut __test = ::ui_lang_runtime::testing::Driver::new(Ducktape::__program(), __config);
::ui_lang_runtime::testing::step("closing_the_last_window_only_unregisters_it", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 854, 3, "tray choose \"Open Ducktape\""), || {
let __value = "Open Ducktape".to_owned();
let __row = __test.tray_command_row(&__value, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 854, 3, "tray choose \"Open Ducktape\""));
let __message = Ducktape::__tray_row(__row).expect("the row-to-handler table has no entry for a row declared with a route");
__test.dispatch(__message, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 854, 3, "tray choose \"Open Ducktape\""));
});
::ui_lang_runtime::testing::step("closing_the_last_window_only_unregisters_it", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 855, 3, "expect (onboarding_win != none)"), || {
let __left = __test.state().onboarding_win.clone(); let __right = ::std::option::Option::None; __test.check_ne(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 855, 3, "expect (onboarding_win != none)"));
});
::ui_lang_runtime::testing::step("closing_the_last_window_only_unregisters_it", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 856, 3, "window closed"), || {
let _ = __test.perform_action(::ui_lang_runtime::testing::Action::WindowClosed, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 856, 3, "window closed"));
});
::ui_lang_runtime::testing::step("closing_the_last_window_only_unregisters_it", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 857, 3, "expect (onboarding_win == none)"), || {
let __left = __test.state().onboarding_win.clone(); let __right = ::std::option::Option::None; __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 857, 3, "expect (onboarding_win == none)"));
});
::ui_lang_runtime::testing::step("closing_the_last_window_only_unregisters_it", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 858, 3, "expect (console_win == none)"), || {
let __left = __test.state().console_win.clone(); let __right = ::std::option::Option::None; __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 858, 3, "expect (console_win == none)"));
});
::ui_lang_runtime::testing::step("closing_the_last_window_only_unregisters_it", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 859, 3, "tray choose \"Open Ducktape\""), || {
let __value = "Open Ducktape".to_owned();
let __row = __test.tray_command_row(&__value, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 859, 3, "tray choose \"Open Ducktape\""));
let __message = Ducktape::__tray_row(__row).expect("the row-to-handler table has no entry for a row declared with a route");
__test.dispatch(__message, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 859, 3, "tray choose \"Open Ducktape\""));
});
::ui_lang_runtime::testing::step("closing_the_last_window_only_unregisters_it", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 860, 3, "expect (onboarding_win != none)"), || {
let __left = __test.state().onboarding_win.clone(); let __right = ::std::option::Option::None; __test.check_ne(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 860, 3, "expect (onboarding_win != none)"));
});
}
#[test]
fn the_status_item_opens_a_window_when_none_is_tracked() {
let __config = ::ui_lang_runtime::testing::Config::new("the_status_item_opens_a_window_when_none_is_tracked").source(::ui_lang_runtime::testing::Location::new("tests/app.ice", 866, 1, "test the_status_item_opens_a_window_when_none_is_tracked")).preset("ui_offline");
let mut __test = ::ui_lang_runtime::testing::Driver::new(Ducktape::__program(), __config);
::ui_lang_runtime::testing::step("the_status_item_opens_a_window_when_none_is_tracked", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 868, 3, "expect (onboarding_win == none)"), || {
let __left = __test.state().onboarding_win.clone(); let __right = ::std::option::Option::None; __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 868, 3, "expect (onboarding_win == none)"));
});
::ui_lang_runtime::testing::step("the_status_item_opens_a_window_when_none_is_tracked", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 869, 3, "expect (console_win == none)"), || {
let __left = __test.state().console_win.clone(); let __right = ::std::option::Option::None; __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 869, 3, "expect (console_win == none)"));
});
::ui_lang_runtime::testing::step("the_status_item_opens_a_window_when_none_is_tracked", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 870, 3, "tray choose \"Open Ducktape\""), || {
let __value = "Open Ducktape".to_owned();
let __row = __test.tray_command_row(&__value, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 870, 3, "tray choose \"Open Ducktape\""));
let __message = Ducktape::__tray_row(__row).expect("the row-to-handler table has no entry for a row declared with a route");
__test.dispatch(__message, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 870, 3, "tray choose \"Open Ducktape\""));
});
::ui_lang_runtime::testing::step("the_status_item_opens_a_window_when_none_is_tracked", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 871, 3, "expect (onboarding_win != none)"), || {
let __left = __test.state().onboarding_win.clone(); let __right = ::std::option::Option::None; __test.check_ne(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 871, 3, "expect (onboarding_win != none)"));
});
}
#[test]
fn the_status_item_reopens_the_console_without_resetting_hub_step() {
let __config = ::ui_lang_runtime::testing::Config::new("the_status_item_reopens_the_console_without_resetting_hub_step").source(::ui_lang_runtime::testing::Location::new("tests/app.ice", 887, 1, "test the_status_item_reopens_the_console_without_resetting_hub_step")).preset("ui_tray_reconnect");
let mut __test = ::ui_lang_runtime::testing::Driver::new(Ducktape::__program(), __config);
::ui_lang_runtime::testing::step("the_status_item_reopens_the_console_without_resetting_hub_step", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 889, 3, "expect (console_win == none)"), || {
let __left = __test.state().console_win.clone(); let __right = ::std::option::Option::None; __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 889, 3, "expect (console_win == none)"));
});
::ui_lang_runtime::testing::step("the_status_item_reopens_the_console_without_resetting_hub_step", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 890, 3, "expect (onboarding_win == none)"), || {
let __left = __test.state().onboarding_win.clone(); let __right = ::std::option::Option::None; __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 890, 3, "expect (onboarding_win == none)"));
});
::ui_lang_runtime::testing::step("the_status_item_reopens_the_console_without_resetting_hub_step", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 891, 3, "tray choose \"Open Ducktape\""), || {
let __value = "Open Ducktape".to_owned();
let __row = __test.tray_command_row(&__value, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 891, 3, "tray choose \"Open Ducktape\""));
let __message = Ducktape::__tray_row(__row).expect("the row-to-handler table has no entry for a row declared with a route");
__test.dispatch(__message, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 891, 3, "tray choose \"Open Ducktape\""));
});
::ui_lang_runtime::testing::step("the_status_item_reopens_the_console_without_resetting_hub_step", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 895, 3, "expect !connected"), || {
let __actual = !__test.state().connected; __test.check(__actual, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 895, 3, "expect !connected"));
});
::ui_lang_runtime::testing::step("the_status_item_reopens_the_console_without_resetting_hub_step", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 896, 3, "expect loading"), || {
let __actual = __test.state().loading; __test.check(__actual, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 896, 3, "expect loading"));
});
::ui_lang_runtime::testing::step("the_status_item_reopens_the_console_without_resetting_hub_step", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 897, 3, "expect (onboarding_win == none)"), || {
let __left = __test.state().onboarding_win.clone(); let __right = ::std::option::Option::None; __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 897, 3, "expect (onboarding_win == none)"));
});
::ui_lang_runtime::testing::step("the_status_item_reopens_the_console_without_resetting_hub_step", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 898, 3, "expect (hub_step == HubStep.live)"), || {
let __left = __test.state().hub_step.clone(); let __right = HubStep::Live; __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 898, 3, "expect (hub_step == HubStep.live)"));
});
}
#[test]
fn the_quit_chord_route_is_armed_only_while_command_is_held() {
let __config = ::ui_lang_runtime::testing::Config::new("the_quit_chord_route_is_armed_only_while_command_is_held").source(::ui_lang_runtime::testing::Location::new("tests/app.ice", 906, 1, "test the_quit_chord_route_is_armed_only_while_command_is_held")).preset("ui_offline");
let mut __test = ::ui_lang_runtime::testing::Driver::new(Ducktape::__program(), __config);
::ui_lang_runtime::testing::step("the_quit_chord_route_is_armed_only_while_command_is_held", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 908, 3, "expect !cmd_held"), || {
let __actual = !__test.state().cmd_held; __test.check(__actual, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 908, 3, "expect !cmd_held"));
});
::ui_lang_runtime::testing::step("the_quit_chord_route_is_armed_only_while_command_is_held", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 909, 3, "key \"q\""), || {
let _ = __test.perform_action(::ui_lang_runtime::testing::Action::Key(::ui_lang_runtime::testing::Key::character("q")), ::ui_lang_runtime::testing::Location::new("tests/app.ice", 909, 3, "key \"q\""));
});
::ui_lang_runtime::testing::step("the_quit_chord_route_is_armed_only_while_command_is_held", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 910, 3, "expect !cmd_held"), || {
let __actual = !__test.state().cmd_held; __test.check(__actual, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 910, 3, "expect !cmd_held"));
});
::ui_lang_runtime::testing::step("the_quit_chord_route_is_armed_only_while_command_is_held", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 913, 3, "modifiers control logo"), || {
let _ = __test.perform_action(::ui_lang_runtime::testing::Action::Modifiers(::ui_lang_runtime::testing::Modifiers::new(false, true, false, true)), ::ui_lang_runtime::testing::Location::new("tests/app.ice", 913, 3, "modifiers control logo"));
});
::ui_lang_runtime::testing::step("the_quit_chord_route_is_armed_only_while_command_is_held", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 914, 3, "expect cmd_held"), || {
let __actual = __test.state().cmd_held; __test.check(__actual, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 914, 3, "expect cmd_held"));
});
::ui_lang_runtime::testing::step("the_quit_chord_route_is_armed_only_while_command_is_held", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 915, 3, "modifiers"), || {
let _ = __test.perform_action(::ui_lang_runtime::testing::Action::Modifiers(::ui_lang_runtime::testing::Modifiers::new(false, false, false, false)), ::ui_lang_runtime::testing::Location::new("tests/app.ice", 915, 3, "modifiers"));
});
::ui_lang_runtime::testing::step("the_quit_chord_route_is_armed_only_while_command_is_held", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 916, 3, "expect !cmd_held"), || {
let __actual = !__test.state().cmd_held; __test.check(__actual, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 916, 3, "expect !cmd_held"));
});
}
#[test]
fn an_unlock_in_place_drops_the_previous_keys_live_rows() {
let __config = ::ui_lang_runtime::testing::Config::new("an_unlock_in_place_drops_the_previous_keys_live_rows").source(::ui_lang_runtime::testing::Location::new("tests/app.ice", 938, 1, "test an_unlock_in_place_drops_the_previous_keys_live_rows")).preset("ui_live_run_seated");
let mut __test = ::ui_lang_runtime::testing::Driver::new(Ducktape::__program(), __config);
::ui_lang_runtime::testing::step("an_unlock_in_place_drops_the_previous_keys_live_rows", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 940, 3, "expect !empty(live_agents)"), || {
let __actual = !(__test.state().live_agents).is_empty(); __test.check(__actual, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 940, 3, "expect !empty(live_agents)"));
});
::ui_lang_runtime::testing::step("an_unlock_in_place_drops_the_previous_keys_live_rows", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 941, 3, "dispatch settings_unlocked(\"bb22\")"), || {
let __message = __DucktapeMessage::SettingsUnlocked("bb22".to_owned()); __test.dispatch(__message, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 941, 3, "dispatch settings_unlocked(\"bb22\")"));
});
::ui_lang_runtime::testing::step("an_unlock_in_place_drops_the_previous_keys_live_rows", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 942, 3, "expect (signer_key == \"bb22\")"), || {
let __left = __test.state().signer_key.to_owned(); let __right = "bb22".to_owned(); __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 942, 3, "expect (signer_key == \"bb22\")"));
});
::ui_lang_runtime::testing::step("an_unlock_in_place_drops_the_previous_keys_live_rows", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 943, 3, "expect empty(live_agents)"), || {
let __actual = (__test.state().live_agents).is_empty(); __test.check(__actual, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 943, 3, "expect empty(live_agents)"));
});
}
#[test]
fn locking_the_seat_takes_the_private_output_with_it() {
let __config = ::ui_lang_runtime::testing::Config::new("locking_the_seat_takes_the_private_output_with_it").source(::ui_lang_runtime::testing::Location::new("tests/app.ice", 945, 1, "test locking_the_seat_takes_the_private_output_with_it")).preset("ui_live_run_seated");
let mut __test = ::ui_lang_runtime::testing::Driver::new(Ducktape::__program(), __config);
::ui_lang_runtime::testing::step("locking_the_seat_takes_the_private_output_with_it", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 947, 3, "expect !empty(live_agents)"), || {
let __actual = !(__test.state().live_agents).is_empty(); __test.check(__actual, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 947, 3, "expect !empty(live_agents)"));
});
::ui_lang_runtime::testing::step("locking_the_seat_takes_the_private_output_with_it", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 948, 3, "dispatch settings_view_event(view_event(\"lock\", \"\"))"), || {
let __message = __DucktapeMessage::SettingsViewEvent(crate::module_view::view_event("lock".to_owned(), "".to_owned())); __test.dispatch(__message, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 948, 3, "dispatch settings_view_event(view_event(\"lock\", \"\"))"));
});
::ui_lang_runtime::testing::step("locking_the_seat_takes_the_private_output_with_it", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 949, 3, "expect empty(signer_key)"), || {
let __actual = (__test.state().signer_key).is_empty(); __test.check(__actual, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 949, 3, "expect empty(signer_key)"));
});
::ui_lang_runtime::testing::step("locking_the_seat_takes_the_private_output_with_it", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 950, 3, "expect empty(password)"), || {
let __actual = (__test.state().password).is_empty(); __test.check(__actual, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 950, 3, "expect empty(password)"));
});
::ui_lang_runtime::testing::step("locking_the_seat_takes_the_private_output_with_it", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 951, 3, "expect empty(live_agents)"), || {
let __actual = (__test.state().live_agents).is_empty(); __test.check(__actual, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 951, 3, "expect empty(live_agents)"));
});
}
#[test]
fn a_chat_address_opened_from_another_tab_lands_on_the_chat_tab() {
let __config = ::ui_lang_runtime::testing::Config::new("a_chat_address_opened_from_another_tab_lands_on_the_chat_tab").source(::ui_lang_runtime::testing::Location::new("tests/app.ice", 958, 1, "test a_chat_address_opened_from_another_tab_lands_on_the_chat_tab")).preset("ui_live_run_seated");
let mut __test = ::ui_lang_runtime::testing::Driver::new(Ducktape::__program(), __config);
::ui_lang_runtime::testing::step("a_chat_address_opened_from_another_tab_lands_on_the_chat_tab", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 960, 3, "dispatch select_shell_tab(ShellTab.agents)"), || {
let __message = __DucktapeMessage::SelectShellTab(ShellTab::Agents); __test.dispatch(__message, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 960, 3, "dispatch select_shell_tab(ShellTab.agents)"));
});
::ui_lang_runtime::testing::step("a_chat_address_opened_from_another_tab_lands_on_the_chat_tab", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 961, 3, "expect (shell_tab == ShellTab.agents)"), || {
let __left = __test.state().shell_tab.clone(); let __right = ShellTab::Agents; __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 961, 3, "expect (shell_tab == ShellTab.agents)"));
});
::ui_lang_runtime::testing::step("a_chat_address_opened_from_another_tab_lands_on_the_chat_tab", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 962, 3, "dispatch open_message_link(\"duck://channel/general\")"), || {
let __message = __DucktapeMessage::OpenMessageLink("duck://channel/general".to_owned()); __test.dispatch(__message, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 962, 3, "dispatch open_message_link(\"duck://channel/general\")"));
});
::ui_lang_runtime::testing::step("a_chat_address_opened_from_another_tab_lands_on_the_chat_tab", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 963, 3, "expect (shell_tab == ShellTab.chat)"), || {
let __left = __test.state().shell_tab.clone(); let __right = ShellTab::Chat; __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 963, 3, "expect (shell_tab == ShellTab.chat)"));
});
}
#[test]
fn a_run_opened_mid_ceremony_retires_the_ceremony_like_any_tab_move() {
let __config = ::ui_lang_runtime::testing::Config::new("a_run_opened_mid_ceremony_retires_the_ceremony_like_any_tab_move").source(::ui_lang_runtime::testing::Location::new("tests/app.ice", 979, 1, "test a_run_opened_mid_ceremony_retires_the_ceremony_like_any_tab_move")).preset("ui_ceremony_on_settings");
let mut __test = ::ui_lang_runtime::testing::Driver::new(Ducktape::__program(), __config);
::ui_lang_runtime::testing::step("a_run_opened_mid_ceremony_retires_the_ceremony_like_any_tab_move", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 981, 3, "expect !empty(account_ceremony_phase)"), || {
let __actual = !(__test.state().account_ceremony_phase).is_empty(); __test.check(__actual, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 981, 3, "expect !empty(account_ceremony_phase)"));
});
::ui_lang_runtime::testing::step("a_run_opened_mid_ceremony_retires_the_ceremony_like_any_tab_move", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 982, 3, "dispatch open_run_panel(\"dispatch-1\")"), || {
let __message = __DucktapeMessage::OpenRunPanel("dispatch-1".to_owned()); __test.dispatch(__message, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 982, 3, "dispatch open_run_panel(\"dispatch-1\")"));
});
::ui_lang_runtime::testing::step("a_run_opened_mid_ceremony_retires_the_ceremony_like_any_tab_move", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 983, 3, "expect (shell_tab == ShellTab.agents)"), || {
let __left = __test.state().shell_tab.clone(); let __right = ShellTab::Agents; __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 983, 3, "expect (shell_tab == ShellTab.agents)"));
});
::ui_lang_runtime::testing::step("a_run_opened_mid_ceremony_retires_the_ceremony_like_any_tab_move", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 984, 3, "expect (agents_open_run == \"dispatch-1\")"), || {
let __left = __test.state().agents_open_run.to_owned(); let __right = "dispatch-1".to_owned(); __test.check_eq(__left, __right, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 984, 3, "expect (agents_open_run == \"dispatch-1\")"));
});
::ui_lang_runtime::testing::step("a_run_opened_mid_ceremony_retires_the_ceremony_like_any_tab_move", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 985, 3, "expect empty(account_ceremony_phase)"), || {
let __actual = (__test.state().account_ceremony_phase).is_empty(); __test.check(__actual, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 985, 3, "expect empty(account_ceremony_phase)"));
});
::ui_lang_runtime::testing::step("a_run_opened_mid_ceremony_retires_the_ceremony_like_any_tab_move", ::ui_lang_runtime::testing::Location::new("tests/app.ice", 986, 3, "expect empty(account_ceremony_qr)"), || {
let __actual = (__test.state().account_ceremony_qr).is_empty(); __test.check(__actual, ::ui_lang_runtime::testing::Location::new("tests/app.ice", 986, 3, "expect empty(account_ceremony_qr)"));
});
}
}
}
include!("app_update.rs");
include!("app_view.rs");
include!("huddle_bd86d4aa.rs");
include!("icon_4308406f.rs");
include!("kit_580ff6a0.rs");
include!("onboarding_aaa7f647.rs");
include!("overlay_a2bdadb4.rs");
include!("overlays_5866449c.rs");
include!("patterns_b06d5dcd.rs");
include!("shell_e6857b62.rs");
