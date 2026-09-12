//! The Agents register as a view on the kernel contract: who may act, which
//! executor they run on, the skills they carry and what their runs did,
//! rendered from a wasm component the desktop app loads from a file.
//!
//! The kernel pushes session facts only (`agents.props`: connected, dark,
//! the signing account, and the run another tab opened for the reader). The
//! register, the run tracker and one run's journal are read here through
//! the kernel's `rpc.query` / `rpc.view`, re-read on every `rpc.live` hit
//! for the `runs` and `identity` planes, and a pause or a save leaves as
//! `op.submit` — the runs message the kernel signs with the seated key. The
//! endpoint, the key and the password never cross: a guest that sees no key
//! cannot leak one.

pub mod host;

macro_rules! __ice_generated_items_4167656e747356696577 { ($($item:item)*) => { $(#[allow(warnings, clippy::all)] $item)* }; }
__ice_generated_items_4167656e747356696577! {
type __IceElement<'a, Message, Theme = ()> = <(&'a (), Message, Theme) as ::ui_lang_guest::wire::Erase>::Node;
pub(crate) type __IceMessage = __AgentsViewMessage;
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
pub struct AgentsView {
pub(crate) __ice_accessibility: ::ui_lang_runtime::Bridge<__AgentsViewMessage>,
#[cfg(all(target_os = "windows", not(test)))]
pub(crate) __ice_accessibility_initial: ::std::option::Option<usize>,
#[cfg(all(target_os = "windows", not(test)))]
pub(crate) __ice_accessibility_pending: ::std::vec::Vec<__AgentsViewMessage>,
pub(crate) active_palette: AppTheme,
pub(crate) rows: ::std::vec::Vec<crate::host::AgentRow>,
pub(crate) runs: ::std::vec::Vec<crate::host::RunRow>,
pub(crate) journal: crate::host::RunJournal,
pub(crate) live: crate::host::LiveRun,
pub(crate) panel: ::std::string::String,
pub(crate) open_run: ::std::string::String,
pub(crate) journal_width: f64,
pub(crate) editor_width: f64,
pub(crate) viewport_width: f64,
pub(crate) expanded_receipt: ::std::string::String,
pub(crate) open_row: crate::host::RunRow,
pub(crate) opened: i64,
pub(crate) capabilities: ::std::vec::Vec<::std::string::String>,
pub(crate) account: ::std::string::String,
pub(crate) committed: i64,
pub(crate) seeded: i64,
pub(crate) connected: bool,
pub(crate) connection_serial: i64,
pub(crate) answered: bool,
pub(crate) host_error: ::std::string::String,
pub(crate) selected: ::std::string::String,
pub(crate) creating: bool,
pub(crate) can_edit: bool,
pub(crate) selected_status: ::std::string::String,
pub(crate) draft_id: ::std::string::String,
pub(crate) draft_name: ::std::string::String,
pub(crate) draft_capability: ::std::option::Option<::std::string::String>,
pub(crate) draft_skills: ::std::vec::Vec<crate::host::AgentSkill>,
pub(crate) skill_name: ::std::string::String,
pub(crate) skill_prefix: ::std::string::String,
pub(crate) skill_snapshot: ::std::string::String,
pub(crate) skill_always: bool,
pub(crate) sent: bool,
pub(crate) __ice_rev: [u64; 34],
}
impl ::std::fmt::Debug for AgentsView { fn fmt(&self, __formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result { __formatter.write_str("AgentsView") } }
#[derive(Clone)]
pub(crate) enum __AgentsViewMessage {
__AccessibilitySnapshot(::std::boxed::Box<::ui_lang_runtime::Snapshot<__AgentsViewMessage>>),
__AccessibilityAction(::ui_lang_runtime::ActionRequest),
__AccessibilityWindow(::iced::window::Id, ::iced::window::Event),
#[cfg(all(any(target_os = "windows", target_os = "macos"), not(test)))]
__AccessibilityNativeWindow(::ui_lang_runtime::NativeWindow),
__AccessibilityFocusNext(::std::option::Option<::iced::window::Id>),
__AccessibilityFocusPrevious(::std::option::Option<::iced::window::Id>),
__TemplateChanged,
JournalResized(f64, f64),
EditorResized(f64, f64),
ViewportChanged(f64, f64),
ToggleReceipt(::std::string::String),
SessionArrived(crate::host::SessionItem),
RegisterArrived(crate::host::RegisterItem),
JournalArrived(crate::host::JournalItem),
LiveArrived(crate::host::LiveRun),
ActDone(crate::host::ActItem),
OpenAgent(::std::string::String),
OpenNew,
CloseEditor,
ChoosePanel(::std::string::String),
OpenRunRow(::std::string::String),
CloseRun,
OpenPlace(::std::string::String),
PickCapabilityOption(::std::string::String),
SetSkillAlways(bool),
AddSkill,
RemoveSkill(::std::string::String),
LoadSkill(::std::string::String, bool),
SetStatus(::std::string::String, bool),
SubmitSave,
SubmitRegister,
__BindDraftId(::std::string::String),
__BindDraftName(::std::string::String),
__BindSkillName(::std::string::String),
__BindSkillPrefix(::std::string::String),
__BindSkillSnapshot(::std::string::String),
}
impl ::std::fmt::Debug for __AgentsViewMessage { fn fmt(&self, __formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result { __formatter.write_str("__AgentsViewMessage") } }
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_AgentSkill(_value: &crate::host::AgentSkill) {
let _: &::std::string::String = &_value.name;
let _: &::std::string::String = &_value.source_prefix;
let _: &::std::string::String = &_value.source_snapshot;
let _: &bool = &_value.always;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_AgentRow(_value: &crate::host::AgentRow) {
let _: &::std::string::String = &_value.id;
let _: &::std::string::String = &_value.name;
let _: &::std::string::String = &_value.initials;
let _: &::std::string::String = &_value.capability;
let _: &::std::string::String = &_value.status;
let _: &::std::string::String = &_value.owner_handle;
let _: &::std::string::String = &_value.controller;
let _: &bool = &_value.live;
let _: &::std::vec::Vec<crate::host::AgentSkill> = &_value.skills;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_RunRow(_value: &crate::host::RunRow) {
let _: &::std::string::String = &_value.run_id;
let _: &::std::string::String = &_value.dispatch_id;
let _: &::std::string::String = &_value.agent_id;
let _: &::std::string::String = &_value.agent_name;
let _: &::std::string::String = &_value.origin;
let _: &::std::string::String = &_value.state;
let _: &::std::string::String = &_value.dispatched;
let _: &::std::string::String = &_value.settled;
let _: &i64 = &_value.attempt;
let _: &::std::string::String = &_value.holder;
let _: &i64 = &_value.actions;
let _: &bool = &_value.degraded;
let _: &::std::string::String = &_value.reason;
let _: &::std::string::String = &_value.output_ref;
let _: &i64 = &_value.pr_number;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_JournalEntry(_value: &crate::host::JournalEntry) {
let _: &::std::string::String = &_value.height;
let _: &::std::string::String = &_value.kind;
let _: &::std::string::String = &_value.summary;
let _: &::std::string::String = &_value.status;
let _: &::std::vec::Vec<crate::host::RunLink> = &_value.targets;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_RunLink(_value: &crate::host::RunLink) {
let _: &::std::string::String = &_value.relation;
let _: &::std::string::String = &_value.kind;
let _: &::std::string::String = &_value.label;
let _: &::std::string::String = &_value.url;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_RunJournal(_value: &crate::host::RunJournal) {
let _: &::std::string::String = &_value.dispatch_id;
let _: &::std::vec::Vec<crate::host::JournalEntry> = &_value.entries;
let _: &::std::vec::Vec<crate::host::RunLink> = &_value.links;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_LiveActivity(_value: &crate::host::LiveActivity) {
let _: &::std::string::String = &_value.label;
let _: &bool = &_value.done;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_LiveRun(_value: &crate::host::LiveRun) {
let _: &bool = &_value.present;
let _: &::std::string::String = &_value.status;
let _: &::std::vec::Vec<crate::host::LiveActivity> = &_value.activity;
let _: &::std::string::String = &_value.answer_preview;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_Session(_value: &crate::host::Session) {
let _: &bool = &_value.connected;
let _: &bool = &_value.dark;
let _: &::std::string::String = &_value.account;
let _: &::std::string::String = &_value.open_run;
let _: &i64 = &_value.opened;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_SessionItem(_value: &crate::host::SessionItem) {
let _: &crate::host::Session = &_value.next;
let _: &::std::string::String = &_value.error;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_RegisterItem(_value: &crate::host::RegisterItem) {
let _: &::std::vec::Vec<crate::host::AgentRow> = &_value.rows;
let _: &::std::vec::Vec<crate::host::RunRow> = &_value.runs;
let _: &::std::vec::Vec<::std::string::String> = &_value.capabilities;
let _: &::std::string::String = &_value.error;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_JournalItem(_value: &crate::host::JournalItem) {
let _: &crate::host::RunJournal = &_value.journal;
let _: &::std::string::String = &_value.error;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_ActItem(_value: &crate::host::ActItem) {
let _: &::std::string::String = &_value.error;
}
#[allow(dead_code)] fn __ui_lang_check_subscription_session() { let _: ::iced::Subscription<crate::host::SessionItem> = crate::host::session(); }
#[allow(dead_code)] fn __ui_lang_check_subscription_register(arg0: i64) { let _: ::iced::Subscription<crate::host::RegisterItem> = crate::host::register(arg0); }
#[allow(dead_code)] fn __ui_lang_check_subscription_run_journal(arg0: ::std::string::String, arg1: i64) { let _: ::iced::Subscription<crate::host::JournalItem> = crate::host::run_journal(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_subscription_live_run(arg0: ::std::string::String, arg1: i64) { let _: ::iced::Subscription<crate::host::LiveRun> = crate::host::live_run(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_subscription_acts() { let _: ::iced::Subscription<crate::host::ActItem> = crate::host::acts(); }
#[allow(dead_code)] fn __ui_lang_check_pure_connection_serial_after(arg0: bool, arg1: bool, arg2: i64) { let _: i64 = crate::host::connection_serial_after(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_drafts_consumed<'a>(arg0: i64, arg1: i64, arg2: bool, arg3: &'a [crate::host::AgentRow], arg4: &'a str) { let _: bool = crate::host::drafts_consumed(arg0, arg1, arg2, arg3, arg4); }
#[allow(dead_code)] fn __ui_lang_check_pure_working_agents<'a>(arg0: &'a [crate::host::AgentRow]) { let _: i64 = crate::host::working_agents(arg0); }
#[allow(dead_code)] fn __ui_lang_check_sync_badge(arg0: i64) { let _: bool = crate::host::badge(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_agents_summary<'a>(arg0: bool, arg1: &'a [crate::host::AgentRow]) { let _: ::std::string::String = crate::host::agents_summary(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_runs_summary<'a>(arg0: &'a [crate::host::RunRow]) { let _: ::std::string::String = crate::host::runs_summary(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_run_named<'a>(arg0: &'a [crate::host::RunRow], arg1: &'a str) { let _: crate::host::RunRow = crate::host::run_named(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_run_at<'a>(arg0: &'a [crate::host::RunRow], arg1: &'a str) { let _: crate::host::RunRow = crate::host::run_at(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_empty_journal() { let _: crate::host::RunJournal = crate::host::empty_journal(); }
#[allow(dead_code)] fn __ui_lang_check_pure_empty_live() { let _: crate::host::LiveRun = crate::host::empty_live(); }
#[allow(dead_code)] fn __ui_lang_check_pure_empty_run() { let _: crate::host::RunRow = crate::host::empty_run(); }
#[allow(dead_code)] fn __ui_lang_check_pure_link_glyph<'a>(arg0: &'a str) { let _: ::std::string::String = crate::host::link_glyph(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_journal_width_after_delta(arg0: f64, arg1: f64, arg2: f64) { let _: f64 = crate::host::journal_width_after_delta(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_editor_width_after_delta(arg0: f64, arg1: f64, arg2: f64) { let _: f64 = crate::host::editor_width_after_delta(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_open_run<'a>(arg0: &'a str) { let _: bool = crate::host::open_run(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_open_link<'a>(arg0: &'a str) { let _: bool = crate::host::open_link(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_skill_count<'a>(arg0: &'a [crate::host::AgentSkill]) { let _: i64 = crate::host::skill_count(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_row_named<'a>(arg0: &'a [crate::host::AgentRow], arg1: &'a str) { let _: crate::host::AgentRow = crate::host::row_named(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_editable<'a>(arg0: bool, arg1: &'a str, arg2: &'a str) { let _: bool = crate::host::editable(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_has<'a>(arg0: &'a [::std::string::String], arg1: &'a str) { let _: bool = crate::host::has(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_with_skill<'a>(arg0: &'a [crate::host::AgentSkill], arg1: &'a str, arg2: &'a str, arg3: &'a str, arg4: bool) { let _: ::std::vec::Vec<crate::host::AgentSkill> = crate::host::with_skill(arg0, arg1, arg2, arg3, arg4); }
#[allow(dead_code)] fn __ui_lang_check_pure_without_skill<'a>(arg0: &'a [crate::host::AgentSkill], arg1: &'a str) { let _: ::std::vec::Vec<crate::host::AgentSkill> = crate::host::without_skill(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_skill_loaded<'a>(arg0: &'a [crate::host::AgentSkill], arg1: &'a str, arg2: bool) { let _: ::std::vec::Vec<crate::host::AgentSkill> = crate::host::skill_loaded(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_skill_mode(arg0: bool) { let _: ::std::string::String = crate::host::skill_mode(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_library_prefix<'a>(arg0: &'a str) { let _: ::std::string::String = crate::host::library_prefix(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_capability_options<'a>(arg0: &'a [::std::string::String], arg1: &'a str) { let _: ::std::vec::Vec<::std::string::String> = crate::host::capability_options(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_some_str<'a>(arg0: &'a str) { let _: ::std::option::Option<::std::string::String> = crate::host::some_str(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_or_empty<'a>(arg0: &'a ::std::option::Option<::std::string::String>) { let _: ::std::string::String = crate::host::or_empty(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_pick_str<'a>(arg0: bool, arg1: &'a str, arg2: &'a str) { let _: ::std::string::String = crate::host::pick_str(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_pick_capability<'a>(arg0: bool, arg1: &'a str, arg2: &'a ::std::option::Option<::std::string::String>) { let _: ::std::option::Option<::std::string::String> = crate::host::pick_capability(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_pick_list<'a>(arg0: bool, arg1: &'a [::std::string::String], arg2: &'a [::std::string::String]) { let _: ::std::vec::Vec<::std::string::String> = crate::host::pick_list(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_pick_skills<'a>(arg0: bool, arg1: &'a [crate::host::AgentSkill], arg2: &'a [crate::host::AgentSkill]) { let _: ::std::vec::Vec<crate::host::AgentSkill> = crate::host::pick_skills(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_valid_agent_id<'a>(arg0: &'a str) { let _: bool = crate::host::valid_agent_id(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_pane_note<'a>(arg0: &'a str) { let _: ::std::string::String = crate::host::pane_note(arg0); }
#[allow(dead_code)] fn __ui_lang_check_sync_status<'a>(arg0: &'a str, arg1: bool) { let _: bool = crate::host::status(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_sync_save<'a>(arg0: &'a str, arg1: &'a str, arg2: &'a str, arg3: &'a [crate::host::AgentSkill]) { let _: bool = crate::host::save(arg0, arg1, arg2, arg3); }
#[allow(dead_code)] fn __ui_lang_check_pure_register_agent<'a>(arg0: &'a str, arg1: &'a str, arg2: &'a str, arg3: &'a [crate::host::AgentSkill]) { let _: bool = crate::host::register_agent(arg0, arg1, arg2, arg3); }
}
__ice_generated_items_4167656e747356696577! {
#[allow(unused_parens)]
impl AgentsView {
#[must_use]
pub fn default_font() -> ::iced::Font { ::iced::Font { family: ::iced::font::Family::Name("Geist"), weight: ::iced::font::Weight::Normal, stretch: ::iced::font::Stretch::Normal, style: ::iced::font::Style::Normal } }
}
}
__ice_generated_items_4167656e747356696577! {
#[allow(unused_parens)]
impl AgentsView {
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
::iced::Theme::custom(::std::format!("AgentsView/{}", __ice_palette.name), ::iced::theme::Palette {
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
fn __title(&self) -> ::std::string::String { "Agents".to_owned() }
}
}
__ice_generated_items_4167656e747356696577! {
#[allow(unused_parens)]
impl AgentsView {
fn __state() -> Self {
Self {
__ice_accessibility: ::ui_lang_runtime::Bridge::new(),
#[cfg(all(target_os = "windows", not(test)))]
__ice_accessibility_initial: ::std::option::Option::None,
#[cfg(all(target_os = "windows", not(test)))]
__ice_accessibility_pending: ::std::vec::Vec::new(),
active_palette: AppTheme::App,
rows: ::std::vec::Vec::new(),
runs: ::std::vec::Vec::new(),
journal: crate::host::empty_journal(),
live: crate::host::empty_live(),
panel: "registry".to_owned(),
open_run: "".to_owned(),
journal_width: 400.0,
editor_width: 400.0,
viewport_width: 1280.0,
expanded_receipt: "".to_owned(),
open_row: crate::host::empty_run(),
opened: 0,
capabilities: ::std::vec::Vec::new(),
account: "".to_owned(),
committed: 0,
seeded: 0,
connected: false,
connection_serial: 0,
answered: false,
host_error: "".to_owned(),
selected: "".to_owned(),
creating: false,
can_edit: false,
selected_status: "".to_owned(),
draft_id: "".to_owned(),
draft_name: "".to_owned(),
draft_capability: ::std::option::Option::None,
draft_skills: ::std::vec::Vec::new(),
skill_name: "".to_owned(),
skill_prefix: "".to_owned(),
skill_snapshot: "".to_owned(),
skill_always: false,
sent: false,
__ice_rev: [::ui_lang_runtime::rev::seed(); 34],
}
}
fn __boot_task(&mut self) -> ::iced::Task<__AgentsViewMessage> {
let task = (|| {
::iced::Task::none()
})();
task
}
pub(crate) fn __boot() -> (Self, ::iced::Task<__AgentsViewMessage>) {
let mut state = Self::__state();
let task = state.__boot_task();
(state, task)
}
pub(crate) const __PREFERRED_WINDOW_SIZE: &'static str = "none";
#[allow(clippy::too_many_arguments)] fn __restore_state(active_palette: AppTheme, rows: ::std::vec::Vec<crate::host::AgentRow>, runs: ::std::vec::Vec<crate::host::RunRow>, journal: crate::host::RunJournal, live: crate::host::LiveRun, panel: ::std::string::String, open_run: ::std::string::String, journal_width: f64, editor_width: f64, viewport_width: f64, expanded_receipt: ::std::string::String, open_row: crate::host::RunRow, opened: i64, capabilities: ::std::vec::Vec<::std::string::String>, account: ::std::string::String, committed: i64, seeded: i64, connected: bool, connection_serial: i64, answered: bool, host_error: ::std::string::String, selected: ::std::string::String, creating: bool, can_edit: bool, selected_status: ::std::string::String, draft_id: ::std::string::String, draft_name: ::std::string::String, draft_capability: ::std::option::Option<::std::string::String>, draft_skills: ::std::vec::Vec<crate::host::AgentSkill>, skill_name: ::std::string::String, skill_prefix: ::std::string::String, skill_snapshot: ::std::string::String, skill_always: bool, sent: bool) -> Self {
Self {
__ice_accessibility: ::ui_lang_runtime::Bridge::new(),
#[cfg(all(target_os = "windows", not(test)))]
__ice_accessibility_initial: ::std::option::Option::None,
#[cfg(all(target_os = "windows", not(test)))]
__ice_accessibility_pending: ::std::vec::Vec::new(),
active_palette: active_palette,
rows: rows,
runs: runs,
journal: journal,
live: live,
panel: panel,
open_run: open_run,
journal_width: journal_width,
editor_width: editor_width,
viewport_width: viewport_width,
expanded_receipt: expanded_receipt,
open_row: open_row,
opened: opened,
capabilities: capabilities,
account: account,
committed: committed,
seeded: seeded,
connected: connected,
connection_serial: connection_serial,
answered: answered,
host_error: host_error,
selected: selected,
creating: creating,
can_edit: can_edit,
selected_status: selected_status,
draft_id: draft_id,
draft_name: draft_name,
draft_capability: draft_capability,
draft_skills: draft_skills,
skill_name: skill_name,
skill_prefix: skill_prefix,
skill_snapshot: skill_snapshot,
skill_always: skill_always,
sent: sent,
__ice_rev: [::ui_lang_runtime::rev::seed(); 34],
}
}
pub(crate) const __SNAPSHOT_SCHEMA: &'static str = "ea29d77b6e03069ca0c580bd8bf52d687528c49195c03419d1d233b539e53355";
pub(crate) fn __snapshot(&self) -> ::std::result::Result<::std::vec::Vec<u8>, ::std::string::String> { ::ui_lang_guest::wire::Snapshot {schema: ::std::string::String::from(Self::__SNAPSHOT_SCHEMA), state: ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("AgentsView"), fields: vec![(::std::string::String::from("active_palette"), match &self.active_palette { AppTheme::App => ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("AppTheme"), fields: vec![(::std::string::String::from("app"), ::ui_lang_guest::wire::SnapshotValue::Unit)] }, AppTheme::AppDark => ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("AppTheme"), fields: vec![(::std::string::String::from("app_dark"), ::ui_lang_guest::wire::SnapshotValue::Unit)] } }), (::std::string::String::from("rows"), ::ui_lang_guest::wire::SnapshotValue::List((&self.rows).iter().map(|__item| ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("AgentRow"), fields: ::std::vec![(::std::string::String::from("id"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).id))), (::std::string::String::from("name"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).name))), (::std::string::String::from("initials"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).initials))), (::std::string::String::from("capability"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).capability))), (::std::string::String::from("status"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).status))), (::std::string::String::from("owner_handle"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).owner_handle))), (::std::string::String::from("controller"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).controller))), (::std::string::String::from("live"), ::ui_lang_guest::wire::SnapshotValue::Bool(*(&(__item).live))), (::std::string::String::from("skills"), ::ui_lang_guest::wire::SnapshotValue::List((&(__item).skills).iter().map(|__item| ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("AgentSkill"), fields: ::std::vec![(::std::string::String::from("name"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).name))), (::std::string::String::from("source_prefix"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).source_prefix))), (::std::string::String::from("source_snapshot"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).source_snapshot))), (::std::string::String::from("always"), ::ui_lang_guest::wire::SnapshotValue::Bool(*(&(__item).always)))] }).collect()))] }).collect())), (::std::string::String::from("runs"), ::ui_lang_guest::wire::SnapshotValue::List((&self.runs).iter().map(|__item| ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("RunRow"), fields: ::std::vec![(::std::string::String::from("run_id"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).run_id))), (::std::string::String::from("dispatch_id"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).dispatch_id))), (::std::string::String::from("agent_id"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).agent_id))), (::std::string::String::from("agent_name"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).agent_name))), (::std::string::String::from("origin"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).origin))), (::std::string::String::from("state"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).state))), (::std::string::String::from("dispatched"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).dispatched))), (::std::string::String::from("settled"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).settled))), (::std::string::String::from("attempt"), ::ui_lang_guest::wire::SnapshotValue::I64(*(&(__item).attempt))), (::std::string::String::from("holder"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).holder))), (::std::string::String::from("actions"), ::ui_lang_guest::wire::SnapshotValue::I64(*(&(__item).actions))), (::std::string::String::from("degraded"), ::ui_lang_guest::wire::SnapshotValue::Bool(*(&(__item).degraded))), (::std::string::String::from("reason"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).reason))), (::std::string::String::from("output_ref"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).output_ref))), (::std::string::String::from("pr_number"), ::ui_lang_guest::wire::SnapshotValue::I64(*(&(__item).pr_number)))] }).collect())), (::std::string::String::from("journal"), ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("RunJournal"), fields: ::std::vec![(::std::string::String::from("dispatch_id"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&self.journal).dispatch_id))), (::std::string::String::from("entries"), ::ui_lang_guest::wire::SnapshotValue::List((&(&self.journal).entries).iter().map(|__item| ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("JournalEntry"), fields: ::std::vec![(::std::string::String::from("height"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).height))), (::std::string::String::from("kind"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).kind))), (::std::string::String::from("summary"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).summary))), (::std::string::String::from("status"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).status))), (::std::string::String::from("targets"), ::ui_lang_guest::wire::SnapshotValue::List((&(__item).targets).iter().map(|__item| ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("RunLink"), fields: ::std::vec![(::std::string::String::from("relation"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).relation))), (::std::string::String::from("kind"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).kind))), (::std::string::String::from("label"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).label))), (::std::string::String::from("url"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).url)))] }).collect()))] }).collect())), (::std::string::String::from("links"), ::ui_lang_guest::wire::SnapshotValue::List((&(&self.journal).links).iter().map(|__item| ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("RunLink"), fields: ::std::vec![(::std::string::String::from("relation"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).relation))), (::std::string::String::from("kind"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).kind))), (::std::string::String::from("label"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).label))), (::std::string::String::from("url"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).url)))] }).collect()))] }), (::std::string::String::from("live"), ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("LiveRun"), fields: ::std::vec![(::std::string::String::from("present"), ::ui_lang_guest::wire::SnapshotValue::Bool(*(&(&self.live).present))), (::std::string::String::from("status"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&self.live).status))), (::std::string::String::from("activity"), ::ui_lang_guest::wire::SnapshotValue::List((&(&self.live).activity).iter().map(|__item| ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("LiveActivity"), fields: ::std::vec![(::std::string::String::from("label"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).label))), (::std::string::String::from("done"), ::ui_lang_guest::wire::SnapshotValue::Bool(*(&(__item).done)))] }).collect())), (::std::string::String::from("answer_preview"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&self.live).answer_preview)))] }), (::std::string::String::from("panel"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.panel))), (::std::string::String::from("open_run"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.open_run))), (::std::string::String::from("journal_width"), ::ui_lang_guest::wire::SnapshotValue::F64(*(&self.journal_width))), (::std::string::String::from("editor_width"), ::ui_lang_guest::wire::SnapshotValue::F64(*(&self.editor_width))), (::std::string::String::from("viewport_width"), ::ui_lang_guest::wire::SnapshotValue::F64(*(&self.viewport_width))), (::std::string::String::from("expanded_receipt"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.expanded_receipt))), (::std::string::String::from("open_row"), ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("RunRow"), fields: ::std::vec![(::std::string::String::from("run_id"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&self.open_row).run_id))), (::std::string::String::from("dispatch_id"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&self.open_row).dispatch_id))), (::std::string::String::from("agent_id"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&self.open_row).agent_id))), (::std::string::String::from("agent_name"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&self.open_row).agent_name))), (::std::string::String::from("origin"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&self.open_row).origin))), (::std::string::String::from("state"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&self.open_row).state))), (::std::string::String::from("dispatched"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&self.open_row).dispatched))), (::std::string::String::from("settled"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&self.open_row).settled))), (::std::string::String::from("attempt"), ::ui_lang_guest::wire::SnapshotValue::I64(*(&(&self.open_row).attempt))), (::std::string::String::from("holder"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&self.open_row).holder))), (::std::string::String::from("actions"), ::ui_lang_guest::wire::SnapshotValue::I64(*(&(&self.open_row).actions))), (::std::string::String::from("degraded"), ::ui_lang_guest::wire::SnapshotValue::Bool(*(&(&self.open_row).degraded))), (::std::string::String::from("reason"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&self.open_row).reason))), (::std::string::String::from("output_ref"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&self.open_row).output_ref))), (::std::string::String::from("pr_number"), ::ui_lang_guest::wire::SnapshotValue::I64(*(&(&self.open_row).pr_number)))] }), (::std::string::String::from("opened"), ::ui_lang_guest::wire::SnapshotValue::I64(*(&self.opened))), (::std::string::String::from("capabilities"), ::ui_lang_guest::wire::SnapshotValue::List((&self.capabilities).iter().map(|__item| ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(__item))).collect())), (::std::string::String::from("account"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.account))), (::std::string::String::from("committed"), ::ui_lang_guest::wire::SnapshotValue::I64(*(&self.committed))), (::std::string::String::from("seeded"), ::ui_lang_guest::wire::SnapshotValue::I64(*(&self.seeded))), (::std::string::String::from("connected"), ::ui_lang_guest::wire::SnapshotValue::Bool(*(&self.connected))), (::std::string::String::from("connection_serial"), ::ui_lang_guest::wire::SnapshotValue::I64(*(&self.connection_serial))), (::std::string::String::from("answered"), ::ui_lang_guest::wire::SnapshotValue::Bool(*(&self.answered))), (::std::string::String::from("host_error"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.host_error))), (::std::string::String::from("selected"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.selected))), (::std::string::String::from("creating"), ::ui_lang_guest::wire::SnapshotValue::Bool(*(&self.creating))), (::std::string::String::from("can_edit"), ::ui_lang_guest::wire::SnapshotValue::Bool(*(&self.can_edit))), (::std::string::String::from("selected_status"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.selected_status))), (::std::string::String::from("draft_id"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.draft_id))), (::std::string::String::from("draft_name"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.draft_name))), (::std::string::String::from("draft_capability"), ::ui_lang_guest::wire::SnapshotValue::Option((&self.draft_capability).as_ref().map(|__item| ::std::boxed::Box::new(::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(__item)))))), (::std::string::String::from("draft_skills"), ::ui_lang_guest::wire::SnapshotValue::List((&self.draft_skills).iter().map(|__item| ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("AgentSkill"), fields: ::std::vec![(::std::string::String::from("name"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).name))), (::std::string::String::from("source_prefix"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).source_prefix))), (::std::string::String::from("source_snapshot"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).source_snapshot))), (::std::string::String::from("always"), ::ui_lang_guest::wire::SnapshotValue::Bool(*(&(__item).always)))] }).collect())), (::std::string::String::from("skill_name"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.skill_name))), (::std::string::String::from("skill_prefix"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.skill_prefix))), (::std::string::String::from("skill_snapshot"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.skill_snapshot))), (::std::string::String::from("skill_always"), ::ui_lang_guest::wire::SnapshotValue::Bool(*(&self.skill_always))), (::std::string::String::from("sent"), ::ui_lang_guest::wire::SnapshotValue::Bool(*(&self.sent)))] }}.encode() }
pub(crate) fn __restore(__bytes: &[u8]) -> ::std::result::Result<Self, ::std::string::String> { let __snapshot = ::ui_lang_guest::wire::Snapshot::decode(__bytes)?; if __snapshot.schema != Self::__SNAPSHOT_SCHEMA { return ::std::result::Result::Err(::std::string::String::from("snapshot schema mismatch")); } let __value = __snapshot.state; ((|| { let ::ui_lang_guest::wire::SnapshotValue::Record {name: __name, fields: __fields} = __value else { return ::std::option::Option::None; }; if __name != "AgentsView" || __fields.len() != 34 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __value) = __fields.next()?; if __name != "active_palette" { return ::std::option::Option::None; } let active_palette: AppTheme = ((|| { let ::ui_lang_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __value else { return ::std::option::Option::None; }; if __name != "AppTheme" || __fields.len() != 1 { return ::std::option::Option::None; } let (__variant, __payload) = __fields.into_iter().next()?; match __variant.as_str() { "app" => matches!(__payload, ::ui_lang_guest::wire::SnapshotValue::Unit).then_some(AppTheme::App), "app_dark" => matches!(__payload, ::ui_lang_guest::wire::SnapshotValue::Unit).then_some(AppTheme::AppDark), _ => ::std::option::Option::None } })())?; let (__name, __value) = __fields.next()?; if __name != "rows" { return ::std::option::Option::None; } let rows: ::std::vec::Vec<crate::host::AgentRow> = (match __value { ::ui_lang_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| (|| { let ::ui_lang_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __item else { return ::std::option::Option::None; }; if __name != "AgentRow" || __fields.len() != 9 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "id" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "name" { return ::std::option::Option::None; } let (__name, __field_2) = __fields.next()?; if __name != "initials" { return ::std::option::Option::None; } let (__name, __field_3) = __fields.next()?; if __name != "capability" { return ::std::option::Option::None; } let (__name, __field_4) = __fields.next()?; if __name != "status" { return ::std::option::Option::None; } let (__name, __field_5) = __fields.next()?; if __name != "owner_handle" { return ::std::option::Option::None; } let (__name, __field_6) = __fields.next()?; if __name != "controller" { return ::std::option::Option::None; } let (__name, __field_7) = __fields.next()?; if __name != "live" { return ::std::option::Option::None; } let (__name, __field_8) = __fields.next()?; if __name != "skills" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::AgentRow { id: (match __field_0 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, name: (match __field_1 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, initials: (match __field_2 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, capability: (match __field_3 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, status: (match __field_4 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, owner_handle: (match __field_5 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, controller: (match __field_6 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, live: (match __field_7 { ::ui_lang_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, skills: (match __field_8 { ::ui_lang_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| (|| { let ::ui_lang_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __item else { return ::std::option::Option::None; }; if __name != "AgentSkill" || __fields.len() != 4 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "name" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "source_prefix" { return ::std::option::Option::None; } let (__name, __field_2) = __fields.next()?; if __name != "source_snapshot" { return ::std::option::Option::None; } let (__name, __field_3) = __fields.next()?; if __name != "always" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::AgentSkill { name: (match __field_0 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, source_prefix: (match __field_1 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, source_snapshot: (match __field_2 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, always: (match __field_3 { ::ui_lang_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })()).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })? }) })()).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "runs" { return ::std::option::Option::None; } let runs: ::std::vec::Vec<crate::host::RunRow> = (match __value { ::ui_lang_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| (|| { let ::ui_lang_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __item else { return ::std::option::Option::None; }; if __name != "RunRow" || __fields.len() != 15 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "run_id" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "dispatch_id" { return ::std::option::Option::None; } let (__name, __field_2) = __fields.next()?; if __name != "agent_id" { return ::std::option::Option::None; } let (__name, __field_3) = __fields.next()?; if __name != "agent_name" { return ::std::option::Option::None; } let (__name, __field_4) = __fields.next()?; if __name != "origin" { return ::std::option::Option::None; } let (__name, __field_5) = __fields.next()?; if __name != "state" { return ::std::option::Option::None; } let (__name, __field_6) = __fields.next()?; if __name != "dispatched" { return ::std::option::Option::None; } let (__name, __field_7) = __fields.next()?; if __name != "settled" { return ::std::option::Option::None; } let (__name, __field_8) = __fields.next()?; if __name != "attempt" { return ::std::option::Option::None; } let (__name, __field_9) = __fields.next()?; if __name != "holder" { return ::std::option::Option::None; } let (__name, __field_10) = __fields.next()?; if __name != "actions" { return ::std::option::Option::None; } let (__name, __field_11) = __fields.next()?; if __name != "degraded" { return ::std::option::Option::None; } let (__name, __field_12) = __fields.next()?; if __name != "reason" { return ::std::option::Option::None; } let (__name, __field_13) = __fields.next()?; if __name != "output_ref" { return ::std::option::Option::None; } let (__name, __field_14) = __fields.next()?; if __name != "pr_number" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::RunRow { run_id: (match __field_0 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, dispatch_id: (match __field_1 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, agent_id: (match __field_2 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, agent_name: (match __field_3 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, origin: (match __field_4 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, state: (match __field_5 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, dispatched: (match __field_6 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, settled: (match __field_7 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, attempt: (match __field_8 { ::ui_lang_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, holder: (match __field_9 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, actions: (match __field_10 { ::ui_lang_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, degraded: (match __field_11 { ::ui_lang_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, reason: (match __field_12 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, output_ref: (match __field_13 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, pr_number: (match __field_14 { ::ui_lang_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })()).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "journal" { return ::std::option::Option::None; } let journal: crate::host::RunJournal = ((|| { let ::ui_lang_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __value else { return ::std::option::Option::None; }; if __name != "RunJournal" || __fields.len() != 3 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "dispatch_id" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "entries" { return ::std::option::Option::None; } let (__name, __field_2) = __fields.next()?; if __name != "links" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::RunJournal { dispatch_id: (match __field_0 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, entries: (match __field_1 { ::ui_lang_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| (|| { let ::ui_lang_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __item else { return ::std::option::Option::None; }; if __name != "JournalEntry" || __fields.len() != 5 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "height" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "kind" { return ::std::option::Option::None; } let (__name, __field_2) = __fields.next()?; if __name != "summary" { return ::std::option::Option::None; } let (__name, __field_3) = __fields.next()?; if __name != "status" { return ::std::option::Option::None; } let (__name, __field_4) = __fields.next()?; if __name != "targets" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::JournalEntry { height: (match __field_0 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, kind: (match __field_1 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, summary: (match __field_2 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, status: (match __field_3 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, targets: (match __field_4 { ::ui_lang_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| (|| { let ::ui_lang_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __item else { return ::std::option::Option::None; }; if __name != "RunLink" || __fields.len() != 4 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "relation" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "kind" { return ::std::option::Option::None; } let (__name, __field_2) = __fields.next()?; if __name != "label" { return ::std::option::Option::None; } let (__name, __field_3) = __fields.next()?; if __name != "url" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::RunLink { relation: (match __field_0 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, kind: (match __field_1 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, label: (match __field_2 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, url: (match __field_3 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })()).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })? }) })()).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })?, links: (match __field_2 { ::ui_lang_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| (|| { let ::ui_lang_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __item else { return ::std::option::Option::None; }; if __name != "RunLink" || __fields.len() != 4 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "relation" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "kind" { return ::std::option::Option::None; } let (__name, __field_2) = __fields.next()?; if __name != "label" { return ::std::option::Option::None; } let (__name, __field_3) = __fields.next()?; if __name != "url" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::RunLink { relation: (match __field_0 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, kind: (match __field_1 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, label: (match __field_2 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, url: (match __field_3 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })()).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })? }) })())?; let (__name, __value) = __fields.next()?; if __name != "live" { return ::std::option::Option::None; } let live: crate::host::LiveRun = ((|| { let ::ui_lang_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __value else { return ::std::option::Option::None; }; if __name != "LiveRun" || __fields.len() != 4 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "present" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "status" { return ::std::option::Option::None; } let (__name, __field_2) = __fields.next()?; if __name != "activity" { return ::std::option::Option::None; } let (__name, __field_3) = __fields.next()?; if __name != "answer_preview" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::LiveRun { present: (match __field_0 { ::ui_lang_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, status: (match __field_1 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, activity: (match __field_2 { ::ui_lang_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| (|| { let ::ui_lang_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __item else { return ::std::option::Option::None; }; if __name != "LiveActivity" || __fields.len() != 2 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "label" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "done" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::LiveActivity { label: (match __field_0 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, done: (match __field_1 { ::ui_lang_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })()).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })?, answer_preview: (match __field_3 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })())?; let (__name, __value) = __fields.next()?; if __name != "panel" { return ::std::option::Option::None; } let panel: ::std::string::String = (match __value { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "open_run" { return ::std::option::Option::None; } let open_run: ::std::string::String = (match __value { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "journal_width" { return ::std::option::Option::None; } let journal_width: f64 = (match __value { ::ui_lang_guest::wire::SnapshotValue::F64(__item) if __item.is_finite() => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "editor_width" { return ::std::option::Option::None; } let editor_width: f64 = (match __value { ::ui_lang_guest::wire::SnapshotValue::F64(__item) if __item.is_finite() => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "viewport_width" { return ::std::option::Option::None; } let viewport_width: f64 = (match __value { ::ui_lang_guest::wire::SnapshotValue::F64(__item) if __item.is_finite() => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "expanded_receipt" { return ::std::option::Option::None; } let expanded_receipt: ::std::string::String = (match __value { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "open_row" { return ::std::option::Option::None; } let open_row: crate::host::RunRow = ((|| { let ::ui_lang_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __value else { return ::std::option::Option::None; }; if __name != "RunRow" || __fields.len() != 15 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "run_id" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "dispatch_id" { return ::std::option::Option::None; } let (__name, __field_2) = __fields.next()?; if __name != "agent_id" { return ::std::option::Option::None; } let (__name, __field_3) = __fields.next()?; if __name != "agent_name" { return ::std::option::Option::None; } let (__name, __field_4) = __fields.next()?; if __name != "origin" { return ::std::option::Option::None; } let (__name, __field_5) = __fields.next()?; if __name != "state" { return ::std::option::Option::None; } let (__name, __field_6) = __fields.next()?; if __name != "dispatched" { return ::std::option::Option::None; } let (__name, __field_7) = __fields.next()?; if __name != "settled" { return ::std::option::Option::None; } let (__name, __field_8) = __fields.next()?; if __name != "attempt" { return ::std::option::Option::None; } let (__name, __field_9) = __fields.next()?; if __name != "holder" { return ::std::option::Option::None; } let (__name, __field_10) = __fields.next()?; if __name != "actions" { return ::std::option::Option::None; } let (__name, __field_11) = __fields.next()?; if __name != "degraded" { return ::std::option::Option::None; } let (__name, __field_12) = __fields.next()?; if __name != "reason" { return ::std::option::Option::None; } let (__name, __field_13) = __fields.next()?; if __name != "output_ref" { return ::std::option::Option::None; } let (__name, __field_14) = __fields.next()?; if __name != "pr_number" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::RunRow { run_id: (match __field_0 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, dispatch_id: (match __field_1 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, agent_id: (match __field_2 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, agent_name: (match __field_3 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, origin: (match __field_4 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, state: (match __field_5 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, dispatched: (match __field_6 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, settled: (match __field_7 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, attempt: (match __field_8 { ::ui_lang_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, holder: (match __field_9 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, actions: (match __field_10 { ::ui_lang_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, degraded: (match __field_11 { ::ui_lang_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, reason: (match __field_12 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, output_ref: (match __field_13 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, pr_number: (match __field_14 { ::ui_lang_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })())?; let (__name, __value) = __fields.next()?; if __name != "opened" { return ::std::option::Option::None; } let opened: i64 = (match __value { ::ui_lang_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "capabilities" { return ::std::option::Option::None; } let capabilities: ::std::vec::Vec<::std::string::String> = (match __value { ::ui_lang_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| match __item { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None }).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "account" { return ::std::option::Option::None; } let account: ::std::string::String = (match __value { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "committed" { return ::std::option::Option::None; } let committed: i64 = (match __value { ::ui_lang_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "seeded" { return ::std::option::Option::None; } let seeded: i64 = (match __value { ::ui_lang_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "connected" { return ::std::option::Option::None; } let connected: bool = (match __value { ::ui_lang_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "connection_serial" { return ::std::option::Option::None; } let connection_serial: i64 = (match __value { ::ui_lang_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "answered" { return ::std::option::Option::None; } let answered: bool = (match __value { ::ui_lang_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "host_error" { return ::std::option::Option::None; } let host_error: ::std::string::String = (match __value { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "selected" { return ::std::option::Option::None; } let selected: ::std::string::String = (match __value { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "creating" { return ::std::option::Option::None; } let creating: bool = (match __value { ::ui_lang_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "can_edit" { return ::std::option::Option::None; } let can_edit: bool = (match __value { ::ui_lang_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "selected_status" { return ::std::option::Option::None; } let selected_status: ::std::string::String = (match __value { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "draft_id" { return ::std::option::Option::None; } let draft_id: ::std::string::String = (match __value { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "draft_name" { return ::std::option::Option::None; } let draft_name: ::std::string::String = (match __value { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "draft_capability" { return ::std::option::Option::None; } let draft_capability: ::std::option::Option<::std::string::String> = (match __value { ::ui_lang_guest::wire::SnapshotValue::Option(::std::option::Option::None) => ::std::option::Option::Some(::std::option::Option::None), ::ui_lang_guest::wire::SnapshotValue::Option(::std::option::Option::Some(__item)) => (match *__item { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None }).map(::std::option::Option::Some), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "draft_skills" { return ::std::option::Option::None; } let draft_skills: ::std::vec::Vec<crate::host::AgentSkill> = (match __value { ::ui_lang_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| (|| { let ::ui_lang_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __item else { return ::std::option::Option::None; }; if __name != "AgentSkill" || __fields.len() != 4 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "name" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "source_prefix" { return ::std::option::Option::None; } let (__name, __field_2) = __fields.next()?; if __name != "source_snapshot" { return ::std::option::Option::None; } let (__name, __field_3) = __fields.next()?; if __name != "always" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::AgentSkill { name: (match __field_0 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, source_prefix: (match __field_1 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, source_snapshot: (match __field_2 { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, always: (match __field_3 { ::ui_lang_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })()).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "skill_name" { return ::std::option::Option::None; } let skill_name: ::std::string::String = (match __value { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "skill_prefix" { return ::std::option::Option::None; } let skill_prefix: ::std::string::String = (match __value { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "skill_snapshot" { return ::std::option::Option::None; } let skill_snapshot: ::std::string::String = (match __value { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "skill_always" { return ::std::option::Option::None; } let skill_always: bool = (match __value { ::ui_lang_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "sent" { return ::std::option::Option::None; } let sent: bool = (match __value { ::ui_lang_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; ::std::option::Option::Some(Self::__restore_state(active_palette, rows, runs, journal, live, panel, open_run, journal_width, editor_width, viewport_width, expanded_receipt, open_row, opened, capabilities, account, committed, seeded, connected, connection_serial, answered, host_error, selected, creating, can_edit, selected_status, draft_id, draft_name, draft_capability, draft_skills, skill_name, skill_prefix, skill_snapshot, skill_always, sent)) })()).ok_or_else(|| ::std::string::String::from("snapshot state mismatch")) }
}
}
__ice_generated_items_4167656e747356696577! {
#[allow(unused_parens)]
impl AgentsView {
}
}
__ice_generated_items_4167656e747356696577! {
#[allow(unused_parens)]
impl AgentsView {
fn __subscription(&self) -> ::iced::Subscription<__AgentsViewMessage> {
::iced::Subscription::batch([
crate::host::session().map(move |__value| __AgentsViewMessage::SessionArrived(__value)),
if self.connected { ::iced::Subscription::batch([crate::host::register(self.connection_serial).map(move |__value| __AgentsViewMessage::RegisterArrived(__value)),
]) } else { ::iced::Subscription::none() },
if (self.connected && (!(self.open_run).is_empty())) { ::iced::Subscription::batch([crate::host::run_journal(self.open_run.to_owned(), self.connection_serial).map(move |__value| __AgentsViewMessage::JournalArrived(__value)),
]) } else { ::iced::Subscription::none() },
if (self.connected && (!(self.open_run).is_empty())) { ::iced::Subscription::batch([crate::host::live_run(self.open_run.to_owned(), self.connection_serial).map(move |__value| __AgentsViewMessage::LiveArrived(__value)),
]) } else { ::iced::Subscription::none() },
crate::host::acts().map(move |__value| __AgentsViewMessage::ActDone(__value)),
])
}
}
}
__ice_generated_items_4167656e747356696577! {
#[allow(unused_parens)]
impl AgentsView {
}
#[cfg(test)] mod __ice_tests { use super::*;
#[test]
fn __ice_view_fits_default_stack() {
::std::thread::Builder::new().stack_size(4 * 1024 * 1024).spawn(|| {
let (__app, _) = AgentsView::__boot();
let _ = __app.__view();
}).unwrap().join().unwrap();
}
}
}
#[allow(warnings, clippy::all)]
mod __ice_group_app_8557f58d {
use super::*;
impl super::AgentsView {
pub(super) fn __ice_component_use_0(&self, __ice_palette: __IcePalette, __ice_use_scope: ::std::string::String, __ice_arg_0: crate::host::RunLink) -> __IceElement<'_, __AgentsViewMessage> { let __component_content: __IceElement<'_, __AgentsViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1393, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: true, key: format!("{}/@container:1393", __ice_use_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (4.0) as f32, right: (9.0) as f32, bottom: (4.0) as f32, left: (9.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((13.0) as f32).max(0.0).min(f32::MAX), ((13.0) as f32).max(0.0).min(f32::MAX), ((13.0) as f32).max(0.0).min(f32::MAX), ((13.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1400, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1401, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:1401", __ice_use_scope), size: ::std::option::Option::Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Medium }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (crate::host::link_glyph(::std::convert::AsRef::as_ref(&(__ice_arg_0.kind)))).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (__ice_arg_0.relation == "from") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1407, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:1407", __ice_use_scope), size: ::std::option::Option::Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[72]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Medium }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("from".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1412, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:1412", __ice_use_scope), size: ::std::option::Option::Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Medium }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: (__ice_arg_0.label.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:1400", __ice_use_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((6.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; __component_content }
}
}

#[allow(warnings, clippy::all)]
mod __ice_group_app_update {
use super::*;
impl super::AgentsView {
#[allow(clippy::assign_op_pattern)]
pub(super) fn __update(&mut self, message: __AgentsViewMessage) -> ::iced::Task<__AgentsViewMessage> {
#[cfg(all(target_os = "windows", not(test)))]
if !self.__ice_accessibility.is_attached() && !matches!(&message, __AgentsViewMessage::__AccessibilityNativeWindow(_)) {
self.__ice_accessibility_pending.push(message);
return ::iced::Task::none();
}
let __task = match message {
__AgentsViewMessage::__AccessibilitySnapshot(__snapshot) => { self.__ice_accessibility.update(*__snapshot); return ::iced::Task::none(); },
__AgentsViewMessage::__AccessibilityAction(__request) => { let __refresh = matches!(__request.action, ::ui_lang_runtime::Action::Focus); let __task = self.__ice_accessibility.dispatch(__request); return if __refresh { __task.chain(::ui_lang_runtime::snapshot::<__AgentsViewMessage>("AgentsView").map(|__snapshot| __AgentsViewMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)))) } else { __task }; },
__AgentsViewMessage::__AccessibilityWindow(__id, __event) => { self.__ice_accessibility.window_event(__id, __event); return ::iced::Task::none(); },
#[cfg(all(target_os = "windows", not(test)))]
__AgentsViewMessage::__AccessibilityNativeWindow(__window) => {
let __id = __window.id();
if !self.__ice_accessibility.attach_window(__window) { return ::iced::Task::none(); }
let __restore = ::iced::window::set_mode(__id, ::iced::window::Mode::Windowed);
let __initial = self.__accessibility_initial_task();
let mut __pending = ::std::vec::Vec::new();
for __message in ::std::mem::take(&mut self.__ice_accessibility_pending) {
__pending.push(self.__update(__message));
}
let __pending = ::iced::Task::batch(__pending);
let __snapshot = ::ui_lang_runtime::snapshot::<__AgentsViewMessage>("AgentsView").map(|__snapshot| __AgentsViewMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)));
return __restore.chain(::iced::Task::batch([__initial, __pending, __snapshot]));
},
#[cfg(all(target_os = "macos", not(test)))]
__AgentsViewMessage::__AccessibilityNativeWindow(__window) => {
if !self.__ice_accessibility.attach_window(__window) { return ::iced::Task::none(); }
return ::ui_lang_runtime::snapshot::<__AgentsViewMessage>("AgentsView").map(|__snapshot| __AgentsViewMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)));
},
__AgentsViewMessage::__AccessibilityFocusNext(__window) => { return ::ui_lang_runtime::focus_next_in::<__AgentsViewMessage>(__window).chain(::ui_lang_runtime::snapshot::<__AgentsViewMessage>("AgentsView").map(|__snapshot| __AgentsViewMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)))); },
__AgentsViewMessage::__AccessibilityFocusPrevious(__window) => { return ::ui_lang_runtime::focus_previous_in::<__AgentsViewMessage>(__window).chain(::ui_lang_runtime::snapshot::<__AgentsViewMessage>("AgentsView").map(|__snapshot| __AgentsViewMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)))); },
__AgentsViewMessage::__TemplateChanged => { return ::iced::Task::none(); },
__AgentsViewMessage::JournalResized(dx, _dy) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("journal_resized", "src/ui/app.ice:157");
let _ = &dx;
let _ = &_dy;
{ let __ice_next = crate::host::journal_width_after_delta(self.journal_width, (-dx), self.viewport_width); if ::ui_lang_runtime::state_changed!(self.journal_width, __ice_next) { self.journal_width = __ice_next; self.__ice_rev[7] += 1; } }
::iced::Task::none()
})(),
__AgentsViewMessage::EditorResized(dx, _dy) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("editor_resized", "src/ui/app.ice:160");
let _ = &dx;
let _ = &_dy;
{ let __ice_next = crate::host::editor_width_after_delta(self.editor_width, (-dx), self.viewport_width); if ::ui_lang_runtime::state_changed!(self.editor_width, __ice_next) { self.editor_width = __ice_next; self.__ice_rev[8] += 1; } }
::iced::Task::none()
})(),
__AgentsViewMessage::ViewportChanged(width, _height) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("viewport_changed", "src/ui/app.ice:163");
let _ = &width;
let _ = &_height;
{ let __ice_next = width; if ::ui_lang_runtime::state_changed!(self.viewport_width, __ice_next) { self.viewport_width = __ice_next; self.__ice_rev[9] += 1; } }
{ let __ice_next = crate::host::journal_width_after_delta(self.journal_width, 0.0, width); if ::ui_lang_runtime::state_changed!(self.journal_width, __ice_next) { self.journal_width = __ice_next; self.__ice_rev[7] += 1; } }
{ let __ice_next = crate::host::editor_width_after_delta(self.editor_width, 0.0, width); if ::ui_lang_runtime::state_changed!(self.editor_width, __ice_next) { self.editor_width = __ice_next; self.__ice_rev[8] += 1; } }
::iced::Task::none()
})(),
__AgentsViewMessage::ToggleReceipt(value) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("toggle_receipt", "src/ui/app.ice:168");
let _ = &value;
{ let __ice_next = crate::host::pick_str((self.expanded_receipt != value), ::std::convert::AsRef::as_ref(&(value)), ::std::convert::AsRef::as_ref(&(""))); if ::ui_lang_runtime::state_changed!(self.expanded_receipt, __ice_next) { self.expanded_receipt = __ice_next; self.__ice_rev[10] += 1; } }
::iced::Task::none()
})(),
__AgentsViewMessage::SessionArrived(item) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("session_arrived", "src/ui/app.ice:178");
let _ = &item;
{ let __ice_next = item.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[20] += 1; } }
if (!(item.error).is_empty()) { return ::iced::Task::none(); }
let next = item.next.clone();
{ let __ice_next = crate::host::connection_serial_after(self.connected, next.connected, self.connection_serial); if ::ui_lang_runtime::state_changed!(self.connection_serial, __ice_next) { self.connection_serial = __ice_next; self.__ice_rev[18] += 1; } }
{ let __ice_next = next.connected; if ::ui_lang_runtime::state_changed!(self.connected, __ice_next) { self.connected = __ice_next; self.__ice_rev[17] += 1; } }
{ let __ice_next = next.account.to_owned(); if ::ui_lang_runtime::state_changed!(self.account, __ice_next) { self.account = __ice_next; self.__ice_rev[14] += 1; } }
{ let __ice_next = next.open_run.to_owned(); if ::ui_lang_runtime::state_changed!(self.open_run, __ice_next) { self.open_run = __ice_next; self.__ice_rev[6] += 1; } }
{ let __ice_next = crate::host::run_at(::std::convert::AsRef::as_ref(&(self.runs)), ::std::convert::AsRef::as_ref(&(self.open_run))); if ::ui_lang_runtime::state_changed!(self.open_row, __ice_next) { self.open_row = __ice_next; self.__ice_rev[11] += 1; } }
let door_pressed = ((next.opened != self.opened) && (!(next.open_run).is_empty()));
{ let __ice_next = next.opened; if ::ui_lang_runtime::state_changed!(self.opened, __ice_next) { self.opened = __ice_next; self.__ice_rev[12] += 1; } }
{ let __ice_next = crate::host::pick_str(door_pressed, ::std::convert::AsRef::as_ref(&("runs")), ::std::convert::AsRef::as_ref(&(self.panel))); if ::ui_lang_runtime::state_changed!(self.panel, __ice_next) { self.panel = __ice_next; self.__ice_rev[5] += 1; } }
let row = crate::host::row_named(::std::convert::AsRef::as_ref(&(self.rows)), ::std::convert::AsRef::as_ref(&(self.selected)));
{ let __ice_next = crate::host::editable(self.connected, ::std::convert::AsRef::as_ref(&(self.account)), ::std::convert::AsRef::as_ref(&(row.controller))); if ::ui_lang_runtime::state_changed!(self.can_edit, __ice_next) { self.can_edit = __ice_next; self.__ice_rev[23] += 1; } }
{ let __ice_next = AppTheme::App; if ::ui_lang_runtime::state_changed!(self.active_palette, __ice_next) { self.active_palette = __ice_next; self.__ice_rev[0] += 1; } }
if (!next.dark) { return ::iced::Task::none(); }
{ let __ice_next = AppTheme::AppDark; if ::ui_lang_runtime::state_changed!(self.active_palette, __ice_next) { self.active_palette = __ice_next; self.__ice_rev[0] += 1; } }
::iced::Task::none()
})(),
__AgentsViewMessage::RegisterArrived(item) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("register_arrived", "src/ui/app.ice:204");
let _ = &item;
{ let __ice_next = item.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[20] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.answered, __ice_next) { self.answered = __ice_next; self.__ice_rev[19] += 1; } }
if (!(item.error).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = item.rows.clone(); if ::ui_lang_runtime::state_changed!(self.rows, __ice_next) { self.rows = __ice_next; self.__ice_rev[1] += 1; } }
{ let __ice_next = item.runs.clone(); if ::ui_lang_runtime::state_changed!(self.runs, __ice_next) { self.runs = __ice_next; self.__ice_rev[2] += 1; } }
{ let __ice_next = item.capabilities.clone(); if ::ui_lang_runtime::state_changed!(self.capabilities, __ice_next) { self.capabilities = __ice_next; self.__ice_rev[13] += 1; } }
{ let __ice_next = crate::host::run_at(::std::convert::AsRef::as_ref(&(self.runs)), ::std::convert::AsRef::as_ref(&(self.open_run))); if ::ui_lang_runtime::state_changed!(self.open_row, __ice_next) { self.open_row = __ice_next; self.__ice_rev[11] += 1; } }
{ let __ice_next = ({ let __ice_call = ::ui_lang_runtime::dev::Span::extern_call("badge", "src/ui/app.ice:50"); crate::host::badge(crate::host::working_agents(::std::convert::AsRef::as_ref(&(self.rows)))) }); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[33] += 1; } }
let consumed = crate::host::drafts_consumed(self.committed, self.seeded, self.creating, ::std::convert::AsRef::as_ref(&(self.rows)), ::std::convert::AsRef::as_ref(&(self.draft_id)));
{ let __ice_next = self.committed; if ::ui_lang_runtime::state_changed!(self.seeded, __ice_next) { self.seeded = __ice_next; self.__ice_rev[16] += 1; } }
{ let __ice_next = crate::host::pick_str((consumed && self.creating), ::std::convert::AsRef::as_ref(&(self.draft_id)), ::std::convert::AsRef::as_ref(&(self.selected))); if ::ui_lang_runtime::state_changed!(self.selected, __ice_next) { self.selected = __ice_next; self.__ice_rev[21] += 1; } }
{ let __ice_next = (self.creating && (!consumed)); if ::ui_lang_runtime::state_changed!(self.creating, __ice_next) { self.creating = __ice_next; self.__ice_rev[22] += 1; } }
let row = crate::host::row_named(::std::convert::AsRef::as_ref(&(self.rows)), ::std::convert::AsRef::as_ref(&(self.selected)));
{ let __ice_next = crate::host::editable(self.connected, ::std::convert::AsRef::as_ref(&(self.account)), ::std::convert::AsRef::as_ref(&(row.controller))); if ::ui_lang_runtime::state_changed!(self.can_edit, __ice_next) { self.can_edit = __ice_next; self.__ice_rev[23] += 1; } }
{ let __ice_next = row.status.to_owned(); if ::ui_lang_runtime::state_changed!(self.selected_status, __ice_next) { self.selected_status = __ice_next; self.__ice_rev[24] += 1; } }
{ let __ice_next = crate::host::pick_str(consumed, ::std::convert::AsRef::as_ref(&(row.name)), ::std::convert::AsRef::as_ref(&(self.draft_name))); if ::ui_lang_runtime::state_changed!(self.draft_name, __ice_next) { self.draft_name = __ice_next; self.__ice_rev[26] += 1; } }
{ let __ice_next = crate::host::pick_capability(consumed, ::std::convert::AsRef::as_ref(&(row.capability)), ::std::borrow::Borrow::borrow(&(self.draft_capability))); if ::ui_lang_runtime::state_changed!(self.draft_capability, __ice_next) { self.draft_capability = __ice_next; self.__ice_rev[27] += 1; } }
{ let __ice_next = crate::host::pick_skills(consumed, ::std::convert::AsRef::as_ref(&(row.skills)), ::std::convert::AsRef::as_ref(&(self.draft_skills))); if ::ui_lang_runtime::state_changed!(self.draft_skills, __ice_next) { self.draft_skills = __ice_next; self.__ice_rev[28] += 1; } }
::iced::Task::none()
})(),
__AgentsViewMessage::JournalArrived(item) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("journal_arrived", "src/ui/app.ice:226");
let _ = &item;
{ let __ice_next = item.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[20] += 1; } }
if ((!(item.error).is_empty()) || (item.journal.dispatch_id != self.open_run)) { return ::iced::Task::none(); }
{ let __ice_next = item.journal.clone(); if ::ui_lang_runtime::state_changed!(self.journal, __ice_next) { self.journal = __ice_next; self.__ice_rev[3] += 1; } }
::iced::Task::none()
})(),
__AgentsViewMessage::LiveArrived(item) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("live_arrived", "src/ui/app.ice:233");
let _ = &item;
{ let __ice_next = item.clone(); if ::ui_lang_runtime::state_changed!(self.live, __ice_next) { self.live = __ice_next; self.__ice_rev[4] += 1; } }
::iced::Task::none()
})(),
__AgentsViewMessage::ActDone(item) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("act_done", "src/ui/app.ice:238");
let _ = &item;
{ let __ice_next = item.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[20] += 1; } }
if (!(item.error).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = (self.committed + 1); if ::ui_lang_runtime::state_changed!(self.committed, __ice_next) { self.committed = __ice_next; self.__ice_rev[15] += 1; } }
::iced::Task::none()
})(),
__AgentsViewMessage::OpenAgent(id) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("open_agent", "src/ui/app.ice:244");
let _ = &id;
let row = crate::host::row_named(::std::convert::AsRef::as_ref(&(self.rows)), ::std::convert::AsRef::as_ref(&(id)));
{ let __ice_next = id.to_owned(); if ::ui_lang_runtime::state_changed!(self.selected, __ice_next) { self.selected = __ice_next; self.__ice_rev[21] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.creating, __ice_next) { self.creating = __ice_next; self.__ice_rev[22] += 1; } }
{ let __ice_next = crate::host::editable(self.connected, ::std::convert::AsRef::as_ref(&(self.account)), ::std::convert::AsRef::as_ref(&(row.controller))); if ::ui_lang_runtime::state_changed!(self.can_edit, __ice_next) { self.can_edit = __ice_next; self.__ice_rev[23] += 1; } }
{ let __ice_next = row.status.to_owned(); if ::ui_lang_runtime::state_changed!(self.selected_status, __ice_next) { self.selected_status = __ice_next; self.__ice_rev[24] += 1; } }
{ let __ice_next = row.id.to_owned(); if ::ui_lang_runtime::state_changed!(self.draft_id, __ice_next) { self.draft_id = __ice_next; self.__ice_rev[25] += 1; } }
{ let __ice_next = row.name.to_owned(); if ::ui_lang_runtime::state_changed!(self.draft_name, __ice_next) { self.draft_name = __ice_next; self.__ice_rev[26] += 1; } }
{ let __ice_next = crate::host::some_str(::std::convert::AsRef::as_ref(&(row.capability))); if ::ui_lang_runtime::state_changed!(self.draft_capability, __ice_next) { self.draft_capability = __ice_next; self.__ice_rev[27] += 1; } }
{ let __ice_next = row.skills.clone(); if ::ui_lang_runtime::state_changed!(self.draft_skills, __ice_next) { self.draft_skills = __ice_next; self.__ice_rev[28] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.skill_name, __ice_next) { self.skill_name = __ice_next; self.__ice_rev[29] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.skill_prefix, __ice_next) { self.skill_prefix = __ice_next; self.__ice_rev[30] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.skill_snapshot, __ice_next) { self.skill_snapshot = __ice_next; self.__ice_rev[31] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.skill_always, __ice_next) { self.skill_always = __ice_next; self.__ice_rev[32] += 1; } }
::iced::Task::none()
})(),
__AgentsViewMessage::OpenNew => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("open_new", "src/ui/app.ice:260");
{ let __ice_next = "registry".to_owned(); if ::ui_lang_runtime::state_changed!(self.panel, __ice_next) { self.panel = __ice_next; self.__ice_rev[5] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.selected, __ice_next) { self.selected = __ice_next; self.__ice_rev[21] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.creating, __ice_next) { self.creating = __ice_next; self.__ice_rev[22] += 1; } }
{ let __ice_next = (self.connected && (!(self.account).is_empty())); if ::ui_lang_runtime::state_changed!(self.can_edit, __ice_next) { self.can_edit = __ice_next; self.__ice_rev[23] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.draft_id, __ice_next) { self.draft_id = __ice_next; self.__ice_rev[25] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.draft_name, __ice_next) { self.draft_name = __ice_next; self.__ice_rev[26] += 1; } }
{ let __ice_next = ::std::option::Option::None; if ::ui_lang_runtime::state_changed!(self.draft_capability, __ice_next) { self.draft_capability = __ice_next; self.__ice_rev[27] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.draft_skills, __ice_next) { self.draft_skills = __ice_next; self.__ice_rev[28] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.skill_name, __ice_next) { self.skill_name = __ice_next; self.__ice_rev[29] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.skill_prefix, __ice_next) { self.skill_prefix = __ice_next; self.__ice_rev[30] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.skill_snapshot, __ice_next) { self.skill_snapshot = __ice_next; self.__ice_rev[31] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.skill_always, __ice_next) { self.skill_always = __ice_next; self.__ice_rev[32] += 1; } }
::iced::Task::none()
})(),
__AgentsViewMessage::CloseEditor => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("close_editor", "src/ui/app.ice:274");
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.selected, __ice_next) { self.selected = __ice_next; self.__ice_rev[21] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.creating, __ice_next) { self.creating = __ice_next; self.__ice_rev[22] += 1; } }
::iced::Task::none()
})(),
__AgentsViewMessage::ChoosePanel(next) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("choose_panel", "src/ui/app.ice:279");
let _ = &next;
{ let __ice_next = next.to_owned(); if ::ui_lang_runtime::state_changed!(self.panel, __ice_next) { self.panel = __ice_next; self.__ice_rev[5] += 1; } }
::iced::Task::none()
})(),
__AgentsViewMessage::OpenRunRow(run_id) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("open_run_row", "src/ui/app.ice:284");
let _ = &run_id;
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.expanded_receipt, __ice_next) { self.expanded_receipt = __ice_next; self.__ice_rev[10] += 1; } }
{ let __ice_next = crate::host::run_named(::std::convert::AsRef::as_ref(&(self.runs)), ::std::convert::AsRef::as_ref(&(run_id))); if ::ui_lang_runtime::state_changed!(self.open_row, __ice_next) { self.open_row = __ice_next; self.__ice_rev[11] += 1; } }
{ let __ice_next = self.open_row.dispatch_id.to_owned(); if ::ui_lang_runtime::state_changed!(self.open_run, __ice_next) { self.open_run = __ice_next; self.__ice_rev[6] += 1; } }
{ let __ice_next = crate::host::open_run(::std::convert::AsRef::as_ref(&(self.open_row.dispatch_id))); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[33] += 1; } }
::iced::Task::none()
})(),
__AgentsViewMessage::CloseRun => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("close_run", "src/ui/app.ice:290");
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.expanded_receipt, __ice_next) { self.expanded_receipt = __ice_next; self.__ice_rev[10] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.open_run, __ice_next) { self.open_run = __ice_next; self.__ice_rev[6] += 1; } }
{ let __ice_next = crate::host::empty_run(); if ::ui_lang_runtime::state_changed!(self.open_row, __ice_next) { self.open_row = __ice_next; self.__ice_rev[11] += 1; } }
{ let __ice_next = crate::host::empty_journal(); if ::ui_lang_runtime::state_changed!(self.journal, __ice_next) { self.journal = __ice_next; self.__ice_rev[3] += 1; } }
{ let __ice_next = crate::host::empty_live(); if ::ui_lang_runtime::state_changed!(self.live, __ice_next) { self.live = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = crate::host::open_run(::std::convert::AsRef::as_ref(&(""))); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[33] += 1; } }
::iced::Task::none()
})(),
__AgentsViewMessage::OpenPlace(url) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("open_place", "src/ui/app.ice:299");
let _ = &url;
{ let __ice_next = crate::host::open_link(::std::convert::AsRef::as_ref(&(url))); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[33] += 1; } }
::iced::Task::none()
})(),
__AgentsViewMessage::PickCapabilityOption(value) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("pick_capability_option", "src/ui/app.ice:302");
let _ = &value;
{ let __ice_next = ::std::option::Option::Some(value.to_owned()); if ::ui_lang_runtime::state_changed!(self.draft_capability, __ice_next) { self.draft_capability = __ice_next; self.__ice_rev[27] += 1; } }
::iced::Task::none()
})(),
__AgentsViewMessage::SetSkillAlways(on) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("set_skill_always", "src/ui/app.ice:305");
let _ = &on;
{ let __ice_next = on; if ::ui_lang_runtime::state_changed!(self.skill_always, __ice_next) { self.skill_always = __ice_next; self.__ice_rev[32] += 1; } }
::iced::Task::none()
})(),
__AgentsViewMessage::AddSkill => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("add_skill", "src/ui/app.ice:308");
{ let __ice_next = crate::host::with_skill(::std::convert::AsRef::as_ref(&(self.draft_skills)), ::std::convert::AsRef::as_ref(&(self.skill_name)), ::std::convert::AsRef::as_ref(&(crate::host::pick_str(((self.skill_prefix).trim().to_owned()).is_empty(), ::std::convert::AsRef::as_ref(&(crate::host::library_prefix(::std::convert::AsRef::as_ref(&(self.skill_name))))), ::std::convert::AsRef::as_ref(&(self.skill_prefix))))), ::std::convert::AsRef::as_ref(&(self.skill_snapshot)), self.skill_always); if ::ui_lang_runtime::state_changed!(self.draft_skills, __ice_next) { self.draft_skills = __ice_next; self.__ice_rev[28] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.skill_name, __ice_next) { self.skill_name = __ice_next; self.__ice_rev[29] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.skill_prefix, __ice_next) { self.skill_prefix = __ice_next; self.__ice_rev[30] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.skill_snapshot, __ice_next) { self.skill_snapshot = __ice_next; self.__ice_rev[31] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.skill_always, __ice_next) { self.skill_always = __ice_next; self.__ice_rev[32] += 1; } }
::iced::Task::none()
})(),
__AgentsViewMessage::RemoveSkill(name) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("remove_skill", "src/ui/app.ice:315");
let _ = &name;
{ let __ice_next = crate::host::without_skill(::std::convert::AsRef::as_ref(&(self.draft_skills)), ::std::convert::AsRef::as_ref(&(name))); if ::ui_lang_runtime::state_changed!(self.draft_skills, __ice_next) { self.draft_skills = __ice_next; self.__ice_rev[28] += 1; } }
::iced::Task::none()
})(),
__AgentsViewMessage::LoadSkill(name, always) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("load_skill", "src/ui/app.ice:318");
let _ = &name;
let _ = &always;
{ let __ice_next = crate::host::skill_loaded(::std::convert::AsRef::as_ref(&(self.draft_skills)), ::std::convert::AsRef::as_ref(&(name)), always); if ::ui_lang_runtime::state_changed!(self.draft_skills, __ice_next) { self.draft_skills = __ice_next; self.__ice_rev[28] += 1; } }
::iced::Task::none()
})(),
__AgentsViewMessage::SetStatus(agent_id, paused) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("set_status", "src/ui/app.ice:321");
let _ = &agent_id;
let _ = &paused;
{ let __ice_next = ({ let __ice_call = ::ui_lang_runtime::dev::Span::extern_call("status", "src/ui/app.ice:82"); crate::host::status(::std::convert::AsRef::as_ref(&(agent_id)), paused) }); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[33] += 1; } }
::iced::Task::none()
})(),
__AgentsViewMessage::SubmitSave => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("submit_save", "src/ui/app.ice:324");
{ let __ice_next = ({ let __ice_call = ::ui_lang_runtime::dev::Span::extern_call("save", "src/ui/app.ice:83"); crate::host::save(::std::convert::AsRef::as_ref(&(self.selected)), ::std::convert::AsRef::as_ref(&(self.draft_name)), ::std::convert::AsRef::as_ref(&(crate::host::or_empty(::std::borrow::Borrow::borrow(&(self.draft_capability))))), ::std::convert::AsRef::as_ref(&(self.draft_skills))) }); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[33] += 1; } }
::iced::Task::none()
})(),
__AgentsViewMessage::SubmitRegister => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("submit_register", "src/ui/app.ice:327");
{ let __ice_next = crate::host::register_agent(::std::convert::AsRef::as_ref(&(self.draft_id)), ::std::convert::AsRef::as_ref(&(self.draft_name)), ::std::convert::AsRef::as_ref(&(crate::host::or_empty(::std::borrow::Borrow::borrow(&(self.draft_capability))))), ::std::convert::AsRef::as_ref(&(self.draft_skills))); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[33] += 1; } }
::iced::Task::none()
})(),
__AgentsViewMessage::__BindDraftId(value) => { { let __ice_next = value; if ::ui_lang_runtime::state_changed!(self.draft_id, __ice_next) { self.draft_id = __ice_next; self.__ice_rev[25] += 1; } } ::iced::Task::none() }
__AgentsViewMessage::__BindDraftName(value) => { { let __ice_next = value; if ::ui_lang_runtime::state_changed!(self.draft_name, __ice_next) { self.draft_name = __ice_next; self.__ice_rev[26] += 1; } } ::iced::Task::none() }
__AgentsViewMessage::__BindSkillName(value) => { { let __ice_next = value; if ::ui_lang_runtime::state_changed!(self.skill_name, __ice_next) { self.skill_name = __ice_next; self.__ice_rev[29] += 1; } } ::iced::Task::none() }
__AgentsViewMessage::__BindSkillPrefix(value) => { { let __ice_next = value; if ::ui_lang_runtime::state_changed!(self.skill_prefix, __ice_next) { self.skill_prefix = __ice_next; self.__ice_rev[30] += 1; } } ::iced::Task::none() }
__AgentsViewMessage::__BindSkillSnapshot(value) => { { let __ice_next = value; if ::ui_lang_runtime::state_changed!(self.skill_snapshot, __ice_next) { self.skill_snapshot = __ice_next; self.__ice_rev[31] += 1; } } ::iced::Task::none() }
};
// Snapshotting the widget tree after every message serves ONLY an attached
// assistive technology (and the test harness, which drives the app through
// this tree) — ungated it walked every widget, built a TreeUpdate nobody
// read, and scheduled a second frame per message.
let __accessibility = if cfg!(test) || ::ui_lang_runtime::accessibility_active() {
::ui_lang_runtime::snapshot::<__AgentsViewMessage>("AgentsView").map(|__snapshot| __AgentsViewMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)))
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
impl super::AgentsView {
pub(super) fn __view(&self) -> __IceElement<'_, __AgentsViewMessage> { let __ice_view = ::ui_lang_runtime::dev::Span::view("AgentsView", "src/ui/app.ice:331"); let __ice_palette = self.__palette(); let __ice_app_theme = Self::__app_theme(__ice_palette); let __ice_content: __IceElement<'_, __AgentsViewMessage> = {
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 331, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let __ice_node_scope = format!("{}/root", "AgentsView"); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[2]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 336, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 337, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Sensor { key: format!("{}/@sensor:337", __ice_node_scope), reset: ::std::option::Option::None, on_show: ::std::option::Option::Some(::ui_lang_guest::slots::handler::<(f32, f32), __AgentsViewMessage>(::std::boxed::Box::new({ let __route = {  move |__size: (f64, f64)| __AgentsViewMessage::ViewportChanged(__size.0, __size.1) }; move |__sent: (f32, f32)| ::std::option::Option::Some(__route((f64::from(__sent.0), f64::from(__sent.1)))) }))), on_resize: ::std::option::Option::Some(::ui_lang_guest::slots::handler::<(f32, f32), __AgentsViewMessage>(::std::boxed::Box::new({ let __route = {  move |__size: (f64, f64)| __AgentsViewMessage::ViewportChanged(__size.0, __size.1) }; move |__sent: (f32, f32)| ::std::option::Option::Some(__route((f64::from(__sent.0), f64::from(__sent.1)))) }))), on_hide: ::std::option::Option::None, anticipate: ::std::option::Option::None, delay: ::std::option::Option::None, child: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 338, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((0.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 339, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:339", __ice_node_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((56.0) as f32)), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (0.0) as f32, right: (22.0) as f32, bottom: (0.0) as f32, left: (22.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 344, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 350, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let __ice_node_scope = format!("{}/title", __ice_node_scope); ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: __ice_node_scope.clone(), size: ::std::option::Option::Some(((16.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Agents".to_owned()).to_string() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (self.panel == "registry") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 356, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let __ice_node_scope = format!("{}/meta", __ice_node_scope); ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: __ice_node_scope.clone(), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[72]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (crate::host::agents_summary(self.connected, ::std::convert::AsRef::as_ref(&(self.rows)))).to_string() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (self.panel == "runs") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 362, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let __ice_node_scope = format!("{}/runs-meta", __ice_node_scope); ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: __ice_node_scope.clone(), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[72]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (crate::host::runs_summary(::std::convert::AsRef::as_ref(&(self.runs)))).to_string() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 367, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if self.connected { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 377, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:377", __ice_node_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 384, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:384", __ice_node_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::None, font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Registry".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Registry".to_owned())), on_press: if ((self.panel == "registry")) { ::std::option::Option::None } else { ::std::option::Option::Some(::ui_lang_guest::slots::message(__AgentsViewMessage::ChoosePanel("registry".to_owned()))) }, width: ::std::option::Option::None, height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((28.0) as f32)), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((5.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[15]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(1.0), radius: ::std::option::Option::Some([8.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[6]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face::default(), hovered: ::std::option::Option::None, pressed: ::std::option::Option::None, disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if self.connected { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 386, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:386", __ice_node_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 393, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:393", __ice_node_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::None, font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Runs".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Runs".to_owned())), on_press: if ((self.panel == "runs")) { ::std::option::Option::None } else { ::std::option::Option::Some(::ui_lang_guest::slots::message(__AgentsViewMessage::ChoosePanel("runs".to_owned()))) }, width: ::std::option::Option::None, height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((28.0) as f32)), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((5.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[15]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(1.0), radius: ::std::option::Option::Some([8.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[6]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face::default(), hovered: ::std::option::Option::None, pressed: ::std::option::Option::None, disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (self.connected && (!(self.account).is_empty())) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 397, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:397", __ice_node_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 403, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:403", __ice_node_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::None, font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("New agent".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("New agent".to_owned())), on_press: ::std::option::Option::Some(::ui_lang_guest::slots::message(__AgentsViewMessage::OpenNew)), width: ::std::option::Option::None, height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((28.0) as f32)), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((5.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[12]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[13]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[40]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(1.0), radius: ::std::option::Option::Some([9.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[6]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face::default(), hovered: ::std::option::Option::None, pressed: ::std::option::Option::None, disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:344", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((10.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 404, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:404", __ice_node_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[60]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 409, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (self.panel == "registry") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 411, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:411", __ice_node_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (12.0) as f32, right: (22.0) as f32, bottom: (10.0) as f32, left: (22.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 417, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:417", __ice_node_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[70]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: (crate::host::pane_note(::std::convert::AsRef::as_ref(&(self.panel)))).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 422, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:422", __ice_node_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[60]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 427, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(self.host_error).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 429, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let __ice_node_scope = format!("{}/host-error", __ice_node_scope); ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: __ice_node_scope.clone(), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[20]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (self.host_error.to_owned()).to_string() } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!self.connected) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 433, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let __ice_node_scope = format!("{}/offline", __ice_node_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (22.0) as f32, right: (22.0) as f32, bottom: (22.0) as f32, left: (22.0) as f32 }), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 440, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 445, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:445", __ice_node_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((42.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((42.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((21.0) as f32).max(0.0).min(f32::MAX), ((21.0) as f32).max(0.0).min(f32::MAX), ((21.0) as f32).max(0.0).min(f32::MAX), ((21.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 455, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:455", __ice_node_scope), size: ::std::option::Option::Some(((20.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("◇".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 456, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:456", __ice_node_scope), size: ::std::option::Option::Some(((16.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Not connected".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 461, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:461", __ice_node_scope), size: ::std::option::Option::Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[70]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Click the network name in the titlebar to pick or reconnect a network.".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:440", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((7.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (self.connected && (self.panel == "runs")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 466, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); if (self.runs).is_empty() { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 468, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:468", __ice_node_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (22.0) as f32, right: (22.0) as f32, bottom: (22.0) as f32, left: (22.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 473, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let __ice_node_scope = format!("{}/no-runs", __ice_node_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (30.0) as f32, right: (30.0) as f32, bottom: (30.0) as f32, left: (30.0) as f32 }), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((12.0) as f32).max(0.0).min(f32::MAX), ((12.0) as f32).max(0.0).min(f32::MAX), ((12.0) as f32).max(0.0).min(f32::MAX), ((12.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 481, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:481", __ice_node_scope), size: ::std::option::Option::Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("No runs yet — every dispatch of an agent lands here with its journal.".to_owned()).to_string() };
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
}); } if (!(self.runs).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 486, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let __ice_node_scope = format!("{}/runs-body", __ice_node_scope); ::ui_lang_guest::wire::Node::Scroll { on_scroll: ::std::option::Option::None, virtual_rows: false, key: __ice_node_scope.clone(), direction: ::ui_lang_guest::wire::ScrollDirection::Vertical, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), bar_hidden: false, bar_width: ::std::option::Option::None, bar_margin: ::std::option::Option::None, scroller_width: ::std::option::Option::None, bar_spacing: ::std::option::Option::None, anchor_x: ::ui_lang_guest::wire::ScrollAnchor::Start, anchor_y: ::ui_lang_guest::wire::ScrollAnchor::Start, auto_scroll: (false), background: ::std::option::Option::None, border: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 491, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); for (__ice_index, run) in self.runs.iter().enumerate() { let __for_scope = format!("{}/@for:1018({})", __ice_node_scope, __ice_index); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 500, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 501, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:501", __for_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 506, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:506", __for_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (11.0) as f32, right: (14.0) as f32, bottom: (11.0) as f32, left: (14.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 513, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 518, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 519, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 524, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:524", __for_scope), size: ::std::option::Option::Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (run.agent_name.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 529, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:529", __for_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: (run.origin.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:519", __for_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 535, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 540, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:540", __for_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[72]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (run.dispatched.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 545, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:545", __for_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[72]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("·".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 550, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:550", __for_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Medium }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (run.actions).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 556, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:556", __for_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Medium }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("actions".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (run.pr_number > 0) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 563, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:563", __for_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[72]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("· PR #".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (run.pr_number > 0) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 569, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:569", __for_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[72]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (run.pr_number).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:535", __for_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((5.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:518", __for_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((3.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if ((run.state == "dispatched") || (run.state == "running")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 578, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:578", __for_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (3.0) as f32, right: (8.0) as f32, bottom: (3.0) as f32, left: (8.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[32]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[33]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 586, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 587, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:587", __for_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((5.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((5.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[34]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((2.5) as f32).max(0.0).min(f32::MAX), ((2.5) as f32).max(0.0).min(f32::MAX), ((2.5) as f32).max(0.0).min(f32::MAX), ((2.5) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 593, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 594, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:594", __for_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[30]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (run.state.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:586", __for_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((5.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (run.state == "accepted") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 601, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:601", __for_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (3.0) as f32, right: (8.0) as f32, bottom: (3.0) as f32, left: (8.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[27]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[28]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 609, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 610, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:610", __for_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((5.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((5.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[29]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((2.5) as f32).max(0.0).min(f32::MAX), ((2.5) as f32).max(0.0).min(f32::MAX), ((2.5) as f32).max(0.0).min(f32::MAX), ((2.5) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 616, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 617, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:617", __for_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[25]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (run.state.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:609", __for_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((5.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((run.state == "rejected") || (run.state == "failed")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 624, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:624", __for_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (3.0) as f32, right: (8.0) as f32, bottom: (3.0) as f32, left: (8.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[22]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[23]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 632, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:632", __for_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[20]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (run.state.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:513", __for_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((13.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from(run.run_id.to_owned())), on_press: ::std::option::Option::Some(::ui_lang_guest::slots::message(__AgentsViewMessage::OpenRunRow(run.run_id.to_owned()))), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((0.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::None, text: ::std::option::Option::None, border: ::std::option::Option::None }, hover_background: ::std::option::Option::None, pressed_background: ::std::option::Option::None, disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::None, focus_ring: ::std::option::Option::None, text_size: ::std::option::Option::Some(13.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[2]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::None, border: ::std::option::Option::None }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[57]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::None, border: ::std::option::Option::None }), pressed: ::std::option::Option::None, disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 640, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:640", __for_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[6]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 645, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:500", __for_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:491", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((11.0) as f32), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (18.0) as f32, right: (18.0) as f32, bottom: (18.0) as f32, left: (18.0) as f32 }), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(self.open_run).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 649, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let __ice_node_scope = format!("{}/journal-resize", __ice_node_scope); ::ui_lang_guest::wire::Node::ResizeHandle { key: __ice_node_scope.clone(), on_press: ::std::option::Option::None, on_release: ::std::option::Option::None, on_drag: ::std::option::Option::Some(::ui_lang_guest::slots::handler::<(f64, f64), __AgentsViewMessage>(::std::boxed::Box::new({ let __route = {  move |__delta: (f64, f64)| __AgentsViewMessage::JournalResized(__delta.0, __delta.1) }; move |__sent: (f64, f64)| ::std::option::Option::Some(__route(__sent)) }))), cursor: ::std::option::Option::Some(::ui_lang_guest::wire::mouse::Cursor::ResizingHorizontally), content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 650, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let __ice_node_scope = format!("{}/journal-divider", __ice_node_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((10.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[54]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 656, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:656", __ice_node_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((2.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[60]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 657, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((2.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
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
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 658, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let __ice_node_scope = format!("{}/journal", __ice_node_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((self.journal_width) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::None }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 665, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Scroll { on_scroll: ::std::option::Option::None, virtual_rows: false, key: format!("{}/@layout:665", __ice_node_scope), direction: ::ui_lang_guest::wire::ScrollDirection::Vertical, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), bar_hidden: false, bar_width: ::std::option::Option::None, bar_margin: ::std::option::Option::None, scroller_width: ::std::option::Option::None, bar_spacing: ::std::option::Option::None, anchor_x: ::ui_lang_guest::wire::ScrollAnchor::Start, anchor_y: ::ui_lang_guest::wire::ScrollAnchor::Start, auto_scroll: (false), background: ::std::option::Option::None, border: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 670, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 675, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 680, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:680", __ice_node_scope), size: ::std::option::Option::Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (self.open_row.agent_name.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 685, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:685", __ice_node_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (self.open_row.state.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 690, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 691, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:691", __ice_node_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 697, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:697", __ice_node_scope), size: ::std::option::Option::Some(((16.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("×".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Close journal".to_owned())), on_press: ::std::option::Option::Some(::ui_lang_guest::slots::message(__AgentsViewMessage::CloseRun)), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((24.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((24.0) as f32)), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((0.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::None, text: ::std::option::Option::None, border: ::std::option::Option::None }, hover_background: ::std::option::Option::None, pressed_background: ::std::option::Option::None, disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::None, focus_ring: ::std::option::Option::None, text_size: ::std::option::Option::Some(13.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face::default(), hovered: ::std::option::Option::None, pressed: ::std::option::Option::None, disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:675", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((10.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 698, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:698", __ice_node_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: (self.open_row.origin.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 710, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::Some((self.expanded_receipt == self.open_run)), description: ::std::option::Option::None, key: format!("{}/@button:710", __ice_node_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 716, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); if (self.expanded_receipt == self.open_run) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 718, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:718", __ice_node_scope), size: ::std::option::Option::Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("▾".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (self.expanded_receipt != self.open_run) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 720, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:720", __ice_node_scope), size: ::std::option::Option::Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("▸".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 721, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:721", __ice_node_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Details".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:716", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((6.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Run details".to_owned())), on_press: ::std::option::Option::Some(::ui_lang_guest::slots::message(__AgentsViewMessage::ToggleReceipt(self.open_run.to_owned()))), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((4.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = ::iced::Color::from_rgba8(0, 0, 0, 0.000000); ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([8.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face::default(), hovered: ::std::option::Option::None, pressed: ::std::option::Option::None, disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (self.expanded_receipt == self.open_run) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 723, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 724, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 725, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:725", __ice_node_scope), size: ::std::option::Option::Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[73]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((64.0) as f32)), align_x: ::std::option::Option::None, content: ("dispatch".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 731, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:731", __ice_node_scope), size: ::std::option::Option::Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: (self.open_run.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:724", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!(self.open_row.output_ref).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 738, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 739, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:739", __ice_node_scope), size: ::std::option::Option::Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[73]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((64.0) as f32)), align_x: ::std::option::Option::None, content: ("output".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 745, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:745", __ice_node_scope), size: ::std::option::Option::Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: (self.open_row.output_ref.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:738", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:723", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((4.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if self.live.present { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 757, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let __ice_node_scope = format!("{}/live", __ice_node_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (9.0) as f32, right: (12.0) as f32, bottom: (9.0) as f32, left: (12.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[32]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[33]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 766, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 767, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Medium, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:767", __ice_node_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Medium }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: (self.live.status.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); for (__ice_index, act) in self.live.activity.iter().enumerate() { let __for_scope = format!("{}/@for:1292({})", __ice_node_scope, __ice_index); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 774, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); if act.done { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 776, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:776", __for_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("✓".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!act.done) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 782, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:782", __for_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("…".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 787, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:787", __for_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: (act.label.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:774", __for_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((5.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(self.live.answer_preview).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 793, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:793", __ice_node_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: (self.live.answer_preview.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:766", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((4.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((self.journal.dispatch_id == self.open_run) && (!(self.journal.links).is_empty())) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 805, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:805", __ice_node_scope), size: ::std::option::Option::Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Relevant".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 810, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); for (__ice_index, link) in self.journal.links.iter().enumerate() { let __for_scope = format!("{}/@for:1333({})", __ice_node_scope, __ice_index); if (!(link.url).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 816, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:816", __for_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 822, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_0(__ice_palette, format!("{}/RunChip@1341", __for_scope), link.clone()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from(link.label.to_owned())), on_press: ::std::option::Option::Some(::ui_lang_guest::slots::message(__AgentsViewMessage::OpenPlace(link.url.to_owned()))), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((0.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[15]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(1.0), radius: ::std::option::Option::Some([8.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[6]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face::default(), hovered: ::std::option::Option::None, pressed: ::std::option::Option::None, disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (link.url).is_empty() { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 824, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_0(__ice_palette, format!("{}/RunChip@1343", __for_scope), link.clone()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:810", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((6.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(self.open_row.reason).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 826, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:826", __ice_node_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (9.0) as f32, right: (12.0) as f32, bottom: (9.0) as f32, left: (12.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[22]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[23]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 835, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:835", __ice_node_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[20]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: (self.open_row.reason.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 840, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:840", __ice_node_scope), size: ::std::option::Option::Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Journal".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (self.journal.dispatch_id != self.open_run) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 846, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:846", __ice_node_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[70]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Reading the journal…".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((self.journal.dispatch_id == self.open_run) && (self.journal.entries).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 851, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:851", __ice_node_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[70]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("This run's journal has no entries yet — the fold may still be catching up to the chain.".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (self.journal.dispatch_id == self.open_run) { for (__ice_index, entry) in self.journal.entries.iter().enumerate() { let __for_scope = format!("{}/@for:1376({})", __ice_node_scope, __ice_index); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 858, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 859, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:859", __for_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[72]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (entry.height.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 864, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 865, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:865", __for_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (entry.kind.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 871, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:871", __for_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: (entry.summary.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!(entry.status).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 877, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:877", __for_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (entry.status.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } for (__ice_index, target) in entry.targets.iter().enumerate() { let __for_scope = format!("{}/@for:1397({})", __for_scope, __ice_index); if (!(target.url).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 880, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:880", __for_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 886, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_0(__ice_palette, format!("{}/RunChip@1405", __for_scope), target.clone()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from(target.label.to_owned())), on_press: ::std::option::Option::Some(::ui_lang_guest::slots::message(__AgentsViewMessage::OpenPlace(target.url.to_owned()))), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((0.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[15]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(1.0), radius: ::std::option::Option::Some([8.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[6]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face::default(), hovered: ::std::option::Option::None, pressed: ::std::option::Option::None, disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (target.url).is_empty() { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 888, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_runtime::grow_stack(|| self.__ice_component_use_0(__ice_palette, format!("{}/RunChip@1407", __for_scope), target.clone()));
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:864", __for_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((2.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:858", __for_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:670", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((12.0) as f32), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (18.0) as f32, right: (18.0) as f32, bottom: (18.0) as f32, left: (18.0) as f32 }), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
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
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:466", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((((self.connected && (self.panel == "registry")) && (self.rows).is_empty()) && self.answered) && (!self.creating)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 890, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:890", __ice_node_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (22.0) as f32, right: (22.0) as f32, bottom: (22.0) as f32, left: (22.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 895, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let __ice_node_scope = format!("{}/empty", __ice_node_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (30.0) as f32, right: (30.0) as f32, bottom: (30.0) as f32, left: (30.0) as f32 }), align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((12.0) as f32).max(0.0).min(f32::MAX), ((12.0) as f32).max(0.0).min(f32::MAX), ((12.0) as f32).max(0.0).min(f32::MAX), ((12.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 903, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:903", __ice_node_scope), size: ::std::option::Option::Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("No model agents configured — models appear here with their capability and skills.".to_owned()).to_string() };
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
}); } if ((self.connected && (self.panel == "registry")) && ((!(self.rows).is_empty()) || self.creating)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 908, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); if (!(self.rows).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 910, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let __ice_node_scope = format!("{}/agents-body", __ice_node_scope); ::ui_lang_guest::wire::Node::Scroll { on_scroll: ::std::option::Option::None, virtual_rows: false, key: __ice_node_scope.clone(), direction: ::ui_lang_guest::wire::ScrollDirection::Vertical, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), bar_hidden: false, bar_width: ::std::option::Option::None, bar_margin: ::std::option::Option::None, scroller_width: ::std::option::Option::None, bar_spacing: ::std::option::Option::None, anchor_x: ::ui_lang_guest::wire::ScrollAnchor::Start, anchor_y: ::ui_lang_guest::wire::ScrollAnchor::Start, auto_scroll: (false), background: ::std::option::Option::None, border: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 915, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); for (__ice_index, agent) in self.rows.iter().enumerate() { let __for_scope = format!("{}/@for:1442({})", __ice_node_scope, __ice_index); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 924, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 925, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:925", __for_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 930, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:930", __for_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (13.0) as f32, right: (14.0) as f32, bottom: (13.0) as f32, left: (14.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 937, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 942, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:942", __for_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((34.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((34.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), align_y: ::std::option::Option::Some(::ui_lang_guest::wire::AlignY::Center), background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX), ((9.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 950, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:950", __for_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[38]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (agent.initials.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 956, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 957, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 962, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:962", __for_scope), size: ::std::option::Option::Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (agent.name.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 967, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:967", __for_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (2.0) as f32, right: (7.0) as f32, bottom: (2.0) as f32, left: (7.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX), ((5.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 973, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:973", __for_scope), size: ::std::option::Option::Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[13]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (agent.capability.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:957", __for_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 981, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 986, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:986", __for_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Medium }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (crate::host::skill_count(::std::convert::AsRef::as_ref(&(agent.skills)))).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 992, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:992", __for_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Medium }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("skills · owner".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (agent.owner_handle).is_empty() { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 999, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:999", __for_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[72]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Medium }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("unowned".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(agent.owner_handle).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1006, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:1006", __for_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[72]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Medium }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("@".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!(agent.owner_handle).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1013, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:1013", __for_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[72]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Medium }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (agent.owner_handle.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:981", __for_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((5.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:956", __for_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((3.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (agent.status == "active") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1022, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:1022", __for_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (3.0) as f32, right: (8.0) as f32, bottom: (3.0) as f32, left: (8.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[27]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[28]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1030, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1031, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:1031", __for_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((5.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((5.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[29]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((2.5) as f32).max(0.0).min(f32::MAX), ((2.5) as f32).max(0.0).min(f32::MAX), ((2.5) as f32).max(0.0).min(f32::MAX), ((2.5) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1037, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1038, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:1038", __for_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[25]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("ACTIVE".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:1030", __for_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((5.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (agent.status == "paused") { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1045, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:1045", __for_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (3.0) as f32, right: (8.0) as f32, bottom: (3.0) as f32, left: (8.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[32]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[33]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1053, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1054, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:1054", __for_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((5.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((5.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[34]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((2.5) as f32).max(0.0).min(f32::MAX), ((2.5) as f32).max(0.0).min(f32::MAX), ((2.5) as f32).max(0.0).min(f32::MAX), ((2.5) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1060, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1061, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:1061", __for_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[30]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("PAUSED".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:1053", __for_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((5.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((agent.status != "active") && (agent.status != "paused")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1068, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:1068", __for_scope), width: ::std::option::Option::None, height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (3.0) as f32, right: (8.0) as f32, bottom: (3.0) as f32, left: (8.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[32]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[33]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX), ((6.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1076, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1077, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:1077", __for_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((5.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((5.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[34]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([((2.5) as f32).max(0.0).min(f32::MAX), ((2.5) as f32).max(0.0).min(f32::MAX), ((2.5) as f32).max(0.0).min(f32::MAX), ((2.5) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1083, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1084, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:1084", __for_scope), size: ::std::option::Option::Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[30]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (agent.status.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:1076", __for_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((5.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::None, height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:937", __for_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((13.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from(agent.name.to_owned())), on_press: ::std::option::Option::Some(::ui_lang_guest::slots::message(__AgentsViewMessage::OpenAgent(agent.id.to_owned()))), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((0.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::None, text: ::std::option::Option::None, border: ::std::option::Option::None }, hover_background: ::std::option::Option::None, pressed_background: ::std::option::Option::None, disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::None, focus_ring: ::std::option::Option::None, text_size: ::std::option::Option::Some(13.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[2]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::None, border: ::std::option::Option::None }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[57]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::None, border: ::std::option::Option::None }), pressed: ::std::option::Option::None, disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1092, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:1092", __for_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[6]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1097, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:924", __for_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:915", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((11.0) as f32), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (18.0) as f32, right: (18.0) as f32, bottom: (18.0) as f32, left: (18.0) as f32 }), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((!(self.selected).is_empty()) || self.creating) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1102, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let __ice_node_scope = format!("{}/editor-resize", __ice_node_scope); ::ui_lang_guest::wire::Node::ResizeHandle { key: __ice_node_scope.clone(), on_press: ::std::option::Option::None, on_release: ::std::option::Option::None, on_drag: ::std::option::Option::Some(::ui_lang_guest::slots::handler::<(f64, f64), __AgentsViewMessage>(::std::boxed::Box::new({ let __route = {  move |__delta: (f64, f64)| __AgentsViewMessage::EditorResized(__delta.0, __delta.1) }; move |__sent: (f64, f64)| ::std::option::Option::Some(__route(__sent)) }))), cursor: ::std::option::Option::Some(::ui_lang_guest::wire::mouse::Cursor::ResizingHorizontally), content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1103, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let __ice_node_scope = format!("{}/editor-divider", __ice_node_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((10.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::None).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::None, snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1104, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((1.0) as f32)) };
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
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1105, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let __ice_node_scope = format!("{}/editor", __ice_node_scope); ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: __ice_node_scope.clone(), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((self.editor_width) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), padding: ::std::option::Option::None, align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::None }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1112, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Scroll { on_scroll: ::std::option::Option::None, virtual_rows: false, key: format!("{}/@layout:1112", __ice_node_scope), direction: ::ui_lang_guest::wire::ScrollDirection::Vertical, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), bar_hidden: false, bar_width: ::std::option::Option::None, bar_margin: ::std::option::Option::None, scroller_width: ::std::option::Option::None, bar_spacing: ::std::option::Option::None, anchor_x: ::ui_lang_guest::wire::ScrollAnchor::Start, anchor_y: ::ui_lang_guest::wire::ScrollAnchor::Start, auto_scroll: (false), background: ::std::option::Option::None, border: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1117, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1122, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); if self.creating { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1128, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:1128", __ice_node_scope), size: ::std::option::Option::Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("New agent".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!self.creating) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1134, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:1134", __ice_node_scope), size: ::std::option::Option::Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (self.draft_name.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1139, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1140, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:1140", __ice_node_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1146, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:1146", __ice_node_scope), size: ::std::option::Option::Some(((16.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("×".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Close editor".to_owned())), on_press: ::std::option::Option::Some(::ui_lang_guest::slots::message(__AgentsViewMessage::CloseEditor)), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((24.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((24.0) as f32)), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((0.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::None, text: ::std::option::Option::None, border: ::std::option::Option::None }, hover_background: ::std::option::Option::None, pressed_background: ::std::option::Option::None, disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::None, focus_ring: ::std::option::Option::None, text_size: ::std::option::Option::Some(13.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face::default(), hovered: ::std::option::Option::None, pressed: ::std::option::Option::None, disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:1122", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((10.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if ((!self.can_edit) && (!self.creating)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1148, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Container { shadow: ::ui_lang_guest::wire::Shadow { color: ::std::option::Option::None, x: ::std::option::Option::None, y: ::std::option::Option::None, blur: ::std::option::Option::None }, max_width: ::std::option::Option::None, max_height: ::std::option::Option::None, clip: false, key: format!("{}/@container:1148", __ice_node_scope), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (9.0) as f32, right: (12.0) as f32, bottom: (9.0) as f32, left: (12.0) as f32 }), align_x: ::std::option::Option::None, align_y: ::std::option::Option::None, background: (::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[32]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) })).map(::ui_lang_guest::wire::Background::Color), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[33]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX), ((8.0) as f32).max(0.0).min(f32::MAX)]) }), snap: ::std::option::Option::None, content: ::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1157, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:1157", __ice_node_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[30]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("Only this agent's controller can change its record. You are reading it.".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!self.creating) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1165, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1170, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:1170", __ice_node_scope), size: ::std::option::Option::Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Standing".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1175, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1176, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:1176", __ice_node_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (self.selected_status.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (self.can_edit && (self.selected_status == "active")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1182, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:1182", __ice_node_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1188, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:1188", __ice_node_scope), size: ::std::option::Option::Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::None, font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Pause".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Pause agent".to_owned())), on_press: ::std::option::Option::Some(::ui_lang_guest::slots::message(__AgentsViewMessage::SetStatus(self.selected.to_owned(), true))), width: ::std::option::Option::None, height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((26.0) as f32)), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((5.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[15]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(1.0), radius: ::std::option::Option::Some([8.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[6]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face::default(), hovered: ::std::option::Option::None, pressed: ::std::option::Option::None, disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (self.can_edit && (self.selected_status == "paused")) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1190, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:1190", __ice_node_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1196, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:1196", __ice_node_scope), size: ::std::option::Option::Some(((11.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::None, font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Resume".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Resume agent".to_owned())), on_press: ::std::option::Option::Some(::ui_lang_guest::slots::message(__AgentsViewMessage::SetStatus(self.selected.to_owned(), false))), width: ::std::option::Option::None, height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((26.0) as f32)), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((5.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[15]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(1.0), radius: ::std::option::Option::Some([8.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[6]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face::default(), hovered: ::std::option::Option::None, pressed: ::std::option::Option::None, disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:1165", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((8.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1199, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1200, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:1200", __ice_node_scope), size: ::std::option::Option::Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Identity".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if self.creating { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1206, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let __ice_node_scope = format!("{}/agent-id", __ice_node_scope); ::ui_lang_guest::wire::Node::Input { options: ::ui_lang_guest::wire::InputOptions { label: ("Agent id".to_owned()).to_string(), description: ::std::option::Option::None, disabled: false, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((7.0) as f32)), text_size: ::std::option::Option::Some((13.0) as f32), line_height: ::std::option::Option::Some((1.2) as f32), align: ::std::option::Option::None, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: __ice_node_scope.clone(), placeholder: ::std::string::String::from("a-dns-label, e.g. chiefduck".to_owned()), value: (self.draft_id).to_string(), on_input: ::ui_lang_guest::slots::handler::<::std::string::String, __AgentsViewMessage>(::std::boxed::Box::new({ let __route = __AgentsViewMessage::__BindDraftId as fn(::std::string::String) -> __AgentsViewMessage; move |__sent: ::std::string::String| ::std::option::Option::Some(__route(__sent)) })), on_submit: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), secure: (false), style: ::std::boxed::Box::new(::ui_lang_guest::wire::InputStyle { utility: ::ui_lang_guest::wire::InputFace { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(1f32), radius: ::std::option::Option::Some([10f32; 4]) }), ..Default::default() }, focus_border: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), focused_hovered: ::std::option::Option::None, active: ::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.160000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }), value: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), placeholder: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), selection: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.180000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.210000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::None, radius: ::std::option::Option::None }), value: ::std::option::Option::None, placeholder: ::std::option::Option::None, selection: ::std::option::Option::None }), focused: ::std::option::Option::None, disabled: ::std::option::Option::Some(::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[6]; __color.a = 0.540000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None, value: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), placeholder: ::std::option::Option::None, selection: ::std::option::Option::None }) }) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if ((self.creating && (!(self.draft_id).is_empty())) && (!crate::host::valid_agent_id(::std::convert::AsRef::as_ref(&(self.draft_id))))) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1219, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:1219", __ice_node_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[20]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align_x: ::std::option::Option::None, content: ("An agent id is a lowercase DNS label: a-z, 0-9 and hyphens, no hyphen at either end.".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!self.creating) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1225, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:1225", __ice_node_scope), size: ::std::option::Option::Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (self.draft_id.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1230, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let __ice_node_scope = format!("{}/agent-name", __ice_node_scope); ::ui_lang_guest::wire::Node::Input { options: ::ui_lang_guest::wire::InputOptions { label: ("Display name".to_owned()).to_string(), description: ::std::option::Option::None, disabled: (!self.can_edit), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((7.0) as f32)), text_size: ::std::option::Option::Some((13.0) as f32), line_height: ::std::option::Option::Some((1.2) as f32), align: ::std::option::Option::None, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: __ice_node_scope.clone(), placeholder: ::std::string::String::from("display name…".to_owned()), value: (self.draft_name).to_string(), on_input: ::ui_lang_guest::slots::handler::<::std::string::String, __AgentsViewMessage>(::std::boxed::Box::new({ let __route = __AgentsViewMessage::__BindDraftName as fn(::std::string::String) -> __AgentsViewMessage; move |__sent: ::std::string::String| ::std::option::Option::Some(__route(__sent)) })), on_submit: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), secure: (false), style: ::std::boxed::Box::new(::ui_lang_guest::wire::InputStyle { utility: ::ui_lang_guest::wire::InputFace { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(1f32), radius: ::std::option::Option::Some([10f32; 4]) }), ..Default::default() }, focus_border: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), focused_hovered: ::std::option::Option::None, active: ::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.160000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }), value: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), placeholder: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), selection: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.180000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.210000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::None, radius: ::std::option::Option::None }), value: ::std::option::Option::None, placeholder: ::std::option::Option::None, selection: ::std::option::Option::None }), focused: ::std::option::Option::None, disabled: ::std::option::Option::Some(::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[6]; __color.a = 0.540000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None, value: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), placeholder: ::std::option::Option::None, selection: ::std::option::Option::None }) }) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:1199", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((6.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1246, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1247, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:1247", __ice_node_scope), size: ::std::option::Option::Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Executor".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if self.can_edit { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1253, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let __ice_node_scope = format!("{}/agent-capability", __ice_node_scope); { let __options = crate::host::capability_options(::std::convert::AsRef::as_ref(&(self.capabilities)), ::std::convert::AsRef::as_ref(&(crate::host::or_empty(::std::borrow::Borrow::borrow(&(self.draft_capability)))))); let __selected = self.draft_capability.clone(); ::ui_lang_guest::wire::Node::PickList { key: __ice_node_scope.clone(), options: __options.iter().map(|__option| __option.to_string()).collect(), selected: __selected.as_ref().and_then(|__chosen| __options.iter().position(|__option| __option == __chosen)).map(|__index| __index as u32), placeholder: ::std::option::Option::Some(("pick a capability…".to_owned()).to_string()), on_select: ::ui_lang_guest::slots::handler::<u32, __AgentsViewMessage>(::std::boxed::Box::new({ let __route = move |__value| __AgentsViewMessage::PickCapabilityOption(__value); let __table: ::std::vec::Vec<__AgentsViewMessage> = __options.iter().cloned().map(__route).collect(); move |__sent: u32| __table.get(__sent as usize).cloned() })), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), style: ::ui_lang_guest::wire::PickListStyle { active: ::std::option::Option::None, hovered: ::std::option::Option::None, opened: ::std::option::Option::None, opened_hovered: ::std::option::Option::None, menu: ::std::option::Option::None }, settings: ::std::boxed::Box::new(::ui_lang_guest::wire::PickOptions { menu_height: ::std::option::Option::None, padding: ::std::option::Option::None, text_size: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, font: ::std::option::Option::None, handle: ::std::option::Option::None, on_open: ::std::option::Option::None, on_close: ::std::option::Option::None }) } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!self.can_edit) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1258, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:1258", __ice_node_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (crate::host::or_empty(::std::borrow::Borrow::borrow(&(self.draft_capability)))).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:1246", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((6.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1266, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1267, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:1267", __ice_node_scope), size: ::std::option::Option::Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Skills".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); for (__ice_index, skill) in self.draft_skills.iter().enumerate() { let __for_scope = format!("{}/@for:1791({})", __ice_node_scope, __ice_index); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1273, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1274, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1275, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:1275", __for_scope), size: ::std::option::Option::Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Semibold }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (skill.name.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1280, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:1280", __for_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[72]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (skill.source_prefix.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (!(skill.source_snapshot).is_empty()) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1286, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:1286", __for_scope), size: ::std::option::Option::Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[72]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (skill.source_snapshot.to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:1274", __for_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((2.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (self.can_edit && skill.always) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1292, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:1292", __for_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1298, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:1298", __for_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::None, font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (crate::host::skill_mode(skill.always)).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Load on demand".to_owned())), on_press: ::std::option::Option::Some(::ui_lang_guest::slots::message(__AgentsViewMessage::LoadSkill(skill.name.to_owned(), false))), width: ::std::option::Option::None, height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((24.0) as f32)), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((4.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[15]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(1.0), radius: ::std::option::Option::Some([8.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[6]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face::default(), hovered: ::std::option::Option::None, pressed: ::std::option::Option::None, disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (self.can_edit && (!skill.always)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1300, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:1300", __for_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1306, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:1306", __for_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::None, font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (crate::host::skill_mode(skill.always)).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Load always".to_owned())), on_press: ::std::option::Option::Some(::ui_lang_guest::slots::message(__AgentsViewMessage::LoadSkill(skill.name.to_owned(), true))), width: ::std::option::Option::None, height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((24.0) as f32)), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((4.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[15]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(1.0), radius: ::std::option::Option::Some([8.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[6]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face::default(), hovered: ::std::option::Option::None, pressed: ::std::option::Option::None, disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if (!self.can_edit) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1308, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::None }, key: format!("{}/@text:1308", __for_scope), size: ::std::option::Option::Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: true, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: (crate::host::skill_mode(skill.always)).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if self.can_edit { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1314, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:1314", __for_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1320, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:1320", __for_scope), size: ::std::option::Option::Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[71]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("×".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Remove skill".to_owned())), on_press: ::std::option::Option::Some(::ui_lang_guest::slots::message(__AgentsViewMessage::RemoveSkill(skill.name.to_owned()))), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((22.0) as f32)), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((22.0) as f32)), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((0.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::None, text: ::std::option::Option::None, border: ::std::option::Option::None }, hover_background: ::std::option::Option::None, pressed_background: ::std::option::Option::None, disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::None, focus_ring: ::std::option::Option::None, text_size: ::std::option::Option::Some(13.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face::default(), hovered: ::std::option::Option::None, pressed: ::std::option::Option::None, disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:1273", __for_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((6.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if self.can_edit { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1322, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1323, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let __ice_node_scope = format!("{}/skill-name", __ice_node_scope); ::ui_lang_guest::wire::Node::Input { options: ::ui_lang_guest::wire::InputOptions { label: ("Skill name".to_owned()).to_string(), description: ::std::option::Option::None, disabled: false, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((7.0) as f32)), text_size: ::std::option::Option::Some((12.5) as f32), line_height: ::std::option::Option::Some((1.2) as f32), align: ::std::option::Option::None, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: __ice_node_scope.clone(), placeholder: ::std::string::String::from("skill name (its mount directory)…".to_owned()), value: (self.skill_name).to_string(), on_input: ::ui_lang_guest::slots::handler::<::std::string::String, __AgentsViewMessage>(::std::boxed::Box::new({ let __route = __AgentsViewMessage::__BindSkillName as fn(::std::string::String) -> __AgentsViewMessage; move |__sent: ::std::string::String| ::std::option::Option::Some(__route(__sent)) })), on_submit: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), secure: (false), style: ::std::boxed::Box::new(::ui_lang_guest::wire::InputStyle { utility: ::ui_lang_guest::wire::InputFace { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(1f32), radius: ::std::option::Option::Some([10f32; 4]) }), ..Default::default() }, focus_border: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), focused_hovered: ::std::option::Option::None, active: ::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.160000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }), value: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), placeholder: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), selection: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.180000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.210000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::None, radius: ::std::option::Option::None }), value: ::std::option::Option::None, placeholder: ::std::option::Option::None, selection: ::std::option::Option::None }), focused: ::std::option::Option::None, disabled: ::std::option::Option::Some(::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[6]; __color.a = 0.540000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None, value: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), placeholder: ::std::option::Option::None, selection: ::std::option::Option::None }) }) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1335, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let __ice_node_scope = format!("{}/skill-prefix", __ice_node_scope); ::ui_lang_guest::wire::Node::Input { options: ::ui_lang_guest::wire::InputOptions { label: ("Skill source prefix".to_owned()).to_string(), description: ::std::option::Option::None, disabled: false, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((7.0) as f32)), text_size: ::std::option::Option::Some((12.5) as f32), line_height: ::std::option::Option::Some((1.2) as f32), align: ::std::option::Option::None, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: __ice_node_scope.clone(), placeholder: ::std::string::String::from("/shared/skills/<name> when left empty".to_owned()), value: (self.skill_prefix).to_string(), on_input: ::ui_lang_guest::slots::handler::<::std::string::String, __AgentsViewMessage>(::std::boxed::Box::new({ let __route = __AgentsViewMessage::__BindSkillPrefix as fn(::std::string::String) -> __AgentsViewMessage; move |__sent: ::std::string::String| ::std::option::Option::Some(__route(__sent)) })), on_submit: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), secure: (false), style: ::std::boxed::Box::new(::ui_lang_guest::wire::InputStyle { utility: ::ui_lang_guest::wire::InputFace { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(1f32), radius: ::std::option::Option::Some([10f32; 4]) }), ..Default::default() }, focus_border: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), focused_hovered: ::std::option::Option::None, active: ::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.160000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }), value: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), placeholder: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), selection: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.180000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.210000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::None, radius: ::std::option::Option::None }), value: ::std::option::Option::None, placeholder: ::std::option::Option::None, selection: ::std::option::Option::None }), focused: ::std::option::Option::None, disabled: ::std::option::Option::Some(::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[6]; __color.a = 0.540000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None, value: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), placeholder: ::std::option::Option::None, selection: ::std::option::Option::None }) }) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1347, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let __ice_node_scope = format!("{}/skill-snapshot", __ice_node_scope); ::ui_lang_guest::wire::Node::Input { options: ::ui_lang_guest::wire::InputOptions { label: ("Skill snapshot pin".to_owned()).to_string(), description: ::std::option::Option::None, disabled: false, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((7.0) as f32)), text_size: ::std::option::Option::Some((12.5) as f32), line_height: ::std::option::Option::Some((1.2) as f32), align: ::std::option::Option::None, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: __ice_node_scope.clone(), placeholder: ::std::string::String::from("snapshot id to pin (optional)".to_owned()), value: (self.skill_snapshot).to_string(), on_input: ::ui_lang_guest::slots::handler::<::std::string::String, __AgentsViewMessage>(::std::boxed::Box::new({ let __route = __AgentsViewMessage::__BindSkillSnapshot as fn(::std::string::String) -> __AgentsViewMessage; move |__sent: ::std::string::String| ::std::option::Option::Some(__route(__sent)) })), on_submit: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), secure: (false), style: ::std::boxed::Box::new(::ui_lang_guest::wire::InputStyle { utility: ::ui_lang_guest::wire::InputFace { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[3]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[39]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(1f32), radius: ::std::option::Option::Some([10f32; 4]) }), ..Default::default() }, focus_border: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), focused_hovered: ::std::option::Option::None, active: ::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.160000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(((1.0) as f32).max(0.0).min(f32::MAX)), radius: ::std::option::Option::Some([((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX), ((7.0) as f32).max(0.0).min(f32::MAX)]) }), value: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[4]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), placeholder: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), selection: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.180000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }) }, hovered: ::std::option::Option::Some(::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[55]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[4]; __color.a = 0.210000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::None, radius: ::std::option::Option::None }), value: ::std::option::Option::None, placeholder: ::std::option::Option::None, selection: ::std::option::Option::None }), focused: ::std::option::Option::None, disabled: ::std::option::Option::Some(::ui_lang_guest::wire::InputFace { icon: ::std::option::Option::None, background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[6]; __color.a = 0.540000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::None, value: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[5]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), placeholder: ::std::option::Option::None, selection: ::std::option::Option::None }) }) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1359, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let mut __children: ::std::vec::Vec<__IceElement<'_, __AgentsViewMessage>> = ::std::vec::Vec::new(); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1360, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = { let __ice_node_scope = format!("{}/skill-always", __ice_node_scope); ::ui_lang_guest::wire::Node::Toggle { key: __ice_node_scope.clone(), kind: ::ui_lang_guest::wire::ToggleKind::Checkbox, label: ("load always (persona)".to_owned()).to_string(), checked: (self.skill_always), on_toggle: ::std::option::Option::Some(::ui_lang_guest::slots::handler::<bool, __AgentsViewMessage>(::std::boxed::Box::new({ let __route = move |__value| __AgentsViewMessage::SetSkillAlways(__value); let __on = __route(true); let __off = __route(false); move |__sent: bool| ::std::option::Option::Some(if __sent { __on.clone() } else { __off.clone() }) }))), width: ::std::option::Option::None, style: ::ui_lang_guest::wire::ToggleStyle { tone: ::std::option::Option::None, active_on: ::std::option::Option::None, active_off: ::std::option::Option::None, hovered_on: ::std::option::Option::None, hovered_off: ::std::option::Option::None, disabled_on: ::std::option::Option::None, disabled_off: ::std::option::Option::None } } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1361, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Space { width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1362, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:1362", __ice_node_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1369, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:1369", __ice_node_scope), size: ::std::option::Option::Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::None, font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Add skill".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Add skill".to_owned())), on_press: if (((self.skill_name).trim().to_owned()).is_empty()) { ::std::option::Option::None } else { ::std::option::Option::Some(::ui_lang_guest::slots::message(__AgentsViewMessage::AddSkill)) }, width: ::std::option::Option::None, height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fixed((28.0) as f32)), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((5.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[12]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[13]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[40]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), width: ::std::option::Option::Some(1.0), radius: ::std::option::Option::Some([9.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[14]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[6]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::None, disabled_text: ::std::option::Option::None, disabled_opacity: ::std::option::Option::Some(0.5f32), focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face::default(), hovered: ::std::option::Option::None, pressed: ::std::option::Option::None, disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:1359", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::Some((6.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::Some(::ui_lang_guest::wire::AlignX::Center), background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:1322", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((6.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:1266", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((6.0) as f32), padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); if (self.can_edit && (!self.creating)) { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1372, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:1372", __ice_node_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1379, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:1379", __ice_node_scope), size: ::std::option::Option::Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::None, font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Save".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Save agent".to_owned())), on_press: if ((((self.draft_name).trim().to_owned()).is_empty() || (crate::host::or_empty(::std::borrow::Borrow::borrow(&(self.draft_capability)))).is_empty())) { ::std::option::Option::None } else { ::std::option::Option::Some(::ui_lang_guest::slots::message(__AgentsViewMessage::SubmitSave)) }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((10.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[9]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([9.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[8]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[7]; __color.a = 0.800000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[10]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[11]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_opacity: ::std::option::Option::None, focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face::default(), hovered: ::std::option::Option::None, pressed: ::std::option::Option::None, disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } if self.creating { __children.push({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1381, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Button { checked: ::std::option::Option::None, expanded: ::std::option::Option::None, description: ::std::option::Option::None, key: format!("{}/@button:1381", __ice_node_scope), content: ::ui_lang_guest::wire::ButtonContent::Child(::std::boxed::Box::new({
#[cfg(test)]
let __ice_render_source_location = ::ui_lang_runtime::testing::Location::new("src/ui/app.ice", 1388, 1, "rendered view node");
#[cfg(test)]
let __ice_render_source = ::ui_lang_runtime::testing::push_render_source(__ice_render_source_location);
let __ice_rendered: __IceElement<'_, __AgentsViewMessage> = ::ui_lang_guest::wire::Node::Text { options: ::ui_lang_guest::wire::TextOptions { height: ::std::option::Option::None, align_y: ::std::option::Option::None, line_height: ::std::option::Option::None, shaping: ::std::option::Option::None, wrapping: ::std::option::Option::None, tracking: 0.0f32, font: ::std::option::Option::Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Normal, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }, key: format!("{}/@text:1388", __ice_node_scope), size: ::std::option::Option::Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)), color: ::std::option::Option::None, font: ::ui_lang_guest::wire::Font { monospace: false, weight: ::ui_lang_guest::wire::Weight::Normal }, width: ::std::option::Option::None, align_x: ::std::option::Option::None, content: ("Register".to_owned()).to_string() };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
})), label: ::std::option::Option::Some(::std::string::String::from("Register agent".to_owned())), on_press: if ((((!crate::host::valid_agent_id(::std::convert::AsRef::as_ref(&(self.draft_id)))) || ((self.draft_name).trim().to_owned()).is_empty()) || (crate::host::or_empty(::std::borrow::Borrow::borrow(&(self.draft_capability)))).is_empty())) { ::std::option::Option::None } else { ::std::option::Option::Some(::ui_lang_guest::slots::message(__AgentsViewMessage::SubmitRegister)) }, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges::all((10.0) as f32)), style: ::ui_lang_guest::wire::ButtonStyle { preset: ::ui_lang_guest::wire::ButtonPreset::Primary, recipe: Some(::ui_lang_guest::wire::ButtonRecipe { base: ::ui_lang_guest::wire::Face { background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[7]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[9]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), border: ::std::option::Option::Some(::ui_lang_guest::wire::Border { color: ::std::option::Option::None, width: ::std::option::Option::None, radius: ::std::option::Option::Some([9.0; 4]) }) }, hover_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[8]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), pressed_background: ::std::option::Option::Some({ let __color: ::iced::Color = { let mut __color = __ice_palette.colors[7]; __color.a = 0.800000; __color }; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_background: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[10]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_text: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[11]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), disabled_opacity: ::std::option::Option::None, focus_ring: ::std::option::Option::Some({ let __color: ::iced::Color = __ice_palette.colors[42]; ::ui_lang_guest::wire::Rgba([__color.r, __color.g, __color.b, __color.a]) }), text_size: ::std::option::Option::Some(12.5f32), line_height: ::std::option::Option::None, font: Some(::ui_lang_guest::wire::NamedFont { family: ::ui_lang_guest::wire::FontFamily::Named("Geist".into()), weight: ::ui_lang_guest::wire::Weight::Semibold, stretch: ::ui_lang_guest::wire::FontStretch::Normal, style: ::ui_lang_guest::wire::FontStyle::Normal }) }), active: ::ui_lang_guest::wire::Face::default(), hovered: ::std::option::Option::None, pressed: ::std::option::Option::None, disabled: ::std::option::Option::None } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:1117", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::Some((14.0) as f32), padding: ::std::option::Option::Some(::ui_lang_guest::wire::Edges { top: (18.0) as f32, right: (18.0) as f32, bottom: (18.0) as f32, left: (18.0) as f32 }), width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::None, align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
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
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:908", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Row, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}); } ::ui_lang_guest::wire::Node::Linear { max_width: ::std::option::Option::None, clip: false, key: format!("{}/@layout:336", __ice_node_scope), wrap: None, axis: ::ui_lang_guest::wire::Axis::Column, spacing: ::std::option::Option::None, padding: ::std::option::Option::None, width: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), height: ::std::option::Option::Some(::ui_lang_guest::wire::Length::Fill), align: ::std::option::Option::None, background: ::std::option::Option::None, border: ::std::option::Option::None, children: __children } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}) } };
#[cfg(test)]
drop(__ice_render_source);
__ice_rendered
}; let __ice_root: __IceElement<'_, __AgentsViewMessage> = __ice_content; __ice_root }

}
}



ui_lang_guest::export_app!(
    AgentsView,
    "Agents",
    "The registry of who may act, on which executor, and what their runs did.",
    ["agents"]
);
