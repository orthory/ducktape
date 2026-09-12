macro_rules! __ice_generated_items_5061676573456469746f7246697874757265 { ($($item:item)*) => { $(#[allow(warnings, clippy::all)] $item)* }; }
__ice_generated_items_5061676573456469746f7246697874757265! {
type __IceElement<'a, Message, Theme = ()> = <(&'a (), Message, Theme) as ::ui_lang_guest::wire::Erase>::Node;
pub(crate) type __IceMessage = __PagesEditorFixtureMessage;
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
}
#[derive(Clone, Copy)]
struct __IcePalette { name: &'static str, colors: [::iced::Color; 4] }
#[allow(dead_code)]
pub struct PagesEditorFixture {
pub(crate) __ice_accessibility: ::ui_lang_runtime::Bridge<__PagesEditorFixtureMessage>,
#[cfg(all(target_os = "windows", not(test)))]
pub(crate) __ice_accessibility_initial: ::std::option::Option<usize>,
#[cfg(all(target_os = "windows", not(test)))]
pub(crate) __ice_accessibility_pending: ::std::vec::Vec<__PagesEditorFixtureMessage>,
pub(crate) formatting_notice: ::std::string::String,
pub(crate) document: ::ui_lang_guest::Editor,
pub(crate) history: crate::editor_binding::HistoryState,
pub(crate) menu: crate::editor_binding::MenuState,
pub(crate) source: crate::document_ingress::DocumentSource,
pub(crate) installed_source: ::std::vec::Vec<u8>,
pub(crate) load_error: ::std::string::String,
pub(crate) paint_dark: bool,
pub(crate) commented: ::std::vec::Vec<i64>,
pub(crate) __ice_rev: [u64; 9],
}
impl ::std::fmt::Debug for PagesEditorFixture { fn fmt(&self, __formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result { __formatter.write_str("PagesEditorFixture") } }
#[derive(Clone)]
pub(crate) enum __PagesEditorFixtureMessage {
__AccessibilitySnapshot(::std::boxed::Box<::ui_lang_runtime::Snapshot<__PagesEditorFixtureMessage>>),
__AccessibilityAction(::ui_lang_runtime::ActionRequest),
__AccessibilityWindow(::iced::window::Id, ::iced::window::Event),
#[cfg(all(any(target_os = "windows", target_os = "macos"), not(test)))]
__AccessibilityNativeWindow(::ui_lang_runtime::NativeWindow),
__AccessibilityFocusNext(::std::option::Option<::iced::window::Id>),
__AccessibilityFocusPrevious(::std::option::Option<::iced::window::Id>),
__TemplateChanged,
Load,
DocumentArrived(crate::document_ingress::DocumentItem),
Committed(crate::editor_binding::EditorUpdate),
__EditDocument(::ui_lang_guest::EditorDocumentUpdate),
__0T646f63756d656e74(::ui_lang_guest::EditorTransaction<__PagesEditorFixtureMessage>),
}
impl ::std::fmt::Debug for __PagesEditorFixtureMessage { fn fmt(&self, __formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result { __formatter.write_str("__PagesEditorFixtureMessage") } }
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_HistoryState(_value: &crate::editor_binding::HistoryState) {
let _: &::std::vec::Vec<u8> = &_value.snapshot;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_MenuState(_value: &crate::editor_binding::MenuState) {
let _: &::std::vec::Vec<u8> = &_value.snapshot;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_EditorUpdate(_value: &crate::editor_binding::EditorUpdate) {
let _: &::std::string::String = &_value.notice;
let _: &crate::editor_binding::HistoryState = &_value.history;
let _: &crate::editor_binding::MenuState = &_value.menu;
let _: &::std::vec::Vec<u8> = &_value.reference;
let _: &::std::vec::Vec<u8> = &_value.interaction;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_DocumentSource(_value: &crate::document_ingress::DocumentSource) {
let _: &::std::vec::Vec<u8> = &_value.reference;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_DocumentItem(_value: &crate::document_ingress::DocumentItem) {
let _: &::std::string::String = &_value.notice;
let _: &::std::vec::Vec<u8> = &_value.source;
let _: &::std::string::String = &_value.text;
let _: &::std::vec::Vec<u8> = &_value.cursor;
let _: &::std::string::String = &_value.error;
}
#[allow(dead_code)] fn __ui_lang_check_pure_initial_history() { let _: crate::editor_binding::HistoryState = crate::editor_binding::initial_history(); }
#[allow(dead_code)] fn __ui_lang_check_pure_initial_menu() { let _: crate::editor_binding::MenuState = crate::editor_binding::initial_menu(); }
#[allow(dead_code)] fn __ui_lang_check_editor_binding_keys() { let _: fn(crate::editor_binding::HistoryState, crate::editor_binding::MenuState) -> ::ui_lang_guest::EditorBinding<crate::editor_binding::EditorUpdate> = crate::editor_binding::keys; }
#[allow(dead_code)] fn __ui_lang_check_editor_highlighter_paint(arg0: crate::editor_binding::MenuState, arg1: bool, arg2: ::std::vec::Vec<i64>, arg3: bool) { let __editor = ::ui_lang_guest::Editor::default(); let _: ::ui_lang_guest::wire::editor_presentation::EditorPresentation = crate::presentation::paint(__editor.state_view(), arg0, arg1, arg2, arg3); }
#[allow(dead_code)] fn __ui_lang_check_pure_empty_source() { let _: crate::document_ingress::DocumentSource = crate::document_ingress::empty_source(); }
#[allow(dead_code)] fn __ui_lang_check_pure_document_editor(arg0: ::std::string::String, arg1: ::std::vec::Vec<u8>) { let _: ::ui_lang_guest::Editor = crate::document_ingress::document_editor(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_subscription_document_source(arg0: crate::document_ingress::DocumentSource) { let _: ::iced::Subscription<crate::document_ingress::DocumentItem> = crate::document_ingress::document_source(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_large_source() { let _: crate::document_ingress::DocumentSource = crate::fixture::large_source(); }
}
__ice_generated_items_5061676573456469746f7246697874757265! {
#[allow(unused_parens)]
impl PagesEditorFixture {
#[must_use]
pub fn default_font() -> ::iced::Font { ::iced::Font::DEFAULT }
}
}
__ice_generated_items_5061676573456469746f7246697874757265! {
#[allow(unused_parens)]
impl PagesEditorFixture {
fn __palette(&self) -> __IcePalette {
__IcePalette { name: "app", colors: [::iced::Color::from_rgba8(255, 255, 255, 1.000000), ::iced::Color::from_rgba8(0, 0, 0, 1.000000), ::iced::Color::from_rgba8(255, 0, 0, 1.000000), ::iced::Color::from_rgba8(255, 0, 255, 1.000000)] }
}
fn __app_theme(__ice_palette: __IcePalette) -> ::iced::Theme {
         ::std::thread_local! { static __ICE_APP_THEME: ::std::cell::RefCell<::std::option::Option<(__IcePalette, ::iced::Theme)>> = const { ::std::cell::RefCell::new(::std::option::Option::None) }; }
         __ICE_APP_THEME.with(|__cached| {
         if let ::std::option::Option::Some((__cached_palette, __cached_theme)) = &*__cached.borrow() {
         if __cached_palette.name == __ice_palette.name && __cached_palette.colors == __ice_palette.colors { return __cached_theme.clone(); }
         }
         let __theme =
::iced::Theme::custom(::std::format!("PagesEditorFixture/{}", __ice_palette.name), ::iced::theme::Palette {
background: __ice_palette.colors[0],
text: __ice_palette.colors[1],
primary: __ice_palette.colors[2],
success: __ice_palette.colors[2],
warning: __ice_palette.colors[3],
danger: __ice_palette.colors[3],
});
*__cached.borrow_mut() = ::std::option::Option::Some((__ice_palette, __theme.clone()));
__theme })
}
pub(crate) fn __theme(&self) -> ::iced::Theme {
Self::__app_theme(self.__palette())
}
fn __title(&self) -> ::std::string::String { "Pages editor binding".to_owned() }
}
}
__ice_generated_items_5061676573456469746f7246697874757265! {
#[allow(unused_parens)]
impl PagesEditorFixture {
fn __state() -> Self {
Self {
__ice_accessibility: ::ui_lang_runtime::Bridge::new(),
#[cfg(all(target_os = "windows", not(test)))]
__ice_accessibility_initial: ::std::option::Option::None,
#[cfg(all(target_os = "windows", not(test)))]
__ice_accessibility_pending: ::std::vec::Vec::new(),
formatting_notice: "".to_owned(),
document: ::ui_lang_guest::Editor::new("- 한글".to_owned()),
history: crate::editor_binding::initial_history(),
menu: crate::editor_binding::initial_menu(),
source: crate::document_ingress::empty_source(),
installed_source: ::std::vec![],
load_error: "".to_owned(),
paint_dark: false,
commented: ::std::vec::Vec::new(),
__ice_rev: [::ui_lang_runtime::rev::seed(); 9],
}
}
fn __boot_task(&mut self) -> ::iced::Task<__PagesEditorFixtureMessage> {
let task = (|| {
::iced::Task::none()
})();
task
}
pub(crate) fn __boot() -> (Self, ::iced::Task<__PagesEditorFixtureMessage>) {
let mut state = Self::__state();
let task = state.__boot_task();
(state, task)
}
pub(crate) const __PREFERRED_WINDOW_SIZE: &'static str = "none";
#[allow(clippy::too_many_arguments)] fn __restore_state(formatting_notice: ::std::string::String, document: ::ui_lang_guest::Editor, history: crate::editor_binding::HistoryState, menu: crate::editor_binding::MenuState, source: crate::document_ingress::DocumentSource, installed_source: ::std::vec::Vec<u8>, load_error: ::std::string::String, paint_dark: bool, commented: ::std::vec::Vec<i64>) -> Self {
Self {
__ice_accessibility: ::ui_lang_runtime::Bridge::new(),
#[cfg(all(target_os = "windows", not(test)))]
__ice_accessibility_initial: ::std::option::Option::None,
#[cfg(all(target_os = "windows", not(test)))]
__ice_accessibility_pending: ::std::vec::Vec::new(),
formatting_notice: formatting_notice,
document: document,
history: history,
menu: menu,
source: source,
installed_source: installed_source,
load_error: load_error,
paint_dark: paint_dark,
commented: commented,
__ice_rev: [::ui_lang_runtime::rev::seed(); 9],
}
}
pub(crate) const __SNAPSHOT_SCHEMA: &'static str = "d58bf2b798ab09bc7584235aa6d369a976f67b3396fb454eef1417f7f456836b";
pub(crate) fn __snapshot(&self) -> ::std::result::Result<::std::vec::Vec<u8>, ::std::string::String> { ::ui_lang_guest::wire::Snapshot {schema: ::std::string::String::from(Self::__SNAPSHOT_SCHEMA), state: ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("PagesEditorFixture"), fields: vec![(::std::string::String::from("formatting_notice"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.formatting_notice))), (::std::string::String::from("document"), ::ui_lang_guest::wire::SnapshotValue::Bytes((&self.document).snapshot())), (::std::string::String::from("history"), ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("HistoryState"), fields: ::std::vec![(::std::string::String::from("snapshot"), ::ui_lang_guest::wire::SnapshotValue::Bytes((&(&self.history).snapshot).clone()))] }), (::std::string::String::from("menu"), ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("MenuState"), fields: ::std::vec![(::std::string::String::from("snapshot"), ::ui_lang_guest::wire::SnapshotValue::Bytes((&(&self.menu).snapshot).clone()))] }), (::std::string::String::from("source"), ::ui_lang_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("DocumentSource"), fields: ::std::vec![(::std::string::String::from("reference"), ::ui_lang_guest::wire::SnapshotValue::Bytes((&(&self.source).reference).clone()))] }), (::std::string::String::from("installed_source"), ::ui_lang_guest::wire::SnapshotValue::Bytes((&self.installed_source).clone())), (::std::string::String::from("load_error"), ::ui_lang_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.load_error))), (::std::string::String::from("paint_dark"), ::ui_lang_guest::wire::SnapshotValue::Bool(*(&self.paint_dark))), (::std::string::String::from("commented"), ::ui_lang_guest::wire::SnapshotValue::List((&self.commented).iter().map(|__item| ::ui_lang_guest::wire::SnapshotValue::I64(*(__item))).collect()))] }}.encode() }
pub(crate) fn __restore(__bytes: &[u8]) -> ::std::result::Result<Self, ::std::string::String> { let __snapshot = ::ui_lang_guest::wire::Snapshot::decode(__bytes)?; if __snapshot.schema != Self::__SNAPSHOT_SCHEMA { return ::std::result::Result::Err(::std::string::String::from("snapshot schema mismatch")); } let __value = __snapshot.state; ((|| { let ::ui_lang_guest::wire::SnapshotValue::Record {name: __name, fields: __fields} = __value else { return ::std::option::Option::None; }; if __name != "PagesEditorFixture" || __fields.len() != 9 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __value) = __fields.next()?; if __name != "formatting_notice" { return ::std::option::Option::None; } let formatting_notice: ::std::string::String = (match __value { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "document" { return ::std::option::Option::None; } let document: ::ui_lang_guest::Editor = (match __value { ::ui_lang_guest::wire::SnapshotValue::Bytes(bytes) => ::ui_lang_guest::Editor::restore(&bytes), _ => None })?; let (__name, __value) = __fields.next()?; if __name != "history" { return ::std::option::Option::None; } let history: crate::editor_binding::HistoryState = ((|| { let ::ui_lang_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __value else { return ::std::option::Option::None; }; if __name != "HistoryState" || __fields.len() != 1 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "snapshot" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::editor_binding::HistoryState { snapshot: (match __field_0 { ::ui_lang_guest::wire::SnapshotValue::Bytes(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })())?; let (__name, __value) = __fields.next()?; if __name != "menu" { return ::std::option::Option::None; } let menu: crate::editor_binding::MenuState = ((|| { let ::ui_lang_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __value else { return ::std::option::Option::None; }; if __name != "MenuState" || __fields.len() != 1 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "snapshot" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::editor_binding::MenuState { snapshot: (match __field_0 { ::ui_lang_guest::wire::SnapshotValue::Bytes(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })())?; let (__name, __value) = __fields.next()?; if __name != "source" { return ::std::option::Option::None; } let source: crate::document_ingress::DocumentSource = ((|| { let ::ui_lang_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __value else { return ::std::option::Option::None; }; if __name != "DocumentSource" || __fields.len() != 1 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "reference" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::document_ingress::DocumentSource { reference: (match __field_0 { ::ui_lang_guest::wire::SnapshotValue::Bytes(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })())?; let (__name, __value) = __fields.next()?; if __name != "installed_source" { return ::std::option::Option::None; } let installed_source: ::std::vec::Vec<u8> = (match __value { ::ui_lang_guest::wire::SnapshotValue::Bytes(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "load_error" { return ::std::option::Option::None; } let load_error: ::std::string::String = (match __value { ::ui_lang_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "paint_dark" { return ::std::option::Option::None; } let paint_dark: bool = (match __value { ::ui_lang_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "commented" { return ::std::option::Option::None; } let commented: ::std::vec::Vec<i64> = (match __value { ::ui_lang_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| match __item { ::ui_lang_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None }).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })?; ::std::option::Option::Some(Self::__restore_state(formatting_notice, document, history, menu, source, installed_source, load_error, paint_dark, commented)) })()).ok_or_else(|| ::std::string::String::from("snapshot state mismatch")) }
}
}
__ice_generated_items_5061676573456469746f7246697874757265! {
#[allow(unused_parens)]
impl PagesEditorFixture {
}
}
__ice_generated_items_5061676573456469746f7246697874757265! {
#[allow(unused_parens)]
impl PagesEditorFixture {
fn __subscription(&self) -> ::iced::Subscription<__PagesEditorFixtureMessage> {
::iced::Subscription::batch([
if ((!(self.source.reference).is_empty()) && (self.source.reference != self.installed_source)) { ::iced::Subscription::batch([crate::document_ingress::document_source(self.source.clone()).map(move |__value| __PagesEditorFixtureMessage::DocumentArrived(__value)),
]) } else { ::iced::Subscription::none() },
])
}
}
}
__ice_generated_items_5061676573456469746f7246697874757265! {
#[allow(unused_parens)]
impl PagesEditorFixture {
}
#[cfg(test)] mod __ice_tests { use super::*;
#[test]
fn __ice_view_fits_default_stack() {
::std::thread::Builder::new().stack_size(4 * 1024 * 1024).spawn(|| {
let (__app, _) = PagesEditorFixture::__boot();
let _ = __app.__view();
}).unwrap().join().unwrap();
}
}
}
include!("app_update.rs");
include!("app_view.rs");
