//! The block explorer as a module-owned view on the kernel contract.
//!
//! The kernel pushes session facts only (`explorer.props`: connected, dark,
//! and the live head and sync line the app already holds). The block window
//! is this view's own — `rpc.blocks`, re-read on every `rpc.live` hit for the
//! `block` plane — and so is the workspace search, which fans out over
//! `rpc.query` and `rpc.view`. A clipboard copy is the one act that leaves as
//! an intent: the OS door is the kernel's. The endpoint, the key and the
//! password never cross.

pub mod host;

macro_rules! __ice_generated_items_4578706c6f72657256696577 { ($($item:item)*) => { $(#[allow(warnings, clippy::all)] $item)* }; }
__ice_generated_items_4578706c6f72657256696577! {
type __IceElement<'a, Message, Theme = ()> = <(&'a (), Message, Theme) as ::ui_lang_guest::wire::Erase>::Node;
pub(crate) type __IceMessage = __ExplorerViewMessage;
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
pub struct ExplorerView {
pub(crate) __ice_accessibility: ::ui_lang_runtime::Bridge<__ExplorerViewMessage>,
#[cfg(all(target_os = "windows", not(test)))]
pub(crate) __ice_accessibility_initial: ::std::option::Option<usize>,
#[cfg(all(target_os = "windows", not(test)))]
pub(crate) __ice_accessibility_pending: ::std::vec::Vec<__ExplorerViewMessage>,
pub(crate) active_palette: AppTheme,
pub(crate) connected: bool,
pub(crate) loading: bool,
pub(crate) blocks: ::std::vec::Vec<crate::host::ExplorerBlock>,
pub(crate) ops: ::std::vec::Vec<crate::host::ExplorerOp>,
pub(crate) head: i64,
pub(crate) sync_line: ::std::string::String,
pub(crate) ledger_serial: i64,
pub(crate) hits: ::std::vec::Vec<crate::host::ExplorerHit>,
pub(crate) kinds: ::std::vec::Vec<crate::host::KindCount>,
pub(crate) partial: ::std::string::String,
pub(crate) searching: bool,
pub(crate) sent_query: ::std::string::String,
pub(crate) search_serial: i64,
pub(crate) query: ::std::string::String,
pub(crate) kind: ::std::string::String,
pub(crate) selected: i64,
pub(crate) viewport_width: f64,
pub(crate) ledger_width: f64,
pub(crate) host_error: ::std::string::String,
pub(crate) sent: bool,
pub(crate) __ice_rev: [u64; 21],
}
impl ::std::fmt::Debug for ExplorerView { fn fmt(&self, __formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result { __formatter.write_str("ExplorerView") } }
#[derive(Clone)]
pub(crate) enum __ExplorerViewMessage {
__AccessibilitySnapshot(::std::boxed::Box<::ui_lang_runtime::Snapshot<__ExplorerViewMessage>>),
__AccessibilityAction(::ui_lang_runtime::ActionRequest),
__AccessibilityWindow(::iced::window::Id, ::iced::window::Event),
#[cfg(all(any(target_os = "windows", target_os = "macos"), not(test)))]
__AccessibilityNativeWindow(::ui_lang_runtime::NativeWindow),
__AccessibilityFocusNext(::std::option::Option<::iced::window::Id>),
__AccessibilityFocusPrevious(::std::option::Option<::iced::window::Id>),
__TemplateChanged,
SessionArrived(crate::host::SessionItem),
LedgerArrived(crate::host::LedgerItem),
SearchArrived(crate::host::SearchItem),
Refresh,
CopyToClipboard(::std::string::String, ::std::string::String),
SearchSubmit,
ClearExplorerSearch,
PickExplorerKind(::std::string::String),
SelectExplorerBlock(i64),
LedgerResized(f64, f64),
ViewportChanged(f64, f64),
__BindQuery(::std::string::String),
}
impl ::std::fmt::Debug for __ExplorerViewMessage { fn fmt(&self, __formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result { __formatter.write_str("__ExplorerViewMessage") } }
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_ExplorerBlock(_value: &crate::host::ExplorerBlock) {
let _: &i64 = &_value.height;
let _: &::std::string::String = &_value.hash;
let _: &::std::string::String = &_value.commit;
let _: &i64 = &_value.op_count;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_ExplorerOp(_value: &crate::host::ExplorerOp) {
let _: &i64 = &_value.height;
let _: &::std::string::String = &_value.proposer;
let _: &::std::string::String = &_value.target;
let _: &::std::string::String = &_value.disposition;
let _: &::std::string::String = &_value.op_hash;
let _: &::std::string::String = &_value.payload;
let _: &::std::string::String = &_value.trace;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_ExplorerHit(_value: &crate::host::ExplorerHit) {
let _: &::std::string::String = &_value.kind;
let _: &::std::string::String = &_value.code;
let _: &::std::string::String = &_value.title;
let _: &::std::string::String = &_value.snippet;
let _: &::std::string::String = &_value.meta;
let _: &::std::string::String = &_value.target;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_KindCount(_value: &crate::host::KindCount) {
let _: &::std::string::String = &_value.kind;
let _: &::std::string::String = &_value.label;
let _: &i64 = &_value.count;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_Session(_value: &crate::host::Session) {
let _: &bool = &_value.connected;
let _: &bool = &_value.dark;
let _: &i64 = &_value.head;
let _: &::std::string::String = &_value.sync_line;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_SessionItem(_value: &crate::host::SessionItem) {
let _: &crate::host::Session = &_value.next;
let _: &::std::string::String = &_value.error;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_LedgerItem(_value: &crate::host::LedgerItem) {
let _: &::std::vec::Vec<crate::host::ExplorerBlock> = &_value.blocks;
let _: &::std::vec::Vec<crate::host::ExplorerOp> = &_value.ops;
let _: &::std::string::String = &_value.error;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_SearchItem(_value: &crate::host::SearchItem) {
let _: &::std::vec::Vec<crate::host::ExplorerHit> = &_value.hits;
let _: &::std::vec::Vec<crate::host::KindCount> = &_value.kinds;
let _: &::std::string::String = &_value.partial;
let _: &::std::string::String = &_value.error;
}
#[allow(dead_code)] fn __ui_lang_check_subscription_session() { let _: ::iced::Subscription<crate::host::SessionItem> = crate::host::session(); }
#[allow(dead_code)] fn __ui_lang_check_subscription_ledger(arg0: i64) { let _: ::iced::Subscription<crate::host::LedgerItem> = crate::host::ledger(arg0); }
#[allow(dead_code)] fn __ui_lang_check_subscription_workspace_search(arg0: ::std::string::String, arg1: i64) { let _: ::iced::Subscription<crate::host::SearchItem> = crate::host::workspace_search(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_connection_serial_after(arg0: bool, arg1: bool, arg2: i64) { let _: i64 = crate::host::connection_serial_after(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_loading_after(arg0: bool, arg1: bool, arg2: bool) { let _: bool = crate::host::loading_after(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_copy<'a>(arg0: &'a str, arg1: &'a str) { let _: bool = crate::host::copy(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_icon<'a>(arg0: &'a str) { let _: ::std::vec::Vec<u8> = crate::host::icon(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_explorer_ops_at<'a>(arg0: &'a [crate::host::ExplorerOp], arg1: i64) { let _: ::std::vec::Vec<crate::host::ExplorerOp> = crate::host::explorer_ops_at(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_height_label(arg0: i64) { let _: ::std::string::String = crate::host::height_label(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_hex<'a>(arg0: &'a str) { let _: ::std::string::String = crate::host::hex(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_plural<'a>(arg0: i64, arg1: &'a str, arg2: &'a str) { let _: ::std::string::String = crate::host::plural(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_search_answer_stands<'a>(arg0: &'a str, arg1: &'a str, arg2: bool) { let _: bool = crate::host::search_answer_stands(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_ledger_width_after_delta(arg0: f64, arg1: f64, arg2: f64) { let _: f64 = crate::host::ledger_width_after_delta(arg0, arg1, arg2); }
}
__ice_generated_items_4578706c6f72657256696577! {
#[allow(unused_parens)]
impl ExplorerView {
#[must_use]
pub fn default_font() -> ::iced::Font { ::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Normal, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal } }
}
}
__ice_generated_items_4578706c6f72657256696577! {
#[allow(unused_parens)]
impl ExplorerView {
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
::iced::Theme::custom(::std::format!("ExplorerView/{}", __ice_palette.name), ::iced::theme::Palette {
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
fn __title(&self) -> ::std::string::String { "Explorer".to_owned() }
}
}
__ice_generated_items_4578706c6f72657256696577! {
#[allow(unused_parens)]
impl ExplorerView {
fn __state() -> Self {
Self {
__ice_accessibility: ::ui_lang_runtime::Bridge::new(),
#[cfg(all(target_os = "windows", not(test)))]
__ice_accessibility_initial: ::std::option::Option::None,
#[cfg(all(target_os = "windows", not(test)))]
__ice_accessibility_pending: ::std::vec::Vec::new(),
active_palette: AppTheme::App,
connected: false,
loading: false,
blocks: ::std::vec::Vec::new(),
ops: ::std::vec::Vec::new(),
head: 0,
sync_line: "".to_owned(),
ledger_serial: 0,
hits: ::std::vec::Vec::new(),
kinds: ::std::vec::Vec::new(),
partial: "".to_owned(),
searching: false,
sent_query: "".to_owned(),
search_serial: 0,
query: "".to_owned(),
kind: "all".to_owned(),
selected: 0,
viewport_width: 1280.0,
ledger_width: 340.0,
host_error: "".to_owned(),
sent: false,
__ice_rev: [::ui_lang_runtime::rev::seed(); 21],
}
}
fn __boot_task(&mut self) -> ::iced::Task<__ExplorerViewMessage> {
let task = (|| {
::iced::Task::none()
})();
task
}
pub(crate) fn __boot() -> (Self, ::iced::Task<__ExplorerViewMessage>) {
let mut state = Self::__state();
let task = state.__boot_task();
(state, task)
}
pub(crate) const __PREFERRED_WINDOW_SIZE: &'static str = "none";
#[allow(clippy::too_many_arguments)] fn __restore_state(active_palette: AppTheme, connected: bool, loading: bool, blocks: ::std::vec::Vec<crate::host::ExplorerBlock>, ops: ::std::vec::Vec<crate::host::ExplorerOp>, head: i64, sync_line: ::std::string::String, ledger_serial: i64, hits: ::std::vec::Vec<crate::host::ExplorerHit>, kinds: ::std::vec::Vec<crate::host::KindCount>, partial: ::std::string::String, searching: bool, sent_query: ::std::string::String, search_serial: i64, query: ::std::string::String, kind: ::std::string::String, selected: i64, viewport_width: f64, ledger_width: f64, host_error: ::std::string::String, sent: bool) -> Self {
Self {
__ice_accessibility: ::ui_lang_runtime::Bridge::new(),
#[cfg(all(target_os = "windows", not(test)))]
__ice_accessibility_initial: ::std::option::Option::None,
#[cfg(all(target_os = "windows", not(test)))]
__ice_accessibility_pending: ::std::vec::Vec::new(),
active_palette: active_palette,
connected: connected,
loading: loading,
blocks: blocks,
ops: ops,
head: head,
sync_line: sync_line,
ledger_serial: ledger_serial,
hits: hits,
kinds: kinds,
partial: partial,
searching: searching,
sent_query: sent_query,
search_serial: search_serial,
query: query,
kind: kind,
selected: selected,
viewport_width: viewport_width,
ledger_width: ledger_width,
host_error: host_error,
sent: sent,
__ice_rev: [::ui_lang_runtime::rev::seed(); 21],
}
}
pub(crate) const __SNAPSHOT_SCHEMA: &'static str = "2d39c39a6f939b7c1759e6f118706ed6fb7b0dee7d160d3f2bab3a3d45e380e9";
pub(crate) fn __snapshot(&self) -> ::std::result::Result<::std::vec::Vec<u8>, ::std::string::String> { ::ui_lang_guest::wire::Snapshot {schema: ::std::string::String::from(Self::__SNAPSHOT_SCHEMA), state: ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("ExplorerView"), fields: vec![(::std::string::String::from("active_palette"), match &self.active_palette { AppTheme::App => ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("AppTheme"), fields: vec![(::std::string::String::from("app"), ::ui_lang_guest::wire::SnapshotValue::Unit)] }, AppTheme::AppDark => ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("AppTheme"), fields: vec![(::std::string::String::from("app_dark"), ::ui_lang_guest::wire::SnapshotValue::Unit)] } }), (::std::string::String::from("connected"), ::ui_lang_guest::wire::SnapshotValue::Bool(*(&self.connected))), (::std::string::String::from("loading"), ::ui_lang_guest::wire::SnapshotValue::Bool(*(&self.loading))), (::std::string::String::from("blocks"), ::ui_lang_guest::wire::SnapshotValue::List((&self.blocks).iter().map(|__item| ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("ExplorerBlock"), fields: ::std::vec![(::std::string::String::from("height"), ::ui_lang_guest::wire::SnapshotValue::I64(*(&(__item).height))), (::std::string::String::from("hash"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).hash))), (::std::string::String::from("commit"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).commit))), (::std::string::String::from("op_count"), ::ui_lang_guest::wire::SnapshotValue::I64(*(&(__item).op_count)))] }).collect())), (::std::string::String::from("ops"), ::ui_lang_guest::wire::SnapshotValue::List((&self.ops).iter().map(|__item| ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("ExplorerOp"), fields: ::std::vec![(::std::string::String::from("height"), ::ui_lang_guest::wire::SnapshotValue::I64(*(&(__item).height))), (::std::string::String::from("proposer"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).proposer))), (::std::string::String::from("target"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).target))), (::std::string::String::from("disposition"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).disposition))), (::std::string::String::from("op_hash"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).op_hash))), (::std::string::String::from("payload"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).payload))), (::std::string::String::from("trace"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).trace)))] }).collect())), (::std::string::String::from("head"), ::ui_lang_guest::wire::SnapshotValue::I64(*(&self.head))), (::std::string::String::from("sync_line"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.sync_line))), (::std::string::String::from("ledger_serial"), ::ui_lang_guest::wire::SnapshotValue::I64(*(&self.ledger_serial))), (::std::string::String::from("hits"), ::ui_lang_guest::wire::SnapshotValue::List((&self.hits).iter().map(|__item| ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("ExplorerHit"), fields: ::std::vec![(::std::string::String::from("kind"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).kind))), (::std::string::String::from("code"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).code))), (::std::string::String::from("title"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).title))), (::std::string::String::from("snippet"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).snippet))), (::std::string::String::from("meta"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).meta))), (::std::string::String::from("target"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).target)))] }).collect())), (::std::string::String::from("kinds"), ::ui_lang_guest::wire::SnapshotValue::List((&self.kinds).iter().map(|__item| ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("KindCount"), fields: ::std::vec![(::std::string::String::from("kind"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).kind))), (::std::string::String::from("label"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).label))), (::std::string::String::from("count"), ::ui_lang_guest::wire::SnapshotValue::I64(*(&(__item).count)))] }).collect())), (::std::string::String::from("partial"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.partial))), (::std::string::String::from("searching"), ::ui_lang_guest::wire::SnapshotValue::Bool(*(&self.searching))), (::std::string::String::from("sent_query"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.sent_query))), (::std::string::String::from("search_serial"), ::ui_lang_guest::wire::SnapshotValue::I64(*(&self.search_serial))), (::std::string::String::from("query"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.query))), (::std::string::String::from("kind"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.kind))), (::std::string::String::from("selected"), ::ui_lang_guest::wire::SnapshotValue::I64(*(&self.selected))), (::std::string::String::from("viewport_width"), ::ui_lang_guest::wire::SnapshotValue::F64(*(&self.viewport_width))), (::std::string::String::from("ledger_width"), ::ui_lang_guest::wire::SnapshotValue::F64(*(&self.ledger_width))), (::std::string::String::from("host_error"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.host_error))), (::std::string::String::from("sent"), ::ui_lang_guest::wire::SnapshotValue::Bool(*(&self.sent)))] }}.encode() }
pub(crate) fn __restore(__bytes: &[u8]) -> ::std::result::Result<Self, ::std::string::String> { let __snapshot = ::ui_lang_guest::wire::Snapshot::decode(__bytes)?; if __snapshot.schema != Self::__SNAPSHOT_SCHEMA { return ::std::result::Result::Err(::std::string::String::from("snapshot schema mismatch")); } let __value = __snapshot.state; ((|| { let ::ui_lang_guest::wire::SnapshotValue::Record {name: __name, fields: __fields} = __value else { return ::std::option::Option::None; }; if __name != "ExplorerView" || __fields.len() != 21 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __value) = __fields.next()?; if __name != "active_palette" { return ::std::option::Option::None; } let active_palette: AppTheme = ((|| { let ::ui_lang_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __value else { return ::std::option::Option::None; }; if __name != "AppTheme" || __fields.len() != 1 { return ::std::option::Option::None; } let (__variant, __payload) = __fields.into_iter().next()?; match __variant.as_str() { "app" => matches!(__payload, ::ui_lang_guest::wire::SnapshotValue::Unit).then_some(AppTheme::App), "app_dark" => matches!(__payload, ::ui_lang_guest::wire::SnapshotValue::Unit).then_some(AppTheme::AppDark), _ => ::std::option::Option::None } })())?; let (__name, __value) = __fields.next()?; if __name != "connected" { return ::std::option::Option::None; } let connected: bool = (match __value { ::ui_lang_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "loading" { return ::std::option::Option::None; } let loading: bool = (match __value { ::ui_lang_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "blocks" { return ::std::option::Option::None; } let blocks: ::std::vec::Vec<crate::host::ExplorerBlock> = (match __value { ::ui_lang_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| (|| { let ::ui_lang_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __item else { return ::std::option::Option::None; }; if __name != "ExplorerBlock" || __fields.len() != 4 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "height" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "hash" { return ::std::option::Option::None; } let (__name, __field_2) = __fields.next()?; if __name != "commit" { return ::std::option::Option::None; } let (__name, __field_3) = __fields.next()?; if __name != "op_count" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::ExplorerBlock { height: (match __field_0 { ::ui_lang_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, hash: (match __field_1 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, commit: (match __field_2 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, op_count: (match __field_3 { ::ui_lang_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })()).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "ops" { return ::std::option::Option::None; } let ops: ::std::vec::Vec<crate::host::ExplorerOp> = (match __value { ::ui_lang_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| (|| { let ::ui_lang_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __item else { return ::std::option::Option::None; }; if __name != "ExplorerOp" || __fields.len() != 7 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "height" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "proposer" { return ::std::option::Option::None; } let (__name, __field_2) = __fields.next()?; if __name != "target" { return ::std::option::Option::None; } let (__name, __field_3) = __fields.next()?; if __name != "disposition" { return ::std::option::Option::None; } let (__name, __field_4) = __fields.next()?; if __name != "op_hash" { return ::std::option::Option::None; } let (__name, __field_5) = __fields.next()?; if __name != "payload" { return ::std::option::Option::None; } let (__name, __field_6) = __fields.next()?; if __name != "trace" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::ExplorerOp { height: (match __field_0 { ::ui_lang_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, proposer: (match __field_1 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, target: (match __field_2 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, disposition: (match __field_3 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, op_hash: (match __field_4 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, payload: (match __field_5 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, trace: (match __field_6 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })()).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "head" { return ::std::option::Option::None; } let head: i64 = (match __value { ::ui_lang_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "sync_line" { return ::std::option::Option::None; } let sync_line: ::std::string::String = (match __value { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "ledger_serial" { return ::std::option::Option::None; } let ledger_serial: i64 = (match __value { ::ui_lang_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "hits" { return ::std::option::Option::None; } let hits: ::std::vec::Vec<crate::host::ExplorerHit> = (match __value { ::ui_lang_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| (|| { let ::ui_lang_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __item else { return ::std::option::Option::None; }; if __name != "ExplorerHit" || __fields.len() != 6 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "kind" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "code" { return ::std::option::Option::None; } let (__name, __field_2) = __fields.next()?; if __name != "title" { return ::std::option::Option::None; } let (__name, __field_3) = __fields.next()?; if __name != "snippet" { return ::std::option::Option::None; } let (__name, __field_4) = __fields.next()?; if __name != "meta" { return ::std::option::Option::None; } let (__name, __field_5) = __fields.next()?; if __name != "target" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::ExplorerHit { kind: (match __field_0 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, code: (match __field_1 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, title: (match __field_2 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, snippet: (match __field_3 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, meta: (match __field_4 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, target: (match __field_5 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })()).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "kinds" { return ::std::option::Option::None; } let kinds: ::std::vec::Vec<crate::host::KindCount> = (match __value { ::ui_lang_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| (|| { let ::ui_lang_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __item else { return ::std::option::Option::None; }; if __name != "KindCount" || __fields.len() != 3 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "kind" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "label" { return ::std::option::Option::None; } let (__name, __field_2) = __fields.next()?; if __name != "count" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::KindCount { kind: (match __field_0 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, label: (match __field_1 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, count: (match __field_2 { ::ui_lang_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })()).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "partial" { return ::std::option::Option::None; } let partial: ::std::string::String = (match __value { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "searching" { return ::std::option::Option::None; } let searching: bool = (match __value { ::ui_lang_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "sent_query" { return ::std::option::Option::None; } let sent_query: ::std::string::String = (match __value { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "search_serial" { return ::std::option::Option::None; } let search_serial: i64 = (match __value { ::ui_lang_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "query" { return ::std::option::Option::None; } let query: ::std::string::String = (match __value { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "kind" { return ::std::option::Option::None; } let kind: ::std::string::String = (match __value { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "selected" { return ::std::option::Option::None; } let selected: i64 = (match __value { ::ui_lang_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "viewport_width" { return ::std::option::Option::None; } let viewport_width: f64 = (match __value { ::ui_lang_guest::wire::SnapshotValue::F64(__item) if __item.is_finite() => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "ledger_width" { return ::std::option::Option::None; } let ledger_width: f64 = (match __value { ::ui_lang_guest::wire::SnapshotValue::F64(__item) if __item.is_finite() => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "host_error" { return ::std::option::Option::None; } let host_error: ::std::string::String = (match __value { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "sent" { return ::std::option::Option::None; } let sent: bool = (match __value { ::ui_lang_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; ::std::option::Option::Some(Self::__restore_state(active_palette, connected, loading, blocks, ops, head, sync_line, ledger_serial, hits, kinds, partial, searching, sent_query, search_serial, query, kind, selected, viewport_width, ledger_width, host_error, sent)) })()).ok_or_else(|| ::std::string::String::from("snapshot state mismatch")) }
}
}
__ice_generated_items_4578706c6f72657256696577! {
#[allow(unused_parens)]
impl ExplorerView {
}
}
__ice_generated_items_4578706c6f72657256696577! {
#[allow(unused_parens)]
impl ExplorerView {
fn __subscription(&self) -> ::iced::Subscription<__ExplorerViewMessage> {
::iced::Subscription::batch([
crate::host::session().map(move |__value| __ExplorerViewMessage::SessionArrived(__value)),
if self.connected { ::iced::Subscription::batch([crate::host::ledger(self.ledger_serial).map(move |__value| __ExplorerViewMessage::LedgerArrived(__value)),
]) } else { ::iced::Subscription::none() },
if (self.connected && (!(self.sent_query).is_empty())) { ::iced::Subscription::batch([crate::host::workspace_search(self.sent_query.to_owned(), self.search_serial).map(move |__value| __ExplorerViewMessage::SearchArrived(__value)),
]) } else { ::iced::Subscription::none() },
])
}
}
}
__ice_generated_items_4578706c6f72657256696577! {
#[allow(unused_parens)]
impl ExplorerView {
}
#[cfg(test)] mod __ice_tests { use super::*;
#[test]
fn __ice_view_fits_default_stack() {
::std::thread::Builder::new().stack_size(4 * 1024 * 1024).spawn(|| {
let (__app, _) = ExplorerView::__boot();
let _ = __app.__view();
}).unwrap().join().unwrap();
}
}
}
#[allow(warnings, clippy::all)]
mod __ice_group_app_8557f58d {
use super::*;
impl super::ExplorerView {
pub(super) fn __ice_component_use_11(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_0: crate::host::ExplorerBlock) -> __IceElement<'_, __ExplorerViewMessage> { let __component_content: __IceElement<'_, __ExplorerViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 697, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 703, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:703", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (__ice_arg_0.height).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 713, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::WordOrGlyph), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:713", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: (crate::host::hex(::std::convert::AsRef::as_ref(&(__ice_arg_0.hash)))).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 725, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:725", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (crate::host::plural(__ice_arg_0.op_count, ::std::convert::AsRef::as_ref(&("op")), ::std::convert::AsRef::as_ref(&("ops")))).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_12(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_0: crate::host::ExplorerBlock, __ice_arg_1: bool) -> __IceElement<'_, __ExplorerViewMessage> { let __component_content: __IceElement<'_, __ExplorerViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 669, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); if __ice_arg_1 { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 671, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::Some(__ice_arg_1), expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:671", __ice_use_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 678, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_11(__ice_palette, format!("{}/ExplorerBlockFace@1702", __ice_use_scope), __ice_arg_0.clone()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Inspect block".to_owned())), on_press: ::std::option::Option::Some(::ui_lang_guest::slots::message((move |__event_0| __ExplorerViewMessage::SelectExplorerBlock(__event_0))(__ice_arg_0.height))), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((6.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([8.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[91]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[57]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), pressed: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::None, border: ::std::option::Option::None }), disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!__ice_arg_1) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 683, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::Some(__ice_arg_1), expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:683", __ice_use_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 690, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_11(__ice_palette, format!("{}/ExplorerBlockFace@1714", __ice_use_scope), __ice_arg_0.clone()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Inspect block".to_owned())), on_press: ::std::option::Option::Some(::ui_lang_guest::slots::message((move |__event_0| __ExplorerViewMessage::SelectExplorerBlock(__event_0))(__ice_arg_0.height))), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((6.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([8.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[57]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), pressed: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::None, border: ::std::option::Option::None }), disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_14(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_1: ::std::string::String) -> __IceElement<'_, __ExplorerViewMessage> { let __component_content: __IceElement<'_, __ExplorerViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 640, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 645, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:645", __ice_use_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("block".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 651, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:651", __ice_use_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 656, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::WordOrGlyph), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:656", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (crate::host::hex(::std::convert::AsRef::as_ref(&(__ice_arg_1)))).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Copy block hash".to_owned())), on_press: ::std::option::Option::Some(::ui_lang_guest::slots::message((move |__event_0, __event_1| __ExplorerViewMessage::CopyToClipboard(__event_0, __event_1))(__ice_arg_1.to_owned(), "Block hash copied".to_owned()))), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((2.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([8.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX)]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[57]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), pressed: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::None, border: ::std::option::Option::None }), disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_15(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_1: ::std::string::String) -> __IceElement<'_, __ExplorerViewMessage> { let __component_content: __IceElement<'_, __ExplorerViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 640, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 645, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:645", __ice_use_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("commit".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 651, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:651", __ice_use_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 656, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::WordOrGlyph), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:656", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (crate::host::hex(::std::convert::AsRef::as_ref(&(__ice_arg_1)))).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Copy commit hash".to_owned())), on_press: ::std::option::Option::Some(::ui_lang_guest::slots::message((move |__event_0, __event_1| __ExplorerViewMessage::CopyToClipboard(__event_0, __event_1))(__ice_arg_1.to_owned(), "Commit hash copied".to_owned()))), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((2.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([8.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX)]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[57]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), pressed: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::None, border: ::std::option::Option::None }), disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_21(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_1: ::std::string::String) -> __IceElement<'_, __ExplorerViewMessage> { let __component_content: __IceElement<'_, __ExplorerViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 640, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 645, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:645", __ice_use_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("hash".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 651, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:651", __ice_use_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 656, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::WordOrGlyph), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:656", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (crate::host::hex(::std::convert::AsRef::as_ref(&(__ice_arg_1)))).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Copy op hash".to_owned())), on_press: ::std::option::Option::Some(::ui_lang_guest::slots::message((move |__event_0, __event_1| __ExplorerViewMessage::CopyToClipboard(__event_0, __event_1))(__ice_arg_1.to_owned(), "Op hash copied".to_owned()))), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((2.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([8.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX)]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[57]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), pressed: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::None, border: ::std::option::Option::None }), disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_22(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_1: ::std::string::String) -> __IceElement<'_, __ExplorerViewMessage> { let __component_content: __IceElement<'_, __ExplorerViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 640, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 645, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:645", __ice_use_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("by".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 651, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:651", __ice_use_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 656, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::WordOrGlyph), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:656", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (crate::host::hex(::std::convert::AsRef::as_ref(&(__ice_arg_1)))).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Copy proposer".to_owned())), on_press: ::std::option::Option::Some(::ui_lang_guest::slots::message((move |__event_0, __event_1| __ExplorerViewMessage::CopyToClipboard(__event_0, __event_1))(__ice_arg_1.to_owned(), "Proposer copied".to_owned()))), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((2.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([8.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX)]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[57]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), pressed: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::None, border: ::std::option::Option::None }), disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
}
}

#[allow(warnings, clippy::all)]
mod __ice_group_app_update {
use super::*;
impl super::ExplorerView {
#[allow(clippy::assign_op_pattern)]
pub(super) fn __update(&mut self, message: __ExplorerViewMessage) -> ::iced::Task<__ExplorerViewMessage> {
#[cfg(all(target_os = "windows", not(test)))]
if !self.__ice_accessibility.is_attached() && !matches!(&message, __ExplorerViewMessage::__AccessibilityNativeWindow(_)) {
self.__ice_accessibility_pending.push(message);
return ::iced::Task::none();
}
let __task = match message {
__ExplorerViewMessage::__AccessibilitySnapshot(__snapshot) => { self.__ice_accessibility.update(*__snapshot); return ::iced::Task::none(); },
__ExplorerViewMessage::__AccessibilityAction(__request) => { let __refresh = matches!(__request.action, ::ui_lang_runtime::Action::Focus); let __task = self.__ice_accessibility.dispatch(__request); return if __refresh { __task.chain(::ui_lang_runtime::snapshot::<__ExplorerViewMessage>("ExplorerView").map(|__snapshot| __ExplorerViewMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)))) } else { __task }; },
__ExplorerViewMessage::__AccessibilityWindow(__id, __event) => { self.__ice_accessibility.window_event(__id, __event); return ::iced::Task::none(); },
#[cfg(all(target_os = "windows", not(test)))]
__ExplorerViewMessage::__AccessibilityNativeWindow(__window) => {
let __id = __window.id();
if !self.__ice_accessibility.attach_window(__window) { return ::iced::Task::none(); }
let __restore = ::iced::window::set_mode(__id, ::iced::window::Mode::Windowed);
let __initial = self.__accessibility_initial_task();
let mut __pending = ::std::vec::Vec::new();
for __message in ::std::mem::take(&mut self.__ice_accessibility_pending) {
__pending.push(self.__update(__message));
}
let __pending = ::iced::Task::batch(__pending);
let __snapshot = ::ui_lang_runtime::snapshot::<__ExplorerViewMessage>("ExplorerView").map(|__snapshot| __ExplorerViewMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)));
return __restore.chain(::iced::Task::batch([__initial, __pending, __snapshot]));
},
#[cfg(all(target_os = "macos", not(test)))]
__ExplorerViewMessage::__AccessibilityNativeWindow(__window) => {
if !self.__ice_accessibility.attach_window(__window) { return ::iced::Task::none(); }
return ::ui_lang_runtime::snapshot::<__ExplorerViewMessage>("ExplorerView").map(|__snapshot| __ExplorerViewMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)));
},
__ExplorerViewMessage::__AccessibilityFocusNext(__window) => { return ::ui_lang_runtime::focus_next_in::<__ExplorerViewMessage>(__window).chain(::ui_lang_runtime::snapshot::<__ExplorerViewMessage>("ExplorerView").map(|__snapshot| __ExplorerViewMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)))); },
__ExplorerViewMessage::__AccessibilityFocusPrevious(__window) => { return ::ui_lang_runtime::focus_previous_in::<__ExplorerViewMessage>(__window).chain(::ui_lang_runtime::snapshot::<__ExplorerViewMessage>("ExplorerView").map(|__snapshot| __ExplorerViewMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)))); },
__ExplorerViewMessage::__TemplateChanged => { return ::iced::Task::none(); },
__ExplorerViewMessage::SessionArrived(item) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("session_arrived", "src/ui/app.ice:84");
let _ = &item;
{ let __ice_next = item.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[19] += 1; } }
if (!(item.error).is_empty()) { return ::iced::Task::none(); }
let next = item.next.clone();
{ let __ice_next = next.head; if ::ui_lang_runtime::state_changed!(self.head, __ice_next) { self.head = __ice_next; self.__ice_rev[5] += 1; } }
{ let __ice_next = next.sync_line.to_owned(); if ::ui_lang_runtime::state_changed!(self.sync_line, __ice_next) { self.sync_line = __ice_next; self.__ice_rev[6] += 1; } }
{ let __ice_next = crate::host::connection_serial_after(self.connected, next.connected, self.ledger_serial); if ::ui_lang_runtime::state_changed!(self.ledger_serial, __ice_next) { self.ledger_serial = __ice_next; self.__ice_rev[7] += 1; } }
{ let __ice_next = crate::host::loading_after(self.connected, next.connected, self.loading); if ::ui_lang_runtime::state_changed!(self.loading, __ice_next) { self.loading = __ice_next; self.__ice_rev[2] += 1; } }
{ let __ice_next = next.connected; if ::ui_lang_runtime::state_changed!(self.connected, __ice_next) { self.connected = __ice_next; self.__ice_rev[1] += 1; } }
{ let __ice_next = AppTheme::App; if ::ui_lang_runtime::state_changed!(self.active_palette, __ice_next) { self.active_palette = __ice_next; self.__ice_rev[0] += 1; } }
if (!next.dark) { return ::iced::Task::none(); }
{ let __ice_next = AppTheme::AppDark; if ::ui_lang_runtime::state_changed!(self.active_palette, __ice_next) { self.active_palette = __ice_next; self.__ice_rev[0] += 1; } }
::iced::Task::none()
})(),
__ExplorerViewMessage::LedgerArrived(item) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("ledger_arrived", "src/ui/app.ice:97");
let _ = &item;
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.loading, __ice_next) { self.loading = __ice_next; self.__ice_rev[2] += 1; } }
{ let __ice_next = item.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[19] += 1; } }
if (!(item.error).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = item.blocks.clone(); if ::ui_lang_runtime::state_changed!(self.blocks, __ice_next) { self.blocks = __ice_next; self.__ice_rev[3] += 1; } }
{ let __ice_next = item.ops.clone(); if ::ui_lang_runtime::state_changed!(self.ops, __ice_next) { self.ops = __ice_next; self.__ice_rev[4] += 1; } }
::iced::Task::none()
})(),
__ExplorerViewMessage::SearchArrived(item) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("search_arrived", "src/ui/app.ice:104");
let _ = &item;
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.searching, __ice_next) { self.searching = __ice_next; self.__ice_rev[11] += 1; } }
{ let __ice_next = item.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[19] += 1; } }
if (!(item.error).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = item.hits.clone(); if ::ui_lang_runtime::state_changed!(self.hits, __ice_next) { self.hits = __ice_next; self.__ice_rev[8] += 1; } }
{ let __ice_next = item.kinds.clone(); if ::ui_lang_runtime::state_changed!(self.kinds, __ice_next) { self.kinds = __ice_next; self.__ice_rev[9] += 1; } }
{ let __ice_next = item.partial.to_owned(); if ::ui_lang_runtime::state_changed!(self.partial, __ice_next) { self.partial = __ice_next; self.__ice_rev[10] += 1; } }
::iced::Task::none()
})(),
__ExplorerViewMessage::Refresh => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("refresh", "src/ui/app.ice:112");
if ((!self.connected) || self.loading) { return ::iced::Task::none(); }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.loading, __ice_next) { self.loading = __ice_next; self.__ice_rev[2] += 1; } }
{ let __ice_next = (self.ledger_serial + 1); if ::ui_lang_runtime::state_changed!(self.ledger_serial, __ice_next) { self.ledger_serial = __ice_next; self.__ice_rev[7] += 1; } }
::iced::Task::none()
})(),
__ExplorerViewMessage::CopyToClipboard(text, label) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("copy_to_clipboard", "src/ui/app.ice:117");
let _ = &text;
let _ = &label;
{ let __ice_next = crate::host::copy(::std::convert::AsRef::as_ref(&(text)), ::std::convert::AsRef::as_ref(&(label))); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[20] += 1; } }
::iced::Task::none()
})(),
__ExplorerViewMessage::SearchSubmit => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("search_submit", "src/ui/app.ice:124");
let blocked = (((!self.connected) || self.searching) || ((self.query).trim().to_owned()).is_empty());
if blocked { return ::iced::Task::none(); }
{ let __ice_next = "all".to_owned(); if ::ui_lang_runtime::state_changed!(self.kind, __ice_next) { self.kind = __ice_next; self.__ice_rev[15] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.hits, __ice_next) { self.hits = __ice_next; self.__ice_rev[8] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.kinds, __ice_next) { self.kinds = __ice_next; self.__ice_rev[9] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.partial, __ice_next) { self.partial = __ice_next; self.__ice_rev[10] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.searching, __ice_next) { self.searching = __ice_next; self.__ice_rev[11] += 1; } }
{ let __ice_next = (self.search_serial + 1); if ::ui_lang_runtime::state_changed!(self.search_serial, __ice_next) { self.search_serial = __ice_next; self.__ice_rev[13] += 1; } }
{ let __ice_next = (self.query).trim().to_owned(); if ::ui_lang_runtime::state_changed!(self.sent_query, __ice_next) { self.sent_query = __ice_next; self.__ice_rev[12] += 1; } }
::iced::Task::none()
})(),
__ExplorerViewMessage::ClearExplorerSearch => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("clear_explorer_search", "src/ui/app.ice:135");
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.query, __ice_next) { self.query = __ice_next; self.__ice_rev[14] += 1; } }
{ let __ice_next = "all".to_owned(); if ::ui_lang_runtime::state_changed!(self.kind, __ice_next) { self.kind = __ice_next; self.__ice_rev[15] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.hits, __ice_next) { self.hits = __ice_next; self.__ice_rev[8] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.kinds, __ice_next) { self.kinds = __ice_next; self.__ice_rev[9] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.partial, __ice_next) { self.partial = __ice_next; self.__ice_rev[10] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.searching, __ice_next) { self.searching = __ice_next; self.__ice_rev[11] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.sent_query, __ice_next) { self.sent_query = __ice_next; self.__ice_rev[12] += 1; } }
::iced::Task::none()
})(),
__ExplorerViewMessage::PickExplorerKind(next) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("pick_explorer_kind", "src/ui/app.ice:144");
let _ = &next;
{ let __ice_next = next.to_owned(); if ::ui_lang_runtime::state_changed!(self.kind, __ice_next) { self.kind = __ice_next; self.__ice_rev[15] += 1; } }
::iced::Task::none()
})(),
__ExplorerViewMessage::SelectExplorerBlock(height) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("select_explorer_block", "src/ui/app.ice:147");
let _ = &height;
{ let __ice_next = height; if ::ui_lang_runtime::state_changed!(self.selected, __ice_next) { self.selected = __ice_next; self.__ice_rev[16] += 1; } }
::iced::Task::none()
})(),
__ExplorerViewMessage::LedgerResized(dx, _dy) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("ledger_resized", "src/ui/app.ice:150");
let _ = &dx;
let _ = &_dy;
{ let __ice_next = crate::host::ledger_width_after_delta(self.ledger_width, dx, self.viewport_width); if ::ui_lang_runtime::state_changed!(self.ledger_width, __ice_next) { self.ledger_width = __ice_next; self.__ice_rev[18] += 1; } }
::iced::Task::none()
})(),
__ExplorerViewMessage::ViewportChanged(width, _height) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("viewport_changed", "src/ui/app.ice:153");
let _ = &width;
let _ = &_height;
{ let __ice_next = width; if ::ui_lang_runtime::state_changed!(self.viewport_width, __ice_next) { self.viewport_width = __ice_next; self.__ice_rev[17] += 1; } }
{ let __ice_next = crate::host::ledger_width_after_delta(self.ledger_width, 0.0, width); if ::ui_lang_runtime::state_changed!(self.ledger_width, __ice_next) { self.ledger_width = __ice_next; self.__ice_rev[18] += 1; } }
::iced::Task::none()
})(),
__ExplorerViewMessage::__BindQuery(value) => { { let __ice_next = value; if ::ui_lang_runtime::state_changed!(self.query, __ice_next) { self.query = __ice_next; self.__ice_rev[14] += 1; } } ::iced::Task::none() }
};
// Snapshotting the widget tree after every message serves ONLY an attached
// assistive technology (and the test harness, which drives the app through
// this tree) — ungated it walked every widget, built a TreeUpdate nobody
// read, and scheduled a second frame per message.
let __accessibility = if cfg!(test) || ::ui_lang_runtime::accessibility_active() {
::ui_lang_runtime::snapshot::<__ExplorerViewMessage>("ExplorerView").map(|__snapshot| __ExplorerViewMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)))
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
impl super::ExplorerView {
pub(super) fn __view(&self) -> __IceElement<'_, __ExplorerViewMessage> { let __ice_view = ::ui_lang_runtime::dev::Span::view("ExplorerView", "src/ui/app.ice:158"); let __ice_palette = self.__palette(); let __ice_app_theme = Self::__app_theme(__ice_palette); let __ice_content: __IceElement<'_, __ExplorerViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 158, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/root", "ExplorerView"); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[2]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 163, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 164, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Sensor { key: format!("{}/@sensor:164", __ice_node_scope), reset: ::std::option::Option::None, on_show: ::std::option::Option::Some(::ui_lang_guest::slots::handler::<(f32, f32), __ExplorerViewMessage>(::std::boxed::Box::new({ let __route = {  move |__size: (f64, f64)| __ExplorerViewMessage::ViewportChanged(__size.0, __size.1) }; move |__sent: (f32, f32)| ::std::option::Option::Some(__route((f64::from(__sent.0), f64::from(__sent.1)))) }))), on_resize: ::std::option::Option::Some(::ui_lang_guest::slots::handler::<(f32, f32), __ExplorerViewMessage>(::std::boxed::Box::new({ let __route = {  move |__size: (f64, f64)| __ExplorerViewMessage::ViewportChanged(__size.0, __size.1) }; move |__sent: (f32, f32)| ::std::option::Option::Some(__route((f64::from(__sent.0), f64::from(__sent.1)))) }))), on_hide: ::std::option::Option::None, anticipate: ::std::option::Option::None, delay: ::std::option::Option::None, child: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 165, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((0.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 166, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 184, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_0(__ice_palette, format!("{}/ScreenTitle@1208", __ice_node_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 196, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 201, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:201", __ice_node_scope), size: ::std::option::Option::Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (crate::host::height_label(self.head)).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!(self.sync_line).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 203, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:203", __ice_node_scope), size: ::std::option::Option::Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (self.sync_line.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:196", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((10.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!(self.host_error).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 209, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/host-error", __ice_node_scope); ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: __ice_node_scope.clone(), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[20]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (self.host_error.to_owned()).to_string() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 211, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::Some(((860.0) as f32).max(0.0).min(f32::MAX)), max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:211", __ice_node_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 212, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 217, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:217", __ice_node_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (2.0) as f32, right: (14.0) as f32, bottom: (2.0) as f32, left: (14.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.5) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((11.0) as f32).max(0.0).min(f32::MAX), ((11.0) as f32).max(0.0).min(f32::MAX), ((11.0) as f32).max(0.0).min(f32::MAX), ((11.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 228, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 233, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_1(__ice_palette, format!("{}/Icon@1257", __ice_node_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 238, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/explorer-search", __ice_node_scope); ::ui_lang_guest::wire::Node::Input { options: ::ui_lang_guest::wire::InputOptions { label: ("Search this workspace".to_owned()).to_string(), description: ::std::option::Option::None, disabled: ((!self.connected) || self.searching), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((6.2) as f32)), text_size: ::std::option::Option::Some((13.0) as f32), line_height: ::std::option::Option::Some((1.2) as f32), align: ::std::option::Option::None, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: __ice_node_scope.clone(), placeholder: ::std::string::String::from("Search messages, pages, issues, files, runs…".to_owned()), value: (self.query).to_string(), on_input: ::ui_lang_guest::slots::handler::<::std::string::String, __ExplorerViewMessage>(::std::boxed::Box::new({ let __route = __ExplorerViewMessage::__BindQuery as fn(::std::string::String) -> __ExplorerViewMessage; move |__sent: ::std::string::String| ::std::option::Option::Some(__route(__sent)) })), on_submit: ::std::option::Option::Some(::ui_lang_guest::slots::message(__ExplorerViewMessage::SearchSubmit)), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), secure: (false), style: ::std::boxed::Box::new(::ui_lang_guest::wire::InputStyle { utility: ::ui_lang_guest::wire::InputFace { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(1f32), radius: ::std::option::Option::Some([10f32; 4]) }), ..Default::default() }, focus_border: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), focused_hovered: ::std::option::Option::None, active: ::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((0.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((0.0) as f32).max(0.0).min(f32::MAX), ((0.0) as f32).max(0.0).min(f32::MAX), ((0.0) as f32).max(0.0).min(f32::MAX), ((0.0) as f32).max(0.0).min(f32::MAX)]) }), value: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), placeholder: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[72]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), selection: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.180000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::None, radius: ::std::option::Option::None }), value: ::std::option::Option::None, placeholder: ::std::option::Option::None, selection: ::std::option::Option::None }), focused: ::std::option::Option::None, disabled: ::std::option::Option::Some(::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, value: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), placeholder: ::std::option::Option::None, selection: ::std::option::Option::None }) }) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!((self.query).trim().to_owned()).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 253, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/explorer-clear", __ice_node_scope); ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: __ice_node_scope.clone(), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 260, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:260", __ice_node_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 266, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:266", __ice_node_scope), size: ::std::option::Option::Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("×".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Clear workspace search".to_owned())), on_press: ::std::option::Option::Some(::ui_lang_guest::slots::message(__ExplorerViewMessage::ClearExplorerSearch)), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((22.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((22.0) as f32)), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((0.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([7.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(13.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX)]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), pressed: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[56]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), disabled: ::std::option::Option::None } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:228", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((10.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if self.searching { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 275, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:275", __ice_node_scope), size: ::std::option::Option::Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[70]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Searching…".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if self.loading { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 281, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:281", __ice_node_scope), size: ::std::option::Option::Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[70]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Loading…".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 286, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:286", __ice_node_scope), content: ::ui_lang_guest::wire::ButtonContent::Label(::std::string::String::from("Refresh")), label: ::std::option::Option::Some(::std::string::String::from("Refresh".to_owned())), on_press: if (self.loading) { ::std::option::Option::None } else { ::std::option::Option::Some(::ui_lang_guest::slots::message(__ExplorerViewMessage::Refresh)) }, width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((7.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[15]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(1.0), radius: ::std::option::Option::Some([8.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[6]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face::default(), hovered: ::std::option::Option::None, pressed: ::std::option::Option::None, disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:212", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((10.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (self.connected && (!(self.partial).is_empty())) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 303, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::Some(((860.0) as f32).max(0.0).min(f32::MAX)), max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:303", __ice_node_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 304, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/explorer-partial", __ice_node_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (8.0) as f32, right: (12.0) as f32, bottom: (8.0) as f32, left: (12.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[32]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[33]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 315, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:315", __ice_node_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[30]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: (self.partial.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (self.connected && (!(self.kinds).is_empty())) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 330, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::Some(((860.0) as f32).max(0.0).min(f32::MAX)), max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:330", __ice_node_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 331, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __items = ::std::vec::Vec::new(); let __flex_child: __IceElement<'_, __ExplorerViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 338, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::Some((self.kind == "all")), expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:338", __ice_node_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 344, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_2(__ice_palette, format!("{}/FilterChip@1368", __ice_node_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Show every result".to_owned())), on_press: ::std::option::Option::Some(::ui_lang_guest::slots::message(__ExplorerViewMessage::PickExplorerKind("all".to_owned()))), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((0.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([8.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX)]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[57]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), pressed: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __items.push((::ui_lang_guest::wire::FlexItem::default(), __flex_child)); for (__ice_index, kind_count) in self.kinds.iter().enumerate() { let __for_scope = format!("{}/@for:1376({})", __ice_node_scope, __ice_index); let __flex_child: __IceElement<'_, __ExplorerViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 353, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::Some((self.kind == kind_count.kind)), expanded: ::std::option::Option::None, description: ::std::option::Option::Some(::std::string::String::from(kind_count.label.to_owned())), key: format!("{}/@button:353", __for_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 360, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_3(__ice_palette, format!("{}/FilterChip@1384", __for_scope), kind_count.label.to_owned(), kind_count.count, (self.kind == kind_count.kind)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Filter results by kind".to_owned())), on_press: ::std::option::Option::Some(::ui_lang_guest::slots::message(__ExplorerViewMessage::PickExplorerKind(kind_count.kind.to_owned()))), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((0.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([8.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX)]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[57]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), pressed: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None }), disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __items.push((::ui_lang_guest::wire::FlexItem::default(), __flex_child)); } let (items, children) = __items.into_iter().unzip(); ::ui_lang_guest::wire::Node::Flex { key: format!("{}/@layout:331", __ice_node_scope), items, children, background: ::std::option::Option::None, border: ::std::option::Option::None, layout: ::ui_lang_guest::wire::FlexLayout { direction: ::ui_lang_guest::wire::FlexDirection::Row, wrap: ::ui_lang_guest::wire::FlexWrap::Wrap, justify: ::std::option::Option::None, items: ::std::option::Option::Some(::ui_lang_guest::wire::FlexItemAlignment::Start), content: ::std::option::Option::None, row_gap: ::std::option::Option::Some((7.0) as f32), column_gap: ::std::option::Option::Some((7.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: (false), surface_width: ::std::option::Option::None, surface_height: ::std::option::Option::None, surface_max_width: ::std::option::Option::None } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:166", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((16.0) as f32), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (22.0) as f32, right: (24.0) as f32, bottom: (0.0) as f32, left: (24.0) as f32 }), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 370, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); if (self.connected && (!(self.hits).is_empty())) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 384, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Scroll { on_scroll: ::std::option::Option::None, virtual_rows: false, key: format!("{}/@layout:384", __ice_node_scope), direction: ::ui_lang_guest::wire::ScrollDirection::Vertical, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), bar_hidden: false, bar_width: ::std::option::Option::None, bar_margin: ::std::option::Option::None, scroller_width: ::std::option::Option::None, bar_spacing: ::std::option::Option::None, anchor_x: ::ui_lang_guest::wire::ScrollAnchor::Start, anchor_y: ::ui_lang_guest::wire::ScrollAnchor::Start, auto_scroll: (false), background: ::std::option::Option::None, border: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 389, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::Some(((860.0) as f32).max(0.0).min(f32::MAX)), max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:389", __ice_node_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 390, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); for (__ice_index, hit) in self.hits.iter().enumerate() { let __for_scope = format!("{}/@for:1415({})", __ice_node_scope, __ice_index); if ((self.kind == "all") || (hit.kind == self.kind)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 393, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_6(__ice_palette, format!("{}/ExplorerCard@1417", __for_scope), hit.clone()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:390", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (self.connected && (!(self.hits).is_empty())) { for (__ice_index, kind_count) in self.kinds.iter().enumerate() { let __for_scope = format!("{}/@for:1423({})", __ice_node_scope, __ice_index); if ((self.kind == kind_count.kind) && (kind_count.count <= 0)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 401, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_7(__ice_palette, format!("{}/EmptyPlate@1425", __for_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } } } if (((self.connected && (self.hits).is_empty()) && (self.partial).is_empty()) && crate::host::search_answer_stands(::std::convert::AsRef::as_ref(&(self.sent_query)), ::std::convert::AsRef::as_ref(&(self.query)), self.searching)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 413, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/explorer-nothing-matched", __ice_node_scope); ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_8(__ice_palette, __ice_node_scope.clone())) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!self.connected) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 418, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_9(__ice_palette, format!("{}/EmptyState@1442", __ice_node_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (((((self.connected && (self.hits).is_empty()) && (self.blocks).is_empty()) && (!self.loading)) && ((self.query).trim().to_owned()).is_empty()) && (self.host_error).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 431, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_10(__ice_palette, format!("{}/EmptyState@1455", __ice_node_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((self.connected && (self.hits).is_empty()) && (!(self.blocks).is_empty())) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 438, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 443, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/ledger-pane", __ice_node_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((self.ledger_width) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (6.0) as f32, right: (6.0) as f32, bottom: (6.0) as f32, left: (6.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[6]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.100000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((10.0) as f32).max(0.0).min(f32::MAX), ((10.0) as f32).max(0.0).min(f32::MAX), ((10.0) as f32).max(0.0).min(f32::MAX), ((10.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 452, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Scroll { on_scroll: ::std::option::Option::None, virtual_rows: false, key: format!("{}/@layout:452", __ice_node_scope), direction: ::ui_lang_guest::wire::ScrollDirection::Vertical, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), bar_hidden: false, bar_width: ::std::option::Option::None, bar_margin: ::std::option::Option::None, scroller_width: ::std::option::Option::None, bar_spacing: ::std::option::Option::None, anchor_x: ::ui_lang_guest::wire::ScrollAnchor::Start, anchor_y: ::ui_lang_guest::wire::ScrollAnchor::Start, auto_scroll: (false), background: ::std::option::Option::None, border: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 460, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); for (__ice_index, block) in self.blocks.iter().enumerate() { let __for_scope = format!("{}/@for:1489({})", __ice_node_scope, __ice_index); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 466, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_12(__ice_palette, format!("{}/ExplorerBlockRow@1490", __for_scope), block.clone(), (block.height == self.selected)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:460", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((1.0) as f32), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (0.0) as f32, right: (10.0) as f32, bottom: (0.0) as f32, left: (0.0) as f32 }), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 469, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/ledger-resize", __ice_node_scope); ::ui_lang_guest::wire::Node::ResizeHandle { key: __ice_node_scope.clone(), on_press: ::std::option::Option::None, on_release: ::std::option::Option::None, on_drag: ::std::option::Option::Some(::ui_lang_guest::slots::handler::<(f64, f64), __ExplorerViewMessage>(::std::boxed::Box::new({ let __route = {  move |__delta: (f64, f64)| __ExplorerViewMessage::LedgerResized(__delta.0, __delta.1) }; move |__sent: (f64, f64)| ::std::option::Option::Some(__route(__sent)) }))), cursor: ::std::option::Option::Some(::ui_lang_guest::wire::mouse::Cursor::ResizingHorizontally), content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 470, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/ledger-divider", __ice_node_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((10.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 471, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 472, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:472", __ice_node_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (8.0) as f32, right: (8.0) as f32, bottom: (8.0) as f32, left: (8.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[6]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.100000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((10.0) as f32).max(0.0).min(f32::MAX), ((10.0) as f32).max(0.0).min(f32::MAX), ((10.0) as f32).max(0.0).min(f32::MAX), ((10.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 481, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __children: Vec<__IceElement<'_, __ExplorerViewMessage>> = Vec::new(); if (self.selected <= 0) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 483, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_13(__ice_palette, format!("{}/EmptyState@1507", __ice_node_scope)));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (self.selected > 0) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 488, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Scroll { on_scroll: ::std::option::Option::None, virtual_rows: false, key: format!("{}/@layout:488", __ice_node_scope), direction: ::ui_lang_guest::wire::ScrollDirection::Vertical, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), bar_hidden: false, bar_width: ::std::option::Option::None, bar_margin: ::std::option::Option::None, scroller_width: ::std::option::Option::None, bar_spacing: ::std::option::Option::None, anchor_x: ::ui_lang_guest::wire::ScrollAnchor::Start, anchor_y: ::ui_lang_guest::wire::ScrollAnchor::Start, auto_scroll: (false), background: ::std::option::Option::None, border: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 493, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); for (__ice_index, block) in self.blocks.iter().enumerate() { let __for_scope = format!("{}/@for:1523({})", __ice_node_scope, __ice_index); if (block.height == self.selected) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 501, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:501", __for_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (8.0) as f32, right: (8.0) as f32, bottom: (8.0) as f32, left: (8.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.100000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 509, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 510, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_14(__ice_palette, format!("{}/DigestRow@1534", __for_scope), block.hash.to_owned()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 518, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_15(__ice_palette, format!("{}/DigestRow@1542", __for_scope), block.commit.to_owned()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:509", __for_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((3.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } } for (__ice_index, op) in crate::host::explorer_ops_at(::std::convert::AsRef::as_ref(&(self.ops)), self.selected).iter().enumerate() { let __for_scope = format!("{}/@for:1550({})", __ice_node_scope, __ice_index); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 527, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:527", __for_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (8.0) as f32, right: (8.0) as f32, bottom: (8.0) as f32, left: (8.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.100000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 535, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 536, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 541, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:541", __for_scope), size: ::std::option::Option::Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (op.target.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 547, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_20(__ice_palette, format!("{}/StatusBadge@1571", __for_scope), op.disposition.to_owned()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 548, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:536", __for_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 576, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_21(__ice_palette, format!("{}/DigestRow@1600", __for_scope), op.op_hash.to_owned()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 584, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_22(__ice_palette, format!("{}/DigestRow@1608", __for_scope), op.proposer.to_owned()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!(op.trace).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 599, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 604, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:604", __for_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("dispatch".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 610, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:610", __for_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: (op.trace.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:599", __for_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Left), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 620, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::WordOrGlyph), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:620", __for_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: (op.payload.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:535", __for_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((3.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:493", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((6.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Stack { key: format!("{}/@layout:481", __ice_node_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, clip: false, under: 0u32, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:438", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((0.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:370", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((11.0) as f32), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (18.0) as f32, right: (24.0) as f32, bottom: (18.0) as f32, left: (24.0) as f32 }), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:163", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __ice_root: __IceElement<'_, __ExplorerViewMessage> = __ice_content; __ice_root }

}
}

#[allow(warnings, clippy::all)]
mod __ice_group_icon_8f6cad39 {
use super::*;
impl super::ExplorerView {
pub(super) fn __ice_component_use_1(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __ExplorerViewMessage> { let __component_content: __IceElement<'_, __ExplorerViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("/home/eddy/dev/ducktape/ducktape/.codex/worktrees/gpui-kit-migration/app/src/ui/components/icon.ice", 13, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); if ("label" == "ink") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("/home/eddy/dev/ducktape/ducktape/.codex/worktrees/gpui-kit-migration/app/src/ui/components/icon.ice", 16, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let (__hash, __bytes) = ::ui_lang_guest::slots::picture(crate::host::icon(::std::convert::AsRef::as_ref(&("search")))); ::ui_lang_guest::wire::Node::Svg { inherit_button_ink: false, key: format!("{}/@media:16", __ice_use_scope), hash: __hash, bytes: __bytes, label: ::std::option::Option::None, color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), hover: ::std::option::Option::None, fit: ::std::option::Option::None, rotation: ::std::option::Option::None, opacity: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((16.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((16.0) as f32)) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!("label" == "ink")) && ("label" == "label")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("/home/eddy/dev/ducktape/ducktape/.codex/worktrees/gpui-kit-migration/app/src/ui/components/icon.ice", 22, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let (__hash, __bytes) = ::ui_lang_guest::slots::picture(crate::host::icon(::std::convert::AsRef::as_ref(&("search")))); ::ui_lang_guest::wire::Node::Svg { inherit_button_ink: false, key: format!("{}/@media:22", __ice_use_scope), hash: __hash, bytes: __bytes, label: ::std::option::Option::None, color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[73]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), hover: ::std::option::Option::None, fit: ::std::option::Option::None, rotation: ::std::option::Option::None, opacity: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((16.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((16.0) as f32)) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!(("label" == "ink") || ("label" == "label"))) && ("label" == "meta")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("/home/eddy/dev/ducktape/ducktape/.codex/worktrees/gpui-kit-migration/app/src/ui/components/icon.ice", 28, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let (__hash, __bytes) = ::ui_lang_guest::slots::picture(crate::host::icon(::std::convert::AsRef::as_ref(&("search")))); ::ui_lang_guest::wire::Node::Svg { inherit_button_ink: false, key: format!("{}/@media:28", __ice_use_scope), hash: __hash, bytes: __bytes, label: ::std::option::Option::None, color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), hover: ::std::option::Option::None, fit: ::std::option::Option::None, rotation: ::std::option::Option::None, opacity: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((16.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((16.0) as f32)) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!((("label" == "ink") || ("label" == "label")) || ("label" == "meta"))) && ("label" == "caption")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("/home/eddy/dev/ducktape/ducktape/.codex/worktrees/gpui-kit-migration/app/src/ui/components/icon.ice", 34, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let (__hash, __bytes) = ::ui_lang_guest::slots::picture(crate::host::icon(::std::convert::AsRef::as_ref(&("search")))); ::ui_lang_guest::wire::Node::Svg { inherit_button_ink: false, key: format!("{}/@media:34", __ice_use_scope), hash: __hash, bytes: __bytes, label: ::std::option::Option::None, color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[70]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), hover: ::std::option::Option::None, fit: ::std::option::Option::None, rotation: ::std::option::Option::None, opacity: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((16.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((16.0) as f32)) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!(((("label" == "ink") || ("label" == "label")) || ("label" == "meta")) || ("label" == "caption"))) && ("label" == "hint")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("/home/eddy/dev/ducktape/ducktape/.codex/worktrees/gpui-kit-migration/app/src/ui/components/icon.ice", 40, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let (__hash, __bytes) = ::ui_lang_guest::slots::picture(crate::host::icon(::std::convert::AsRef::as_ref(&("search")))); ::ui_lang_guest::wire::Node::Svg { inherit_button_ink: false, key: format!("{}/@media:40", __ice_use_scope), hash: __hash, bytes: __bytes, label: ::std::option::Option::None, color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[72]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), hover: ::std::option::Option::None, fit: ::std::option::Option::None, rotation: ::std::option::Option::None, opacity: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((16.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((16.0) as f32)) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!((((("label" == "ink") || ("label" == "label")) || ("label" == "meta")) || ("label" == "caption")) || ("label" == "hint"))) && ("label" == "idle")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("/home/eddy/dev/ducktape/ducktape/.codex/worktrees/gpui-kit-migration/app/src/ui/components/icon.ice", 46, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let (__hash, __bytes) = ::ui_lang_guest::slots::picture(crate::host::icon(::std::convert::AsRef::as_ref(&("search")))); ::ui_lang_guest::wire::Node::Svg { inherit_button_ink: false, key: format!("{}/@media:46", __ice_use_scope), hash: __hash, bytes: __bytes, label: ::std::option::Option::None, color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[74]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), hover: ::std::option::Option::None, fit: ::std::option::Option::None, rotation: ::std::option::Option::None, opacity: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((16.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((16.0) as f32)) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!(((((("label" == "ink") || ("label" == "label")) || ("label" == "meta")) || ("label" == "caption")) || ("label" == "hint")) || ("label" == "idle"))) && ("label" == "accent")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("/home/eddy/dev/ducktape/ducktape/.codex/worktrees/gpui-kit-migration/app/src/ui/components/icon.ice", 52, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let (__hash, __bytes) = ::ui_lang_guest::slots::picture(crate::host::icon(::std::convert::AsRef::as_ref(&("search")))); ::ui_lang_guest::wire::Node::Svg { inherit_button_ink: false, key: format!("{}/@media:52", __ice_use_scope), hash: __hash, bytes: __bytes, label: ::std::option::Option::None, color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[16]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), hover: ::std::option::Option::None, fit: ::std::option::Option::None, rotation: ::std::option::Option::None, opacity: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((16.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((16.0) as f32)) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!((((((("label" == "ink") || ("label" == "label")) || ("label" == "meta")) || ("label" == "caption")) || ("label" == "hint")) || ("label" == "idle")) || ("label" == "accent"))) && ("label" == "success")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("/home/eddy/dev/ducktape/ducktape/.codex/worktrees/gpui-kit-migration/app/src/ui/components/icon.ice", 58, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let (__hash, __bytes) = ::ui_lang_guest::slots::picture(crate::host::icon(::std::convert::AsRef::as_ref(&("search")))); ::ui_lang_guest::wire::Node::Svg { inherit_button_ink: false, key: format!("{}/@media:58", __ice_use_scope), hash: __hash, bytes: __bytes, label: ::std::option::Option::None, color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[25]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), hover: ::std::option::Option::None, fit: ::std::option::Option::None, rotation: ::std::option::Option::None, opacity: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((16.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((16.0) as f32)) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!(((((((("label" == "ink") || ("label" == "label")) || ("label" == "meta")) || ("label" == "caption")) || ("label" == "hint")) || ("label" == "idle")) || ("label" == "accent")) || ("label" == "success"))) && ("label" == "warning")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("/home/eddy/dev/ducktape/ducktape/.codex/worktrees/gpui-kit-migration/app/src/ui/components/icon.ice", 64, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let (__hash, __bytes) = ::ui_lang_guest::slots::picture(crate::host::icon(::std::convert::AsRef::as_ref(&("search")))); ::ui_lang_guest::wire::Node::Svg { inherit_button_ink: false, key: format!("{}/@media:64", __ice_use_scope), hash: __hash, bytes: __bytes, label: ::std::option::Option::None, color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[30]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), hover: ::std::option::Option::None, fit: ::std::option::Option::None, rotation: ::std::option::Option::None, opacity: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((16.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((16.0) as f32)) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!((((((((("label" == "ink") || ("label" == "label")) || ("label" == "meta")) || ("label" == "caption")) || ("label" == "hint")) || ("label" == "idle")) || ("label" == "accent")) || ("label" == "success")) || ("label" == "warning"))) && ("label" == "danger")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("/home/eddy/dev/ducktape/ducktape/.codex/worktrees/gpui-kit-migration/app/src/ui/components/icon.ice", 70, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let (__hash, __bytes) = ::ui_lang_guest::slots::picture(crate::host::icon(::std::convert::AsRef::as_ref(&("search")))); ::ui_lang_guest::wire::Node::Svg { inherit_button_ink: false, key: format!("{}/@media:70", __ice_use_scope), hash: __hash, bytes: __bytes, label: ::std::option::Option::None, color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[20]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), hover: ::std::option::Option::None, fit: ::std::option::Option::None, rotation: ::std::option::Option::None, opacity: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((16.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((16.0) as f32)) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!(((((((((("label" == "ink") || ("label" == "label")) || ("label" == "meta")) || ("label" == "caption")) || ("label" == "hint")) || ("label" == "idle")) || ("label" == "accent")) || ("label" == "success")) || ("label" == "warning")) || ("label" == "danger"))) && ("label" == "paper")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("/home/eddy/dev/ducktape/ducktape/.codex/worktrees/gpui-kit-migration/app/src/ui/components/icon.ice", 76, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let (__hash, __bytes) = ::ui_lang_guest::slots::picture(crate::host::icon(::std::convert::AsRef::as_ref(&("search")))); ::ui_lang_guest::wire::Node::Svg { inherit_button_ink: false, key: format!("{}/@media:76", __ice_use_scope), hash: __hash, bytes: __bytes, label: ::std::option::Option::None, color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[38]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), hover: ::std::option::Option::None, fit: ::std::option::Option::None, rotation: ::std::option::Option::None, opacity: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((16.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((16.0) as f32)) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!((((((((((("label" == "ink") || ("label" == "label")) || ("label" == "meta")) || ("label" == "caption")) || ("label" == "hint")) || ("label" == "idle")) || ("label" == "accent")) || ("label" == "success")) || ("label" == "warning")) || ("label" == "danger")) || ("label" == "paper"))) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("/home/eddy/dev/ducktape/ducktape/.codex/worktrees/gpui-kit-migration/app/src/ui/components/icon.ice", 82, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let (__hash, __bytes) = ::ui_lang_guest::slots::picture(crate::host::icon(::std::convert::AsRef::as_ref(&("search")))); ::ui_lang_guest::wire::Node::Svg { inherit_button_ink: false, key: format!("{}/@media:82", __ice_use_scope), hash: __hash, bytes: __bytes, label: ::std::option::Option::None, color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), hover: ::std::option::Option::None, fit: ::std::option::Option::None, rotation: ::std::option::Option::None, opacity: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((16.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((16.0) as f32)) } };
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
impl super::ExplorerView {
pub(super) fn __ice_component_use_0(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __ExplorerViewMessage> { let __component_content: __IceElement<'_, __ExplorerViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 44, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 45, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:45", __ice_use_scope), size: ::std::option::Option::Some(((16.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Explorer".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if ("Search everything this workspace has recorded, or read the blocks that carried operations — an idle block keeps no row, so heights skip — newest first, each one openable for the ops it carried." != "") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 52, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::Some(((620.0) as f32).max(0.0).min(f32::MAX)), max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:52", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 53, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::Some(::ui_lang_guest::wire::LineHeight::Relative(((1.5) as f32).max(f32::EPSILON).min(f32::MAX))), shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:53", __ice_use_scope), size: ::std::option::Option::Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[70]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Search everything this workspace has recorded, or read the blocks that carried operations — an idle block keeps no row, so heights skip — newest first, each one openable for the ops it carried.".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((3.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_2(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __ExplorerViewMessage> { let __component_content: __IceElement<'_, __ExplorerViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 84, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); if (self.kind == "all") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 86, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:86", __ice_use_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (6.0) as f32, right: (11.0) as f32, bottom: (6.0) as f32, left: (11.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 94, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 95, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:95", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[9]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("All".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 101, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:101", __ice_use_scope), size: ::std::option::Option::Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (((self.hits).len() as i64)).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:94", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((6.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(self.kind == "all")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 108, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:108", __ice_use_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (6.0) as f32, right: (11.0) as f32, bottom: (6.0) as f32, left: (11.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 116, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 117, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:117", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[13]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("All".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 123, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:123", __ice_use_scope), size: ::std::option::Option::Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[73]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (((self.hits).len() as i64)).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:116", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((6.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
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
pub(super) fn __ice_component_use_3(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_0: ::std::string::String, __ice_arg_1: i64, __ice_arg_2: bool) -> __IceElement<'_, __ExplorerViewMessage> { let __component_content: __IceElement<'_, __ExplorerViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 84, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); if __ice_arg_2 { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 86, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:86", __ice_use_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (6.0) as f32, right: (11.0) as f32, bottom: (6.0) as f32, left: (11.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 94, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 95, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:95", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[9]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (__ice_arg_0.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 101, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:101", __ice_use_scope), size: ::std::option::Option::Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (__ice_arg_1).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:94", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((6.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!__ice_arg_2) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 108, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:108", __ice_use_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (6.0) as f32, right: (11.0) as f32, bottom: (6.0) as f32, left: (11.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 116, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 117, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:117", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[13]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (__ice_arg_0.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 123, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:123", __ice_use_scope), size: ::std::option::Option::Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[73]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (__ice_arg_1).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:116", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((6.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
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
pub(super) fn __ice_component_use_4(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_0: ::std::string::String, __ice_arg_1: ::std::string::String) -> __IceElement<'_, __ExplorerViewMessage> { let __component_content: __IceElement<'_, __ExplorerViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 248, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); if (__ice_arg_0 == "page") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 251, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:251", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((28.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((28.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[119]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 259, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:259", __ice_use_scope), size: ::std::option::Option::Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[118]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (__ice_arg_1.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!(__ice_arg_0 == "page")) && (__ice_arg_0 == "code")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 266, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:266", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((28.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((28.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[121]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 274, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:274", __ice_use_scope), size: ::std::option::Option::Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[120]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (__ice_arg_1.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!((__ice_arg_0 == "page") || (__ice_arg_0 == "code"))) && (__ice_arg_0 == "file")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 281, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:281", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((28.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((28.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[123]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 289, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:289", __ice_use_scope), size: ::std::option::Option::Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[122]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (__ice_arg_1.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!(((__ice_arg_0 == "page") || (__ice_arg_0 == "code")) || (__ice_arg_0 == "file"))) && (__ice_arg_0 == "run")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 296, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:296", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((28.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((28.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[125]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 304, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:304", __ice_use_scope), size: ::std::option::Option::Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[124]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (__ice_arg_1.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!((((__ice_arg_0 == "page") || (__ice_arg_0 == "code")) || (__ice_arg_0 == "file")) || (__ice_arg_0 == "run"))) && (__ice_arg_0 == "task")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 311, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:311", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((28.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((28.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[127]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 319, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:319", __ice_use_scope), size: ::std::option::Option::Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[126]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (__ice_arg_1.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(((((__ice_arg_0 == "page") || (__ice_arg_0 == "code")) || (__ice_arg_0 == "file")) || (__ice_arg_0 == "run")) || (__ice_arg_0 == "task"))) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 326, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:326", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((28.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((28.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[77]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 334, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:334", __ice_use_scope), size: ::std::option::Option::Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[76]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (__ice_arg_1.to_owned()).to_string() };
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
pub(super) fn __ice_component_use_5(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_0: ::std::string::String) -> __IceElement<'_, __ExplorerViewMessage> { let __component_content: __IceElement<'_, __ExplorerViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 342, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); if (__ice_arg_0 == "page") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 345, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:345", __ice_use_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (2.0) as f32, right: (5.0) as f32, bottom: (2.0) as f32, left: (5.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[119]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((4.0) as f32).max(0.0).min(f32::MAX), ((4.0) as f32).max(0.0).min(f32::MAX), ((4.0) as f32).max(0.0).min(f32::MAX), ((4.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 351, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:351", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[118]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("PAGE".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!(__ice_arg_0 == "page")) && (__ice_arg_0 == "code")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 358, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:358", __ice_use_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (2.0) as f32, right: (5.0) as f32, bottom: (2.0) as f32, left: (5.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[121]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((4.0) as f32).max(0.0).min(f32::MAX), ((4.0) as f32).max(0.0).min(f32::MAX), ((4.0) as f32).max(0.0).min(f32::MAX), ((4.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 364, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:364", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[120]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("CODE".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!((__ice_arg_0 == "page") || (__ice_arg_0 == "code"))) && (__ice_arg_0 == "file")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 371, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:371", __ice_use_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (2.0) as f32, right: (5.0) as f32, bottom: (2.0) as f32, left: (5.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[123]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((4.0) as f32).max(0.0).min(f32::MAX), ((4.0) as f32).max(0.0).min(f32::MAX), ((4.0) as f32).max(0.0).min(f32::MAX), ((4.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 377, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:377", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[122]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("FILE".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!(((__ice_arg_0 == "page") || (__ice_arg_0 == "code")) || (__ice_arg_0 == "file"))) && (__ice_arg_0 == "run")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 384, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:384", __ice_use_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (2.0) as f32, right: (5.0) as f32, bottom: (2.0) as f32, left: (5.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[125]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((4.0) as f32).max(0.0).min(f32::MAX), ((4.0) as f32).max(0.0).min(f32::MAX), ((4.0) as f32).max(0.0).min(f32::MAX), ((4.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 390, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:390", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[124]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("RUN".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!((((__ice_arg_0 == "page") || (__ice_arg_0 == "code")) || (__ice_arg_0 == "file")) || (__ice_arg_0 == "run"))) && (__ice_arg_0 == "task")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 397, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:397", __ice_use_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (2.0) as f32, right: (5.0) as f32, bottom: (2.0) as f32, left: (5.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[127]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((4.0) as f32).max(0.0).min(f32::MAX), ((4.0) as f32).max(0.0).min(f32::MAX), ((4.0) as f32).max(0.0).min(f32::MAX), ((4.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 403, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:403", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[126]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("TASK".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(((((__ice_arg_0 == "page") || (__ice_arg_0 == "code")) || (__ice_arg_0 == "file")) || (__ice_arg_0 == "run")) || (__ice_arg_0 == "task"))) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 410, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:410", __ice_use_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (2.0) as f32, right: (5.0) as f32, bottom: (2.0) as f32, left: (5.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[77]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((4.0) as f32).max(0.0).min(f32::MAX), ((4.0) as f32).max(0.0).min(f32::MAX), ((4.0) as f32).max(0.0).min(f32::MAX), ((4.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 416, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:416", __ice_use_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[76]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("MESSAGE".to_owned()).to_string() };
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
pub(super) fn __ice_component_use_6(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_0: crate::host::ExplorerHit) -> __IceElement<'_, __ExplorerViewMessage> { let __component_content: __IceElement<'_, __ExplorerViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 131, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: true, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (13.0) as f32, right: (15.0) as f32, bottom: (13.0) as f32, left: (15.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[60]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((11.0) as f32).max(0.0).min(f32::MAX), ((11.0) as f32).max(0.0).min(f32::MAX), ((11.0) as f32).max(0.0).min(f32::MAX), ((11.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 141, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 146, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_4(__ice_palette, format!("{}/ExplorerKindPlate@766", __ice_use_scope), __ice_arg_0.kind.to_owned(), __ice_arg_0.code.to_owned()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 147, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 148, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 153, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:153", __ice_use_scope), size: ::std::option::Option::Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: (__ice_arg_0.title.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 160, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_5(__ice_palette, format!("{}/ExplorerKindBadge@780", __ice_use_scope), __ice_arg_0.kind.to_owned()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:148", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 164, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::Some(::ui_lang_guest::wire::LineHeight::Relative(((1.5) as f32).max(f32::EPSILON).min(f32::MAX))), shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::Word), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:164", __ice_use_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[41]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: (__ice_arg_0.snippet.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 171, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::Some(::ui_lang_guest::wire::Wrapping::None), tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist Mono".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:171", __ice_use_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[73]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: (__ice_arg_0.meta.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:147", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((3.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:141", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((12.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Left), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_7(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __ExplorerViewMessage> { let __component_content: __IceElement<'_, __ExplorerViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 5, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (30.0) as f32, right: (30.0) as f32, bottom: (30.0) as f32, left: (30.0) as f32 }), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((12.0) as f32).max(0.0).min(f32::MAX), ((12.0) as f32).max(0.0).min(f32::MAX), ((12.0) as f32).max(0.0).min(f32::MAX), ((12.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 14, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:14", __ice_use_scope), size: ::std::option::Option::Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Nothing of that kind matched — the other chips still hold results.".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_8(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __ExplorerViewMessage> { let __component_content: __IceElement<'_, __ExplorerViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 5, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (30.0) as f32, right: (30.0) as f32, bottom: (30.0) as f32, left: (30.0) as f32 }), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((12.0) as f32).max(0.0).min(f32::MAX), ((12.0) as f32).max(0.0).min(f32::MAX), ((12.0) as f32).max(0.0).min(f32::MAX), ((12.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 14, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:14", __ice_use_scope), size: ::std::option::Option::Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Nothing matched that query in this workspace.".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_9(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __ExplorerViewMessage> { let __component_content: __IceElement<'_, __ExplorerViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 17, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (22.0) as f32, right: (22.0) as f32, bottom: (22.0) as f32, left: (22.0) as f32 }), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 24, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 29, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:29", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((42.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((42.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((21.0) as f32).max(0.0).min(f32::MAX), ((21.0) as f32).max(0.0).min(f32::MAX), ((21.0) as f32).max(0.0).min(f32::MAX), ((21.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 39, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:39", __ice_use_scope), size: ::std::option::Option::Some(((20.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("◇".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 40, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::Some(::ui_lang_guest::wire::LineHeight::Relative(1.35f32)), shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:40", __ice_use_scope), size: ::std::option::Option::Some(16.0f32), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Not connected".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 41, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::Some(::ui_lang_guest::wire::LineHeight::Relative(1.5f32)), shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:41", __ice_use_scope), size: ::std::option::Option::Some(12.5f32), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Click the network name in the titlebar to pick or reconnect a network.".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:24", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((7.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_10(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __ExplorerViewMessage> { let __component_content: __IceElement<'_, __ExplorerViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 17, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (22.0) as f32, right: (22.0) as f32, bottom: (22.0) as f32, left: (22.0) as f32 }), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 24, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 29, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:29", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((42.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((42.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((21.0) as f32).max(0.0).min(f32::MAX), ((21.0) as f32).max(0.0).min(f32::MAX), ((21.0) as f32).max(0.0).min(f32::MAX), ((21.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 39, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:39", __ice_use_scope), size: ::std::option::Option::Some(((20.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("◇".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 40, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::Some(::ui_lang_guest::wire::LineHeight::Relative(1.35f32)), shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:40", __ice_use_scope), size: ::std::option::Option::Some(16.0f32), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("No blocks yet".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 41, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::Some(::ui_lang_guest::wire::LineHeight::Relative(1.5f32)), shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:41", __ice_use_scope), size: ::std::option::Option::Some(12.5f32), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Blocks that carried operations appear here as they finalize.".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:24", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((7.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_13(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String) -> __IceElement<'_, __ExplorerViewMessage> { let __component_content: __IceElement<'_, __ExplorerViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 17, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (22.0) as f32, right: (22.0) as f32, bottom: (22.0) as f32, left: (22.0) as f32 }), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 24, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 29, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:29", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((42.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((42.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((21.0) as f32).max(0.0).min(f32::MAX), ((21.0) as f32).max(0.0).min(f32::MAX), ((21.0) as f32).max(0.0).min(f32::MAX), ((21.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 39, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:39", __ice_use_scope), size: ::std::option::Option::Some(((20.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("◇".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 40, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::Some(::ui_lang_guest::wire::LineHeight::Relative(1.35f32)), shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:40", __ice_use_scope), size: ::std::option::Option::Some(16.0f32), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Select a block".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 41, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::Some(::ui_lang_guest::wire::LineHeight::Relative(1.5f32)), shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:41", __ice_use_scope), size: ::std::option::Option::Some(12.5f32), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Its operations and dispatch traces appear here.".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:24", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((7.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_16(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_0: ::std::string::String) -> __IceElement<'_, __ExplorerViewMessage> { let __component_content: __IceElement<'_, __ExplorerViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 180, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (3.0) as f32, right: (7.0) as f32, bottom: (3.0) as f32, left: (7.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[27]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[28]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 188, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 189, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:189", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((6.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((6.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[29]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((3.0) as f32).max(0.0).min(f32::MAX), ((3.0) as f32).max(0.0).min(f32::MAX), ((3.0) as f32).max(0.0).min(f32::MAX), ((3.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 195, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 196, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::Some(::ui_lang_guest::wire::LineHeight::Relative(1.35f32)), shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:196", __ice_use_scope), size: ::std::option::Option::Some(9.0f32), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (__ice_arg_0.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:188", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((5.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_17(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_0: ::std::string::String) -> __IceElement<'_, __ExplorerViewMessage> { let __component_content: __IceElement<'_, __ExplorerViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 199, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (3.0) as f32, right: (7.0) as f32, bottom: (3.0) as f32, left: (7.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[32]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[33]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 207, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 208, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:208", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((6.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((6.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[34]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((3.0) as f32).max(0.0).min(f32::MAX), ((3.0) as f32).max(0.0).min(f32::MAX), ((3.0) as f32).max(0.0).min(f32::MAX), ((3.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 214, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 215, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::Some(::ui_lang_guest::wire::LineHeight::Relative(1.35f32)), shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:215", __ice_use_scope), size: ::std::option::Option::Some(9.0f32), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (__ice_arg_0.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:207", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((5.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_18(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_0: ::std::string::String) -> __IceElement<'_, __ExplorerViewMessage> { let __component_content: __IceElement<'_, __ExplorerViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 218, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (3.0) as f32, right: (7.0) as f32, bottom: (3.0) as f32, left: (7.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[22]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[23]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 226, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 227, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:227", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((6.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((6.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[24]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((3.0) as f32).max(0.0).min(f32::MAX), ((3.0) as f32).max(0.0).min(f32::MAX), ((3.0) as f32).max(0.0).min(f32::MAX), ((3.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 233, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 234, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::Some(::ui_lang_guest::wire::LineHeight::Relative(1.35f32)), shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:234", __ice_use_scope), size: ::std::option::Option::Some(9.0f32), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (__ice_arg_0.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:226", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((5.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_19(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_0: ::std::string::String) -> __IceElement<'_, __ExplorerViewMessage> { let __component_content: __IceElement<'_, __ExplorerViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 237, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let __ice_node_scope = format!("{}/root", __ice_use_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (3.0) as f32, right: (7.0) as f32, bottom: (3.0) as f32, left: (7.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[40]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 245, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::Some(::ui_lang_guest::wire::LineHeight::Relative(1.35f32)), shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:245", __ice_use_scope), size: ::std::option::Option::Some(9.0f32), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[13]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (__ice_arg_0.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
pub(super) fn __ice_component_use_20(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_0: ::std::string::String) -> __IceElement<'_, __ExplorerViewMessage> { let __component_content: __IceElement<'_, __ExplorerViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 60, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __ExplorerViewMessage>> = ::std::vec::Vec::new(); if (__ice_arg_0 == "active") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 63, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_16(__ice_palette, format!("{}/Badge.Success@683", __ice_use_scope), __ice_arg_0.to_owned()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!(__ice_arg_0 == "active")) && (__ice_arg_0 == "paused")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 65, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_17(__ice_palette, format!("{}/Badge.Warning@685", __ice_use_scope), __ice_arg_0.to_owned()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!((__ice_arg_0 == "active") || (__ice_arg_0 == "paused"))) && (__ice_arg_0 == "open")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 67, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_16(__ice_palette, format!("{}/Badge.Success@687", __ice_use_scope), __ice_arg_0.to_owned()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!(((__ice_arg_0 == "active") || (__ice_arg_0 == "paused")) || (__ice_arg_0 == "open"))) && (__ice_arg_0 == "closed")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 69, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_18(__ice_palette, format!("{}/Badge.Destructive@689", __ice_use_scope), __ice_arg_0.to_owned()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!((((__ice_arg_0 == "active") || (__ice_arg_0 == "paused")) || (__ice_arg_0 == "open")) || (__ice_arg_0 == "closed"))) && (__ice_arg_0 == "merged")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 71, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_16(__ice_palette, format!("{}/Badge.Success@691", __ice_use_scope), __ice_arg_0.to_owned()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!(((((__ice_arg_0 == "active") || (__ice_arg_0 == "paused")) || (__ice_arg_0 == "open")) || (__ice_arg_0 == "closed")) || (__ice_arg_0 == "merged"))) && (__ice_arg_0 == "passed")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 73, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_16(__ice_palette, format!("{}/Badge.Success@693", __ice_use_scope), __ice_arg_0.to_owned()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!((((((__ice_arg_0 == "active") || (__ice_arg_0 == "paused")) || (__ice_arg_0 == "open")) || (__ice_arg_0 == "closed")) || (__ice_arg_0 == "merged")) || (__ice_arg_0 == "passed"))) && (__ice_arg_0 == "rejected")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 75, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_18(__ice_palette, format!("{}/Badge.Destructive@695", __ice_use_scope), __ice_arg_0.to_owned()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!(((((((__ice_arg_0 == "active") || (__ice_arg_0 == "paused")) || (__ice_arg_0 == "open")) || (__ice_arg_0 == "closed")) || (__ice_arg_0 == "merged")) || (__ice_arg_0 == "passed")) || (__ice_arg_0 == "rejected"))) && (__ice_arg_0 == "applied")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 77, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_16(__ice_palette, format!("{}/Badge.Success@697", __ice_use_scope), __ice_arg_0.to_owned()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!((((((((__ice_arg_0 == "active") || (__ice_arg_0 == "paused")) || (__ice_arg_0 == "open")) || (__ice_arg_0 == "closed")) || (__ice_arg_0 == "merged")) || (__ice_arg_0 == "passed")) || (__ice_arg_0 == "rejected")) || (__ice_arg_0 == "applied"))) && (__ice_arg_0 == "discarded")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 79, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_17(__ice_palette, format!("{}/Badge.Warning@699", __ice_use_scope), __ice_arg_0.to_owned()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(((((((((__ice_arg_0 == "active") || (__ice_arg_0 == "paused")) || (__ice_arg_0 == "open")) || (__ice_arg_0 == "closed")) || (__ice_arg_0 == "merged")) || (__ice_arg_0 == "passed")) || (__ice_arg_0 == "rejected")) || (__ice_arg_0 == "applied")) || (__ice_arg_0 == "discarded"))) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/kit.ice", 81, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __ExplorerViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_19(__ice_palette, format!("{}/Badge.Outline@701", __ice_use_scope), __ice_arg_0.to_owned()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:60", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
}
}



ui_lang_guest::export_app!(
    ExplorerView,
    "Explorer",
    "The ledger this network wrote: blocks, their operations, and a search over the workspace.",
    ["explorer"]
);
