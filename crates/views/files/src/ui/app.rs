macro_rules! __ice_generated_items_46696c657356696577 { ($($item:item)*) => { $(#[allow(warnings, clippy::all)] $item)* }; }
__ice_generated_items_46696c657356696577! {
type __IceElement<'a, Message, Theme = ()> = <(&'a (), Message, Theme) as ::ducktape_view_guest::wire::Erase>::Node;
pub(crate) type __IceMessage = __FilesViewMessage;
 #[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum AppTheme {
App,
AppDark,
}
#[derive(Clone, Copy)]
struct __IcePalette { name: &'static str, colors: [::ducktape_view_guest::wire::Rgba; 128] }
#[derive(Default)]
struct __IceDerivedCache {
refusal: ::std::cell::OnceCell<::std::string::String>,
loading: ::std::cell::OnceCell<bool>,
draft_here: ::std::cell::OnceCell<bool>,
draft_parked: ::std::cell::OnceCell<bool>,
edit_context: ::std::cell::OnceCell<::std::string::String>,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Tone {
Light,
Dark,
}
#[allow(dead_code)]
pub(crate) struct __IceFilesScreenState {
history_open: bool,
__ice_rev: [u64; 1],
}
impl ::std::default::Default for __IceFilesScreenState {
fn default() -> Self { Self {
history_open: false,
__ice_rev: [::ducktape_view_guest::rev::seed(); 1],
} }
}
#[cfg(test)]
#[allow(non_camel_case_types, dead_code)]
#[derive(Clone)]
pub(crate) struct __IceTestState_files_screen {
pub(crate) history_open: bool,
}
#[cfg(test)]
#[allow(dead_code)]
impl FilesView {
pub(crate) fn __ice_test_state_files_screen(&self, scope: &str) -> ::std::option::Option<__IceTestState_files_screen> { let __ice_view = |__state: &__IceFilesScreenState| __IceTestState_files_screen { history_open: __state.history_open.clone(), }; let __ice_stored = self.__ice_component_046696c657353637265656e.get(scope).map(__ice_view); __ice_stored }
pub(crate) fn __ice_test_message_files_screen_fs_toggle_history(scope: ::std::string::String) -> __FilesViewMessage { __FilesViewMessage::__0C46696c657353637265656eH66735f746f67676c655f686973746f7279(scope) }
}
#[allow(dead_code)]
pub struct FilesView {
pub(crate) active_palette: AppTheme,
pub(crate) connected: bool,
pub(crate) dark: bool,
pub(crate) chain: ::std::string::String,
pub(crate) generation: i64,
pub(crate) route_serial: i64,
pub(crate) path: ::std::string::String,
pub(crate) listed: bool,
pub(crate) entries: ::std::vec::Vec<crate::host::FsEntry>,
pub(crate) directories: ::std::vec::Vec<crate::host::FsEntry>,
pub(crate) history: ::std::vec::Vec<crate::host::FsSnapshot>,
pub(crate) omitted: i64,
pub(crate) diff_omitted: i64,
pub(crate) preview_path: ::std::string::String,
pub(crate) preview_entry: crate::host::FsEntry,
pub(crate) preview_base: ::std::string::String,
pub(crate) preview_text: ::std::string::String,
pub(crate) preview_display_text: ::std::string::String,
pub(crate) preview_clipped: bool,
pub(crate) preview_truncated: bool,
pub(crate) preview_binary: bool,
pub(crate) preview_picture: bool,
pub(crate) preview_width: i64,
pub(crate) preview_height: i64,
pub(crate) delete_target: ::std::string::String,
pub(crate) diff_from: ::std::string::String,
pub(crate) diff: ::std::vec::Vec<crate::host::FsDiffEntry>,
pub(crate) acting: bool,
pub(crate) saving: bool,
pub(crate) notice: ::std::string::String,
pub(crate) new_name: ::std::string::String,
pub(crate) draft: ::ducktape_view_guest::Editor,
pub(crate) editing: bool,
pub(crate) draft_chain: ::std::string::String,
pub(crate) draft_path: ::std::string::String,
pub(crate) draft_base: ::std::string::String,
pub(crate) draft_id: i64,
pub(crate) sent: bool,
pub(crate) viewport_width: f64,
pub(crate) viewport_height: f64,
pub(crate) tree_width: f64,
pub(crate) preview_pane_height: f64,
pub(crate) object_width: f64,
pub(crate) __ice_derived: __IceDerivedCache,
pub(crate) __ice_rev: [u64; 43],
pub(crate) __ice_component_046696c657353637265656e: ::std::collections::HashMap<::std::string::String, __IceFilesScreenState>,
pub(crate) __ice_component_046696c657353637265656e_initial: __IceFilesScreenState,
}
impl ::std::fmt::Debug for FilesView { fn fmt(&self, __formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result { __formatter.write_str("FilesView") } }
#[derive(Clone)]
pub(crate) enum __FilesViewMessage {
TreeResized(f64, f64),
PreviewResized(f64, f64),
ObjectResized(f64, f64),
ViewportChanged(f64, f64),
SessionArrived(crate::host::SessionItem),
RouteTo(::std::string::String),
ListingArrived(crate::host::ListingItem),
PreviewArrived(crate::host::PreviewItem),
DiffArrived(crate::host::DiffItem),
ActDone(crate::host::ActItem),
OpenDirAt(::std::string::String),
OpenFileAt(::std::string::String),
MkdirSubmit,
NewFileSubmit,
ArmDeleteAt(::std::string::String),
DisarmDeleteNow,
DeleteSubmit,
CloseDiffNow,
ShowDiffOf(::std::string::String),
BeginEdit(::std::string::String),
CancelEdit(::std::string::String),
DiscardDraft(i64),
SaveEdit(::std::string::String),
OpenLinkAt(::std::string::String),
__0C46696c657353637265656eH66735f746f67676c655f686973746f7279(::std::string::String),
__BindNewName(::std::string::String),
__EditDraft(::ducktape_view_guest::EditorDocumentUpdate),
__0T6472616674(::ducktape_view_guest::EditorTransaction<__FilesViewMessage>),
__ExternNoop,
}
impl ::std::fmt::Debug for __FilesViewMessage { fn fmt(&self, __formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result { __formatter.write_str("__FilesViewMessage") } }
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_FsEntry(_value: &crate::host::FsEntry) {
let _: &i64 = &_value.key;
let _: &::std::string::String = &_value.path;
let _: &::std::string::String = &_value.name;
let _: &::std::string::String = &_value.kind;
let _: &i64 = &_value.size;
let _: &::std::string::String = &_value.object;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_FsSnapshot(_value: &crate::host::FsSnapshot) {
let _: &::std::string::String = &_value.id;
let _: &::std::string::String = &_value.short_id;
let _: &::std::string::String = &_value.author;
let _: &i64 = &_value.height;
let _: &::std::string::String = &_value.message;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_FsDiffEntry(_value: &crate::host::FsDiffEntry) {
let _: &::std::string::String = &_value.path;
let _: &::std::string::String = &_value.kind;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_Session(_value: &crate::host::Session) {
let _: &bool = &_value.connected;
let _: &bool = &_value.dark;
let _: &::std::string::String = &_value.chain;
let _: &::std::string::String = &_value.route;
let _: &i64 = &_value.route_serial;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_SessionItem(_value: &crate::host::SessionItem) {
let _: &crate::host::Session = &_value.next;
let _: &::std::string::String = &_value.error;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_ListingItem(_value: &crate::host::ListingItem) {
let _: &::std::vec::Vec<crate::host::FsEntry> = &_value.entries;
let _: &::std::vec::Vec<crate::host::FsEntry> = &_value.directories;
let _: &::std::vec::Vec<crate::host::FsSnapshot> = &_value.history;
let _: &i64 = &_value.omitted;
let _: &::std::string::String = &_value.error;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_PreviewItem(_value: &crate::host::PreviewItem) {
let _: &::std::string::String = &_value.path;
let _: &::std::string::String = &_value.base;
let _: &::std::string::String = &_value.text;
let _: &::std::string::String = &_value.display_text;
let _: &bool = &_value.clipped;
let _: &bool = &_value.truncated;
let _: &bool = &_value.binary;
let _: &bool = &_value.picture;
let _: &i64 = &_value.width;
let _: &i64 = &_value.height;
let _: &::std::string::String = &_value.error;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_DiffItem(_value: &crate::host::DiffItem) {
let _: &::std::vec::Vec<crate::host::FsDiffEntry> = &_value.entries;
let _: &i64 = &_value.omitted;
let _: &::std::string::String = &_value.error;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_ActItem(_value: &crate::host::ActItem) {
let _: &::std::string::String = &_value.kind;
let _: &::std::string::String = &_value.error;
}
#[allow(dead_code)] fn __ui_lang_check_subscription_session() { let _: ::ducktape_view_guest::Subscription<crate::host::SessionItem> = crate::host::session(); }
#[allow(dead_code)] fn __ui_lang_check_subscription_listing(arg0: i64, arg1: ::std::string::String) { let _: ::ducktape_view_guest::Subscription<crate::host::ListingItem> = crate::host::listing(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_subscription_preview(arg0: i64, arg1: ::std::string::String) { let _: ::ducktape_view_guest::Subscription<crate::host::PreviewItem> = crate::host::preview(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_subscription_diff(arg0: i64, arg1: ::std::string::String) { let _: ::ducktape_view_guest::Subscription<crate::host::DiffItem> = crate::host::diff(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_subscription_acts() { let _: ::ducktape_view_guest::Subscription<crate::host::ActItem> = crate::host::acts(); }
#[allow(dead_code)] fn __ui_lang_check_pure_generation_after(arg0: bool, arg1: bool, arg2: i64) { let _: i64 = crate::host::generation_after(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_tone_of(arg0: bool) { let _: Tone = crate::host::tone_of(arg0); }
#[allow(dead_code)] fn __ui_lang_check_sync_make_dir<'a>(arg0: &'a str, arg1: &'a str) { let _: bool = crate::host::make_dir(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_sync_make_file<'a>(arg0: &'a str, arg1: &'a str) { let _: bool = crate::host::make_file(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_sync_delete_object<'a>(arg0: &'a str) { let _: bool = crate::host::delete_object(arg0); }
#[allow(dead_code)] fn __ui_lang_check_sync_save<'a>(arg0: &'a str, arg1: &'a str, arg2: &'a str) { let _: bool = crate::host::save(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_sync_open_link<'a>(arg0: &'a str) { let _: bool = crate::host::open_link(arg0); }
#[allow(dead_code)] fn __ui_lang_check_sync_at<'a>(arg0: &'a str) { let _: bool = crate::host::at(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_write_refusal<'a>(arg0: &'a str) { let _: ::std::string::String = crate::host::write_refusal(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_fs_parent<'a>(arg0: &'a str) { let _: ::std::string::String = crate::host::fs_parent(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_entry_named<'a>(arg0: &'a [crate::host::FsEntry], arg1: &'a str) { let _: crate::host::FsEntry = crate::host::entry_named(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_edit_token<'a>(arg0: &'a str, arg1: &'a str, arg2: &'a str, arg3: i64) { let _: ::std::string::String = crate::host::edit_token(arg0, arg1, arg2, arg3); }
#[allow(dead_code)] fn __ui_lang_check_pure_icon<'a>(arg0: &'a str) { let _: ::std::vec::Vec<u8> = crate::host::icon(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_no_fs_entry() { let _: crate::host::FsEntry = crate::host::no_fs_entry(); }
#[allow(dead_code)] fn __ui_lang_check_pure_keep_draft<'a>(arg0: bool, arg1: &'a str) { let _: ::std::string::String = crate::host::keep_draft(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_keep_str<'a>(arg0: bool, arg1: &'a str, arg2: &'a str) { let _: ::std::string::String = crate::host::keep_str(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_fs_counts_summary<'a>(arg0: bool, arg1: bool, arg2: &'a [crate::host::FsEntry]) { let _: ::std::string::String = crate::host::fs_counts_summary(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_size_label(arg0: i64) { let _: ::std::string::String = crate::host::size_label(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_height_label(arg0: i64) { let _: ::std::string::String = crate::host::height_label(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_picture_caption(arg0: i64, arg1: i64) { let _: ::std::string::String = crate::host::picture_caption(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_markdown_path<'a>(arg0: &'a str) { let _: bool = crate::host::markdown_path(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_tree_width_after_delta(arg0: f64, arg1: f64, arg2: f64, arg3: f64) { let _: f64 = crate::host::tree_width_after_delta(arg0, arg1, arg2, arg3); }
#[allow(dead_code)] fn __ui_lang_check_pure_preview_height_after_delta(arg0: f64, arg1: f64, arg2: f64) { let _: f64 = crate::host::preview_height_after_delta(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_object_width_after_delta(arg0: f64, arg1: f64, arg2: f64, arg3: f64) { let _: f64 = crate::host::object_width_after_delta(arg0, arg1, arg2, arg3); }
}
__ice_generated_items_46696c657356696577! {
#[allow(unused_parens)]
impl FilesView {
#[must_use]
fn __ice_derived_refusal(&self) -> &::std::string::String { self.__ice_derived.refusal.get_or_init(|| crate::host::write_refusal(::std::convert::AsRef::as_ref(&(self.path)))) }
fn __ice_derived_loading(&self) -> &bool { self.__ice_derived.loading.get_or_init(|| ((self.acting || self.saving) || (self.connected && (!self.listed)))) }
fn __ice_derived_draft_here(&self) -> &bool { self.__ice_derived.draft_here.get_or_init(|| ((self.editing && (self.draft_path == self.preview_path)) && (self.draft_chain == self.chain))) }
fn __ice_derived_draft_parked(&self) -> &bool { self.__ice_derived.draft_parked.get_or_init(|| (self.editing && (!(*self.__ice_derived_draft_here())))) }
fn __ice_derived_edit_context(&self) -> &::std::string::String { self.__ice_derived.edit_context.get_or_init(|| crate::host::edit_token(::std::convert::AsRef::as_ref(&(self.chain)), ::std::convert::AsRef::as_ref(&(self.preview_path)), ::std::convert::AsRef::as_ref(&(self.preview_base)), self.draft_id)) }
}
}
__ice_generated_items_46696c657356696577! {
#[allow(unused_parens)]
impl FilesView {
fn __palette(&self) -> __IcePalette {
match self.active_palette.clone() {
AppTheme::App => __IcePalette { name: "app", colors: [::ducktape_view_guest::wire::Rgba([58.0 / 255.0, 56.0 / 255.0, 51.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([212.0 / 255.0, 210.0 / 255.0, 202.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([253.0 / 255.0, 253.0 / 255.0, 251.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([255.0 / 255.0, 255.0 / 255.0, 255.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([44.0 / 255.0, 43.0 / 255.0, 39.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([107.0 / 255.0, 105.0 / 255.0, 98.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([246.0 / 255.0, 245.0 / 255.0, 242.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([38.0 / 255.0, 37.0 / 255.0, 31.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([50.0 / 255.0, 47.0 / 255.0, 40.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([255.0 / 255.0, 255.0 / 255.0, 255.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([236.0 / 255.0, 235.0 / 255.0, 230.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([179.0 / 255.0, 177.0 / 255.0, 168.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([255.0 / 255.0, 255.0 / 255.0, 255.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([94.0 / 255.0, 92.0 / 255.0, 85.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([243.0 / 255.0, 242.0 / 255.0, 239.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([63.0 / 255.0, 62.0 / 255.0, 57.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([160.0 / 255.0, 90.0 / 255.0, 60.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([255.0 / 255.0, 255.0 / 255.0, 255.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([249.0 / 255.0, 241.0 / 255.0, 234.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([231.0 / 255.0, 210.0 / 255.0, 196.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([184.0 / 255.0, 84.0 / 255.0, 76.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([255.0 / 255.0, 255.0 / 255.0, 255.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([253.0 / 255.0, 244.0 / 255.0, 243.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([239.0 / 255.0, 214.0 / 255.0, 211.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([224.0 / 255.0, 101.0 / 255.0, 92.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([95.0 / 255.0, 158.0 / 255.0, 116.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([21.0 / 255.0, 20.0 / 255.0, 16.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([238.0 / 255.0, 245.0 / 255.0, 240.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([207.0 / 255.0, 227.0 / 255.0, 215.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([92.0 / 255.0, 180.0 / 255.0, 95.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([160.0 / 255.0, 123.0 / 255.0, 50.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([21.0 / 255.0, 20.0 / 255.0, 16.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([251.0 / 255.0, 244.0 / 255.0, 230.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([236.0 / 255.0, 220.0 / 255.0, 174.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([227.0 / 255.0, 180.0 / 255.0, 67.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([210.0 / 255.0, 208.0 / 255.0, 199.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([79.0 / 255.0, 77.0 / 255.0, 71.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([38.0 / 255.0, 37.0 / 255.0, 31.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([243.0 / 255.0, 241.0 / 255.0, 234.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([231.0 / 255.0, 230.0 / 255.0, 226.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([224.0 / 255.0, 223.0 / 255.0, 215.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([138.0 / 255.0, 137.0 / 255.0, 131.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([38.0 / 255.0, 37.0 / 255.0, 31.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([253.0 / 255.0, 252.0 / 255.0, 250.0 / 255.0, 0.501961]), ::ducktape_view_guest::wire::Rgba([253.0 / 255.0, 252.0 / 255.0, 250.0 / 255.0, 0.619608]), ::ducktape_view_guest::wire::Rgba([253.0 / 255.0, 252.0 / 255.0, 250.0 / 255.0, 0.858824]), ::ducktape_view_guest::wire::Rgba([40.0 / 255.0, 38.0 / 255.0, 34.0 / 255.0, 0.129412]), ::ducktape_view_guest::wire::Rgba([40.0 / 255.0, 38.0 / 255.0, 34.0 / 255.0, 0.219608]), ::ducktape_view_guest::wire::Rgba([40.0 / 255.0, 38.0 / 255.0, 34.0 / 255.0, 0.301961]), ::ducktape_view_guest::wire::Rgba([40.0 / 255.0, 38.0 / 255.0, 34.0 / 255.0, 0.219608]), ::ducktape_view_guest::wire::Rgba([40.0 / 255.0, 38.0 / 255.0, 34.0 / 255.0, 0.101961]), ::ducktape_view_guest::wire::Rgba([227.0 / 255.0, 225.0 / 255.0, 217.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([236.0 / 255.0, 234.0 / 255.0, 227.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([250.0 / 255.0, 250.0 / 255.0, 248.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([251.0 / 255.0, 251.0 / 255.0, 249.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([243.0 / 255.0, 242.0 / 255.0, 239.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([236.0 / 255.0, 235.0 / 255.0, 230.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([248.0 / 255.0, 247.0 / 255.0, 243.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([240.0 / 255.0, 239.0 / 255.0, 234.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([214.0 / 255.0, 212.0 / 255.0, 204.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([239.0 / 255.0, 238.0 / 255.0, 233.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([9.0 / 255.0, 11.0 / 255.0, 14.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([36.0 / 255.0, 42.0 / 255.0, 51.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([236.0 / 255.0, 233.0 / 255.0, 225.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([236.0 / 255.0, 214.0 / 255.0, 208.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([253.0 / 255.0, 246.0 / 255.0, 244.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([163.0 / 255.0, 82.0 / 255.0, 72.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([143.0 / 255.0, 70.0 / 255.0, 61.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([50.0 / 255.0, 47.0 / 255.0, 40.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([58.0 / 255.0, 57.0 / 255.0, 52.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([154.0 / 255.0, 152.0 / 255.0, 143.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([167.0 / 255.0, 165.0 / 255.0, 155.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([179.0 / 255.0, 177.0 / 255.0, 168.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([189.0 / 255.0, 187.0 / 255.0, 177.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([203.0 / 255.0, 201.0 / 255.0, 191.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([123.0 / 255.0, 167.0 / 255.0, 140.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([95.0 / 255.0, 122.0 / 255.0, 158.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([238.0 / 255.0, 242.0 / 255.0, 247.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([218.0 / 255.0, 226.0 / 255.0, 236.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([127.0 / 255.0, 154.0 / 255.0, 184.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([163.0 / 255.0, 82.0 / 255.0, 72.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([251.0 / 255.0, 236.0 / 255.0, 234.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([236.0 / 255.0, 207.0 / 255.0, 201.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([207.0 / 255.0, 106.0 / 255.0, 94.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([251.0 / 255.0, 248.0 / 255.0, 240.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([40.0 / 255.0, 38.0 / 255.0, 34.0 / 255.0, 0.341176]), ::ducktape_view_guest::wire::Rgba([247.0 / 255.0, 246.0 / 255.0, 242.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([250.0 / 255.0, 249.0 / 255.0, 246.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([252.0 / 255.0, 251.0 / 255.0, 249.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([251.0 / 255.0, 250.0 / 255.0, 247.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([253.0 / 255.0, 248.0 / 255.0, 243.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([240.0 / 255.0, 236.0 / 255.0, 225.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([244.0 / 255.0, 231.0 / 255.0, 200.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([217.0 / 255.0, 216.0 / 255.0, 208.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([213.0 / 255.0, 211.0 / 255.0, 202.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([182.0 / 255.0, 180.0 / 255.0, 168.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([200.0 / 255.0, 198.0 / 255.0, 188.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([194.0 / 255.0, 192.0 / 255.0, 182.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([208.0 / 255.0, 206.0 / 255.0, 196.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([220.0 / 255.0, 219.0 / 255.0, 212.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([122.0 / 255.0, 120.0 / 255.0, 114.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([126.0 / 255.0, 158.0 / 255.0, 136.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([102.0 / 255.0, 100.0 / 255.0, 94.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([122.0 / 255.0, 111.0 / 255.0, 158.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([241.0 / 255.0, 237.0 / 255.0, 245.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([221.0 / 255.0, 210.0 / 255.0, 230.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([240.0 / 255.0, 245.0 / 255.0, 241.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([220.0 / 255.0, 235.0 / 255.0, 224.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([238.0 / 255.0, 246.0 / 255.0, 239.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([225.0 / 255.0, 239.0 / 255.0, 227.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([47.0 / 255.0, 107.0 / 255.0, 65.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([251.0 / 255.0, 238.0 / 255.0, 236.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([244.0 / 255.0, 221.0 / 255.0, 216.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([161.0 / 255.0, 67.0 / 255.0, 56.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([246.0 / 255.0, 243.0 / 255.0, 249.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([74.0 / 255.0, 72.0 / 255.0, 67.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([224.0 / 255.0, 145.0 / 255.0, 138.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([160.0 / 255.0, 138.0 / 255.0, 90.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([95.0 / 255.0, 138.0 / 255.0, 114.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([237.0 / 255.0, 244.0 / 255.0, 239.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([122.0 / 255.0, 111.0 / 255.0, 158.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([241.0 / 255.0, 239.0 / 255.0, 247.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([74.0 / 255.0, 72.0 / 255.0, 67.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([242.0 / 255.0, 241.0 / 255.0, 237.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([185.0 / 255.0, 113.0 / 255.0, 78.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([250.0 / 255.0, 240.0 / 255.0, 233.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([192.0 / 255.0, 138.0 / 255.0, 62.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([250.0 / 255.0, 243.0 / 255.0, 230.0 / 255.0, 1.000000])] },
AppTheme::AppDark => __IcePalette { name: "app_dark", colors: [::ducktape_view_guest::wire::Rgba([212.0 / 255.0, 210.0 / 255.0, 202.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([69.0 / 255.0, 68.0 / 255.0, 60.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([27.0 / 255.0, 26.0 / 255.0, 22.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([34.0 / 255.0, 33.0 / 255.0, 29.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([232.0 / 255.0, 230.0 / 255.0, 223.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([168.0 / 255.0, 166.0 / 255.0, 156.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([38.0 / 255.0, 37.0 / 255.0, 31.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([232.0 / 255.0, 230.0 / 255.0, 223.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([244.0 / 255.0, 242.0 / 255.0, 234.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([27.0 / 255.0, 26.0 / 255.0, 22.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([51.0 / 255.0, 50.0 / 255.0, 44.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([107.0 / 255.0, 106.0 / 255.0, 97.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([42.0 / 255.0, 41.0 / 255.0, 37.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([181.0 / 255.0, 179.0 / 255.0, 169.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([46.0 / 255.0, 45.0 / 255.0, 39.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([207.0 / 255.0, 205.0 / 255.0, 196.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([201.0 / 255.0, 138.0 / 255.0, 99.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([27.0 / 255.0, 26.0 / 255.0, 22.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([51.0 / 255.0, 38.0 / 255.0, 29.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([74.0 / 255.0, 56.0 / 255.0, 43.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([217.0 / 255.0, 123.0 / 255.0, 114.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([27.0 / 255.0, 26.0 / 255.0, 22.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([51.0 / 255.0, 33.0 / 255.0, 31.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([77.0 / 255.0, 47.0 / 255.0, 44.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([224.0 / 255.0, 101.0 / 255.0, 92.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([127.0 / 255.0, 184.0 / 255.0, 148.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([21.0 / 255.0, 20.0 / 255.0, 16.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([30.0 / 255.0, 42.0 / 255.0, 34.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([50.0 / 255.0, 71.0 / 255.0, 58.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([92.0 / 255.0, 180.0 / 255.0, 95.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([212.0 / 255.0, 169.0 / 255.0, 78.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([21.0 / 255.0, 20.0 / 255.0, 16.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([46.0 / 255.0, 39.0 / 255.0, 23.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([77.0 / 255.0, 63.0 / 255.0, 34.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([227.0 / 255.0, 180.0 / 255.0, 67.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([58.0 / 255.0, 57.0 / 255.0, 49.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([207.0 / 255.0, 205.0 / 255.0, 196.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([243.0 / 255.0, 241.0 / 255.0, 234.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([38.0 / 255.0, 37.0 / 255.0, 31.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([53.0 / 255.0, 52.0 / 255.0, 46.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([59.0 / 255.0, 58.0 / 255.0, 51.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([133.0 / 255.0, 131.0 / 255.0, 123.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([232.0 / 255.0, 230.0 / 255.0, 223.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([27.0 / 255.0, 26.0 / 255.0, 22.0 / 255.0, 0.501961]), ::ducktape_view_guest::wire::Rgba([27.0 / 255.0, 26.0 / 255.0, 22.0 / 255.0, 0.619608]), ::ducktape_view_guest::wire::Rgba([27.0 / 255.0, 26.0 / 255.0, 22.0 / 255.0, 0.858824]), ::ducktape_view_guest::wire::Rgba([0.0 / 255.0, 0.0 / 255.0, 0.0 / 255.0, 0.250980]), ::ducktape_view_guest::wire::Rgba([0.0 / 255.0, 0.0 / 255.0, 0.0 / 255.0, 0.349020]), ::ducktape_view_guest::wire::Rgba([0.0 / 255.0, 0.0 / 255.0, 0.0 / 255.0, 0.450980]), ::ducktape_view_guest::wire::Rgba([0.0 / 255.0, 0.0 / 255.0, 0.0 / 255.0, 0.349020]), ::ducktape_view_guest::wire::Rgba([0.0 / 255.0, 0.0 / 255.0, 0.0 / 255.0, 0.149020]), ::ducktape_view_guest::wire::Rgba([18.0 / 255.0, 17.0 / 255.0, 16.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([25.0 / 255.0, 24.0 / 255.0, 21.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([32.0 / 255.0, 31.0 / 255.0, 27.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([30.0 / 255.0, 29.0 / 255.0, 25.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([42.0 / 255.0, 41.0 / 255.0, 37.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([49.0 / 255.0, 48.0 / 255.0, 43.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([36.0 / 255.0, 35.0 / 255.0, 30.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([40.0 / 255.0, 39.0 / 255.0, 34.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([14.0 / 255.0, 13.0 / 255.0, 11.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([44.0 / 255.0, 43.0 / 255.0, 38.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([9.0 / 255.0, 11.0 / 255.0, 14.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([36.0 / 255.0, 42.0 / 255.0, 51.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([48.0 / 255.0, 47.0 / 255.0, 41.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([77.0 / 255.0, 47.0 / 255.0, 44.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([42.0 / 255.0, 29.0 / 255.0, 27.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([194.0 / 255.0, 90.0 / 255.0, 79.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([211.0 / 255.0, 104.0 / 255.0, 92.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([244.0 / 255.0, 242.0 / 255.0, 234.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([220.0 / 255.0, 218.0 / 255.0, 210.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([143.0 / 255.0, 141.0 / 255.0, 132.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([124.0 / 255.0, 122.0 / 255.0, 113.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([107.0 / 255.0, 106.0 / 255.0, 97.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([96.0 / 255.0, 95.0 / 255.0, 86.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([85.0 / 255.0, 84.0 / 255.0, 76.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([123.0 / 255.0, 167.0 / 255.0, 140.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([127.0 / 255.0, 154.0 / 255.0, 184.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([30.0 / 255.0, 37.0 / 255.0, 48.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([48.0 / 255.0, 62.0 / 255.0, 82.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([127.0 / 255.0, 154.0 / 255.0, 184.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([211.0 / 255.0, 104.0 / 255.0, 92.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([48.0 / 255.0, 31.0 / 255.0, 28.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([77.0 / 255.0, 47.0 / 255.0, 44.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([207.0 / 255.0, 106.0 / 255.0, 94.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([42.0 / 255.0, 37.0 / 255.0, 23.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([0.0 / 255.0, 0.0 / 255.0, 0.0 / 255.0, 0.501961]), ::ducktape_view_guest::wire::Rgba([32.0 / 255.0, 31.0 / 255.0, 26.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([35.0 / 255.0, 34.0 / 255.0, 29.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([38.0 / 255.0, 37.0 / 255.0, 32.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([38.0 / 255.0, 36.0 / 255.0, 24.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([42.0 / 255.0, 34.0 / 255.0, 27.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([53.0 / 255.0, 50.0 / 255.0, 42.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([69.0 / 255.0, 58.0 / 255.0, 30.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([63.0 / 255.0, 62.0 / 255.0, 54.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([69.0 / 255.0, 68.0 / 255.0, 60.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([110.0 / 255.0, 109.0 / 255.0, 99.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([91.0 / 255.0, 90.0 / 255.0, 82.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([98.0 / 255.0, 97.0 / 255.0, 90.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([74.0 / 255.0, 73.0 / 255.0, 65.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([51.0 / 255.0, 50.0 / 255.0, 44.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([163.0 / 255.0, 161.0 / 255.0, 152.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([126.0 / 255.0, 158.0 / 255.0, 136.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([157.0 / 255.0, 155.0 / 255.0, 146.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([168.0 / 255.0, 154.0 / 255.0, 201.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([42.0 / 255.0, 38.0 / 255.0, 51.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([68.0 / 255.0, 60.0 / 255.0, 87.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([30.0 / 255.0, 42.0 / 255.0, 34.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([50.0 / 255.0, 71.0 / 255.0, 58.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([29.0 / 255.0, 42.0 / 255.0, 32.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([36.0 / 255.0, 53.0 / 255.0, 42.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([143.0 / 255.0, 201.0 / 255.0, 162.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([47.0 / 255.0, 31.0 / 255.0, 28.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([61.0 / 255.0, 39.0 / 255.0, 35.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([222.0 / 255.0, 139.0 / 255.0, 127.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([38.0 / 255.0, 35.0 / 255.0, 48.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([46.0 / 255.0, 45.0 / 255.0, 40.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([160.0 / 255.0, 92.0 / 255.0, 85.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([192.0 / 255.0, 168.0 / 255.0, 110.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([127.0 / 255.0, 184.0 / 255.0, 148.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([30.0 / 255.0, 42.0 / 255.0, 34.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([168.0 / 255.0, 154.0 / 255.0, 201.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([42.0 / 255.0, 38.0 / 255.0, 51.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([207.0 / 255.0, 205.0 / 255.0, 196.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([46.0 / 255.0, 45.0 / 255.0, 40.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([208.0 / 255.0, 144.0 / 255.0, 104.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([51.0 / 255.0, 38.0 / 255.0, 29.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([212.0 / 255.0, 169.0 / 255.0, 78.0 / 255.0, 1.000000]), ::ducktape_view_guest::wire::Rgba([46.0 / 255.0, 39.0 / 255.0, 23.0 / 255.0, 1.000000])] },
}
}
 fn __title(&self) -> ::std::string::String { "Files".to_owned() }
}
}
__ice_generated_items_46696c657356696577! {
#[allow(unused_parens)]
impl FilesView {
fn __state() -> Self {
Self {
active_palette: AppTheme::App,
connected: false,
dark: false,
chain: "".to_owned(),
generation: 0,
route_serial: 0,
path: "/shared".to_owned(),
listed: false,
entries: ::std::vec::Vec::new(),
directories: ::std::vec::Vec::new(),
history: ::std::vec::Vec::new(),
omitted: 0,
diff_omitted: 0,
preview_path: "".to_owned(),
preview_entry: crate::host::no_fs_entry(),
preview_base: "".to_owned(),
preview_text: "".to_owned(),
preview_display_text: "".to_owned(),
preview_clipped: false,
preview_truncated: false,
preview_binary: false,
preview_picture: false,
preview_width: 0,
preview_height: 0,
delete_target: "".to_owned(),
diff_from: "".to_owned(),
diff: ::std::vec::Vec::new(),
acting: false,
saving: false,
notice: "".to_owned(),
new_name: "".to_owned(),
draft: ::ducktape_view_guest::Editor::new("".to_owned()),
editing: false,
draft_chain: "".to_owned(),
draft_path: "".to_owned(),
draft_base: "".to_owned(),
draft_id: 0,
sent: false,
viewport_width: 1280.0,
viewport_height: 700.0,
tree_width: 206.0,
preview_pane_height: 300.0,
object_width: 306.0,
__ice_derived: ::std::default::Default::default(),
__ice_rev: [::ducktape_view_guest::rev::seed(); 43],
__ice_component_046696c657353637265656e: ::std::collections::HashMap::new(),
__ice_component_046696c657353637265656e_initial: ::std::default::Default::default(),
}
}
fn __boot_task(&mut self) -> ::ducktape_view_guest::Task<__FilesViewMessage> {
let task = (|| {
::ducktape_view_guest::Task::none()
})();
task
}
pub(crate) fn __boot() -> (Self, ::ducktape_view_guest::Task<__FilesViewMessage>) {
let mut state = Self::__state();
let task = state.__boot_task();
(state, task)
}
pub(crate) const __PREFERRED_WINDOW_SIZE: &'static str = "none";
#[allow(clippy::too_many_arguments)] fn __restore_state(active_palette: AppTheme, connected: bool, dark: bool, chain: ::std::string::String, generation: i64, route_serial: i64, path: ::std::string::String, listed: bool, entries: ::std::vec::Vec<crate::host::FsEntry>, directories: ::std::vec::Vec<crate::host::FsEntry>, history: ::std::vec::Vec<crate::host::FsSnapshot>, omitted: i64, diff_omitted: i64, preview_path: ::std::string::String, preview_entry: crate::host::FsEntry, preview_base: ::std::string::String, preview_text: ::std::string::String, preview_display_text: ::std::string::String, preview_clipped: bool, preview_truncated: bool, preview_binary: bool, preview_picture: bool, preview_width: i64, preview_height: i64, delete_target: ::std::string::String, diff_from: ::std::string::String, diff: ::std::vec::Vec<crate::host::FsDiffEntry>, acting: bool, saving: bool, notice: ::std::string::String, new_name: ::std::string::String, draft: ::ducktape_view_guest::Editor, editing: bool, draft_chain: ::std::string::String, draft_path: ::std::string::String, draft_base: ::std::string::String, draft_id: i64, sent: bool, viewport_width: f64, viewport_height: f64, tree_width: f64, preview_pane_height: f64, object_width: f64, __ice_component_046696c657353637265656e: ::std::collections::HashMap<::std::string::String, __IceFilesScreenState>, __ice_component_046696c657353637265656e_initial: __IceFilesScreenState) -> Self {
Self {
active_palette: active_palette,
connected: connected,
dark: dark,
chain: chain,
generation: generation,
route_serial: route_serial,
path: path,
listed: listed,
entries: entries,
directories: directories,
history: history,
omitted: omitted,
diff_omitted: diff_omitted,
preview_path: preview_path,
preview_entry: preview_entry,
preview_base: preview_base,
preview_text: preview_text,
preview_display_text: preview_display_text,
preview_clipped: preview_clipped,
preview_truncated: preview_truncated,
preview_binary: preview_binary,
preview_picture: preview_picture,
preview_width: preview_width,
preview_height: preview_height,
delete_target: delete_target,
diff_from: diff_from,
diff: diff,
acting: acting,
saving: saving,
notice: notice,
new_name: new_name,
draft: draft,
editing: editing,
draft_chain: draft_chain,
draft_path: draft_path,
draft_base: draft_base,
draft_id: draft_id,
sent: sent,
viewport_width: viewport_width,
viewport_height: viewport_height,
tree_width: tree_width,
preview_pane_height: preview_pane_height,
object_width: object_width,
__ice_derived: ::std::default::Default::default(),
__ice_rev: [::ducktape_view_guest::rev::seed(); 43],
__ice_component_046696c657353637265656e: __ice_component_046696c657353637265656e,
__ice_component_046696c657353637265656e_initial: __ice_component_046696c657353637265656e_initial,
}
}
pub(crate) const __SNAPSHOT_SCHEMA: &'static str = "524542bd8f5b55e48d5274087dd4997a644784746056385b50157aadfe239bba";
pub(crate) fn __snapshot(&self) -> ::std::result::Result<::std::vec::Vec<u8>, ::std::string::String> { ::ducktape_view_guest::wire::Snapshot {schema: ::std::string::String::from(Self::__SNAPSHOT_SCHEMA), state: ::ducktape_view_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("FilesView"), fields: vec![(::std::string::String::from("active_palette"), match &self.active_palette { AppTheme::App => ::ducktape_view_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("AppTheme"), fields: vec![(::std::string::String::from("app"), ::ducktape_view_guest::wire::SnapshotValue::Unit)] }, AppTheme::AppDark => ::ducktape_view_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("AppTheme"), fields: vec![(::std::string::String::from("app_dark"), ::ducktape_view_guest::wire::SnapshotValue::Unit)] } }), (::std::string::String::from("connected"), ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.connected))), (::std::string::String::from("dark"), ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.dark))), (::std::string::String::from("chain"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.chain))), (::std::string::String::from("generation"), ::ducktape_view_guest::wire::SnapshotValue::I64(*(&self.generation))), (::std::string::String::from("route_serial"), ::ducktape_view_guest::wire::SnapshotValue::I64(*(&self.route_serial))), (::std::string::String::from("path"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.path))), (::std::string::String::from("listed"), ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.listed))), (::std::string::String::from("entries"), ::ducktape_view_guest::wire::SnapshotValue::List((&self.entries).iter().map(|__item| ::ducktape_view_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("FsEntry"), fields: ::std::vec![(::std::string::String::from("key"), ::ducktape_view_guest::wire::SnapshotValue::I64(*(&(__item).key))), (::std::string::String::from("path"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).path))), (::std::string::String::from("name"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).name))), (::std::string::String::from("kind"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).kind))), (::std::string::String::from("size"), ::ducktape_view_guest::wire::SnapshotValue::I64(*(&(__item).size))), (::std::string::String::from("object"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).object)))] }).collect())), (::std::string::String::from("directories"), ::ducktape_view_guest::wire::SnapshotValue::List((&self.directories).iter().map(|__item| ::ducktape_view_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("FsEntry"), fields: ::std::vec![(::std::string::String::from("key"), ::ducktape_view_guest::wire::SnapshotValue::I64(*(&(__item).key))), (::std::string::String::from("path"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).path))), (::std::string::String::from("name"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).name))), (::std::string::String::from("kind"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).kind))), (::std::string::String::from("size"), ::ducktape_view_guest::wire::SnapshotValue::I64(*(&(__item).size))), (::std::string::String::from("object"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).object)))] }).collect())), (::std::string::String::from("history"), ::ducktape_view_guest::wire::SnapshotValue::List((&self.history).iter().map(|__item| ::ducktape_view_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("FsSnapshot"), fields: ::std::vec![(::std::string::String::from("id"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).id))), (::std::string::String::from("short_id"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).short_id))), (::std::string::String::from("author"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).author))), (::std::string::String::from("height"), ::ducktape_view_guest::wire::SnapshotValue::I64(*(&(__item).height))), (::std::string::String::from("message"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).message)))] }).collect())), (::std::string::String::from("omitted"), ::ducktape_view_guest::wire::SnapshotValue::I64(*(&self.omitted))), (::std::string::String::from("diff_omitted"), ::ducktape_view_guest::wire::SnapshotValue::I64(*(&self.diff_omitted))), (::std::string::String::from("preview_path"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.preview_path))), (::std::string::String::from("preview_entry"), ::ducktape_view_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("FsEntry"), fields: ::std::vec![(::std::string::String::from("key"), ::ducktape_view_guest::wire::SnapshotValue::I64(*(&(&self.preview_entry).key))), (::std::string::String::from("path"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&self.preview_entry).path))), (::std::string::String::from("name"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&self.preview_entry).name))), (::std::string::String::from("kind"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&self.preview_entry).kind))), (::std::string::String::from("size"), ::ducktape_view_guest::wire::SnapshotValue::I64(*(&(&self.preview_entry).size))), (::std::string::String::from("object"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&self.preview_entry).object)))] }), (::std::string::String::from("preview_base"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.preview_base))), (::std::string::String::from("preview_text"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.preview_text))), (::std::string::String::from("preview_display_text"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.preview_display_text))), (::std::string::String::from("preview_clipped"), ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.preview_clipped))), (::std::string::String::from("preview_truncated"), ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.preview_truncated))), (::std::string::String::from("preview_binary"), ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.preview_binary))), (::std::string::String::from("preview_picture"), ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.preview_picture))), (::std::string::String::from("preview_width"), ::ducktape_view_guest::wire::SnapshotValue::I64(*(&self.preview_width))), (::std::string::String::from("preview_height"), ::ducktape_view_guest::wire::SnapshotValue::I64(*(&self.preview_height))), (::std::string::String::from("delete_target"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.delete_target))), (::std::string::String::from("diff_from"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.diff_from))), (::std::string::String::from("diff"), ::ducktape_view_guest::wire::SnapshotValue::List((&self.diff).iter().map(|__item| ::ducktape_view_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("FsDiffEntry"), fields: ::std::vec![(::std::string::String::from("path"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).path))), (::std::string::String::from("kind"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).kind)))] }).collect())), (::std::string::String::from("acting"), ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.acting))), (::std::string::String::from("saving"), ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.saving))), (::std::string::String::from("notice"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.notice))), (::std::string::String::from("new_name"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.new_name))), (::std::string::String::from("draft"), ::ducktape_view_guest::wire::SnapshotValue::Bytes((&self.draft).snapshot())), (::std::string::String::from("editing"), ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.editing))), (::std::string::String::from("draft_chain"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.draft_chain))), (::std::string::String::from("draft_path"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.draft_path))), (::std::string::String::from("draft_base"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.draft_base))), (::std::string::String::from("draft_id"), ::ducktape_view_guest::wire::SnapshotValue::I64(*(&self.draft_id))), (::std::string::String::from("sent"), ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.sent))), (::std::string::String::from("viewport_width"), ::ducktape_view_guest::wire::SnapshotValue::F64(*(&self.viewport_width))), (::std::string::String::from("viewport_height"), ::ducktape_view_guest::wire::SnapshotValue::F64(*(&self.viewport_height))), (::std::string::String::from("tree_width"), ::ducktape_view_guest::wire::SnapshotValue::F64(*(&self.tree_width))), (::std::string::String::from("preview_pane_height"), ::ducktape_view_guest::wire::SnapshotValue::F64(*(&self.preview_pane_height))), (::std::string::String::from("object_width"), ::ducktape_view_guest::wire::SnapshotValue::F64(*(&self.object_width))), (::std::string::String::from("__ice_component_046696c657353637265656e"), { let __values = &self.__ice_component_046696c657353637265656e; let mut __scopes = __values.keys().collect::<::std::vec::Vec<_>>(); __scopes.sort(); ::ducktape_view_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("FilesScreen instances"), fields: __scopes.into_iter().map(|__scope| { let __component = &__values[__scope]; (__scope.clone(), ::ducktape_view_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("FilesScreen"), fields: vec![(::std::string::String::from("history_open"), ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&__component.history_open)))] }) }).collect() } }), (::std::string::String::from("__ice_component_046696c657353637265656e_initial"), { let __component = &self.__ice_component_046696c657353637265656e_initial; ::ducktape_view_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("FilesScreen"), fields: vec![(::std::string::String::from("history_open"), ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&__component.history_open)))] } })] }}.encode() }
pub(crate) fn __restore(__bytes: &[u8]) -> ::std::result::Result<Self, ::std::string::String> { let __snapshot = ::ducktape_view_guest::wire::Snapshot::decode(__bytes)?; if __snapshot.schema != Self::__SNAPSHOT_SCHEMA { return ::std::result::Result::Err(::std::string::String::from("snapshot schema mismatch")); } let __value = __snapshot.state; ((|| { let ::ducktape_view_guest::wire::SnapshotValue::Record {name: __name, fields: __fields} = __value else { return ::std::option::Option::None; }; if __name != "FilesView" || __fields.len() != 45 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __value) = __fields.next()?; if __name != "active_palette" { return ::std::option::Option::None; } let active_palette: AppTheme = ((|| { let ::ducktape_view_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __value else { return ::std::option::Option::None; }; if __name != "AppTheme" || __fields.len() != 1 { return ::std::option::Option::None; } let (__variant, __payload) = __fields.into_iter().next()?; match __variant.as_str() { "app" => matches!(__payload, ::ducktape_view_guest::wire::SnapshotValue::Unit).then_some(AppTheme::App), "app_dark" => matches!(__payload, ::ducktape_view_guest::wire::SnapshotValue::Unit).then_some(AppTheme::AppDark), _ => ::std::option::Option::None } })())?; let (__name, __value) = __fields.next()?; if __name != "connected" { return ::std::option::Option::None; } let connected: bool = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "dark" { return ::std::option::Option::None; } let dark: bool = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "chain" { return ::std::option::Option::None; } let chain: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "generation" { return ::std::option::Option::None; } let generation: i64 = (match __value { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "route_serial" { return ::std::option::Option::None; } let route_serial: i64 = (match __value { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "path" { return ::std::option::Option::None; } let path: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "listed" { return ::std::option::Option::None; } let listed: bool = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "entries" { return ::std::option::Option::None; } let entries: ::std::vec::Vec<crate::host::FsEntry> = (match __value { ::ducktape_view_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| (|| { let ::ducktape_view_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __item else { return ::std::option::Option::None; }; if __name != "FsEntry" || __fields.len() != 6 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "key" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "path" { return ::std::option::Option::None; } let (__name, __field_2) = __fields.next()?; if __name != "name" { return ::std::option::Option::None; } let (__name, __field_3) = __fields.next()?; if __name != "kind" { return ::std::option::Option::None; } let (__name, __field_4) = __fields.next()?; if __name != "size" { return ::std::option::Option::None; } let (__name, __field_5) = __fields.next()?; if __name != "object" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::FsEntry { key: (match __field_0 { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, path: (match __field_1 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, name: (match __field_2 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, kind: (match __field_3 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, size: (match __field_4 { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, object: (match __field_5 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })()).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "directories" { return ::std::option::Option::None; } let directories: ::std::vec::Vec<crate::host::FsEntry> = (match __value { ::ducktape_view_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| (|| { let ::ducktape_view_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __item else { return ::std::option::Option::None; }; if __name != "FsEntry" || __fields.len() != 6 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "key" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "path" { return ::std::option::Option::None; } let (__name, __field_2) = __fields.next()?; if __name != "name" { return ::std::option::Option::None; } let (__name, __field_3) = __fields.next()?; if __name != "kind" { return ::std::option::Option::None; } let (__name, __field_4) = __fields.next()?; if __name != "size" { return ::std::option::Option::None; } let (__name, __field_5) = __fields.next()?; if __name != "object" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::FsEntry { key: (match __field_0 { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, path: (match __field_1 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, name: (match __field_2 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, kind: (match __field_3 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, size: (match __field_4 { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, object: (match __field_5 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })()).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "history" { return ::std::option::Option::None; } let history: ::std::vec::Vec<crate::host::FsSnapshot> = (match __value { ::ducktape_view_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| (|| { let ::ducktape_view_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __item else { return ::std::option::Option::None; }; if __name != "FsSnapshot" || __fields.len() != 5 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "id" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "short_id" { return ::std::option::Option::None; } let (__name, __field_2) = __fields.next()?; if __name != "author" { return ::std::option::Option::None; } let (__name, __field_3) = __fields.next()?; if __name != "height" { return ::std::option::Option::None; } let (__name, __field_4) = __fields.next()?; if __name != "message" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::FsSnapshot { id: (match __field_0 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, short_id: (match __field_1 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, author: (match __field_2 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, height: (match __field_3 { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, message: (match __field_4 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })()).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "omitted" { return ::std::option::Option::None; } let omitted: i64 = (match __value { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "diff_omitted" { return ::std::option::Option::None; } let diff_omitted: i64 = (match __value { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "preview_path" { return ::std::option::Option::None; } let preview_path: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "preview_entry" { return ::std::option::Option::None; } let preview_entry: crate::host::FsEntry = ((|| { let ::ducktape_view_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __value else { return ::std::option::Option::None; }; if __name != "FsEntry" || __fields.len() != 6 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "key" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "path" { return ::std::option::Option::None; } let (__name, __field_2) = __fields.next()?; if __name != "name" { return ::std::option::Option::None; } let (__name, __field_3) = __fields.next()?; if __name != "kind" { return ::std::option::Option::None; } let (__name, __field_4) = __fields.next()?; if __name != "size" { return ::std::option::Option::None; } let (__name, __field_5) = __fields.next()?; if __name != "object" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::FsEntry { key: (match __field_0 { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, path: (match __field_1 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, name: (match __field_2 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, kind: (match __field_3 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, size: (match __field_4 { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, object: (match __field_5 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })())?; let (__name, __value) = __fields.next()?; if __name != "preview_base" { return ::std::option::Option::None; } let preview_base: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "preview_text" { return ::std::option::Option::None; } let preview_text: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "preview_display_text" { return ::std::option::Option::None; } let preview_display_text: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "preview_clipped" { return ::std::option::Option::None; } let preview_clipped: bool = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "preview_truncated" { return ::std::option::Option::None; } let preview_truncated: bool = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "preview_binary" { return ::std::option::Option::None; } let preview_binary: bool = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "preview_picture" { return ::std::option::Option::None; } let preview_picture: bool = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "preview_width" { return ::std::option::Option::None; } let preview_width: i64 = (match __value { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "preview_height" { return ::std::option::Option::None; } let preview_height: i64 = (match __value { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "delete_target" { return ::std::option::Option::None; } let delete_target: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "diff_from" { return ::std::option::Option::None; } let diff_from: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "diff" { return ::std::option::Option::None; } let diff: ::std::vec::Vec<crate::host::FsDiffEntry> = (match __value { ::ducktape_view_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| (|| { let ::ducktape_view_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __item else { return ::std::option::Option::None; }; if __name != "FsDiffEntry" || __fields.len() != 2 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "path" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "kind" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::FsDiffEntry { path: (match __field_0 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, kind: (match __field_1 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })()).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "acting" { return ::std::option::Option::None; } let acting: bool = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "saving" { return ::std::option::Option::None; } let saving: bool = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "notice" { return ::std::option::Option::None; } let notice: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "new_name" { return ::std::option::Option::None; } let new_name: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "draft" { return ::std::option::Option::None; } let draft: ::ducktape_view_guest::Editor = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Bytes(bytes) => ::ducktape_view_guest::Editor::restore(&bytes), _ => None })?; let (__name, __value) = __fields.next()?; if __name != "editing" { return ::std::option::Option::None; } let editing: bool = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "draft_chain" { return ::std::option::Option::None; } let draft_chain: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "draft_path" { return ::std::option::Option::None; } let draft_path: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "draft_base" { return ::std::option::Option::None; } let draft_base: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "draft_id" { return ::std::option::Option::None; } let draft_id: i64 = (match __value { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "sent" { return ::std::option::Option::None; } let sent: bool = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "viewport_width" { return ::std::option::Option::None; } let viewport_width: f64 = (match __value { ::ducktape_view_guest::wire::SnapshotValue::F64(__item) if __item.is_finite() => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "viewport_height" { return ::std::option::Option::None; } let viewport_height: f64 = (match __value { ::ducktape_view_guest::wire::SnapshotValue::F64(__item) if __item.is_finite() => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "tree_width" { return ::std::option::Option::None; } let tree_width: f64 = (match __value { ::ducktape_view_guest::wire::SnapshotValue::F64(__item) if __item.is_finite() => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "preview_pane_height" { return ::std::option::Option::None; } let preview_pane_height: f64 = (match __value { ::ducktape_view_guest::wire::SnapshotValue::F64(__item) if __item.is_finite() => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "object_width" { return ::std::option::Option::None; } let object_width: f64 = (match __value { ::ducktape_view_guest::wire::SnapshotValue::F64(__item) if __item.is_finite() => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "__ice_component_046696c657353637265656e" { return ::std::option::Option::None; } let __ice_component_046696c657353637265656e: ::std::collections::HashMap<::std::string::String, __IceFilesScreenState> = ((|| { let ::ducktape_view_guest::wire::SnapshotValue::Record {name: __name, fields: __fields} = __value else { return ::std::option::Option::None; }; if __name != "FilesScreen instances" { return ::std::option::Option::None; } let mut __values = ::std::collections::HashMap::new(); for (__scope, __value) in __fields { let __component = ((|| { let ::ducktape_view_guest::wire::SnapshotValue::Record {name: __name, fields: __fields} = __value else { return ::std::option::Option::None; }; if __name != "FilesScreen" || __fields.len() != 1 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __value) = __fields.next()?; if __name != "history_open" { return ::std::option::Option::None; } let history_open: bool = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; ::std::option::Option::Some(__IceFilesScreenState {history_open: history_open,__ice_rev: [::ducktape_view_guest::rev::seed(); 1],}) })())?; if __values.insert(__scope, __component).is_some() { return ::std::option::Option::None; } } ::std::option::Option::Some(__values) })())?; let (__name, __value) = __fields.next()?; if __name != "__ice_component_046696c657353637265656e_initial" { return ::std::option::Option::None; } let __ice_component_046696c657353637265656e_initial: __IceFilesScreenState = ((|| { let ::ducktape_view_guest::wire::SnapshotValue::Record {name: __name, fields: __fields} = __value else { return ::std::option::Option::None; }; if __name != "FilesScreen" || __fields.len() != 1 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __value) = __fields.next()?; if __name != "history_open" { return ::std::option::Option::None; } let history_open: bool = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; ::std::option::Option::Some(__IceFilesScreenState {history_open: history_open,__ice_rev: [::ducktape_view_guest::rev::seed(); 1],}) })())?; ::std::option::Option::Some(Self::__restore_state(active_palette, connected, dark, chain, generation, route_serial, path, listed, entries, directories, history, omitted, diff_omitted, preview_path, preview_entry, preview_base, preview_text, preview_display_text, preview_clipped, preview_truncated, preview_binary, preview_picture, preview_width, preview_height, delete_target, diff_from, diff, acting, saving, notice, new_name, draft, editing, draft_chain, draft_path, draft_base, draft_id, sent, viewport_width, viewport_height, tree_width, preview_pane_height, object_width, __ice_component_046696c657353637265656e, __ice_component_046696c657353637265656e_initial)) })()).ok_or_else(|| ::std::string::String::from("snapshot state mismatch")) }
}
}
__ice_generated_items_46696c657356696577! {
#[allow(unused_parens)]
impl FilesView {
}
}
__ice_generated_items_46696c657356696577! {
#[allow(unused_parens)]
impl FilesView {
fn __subscription(&self) -> ::ducktape_view_guest::Subscription<__FilesViewMessage> {
::ducktape_view_guest::Subscription::batch([
crate::host::session().map(move |__value| __FilesViewMessage::SessionArrived(__value)),
if self.connected { ::ducktape_view_guest::Subscription::batch([crate::host::listing(self.generation, self.path.to_owned()).map(move |__value| __FilesViewMessage::ListingArrived(__value)),
]) } else { ::ducktape_view_guest::Subscription::none() },
if (self.connected && (!(self.preview_path).is_empty())) { ::ducktape_view_guest::Subscription::batch([crate::host::preview(self.generation, self.preview_path.to_owned()).map(move |__value| __FilesViewMessage::PreviewArrived(__value)),
]) } else { ::ducktape_view_guest::Subscription::none() },
if (self.connected && (!(self.diff_from).is_empty())) { ::ducktape_view_guest::Subscription::batch([crate::host::diff(self.generation, self.diff_from.to_owned()).map(move |__value| __FilesViewMessage::DiffArrived(__value)),
]) } else { ::ducktape_view_guest::Subscription::none() },
crate::host::acts().map(move |__value| __FilesViewMessage::ActDone(__value)),
])
}
}
}
__ice_generated_items_46696c657356696577! {
#[allow(unused_parens)]
impl FilesView {
}
#[cfg(test)] mod __ice_tests { use super::*;
#[test]
fn __ice_view_fits_default_stack() {
::std::thread::Builder::new().stack_size(4 * 1024 * 1024).spawn(|| {
let (__app, _) = FilesView::__boot();
let _ = __app.__view();
}).unwrap().join().unwrap();
}
}
}
include!("app_update.rs");
include!("app_view.rs");
include!("browser.rs");
include!("files.rs");
include!("icon.rs");
include!("kit.rs");
