#[allow(warnings, clippy::all)]
mod __ice_group_app_update {
use super::*;
impl super::PagesEditorFixture {
#[allow(clippy::assign_op_pattern)]
pub(super) fn __update(&mut self, message: __PagesEditorFixtureMessage) -> ::iced::Task<__PagesEditorFixtureMessage> {
#[cfg(all(target_os = "windows", not(test)))]
if !self.__ice_accessibility.is_attached() && !matches!(&message, __PagesEditorFixtureMessage::__AccessibilityNativeWindow(_)) {
self.__ice_accessibility_pending.push(message);
return ::iced::Task::none();
}
let __task = match message {
__PagesEditorFixtureMessage::__AccessibilitySnapshot(__snapshot) => { self.__ice_accessibility.update(*__snapshot); return ::iced::Task::none(); },
__PagesEditorFixtureMessage::__AccessibilityAction(__request) => { let __refresh = matches!(__request.action, ::ui_lang_runtime::Action::Focus); let __task = self.__ice_accessibility.dispatch(__request); return if __refresh { __task.chain(::ui_lang_runtime::snapshot::<__PagesEditorFixtureMessage>("PagesEditorFixture").map(|__snapshot| __PagesEditorFixtureMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)))) } else { __task }; },
__PagesEditorFixtureMessage::__AccessibilityWindow(__id, __event) => { self.__ice_accessibility.window_event(__id, __event); return ::iced::Task::none(); },
#[cfg(all(target_os = "windows", not(test)))]
__PagesEditorFixtureMessage::__AccessibilityNativeWindow(__window) => {
let __id = __window.id();
if !self.__ice_accessibility.attach_window(__window) { return ::iced::Task::none(); }
let __restore = ::iced::window::set_mode(__id, ::iced::window::Mode::Windowed);
let __initial = self.__accessibility_initial_task();
let mut __pending = ::std::vec::Vec::new();
for __message in ::std::mem::take(&mut self.__ice_accessibility_pending) {
__pending.push(self.__update(__message));
}
let __pending = ::iced::Task::batch(__pending);
let __snapshot = ::ui_lang_runtime::snapshot::<__PagesEditorFixtureMessage>("PagesEditorFixture").map(|__snapshot| __PagesEditorFixtureMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)));
return __restore.chain(::iced::Task::batch([__initial, __pending, __snapshot]));
},
#[cfg(all(target_os = "macos", not(test)))]
__PagesEditorFixtureMessage::__AccessibilityNativeWindow(__window) => {
if !self.__ice_accessibility.attach_window(__window) { return ::iced::Task::none(); }
return ::ui_lang_runtime::snapshot::<__PagesEditorFixtureMessage>("PagesEditorFixture").map(|__snapshot| __PagesEditorFixtureMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)));
},
__PagesEditorFixtureMessage::__AccessibilityFocusNext(__window) => { return ::ui_lang_runtime::focus_next_in::<__PagesEditorFixtureMessage>(__window).chain(::ui_lang_runtime::snapshot::<__PagesEditorFixtureMessage>("PagesEditorFixture").map(|__snapshot| __PagesEditorFixtureMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)))); },
__PagesEditorFixtureMessage::__AccessibilityFocusPrevious(__window) => { return ::ui_lang_runtime::focus_previous_in::<__PagesEditorFixtureMessage>(__window).chain(::ui_lang_runtime::snapshot::<__PagesEditorFixtureMessage>("PagesEditorFixture").map(|__snapshot| __PagesEditorFixtureMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)))); },
__PagesEditorFixtureMessage::__TemplateChanged => { return ::iced::Task::none(); },
__PagesEditorFixtureMessage::Load => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("load", "src/ui/app.ice:51");
{ let __ice_next = crate::fixture::large_source(); if ::ui_lang_runtime::state_changed!(self.source, __ice_next) { self.source = __ice_next; self.__ice_rev[4] += 1; } }
::iced::Task::none()
})(),
__PagesEditorFixtureMessage::DocumentArrived(item) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("document_arrived", "src/ui/app.ice:54");
let _ = &item;
if (item.source != self.source.reference) { return ::iced::Task::none(); }
{ let __ice_next = item.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.load_error, __ice_next) { self.load_error = __ice_next; self.__ice_rev[6] += 1; } }
if (!(item.error).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = item.notice.to_owned(); if ::ui_lang_runtime::state_changed!(self.formatting_notice, __ice_next) { self.formatting_notice = __ice_next; self.__ice_rev[0] += 1; } }
{ let __reset = self.document.reset_revision(); let __next = crate::document_ingress::document_editor(item.text.to_owned(), item.cursor.clone()); self.document.replace(__next, __reset); }; self.__ice_rev[1] += 1;
{ let __ice_next = item.source.clone(); if ::ui_lang_runtime::state_changed!(self.installed_source, __ice_next) { self.installed_source = __ice_next; self.__ice_rev[5] += 1; } }
{ let __ice_next = crate::editor_binding::initial_menu(); if ::ui_lang_runtime::state_changed!(self.menu, __ice_next) { self.menu = __ice_next; self.__ice_rev[3] += 1; } }
::iced::Task::none()
})(),
__PagesEditorFixtureMessage::Committed(next) => (|| {
let __ice_turn = ::ui_lang_runtime::dev::Span::handler("committed", "src/ui/app.ice:63");
let _ = &next;
{ let __ice_next = next.notice.to_owned(); if ::ui_lang_runtime::state_changed!(self.formatting_notice, __ice_next) { self.formatting_notice = __ice_next; self.__ice_rev[0] += 1; } }
{ let __ice_next = next.history.clone(); if ::ui_lang_runtime::state_changed!(self.history, __ice_next) { self.history = __ice_next; self.__ice_rev[2] += 1; } }
{ let __ice_next = next.menu.clone(); if ::ui_lang_runtime::state_changed!(self.menu, __ice_next) { self.menu = __ice_next; self.__ice_rev[3] += 1; } }
::iced::Task::none()
})(),
__PagesEditorFixtureMessage::__0T646f63756d656e74(__transaction) => { let __route = __transaction.apply(&mut self.document); self.__ice_rev[1] += 1; __route.map_or_else(::iced::Task::none, ::iced::Task::done) },
__PagesEditorFixtureMessage::__EditDocument(__document) => { __document.apply(&mut self.document); self.__ice_rev[1] += 1; ::iced::Task::none() }
};
// Snapshotting the widget tree after every message serves ONLY an attached
// assistive technology (and the test harness, which drives the app through
// this tree) — ungated it walked every widget, built a TreeUpdate nobody
// read, and scheduled a second frame per message.
let __accessibility = if cfg!(test) || ::ui_lang_runtime::accessibility_active() {
::ui_lang_runtime::snapshot::<__PagesEditorFixtureMessage>("PagesEditorFixture").map(|__snapshot| __PagesEditorFixtureMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)))
} else {
::iced::Task::none()
};
::iced::Task::batch([__task, __accessibility])
}

}
}
