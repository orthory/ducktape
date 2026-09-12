//! This node's operator screen as a view on the kernel contract: status,
//! standing, peers, the log ring and the code registry, rendered from a
//! wasm component the desktop app loads from a file.
//!
//! The kernel pushes session facts only (`node.props`: connected, dark, this
//! seat's admin standing and tier, the app's connection reading, the
//! daemon's workspace directory and the wall clock). The node's own facts,
//! its peers and its code registry are read here through the kernel's
//! `rpc.status` / `rpc.peers` / `rpc.query`, re-read on every `rpc.live` hit
//! for the `block` plane, and the log ring arrives through `rpc.stream` on
//! the node's own `logs` topic — the timeline is this view's state, not the
//! app's. Retuning the running node's tracing filter leaves as one
//! `rpc.admin` POST the kernel signs with the seated key; the clipboard is
//! the one intent left, because it is an OS door.

pub mod host;

macro_rules! __ice_generated_items_4e6f646556696577 { ($($item:item)*) => { $(#[allow(warnings, clippy::all)] $item)* }; }
__ice_generated_items_4e6f646556696577! {
type __IceElement<'a, Message, Theme = ()> = <(&'a (), Message, Theme) as ::ui_lang_guest::wire::Erase>::Node;
pub(crate) type __IceMessage = __NodeViewMessage;
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
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum NodeTab {
Overview,
Permissions,
Activity,
Modules,
}
#[allow(dead_code)]
pub struct NodeView {
pub(crate) __ice_accessibility: ::ui_lang_runtime::Bridge<__NodeViewMessage>,
#[cfg(all(target_os = "windows", not(test)))]
pub(crate) __ice_accessibility_initial: ::std::option::Option<usize>,
#[cfg(all(target_os = "windows", not(test)))]
pub(crate) __ice_accessibility_pending: ::std::vec::Vec<__NodeViewMessage>,
pub(crate) active_palette: AppTheme,
pub(crate) node_data_dir: ::std::string::String,
pub(crate) tier: ::std::string::String,
pub(crate) admin: bool,
pub(crate) status: ::std::string::String,
pub(crate) wall_now: i64,
pub(crate) connected: bool,
pub(crate) connection_serial: i64,
pub(crate) node_tab: NodeTab,
pub(crate) facts: crate::host::NodeFacts,
pub(crate) loading: bool,
pub(crate) module_rows: ::std::vec::Vec<crate::host::ModuleRow>,
pub(crate) node_peers: ::std::vec::Vec<crate::host::PeerRow>,
pub(crate) log_lines: ::std::vec::Vec<crate::host::LogRow>,
pub(crate) node_log_filter: ::std::string::String,
pub(crate) live_log_filter: ::std::string::String,
pub(crate) live_filter_note: ::std::string::String,
pub(crate) host_error: ::std::string::String,
pub(crate) sent: bool,
pub(crate) __ice_rev: [u64; 19],
}
impl ::std::fmt::Debug for NodeView { fn fmt(&self, __formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result { __formatter.write_str("NodeView") } }
#[derive(Clone)]
pub(crate) enum __NodeViewMessage {
__AccessibilitySnapshot(::std::boxed::Box<::ui_lang_runtime::Snapshot<__NodeViewMessage>>),
__AccessibilityAction(::ui_lang_runtime::ActionRequest),
__AccessibilityWindow(::iced::window::Id, ::iced::window::Event),
#[cfg(all(any(target_os = "windows", target_os = "macos"), not(test)))]
__AccessibilityNativeWindow(::ui_lang_runtime::NativeWindow),
__AccessibilityFocusNext(::std::option::Option<::iced::window::Id>),
__AccessibilityFocusPrevious(::std::option::Option<::iced::window::Id>),
__TemplateChanged,
SessionArrived(crate::host::SessionItem),
FactsArrived(crate::host::FactsItem),
PeersArrived(crate::host::PeersItem),
ModulesArrived(crate::host::ModulesItem),
LogsArrived(crate::host::LogItem),
ActDone(crate::host::ActItem),
SelectNodeTab(NodeTab),
OpenNodeModules,
NodeLogFilterChanged(::std::string::String),
LiveLogFilterChanged(::std::string::String),
ApplyLiveLogFilter,
CopyToClipboard(::std::string::String, ::std::string::String),
__BindNodeLogFilter(::std::string::String),
__BindLiveLogFilter(::std::string::String),
}
impl ::std::fmt::Debug for __NodeViewMessage { fn fmt(&self, __formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result { __formatter.write_str("__NodeViewMessage") } }
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_HostError(_value: &crate::host::HostError) {
let _: &::std::string::String = &_value.message;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_PeerRow(_value: &crate::host::PeerRow) {
let _: &::std::string::String = &_value.key;
let _: &::std::string::String = &_value.role;
let _: &bool = &_value.live;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_ModuleRow(_value: &crate::host::ModuleRow) {
let _: &::std::string::String = &_value.id;
let _: &::std::string::String = &_value.category;
let _: &::std::string::String = &_value.root;
let _: &::std::string::String = &_value.code_hash;
let _: &::std::string::String = &_value.pending_hash;
let _: &i64 = &_value.activation_height;
let _: &i64 = &_value.readiness;
let _: &bool = &_value.ready;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_LogRow(_value: &crate::host::LogRow) {
let _: &::std::string::String = &_value.cursor;
let _: &::std::string::String = &_value.time;
let _: &::std::string::String = &_value.level;
let _: &::std::string::String = &_value.message;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_NodeFacts(_value: &crate::host::NodeFacts) {
let _: &::std::string::String = &_value.node_key;
let _: &i64 = &_value.node_height;
let _: &i64 = &_value.node_checkpoint;
let _: &i64 = &_value.node_last_finalized;
let _: &::std::string::String = &_value.node_reachable_label;
let _: &::std::string::String = &_value.node_quorum_label;
let _: &::std::string::String = &_value.node_version;
let _: &::std::string::String = &_value.node_root_hash;
let _: &::std::string::String = &_value.sync_line;
let _: &i64 = &_value.node_phase_since;
let _: &i64 = &_value.node_sync_retries;
let _: &i64 = &_value.node_sync_failures;
let _: &::std::string::String = &_value.node_sync_last_error;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_Session(_value: &crate::host::Session) {
let _: &bool = &_value.connected;
let _: &bool = &_value.dark;
let _: &bool = &_value.admin;
let _: &::std::string::String = &_value.tier;
let _: &::std::string::String = &_value.status;
let _: &::std::string::String = &_value.data_dir;
let _: &i64 = &_value.wall_now;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_SessionItem(_value: &crate::host::SessionItem) {
let _: &crate::host::Session = &_value.next;
let _: &::std::string::String = &_value.error;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_FactsItem(_value: &crate::host::FactsItem) {
let _: &crate::host::NodeFacts = &_value.facts;
let _: &::std::string::String = &_value.error;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_PeersItem(_value: &crate::host::PeersItem) {
let _: &::std::vec::Vec<crate::host::PeerRow> = &_value.rows;
let _: &::std::string::String = &_value.error;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_ModulesItem(_value: &crate::host::ModulesItem) {
let _: &::std::vec::Vec<crate::host::ModuleRow> = &_value.rows;
let _: &::std::string::String = &_value.error;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_LogItem(_value: &crate::host::LogItem) {
let _: &::std::vec::Vec<crate::host::LogRow> = &_value.lines;
let _: &::std::string::String = &_value.error;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_ActItem(_value: &crate::host::ActItem) {
let _: &::std::string::String = &_value.reply;
let _: &::std::string::String = &_value.error;
}
#[allow(dead_code)] fn __ui_lang_check_subscription_session() { let _: ::iced::Subscription<crate::host::SessionItem> = crate::host::session(); }
#[allow(dead_code)] fn __ui_lang_check_subscription_facts(arg0: i64) { let _: ::iced::Subscription<crate::host::FactsItem> = crate::host::facts(arg0); }
#[allow(dead_code)] fn __ui_lang_check_subscription_peers(arg0: i64) { let _: ::iced::Subscription<crate::host::PeersItem> = crate::host::peers(arg0); }
#[allow(dead_code)] fn __ui_lang_check_subscription_modules(arg0: i64) { let _: ::iced::Subscription<crate::host::ModulesItem> = crate::host::modules(arg0); }
#[allow(dead_code)] fn __ui_lang_check_subscription_logs(arg0: i64) { let _: ::iced::Subscription<crate::host::LogItem> = crate::host::logs(arg0); }
#[allow(dead_code)] fn __ui_lang_check_subscription_acts() { let _: ::iced::Subscription<crate::host::ActItem> = crate::host::acts(); }
#[allow(dead_code)] fn __ui_lang_check_pure_connection_serial_after(arg0: bool, arg1: bool, arg2: i64) { let _: i64 = crate::host::connection_serial_after(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_empty_facts() { let _: crate::host::NodeFacts = crate::host::empty_facts(); }
#[allow(dead_code)] fn __ui_lang_check_pure_push_logs<'a>(arg0: &'a [crate::host::LogRow], arg1: &'a [crate::host::LogRow]) { let _: ::std::vec::Vec<crate::host::LogRow> = crate::host::push_logs(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_visible_log<'a>(arg0: &'a [crate::host::LogRow], arg1: &'a str) { let _: ::std::vec::Vec<crate::host::LogRow> = crate::host::visible_log(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_log_note(arg0: i64, arg1: i64) { let _: ::std::string::String = crate::host::log_note(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_sync_set_log_filter<'a>(arg0: &'a str) { let _: bool = crate::host::set_log_filter(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_copy<'a>(arg0: &'a str, arg1: &'a str) { let _: bool = crate::host::copy(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_icon<'a>(arg0: &'a str) { let _: ::std::vec::Vec<u8> = crate::host::icon(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_connection_degraded<'a>(arg0: &'a str) { let _: bool = crate::host::connection_degraded(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_reading_pair<'a>(arg0: &'a str, arg1: &'a str) { let _: ::std::string::String = crate::host::reading_pair(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_count_label(arg0: i64) { let _: ::std::string::String = crate::host::count_label(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_keep_str<'a>(arg0: bool, arg1: &'a str, arg2: &'a str) { let _: ::std::string::String = crate::host::keep_str(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_initial_of<'a>(arg0: &'a str) { let _: ::std::string::String = crate::host::initial_of(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_height_label_short(arg0: i64) { let _: ::std::string::String = crate::host::height_label_short(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_relative_time(arg0: i64, arg1: i64) { let _: ::std::string::String = crate::host::relative_time(arg0, arg1); }
}
__ice_generated_items_4e6f646556696577! {
#[allow(unused_parens)]
impl NodeView {
#[must_use]
pub fn default_font() -> ::iced::Font { ::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Normal, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal } }
}
}
__ice_generated_items_4e6f646556696577! {
#[allow(unused_parens)]
impl NodeView {
fn __palette(&self) -> __IcePalette {
match self.active_palette.clone() {
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
::iced::Theme::custom(::std::format!("NodeView/{}", __ice_palette.name), ::iced::theme::Palette {
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
pub(crate) fn __theme(&self) -> ::iced::Theme {
Self::__app_theme(self.__palette())
}
fn __title(&self) -> ::std::string::String { "Node".to_owned() }
}
}
__ice_generated_items_4e6f646556696577! {
#[allow(unused_parens)]
impl NodeView {
fn __state() -> Self {
Self {
__ice_accessibility: ::ui_lang_runtime::Bridge::new(),
#[cfg(all(target_os = "windows", not(test)))]
__ice_accessibility_initial: ::std::option::Option::None,
#[cfg(all(target_os = "windows", not(test)))]
__ice_accessibility_pending: ::std::vec::Vec::new(),
active_palette: AppTheme::App,
node_data_dir: "".to_owned(),
tier: "".to_owned(),
admin: false,
status: "".to_owned(),
wall_now: 0,
connected: false,
connection_serial: 0,
node_tab: NodeTab::Overview,
facts: crate::host::empty_facts(),
loading: true,
module_rows: ::std::vec::Vec::new(),
node_peers: ::std::vec::Vec::new(),
log_lines: ::std::vec::Vec::new(),
node_log_filter: "".to_owned(),
live_log_filter: "".to_owned(),
live_filter_note: "".to_owned(),
host_error: "".to_owned(),
sent: false,
__ice_rev: [::ui_lang_runtime::rev::seed(); 19],
}
}
fn __boot_task(&mut self) -> ::iced::Task<__NodeViewMessage> {
let task = (|| {
::iced::Task::none()
})();
task
}
pub(crate) fn __boot() -> (Self, ::iced::Task<__NodeViewMessage>) {
let mut state = Self::__state();
let task = state.__boot_task();
(state, task)
}
pub(crate) const __PREFERRED_WINDOW_SIZE: &'static str = "none";
#[allow(clippy::too_many_arguments)] fn __restore_state(active_palette: AppTheme, node_data_dir: ::std::string::String, tier: ::std::string::String, admin: bool, status: ::std::string::String, wall_now: i64, connected: bool, connection_serial: i64, node_tab: NodeTab, facts: crate::host::NodeFacts, loading: bool, module_rows: ::std::vec::Vec<crate::host::ModuleRow>, node_peers: ::std::vec::Vec<crate::host::PeerRow>, log_lines: ::std::vec::Vec<crate::host::LogRow>, node_log_filter: ::std::string::String, live_log_filter: ::std::string::String, live_filter_note: ::std::string::String, host_error: ::std::string::String, sent: bool) -> Self {
Self {
__ice_accessibility: ::ui_lang_runtime::Bridge::new(),
#[cfg(all(target_os = "windows", not(test)))]
__ice_accessibility_initial: ::std::option::Option::None,
#[cfg(all(target_os = "windows", not(test)))]
__ice_accessibility_pending: ::std::vec::Vec::new(),
active_palette: active_palette,
node_data_dir: node_data_dir,
tier: tier,
admin: admin,
status: status,
wall_now: wall_now,
connected: connected,
connection_serial: connection_serial,
node_tab: node_tab,
facts: facts,
loading: loading,
module_rows: module_rows,
node_peers: node_peers,
log_lines: log_lines,
node_log_filter: node_log_filter,
live_log_filter: live_log_filter,
live_filter_note: live_filter_note,
host_error: host_error,
sent: sent,
__ice_rev: [::ui_lang_runtime::rev::seed(); 19],
}
}
pub(crate) const __SNAPSHOT_SCHEMA: &'static str = "2aca5358ae16aabea0acc404d80c7acb4614980b4df1b00c1feb863a13525343";
pub(crate) fn __snapshot(&self) -> ::std::result::Result<::std::vec::Vec<u8>, ::std::string::String> { ::ui_lang_guest::wire::Snapshot {schema: ::std::string::String::from(Self::__SNAPSHOT_SCHEMA), state: ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("NodeView"), fields: vec![(::std::string::String::from("active_palette"), match &self.active_palette { AppTheme::App => ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("AppTheme"), fields: vec![(::std::string::String::from("app"), ::ui_lang_guest::wire::SnapshotValue::Unit)] }, AppTheme::AppDark => ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("AppTheme"), fields: vec![(::std::string::String::from("app_dark"), ::ui_lang_guest::wire::SnapshotValue::Unit)] } }), (::std::string::String::from("node_data_dir"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.node_data_dir))), (::std::string::String::from("tier"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.tier))), (::std::string::String::from("admin"), ::ui_lang_guest::wire::SnapshotValue::Bool(*(&self.admin))), (::std::string::String::from("status"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.status))), (::std::string::String::from("wall_now"), ::ui_lang_guest::wire::SnapshotValue::I64(*(&self.wall_now))), (::std::string::String::from("connected"), ::ui_lang_guest::wire::SnapshotValue::Bool(*(&self.connected))), (::std::string::String::from("connection_serial"), ::ui_lang_guest::wire::SnapshotValue::I64(*(&self.connection_serial))), (::std::string::String::from("node_tab"), match &self.node_tab { NodeTab::Overview => ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("NodeTab"), fields: vec![(::std::string::String::from("overview"), ::ui_lang_guest::wire::SnapshotValue::Unit)] }, NodeTab::Permissions => ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("NodeTab"), fields: vec![(::std::string::String::from("permissions"), ::ui_lang_guest::wire::SnapshotValue::Unit)] }, NodeTab::Activity => ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("NodeTab"), fields: vec![(::std::string::String::from("activity"), ::ui_lang_guest::wire::SnapshotValue::Unit)] }, NodeTab::Modules => ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("NodeTab"), fields: vec![(::std::string::String::from("modules"), ::ui_lang_guest::wire::SnapshotValue::Unit)] } }), (::std::string::String::from("facts"), ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("NodeFacts"), fields: ::std::vec![(::std::string::String::from("node_key"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&self.facts).node_key))), (::std::string::String::from("node_height"), ::ui_lang_guest::wire::SnapshotValue::I64(*(&(&self.facts).node_height))), (::std::string::String::from("node_checkpoint"), ::ui_lang_guest::wire::SnapshotValue::I64(*(&(&self.facts).node_checkpoint))), (::std::string::String::from("node_last_finalized"), ::ui_lang_guest::wire::SnapshotValue::I64(*(&(&self.facts).node_last_finalized))), (::std::string::String::from("node_reachable_label"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&self.facts).node_reachable_label))), (::std::string::String::from("node_quorum_label"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&self.facts).node_quorum_label))), (::std::string::String::from("node_version"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&self.facts).node_version))), (::std::string::String::from("node_root_hash"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&self.facts).node_root_hash))), (::std::string::String::from("sync_line"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&self.facts).sync_line))), (::std::string::String::from("node_phase_since"), ::ui_lang_guest::wire::SnapshotValue::I64(*(&(&self.facts).node_phase_since))), (::std::string::String::from("node_sync_retries"), ::ui_lang_guest::wire::SnapshotValue::I64(*(&(&self.facts).node_sync_retries))), (::std::string::String::from("node_sync_failures"), ::ui_lang_guest::wire::SnapshotValue::I64(*(&(&self.facts).node_sync_failures))), (::std::string::String::from("node_sync_last_error"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&self.facts).node_sync_last_error)))] }), (::std::string::String::from("loading"), ::ui_lang_guest::wire::SnapshotValue::Bool(*(&self.loading))), (::std::string::String::from("module_rows"), ::ui_lang_guest::wire::SnapshotValue::List((&self.module_rows).iter().map(|__item| ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("ModuleRow"), fields: ::std::vec![(::std::string::String::from("id"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).id))), (::std::string::String::from("category"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).category))), (::std::string::String::from("root"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).root))), (::std::string::String::from("code_hash"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).code_hash))), (::std::string::String::from("pending_hash"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).pending_hash))), (::std::string::String::from("activation_height"), ::ui_lang_guest::wire::SnapshotValue::I64(*(&(__item).activation_height))), (::std::string::String::from("readiness"), ::ui_lang_guest::wire::SnapshotValue::I64(*(&(__item).readiness))), (::std::string::String::from("ready"), ::ui_lang_guest::wire::SnapshotValue::Bool(*(&(__item).ready)))] }).collect())), (::std::string::String::from("node_peers"), ::ui_lang_guest::wire::SnapshotValue::List((&self.node_peers).iter().map(|__item| ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("PeerRow"), fields: ::std::vec![(::std::string::String::from("key"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).key))), (::std::string::String::from("role"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).role))), (::std::string::String::from("live"), ::ui_lang_guest::wire::SnapshotValue::Bool(*(&(__item).live)))] }).collect())), (::std::string::String::from("log_lines"), ::ui_lang_guest::wire::SnapshotValue::List((&self.log_lines).iter().map(|__item| ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("LogRow"), fields: ::std::vec![(::std::string::String::from("cursor"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).cursor))), (::std::string::String::from("time"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).time))), (::std::string::String::from("level"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).level))), (::std::string::String::from("message"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).message)))] }).collect())), (::std::string::String::from("node_log_filter"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.node_log_filter))), (::std::string::String::from("live_log_filter"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.live_log_filter))), (::std::string::String::from("live_filter_note"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.live_filter_note))), (::std::string::String::from("host_error"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.host_error))), (::std::string::String::from("sent"), ::ui_lang_guest::wire::SnapshotValue::Bool(*(&self.sent)))] }}.encode() }
pub(crate) fn __restore(__bytes: &[u8]) -> ::std::result::Result<Self, ::std::string::String> { let __snapshot = ::ui_lang_guest::wire::Snapshot::decode(__bytes)?; if __snapshot.schema != Self::__SNAPSHOT_SCHEMA { return ::std::result::Result::Err(::std::string::String::from("snapshot schema mismatch")); } let __value = __snapshot.state; ((|| { let ::ui_lang_guest::wire::SnapshotValue::Record {name: __name, fields: __fields} = __value else { return ::std::option::Option::None; }; if __name != "NodeView" || __fields.len() != 19 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __value) = __fields.next()?; if __name != "active_palette" { return ::std::option::Option::None; } let active_palette: AppTheme = ((|| { let ::ui_lang_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __value else { return ::std::option::Option::None; }; if __name != "AppTheme" || __fields.len() != 1 { return ::std::option::Option::None; } let (__variant, __payload) = __fields.into_iter().next()?; match __variant.as_str() { "app" => matches!(__payload, ::ui_lang_guest::wire::SnapshotValue::Unit).then_some(AppTheme::App), "app_dark" => matches!(__payload, ::ui_lang_guest::wire::SnapshotValue::Unit).then_some(AppTheme::AppDark), _ => ::std::option::Option::None } })())?; let (__name, __value) = __fields.next()?; if __name != "node_data_dir" { return ::std::option::Option::None; } let node_data_dir: ::std::string::String = (match __value { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "tier" { return ::std::option::Option::None; } let tier: ::std::string::String = (match __value { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "admin" { return ::std::option::Option::None; } let admin: bool = (match __value { ::ui_lang_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "status" { return ::std::option::Option::None; } let status: ::std::string::String = (match __value { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "wall_now" { return ::std::option::Option::None; } let wall_now: i64 = (match __value { ::ui_lang_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "connected" { return ::std::option::Option::None; } let connected: bool = (match __value { ::ui_lang_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "connection_serial" { return ::std::option::Option::None; } let connection_serial: i64 = (match __value { ::ui_lang_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "node_tab" { return ::std::option::Option::None; } let node_tab: NodeTab = ((|| { let ::ui_lang_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __value else { return ::std::option::Option::None; }; if __name != "NodeTab" || __fields.len() != 1 { return ::std::option::Option::None; } let (__variant, __payload) = __fields.into_iter().next()?; match __variant.as_str() { "overview" => matches!(__payload, ::ui_lang_guest::wire::SnapshotValue::Unit).then_some(NodeTab::Overview), "permissions" => matches!(__payload, ::ui_lang_guest::wire::SnapshotValue::Unit).then_some(NodeTab::Permissions), "activity" => matches!(__payload, ::ui_lang_guest::wire::SnapshotValue::Unit).then_some(NodeTab::Activity), "modules" => matches!(__payload, ::ui_lang_guest::wire::SnapshotValue::Unit).then_some(NodeTab::Modules), _ => ::std::option::Option::None } })())?; let (__name, __value) = __fields.next()?; if __name != "facts" { return ::std::option::Option::None; } let facts: crate::host::NodeFacts = ((|| { let ::ui_lang_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __value else { return ::std::option::Option::None; }; if __name != "NodeFacts" || __fields.len() != 13 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "node_key" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "node_height" { return ::std::option::Option::None; } let (__name, __field_2) = __fields.next()?; if __name != "node_checkpoint" { return ::std::option::Option::None; } let (__name, __field_3) = __fields.next()?; if __name != "node_last_finalized" { return ::std::option::Option::None; } let (__name, __field_4) = __fields.next()?; if __name != "node_reachable_label" { return ::std::option::Option::None; } let (__name, __field_5) = __fields.next()?; if __name != "node_quorum_label" { return ::std::option::Option::None; } let (__name, __field_6) = __fields.next()?; if __name != "node_version" { return ::std::option::Option::None; } let (__name, __field_7) = __fields.next()?; if __name != "node_root_hash" { return ::std::option::Option::None; } let (__name, __field_8) = __fields.next()?; if __name != "sync_line" { return ::std::option::Option::None; } let (__name, __field_9) = __fields.next()?; if __name != "node_phase_since" { return ::std::option::Option::None; } let (__name, __field_10) = __fields.next()?; if __name != "node_sync_retries" { return ::std::option::Option::None; } let (__name, __field_11) = __fields.next()?; if __name != "node_sync_failures" { return ::std::option::Option::None; } let (__name, __field_12) = __fields.next()?; if __name != "node_sync_last_error" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::NodeFacts { node_key: (match __field_0 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, node_height: (match __field_1 { ::ui_lang_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, node_checkpoint: (match __field_2 { ::ui_lang_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, node_last_finalized: (match __field_3 { ::ui_lang_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, node_reachable_label: (match __field_4 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, node_quorum_label: (match __field_5 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, node_version: (match __field_6 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, node_root_hash: (match __field_7 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, sync_line: (match __field_8 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, node_phase_since: (match __field_9 { ::ui_lang_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, node_sync_retries: (match __field_10 { ::ui_lang_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, node_sync_failures: (match __field_11 { ::ui_lang_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, node_sync_last_error: (match __field_12 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })())?; let (__name, __value) = __fields.next()?; if __name != "loading" { return ::std::option::Option::None; } let loading: bool = (match __value { ::ui_lang_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "module_rows" { return ::std::option::Option::None; } let module_rows: ::std::vec::Vec<crate::host::ModuleRow> = (match __value { ::ui_lang_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| (|| { let ::ui_lang_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __item else { return ::std::option::Option::None; }; if __name != "ModuleRow" || __fields.len() != 8 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "id" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "category" { return ::std::option::Option::None; } let (__name, __field_2) = __fields.next()?; if __name != "root" { return ::std::option::Option::None; } let (__name, __field_3) = __fields.next()?; if __name != "code_hash" { return ::std::option::Option::None; } let (__name, __field_4) = __fields.next()?; if __name != "pending_hash" { return ::std::option::Option::None; } let (__name, __field_5) = __fields.next()?; if __name != "activation_height" { return ::std::option::Option::None; } let (__name, __field_6) = __fields.next()?; if __name != "readiness" { return ::std::option::Option::None; } let (__name, __field_7) = __fields.next()?; if __name != "ready" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::ModuleRow { id: (match __field_0 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, category: (match __field_1 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, root: (match __field_2 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, code_hash: (match __field_3 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, pending_hash: (match __field_4 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, activation_height: (match __field_5 { ::ui_lang_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, readiness: (match __field_6 { ::ui_lang_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, ready: (match __field_7 { ::ui_lang_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })()).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "node_peers" { return ::std::option::Option::None; } let node_peers: ::std::vec::Vec<crate::host::PeerRow> = (match __value { ::ui_lang_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| (|| { let ::ui_lang_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __item else { return ::std::option::Option::None; }; if __name != "PeerRow" || __fields.len() != 3 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "key" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "role" { return ::std::option::Option::None; } let (__name, __field_2) = __fields.next()?; if __name != "live" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::PeerRow { key: (match __field_0 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, role: (match __field_1 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, live: (match __field_2 { ::ui_lang_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })()).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "log_lines" { return ::std::option::Option::None; } let log_lines: ::std::vec::Vec<crate::host::LogRow> = (match __value { ::ui_lang_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| (|| { let ::ui_lang_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __item else { return ::std::option::Option::None; }; if __name != "LogRow" || __fields.len() != 4 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "cursor" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "time" { return ::std::option::Option::None; } let (__name, __field_2) = __fields.next()?; if __name != "level" { return ::std::option::Option::None; } let (__name, __field_3) = __fields.next()?; if __name != "message" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::LogRow { cursor: (match __field_0 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, time: (match __field_1 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, level: (match __field_2 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, message: (match __field_3 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })()).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "node_log_filter" { return ::std::option::Option::None; } let node_log_filter: ::std::string::String = (match __value { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "live_log_filter" { return ::std::option::Option::None; } let live_log_filter: ::std::string::String = (match __value { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "live_filter_note" { return ::std::option::Option::None; } let live_filter_note: ::std::string::String = (match __value { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "host_error" { return ::std::option::Option::None; } let host_error: ::std::string::String = (match __value { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "sent" { return ::std::option::Option::None; } let sent: bool = (match __value { ::ui_lang_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; ::std::option::Option::Some(Self::__restore_state(active_palette, node_data_dir, tier, admin, status, wall_now, connected, connection_serial, node_tab, facts, loading, module_rows, node_peers, log_lines, node_log_filter, live_log_filter, live_filter_note, host_error, sent)) })()).ok_or_else(|| ::std::string::String::from("snapshot state mismatch")) }
}
}
__ice_generated_items_4e6f646556696577! {
#[allow(unused_parens)]
impl NodeView {
}
}
__ice_generated_items_4e6f646556696577! {
#[allow(unused_parens)]
impl NodeView {
fn __subscription(&self) -> ::iced::Subscription<__NodeViewMessage> {
::iced::Subscription::batch([
crate::host::session().map(move |__value| __NodeViewMessage::SessionArrived(__value)),
if self.connected { ::iced::Subscription::batch([crate::host::facts(self.connection_serial).map(move |__value| __NodeViewMessage::FactsArrived(__value)),
]) } else { ::iced::Subscription::none() },
if (self.connected && (self.node_tab == NodeTab::Overview)) { ::iced::Subscription::batch([crate::host::peers(self.connection_serial).map(move |__value| __NodeViewMessage::PeersArrived(__value)),
]) } else { ::iced::Subscription::none() },
if (self.connected && (self.node_tab == NodeTab::Modules)) { ::iced::Subscription::batch([crate::host::modules(self.connection_serial).map(move |__value| __NodeViewMessage::ModulesArrived(__value)),
]) } else { ::iced::Subscription::none() },
if (self.connected && (self.node_tab == NodeTab::Activity)) { ::iced::Subscription::batch([crate::host::logs(self.connection_serial).map(move |__value| __NodeViewMessage::LogsArrived(__value)),
]) } else { ::iced::Subscription::none() },
crate::host::acts().map(move |__value| __NodeViewMessage::ActDone(__value)),
])
}
}
}
__ice_generated_items_4e6f646556696577! {
#[allow(unused_parens)]
impl NodeView {
}
#[cfg(test)] mod __ice_tests { use super::*;
#[test]
fn __ice_view_fits_default_stack() {
::std::thread::Builder::new().stack_size(4 * 1024 * 1024).spawn(|| {
let (__app, _) = NodeView::__boot();
let _ = __app.__view();
}).unwrap().join().unwrap();
}
}
}
#[allow(warnings, clippy::all)]
mod __ice_group_app_8557f58d {
use super::*;
impl super::NodeView {
pub(super) fn __ice_component_use_78(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 232, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/node-body", __ice_use_scope); ::ui_lang_guest::wire::Node::Scroll { on_scroll: ::std::option::Option::None, virtual_rows: false, key: __ice_node_scope.clone(), direction: ::ui_lang_guest::wire::ScrollDirection::Vertical, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), bar_hidden: false, bar_width: ::std::option::Option::None, bar_margin: ::std::option::Option::None, scroller_width: ::std::option::Option::None, bar_spacing: ::std::option::Option::None, anchor_x: ::ui_lang_guest::wire::ScrollAnchor::Start, anchor_y: ::ui_lang_guest::wire::ScrollAnchor::Start, auto_scroll: (false), background: ::std::option::Option::None, border: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 237, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 242, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 243, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 248, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:248", __ice_use_scope), size: ::std::option::Option::Some(((16.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("This node".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 254, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_1(__ice_palette, format!("{}/StatusPill@2118", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 255, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:243", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((10.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 256, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 257, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/node-overview-tab", __ice_node_scope); ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::Some((self.node_tab == NodeTab::Overview)), expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: __ice_node_scope.clone(), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 263, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:263", __ice_use_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (0.0) as f32, right: (15.0) as f32, bottom: (0.0) as f32, left: (15.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 264, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_2(__ice_palette, format!("{}/TabLabel@2128", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Node overview".to_owned())), on_press: ::std::option::Option::Some(::ui_lang_guest::slots::message((move |__event_0| __NodeViewMessage::SelectNodeTab(__event_0))(NodeTab::Overview))), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((0.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([8.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX)]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[57]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), pressed: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), disabled: ::std::option::Option::None } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 272, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/node-permissions-tab", __ice_node_scope); ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::Some((self.node_tab == NodeTab::Permissions)), expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: __ice_node_scope.clone(), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 278, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:278", __ice_use_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (0.0) as f32, right: (15.0) as f32, bottom: (0.0) as f32, left: (15.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 279, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_3(__ice_palette, format!("{}/TabLabel@2143", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Node permissions".to_owned())), on_press: ::std::option::Option::Some(::ui_lang_guest::slots::message((move |__event_0| __NodeViewMessage::SelectNodeTab(__event_0))(NodeTab::Permissions))), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((0.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([8.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX)]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[57]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), pressed: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), disabled: ::std::option::Option::None } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 287, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/node-activity-tab", __ice_node_scope); ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::Some((self.node_tab == NodeTab::Activity)), expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: __ice_node_scope.clone(), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 293, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:293", __ice_use_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (0.0) as f32, right: (15.0) as f32, bottom: (0.0) as f32, left: (15.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 294, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_4(__ice_palette, format!("{}/TabLabel@2158", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Node activity".to_owned())), on_press: ::std::option::Option::Some(::ui_lang_guest::slots::message((move |__event_0| __NodeViewMessage::SelectNodeTab(__event_0))(NodeTab::Activity))), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((0.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([8.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX)]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[57]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), pressed: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), disabled: ::std::option::Option::None } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 302, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/node-modules-tab", __ice_node_scope); ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::Some((self.node_tab == NodeTab::Modules)), expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: __ice_node_scope.clone(), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 308, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:308", __ice_use_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (0.0) as f32, right: (15.0) as f32, bottom: (0.0) as f32, left: (15.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 309, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_5(__ice_palette, format!("{}/TabLabel@2173", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Node modules".to_owned())), on_press: ::std::option::Option::Some(::ui_lang_guest::slots::message((move || __NodeViewMessage::OpenNodeModules)())), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((0.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([8.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX)]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[57]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), pressed: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), disabled: ::std::option::Option::None } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:256", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((3.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); match &(self.node_tab) { NodeTab::Modules => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 319, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_17(__ice_palette, format!("{}/ModulesPanel@2183", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, NodeTab::Permissions => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 321, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 322, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_40(__ice_palette, format!("{}/NodeAccessCard@2186", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 323, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_55(__ice_palette, format!("{}/PermissionMatrix@2187", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:321", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((18.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, NodeTab::Activity => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 325, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_59(__ice_palette, format!("{}/LogTimeline.Frame@2189", __ice_use_scope), (move || __NodeViewMessage::ApplyLiveLogFilter).clone(), (move |__event_0, __event_1| __NodeViewMessage::CopyToClipboard(__event_0, __event_1)).clone(), (move |__event_0| __NodeViewMessage::LiveLogFilterChanged(__event_0)).clone(), (move |__event_0| __NodeViewMessage::NodeLogFilterChanged(__event_0)).clone(), (move || __NodeViewMessage::OpenNodeModules).clone(), (move |__event_0| __NodeViewMessage::SelectNodeTab(__event_0)).clone()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, NodeTab::Overview => { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 375, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 376, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_60(__ice_palette, format!("{}/GroupLabel@2240", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 377, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_61(__ice_palette, format!("{}/MemberFactRow@2241", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 378, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_62(__ice_palette, format!("{}/MemberFactRow@2242", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 382, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:382", __ice_use_scope), content: ::ui_lang_guest::wire::ButtonContent::Label(::std::string::String::from("Copy node key")), label: ::std::option::Option::None, on_press: if ((self.facts.node_key).is_empty()) { ::std::option::Option::None } else { ::std::option::Option::Some(::ui_lang_guest::slots::message((move |__event_0, __event_1| __NodeViewMessage::CopyToClipboard(__event_0, __event_1))(self.facts.node_key.to_owned(), "Node key copied".to_owned()))) }, width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((7.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[12]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[13]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[40]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(1.0), radius: ::std::option::Option::Some([9.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[6]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face::default(), hovered: ::std::option::Option::None, pressed: ::std::option::Option::None, disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 387, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_63(__ice_palette, format!("{}/GroupLabel@2251", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 392, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __items = ::std::vec::Vec::new(); let __ice_min_cell = ((170.0) as f32).max(f32::EPSILON).min(f32::MAX); let __flex_child: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 393, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_64(__ice_palette, format!("{}/StatCard@2257", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __items.push((::ui_lang_guest::wire::FlexItem { grow: Some(1.0), shrink: 0.0, basis: ::ui_lang_guest::wire::FlexBasis::Fixed(__ice_min_cell), ..Default::default() }, __flex_child)); let __flex_child: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 398, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_65(__ice_palette, format!("{}/StatCard@2262", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __items.push((::ui_lang_guest::wire::FlexItem { grow: Some(1.0), shrink: 0.0, basis: ::ui_lang_guest::wire::FlexBasis::Fixed(__ice_min_cell), ..Default::default() }, __flex_child)); let __flex_child: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 403, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_66(__ice_palette, format!("{}/StatCard@2267", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __items.push((::ui_lang_guest::wire::FlexItem { grow: Some(1.0), shrink: 0.0, basis: ::ui_lang_guest::wire::FlexBasis::Fixed(__ice_min_cell), ..Default::default() }, __flex_child)); let (items, children) = __items.into_iter().unzip(); ::ui_lang_guest::wire::Node::Flex { key: format!("{}/@layout:392", __ice_use_scope), items, children, background: ::std::option::Option::None, border: ::std::option::Option::None, layout: ::ui_lang_guest::wire::FlexLayout { direction: ::ui_lang_guest::wire::FlexDirection::Row, wrap: ::ui_lang_guest::wire::FlexWrap::Wrap, justify: ::std::option::Option::None, items: ::std::option::Option::None, content: ::std::option::Option::None, row_gap: ::std::option::Option::Some((10.0) as f32), column_gap: ::std::option::Option::Some((10.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: (false), surface_width: ::std::option::Option::None, surface_height: ::std::option::Option::None, surface_max_width: ::std::option::Option::None } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if self.admin { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 409, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __items = ::std::vec::Vec::new(); let __ice_min_cell = ((170.0) as f32).max(f32::EPSILON).min(f32::MAX); let __flex_child: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 410, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_67(__ice_palette, format!("{}/StatCard@2274", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __items.push((::ui_lang_guest::wire::FlexItem { grow: Some(1.0), shrink: 0.0, basis: ::ui_lang_guest::wire::FlexBasis::Fixed(__ice_min_cell), ..Default::default() }, __flex_child)); let (items, children) = __items.into_iter().unzip(); ::ui_lang_guest::wire::Node::Flex { key: format!("{}/@layout:409", __ice_use_scope), items, children, background: ::std::option::Option::None, border: ::std::option::Option::None, layout: ::ui_lang_guest::wire::FlexLayout { direction: ::ui_lang_guest::wire::FlexDirection::Row, wrap: ::ui_lang_guest::wire::FlexWrap::Wrap, justify: ::std::option::Option::None, items: ::std::option::Option::None, content: ::std::option::Option::None, row_gap: ::std::option::Option::Some((10.0) as f32), column_gap: ::std::option::Option::Some((10.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: (false), surface_width: ::std::option::Option::None, surface_height: ::std::option::Option::None, surface_max_width: ::std::option::Option::None } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 415, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_75(__ice_palette, format!("{}/GroupCard@2279", __ice_use_scope), (move || __NodeViewMessage::ApplyLiveLogFilter).clone(), (move |__event_0, __event_1| __NodeViewMessage::CopyToClipboard(__event_0, __event_1)).clone(), (move |__event_0| __NodeViewMessage::LiveLogFilterChanged(__event_0)).clone(), (move |__event_0| __NodeViewMessage::NodeLogFilterChanged(__event_0)).clone(), (move || __NodeViewMessage::OpenNodeModules).clone(), (move |__event_0| __NodeViewMessage::SelectNodeTab(__event_0)).clone()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!(self.node_peers).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 447, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 448, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_76(__ice_palette, format!("{}/GroupLabel@2312", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 449, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_77(__ice_palette, format!("{}/GroupCard@2313", __ice_use_scope), (move || __NodeViewMessage::ApplyLiveLogFilter).clone(), (move |__event_0, __event_1| __NodeViewMessage::CopyToClipboard(__event_0, __event_1)).clone(), (move |__event_0| __NodeViewMessage::LiveLogFilterChanged(__event_0)).clone(), (move |__event_0| __NodeViewMessage::NodeLogFilterChanged(__event_0)).clone(), (move || __NodeViewMessage::OpenNodeModules).clone(), (move |__event_0| __NodeViewMessage::SelectNodeTab(__event_0)).clone()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:447", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((9.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:375", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((13.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); }, } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:242", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((13.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:237", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((18.0) as f32), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (22.0) as f32, right: (22.0) as f32, bottom: (22.0) as f32, left: (22.0) as f32 }), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
}
}

#[allow(warnings, clippy::all)]
mod __ice_group_app_update {
use super::*;
impl super::NodeView {
#[allow(clippy::assign_op_pattern)]
pub(super) fn __update(&mut self, message: __NodeViewMessage) -> ::iced::Task<__NodeViewMessage> {
#[cfg(all(target_os = "windows", not(test)))]
if !self.__ice_accessibility.is_attached() && !matches!(&message, __NodeViewMessage::__AccessibilityNativeWindow(_)) {
self.__ice_accessibility_pending.push(message);
return ::iced::Task::none();
}
let __task = match message {
__NodeViewMessage::__AccessibilitySnapshot(__snapshot) => { self.__ice_accessibility.update(*__snapshot); return ::iced::Task::none(); },
__NodeViewMessage::__AccessibilityAction(__request) => { let __refresh = matches!(__request.action, ::ui_lang_runtime::Action::Focus); let __task = self.__ice_accessibility.dispatch(__request); return if __refresh { __task.chain(::ui_lang_runtime::snapshot::<__NodeViewMessage>("NodeView").map(|__snapshot| __NodeViewMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)))) } else { __task }; },
__NodeViewMessage::__AccessibilityWindow(__id, __event) => { self.__ice_accessibility.window_event(__id, __event); return ::iced::Task::none(); },
#[cfg(all(target_os = "windows", not(test)))]
__NodeViewMessage::__AccessibilityNativeWindow(__window) => {
let __id = __window.id();
if !self.__ice_accessibility.attach_window(__window) { return ::iced::Task::none(); }
let __restore = ::iced::window::set_mode(__id, ::iced::window::Mode::Windowed);
let __initial = self.__accessibility_initial_task();
let mut __pending = ::std::vec::Vec::new();
for __message in ::std::mem::take(&mut self.__ice_accessibility_pending) {
__pending.push(self.__update(__message));
}
let __pending = ::iced::Task::batch(__pending);
let __snapshot = ::ui_lang_runtime::snapshot::<__NodeViewMessage>("NodeView").map(|__snapshot| __NodeViewMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)));
return __restore.chain(::iced::Task::batch([__initial, __pending, __snapshot]));
},
#[cfg(all(target_os = "macos", not(test)))]
__NodeViewMessage::__AccessibilityNativeWindow(__window) => {
if !self.__ice_accessibility.attach_window(__window) { return ::iced::Task::none(); }
return ::ui_lang_runtime::snapshot::<__NodeViewMessage>("NodeView").map(|__snapshot| __NodeViewMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)));
},
__NodeViewMessage::__AccessibilityFocusNext(__window) => { return ::ui_lang_runtime::focus_next_in::<__NodeViewMessage>(__window).chain(::ui_lang_runtime::snapshot::<__NodeViewMessage>("NodeView").map(|__snapshot| __NodeViewMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)))); },
__NodeViewMessage::__AccessibilityFocusPrevious(__window) => { return ::ui_lang_runtime::focus_previous_in::<__NodeViewMessage>(__window).chain(::ui_lang_runtime::snapshot::<__NodeViewMessage>("NodeView").map(|__snapshot| __NodeViewMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)))); },
__NodeViewMessage::__TemplateChanged => { return ::iced::Task::none(); },
__NodeViewMessage::SessionArrived(item) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("session_arrived", "src/ui/app.ice:116");
let _ = &item;
{ let __ice_next = item.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[17] += 1; } }
if (!(item.error).is_empty()) { return ::iced::Task::none(); }
let next = item.next.clone();
{ let __ice_next = crate::host::connection_serial_after(self.connected, next.connected, self.connection_serial); if ::ui_lang_runtime::state_changed!(self.connection_serial, __ice_next) { self.connection_serial = __ice_next; self.__ice_rev[7] += 1; } }
{ let __ice_next = next.connected; if ::ui_lang_runtime::state_changed!(self.connected, __ice_next) { self.connected = __ice_next; self.__ice_rev[6] += 1; } }
{ let __ice_next = next.admin; if ::ui_lang_runtime::state_changed!(self.admin, __ice_next) { self.admin = __ice_next; self.__ice_rev[3] += 1; } }
{ let __ice_next = next.tier.to_owned(); if ::ui_lang_runtime::state_changed!(self.tier, __ice_next) { self.tier = __ice_next; self.__ice_rev[2] += 1; } }
{ let __ice_next = next.status.to_owned(); if ::ui_lang_runtime::state_changed!(self.status, __ice_next) { self.status = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = next.data_dir.to_owned(); if ::ui_lang_runtime::state_changed!(self.node_data_dir, __ice_next) { self.node_data_dir = __ice_next; self.__ice_rev[1] += 1; } }
{ let __ice_next = next.wall_now; if ::ui_lang_runtime::state_changed!(self.wall_now, __ice_next) { self.wall_now = __ice_next; self.__ice_rev[5] += 1; } }
{ let __ice_next = AppTheme::App; if ::ui_lang_runtime::state_changed!(self.active_palette, __ice_next) { self.active_palette = __ice_next; self.__ice_rev[0] += 1; } }
if (!next.dark) { return ::iced::Task::none(); }
{ let __ice_next = AppTheme::AppDark; if ::ui_lang_runtime::state_changed!(self.active_palette, __ice_next) { self.active_palette = __ice_next; self.__ice_rev[0] += 1; } }
::iced::Task::none()
})(),
__NodeViewMessage::FactsArrived(item) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("facts_arrived", "src/ui/app.ice:131");
let _ = &item;
{ let __ice_next = item.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[17] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.loading, __ice_next) { self.loading = __ice_next; self.__ice_rev[10] += 1; } }
if (!(item.error).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = item.facts.clone(); if ::ui_lang_runtime::state_changed!(self.facts, __ice_next) { self.facts = __ice_next; self.__ice_rev[9] += 1; } }
::iced::Task::none()
})(),
__NodeViewMessage::PeersArrived(item) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("peers_arrived", "src/ui/app.ice:137");
let _ = &item;
{ let __ice_next = item.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[17] += 1; } }
if (!(item.error).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = item.rows.clone(); if ::ui_lang_runtime::state_changed!(self.node_peers, __ice_next) { self.node_peers = __ice_next; self.__ice_rev[12] += 1; } }
::iced::Task::none()
})(),
__NodeViewMessage::ModulesArrived(item) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("modules_arrived", "src/ui/app.ice:142");
let _ = &item;
{ let __ice_next = item.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[17] += 1; } }
if (!(item.error).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = item.rows.clone(); if ::ui_lang_runtime::state_changed!(self.module_rows, __ice_next) { self.module_rows = __ice_next; self.__ice_rev[11] += 1; } }
::iced::Task::none()
})(),
__NodeViewMessage::LogsArrived(item) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("logs_arrived", "src/ui/app.ice:148");
let _ = &item;
{ let __ice_next = item.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[17] += 1; } }
{ let __ice_next = crate::host::push_logs(::std::convert::AsRef::as_ref(&(self.log_lines)), ::std::convert::AsRef::as_ref(&(item.lines))); if ::ui_lang_runtime::state_changed!(self.log_lines, __ice_next) { self.log_lines = __ice_next; self.__ice_rev[13] += 1; } }
::iced::Task::none()
})(),
__NodeViewMessage::ActDone(item) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("act_done", "src/ui/app.ice:153");
let _ = &item;
{ let __ice_next = item.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[17] += 1; } }
{ let __ice_next = crate::host::keep_str((item.error).is_empty(), ::std::convert::AsRef::as_ref(&(item.reply)), ::std::convert::AsRef::as_ref(&(item.error))); if ::ui_lang_runtime::state_changed!(self.live_filter_note, __ice_next) { self.live_filter_note = __ice_next; self.__ice_rev[16] += 1; } }
::iced::Task::none()
})(),
__NodeViewMessage::SelectNodeTab(next) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("select_node_tab", "src/ui/app.ice:157");
let _ = &next;
{ let __ice_next = next.clone(); if ::ui_lang_runtime::state_changed!(self.node_tab, __ice_next) { self.node_tab = __ice_next; self.__ice_rev[8] += 1; } }
::iced::Task::none()
})(),
__NodeViewMessage::OpenNodeModules => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("open_node_modules", "src/ui/app.ice:160");
{ let __ice_next = NodeTab::Modules; if ::ui_lang_runtime::state_changed!(self.node_tab, __ice_next) { self.node_tab = __ice_next; self.__ice_rev[8] += 1; } }
::iced::Task::none()
})(),
__NodeViewMessage::NodeLogFilterChanged(next) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("node_log_filter_changed", "src/ui/app.ice:164");
let _ = &next;
{ let __ice_next = next.to_owned(); if ::ui_lang_runtime::state_changed!(self.node_log_filter, __ice_next) { self.node_log_filter = __ice_next; self.__ice_rev[14] += 1; } }
::iced::Task::none()
})(),
__NodeViewMessage::LiveLogFilterChanged(next) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("live_log_filter_changed", "src/ui/app.ice:167");
let _ = &next;
{ let __ice_next = next.to_owned(); if ::ui_lang_runtime::state_changed!(self.live_log_filter, __ice_next) { self.live_log_filter = __ice_next; self.__ice_rev[15] += 1; } }
::iced::Task::none()
})(),
__NodeViewMessage::ApplyLiveLogFilter => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("apply_live_log_filter", "src/ui/app.ice:173");
if ((!self.admin) || (self.live_log_filter).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.live_filter_note, __ice_next) { self.live_filter_note = __ice_next; self.__ice_rev[16] += 1; } }
{ let __ice_next = ({ let __ice_call = ::ui_lang_runtime::dev::Span::extern_call("set_log_filter", "src/ui/app.ice:62"); crate::host::set_log_filter(::std::convert::AsRef::as_ref(&(self.live_log_filter))) }); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[18] += 1; } }
::iced::Task::none()
})(),
__NodeViewMessage::CopyToClipboard(text, label) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("copy_to_clipboard", "src/ui/app.ice:178");
let _ = &text;
let _ = &label;
{ let __ice_next = crate::host::copy(::std::convert::AsRef::as_ref(&(text)), ::std::convert::AsRef::as_ref(&(label))); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[18] += 1; } }
::iced::Task::none()
})(),
__NodeViewMessage::__BindNodeLogFilter(value) => { { let __ice_next = value; if ::ui_lang_runtime::state_changed!(self.node_log_filter, __ice_next) { self.node_log_filter = __ice_next; self.__ice_rev[14] += 1; } } ::iced::Task::none() }
__NodeViewMessage::__BindLiveLogFilter(value) => { { let __ice_next = value; if ::ui_lang_runtime::state_changed!(self.live_log_filter, __ice_next) { self.live_log_filter = __ice_next; self.__ice_rev[15] += 1; } } ::iced::Task::none() }
};
// Snapshotting the widget tree after every message serves ONLY an attached
// assistive technology (and the test harness, which drives the app through
// this tree) — ungated it walked every widget, built a TreeUpdate nobody
// read, and scheduled a second frame per message.
let __accessibility = if cfg!(test) || ::ui_lang_runtime::accessibility_active() {
::ui_lang_runtime::snapshot::<__NodeViewMessage>("NodeView").map(|__snapshot| __NodeViewMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)))
} else {
::iced::Task::none()
};
::iced::Task::batch([__task, __accessibility])
}

}
}

#[allow(warnings, clippy::all)]
mod __ice_group_app_view {
use super::*;
impl super::NodeView {
pub(super) fn __view(&self) -> __IceElement<'_, __NodeViewMessage> { let __ice_view = ::ui_lang_runtime::dev::Span::view("NodeView", "src/ui/app.ice:182"); let __ice_palette = self.__palette(); let __ice_app_theme = Self::__app_theme(__ice_palette); let __ice_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 182, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", "NodeView"); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[2]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 187, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); if (!(self.host_error).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 191, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:191", __ice_node_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (13.0) as f32, right: (22.0) as f32, bottom: (0.0) as f32, left: (22.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 192, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/host-error", __ice_node_scope); ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: __ice_node_scope.clone(), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[20]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (self.host_error.to_owned()).to_string() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!self.connected) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 194, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 199, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::None, height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 200, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:200", __ice_node_scope), size: ::std::option::Option::Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Not connected".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 201, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::None, height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:194", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if self.connected { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 203, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/node", __ice_node_scope); ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_78(__ice_palette, __ice_node_scope.clone())) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:187", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __ice_root: __IceElement<'_, __NodeViewMessage> = __ice_content; __ice_root }

}
}

#[allow(warnings, clippy::all)]
mod __ice_group_icon_8f6cad39 {
use super::*;
impl super::NodeView {
pub(super) fn __ice_component_use_27(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("/home/eddy/dev/ducktape/ducktape/.codex/worktrees/gpui-kit-migration/app/src/ui/components/icon.ice", 13, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); if ("idle" == "ink") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("/home/eddy/dev/ducktape/ducktape/.codex/worktrees/gpui-kit-migration/app/src/ui/components/icon.ice", 16, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let (__hash, __bytes) = ::ui_lang_guest::slots::picture(crate::host::icon(::std::convert::AsRef::as_ref(&("lock")))); ::ui_lang_guest::wire::Node::Svg { inherit_button_ink: false, key: format!("{}/@media:16", __ice_use_scope), hash: __hash, bytes: __bytes, label: ::std::option::Option::None, color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), hover: ::std::option::Option::None, fit: ::std::option::Option::None, rotation: ::std::option::Option::None, opacity: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((11.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((11.0) as f32)) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!("idle" == "ink")) && ("idle" == "label")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("/home/eddy/dev/ducktape/ducktape/.codex/worktrees/gpui-kit-migration/app/src/ui/components/icon.ice", 22, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let (__hash, __bytes) = ::ui_lang_guest::slots::picture(crate::host::icon(::std::convert::AsRef::as_ref(&("lock")))); ::ui_lang_guest::wire::Node::Svg { inherit_button_ink: false, key: format!("{}/@media:22", __ice_use_scope), hash: __hash, bytes: __bytes, label: ::std::option::Option::None, color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[73]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), hover: ::std::option::Option::None, fit: ::std::option::Option::None, rotation: ::std::option::Option::None, opacity: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((11.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((11.0) as f32)) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!(("idle" == "ink") || ("idle" == "label"))) && ("idle" == "meta")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("/home/eddy/dev/ducktape/ducktape/.codex/worktrees/gpui-kit-migration/app/src/ui/components/icon.ice", 28, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let (__hash, __bytes) = ::ui_lang_guest::slots::picture(crate::host::icon(::std::convert::AsRef::as_ref(&("lock")))); ::ui_lang_guest::wire::Node::Svg { inherit_button_ink: false, key: format!("{}/@media:28", __ice_use_scope), hash: __hash, bytes: __bytes, label: ::std::option::Option::None, color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), hover: ::std::option::Option::None, fit: ::std::option::Option::None, rotation: ::std::option::Option::None, opacity: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((11.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((11.0) as f32)) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!((("idle" == "ink") || ("idle" == "label")) || ("idle" == "meta"))) && ("idle" == "caption")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("/home/eddy/dev/ducktape/ducktape/.codex/worktrees/gpui-kit-migration/app/src/ui/components/icon.ice", 34, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let (__hash, __bytes) = ::ui_lang_guest::slots::picture(crate::host::icon(::std::convert::AsRef::as_ref(&("lock")))); ::ui_lang_guest::wire::Node::Svg { inherit_button_ink: false, key: format!("{}/@media:34", __ice_use_scope), hash: __hash, bytes: __bytes, label: ::std::option::Option::None, color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[70]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), hover: ::std::option::Option::None, fit: ::std::option::Option::None, rotation: ::std::option::Option::None, opacity: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((11.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((11.0) as f32)) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!(((("idle" == "ink") || ("idle" == "label")) || ("idle" == "meta")) || ("idle" == "caption"))) && ("idle" == "hint")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("/home/eddy/dev/ducktape/ducktape/.codex/worktrees/gpui-kit-migration/app/src/ui/components/icon.ice", 40, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let (__hash, __bytes) = ::ui_lang_guest::slots::picture(crate::host::icon(::std::convert::AsRef::as_ref(&("lock")))); ::ui_lang_guest::wire::Node::Svg { inherit_button_ink: false, key: format!("{}/@media:40", __ice_use_scope), hash: __hash, bytes: __bytes, label: ::std::option::Option::None, color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[72]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), hover: ::std::option::Option::None, fit: ::std::option::Option::None, rotation: ::std::option::Option::None, opacity: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((11.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((11.0) as f32)) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!((((("idle" == "ink") || ("idle" == "label")) || ("idle" == "meta")) || ("idle" == "caption")) || ("idle" == "hint"))) && ("idle" == "idle")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("/home/eddy/dev/ducktape/ducktape/.codex/worktrees/gpui-kit-migration/app/src/ui/components/icon.ice", 46, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let (__hash, __bytes) = ::ui_lang_guest::slots::picture(crate::host::icon(::std::convert::AsRef::as_ref(&("lock")))); ::ui_lang_guest::wire::Node::Svg { inherit_button_ink: false, key: format!("{}/@media:46", __ice_use_scope), hash: __hash, bytes: __bytes, label: ::std::option::Option::None, color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[74]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), hover: ::std::option::Option::None, fit: ::std::option::Option::None, rotation: ::std::option::Option::None, opacity: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((11.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((11.0) as f32)) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!(((((("idle" == "ink") || ("idle" == "label")) || ("idle" == "meta")) || ("idle" == "caption")) || ("idle" == "hint")) || ("idle" == "idle"))) && ("idle" == "accent")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("/home/eddy/dev/ducktape/ducktape/.codex/worktrees/gpui-kit-migration/app/src/ui/components/icon.ice", 52, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let (__hash, __bytes) = ::ui_lang_guest::slots::picture(crate::host::icon(::std::convert::AsRef::as_ref(&("lock")))); ::ui_lang_guest::wire::Node::Svg { inherit_button_ink: false, key: format!("{}/@media:52", __ice_use_scope), hash: __hash, bytes: __bytes, label: ::std::option::Option::None, color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[16]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), hover: ::std::option::Option::None, fit: ::std::option::Option::None, rotation: ::std::option::Option::None, opacity: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((11.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((11.0) as f32)) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!((((((("idle" == "ink") || ("idle" == "label")) || ("idle" == "meta")) || ("idle" == "caption")) || ("idle" == "hint")) || ("idle" == "idle")) || ("idle" == "accent"))) && ("idle" == "success")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("/home/eddy/dev/ducktape/ducktape/.codex/worktrees/gpui-kit-migration/app/src/ui/components/icon.ice", 58, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let (__hash, __bytes) = ::ui_lang_guest::slots::picture(crate::host::icon(::std::convert::AsRef::as_ref(&("lock")))); ::ui_lang_guest::wire::Node::Svg { inherit_button_ink: false, key: format!("{}/@media:58", __ice_use_scope), hash: __hash, bytes: __bytes, label: ::std::option::Option::None, color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[25]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), hover: ::std::option::Option::None, fit: ::std::option::Option::None, rotation: ::std::option::Option::None, opacity: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((11.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((11.0) as f32)) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!(((((((("idle" == "ink") || ("idle" == "label")) || ("idle" == "meta")) || ("idle" == "caption")) || ("idle" == "hint")) || ("idle" == "idle")) || ("idle" == "accent")) || ("idle" == "success"))) && ("idle" == "warning")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("/home/eddy/dev/ducktape/ducktape/.codex/worktrees/gpui-kit-migration/app/src/ui/components/icon.ice", 64, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let (__hash, __bytes) = ::ui_lang_guest::slots::picture(crate::host::icon(::std::convert::AsRef::as_ref(&("lock")))); ::ui_lang_guest::wire::Node::Svg { inherit_button_ink: false, key: format!("{}/@media:64", __ice_use_scope), hash: __hash, bytes: __bytes, label: ::std::option::Option::None, color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[30]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), hover: ::std::option::Option::None, fit: ::std::option::Option::None, rotation: ::std::option::Option::None, opacity: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((11.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((11.0) as f32)) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!((((((((("idle" == "ink") || ("idle" == "label")) || ("idle" == "meta")) || ("idle" == "caption")) || ("idle" == "hint")) || ("idle" == "idle")) || ("idle" == "accent")) || ("idle" == "success")) || ("idle" == "warning"))) && ("idle" == "danger")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("/home/eddy/dev/ducktape/ducktape/.codex/worktrees/gpui-kit-migration/app/src/ui/components/icon.ice", 70, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let (__hash, __bytes) = ::ui_lang_guest::slots::picture(crate::host::icon(::std::convert::AsRef::as_ref(&("lock")))); ::ui_lang_guest::wire::Node::Svg { inherit_button_ink: false, key: format!("{}/@media:70", __ice_use_scope), hash: __hash, bytes: __bytes, label: ::std::option::Option::None, color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[20]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), hover: ::std::option::Option::None, fit: ::std::option::Option::None, rotation: ::std::option::Option::None, opacity: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((11.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((11.0) as f32)) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!(((((((((("idle" == "ink") || ("idle" == "label")) || ("idle" == "meta")) || ("idle" == "caption")) || ("idle" == "hint")) || ("idle" == "idle")) || ("idle" == "accent")) || ("idle" == "success")) || ("idle" == "warning")) || ("idle" == "danger"))) && ("idle" == "paper")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("/home/eddy/dev/ducktape/ducktape/.codex/worktrees/gpui-kit-migration/app/src/ui/components/icon.ice", 76, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let (__hash, __bytes) = ::ui_lang_guest::slots::picture(crate::host::icon(::std::convert::AsRef::as_ref(&("lock")))); ::ui_lang_guest::wire::Node::Svg { inherit_button_ink: false, key: format!("{}/@media:76", __ice_use_scope), hash: __hash, bytes: __bytes, label: ::std::option::Option::None, color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[38]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), hover: ::std::option::Option::None, fit: ::std::option::Option::None, rotation: ::std::option::Option::None, opacity: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((11.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((11.0) as f32)) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!((((((((((("idle" == "ink") || ("idle" == "label")) || ("idle" == "meta")) || ("idle" == "caption")) || ("idle" == "hint")) || ("idle" == "idle")) || ("idle" == "accent")) || ("idle" == "success")) || ("idle" == "warning")) || ("idle" == "danger")) || ("idle" == "paper"))) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("/home/eddy/dev/ducktape/ducktape/.codex/worktrees/gpui-kit-migration/app/src/ui/components/icon.ice", 82, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let (__hash, __bytes) = ::ui_lang_guest::slots::picture(crate::host::icon(::std::convert::AsRef::as_ref(&("lock")))); ::ui_lang_guest::wire::Node::Svg { inherit_button_ink: false, key: format!("{}/@media:82", __ice_use_scope), hash: __hash, bytes: __bytes, label: ::std::option::Option::None, color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), hover: ::std::option::Option::None, fit: ::std::option::Option::None, rotation: ::std::option::Option::None, opacity: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((11.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((11.0) as f32)) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
}
}

#[allow(warnings, clippy::all)]
mod __ice_group_kit_7309d6fe {
use super::*;
impl super::NodeView {
pub(super) fn __ice_component_use_0(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 269, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); if crate::host::connection_degraded(::std::convert::AsRef::as_ref(&(self.status))) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 271, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:271", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((6.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((6.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[83]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([(((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX), (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX), (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX), (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 277, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!crate::host::connection_degraded(::std::convert::AsRef::as_ref(&(self.status)))) && self.loading) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 279, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:279", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((6.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((6.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[34]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([(((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX), (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX), (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX), (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 285, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!crate::host::connection_degraded(::std::convert::AsRef::as_ref(&(self.status)))) && (!self.loading)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 287, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:287", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((6.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((6.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[29]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([(((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX), (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX), (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX), (((6.0 / 2.0)) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 293, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_1(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 116, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (3.0) as f32, right: (8.0) as f32, bottom: (3.0) as f32, left: (8.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 124, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 125, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_0(__ice_palette, format!("{}/StatusDot@1718", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if crate::host::connection_degraded(::std::convert::AsRef::as_ref(&(self.status))) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 131, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:131", __ice_use_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[41]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Stopped".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!crate::host::connection_degraded(::std::convert::AsRef::as_ref(&(self.status)))) && self.loading) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 138, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:138", __ice_use_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[41]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Syncing…".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!crate::host::connection_degraded(::std::convert::AsRef::as_ref(&(self.status)))) && (!self.loading)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 145, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:145", __ice_use_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[41]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Synced".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:124", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((5.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_2(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 153, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 154, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); if (self.node_tab == NodeTab::Overview) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 161, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:161", __ice_use_scope), size: ::std::option::Option::Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Overview".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(self.node_tab == NodeTab::Overview)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 168, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:168", __ice_use_scope), size: ::std::option::Option::Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Overview".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (0 > 0) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 175, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:175", __ice_use_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (1.0) as f32, right: (7.0) as f32, bottom: (1.0) as f32, left: (7.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 181, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:181", __ice_use_scope), size: ::std::option::Option::Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (0).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:154", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((7.0) as f32), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (10.0) as f32, right: (0.0) as f32, bottom: (10.0) as f32, left: (0.0) as f32 }), width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (self.node_tab == NodeTab::Overview) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 188, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:188", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((2.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 193, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(self.node_tab == NodeTab::Overview)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 195, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:195", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((2.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 200, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_3(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 153, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 154, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); if (self.node_tab == NodeTab::Permissions) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 161, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:161", __ice_use_scope), size: ::std::option::Option::Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Permissions".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(self.node_tab == NodeTab::Permissions)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 168, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:168", __ice_use_scope), size: ::std::option::Option::Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Permissions".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (0 > 0) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 175, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:175", __ice_use_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (1.0) as f32, right: (7.0) as f32, bottom: (1.0) as f32, left: (7.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 181, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:181", __ice_use_scope), size: ::std::option::Option::Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (0).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:154", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((7.0) as f32), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (10.0) as f32, right: (0.0) as f32, bottom: (10.0) as f32, left: (0.0) as f32 }), width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (self.node_tab == NodeTab::Permissions) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 188, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:188", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((2.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 193, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(self.node_tab == NodeTab::Permissions)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 195, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:195", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((2.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 200, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_4(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 153, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 154, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); if (self.node_tab == NodeTab::Activity) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 161, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:161", __ice_use_scope), size: ::std::option::Option::Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Activity".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(self.node_tab == NodeTab::Activity)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 168, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:168", __ice_use_scope), size: ::std::option::Option::Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Activity".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (0 > 0) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 175, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:175", __ice_use_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (1.0) as f32, right: (7.0) as f32, bottom: (1.0) as f32, left: (7.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 181, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:181", __ice_use_scope), size: ::std::option::Option::Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (0).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:154", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((7.0) as f32), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (10.0) as f32, right: (0.0) as f32, bottom: (10.0) as f32, left: (0.0) as f32 }), width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (self.node_tab == NodeTab::Activity) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 188, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:188", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((2.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 193, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(self.node_tab == NodeTab::Activity)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 195, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:195", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((2.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 200, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_5(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 153, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 154, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); if (self.node_tab == NodeTab::Modules) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 161, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:161", __ice_use_scope), size: ::std::option::Option::Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Modules".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(self.node_tab == NodeTab::Modules)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 168, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:168", __ice_use_scope), size: ::std::option::Option::Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Modules".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (((self.module_rows).len() as i64) > 0) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 175, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:175", __ice_use_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (1.0) as f32, right: (7.0) as f32, bottom: (1.0) as f32, left: (7.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 181, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:181", __ice_use_scope), size: ::std::option::Option::Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (((self.module_rows).len() as i64)).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:154", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((7.0) as f32), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (10.0) as f32, right: (0.0) as f32, bottom: (10.0) as f32, left: (0.0) as f32 }), width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (self.node_tab == NodeTab::Modules) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 188, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:188", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((2.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 193, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(self.node_tab == NodeTab::Modules)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 195, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:195", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((2.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 200, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_6(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 5, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: __ice_node_scope.clone(), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[73]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("REGISTERED".to_owned()).to_string() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_7(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 66, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (30.0) as f32, right: (30.0) as f32, bottom: (30.0) as f32, left: (30.0) as f32 }), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((12.0) as f32).max(0.0).min(f32::MAX), ((12.0) as f32).max(0.0).min(f32::MAX), ((12.0) as f32).max(0.0).min(f32::MAX), ((12.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 75, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:75", __ice_use_scope), size: ::std::option::Option::Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("The node has not answered with its module set yet.".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_11(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 57, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((7.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((7.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[29]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([(((7.0 / 2.0)) as f32).max(0.0).min(f32::MAX), (((7.0 / 2.0)) as f32).max(0.0).min(f32::MAX), (((7.0 / 2.0)) as f32).max(0.0).min(f32::MAX), (((7.0 / 2.0)) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 63, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_18(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 5, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: __ice_node_scope.clone(), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[73]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("YOUR ACCESS".to_owned()).to_string() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_32(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 78, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (11.0) as f32, right: (13.0) as f32, bottom: (11.0) as f32, left: (13.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[84]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[33]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 87, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 92, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 93, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:93", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((6.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((6.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[34]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((3.0) as f32).max(0.0).min(f32::MAX), ((3.0) as f32).max(0.0).min(f32::MAX), ((3.0) as f32).max(0.0).min(f32::MAX), ((3.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 99, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:92", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (4.0) as f32, right: (0.0) as f32, bottom: (0.0) as f32, left: (0.0) as f32 }), width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 100, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 101, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::Some(::ui_lang_guest::wire::LineHeight::Relative(((1.45) as f32).max(f32::EPSILON).min(f32::MAX))), shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:101", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[30]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("Only a validator may open a membership proposal.".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if ("Ask a validator to propose this node for the validator set — this device cannot open it." != "") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 108, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::Some(::ui_lang_guest::wire::LineHeight::Relative(((1.45) as f32).max(f32::EPSILON).min(f32::MAX))), shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:108", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[70]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("Ask a validator to propose this node for the validator set — this device cannot open it.".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:100", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((2.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:87", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Left), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_60(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 5, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: __ice_node_scope.clone(), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[73]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("NODE".to_owned()).to_string() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_61(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 235, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (9.0) as f32, right: (12.0) as f32, bottom: (9.0) as f32, left: (12.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[63]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 244, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 249, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:249", __ice_use_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("public key".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 260, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::WordOrGlyph), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:260", __ice_use_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[13]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: (crate::host::keep_str((!(self.facts.node_key).is_empty()), ::std::convert::AsRef::as_ref(&(self.facts.node_key)), ::std::convert::AsRef::as_ref(&("—"))).to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:244", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((10.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Left), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_62(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 235, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (9.0) as f32, right: (12.0) as f32, bottom: (9.0) as f32, left: (12.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[63]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 244, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 249, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:249", __ice_use_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("data directory".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 260, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::WordOrGlyph), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:260", __ice_use_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[13]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: (crate::host::keep_str((!(self.node_data_dir).is_empty()), ::std::convert::AsRef::as_ref(&(self.node_data_dir)), ::std::convert::AsRef::as_ref(&("—"))).to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:244", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((10.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Left), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_63(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 5, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: __ice_node_scope.clone(), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[73]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("NETWORK".to_owned()).to_string() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_64(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 203, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (11.0) as f32, right: (13.0) as f32, bottom: (11.0) as f32, left: (13.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[63]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((10.0) as f32).max(0.0).min(f32::MAX), ((10.0) as f32).max(0.0).min(f32::MAX), ((10.0) as f32).max(0.0).min(f32::MAX), ((10.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 212, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 213, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:213", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[73]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("HEIGHT".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 219, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 220, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:220", __ice_use_scope), size: ::std::option::Option::Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (crate::host::height_label_short(self.facts.node_height).to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if ("" != "") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 227, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:227", __ice_use_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:219", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((4.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:212", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((3.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_65(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 203, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (11.0) as f32, right: (13.0) as f32, bottom: (11.0) as f32, left: (13.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[63]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((10.0) as f32).max(0.0).min(f32::MAX), ((10.0) as f32).max(0.0).min(f32::MAX), ((10.0) as f32).max(0.0).min(f32::MAX), ((10.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 212, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 213, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:213", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[73]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("CHECKPOINT".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 219, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 220, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:220", __ice_use_scope), size: ::std::option::Option::Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (crate::host::height_label_short(self.facts.node_checkpoint).to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if ("" != "") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 227, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:227", __ice_use_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:219", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((4.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:212", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((3.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_66(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 203, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (11.0) as f32, right: (13.0) as f32, bottom: (11.0) as f32, left: (13.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[63]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((10.0) as f32).max(0.0).min(f32::MAX), ((10.0) as f32).max(0.0).min(f32::MAX), ((10.0) as f32).max(0.0).min(f32::MAX), ((10.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 212, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 213, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:213", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[73]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("LAST FINALIZED".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 219, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 220, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:220", __ice_use_scope), size: ::std::option::Option::Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (crate::host::relative_time(self.facts.node_last_finalized, self.wall_now).to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if ("" != "") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 227, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:227", __ice_use_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:219", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((4.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:212", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((3.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_67(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 203, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (11.0) as f32, right: (13.0) as f32, bottom: (11.0) as f32, left: (13.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[63]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((10.0) as f32).max(0.0).min(f32::MAX), ((10.0) as f32).max(0.0).min(f32::MAX), ((10.0) as f32).max(0.0).min(f32::MAX), ((10.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 212, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 213, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:213", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[73]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("VALIDATORS REACHED".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 219, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 220, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:220", __ice_use_scope), size: ::std::option::Option::Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (crate::host::reading_pair(::std::convert::AsRef::as_ref(&(self.facts.node_reachable_label)), ::std::convert::AsRef::as_ref(&(self.facts.node_quorum_label))).to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if ("of quorum" != "") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 227, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:227", __ice_use_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("of quorum".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:219", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((4.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:212", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((3.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_68(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 25, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 26, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:26", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (13.0) as f32, right: (15.0) as f32, bottom: (13.0) as f32, left: (15.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 31, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 36, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:36", __ice_use_scope), size: ::std::option::Option::Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[15]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Node version".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 41, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 42, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:42", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[13]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("—".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:31", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((10.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!false) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 49, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:49", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 54, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_69(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 25, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 26, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:26", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (13.0) as f32, right: (15.0) as f32, bottom: (13.0) as f32, left: (15.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 31, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 36, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:36", __ice_use_scope), size: ::std::option::Option::Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[15]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Node version".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 41, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 42, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:42", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[13]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (self.facts.node_version.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:31", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((10.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!false) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 49, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:49", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 54, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_71(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 25, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 26, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:26", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (13.0) as f32, right: (15.0) as f32, bottom: (13.0) as f32, left: (15.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 31, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 36, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:36", __ice_use_scope), size: ::std::option::Option::Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[15]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Phase".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 41, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 42, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:42", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[13]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (crate::host::reading_pair(::std::convert::AsRef::as_ref(&(self.facts.sync_line)), ::std::convert::AsRef::as_ref(&(crate::host::relative_time(self.facts.node_phase_since, self.wall_now)))).to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:31", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((10.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!false) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 49, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:49", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 54, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_72(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 25, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 26, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:26", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (13.0) as f32, right: (15.0) as f32, bottom: (13.0) as f32, left: (15.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 31, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 36, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:36", __ice_use_scope), size: ::std::option::Option::Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[15]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Sync retries / failures, cumulative".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 41, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 42, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:42", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[13]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (crate::host::reading_pair(::std::convert::AsRef::as_ref(&(crate::host::count_label(self.facts.node_sync_retries))), ::std::convert::AsRef::as_ref(&(crate::host::count_label(self.facts.node_sync_failures)))).to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:31", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((10.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!(self.facts.node_sync_last_error).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 49, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:49", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 54, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_73(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 25, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 26, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:26", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (13.0) as f32, right: (15.0) as f32, bottom: (13.0) as f32, left: (15.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 31, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 36, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:36", __ice_use_scope), size: ::std::option::Option::Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[15]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Last sync error".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 41, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 42, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:42", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[13]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (self.facts.node_sync_last_error.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:31", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((10.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!false) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 49, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:49", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 54, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_74(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 25, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 26, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:26", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (13.0) as f32, right: (15.0) as f32, bottom: (13.0) as f32, left: (15.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 31, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 36, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:36", __ice_use_scope), size: ::std::option::Option::Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[15]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("App hash".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 41, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 42, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:42", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[13]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (self.facts.node_root_hash.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:31", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((10.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!true) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 49, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:49", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 54, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_75(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_cb_0: impl Fn() -> __NodeViewMessage + Clone + 'static, __ice_cb_1: impl Fn(::std::string::String, ::std::string::String) -> __NodeViewMessage + Clone + 'static, __ice_cb_2: impl Fn(::std::string::String) -> __NodeViewMessage + Clone + 'static, __ice_cb_3: impl Fn(::std::string::String) -> __NodeViewMessage + Clone + 'static, __ice_cb_4: impl Fn() -> __NodeViewMessage + Clone + 'static, __ice_cb_5: impl Fn(NodeTab) -> __NodeViewMessage + Clone + 'static) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 13, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: true, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[63]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((11.0) as f32).max(0.0).min(f32::MAX), ((11.0) as f32).max(0.0).min(f32::MAX), ((11.0) as f32).max(0.0).min(f32::MAX), ((11.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 21, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 22, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = (|| { let __slot_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 416, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 417, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_70(__ice_palette, format!("{}/NodeBuildRow@2281", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 418, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_71(__ice_palette, format!("{}/KeyValueRow@2282", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 427, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_72(__ice_palette, format!("{}/KeyValueRow@2291", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!(self.facts.node_sync_last_error).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 436, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_73(__ice_palette, format!("{}/KeyValueRow@2300", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 441, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_74(__ice_palette, format!("{}/KeyValueRow@2305", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:416", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:21", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_76(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 5, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: __ice_node_scope.clone(), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[73]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("PEERS".to_owned()).to_string() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_77(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_cb_0: impl Fn() -> __NodeViewMessage + Clone + 'static, __ice_cb_1: impl Fn(::std::string::String, ::std::string::String) -> __NodeViewMessage + Clone + 'static, __ice_cb_2: impl Fn(::std::string::String) -> __NodeViewMessage + Clone + 'static, __ice_cb_3: impl Fn(::std::string::String) -> __NodeViewMessage + Clone + 'static, __ice_cb_4: impl Fn() -> __NodeViewMessage + Clone + 'static, __ice_cb_5: impl Fn(NodeTab) -> __NodeViewMessage + Clone + 'static) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 13, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: true, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[63]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((11.0) as f32).max(0.0).min(f32::MAX), ((11.0) as f32).max(0.0).min(f32::MAX), ((11.0) as f32).max(0.0).min(f32::MAX), ((11.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 21, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 22, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = (|| { let __slot_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 450, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); for (__ice_index, peer) in self.node_peers.iter().enumerate() { let __for_scope = format!("{}/@for:2315({})", __ice_use_scope, __ice_index); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 452, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:452", __for_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (11.0) as f32, right: (15.0) as f32, bottom: (11.0) as f32, left: (15.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 457, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); if peer.live { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 463, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_11(__ice_palette, format!("{}/Dot@2327", __for_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!peer.live) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 465, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:465", __for_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((7.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((7.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[98]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((3.5) as f32).max(0.0).min(f32::MAX), ((3.5) as f32).max(0.0).min(f32::MAX), ((3.5) as f32).max(0.0).min(f32::MAX), ((3.5) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 471, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 472, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:472", __for_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: (peer.key.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 479, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:479", __for_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (peer.role.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:457", __for_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:450", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:21", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
}
}

#[allow(warnings, clippy::all)]
mod __ice_group_log_timeline_443633be {
use super::*;
impl super::NodeView {
pub(super) fn __ice_component_use_59(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_cb_0: impl Fn() -> __NodeViewMessage + Clone + 'static, __ice_cb_1: impl Fn(::std::string::String, ::std::string::String) -> __NodeViewMessage + Clone + 'static, __ice_cb_2: impl Fn(::std::string::String) -> __NodeViewMessage + Clone + 'static, __ice_cb_3: impl Fn(::std::string::String) -> __NodeViewMessage + Clone + 'static, __ice_cb_4: impl Fn() -> __NodeViewMessage + Clone + 'static, __ice_cb_5: impl Fn(NodeTab) -> __NodeViewMessage + Clone + 'static) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/log-timeline.ice", 6, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: true, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: 20.0, right: 20.0, bottom: 20.0, left: 20.0 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(1.0), radius: ::std::option::Option::Some([((11.0) as f32).max(0.0).min(f32::MAX), ((11.0) as f32).max(0.0).min(f32::MAX), ((11.0) as f32).max(0.0).min(f32::MAX), ((11.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/log-timeline.ice", 7, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/log-timeline.ice", 8, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/log-timeline.ice", 9, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::Some(::ui_lang_guest::wire::LineHeight::Relative(1.35f32)), shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:9", __ice_use_scope), size: ::std::option::Option::Some(16.0f32), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Log ring".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/log-timeline.ice", 10, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::Some(::ui_lang_guest::wire::LineHeight::Relative(1.5f32)), shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:10", __ice_use_scope), size: ::std::option::Option::Some(12.5f32), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Live node events retained in the in-memory ring.".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:8", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((4.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/log-timeline.ice", 11, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = (|| { let __slot_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 329, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 330, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 331, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/log-filter", __ice_node_scope); ::ui_lang_guest::wire::Node::Input { options: ::ui_lang_guest::wire::InputOptions { label: ("Filter logs".to_owned()).to_string(), description: ::std::option::Option::None, disabled: false, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((6.2) as f32)), text_size: ::std::option::Option::Some((13.0) as f32), line_height: ::std::option::Option::Some((1.2) as f32), align: ::std::option::Option::None, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: __ice_node_scope.clone(), placeholder: ::std::string::String::from("filter logs…".to_owned()), value: (self.node_log_filter).to_string(), on_input: ::ui_lang_guest::slots::handler::<::std::string::String, __NodeViewMessage>(::std::boxed::Box::new({ let __route = { let __route_callback = (__ice_cb_3).clone(); move |__value| (__route_callback)(__value) }; move |__sent: ::std::string::String| ::std::option::Option::Some(__route(__sent)) })), on_submit: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((200.0) as f32)), secure: (false), style: ::std::boxed::Box::new(::ui_lang_guest::wire::InputStyle { utility: ::ui_lang_guest::wire::InputFace { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(1f32), radius: ::std::option::Option::Some([10f32; 4]) }), ..Default::default() }, focus_border: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), focused_hovered: ::std::option::Option::None, active: ::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX)]) }), value: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), placeholder: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[72]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), selection: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.180000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[6]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[40]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::None, radius: ::std::option::Option::None }), value: ::std::option::Option::None, placeholder: ::std::option::Option::None, selection: ::std::option::Option::None }), focused: ::std::option::Option::None, disabled: ::std::option::Option::None }) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 343, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if self.admin { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 349, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/live-log-filter", __ice_node_scope); ::ui_lang_guest::wire::Node::Input { options: ::ui_lang_guest::wire::InputOptions { label: ("Live tracing filter".to_owned()).to_string(), description: ::std::option::Option::None, disabled: false, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((6.2) as f32)), text_size: ::std::option::Option::Some((13.0) as f32), line_height: ::std::option::Option::Some((1.2) as f32), align: ::std::option::Option::None, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: __ice_node_scope.clone(), placeholder: ::std::string::String::from("info,ducktape::join=debug".to_owned()), value: (self.live_log_filter).to_string(), on_input: ::ui_lang_guest::slots::handler::<::std::string::String, __NodeViewMessage>(::std::boxed::Box::new({ let __route = { let __route_callback = (__ice_cb_2).clone(); move |__value| (__route_callback)(__value) }; move |__sent: ::std::string::String| ::std::option::Option::Some(__route(__sent)) })), on_submit: ::std::option::Option::Some(::ui_lang_guest::slots::message((__ice_cb_0)())), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((260.0) as f32)), secure: (false), style: ::std::boxed::Box::new(::ui_lang_guest::wire::InputStyle { utility: ::ui_lang_guest::wire::InputFace { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(1f32), radius: ::std::option::Option::Some([10f32; 4]) }), ..Default::default() }, focus_border: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), focused_hovered: ::std::option::Option::None, active: ::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX)]) }), value: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), placeholder: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[72]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), selection: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.180000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[6]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[40]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::None, radius: ::std::option::Option::None }), value: ::std::option::Option::None, placeholder: ::std::option::Option::None, selection: ::std::option::Option::None }), focused: ::std::option::Option::None, disabled: ::std::option::Option::None }) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if self.admin { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 363, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:363", __ice_use_scope), content: ::ui_lang_guest::wire::ButtonContent::Label(::std::string::String::from("Retune")), label: ::std::option::Option::None, on_press: if ((self.live_log_filter).is_empty()) { ::std::option::Option::None } else { ::std::option::Option::Some(::ui_lang_guest::slots::message((__ice_cb_0)())) }, width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((7.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[12]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[13]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[40]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(1.0), radius: ::std::option::Option::Some([9.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[6]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face::default(), hovered: ::std::option::Option::None, pressed: ::std::option::Option::None, disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:330", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((9.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Right), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!(self.live_filter_note).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 369, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:369", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (self.live_filter_note.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 370, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_58(__ice_palette, format!("{}/LogConsole@2234", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:329", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((9.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __slot_content })();
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:7", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((12.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
}
}

#[allow(warnings, clippy::all)]
mod __ice_group_node_b786ec24 {
use super::*;
impl super::NodeView {
pub(super) fn __ice_component_use_8(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_0: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 722, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); if (__ice_arg_0).is_empty() { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 724, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:724", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[13]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("—".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(__ice_arg_0).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 731, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:731", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[13]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (__ice_arg_0.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_9(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_1: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 712, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 713, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:713", __ice_use_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[73]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("root".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 719, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_8(__ice_palette, format!("{}/ModuleHash@1353", __ice_use_scope), __ice_arg_1.to_owned()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((5.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_10(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_1: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 712, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 713, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:713", __ice_use_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[73]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("code".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 719, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_8(__ice_palette, format!("{}/ModuleHash@1353", __ice_use_scope), __ice_arg_1.to_owned()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((5.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_12(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_0: bool) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 766, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); if __ice_arg_0 { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 768, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:768", __ice_use_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (4.0) as f32, right: (9.0) as f32, bottom: (4.0) as f32, left: (9.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[18]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[19]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 776, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:776", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[16]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("SWAP ARMED".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!__ice_arg_0) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 783, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:783", __ice_use_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (4.0) as f32, right: (9.0) as f32, bottom: (4.0) as f32, left: (9.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[32]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[33]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 791, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:791", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[30]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("SWAP PENDING".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_13(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_0: bool, __ice_arg_1: bool) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 741, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); if (!__ice_arg_0) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 743, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:743", __ice_use_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (4.0) as f32, right: (9.0) as f32, bottom: (4.0) as f32, left: (9.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[27]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[28]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 751, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 752, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_11(__ice_palette, format!("{}/Dot@1386", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 753, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:753", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[25]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("ACTIVE".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:751", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((6.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if __ice_arg_0 { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 760, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_12(__ice_palette, format!("{}/ModuleSwapChip@1394", __ice_use_scope), __ice_arg_1));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_14(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_1: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 712, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 713, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:713", __ice_use_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[73]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("target".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 719, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_8(__ice_palette, format!("{}/ModuleHash@1353", __ice_use_scope), __ice_arg_1.to_owned()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((5.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_15(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_0: crate::host::ModuleRow) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 804, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (11.0) as f32, right: (13.0) as f32, bottom: (11.0) as f32, left: (13.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[90]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[19]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 815, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 816, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 821, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:821", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[16]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("PENDING SWAP".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 827, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 828, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_14(__ice_palette, format!("{}/ModuleHashField@1462", __ice_use_scope), __ice_arg_0.pending_hash.to_owned()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:816", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((7.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 829, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 834, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 835, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:835", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[73]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("ACTIVATES AT".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 841, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:841", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (crate::host::height_label_short(__ice_arg_0.activation_height)).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:834", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((3.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 847, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 848, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:848", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[73]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("READY SIGNALS".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 854, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:854", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (__ice_arg_0.readiness).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:847", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((3.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:829", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((22.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Left), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:815", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((9.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_16(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_0: crate::host::ModuleRow) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 652, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (13.0) as f32, right: (15.0) as f32, bottom: (13.0) as f32, left: (15.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[63]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((11.0) as f32).max(0.0).min(f32::MAX), ((11.0) as f32).max(0.0).min(f32::MAX), ((11.0) as f32).max(0.0).min(f32::MAX), ((11.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 663, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 664, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 669, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:669", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((40.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((40.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((10.0) as f32).max(0.0).min(f32::MAX), ((10.0) as f32).max(0.0).min(f32::MAX), ((10.0) as f32).max(0.0).min(f32::MAX), ((10.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 677, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:677", __ice_use_scope), size: ::std::option::Option::Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[70]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (crate::host::initial_of(::std::convert::AsRef::as_ref(&(__ice_arg_0.id)))).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 683, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 684, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:684", __ice_use_scope), size: ::std::option::Option::Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (__ice_arg_0.id.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 690, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 696, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:696", __ice_use_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[70]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (__ice_arg_0.category.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 702, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_9(__ice_palette, format!("{}/ModuleHashField@1336", __ice_use_scope), __ice_arg_0.root.to_owned()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 703, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_10(__ice_palette, format!("{}/ModuleHashField@1337", __ice_use_scope), __ice_arg_0.code_hash.to_owned()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:690", __ice_use_scope), wrap: Some(::ui_lang_guest::wire::Wrap { spacing: ::std::option::Option::Some((3.0) as f32), align: ::std::option::Option::None }), axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:683", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((3.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 704, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_13(__ice_palette, format!("{}/ModuleStateChip@1338", __ice_use_scope), (!(__ice_arg_0.pending_hash).is_empty()), __ice_arg_0.ready));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:664", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((11.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!(__ice_arg_0.pending_hash).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 706, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_15(__ice_palette, format!("{}/ModulePendingPlate@1340", __ice_use_scope), __ice_arg_0.clone()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:663", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((11.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_17(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 607, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 608, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 613, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_6(__ice_palette, format!("{}/GroupLabel@1247", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 614, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:614", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[60]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 619, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 620, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 621, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:621", __ice_use_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[73]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (((self.module_rows).len() as i64)).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 627, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:627", __ice_use_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[73]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("registered".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:620", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((5.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:608", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((12.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (self.module_rows).is_empty() { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 634, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_7(__ice_palette, format!("{}/EmptyPlate@1268", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(self.module_rows).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 636, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); for (__ice_index, entry) in self.module_rows.iter().enumerate() { let __for_scope = format!("{}/@for:1271({})", __ice_use_scope, __ice_index); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 638, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_16(__ice_palette, format!("{}/ModuleCard@1272", __for_scope), entry.clone()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:636", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((9.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((13.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_19(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 347, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 349, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 354, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:354", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[27]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 362, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:362", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[25]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("✓".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 368, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:368", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[15]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("Sign quorum & finalize rounds".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:349", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!true) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 374, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 379, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:379", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 387, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:387", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[74]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("–".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 393, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:393", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[74]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("Sign quorum & finalize rounds".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:374", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_20(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 347, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 349, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 354, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:354", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[27]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 362, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:362", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[25]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("✓".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 368, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:368", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[15]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("Invite members & assign roles".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:349", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!true) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 374, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 379, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:379", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 387, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:387", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[74]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("–".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 393, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:393", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[74]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("Invite members & assign roles".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:374", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_21(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 347, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 349, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 354, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:354", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[27]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 362, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:362", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[25]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("✓".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 368, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:368", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[15]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("Install & remove modules".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:349", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!true) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 374, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 379, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:379", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 387, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:387", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[74]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("–".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 393, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:393", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[74]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("Install & remove modules".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:374", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_22(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 347, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 349, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 354, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:354", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[27]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 362, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:362", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[25]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("✓".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 368, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:368", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[15]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("Edit network settings".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:349", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!true) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 374, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 379, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:379", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 387, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:387", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[74]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("–".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 393, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:393", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[74]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("Edit network settings".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:374", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_23(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 347, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 349, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 354, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:354", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[27]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 362, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:362", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[25]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("✓".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 368, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:368", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[15]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("Read & verify finality".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:349", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!true) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 374, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 379, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:379", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 387, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:387", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[74]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("–".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 393, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:393", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[74]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("Read & verify finality".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:374", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_24(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 347, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 349, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 354, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:354", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[27]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 362, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:362", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[25]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("✓".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 368, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:368", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[15]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("Send · react · thread".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:349", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!true) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 374, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 379, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:379", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 387, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:387", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[74]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("–".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 393, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:393", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[74]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("Send · react · thread".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:374", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_25(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 347, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); if (!false) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 374, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 379, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:379", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 387, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:387", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[74]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("–".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 393, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:393", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[74]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("Propose modules & members".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:374", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_26(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 347, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); if (!false) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 374, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 379, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:379", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 387, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:387", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[74]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("–".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 393, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:393", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[74]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("Sign quorum · finalize".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:374", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_28(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 401, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (5.0) as f32, right: (10.0) as f32, bottom: (5.0) as f32, left: (10.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[87]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[60]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 411, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 412, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_27(__ice_palette, format!("{}/Icon@1046", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 417, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:417", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Propose modules & members".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:411", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((6.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_29(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 401, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (5.0) as f32, right: (10.0) as f32, bottom: (5.0) as f32, left: (10.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[87]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[60]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 411, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 412, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_27(__ice_palette, format!("{}/Icon@1046", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 417, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:417", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Invite members".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:411", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((6.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_30(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 401, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (5.0) as f32, right: (10.0) as f32, bottom: (5.0) as f32, left: (10.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[87]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[60]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 411, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 412, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_27(__ice_palette, format!("{}/Icon@1046", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 417, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:417", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Change roles".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:411", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((6.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_31(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 401, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (5.0) as f32, right: (10.0) as f32, bottom: (5.0) as f32, left: (10.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[87]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[60]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 411, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 412, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_27(__ice_palette, format!("{}/Icon@1046", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 417, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:417", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Network settings".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:411", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((6.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_33(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 347, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 349, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 354, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:354", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[27]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 362, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:362", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[25]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("✓".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 368, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:368", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[15]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("Read chat & threads".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:349", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!true) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 374, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 379, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:379", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 387, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:387", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[74]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("–".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 393, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:393", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[74]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("Read chat & threads".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:374", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_34(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 347, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 349, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 354, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:354", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[27]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 362, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:362", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[25]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("✓".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 368, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:368", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[15]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("Read governance".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:349", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!true) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 374, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 379, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:379", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 387, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:387", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[74]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("–".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 393, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:393", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[74]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("Read governance".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:374", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_35(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 347, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 349, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 354, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:354", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[27]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 362, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:362", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[25]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("✓".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 368, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:368", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[15]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("Browse Forge".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:349", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!true) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 374, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 379, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:379", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((17.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX), ((8.5) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 387, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:387", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[74]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("–".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 393, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:393", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[74]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("Browse Forge".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:374", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_36(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 401, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (5.0) as f32, right: (10.0) as f32, bottom: (5.0) as f32, left: (10.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[87]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[60]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 411, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 412, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_27(__ice_palette, format!("{}/Icon@1046", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 417, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:417", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Propose".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:411", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((6.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_37(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 401, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (5.0) as f32, right: (10.0) as f32, bottom: (5.0) as f32, left: (10.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[87]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[60]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 411, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 412, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_27(__ice_palette, format!("{}/Icon@1046", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 417, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:417", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Forge contribute & merge".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:411", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((6.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_38(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 401, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (5.0) as f32, right: (10.0) as f32, bottom: (5.0) as f32, left: (10.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[87]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[60]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 411, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 412, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_27(__ice_palette, format!("{}/Icon@1046", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 417, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:417", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Sign quorum".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:411", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((6.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_39(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 401, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (5.0) as f32, right: (10.0) as f32, bottom: (5.0) as f32, left: (10.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[87]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[60]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 411, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 412, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_27(__ice_palette, format!("{}/Icon@1046", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 417, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:417", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Invite".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:411", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((6.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_40(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 70, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 71, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_18(__ice_palette, format!("{}/GroupLabel@705", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (self.tier == "validator") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 74, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:74", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (16.0) as f32, right: (18.0) as f32, bottom: (16.0) as f32, left: (18.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[106]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[28]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((13.0) as f32).max(0.0).min(f32::MAX), ((13.0) as f32).max(0.0).min(f32::MAX), ((13.0) as f32).max(0.0).min(f32::MAX), ((13.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 85, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 86, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 87, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 92, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:92", __ice_use_scope), size: ::std::option::Option::Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("This node".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 98, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:98", __ice_use_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (2.0) as f32, right: (7.0) as f32, bottom: (2.0) as f32, left: (7.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 104, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:104", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[9]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("VALIDATOR · QUORUM SEAT".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:87", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((7.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 110, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:110", __ice_use_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[70]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("signs quorum · finalizes rounds · stores all history".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:86", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((3.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 116, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 117, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 118, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_19(__ice_palette, format!("{}/CapabilityCheck@752", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 119, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_20(__ice_palette, format!("{}/CapabilityCheck@753", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:117", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((9.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 120, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 121, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_21(__ice_palette, format!("{}/CapabilityCheck@755", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 122, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_22(__ice_palette, format!("{}/CapabilityCheck@756", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:120", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((9.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:116", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((9.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 123, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 124, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:124", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[28]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 129, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 130, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::Some(::ui_lang_guest::wire::LineHeight::Relative(((1.5) as f32).max(f32::EPSILON).min(f32::MAX))), shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:130", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[70]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("A quorum seat is granted and revoked by quorum only — this device cannot change it.".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:123", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((13.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:85", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((14.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!(self.tier == "validator")) && (self.tier == "resident")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 137, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:137", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (16.0) as f32, right: (18.0) as f32, bottom: (16.0) as f32, left: (18.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[63]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((13.0) as f32).max(0.0).min(f32::MAX), ((13.0) as f32).max(0.0).min(f32::MAX), ((13.0) as f32).max(0.0).min(f32::MAX), ((13.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 148, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 149, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 150, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 155, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:155", __ice_use_scope), size: ::std::option::Option::Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("This node".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 161, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:161", __ice_use_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (2.0) as f32, right: (7.0) as f32, bottom: (2.0) as f32, left: (7.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[40]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 169, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:169", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("RESIDENT · FULL NODE".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:150", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((7.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 175, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:175", __ice_use_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[70]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("full node · stores all history · cannot sign quorum".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:149", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((3.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 181, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 182, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 183, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_23(__ice_palette, format!("{}/CapabilityCheck@817", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 184, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_24(__ice_palette, format!("{}/CapabilityCheck@818", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:182", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((9.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 185, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 190, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_25(__ice_palette, format!("{}/CapabilityCheck@824", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 191, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_26(__ice_palette, format!("{}/CapabilityCheck@825", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:185", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((9.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:181", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((9.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 192, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 193, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:193", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[60]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 198, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 199, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 200, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:200", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[30]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("VALIDATORS ONLY · QUORUM-GATED".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 206, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 211, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_28(__ice_palette, format!("{}/GatedChip@845", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 212, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_29(__ice_palette, format!("{}/GatedChip@846", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 213, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_30(__ice_palette, format!("{}/GatedChip@847", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 214, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_31(__ice_palette, format!("{}/GatedChip@848", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:206", __ice_use_scope), wrap: Some(::ui_lang_guest::wire::Wrap { spacing: ::std::option::Option::Some((7.0) as f32), align: ::std::option::Option::None }), axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((7.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 221, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_32(__ice_palette, format!("{}/GateNote@855", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:199", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((9.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:192", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((13.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:148", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((14.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!((self.tier == "validator") || (self.tier == "resident"))) && (self.tier == "guest")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 226, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:226", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (16.0) as f32, right: (18.0) as f32, bottom: (16.0) as f32, left: (18.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[84]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[33]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((13.0) as f32).max(0.0).min(f32::MAX), ((13.0) as f32).max(0.0).min(f32::MAX), ((13.0) as f32).max(0.0).min(f32::MAX), ((13.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 237, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 238, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 239, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 244, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:244", __ice_use_scope), size: ::std::option::Option::Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("This node".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 250, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:250", __ice_use_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (2.0) as f32, right: (7.0) as f32, bottom: (2.0) as f32, left: (7.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[32]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[33]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 258, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:258", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[30]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("GUEST · LIGHT NODE".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:239", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((7.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 264, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:264", __ice_use_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[70]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("read-only · verifies finalized headers".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:238", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((3.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 270, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 271, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 272, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_23(__ice_palette, format!("{}/CapabilityCheck@906", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 273, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_33(__ice_palette, format!("{}/CapabilityCheck@907", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:271", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((9.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 274, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 275, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_34(__ice_palette, format!("{}/CapabilityCheck@909", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 276, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_35(__ice_palette, format!("{}/CapabilityCheck@910", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:274", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((9.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:270", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((9.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 277, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 278, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:278", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[33]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 283, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 284, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 285, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:285", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[30]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("GUEST · NO SIGNING, NO CONTRIBUTION".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 291, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 296, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_36(__ice_palette, format!("{}/GatedChip@930", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 297, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_37(__ice_palette, format!("{}/GatedChip@931", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 298, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_38(__ice_palette, format!("{}/GatedChip@932", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 299, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_39(__ice_palette, format!("{}/GatedChip@933", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:291", __ice_use_scope), wrap: Some(::ui_lang_guest::wire::Wrap { spacing: ::std::option::Option::Some((7.0) as f32), align: ::std::option::Option::None }), axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((7.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:284", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((9.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:277", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((13.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:237", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((14.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(((self.tier == "validator") || (self.tier == "resident")) || (self.tier == "guest"))) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 301, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:301", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (16.0) as f32, right: (18.0) as f32, bottom: (16.0) as f32, left: (18.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[63]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((13.0) as f32).max(0.0).min(f32::MAX), ((13.0) as f32).max(0.0).min(f32::MAX), ((13.0) as f32).max(0.0).min(f32::MAX), ((13.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 312, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 313, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 318, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:318", __ice_use_scope), size: ::std::option::Option::Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("This node".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 324, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:324", __ice_use_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (2.0) as f32, right: (7.0) as f32, bottom: (2.0) as f32, left: (7.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 330, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:330", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("STANDING UNKNOWN".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:313", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((7.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 336, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::Some(::ui_lang_guest::wire::LineHeight::Relative(((1.5) as f32).max(f32::EPSILON).min(f32::MAX))), shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:336", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[70]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("The valset roster has not answered, so this node's standing is not known yet. Nothing is claimed until it does.".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:312", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((9.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((9.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_41(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 489, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); if (self.tier == "validator") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 491, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:491", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((92.0) as f32)), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (10.0) as f32, right: (0.0) as f32, bottom: (10.0) as f32, left: (0.0) as f32 }), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[91]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 498, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:498", __ice_use_scope), size: ::std::option::Option::Some(((9.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[69]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Validator".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(self.tier == "validator")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 505, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:505", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((92.0) as f32)), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (10.0) as f32, right: (0.0) as f32, bottom: (10.0) as f32, left: (0.0) as f32 }), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 512, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:512", __ice_use_scope), size: ::std::option::Option::Some(((9.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[69]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Validator".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_42(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 489, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); if (self.tier == "resident") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 491, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:491", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((92.0) as f32)), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (10.0) as f32, right: (0.0) as f32, bottom: (10.0) as f32, left: (0.0) as f32 }), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[91]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 498, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:498", __ice_use_scope), size: ::std::option::Option::Some(((9.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[69]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Full".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(self.tier == "resident")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 505, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:505", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((92.0) as f32)), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (10.0) as f32, right: (0.0) as f32, bottom: (10.0) as f32, left: (0.0) as f32 }), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 512, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:512", __ice_use_scope), size: ::std::option::Option::Some(((9.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[69]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Full".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_43(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 489, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); if (self.tier == "guest") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 491, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:491", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((92.0) as f32)), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (10.0) as f32, right: (0.0) as f32, bottom: (10.0) as f32, left: (0.0) as f32 }), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[91]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 498, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:498", __ice_use_scope), size: ::std::option::Option::Some(((9.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[69]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Light".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(self.tier == "guest")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 505, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:505", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((92.0) as f32)), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (10.0) as f32, right: (0.0) as f32, bottom: (10.0) as f32, left: (0.0) as f32 }), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 512, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:512", __ice_use_scope), size: ::std::option::Option::Some(((9.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[69]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Light".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_44(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 566, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 568, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:568", __ice_use_scope), size: ::std::option::Option::Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[25]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("✓".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!true) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 575, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:575", __ice_use_scope), size: ::std::option::Option::Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[98]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("−".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_45(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 545, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); if (self.tier == "validator") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 547, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:547", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((92.0) as f32)), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (11.0) as f32, right: (0.0) as f32, bottom: (11.0) as f32, left: (0.0) as f32 }), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[86]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 554, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_44(__ice_palette, format!("{}/MatrixTick@1188", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(self.tier == "validator")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 556, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:556", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((92.0) as f32)), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (11.0) as f32, right: (0.0) as f32, bottom: (11.0) as f32, left: (0.0) as f32 }), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 563, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_44(__ice_palette, format!("{}/MatrixTick@1197", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_46(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 545, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); if (self.tier == "resident") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 547, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:547", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((92.0) as f32)), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (11.0) as f32, right: (0.0) as f32, bottom: (11.0) as f32, left: (0.0) as f32 }), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[86]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 554, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_44(__ice_palette, format!("{}/MatrixTick@1188", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(self.tier == "resident")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 556, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:556", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((92.0) as f32)), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (11.0) as f32, right: (0.0) as f32, bottom: (11.0) as f32, left: (0.0) as f32 }), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 563, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_44(__ice_palette, format!("{}/MatrixTick@1197", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_47(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 545, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); if (self.tier == "guest") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 547, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:547", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((92.0) as f32)), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (11.0) as f32, right: (0.0) as f32, bottom: (11.0) as f32, left: (0.0) as f32 }), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[86]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 554, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_44(__ice_palette, format!("{}/MatrixTick@1188", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(self.tier == "guest")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 556, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:556", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((92.0) as f32)), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (11.0) as f32, right: (0.0) as f32, bottom: (11.0) as f32, left: (0.0) as f32 }), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 563, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_44(__ice_palette, format!("{}/MatrixTick@1197", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_48(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 520, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 521, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:521", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 526, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 527, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 528, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:528", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (11.0) as f32, right: (14.0) as f32, bottom: (11.0) as f32, left: (14.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 535, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:535", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[15]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Read & verify finality".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 536, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_45(__ice_palette, format!("{}/MatrixCell@1170", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 537, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_46(__ice_palette, format!("{}/MatrixCell@1171", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 538, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_47(__ice_palette, format!("{}/MatrixCell@1172", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:527", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_49(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 566, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); if (!false) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 575, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:575", __ice_use_scope), size: ::std::option::Option::Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[98]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("−".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_50(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 545, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); if (self.tier == "guest") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 547, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:547", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((92.0) as f32)), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (11.0) as f32, right: (0.0) as f32, bottom: (11.0) as f32, left: (0.0) as f32 }), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[86]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 554, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_49(__ice_palette, format!("{}/MatrixTick@1188", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(self.tier == "guest")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 556, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:556", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((92.0) as f32)), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (11.0) as f32, right: (0.0) as f32, bottom: (11.0) as f32, left: (0.0) as f32 }), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 563, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_49(__ice_palette, format!("{}/MatrixTick@1197", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_51(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 520, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 521, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:521", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 526, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 527, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 528, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:528", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (11.0) as f32, right: (14.0) as f32, bottom: (11.0) as f32, left: (14.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 535, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:535", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[15]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Send · react · thread".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 536, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_45(__ice_palette, format!("{}/MatrixCell@1170", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 537, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_46(__ice_palette, format!("{}/MatrixCell@1171", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 538, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_50(__ice_palette, format!("{}/MatrixCell@1172", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:527", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_52(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 545, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); if (self.tier == "resident") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 547, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:547", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((92.0) as f32)), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (11.0) as f32, right: (0.0) as f32, bottom: (11.0) as f32, left: (0.0) as f32 }), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[86]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 554, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_49(__ice_palette, format!("{}/MatrixTick@1188", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(self.tier == "resident")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 556, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:556", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((92.0) as f32)), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (11.0) as f32, right: (0.0) as f32, bottom: (11.0) as f32, left: (0.0) as f32 }), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 563, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_49(__ice_palette, format!("{}/MatrixTick@1197", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_53(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 520, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 521, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:521", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 526, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 527, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 528, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:528", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (11.0) as f32, right: (14.0) as f32, bottom: (11.0) as f32, left: (14.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 535, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:535", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[15]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Propose modules & members".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 536, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_45(__ice_palette, format!("{}/MatrixCell@1170", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 537, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_52(__ice_palette, format!("{}/MatrixCell@1171", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 538, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_50(__ice_palette, format!("{}/MatrixCell@1172", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:527", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_54(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 520, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 521, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:521", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 526, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 527, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 528, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:528", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (11.0) as f32, right: (14.0) as f32, bottom: (11.0) as f32, left: (14.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 535, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:535", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[15]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Sign quorum · finalize".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 536, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_45(__ice_palette, format!("{}/MatrixCell@1170", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 537, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_52(__ice_palette, format!("{}/MatrixCell@1171", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 538, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_50(__ice_palette, format!("{}/MatrixCell@1172", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:527", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_55(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 435, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 436, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::Some(((640.0) as f32).max(0.0).min(f32::MAX)), max_height: ::std::option::Option::None, clip: true, key: format!("{}/@container:436", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[63]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((12.0) as f32).max(0.0).min(f32::MAX), ((12.0) as f32).max(0.0).min(f32::MAX), ((12.0) as f32).max(0.0).min(f32::MAX), ((12.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 445, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 446, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:446", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[87]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 447, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 448, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:448", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (10.0) as f32, right: (14.0) as f32, bottom: (10.0) as f32, left: (14.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 455, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:455", __ice_use_scope), size: ::std::option::Option::Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[70]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("capability".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 456, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_41(__ice_palette, format!("{}/MatrixHead@1090", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 457, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_42(__ice_palette, format!("{}/MatrixHead@1091", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 458, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_43(__ice_palette, format!("{}/MatrixHead@1092", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:447", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 459, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_48(__ice_palette, format!("{}/MatrixRow@1093", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 466, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_51(__ice_palette, format!("{}/MatrixRow@1100", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 473, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_53(__ice_palette, format!("{}/MatrixRow@1107", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 480, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_54(__ice_palette, format!("{}/MatrixRow@1114", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:445", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((13.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_56(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_0: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 935, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); if (__ice_arg_0 == "ERROR") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 937, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:937", __ice_use_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[20]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((48.0) as f32)), align_x: ::std::option::Option::None, content: (__ice_arg_0.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (__ice_arg_0 == "WARN") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 945, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:945", __ice_use_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[30]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((48.0) as f32)), align_x: ::std::option::Option::None, content: (__ice_arg_0.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((__ice_arg_0 != "ERROR") && (__ice_arg_0 != "WARN")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 953, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:953", __ice_use_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((48.0) as f32)), align_x: ::std::option::Option::None, content: (__ice_arg_0.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_57(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_0: crate::host::LogRow) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 911, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 916, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:916", __ice_use_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((170.0) as f32)), align_x: ::std::option::Option::None, content: (__ice_arg_0.time.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 923, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_56(__ice_palette, format!("{}/LogLevel@1557", __ice_use_scope), __ice_arg_0.level.to_owned()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 924, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:924", __ice_use_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: (__ice_arg_0.message.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((6.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Left), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_58(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 873, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((420.0) as f32)), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (13.0) as f32, right: (13.0) as f32, bottom: (13.0) as f32, left: (13.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[60]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((11.0) as f32).max(0.0).min(f32::MAX), ((11.0) as f32).max(0.0).min(f32::MAX), ((11.0) as f32).max(0.0).min(f32::MAX), ((11.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 882, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 883, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 884, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:884", __ice_use_scope), size: ::std::option::Option::Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[73]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("NODE LOG".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 890, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 891, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:891", __ice_use_scope), size: ::std::option::Option::Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (crate::host::count_label(((crate::host::visible_log(::std::convert::AsRef::as_ref(&(self.log_lines)), ::std::convert::AsRef::as_ref(&(self.node_log_filter)))).len() as i64))).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:883", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!(crate::host::log_note(((self.log_lines).len() as i64), ((crate::host::visible_log(::std::convert::AsRef::as_ref(&(self.log_lines)), ::std::convert::AsRef::as_ref(&(self.node_log_filter)))).len() as i64))).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 898, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:898", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (crate::host::log_note(((self.log_lines).len() as i64), ((crate::host::visible_log(::std::convert::AsRef::as_ref(&(self.log_lines)), ::std::convert::AsRef::as_ref(&(self.node_log_filter)))).len() as i64)).to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 899, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/node-log-lines", __ice_node_scope); ::ui_lang_guest::wire::Node::Scroll { on_scroll: ::std::option::Option::None, virtual_rows: false, key: __ice_node_scope.clone(), direction: ::ui_lang_guest::wire::ScrollDirection::Vertical, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), bar_hidden: false, bar_width: ::std::option::Option::None, bar_margin: ::std::option::Option::None, scroller_width: ::std::option::Option::None, bar_spacing: ::std::option::Option::None, anchor_x: ::ui_lang_guest::wire::ScrollAnchor::Start, anchor_y: ::ui_lang_guest::wire::ScrollAnchor::End, auto_scroll: (true), background: ::std::option::Option::None, border: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 906, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); for (__ice_index, line) in crate::host::visible_log(::std::convert::AsRef::as_ref(&(self.log_lines)), ::std::convert::AsRef::as_ref(&(self.node_log_filter))).iter().enumerate() { let __for_scope = format!("{}/@for:1541({})", __ice_use_scope, __ice_index); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 908, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_57(__ice_palette, format!("{}/LogLine@1542", __for_scope), line.clone()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:906", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((1.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:882", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_70(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __NodeViewMessage> { let __component_content: __IceElement<'_, __NodeViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 37, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __NodeViewMessage>> = ::std::vec::Vec::new(); if (self.facts.node_version).is_empty() { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 39, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_68(__ice_palette, format!("{}/KeyValueRow@673", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(self.facts.node_version).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/node.ice", 45, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __NodeViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_69(__ice_palette, format!("{}/KeyValueRow@679", __ice_use_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
}
}



ui_lang_guest::export_app!(
    NodeView,
    "Node",
    "This node: coherent status, standing, peers, logs and the code registry.",
    ["node"]
);
