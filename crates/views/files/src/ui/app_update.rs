#[allow(warnings, clippy::all)]
mod __ice_group_app_update {
    use super::*;
    impl super::FilesView {
        #[allow(clippy::assign_op_pattern)]
        pub(super) fn __update(
            &mut self,
            message: __FilesViewMessage,
        ) -> ::ducktape_view_guest::Task<__FilesViewMessage> {
            match message {
__FilesViewMessage::TreeResized(dx, _dy) => (|| {

let _ = &dx;
let _ = &_dy;
{ let __ice_next = crate::host::tree_width_after_delta(self.tree_width, dx, self.viewport_width, self.object_width); if ::ducktape_view_guest::state_changed!(self.tree_width, __ice_next) { self.tree_width = __ice_next; self.__ice_rev[40] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__FilesViewMessage::PreviewResized(_dx, dy) => (|| {

let _ = &_dx;
let _ = &dy;
{ let __ice_next = crate::host::preview_height_after_delta(self.preview_pane_height, (-dy), self.viewport_height); if ::ducktape_view_guest::state_changed!(self.preview_pane_height, __ice_next) { self.preview_pane_height = __ice_next; self.__ice_rev[41] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__FilesViewMessage::ObjectResized(dx, _dy) => (|| {

let _ = &dx;
let _ = &_dy;
{ let __ice_next = crate::host::object_width_after_delta(self.object_width, (-dx), self.viewport_width, self.tree_width); if ::ducktape_view_guest::state_changed!(self.object_width, __ice_next) { self.object_width = __ice_next; self.__ice_rev[42] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__FilesViewMessage::ViewportChanged(width, height) => (|| {

let _ = &width;
let _ = &height;
{ let __ice_next = width; if ::ducktape_view_guest::state_changed!(self.viewport_width, __ice_next) { self.viewport_width = __ice_next; self.__ice_rev[38] += 1; } }
{ let __ice_next = height; if ::ducktape_view_guest::state_changed!(self.viewport_height, __ice_next) { self.viewport_height = __ice_next; self.__ice_rev[39] += 1; } }
{ let __ice_next = crate::host::tree_width_after_delta(self.tree_width, 0.0, width, self.object_width); if ::ducktape_view_guest::state_changed!(self.tree_width, __ice_next) { self.tree_width = __ice_next; self.__ice_rev[40] += 1; } }
{ let __ice_next = crate::host::preview_height_after_delta(self.preview_pane_height, 0.0, height); if ::ducktape_view_guest::state_changed!(self.preview_pane_height, __ice_next) { self.preview_pane_height = __ice_next; self.__ice_rev[41] += 1; } }
{ let __ice_next = crate::host::object_width_after_delta(self.object_width, 0.0, width, self.tree_width); if ::ducktape_view_guest::state_changed!(self.object_width, __ice_next) { self.object_width = __ice_next; self.__ice_rev[42] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__FilesViewMessage::SessionArrived(item) => (|| {

let _ = &item;
{ let __ice_next = crate::host::keep_str((!(item.error).is_empty()), ::std::convert::AsRef::as_ref(&(item.error)), ::std::convert::AsRef::as_ref(&(self.notice))); if ::ducktape_view_guest::state_changed!(self.notice, __ice_next) { self.notice = __ice_next; self.__ice_rev[29] += 1; } }
if (!(item.error).is_empty()) { return ::ducktape_view_guest::Task::none(); }
let next = item.next.clone();
{ let __ice_next = crate::host::generation_after(self.connected, next.connected, self.generation); if ::ducktape_view_guest::state_changed!(self.generation, __ice_next) { self.generation = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = (self.listed && next.connected); if ::ducktape_view_guest::state_changed!(self.listed, __ice_next) { self.listed = __ice_next; self.__ice_rev[7] += 1; self.__ice_derived.loading.take(); } }
{ let __ice_next = next.connected; if ::ducktape_view_guest::state_changed!(self.connected, __ice_next) { self.connected = __ice_next; self.__ice_rev[1] += 1; self.__ice_derived.loading.take(); } }
{ let __ice_next = next.chain.to_owned(); if ::ducktape_view_guest::state_changed!(self.chain, __ice_next) { self.chain = __ice_next; self.__ice_rev[3] += 1; self.__ice_derived.draft_here.take(); self.__ice_derived.draft_parked.take(); self.__ice_derived.edit_context.take(); } }
{ let __ice_next = next.dark; if ::ducktape_view_guest::state_changed!(self.dark, __ice_next) { self.dark = __ice_next; self.__ice_rev[2] += 1; } }
let routed = ((next.route_serial != self.route_serial) && (!(next.route).is_empty()));
{ let __ice_next = next.route_serial; if ::ducktape_view_guest::state_changed!(self.route_serial, __ice_next) { self.route_serial = __ice_next; self.__ice_rev[5] += 1; } }
let landing = crate::host::keep_str(routed, ::std::convert::AsRef::as_ref(&(next.route)), ::std::convert::AsRef::as_ref(&("")));
return match crate::host::tone_of(next.dark) {
Tone::Light => (|| {
{ let __ice_next = AppTheme::App; if ::ducktape_view_guest::state_changed!(self.active_palette, __ice_next) { self.active_palette = __ice_next; self.__ice_rev[0] += 1; } }
return (::ducktape_view_guest::Task::done(landing.to_owned())).map(|value| __FilesViewMessage::RouteTo(value));
})(),
Tone::Dark => (|| {
{ let __ice_next = AppTheme::AppDark; if ::ducktape_view_guest::state_changed!(self.active_palette, __ice_next) { self.active_palette = __ice_next; self.__ice_rev[0] += 1; } }
return (::ducktape_view_guest::Task::done(landing.to_owned())).map(|value| __FilesViewMessage::RouteTo(value));
})(),
};
})(),
__FilesViewMessage::RouteTo(target) => (|| {

let _ = &target;
if (target).is_empty() { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.notice, __ice_next) { self.notice = __ice_next; self.__ice_rev[29] += 1; } }
{ let __ice_next = (self.generation + 1); if ::ducktape_view_guest::state_changed!(self.generation, __ice_next) { self.generation = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = crate::host::fs_parent(::std::convert::AsRef::as_ref(&(target))); if ::ducktape_view_guest::state_changed!(self.path, __ice_next) { self.path = __ice_next; self.__ice_rev[6] += 1; self.__ice_derived.refusal.take(); } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.listed, __ice_next) { self.listed = __ice_next; self.__ice_rev[7] += 1; self.__ice_derived.loading.take(); } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ducktape_view_guest::state_changed!(self.entries, __ice_next) { self.entries = __ice_next; self.__ice_rev[8] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ducktape_view_guest::state_changed!(self.directories, __ice_next) { self.directories = __ice_next; self.__ice_rev[9] += 1; } }
{ let __ice_next = 0; if ::ducktape_view_guest::state_changed!(self.omitted, __ice_next) { self.omitted = __ice_next; self.__ice_rev[11] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.diff_from, __ice_next) { self.diff_from = __ice_next; self.__ice_rev[25] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ducktape_view_guest::state_changed!(self.diff, __ice_next) { self.diff = __ice_next; self.__ice_rev[26] += 1; } }
{ let __ice_next = 0; if ::ducktape_view_guest::state_changed!(self.diff_omitted, __ice_next) { self.diff_omitted = __ice_next; self.__ice_rev[12] += 1; } }
{ let __ice_next = target.to_owned(); if ::ducktape_view_guest::state_changed!(self.preview_path, __ice_next) { self.preview_path = __ice_next; self.__ice_rev[13] += 1; self.__ice_derived.draft_here.take(); self.__ice_derived.draft_parked.take(); self.__ice_derived.edit_context.take(); } }
{ let __ice_next = crate::host::no_fs_entry(); if ::ducktape_view_guest::state_changed!(self.preview_entry, __ice_next) { self.preview_entry = __ice_next; self.__ice_rev[14] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.preview_base, __ice_next) { self.preview_base = __ice_next; self.__ice_rev[15] += 1; self.__ice_derived.edit_context.take(); } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.preview_text, __ice_next) { self.preview_text = __ice_next; self.__ice_rev[16] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.preview_display_text, __ice_next) { self.preview_display_text = __ice_next; self.__ice_rev[17] += 1; } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.preview_clipped, __ice_next) { self.preview_clipped = __ice_next; self.__ice_rev[18] += 1; } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.preview_truncated, __ice_next) { self.preview_truncated = __ice_next; self.__ice_rev[19] += 1; } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.preview_binary, __ice_next) { self.preview_binary = __ice_next; self.__ice_rev[20] += 1; } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.preview_picture, __ice_next) { self.preview_picture = __ice_next; self.__ice_rev[21] += 1; } }
{ let __ice_next = 0; if ::ducktape_view_guest::state_changed!(self.preview_width, __ice_next) { self.preview_width = __ice_next; self.__ice_rev[22] += 1; } }
{ let __ice_next = 0; if ::ducktape_view_guest::state_changed!(self.preview_height, __ice_next) { self.preview_height = __ice_next; self.__ice_rev[23] += 1; } }
{ let __ice_next = ({ crate::host::at(::std::convert::AsRef::as_ref(&(self.path))) }); if ::ducktape_view_guest::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[37] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__FilesViewMessage::ListingArrived(item) => (|| {

let _ = &item;
{ let __ice_next = crate::host::keep_str((!(item.error).is_empty()), ::std::convert::AsRef::as_ref(&(item.error)), ::std::convert::AsRef::as_ref(&(self.notice))); if ::ducktape_view_guest::state_changed!(self.notice, __ice_next) { self.notice = __ice_next; self.__ice_rev[29] += 1; } }
if (!(item.error).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = true; if ::ducktape_view_guest::state_changed!(self.listed, __ice_next) { self.listed = __ice_next; self.__ice_rev[7] += 1; self.__ice_derived.loading.take(); } }
{ let __ice_next = item.entries.clone(); if ::ducktape_view_guest::state_changed!(self.entries, __ice_next) { self.entries = __ice_next; self.__ice_rev[8] += 1; } }
{ let __ice_next = item.directories.clone(); if ::ducktape_view_guest::state_changed!(self.directories, __ice_next) { self.directories = __ice_next; self.__ice_rev[9] += 1; } }
{ let __ice_next = item.history.clone(); if ::ducktape_view_guest::state_changed!(self.history, __ice_next) { self.history = __ice_next; self.__ice_rev[10] += 1; } }
{ let __ice_next = item.omitted; if ::ducktape_view_guest::state_changed!(self.omitted, __ice_next) { self.omitted = __ice_next; self.__ice_rev[11] += 1; } }
{ let __ice_next = crate::host::entry_named(::std::convert::AsRef::as_ref(&(item.entries)), ::std::convert::AsRef::as_ref(&(self.preview_path))); if ::ducktape_view_guest::state_changed!(self.preview_entry, __ice_next) { self.preview_entry = __ice_next; self.__ice_rev[14] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__FilesViewMessage::PreviewArrived(item) => (|| {

let _ = &item;
if (item.path != self.preview_path) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = crate::host::keep_str((!(item.error).is_empty()), ::std::convert::AsRef::as_ref(&(item.error)), ::std::convert::AsRef::as_ref(&(self.notice))); if ::ducktape_view_guest::state_changed!(self.notice, __ice_next) { self.notice = __ice_next; self.__ice_rev[29] += 1; } }
if (!(item.error).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = item.base.to_owned(); if ::ducktape_view_guest::state_changed!(self.preview_base, __ice_next) { self.preview_base = __ice_next; self.__ice_rev[15] += 1; self.__ice_derived.edit_context.take(); } }
{ let __ice_next = item.text.to_owned(); if ::ducktape_view_guest::state_changed!(self.preview_text, __ice_next) { self.preview_text = __ice_next; self.__ice_rev[16] += 1; } }
{ let __ice_next = item.display_text.to_owned(); if ::ducktape_view_guest::state_changed!(self.preview_display_text, __ice_next) { self.preview_display_text = __ice_next; self.__ice_rev[17] += 1; } }
{ let __ice_next = item.clipped; if ::ducktape_view_guest::state_changed!(self.preview_clipped, __ice_next) { self.preview_clipped = __ice_next; self.__ice_rev[18] += 1; } }
{ let __ice_next = item.truncated; if ::ducktape_view_guest::state_changed!(self.preview_truncated, __ice_next) { self.preview_truncated = __ice_next; self.__ice_rev[19] += 1; } }
{ let __ice_next = item.binary; if ::ducktape_view_guest::state_changed!(self.preview_binary, __ice_next) { self.preview_binary = __ice_next; self.__ice_rev[20] += 1; } }
{ let __ice_next = item.picture; if ::ducktape_view_guest::state_changed!(self.preview_picture, __ice_next) { self.preview_picture = __ice_next; self.__ice_rev[21] += 1; } }
{ let __ice_next = item.width; if ::ducktape_view_guest::state_changed!(self.preview_width, __ice_next) { self.preview_width = __ice_next; self.__ice_rev[22] += 1; } }
{ let __ice_next = item.height; if ::ducktape_view_guest::state_changed!(self.preview_height, __ice_next) { self.preview_height = __ice_next; self.__ice_rev[23] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__FilesViewMessage::DiffArrived(item) => (|| {

let _ = &item;
{ let __ice_next = crate::host::keep_str((!(item.error).is_empty()), ::std::convert::AsRef::as_ref(&(item.error)), ::std::convert::AsRef::as_ref(&(self.notice))); if ::ducktape_view_guest::state_changed!(self.notice, __ice_next) { self.notice = __ice_next; self.__ice_rev[29] += 1; } }
if (!(item.error).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = item.entries.clone(); if ::ducktape_view_guest::state_changed!(self.diff, __ice_next) { self.diff = __ice_next; self.__ice_rev[26] += 1; } }
{ let __ice_next = item.omitted; if ::ducktape_view_guest::state_changed!(self.diff_omitted, __ice_next) { self.diff_omitted = __ice_next; self.__ice_rev[12] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__FilesViewMessage::ActDone(item) => (|| {

let _ = &item;
{ let __ice_next = item.error.to_owned(); if ::ducktape_view_guest::state_changed!(self.notice, __ice_next) { self.notice = __ice_next; self.__ice_rev[29] += 1; } }
let ok = (item.error).is_empty();
let saved = (item.kind == "save");
let named = ((item.kind == "mkdir") || (item.kind == "new_file"));
{ let __ice_next = (self.acting && saved); if ::ducktape_view_guest::state_changed!(self.acting, __ice_next) { self.acting = __ice_next; self.__ice_rev[27] += 1; self.__ice_derived.loading.take(); } }
{ let __ice_next = (self.saving && (!saved)); if ::ducktape_view_guest::state_changed!(self.saving, __ice_next) { self.saving = __ice_next; self.__ice_rev[28] += 1; self.__ice_derived.loading.take(); } }
{ let __ice_next = (self.editing && (!(saved && ok))); if ::ducktape_view_guest::state_changed!(self.editing, __ice_next) { self.editing = __ice_next; self.__ice_rev[32] += 1; self.__ice_derived.draft_here.take(); self.__ice_derived.draft_parked.take(); } }
{ let __ice_next = crate::host::keep_str(((item.kind == "delete") && ok), ::std::convert::AsRef::as_ref(&("")), ::std::convert::AsRef::as_ref(&(self.delete_target))); if ::ducktape_view_guest::state_changed!(self.delete_target, __ice_next) { self.delete_target = __ice_next; self.__ice_rev[24] += 1; } }
{ let __ice_next = crate::host::keep_draft((named && ok), ::std::convert::AsRef::as_ref(&(self.new_name))); if ::ducktape_view_guest::state_changed!(self.new_name, __ice_next) { self.new_name = __ice_next; self.__ice_rev[30] += 1; } }
{ let __ice_next = (self.generation + 1); if ::ducktape_view_guest::state_changed!(self.generation, __ice_next) { self.generation = __ice_next; self.__ice_rev[4] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__FilesViewMessage::OpenDirAt(target) => (|| {

let _ = &target;
if (((*self.__ice_derived_loading()) || (!self.connected)) || (target == self.path)) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.notice, __ice_next) { self.notice = __ice_next; self.__ice_rev[29] += 1; } }
{ let __ice_next = target.to_owned(); if ::ducktape_view_guest::state_changed!(self.path, __ice_next) { self.path = __ice_next; self.__ice_rev[6] += 1; self.__ice_derived.refusal.take(); } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.listed, __ice_next) { self.listed = __ice_next; self.__ice_rev[7] += 1; self.__ice_derived.loading.take(); } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ducktape_view_guest::state_changed!(self.entries, __ice_next) { self.entries = __ice_next; self.__ice_rev[8] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ducktape_view_guest::state_changed!(self.directories, __ice_next) { self.directories = __ice_next; self.__ice_rev[9] += 1; } }
{ let __ice_next = 0; if ::ducktape_view_guest::state_changed!(self.omitted, __ice_next) { self.omitted = __ice_next; self.__ice_rev[11] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.diff_from, __ice_next) { self.diff_from = __ice_next; self.__ice_rev[25] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ducktape_view_guest::state_changed!(self.diff, __ice_next) { self.diff = __ice_next; self.__ice_rev[26] += 1; } }
{ let __ice_next = 0; if ::ducktape_view_guest::state_changed!(self.diff_omitted, __ice_next) { self.diff_omitted = __ice_next; self.__ice_rev[12] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.preview_path, __ice_next) { self.preview_path = __ice_next; self.__ice_rev[13] += 1; self.__ice_derived.draft_here.take(); self.__ice_derived.draft_parked.take(); self.__ice_derived.edit_context.take(); } }
{ let __ice_next = crate::host::no_fs_entry(); if ::ducktape_view_guest::state_changed!(self.preview_entry, __ice_next) { self.preview_entry = __ice_next; self.__ice_rev[14] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.preview_base, __ice_next) { self.preview_base = __ice_next; self.__ice_rev[15] += 1; self.__ice_derived.edit_context.take(); } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.preview_text, __ice_next) { self.preview_text = __ice_next; self.__ice_rev[16] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.preview_display_text, __ice_next) { self.preview_display_text = __ice_next; self.__ice_rev[17] += 1; } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.preview_picture, __ice_next) { self.preview_picture = __ice_next; self.__ice_rev[21] += 1; } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.preview_binary, __ice_next) { self.preview_binary = __ice_next; self.__ice_rev[20] += 1; } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.preview_truncated, __ice_next) { self.preview_truncated = __ice_next; self.__ice_rev[19] += 1; } }
{ let __ice_next = ({ crate::host::at(::std::convert::AsRef::as_ref(&(self.path))) }); if ::ducktape_view_guest::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[37] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__FilesViewMessage::OpenFileAt(target) => (|| {

let _ = &target;
if (((*self.__ice_derived_loading()) || (!self.connected)) || (target == self.preview_path)) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.notice, __ice_next) { self.notice = __ice_next; self.__ice_rev[29] += 1; } }
{ let __ice_next = target.to_owned(); if ::ducktape_view_guest::state_changed!(self.preview_path, __ice_next) { self.preview_path = __ice_next; self.__ice_rev[13] += 1; self.__ice_derived.draft_here.take(); self.__ice_derived.draft_parked.take(); self.__ice_derived.edit_context.take(); } }
{ let __ice_next = crate::host::entry_named(::std::convert::AsRef::as_ref(&(self.entries)), ::std::convert::AsRef::as_ref(&(target))); if ::ducktape_view_guest::state_changed!(self.preview_entry, __ice_next) { self.preview_entry = __ice_next; self.__ice_rev[14] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.preview_base, __ice_next) { self.preview_base = __ice_next; self.__ice_rev[15] += 1; self.__ice_derived.edit_context.take(); } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.preview_text, __ice_next) { self.preview_text = __ice_next; self.__ice_rev[16] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.preview_display_text, __ice_next) { self.preview_display_text = __ice_next; self.__ice_rev[17] += 1; } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.preview_clipped, __ice_next) { self.preview_clipped = __ice_next; self.__ice_rev[18] += 1; } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.preview_truncated, __ice_next) { self.preview_truncated = __ice_next; self.__ice_rev[19] += 1; } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.preview_binary, __ice_next) { self.preview_binary = __ice_next; self.__ice_rev[20] += 1; } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.preview_picture, __ice_next) { self.preview_picture = __ice_next; self.__ice_rev[21] += 1; } }
{ let __ice_next = 0; if ::ducktape_view_guest::state_changed!(self.preview_width, __ice_next) { self.preview_width = __ice_next; self.__ice_rev[22] += 1; } }
{ let __ice_next = 0; if ::ducktape_view_guest::state_changed!(self.preview_height, __ice_next) { self.preview_height = __ice_next; self.__ice_rev[23] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__FilesViewMessage::MkdirSubmit => (|| {

if ((((*self.__ice_derived_loading()) || (!self.connected)) || ((self.new_name).trim().to_owned()).is_empty()) || (!((*self.__ice_derived_refusal())).is_empty())) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.notice, __ice_next) { self.notice = __ice_next; self.__ice_rev[29] += 1; } }
{ let __ice_next = true; if ::ducktape_view_guest::state_changed!(self.acting, __ice_next) { self.acting = __ice_next; self.__ice_rev[27] += 1; self.__ice_derived.loading.take(); } }
{ let __ice_next = ({ crate::host::make_dir(::std::convert::AsRef::as_ref(&(self.path)), ::std::convert::AsRef::as_ref(&((self.new_name).trim().to_owned()))) }); if ::ducktape_view_guest::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[37] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__FilesViewMessage::NewFileSubmit => (|| {

if ((((*self.__ice_derived_loading()) || (!self.connected)) || ((self.new_name).trim().to_owned()).is_empty()) || (!((*self.__ice_derived_refusal())).is_empty())) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.notice, __ice_next) { self.notice = __ice_next; self.__ice_rev[29] += 1; } }
{ let __ice_next = true; if ::ducktape_view_guest::state_changed!(self.acting, __ice_next) { self.acting = __ice_next; self.__ice_rev[27] += 1; self.__ice_derived.loading.take(); } }
{ let __ice_next = ({ crate::host::make_file(::std::convert::AsRef::as_ref(&(self.path)), ::std::convert::AsRef::as_ref(&((self.new_name).trim().to_owned()))) }); if ::ducktape_view_guest::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[37] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__FilesViewMessage::ArmDeleteAt(target) => (|| {

let _ = &target;
{ let __ice_next = target.to_owned(); if ::ducktape_view_guest::state_changed!(self.delete_target, __ice_next) { self.delete_target = __ice_next; self.__ice_rev[24] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__FilesViewMessage::DisarmDeleteNow => (|| {

{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.delete_target, __ice_next) { self.delete_target = __ice_next; self.__ice_rev[24] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__FilesViewMessage::DeleteSubmit => (|| {

if (((*self.__ice_derived_loading()) || (!self.connected)) || (self.delete_target).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = crate::host::write_refusal(::std::convert::AsRef::as_ref(&(crate::host::fs_parent(::std::convert::AsRef::as_ref(&(self.delete_target)))))); if ::ducktape_view_guest::state_changed!(self.notice, __ice_next) { self.notice = __ice_next; self.__ice_rev[29] += 1; } }
if (!(self.notice).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = true; if ::ducktape_view_guest::state_changed!(self.acting, __ice_next) { self.acting = __ice_next; self.__ice_rev[27] += 1; self.__ice_derived.loading.take(); } }
{ let __ice_next = ({ crate::host::delete_object(::std::convert::AsRef::as_ref(&(self.delete_target))) }); if ::ducktape_view_guest::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[37] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__FilesViewMessage::CloseDiffNow => (|| {

{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.diff_from, __ice_next) { self.diff_from = __ice_next; self.__ice_rev[25] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ducktape_view_guest::state_changed!(self.diff, __ice_next) { self.diff = __ice_next; self.__ice_rev[26] += 1; } }
{ let __ice_next = 0; if ::ducktape_view_guest::state_changed!(self.diff_omitted, __ice_next) { self.diff_omitted = __ice_next; self.__ice_rev[12] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__FilesViewMessage::ShowDiffOf(id) => (|| {

let _ = &id;
if ((*self.__ice_derived_loading()) || (!self.connected)) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.notice, __ice_next) { self.notice = __ice_next; self.__ice_rev[29] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ducktape_view_guest::state_changed!(self.diff, __ice_next) { self.diff = __ice_next; self.__ice_rev[26] += 1; } }
{ let __ice_next = 0; if ::ducktape_view_guest::state_changed!(self.diff_omitted, __ice_next) { self.diff_omitted = __ice_next; self.__ice_rev[12] += 1; } }
{ let __ice_next = id.to_owned(); if ::ducktape_view_guest::state_changed!(self.diff_from, __ice_next) { self.diff_from = __ice_next; self.__ice_rev[25] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__FilesViewMessage::BeginEdit(token) => (|| {

let _ = &token;
if ((((((((((token != (*self.__ice_derived_edit_context())) || self.editing) || (*self.__ice_derived_loading())) || (!self.connected)) || (self.chain).is_empty()) || (self.preview_base).is_empty()) || self.preview_binary) || self.preview_picture) || self.preview_truncated) || (self.preview_path).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = true; if ::ducktape_view_guest::state_changed!(self.editing, __ice_next) { self.editing = __ice_next; self.__ice_rev[32] += 1; self.__ice_derived.draft_here.take(); self.__ice_derived.draft_parked.take(); } }
{ let __ice_next = (self.draft_id + 1); if ::ducktape_view_guest::state_changed!(self.draft_id, __ice_next) { self.draft_id = __ice_next; self.__ice_rev[36] += 1; self.__ice_derived.edit_context.take(); } }
{ let __ice_next = self.chain.to_owned(); if ::ducktape_view_guest::state_changed!(self.draft_chain, __ice_next) { self.draft_chain = __ice_next; self.__ice_rev[33] += 1; self.__ice_derived.draft_here.take(); self.__ice_derived.draft_parked.take(); } }
{ let __ice_next = self.preview_path.to_owned(); if ::ducktape_view_guest::state_changed!(self.draft_path, __ice_next) { self.draft_path = __ice_next; self.__ice_rev[34] += 1; self.__ice_derived.draft_here.take(); self.__ice_derived.draft_parked.take(); } }
{ let __ice_next = self.preview_base.to_owned(); if ::ducktape_view_guest::state_changed!(self.draft_base, __ice_next) { self.draft_base = __ice_next; self.__ice_rev[35] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.notice, __ice_next) { self.notice = __ice_next; self.__ice_rev[29] += 1; } }
{ let __reset = self.draft.reset_revision(); let __next = ::ducktape_view_guest::Editor::new(self.preview_text.to_owned()); self.draft.replace(__next, __reset); }; self.__ice_rev[31] += 1;
::ducktape_view_guest::Task::none()
})(),
__FilesViewMessage::CancelEdit(token) => (|| {

let _ = &token;
if (token != (*self.__ice_derived_edit_context())) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.editing, __ice_next) { self.editing = __ice_next; self.__ice_rev[32] += 1; self.__ice_derived.draft_here.take(); self.__ice_derived.draft_parked.take(); } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.saving, __ice_next) { self.saving = __ice_next; self.__ice_rev[28] += 1; self.__ice_derived.loading.take(); } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.notice, __ice_next) { self.notice = __ice_next; self.__ice_rev[29] += 1; } }
{ let __reset = self.draft.reset_revision(); let __next = ::ducktape_view_guest::Editor::new("".to_owned()); self.draft.replace(__next, __reset); }; self.__ice_rev[31] += 1;
::ducktape_view_guest::Task::none()
})(),
__FilesViewMessage::DiscardDraft(id) => (|| {

let _ = &id;
if (id != self.draft_id) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.editing, __ice_next) { self.editing = __ice_next; self.__ice_rev[32] += 1; self.__ice_derived.draft_here.take(); self.__ice_derived.draft_parked.take(); } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.saving, __ice_next) { self.saving = __ice_next; self.__ice_rev[28] += 1; self.__ice_derived.loading.take(); } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.notice, __ice_next) { self.notice = __ice_next; self.__ice_rev[29] += 1; } }
{ let __reset = self.draft.reset_revision(); let __next = ::ducktape_view_guest::Editor::new("".to_owned()); self.draft.replace(__next, __reset); }; self.__ice_rev[31] += 1;
::ducktape_view_guest::Task::none()
})(),
__FilesViewMessage::SaveEdit(token) => (|| {

let _ = &token;
if (((((token != (*self.__ice_derived_edit_context())) || (!(*self.__ice_derived_draft_here()))) || (*self.__ice_derived_loading())) || (!self.connected)) || (self.draft_base).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.notice, __ice_next) { self.notice = __ice_next; self.__ice_rev[29] += 1; } }
{ let __ice_next = true; if ::ducktape_view_guest::state_changed!(self.saving, __ice_next) { self.saving = __ice_next; self.__ice_rev[28] += 1; self.__ice_derived.loading.take(); } }
{ let __ice_next = ({ crate::host::save(::std::convert::AsRef::as_ref(&(self.draft_path)), ::std::convert::AsRef::as_ref(&(self.draft_base)), ::std::convert::AsRef::as_ref(&((self.draft).text()))) }); if ::ducktape_view_guest::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[37] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__FilesViewMessage::OpenLinkAt(url) => (|| {

let _ = &url;
{ let __ice_next = ({ crate::host::open_link(::std::convert::AsRef::as_ref(&(url))) }); if ::ducktape_view_guest::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[37] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__FilesViewMessage::__0C46696c657353637265656eH66735f746f67676c655f686973746f7279(__scope) => (|| {

::ducktape_view_guest::invalidate_component("FilesScreen", &(__scope.clone())); let __local = self.__ice_component_046696c657353637265656e.entry(__scope.clone()).or_insert_with(|| __IceFilesScreenState {history_open: self.__ice_component_046696c657353637265656e_initial.history_open.clone(),});
__local.history_open = (!__local.history_open);
::ducktape_view_guest::Task::none()
})(),
__FilesViewMessage::__BindNewName(value) => { { let __ice_next = value; if ::ducktape_view_guest::state_changed!(self.new_name, __ice_next) { self.new_name = __ice_next; self.__ice_rev[30] += 1; } } ::ducktape_view_guest::Task::none() }
__FilesViewMessage::__0T6472616674(__transaction) => { let __route = __transaction.apply(&mut self.draft); self.__ice_rev[31] += 1; __route.map_or_else(::ducktape_view_guest::Task::none, ::ducktape_view_guest::Task::done) },
__FilesViewMessage::__EditDraft(__document) => { __document.apply(&mut self.draft); self.__ice_rev[31] += 1; ::ducktape_view_guest::Task::none() }
__FilesViewMessage::__ExternNoop => ::ducktape_view_guest::Task::none(),
}
        }
    }
}
