#[allow(warnings, clippy::all)]
mod __ice_group_app_update {
use super::*;
impl super::PagesView {
#[allow(clippy::assign_op_pattern)]
pub(super) fn __update(&mut self, message: __PagesViewMessage) -> ::ducktape_view_guest::Task<__PagesViewMessage> {
let __task = match message {
__PagesViewMessage::SessionArrived(item) => (|| {

let _ = &item;
{ let __ice_next = item.error.to_owned(); if ::ducktape_view_guest::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[22] += 1; } }
if (!(item.error).is_empty()) { return ::ducktape_view_guest::Task::none(); }
let next = item.next.clone();
{ let __ice_next = crate::host::connection_serial_after(self.connected, next.connected, self.register_serial); if ::ducktape_view_guest::state_changed!(self.register_serial, __ice_next) { self.register_serial = __ice_next; self.__ice_rev[19] += 1; } }
{ let __ice_next = next.connected; if ::ducktape_view_guest::state_changed!(self.connected, __ice_next) { self.connected = __ice_next; self.__ice_rev[16] += 1; } }
{ let __ice_next = next.chain.to_owned(); if ::ducktape_view_guest::state_changed!(self.chain, __ice_next) { self.chain = __ice_next; self.__ice_rev[17] += 1; } }
{ let __ice_next = next.dark; if ::ducktape_view_guest::state_changed!(self.document_dark, __ice_next) { self.document_dark = __ice_next; self.__ice_rev[12] += 1; } }
{ let __ice_next = crate::editor_view::document_presentation(::std::borrow::Borrow::borrow(&(self.document)), self.document_menu.clone(), self.document_dark, self.document_commented.clone(), self.document_marks.clone(), self.document_focused, self.document_reserve.clone()); if ::ducktape_view_guest::state_changed!(self.document_paint, __ice_next) { self.document_paint = __ice_next; self.__ice_rev[7] += 1; } }
let route_moved = ((crate::host::route_arrived(next.route_serial, self.route_serial) && (!(next.route_page).is_empty())) && (next.route_page != self.active_page));
{ let __ice_next = next.route_serial; if ::ducktape_view_guest::state_changed!(self.route_serial, __ice_next) { self.route_serial = __ice_next; self.__ice_rev[18] += 1; } }
{ let __ice_next = crate::host::keep_str(route_moved, ::std::convert::AsRef::as_ref(&(next.route_page)), ::std::convert::AsRef::as_ref(&(self.active_page))); if ::ducktape_view_guest::state_changed!(self.active_page, __ice_next) { self.active_page = __ice_next; self.__ice_rev[32] += 1; } }
{ let __ice_next = (self.loading || route_moved); if ::ducktape_view_guest::state_changed!(self.loading, __ice_next) { self.loading = __ice_next; self.__ice_rev[20] += 1; } }
{ let __ice_next = crate::host::page_address(::std::convert::AsRef::as_ref(&(self.active_page)), ::std::convert::AsRef::as_ref(&(self.chain))); if ::ducktape_view_guest::state_changed!(self.page_link, __ice_next) { self.page_link = __ice_next; self.__ice_rev[23] += 1; } }
{ let __ice_next = AppTheme::App; if ::ducktape_view_guest::state_changed!(self.active_palette, __ice_next) { self.active_palette = __ice_next; self.__ice_rev[15] += 1; } }
if (!next.dark) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = AppTheme::AppDark; if ::ducktape_view_guest::state_changed!(self.active_palette, __ice_next) { self.active_palette = __ice_next; self.__ice_rev[15] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::CommentPointerMoved(_x, y) => (|| {

let _ = &_x;
let _ = &y;
{ let __ice_next = y; if ::ducktape_view_guest::state_changed!(self.pointer_y, __ice_next) { self.pointer_y = __ice_next; self.__ice_rev[0] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::ChoosePage(id) => (|| {

let _ = &id;
if ((!(self.host_error).is_empty()) || (id).is_empty()) { return ::ducktape_view_guest::Task::none(); }
if (self.loading || self.busy) { return ::ducktape_view_guest::Task::none(); }
if (id == self.active_page) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = id.to_owned(); if ::ducktape_view_guest::state_changed!(self.active_page, __ice_next) { self.active_page = __ice_next; self.__ice_rev[32] += 1; } }
{ let __ice_next = crate::host::page_display_title(::std::convert::AsRef::as_ref(&(self.pages)), ::std::convert::AsRef::as_ref(&(id)), ::std::convert::AsRef::as_ref(&(self.active_page_title))); if ::ducktape_view_guest::state_changed!(self.active_page_title, __ice_next) { self.active_page_title = __ice_next; self.__ice_rev[33] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.active_page_parent, __ice_next) { self.active_page_parent = __ice_next; self.__ice_rev[34] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ducktape_view_guest::state_changed!(self.blocks, __ice_next) { self.blocks = __ice_next; self.__ice_rev[25] += 1; } }
{ let __ice_next = crate::host::page_address(::std::convert::AsRef::as_ref(&(id)), ::std::convert::AsRef::as_ref(&(self.chain))); if ::ducktape_view_guest::state_changed!(self.page_link, __ice_next) { self.page_link = __ice_next; self.__ice_rev[23] += 1; } }
{ let __ice_next = true; if ::ducktape_view_guest::state_changed!(self.loading, __ice_next) { self.loading = __ice_next; self.__ice_rev[20] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::RegisterArrived(item) => (|| {

let _ = &item;
{ let __ice_next = item.error.to_owned(); if ::ducktape_view_guest::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[22] += 1; } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.loading, __ice_next) { self.loading = __ice_next; self.__ice_rev[20] += 1; } }
if (!(item.error).is_empty()) { return ::ducktape_view_guest::Task::none(); }
let page_moved = (item.active_page != self.buffer_page);
let comments_carry = (self.block_comments_open && (!page_moved));
{ let __ice_next = crate::host::remember_draft(::std::convert::AsRef::as_ref(&(self.orphaned_comment_drafts)), ::std::convert::AsRef::as_ref(&(crate::host::keep_str(page_moved, ::std::convert::AsRef::as_ref(&(self.block_comment_draft)), ::std::convert::AsRef::as_ref(&("")))))); if ::ducktape_view_guest::state_changed!(self.orphaned_comment_drafts, __ice_next) { self.orphaned_comment_drafts = __ice_next; self.__ice_rev[43] += 1; } }
{ let __ice_next = crate::host::keep_str(page_moved, ::std::convert::AsRef::as_ref(&("")), ::std::convert::AsRef::as_ref(&(self.block_comment_draft))); if ::ducktape_view_guest::state_changed!(self.block_comment_draft, __ice_next) { self.block_comment_draft = __ice_next; self.__ice_rev[56] += 1; } }
{ let __ice_next = comments_carry; if ::ducktape_view_guest::state_changed!(self.block_comments_open, __ice_next) { self.block_comments_open = __ice_next; self.__ice_rev[44] += 1; } }
{ let __ice_next = crate::host::keep_i64(comments_carry, self.comment_anchor_line, 0); if ::ducktape_view_guest::state_changed!(self.comment_anchor_line, __ice_next) { self.comment_anchor_line = __ice_next; self.__ice_rev[2] += 1; } }
{ let __ice_next = crate::host::comments_reserve(self.pages_pane_width, comments_carry, self.comment_anchor_line, self.comments_card_height); if ::ducktape_view_guest::state_changed!(self.document_reserve, __ice_next) { self.document_reserve = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = crate::host::keep_str(comments_carry, ::std::convert::AsRef::as_ref(&(self.scope_target)), ::std::convert::AsRef::as_ref(&(""))); if ::ducktape_view_guest::state_changed!(self.scope_target, __ice_next) { self.scope_target = __ice_next; self.__ice_rev[45] += 1; } }
{ let __ice_next = (self.scope_pinned && comments_carry); if ::ducktape_view_guest::state_changed!(self.scope_pinned, __ice_next) { self.scope_pinned = __ice_next; self.__ice_rev[46] += 1; } }
{ let __ice_next = crate::host::keep_str(comments_carry, ::std::convert::AsRef::as_ref(&(self.reply_thread)), ::std::convert::AsRef::as_ref(&(""))); if ::ducktape_view_guest::state_changed!(self.reply_thread, __ice_next) { self.reply_thread = __ice_next; self.__ice_rev[51] += 1; } }
{ let __ice_next = crate::host::keep_str(comments_carry, ::std::convert::AsRef::as_ref(&(self.reply_draft)), ::std::convert::AsRef::as_ref(&(""))); if ::ducktape_view_guest::state_changed!(self.reply_draft, __ice_next) { self.reply_draft = __ice_next; self.__ice_rev[57] += 1; } }
self.expanded_threads = crate::host::kept_ids(comments_carry, ::std::mem::take(&mut self.expanded_threads)); self.__ice_rev[52] += 1;
{ let __ice_next = (self.resolved_open && comments_carry); if ::ducktape_view_guest::state_changed!(self.resolved_open, __ice_next) { self.resolved_open = __ice_next; self.__ice_rev[53] += 1; } }
{ let __ice_next = (self.page_searching && (!page_moved)); if ::ducktape_view_guest::state_changed!(self.page_searching, __ice_next) { self.page_searching = __ice_next; self.__ice_rev[35] += 1; } }
{ let __ice_next = crate::host::keep_str(page_moved, ::std::convert::AsRef::as_ref(&("")), ::std::convert::AsRef::as_ref(&(self.page_search_query))); if ::ducktape_view_guest::state_changed!(self.page_search_query, __ice_next) { self.page_search_query = __ice_next; self.__ice_rev[37] += 1; } }
{ let __ice_next = (self.page_delete_armed && (!page_moved)); if ::ducktape_view_guest::state_changed!(self.page_delete_armed, __ice_next) { self.page_delete_armed = __ice_next; self.__ice_rev[39] += 1; } }
{ let __ice_next = crate::host::keep_str(page_moved, ::std::convert::AsRef::as_ref(&("idle")), ::std::convert::AsRef::as_ref(&(self.autosave))); if ::ducktape_view_guest::state_changed!(self.autosave, __ice_next) { self.autosave = __ice_next; self.__ice_rev[40] += 1; } }
{ let __ice_next = item.pages.clone(); if ::ducktape_view_guest::state_changed!(self.pages, __ice_next) { self.pages = __ice_next; self.__ice_rev[24] += 1; } }
{ let __ice_next = item.blocks.clone(); if ::ducktape_view_guest::state_changed!(self.blocks, __ice_next) { self.blocks = __ice_next; self.__ice_rev[25] += 1; } }
{ let __ice_next = item.subpages.clone(); if ::ducktape_view_guest::state_changed!(self.subpages, __ice_next) { self.subpages = __ice_next; self.__ice_rev[42] += 1; } }
{ let __ice_next = item.active_page_title.to_owned(); if ::ducktape_view_guest::state_changed!(self.active_page_title, __ice_next) { self.active_page_title = __ice_next; self.__ice_rev[33] += 1; } }
{ let __ice_next = item.active_page_parent.to_owned(); if ::ducktape_view_guest::state_changed!(self.active_page_parent, __ice_next) { self.active_page_parent = __ice_next; self.__ice_rev[34] += 1; } }
{ let __ice_next = item.thread_total; if ::ducktape_view_guest::state_changed!(self.thread_total, __ice_next) { self.thread_total = __ice_next; self.__ice_rev[47] += 1; } }
{ let __ice_next = item.comment_rows.clone(); if ::ducktape_view_guest::state_changed!(self.comment_rows, __ice_next) { self.comment_rows = __ice_next; self.__ice_rev[48] += 1; } }
{ let __ice_next = item.commented_hits.clone(); if ::ducktape_view_guest::state_changed!(self.commented_hits, __ice_next) { self.commented_hits = __ice_next; self.__ice_rev[50] += 1; } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.threads_loading, __ice_next) { self.threads_loading = __ice_next; self.__ice_rev[49] += 1; } }
{ let __ice_next = crate::host::commented_lines(::std::convert::AsRef::as_ref(&(item.blocks)), ::std::convert::AsRef::as_ref(&(item.commented_hits))); if ::ducktape_view_guest::state_changed!(self.document_commented, __ice_next) { self.document_commented = __ice_next; self.__ice_rev[13] += 1; } }
{ let __ice_next = crate::host::comment_marks(::std::convert::AsRef::as_ref(&(item.blocks)), ::std::convert::AsRef::as_ref(&(item.commented_hits))); if ::ducktape_view_guest::state_changed!(self.document_marks, __ice_next) { self.document_marks = __ice_next; self.__ice_rev[14] += 1; } }
{ let __ice_next = crate::host::page_address(::std::convert::AsRef::as_ref(&(item.active_page)), ::std::convert::AsRef::as_ref(&(self.chain))); if ::ducktape_view_guest::state_changed!(self.page_link, __ice_next) { self.page_link = __ice_next; self.__ice_rev[23] += 1; } }
let install = crate::host::install_decision(::std::convert::AsRef::as_ref(&(crate::host::document_text(::std::borrow::Borrow::borrow(&(self.document))))), ::std::convert::AsRef::as_ref(&(self.buffer_page)), ::std::convert::AsRef::as_ref(&(item.active_page)), ::std::convert::AsRef::as_ref(&(self.page_saved_text)), ::std::convert::AsRef::as_ref(&(item.document)));
{ let __ice_next = item.active_page.to_owned(); if ::ducktape_view_guest::state_changed!(self.active_page, __ice_next) { self.active_page = __ice_next; self.__ice_rev[32] += 1; } }
{ let __ice_next = item.active_page.to_owned(); if ::ducktape_view_guest::state_changed!(self.buffer_page, __ice_next) { self.buffer_page = __ice_next; self.__ice_rev[61] += 1; } }
{ let __ice_next = crate::editor_view::document_presentation(::std::borrow::Borrow::borrow(&(self.document)), self.document_menu.clone(), self.document_dark, self.document_commented.clone(), self.document_marks.clone(), self.document_focused, self.document_reserve.clone()); if ::ducktape_view_guest::state_changed!(self.document_paint, __ice_next) { self.document_paint = __ice_next; self.__ice_rev[7] += 1; } }
if (!install) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = item.document.to_owned(); if ::ducktape_view_guest::state_changed!(self.page_saved_text, __ice_next) { self.page_saved_text = __ice_next; self.__ice_rev[60] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.page_refusal, __ice_next) { self.page_refusal = __ice_next; self.__ice_rev[41] += 1; } }
{ let __reset = self.document.reset_revision(); let __next = crate::host::document_editor(::std::convert::AsRef::as_ref(&(item.document))); self.document.replace(__next, __reset); }; self.__ice_rev[8] += 1;
{ let __ice_next = crate::editor_binding::initial_menu(); if ::ducktape_view_guest::state_changed!(self.document_menu, __ice_next) { self.document_menu = __ice_next; self.__ice_rev[10] += 1; } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.document_focused, __ice_next) { self.document_focused = __ice_next; self.__ice_rev[5] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.document_error, __ice_next) { self.document_error = __ice_next; self.__ice_rev[11] += 1; } }
{ let __ice_next = crate::editor_view::document_presentation(::std::borrow::Borrow::borrow(&(self.document)), self.document_menu.clone(), self.document_dark, self.document_commented.clone(), self.document_marks.clone(), self.document_focused, self.document_reserve.clone()); if ::ducktape_view_guest::state_changed!(self.document_paint, __ice_next) { self.document_paint = __ice_next; self.__ice_rev[7] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::SearchArrived(item) => (|| {

let _ = &item;
{ let __ice_next = item.error.to_owned(); if ::ducktape_view_guest::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[22] += 1; } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.page_searching, __ice_next) { self.page_searching = __ice_next; self.__ice_rev[35] += 1; } }
if (item.query != self.page_search_query) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = item.hits.clone(); if ::ducktape_view_guest::state_changed!(self.page_search_hits, __ice_next) { self.page_search_hits = __ice_next; self.__ice_rev[36] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::ActDone(item) => (|| {

let _ = &item;
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.busy, __ice_next) { self.busy = __ice_next; self.__ice_rev[21] += 1; } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.threads_loading, __ice_next) { self.threads_loading = __ice_next; self.__ice_rev[49] += 1; } }
{ let __ice_next = item.error.to_owned(); if ::ducktape_view_guest::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[22] += 1; } }
{ let __ice_next = (self.register_serial + 1); if ::ducktape_view_guest::state_changed!(self.register_serial, __ice_next) { self.register_serial = __ice_next; self.__ice_rev[19] += 1; } }
let refused = (!(item.error).is_empty());
{ let __ice_next = crate::host::keep_str(refused, ::std::convert::AsRef::as_ref(&(self.pending_page)), ::std::convert::AsRef::as_ref(&(self.page_draft))); if ::ducktape_view_guest::state_changed!(self.page_draft, __ice_next) { self.page_draft = __ice_next; self.__ice_rev[54] += 1; } }
{ let __ice_next = crate::host::keep_str((refused && (!(self.reply_thread).is_empty())), ::std::convert::AsRef::as_ref(&(self.pending_comment)), ::std::convert::AsRef::as_ref(&(self.reply_draft))); if ::ducktape_view_guest::state_changed!(self.reply_draft, __ice_next) { self.reply_draft = __ice_next; self.__ice_rev[57] += 1; } }
{ let __ice_next = crate::host::keep_str((refused && (self.reply_thread).is_empty()), ::std::convert::AsRef::as_ref(&(self.pending_comment)), ::std::convert::AsRef::as_ref(&(self.block_comment_draft))); if ::ducktape_view_guest::state_changed!(self.block_comment_draft, __ice_next) { self.block_comment_draft = __ice_next; self.__ice_rev[56] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.pending_page, __ice_next) { self.pending_page = __ice_next; self.__ice_rev[58] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.pending_comment, __ice_next) { self.pending_comment = __ice_next; self.__ice_rev[59] += 1; } }
if refused { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.page_create_open, __ice_next) { self.page_create_open = __ice_next; self.__ice_rev[31] += 1; } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.page_delete_armed, __ice_next) { self.page_delete_armed = __ice_next; self.__ice_rev[39] += 1; } }
{ let __ice_next = crate::host::keep_str((!(item.page).is_empty()), ::std::convert::AsRef::as_ref(&(item.page)), ::std::convert::AsRef::as_ref(&(self.active_page))); if ::ducktape_view_guest::state_changed!(self.active_page, __ice_next) { self.active_page = __ice_next; self.__ice_rev[32] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::SaveDone(item) => (|| {

let _ = &item;
{ let __ice_next = "error".to_owned(); if ::ducktape_view_guest::state_changed!(self.autosave, __ice_next) { self.autosave = __ice_next; self.__ice_rev[40] += 1; } }
{ let __ice_next = item.error.to_owned(); if ::ducktape_view_guest::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[22] += 1; } }
if (!(item.error).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[22] += 1; } }
{ let __ice_next = item.refusal.to_owned(); if ::ducktape_view_guest::state_changed!(self.page_refusal, __ice_next) { self.page_refusal = __ice_next; self.__ice_rev[41] += 1; } }
{ let __ice_next = crate::host::baseline_at_submitted_title(::std::convert::AsRef::as_ref(&(crate::host::saved_baseline(item.written, ::std::convert::AsRef::as_ref(&(item.document)), ::std::convert::AsRef::as_ref(&(self.page_inflight_text))))), ::std::convert::AsRef::as_ref(&(self.page_inflight_text))); if ::ducktape_view_guest::state_changed!(self.page_saved_text, __ice_next) { self.page_saved_text = __ice_next; self.__ice_rev[60] += 1; } }
{ let __ice_next = "saved".to_owned(); if ::ducktape_view_guest::state_changed!(self.autosave, __ice_next) { self.autosave = __ice_next; self.__ice_rev[40] += 1; } }
{ let __ice_next = (self.register_serial + crate::host::keep_i64(item.written, 1, 0)); if ::ducktape_view_guest::state_changed!(self.register_serial, __ice_next) { self.register_serial = __ice_next; self.__ice_rev[19] += 1; } }
if (item.refusal).is_empty() { return ::ducktape_view_guest::Task::none(); }
let untouched = (crate::host::document_text(::std::borrow::Borrow::borrow(&(self.document))) == self.page_inflight_text);
{ let __ice_next = crate::host::baseline_at_submitted_title(::std::convert::AsRef::as_ref(&(item.document)), ::std::convert::AsRef::as_ref(&(self.page_inflight_text))); if ::ducktape_view_guest::state_changed!(self.page_saved_text, __ice_next) { self.page_saved_text = __ice_next; self.__ice_rev[60] += 1; } }
{ let __ice_next = "idle".to_owned(); if ::ducktape_view_guest::state_changed!(self.autosave, __ice_next) { self.autosave = __ice_next; self.__ice_rev[40] += 1; } }
if (!untouched) { return ::ducktape_view_guest::Task::none(); }
{ let __reset = self.document.reset_revision(); let __next = crate::host::document_editor(::std::convert::AsRef::as_ref(&(item.document))); self.document.replace(__next, __reset); }; self.__ice_rev[8] += 1;
{ let __ice_next = crate::editor_view::document_presentation(::std::borrow::Borrow::borrow(&(self.document)), self.document_menu.clone(), self.document_dark, self.document_commented.clone(), self.document_marks.clone(), self.document_focused, self.document_reserve.clone()); if ::ducktape_view_guest::state_changed!(self.document_paint, __ice_next) { self.document_paint = __ice_next; self.__ice_rev[7] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::PageAutosaveTick => (|| {

if (!(self.host_error).is_empty()) { return ::ducktape_view_guest::Task::none(); }
if (((self.busy || self.loading) || (self.active_page).is_empty()) || (self.active_page != self.buffer_page)) { return ::ducktape_view_guest::Task::none(); }
if (self.autosave == "saving") { return ::ducktape_view_guest::Task::none(); }
let text = crate::host::document_text(::std::borrow::Borrow::borrow(&(self.document)));
if (text == self.page_saved_text) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "idle".to_owned(); if ::ducktape_view_guest::state_changed!(self.autosave, __ice_next) { self.autosave = __ice_next; self.__ice_rev[40] += 1; } }
if crate::host::has_unclosed_fence(::std::convert::AsRef::as_ref(&(text))) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "saving".to_owned(); if ::ducktape_view_guest::state_changed!(self.autosave, __ice_next) { self.autosave = __ice_next; self.__ice_rev[40] += 1; } }
{ let __ice_next = text.to_owned(); if ::ducktape_view_guest::state_changed!(self.page_inflight_text, __ice_next) { self.page_inflight_text = __ice_next; self.__ice_rev[62] += 1; } }
{ let __ice_next = ({ crate::host::save(::std::convert::AsRef::as_ref(&(self.active_page)), ::std::convert::AsRef::as_ref(&(text)), ::std::convert::AsRef::as_ref(&(self.page_saved_text))) }); if ::ducktape_view_guest::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[63] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::TogglePageCreate => (|| {

if (!(self.host_error).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = (!self.page_create_open); if ::ducktape_view_guest::state_changed!(self.page_create_open, __ice_next) { self.page_create_open = __ice_next; self.__ice_rev[31] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::CreatePageSubmit => (|| {

if (!(self.host_error).is_empty()) { return ::ducktape_view_guest::Task::none(); }
if (((self.loading || self.busy) || (!self.connected)) || ((self.page_draft).trim().to_owned()).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = true; if ::ducktape_view_guest::state_changed!(self.busy, __ice_next) { self.busy = __ice_next; self.__ice_rev[21] += 1; } }
{ let __ice_next = (self.page_draft).trim().to_owned(); if ::ducktape_view_guest::state_changed!(self.pending_page, __ice_next) { self.pending_page = __ice_next; self.__ice_rev[58] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.page_draft, __ice_next) { self.page_draft = __ice_next; self.__ice_rev[54] += 1; } }
{ let __ice_next = ({ crate::host::create(::std::convert::AsRef::as_ref(&(self.pending_page))) }); if ::ducktape_view_guest::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[63] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::ArmPageDelete => (|| {

if (!(self.host_error).is_empty()) { return ::ducktape_view_guest::Task::none(); }
if ((self.loading || self.busy) || (self.active_page).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.page_menu_open, __ice_next) { self.page_menu_open = __ice_next; self.__ice_rev[30] += 1; } }
{ let __ice_next = true; if ::ducktape_view_guest::state_changed!(self.page_delete_armed, __ice_next) { self.page_delete_armed = __ice_next; self.__ice_rev[39] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::DisarmPageDelete => (|| {

{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.page_delete_armed, __ice_next) { self.page_delete_armed = __ice_next; self.__ice_rev[39] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::DeletePageSubmit => (|| {

if (!(self.host_error).is_empty()) { return ::ducktape_view_guest::Task::none(); }
if (((self.loading || self.busy) || (self.active_page).is_empty()) || (!self.page_delete_armed)) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = true; if ::ducktape_view_guest::state_changed!(self.busy, __ice_next) { self.busy = __ice_next; self.__ice_rev[21] += 1; } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.page_delete_armed, __ice_next) { self.page_delete_armed = __ice_next; self.__ice_rev[39] += 1; } }
{ let __ice_next = crate::host::remember_draft(::std::convert::AsRef::as_ref(&(self.orphaned_comment_drafts)), ::std::convert::AsRef::as_ref(&(self.block_comment_draft))); if ::ducktape_view_guest::state_changed!(self.orphaned_comment_drafts, __ice_next) { self.orphaned_comment_drafts = __ice_next; self.__ice_rev[43] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.block_comment_draft, __ice_next) { self.block_comment_draft = __ice_next; self.__ice_rev[56] += 1; } }
{ let __ice_next = ({ crate::host::delete(::std::convert::AsRef::as_ref(&(self.active_page))) }); if ::ducktape_view_guest::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[63] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::SearchPagesSubmit => (|| {

if (!(self.host_error).is_empty()) { return ::ducktape_view_guest::Task::none(); }
if (self.page_searching || ((self.page_search_draft).trim().to_owned()).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = true; if ::ducktape_view_guest::state_changed!(self.page_searching, __ice_next) { self.page_searching = __ice_next; self.__ice_rev[35] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ducktape_view_guest::state_changed!(self.page_search_hits, __ice_next) { self.page_search_hits = __ice_next; self.__ice_rev[36] += 1; } }
{ let __ice_next = (self.page_search_draft).trim().to_owned(); if ::ducktape_view_guest::state_changed!(self.page_search_query, __ice_next) { self.page_search_query = __ice_next; self.__ice_rev[37] += 1; } }
{ let __ice_next = (self.page_search_serial + 1); if ::ducktape_view_guest::state_changed!(self.page_search_serial, __ice_next) { self.page_search_serial = __ice_next; self.__ice_rev[38] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::ClearPageSearch => (|| {

{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.page_search_draft, __ice_next) { self.page_search_draft = __ice_next; self.__ice_rev[55] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ducktape_view_guest::state_changed!(self.page_search_hits, __ice_next) { self.page_search_hits = __ice_next; self.__ice_rev[36] += 1; } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.page_searching, __ice_next) { self.page_searching = __ice_next; self.__ice_rev[35] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.page_search_query, __ice_next) { self.page_search_query = __ice_next; self.__ice_rev[37] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::OpenPageSearchHit(page_id, _block_id) => (|| {

let _ = &page_id;
let _ = &_block_id;
if (!(self.host_error).is_empty()) { return ::ducktape_view_guest::Task::none(); }
if (self.loading || self.busy) { return ::ducktape_view_guest::Task::none(); }
return (::ducktape_view_guest::Task::done(page_id.to_owned())).map(|value| __PagesViewMessage::ChoosePage(value));
})(),
__PagesViewMessage::UseOrphanedCommentDraft(draft) => (|| {

let _ = &draft;
if (!(self.host_error).is_empty()) { return ::ducktape_view_guest::Task::none(); }
if ((self.loading || self.busy) || (!((self.block_comment_draft).trim().to_owned()).is_empty())) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = draft.to_owned(); if ::ducktape_view_guest::state_changed!(self.block_comment_draft, __ice_next) { self.block_comment_draft = __ice_next; self.__ice_rev[56] += 1; } }
{ let __ice_next = true; if ::ducktape_view_guest::state_changed!(self.block_comments_open, __ice_next) { self.block_comments_open = __ice_next; self.__ice_rev[44] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.scope_target, __ice_next) { self.scope_target = __ice_next; self.__ice_rev[45] += 1; } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.scope_pinned, __ice_next) { self.scope_pinned = __ice_next; self.__ice_rev[46] += 1; } }
{ let __ice_next = crate::host::forget_draft(::std::convert::AsRef::as_ref(&(self.orphaned_comment_drafts)), ::std::convert::AsRef::as_ref(&(draft))); if ::ducktape_view_guest::state_changed!(self.orphaned_comment_drafts, __ice_next) { self.orphaned_comment_drafts = __ice_next; self.__ice_rev[43] += 1; } }
{ let __ice_next = 0; if ::ducktape_view_guest::state_changed!(self.comment_anchor_line, __ice_next) { self.comment_anchor_line = __ice_next; self.__ice_rev[2] += 1; } }
let recovered = crate::host::comments_reserve(self.pages_pane_width, true, 0, self.comments_card_height);
if ((recovered.line == self.document_reserve.line) && (recovered.height == self.document_reserve.height)) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = recovered.clone(); if ::ducktape_view_guest::state_changed!(self.document_reserve, __ice_next) { self.document_reserve = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = crate::editor_view::document_presentation(::std::borrow::Borrow::borrow(&(self.document)), self.document_menu.clone(), self.document_dark, self.document_commented.clone(), self.document_marks.clone(), self.document_focused, self.document_reserve.clone()); if ::ducktape_view_guest::state_changed!(self.document_paint, __ice_next) { self.document_paint = __ice_next; self.__ice_rev[7] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::DiscardOrphanedCommentDraft(draft) => (|| {

let _ = &draft;
{ let __ice_next = crate::host::forget_draft(::std::convert::AsRef::as_ref(&(self.orphaned_comment_drafts)), ::std::convert::AsRef::as_ref(&(draft))); if ::ducktape_view_guest::state_changed!(self.orphaned_comment_drafts, __ice_next) { self.orphaned_comment_drafts = __ice_next; self.__ice_rev[43] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::ToggleBlockComments => (|| {

{ let __ice_next = (-1.0); if ::ducktape_view_guest::state_changed!(self.comment_anchor_y, __ice_next) { self.comment_anchor_y = __ice_next; self.__ice_rev[1] += 1; } }
{ let __ice_next = 0; if ::ducktape_view_guest::state_changed!(self.comment_anchor_line, __ice_next) { self.comment_anchor_line = __ice_next; self.__ice_rev[2] += 1; } }
if (!(self.host_error).is_empty()) { return ::ducktape_view_guest::Task::none(); }
if ((self.loading || self.busy) || (self.active_page).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = crate::host::remember_draft(::std::convert::AsRef::as_ref(&(self.orphaned_comment_drafts)), ::std::convert::AsRef::as_ref(&(self.block_comment_draft))); if ::ducktape_view_guest::state_changed!(self.orphaned_comment_drafts, __ice_next) { self.orphaned_comment_drafts = __ice_next; self.__ice_rev[43] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.block_comment_draft, __ice_next) { self.block_comment_draft = __ice_next; self.__ice_rev[56] += 1; } }
{ let __ice_next = (!self.block_comments_open); if ::ducktape_view_guest::state_changed!(self.block_comments_open, __ice_next) { self.block_comments_open = __ice_next; self.__ice_rev[44] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.scope_target, __ice_next) { self.scope_target = __ice_next; self.__ice_rev[45] += 1; } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.scope_pinned, __ice_next) { self.scope_pinned = __ice_next; self.__ice_rev[46] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.reply_thread, __ice_next) { self.reply_thread = __ice_next; self.__ice_rev[51] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.reply_draft, __ice_next) { self.reply_draft = __ice_next; self.__ice_rev[57] += 1; } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.resolved_open, __ice_next) { self.resolved_open = __ice_next; self.__ice_rev[53] += 1; } }
let toggled_reserve = crate::host::comments_reserve(self.pages_pane_width, self.block_comments_open, 0, self.comments_card_height);
if ((toggled_reserve.line == self.document_reserve.line) && (toggled_reserve.height == self.document_reserve.height)) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = toggled_reserve.clone(); if ::ducktape_view_guest::state_changed!(self.document_reserve, __ice_next) { self.document_reserve = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = crate::editor_view::document_presentation(::std::borrow::Borrow::borrow(&(self.document)), self.document_menu.clone(), self.document_dark, self.document_commented.clone(), self.document_marks.clone(), self.document_focused, self.document_reserve.clone()); if ::ducktape_view_guest::state_changed!(self.document_paint, __ice_next) { self.document_paint = __ice_next; self.__ice_rev[7] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::CloseBlockComments => (|| {

{ let __ice_next = (-1.0); if ::ducktape_view_guest::state_changed!(self.comment_anchor_y, __ice_next) { self.comment_anchor_y = __ice_next; self.__ice_rev[1] += 1; } }
{ let __ice_next = 0; if ::ducktape_view_guest::state_changed!(self.comment_anchor_line, __ice_next) { self.comment_anchor_line = __ice_next; self.__ice_rev[2] += 1; } }
{ let __ice_next = crate::host::remember_draft(::std::convert::AsRef::as_ref(&(self.orphaned_comment_drafts)), ::std::convert::AsRef::as_ref(&(self.block_comment_draft))); if ::ducktape_view_guest::state_changed!(self.orphaned_comment_drafts, __ice_next) { self.orphaned_comment_drafts = __ice_next; self.__ice_rev[43] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.block_comment_draft, __ice_next) { self.block_comment_draft = __ice_next; self.__ice_rev[56] += 1; } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.block_comments_open, __ice_next) { self.block_comments_open = __ice_next; self.__ice_rev[44] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.scope_target, __ice_next) { self.scope_target = __ice_next; self.__ice_rev[45] += 1; } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.scope_pinned, __ice_next) { self.scope_pinned = __ice_next; self.__ice_rev[46] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.reply_thread, __ice_next) { self.reply_thread = __ice_next; self.__ice_rev[51] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.reply_draft, __ice_next) { self.reply_draft = __ice_next; self.__ice_rev[57] += 1; } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.resolved_open, __ice_next) { self.resolved_open = __ice_next; self.__ice_rev[53] += 1; } }
if (self.document_reserve.height == 0) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = crate::editor_view::no_reserve(); if ::ducktape_view_guest::state_changed!(self.document_reserve, __ice_next) { self.document_reserve = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = crate::editor_view::document_presentation(::std::borrow::Borrow::borrow(&(self.document)), self.document_menu.clone(), self.document_dark, self.document_commented.clone(), self.document_marks.clone(), self.document_focused, self.document_reserve.clone()); if ::ducktape_view_guest::state_changed!(self.document_paint, __ice_next) { self.document_paint = __ice_next; self.__ice_rev[7] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::NarrowCommentScope(target) => (|| {

let _ = &target;
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.reply_thread, __ice_next) { self.reply_thread = __ice_next; self.__ice_rev[51] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.reply_draft, __ice_next) { self.reply_draft = __ice_next; self.__ice_rev[57] += 1; } }
if (!(self.host_error).is_empty()) { return ::ducktape_view_guest::Task::none(); }
if ((self.loading || self.busy) || (target).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = target.to_owned(); if ::ducktape_view_guest::state_changed!(self.scope_target, __ice_next) { self.scope_target = __ice_next; self.__ice_rev[45] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::WidenCommentScope => (|| {

{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.reply_thread, __ice_next) { self.reply_thread = __ice_next; self.__ice_rev[51] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.reply_draft, __ice_next) { self.reply_draft = __ice_next; self.__ice_rev[57] += 1; } }
if (!(self.host_error).is_empty()) { return ::ducktape_view_guest::Task::none(); }
if ((self.loading || self.busy) || self.scope_pinned) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.scope_target, __ice_next) { self.scope_target = __ice_next; self.__ice_rev[45] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::ResolveThreadSubmit(id, resolved) => (|| {

let _ = &id;
let _ = &resolved;
if (!(self.host_error).is_empty()) { return ::ducktape_view_guest::Task::none(); }
if ((((self.loading || self.busy) || self.threads_loading) || (!self.block_comments_open)) || (id).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = true; if ::ducktape_view_guest::state_changed!(self.busy, __ice_next) { self.busy = __ice_next; self.__ice_rev[21] += 1; } }
{ let __ice_next = true; if ::ducktape_view_guest::state_changed!(self.threads_loading, __ice_next) { self.threads_loading = __ice_next; self.__ice_rev[49] += 1; } }
{ let __ice_next = ({ crate::host::resolve(::std::convert::AsRef::as_ref(&(id)), resolved) }); if ::ducktape_view_guest::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[63] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::SelectReplyThread(id) => (|| {

let _ = &id;
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.reply_draft, __ice_next) { self.reply_draft = __ice_next; self.__ice_rev[57] += 1; } }
{ let __ice_next = crate::host::reply_thread_after_press(::std::convert::AsRef::as_ref(&(self.reply_thread)), ::std::convert::AsRef::as_ref(&(id))); if ::ducktape_view_guest::state_changed!(self.reply_thread, __ice_next) { self.reply_thread = __ice_next; self.__ice_rev[51] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::ToggleThreadReplies(id) => (|| {

let _ = &id;
self.expanded_threads = crate::host::toggled(::std::mem::take(&mut self.expanded_threads), ::std::convert::AsRef::as_ref(&(id))); self.__ice_rev[52] += 1;
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::ToggleResolvedComments => (|| {

{ let __ice_next = (!self.resolved_open); if ::ducktape_view_guest::state_changed!(self.resolved_open, __ice_next) { self.resolved_open = __ice_next; self.__ice_rev[53] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::PostThreadReply(id) => (|| {

let _ = &id;
if (!(self.host_error).is_empty()) { return ::ducktape_view_guest::Task::none(); }
if ((((self.loading || self.busy) || self.threads_loading) || (!self.block_comments_open)) || ((self.reply_draft).trim().to_owned()).is_empty()) { return ::ducktape_view_guest::Task::none(); }
let reply_target = crate::host::comment_post_target(::std::convert::AsRef::as_ref(&(self.comment_rows)), ::std::convert::AsRef::as_ref(&(id)), ::std::convert::AsRef::as_ref(&(self.scope_target)));
if (reply_target).is_empty() { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = true; if ::ducktape_view_guest::state_changed!(self.busy, __ice_next) { self.busy = __ice_next; self.__ice_rev[21] += 1; } }
{ let __ice_next = true; if ::ducktape_view_guest::state_changed!(self.threads_loading, __ice_next) { self.threads_loading = __ice_next; self.__ice_rev[49] += 1; } }
{ let __ice_next = (self.reply_draft).trim().to_owned(); if ::ducktape_view_guest::state_changed!(self.pending_comment, __ice_next) { self.pending_comment = __ice_next; self.__ice_rev[59] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.reply_draft, __ice_next) { self.reply_draft = __ice_next; self.__ice_rev[57] += 1; } }
{ let __ice_next = ({ crate::host::post(::std::convert::AsRef::as_ref(&(self.pending_comment)), ::std::convert::AsRef::as_ref(&(reply_target)), ::std::convert::AsRef::as_ref(&(id))) }); if ::ducktape_view_guest::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[63] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::PostBlockCommentSubmit => (|| {

if (!(self.host_error).is_empty()) { return ::ducktape_view_guest::Task::none(); }
if (((((self.loading || self.busy) || self.threads_loading) || (!self.block_comments_open)) || (self.active_page).is_empty()) || ((self.block_comment_draft).trim().to_owned()).is_empty()) { return ::ducktape_view_guest::Task::none(); }
let fresh_target = crate::host::keep_str((!(self.scope_target).is_empty()), ::std::convert::AsRef::as_ref(&(self.scope_target)), ::std::convert::AsRef::as_ref(&(self.active_page)));
{ let __ice_next = true; if ::ducktape_view_guest::state_changed!(self.busy, __ice_next) { self.busy = __ice_next; self.__ice_rev[21] += 1; } }
{ let __ice_next = true; if ::ducktape_view_guest::state_changed!(self.threads_loading, __ice_next) { self.threads_loading = __ice_next; self.__ice_rev[49] += 1; } }
{ let __ice_next = (self.block_comment_draft).trim().to_owned(); if ::ducktape_view_guest::state_changed!(self.pending_comment, __ice_next) { self.pending_comment = __ice_next; self.__ice_rev[59] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.block_comment_draft, __ice_next) { self.block_comment_draft = __ice_next; self.__ice_rev[56] += 1; } }
{ let __ice_next = ({ crate::host::post(::std::convert::AsRef::as_ref(&(self.pending_comment)), ::std::convert::AsRef::as_ref(&(fresh_target)), ::std::convert::AsRef::as_ref(&(""))) }); if ::ducktape_view_guest::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[63] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::CopyToClipboard(text, label) => (|| {

let _ = &text;
let _ = &label;
{ let __ice_next = ({ crate::host::copy(::std::convert::AsRef::as_ref(&(text)), ::std::convert::AsRef::as_ref(&(label))) }); if ::ducktape_view_guest::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[63] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::DocumentPointerReleased(_button) => (|| {

let _ = &_button;
{ let __ice_next = (self.focus_query + 1); if ::ducktape_view_guest::state_changed!(self.focus_query, __ice_next) { self.focus_query = __ice_next; self.__ice_rev[6] += 1; } }
let query = self.focus_query;
return ::ducktape_view_guest::widget::is_focused(::std::string::String::from("PagesView/root/pages/document")).map(move |value| __PagesViewMessage::DocumentFocusChecked(query, value));
})(),
__PagesViewMessage::DocumentKeyReleased(_key) => (|| {

let _ = &_key;
{ let __ice_next = (self.focus_query + 1); if ::ducktape_view_guest::state_changed!(self.focus_query, __ice_next) { self.focus_query = __ice_next; self.__ice_rev[6] += 1; } }
let query = self.focus_query;
return ::ducktape_view_guest::widget::is_focused(::std::string::String::from("PagesView/root/pages/document")).map(move |value| __PagesViewMessage::DocumentFocusChecked(query, value));
})(),
__PagesViewMessage::DocumentWindowFocused => (|| {

{ let __ice_next = (self.focus_query + 1); if ::ducktape_view_guest::state_changed!(self.focus_query, __ice_next) { self.focus_query = __ice_next; self.__ice_rev[6] += 1; } }
let query = self.focus_query;
return ::ducktape_view_guest::widget::is_focused(::std::string::String::from("PagesView/root/pages/document")).map(move |value| __PagesViewMessage::DocumentFocusChecked(query, value));
})(),
__PagesViewMessage::DocumentWindowUnfocused => (|| {

{ let __ice_next = (self.focus_query + 1); if ::ducktape_view_guest::state_changed!(self.focus_query, __ice_next) { self.focus_query = __ice_next; self.__ice_rev[6] += 1; } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.document_focused, __ice_next) { self.document_focused = __ice_next; self.__ice_rev[5] += 1; } }
{ let __ice_next = crate::editor_view::document_presentation(::std::borrow::Borrow::borrow(&(self.document)), self.document_menu.clone(), self.document_dark, self.document_commented.clone(), self.document_marks.clone(), self.document_focused, self.document_reserve.clone()); if ::ducktape_view_guest::state_changed!(self.document_paint, __ice_next) { self.document_paint = __ice_next; self.__ice_rev[7] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::DocumentFocusChecked(query, focused) => (|| {

let _ = &query;
let _ = &focused;
if ((query != self.focus_query) || (focused == self.document_focused)) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = focused; if ::ducktape_view_guest::state_changed!(self.document_focused, __ice_next) { self.document_focused = __ice_next; self.__ice_rev[5] += 1; } }
{ let __ice_next = crate::editor_view::document_presentation(::std::borrow::Borrow::borrow(&(self.document)), self.document_menu.clone(), self.document_dark, self.document_commented.clone(), self.document_marks.clone(), self.document_focused, self.document_reserve.clone()); if ::ducktape_view_guest::state_changed!(self.document_paint, __ice_next) { self.document_paint = __ice_next; self.__ice_rev[7] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::SidebarResized(dx, _dy) => (|| {

let _ = &dx;
let _ = &_dy;
{ let __ice_next = crate::host::sidebar_width_after_delta(self.sidebar_width, dx, self.pages_viewport_width); if ::ducktape_view_guest::state_changed!(self.sidebar_width, __ice_next) { self.sidebar_width = __ice_next; self.__ice_rev[29] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::PagesViewportChanged(width, _height) => (|| {

let _ = &width;
let _ = &_height;
{ let __ice_next = width; if ::ducktape_view_guest::state_changed!(self.pages_viewport_width, __ice_next) { self.pages_viewport_width = __ice_next; self.__ice_rev[26] += 1; } }
{ let __ice_next = crate::host::sidebar_width_after_delta(self.sidebar_width, 0.0, width); if ::ducktape_view_guest::state_changed!(self.sidebar_width, __ice_next) { self.sidebar_width = __ice_next; self.__ice_rev[29] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::PagesPaneResized(width, _height) => (|| {

let _ = &width;
let _ = &_height;
{ let __ice_next = width; if ::ducktape_view_guest::state_changed!(self.pages_pane_width, __ice_next) { self.pages_pane_width = __ice_next; self.__ice_rev[28] += 1; } }
let next = crate::host::comments_reserve(width, self.block_comments_open, self.comment_anchor_line, self.comments_card_height);
if ((next.line == self.document_reserve.line) && (next.height == self.document_reserve.height)) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = next.clone(); if ::ducktape_view_guest::state_changed!(self.document_reserve, __ice_next) { self.document_reserve = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = crate::editor_view::document_presentation(::std::borrow::Borrow::borrow(&(self.document)), self.document_menu.clone(), self.document_dark, self.document_commented.clone(), self.document_marks.clone(), self.document_focused, self.document_reserve.clone()); if ::ducktape_view_guest::state_changed!(self.document_paint, __ice_next) { self.document_paint = __ice_next; self.__ice_rev[7] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::CommentsCardMeasured(_width, height) => (|| {

let _ = &_width;
let _ = &height;
let measured = crate::host::measured_card_height(self.comments_card_height, height);
if (measured == self.comments_card_height) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = measured; if ::ducktape_view_guest::state_changed!(self.comments_card_height, __ice_next) { self.comments_card_height = __ice_next; self.__ice_rev[3] += 1; } }
let next = crate::host::comments_reserve(self.pages_pane_width, self.block_comments_open, self.comment_anchor_line, measured);
if ((next.line == self.document_reserve.line) && (next.height == self.document_reserve.height)) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = next.clone(); if ::ducktape_view_guest::state_changed!(self.document_reserve, __ice_next) { self.document_reserve = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = crate::editor_view::document_presentation(::std::borrow::Borrow::borrow(&(self.document)), self.document_menu.clone(), self.document_dark, self.document_commented.clone(), self.document_marks.clone(), self.document_focused, self.document_reserve.clone()); if ::ducktape_view_guest::state_changed!(self.document_paint, __ice_next) { self.document_paint = __ice_next; self.__ice_rev[7] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::TogglePageMenu => (|| {

{ let __ice_next = (!self.page_menu_open); if ::ducktape_view_guest::state_changed!(self.page_menu_open, __ice_next) { self.page_menu_open = __ice_next; self.__ice_rev[30] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::ClosePageMenu => (|| {

{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.page_menu_open, __ice_next) { self.page_menu_open = __ice_next; self.__ice_rev[30] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::DocumentCommitted(next) => (|| {

let _ = &next;
{ let __ice_next = next.history.clone(); if ::ducktape_view_guest::state_changed!(self.document_history, __ice_next) { self.document_history = __ice_next; self.__ice_rev[9] += 1; } }
{ let __ice_next = next.menu.clone(); if ::ducktape_view_guest::state_changed!(self.document_menu, __ice_next) { self.document_menu = __ice_next; self.__ice_rev[10] += 1; } }
{ let __ice_next = crate::editor_view::document_presentation(::std::borrow::Borrow::borrow(&(self.document)), self.document_menu.clone(), self.document_dark, self.document_commented.clone(), self.document_marks.clone(), self.document_focused, self.document_reserve.clone()); if ::ducktape_view_guest::state_changed!(self.document_paint, __ice_next) { self.document_paint = __ice_next; self.__ice_rev[7] += 1; } }
let comment_line = crate::host::navigation_comment_line(next.interaction.clone());
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.page_refusal, __ice_next) { self.page_refusal = __ice_next; self.__ice_rev[41] += 1; } }
{ let __ice_next = ({ crate::host::open_link(::std::convert::AsRef::as_ref(&(crate::host::navigation_link(next.interaction.clone())))) }); if ::ducktape_view_guest::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[63] += 1; } }
if ((((comment_line < 0) || self.loading) || self.busy) || (self.active_page).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = crate::host::remember_draft(::std::convert::AsRef::as_ref(&(self.orphaned_comment_drafts)), ::std::convert::AsRef::as_ref(&(self.block_comment_draft))); if ::ducktape_view_guest::state_changed!(self.orphaned_comment_drafts, __ice_next) { self.orphaned_comment_drafts = __ice_next; self.__ice_rev[43] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.block_comment_draft, __ice_next) { self.block_comment_draft = __ice_next; self.__ice_rev[56] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.reply_thread, __ice_next) { self.reply_thread = __ice_next; self.__ice_rev[51] += 1; } }
{ let __ice_next = "".to_owned(); if ::ducktape_view_guest::state_changed!(self.reply_draft, __ice_next) { self.reply_draft = __ice_next; self.__ice_rev[57] += 1; } }
{ let __ice_next = false; if ::ducktape_view_guest::state_changed!(self.resolved_open, __ice_next) { self.resolved_open = __ice_next; self.__ice_rev[53] += 1; } }
{ let __ice_next = crate::host::block_at_line(::std::convert::AsRef::as_ref(&(self.blocks)), comment_line); if ::ducktape_view_guest::state_changed!(self.scope_target, __ice_next) { self.scope_target = __ice_next; self.__ice_rev[45] += 1; } }
{ let __ice_next = (!(self.scope_target).is_empty()); if ::ducktape_view_guest::state_changed!(self.scope_pinned, __ice_next) { self.scope_pinned = __ice_next; self.__ice_rev[46] += 1; } }
{ let __ice_next = self.pointer_y; if ::ducktape_view_guest::state_changed!(self.comment_anchor_y, __ice_next) { self.comment_anchor_y = __ice_next; self.__ice_rev[1] += 1; } }
{ let __ice_next = true; if ::ducktape_view_guest::state_changed!(self.block_comments_open, __ice_next) { self.block_comments_open = __ice_next; self.__ice_rev[44] += 1; } }
{ let __ice_next = comment_line; if ::ducktape_view_guest::state_changed!(self.comment_anchor_line, __ice_next) { self.comment_anchor_line = __ice_next; self.__ice_rev[2] += 1; } }
let opened = crate::host::comments_reserve(self.pages_pane_width, true, comment_line, self.comments_card_height);
if ((opened.line == self.document_reserve.line) && (opened.height == self.document_reserve.height)) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = opened.clone(); if ::ducktape_view_guest::state_changed!(self.document_reserve, __ice_next) { self.document_reserve = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = crate::editor_view::document_presentation(::std::borrow::Borrow::borrow(&(self.document)), self.document_menu.clone(), self.document_dark, self.document_commented.clone(), self.document_marks.clone(), self.document_focused, self.document_reserve.clone()); if ::ducktape_view_guest::state_changed!(self.document_paint, __ice_next) { self.document_paint = __ice_next; self.__ice_rev[7] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesViewMessage::__BindPageDraft(value) => { { let __ice_next = value; if ::ducktape_view_guest::state_changed!(self.page_draft, __ice_next) { self.page_draft = __ice_next; self.__ice_rev[54] += 1; } } ::ducktape_view_guest::Task::none() }
__PagesViewMessage::__BindPageSearchDraft(value) => { { let __ice_next = value; if ::ducktape_view_guest::state_changed!(self.page_search_draft, __ice_next) { self.page_search_draft = __ice_next; self.__ice_rev[55] += 1; } } ::ducktape_view_guest::Task::none() }
__PagesViewMessage::__BindReplyDraft(value) => { { let __ice_next = value; if ::ducktape_view_guest::state_changed!(self.reply_draft, __ice_next) { self.reply_draft = __ice_next; self.__ice_rev[57] += 1; } } ::ducktape_view_guest::Task::none() }
__PagesViewMessage::__BindBlockCommentDraft(value) => { { let __ice_next = value; if ::ducktape_view_guest::state_changed!(self.block_comment_draft, __ice_next) { self.block_comment_draft = __ice_next; self.__ice_rev[56] += 1; } } ::ducktape_view_guest::Task::none() }
__PagesViewMessage::__0T646f63756d656e74(__transaction) => { let __route = __transaction.apply(&mut self.document); self.__ice_rev[8] += 1; __route.map_or_else(::ducktape_view_guest::Task::none, ::ducktape_view_guest::Task::done) },
__PagesViewMessage::__EditDocument(__document) => { __document.apply(&mut self.document); self.__ice_rev[8] += 1; ::ducktape_view_guest::Task::none() }
__PagesViewMessage::__ExternNoop => ::ducktape_view_guest::Task::none(),
};
__task
}

}
}
