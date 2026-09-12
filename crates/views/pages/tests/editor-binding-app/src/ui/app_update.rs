#[allow(warnings, clippy::all)]
mod __ice_group_app_update {
use super::*;
impl super::PagesEditorFixture {
#[allow(clippy::assign_op_pattern)]
pub(super) fn __update(&mut self, message: __PagesEditorFixtureMessage) -> ::ducktape_view_guest::Task<__PagesEditorFixtureMessage> {
let __task = match message {
__PagesEditorFixtureMessage::Load => (|| {

{ let __ice_next = crate::fixture::large_source(); if ::ducktape_view_guest::state_changed!(self.source, __ice_next) { self.source = __ice_next; self.__ice_rev[4] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesEditorFixtureMessage::DocumentArrived(item) => (|| {

let _ = &item;
if (item.source != self.source.reference) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = item.error.to_owned(); if ::ducktape_view_guest::state_changed!(self.load_error, __ice_next) { self.load_error = __ice_next; self.__ice_rev[6] += 1; } }
if (!(item.error).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = item.notice.to_owned(); if ::ducktape_view_guest::state_changed!(self.formatting_notice, __ice_next) { self.formatting_notice = __ice_next; self.__ice_rev[0] += 1; } }
{ let __reset = self.document.reset_revision(); let __next = crate::document_ingress::document_editor(item.text.to_owned(), item.cursor.clone()); self.document.replace(__next, __reset); }; self.__ice_rev[1] += 1;
{ let __ice_next = item.source.clone(); if ::ducktape_view_guest::state_changed!(self.installed_source, __ice_next) { self.installed_source = __ice_next; self.__ice_rev[5] += 1; } }
{ let __ice_next = crate::editor_binding::initial_menu(); if ::ducktape_view_guest::state_changed!(self.menu, __ice_next) { self.menu = __ice_next; self.__ice_rev[3] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesEditorFixtureMessage::Committed(next) => (|| {

let _ = &next;
{ let __ice_next = next.notice.to_owned(); if ::ducktape_view_guest::state_changed!(self.formatting_notice, __ice_next) { self.formatting_notice = __ice_next; self.__ice_rev[0] += 1; } }
{ let __ice_next = next.history.clone(); if ::ducktape_view_guest::state_changed!(self.history, __ice_next) { self.history = __ice_next; self.__ice_rev[2] += 1; } }
{ let __ice_next = next.menu.clone(); if ::ducktape_view_guest::state_changed!(self.menu, __ice_next) { self.menu = __ice_next; self.__ice_rev[3] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__PagesEditorFixtureMessage::__0T646f63756d656e74(__transaction) => { let __route = __transaction.apply(&mut self.document); self.__ice_rev[1] += 1; __route.map_or_else(::ducktape_view_guest::Task::none, ::ducktape_view_guest::Task::done) },
__PagesEditorFixtureMessage::__EditDocument(__document) => { __document.apply(&mut self.document); self.__ice_rev[1] += 1; ::ducktape_view_guest::Task::none() }
};
__task
}

}
}
