#[allow(warnings, clippy::all)]
mod __ice_group_app_update {
use super::*;
impl super::PagesView {
#[allow(clippy::assign_op_pattern)]
pub(super) fn __update(&mut self, message: __PagesViewMessage) -> ::iced::Task<__PagesViewMessage> {
#[cfg(all(target_os = "windows", not(test)))]
if !self.__ice_accessibility.is_attached() && !matches!(&message, __PagesViewMessage::__AccessibilityNativeWindow(_)) {
self.__ice_accessibility_pending.push(message);
return ::iced::Task::none();
}
let __task = match message {
__PagesViewMessage::__AccessibilitySnapshot(__snapshot) => { self.__ice_accessibility.update(*__snapshot); return ::iced::Task::none(); },
__PagesViewMessage::__AccessibilityAction(__request) => { let __refresh = matches!(__request.action, ::ui_lang_runtime::Action::Focus); let __task = self.__ice_accessibility.dispatch(__request); return if __refresh { __task.chain(::ui_lang_runtime::snapshot::<__PagesViewMessage>("PagesView").map(|__snapshot| __PagesViewMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)))) } else { __task }; },
__PagesViewMessage::__AccessibilityWindow(__id, __event) => { self.__ice_accessibility.window_event(__id, __event); return ::iced::Task::none(); },
#[cfg(all(target_os = "windows", not(test)))]
__PagesViewMessage::__AccessibilityNativeWindow(__window) => {
let __id = __window.id();
if !self.__ice_accessibility.attach_window(__window) { return ::iced::Task::none(); }
let __restore = ::iced::window::set_mode(__id, ::iced::window::Mode::Windowed);
let __initial = self.__accessibility_initial_task();
let mut __pending = ::std::vec::Vec::new();
for __message in ::std::mem::take(&mut self.__ice_accessibility_pending) {
__pending.push(self.__update(__message));
}
let __pending = ::iced::Task::batch(__pending);
let __snapshot = ::ui_lang_runtime::snapshot::<__PagesViewMessage>("PagesView").map(|__snapshot| __PagesViewMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)));
return __restore.chain(::iced::Task::batch([__initial, __pending, __snapshot]));
},
#[cfg(all(target_os = "macos", not(test)))]
__PagesViewMessage::__AccessibilityNativeWindow(__window) => {
if !self.__ice_accessibility.attach_window(__window) { return ::iced::Task::none(); }
return ::ui_lang_runtime::snapshot::<__PagesViewMessage>("PagesView").map(|__snapshot| __PagesViewMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)));
},
__PagesViewMessage::__AccessibilityFocusNext(__window) => { return ::ui_lang_runtime::focus_next_in::<__PagesViewMessage>(__window).chain(::ui_lang_runtime::snapshot::<__PagesViewMessage>("PagesView").map(|__snapshot| __PagesViewMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)))); },
__PagesViewMessage::__AccessibilityFocusPrevious(__window) => { return ::ui_lang_runtime::focus_previous_in::<__PagesViewMessage>(__window).chain(::ui_lang_runtime::snapshot::<__PagesViewMessage>("PagesView").map(|__snapshot| __PagesViewMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)))); },
__PagesViewMessage::__TemplateChanged => { return ::iced::Task::none(); },
__PagesViewMessage::SessionArrived(item) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("session_arrived", "src/ui/app.ice:245");
let _ = &item;
{ let __ice_next = item.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[22] += 1; } }
if (!(item.error).is_empty()) { return ::iced::Task::none(); }
let next = item.next.clone();
{ let __ice_next = crate::host::connection_serial_after(self.connected, next.connected, self.register_serial); if ::ui_lang_runtime::state_changed!(self.register_serial, __ice_next) { self.register_serial = __ice_next; self.__ice_rev[19] += 1; } }
{ let __ice_next = next.connected; if ::ui_lang_runtime::state_changed!(self.connected, __ice_next) { self.connected = __ice_next; self.__ice_rev[16] += 1; } }
{ let __ice_next = next.chain.to_owned(); if ::ui_lang_runtime::state_changed!(self.chain, __ice_next) { self.chain = __ice_next; self.__ice_rev[17] += 1; } }
{ let __ice_next = next.dark; if ::ui_lang_runtime::state_changed!(self.document_dark, __ice_next) { self.document_dark = __ice_next; self.__ice_rev[12] += 1; } }
{ let __ice_next = crate::editor_view::document_presentation(::std::borrow::Borrow::borrow(&(self.document)), self.document_menu.clone(), self.document_dark, self.document_commented.clone(), self.document_marks.clone(), self.document_focused, self.document_reserve.clone()); if ::ui_lang_runtime::state_changed!(self.document_paint, __ice_next) { self.document_paint = __ice_next; self.__ice_rev[7] += 1; } }
let route_moved = ((crate::host::route_arrived(next.route_serial, self.route_serial) && (!(next.route_page).is_empty())) && (next.route_page != self.active_page));
{ let __ice_next = next.route_serial; if ::ui_lang_runtime::state_changed!(self.route_serial, __ice_next) { self.route_serial = __ice_next; self.__ice_rev[18] += 1; } }
{ let __ice_next = crate::host::keep_str(route_moved, ::std::convert::AsRef::as_ref(&(next.route_page)), ::std::convert::AsRef::as_ref(&(self.active_page))); if ::ui_lang_runtime::state_changed!(self.active_page, __ice_next) { self.active_page = __ice_next; self.__ice_rev[32] += 1; } }
{ let __ice_next = (self.loading || route_moved); if ::ui_lang_runtime::state_changed!(self.loading, __ice_next) { self.loading = __ice_next; self.__ice_rev[20] += 1; } }
{ let __ice_next = crate::host::page_address(::std::convert::AsRef::as_ref(&(self.active_page)), ::std::convert::AsRef::as_ref(&(self.chain))); if ::ui_lang_runtime::state_changed!(self.page_link, __ice_next) { self.page_link = __ice_next; self.__ice_rev[23] += 1; } }
{ let __ice_next = AppTheme::App; if ::ui_lang_runtime::state_changed!(self.active_palette, __ice_next) { self.active_palette = __ice_next; self.__ice_rev[15] += 1; } }
if (!next.dark) { return ::iced::Task::none(); }
{ let __ice_next = AppTheme::AppDark; if ::ui_lang_runtime::state_changed!(self.active_palette, __ice_next) { self.active_palette = __ice_next; self.__ice_rev[15] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::CommentPointerMoved(_x, y) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("comment_pointer_moved", "src/ui/app.ice:270");
let _ = &_x;
let _ = &y;
{ let __ice_next = y; if ::ui_lang_runtime::state_changed!(self.pointer_y, __ice_next) { self.pointer_y = __ice_next; self.__ice_rev[0] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::ChoosePage(id) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("choose_page", "src/ui/app.ice:282");
let _ = &id;
if ((!(self.host_error).is_empty()) || (id).is_empty()) { return ::iced::Task::none(); }
if (self.loading || self.busy) { return ::iced::Task::none(); }
if (id == self.active_page) { return ::iced::Task::none(); }
{ let __ice_next = id.to_owned(); if ::ui_lang_runtime::state_changed!(self.active_page, __ice_next) { self.active_page = __ice_next; self.__ice_rev[32] += 1; } }
{ let __ice_next = crate::host::page_display_title(::std::convert::AsRef::as_ref(&(self.pages)), ::std::convert::AsRef::as_ref(&(id)), ::std::convert::AsRef::as_ref(&(self.active_page_title))); if ::ui_lang_runtime::state_changed!(self.active_page_title, __ice_next) { self.active_page_title = __ice_next; self.__ice_rev[33] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.active_page_parent, __ice_next) { self.active_page_parent = __ice_next; self.__ice_rev[34] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.blocks, __ice_next) { self.blocks = __ice_next; self.__ice_rev[25] += 1; } }
{ let __ice_next = crate::host::page_address(::std::convert::AsRef::as_ref(&(id)), ::std::convert::AsRef::as_ref(&(self.chain))); if ::ui_lang_runtime::state_changed!(self.page_link, __ice_next) { self.page_link = __ice_next; self.__ice_rev[23] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.loading, __ice_next) { self.loading = __ice_next; self.__ice_rev[20] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::RegisterArrived(item) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("register_arrived", "src/ui/app.ice:293");
let _ = &item;
{ let __ice_next = item.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[22] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.loading, __ice_next) { self.loading = __ice_next; self.__ice_rev[20] += 1; } }
if (!(item.error).is_empty()) { return ::iced::Task::none(); }
let page_moved = (item.active_page != self.buffer_page);
let comments_carry = (self.block_comments_open && (!page_moved));
{ let __ice_next = crate::host::remember_draft(::std::convert::AsRef::as_ref(&(self.orphaned_comment_drafts)), ::std::convert::AsRef::as_ref(&(crate::host::keep_str(page_moved, ::std::convert::AsRef::as_ref(&(self.block_comment_draft)), ::std::convert::AsRef::as_ref(&("")))))); if ::ui_lang_runtime::state_changed!(self.orphaned_comment_drafts, __ice_next) { self.orphaned_comment_drafts = __ice_next; self.__ice_rev[43] += 1; } }
{ let __ice_next = crate::host::keep_str(page_moved, ::std::convert::AsRef::as_ref(&("")), ::std::convert::AsRef::as_ref(&(self.block_comment_draft))); if ::ui_lang_runtime::state_changed!(self.block_comment_draft, __ice_next) { self.block_comment_draft = __ice_next; self.__ice_rev[56] += 1; } }
{ let __ice_next = comments_carry; if ::ui_lang_runtime::state_changed!(self.block_comments_open, __ice_next) { self.block_comments_open = __ice_next; self.__ice_rev[44] += 1; } }
{ let __ice_next = crate::host::keep_i64(comments_carry, self.comment_anchor_line, 0); if ::ui_lang_runtime::state_changed!(self.comment_anchor_line, __ice_next) { self.comment_anchor_line = __ice_next; self.__ice_rev[2] += 1; } }
{ let __ice_next = crate::host::comments_reserve(self.pages_pane_width, comments_carry, self.comment_anchor_line, self.comments_card_height); if ::ui_lang_runtime::state_changed!(self.document_reserve, __ice_next) { self.document_reserve = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = crate::host::keep_str(comments_carry, ::std::convert::AsRef::as_ref(&(self.scope_target)), ::std::convert::AsRef::as_ref(&(""))); if ::ui_lang_runtime::state_changed!(self.scope_target, __ice_next) { self.scope_target = __ice_next; self.__ice_rev[45] += 1; } }
{ let __ice_next = (self.scope_pinned && comments_carry); if ::ui_lang_runtime::state_changed!(self.scope_pinned, __ice_next) { self.scope_pinned = __ice_next; self.__ice_rev[46] += 1; } }
{ let __ice_next = crate::host::keep_str(comments_carry, ::std::convert::AsRef::as_ref(&(self.reply_thread)), ::std::convert::AsRef::as_ref(&(""))); if ::ui_lang_runtime::state_changed!(self.reply_thread, __ice_next) { self.reply_thread = __ice_next; self.__ice_rev[51] += 1; } }
{ let __ice_next = crate::host::keep_str(comments_carry, ::std::convert::AsRef::as_ref(&(self.reply_draft)), ::std::convert::AsRef::as_ref(&(""))); if ::ui_lang_runtime::state_changed!(self.reply_draft, __ice_next) { self.reply_draft = __ice_next; self.__ice_rev[57] += 1; } }
self.expanded_threads = crate::host::kept_ids(comments_carry, ::std::mem::take(&mut self.expanded_threads)); self.__ice_rev[52] += 1;
{ let __ice_next = (self.resolved_open && comments_carry); if ::ui_lang_runtime::state_changed!(self.resolved_open, __ice_next) { self.resolved_open = __ice_next; self.__ice_rev[53] += 1; } }
{ let __ice_next = (self.page_searching && (!page_moved)); if ::ui_lang_runtime::state_changed!(self.page_searching, __ice_next) { self.page_searching = __ice_next; self.__ice_rev[35] += 1; } }
{ let __ice_next = crate::host::keep_str(page_moved, ::std::convert::AsRef::as_ref(&("")), ::std::convert::AsRef::as_ref(&(self.page_search_query))); if ::ui_lang_runtime::state_changed!(self.page_search_query, __ice_next) { self.page_search_query = __ice_next; self.__ice_rev[37] += 1; } }
{ let __ice_next = (self.page_delete_armed && (!page_moved)); if ::ui_lang_runtime::state_changed!(self.page_delete_armed, __ice_next) { self.page_delete_armed = __ice_next; self.__ice_rev[39] += 1; } }
{ let __ice_next = crate::host::keep_str(page_moved, ::std::convert::AsRef::as_ref(&("idle")), ::std::convert::AsRef::as_ref(&(self.autosave))); if ::ui_lang_runtime::state_changed!(self.autosave, __ice_next) { self.autosave = __ice_next; self.__ice_rev[40] += 1; } }
{ let __ice_next = item.pages.clone(); if ::ui_lang_runtime::state_changed!(self.pages, __ice_next) { self.pages = __ice_next; self.__ice_rev[24] += 1; } }
{ let __ice_next = item.blocks.clone(); if ::ui_lang_runtime::state_changed!(self.blocks, __ice_next) { self.blocks = __ice_next; self.__ice_rev[25] += 1; } }
{ let __ice_next = item.subpages.clone(); if ::ui_lang_runtime::state_changed!(self.subpages, __ice_next) { self.subpages = __ice_next; self.__ice_rev[42] += 1; } }
{ let __ice_next = item.active_page_title.to_owned(); if ::ui_lang_runtime::state_changed!(self.active_page_title, __ice_next) { self.active_page_title = __ice_next; self.__ice_rev[33] += 1; } }
{ let __ice_next = item.active_page_parent.to_owned(); if ::ui_lang_runtime::state_changed!(self.active_page_parent, __ice_next) { self.active_page_parent = __ice_next; self.__ice_rev[34] += 1; } }
{ let __ice_next = item.thread_total; if ::ui_lang_runtime::state_changed!(self.thread_total, __ice_next) { self.thread_total = __ice_next; self.__ice_rev[47] += 1; } }
{ let __ice_next = item.comment_rows.clone(); if ::ui_lang_runtime::state_changed!(self.comment_rows, __ice_next) { self.comment_rows = __ice_next; self.__ice_rev[48] += 1; } }
{ let __ice_next = item.commented_hits.clone(); if ::ui_lang_runtime::state_changed!(self.commented_hits, __ice_next) { self.commented_hits = __ice_next; self.__ice_rev[50] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.threads_loading, __ice_next) { self.threads_loading = __ice_next; self.__ice_rev[49] += 1; } }
{ let __ice_next = crate::host::commented_lines(::std::convert::AsRef::as_ref(&(item.blocks)), ::std::convert::AsRef::as_ref(&(item.commented_hits))); if ::ui_lang_runtime::state_changed!(self.document_commented, __ice_next) { self.document_commented = __ice_next; self.__ice_rev[13] += 1; } }
{ let __ice_next = crate::host::comment_marks(::std::convert::AsRef::as_ref(&(item.blocks)), ::std::convert::AsRef::as_ref(&(item.commented_hits))); if ::ui_lang_runtime::state_changed!(self.document_marks, __ice_next) { self.document_marks = __ice_next; self.__ice_rev[14] += 1; } }
{ let __ice_next = crate::host::page_address(::std::convert::AsRef::as_ref(&(item.active_page)), ::std::convert::AsRef::as_ref(&(self.chain))); if ::ui_lang_runtime::state_changed!(self.page_link, __ice_next) { self.page_link = __ice_next; self.__ice_rev[23] += 1; } }
let install = crate::host::install_decision(::std::convert::AsRef::as_ref(&(crate::host::document_text(::std::borrow::Borrow::borrow(&(self.document))))), ::std::convert::AsRef::as_ref(&(self.buffer_page)), ::std::convert::AsRef::as_ref(&(item.active_page)), ::std::convert::AsRef::as_ref(&(self.page_saved_text)), ::std::convert::AsRef::as_ref(&(item.document)));
{ let __ice_next = item.active_page.to_owned(); if ::ui_lang_runtime::state_changed!(self.active_page, __ice_next) { self.active_page = __ice_next; self.__ice_rev[32] += 1; } }
{ let __ice_next = item.active_page.to_owned(); if ::ui_lang_runtime::state_changed!(self.buffer_page, __ice_next) { self.buffer_page = __ice_next; self.__ice_rev[61] += 1; } }
{ let __ice_next = crate::editor_view::document_presentation(::std::borrow::Borrow::borrow(&(self.document)), self.document_menu.clone(), self.document_dark, self.document_commented.clone(), self.document_marks.clone(), self.document_focused, self.document_reserve.clone()); if ::ui_lang_runtime::state_changed!(self.document_paint, __ice_next) { self.document_paint = __ice_next; self.__ice_rev[7] += 1; } }
if (!install) { return ::iced::Task::none(); }
{ let __ice_next = item.document.to_owned(); if ::ui_lang_runtime::state_changed!(self.page_saved_text, __ice_next) { self.page_saved_text = __ice_next; self.__ice_rev[60] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.page_refusal, __ice_next) { self.page_refusal = __ice_next; self.__ice_rev[41] += 1; } }
{ let __reset = self.document.reset_revision(); let __next = crate::host::document_editor(::std::convert::AsRef::as_ref(&(item.document))); self.document.replace(__next, __reset); }; self.__ice_rev[8] += 1;
{ let __ice_next = crate::editor_binding::initial_menu(); if ::ui_lang_runtime::state_changed!(self.document_menu, __ice_next) { self.document_menu = __ice_next; self.__ice_rev[10] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.document_focused, __ice_next) { self.document_focused = __ice_next; self.__ice_rev[5] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.document_error, __ice_next) { self.document_error = __ice_next; self.__ice_rev[11] += 1; } }
{ let __ice_next = crate::editor_view::document_presentation(::std::borrow::Borrow::borrow(&(self.document)), self.document_menu.clone(), self.document_dark, self.document_commented.clone(), self.document_marks.clone(), self.document_focused, self.document_reserve.clone()); if ::ui_lang_runtime::state_changed!(self.document_paint, __ice_next) { self.document_paint = __ice_next; self.__ice_rev[7] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::SearchArrived(item) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("search_arrived", "src/ui/app.ice:359");
let _ = &item;
{ let __ice_next = item.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[22] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.page_searching, __ice_next) { self.page_searching = __ice_next; self.__ice_rev[35] += 1; } }
if (item.query != self.page_search_query) { return ::iced::Task::none(); }
{ let __ice_next = item.hits.clone(); if ::ui_lang_runtime::state_changed!(self.page_search_hits, __ice_next) { self.page_search_hits = __ice_next; self.__ice_rev[36] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::ActDone(item) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("act_done", "src/ui/app.ice:365");
let _ = &item;
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.busy, __ice_next) { self.busy = __ice_next; self.__ice_rev[21] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.threads_loading, __ice_next) { self.threads_loading = __ice_next; self.__ice_rev[49] += 1; } }
{ let __ice_next = item.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[22] += 1; } }
{ let __ice_next = (self.register_serial + 1); if ::ui_lang_runtime::state_changed!(self.register_serial, __ice_next) { self.register_serial = __ice_next; self.__ice_rev[19] += 1; } }
let refused = (!(item.error).is_empty());
{ let __ice_next = crate::host::keep_str(refused, ::std::convert::AsRef::as_ref(&(self.pending_page)), ::std::convert::AsRef::as_ref(&(self.page_draft))); if ::ui_lang_runtime::state_changed!(self.page_draft, __ice_next) { self.page_draft = __ice_next; self.__ice_rev[54] += 1; } }
{ let __ice_next = crate::host::keep_str((refused && (!(self.reply_thread).is_empty())), ::std::convert::AsRef::as_ref(&(self.pending_comment)), ::std::convert::AsRef::as_ref(&(self.reply_draft))); if ::ui_lang_runtime::state_changed!(self.reply_draft, __ice_next) { self.reply_draft = __ice_next; self.__ice_rev[57] += 1; } }
{ let __ice_next = crate::host::keep_str((refused && (self.reply_thread).is_empty()), ::std::convert::AsRef::as_ref(&(self.pending_comment)), ::std::convert::AsRef::as_ref(&(self.block_comment_draft))); if ::ui_lang_runtime::state_changed!(self.block_comment_draft, __ice_next) { self.block_comment_draft = __ice_next; self.__ice_rev[56] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.pending_page, __ice_next) { self.pending_page = __ice_next; self.__ice_rev[58] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.pending_comment, __ice_next) { self.pending_comment = __ice_next; self.__ice_rev[59] += 1; } }
if refused { return ::iced::Task::none(); }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.page_create_open, __ice_next) { self.page_create_open = __ice_next; self.__ice_rev[31] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.page_delete_armed, __ice_next) { self.page_delete_armed = __ice_next; self.__ice_rev[39] += 1; } }
{ let __ice_next = crate::host::keep_str((!(item.page).is_empty()), ::std::convert::AsRef::as_ref(&(item.page)), ::std::convert::AsRef::as_ref(&(self.active_page))); if ::ui_lang_runtime::state_changed!(self.active_page, __ice_next) { self.active_page = __ice_next; self.__ice_rev[32] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::SaveDone(item) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("save_done", "src/ui/app.ice:384");
let _ = &item;
{ let __ice_next = "error".to_owned(); if ::ui_lang_runtime::state_changed!(self.autosave, __ice_next) { self.autosave = __ice_next; self.__ice_rev[40] += 1; } }
{ let __ice_next = item.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[22] += 1; } }
if (!(item.error).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[22] += 1; } }
{ let __ice_next = item.refusal.to_owned(); if ::ui_lang_runtime::state_changed!(self.page_refusal, __ice_next) { self.page_refusal = __ice_next; self.__ice_rev[41] += 1; } }
{ let __ice_next = crate::host::baseline_at_submitted_title(::std::convert::AsRef::as_ref(&(crate::host::saved_baseline(item.written, ::std::convert::AsRef::as_ref(&(item.document)), ::std::convert::AsRef::as_ref(&(self.page_inflight_text))))), ::std::convert::AsRef::as_ref(&(self.page_inflight_text))); if ::ui_lang_runtime::state_changed!(self.page_saved_text, __ice_next) { self.page_saved_text = __ice_next; self.__ice_rev[60] += 1; } }
{ let __ice_next = "saved".to_owned(); if ::ui_lang_runtime::state_changed!(self.autosave, __ice_next) { self.autosave = __ice_next; self.__ice_rev[40] += 1; } }
{ let __ice_next = (self.register_serial + crate::host::keep_i64(item.written, 1, 0)); if ::ui_lang_runtime::state_changed!(self.register_serial, __ice_next) { self.register_serial = __ice_next; self.__ice_rev[19] += 1; } }
if (item.refusal).is_empty() { return ::iced::Task::none(); }
let untouched = (crate::host::document_text(::std::borrow::Borrow::borrow(&(self.document))) == self.page_inflight_text);
{ let __ice_next = crate::host::baseline_at_submitted_title(::std::convert::AsRef::as_ref(&(item.document)), ::std::convert::AsRef::as_ref(&(self.page_inflight_text))); if ::ui_lang_runtime::state_changed!(self.page_saved_text, __ice_next) { self.page_saved_text = __ice_next; self.__ice_rev[60] += 1; } }
{ let __ice_next = "idle".to_owned(); if ::ui_lang_runtime::state_changed!(self.autosave, __ice_next) { self.autosave = __ice_next; self.__ice_rev[40] += 1; } }
if (!untouched) { return ::iced::Task::none(); }
{ let __reset = self.document.reset_revision(); let __next = crate::host::document_editor(::std::convert::AsRef::as_ref(&(item.document))); self.document.replace(__next, __reset); }; self.__ice_rev[8] += 1;
{ let __ice_next = crate::editor_view::document_presentation(::std::borrow::Borrow::borrow(&(self.document)), self.document_menu.clone(), self.document_dark, self.document_commented.clone(), self.document_marks.clone(), self.document_focused, self.document_reserve.clone()); if ::ui_lang_runtime::state_changed!(self.document_paint, __ice_next) { self.document_paint = __ice_next; self.__ice_rev[7] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::PageAutosaveTick => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("page_autosave_tick", "src/ui/app.ice:409");
if (!(self.host_error).is_empty()) { return ::iced::Task::none(); }
if (((self.busy || self.loading) || (self.active_page).is_empty()) || (self.active_page != self.buffer_page)) { return ::iced::Task::none(); }
if (self.autosave == "saving") { return ::iced::Task::none(); }
let text = crate::host::document_text(::std::borrow::Borrow::borrow(&(self.document)));
if (text == self.page_saved_text) { return ::iced::Task::none(); }
{ let __ice_next = "idle".to_owned(); if ::ui_lang_runtime::state_changed!(self.autosave, __ice_next) { self.autosave = __ice_next; self.__ice_rev[40] += 1; } }
if crate::host::has_unclosed_fence(::std::convert::AsRef::as_ref(&(text))) { return ::iced::Task::none(); }
{ let __ice_next = "saving".to_owned(); if ::ui_lang_runtime::state_changed!(self.autosave, __ice_next) { self.autosave = __ice_next; self.__ice_rev[40] += 1; } }
{ let __ice_next = text.to_owned(); if ::ui_lang_runtime::state_changed!(self.page_inflight_text, __ice_next) { self.page_inflight_text = __ice_next; self.__ice_rev[62] += 1; } }
{ let __ice_next = ({ let __ice_call = ::ui_lang_runtime::dev::Span::extern_call("save", "src/ui/app.ice:63"); crate::host::save(::std::convert::AsRef::as_ref(&(self.active_page)), ::std::convert::AsRef::as_ref(&(text)), ::std::convert::AsRef::as_ref(&(self.page_saved_text))) }); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[63] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::TogglePageCreate => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("toggle_page_create", "src/ui/app.ice:432");
if (!(self.host_error).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = (!self.page_create_open); if ::ui_lang_runtime::state_changed!(self.page_create_open, __ice_next) { self.page_create_open = __ice_next; self.__ice_rev[31] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::CreatePageSubmit => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("create_page_submit", "src/ui/app.ice:438");
if (!(self.host_error).is_empty()) { return ::iced::Task::none(); }
if (((self.loading || self.busy) || (!self.connected)) || ((self.page_draft).trim().to_owned()).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.busy, __ice_next) { self.busy = __ice_next; self.__ice_rev[21] += 1; } }
{ let __ice_next = (self.page_draft).trim().to_owned(); if ::ui_lang_runtime::state_changed!(self.pending_page, __ice_next) { self.pending_page = __ice_next; self.__ice_rev[58] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.page_draft, __ice_next) { self.page_draft = __ice_next; self.__ice_rev[54] += 1; } }
{ let __ice_next = ({ let __ice_call = ::ui_lang_runtime::dev::Span::extern_call("create", "src/ui/app.ice:59"); crate::host::create(::std::convert::AsRef::as_ref(&(self.pending_page))) }); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[63] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::ArmPageDelete => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("arm_page_delete", "src/ui/app.ice:446");
if (!(self.host_error).is_empty()) { return ::iced::Task::none(); }
if ((self.loading || self.busy) || (self.active_page).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.page_menu_open, __ice_next) { self.page_menu_open = __ice_next; self.__ice_rev[30] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.page_delete_armed, __ice_next) { self.page_delete_armed = __ice_next; self.__ice_rev[39] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::DisarmPageDelete => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("disarm_page_delete", "src/ui/app.ice:452");
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.page_delete_armed, __ice_next) { self.page_delete_armed = __ice_next; self.__ice_rev[39] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::DeletePageSubmit => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("delete_page_submit", "src/ui/app.ice:455");
if (!(self.host_error).is_empty()) { return ::iced::Task::none(); }
if (((self.loading || self.busy) || (self.active_page).is_empty()) || (!self.page_delete_armed)) { return ::iced::Task::none(); }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.busy, __ice_next) { self.busy = __ice_next; self.__ice_rev[21] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.page_delete_armed, __ice_next) { self.page_delete_armed = __ice_next; self.__ice_rev[39] += 1; } }
{ let __ice_next = crate::host::remember_draft(::std::convert::AsRef::as_ref(&(self.orphaned_comment_drafts)), ::std::convert::AsRef::as_ref(&(self.block_comment_draft))); if ::ui_lang_runtime::state_changed!(self.orphaned_comment_drafts, __ice_next) { self.orphaned_comment_drafts = __ice_next; self.__ice_rev[43] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.block_comment_draft, __ice_next) { self.block_comment_draft = __ice_next; self.__ice_rev[56] += 1; } }
{ let __ice_next = ({ let __ice_call = ::ui_lang_runtime::dev::Span::extern_call("delete", "src/ui/app.ice:60"); crate::host::delete(::std::convert::AsRef::as_ref(&(self.active_page))) }); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[63] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::SearchPagesSubmit => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("search_pages_submit", "src/ui/app.ice:464");
if (!(self.host_error).is_empty()) { return ::iced::Task::none(); }
if (self.page_searching || ((self.page_search_draft).trim().to_owned()).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.page_searching, __ice_next) { self.page_searching = __ice_next; self.__ice_rev[35] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.page_search_hits, __ice_next) { self.page_search_hits = __ice_next; self.__ice_rev[36] += 1; } }
{ let __ice_next = (self.page_search_draft).trim().to_owned(); if ::ui_lang_runtime::state_changed!(self.page_search_query, __ice_next) { self.page_search_query = __ice_next; self.__ice_rev[37] += 1; } }
{ let __ice_next = (self.page_search_serial + 1); if ::ui_lang_runtime::state_changed!(self.page_search_serial, __ice_next) { self.page_search_serial = __ice_next; self.__ice_rev[38] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::ClearPageSearch => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("clear_page_search", "src/ui/app.ice:472");
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.page_search_draft, __ice_next) { self.page_search_draft = __ice_next; self.__ice_rev[55] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.page_search_hits, __ice_next) { self.page_search_hits = __ice_next; self.__ice_rev[36] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.page_searching, __ice_next) { self.page_searching = __ice_next; self.__ice_rev[35] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.page_search_query, __ice_next) { self.page_search_query = __ice_next; self.__ice_rev[37] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::OpenPageSearchHit(page_id, _block_id) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("open_page_search_hit", "src/ui/app.ice:478");
let _ = &page_id;
let _ = &_block_id;
if (!(self.host_error).is_empty()) { return ::iced::Task::none(); }
if (self.loading || self.busy) { return ::iced::Task::none(); }
return (::iced::Task::done(page_id.to_owned())).map(|value| __PagesViewMessage::ChoosePage(value));
})(),
__PagesViewMessage::UseOrphanedCommentDraft(draft) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("use_orphaned_comment_draft", "src/ui/app.ice:485");
let _ = &draft;
if (!(self.host_error).is_empty()) { return ::iced::Task::none(); }
if ((self.loading || self.busy) || (!((self.block_comment_draft).trim().to_owned()).is_empty())) { return ::iced::Task::none(); }
{ let __ice_next = draft.to_owned(); if ::ui_lang_runtime::state_changed!(self.block_comment_draft, __ice_next) { self.block_comment_draft = __ice_next; self.__ice_rev[56] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.block_comments_open, __ice_next) { self.block_comments_open = __ice_next; self.__ice_rev[44] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.scope_target, __ice_next) { self.scope_target = __ice_next; self.__ice_rev[45] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.scope_pinned, __ice_next) { self.scope_pinned = __ice_next; self.__ice_rev[46] += 1; } }
{ let __ice_next = crate::host::forget_draft(::std::convert::AsRef::as_ref(&(self.orphaned_comment_drafts)), ::std::convert::AsRef::as_ref(&(draft))); if ::ui_lang_runtime::state_changed!(self.orphaned_comment_drafts, __ice_next) { self.orphaned_comment_drafts = __ice_next; self.__ice_rev[43] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.comment_anchor_line, __ice_next) { self.comment_anchor_line = __ice_next; self.__ice_rev[2] += 1; } }
let recovered = crate::host::comments_reserve(self.pages_pane_width, true, 0, self.comments_card_height);
if ((recovered.line == self.document_reserve.line) && (recovered.height == self.document_reserve.height)) { return ::iced::Task::none(); }
{ let __ice_next = recovered.clone(); if ::ui_lang_runtime::state_changed!(self.document_reserve, __ice_next) { self.document_reserve = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = crate::editor_view::document_presentation(::std::borrow::Borrow::borrow(&(self.document)), self.document_menu.clone(), self.document_dark, self.document_commented.clone(), self.document_marks.clone(), self.document_focused, self.document_reserve.clone()); if ::ui_lang_runtime::state_changed!(self.document_paint, __ice_next) { self.document_paint = __ice_next; self.__ice_rev[7] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::DiscardOrphanedCommentDraft(draft) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("discard_orphaned_comment_draft", "src/ui/app.ice:502");
let _ = &draft;
{ let __ice_next = crate::host::forget_draft(::std::convert::AsRef::as_ref(&(self.orphaned_comment_drafts)), ::std::convert::AsRef::as_ref(&(draft))); if ::ui_lang_runtime::state_changed!(self.orphaned_comment_drafts, __ice_next) { self.orphaned_comment_drafts = __ice_next; self.__ice_rev[43] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::ToggleBlockComments => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("toggle_block_comments", "src/ui/app.ice:510");
{ let __ice_next = (-1.0); if ::ui_lang_runtime::state_changed!(self.comment_anchor_y, __ice_next) { self.comment_anchor_y = __ice_next; self.__ice_rev[1] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.comment_anchor_line, __ice_next) { self.comment_anchor_line = __ice_next; self.__ice_rev[2] += 1; } }
if (!(self.host_error).is_empty()) { return ::iced::Task::none(); }
if ((self.loading || self.busy) || (self.active_page).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = crate::host::remember_draft(::std::convert::AsRef::as_ref(&(self.orphaned_comment_drafts)), ::std::convert::AsRef::as_ref(&(self.block_comment_draft))); if ::ui_lang_runtime::state_changed!(self.orphaned_comment_drafts, __ice_next) { self.orphaned_comment_drafts = __ice_next; self.__ice_rev[43] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.block_comment_draft, __ice_next) { self.block_comment_draft = __ice_next; self.__ice_rev[56] += 1; } }
{ let __ice_next = (!self.block_comments_open); if ::ui_lang_runtime::state_changed!(self.block_comments_open, __ice_next) { self.block_comments_open = __ice_next; self.__ice_rev[44] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.scope_target, __ice_next) { self.scope_target = __ice_next; self.__ice_rev[45] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.scope_pinned, __ice_next) { self.scope_pinned = __ice_next; self.__ice_rev[46] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.reply_thread, __ice_next) { self.reply_thread = __ice_next; self.__ice_rev[51] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.reply_draft, __ice_next) { self.reply_draft = __ice_next; self.__ice_rev[57] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.resolved_open, __ice_next) { self.resolved_open = __ice_next; self.__ice_rev[53] += 1; } }
let toggled_reserve = crate::host::comments_reserve(self.pages_pane_width, self.block_comments_open, 0, self.comments_card_height);
if ((toggled_reserve.line == self.document_reserve.line) && (toggled_reserve.height == self.document_reserve.height)) { return ::iced::Task::none(); }
{ let __ice_next = toggled_reserve.clone(); if ::ui_lang_runtime::state_changed!(self.document_reserve, __ice_next) { self.document_reserve = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = crate::editor_view::document_presentation(::std::borrow::Borrow::borrow(&(self.document)), self.document_menu.clone(), self.document_dark, self.document_commented.clone(), self.document_marks.clone(), self.document_focused, self.document_reserve.clone()); if ::ui_lang_runtime::state_changed!(self.document_paint, __ice_next) { self.document_paint = __ice_next; self.__ice_rev[7] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::CloseBlockComments => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("close_block_comments", "src/ui/app.ice:528");
{ let __ice_next = (-1.0); if ::ui_lang_runtime::state_changed!(self.comment_anchor_y, __ice_next) { self.comment_anchor_y = __ice_next; self.__ice_rev[1] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.comment_anchor_line, __ice_next) { self.comment_anchor_line = __ice_next; self.__ice_rev[2] += 1; } }
{ let __ice_next = crate::host::remember_draft(::std::convert::AsRef::as_ref(&(self.orphaned_comment_drafts)), ::std::convert::AsRef::as_ref(&(self.block_comment_draft))); if ::ui_lang_runtime::state_changed!(self.orphaned_comment_drafts, __ice_next) { self.orphaned_comment_drafts = __ice_next; self.__ice_rev[43] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.block_comment_draft, __ice_next) { self.block_comment_draft = __ice_next; self.__ice_rev[56] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.block_comments_open, __ice_next) { self.block_comments_open = __ice_next; self.__ice_rev[44] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.scope_target, __ice_next) { self.scope_target = __ice_next; self.__ice_rev[45] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.scope_pinned, __ice_next) { self.scope_pinned = __ice_next; self.__ice_rev[46] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.reply_thread, __ice_next) { self.reply_thread = __ice_next; self.__ice_rev[51] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.reply_draft, __ice_next) { self.reply_draft = __ice_next; self.__ice_rev[57] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.resolved_open, __ice_next) { self.resolved_open = __ice_next; self.__ice_rev[53] += 1; } }
if (self.document_reserve.height == 0) { return ::iced::Task::none(); }
{ let __ice_next = crate::editor_view::no_reserve(); if ::ui_lang_runtime::state_changed!(self.document_reserve, __ice_next) { self.document_reserve = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = crate::editor_view::document_presentation(::std::borrow::Borrow::borrow(&(self.document)), self.document_menu.clone(), self.document_dark, self.document_commented.clone(), self.document_marks.clone(), self.document_focused, self.document_reserve.clone()); if ::ui_lang_runtime::state_changed!(self.document_paint, __ice_next) { self.document_paint = __ice_next; self.__ice_rev[7] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::NarrowCommentScope(target) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("narrow_comment_scope", "src/ui/app.ice:546");
let _ = &target;
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.reply_thread, __ice_next) { self.reply_thread = __ice_next; self.__ice_rev[51] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.reply_draft, __ice_next) { self.reply_draft = __ice_next; self.__ice_rev[57] += 1; } }
if (!(self.host_error).is_empty()) { return ::iced::Task::none(); }
if ((self.loading || self.busy) || (target).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = target.to_owned(); if ::ui_lang_runtime::state_changed!(self.scope_target, __ice_next) { self.scope_target = __ice_next; self.__ice_rev[45] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::WidenCommentScope => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("widen_comment_scope", "src/ui/app.ice:555");
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.reply_thread, __ice_next) { self.reply_thread = __ice_next; self.__ice_rev[51] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.reply_draft, __ice_next) { self.reply_draft = __ice_next; self.__ice_rev[57] += 1; } }
if (!(self.host_error).is_empty()) { return ::iced::Task::none(); }
if ((self.loading || self.busy) || self.scope_pinned) { return ::iced::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.scope_target, __ice_next) { self.scope_target = __ice_next; self.__ice_rev[45] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::ResolveThreadSubmit(id, resolved) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("resolve_thread_submit", "src/ui/app.ice:562");
let _ = &id;
let _ = &resolved;
if (!(self.host_error).is_empty()) { return ::iced::Task::none(); }
if ((((self.loading || self.busy) || self.threads_loading) || (!self.block_comments_open)) || (id).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.busy, __ice_next) { self.busy = __ice_next; self.__ice_rev[21] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.threads_loading, __ice_next) { self.threads_loading = __ice_next; self.__ice_rev[49] += 1; } }
{ let __ice_next = ({ let __ice_call = ::ui_lang_runtime::dev::Span::extern_call("resolve", "src/ui/app.ice:62"); crate::host::resolve(::std::convert::AsRef::as_ref(&(id)), resolved) }); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[63] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::SelectReplyThread(id) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("select_reply_thread", "src/ui/app.ice:572");
let _ = &id;
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.reply_draft, __ice_next) { self.reply_draft = __ice_next; self.__ice_rev[57] += 1; } }
{ let __ice_next = crate::host::reply_thread_after_press(::std::convert::AsRef::as_ref(&(self.reply_thread)), ::std::convert::AsRef::as_ref(&(id))); if ::ui_lang_runtime::state_changed!(self.reply_thread, __ice_next) { self.reply_thread = __ice_next; self.__ice_rev[51] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::ToggleThreadReplies(id) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("toggle_thread_replies", "src/ui/app.ice:576");
let _ = &id;
self.expanded_threads = crate::host::toggled(::std::mem::take(&mut self.expanded_threads), ::std::convert::AsRef::as_ref(&(id))); self.__ice_rev[52] += 1;
::iced::Task::none()
})(),
__PagesViewMessage::ToggleResolvedComments => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("toggle_resolved_comments", "src/ui/app.ice:579");
{ let __ice_next = (!self.resolved_open); if ::ui_lang_runtime::state_changed!(self.resolved_open, __ice_next) { self.resolved_open = __ice_next; self.__ice_rev[53] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::PostThreadReply(id) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("post_thread_reply", "src/ui/app.ice:586");
let _ = &id;
if (!(self.host_error).is_empty()) { return ::iced::Task::none(); }
if ((((self.loading || self.busy) || self.threads_loading) || (!self.block_comments_open)) || ((self.reply_draft).trim().to_owned()).is_empty()) { return ::iced::Task::none(); }
let reply_target = crate::host::comment_post_target(::std::convert::AsRef::as_ref(&(self.comment_rows)), ::std::convert::AsRef::as_ref(&(id)), ::std::convert::AsRef::as_ref(&(self.scope_target)));
if (reply_target).is_empty() { return ::iced::Task::none(); }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.busy, __ice_next) { self.busy = __ice_next; self.__ice_rev[21] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.threads_loading, __ice_next) { self.threads_loading = __ice_next; self.__ice_rev[49] += 1; } }
{ let __ice_next = (self.reply_draft).trim().to_owned(); if ::ui_lang_runtime::state_changed!(self.pending_comment, __ice_next) { self.pending_comment = __ice_next; self.__ice_rev[59] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.reply_draft, __ice_next) { self.reply_draft = __ice_next; self.__ice_rev[57] += 1; } }
{ let __ice_next = ({ let __ice_call = ::ui_lang_runtime::dev::Span::extern_call("post", "src/ui/app.ice:61"); crate::host::post(::std::convert::AsRef::as_ref(&(self.pending_comment)), ::std::convert::AsRef::as_ref(&(reply_target)), ::std::convert::AsRef::as_ref(&(id))) }); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[63] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::PostBlockCommentSubmit => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("post_block_comment_submit", "src/ui/app.ice:600");
if (!(self.host_error).is_empty()) { return ::iced::Task::none(); }
if (((((self.loading || self.busy) || self.threads_loading) || (!self.block_comments_open)) || (self.active_page).is_empty()) || ((self.block_comment_draft).trim().to_owned()).is_empty()) { return ::iced::Task::none(); }
let fresh_target = crate::host::keep_str((!(self.scope_target).is_empty()), ::std::convert::AsRef::as_ref(&(self.scope_target)), ::std::convert::AsRef::as_ref(&(self.active_page)));
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.busy, __ice_next) { self.busy = __ice_next; self.__ice_rev[21] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.threads_loading, __ice_next) { self.threads_loading = __ice_next; self.__ice_rev[49] += 1; } }
{ let __ice_next = (self.block_comment_draft).trim().to_owned(); if ::ui_lang_runtime::state_changed!(self.pending_comment, __ice_next) { self.pending_comment = __ice_next; self.__ice_rev[59] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.block_comment_draft, __ice_next) { self.block_comment_draft = __ice_next; self.__ice_rev[56] += 1; } }
{ let __ice_next = ({ let __ice_call = ::ui_lang_runtime::dev::Span::extern_call("post", "src/ui/app.ice:61"); crate::host::post(::std::convert::AsRef::as_ref(&(self.pending_comment)), ::std::convert::AsRef::as_ref(&(fresh_target)), ::std::convert::AsRef::as_ref(&(""))) }); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[63] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::CopyToClipboard(text, label) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("copy_to_clipboard", "src/ui/app.ice:610");
let _ = &text;
let _ = &label;
{ let __ice_next = ({ let __ice_call = ::ui_lang_runtime::dev::Span::extern_call("copy", "src/ui/app.ice:64"); crate::host::copy(::std::convert::AsRef::as_ref(&(text)), ::std::convert::AsRef::as_ref(&(label))) }); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[63] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::DocumentPointerReleased(_button) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("document_pointer_released", "src/ui/app.ice:613");
let _ = &_button;
{ let __ice_next = (self.focus_query + 1); if ::ui_lang_runtime::state_changed!(self.focus_query, __ice_next) { self.focus_query = __ice_next; self.__ice_rev[6] += 1; } }
let query = self.focus_query;
return ::ui_lang_guest::widget::is_focused(::std::string::String::from("PagesView/root/pages/document")).map(move |value| __PagesViewMessage::DocumentFocusChecked(query, value));
})(),
__PagesViewMessage::DocumentKeyReleased(_key) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("document_key_released", "src/ui/app.ice:618");
let _ = &_key;
{ let __ice_next = (self.focus_query + 1); if ::ui_lang_runtime::state_changed!(self.focus_query, __ice_next) { self.focus_query = __ice_next; self.__ice_rev[6] += 1; } }
let query = self.focus_query;
return ::ui_lang_guest::widget::is_focused(::std::string::String::from("PagesView/root/pages/document")).map(move |value| __PagesViewMessage::DocumentFocusChecked(query, value));
})(),
__PagesViewMessage::DocumentWindowFocused => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("document_window_focused", "src/ui/app.ice:623");
{ let __ice_next = (self.focus_query + 1); if ::ui_lang_runtime::state_changed!(self.focus_query, __ice_next) { self.focus_query = __ice_next; self.__ice_rev[6] += 1; } }
let query = self.focus_query;
return ::ui_lang_guest::widget::is_focused(::std::string::String::from("PagesView/root/pages/document")).map(move |value| __PagesViewMessage::DocumentFocusChecked(query, value));
})(),
__PagesViewMessage::DocumentWindowUnfocused => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("document_window_unfocused", "src/ui/app.ice:628");
{ let __ice_next = (self.focus_query + 1); if ::ui_lang_runtime::state_changed!(self.focus_query, __ice_next) { self.focus_query = __ice_next; self.__ice_rev[6] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.document_focused, __ice_next) { self.document_focused = __ice_next; self.__ice_rev[5] += 1; } }
{ let __ice_next = crate::editor_view::document_presentation(::std::borrow::Borrow::borrow(&(self.document)), self.document_menu.clone(), self.document_dark, self.document_commented.clone(), self.document_marks.clone(), self.document_focused, self.document_reserve.clone()); if ::ui_lang_runtime::state_changed!(self.document_paint, __ice_next) { self.document_paint = __ice_next; self.__ice_rev[7] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::DocumentFocusChecked(query, focused) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("document_focus_checked", "src/ui/app.ice:633");
let _ = &query;
let _ = &focused;
if ((query != self.focus_query) || (focused == self.document_focused)) { return ::iced::Task::none(); }
{ let __ice_next = focused; if ::ui_lang_runtime::state_changed!(self.document_focused, __ice_next) { self.document_focused = __ice_next; self.__ice_rev[5] += 1; } }
{ let __ice_next = crate::editor_view::document_presentation(::std::borrow::Borrow::borrow(&(self.document)), self.document_menu.clone(), self.document_dark, self.document_commented.clone(), self.document_marks.clone(), self.document_focused, self.document_reserve.clone()); if ::ui_lang_runtime::state_changed!(self.document_paint, __ice_next) { self.document_paint = __ice_next; self.__ice_rev[7] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::SidebarResized(dx, _dy) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("sidebar_resized", "src/ui/app.ice:638");
let _ = &dx;
let _ = &_dy;
{ let __ice_next = crate::host::sidebar_width_after_delta(self.sidebar_width, dx, self.pages_viewport_width); if ::ui_lang_runtime::state_changed!(self.sidebar_width, __ice_next) { self.sidebar_width = __ice_next; self.__ice_rev[29] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::PagesViewportChanged(width, _height) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("pages_viewport_changed", "src/ui/app.ice:641");
let _ = &width;
let _ = &_height;
{ let __ice_next = width; if ::ui_lang_runtime::state_changed!(self.pages_viewport_width, __ice_next) { self.pages_viewport_width = __ice_next; self.__ice_rev[26] += 1; } }
{ let __ice_next = crate::host::sidebar_width_after_delta(self.sidebar_width, 0.0, width); if ::ui_lang_runtime::state_changed!(self.sidebar_width, __ice_next) { self.sidebar_width = __ice_next; self.__ice_rev[29] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::PagesPaneResized(width, _height) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("pages_pane_resized", "src/ui/app.ice:655");
let _ = &width;
let _ = &_height;
{ let __ice_next = width; if ::ui_lang_runtime::state_changed!(self.pages_pane_width, __ice_next) { self.pages_pane_width = __ice_next; self.__ice_rev[28] += 1; } }
let next = crate::host::comments_reserve(width, self.block_comments_open, self.comment_anchor_line, self.comments_card_height);
if ((next.line == self.document_reserve.line) && (next.height == self.document_reserve.height)) { return ::iced::Task::none(); }
{ let __ice_next = next.clone(); if ::ui_lang_runtime::state_changed!(self.document_reserve, __ice_next) { self.document_reserve = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = crate::editor_view::document_presentation(::std::borrow::Borrow::borrow(&(self.document)), self.document_menu.clone(), self.document_dark, self.document_commented.clone(), self.document_marks.clone(), self.document_focused, self.document_reserve.clone()); if ::ui_lang_runtime::state_changed!(self.document_paint, __ice_next) { self.document_paint = __ice_next; self.__ice_rev[7] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::CommentsCardMeasured(_width, height) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("comments_card_measured", "src/ui/app.ice:665");
let _ = &_width;
let _ = &height;
let measured = crate::host::measured_card_height(self.comments_card_height, height);
if (measured == self.comments_card_height) { return ::iced::Task::none(); }
{ let __ice_next = measured; if ::ui_lang_runtime::state_changed!(self.comments_card_height, __ice_next) { self.comments_card_height = __ice_next; self.__ice_rev[3] += 1; } }
let next = crate::host::comments_reserve(self.pages_pane_width, self.block_comments_open, self.comment_anchor_line, measured);
if ((next.line == self.document_reserve.line) && (next.height == self.document_reserve.height)) { return ::iced::Task::none(); }
{ let __ice_next = next.clone(); if ::ui_lang_runtime::state_changed!(self.document_reserve, __ice_next) { self.document_reserve = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = crate::editor_view::document_presentation(::std::borrow::Borrow::borrow(&(self.document)), self.document_menu.clone(), self.document_dark, self.document_commented.clone(), self.document_marks.clone(), self.document_focused, self.document_reserve.clone()); if ::ui_lang_runtime::state_changed!(self.document_paint, __ice_next) { self.document_paint = __ice_next; self.__ice_rev[7] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::TogglePageMenu => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("toggle_page_menu", "src/ui/app.ice:674");
{ let __ice_next = (!self.page_menu_open); if ::ui_lang_runtime::state_changed!(self.page_menu_open, __ice_next) { self.page_menu_open = __ice_next; self.__ice_rev[30] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::ClosePageMenu => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("close_page_menu", "src/ui/app.ice:677");
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.page_menu_open, __ice_next) { self.page_menu_open = __ice_next; self.__ice_rev[30] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::DocumentCommitted(next) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("document_committed", "src/ui/app.ice:682");
let _ = &next;
{ let __ice_next = next.history.clone(); if ::ui_lang_runtime::state_changed!(self.document_history, __ice_next) { self.document_history = __ice_next; self.__ice_rev[9] += 1; } }
{ let __ice_next = next.menu.clone(); if ::ui_lang_runtime::state_changed!(self.document_menu, __ice_next) { self.document_menu = __ice_next; self.__ice_rev[10] += 1; } }
{ let __ice_next = crate::editor_view::document_presentation(::std::borrow::Borrow::borrow(&(self.document)), self.document_menu.clone(), self.document_dark, self.document_commented.clone(), self.document_marks.clone(), self.document_focused, self.document_reserve.clone()); if ::ui_lang_runtime::state_changed!(self.document_paint, __ice_next) { self.document_paint = __ice_next; self.__ice_rev[7] += 1; } }
let comment_line = crate::host::navigation_comment_line(next.interaction.clone());
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.page_refusal, __ice_next) { self.page_refusal = __ice_next; self.__ice_rev[41] += 1; } }
{ let __ice_next = ({ let __ice_call = ::ui_lang_runtime::dev::Span::extern_call("open_link", "src/ui/app.ice:65"); crate::host::open_link(::std::convert::AsRef::as_ref(&(crate::host::navigation_link(next.interaction.clone())))) }); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[63] += 1; } }
if ((((comment_line < 0) || self.loading) || self.busy) || (self.active_page).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = crate::host::remember_draft(::std::convert::AsRef::as_ref(&(self.orphaned_comment_drafts)), ::std::convert::AsRef::as_ref(&(self.block_comment_draft))); if ::ui_lang_runtime::state_changed!(self.orphaned_comment_drafts, __ice_next) { self.orphaned_comment_drafts = __ice_next; self.__ice_rev[43] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.block_comment_draft, __ice_next) { self.block_comment_draft = __ice_next; self.__ice_rev[56] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.reply_thread, __ice_next) { self.reply_thread = __ice_next; self.__ice_rev[51] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.reply_draft, __ice_next) { self.reply_draft = __ice_next; self.__ice_rev[57] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.resolved_open, __ice_next) { self.resolved_open = __ice_next; self.__ice_rev[53] += 1; } }
{ let __ice_next = crate::host::block_at_line(::std::convert::AsRef::as_ref(&(self.blocks)), comment_line); if ::ui_lang_runtime::state_changed!(self.scope_target, __ice_next) { self.scope_target = __ice_next; self.__ice_rev[45] += 1; } }
{ let __ice_next = (!(self.scope_target).is_empty()); if ::ui_lang_runtime::state_changed!(self.scope_pinned, __ice_next) { self.scope_pinned = __ice_next; self.__ice_rev[46] += 1; } }
{ let __ice_next = self.pointer_y; if ::ui_lang_runtime::state_changed!(self.comment_anchor_y, __ice_next) { self.comment_anchor_y = __ice_next; self.__ice_rev[1] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.block_comments_open, __ice_next) { self.block_comments_open = __ice_next; self.__ice_rev[44] += 1; } }
{ let __ice_next = comment_line; if ::ui_lang_runtime::state_changed!(self.comment_anchor_line, __ice_next) { self.comment_anchor_line = __ice_next; self.__ice_rev[2] += 1; } }
let opened = crate::host::comments_reserve(self.pages_pane_width, true, comment_line, self.comments_card_height);
if ((opened.line == self.document_reserve.line) && (opened.height == self.document_reserve.height)) { return ::iced::Task::none(); }
{ let __ice_next = opened.clone(); if ::ui_lang_runtime::state_changed!(self.document_reserve, __ice_next) { self.document_reserve = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = crate::editor_view::document_presentation(::std::borrow::Borrow::borrow(&(self.document)), self.document_menu.clone(), self.document_dark, self.document_commented.clone(), self.document_marks.clone(), self.document_focused, self.document_reserve.clone()); if ::ui_lang_runtime::state_changed!(self.document_paint, __ice_next) { self.document_paint = __ice_next; self.__ice_rev[7] += 1; } }
::iced::Task::none()
})(),
__PagesViewMessage::__BindPageDraft(value) => { { let __ice_next = value; if ::ui_lang_runtime::state_changed!(self.page_draft, __ice_next) { self.page_draft = __ice_next; self.__ice_rev[54] += 1; } } ::iced::Task::none() }
__PagesViewMessage::__BindPageSearchDraft(value) => { { let __ice_next = value; if ::ui_lang_runtime::state_changed!(self.page_search_draft, __ice_next) { self.page_search_draft = __ice_next; self.__ice_rev[55] += 1; } } ::iced::Task::none() }
__PagesViewMessage::__BindReplyDraft(value) => { { let __ice_next = value; if ::ui_lang_runtime::state_changed!(self.reply_draft, __ice_next) { self.reply_draft = __ice_next; self.__ice_rev[57] += 1; } } ::iced::Task::none() }
__PagesViewMessage::__BindBlockCommentDraft(value) => { { let __ice_next = value; if ::ui_lang_runtime::state_changed!(self.block_comment_draft, __ice_next) { self.block_comment_draft = __ice_next; self.__ice_rev[56] += 1; } } ::iced::Task::none() }
__PagesViewMessage::__0T646f63756d656e74(__transaction) => { let __route = __transaction.apply(&mut self.document); self.__ice_rev[8] += 1; __route.map_or_else(::iced::Task::none, ::iced::Task::done) },
__PagesViewMessage::__EditDocument(__document) => { __document.apply(&mut self.document); self.__ice_rev[8] += 1; ::iced::Task::none() }
__PagesViewMessage::__ExternNoop => ::iced::Task::none(),
};
// Snapshotting the widget tree after every message serves ONLY an attached
// assistive technology (and the test harness, which drives the app through
// this tree) — ungated it walked every widget, built a TreeUpdate nobody
// read, and scheduled a second frame per message.
let __accessibility = if cfg!(test) || ::ui_lang_runtime::accessibility_active() {
::ui_lang_runtime::snapshot::<__PagesViewMessage>("PagesView").map(|__snapshot| __PagesViewMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)))
} else {
::iced::Task::none()
};
::iced::Task::batch([__task, __accessibility])
}

}
}
