#[allow(warnings, clippy::all)]
mod __ice_group_app_update {
use super::*;
impl super::Ducktape {
#[allow(clippy::assign_op_pattern)]
pub(super) fn __update(&mut self, message: __DucktapeMessage) -> ::ducktape_view_guest::Task<__DucktapeMessage> {
let __task = match message {
__DucktapeMessage::__AccessibilitySnapshot(__snapshot) => { self.__ice_accessibility.update(*__snapshot); return ::ducktape_view_guest::Task::none(); },
__DucktapeMessage::__AccessibilityAction(__request) => { let __refresh = matches!(__request.action, ::ui_lang_runtime::Action::Focus); let __task = self.__ice_accessibility.dispatch(__request); return if __refresh { __task.chain(::ui_lang_runtime::snapshot::<__DucktapeMessage>("Ducktape").map(|__snapshot| __DucktapeMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)))) } else { __task }; },
__DucktapeMessage::__AccessibilityWindow(__id, __event) => { self.__ice_accessibility.window_event(__id, __event); return ::ducktape_view_guest::Task::none(); },
#[cfg(all(any(target_os = "windows", target_os = "macos"), not(test)))]
__DucktapeMessage::__AccessibilityNativeWindow(_) => { return ::ducktape_view_guest::Task::none(); },
#[cfg(all(target_os = "macos", not(test)))]
__DucktapeMessage::__AccessibilityWindowEvent(__id, __event) => {
let __opened = matches!(__event, ::iced::window::Event::Opened { .. });
self.__ice_accessibility.window_event(__id, __event.clone());
self.__ice_accessibility_windows.window_event(__id, __event);
if __opened {
return ::ui_lang_runtime::native_window(__id).map(__DucktapeMessage::__AccessibilityWindowNative);
}
return ::ducktape_view_guest::Task::none();
},
#[cfg(all(target_os = "macos", not(test)))]
__DucktapeMessage::__AccessibilityWindowNative(__window) => {
let __id = __window.id();
if !self.__ice_accessibility_windows.attach(__window) { return ::ducktape_view_guest::Task::none(); }
return ::ui_lang_runtime::snapshot_in::<__DucktapeMessage>("Ducktape", __id).map(move |__snapshot| __DucktapeMessage::__AccessibilityWindowSnapshot(__id, ::std::boxed::Box::new(__snapshot)));
},
#[cfg(all(target_os = "macos", not(test)))]
__DucktapeMessage::__AccessibilityWindowSnapshot(__id, __snapshot) => { self.__ice_accessibility_windows.update(__id, *__snapshot); return ::ducktape_view_guest::Task::none(); },
#[cfg(all(target_os = "macos", not(test)))]
__DucktapeMessage::__AccessibilityWindowAction(__id, __request) => {
let __task = self.__ice_accessibility_windows.dispatch(__id, __request);
return __task.chain(::ui_lang_runtime::snapshot_in::<__DucktapeMessage>("Ducktape", __id).map(move |__snapshot| __DucktapeMessage::__AccessibilityWindowSnapshot(__id, ::std::boxed::Box::new(__snapshot))));
},
__DucktapeMessage::__AccessibilityFocusNext(__window) => { return ::ui_lang_runtime::focus_next_in::<__DucktapeMessage>(__window).chain(::ui_lang_runtime::snapshot::<__DucktapeMessage>("Ducktape").map(|__snapshot| __DucktapeMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)))); },
__DucktapeMessage::__AccessibilityFocusPrevious(__window) => { return ::ui_lang_runtime::focus_previous_in::<__DucktapeMessage>(__window).chain(::ui_lang_runtime::snapshot::<__DucktapeMessage>("Ducktape").map(|__snapshot| __DucktapeMessage::__AccessibilitySnapshot(::std::boxed::Box::new(__snapshot)))); },
__DucktapeMessage::__TemplateChanged => { return ::ducktape_view_guest::Task::none(); },
__DucktapeMessage::__RequestLane0(__generation, __message) => { if self.__ice_run_lane_0_generation == __generation { self.__ice_run_lane_0_handle = ::std::option::Option::None; return self.__update(*__message); } return ::ducktape_view_guest::Task::none(); },
__DucktapeMessage::__RequestLane1(__generation, __message) => { if self.__ice_run_lane_1_generation == __generation { self.__ice_run_lane_1_handle = ::std::option::Option::None; return self.__update(*__message); } return ::ducktape_view_guest::Task::none(); },
__DucktapeMessage::__RequestLane2(__generation, __message) => { if self.__ice_run_lane_2_generation == __generation { self.__ice_run_lane_2_handle = ::std::option::Option::None; return self.__update(*__message); } return ::ducktape_view_guest::Task::none(); },
__DucktapeMessage::__RequestLane3(__generation, __message) => { if self.__ice_run_lane_3_generation == __generation { self.__ice_run_lane_3_handle = ::std::option::Option::None; return self.__update(*__message); } return ::ducktape_view_guest::Task::none(); },
__DucktapeMessage::__RequestLane4(__generation, __message) => { if self.__ice_run_lane_4_generation == __generation { self.__ice_run_lane_4_handle = ::std::option::Option::None; return self.__update(*__message); } return ::ducktape_view_guest::Task::none(); },
__DucktapeMessage::__RequestLane5(__generation, __message) => { if self.__ice_run_lane_5_generation == __generation { self.__ice_run_lane_5_handle = ::std::option::Option::None; return self.__update(*__message); } return ::ducktape_view_guest::Task::none(); },
__DucktapeMessage::__RequestLane6(__generation, __message) => { if self.__ice_run_lane_6_generation == __generation { self.__ice_run_lane_6_handle = ::std::option::Option::None; return self.__update(*__message); } return ::ducktape_view_guest::Task::none(); },
__DucktapeMessage::__RequestLane7(__generation, __message) => { if self.__ice_run_lane_7_generation == __generation { self.__ice_run_lane_7_handle = ::std::option::Option::None; return self.__update(*__message); } return ::ducktape_view_guest::Task::none(); },
__DucktapeMessage::__RequestLane8(__generation, __message) => { if self.__ice_run_lane_8_generation == __generation { self.__ice_run_lane_8_handle = ::std::option::Option::None; return self.__update(*__message); } return ::ducktape_view_guest::Task::none(); },
__DucktapeMessage::__RequestLane9(__generation, __message) => { if self.__ice_run_lane_9_generation == __generation { self.__ice_run_lane_9_handle = ::std::option::Option::None; return self.__update(*__message); } return ::ducktape_view_guest::Task::none(); },
__DucktapeMessage::__RequestLane10(__generation, __message) => { if self.__ice_run_lane_10_generation == __generation { if let ::std::option::Option::Some(__message) = __message { return self.__update(*__message); } self.__ice_run_lane_10_handle = ::std::option::Option::None; } return ::ducktape_view_guest::Task::none(); },
__DucktapeMessage::__RequestLane11(__generation, __message) => { if self.__ice_run_lane_11_generation == __generation { self.__ice_run_lane_11_handle = ::std::option::Option::None; return self.__update(*__message); } return ::ducktape_view_guest::Task::none(); },
__DucktapeMessage::__RequestLane12(__generation, __message) => { if self.__ice_run_lane_12_generation == __generation { self.__ice_run_lane_12_handle = ::std::option::Option::None; return self.__update(*__message); } return ::ducktape_view_guest::Task::none(); },
__DucktapeMessage::__RequestLane13(__generation, __message) => { if self.__ice_run_lane_13_generation == __generation { self.__ice_run_lane_13_handle = ::std::option::Option::None; return self.__update(*__message); } return ::ducktape_view_guest::Task::none(); },
__DucktapeMessage::__RequestLane14(__generation, __message) => { if self.__ice_run_lane_14_generation == __generation { self.__ice_run_lane_14_handle = ::std::option::Option::None; return self.__update(*__message); } return ::ducktape_view_guest::Task::none(); },
__DucktapeMessage::__RequestLane15(__generation, __message) => { if self.__ice_run_lane_15_generation == __generation { self.__ice_run_lane_15_handle = ::std::option::Option::None; return self.__update(*__message); } return ::ducktape_view_guest::Task::none(); },
__DucktapeMessage::__RequestLane16(__generation, __message) => { if self.__ice_run_lane_16_generation == __generation { self.__ice_run_lane_16_handle = ::std::option::Option::None; return self.__update(*__message); } return ::ducktape_view_guest::Task::none(); },
__DucktapeMessage::__RequestLane17(__generation, __message) => { if self.__ice_run_lane_17_generation == __generation { self.__ice_run_lane_17_handle = ::std::option::Option::None; return self.__update(*__message); } return ::ducktape_view_guest::Task::none(); },
__DucktapeMessage::__RequestLane18(__generation, __message) => { if self.__ice_run_lane_18_generation == __generation { self.__ice_run_lane_18_handle = ::std::option::Option::None; return self.__update(*__message); } return ::ducktape_view_guest::Task::none(); },
__DucktapeMessage::__RequestLane19(__generation, __message) => { if self.__ice_run_lane_19_generation == __generation { self.__ice_run_lane_19_handle = ::std::option::Option::None; return self.__update(*__message); } return ::ducktape_view_guest::Task::none(); },
__DucktapeMessage::__RequestLane20(__generation, __message) => { if self.__ice_run_lane_20_generation == __generation { if let ::std::option::Option::Some(__message) = __message { return self.__update(*__message); } self.__ice_run_lane_20_handle = ::std::option::Option::None; } return ::ducktape_view_guest::Task::none(); },
__DucktapeMessage::__RequestLane21(__generation, __message) => { if self.__ice_run_lane_21_generation == __generation { self.__ice_run_lane_21_handle = ::std::option::Option::None; return self.__update(*__message); } return ::ducktape_view_guest::Task::none(); },
__DucktapeMessage::__RequestLane22(__generation, __message) => { if self.__ice_run_lane_22_generation == __generation { self.__ice_run_lane_22_handle = ::std::option::Option::None; return self.__update(*__message); } return ::ducktape_view_guest::Task::none(); },
__DucktapeMessage::__RequestLane23(__generation, __message) => { if self.__ice_run_lane_23_generation == __generation { self.__ice_run_lane_23_handle = ::std::option::Option::None; return self.__update(*__message); } return ::ducktape_view_guest::Task::none(); },
__DucktapeMessage::__RequestLane24(__generation, __message) => { if self.__ice_run_lane_24_generation == __generation { if let ::std::option::Option::Some(__message) = __message { return self.__update(*__message); } self.__ice_run_lane_24_handle = ::std::option::Option::None; } return ::ducktape_view_guest::Task::none(); },
__DucktapeMessage::__RequestLane25(__generation, __message) => { if self.__ice_run_lane_25_generation == __generation { self.__ice_run_lane_25_handle = ::std::option::Option::None; return self.__update(*__message); } return ::ducktape_view_guest::Task::none(); },
__DucktapeMessage::__RequestLane26(__generation, __message) => { if self.__ice_run_lane_26_generation == __generation { if let ::std::option::Option::Some(__message) = __message { return self.__update(*__message); } self.__ice_run_lane_26_handle = ::std::option::Option::None; } return ::ducktape_view_guest::Task::none(); },
__DucktapeMessage::AppearanceLoaded(mode) => (|| {
let _ = &mode;
{ let __ice_next = mode.clone(); if ::ui_lang_runtime::state_changed!(self.appearance, __ice_next) { self.appearance = __ice_next; self.__ice_rev[1] += 1; self.__ice_derived.dark.take(); self.__ice_derived.app_background.take(); self.__ice_derived.app_text.take(); } }
{ let __ice_next = AppTheme::App; if ::ui_lang_runtime::state_changed!(self.app_palette, __ice_next) { self.app_palette = __ice_next; self.__ice_rev[0] += 1; } }
if (self.appearance != Appearance::Dark) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = AppTheme::AppDark; if ::ui_lang_runtime::state_changed!(self.app_palette, __ice_next) { self.app_palette = __ice_next; self.__ice_rev[0] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::SetAppearanceLight => (|| {
{ let __ice_next = Appearance::Light; if ::ui_lang_runtime::state_changed!(self.appearance, __ice_next) { self.appearance = __ice_next; self.__ice_rev[1] += 1; self.__ice_derived.dark.take(); self.__ice_derived.app_background.take(); self.__ice_derived.app_text.take(); } }
{ let __ice_next = AppTheme::App; if ::ui_lang_runtime::state_changed!(self.app_palette, __ice_next) { self.app_palette = __ice_next; self.__ice_rev[0] += 1; } }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::save_appearance(self.appearance.clone()) }), |value| __DucktapeMessage::AppearanceSaved(value)); self.__ice_run_lane_0_generation = self.__ice_run_lane_0_generation.wrapping_add(1); let __generation = self.__ice_run_lane_0_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_0_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane0(__generation, ::std::boxed::Box::new(__message))) };
})(),
__DucktapeMessage::SetAppearanceDark => (|| {
{ let __ice_next = Appearance::Dark; if ::ui_lang_runtime::state_changed!(self.appearance, __ice_next) { self.appearance = __ice_next; self.__ice_rev[1] += 1; self.__ice_derived.dark.take(); self.__ice_derived.app_background.take(); self.__ice_derived.app_text.take(); } }
{ let __ice_next = AppTheme::AppDark; if ::ui_lang_runtime::state_changed!(self.app_palette, __ice_next) { self.app_palette = __ice_next; self.__ice_rev[0] += 1; } }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::save_appearance(self.appearance.clone()) }), |value| __DucktapeMessage::AppearanceSaved(value)); self.__ice_run_lane_0_generation = self.__ice_run_lane_0_generation.wrapping_add(1); let __generation = self.__ice_run_lane_0_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_0_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane0(__generation, ::std::boxed::Box::new(__message))) };
})(),
__DucktapeMessage::AppearanceSaved(_written) => (|| {
let _ = &_written;
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::DesktopNotificationsLoaded(enabled) => (|| {
let _ = &enabled;
{ let __ice_next = enabled; if ::ui_lang_runtime::state_changed!(self.desktop_notifications, __ice_next) { self.desktop_notifications = __ice_next; self.__ice_rev[2] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::DesktopNotificationsSaved(_written) => (|| {
let _ = &_written;
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::Reconnect => (|| {
if (self.loading || ((self.mutation_phase != MutationPhase::Idle) && (self.mutation_phase != MutationPhase::Recovering))) { return ::ducktape_view_guest::Task::none(); }
self.__ice_run_lane_10_generation = self.__ice_run_lane_10_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_10_handle.take() { __previous.abort(); }
self.__ice_run_lane_11_generation = self.__ice_run_lane_11_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_11_handle.take() { __previous.abort(); }
{ let __ice_next = (self.account_busy && (self.account_ceremony_phase).is_empty()); if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_phase, __ice_next) { self.account_ceremony_phase = __ice_next; self.__ice_rev[80] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_qr, __ice_next) { self.account_ceremony_qr = __ice_next; self.__ice_rev[81] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_detail, __ice_next) { self.account_ceremony_detail = __ice_next; self.__ice_rev[82] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_left, __ice_next) { self.account_ceremony_left = __ice_next; self.__ice_rev[83] += 1; } }
self.__ice_run_lane_15_generation = self.__ice_run_lane_15_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_15_handle.take() { __previous.abort(); }
self.__ice_run_lane_16_generation = self.__ice_run_lane_16_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_16_handle.take() { __previous.abort(); }
self.__ice_run_lane_8_generation = self.__ice_run_lane_8_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_8_handle.take() { __previous.abort(); }
{ let __ice_next = (self.hydration_generation + 1); if ::ui_lang_runtime::state_changed!(self.hydration_generation, __ice_next) { self.hydration_generation = __ice_next; self.__ice_rev[15] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.hydration_retry_attempt, __ice_next) { self.hydration_retry_attempt = __ice_next; self.__ice_rev[18] += 1; } }
{ let __ice_next = MutationPhase::Idle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.loading, __ice_next) { self.loading = __ice_next; self.__ice_rev[9] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.connected, __ice_next) { self.connected = __ice_next; self.__ice_rev[8] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.channels, __ice_next) { self.channels = __ice_next; self.__ice_rev[22] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.rooms, __ice_next) { self.rooms = __ice_next; self.__ice_rev[23] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.dm_rows, __ice_next) { self.dm_rows = __ice_next; self.__ice_rev[51] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.chat_at_tail, __ice_next) { self.chat_at_tail = __ice_next; self.__ice_rev[47] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.chat_land_seq, __ice_next) { self.chat_land_seq = __ice_next; self.__ice_rev[33] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.chat_pending_sends, __ice_next) { self.chat_pending_sends = __ice_next; self.__ice_rev[41] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.chat_edit_seq, __ice_next) { self.chat_edit_seq = __ice_next; self.__ice_rev[34] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.chat_edit_rev, __ice_next) { self.chat_edit_rev = __ice_next; self.__ice_rev[35] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.channel_reads, __ice_next) { self.channel_reads = __ice_next; self.__ice_rev[25] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.unread_boundary, __ice_next) { self.unread_boundary = __ice_next; self.__ice_rev[26] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.active_channel, __ice_next) { self.active_channel = __ice_next; self.__ice_rev[27] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.active_dm_peer, __ice_next) { self.active_dm_peer = __ice_next; self.__ice_rev[53] += 1; } }
{ let __ice_next = crate::backend::no_dm_peer(); if ::ui_lang_runtime::state_changed!(self.active_dm, __ice_next) { self.active_dm = __ice_next; self.__ice_rev[54] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.history_view, __ice_next) { self.history_view = __ice_next; self.__ice_rev[48] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.active_channel_name, __ice_next) { self.active_channel_name = __ice_next; self.__ice_rev[28] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.active_channel_archived, __ice_next) { self.active_channel_archived = __ice_next; self.__ice_rev[29] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.active_channel_members_only, __ice_next) { self.active_channel_members_only = __ice_next; self.__ice_rev[30] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.channel_members, __ice_next) { self.channel_members = __ice_next; self.__ice_rev[31] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.post_refusal, __ice_next) { self.post_refusal = __ice_next; self.__ice_rev[32] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.pending_channel, __ice_next) { self.pending_channel = __ice_next; self.__ice_rev[46] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.page_route, __ice_next) { self.page_route = __ice_next; self.__ice_rev[119] += 1; } }
{ let __ice_next = SearchPhase::Idle; if ::ui_lang_runtime::state_changed!(self.palette_search_phase, __ice_next) { self.palette_search_phase = __ice_next; self.__ice_rev[114] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
{ let __ice_next = "Connecting…".to_owned(); if ::ui_lang_runtime::state_changed!(self.status, __ice_next) { self.status = __ice_next; self.__ice_rev[7] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.bell_marking, __ice_next) { self.bell_marking = __ice_next; self.__ice_rev[111] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.bell_error, __ice_next) { self.bell_error = __ice_next; self.__ice_rev[112] += 1; } }
self.__ice_run_lane_13_generation = self.__ice_run_lane_13_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_13_handle.take() { __previous.abort(); }
self.__ice_run_lane_14_generation = self.__ice_run_lane_14_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_14_handle.take() { __previous.abort(); }
self.__ice_run_lane_4_generation = self.__ice_run_lane_4_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_4_handle.take() { __previous.abort(); }
self.__ice_run_lane_9_generation = self.__ice_run_lane_9_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_9_handle.take() { __previous.abort(); }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.bell_items, __ice_next) { self.bell_items = __ice_next; self.__ice_rev[107] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.bell_presentations, __ice_next) { self.bell_presentations = __ice_next; self.__ice_rev[108] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.bell_unread, __ice_next) { self.bell_unread = __ice_next; self.__ice_rev[106] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.bell_read_through, __ice_next) { self.bell_read_through = __ice_next; self.__ice_rev[109] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.bell_clear_through, __ice_next) { self.bell_clear_through = __ice_next; self.__ice_rev[110] += 1; } }
{ let __ice_next = (self.connect_generation + 1); if ::ui_lang_runtime::state_changed!(self.connect_generation, __ice_next) { self.connect_generation = __ice_next; self.__ice_rev[16] += 1; } }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::connect(self.connected_rpc.to_owned(), self.hydration_retry_attempt, self.connect_generation) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::WorkspaceConnected(value), ::std::result::Result::Err(error) => __DucktapeMessage::ConnectFailed(error) }); self.__ice_run_lane_1_generation = self.__ice_run_lane_1_generation.wrapping_add(1); let __generation = self.__ice_run_lane_1_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_1_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane1(__generation, ::std::boxed::Box::new(__message))) };
})(),
__DucktapeMessage::WorkspaceConnected(next) => (|| {
let _ = &next;
if (next.generation != self.connect_generation) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = next.rpc.to_owned(); if ::ui_lang_runtime::state_changed!(self.rpc, __ice_next) { self.rpc = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = next.rpc.to_owned(); if ::ui_lang_runtime::state_changed!(self.connected_rpc, __ice_next) { self.connected_rpc = __ice_next; self.__ice_rev[5] += 1; } }
{ let __ice_next = crate::backend::network_label(self.network_chain_id.to_owned(), self.connected_rpc.to_owned()); if ::ui_lang_runtime::state_changed!(self.network_name, __ice_next) { self.network_name = __ice_next; self.__ice_rev[124] += 1; } }
{ let __ice_next = next.status.to_owned(); if ::ui_lang_runtime::state_changed!(self.status, __ice_next) { self.status = __ice_next; self.__ice_rev[7] += 1; } }
{ let __ice_next = next.height; if ::ui_lang_runtime::state_changed!(self.block_height, __ice_next) { self.block_height = __ice_next; self.__ice_rev[14] += 1; } }
{ let __ice_next = next.channels.clone(); if ::ui_lang_runtime::state_changed!(self.channels, __ice_next) { self.channels = __ice_next; self.__ice_rev[22] += 1; } }
{ let __ice_next = self.network_chain_id.to_owned(); if ::ui_lang_runtime::state_changed!(self.chat_chain_id, __ice_next) { self.chat_chain_id = __ice_next; self.__ice_rev[49] += 1; } }
self.channel_reads = crate::backend::initial_channel_reads(next.channels.clone(), ::std::mem::take(&mut self.channel_reads)); self.__ice_rev[25] += 1;
{ let __ice_next = crate::backend::chat_sidebar_rooms(self.channels.clone(), self.dm_peers.clone(), self.channel_reads.clone()); if ::ui_lang_runtime::state_changed!(self.rooms, __ice_next) { self.rooms = __ice_next; self.__ice_rev[23] += 1; } }
{ let __ice_next = crate::backend::chat_sidebar_dms(self.channels.clone(), self.dm_peers.clone(), self.channel_reads.clone()); if ::ui_lang_runtime::state_changed!(self.dm_rows, __ice_next) { self.dm_rows = __ice_next; self.__ice_rev[51] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.unread_boundary, __ice_next) { self.unread_boundary = __ice_next; self.__ice_rev[26] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.history_view, __ice_next) { self.history_view = __ice_next; self.__ice_rev[48] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.chat_at_tail, __ice_next) { self.chat_at_tail = __ice_next; self.__ice_rev[47] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.chat_land_seq, __ice_next) { self.chat_land_seq = __ice_next; self.__ice_rev[33] += 1; } }
{ let __ice_next = next.active_channel.to_owned(); if ::ui_lang_runtime::state_changed!(self.active_channel, __ice_next) { self.active_channel = __ice_next; self.__ice_rev[27] += 1; } }
{ let __ice_next = crate::backend::dm_peer_of_channel(self.active_dm_peer.to_owned(), self.dm_peers.clone(), self.active_channel.to_owned()); if ::ui_lang_runtime::state_changed!(self.active_dm_peer, __ice_next) { self.active_dm_peer = __ice_next; self.__ice_rev[53] += 1; } }
{ let __ice_next = crate::backend::dm_peer_named(self.dm_peers.clone(), self.active_dm_peer.to_owned()); if ::ui_lang_runtime::state_changed!(self.active_dm, __ice_next) { self.active_dm = __ice_next; self.__ice_rev[54] += 1; } }
{ let __ice_next = next.active_channel_name.to_owned(); if ::ui_lang_runtime::state_changed!(self.active_channel_name, __ice_next) { self.active_channel_name = __ice_next; self.__ice_rev[28] += 1; } }
{ let __ice_next = next.active_channel_archived; if ::ui_lang_runtime::state_changed!(self.active_channel_archived, __ice_next) { self.active_channel_archived = __ice_next; self.__ice_rev[29] += 1; } }
{ let __ice_next = next.active_channel_members_only; if ::ui_lang_runtime::state_changed!(self.active_channel_members_only, __ice_next) { self.active_channel_members_only = __ice_next; self.__ice_rev[30] += 1; } }
{ let __ice_next = crate::backend::keep_i64(self.huddle_joined, self.huddle_joined_at, self.huddle_now); if ::ui_lang_runtime::state_changed!(self.huddle_joined_at, __ice_next) { self.huddle_joined_at = __ice_next; self.__ice_rev[145] += 1; } }
let huddle = crate::backend::huddle_after_load(true, self.huddle_joined, self.huddle_channel.to_owned(), self.huddle_channel_name.to_owned(), self.huddle_roster.clone(), self.active_channel.to_owned(), self.active_channel_name.to_owned(), next.huddle_roster.clone());
{ let __ice_next = huddle.joined; if ::ui_lang_runtime::state_changed!(self.huddle_joined, __ice_next) { self.huddle_joined = __ice_next; self.__ice_rev[142] += 1; } }
{ let __ice_next = huddle.roster.clone(); if ::ui_lang_runtime::state_changed!(self.huddle_roster, __ice_next) { self.huddle_roster = __ice_next; self.__ice_rev[154] += 1; } }
{ let __ice_next = crate::call::huddle_tile_rows(self.huddle_roster.clone(), self.call_peers.clone(), self.call_muted); if ::ui_lang_runtime::state_changed!(self.huddle_rows, __ice_next) { self.huddle_rows = __ice_next; self.__ice_rev[155] += 1; } }
{ let __ice_next = huddle.channel.to_owned(); if ::ui_lang_runtime::state_changed!(self.huddle_channel, __ice_next) { self.huddle_channel = __ice_next; self.__ice_rev[143] += 1; } }
{ let __ice_next = huddle.channel_name.to_owned(); if ::ui_lang_runtime::state_changed!(self.huddle_channel_name, __ice_next) { self.huddle_channel_name = __ice_next; self.__ice_rev[144] += 1; } }
{ let __ice_next = next.channel_members.clone(); if ::ui_lang_runtime::state_changed!(self.channel_members, __ice_next) { self.channel_members = __ice_next; self.__ice_rev[31] += 1; } }
{ let __ice_next = ({ crate::module_view::chat_composer_roster(::std::convert::AsRef::as_ref(&(crate::backend::composer_scope(::std::convert::AsRef::as_ref(&(self.connected_rpc)), ::std::convert::AsRef::as_ref(&(self.active_channel))))), ::std::convert::AsRef::as_ref(&(self.channel_members))) }); if ::ui_lang_runtime::state_changed!(self.composer_roster_set, __ice_next) { self.composer_roster_set = __ice_next; self.__ice_rev[37] += 1; } }
{ let __ice_next = crate::backend::post_gate(self.active_channel_archived, self.active_channel_members_only, self.channel_members.clone(), self.settings_user_key.to_owned()); if ::ui_lang_runtime::state_changed!(self.post_refusal, __ice_next) { self.post_refusal = __ice_next; self.__ice_rev[32] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.connected, __ice_next) { self.connected = __ice_next; self.__ice_rev[8] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.loading, __ice_next) { self.loading = __ice_next; self.__ice_rev[9] += 1; } }
{ let __ice_next = MutationPhase::Idle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.hydration_retry_attempt, __ice_next) { self.hydration_retry_attempt = __ice_next; self.__ice_rev[18] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
{ let __ice_next = (self.members_generation + 1); if ::ui_lang_runtime::state_changed!(self.members_generation, __ice_next) { self.members_generation = __ice_next; self.__ice_rev[58] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.agents_open_run, __ice_next) { self.agents_open_run = __ice_next; self.__ice_rev[60] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.agents_live, __ice_next) { self.agents_live = __ice_next; self.__ice_rev[62] += 1; } }
{ let __ice_next = (self.account_generation + 1); if ::ui_lang_runtime::state_changed!(self.account_generation, __ice_next) { self.account_generation = __ice_next; self.__ice_rev[76] += 1; } }
{ let __ice_next = (self.settings_generation + 1); if ::ui_lang_runtime::state_changed!(self.settings_generation, __ice_next) { self.settings_generation = __ice_next; self.__ice_rev[71] += 1; } }
{ let __ice_next = (self.dm_peers_generation + 1); if ::ui_lang_runtime::state_changed!(self.dm_peers_generation, __ice_next) { self.dm_peers_generation = __ice_next; self.__ice_rev[52] += 1; } }
return ::ducktape_view_guest::Task::batch([
{ // __ICE_SOURCE 178 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6c6966656379636c652e696365
{ let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::load_dm_peers(self.connected_rpc.to_owned(), self.dm_peers_generation) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::DmPeersLoaded(value), ::std::result::Result::Err(error) => __DucktapeMessage::DmPeersFailed(error) }); self.__ice_run_lane_2_generation = self.__ice_run_lane_2_generation.wrapping_add(1); let __generation = self.__ice_run_lane_2_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_2_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane2(__generation, ::std::boxed::Box::new(__message))) }
},
{ // __ICE_SOURCE 179 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6c6966656379636c652e696365
{ let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::load_node_facts(self.connected_rpc.to_owned()) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::NodeFactsLoaded(value), ::std::result::Result::Err(error) => __DucktapeMessage::NodeFactsFailed(error) }); self.__ice_run_lane_3_generation = self.__ice_run_lane_3_generation.wrapping_add(1); let __generation = self.__ice_run_lane_3_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_3_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane3(__generation, ::std::boxed::Box::new(__message))) }
},
{ // __ICE_SOURCE 180 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6c6966656379636c652e696365
{ let __task = { let __ice_run_route_9_0 = self.connect_generation;
let __ice_run_route_9_1 = self.account_number.to_owned();
let __ice_run_route_10_0 = self.connect_generation;
let __ice_run_route_10_1 = self.account_number.to_owned();
::ducktape_view_guest::Task::perform(({ crate::backend::load_bell(self.connected_rpc.to_owned(), self.account_number.to_owned()) }), move |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::BellLoaded(__ice_run_route_9_0, __ice_run_route_9_1, value), ::std::result::Result::Err(error) => __DucktapeMessage::BellFailed(__ice_run_route_10_0, __ice_run_route_10_1, error) }) }; self.__ice_run_lane_4_generation = self.__ice_run_lane_4_generation.wrapping_add(1); let __generation = self.__ice_run_lane_4_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_4_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane4(__generation, ::std::boxed::Box::new(__message))) }
},
{ // __ICE_SOURCE 181 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6c6966656379636c652e696365
{ let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::load_members(self.connected_rpc.to_owned(), self.members_generation) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::MembersLoaded(value), ::std::result::Result::Err(error) => __DucktapeMessage::MembersFailed(error) }); self.__ice_run_lane_5_generation = self.__ice_run_lane_5_generation.wrapping_add(1); let __generation = self.__ice_run_lane_5_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_5_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane5(__generation, ::std::boxed::Box::new(__message))) }
},
{ // __ICE_SOURCE 182 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6c6966656379636c652e696365
{ let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::load_settings_facts(self.connected_rpc.to_owned(), self.settings_generation) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::SettingsLoaded(value), ::std::result::Result::Err(error) => __DucktapeMessage::SettingsFailed(error) }); self.__ice_run_lane_6_generation = self.__ice_run_lane_6_generation.wrapping_add(1); let __generation = self.__ice_run_lane_6_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_6_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane6(__generation, ::std::boxed::Box::new(__message))) }
},
{ // __ICE_SOURCE 183 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6c6966656379636c652e696365
{ let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::load_account(self.connected_rpc.to_owned(), self.account_generation) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::AccountLoaded(value), ::std::result::Result::Err(error) => __DucktapeMessage::AccountFailed(error) }); self.__ice_run_lane_7_generation = self.__ice_run_lane_7_generation.wrapping_add(1); let __generation = self.__ice_run_lane_7_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_7_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane7(__generation, ::std::boxed::Box::new(__message))) }
},
{ // __ICE_SOURCE 186 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6c6966656379636c652e696365
::crate::shell::close::<__DucktapeMessage>(({ crate::backend::window_target_unless(self.huddle_joined, self.huddle_win.clone()) }))
},
{ // __ICE_SOURCE 190 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6c6966656379636c652e696365
(::ducktape_view_guest::Task::done(true)).map(|value| { let _ = &value; __DucktapeMessage::ConsoleEntryAnswered })
},
]);
})(),
__DucktapeMessage::ConsoleEntryAnswered => (|| {
return match self.console_entry.clone() {
ConsoleEntry::Entering => (|| {
{ let __ice_next = ConsoleEntry::Idle; if ::ui_lang_runtime::state_changed!(self.console_entry, __ice_next) { self.console_entry = __ice_next; self.__ice_rev[132] += 1; self.__ice_derived.hub_busy.take(); } }
return { let (_, __task) = ::crate::shell::open(Self::__window_1()); __task.map(move |value| __DucktapeMessage::ConsoleOpened(value)) };
})(),
ConsoleEntry::Idle => (|| {
{ let __ice_next = ConsoleEntry::Idle; if ::ui_lang_runtime::state_changed!(self.console_entry, __ice_next) { self.console_entry = __ice_next; self.__ice_rev[132] += 1; self.__ice_derived.hub_busy.take(); } }
::ducktape_view_guest::Task::none()
})(),
};
})(),
__DucktapeMessage::LiveUpdated(next) => (|| {
let _ = &next;
{ let __ice_next = next.status.to_owned(); if ::ui_lang_runtime::state_changed!(self.status, __ice_next) { self.status = __ice_next; self.__ice_rev[7] += 1; } }
{ let __ice_next = crate::backend::keep_i64((next.height >= 0), next.height, self.block_height); if ::ui_lang_runtime::state_changed!(self.block_height, __ice_next) { self.block_height = __ice_next; self.__ice_rev[14] += 1; } }
{ let __ice_next = ({ crate::module_view::view_block_hit(self.block_height, self.views_live_serial) }); if ::ui_lang_runtime::state_changed!(self.views_live_serial, __ice_next) { self.views_live_serial = __ice_next; self.__ice_rev[10] += 1; } }
return match next.kind.clone() {
LiveKind::Retry => (|| {
if true { return ::ducktape_view_guest::Task::none(); }
::ducktape_view_guest::Task::none()
})(),
LiveKind::Tip => (|| {
if true { return ::ducktape_view_guest::Task::none(); }
::ducktape_view_guest::Task::none()
})(),
LiveKind::Ready => (|| {
{ let __ice_next = (self.hydration_generation + 1); if ::ui_lang_runtime::state_changed!(self.hydration_generation, __ice_next) { self.hydration_generation = __ice_next; self.__ice_rev[15] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.hydration_retry_attempt, __ice_next) { self.hydration_retry_attempt = __ice_next; self.__ice_rev[18] += 1; } }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::live_resync_load(self.connected_rpc.to_owned(), self.active_channel.to_owned(), next.load_chat, next.debounce, self.hydration_generation, 0) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::LiveResynced(value), ::std::result::Result::Err(error) => __DucktapeMessage::LiveResyncFailed(error) }); self.__ice_run_lane_8_generation = self.__ice_run_lane_8_generation.wrapping_add(1); let __generation = self.__ice_run_lane_8_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_8_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane8(__generation, ::std::boxed::Box::new(__message))) };
})(),
LiveKind::Chat => (|| {
{ let __ice_next = ({ crate::module_view::view_live_hit(::std::convert::AsRef::as_ref(&(next.module)), self.views_live_serial) }); if ::ui_lang_runtime::state_changed!(self.views_live_serial, __ice_next) { self.views_live_serial = __ice_next; self.__ice_rev[10] += 1; } }
let folded_chat = crate::backend::fold_live_chat(next.chat.clone(), self.channels.clone(), self.channel_members.clone(), self.channel_reads.clone(), self.dm_peers.clone(), self.settings_user_key.to_owned(), self.active_channel.to_owned(), self.history_view, (self.shell_tab == ShellTab::Chat), self.active_channel_name.to_owned(), self.active_channel_archived, self.active_channel_members_only);
{ let __ice_next = folded_chat.channels.clone(); if ::ui_lang_runtime::state_changed!(self.channels, __ice_next) { self.channels = __ice_next; self.__ice_rev[22] += 1; } }
{ let __ice_next = folded_chat.channel_members.clone(); if ::ui_lang_runtime::state_changed!(self.channel_members, __ice_next) { self.channel_members = __ice_next; self.__ice_rev[31] += 1; } }
{ let __ice_next = ({ crate::module_view::chat_composer_roster(::std::convert::AsRef::as_ref(&(crate::backend::composer_scope(::std::convert::AsRef::as_ref(&(self.connected_rpc)), ::std::convert::AsRef::as_ref(&(self.active_channel))))), ::std::convert::AsRef::as_ref(&(self.channel_members))) }); if ::ui_lang_runtime::state_changed!(self.composer_roster_set, __ice_next) { self.composer_roster_set = __ice_next; self.__ice_rev[37] += 1; } }
{ let __ice_next = folded_chat.channel_reads.clone(); if ::ui_lang_runtime::state_changed!(self.channel_reads, __ice_next) { self.channel_reads = __ice_next; self.__ice_rev[25] += 1; } }
{ let __ice_next = folded_chat.rooms.clone(); if ::ui_lang_runtime::state_changed!(self.rooms, __ice_next) { self.rooms = __ice_next; self.__ice_rev[23] += 1; } }
{ let __ice_next = folded_chat.dm_rows.clone(); if ::ui_lang_runtime::state_changed!(self.dm_rows, __ice_next) { self.dm_rows = __ice_next; self.__ice_rev[51] += 1; } }
{ let __ice_next = folded_chat.active_channel_name.to_owned(); if ::ui_lang_runtime::state_changed!(self.active_channel_name, __ice_next) { self.active_channel_name = __ice_next; self.__ice_rev[28] += 1; } }
{ let __ice_next = folded_chat.active_channel_archived; if ::ui_lang_runtime::state_changed!(self.active_channel_archived, __ice_next) { self.active_channel_archived = __ice_next; self.__ice_rev[29] += 1; } }
{ let __ice_next = folded_chat.active_channel_members_only; if ::ui_lang_runtime::state_changed!(self.active_channel_members_only, __ice_next) { self.active_channel_members_only = __ice_next; self.__ice_rev[30] += 1; } }
{ let __ice_next = folded_chat.post_refusal.to_owned(); if ::ui_lang_runtime::state_changed!(self.post_refusal, __ice_next) { self.post_refusal = __ice_next; self.__ice_rev[32] += 1; } }
if (!folded_chat.refresh_chat) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = (self.hydration_generation + 1); if ::ui_lang_runtime::state_changed!(self.hydration_generation, __ice_next) { self.hydration_generation = __ice_next; self.__ice_rev[15] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.hydration_retry_attempt, __ice_next) { self.hydration_retry_attempt = __ice_next; self.__ice_rev[18] += 1; } }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::live_resync_load(self.connected_rpc.to_owned(), self.active_channel.to_owned(), true, false, self.hydration_generation, 0) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::LiveResynced(value), ::std::result::Result::Err(error) => __DucktapeMessage::LiveResyncFailed(error) }); self.__ice_run_lane_8_generation = self.__ice_run_lane_8_generation.wrapping_add(1); let __generation = self.__ice_run_lane_8_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_8_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane8(__generation, ::std::boxed::Box::new(__message))) };
})(),
LiveKind::Bell => (|| {
{ let __ice_next = crate::backend::keep_i64(((next.bell.kind == "read") && (next.bell.up_to_seq > self.bell_read_through)), next.bell.up_to_seq, self.bell_read_through); if ::ui_lang_runtime::state_changed!(self.bell_read_through, __ice_next) { self.bell_read_through = __ice_next; self.__ice_rev[109] += 1; } }
{ let __ice_next = crate::backend::keep_i64(((next.bell.kind == "cleared") && (next.bell.up_to_seq > self.bell_clear_through)), next.bell.up_to_seq, self.bell_clear_through); if ::ui_lang_runtime::state_changed!(self.bell_clear_through, __ice_next) { self.bell_clear_through = __ice_next; self.__ice_rev[110] += 1; } }
self.bell_items = crate::backend::merge_bell_loaded(crate::backend::apply_bell(::std::mem::take(&mut self.bell_items), next.bell.clone()), ::std::vec::Vec::new(), self.bell_read_through, self.bell_clear_through); self.__ice_rev[107] += 1;
{ let __ice_next = crate::backend::bell_unread_count(::std::convert::AsRef::as_ref(&(self.bell_items)), ::std::convert::AsRef::as_ref(&(self.account_number)), ::std::convert::AsRef::as_ref(&(self.settings_user_key))); if ::ui_lang_runtime::state_changed!(self.bell_unread, __ice_next) { self.bell_unread = __ice_next; self.__ice_rev[106] += 1; } }
self.bell_presentations = crate::backend::merge_bell_presentations(crate::backend::bell_visible_items(::std::convert::AsRef::as_ref(&(self.bell_items)), ::std::convert::AsRef::as_ref(&(self.account_number)), ::std::convert::AsRef::as_ref(&(self.settings_user_key))), ::std::mem::take(&mut self.bell_presentations), ::std::vec::Vec::new()); self.__ice_rev[108] += 1;
if (next.bell.kind != "delivered") { return ::ducktape_view_guest::Task::none(); }
return { let __task = { let __ice_run_route_23_0 = self.connect_generation;
let __ice_run_route_23_1 = self.account_number.to_owned();
let __ice_run_route_24_0 = self.connect_generation;
let __ice_run_route_24_1 = self.account_number.to_owned();
::ducktape_view_guest::Task::perform(({ crate::backend::load_bell_presentations(self.connected_rpc.to_owned(), crate::backend::bell_missing_items(crate::backend::bell_visible_items(::std::convert::AsRef::as_ref(&(self.bell_items)), ::std::convert::AsRef::as_ref(&(self.account_number)), ::std::convert::AsRef::as_ref(&(self.settings_user_key))), ::std::convert::AsRef::as_ref(&(self.bell_presentations)))) }), move |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::BellContextLoaded(__ice_run_route_23_0, __ice_run_route_23_1, value), ::std::result::Result::Err(error) => __DucktapeMessage::BellFailed(__ice_run_route_24_0, __ice_run_route_24_1, error) }) }; self.__ice_run_lane_9_generation = self.__ice_run_lane_9_generation.wrapping_add(1); let __generation = self.__ice_run_lane_9_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_9_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane9(__generation, ::std::boxed::Box::new(__message))) };
})(),
LiveKind::Plane => (|| {
{ let __ice_next = ({ crate::module_view::view_live_hit(::std::convert::AsRef::as_ref(&(next.module)), self.views_live_serial) }); if ::ui_lang_runtime::state_changed!(self.views_live_serial, __ice_next) { self.views_live_serial = __ice_next; self.__ice_rev[10] += 1; } }
{ let __ice_next = crate::backend::keep_i64(crate::backend::plane_live_hit(next.kind.clone(), next.module.to_owned(), "valset".to_owned()), (self.members_generation + 1), self.members_generation); if ::ui_lang_runtime::state_changed!(self.members_generation, __ice_next) { self.members_generation = __ice_next; self.__ice_rev[58] += 1; } }
{ let __ice_next = crate::backend::keep_i64(crate::backend::plane_live_hit(next.kind.clone(), next.module.to_owned(), "identity".to_owned()), (self.account_generation + 1), self.account_generation); if ::ui_lang_runtime::state_changed!(self.account_generation, __ice_next) { self.account_generation = __ice_next; self.__ice_rev[76] += 1; } }
{ let __ice_next = crate::backend::keep_i64(crate::backend::plane_live_hit(next.kind.clone(), next.module.to_owned(), "identity".to_owned()), (self.dm_peers_generation + 1), self.dm_peers_generation); if ::ui_lang_runtime::state_changed!(self.dm_peers_generation, __ice_next) { self.dm_peers_generation = __ice_next; self.__ice_rev[52] += 1; } }
return ::ducktape_view_guest::Task::batch([
{ // __ICE_SOURCE 264 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6c6966656379636c652e696365
((::ducktape_view_guest::Task::done(crate::backend::load_request(crate::backend::plane_live_hit(next.kind.clone(), next.module.to_owned(), "valset".to_owned()), self.connected_rpc.to_owned(), "".to_owned(), self.members_generation))).and_then(move |request| ::ducktape_view_guest::Task::done(request.clone()))).map(|value| __DucktapeMessage::MembersLoadSelected(value))
},
{ // __ICE_SOURCE 268 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6c6966656379636c652e696365
((::ducktape_view_guest::Task::done(crate::backend::load_request(crate::backend::plane_live_hit(next.kind.clone(), next.module.to_owned(), "identity".to_owned()), self.connected_rpc.to_owned(), "".to_owned(), self.account_generation))).and_then(move |request| ::ducktape_view_guest::Task::done(request.clone()))).map(|value| __DucktapeMessage::AccountLoadSelected(value))
},
{ // __ICE_SOURCE 272 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6c6966656379636c652e696365
((::ducktape_view_guest::Task::done(crate::backend::load_request(crate::backend::plane_live_hit(next.kind.clone(), next.module.to_owned(), "identity".to_owned()), self.connected_rpc.to_owned(), "".to_owned(), self.dm_peers_generation))).and_then(move |request| ::ducktape_view_guest::Task::done(request.clone()))).map(|value| __DucktapeMessage::DmPeersLoadSelected(value))
},
{ // __ICE_SOURCE 279 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6c6966656379636c652e696365
((::ducktape_view_guest::Task::done(crate::backend::load_request(crate::backend::plane_live_hit(next.kind.clone(), next.module.to_owned(), "identity".to_owned()), self.connected_rpc.to_owned(), "".to_owned(), self.hydration_generation))).and_then(move |request| ::ducktape_view_guest::Task::done(request.clone()))).map(|value| __DucktapeMessage::NamesMovedSelected(value))
},
]);
})(),
LiveKind::Resync => (|| {
{ let __ice_next = ({ crate::module_view::view_live_hit(::std::convert::AsRef::as_ref(&(next.module)), self.views_live_serial) }); if ::ui_lang_runtime::state_changed!(self.views_live_serial, __ice_next) { self.views_live_serial = __ice_next; self.__ice_rev[10] += 1; } }
if (!next.load_chat) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = (self.hydration_generation + 1); if ::ui_lang_runtime::state_changed!(self.hydration_generation, __ice_next) { self.hydration_generation = __ice_next; self.__ice_rev[15] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.hydration_retry_attempt, __ice_next) { self.hydration_retry_attempt = __ice_next; self.__ice_rev[18] += 1; } }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::live_resync_load(self.connected_rpc.to_owned(), self.active_channel.to_owned(), next.load_chat, next.debounce, self.hydration_generation, 0) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::LiveResynced(value), ::std::result::Result::Err(error) => __DucktapeMessage::LiveResyncFailed(error) }); self.__ice_run_lane_8_generation = self.__ice_run_lane_8_generation.wrapping_add(1); let __generation = self.__ice_run_lane_8_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_8_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane8(__generation, ::std::boxed::Box::new(__message))) };
})(),
};
})(),
__DucktapeMessage::LiveResynced(next) => (|| {
let _ = &next;
if (next.generation != self.hydration_generation) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.hydration_retry_attempt, __ice_next) { self.hydration_retry_attempt = __ice_next; self.__ice_rev[18] += 1; } }
let chain_left_behind = crate::backend::chain_moved(self.chat_chain_id.to_owned(), self.network_chain_id.to_owned());
self.channels = crate::backend::keep_channels(next.chat_loaded, chain_left_behind, next.channels.clone(), ::std::mem::take(&mut self.channels)); self.__ice_rev[22] += 1;
self.channel_reads = crate::backend::initial_channel_reads(self.channels.clone(), ::std::mem::take(&mut self.channel_reads)); self.__ice_rev[25] += 1;
{ let __ice_next = crate::backend::keep_str((next.chat_loaded && (!(self.network_chain_id).is_empty())), ::std::convert::AsRef::as_ref(&(self.network_chain_id)), ::std::convert::AsRef::as_ref(&(self.chat_chain_id))); if ::ui_lang_runtime::state_changed!(self.chat_chain_id, __ice_next) { self.chat_chain_id = __ice_next; self.__ice_rev[49] += 1; } }
{ let __ice_next = (self.history_view && (!next.chat_loaded)); if ::ui_lang_runtime::state_changed!(self.history_view, __ice_next) { self.history_view = __ice_next; self.__ice_rev[48] += 1; } }
{ let __ice_next = crate::backend::keep_i64(next.chat_loaded, 0, self.chat_land_seq); if ::ui_lang_runtime::state_changed!(self.chat_land_seq, __ice_next) { self.chat_land_seq = __ice_next; self.__ice_rev[33] += 1; } }
{ let __ice_next = crate::backend::keep_str(next.chat_loaded, ::std::convert::AsRef::as_ref(&(next.active_channel)), ::std::convert::AsRef::as_ref(&(self.active_channel))); if ::ui_lang_runtime::state_changed!(self.active_channel, __ice_next) { self.active_channel = __ice_next; self.__ice_rev[27] += 1; } }
{ let __ice_next = crate::backend::keep_str((next.chat_loaded && (!self.loading)), ::std::convert::AsRef::as_ref(&(crate::backend::dm_peer_of_channel(self.active_dm_peer.to_owned(), self.dm_peers.clone(), self.active_channel.to_owned()))), ::std::convert::AsRef::as_ref(&(self.active_dm_peer))); if ::ui_lang_runtime::state_changed!(self.active_dm_peer, __ice_next) { self.active_dm_peer = __ice_next; self.__ice_rev[53] += 1; } }
{ let __ice_next = crate::backend::dm_peer_named(self.dm_peers.clone(), self.active_dm_peer.to_owned()); if ::ui_lang_runtime::state_changed!(self.active_dm, __ice_next) { self.active_dm = __ice_next; self.__ice_rev[54] += 1; } }
{ let __ice_next = crate::backend::keep_str(next.chat_loaded, ::std::convert::AsRef::as_ref(&(next.active_channel_name)), ::std::convert::AsRef::as_ref(&(self.active_channel_name))); if ::ui_lang_runtime::state_changed!(self.active_channel_name, __ice_next) { self.active_channel_name = __ice_next; self.__ice_rev[28] += 1; } }
{ let __ice_next = crate::backend::keep_bool(next.chat_loaded, next.active_channel_archived, self.active_channel_archived); if ::ui_lang_runtime::state_changed!(self.active_channel_archived, __ice_next) { self.active_channel_archived = __ice_next; self.__ice_rev[29] += 1; } }
{ let __ice_next = crate::backend::keep_bool(next.chat_loaded, next.active_channel_members_only, self.active_channel_members_only); if ::ui_lang_runtime::state_changed!(self.active_channel_members_only, __ice_next) { self.active_channel_members_only = __ice_next; self.__ice_rev[30] += 1; } }
{ let __ice_next = crate::backend::keep_i64(self.huddle_joined, self.huddle_joined_at, self.huddle_now); if ::ui_lang_runtime::state_changed!(self.huddle_joined_at, __ice_next) { self.huddle_joined_at = __ice_next; self.__ice_rev[145] += 1; } }
let huddle = crate::backend::huddle_after_load(next.chat_loaded, self.huddle_joined, self.huddle_channel.to_owned(), self.huddle_channel_name.to_owned(), self.huddle_roster.clone(), self.active_channel.to_owned(), self.active_channel_name.to_owned(), next.huddle_roster.clone());
{ let __ice_next = huddle.joined; if ::ui_lang_runtime::state_changed!(self.huddle_joined, __ice_next) { self.huddle_joined = __ice_next; self.__ice_rev[142] += 1; } }
{ let __ice_next = huddle.roster.clone(); if ::ui_lang_runtime::state_changed!(self.huddle_roster, __ice_next) { self.huddle_roster = __ice_next; self.__ice_rev[154] += 1; } }
{ let __ice_next = crate::call::huddle_tile_rows(self.huddle_roster.clone(), self.call_peers.clone(), self.call_muted); if ::ui_lang_runtime::state_changed!(self.huddle_rows, __ice_next) { self.huddle_rows = __ice_next; self.__ice_rev[155] += 1; } }
{ let __ice_next = huddle.channel.to_owned(); if ::ui_lang_runtime::state_changed!(self.huddle_channel, __ice_next) { self.huddle_channel = __ice_next; self.__ice_rev[143] += 1; } }
{ let __ice_next = huddle.channel_name.to_owned(); if ::ui_lang_runtime::state_changed!(self.huddle_channel_name, __ice_next) { self.huddle_channel_name = __ice_next; self.__ice_rev[144] += 1; } }
self.channel_members = crate::backend::keep_members(next.chat_loaded, next.channel_members.clone(), ::std::mem::take(&mut self.channel_members)); self.__ice_rev[31] += 1;
{ let __ice_next = crate::backend::post_gate(self.active_channel_archived, self.active_channel_members_only, self.channel_members.clone(), self.settings_user_key.to_owned()); if ::ui_lang_runtime::state_changed!(self.post_refusal, __ice_next) { self.post_refusal = __ice_next; self.__ice_rev[32] += 1; } }
let resync_tail_channel = crate::backend::keep_str(((!self.history_view) && (self.shell_tab == ShellTab::Chat)), ::std::convert::AsRef::as_ref(&(self.active_channel)), ::std::convert::AsRef::as_ref(&("")));
{ let __ice_next = crate::backend::frozen_unread_boundary(self.channel_reads.clone(), self.channels.clone(), self.active_channel.to_owned(), self.active_channel.to_owned(), self.unread_boundary); if ::ui_lang_runtime::state_changed!(self.unread_boundary, __ice_next) { self.unread_boundary = __ice_next; self.__ice_rev[26] += 1; } }
self.channel_reads = crate::backend::mark_channel_read(::std::mem::take(&mut self.channel_reads), resync_tail_channel.to_owned(), crate::backend::channel_head_seq(self.channels.clone(), resync_tail_channel.to_owned())); self.__ice_rev[25] += 1;
{ let __ice_next = crate::backend::chat_sidebar_rooms(self.channels.clone(), self.dm_peers.clone(), self.channel_reads.clone()); if ::ui_lang_runtime::state_changed!(self.rooms, __ice_next) { self.rooms = __ice_next; self.__ice_rev[23] += 1; } }
{ let __ice_next = crate::backend::chat_sidebar_dms(self.channels.clone(), self.dm_peers.clone(), self.channel_reads.clone()); if ::ui_lang_runtime::state_changed!(self.dm_rows, __ice_next) { self.dm_rows = __ice_next; self.__ice_rev[51] += 1; } }
{ let __ice_next = crate::backend::mutation_phase_after_recovery(self.mutation_phase.clone()); if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
return ::ducktape_view_guest::Task::batch([
{ // __ICE_SOURCE 413 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6c6966656379636c652e696365
::crate::shell::close::<__DucktapeMessage>(({ crate::backend::window_target_unless(self.huddle_joined, self.huddle_win.clone()) }))
},
]);
})(),
__DucktapeMessage::LiveResyncFailed(cause) => (|| {
let _ = &cause;
if (cause.generation != self.hydration_generation) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "Sync delayed".to_owned(); if ::ui_lang_runtime::state_changed!(self.status, __ice_next) { self.status = __ice_next; self.__ice_rev[7] += 1; } }
{ let __ice_next = "Live sync interrupted. Retrying…".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
{ let __ice_next = (self.hydration_retry_attempt + 1); if ::ui_lang_runtime::state_changed!(self.hydration_retry_attempt, __ice_next) { self.hydration_retry_attempt = __ice_next; self.__ice_rev[18] += 1; } }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::live_resync_load(self.connected_rpc.to_owned(), self.active_channel.to_owned(), true, false, self.hydration_generation, self.hydration_retry_attempt) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::LiveResynced(value), ::std::result::Result::Err(error) => __DucktapeMessage::LiveResyncFailed(error) }); self.__ice_run_lane_8_generation = self.__ice_run_lane_8_generation.wrapping_add(1); let __generation = self.__ice_run_lane_8_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_8_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane8(__generation, ::std::boxed::Box::new(__message))) };
})(),
__DucktapeMessage::SelectShellTab(next) => (|| {
let _ = &next;
let staying_on_settings = ((self.shell_tab == ShellTab::Settings) && (next == ShellTab::Settings));
let keeping_authentication = (staying_on_settings && (!(self.account_ceremony_phase).is_empty()));
if keeping_authentication { return ::ducktape_view_guest::Task::none(); }
self.__ice_run_lane_10_generation = self.__ice_run_lane_10_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_10_handle.take() { __previous.abort(); }
self.__ice_run_lane_11_generation = self.__ice_run_lane_11_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_11_handle.take() { __previous.abort(); }
{ let __ice_next = (self.account_busy && (self.account_ceremony_phase).is_empty()); if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_phase, __ice_next) { self.account_ceremony_phase = __ice_next; self.__ice_rev[80] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_qr, __ice_next) { self.account_ceremony_qr = __ice_next; self.__ice_rev[81] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_detail, __ice_next) { self.account_ceremony_detail = __ice_next; self.__ice_rev[82] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_left, __ice_next) { self.account_ceremony_left = __ice_next; self.__ice_rev[83] += 1; } }
{ let __ice_next = next.clone(); if ::ui_lang_runtime::state_changed!(self.shell_tab, __ice_next) { self.shell_tab = __ice_next; self.__ice_rev[55] += 1; } }
let chat_tab_channel = crate::backend::keep_str(((self.shell_tab == ShellTab::Chat) && (!self.history_view)), ::std::convert::AsRef::as_ref(&(self.active_channel)), ::std::convert::AsRef::as_ref(&("")));
let chat_tab_arrivals = (crate::backend::channel_head_seq(self.channels.clone(), chat_tab_channel.to_owned()) > crate::backend::channel_last_read(self.channel_reads.clone(), chat_tab_channel.to_owned()));
{ let __ice_next = crate::backend::keep_i64(chat_tab_arrivals, crate::backend::channel_last_read(self.channel_reads.clone(), chat_tab_channel.to_owned()), self.unread_boundary); if ::ui_lang_runtime::state_changed!(self.unread_boundary, __ice_next) { self.unread_boundary = __ice_next; self.__ice_rev[26] += 1; } }
self.channel_reads = crate::backend::mark_channel_read(::std::mem::take(&mut self.channel_reads), chat_tab_channel.to_owned(), crate::backend::channel_head_seq(self.channels.clone(), chat_tab_channel.to_owned())); self.__ice_rev[25] += 1;
{ let __ice_next = crate::backend::chat_sidebar_rooms(self.channels.clone(), self.dm_peers.clone(), self.channel_reads.clone()); if ::ui_lang_runtime::state_changed!(self.rooms, __ice_next) { self.rooms = __ice_next; self.__ice_rev[23] += 1; } }
{ let __ice_next = crate::backend::chat_sidebar_dms(self.channels.clone(), self.dm_peers.clone(), self.channel_reads.clone()); if ::ui_lang_runtime::state_changed!(self.dm_rows, __ice_next) { self.dm_rows = __ice_next; self.__ice_rev[51] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
if (!self.connected) { return ::ducktape_view_guest::Task::none(); }
if ((self.shell_tab == ShellTab::Chat) || (self.shell_tab == ShellTab::Pages)) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = (self.members_generation + 1); if ::ui_lang_runtime::state_changed!(self.members_generation, __ice_next) { self.members_generation = __ice_next; self.__ice_rev[58] += 1; } }
{ let __ice_next = (self.account_generation + 1); if ::ui_lang_runtime::state_changed!(self.account_generation, __ice_next) { self.account_generation = __ice_next; self.__ice_rev[76] += 1; } }
{ let __ice_next = crate::backend::keep_i64((self.shell_tab == ShellTab::Settings), (self.settings_generation + 1), self.settings_generation); if ::ui_lang_runtime::state_changed!(self.settings_generation, __ice_next) { self.settings_generation = __ice_next; self.__ice_rev[71] += 1; } }
return ::ducktape_view_guest::Task::batch([
{ // __ICE_SOURCE 481 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6c6966656379636c652e696365
((::ducktape_view_guest::Task::done(crate::backend::load_request(crate::backend::tab_reads_plane(self.shell_tab.clone(), "members".to_owned()), self.connected_rpc.to_owned(), "".to_owned(), self.members_generation))).and_then(move |request| ::ducktape_view_guest::Task::done(request.clone()))).map(|value| __DucktapeMessage::MembersLoadSelected(value))
},
{ // __ICE_SOURCE 485 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6c6966656379636c652e696365
((::ducktape_view_guest::Task::done(crate::backend::load_request((self.shell_tab == ShellTab::Settings), self.connected_rpc.to_owned(), "".to_owned(), self.settings_generation))).and_then(move |request| ::ducktape_view_guest::Task::done(request.clone()))).map(|value| __DucktapeMessage::SettingsLoadSelected(value))
},
{ // __ICE_SOURCE 489 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6c6966656379636c652e696365
((::ducktape_view_guest::Task::done(crate::backend::load_request(crate::backend::tab_reads_plane(self.shell_tab.clone(), "account".to_owned()), self.connected_rpc.to_owned(), "".to_owned(), self.account_generation))).and_then(move |request| ::ducktape_view_guest::Task::done(request.clone()))).map(|value| __DucktapeMessage::AccountLoadSelected(value))
},
]);
})(),
__DucktapeMessage::MembersLoadSelected(request) => (|| {
let _ = &request;
let obsolete_request = ((request.rpc != self.connected_rpc) || (request.generation != self.members_generation));
if obsolete_request { return ::ducktape_view_guest::Task::none(); }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::load_members(request.rpc.to_owned(), request.generation) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::MembersLoaded(value), ::std::result::Result::Err(error) => __DucktapeMessage::MembersFailed(error) }); self.__ice_run_lane_5_generation = self.__ice_run_lane_5_generation.wrapping_add(1); let __generation = self.__ice_run_lane_5_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_5_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane5(__generation, ::std::boxed::Box::new(__message))) };
})(),
__DucktapeMessage::SettingsLoadSelected(request) => (|| {
let _ = &request;
let obsolete_request = ((request.rpc != self.connected_rpc) || (request.generation != self.settings_generation));
let unmounted = (self.shell_tab != ShellTab::Settings);
if (obsolete_request || unmounted) { return ::ducktape_view_guest::Task::none(); }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::load_settings_facts(request.rpc.to_owned(), request.generation) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::SettingsLoaded(value), ::std::result::Result::Err(error) => __DucktapeMessage::SettingsFailed(error) }); self.__ice_run_lane_6_generation = self.__ice_run_lane_6_generation.wrapping_add(1); let __generation = self.__ice_run_lane_6_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_6_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane6(__generation, ::std::boxed::Box::new(__message))) };
})(),
__DucktapeMessage::AccountLoadSelected(request) => (|| {
let _ = &request;
let obsolete_request = ((request.rpc != self.connected_rpc) || (request.generation != self.account_generation));
if obsolete_request { return ::ducktape_view_guest::Task::none(); }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::load_account(request.rpc.to_owned(), request.generation) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::AccountLoaded(value), ::std::result::Result::Err(error) => __DucktapeMessage::AccountFailed(error) }); self.__ice_run_lane_7_generation = self.__ice_run_lane_7_generation.wrapping_add(1); let __generation = self.__ice_run_lane_7_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_7_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane7(__generation, ::std::boxed::Box::new(__message))) };
})(),
__DucktapeMessage::DmPeersLoadSelected(request) => (|| {
let _ = &request;
let obsolete_request = ((request.rpc != self.connected_rpc) || (request.generation != self.dm_peers_generation));
if obsolete_request { return ::ducktape_view_guest::Task::none(); }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::load_dm_peers(request.rpc.to_owned(), request.generation) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::DmPeersLoaded(value), ::std::result::Result::Err(error) => __DucktapeMessage::DmPeersFailed(error) }); self.__ice_run_lane_2_generation = self.__ice_run_lane_2_generation.wrapping_add(1); let __generation = self.__ice_run_lane_2_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_2_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane2(__generation, ::std::boxed::Box::new(__message))) };
})(),
__DucktapeMessage::NamesMovedSelected(request) => (|| {
let _ = &request;
let obsolete_request = ((request.rpc != self.connected_rpc) || (request.generation != self.hydration_generation));
if obsolete_request { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = (self.hydration_generation + 1); if ::ui_lang_runtime::state_changed!(self.hydration_generation, __ice_next) { self.hydration_generation = __ice_next; self.__ice_rev[15] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.hydration_retry_attempt, __ice_next) { self.hydration_retry_attempt = __ice_next; self.__ice_rev[18] += 1; } }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::live_resync_load(self.connected_rpc.to_owned(), self.active_channel.to_owned(), true, false, self.hydration_generation, 0) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::LiveResynced(value), ::std::result::Result::Err(error) => __DucktapeMessage::LiveResyncFailed(error) }); self.__ice_run_lane_8_generation = self.__ice_run_lane_8_generation.wrapping_add(1); let __generation = self.__ice_run_lane_8_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_8_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane8(__generation, ::std::boxed::Box::new(__message))) };
})(),
__DucktapeMessage::Tick => (|| {
{ let __ice_next = (self.huddle_now + 1); if ::ui_lang_runtime::state_changed!(self.huddle_now, __ice_next) { self.huddle_now = __ice_next; self.__ice_rev[146] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::WallTick => (|| {
{ let __ice_next = ({ crate::backend::current_wall_seconds() }); if ::ui_lang_runtime::state_changed!(self.wall_now, __ice_next) { self.wall_now = __ice_next; self.__ice_rev[3] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::WindowWasClosed(id) => (|| {
let _ = &id;
let closed_welcome = ((self.onboarding_win == ::std::option::Option::Some(id)) && (self.hub_step == HubStep::Account));
let closed_account = (self.console_win == ::std::option::Option::Some(id));
let retirement = crate::backend::ceremony_retirement(closed_welcome, closed_account);
{ let __ice_next = crate::backend::without_window(self.onboarding_win.clone(), id); if ::ui_lang_runtime::state_changed!(self.onboarding_win, __ice_next) { self.onboarding_win = __ice_next; self.__ice_rev[121] += 1; } }
{ let __ice_next = crate::backend::without_window(self.console_win.clone(), id); if ::ui_lang_runtime::state_changed!(self.console_win, __ice_next) { self.console_win = __ice_next; self.__ice_rev[122] += 1; } }
{ let __ice_next = crate::backend::without_window(self.huddle_win.clone(), id); if ::ui_lang_runtime::state_changed!(self.huddle_win, __ice_next) { self.huddle_win = __ice_next; self.__ice_rev[123] += 1; } }
let leaving = crate::backend::last_window_closed_exits(self.console_win.clone(), self.onboarding_win.clone());
return match retirement.clone() {
CeremonyRetirement::Welcome => (|| {
self.__ice_run_lane_24_generation = self.__ice_run_lane_24_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_24_handle.take() { __previous.abort(); }
self.__ice_run_lane_25_generation = self.__ice_run_lane_25_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_25_handle.take() { __previous.abort(); }
{ let __ice_next = MutationPhase::Idle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_phase, __ice_next) { self.ceremony_phase = __ice_next; self.__ice_rev[138] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_qr, __ice_next) { self.ceremony_qr = __ice_next; self.__ice_rev[139] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_detail, __ice_next) { self.ceremony_detail = __ice_next; self.__ice_rev[140] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_left, __ice_next) { self.ceremony_left = __ice_next; self.__ice_rev[141] += 1; } }
if (!leaving) { return ::ducktape_view_guest::Task::none(); }
return ::crate::shell::quit::<__DucktapeMessage>();
})(),
CeremonyRetirement::Account => (|| {
self.__ice_run_lane_10_generation = self.__ice_run_lane_10_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_10_handle.take() { __previous.abort(); }
self.__ice_run_lane_11_generation = self.__ice_run_lane_11_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_11_handle.take() { __previous.abort(); }
{ let __ice_next = (self.account_busy && (self.account_ceremony_phase).is_empty()); if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_phase, __ice_next) { self.account_ceremony_phase = __ice_next; self.__ice_rev[80] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_qr, __ice_next) { self.account_ceremony_qr = __ice_next; self.__ice_rev[81] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_detail, __ice_next) { self.account_ceremony_detail = __ice_next; self.__ice_rev[82] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_left, __ice_next) { self.account_ceremony_left = __ice_next; self.__ice_rev[83] += 1; } }
if (!leaving) { return ::ducktape_view_guest::Task::none(); }
return ::crate::shell::quit::<__DucktapeMessage>();
})(),
CeremonyRetirement::Keep => (|| {
if (!leaving) { return ::ducktape_view_guest::Task::none(); }
return ::crate::shell::quit::<__DucktapeMessage>();
})(),
};
})(),
__DucktapeMessage::TrayOpen => (|| {
let window_tracked = ((self.console_win != ::std::option::Option::None) || (self.onboarding_win != ::std::option::Option::None));
let opening = crate::backend::tray_open_action(self.connected, window_tracked);
return match opening.clone() {
TrayOpen::Launch => (|| {
return { let (_, __task) = ::crate::shell::open(Self::__window_0()); __task.map(move |value| __DucktapeMessage::OnboardingOpened(value)) };
})(),
TrayOpen::Console => (|| {
return (::ducktape_view_guest::Task::done(true)).map(|value| { let _ = &value; __DucktapeMessage::NetworkEntered });
})(),
TrayOpen::Raise => (|| {
return ::ducktape_view_guest::Task::batch([
{ // __ICE_SOURCE 713 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6c6966656379636c652e696365
::crate::shell::raise::<__DucktapeMessage>(({ crate::backend::window_target(self.console_win.clone()) }))
},
{ // __ICE_SOURCE 714 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6c6966656379636c652e696365
::crate::shell::raise::<__DucktapeMessage>(({ crate::backend::window_target(self.onboarding_win.clone()) }))
},
]);
})(),
};
})(),
__DucktapeMessage::TrayQuit => (|| {
self.__ice_run_lane_24_generation = self.__ice_run_lane_24_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_24_handle.take() { __previous.abort(); }
self.__ice_run_lane_25_generation = self.__ice_run_lane_25_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_25_handle.take() { __previous.abort(); }
self.__ice_run_lane_10_generation = self.__ice_run_lane_10_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_10_handle.take() { __previous.abort(); }
self.__ice_run_lane_11_generation = self.__ice_run_lane_11_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_11_handle.take() { __previous.abort(); }
return ::crate::shell::quit::<__DucktapeMessage>();
})(),
__DucktapeMessage::ModifierStateChanged(mods) => (|| {
let _ = &mods;
{ let __ice_next = crate::backend::command_held(mods); if ::ui_lang_runtime::state_changed!(self.cmd_held, __ice_next) { self.cmd_held = __ice_next; self.__ice_rev[11] += 1; } }
{ let __ice_next = crate::backend::shift_held(mods); if ::ui_lang_runtime::state_changed!(self.shift_held, __ice_next) { self.shift_held = __ice_next; self.__ice_rev[12] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::DragLaunchWindow => (|| {
return ::crate::shell::oldest().and_then(move |__window| ::crate::shell::drag::<__DucktapeMessage>(__window));
})(),
__DucktapeMessage::CloseLaunchWindow => (|| {
return ::crate::shell::close::<__DucktapeMessage>(({ crate::backend::window_target(self.onboarding_win.clone()) }));
})(),
__DucktapeMessage::WindowFocused(id) => (|| {
let _ = &id;
{ let __ice_next = ::std::option::Option::Some(id); if ::ui_lang_runtime::state_changed!(self.focused_win, __ice_next) { self.focused_win = __ice_next; self.__ice_rev[13] += 1; } }
return ({ crate::backend::note_window_focus(true) }).map(|value| { let _ = &value; __DucktapeMessage::WindowFocusNoted });
})(),
__DucktapeMessage::WindowUnfocused(id) => (|| {
let _ = &id;
{ let __ice_next = crate::backend::without_window(self.focused_win.clone(), id); if ::ui_lang_runtime::state_changed!(self.focused_win, __ice_next) { self.focused_win = __ice_next; self.__ice_rev[13] += 1; } }
return ({ crate::backend::note_window_focus((self.focused_win != ::std::option::Option::None)) }).map(|value| { let _ = &value; __DucktapeMessage::WindowFocusNoted });
})(),
__DucktapeMessage::WindowFocusNoted => (|| {
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::CommandChordPressed(event) => (|| {
let _ = &event;
let chord = crate::backend::command_chord(event.key.clone(), event.modifiers);
return match chord.clone() {
CommandChord::Quit => (|| {
self.__ice_run_lane_24_generation = self.__ice_run_lane_24_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_24_handle.take() { __previous.abort(); }
self.__ice_run_lane_25_generation = self.__ice_run_lane_25_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_25_handle.take() { __previous.abort(); }
self.__ice_run_lane_10_generation = self.__ice_run_lane_10_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_10_handle.take() { __previous.abort(); }
self.__ice_run_lane_11_generation = self.__ice_run_lane_11_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_11_handle.take() { __previous.abort(); }
return ::crate::shell::quit::<__DucktapeMessage>();
})(),
CommandChord::CloseWindow => (|| {
return ::crate::shell::close::<__DucktapeMessage>(({ crate::backend::window_target(self.focused_win.clone()) }));
})(),
CommandChord::Ignored => (|| {
if true { return ::ducktape_view_guest::Task::none(); }
::ducktape_view_guest::Task::none()
})(),
};
})(),
__DucktapeMessage::TrayOpenBell => (|| {
if (self.console_win == ::std::option::Option::None) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.bell_open, __ice_next) { self.bell_open = __ice_next; self.__ice_rev[105] += 1; } }
return ::crate::shell::raise::<__DucktapeMessage>(({ crate::backend::window_target(self.console_win.clone()) }));
})(),
__DucktapeMessage::TrayGoChat => (|| {
if (self.console_win == ::std::option::Option::None) { return ::ducktape_view_guest::Task::none(); }
return ::ducktape_view_guest::Task::batch([
{ // __ICE_SOURCE 789 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6c6966656379636c652e696365
::crate::shell::raise::<__DucktapeMessage>(({ crate::backend::window_target(self.console_win.clone()) }))
},
{ // __ICE_SOURCE 790 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6c6966656379636c652e696365
(::ducktape_view_guest::Task::done(ShellTab::Chat)).map(|value| __DucktapeMessage::SelectShellTab(value))
},
]);
})(),
__DucktapeMessage::TrayGoPages => (|| {
if (self.console_win == ::std::option::Option::None) { return ::ducktape_view_guest::Task::none(); }
return ::ducktape_view_guest::Task::batch([
{ // __ICE_SOURCE 797 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6c6966656379636c652e696365
::crate::shell::raise::<__DucktapeMessage>(({ crate::backend::window_target(self.console_win.clone()) }))
},
{ // __ICE_SOURCE 798 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6c6966656379636c652e696365
(::ducktape_view_guest::Task::done(ShellTab::Pages)).map(|value| __DucktapeMessage::SelectShellTab(value))
},
]);
})(),
__DucktapeMessage::TrayGoNode => (|| {
if (self.console_win == ::std::option::Option::None) { return ::ducktape_view_guest::Task::none(); }
return ::ducktape_view_guest::Task::batch([
{ // __ICE_SOURCE 805 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6c6966656379636c652e696365
::crate::shell::raise::<__DucktapeMessage>(({ crate::backend::window_target(self.console_win.clone()) }))
},
{ // __ICE_SOURCE 806 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6c6966656379636c652e696365
(::ducktape_view_guest::Task::done(ShellTab::Node)).map(|value| __DucktapeMessage::SelectShellTab(value))
},
]);
})(),
__DucktapeMessage::TrayGoSettings => (|| {
if (self.console_win == ::std::option::Option::None) { return ::ducktape_view_guest::Task::none(); }
return ::ducktape_view_guest::Task::batch([
{ // __ICE_SOURCE 813 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6c6966656379636c652e696365
::crate::shell::raise::<__DucktapeMessage>(({ crate::backend::window_target(self.console_win.clone()) }))
},
{ // __ICE_SOURCE 814 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6c6966656379636c652e696365
(::ducktape_view_guest::Task::done(ShellTab::Settings)).map(|value| __DucktapeMessage::SelectShellTab(value))
},
]);
})(),
__DucktapeMessage::TrayReconnect => (|| {
if (self.console_win == ::std::option::Option::None) { return ::ducktape_view_guest::Task::none(); }
return (::ducktape_view_guest::Task::done(true)).map(|value| { let _ = &value; __DucktapeMessage::Reconnect });
})(),
__DucktapeMessage::TrayCopyNodeKey => (|| {
if ((self.console_win == ::std::option::Option::None) || (self.node_key).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "Copied node key".to_owned(); if ::ui_lang_runtime::state_changed!(self.toast, __ice_next) { self.toast = __ice_next; self.__ice_rev[117] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.toast_age, __ice_next) { self.toast_age = __ice_next; self.__ice_rev[118] += 1; } }
return ::crate::shell::clipboard::<__DucktapeMessage>(self.node_key.to_owned());
})(),
__DucktapeMessage::MutationFailed(cause) => (|| {
let _ = &cause;
{ let __ice_next = crate::backend::message_seq_after_failure(self.chat_edit_seq, self.mutation_phase.clone(), cause.committed); if ::ui_lang_runtime::state_changed!(self.chat_edit_seq, __ice_next) { self.chat_edit_seq = __ice_next; self.__ice_rev[34] += 1; } }
{ let __ice_next = crate::backend::message_seq_after_failure(self.chat_edit_rev, self.mutation_phase.clone(), cause.committed); if ::ui_lang_runtime::state_changed!(self.chat_edit_rev, __ice_next) { self.chat_edit_rev = __ice_next; self.__ice_rev[35] += 1; } }
{ let __ice_next = crate::backend::mutation_failure_phase(cause.committed); if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = crate::backend::restore_draft(self.channel_draft.to_owned(), self.pending_channel.to_owned(), cause.committed); if ::ui_lang_runtime::state_changed!(self.channel_draft, __ice_next) { self.channel_draft = __ice_next; self.__ice_rev[43] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.pending_channel, __ice_next) { self.pending_channel = __ice_next; self.__ice_rev[46] += 1; } }
{ let __ice_next = cause.message.to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
if (!cause.committed) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = (self.hydration_generation + 1); if ::ui_lang_runtime::state_changed!(self.hydration_generation, __ice_next) { self.hydration_generation = __ice_next; self.__ice_rev[15] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.hydration_retry_attempt, __ice_next) { self.hydration_retry_attempt = __ice_next; self.__ice_rev[18] += 1; } }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::live_resync_load(self.connected_rpc.to_owned(), self.active_channel.to_owned(), true, false, self.hydration_generation, 0) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::LiveResynced(value), ::std::result::Result::Err(error) => __DucktapeMessage::LiveResyncFailed(error) }); self.__ice_run_lane_8_generation = self.__ice_run_lane_8_generation.wrapping_add(1); let __generation = self.__ice_run_lane_8_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_8_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane8(__generation, ::std::boxed::Box::new(__message))) };
})(),
__DucktapeMessage::DismissError => (|| {
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::ConnectFailed(cause) => (|| {
let _ = &cause;
if (cause.generation != self.connect_generation) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = (self.hydration_generation + 1); if ::ui_lang_runtime::state_changed!(self.hydration_generation, __ice_next) { self.hydration_generation = __ice_next; self.__ice_rev[15] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.bell_marking, __ice_next) { self.bell_marking = __ice_next; self.__ice_rev[111] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.bell_error, __ice_next) { self.bell_error = __ice_next; self.__ice_rev[112] += 1; } }
self.__ice_run_lane_13_generation = self.__ice_run_lane_13_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_13_handle.take() { __previous.abort(); }
self.__ice_run_lane_14_generation = self.__ice_run_lane_14_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_14_handle.take() { __previous.abort(); }
self.__ice_run_lane_4_generation = self.__ice_run_lane_4_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_4_handle.take() { __previous.abort(); }
self.__ice_run_lane_9_generation = self.__ice_run_lane_9_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_9_handle.take() { __previous.abort(); }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.bell_items, __ice_next) { self.bell_items = __ice_next; self.__ice_rev[107] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.bell_presentations, __ice_next) { self.bell_presentations = __ice_next; self.__ice_rev[108] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.bell_unread, __ice_next) { self.bell_unread = __ice_next; self.__ice_rev[106] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.bell_read_through, __ice_next) { self.bell_read_through = __ice_next; self.__ice_rev[109] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.bell_clear_through, __ice_next) { self.bell_clear_through = __ice_next; self.__ice_rev[110] += 1; } }
{ let __ice_next = (self.connect_generation + 1); if ::ui_lang_runtime::state_changed!(self.connect_generation, __ice_next) { self.connect_generation = __ice_next; self.__ice_rev[16] += 1; } }
{ let __ice_next = (self.hydration_retry_attempt + 1); if ::ui_lang_runtime::state_changed!(self.hydration_retry_attempt, __ice_next) { self.hydration_retry_attempt = __ice_next; self.__ice_rev[18] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.loading, __ice_next) { self.loading = __ice_next; self.__ice_rev[9] += 1; } }
{ let __ice_next = "Offline".to_owned(); if ::ui_lang_runtime::state_changed!(self.status, __ice_next) { self.status = __ice_next; self.__ice_rev[7] += 1; } }
{ let __ice_next = cause.message.to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
{ let __ice_next = crate::backend::keep_str((self.console_entry == ConsoleEntry::Entering), ::std::convert::AsRef::as_ref(&(cause.message)), ::std::convert::AsRef::as_ref(&(self.onboarding_error))); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::connect(self.connected_rpc.to_owned(), self.hydration_retry_attempt, self.connect_generation) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::WorkspaceConnected(value), ::std::result::Result::Err(error) => __DucktapeMessage::ConnectFailed(error) }); self.__ice_run_lane_1_generation = self.__ice_run_lane_1_generation.wrapping_add(1); let __generation = self.__ice_run_lane_1_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_1_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane1(__generation, ::std::boxed::Box::new(__message))) };
})(),
__DucktapeMessage::ForgeViewEvent(event) => (|| {
let _ = &event;
return match crate::module_view::forge_intent(::std::borrow::Borrow::borrow(&(event))) {
ForgeIntent::OpenLink => (|| {
return (::ducktape_view_guest::Task::done(crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("url"))))).map(|value| __DucktapeMessage::OpenMessageLink(value));
})(),
ForgeIntent::Composer => (|| {
let scope = crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("scope")));
return { let __ice_run_route_60_0 = scope.to_owned();
::ducktape_view_guest::Task::perform(({ crate::backend::duck_echo_str(crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("body")))) }), move |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::ForgeComposerEvent(__ice_run_route_60_0, value), ::std::result::Result::Err(error) => __DucktapeMessage::ExternalUrlFailed(error) }) };
})(),
ForgeIntent::Copy => (|| {
{ let __ice_next = crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("label"))); if ::ui_lang_runtime::state_changed!(self.toast, __ice_next) { self.toast = __ice_next; self.__ice_rev[117] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.toast_age, __ice_next) { self.toast_age = __ice_next; self.__ice_rev[118] += 1; } }
return ::crate::shell::clipboard::<__DucktapeMessage>(crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("text"))));
})(),
};
})(),
__DucktapeMessage::ForgeComposerEvent(scope, body) => (|| {
let _ = &scope;
let _ = &body;
let channel = crate::backend::scope_channel(::std::convert::AsRef::as_ref(&(scope)), ::std::convert::AsRef::as_ref(&(self.connected_rpc)));
return match crate::backend::submit_verdict(self.loading, self.connected, channel.to_owned(), self.forge_note_pending.to_owned(), (!(channel).is_empty()), scope.to_owned(), scope.to_owned()) {
SubmitVerdict::Refused => (|| {
{ let __ice_next = ({ crate::module_view::chat_composer_unsent(::std::convert::AsRef::as_ref(&(scope)), ::std::convert::AsRef::as_ref(&(body)), false) }); if ::ui_lang_runtime::state_changed!(self.composer_stashed, __ice_next) { self.composer_stashed = __ice_next; self.__ice_rev[36] += 1; } }
::ducktape_view_guest::Task::none()
})(),
SubmitVerdict::Admitted => (|| {
let op = ({ crate::backend::fresh_operation_id("forge-note".to_owned()) });
{ let __ice_next = op.to_owned(); if ::ui_lang_runtime::state_changed!(self.forge_note_pending, __ice_next) { self.forge_note_pending = __ice_next; self.__ice_rev[63] += 1; } }
return { let __ice_run_route_62_0 = op.to_owned();
let __ice_run_route_63_0 = scope.to_owned();
let __ice_run_route_63_1 = op.to_owned();
::ducktape_view_guest::Task::perform(({ crate::backend::send_message(self.connected_rpc.to_owned(), self.password.to_owned(), channel.to_owned(), op.to_owned(), (body).trim().to_owned()) }), move |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::ForgeNoteSent(__ice_run_route_62_0, value), ::std::result::Result::Err(error) => __DucktapeMessage::ForgeNoteFailed(__ice_run_route_63_0, __ice_run_route_63_1, error) }) };
})(),
};
})(),
__DucktapeMessage::ForgeNoteSent(op, next) => (|| {
let _ = &op;
let _ = &next;
if (op != self.forge_note_pending) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.forge_note_pending, __ice_next) { self.forge_note_pending = __ice_next; self.__ice_rev[63] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::ForgeNoteFailed(scope, op, cause) => (|| {
let _ = &scope;
let _ = &op;
let _ = &cause;
{ let __ice_next = ({ crate::module_view::chat_composer_unsent(::std::convert::AsRef::as_ref(&(scope)), ::std::convert::AsRef::as_ref(&(cause.body)), cause.committed) }); if ::ui_lang_runtime::state_changed!(self.composer_stashed, __ice_next) { self.composer_stashed = __ice_next; self.__ice_rev[36] += 1; } }
if (op != self.forge_note_pending) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.forge_note_pending, __ice_next) { self.forge_note_pending = __ice_next; self.__ice_rev[63] += 1; } }
{ let __ice_next = cause.message.to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::FilesViewEvent(event) => (|| {
let _ = &event;
{ let __ice_next = crate::backend::keep_str((event.kind == "at"), ::std::convert::AsRef::as_ref(&(crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("path"))))), ::std::convert::AsRef::as_ref(&(self.fs_drop_dir))); if ::ui_lang_runtime::state_changed!(self.fs_drop_dir, __ice_next) { self.fs_drop_dir = __ice_next; self.__ice_rev[100] += 1; } }
if (event.kind != "open_link") { return ::ducktape_view_guest::Task::none(); }
return (::ducktape_view_guest::Task::done(crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("url"))))).map(|value| __DucktapeMessage::OpenMessageLink(value));
})(),
__DucktapeMessage::FsFileDropped(path) => (|| {
let _ = &path;
if (((self.shell_tab != ShellTab::Files) || self.fs_dropping) || (!self.connected)) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = crate::backend::files_write_gate(self.fs_drop_dir.to_owned(), self.settings_user_key.to_owned()); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
if (!(self.error).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.fs_dropping, __ice_next) { self.fs_dropping = __ice_next; self.__ice_rev[101] += 1; } }
return ::ducktape_view_guest::Task::perform(({ crate::backend::files_upload(self.connected_rpc.to_owned(), self.password.to_owned(), self.fs_drop_dir.to_owned(), path.to_owned()) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::FsDropped(value), ::std::result::Result::Err(error) => __DucktapeMessage::FsDropFailed(error) });
})(),
__DucktapeMessage::FsDropped(_result) => (|| {
let _ = &_result;
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.fs_dropping, __ice_next) { self.fs_dropping = __ice_next; self.__ice_rev[101] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::FsDropFailed(cause) => (|| {
let _ = &cause;
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.fs_dropping, __ice_next) { self.fs_dropping = __ice_next; self.__ice_rev[101] += 1; } }
{ let __ice_next = cause.message.to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::AccountLoaded(next) => (|| {
let _ = &next;
if (next.generation != self.account_generation) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = next.exists; if ::ui_lang_runtime::state_changed!(self.account_exists, __ice_next) { self.account_exists = __ice_next; self.__ice_rev[72] += 1; } }
{ let __ice_next = (self.bell_marking && (self.account_number == next.number)); if ::ui_lang_runtime::state_changed!(self.bell_marking, __ice_next) { self.bell_marking = __ice_next; self.__ice_rev[111] += 1; } }
self.bell_items = crate::backend::bell_account_items(::std::mem::take(&mut self.bell_items), ::std::convert::AsRef::as_ref(&(self.account_number)), ::std::convert::AsRef::as_ref(&(next.number))); self.__ice_rev[107] += 1;
self.bell_presentations = crate::backend::merge_bell_presentations(crate::backend::bell_visible_items(::std::convert::AsRef::as_ref(&(self.bell_items)), ::std::convert::AsRef::as_ref(&(next.number)), ::std::convert::AsRef::as_ref(&(self.settings_user_key))), ::std::mem::take(&mut self.bell_presentations), ::std::vec::Vec::new()); self.__ice_rev[108] += 1;
{ let __ice_next = crate::backend::bell_unread_count(::std::convert::AsRef::as_ref(&(self.bell_items)), ::std::convert::AsRef::as_ref(&(next.number)), ::std::convert::AsRef::as_ref(&(self.settings_user_key))); if ::ui_lang_runtime::state_changed!(self.bell_unread, __ice_next) { self.bell_unread = __ice_next; self.__ice_rev[106] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.bell_error, __ice_next) { self.bell_error = __ice_next; self.__ice_rev[112] += 1; } }
{ let __ice_next = crate::backend::keep_i64((self.account_number == next.number), self.bell_read_through, 0); if ::ui_lang_runtime::state_changed!(self.bell_read_through, __ice_next) { self.bell_read_through = __ice_next; self.__ice_rev[109] += 1; } }
{ let __ice_next = crate::backend::keep_i64((self.account_number == next.number), self.bell_clear_through, 0); if ::ui_lang_runtime::state_changed!(self.bell_clear_through, __ice_next) { self.bell_clear_through = __ice_next; self.__ice_rev[110] += 1; } }
self.__ice_run_lane_9_generation = self.__ice_run_lane_9_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_9_handle.take() { __previous.abort(); }
self.__ice_run_lane_13_generation = self.__ice_run_lane_13_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_13_handle.take() { __previous.abort(); }
self.__ice_run_lane_14_generation = self.__ice_run_lane_14_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_14_handle.take() { __previous.abort(); }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.bell_marking, __ice_next) { self.bell_marking = __ice_next; self.__ice_rev[111] += 1; } }
{ let __ice_next = next.number.to_owned(); if ::ui_lang_runtime::state_changed!(self.account_number, __ice_next) { self.account_number = __ice_next; self.__ice_rev[73] += 1; } }
{ let __ice_next = next.name.to_owned(); if ::ui_lang_runtime::state_changed!(self.account_name, __ice_next) { self.account_name = __ice_next; self.__ice_rev[74] += 1; } }
{ let __ice_next = next.bio.to_owned(); if ::ui_lang_runtime::state_changed!(self.account_bio, __ice_next) { self.account_bio = __ice_next; self.__ice_rev[75] += 1; } }
return { let __task = { let __ice_run_route_67_0 = self.connect_generation;
let __ice_run_route_67_1 = next.number.to_owned();
let __ice_run_route_68_0 = self.connect_generation;
let __ice_run_route_68_1 = next.number.to_owned();
::ducktape_view_guest::Task::perform(({ crate::backend::load_bell(self.connected_rpc.to_owned(), self.account_number.to_owned()) }), move |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::BellLoaded(__ice_run_route_67_0, __ice_run_route_67_1, value), ::std::result::Result::Err(error) => __DucktapeMessage::BellFailed(__ice_run_route_68_0, __ice_run_route_68_1, error) }) }; self.__ice_run_lane_4_generation = self.__ice_run_lane_4_generation.wrapping_add(1); let __generation = self.__ice_run_lane_4_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_4_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane4(__generation, ::std::boxed::Box::new(__message))) };
})(),
__DucktapeMessage::AccountFailed(cause) => (|| {
let _ = &cause;
if (cause.generation != self.account_generation) { return ::ducktape_view_guest::Task::none(); }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::AccountRenamed(_result) => (|| {
let _ = &_result;
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = (self.account_generation + 1); if ::ui_lang_runtime::state_changed!(self.account_generation, __ice_next) { self.account_generation = __ice_next; self.__ice_rev[76] += 1; } }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::load_account(self.connected_rpc.to_owned(), self.account_generation) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::AccountLoaded(value), ::std::result::Result::Err(error) => __DucktapeMessage::AccountFailed(error) }); self.__ice_run_lane_7_generation = self.__ice_run_lane_7_generation.wrapping_add(1); let __generation = self.__ice_run_lane_7_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_7_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane7(__generation, ::std::boxed::Box::new(__message))) };
})(),
__DucktapeMessage::AccountRenameFailed(cause) => (|| {
let _ = &cause;
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = cause.message.to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::AccountTicketMinted(ticket) => (|| {
let _ = &ticket;
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = ticket.to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ticket, __ice_next) { self.account_ticket = __ice_next; self.__ice_rev[78] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::AccountCeremonyStepped(next) => (|| {
let _ = &next;
let phase = crate::backend::ceremony_phase(::std::borrow::Borrow::borrow(&(next)));
{ let __ice_next = next.phase.to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_phase, __ice_next) { self.account_ceremony_phase = __ice_next; self.__ice_rev[80] += 1; } }
{ let __ice_next = next.qr.to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_qr, __ice_next) { self.account_ceremony_qr = __ice_next; self.__ice_rev[81] += 1; } }
{ let __ice_next = next.detail.to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_detail, __ice_next) { self.account_ceremony_detail = __ice_next; self.__ice_rev[82] += 1; } }
{ let __ice_next = next.left.to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_left, __ice_next) { self.account_ceremony_left = __ice_next; self.__ice_rev[83] += 1; } }
return match phase.clone() {
CeremonyPhase::Done => (|| {
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_phase, __ice_next) { self.account_ceremony_phase = __ice_next; self.__ice_rev[80] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_qr, __ice_next) { self.account_ceremony_qr = __ice_next; self.__ice_rev[81] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = (self.account_generation + 1); if ::ui_lang_runtime::state_changed!(self.account_generation, __ice_next) { self.account_generation = __ice_next; self.__ice_rev[76] += 1; } }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::load_account(self.connected_rpc.to_owned(), self.account_generation) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::AccountLoaded(value), ::std::result::Result::Err(error) => __DucktapeMessage::AccountFailed(error) }); self.__ice_run_lane_7_generation = self.__ice_run_lane_7_generation.wrapping_add(1); let __generation = self.__ice_run_lane_7_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_7_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane7(__generation, ::std::boxed::Box::new(__message))) };
})(),
CeremonyPhase::Failed => (|| {
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_phase, __ice_next) { self.account_ceremony_phase = __ice_next; self.__ice_rev[80] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_qr, __ice_next) { self.account_ceremony_qr = __ice_next; self.__ice_rev[81] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = next.detail.to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
::ducktape_view_guest::Task::none()
})(),
CeremonyPhase::ShowQr => (|| {
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
::ducktape_view_guest::Task::none()
})(),
CeremonyPhase::Working => (|| {
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
::ducktape_view_guest::Task::none()
})(),
};
})(),
__DucktapeMessage::AccountChanged(_result) => (|| {
let _ = &_result;
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_phase, __ice_next) { self.account_ceremony_phase = __ice_next; self.__ice_rev[80] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_qr, __ice_next) { self.account_ceremony_qr = __ice_next; self.__ice_rev[81] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_detail, __ice_next) { self.account_ceremony_detail = __ice_next; self.__ice_rev[82] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_left, __ice_next) { self.account_ceremony_left = __ice_next; self.__ice_rev[83] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ticket, __ice_next) { self.account_ticket = __ice_next; self.__ice_rev[78] += 1; } }
{ let __ice_next = (self.account_generation + 1); if ::ui_lang_runtime::state_changed!(self.account_generation, __ice_next) { self.account_generation = __ice_next; self.__ice_rev[76] += 1; } }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::load_account(self.connected_rpc.to_owned(), self.account_generation) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::AccountLoaded(value), ::std::result::Result::Err(error) => __DucktapeMessage::AccountFailed(error) }); self.__ice_run_lane_7_generation = self.__ice_run_lane_7_generation.wrapping_add(1); let __generation = self.__ice_run_lane_7_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_7_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane7(__generation, ::std::boxed::Box::new(__message))) };
})(),
__DucktapeMessage::AccountOpFailed(cause) => (|| {
let _ = &cause;
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_phase, __ice_next) { self.account_ceremony_phase = __ice_next; self.__ice_rev[80] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_qr, __ice_next) { self.account_ceremony_qr = __ice_next; self.__ice_rev[81] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_detail, __ice_next) { self.account_ceremony_detail = __ice_next; self.__ice_rev[82] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_left, __ice_next) { self.account_ceremony_left = __ice_next; self.__ice_rev[83] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = cause.message.to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::OpenRunPanel(dispatch_id) => (|| {
let _ = &dispatch_id;
self.__ice_run_lane_10_generation = self.__ice_run_lane_10_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_10_handle.take() { __previous.abort(); }
self.__ice_run_lane_11_generation = self.__ice_run_lane_11_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_11_handle.take() { __previous.abort(); }
{ let __ice_next = (self.account_busy && (self.account_ceremony_phase).is_empty()); if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_phase, __ice_next) { self.account_ceremony_phase = __ice_next; self.__ice_rev[80] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_qr, __ice_next) { self.account_ceremony_qr = __ice_next; self.__ice_rev[81] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_detail, __ice_next) { self.account_ceremony_detail = __ice_next; self.__ice_rev[82] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_left, __ice_next) { self.account_ceremony_left = __ice_next; self.__ice_rev[83] += 1; } }
{ let __ice_next = ShellTab::Agents; if ::ui_lang_runtime::state_changed!(self.shell_tab, __ice_next) { self.shell_tab = __ice_next; self.__ice_rev[55] += 1; } }
{ let __ice_next = dispatch_id.to_owned(); if ::ui_lang_runtime::state_changed!(self.agents_open_run, __ice_next) { self.agents_open_run = __ice_next; self.__ice_rev[60] += 1; } }
{ let __ice_next = (self.agents_opened + 1); if ::ui_lang_runtime::state_changed!(self.agents_opened, __ice_next) { self.agents_opened = __ice_next; self.__ice_rev[61] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::GovernanceViewEvent(event) => (|| {
let _ = &event;
if (event.kind != "badge") { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = crate::module_view::event_int(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("count"))); if ::ui_lang_runtime::state_changed!(self.gov_open, __ice_next) { self.gov_open = __ice_next; self.__ice_rev[59] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::MembersViewEvent(event) => (|| {
let _ = &event;
if ((!self.connected) || (event.kind != "copy")) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("label"))); if ::ui_lang_runtime::state_changed!(self.toast, __ice_next) { self.toast = __ice_next; self.__ice_rev[117] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.toast_age, __ice_next) { self.toast_age = __ice_next; self.__ice_rev[118] += 1; } }
return ::crate::shell::clipboard::<__DucktapeMessage>(crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("text"))));
})(),
__DucktapeMessage::MembersLoaded(next) => (|| {
let _ = &next;
if (next.generation != self.members_generation) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.members_answered, __ice_next) { self.members_answered = __ice_next; self.__ice_rev[56] += 1; } }
{ let __ice_next = next.members.clone(); if ::ui_lang_runtime::state_changed!(self.members_rows, __ice_next) { self.members_rows = __ice_next; self.__ice_rev[57] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::MembersFailed(cause) => (|| {
let _ = &cause;
if (cause.generation != self.members_generation) { return ::ducktape_view_guest::Task::none(); }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::DmPeersLoaded(next) => (|| {
let _ = &next;
if (next.generation != self.dm_peers_generation) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = next.peers.clone(); if ::ui_lang_runtime::state_changed!(self.dm_peers, __ice_next) { self.dm_peers = __ice_next; self.__ice_rev[50] += 1; } }
{ let __ice_next = crate::backend::chat_sidebar_rooms(self.channels.clone(), self.dm_peers.clone(), self.channel_reads.clone()); if ::ui_lang_runtime::state_changed!(self.rooms, __ice_next) { self.rooms = __ice_next; self.__ice_rev[23] += 1; } }
{ let __ice_next = crate::backend::chat_sidebar_dms(self.channels.clone(), self.dm_peers.clone(), self.channel_reads.clone()); if ::ui_lang_runtime::state_changed!(self.dm_rows, __ice_next) { self.dm_rows = __ice_next; self.__ice_rev[51] += 1; } }
{ let __ice_next = crate::backend::dm_peer_named(self.dm_peers.clone(), self.active_dm_peer.to_owned()); if ::ui_lang_runtime::state_changed!(self.active_dm, __ice_next) { self.active_dm = __ice_next; self.__ice_rev[54] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::DmPeersFailed(cause) => (|| {
let _ = &cause;
if (cause.generation != self.dm_peers_generation) { return ::ducktape_view_guest::Task::none(); }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::AgentsViewEvent(event) => (|| {
let _ = &event;
if (!self.connected) { return ::ducktape_view_guest::Task::none(); }
return match crate::module_view::agents_intent(::std::borrow::Borrow::borrow(&(event))) {
AgentsIntent::Badge => (|| {
{ let __ice_next = (crate::module_view::event_int(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("count"))) > 0); if ::ui_lang_runtime::state_changed!(self.agents_live, __ice_next) { self.agents_live = __ice_next; self.__ice_rev[62] += 1; } }
::ducktape_view_guest::Task::none()
})(),
AgentsIntent::Register => (|| {
return ::ducktape_view_guest::Task::perform(({ crate::backend::register_agent(self.connected_rpc.to_owned(), self.password.to_owned(), self.account_number.to_owned(), event.detail.to_owned()) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::AgentStatusSet(value), ::std::result::Result::Err(error) => __DucktapeMessage::MutationFailed(error) });
})(),
AgentsIntent::OpenRun => (|| {
return (::ducktape_view_guest::Task::done(crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("dispatch_id"))))).map(|value| __DucktapeMessage::OpenRunPanel(value));
})(),
AgentsIntent::OpenLink => (|| {
return (::ducktape_view_guest::Task::done(crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("url"))))).map(|value| __DucktapeMessage::OpenMessageLink(value));
})(),
};
})(),
__DucktapeMessage::AgentStatusSet(_result) => (|| {
let _ = &_result;
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::NodeViewEvent(event) => (|| {
let _ = &event;
if (event.kind != "copy") { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("label"))); if ::ui_lang_runtime::state_changed!(self.toast, __ice_next) { self.toast = __ice_next; self.__ice_rev[117] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.toast_age, __ice_next) { self.toast_age = __ice_next; self.__ice_rev[118] += 1; } }
return ::crate::shell::clipboard::<__DucktapeMessage>(crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("text"))));
})(),
__DucktapeMessage::NodeFactsLoaded(next) => (|| {
let _ = &next;
{ let __ice_next = next.public_key.to_owned(); if ::ui_lang_runtime::state_changed!(self.node_key, __ice_next) { self.node_key = __ice_next; self.__ice_rev[66] += 1; } }
{ let __ice_next = next.version.to_owned(); if ::ui_lang_runtime::state_changed!(self.node_version, __ice_next) { self.node_version = __ice_next; self.__ice_rev[84] += 1; } }
{ let __ice_next = next.root_hash.to_owned(); if ::ui_lang_runtime::state_changed!(self.node_root_hash, __ice_next) { self.node_root_hash = __ice_next; self.__ice_rev[85] += 1; } }
{ let __ice_next = next.chain_id.to_owned(); if ::ui_lang_runtime::state_changed!(self.network_chain_id, __ice_next) { self.network_chain_id = __ice_next; self.__ice_rev[86] += 1; } }
{ let __ice_next = crate::backend::network_label(self.network_chain_id.to_owned(), self.connected_rpc.to_owned()); if ::ui_lang_runtime::state_changed!(self.network_name, __ice_next) { self.network_name = __ice_next; self.__ice_rev[124] += 1; } }
{ let __ice_next = next.last_finalized_at; if ::ui_lang_runtime::state_changed!(self.node_last_finalized, __ice_next) { self.node_last_finalized = __ice_next; self.__ice_rev[87] += 1; } }
{ let __ice_next = next.checkpoint_height; if ::ui_lang_runtime::state_changed!(self.node_checkpoint, __ice_next) { self.node_checkpoint = __ice_next; self.__ice_rev[88] += 1; } }
{ let __ice_next = next.height; if ::ui_lang_runtime::state_changed!(self.node_height, __ice_next) { self.node_height = __ice_next; self.__ice_rev[89] += 1; } }
{ let __ice_next = crate::backend::optional_number(next.view.clone()); if ::ui_lang_runtime::state_changed!(self.node_view_label, __ice_next) { self.node_view_label = __ice_next; self.__ice_rev[97] += 1; } }
{ let __ice_next = crate::backend::optional_number(next.quorum.clone()); if ::ui_lang_runtime::state_changed!(self.node_quorum_label, __ice_next) { self.node_quorum_label = __ice_next; self.__ice_rev[98] += 1; } }
{ let __ice_next = crate::backend::optional_number(next.reachable_validators.clone()); if ::ui_lang_runtime::state_changed!(self.node_reachable_label, __ice_next) { self.node_reachable_label = __ice_next; self.__ice_rev[99] += 1; } }
{ let __ice_next = next.phase.to_owned(); if ::ui_lang_runtime::state_changed!(self.node_phase, __ice_next) { self.node_phase = __ice_next; self.__ice_rev[90] += 1; } }
{ let __ice_next = next.phase_since; if ::ui_lang_runtime::state_changed!(self.node_phase_since, __ice_next) { self.node_phase_since = __ice_next; self.__ice_rev[91] += 1; } }
{ let __ice_next = next.sync_target; if ::ui_lang_runtime::state_changed!(self.node_sync_target, __ice_next) { self.node_sync_target = __ice_next; self.__ice_rev[92] += 1; } }
{ let __ice_next = next.sync_applied; if ::ui_lang_runtime::state_changed!(self.node_sync_applied, __ice_next) { self.node_sync_applied = __ice_next; self.__ice_rev[93] += 1; } }
{ let __ice_next = next.sync_retries; if ::ui_lang_runtime::state_changed!(self.node_sync_retries, __ice_next) { self.node_sync_retries = __ice_next; self.__ice_rev[94] += 1; } }
{ let __ice_next = next.sync_failures; if ::ui_lang_runtime::state_changed!(self.node_sync_failures, __ice_next) { self.node_sync_failures = __ice_next; self.__ice_rev[95] += 1; } }
{ let __ice_next = next.sync_last_error.to_owned(); if ::ui_lang_runtime::state_changed!(self.node_sync_last_error, __ice_next) { self.node_sync_last_error = __ice_next; self.__ice_rev[96] += 1; } }
let launch_link = self.startup_duck_link.to_owned();
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.startup_duck_link, __ice_next) { self.startup_duck_link = __ice_next; self.__ice_rev[21] += 1; } }
if (launch_link).is_empty() { return ::ducktape_view_guest::Task::none(); }
return ::ducktape_view_guest::Task::perform(({ crate::backend::duck_echo_str(launch_link.to_owned()) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::OpenMessageLink(value), ::std::result::Result::Err(error) => __DucktapeMessage::ExternalUrlFailed(error) });
})(),
__DucktapeMessage::NodeFactsFailed(_cause) => (|| {
let _ = &_cause;
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::NodeStatusPushed(next) => (|| {
let _ = &next;
{ let __ice_next = next.public_key.to_owned(); if ::ui_lang_runtime::state_changed!(self.node_key, __ice_next) { self.node_key = __ice_next; self.__ice_rev[66] += 1; } }
{ let __ice_next = next.version.to_owned(); if ::ui_lang_runtime::state_changed!(self.node_version, __ice_next) { self.node_version = __ice_next; self.__ice_rev[84] += 1; } }
{ let __ice_next = next.root_hash.to_owned(); if ::ui_lang_runtime::state_changed!(self.node_root_hash, __ice_next) { self.node_root_hash = __ice_next; self.__ice_rev[85] += 1; } }
{ let __ice_next = next.chain_id.to_owned(); if ::ui_lang_runtime::state_changed!(self.network_chain_id, __ice_next) { self.network_chain_id = __ice_next; self.__ice_rev[86] += 1; } }
{ let __ice_next = crate::backend::network_label(self.network_chain_id.to_owned(), self.connected_rpc.to_owned()); if ::ui_lang_runtime::state_changed!(self.network_name, __ice_next) { self.network_name = __ice_next; self.__ice_rev[124] += 1; } }
{ let __ice_next = next.last_finalized_at; if ::ui_lang_runtime::state_changed!(self.node_last_finalized, __ice_next) { self.node_last_finalized = __ice_next; self.__ice_rev[87] += 1; } }
{ let __ice_next = next.checkpoint_height; if ::ui_lang_runtime::state_changed!(self.node_checkpoint, __ice_next) { self.node_checkpoint = __ice_next; self.__ice_rev[88] += 1; } }
{ let __ice_next = next.height; if ::ui_lang_runtime::state_changed!(self.node_height, __ice_next) { self.node_height = __ice_next; self.__ice_rev[89] += 1; } }
{ let __ice_next = crate::backend::optional_number(next.view.clone()); if ::ui_lang_runtime::state_changed!(self.node_view_label, __ice_next) { self.node_view_label = __ice_next; self.__ice_rev[97] += 1; } }
{ let __ice_next = crate::backend::optional_number(next.quorum.clone()); if ::ui_lang_runtime::state_changed!(self.node_quorum_label, __ice_next) { self.node_quorum_label = __ice_next; self.__ice_rev[98] += 1; } }
{ let __ice_next = crate::backend::optional_number(next.reachable_validators.clone()); if ::ui_lang_runtime::state_changed!(self.node_reachable_label, __ice_next) { self.node_reachable_label = __ice_next; self.__ice_rev[99] += 1; } }
{ let __ice_next = next.phase.to_owned(); if ::ui_lang_runtime::state_changed!(self.node_phase, __ice_next) { self.node_phase = __ice_next; self.__ice_rev[90] += 1; } }
{ let __ice_next = next.phase_since; if ::ui_lang_runtime::state_changed!(self.node_phase_since, __ice_next) { self.node_phase_since = __ice_next; self.__ice_rev[91] += 1; } }
{ let __ice_next = next.sync_target; if ::ui_lang_runtime::state_changed!(self.node_sync_target, __ice_next) { self.node_sync_target = __ice_next; self.__ice_rev[92] += 1; } }
{ let __ice_next = next.sync_applied; if ::ui_lang_runtime::state_changed!(self.node_sync_applied, __ice_next) { self.node_sync_applied = __ice_next; self.__ice_rev[93] += 1; } }
{ let __ice_next = next.sync_retries; if ::ui_lang_runtime::state_changed!(self.node_sync_retries, __ice_next) { self.node_sync_retries = __ice_next; self.__ice_rev[94] += 1; } }
{ let __ice_next = next.sync_failures; if ::ui_lang_runtime::state_changed!(self.node_sync_failures, __ice_next) { self.node_sync_failures = __ice_next; self.__ice_rev[95] += 1; } }
{ let __ice_next = next.sync_last_error.to_owned(); if ::ui_lang_runtime::state_changed!(self.node_sync_last_error, __ice_next) { self.node_sync_last_error = __ice_next; self.__ice_rev[96] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::SettingsLoaded(next) => (|| {
let _ = &next;
if (next.generation != self.settings_generation) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = next.data_dir.to_owned(); if ::ui_lang_runtime::state_changed!(self.node_data_dir, __ice_next) { self.node_data_dir = __ice_next; self.__ice_rev[67] += 1; } }
{ let __ice_next = next.key_path.to_owned(); if ::ui_lang_runtime::state_changed!(self.settings_key_path, __ice_next) { self.settings_key_path = __ice_next; self.__ice_rev[68] += 1; } }
{ let __ice_next = next.key_state.to_owned(); if ::ui_lang_runtime::state_changed!(self.settings_key_state, __ice_next) { self.settings_key_state = __ice_next; self.__ice_rev[69] += 1; } }
{ let __ice_next = next.user_key.to_owned(); if ::ui_lang_runtime::state_changed!(self.settings_user_key, __ice_next) { self.settings_user_key = __ice_next; self.__ice_rev[70] += 1; } }
{ let __ice_next = crate::backend::post_gate(self.active_channel_archived, self.active_channel_members_only, self.channel_members.clone(), self.settings_user_key.to_owned()); if ::ui_lang_runtime::state_changed!(self.post_refusal, __ice_next) { self.post_refusal = __ice_next; self.__ice_rev[32] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::SettingsFailed(cause) => (|| {
let _ = &cause;
if (cause.generation != self.settings_generation) { return ::ducktape_view_guest::Task::none(); }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::SettingsViewEvent(event) => (|| {
let _ = &event;
return match crate::module_view::settings_intent(::std::borrow::Borrow::borrow(&(event))) {
SettingsIntent::Tab => (|| {
return (::ducktape_view_guest::Task::done(crate::module_view::settings_event_tab(::std::borrow::Borrow::borrow(&(event))))).map(|value| __DucktapeMessage::SelectShellTab(value));
})(),
SettingsIntent::Reconnect => (|| {
return (::ducktape_view_guest::Task::done(true)).map(|value| { let _ = &value; __DucktapeMessage::Reconnect });
})(),
SettingsIntent::SwitchNetwork => (|| {
return (::ducktape_view_guest::Task::done(true)).map(|value| { let _ = &value; __DucktapeMessage::SwitchNetwork });
})(),
SettingsIntent::Unlock => (|| {
if ((self.mutation_phase != MutationPhase::Idle) || (crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("password")))).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
{ let __ice_next = crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("password"))); if ::ui_lang_runtime::state_changed!(self.password, __ice_next) { self.password = __ice_next; self.__ice_rev[6] += 1; } }
return ::ducktape_view_guest::Task::perform(({ crate::backend::unlock_user_key(self.connected_rpc.to_owned(), self.password.to_owned()) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::SettingsUnlocked(value), ::std::result::Result::Err(error) => __DucktapeMessage::SettingsUnlockFailed(error) });
})(),
SettingsIntent::Lock => (|| {
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.password, __ice_next) { self.password = __ice_next; self.__ice_rev[6] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.signer_key, __ice_next) { self.signer_key = __ice_next; self.__ice_rev[17] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.live_agents, __ice_next) { self.live_agents = __ice_next; self.__ice_rev[39] += 1; } }
return (::ducktape_view_guest::Task::perform(({ crate::backend::lock_signer() }), |value| value)).discard::<__DucktapeMessage>();
})(),
SettingsIntent::Rename => (|| {
if ((((!self.connected) || (!self.account_exists)) || self.account_busy) || (crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("name")))).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
return ::ducktape_view_guest::Task::perform(({ crate::backend::set_account_name(self.connected_rpc.to_owned(), self.password.to_owned(), crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("name")))) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::AccountRenamed(value), ::std::result::Result::Err(error) => __DucktapeMessage::AccountRenameFailed(error) });
})(),
SettingsIntent::Create => (|| {
if (((((!self.connected) || self.account_exists) || self.account_busy) || (self.password).is_empty()) || (crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("name")))).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
return ::ducktape_view_guest::Task::perform(({ crate::backend::create_account(self.connected_rpc.to_owned(), self.password.to_owned(), crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("name")))) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::AccountChanged(value), ::std::result::Result::Err(error) => __DucktapeMessage::AccountOpFailed(error) });
})(),
SettingsIntent::KeyAdd => (|| {
if (((((!self.connected) || (!self.account_exists)) || self.account_busy) || (self.password).is_empty()) || (crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("pubkey")))).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ticket, __ice_next) { self.account_ticket = __ice_next; self.__ice_rev[78] += 1; } }
return ::ducktape_view_guest::Task::perform(({ crate::backend::mint_key_ticket(self.connected_rpc.to_owned(), self.password.to_owned(), self.network_chain_id.to_owned(), crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("pubkey"))), crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("label")))) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::AccountTicketMinted(value), ::std::result::Result::Err(error) => __DucktapeMessage::AccountOpFailed(error) });
})(),
SettingsIntent::Join => (|| {
if ((((!self.connected) || self.account_busy) || (self.password).is_empty()) || (crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("ticket")))).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
return ::ducktape_view_guest::Task::perform(({ crate::backend::join_with_ticket(self.connected_rpc.to_owned(), self.password.to_owned(), crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("ticket")))) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::AccountChanged(value), ::std::result::Result::Err(error) => __DucktapeMessage::AccountOpFailed(error) });
})(),
SettingsIntent::KeyRemove => (|| {
if ((((!self.connected) || (!self.account_exists)) || self.account_busy) || (self.password).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
return ::ducktape_view_guest::Task::perform(({ crate::backend::remove_account_key(self.connected_rpc.to_owned(), self.password.to_owned(), crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("pubkey")))) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::AccountChanged(value), ::std::result::Result::Err(error) => __DucktapeMessage::AccountOpFailed(error) });
})(),
SettingsIntent::Passkey => (|| {
if ((((!self.connected) || (!self.account_exists)) || self.account_busy) || (self.password).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
{ let __ice_next = "working".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_phase, __ice_next) { self.account_ceremony_phase = __ice_next; self.__ice_rev[80] += 1; } }
{ let __ice_next = "Preparing the passkey…".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_detail, __ice_next) { self.account_ceremony_detail = __ice_next; self.__ice_rev[82] += 1; } }
return { let __task = ::ducktape_view_guest::Task::run(({ crate::backend::add_passkey_by_qr(self.connected_rpc.to_owned(), self.password.to_owned(), self.network_chain_id.to_owned(), crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("label")))) }), |value| __DucktapeMessage::AccountCeremonyStepped(value)); self.__ice_run_lane_10_generation = self.__ice_run_lane_10_generation.wrapping_add(1); let __generation = self.__ice_run_lane_10_generation; let __task = __task.map(move |__message| __DucktapeMessage::__RequestLane10(__generation, ::std::option::Option::Some(::std::boxed::Box::new(__message)))).chain(::ducktape_view_guest::Task::done(__DucktapeMessage::__RequestLane10(__generation, ::std::option::Option::None))); let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_10_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task };
})(),
SettingsIntent::PasskeyDesktop => (|| {
if ((((!self.connected) || (!self.account_exists)) || self.account_busy) || (self.password).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
{ let __ice_next = "working".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_phase, __ice_next) { self.account_ceremony_phase = __ice_next; self.__ice_rev[80] += 1; } }
{ let __ice_next = "Continue in the browser…".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_detail, __ice_next) { self.account_ceremony_detail = __ice_next; self.__ice_rev[82] += 1; } }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::register_passkey(self.connected_rpc.to_owned(), self.password.to_owned(), self.network_chain_id.to_owned(), crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("label")))) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::AccountChanged(value), ::std::result::Result::Err(error) => __DucktapeMessage::AccountOpFailed(error) }); self.__ice_run_lane_11_generation = self.__ice_run_lane_11_generation.wrapping_add(1); let __generation = self.__ice_run_lane_11_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_11_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane11(__generation, ::std::boxed::Box::new(__message))) };
})(),
SettingsIntent::CeremonyCancel => (|| {
self.__ice_run_lane_10_generation = self.__ice_run_lane_10_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_10_handle.take() { __previous.abort(); }
self.__ice_run_lane_11_generation = self.__ice_run_lane_11_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_11_handle.take() { __previous.abort(); }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_phase, __ice_next) { self.account_ceremony_phase = __ice_next; self.__ice_rev[80] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_qr, __ice_next) { self.account_ceremony_qr = __ice_next; self.__ice_rev[81] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_detail, __ice_next) { self.account_ceremony_detail = __ice_next; self.__ice_rev[82] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_left, __ice_next) { self.account_ceremony_left = __ice_next; self.__ice_rev[83] += 1; } }
::ducktape_view_guest::Task::none()
})(),
SettingsIntent::Wallet => (|| {
if ((((!self.connected) || (!self.account_exists)) || self.account_busy) || (self.password).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
{ let __ice_next = "working".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_phase, __ice_next) { self.account_ceremony_phase = __ice_next; self.__ice_rev[80] += 1; } }
{ let __ice_next = "Continue in the browser…".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_detail, __ice_next) { self.account_ceremony_detail = __ice_next; self.__ice_rev[82] += 1; } }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::link_wallet(self.connected_rpc.to_owned(), self.password.to_owned(), self.network_chain_id.to_owned(), crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("label")))) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::AccountChanged(value), ::std::result::Result::Err(error) => __DucktapeMessage::AccountOpFailed(error) }); self.__ice_run_lane_11_generation = self.__ice_run_lane_11_generation.wrapping_add(1); let __generation = self.__ice_run_lane_11_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_11_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane11(__generation, ::std::boxed::Box::new(__message))) };
})(),
SettingsIntent::Login => (|| {
if ((((!self.connected) || self.account_exists) || self.account_busy) || (self.password).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
{ let __ice_next = "working".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_phase, __ice_next) { self.account_ceremony_phase = __ice_next; self.__ice_rev[80] += 1; } }
{ let __ice_next = "Continue in the browser…".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_detail, __ice_next) { self.account_ceremony_detail = __ice_next; self.__ice_rev[82] += 1; } }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::login_with_passkey(self.connected_rpc.to_owned(), self.password.to_owned(), self.network_chain_id.to_owned(), "".to_owned()) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::AccountChanged(value), ::std::result::Result::Err(error) => __DucktapeMessage::AccountOpFailed(error) }); self.__ice_run_lane_11_generation = self.__ice_run_lane_11_generation.wrapping_add(1); let __generation = self.__ice_run_lane_11_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_11_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane11(__generation, ::std::boxed::Box::new(__message))) };
})(),
SettingsIntent::Copy => (|| {
{ let __ice_next = crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("label"))); if ::ui_lang_runtime::state_changed!(self.toast, __ice_next) { self.toast = __ice_next; self.__ice_rev[117] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.toast_age, __ice_next) { self.toast_age = __ice_next; self.__ice_rev[118] += 1; } }
return ::crate::shell::clipboard::<__DucktapeMessage>(crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("text"))));
})(),
SettingsIntent::Light => (|| {
return (::ducktape_view_guest::Task::done(true)).map(|value| { let _ = &value; __DucktapeMessage::SetAppearanceLight });
})(),
SettingsIntent::Dark => (|| {
return (::ducktape_view_guest::Task::done(true)).map(|value| { let _ = &value; __DucktapeMessage::SetAppearanceDark });
})(),
SettingsIntent::Notifications => (|| {
{ let __ice_next = crate::module_view::event_flag(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("enabled"))); if ::ui_lang_runtime::state_changed!(self.desktop_notifications, __ice_next) { self.desktop_notifications = __ice_next; self.__ice_rev[2] += 1; } }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::save_desktop_notifications(self.desktop_notifications) }), |value| __DucktapeMessage::DesktopNotificationsSaved(value)); self.__ice_run_lane_12_generation = self.__ice_run_lane_12_generation.wrapping_add(1); let __generation = self.__ice_run_lane_12_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_12_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane12(__generation, ::std::boxed::Box::new(__message))) };
})(),
};
})(),
__DucktapeMessage::SettingsUnlocked(pubkey) => (|| {
let _ = &pubkey;
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
{ let __ice_next = pubkey.to_owned(); if ::ui_lang_runtime::state_changed!(self.signer_key, __ice_next) { self.signer_key = __ice_next; self.__ice_rev[17] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.live_agents, __ice_next) { self.live_agents = __ice_next; self.__ice_rev[39] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::SettingsUnlockFailed(cause) => (|| {
let _ = &cause;
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.password, __ice_next) { self.password = __ice_next; self.__ice_rev[6] += 1; } }
{ let __ice_next = cause.message.to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::CopyToClipboard(text, label) => (|| {
let _ = &text;
let _ = &label;
{ let __ice_next = label.to_owned(); if ::ui_lang_runtime::state_changed!(self.toast, __ice_next) { self.toast = __ice_next; self.__ice_rev[117] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.toast_age, __ice_next) { self.toast_age = __ice_next; self.__ice_rev[118] += 1; } }
return ::crate::shell::clipboard::<__DucktapeMessage>(text.to_owned());
})(),
__DucktapeMessage::DismissToast => (|| {
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.toast, __ice_next) { self.toast = __ice_next; self.__ice_rev[117] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.toast_age, __ice_next) { self.toast_age = __ice_next; self.__ice_rev[118] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::ToastTick => (|| {
{ let __ice_next = (self.toast_age + 1); if ::ui_lang_runtime::state_changed!(self.toast_age, __ice_next) { self.toast_age = __ice_next; self.__ice_rev[118] += 1; } }
if (self.toast_age < 9) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.toast, __ice_next) { self.toast = __ice_next; self.__ice_rev[117] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.toast_age, __ice_next) { self.toast_age = __ice_next; self.__ice_rev[118] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::ExplorerViewEvent(event) => (|| {
let _ = &event;
if (event.kind != "copy") { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("label"))); if ::ui_lang_runtime::state_changed!(self.toast, __ice_next) { self.toast = __ice_next; self.__ice_rev[117] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.toast_age, __ice_next) { self.toast_age = __ice_next; self.__ice_rev[118] += 1; } }
return ::crate::shell::clipboard::<__DucktapeMessage>(crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("text"))));
})(),
__DucktapeMessage::ClosePalette => (|| {
self.__ice_run_lane_15_generation = self.__ice_run_lane_15_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_15_handle.take() { __previous.abort(); }
{ let __ice_next = SearchPhase::Idle; if ::ui_lang_runtime::state_changed!(self.palette_search_phase, __ice_next) { self.palette_search_phase = __ice_next; self.__ice_rev[114] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.palette_open, __ice_next) { self.palette_open = __ice_next; self.__ice_rev[104] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::ToggleBell => (|| {
{ let __ice_next = (!self.bell_open); if ::ui_lang_runtime::state_changed!(self.bell_open, __ice_next) { self.bell_open = __ice_next; self.__ice_rev[105] += 1; } }
if (!self.bell_open) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.bell_error, __ice_next) { self.bell_error = __ice_next; self.__ice_rev[112] += 1; } }
return { let __task = { let __ice_run_route_106_0 = self.connect_generation;
let __ice_run_route_106_1 = self.account_number.to_owned();
let __ice_run_route_107_0 = self.connect_generation;
let __ice_run_route_107_1 = self.account_number.to_owned();
::ducktape_view_guest::Task::perform(({ crate::backend::load_bell(self.connected_rpc.to_owned(), self.account_number.to_owned()) }), move |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::BellLoaded(__ice_run_route_106_0, __ice_run_route_106_1, value), ::std::result::Result::Err(error) => __DucktapeMessage::BellFailed(__ice_run_route_107_0, __ice_run_route_107_1, error) }) }; self.__ice_run_lane_4_generation = self.__ice_run_lane_4_generation.wrapping_add(1); let __generation = self.__ice_run_lane_4_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_4_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane4(__generation, ::std::boxed::Box::new(__message))) };
})(),
__DucktapeMessage::ReloadBell => (|| {
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.bell_error, __ice_next) { self.bell_error = __ice_next; self.__ice_rev[112] += 1; } }
return { let __task = { let __ice_run_route_108_0 = self.connect_generation;
let __ice_run_route_108_1 = self.account_number.to_owned();
let __ice_run_route_109_0 = self.connect_generation;
let __ice_run_route_109_1 = self.account_number.to_owned();
::ducktape_view_guest::Task::perform(({ crate::backend::load_bell(self.connected_rpc.to_owned(), self.account_number.to_owned()) }), move |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::BellLoaded(__ice_run_route_108_0, __ice_run_route_108_1, value), ::std::result::Result::Err(error) => __DucktapeMessage::BellFailed(__ice_run_route_109_0, __ice_run_route_109_1, error) }) }; self.__ice_run_lane_4_generation = self.__ice_run_lane_4_generation.wrapping_add(1); let __generation = self.__ice_run_lane_4_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_4_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane4(__generation, ::std::boxed::Box::new(__message))) };
})(),
__DucktapeMessage::CloseBell => (|| {
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.bell_open, __ice_next) { self.bell_open = __ice_next; self.__ice_rev[105] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::MarkBellReadSubmit => (|| {
if ((self.bell_unread <= 0) || self.bell_marking) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.bell_marking, __ice_next) { self.bell_marking = __ice_next; self.__ice_rev[111] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.bell_error, __ice_next) { self.bell_error = __ice_next; self.__ice_rev[112] += 1; } }
return { let __task = { let __ice_run_route_110_0 = self.connect_generation;
let __ice_run_route_110_1 = self.account_number.to_owned();
let __ice_run_route_111_0 = self.connect_generation;
let __ice_run_route_111_1 = self.account_number.to_owned();
::ducktape_view_guest::Task::perform(({ crate::backend::mark_bell_read(self.connected_rpc.to_owned(), self.password.to_owned(), self.account_number.to_owned(), crate::backend::bell_head(self.bell_items.clone())) }), move |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::BellMarked(__ice_run_route_110_0, __ice_run_route_110_1, value), ::std::result::Result::Err(error) => __DucktapeMessage::BellMarkFailed(__ice_run_route_111_0, __ice_run_route_111_1, error) }) }; self.__ice_run_lane_13_generation = self.__ice_run_lane_13_generation.wrapping_add(1); let __generation = self.__ice_run_lane_13_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_13_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane13(__generation, ::std::boxed::Box::new(__message))) };
})(),
__DucktapeMessage::BellLoaded(generation, account, next) => (|| {
let _ = &generation;
let _ = &account;
let _ = &next;
if ((generation != self.connect_generation) || (account != self.account_number)) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.bell_error, __ice_next) { self.bell_error = __ice_next; self.__ice_rev[112] += 1; } }
self.bell_items = crate::backend::merge_bell_loaded(::std::mem::take(&mut self.bell_items), next.items.clone(), self.bell_read_through, self.bell_clear_through); self.__ice_rev[107] += 1;
self.bell_presentations = crate::backend::merge_bell_presentations(crate::backend::bell_visible_items(::std::convert::AsRef::as_ref(&(self.bell_items)), ::std::convert::AsRef::as_ref(&(self.account_number)), ::std::convert::AsRef::as_ref(&(self.settings_user_key))), ::std::mem::take(&mut self.bell_presentations), next.presentations.clone()); self.__ice_rev[108] += 1;
{ let __ice_next = crate::backend::bell_unread_count(::std::convert::AsRef::as_ref(&(self.bell_items)), ::std::convert::AsRef::as_ref(&(self.account_number)), ::std::convert::AsRef::as_ref(&(self.settings_user_key))); if ::ui_lang_runtime::state_changed!(self.bell_unread, __ice_next) { self.bell_unread = __ice_next; self.__ice_rev[106] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::BellContextLoaded(generation, account, next) => (|| {
let _ = &generation;
let _ = &account;
let _ = &next;
if ((generation != self.connect_generation) || (account != self.account_number)) { return ::ducktape_view_guest::Task::none(); }
self.bell_presentations = crate::backend::merge_bell_presentations(crate::backend::bell_visible_items(::std::convert::AsRef::as_ref(&(self.bell_items)), ::std::convert::AsRef::as_ref(&(self.account_number)), ::std::convert::AsRef::as_ref(&(self.settings_user_key))), ::std::mem::take(&mut self.bell_presentations), next.clone()); self.__ice_rev[108] += 1;
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.bell_error, __ice_next) { self.bell_error = __ice_next; self.__ice_rev[112] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::BellFailed(generation, account, cause) => (|| {
let _ = &generation;
let _ = &account;
let _ = &cause;
if ((generation != self.connect_generation) || (account != self.account_number)) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = cause.message.to_owned(); if ::ui_lang_runtime::state_changed!(self.bell_error, __ice_next) { self.bell_error = __ice_next; self.__ice_rev[112] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::BellMarked(generation, account, delta) => (|| {
let _ = &generation;
let _ = &account;
let _ = &delta;
if ((generation != self.connect_generation) || (account != self.account_number)) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.bell_marking, __ice_next) { self.bell_marking = __ice_next; self.__ice_rev[111] += 1; } }
{ let __ice_next = crate::backend::keep_i64((delta.up_to_seq > self.bell_read_through), delta.up_to_seq, self.bell_read_through); if ::ui_lang_runtime::state_changed!(self.bell_read_through, __ice_next) { self.bell_read_through = __ice_next; self.__ice_rev[109] += 1; } }
self.bell_items = crate::backend::apply_bell(::std::mem::take(&mut self.bell_items), delta.clone()); self.__ice_rev[107] += 1;
{ let __ice_next = crate::backend::bell_unread_count(::std::convert::AsRef::as_ref(&(self.bell_items)), ::std::convert::AsRef::as_ref(&(self.account_number)), ::std::convert::AsRef::as_ref(&(self.settings_user_key))); if ::ui_lang_runtime::state_changed!(self.bell_unread, __ice_next) { self.bell_unread = __ice_next; self.__ice_rev[106] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::BellMarkFailed(generation, account, cause) => (|| {
let _ = &generation;
let _ = &account;
let _ = &cause;
if ((generation != self.connect_generation) || (account != self.account_number)) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.bell_marking, __ice_next) { self.bell_marking = __ice_next; self.__ice_rev[111] += 1; } }
{ let __ice_next = cause.message.to_owned(); if ::ui_lang_runtime::state_changed!(self.bell_error, __ice_next) { self.bell_error = __ice_next; self.__ice_rev[112] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::BellOpenItem(generation, account, context) => (|| {
let _ = &generation;
let _ = &account;
let _ = &context;
if ((generation != self.connect_generation) || (account != self.account_number)) { return ::ducktape_view_guest::Task::none(); }
if ((context.target == BellTarget::Unavailable) || (context.object).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.bell_open, __ice_next) { self.bell_open = __ice_next; self.__ice_rev[105] += 1; } }
return match context.target.clone() {
BellTarget::Run => (|| {
return (::ducktape_view_guest::Task::done(context.object.to_owned())).map(|value| __DucktapeMessage::OpenRunPanel(value));
})(),
BellTarget::Page => (|| {
return { let __task = { let __ice_run_route_113_1 = context.anchor.to_owned();
::ducktape_view_guest::Task::perform(({ crate::backend::duck_echo_str(context.object.to_owned()) }), move |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::OpenPageSearchHit(value, __ice_run_route_113_1), ::std::result::Result::Err(error) => __DucktapeMessage::ExternalUrlFailed(error) }) }; self.__ice_run_lane_14_generation = self.__ice_run_lane_14_generation.wrapping_add(1); let __generation = self.__ice_run_lane_14_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_14_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane14(__generation, ::std::boxed::Box::new(__message))) };
})(),
BellTarget::Message => (|| {
return (::ducktape_view_guest::Task::done(crate::backend::bell_link(::std::borrow::Borrow::borrow(&(context)), self.network_chain_id.to_owned()))).map(|value| __DucktapeMessage::OpenMessageLink(value));
})(),
BellTarget::Forge => (|| {
return (::ducktape_view_guest::Task::done(crate::backend::bell_link(::std::borrow::Borrow::borrow(&(context)), self.network_chain_id.to_owned()))).map(|value| __DucktapeMessage::OpenMessageLink(value));
})(),
BellTarget::Repo => (|| {
return (::ducktape_view_guest::Task::done(crate::backend::bell_link(::std::borrow::Borrow::borrow(&(context)), self.network_chain_id.to_owned()))).map(|value| __DucktapeMessage::OpenMessageLink(value));
})(),
BellTarget::Unavailable => (|| {
if true { return ::ducktape_view_guest::Task::none(); }
::ducktape_view_guest::Task::none()
})(),
};
})(),
__DucktapeMessage::GlobalKeyPressed(event) => (|| {
let _ = &event;
let escape_key = crate::backend::escape_target(event.key.clone(), self.palette_open, self.bell_open, self.channel_create_open);
let palette_key = crate::backend::palette_key_action(event.key.clone(), event.modifiers, self.palette_open);
if ((escape_key).is_empty() && (palette_key == "none")) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = (self.bell_open && (escape_key != "bell")); if ::ui_lang_runtime::state_changed!(self.bell_open, __ice_next) { self.bell_open = __ice_next; self.__ice_rev[105] += 1; } }
{ let __ice_next = (self.channel_create_open && (escape_key != "channel_create")); if ::ui_lang_runtime::state_changed!(self.channel_create_open, __ice_next) { self.channel_create_open = __ice_next; self.__ice_rev[44] += 1; } }
if (palette_key == "none") { return ::ducktape_view_guest::Task::none(); }
if ((palette_key == "open") && (!self.connected)) { return ::ducktape_view_guest::Task::none(); }
self.__ice_run_lane_15_generation = self.__ice_run_lane_15_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_15_handle.take() { __previous.abort(); }
{ let __ice_next = (palette_key == "open"); if ::ui_lang_runtime::state_changed!(self.palette_open, __ice_next) { self.palette_open = __ice_next; self.__ice_rev[104] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.palette_draft, __ice_next) { self.palette_draft = __ice_next; self.__ice_rev[113] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.palette_chat_hits, __ice_next) { self.palette_chat_hits = __ice_next; self.__ice_rev[115] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.palette_page_hits, __ice_next) { self.palette_page_hits = __ice_next; self.__ice_rev[116] += 1; } }
{ let __ice_next = SearchPhase::Idle; if ::ui_lang_runtime::state_changed!(self.palette_search_phase, __ice_next) { self.palette_search_phase = __ice_next; self.__ice_rev[114] += 1; } }
if (!self.palette_open) { return ::ducktape_view_guest::Task::none(); }
return ::crate::shell::focus::<__DucktapeMessage>("Ducktape/workspace-tabs/overlays/palette-input".to_owned());
})(),
__DucktapeMessage::PaletteChanged(next) => (|| {
let _ = &next;
self.__ice_run_lane_15_generation = self.__ice_run_lane_15_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_15_handle.take() { __previous.abort(); }
{ let __ice_next = next.to_owned(); if ::ui_lang_runtime::state_changed!(self.palette_draft, __ice_next) { self.palette_draft = __ice_next; self.__ice_rev[113] += 1; } }
{ let __ice_next = SearchPhase::Idle; if ::ui_lang_runtime::state_changed!(self.palette_search_phase, __ice_next) { self.palette_search_phase = __ice_next; self.__ice_rev[114] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.palette_chat_hits, __ice_next) { self.palette_chat_hits = __ice_next; self.__ice_rev[115] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.palette_page_hits, __ice_next) { self.palette_page_hits = __ice_next; self.__ice_rev[116] += 1; } }
if ((self.palette_draft).trim().to_owned()).is_empty() { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = SearchPhase::Searching; if ::ui_lang_runtime::state_changed!(self.palette_search_phase, __ice_next) { self.palette_search_phase = __ice_next; self.__ice_rev[114] += 1; } }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::palette_search(self.connected_rpc.to_owned(), (self.palette_draft).trim().to_owned()) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::PaletteResults(value), ::std::result::Result::Err(error) => __DucktapeMessage::PaletteSearchFailed(error) }); self.__ice_run_lane_15_generation = self.__ice_run_lane_15_generation.wrapping_add(1); let __generation = self.__ice_run_lane_15_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_15_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane15(__generation, ::std::boxed::Box::new(__message))) };
})(),
__DucktapeMessage::PaletteResults(next) => (|| {
let _ = &next;
{ let __ice_next = next.chat_hits.clone(); if ::ui_lang_runtime::state_changed!(self.palette_chat_hits, __ice_next) { self.palette_chat_hits = __ice_next; self.__ice_rev[115] += 1; } }
{ let __ice_next = next.page_hits.clone(); if ::ui_lang_runtime::state_changed!(self.palette_page_hits, __ice_next) { self.palette_page_hits = __ice_next; self.__ice_rev[116] += 1; } }
{ let __ice_next = SearchPhase::Done; if ::ui_lang_runtime::state_changed!(self.palette_search_phase, __ice_next) { self.palette_search_phase = __ice_next; self.__ice_rev[114] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::PaletteSearchFailed(cause) => (|| {
let _ = &cause;
{ let __ice_next = SearchPhase::Idle; if ::ui_lang_runtime::state_changed!(self.palette_search_phase, __ice_next) { self.palette_search_phase = __ice_next; self.__ice_rev[114] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.palette_chat_hits, __ice_next) { self.palette_chat_hits = __ice_next; self.__ice_rev[115] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.palette_page_hits, __ice_next) { self.palette_page_hits = __ice_next; self.__ice_rev[116] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::OpenChatSearchHit(channel_id, target_seq) => (|| {
let _ = &channel_id;
let _ = &target_seq;
if (self.mutation_phase != MutationPhase::Idle) { return ::ducktape_view_guest::Task::none(); }
let next_channel = crate::backend::channel_switch_facts(self.channel_reads.clone(), self.channels.clone(), self.active_channel.to_owned(), channel_id.to_owned(), self.unread_boundary, self.active_channel_name.to_owned());
{ let __ice_next = next_channel.unread_boundary; if ::ui_lang_runtime::state_changed!(self.unread_boundary, __ice_next) { self.unread_boundary = __ice_next; self.__ice_rev[26] += 1; } }
{ let __ice_next = channel_id.to_owned(); if ::ui_lang_runtime::state_changed!(self.active_channel, __ice_next) { self.active_channel = __ice_next; self.__ice_rev[27] += 1; } }
{ let __ice_next = target_seq; if ::ui_lang_runtime::state_changed!(self.chat_land_seq, __ice_next) { self.chat_land_seq = __ice_next; self.__ice_rev[33] += 1; } }
{ let __ice_next = crate::backend::dm_peer_of_channel(self.active_dm_peer.to_owned(), self.dm_peers.clone(), self.active_channel.to_owned()); if ::ui_lang_runtime::state_changed!(self.active_dm_peer, __ice_next) { self.active_dm_peer = __ice_next; self.__ice_rev[53] += 1; } }
{ let __ice_next = crate::backend::dm_peer_named(self.dm_peers.clone(), self.active_dm_peer.to_owned()); if ::ui_lang_runtime::state_changed!(self.active_dm, __ice_next) { self.active_dm = __ice_next; self.__ice_rev[54] += 1; } }
{ let __ice_next = next_channel.name.to_owned(); if ::ui_lang_runtime::state_changed!(self.active_channel_name, __ice_next) { self.active_channel_name = __ice_next; self.__ice_rev[28] += 1; } }
{ let __ice_next = next_channel.archived; if ::ui_lang_runtime::state_changed!(self.active_channel_archived, __ice_next) { self.active_channel_archived = __ice_next; self.__ice_rev[29] += 1; } }
{ let __ice_next = next_channel.members_only; if ::ui_lang_runtime::state_changed!(self.active_channel_members_only, __ice_next) { self.active_channel_members_only = __ice_next; self.__ice_rev[30] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.history_view, __ice_next) { self.history_view = __ice_next; self.__ice_rev[48] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.chat_at_tail, __ice_next) { self.chat_at_tail = __ice_next; self.__ice_rev[47] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.channel_members, __ice_next) { self.channel_members = __ice_next; self.__ice_rev[31] += 1; } }
let post_gate_known = (!self.active_channel_members_only);
{ let __ice_next = crate::backend::keep_str(post_gate_known, ::std::convert::AsRef::as_ref(&(crate::backend::post_gate(self.active_channel_archived, self.active_channel_members_only, self.channel_members.clone(), self.settings_user_key.to_owned()))), ::std::convert::AsRef::as_ref(&(""))); if ::ui_lang_runtime::state_changed!(self.post_refusal, __ice_next) { self.post_refusal = __ice_next; self.__ice_rev[32] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.palette_open, __ice_next) { self.palette_open = __ice_next; self.__ice_rev[104] += 1; } }
self.__ice_run_lane_10_generation = self.__ice_run_lane_10_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_10_handle.take() { __previous.abort(); }
self.__ice_run_lane_11_generation = self.__ice_run_lane_11_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_11_handle.take() { __previous.abort(); }
{ let __ice_next = (self.account_busy && (self.account_ceremony_phase).is_empty()); if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_phase, __ice_next) { self.account_ceremony_phase = __ice_next; self.__ice_rev[80] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_qr, __ice_next) { self.account_ceremony_qr = __ice_next; self.__ice_rev[81] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_detail, __ice_next) { self.account_ceremony_detail = __ice_next; self.__ice_rev[82] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_left, __ice_next) { self.account_ceremony_left = __ice_next; self.__ice_rev[83] += 1; } }
{ let __ice_next = ShellTab::Chat; if ::ui_lang_runtime::state_changed!(self.shell_tab, __ice_next) { self.shell_tab = __ice_next; self.__ice_rev[55] += 1; } }
{ let __ice_next = (self.hydration_generation + 1); if ::ui_lang_runtime::state_changed!(self.hydration_generation, __ice_next) { self.hydration_generation = __ice_next; self.__ice_rev[15] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.hydration_retry_attempt, __ice_next) { self.hydration_retry_attempt = __ice_next; self.__ice_rev[18] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.loading, __ice_next) { self.loading = __ice_next; self.__ice_rev[9] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
{ let __ice_next = (self.chat_generation + 1); if ::ui_lang_runtime::state_changed!(self.chat_generation, __ice_next) { self.chat_generation = __ice_next; self.__ice_rev[24] += 1; } }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::load_channel_window(self.connected_rpc.to_owned(), self.active_channel.to_owned(), self.chat_generation) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::ChatUpdated(value), ::std::result::Result::Err(error) => __DucktapeMessage::ChatLoadFailed(error) }); self.__ice_run_lane_16_generation = self.__ice_run_lane_16_generation.wrapping_add(1); let __generation = self.__ice_run_lane_16_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_16_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane16(__generation, ::std::boxed::Box::new(__message))) };
})(),
__DucktapeMessage::ChooseChannel(id) => (|| {
let _ = &id;
if (self.mutation_phase != MutationPhase::Idle) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.active_dm_peer, __ice_next) { self.active_dm_peer = __ice_next; self.__ice_rev[53] += 1; } }
{ let __ice_next = crate::backend::no_dm_peer(); if ::ui_lang_runtime::state_changed!(self.active_dm, __ice_next) { self.active_dm = __ice_next; self.__ice_rev[54] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.history_view, __ice_next) { self.history_view = __ice_next; self.__ice_rev[48] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.chat_at_tail, __ice_next) { self.chat_at_tail = __ice_next; self.__ice_rev[47] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.chat_land_seq, __ice_next) { self.chat_land_seq = __ice_next; self.__ice_rev[33] += 1; } }
let next_channel = crate::backend::channel_switch_facts(self.channel_reads.clone(), self.channels.clone(), self.active_channel.to_owned(), id.to_owned(), self.unread_boundary, self.active_channel_name.to_owned());
{ let __ice_next = next_channel.unread_boundary; if ::ui_lang_runtime::state_changed!(self.unread_boundary, __ice_next) { self.unread_boundary = __ice_next; self.__ice_rev[26] += 1; } }
{ let __ice_next = id.to_owned(); if ::ui_lang_runtime::state_changed!(self.active_channel, __ice_next) { self.active_channel = __ice_next; self.__ice_rev[27] += 1; } }
{ let __ice_next = next_channel.name.to_owned(); if ::ui_lang_runtime::state_changed!(self.active_channel_name, __ice_next) { self.active_channel_name = __ice_next; self.__ice_rev[28] += 1; } }
{ let __ice_next = next_channel.archived; if ::ui_lang_runtime::state_changed!(self.active_channel_archived, __ice_next) { self.active_channel_archived = __ice_next; self.__ice_rev[29] += 1; } }
{ let __ice_next = next_channel.members_only; if ::ui_lang_runtime::state_changed!(self.active_channel_members_only, __ice_next) { self.active_channel_members_only = __ice_next; self.__ice_rev[30] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.channel_members, __ice_next) { self.channel_members = __ice_next; self.__ice_rev[31] += 1; } }
let post_gate_known = (!self.active_channel_members_only);
{ let __ice_next = crate::backend::keep_str(post_gate_known, ::std::convert::AsRef::as_ref(&(crate::backend::post_gate(self.active_channel_archived, self.active_channel_members_only, self.channel_members.clone(), self.settings_user_key.to_owned()))), ::std::convert::AsRef::as_ref(&(""))); if ::ui_lang_runtime::state_changed!(self.post_refusal, __ice_next) { self.post_refusal = __ice_next; self.__ice_rev[32] += 1; } }
{ let __ice_next = (self.hydration_generation + 1); if ::ui_lang_runtime::state_changed!(self.hydration_generation, __ice_next) { self.hydration_generation = __ice_next; self.__ice_rev[15] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.hydration_retry_attempt, __ice_next) { self.hydration_retry_attempt = __ice_next; self.__ice_rev[18] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.loading, __ice_next) { self.loading = __ice_next; self.__ice_rev[9] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
{ let __ice_next = (self.chat_generation + 1); if ::ui_lang_runtime::state_changed!(self.chat_generation, __ice_next) { self.chat_generation = __ice_next; self.__ice_rev[24] += 1; } }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::load_channel_window(self.connected_rpc.to_owned(), self.active_channel.to_owned(), self.chat_generation) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::ChatUpdated(value), ::std::result::Result::Err(error) => __DucktapeMessage::ChatLoadFailed(error) }); self.__ice_run_lane_16_generation = self.__ice_run_lane_16_generation.wrapping_add(1); let __generation = self.__ice_run_lane_16_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_16_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane16(__generation, ::std::boxed::Box::new(__message))) };
})(),
__DucktapeMessage::ChooseDm(peer_key) => (|| {
let _ = &peer_key;
if ((self.mutation_phase != MutationPhase::Idle) || (peer_key).is_empty()) { return ::ducktape_view_guest::Task::none(); }
self.__ice_run_lane_16_generation = self.__ice_run_lane_16_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_16_handle.take() { __previous.abort(); }
{ let __ice_next = peer_key.to_owned(); if ::ui_lang_runtime::state_changed!(self.active_dm_peer, __ice_next) { self.active_dm_peer = __ice_next; self.__ice_rev[53] += 1; } }
{ let __ice_next = crate::backend::dm_peer_named(self.dm_peers.clone(), self.active_dm_peer.to_owned()); if ::ui_lang_runtime::state_changed!(self.active_dm, __ice_next) { self.active_dm = __ice_next; self.__ice_rev[54] += 1; } }
let dm_room = crate::backend::dm_room_of_peer(self.dm_peers.clone(), self.active_dm_peer.to_owned());
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.history_view, __ice_next) { self.history_view = __ice_next; self.__ice_rev[48] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.chat_at_tail, __ice_next) { self.chat_at_tail = __ice_next; self.__ice_rev[47] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.chat_land_seq, __ice_next) { self.chat_land_seq = __ice_next; self.__ice_rev[33] += 1; } }
let next_channel = crate::backend::channel_switch_facts(self.channel_reads.clone(), self.channels.clone(), self.active_channel.to_owned(), dm_room.to_owned(), self.unread_boundary, self.active_channel_name.to_owned());
{ let __ice_next = next_channel.unread_boundary; if ::ui_lang_runtime::state_changed!(self.unread_boundary, __ice_next) { self.unread_boundary = __ice_next; self.__ice_rev[26] += 1; } }
{ let __ice_next = dm_room.to_owned(); if ::ui_lang_runtime::state_changed!(self.active_channel, __ice_next) { self.active_channel = __ice_next; self.__ice_rev[27] += 1; } }
{ let __ice_next = next_channel.name.to_owned(); if ::ui_lang_runtime::state_changed!(self.active_channel_name, __ice_next) { self.active_channel_name = __ice_next; self.__ice_rev[28] += 1; } }
{ let __ice_next = next_channel.archived; if ::ui_lang_runtime::state_changed!(self.active_channel_archived, __ice_next) { self.active_channel_archived = __ice_next; self.__ice_rev[29] += 1; } }
{ let __ice_next = next_channel.members_only; if ::ui_lang_runtime::state_changed!(self.active_channel_members_only, __ice_next) { self.active_channel_members_only = __ice_next; self.__ice_rev[30] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.channel_members, __ice_next) { self.channel_members = __ice_next; self.__ice_rev[31] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.post_refusal, __ice_next) { self.post_refusal = __ice_next; self.__ice_rev[32] += 1; } }
{ let __ice_next = (self.hydration_generation + 1); if ::ui_lang_runtime::state_changed!(self.hydration_generation, __ice_next) { self.hydration_generation = __ice_next; self.__ice_rev[15] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.hydration_retry_attempt, __ice_next) { self.hydration_retry_attempt = __ice_next; self.__ice_rev[18] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.loading, __ice_next) { self.loading = __ice_next; self.__ice_rev[9] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
{ let __ice_next = (self.chat_generation + 1); if ::ui_lang_runtime::state_changed!(self.chat_generation, __ice_next) { self.chat_generation = __ice_next; self.__ice_rev[24] += 1; } }
return ::ducktape_view_guest::Task::perform(({ crate::backend::open_dm(self.connected_rpc.to_owned(), self.password.to_owned(), self.active_dm_peer.to_owned(), self.chat_generation) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::ChatUpdated(value), ::std::result::Result::Err(error) => __DucktapeMessage::ChatLoadFailed(error) });
})(),
__DucktapeMessage::CreateChannelSubmit => (|| {
if ((self.loading || (self.mutation_phase != MutationPhase::Idle)) || ((self.channel_draft).trim().to_owned()).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = (self.hydration_generation + 1); if ::ui_lang_runtime::state_changed!(self.hydration_generation, __ice_next) { self.hydration_generation = __ice_next; self.__ice_rev[15] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.hydration_retry_attempt, __ice_next) { self.hydration_retry_attempt = __ice_next; self.__ice_rev[18] += 1; } }
{ let __ice_next = MutationPhase::Channel; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = (self.channel_draft).trim().to_owned(); if ::ui_lang_runtime::state_changed!(self.pending_channel, __ice_next) { self.pending_channel = __ice_next; self.__ice_rev[46] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.channel_draft, __ice_next) { self.channel_draft = __ice_next; self.__ice_rev[43] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
{ let __ice_next = (self.chat_generation + 1); if ::ui_lang_runtime::state_changed!(self.chat_generation, __ice_next) { self.chat_generation = __ice_next; self.__ice_rev[24] += 1; } }
return ::ducktape_view_guest::Task::perform(({ crate::backend::create_channel(self.connected_rpc.to_owned(), self.password.to_owned(), self.pending_channel.to_owned(), self.channel_create_members_only, self.chat_generation) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::ChannelCreated(value), ::std::result::Result::Err(error) => __DucktapeMessage::MutationFailed(error) });
})(),
__DucktapeMessage::ToggleChannelCreateMembersOnly => (|| {
{ let __ice_next = (!self.channel_create_members_only); if ::ui_lang_runtime::state_changed!(self.channel_create_members_only, __ice_next) { self.channel_create_members_only = __ice_next; self.__ice_rev[45] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::ToggleChannelCreate => (|| {
{ let __ice_next = (!self.channel_create_open); if ::ui_lang_runtime::state_changed!(self.channel_create_open, __ice_next) { self.channel_create_open = __ice_next; self.__ice_rev[44] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::JoinHuddleSubmit => (|| {
if (((self.loading || (self.mutation_phase != MutationPhase::Idle)) || (self.active_channel).is_empty()) || self.active_channel_archived) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = (self.hydration_generation + 1); if ::ui_lang_runtime::state_changed!(self.hydration_generation, __ice_next) { self.hydration_generation = __ice_next; self.__ice_rev[15] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.hydration_retry_attempt, __ice_next) { self.hydration_retry_attempt = __ice_next; self.__ice_rev[18] += 1; } }
{ let __ice_next = MutationPhase::Huddle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
return ::ducktape_view_guest::Task::perform(({ crate::backend::join_huddle(self.connected_rpc.to_owned(), self.password.to_owned(), self.active_channel.to_owned()) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::HuddleJoinedAck(value), ::std::result::Result::Err(error) => __DucktapeMessage::MutationFailed(error) });
})(),
__DucktapeMessage::HuddleJoinedAck(_result) => (|| {
let _ = &_result;
{ let __ice_next = MutationPhase::Idle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.huddle_joined, __ice_next) { self.huddle_joined = __ice_next; self.__ice_rev[142] += 1; } }
{ let __ice_next = self.active_channel.to_owned(); if ::ui_lang_runtime::state_changed!(self.huddle_channel, __ice_next) { self.huddle_channel = __ice_next; self.__ice_rev[143] += 1; } }
{ let __ice_next = self.active_channel_name.to_owned(); if ::ui_lang_runtime::state_changed!(self.huddle_channel_name, __ice_next) { self.huddle_channel_name = __ice_next; self.__ice_rev[144] += 1; } }
{ let __ice_next = self.huddle_now; if ::ui_lang_runtime::state_changed!(self.huddle_joined_at, __ice_next) { self.huddle_joined_at = __ice_next; self.__ice_rev[145] += 1; } }
{ let __ice_next = (self.chat_generation + 1); if ::ui_lang_runtime::state_changed!(self.chat_generation, __ice_next) { self.chat_generation = __ice_next; self.__ice_rev[24] += 1; } }
return ::ducktape_view_guest::Task::batch([
{ // __ICE_SOURCE 228 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f636861742e696365
(::ducktape_view_guest::Task::done(true)).map(|value| { let _ = &value; __DucktapeMessage::ShowHuddle })
},
{ // __ICE_SOURCE 231 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f636861742e696365
{ let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::load_channel_window(self.connected_rpc.to_owned(), self.active_channel.to_owned(), self.chat_generation) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::ChatUpdated(value), ::std::result::Result::Err(error) => __DucktapeMessage::ChatLoadFailed(error) }); self.__ice_run_lane_16_generation = self.__ice_run_lane_16_generation.wrapping_add(1); let __generation = self.__ice_run_lane_16_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_16_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane16(__generation, ::std::boxed::Box::new(__message))) }
},
]);
})(),
__DucktapeMessage::ChatBeginEdit(scope, body, seq, rev) => (|| {
let _ = &scope;
let _ = &body;
let _ = &seq;
let _ = &rev;
if ((body).is_empty() || (seq <= 0)) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = seq; if ::ui_lang_runtime::state_changed!(self.chat_edit_seq, __ice_next) { self.chat_edit_seq = __ice_next; self.__ice_rev[34] += 1; } }
{ let __ice_next = rev; if ::ui_lang_runtime::state_changed!(self.chat_edit_rev, __ice_next) { self.chat_edit_rev = __ice_next; self.__ice_rev[35] += 1; } }
{ let __ice_next = ({ crate::module_view::chat_composer_seed(::std::convert::AsRef::as_ref(&(scope)), ::std::convert::AsRef::as_ref(&(body))) }); if ::ui_lang_runtime::state_changed!(self.composer_seeded, __ice_next) { self.composer_seeded = __ice_next; self.__ice_rev[38] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::ComposerSubmitted(kind, pending_body, pending_id, scope) => (|| {
let _ = &kind;
let _ = &pending_body;
let _ = &pending_id;
let _ = &scope;
return match kind.clone() {
ComposerKind::Message => (|| {
return match crate::backend::submit_verdict(self.loading, self.connected, self.active_channel.to_owned(), self.post_refusal.to_owned(), true, scope.to_owned(), crate::backend::composer_scope(::std::convert::AsRef::as_ref(&(self.connected_rpc)), ::std::convert::AsRef::as_ref(&(self.active_channel)))) {
SubmitVerdict::Refused => (|| {
{ let __ice_next = ({ crate::module_view::chat_composer_unsent(::std::convert::AsRef::as_ref(&(scope)), ::std::convert::AsRef::as_ref(&(pending_body)), false) }); if ::ui_lang_runtime::state_changed!(self.composer_stashed, __ice_next) { self.composer_stashed = __ice_next; self.__ice_rev[36] += 1; } }
::ducktape_view_guest::Task::none()
})(),
SubmitVerdict::Admitted => (|| {
{ let __ice_next = (self.hydration_generation + 1); if ::ui_lang_runtime::state_changed!(self.hydration_generation, __ice_next) { self.hydration_generation = __ice_next; self.__ice_rev[15] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.hydration_retry_attempt, __ice_next) { self.hydration_retry_attempt = __ice_next; self.__ice_rev[18] += 1; } }
self.chat_pending_sends = crate::backend::send_pending(::std::mem::take(&mut self.chat_pending_sends), pending_id.to_owned(), pending_body.to_owned(), 0); self.__ice_rev[41] += 1;
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.chat_at_tail, __ice_next) { self.chat_at_tail = __ice_next; self.__ice_rev[47] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.history_view, __ice_next) { self.history_view = __ice_next; self.__ice_rev[48] += 1; } }
{ let __ice_next = (self.chat_sent_serial + 1); if ::ui_lang_runtime::state_changed!(self.chat_sent_serial, __ice_next) { self.chat_sent_serial = __ice_next; self.__ice_rev[40] += 1; } }
return ::ducktape_view_guest::Task::perform(({ crate::backend::send_message(self.connected_rpc.to_owned(), self.password.to_owned(), self.active_channel.to_owned(), pending_id.to_owned(), pending_body.to_owned()) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::MessageSent(value), ::std::result::Result::Err(error) => __DucktapeMessage::MessageSendFailed(error) });
})(),
};
})(),
ComposerKind::Reply => (|| {
let thread_seq = crate::backend::scope_thread_seq(::std::convert::AsRef::as_ref(&(scope)));
return match crate::backend::submit_verdict(false, self.connected, self.active_channel.to_owned(), self.post_refusal.to_owned(), (thread_seq > 0), scope.to_owned(), crate::backend::thread_scope(::std::convert::AsRef::as_ref(&(self.connected_rpc)), ::std::convert::AsRef::as_ref(&(self.active_channel)), thread_seq)) {
SubmitVerdict::Refused => (|| {
{ let __ice_next = ({ crate::module_view::chat_composer_unsent(::std::convert::AsRef::as_ref(&(scope)), ::std::convert::AsRef::as_ref(&(pending_body)), false) }); if ::ui_lang_runtime::state_changed!(self.composer_stashed, __ice_next) { self.composer_stashed = __ice_next; self.__ice_rev[36] += 1; } }
::ducktape_view_guest::Task::none()
})(),
SubmitVerdict::Admitted => (|| {
{ let __ice_next = (self.hydration_generation + 1); if ::ui_lang_runtime::state_changed!(self.hydration_generation, __ice_next) { self.hydration_generation = __ice_next; self.__ice_rev[15] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.hydration_retry_attempt, __ice_next) { self.hydration_retry_attempt = __ice_next; self.__ice_rev[18] += 1; } }
self.chat_pending_sends = crate::backend::send_pending(::std::mem::take(&mut self.chat_pending_sends), pending_id.to_owned(), pending_body.to_owned(), thread_seq); self.__ice_rev[41] += 1;
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
return ::ducktape_view_guest::Task::perform(({ crate::backend::send_reply(self.connected_rpc.to_owned(), self.password.to_owned(), self.active_channel.to_owned(), thread_seq, pending_id.to_owned(), pending_body.to_owned()) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::ThreadReplySent(value), ::std::result::Result::Err(error) => __DucktapeMessage::ThreadReplySendFailed(error) });
})(),
};
})(),
ComposerKind::Edit => (|| {
if (scope != crate::backend::edit_scope(::std::convert::AsRef::as_ref(&(self.connected_rpc)), ::std::convert::AsRef::as_ref(&(self.active_channel)), self.chat_edit_seq)) { return ::ducktape_view_guest::Task::none(); }
return (::ducktape_view_guest::Task::done((pending_body).trim().to_owned())).map(|value| __DucktapeMessage::EditMessageSubmit(value));
})(),
ComposerKind::ThreadEdit => (|| {
if (scope != crate::backend::edit_scope(::std::convert::AsRef::as_ref(&(self.connected_rpc)), ::std::convert::AsRef::as_ref(&(self.active_channel)), self.chat_edit_seq)) { return ::ducktape_view_guest::Task::none(); }
return (::ducktape_view_guest::Task::done((pending_body).trim().to_owned())).map(|value| __DucktapeMessage::EditMessageSubmit(value));
})(),
};
})(),
__DucktapeMessage::EditMessageSubmit(text) => (|| {
let _ = &text;
if ((((self.loading || (self.mutation_phase != MutationPhase::Idle)) || (self.active_channel).is_empty()) || (self.chat_edit_seq <= 0)) || ((text).trim().to_owned()).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = (self.hydration_generation + 1); if ::ui_lang_runtime::state_changed!(self.hydration_generation, __ice_next) { self.hydration_generation = __ice_next; self.__ice_rev[15] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.hydration_retry_attempt, __ice_next) { self.hydration_retry_attempt = __ice_next; self.__ice_rev[18] += 1; } }
{ let __ice_next = MutationPhase::MessageEdit; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
return ::ducktape_view_guest::Task::perform(({ crate::backend::edit_message(self.connected_rpc.to_owned(), self.password.to_owned(), self.active_channel.to_owned(), self.chat_edit_seq, self.chat_edit_rev, (text).trim().to_owned()) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::ChatAcked(value), ::std::result::Result::Err(error) => __DucktapeMessage::MutationFailed(error) });
})(),
__DucktapeMessage::MessageSent(next) => (|| {
let _ = &next;
self.chat_pending_sends = crate::backend::send_settled(::std::mem::take(&mut self.chat_pending_sends), ::std::convert::AsRef::as_ref(&(next.operation_id))); self.__ice_rev[41] += 1;
if (self.active_channel != next.channel_id) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::MessageSendFailed(cause) => (|| {
let _ = &cause;
{ let __ice_next = cause.message.to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
self.chat_pending_sends = crate::backend::send_failed(::std::mem::take(&mut self.chat_pending_sends), ::std::convert::AsRef::as_ref(&(cause.operation_id)), cause.committed); self.__ice_rev[41] += 1;
{ let __ice_next = ({ crate::module_view::chat_composer_unsent(::std::convert::AsRef::as_ref(&(crate::backend::composer_scope(::std::convert::AsRef::as_ref(&(self.connected_rpc)), ::std::convert::AsRef::as_ref(&(cause.scope_id))))), ::std::convert::AsRef::as_ref(&(cause.body)), cause.committed) }); if ::ui_lang_runtime::state_changed!(self.composer_stashed, __ice_next) { self.composer_stashed = __ice_next; self.__ice_rev[36] += 1; } }
if ((self.active_channel != cause.scope_id) || (!cause.committed)) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = (self.hydration_generation + 1); if ::ui_lang_runtime::state_changed!(self.hydration_generation, __ice_next) { self.hydration_generation = __ice_next; self.__ice_rev[15] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.hydration_retry_attempt, __ice_next) { self.hydration_retry_attempt = __ice_next; self.__ice_rev[18] += 1; } }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::live_resync_load(self.connected_rpc.to_owned(), self.active_channel.to_owned(), true, false, self.hydration_generation, 0) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::LiveResynced(value), ::std::result::Result::Err(error) => __DucktapeMessage::LiveResyncFailed(error) }); self.__ice_run_lane_8_generation = self.__ice_run_lane_8_generation.wrapping_add(1); let __generation = self.__ice_run_lane_8_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_8_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane8(__generation, ::std::boxed::Box::new(__message))) };
})(),
__DucktapeMessage::ThreadReplySendFailed(cause) => (|| {
let _ = &cause;
{ let __ice_next = cause.message.to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
self.chat_pending_sends = crate::backend::send_failed(::std::mem::take(&mut self.chat_pending_sends), ::std::convert::AsRef::as_ref(&(cause.operation_id)), cause.committed); self.__ice_rev[41] += 1;
{ let __ice_next = ({ crate::module_view::chat_composer_unsent(::std::convert::AsRef::as_ref(&(crate::backend::thread_scope(::std::convert::AsRef::as_ref(&(self.connected_rpc)), ::std::convert::AsRef::as_ref(&(cause.scope_id)), cause.thread_seq))), ::std::convert::AsRef::as_ref(&(cause.body)), cause.committed) }); if ::ui_lang_runtime::state_changed!(self.composer_stashed, __ice_next) { self.composer_stashed = __ice_next; self.__ice_rev[36] += 1; } }
if ((self.active_channel != cause.scope_id) || (!cause.committed)) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = (self.hydration_generation + 1); if ::ui_lang_runtime::state_changed!(self.hydration_generation, __ice_next) { self.hydration_generation = __ice_next; self.__ice_rev[15] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.hydration_retry_attempt, __ice_next) { self.hydration_retry_attempt = __ice_next; self.__ice_rev[18] += 1; } }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::live_resync_load(self.connected_rpc.to_owned(), self.active_channel.to_owned(), true, false, self.hydration_generation, 0) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::LiveResynced(value), ::std::result::Result::Err(error) => __DucktapeMessage::LiveResyncFailed(error) }); self.__ice_run_lane_8_generation = self.__ice_run_lane_8_generation.wrapping_add(1); let __generation = self.__ice_run_lane_8_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_8_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane8(__generation, ::std::boxed::Box::new(__message))) };
})(),
__DucktapeMessage::ThreadReplySent(next) => (|| {
let _ = &next;
self.chat_pending_sends = crate::backend::send_settled(::std::mem::take(&mut self.chat_pending_sends), ::std::convert::AsRef::as_ref(&(next.operation_id))); self.__ice_rev[41] += 1;
if (self.active_channel != next.channel_id) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::ChatUpdated(next) => (|| {
let _ = &next;
if (next.generation != self.chat_generation) { return ::ducktape_view_guest::Task::none(); }
self.channels = crate::backend::upsert_channel_rows(::std::mem::take(&mut self.channels), next.channels.clone()); self.__ice_rev[22] += 1;
{ let __ice_next = crate::backend::frozen_unread_boundary(self.channel_reads.clone(), self.channels.clone(), self.active_channel.to_owned(), next.active_channel.to_owned(), self.unread_boundary); if ::ui_lang_runtime::state_changed!(self.unread_boundary, __ice_next) { self.unread_boundary = __ice_next; self.__ice_rev[26] += 1; } }
self.channel_reads = crate::backend::mark_channel_read(::std::mem::take(&mut self.channel_reads), next.active_channel.to_owned(), crate::backend::channel_head_seq(self.channels.clone(), next.active_channel.to_owned())); self.__ice_rev[25] += 1;
{ let __ice_next = crate::backend::chat_sidebar_rooms(self.channels.clone(), self.dm_peers.clone(), self.channel_reads.clone()); if ::ui_lang_runtime::state_changed!(self.rooms, __ice_next) { self.rooms = __ice_next; self.__ice_rev[23] += 1; } }
{ let __ice_next = crate::backend::chat_sidebar_dms(self.channels.clone(), self.dm_peers.clone(), self.channel_reads.clone()); if ::ui_lang_runtime::state_changed!(self.dm_rows, __ice_next) { self.dm_rows = __ice_next; self.__ice_rev[51] += 1; } }
let landed_elsewhere = (self.active_channel != next.active_channel);
{ let __ice_next = (self.history_view && (!landed_elsewhere)); if ::ui_lang_runtime::state_changed!(self.history_view, __ice_next) { self.history_view = __ice_next; self.__ice_rev[48] += 1; } }
{ let __ice_next = crate::backend::keep_i64(landed_elsewhere, 0, self.chat_land_seq); if ::ui_lang_runtime::state_changed!(self.chat_land_seq, __ice_next) { self.chat_land_seq = __ice_next; self.__ice_rev[33] += 1; } }
{ let __ice_next = next.active_channel.to_owned(); if ::ui_lang_runtime::state_changed!(self.active_channel, __ice_next) { self.active_channel = __ice_next; self.__ice_rev[27] += 1; } }
{ let __ice_next = crate::backend::dm_peer_of_channel(self.active_dm_peer.to_owned(), self.dm_peers.clone(), self.active_channel.to_owned()); if ::ui_lang_runtime::state_changed!(self.active_dm_peer, __ice_next) { self.active_dm_peer = __ice_next; self.__ice_rev[53] += 1; } }
{ let __ice_next = crate::backend::dm_peer_named(self.dm_peers.clone(), self.active_dm_peer.to_owned()); if ::ui_lang_runtime::state_changed!(self.active_dm, __ice_next) { self.active_dm = __ice_next; self.__ice_rev[54] += 1; } }
{ let __ice_next = next.active_channel_name.to_owned(); if ::ui_lang_runtime::state_changed!(self.active_channel_name, __ice_next) { self.active_channel_name = __ice_next; self.__ice_rev[28] += 1; } }
{ let __ice_next = next.active_channel_archived; if ::ui_lang_runtime::state_changed!(self.active_channel_archived, __ice_next) { self.active_channel_archived = __ice_next; self.__ice_rev[29] += 1; } }
{ let __ice_next = next.active_channel_members_only; if ::ui_lang_runtime::state_changed!(self.active_channel_members_only, __ice_next) { self.active_channel_members_only = __ice_next; self.__ice_rev[30] += 1; } }
{ let __ice_next = crate::backend::keep_i64(self.huddle_joined, self.huddle_joined_at, self.huddle_now); if ::ui_lang_runtime::state_changed!(self.huddle_joined_at, __ice_next) { self.huddle_joined_at = __ice_next; self.__ice_rev[145] += 1; } }
let huddle = crate::backend::huddle_after_load(true, self.huddle_joined, self.huddle_channel.to_owned(), self.huddle_channel_name.to_owned(), self.huddle_roster.clone(), self.active_channel.to_owned(), self.active_channel_name.to_owned(), next.huddle_roster.clone());
{ let __ice_next = huddle.joined; if ::ui_lang_runtime::state_changed!(self.huddle_joined, __ice_next) { self.huddle_joined = __ice_next; self.__ice_rev[142] += 1; } }
{ let __ice_next = huddle.roster.clone(); if ::ui_lang_runtime::state_changed!(self.huddle_roster, __ice_next) { self.huddle_roster = __ice_next; self.__ice_rev[154] += 1; } }
{ let __ice_next = crate::call::huddle_tile_rows(self.huddle_roster.clone(), self.call_peers.clone(), self.call_muted); if ::ui_lang_runtime::state_changed!(self.huddle_rows, __ice_next) { self.huddle_rows = __ice_next; self.__ice_rev[155] += 1; } }
{ let __ice_next = huddle.channel.to_owned(); if ::ui_lang_runtime::state_changed!(self.huddle_channel, __ice_next) { self.huddle_channel = __ice_next; self.__ice_rev[143] += 1; } }
{ let __ice_next = huddle.channel_name.to_owned(); if ::ui_lang_runtime::state_changed!(self.huddle_channel_name, __ice_next) { self.huddle_channel_name = __ice_next; self.__ice_rev[144] += 1; } }
{ let __ice_next = next.channel_members.clone(); if ::ui_lang_runtime::state_changed!(self.channel_members, __ice_next) { self.channel_members = __ice_next; self.__ice_rev[31] += 1; } }
{ let __ice_next = ({ crate::module_view::chat_composer_roster(::std::convert::AsRef::as_ref(&(crate::backend::composer_scope(::std::convert::AsRef::as_ref(&(self.connected_rpc)), ::std::convert::AsRef::as_ref(&(self.active_channel))))), ::std::convert::AsRef::as_ref(&(self.channel_members))) }); if ::ui_lang_runtime::state_changed!(self.composer_roster_set, __ice_next) { self.composer_roster_set = __ice_next; self.__ice_rev[37] += 1; } }
{ let __ice_next = crate::backend::post_gate(self.active_channel_archived, self.active_channel_members_only, self.channel_members.clone(), self.settings_user_key.to_owned()); if ::ui_lang_runtime::state_changed!(self.post_refusal, __ice_next) { self.post_refusal = __ice_next; self.__ice_rev[32] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.loading, __ice_next) { self.loading = __ice_next; self.__ice_rev[9] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
return ::crate::shell::close::<__DucktapeMessage>(({ crate::backend::window_target_unless(self.huddle_joined, self.huddle_win.clone()) }));
})(),
__DucktapeMessage::ChatLoadFailed(cause) => (|| {
let _ = &cause;
if (cause.generation != self.chat_generation) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = (self.hydration_generation + 1); if ::ui_lang_runtime::state_changed!(self.hydration_generation, __ice_next) { self.hydration_generation = __ice_next; self.__ice_rev[15] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.hydration_retry_attempt, __ice_next) { self.hydration_retry_attempt = __ice_next; self.__ice_rev[18] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.loading, __ice_next) { self.loading = __ice_next; self.__ice_rev[9] += 1; } }
{ let __ice_next = cause.message.to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::ChannelCreated(next) => (|| {
let _ = &next;
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.pending_channel, __ice_next) { self.pending_channel = __ice_next; self.__ice_rev[46] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.channel_create_open, __ice_next) { self.channel_create_open = __ice_next; self.__ice_rev[44] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.channel_create_members_only, __ice_next) { self.channel_create_members_only = __ice_next; self.__ice_rev[45] += 1; } }
{ let __ice_next = MutationPhase::Idle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
if (next.generation != self.chat_generation) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.history_view, __ice_next) { self.history_view = __ice_next; self.__ice_rev[48] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.chat_at_tail, __ice_next) { self.chat_at_tail = __ice_next; self.__ice_rev[47] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.chat_land_seq, __ice_next) { self.chat_land_seq = __ice_next; self.__ice_rev[33] += 1; } }
self.channels = crate::backend::upsert_channel_rows(::std::mem::take(&mut self.channels), next.channels.clone()); self.__ice_rev[22] += 1;
{ let __ice_next = crate::backend::frozen_unread_boundary(self.channel_reads.clone(), self.channels.clone(), self.active_channel.to_owned(), next.active_channel.to_owned(), self.unread_boundary); if ::ui_lang_runtime::state_changed!(self.unread_boundary, __ice_next) { self.unread_boundary = __ice_next; self.__ice_rev[26] += 1; } }
self.channel_reads = crate::backend::mark_channel_read(::std::mem::take(&mut self.channel_reads), next.active_channel.to_owned(), crate::backend::channel_head_seq(self.channels.clone(), next.active_channel.to_owned())); self.__ice_rev[25] += 1;
{ let __ice_next = crate::backend::chat_sidebar_rooms(self.channels.clone(), self.dm_peers.clone(), self.channel_reads.clone()); if ::ui_lang_runtime::state_changed!(self.rooms, __ice_next) { self.rooms = __ice_next; self.__ice_rev[23] += 1; } }
{ let __ice_next = crate::backend::chat_sidebar_dms(self.channels.clone(), self.dm_peers.clone(), self.channel_reads.clone()); if ::ui_lang_runtime::state_changed!(self.dm_rows, __ice_next) { self.dm_rows = __ice_next; self.__ice_rev[51] += 1; } }
{ let __ice_next = next.active_channel.to_owned(); if ::ui_lang_runtime::state_changed!(self.active_channel, __ice_next) { self.active_channel = __ice_next; self.__ice_rev[27] += 1; } }
{ let __ice_next = crate::backend::dm_peer_of_channel(self.active_dm_peer.to_owned(), self.dm_peers.clone(), self.active_channel.to_owned()); if ::ui_lang_runtime::state_changed!(self.active_dm_peer, __ice_next) { self.active_dm_peer = __ice_next; self.__ice_rev[53] += 1; } }
{ let __ice_next = crate::backend::dm_peer_named(self.dm_peers.clone(), self.active_dm_peer.to_owned()); if ::ui_lang_runtime::state_changed!(self.active_dm, __ice_next) { self.active_dm = __ice_next; self.__ice_rev[54] += 1; } }
{ let __ice_next = next.active_channel_name.to_owned(); if ::ui_lang_runtime::state_changed!(self.active_channel_name, __ice_next) { self.active_channel_name = __ice_next; self.__ice_rev[28] += 1; } }
{ let __ice_next = next.active_channel_archived; if ::ui_lang_runtime::state_changed!(self.active_channel_archived, __ice_next) { self.active_channel_archived = __ice_next; self.__ice_rev[29] += 1; } }
{ let __ice_next = next.active_channel_members_only; if ::ui_lang_runtime::state_changed!(self.active_channel_members_only, __ice_next) { self.active_channel_members_only = __ice_next; self.__ice_rev[30] += 1; } }
{ let __ice_next = crate::backend::keep_i64(self.huddle_joined, self.huddle_joined_at, self.huddle_now); if ::ui_lang_runtime::state_changed!(self.huddle_joined_at, __ice_next) { self.huddle_joined_at = __ice_next; self.__ice_rev[145] += 1; } }
let huddle = crate::backend::huddle_after_load(true, self.huddle_joined, self.huddle_channel.to_owned(), self.huddle_channel_name.to_owned(), self.huddle_roster.clone(), self.active_channel.to_owned(), self.active_channel_name.to_owned(), next.huddle_roster.clone());
{ let __ice_next = huddle.joined; if ::ui_lang_runtime::state_changed!(self.huddle_joined, __ice_next) { self.huddle_joined = __ice_next; self.__ice_rev[142] += 1; } }
{ let __ice_next = huddle.roster.clone(); if ::ui_lang_runtime::state_changed!(self.huddle_roster, __ice_next) { self.huddle_roster = __ice_next; self.__ice_rev[154] += 1; } }
{ let __ice_next = crate::call::huddle_tile_rows(self.huddle_roster.clone(), self.call_peers.clone(), self.call_muted); if ::ui_lang_runtime::state_changed!(self.huddle_rows, __ice_next) { self.huddle_rows = __ice_next; self.__ice_rev[155] += 1; } }
{ let __ice_next = huddle.channel.to_owned(); if ::ui_lang_runtime::state_changed!(self.huddle_channel, __ice_next) { self.huddle_channel = __ice_next; self.__ice_rev[143] += 1; } }
{ let __ice_next = huddle.channel_name.to_owned(); if ::ui_lang_runtime::state_changed!(self.huddle_channel_name, __ice_next) { self.huddle_channel_name = __ice_next; self.__ice_rev[144] += 1; } }
{ let __ice_next = next.channel_members.clone(); if ::ui_lang_runtime::state_changed!(self.channel_members, __ice_next) { self.channel_members = __ice_next; self.__ice_rev[31] += 1; } }
{ let __ice_next = ({ crate::module_view::chat_composer_roster(::std::convert::AsRef::as_ref(&(crate::backend::composer_scope(::std::convert::AsRef::as_ref(&(self.connected_rpc)), ::std::convert::AsRef::as_ref(&(self.active_channel))))), ::std::convert::AsRef::as_ref(&(self.channel_members))) }); if ::ui_lang_runtime::state_changed!(self.composer_roster_set, __ice_next) { self.composer_roster_set = __ice_next; self.__ice_rev[37] += 1; } }
{ let __ice_next = crate::backend::post_gate(self.active_channel_archived, self.active_channel_members_only, self.channel_members.clone(), self.settings_user_key.to_owned()); if ::ui_lang_runtime::state_changed!(self.post_refusal, __ice_next) { self.post_refusal = __ice_next; self.__ice_rev[32] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
return ::crate::shell::close::<__DucktapeMessage>(({ crate::backend::window_target_unless(self.huddle_joined, self.huddle_win.clone()) }));
})(),
__DucktapeMessage::LiveAgentsEvent(next) => (|| {
let _ = &next;
if crate::backend::live_agents_stale(::std::borrow::Borrow::borrow(&(next)), ::std::convert::AsRef::as_ref(&(self.connected_rpc)), ::std::convert::AsRef::as_ref(&(self.network_chain_id)), self.connect_generation, ::std::convert::AsRef::as_ref(&(self.signer_key))) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = next.rows.clone(); if ::ui_lang_runtime::state_changed!(self.live_agents, __ice_next) { self.live_agents = __ice_next; self.__ice_rev[39] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::LiveCancelAcked(_ok) => (|| {
let _ = &_ok;
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::ChatAcked(_result) => (|| {
let _ = &_result;
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.chat_edit_seq, __ice_next) { self.chat_edit_seq = __ice_next; self.__ice_rev[34] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.chat_edit_rev, __ice_next) { self.chat_edit_rev = __ice_next; self.__ice_rev[35] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.pending_channel, __ice_next) { self.pending_channel = __ice_next; self.__ice_rev[46] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.channel_create_open, __ice_next) { self.channel_create_open = __ice_next; self.__ice_rev[44] += 1; } }
{ let __ice_next = MutationPhase::Idle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::CopyMessageLink(link) => (|| {
let _ = &link;
if (link).is_empty() { return ::ducktape_view_guest::Task::none(); }
return { let __ice_run_route_145_1 = "Message link copied".to_owned();
::ducktape_view_guest::Task::perform(({ crate::backend::duck_echo_str(link.to_owned()) }), move |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::CopyToClipboard(value, __ice_run_route_145_1), ::std::result::Result::Err(error) => __DucktapeMessage::ExternalUrlFailed(error) }) };
})(),
__DucktapeMessage::OpenMessageLink(url) => (|| {
let _ = &url;
if (url).is_empty() { return ::ducktape_view_guest::Task::none(); }
let link = crate::backend::resolve_duck_link(url.to_owned(), self.network_chain_id.to_owned());
return match link.kind.clone() {
DuckKind::Unknown => (|| {
{ let __ice_next = "this link names nothing the app can open".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
::ducktape_view_guest::Task::none()
})(),
DuckKind::ForeignNetwork => (|| {
{ let __ice_next = crate::backend::foreign_network_error(link.net.to_owned(), self.network_chain_id.to_owned()); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
::ducktape_view_guest::Task::none()
})(),
DuckKind::Web => (|| {
return ::ducktape_view_guest::Task::perform(({ crate::backend::open_external_url(url.to_owned()) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::ExternalUrlOpened(value), ::std::result::Result::Err(error) => __DucktapeMessage::ExternalUrlFailed(error) });
})(),
DuckKind::Page => (|| {
return { let __ice_run_route_149_1 = link.block.to_owned();
::ducktape_view_guest::Task::perform(({ crate::backend::duck_echo_str(link.page.to_owned()) }), move |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::OpenPageSearchHit(value, __ice_run_route_149_1), ::std::result::Result::Err(error) => __DucktapeMessage::ExternalUrlFailed(error) }) };
})(),
DuckKind::Run => (|| {
return (::ducktape_view_guest::Task::done(link.dispatch.to_owned())).map(|value| __DucktapeMessage::OpenRunPanel(value));
})(),
DuckKind::Files => (|| {
{ let __ice_next = link.path.to_owned(); if ::ui_lang_runtime::state_changed!(self.fs_route, __ice_next) { self.fs_route = __ice_next; self.__ice_rev[102] += 1; } }
{ let __ice_next = (self.fs_route_serial + 1); if ::ui_lang_runtime::state_changed!(self.fs_route_serial, __ice_next) { self.fs_route_serial = __ice_next; self.__ice_rev[103] += 1; } }
self.__ice_run_lane_10_generation = self.__ice_run_lane_10_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_10_handle.take() { __previous.abort(); }
self.__ice_run_lane_11_generation = self.__ice_run_lane_11_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_11_handle.take() { __previous.abort(); }
{ let __ice_next = (self.account_busy && (self.account_ceremony_phase).is_empty()); if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_phase, __ice_next) { self.account_ceremony_phase = __ice_next; self.__ice_rev[80] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_qr, __ice_next) { self.account_ceremony_qr = __ice_next; self.__ice_rev[81] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_detail, __ice_next) { self.account_ceremony_detail = __ice_next; self.__ice_rev[82] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_left, __ice_next) { self.account_ceremony_left = __ice_next; self.__ice_rev[83] += 1; } }
{ let __ice_next = ShellTab::Files; if ::ui_lang_runtime::state_changed!(self.shell_tab, __ice_next) { self.shell_tab = __ice_next; self.__ice_rev[55] += 1; } }
::ducktape_view_guest::Task::none()
})(),
DuckKind::ForgeRepo => (|| {
{ let __ice_next = url.to_owned(); if ::ui_lang_runtime::state_changed!(self.forge_link, __ice_next) { self.forge_link = __ice_next; self.__ice_rev[64] += 1; } }
{ let __ice_next = (self.forge_link_tick + 1); if ::ui_lang_runtime::state_changed!(self.forge_link_tick, __ice_next) { self.forge_link_tick = __ice_next; self.__ice_rev[65] += 1; } }
self.__ice_run_lane_10_generation = self.__ice_run_lane_10_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_10_handle.take() { __previous.abort(); }
self.__ice_run_lane_11_generation = self.__ice_run_lane_11_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_11_handle.take() { __previous.abort(); }
{ let __ice_next = (self.account_busy && (self.account_ceremony_phase).is_empty()); if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_phase, __ice_next) { self.account_ceremony_phase = __ice_next; self.__ice_rev[80] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_qr, __ice_next) { self.account_ceremony_qr = __ice_next; self.__ice_rev[81] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_detail, __ice_next) { self.account_ceremony_detail = __ice_next; self.__ice_rev[82] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_left, __ice_next) { self.account_ceremony_left = __ice_next; self.__ice_rev[83] += 1; } }
{ let __ice_next = ShellTab::Forge; if ::ui_lang_runtime::state_changed!(self.shell_tab, __ice_next) { self.shell_tab = __ice_next; self.__ice_rev[55] += 1; } }
::ducktape_view_guest::Task::none()
})(),
DuckKind::ForgeItem => (|| {
{ let __ice_next = url.to_owned(); if ::ui_lang_runtime::state_changed!(self.forge_link, __ice_next) { self.forge_link = __ice_next; self.__ice_rev[64] += 1; } }
{ let __ice_next = (self.forge_link_tick + 1); if ::ui_lang_runtime::state_changed!(self.forge_link_tick, __ice_next) { self.forge_link_tick = __ice_next; self.__ice_rev[65] += 1; } }
self.__ice_run_lane_10_generation = self.__ice_run_lane_10_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_10_handle.take() { __previous.abort(); }
self.__ice_run_lane_11_generation = self.__ice_run_lane_11_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_11_handle.take() { __previous.abort(); }
{ let __ice_next = (self.account_busy && (self.account_ceremony_phase).is_empty()); if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_phase, __ice_next) { self.account_ceremony_phase = __ice_next; self.__ice_rev[80] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_qr, __ice_next) { self.account_ceremony_qr = __ice_next; self.__ice_rev[81] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_detail, __ice_next) { self.account_ceremony_detail = __ice_next; self.__ice_rev[82] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_left, __ice_next) { self.account_ceremony_left = __ice_next; self.__ice_rev[83] += 1; } }
{ let __ice_next = ShellTab::Forge; if ::ui_lang_runtime::state_changed!(self.shell_tab, __ice_next) { self.shell_tab = __ice_next; self.__ice_rev[55] += 1; } }
::ducktape_view_guest::Task::none()
})(),
DuckKind::ForgeBlob => (|| {
{ let __ice_next = url.to_owned(); if ::ui_lang_runtime::state_changed!(self.forge_link, __ice_next) { self.forge_link = __ice_next; self.__ice_rev[64] += 1; } }
{ let __ice_next = (self.forge_link_tick + 1); if ::ui_lang_runtime::state_changed!(self.forge_link_tick, __ice_next) { self.forge_link_tick = __ice_next; self.__ice_rev[65] += 1; } }
self.__ice_run_lane_10_generation = self.__ice_run_lane_10_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_10_handle.take() { __previous.abort(); }
self.__ice_run_lane_11_generation = self.__ice_run_lane_11_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_11_handle.take() { __previous.abort(); }
{ let __ice_next = (self.account_busy && (self.account_ceremony_phase).is_empty()); if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_phase, __ice_next) { self.account_ceremony_phase = __ice_next; self.__ice_rev[80] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_qr, __ice_next) { self.account_ceremony_qr = __ice_next; self.__ice_rev[81] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_detail, __ice_next) { self.account_ceremony_detail = __ice_next; self.__ice_rev[82] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_left, __ice_next) { self.account_ceremony_left = __ice_next; self.__ice_rev[83] += 1; } }
{ let __ice_next = ShellTab::Forge; if ::ui_lang_runtime::state_changed!(self.shell_tab, __ice_next) { self.shell_tab = __ice_next; self.__ice_rev[55] += 1; } }
::ducktape_view_guest::Task::none()
})(),
DuckKind::Channel => (|| {
self.__ice_run_lane_10_generation = self.__ice_run_lane_10_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_10_handle.take() { __previous.abort(); }
self.__ice_run_lane_11_generation = self.__ice_run_lane_11_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_11_handle.take() { __previous.abort(); }
{ let __ice_next = (self.account_busy && (self.account_ceremony_phase).is_empty()); if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_phase, __ice_next) { self.account_ceremony_phase = __ice_next; self.__ice_rev[80] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_qr, __ice_next) { self.account_ceremony_qr = __ice_next; self.__ice_rev[81] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_detail, __ice_next) { self.account_ceremony_detail = __ice_next; self.__ice_rev[82] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_left, __ice_next) { self.account_ceremony_left = __ice_next; self.__ice_rev[83] += 1; } }
{ let __ice_next = ShellTab::Chat; if ::ui_lang_runtime::state_changed!(self.shell_tab, __ice_next) { self.shell_tab = __ice_next; self.__ice_rev[55] += 1; } }
return ::ducktape_view_guest::Task::perform(({ crate::backend::duck_echo_str(link.channel.to_owned()) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::ChooseChannel(value), ::std::result::Result::Err(error) => __DucktapeMessage::ExternalUrlFailed(error) });
})(),
DuckKind::ChannelMessage => (|| {
return { let __ice_run_route_154_1 = link.seq;
::ducktape_view_guest::Task::perform(({ crate::backend::duck_echo_str(link.channel.to_owned()) }), move |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::OpenChatSearchHit(value, __ice_run_route_154_1), ::std::result::Result::Err(error) => __DucktapeMessage::ExternalUrlFailed(error) }) };
})(),
DuckKind::Account => (|| {
self.__ice_run_lane_10_generation = self.__ice_run_lane_10_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_10_handle.take() { __previous.abort(); }
self.__ice_run_lane_11_generation = self.__ice_run_lane_11_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_11_handle.take() { __previous.abort(); }
{ let __ice_next = (self.account_busy && (self.account_ceremony_phase).is_empty()); if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_phase, __ice_next) { self.account_ceremony_phase = __ice_next; self.__ice_rev[80] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_qr, __ice_next) { self.account_ceremony_qr = __ice_next; self.__ice_rev[81] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_detail, __ice_next) { self.account_ceremony_detail = __ice_next; self.__ice_rev[82] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_left, __ice_next) { self.account_ceremony_left = __ice_next; self.__ice_rev[83] += 1; } }
{ let __ice_next = ShellTab::Chat; if ::ui_lang_runtime::state_changed!(self.shell_tab, __ice_next) { self.shell_tab = __ice_next; self.__ice_rev[55] += 1; } }
return ::ducktape_view_guest::Task::perform(({ crate::backend::duck_echo_str(link.account.to_owned()) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::ChooseDm(value), ::std::result::Result::Err(error) => __DucktapeMessage::ExternalUrlFailed(error) });
})(),
};
})(),
__DucktapeMessage::ChatScrolled(_absolute_x, _absolute_y, _relative_x, relative_y) => (|| {
let _ = &_absolute_x;
let _ = &_absolute_y;
let _ = &_relative_x;
let _ = &relative_y;
{ let __ice_next = crate::backend::near_scroll_tail(relative_y); if ::ui_lang_runtime::state_changed!(self.chat_at_tail, __ice_next) { self.chat_at_tail = __ice_next; self.__ice_rev[47] += 1; } }
{ let __ice_next = ((!self.chat_at_tail) || (self.chat_land_seq > 0)); if ::ui_lang_runtime::state_changed!(self.history_view, __ice_next) { self.history_view = __ice_next; self.__ice_rev[48] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::CopyChordPressed(event) => (|| {
let _ = &event;
if (!crate::backend::is_copy_chord(event.key.clone(), event.modifiers)) { return ::ducktape_view_guest::Task::none(); }
if (self.shell_tab != ShellTab::Chat) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = (self.chat_copy_chord_serial + 1); if ::ui_lang_runtime::state_changed!(self.chat_copy_chord_serial, __ice_next) { self.chat_copy_chord_serial = __ice_next; self.__ice_rev[42] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::ChatViewEvent(event) => (|| {
let _ = &event;
return match crate::module_view::chat_intent(::std::borrow::Borrow::borrow(&(event))) {
ChatIntent::OpenHit => (|| {
let target_seq = crate::module_view::event_int(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("target_seq")));
return { let __ice_run_route_158_1 = target_seq;
::ducktape_view_guest::Task::perform(({ crate::backend::duck_echo_str(crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("channel")))) }), move |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::OpenChatSearchHit(value, __ice_run_route_158_1), ::std::result::Result::Err(error) => __DucktapeMessage::ExternalUrlFailed(error) }) };
})(),
ChatIntent::ToggleCreate => (|| {
return (::ducktape_view_guest::Task::done(true)).map(|value| { let _ = &value; __DucktapeMessage::ToggleChannelCreate });
})(),
ChatIntent::ChooseChannel => (|| {
return (::ducktape_view_guest::Task::done(crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("id"))))).map(|value| __DucktapeMessage::ChooseChannel(value));
})(),
ChatIntent::ChooseDm => (|| {
return (::ducktape_view_guest::Task::done(crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("key"))))).map(|value| __DucktapeMessage::ChooseDm(value));
})(),
ChatIntent::ShowHuddle => (|| {
return (::ducktape_view_guest::Task::done(true)).map(|value| { let _ = &value; __DucktapeMessage::ShowHuddle });
})(),
ChatIntent::LeaveHuddle => (|| {
return (::ducktape_view_guest::Task::done(true)).map(|value| { let _ = &value; __DucktapeMessage::LeaveHuddleHere });
})(),
ChatIntent::JoinHuddle => (|| {
return (::ducktape_view_guest::Task::done(true)).map(|value| { let _ = &value; __DucktapeMessage::JoinHuddleSubmit });
})(),
ChatIntent::Scrolled => (|| {
let absolute_y = crate::module_view::event_num(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("absolute_y")));
let relative_x = crate::module_view::event_num(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("relative_x")));
let relative_y = crate::module_view::event_num(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("relative_y")));
return { let __ice_run_route_166_1 = absolute_y;
let __ice_run_route_166_2 = relative_x;
let __ice_run_route_166_3 = relative_y;
::ducktape_view_guest::Task::perform(({ crate::backend::duck_echo_f64(crate::module_view::event_num(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("absolute_x")))) }), move |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::ChatScrolled(value, __ice_run_route_166_1, __ice_run_route_166_2, __ice_run_route_166_3), ::std::result::Result::Err(error) => __DucktapeMessage::ExternalUrlFailed(error) }) };
})(),
ChatIntent::OpenLink => (|| {
return (::ducktape_view_guest::Task::done(crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("url"))))).map(|value| __DucktapeMessage::OpenMessageLink(value));
})(),
ChatIntent::Copy => (|| {
let label = crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("label")));
return { let __ice_run_route_169_1 = label.to_owned();
::ducktape_view_guest::Task::perform(({ crate::backend::duck_echo_str(crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("text")))) }), move |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::CopyToClipboard(value, __ice_run_route_169_1), ::std::result::Result::Err(error) => __DucktapeMessage::ExternalUrlFailed(error) }) };
})(),
ChatIntent::CopyLink => (|| {
return (::ducktape_view_guest::Task::done(crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("link"))))).map(|value| __DucktapeMessage::CopyMessageLink(value));
})(),
ChatIntent::BeginEdit => (|| {
let body = crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("body")));
let seq = crate::module_view::event_int(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("seq")));
let rev = crate::module_view::event_int(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("rev")));
return { let __ice_run_route_172_1 = body.to_owned();
let __ice_run_route_172_2 = seq;
let __ice_run_route_172_3 = rev;
::ducktape_view_guest::Task::perform(({ crate::backend::duck_echo_str(crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("scope")))) }), move |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::ChatBeginEdit(value, __ice_run_route_172_1, __ice_run_route_172_2, __ice_run_route_172_3), ::std::result::Result::Err(error) => __DucktapeMessage::ExternalUrlFailed(error) }) };
})(),
ChatIntent::CancelRun => (|| {
return ::ducktape_view_guest::Task::perform(({ crate::backend::cancel_agent_run(self.connected_rpc.to_owned(), self.password.to_owned(), crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("run_id")))) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::LiveCancelAcked(value), ::std::result::Result::Err(error) => __DucktapeMessage::MutationFailed(error) });
})(),
ChatIntent::OpenRun => (|| {
return (::ducktape_view_guest::Task::done(crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("dispatch_id"))))).map(|value| __DucktapeMessage::OpenRunPanel(value));
})(),
ChatIntent::Composer => (|| {
let kind = crate::module_view::chat_event_kind(::std::borrow::Borrow::borrow(&(event)));
let id = crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("id")));
let scope = crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("scope")));
return { let __ice_run_route_177_0 = kind.clone();
let __ice_run_route_177_2 = id.to_owned();
let __ice_run_route_177_3 = scope.to_owned();
::ducktape_view_guest::Task::perform(({ crate::backend::duck_echo_str(crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("body")))) }), move |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::ComposerSubmitted(__ice_run_route_177_0, value, __ice_run_route_177_2, __ice_run_route_177_3), ::std::result::Result::Err(error) => __DucktapeMessage::ExternalUrlFailed(error) }) };
})(),
};
})(),
__DucktapeMessage::PagesViewEvent(event) => (|| {
let _ = &event;
return match crate::module_view::pages_intent(::std::borrow::Borrow::borrow(&(event))) {
PagesIntent::OpenLink => (|| {
return (::ducktape_view_guest::Task::done(crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("link"))))).map(|value| __DucktapeMessage::OpenMessageLink(value));
})(),
PagesIntent::Copy => (|| {
{ let __ice_next = crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("label"))); if ::ui_lang_runtime::state_changed!(self.toast, __ice_next) { self.toast = __ice_next; self.__ice_rev[117] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.toast_age, __ice_next) { self.toast_age = __ice_next; self.__ice_rev[118] += 1; } }
return ::crate::shell::clipboard::<__DucktapeMessage>(crate::module_view::event_text(::std::borrow::Borrow::borrow(&(event)), ::std::convert::AsRef::as_ref(&("text"))));
})(),
};
})(),
__DucktapeMessage::OpenPageSearchHit(page_id, _block_id) => (|| {
let _ = &page_id;
let _ = &_block_id;
if (page_id).is_empty() { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.palette_open, __ice_next) { self.palette_open = __ice_next; self.__ice_rev[104] += 1; } }
{ let __ice_next = ShellTab::Pages; if ::ui_lang_runtime::state_changed!(self.shell_tab, __ice_next) { self.shell_tab = __ice_next; self.__ice_rev[55] += 1; } }
{ let __ice_next = page_id.to_owned(); if ::ui_lang_runtime::state_changed!(self.page_route, __ice_next) { self.page_route = __ice_next; self.__ice_rev[119] += 1; } }
{ let __ice_next = (self.page_route_serial + 1); if ::ui_lang_runtime::state_changed!(self.page_route_serial, __ice_next) { self.page_route_serial = __ice_next; self.__ice_rev[120] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::ExternalUrlOpened(_opened) => (|| {
let _ = &_opened;
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::ExternalUrlFailed(cause) => (|| {
let _ = &cause;
{ let __ice_next = cause.message.to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::OnboardingOpened(id) => (|| {
let _ = &id;
{ let __ice_next = ::std::option::Option::Some(id); if ::ui_lang_runtime::state_changed!(self.onboarding_win, __ice_next) { self.onboarding_win = __ice_next; self.__ice_rev[121] += 1; } }
return ::ducktape_view_guest::Task::batch([
{ // __ICE_SOURCE 31 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6f6e626f617264696e672e696365
{ let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::load_appearance() }), |value| __DucktapeMessage::AppearanceLoaded(value)); self.__ice_run_lane_17_generation = self.__ice_run_lane_17_generation.wrapping_add(1); let __generation = self.__ice_run_lane_17_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_17_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane17(__generation, ::std::boxed::Box::new(__message))) }
},
{ // __ICE_SOURCE 32 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6f6e626f617264696e672e696365
{ let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::load_desktop_notifications() }), |value| __DucktapeMessage::DesktopNotificationsLoaded(value)); self.__ice_run_lane_18_generation = self.__ice_run_lane_18_generation.wrapping_add(1); let __generation = self.__ice_run_lane_18_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_18_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane18(__generation, ::std::boxed::Box::new(__message))) }
},
{ // __ICE_SOURCE 33 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6f6e626f617264696e672e696365
{ let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::hub_state() }), |value| __DucktapeMessage::HubBooted(value)); self.__ice_run_lane_19_generation = self.__ice_run_lane_19_generation.wrapping_add(1); let __generation = self.__ice_run_lane_19_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_19_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane19(__generation, ::std::boxed::Box::new(__message))) }
},
]);
})(),
__DucktapeMessage::HubBooted(state) => (|| {
let _ = &state;
{ let __ice_next = state.networks.clone(); if ::ui_lang_runtime::state_changed!(self.hub_networks, __ice_next) { self.hub_networks = __ice_next; self.__ice_rev[126] += 1; } }
{ let __ice_next = state.preselect.to_owned(); if ::ui_lang_runtime::state_changed!(self.hub_selected, __ice_next) { self.hub_selected = __ice_next; self.__ice_rev[127] += 1; } }
{ let __ice_next = HubStep::Networks; if ::ui_lang_runtime::state_changed!(self.hub_step, __ice_next) { self.hub_step = __ice_next; self.__ice_rev[125] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
return { let __task = ::ducktape_view_guest::Task::run(({ crate::backend::probe_known_networks() }), |value| __DucktapeMessage::NetworkProbed(value)); self.__ice_run_lane_20_generation = self.__ice_run_lane_20_generation.wrapping_add(1); let __generation = self.__ice_run_lane_20_generation; let __task = __task.map(move |__message| __DucktapeMessage::__RequestLane20(__generation, ::std::option::Option::Some(::std::boxed::Box::new(__message)))).chain(::ducktape_view_guest::Task::done(__DucktapeMessage::__RequestLane20(__generation, ::std::option::Option::None))); let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_20_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task };
})(),
__DucktapeMessage::HubRefreshed(state) => (|| {
let _ = &state;
{ let __ice_next = state.networks.clone(); if ::ui_lang_runtime::state_changed!(self.hub_networks, __ice_next) { self.hub_networks = __ice_next; self.__ice_rev[126] += 1; } }
{ let __ice_next = crate::backend::refreshed_hub_selection(state.networks.clone(), self.hub_selected.to_owned(), state.preselect.to_owned()); if ::ui_lang_runtime::state_changed!(self.hub_selected, __ice_next) { self.hub_selected = __ice_next; self.__ice_rev[127] += 1; } }
return { let __task = ::ducktape_view_guest::Task::run(({ crate::backend::probe_known_networks() }), |value| __DucktapeMessage::NetworkProbed(value)); self.__ice_run_lane_20_generation = self.__ice_run_lane_20_generation.wrapping_add(1); let __generation = self.__ice_run_lane_20_generation; let __task = __task.map(move |__message| __DucktapeMessage::__RequestLane20(__generation, ::std::option::Option::Some(::std::boxed::Box::new(__message)))).chain(::ducktape_view_guest::Task::done(__DucktapeMessage::__RequestLane20(__generation, ::std::option::Option::None))); let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_20_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task };
})(),
__DucktapeMessage::NetworkProbed(probe) => (|| {
let _ = &probe;
self.hub_networks = crate::backend::apply_network_probe(::std::mem::take(&mut self.hub_networks), probe.clone()); self.__ice_rev[126] += 1;
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::PickWallet(name) => (|| {
let _ = &name;
{ let __ice_next = name.to_owned(); if ::ui_lang_runtime::state_changed!(self.hub_wallet_selected, __ice_next) { self.hub_wallet_selected = __ice_next; self.__ice_rev[129] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::UnlockSubmit(pw) => (|| {
let _ = &pw;
if (((self.mutation_phase != MutationPhase::Idle) || (pw).is_empty()) || (self.hub_wallet_selected).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = pw.to_owned(); if ::ui_lang_runtime::state_changed!(self.password, __ice_next) { self.password = __ice_next; self.__ice_rev[6] += 1; } }
{ let __ice_next = MutationPhase::Onboarding; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
return ::ducktape_view_guest::Task::perform(({ crate::backend::unlock_wallet(self.rpc.to_owned(), self.hub_wallet_selected.to_owned(), self.password.to_owned()) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::KeyUnlocked(value), ::std::result::Result::Err(error) => __DucktapeMessage::LoginFailed(error) });
})(),
__DucktapeMessage::KeyUnlocked(pubkey) => (|| {
let _ = &pubkey;
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = pubkey.to_owned(); if ::ui_lang_runtime::state_changed!(self.signer_key, __ice_next) { self.signer_key = __ice_next; self.__ice_rev[17] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.live_agents, __ice_next) { self.live_agents = __ice_next; self.__ice_rev[39] += 1; } }
return ::ducktape_view_guest::Task::batch([
{ // __ICE_SOURCE 78 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6f6e626f617264696e672e696365
{ let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::load_account(self.rpc.to_owned(), self.account_generation) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::AccountProbed(value), ::std::result::Result::Err(error) => __DucktapeMessage::AccountProbeFailed(error) }); self.__ice_run_lane_21_generation = self.__ice_run_lane_21_generation.wrapping_add(1); let __generation = self.__ice_run_lane_21_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_21_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane21(__generation, ::std::boxed::Box::new(__message))) }
},
{ // __ICE_SOURCE 79 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6f6e626f617264696e672e696365
{ let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::chain_id_of(self.rpc.to_owned()) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::ChainNamed(value), ::std::result::Result::Err(error) => __DucktapeMessage::ChainProbeFailed(error) }); self.__ice_run_lane_22_generation = self.__ice_run_lane_22_generation.wrapping_add(1); let __generation = self.__ice_run_lane_22_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_22_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane22(__generation, ::std::boxed::Box::new(__message))) }
},
]);
})(),
__DucktapeMessage::LoginSkip => (|| {
if (self.mutation_phase != MutationPhase::Idle) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.password, __ice_next) { self.password = __ice_next; self.__ice_rev[6] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.hub_wallet_selected, __ice_next) { self.hub_wallet_selected = __ice_next; self.__ice_rev[129] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
return (::ducktape_view_guest::Task::done(true)).map(|value| { let _ = &value; __DucktapeMessage::NetworkEntered });
})(),
__DucktapeMessage::PasswordSubmit(pw) => (|| {
let _ = &pw;
if ((self.mutation_phase != MutationPhase::Idle) || (pw).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = pw.to_owned(); if ::ui_lang_runtime::state_changed!(self.password, __ice_next) { self.password = __ice_next; self.__ice_rev[6] += 1; } }
{ let __ice_next = MutationPhase::Onboarding; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
return ::ducktape_view_guest::Task::perform(({ crate::backend::create_device_key(self.rpc.to_owned(), self.password.to_owned()) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::DeviceKeyCreated(value), ::std::result::Result::Err(error) => __DucktapeMessage::LoginFailed(error) });
})(),
__DucktapeMessage::DeviceKeyCreated(_name) => (|| {
let _ = &_name;
{ let __ice_next = MutationPhase::Idle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = HubStep::Phrase; if ::ui_lang_runtime::state_changed!(self.hub_step, __ice_next) { self.hub_step = __ice_next; self.__ice_rev[125] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::PhraseWrittenDown => (|| {
if (self.mutation_phase != MutationPhase::Idle) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = HubStep::Confirm; if ::ui_lang_runtime::state_changed!(self.hub_step, __ice_next) { self.hub_step = __ice_next; self.__ice_rev[125] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::ShowPhraseAgain => (|| {
if (self.mutation_phase != MutationPhase::Idle) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = HubStep::Phrase; if ::ui_lang_runtime::state_changed!(self.hub_step, __ice_next) { self.hub_step = __ice_next; self.__ice_rev[125] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::ConfirmPhraseSubmit(answer) => (|| {
let _ = &answer;
if ((self.mutation_phase != MutationPhase::Idle) || ((answer).trim().to_owned()).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = MutationPhase::Onboarding; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
return ::ducktape_view_guest::Task::perform(({ crate::backend::confirm_recovery_phrase(self.rpc.to_owned(), answer.to_owned(), self.password.to_owned()) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::PhraseConfirmed(value), ::std::result::Result::Err(error) => __DucktapeMessage::PhraseConfirmFailed(error) });
})(),
__DucktapeMessage::PhraseConfirmed(pubkey) => (|| {
let _ = &pubkey;
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = pubkey.to_owned(); if ::ui_lang_runtime::state_changed!(self.signer_key, __ice_next) { self.signer_key = __ice_next; self.__ice_rev[17] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.live_agents, __ice_next) { self.live_agents = __ice_next; self.__ice_rev[39] += 1; } }
return ::ducktape_view_guest::Task::batch([
{ // __ICE_SOURCE 138 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6f6e626f617264696e672e696365
{ let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::load_account(self.rpc.to_owned(), self.account_generation) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::AccountProbed(value), ::std::result::Result::Err(error) => __DucktapeMessage::AccountProbeFailed(error) }); self.__ice_run_lane_21_generation = self.__ice_run_lane_21_generation.wrapping_add(1); let __generation = self.__ice_run_lane_21_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_21_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane21(__generation, ::std::boxed::Box::new(__message))) }
},
{ // __ICE_SOURCE 139 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6f6e626f617264696e672e696365
{ let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::chain_id_of(self.rpc.to_owned()) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::ChainNamed(value), ::std::result::Result::Err(error) => __DucktapeMessage::ChainProbeFailed(error) }); self.__ice_run_lane_22_generation = self.__ice_run_lane_22_generation.wrapping_add(1); let __generation = self.__ice_run_lane_22_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_22_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane22(__generation, ::std::boxed::Box::new(__message))) }
},
]);
})(),
__DucktapeMessage::PhraseConfirmFailed(cause) => (|| {
let _ = &cause;
{ let __ice_next = MutationPhase::Idle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = cause.message.to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::GoRestore => (|| {
if (self.mutation_phase != MutationPhase::Idle) { return ::ducktape_view_guest::Task::none(); }
self.__ice_secrets.clear("restore_words");
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = HubStep::Restore; if ::ui_lang_runtime::state_changed!(self.hub_step, __ice_next) { self.hub_step = __ice_next; self.__ice_rev[125] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::GoLogin => (|| {
if (self.mutation_phase != MutationPhase::Idle) { return ::ducktape_view_guest::Task::none(); }
self.__ice_secrets.clear("restore_words");
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = crate::backend::hub_entry_step(self.hub_wallets.clone()); if ::ui_lang_runtime::state_changed!(self.hub_step, __ice_next) { self.hub_step = __ice_next; self.__ice_rev[125] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::RestoreSubmit(name, pw) => (|| {
let _ = &name;
let _ = &pw;
if ((((self.mutation_phase != MutationPhase::Idle) || (self.__ice_secrets.text("restore_words")).is_empty()) || (pw).is_empty()) || (name).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = pw.to_owned(); if ::ui_lang_runtime::state_changed!(self.password, __ice_next) { self.password = __ice_next; self.__ice_rev[6] += 1; } }
{ let __ice_next = name.to_owned(); if ::ui_lang_runtime::state_changed!(self.hub_wallet_selected, __ice_next) { self.hub_wallet_selected = __ice_next; self.__ice_rev[129] += 1; } }
{ let __ice_next = MutationPhase::Onboarding; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
return ::ducktape_view_guest::Task::perform(({ crate::backend::restore_user_key(self.rpc.to_owned(), name.to_owned(), self.__ice_secrets.read("restore_words"), self.password.to_owned()) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::KeyRestored(value), ::std::result::Result::Err(error) => __DucktapeMessage::LoginFailed(error) });
})(),
__DucktapeMessage::KeyRestored(pubkey) => (|| {
let _ = &pubkey;
self.__ice_secrets.clear("restore_words");
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = pubkey.to_owned(); if ::ui_lang_runtime::state_changed!(self.signer_key, __ice_next) { self.signer_key = __ice_next; self.__ice_rev[17] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.live_agents, __ice_next) { self.live_agents = __ice_next; self.__ice_rev[39] += 1; } }
return ::ducktape_view_guest::Task::batch([
{ // __ICE_SOURCE 175 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6f6e626f617264696e672e696365
{ let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::load_account(self.rpc.to_owned(), self.account_generation) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::AccountProbed(value), ::std::result::Result::Err(error) => __DucktapeMessage::AccountProbeFailed(error) }); self.__ice_run_lane_21_generation = self.__ice_run_lane_21_generation.wrapping_add(1); let __generation = self.__ice_run_lane_21_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_21_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane21(__generation, ::std::boxed::Box::new(__message))) }
},
{ // __ICE_SOURCE 176 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6f6e626f617264696e672e696365
{ let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::chain_id_of(self.rpc.to_owned()) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::ChainNamed(value), ::std::result::Result::Err(error) => __DucktapeMessage::ChainProbeFailed(error) }); self.__ice_run_lane_22_generation = self.__ice_run_lane_22_generation.wrapping_add(1); let __generation = self.__ice_run_lane_22_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_22_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane22(__generation, ::std::boxed::Box::new(__message))) }
},
]);
})(),
__DucktapeMessage::LoginFailed(cause) => (|| {
let _ = &cause;
{ let __ice_next = MutationPhase::Idle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.password, __ice_next) { self.password = __ice_next; self.__ice_rev[6] += 1; } }
{ let __ice_next = cause.message.to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::PickNetwork(id) => (|| {
let _ = &id;
{ let __ice_next = id.to_owned(); if ::ui_lang_runtime::state_changed!(self.hub_selected, __ice_next) { self.hub_selected = __ice_next; self.__ice_rev[127] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::OpenNetworkSubmit => (|| {
if ((self.mutation_phase != MutationPhase::Idle) || (crate::backend::selected_network_endpoint(self.hub_networks.clone(), self.hub_selected.to_owned())).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = crate::backend::selected_network_endpoint(self.hub_networks.clone(), self.hub_selected.to_owned()); if ::ui_lang_runtime::state_changed!(self.rpc, __ice_next) { self.rpc = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = crate::backend::selected_network_name(self.hub_networks.clone(), self.hub_selected.to_owned()); if ::ui_lang_runtime::state_changed!(self.network_name, __ice_next) { self.network_name = __ice_next; self.__ice_rev[124] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.password, __ice_next) { self.password = __ice_next; self.__ice_rev[6] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.hub_wallet_selected, __ice_next) { self.hub_wallet_selected = __ice_next; self.__ice_rev[129] += 1; } }
{ let __ice_next = MutationPhase::Onboarding; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::load_wallets(self.rpc.to_owned()) }), |value| __DucktapeMessage::WalletsLoaded(value)); self.__ice_run_lane_23_generation = self.__ice_run_lane_23_generation.wrapping_add(1); let __generation = self.__ice_run_lane_23_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_23_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane23(__generation, ::std::boxed::Box::new(__message))) };
})(),
__DucktapeMessage::ConnectRemoteSubmit(endpoint) => (|| {
let _ = &endpoint;
if ((self.mutation_phase != MutationPhase::Idle) || ((endpoint).trim().to_owned()).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = ({ crate::backend::canonical_endpoint(endpoint.to_owned()) }); if ::ui_lang_runtime::state_changed!(self.rpc, __ice_next) { self.rpc = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = crate::backend::network_label("".to_owned(), self.rpc.to_owned()); if ::ui_lang_runtime::state_changed!(self.network_name, __ice_next) { self.network_name = __ice_next; self.__ice_rev[124] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.password, __ice_next) { self.password = __ice_next; self.__ice_rev[6] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.hub_wallet_selected, __ice_next) { self.hub_wallet_selected = __ice_next; self.__ice_rev[129] += 1; } }
{ let __ice_next = MutationPhase::Onboarding; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::load_wallets(self.rpc.to_owned()) }), |value| __DucktapeMessage::WalletsLoaded(value)); self.__ice_run_lane_23_generation = self.__ice_run_lane_23_generation.wrapping_add(1); let __generation = self.__ice_run_lane_23_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_23_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane23(__generation, ::std::boxed::Box::new(__message))) };
})(),
__DucktapeMessage::WalletsLoaded(list) => (|| {
let _ = &list;
let door = crate::backend::wallet_door(::std::borrow::Borrow::borrow(&(list)));
{ let __ice_next = MutationPhase::Idle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = list.wallets.clone(); if ::ui_lang_runtime::state_changed!(self.hub_wallets, __ice_next) { self.hub_wallets = __ice_next; self.__ice_rev[128] += 1; } }
{ let __ice_next = crate::backend::preselect_wallet(list.wallets.clone()); if ::ui_lang_runtime::state_changed!(self.hub_wallet_selected, __ice_next) { self.hub_wallet_selected = __ice_next; self.__ice_rev[129] += 1; } }
{ let __ice_next = list.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
return match door.clone() {
WalletDoor::Wallets => (|| {
{ let __ice_next = HubStep::Wallets; if ::ui_lang_runtime::state_changed!(self.hub_step, __ice_next) { self.hub_step = __ice_next; self.__ice_rev[125] += 1; } }
::ducktape_view_guest::Task::none()
})(),
WalletDoor::Password => (|| {
{ let __ice_next = HubStep::Password; if ::ui_lang_runtime::state_changed!(self.hub_step, __ice_next) { self.hub_step = __ice_next; self.__ice_rev[125] += 1; } }
::ducktape_view_guest::Task::none()
})(),
WalletDoor::Unreached => (|| {
{ let __ice_next = self.hub_step.clone(); if ::ui_lang_runtime::state_changed!(self.hub_step, __ice_next) { self.hub_step = __ice_next; self.__ice_rev[125] += 1; } }
::ducktape_view_guest::Task::none()
})(),
};
})(),
__DucktapeMessage::ChainNamed(id) => (|| {
let _ = &id;
{ let __ice_next = id.to_owned(); if ::ui_lang_runtime::state_changed!(self.hub_chain_id, __ice_next) { self.hub_chain_id = __ice_next; self.__ice_rev[136] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::ChainProbeFailed(_cause) => (|| {
let _ = &_cause;
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.hub_chain_id, __ice_next) { self.hub_chain_id = __ice_next; self.__ice_rev[136] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::AccountProbed(next) => (|| {
let _ = &next;
{ let __ice_next = MutationPhase::Idle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
let probe = crate::backend::account_probe(next.exists);
return match probe.clone() {
AccountProbe::Found => (|| {
return (::ducktape_view_guest::Task::done(true)).map(|value| { let _ = &value; __DucktapeMessage::NetworkEntered });
})(),
AccountProbe::Missing => (|| {
{ let __ice_next = crate::backend::network_label(self.hub_chain_id.to_owned(), self.rpc.to_owned()); if ::ui_lang_runtime::state_changed!(self.network_name, __ice_next) { self.network_name = __ice_next; self.__ice_rev[124] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_phase, __ice_next) { self.ceremony_phase = __ice_next; self.__ice_rev[138] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_qr, __ice_next) { self.ceremony_qr = __ice_next; self.__ice_rev[139] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_detail, __ice_next) { self.ceremony_detail = __ice_next; self.__ice_rev[140] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_left, __ice_next) { self.ceremony_left = __ice_next; self.__ice_rev[141] += 1; } }
{ let __ice_next = HubStep::Account; if ::ui_lang_runtime::state_changed!(self.hub_step, __ice_next) { self.hub_step = __ice_next; self.__ice_rev[125] += 1; } }
::ducktape_view_guest::Task::none()
})(),
};
})(),
__DucktapeMessage::AccountProbeFailed(cause) => (|| {
let _ = &cause;
{ let __ice_next = MutationPhase::Idle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = cause.message.to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::WelcomeSkip => (|| {
if (self.mutation_phase != MutationPhase::Idle) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
return (::ducktape_view_guest::Task::done(true)).map(|value| { let _ = &value; __DucktapeMessage::NetworkEntered });
})(),
__DucktapeMessage::WelcomeCancel => (|| {
self.__ice_run_lane_24_generation = self.__ice_run_lane_24_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_24_handle.take() { __previous.abort(); }
self.__ice_run_lane_25_generation = self.__ice_run_lane_25_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_25_handle.take() { __previous.abort(); }
{ let __ice_next = MutationPhase::Idle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_phase, __ice_next) { self.ceremony_phase = __ice_next; self.__ice_rev[138] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_qr, __ice_next) { self.ceremony_qr = __ice_next; self.__ice_rev[139] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_detail, __ice_next) { self.ceremony_detail = __ice_next; self.__ice_rev[140] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_left, __ice_next) { self.ceremony_left = __ice_next; self.__ice_rev[141] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::WelcomeCreateSubmit(name) => (|| {
let _ = &name;
if (((self.mutation_phase != MutationPhase::Idle) || (name).is_empty()) || (self.hub_chain_id).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = MutationPhase::Onboarding; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
return { let __task = ::ducktape_view_guest::Task::run(({ crate::backend::create_account_by_qr(self.rpc.to_owned(), self.password.to_owned(), self.hub_chain_id.to_owned(), name.to_owned()) }), |value| __DucktapeMessage::CeremonyStepped(value)); self.__ice_run_lane_24_generation = self.__ice_run_lane_24_generation.wrapping_add(1); let __generation = self.__ice_run_lane_24_generation; let __task = __task.map(move |__message| __DucktapeMessage::__RequestLane24(__generation, ::std::option::Option::Some(::std::boxed::Box::new(__message)))).chain(::ducktape_view_guest::Task::done(__DucktapeMessage::__RequestLane24(__generation, ::std::option::Option::None))); let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_24_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task };
})(),
__DucktapeMessage::WelcomeLoginSubmit => (|| {
if ((self.mutation_phase != MutationPhase::Idle) || (self.hub_chain_id).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = MutationPhase::Onboarding; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
return { let __task = ::ducktape_view_guest::Task::run(({ crate::backend::login_by_qr(self.rpc.to_owned(), self.password.to_owned(), self.hub_chain_id.to_owned()) }), |value| __DucktapeMessage::CeremonyStepped(value)); self.__ice_run_lane_24_generation = self.__ice_run_lane_24_generation.wrapping_add(1); let __generation = self.__ice_run_lane_24_generation; let __task = __task.map(move |__message| __DucktapeMessage::__RequestLane24(__generation, ::std::option::Option::Some(::std::boxed::Box::new(__message)))).chain(::ducktape_view_guest::Task::done(__DucktapeMessage::__RequestLane24(__generation, ::std::option::Option::None))); let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_24_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task };
})(),
__DucktapeMessage::WelcomeDesktop => (|| {
if (self.ceremony_phase != "show_qr") { return ::ducktape_view_guest::Task::none(); }
self.__ice_run_lane_24_generation = self.__ice_run_lane_24_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_24_handle.take() { __previous.abort(); }
{ let __ice_next = "working".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_phase, __ice_next) { self.ceremony_phase = __ice_next; self.__ice_rev[138] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_qr, __ice_next) { self.ceremony_qr = __ice_next; self.__ice_rev[139] += 1; } }
{ let __ice_next = "Continue in the browser…".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_detail, __ice_next) { self.ceremony_detail = __ice_next; self.__ice_rev[140] += 1; } }
let door = crate::backend::welcome_door(::std::convert::AsRef::as_ref(&(self.welcome_name_draft)));
return match door.clone() {
WelcomeDoor::Create => (|| {
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::register_passkey(self.rpc.to_owned(), self.password.to_owned(), self.hub_chain_id.to_owned(), "".to_owned()) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::WelcomeDesktopDone(value), ::std::result::Result::Err(error) => __DucktapeMessage::WelcomeFailed(error) }); self.__ice_run_lane_25_generation = self.__ice_run_lane_25_generation.wrapping_add(1); let __generation = self.__ice_run_lane_25_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_25_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane25(__generation, ::std::boxed::Box::new(__message))) };
})(),
WelcomeDoor::Login => (|| {
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::login_with_passkey(self.rpc.to_owned(), self.password.to_owned(), self.hub_chain_id.to_owned(), "".to_owned()) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::WelcomeDesktopDone(value), ::std::result::Result::Err(error) => __DucktapeMessage::WelcomeFailed(error) }); self.__ice_run_lane_25_generation = self.__ice_run_lane_25_generation.wrapping_add(1); let __generation = self.__ice_run_lane_25_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_25_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane25(__generation, ::std::boxed::Box::new(__message))) };
})(),
};
})(),
__DucktapeMessage::WelcomeDesktopDone(_ok) => (|| {
let _ = &_ok;
{ let __ice_next = MutationPhase::Idle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_phase, __ice_next) { self.ceremony_phase = __ice_next; self.__ice_rev[138] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_qr, __ice_next) { self.ceremony_qr = __ice_next; self.__ice_rev[139] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_detail, __ice_next) { self.ceremony_detail = __ice_next; self.__ice_rev[140] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_left, __ice_next) { self.ceremony_left = __ice_next; self.__ice_rev[141] += 1; } }
return (::ducktape_view_guest::Task::done(true)).map(|value| { let _ = &value; __DucktapeMessage::NetworkEntered });
})(),
__DucktapeMessage::CeremonyStepped(next) => (|| {
let _ = &next;
let phase = crate::backend::ceremony_phase(::std::borrow::Borrow::borrow(&(next)));
{ let __ice_next = next.phase.to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_phase, __ice_next) { self.ceremony_phase = __ice_next; self.__ice_rev[138] += 1; } }
{ let __ice_next = next.qr.to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_qr, __ice_next) { self.ceremony_qr = __ice_next; self.__ice_rev[139] += 1; } }
{ let __ice_next = next.detail.to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_detail, __ice_next) { self.ceremony_detail = __ice_next; self.__ice_rev[140] += 1; } }
{ let __ice_next = next.left.to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_left, __ice_next) { self.ceremony_left = __ice_next; self.__ice_rev[141] += 1; } }
return match phase.clone() {
CeremonyPhase::Done => (|| {
{ let __ice_next = MutationPhase::Idle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_phase, __ice_next) { self.ceremony_phase = __ice_next; self.__ice_rev[138] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_qr, __ice_next) { self.ceremony_qr = __ice_next; self.__ice_rev[139] += 1; } }
return (::ducktape_view_guest::Task::done(true)).map(|value| { let _ = &value; __DucktapeMessage::NetworkEntered });
})(),
CeremonyPhase::Failed => (|| {
{ let __ice_next = MutationPhase::Idle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_phase, __ice_next) { self.ceremony_phase = __ice_next; self.__ice_rev[138] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_qr, __ice_next) { self.ceremony_qr = __ice_next; self.__ice_rev[139] += 1; } }
{ let __ice_next = next.detail.to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
::ducktape_view_guest::Task::none()
})(),
CeremonyPhase::ShowQr => (|| {
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
::ducktape_view_guest::Task::none()
})(),
CeremonyPhase::Working => (|| {
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
::ducktape_view_guest::Task::none()
})(),
};
})(),
__DucktapeMessage::WelcomeFailed(cause) => (|| {
let _ = &cause;
{ let __ice_next = MutationPhase::Idle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_phase, __ice_next) { self.ceremony_phase = __ice_next; self.__ice_rev[138] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_qr, __ice_next) { self.ceremony_qr = __ice_next; self.__ice_rev[139] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_detail, __ice_next) { self.ceremony_detail = __ice_next; self.__ice_rev[140] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_left, __ice_next) { self.ceremony_left = __ice_next; self.__ice_rev[141] += 1; } }
{ let __ice_next = cause.message.to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::NetworkEntered => (|| {
self.__ice_run_lane_24_generation = self.__ice_run_lane_24_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_24_handle.take() { __previous.abort(); }
self.__ice_run_lane_25_generation = self.__ice_run_lane_25_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_25_handle.take() { __previous.abort(); }
{ let __ice_next = MutationPhase::Idle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_phase, __ice_next) { self.ceremony_phase = __ice_next; self.__ice_rev[138] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_qr, __ice_next) { self.ceremony_qr = __ice_next; self.__ice_rev[139] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_detail, __ice_next) { self.ceremony_detail = __ice_next; self.__ice_rev[140] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_left, __ice_next) { self.ceremony_left = __ice_next; self.__ice_rev[141] += 1; } }
self.__ice_run_lane_10_generation = self.__ice_run_lane_10_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_10_handle.take() { __previous.abort(); }
self.__ice_run_lane_11_generation = self.__ice_run_lane_11_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_11_handle.take() { __previous.abort(); }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.account_banner_dismissed, __ice_next) { self.account_banner_dismissed = __ice_next; self.__ice_rev[79] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_phase, __ice_next) { self.account_ceremony_phase = __ice_next; self.__ice_rev[80] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_qr, __ice_next) { self.account_ceremony_qr = __ice_next; self.__ice_rev[81] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_detail, __ice_next) { self.account_ceremony_detail = __ice_next; self.__ice_rev[82] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_left, __ice_next) { self.account_ceremony_left = __ice_next; self.__ice_rev[83] += 1; } }
self.__ice_run_lane_15_generation = self.__ice_run_lane_15_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_15_handle.take() { __previous.abort(); }
self.__ice_run_lane_16_generation = self.__ice_run_lane_16_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_16_handle.take() { __previous.abort(); }
self.__ice_run_lane_8_generation = self.__ice_run_lane_8_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_8_handle.take() { __previous.abort(); }
{ let __ice_next = ({ crate::backend::current_wall_seconds() }); if ::ui_lang_runtime::state_changed!(self.wall_now, __ice_next) { self.wall_now = __ice_next; self.__ice_rev[3] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.connected, __ice_next) { self.connected = __ice_next; self.__ice_rev[8] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.loading, __ice_next) { self.loading = __ice_next; self.__ice_rev[9] += 1; } }
{ let __ice_next = "Connecting…".to_owned(); if ::ui_lang_runtime::state_changed!(self.status, __ice_next) { self.status = __ice_next; self.__ice_rev[7] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = self.rpc.to_owned(); if ::ui_lang_runtime::state_changed!(self.connected_rpc, __ice_next) { self.connected_rpc = __ice_next; self.__ice_rev[5] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.network_chain_id, __ice_next) { self.network_chain_id = __ice_next; self.__ice_rev[86] += 1; } }
{ let __ice_next = crate::backend::network_label(self.network_chain_id.to_owned(), self.connected_rpc.to_owned()); if ::ui_lang_runtime::state_changed!(self.network_name, __ice_next) { self.network_name = __ice_next; self.__ice_rev[124] += 1; } }
{ let __ice_next = (self.hydration_generation + 1); if ::ui_lang_runtime::state_changed!(self.hydration_generation, __ice_next) { self.hydration_generation = __ice_next; self.__ice_rev[15] += 1; } }
{ let __ice_next = (self.connect_generation + 1); if ::ui_lang_runtime::state_changed!(self.connect_generation, __ice_next) { self.connect_generation = __ice_next; self.__ice_rev[16] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.hydration_retry_attempt, __ice_next) { self.hydration_retry_attempt = __ice_next; self.__ice_rev[18] += 1; } }
{ let __ice_next = MutationPhase::Idle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.channels, __ice_next) { self.channels = __ice_next; self.__ice_rev[22] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.rooms, __ice_next) { self.rooms = __ice_next; self.__ice_rev[23] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.dm_rows, __ice_next) { self.dm_rows = __ice_next; self.__ice_rev[51] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.chat_at_tail, __ice_next) { self.chat_at_tail = __ice_next; self.__ice_rev[47] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.chat_land_seq, __ice_next) { self.chat_land_seq = __ice_next; self.__ice_rev[33] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.chat_pending_sends, __ice_next) { self.chat_pending_sends = __ice_next; self.__ice_rev[41] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.chat_edit_seq, __ice_next) { self.chat_edit_seq = __ice_next; self.__ice_rev[34] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.chat_edit_rev, __ice_next) { self.chat_edit_rev = __ice_next; self.__ice_rev[35] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.channel_reads, __ice_next) { self.channel_reads = __ice_next; self.__ice_rev[25] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.unread_boundary, __ice_next) { self.unread_boundary = __ice_next; self.__ice_rev[26] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.active_channel, __ice_next) { self.active_channel = __ice_next; self.__ice_rev[27] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.active_dm_peer, __ice_next) { self.active_dm_peer = __ice_next; self.__ice_rev[53] += 1; } }
{ let __ice_next = crate::backend::no_dm_peer(); if ::ui_lang_runtime::state_changed!(self.active_dm, __ice_next) { self.active_dm = __ice_next; self.__ice_rev[54] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.history_view, __ice_next) { self.history_view = __ice_next; self.__ice_rev[48] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.active_channel_name, __ice_next) { self.active_channel_name = __ice_next; self.__ice_rev[28] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.active_channel_archived, __ice_next) { self.active_channel_archived = __ice_next; self.__ice_rev[29] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.active_channel_members_only, __ice_next) { self.active_channel_members_only = __ice_next; self.__ice_rev[30] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.channel_members, __ice_next) { self.channel_members = __ice_next; self.__ice_rev[31] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.post_refusal, __ice_next) { self.post_refusal = __ice_next; self.__ice_rev[32] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.channel_draft, __ice_next) { self.channel_draft = __ice_next; self.__ice_rev[43] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.pending_channel, __ice_next) { self.pending_channel = __ice_next; self.__ice_rev[46] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.page_route, __ice_next) { self.page_route = __ice_next; self.__ice_rev[119] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.palette_draft, __ice_next) { self.palette_draft = __ice_next; self.__ice_rev[113] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.palette_chat_hits, __ice_next) { self.palette_chat_hits = __ice_next; self.__ice_rev[115] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.palette_page_hits, __ice_next) { self.palette_page_hits = __ice_next; self.__ice_rev[116] += 1; } }
{ let __ice_next = SearchPhase::Idle; if ::ui_lang_runtime::state_changed!(self.palette_search_phase, __ice_next) { self.palette_search_phase = __ice_next; self.__ice_rev[114] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.forge_note_pending, __ice_next) { self.forge_note_pending = __ice_next; self.__ice_rev[63] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.forge_link, __ice_next) { self.forge_link = __ice_next; self.__ice_rev[64] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.huddle_joined, __ice_next) { self.huddle_joined = __ice_next; self.__ice_rev[142] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.huddle_channel, __ice_next) { self.huddle_channel = __ice_next; self.__ice_rev[143] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.huddle_channel_name, __ice_next) { self.huddle_channel_name = __ice_next; self.__ice_rev[144] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.huddle_joined_at, __ice_next) { self.huddle_joined_at = __ice_next; self.__ice_rev[145] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.huddle_roster, __ice_next) { self.huddle_roster = __ice_next; self.__ice_rev[154] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.huddle_rows, __ice_next) { self.huddle_rows = __ice_next; self.__ice_rev[155] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.call_status, __ice_next) { self.call_status = __ice_next; self.__ice_rev[147] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.call_muted, __ice_next) { self.call_muted = __ice_next; self.__ice_rev[148] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.call_camera, __ice_next) { self.call_camera = __ice_next; self.__ice_rev[150] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.call_sharing, __ice_next) { self.call_sharing = __ice_next; self.__ice_rev[151] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.call_video_live, __ice_next) { self.call_video_live = __ice_next; self.__ice_rev[152] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.huddle_stage, __ice_next) { self.huddle_stage = __ice_next; self.__ice_rev[153] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.call_peers, __ice_next) { self.call_peers = __ice_next; self.__ice_rev[149] += 1; } }
if (self.connected_rpc).is_empty() { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = ConsoleEntry::Entering; if ::ui_lang_runtime::state_changed!(self.console_entry, __ice_next) { self.console_entry = __ice_next; self.__ice_rev[132] += 1; self.__ice_derived.hub_busy.take(); } }
return ::ducktape_view_guest::Task::batch([
{ // __ICE_SOURCE 486 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6f6e626f617264696e672e696365
(::ducktape_view_guest::Task::perform(({ crate::backend::remember_network(self.connected_rpc.to_owned()) }), |value| value)).discard::<__DucktapeMessage>()
},
{ // __ICE_SOURCE 489 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6f6e626f617264696e672e696365
{ let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::connect(self.connected_rpc.to_owned(), 0, self.connect_generation) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::WorkspaceConnected(value), ::std::result::Result::Err(error) => __DucktapeMessage::ConnectFailed(error) }); self.__ice_run_lane_1_generation = self.__ice_run_lane_1_generation.wrapping_add(1); let __generation = self.__ice_run_lane_1_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_1_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane1(__generation, ::std::boxed::Box::new(__message))) }
},
]);
})(),
__DucktapeMessage::ConsoleOpened(id) => (|| {
let _ = &id;
{ let __ice_next = ::std::option::Option::Some(id); if ::ui_lang_runtime::state_changed!(self.console_win, __ice_next) { self.console_win = __ice_next; self.__ice_rev[122] += 1; } }
return ::crate::shell::close::<__DucktapeMessage>(({ crate::backend::window_target(self.onboarding_win.clone()) }));
})(),
__DucktapeMessage::ForgetNetworkSubmit(id) => (|| {
let _ = &id;
if (self.mutation_phase != MutationPhase::Idle) { return ::ducktape_view_guest::Task::none(); }
return ::ducktape_view_guest::Task::perform(({ crate::backend::forget_network(id.to_owned()) }), |value| __DucktapeMessage::NetworkForgotten(value));
})(),
__DucktapeMessage::NetworkForgotten(_written) => (|| {
let _ = &_written;
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::hub_state() }), |value| __DucktapeMessage::HubRefreshed(value)); self.__ice_run_lane_19_generation = self.__ice_run_lane_19_generation.wrapping_add(1); let __generation = self.__ice_run_lane_19_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_19_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane19(__generation, ::std::boxed::Box::new(__message))) };
})(),
__DucktapeMessage::GoJoin => (|| {
if (self.mutation_phase != MutationPhase::Idle) { return ::ducktape_view_guest::Task::none(); }
self.__ice_secrets.clear("join_invite");
{ let __ice_next = HubStep::Join; if ::ui_lang_runtime::state_changed!(self.hub_step, __ice_next) { self.hub_step = __ice_next; self.__ice_rev[125] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::GoNetworks => (|| {
let unrelated_mutation = ((self.mutation_phase != MutationPhase::Idle) && (self.hub_step != HubStep::Account));
if unrelated_mutation { return ::ducktape_view_guest::Task::none(); }
self.__ice_run_lane_24_generation = self.__ice_run_lane_24_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_24_handle.take() { __previous.abort(); }
self.__ice_run_lane_25_generation = self.__ice_run_lane_25_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_25_handle.take() { __previous.abort(); }
self.__ice_run_lane_23_generation = self.__ice_run_lane_23_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_23_handle.take() { __previous.abort(); }
{ let __ice_next = MutationPhase::Idle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_phase, __ice_next) { self.ceremony_phase = __ice_next; self.__ice_rev[138] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_qr, __ice_next) { self.ceremony_qr = __ice_next; self.__ice_rev[139] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_detail, __ice_next) { self.ceremony_detail = __ice_next; self.__ice_rev[140] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_left, __ice_next) { self.ceremony_left = __ice_next; self.__ice_rev[141] += 1; } }
self.__ice_secrets.clear("restore_words");
self.__ice_secrets.clear("join_invite");
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.password, __ice_next) { self.password = __ice_next; self.__ice_rev[6] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.hub_wallets, __ice_next) { self.hub_wallets = __ice_next; self.__ice_rev[128] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.hub_wallet_selected, __ice_next) { self.hub_wallet_selected = __ice_next; self.__ice_rev[129] += 1; } }
{ let __ice_next = HubStep::Networks; if ::ui_lang_runtime::state_changed!(self.hub_step, __ice_next) { self.hub_step = __ice_next; self.__ice_rev[125] += 1; } }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::hub_state() }), |value| __DucktapeMessage::HubRefreshed(value)); self.__ice_run_lane_19_generation = self.__ice_run_lane_19_generation.wrapping_add(1); let __generation = self.__ice_run_lane_19_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_19_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane19(__generation, ::std::boxed::Box::new(__message))) };
})(),
__DucktapeMessage::JoinNetworkSubmit => (|| {
if ((self.mutation_phase != MutationPhase::Idle) || (self.__ice_secrets.text("join_invite")).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = MutationPhase::Onboarding; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
return ::ducktape_view_guest::Task::perform(({ crate::backend::join_network(self.__ice_secrets.read("join_invite")) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::WorkspaceMaterialized(value), ::std::result::Result::Err(error) => __DucktapeMessage::OnboardingFailed(error) });
})(),
__DucktapeMessage::WorkspaceMaterialized(init) => (|| {
let _ = &init;
self.__ice_secrets.clear("join_invite");
{ let __ice_next = MutationPhase::Idle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = init.chain_id.to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_name, __ice_next) { self.onboarding_name = __ice_next; self.__ice_rev[130] += 1; } }
{ let __ice_next = init.rpc.to_owned(); if ::ui_lang_runtime::state_changed!(self.rpc, __ice_next) { self.rpc = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.invite_link, __ice_next) { self.invite_link = __ice_next; self.__ice_rev[133] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.provision_steps, __ice_next) { self.provision_steps = __ice_next; self.__ice_rev[134] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.provision_index, __ice_next) { self.provision_index = __ice_next; self.__ice_rev[135] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = HubStep::Provisioning; if ::ui_lang_runtime::state_changed!(self.hub_step, __ice_next) { self.hub_step = __ice_next; self.__ice_rev[125] += 1; } }
return { let __task = ::ducktape_view_guest::Task::run(({ crate::backend::provision_progress(init.chain_id.to_owned(), init.rpc.to_owned()) }), |value| __DucktapeMessage::ProvisionStepped(value)); self.__ice_run_lane_26_generation = self.__ice_run_lane_26_generation.wrapping_add(1); let __generation = self.__ice_run_lane_26_generation; let __task = __task.map(move |__message| __DucktapeMessage::__RequestLane26(__generation, ::std::option::Option::Some(::std::boxed::Box::new(__message)))).chain(::ducktape_view_guest::Task::done(__DucktapeMessage::__RequestLane26(__generation, ::std::option::Option::None))); let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_26_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task };
})(),
__DucktapeMessage::ProvisionStepped(step) => (|| {
let _ = &step;
let settled = step.settled;
{ let __ice_next = step.index; if ::ui_lang_runtime::state_changed!(self.provision_index, __ice_next) { self.provision_index = __ice_next; self.__ice_rev[135] += 1; } }
{ let __ice_next = ::std::vec![step.clone()]; if ::ui_lang_runtime::state_changed!(self.provision_steps, __ice_next) { self.provision_steps = __ice_next; self.__ice_rev[134] += 1; } }
if ((self.provision_index != 5) || (!settled)) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = HubStep::Live; if ::ui_lang_runtime::state_changed!(self.hub_step, __ice_next) { self.hub_step = __ice_next; self.__ice_rev[125] += 1; } }
return ::ducktape_view_guest::Task::perform(({ crate::backend::mint_invite(self.onboarding_name.to_owned()) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::OnboardingInviteMinted(value), ::std::result::Result::Err(error) => __DucktapeMessage::OnboardingFailed(error) });
})(),
__DucktapeMessage::OnboardingInviteMinted(blob) => (|| {
let _ = &blob;
{ let __ice_next = blob.to_owned(); if ::ui_lang_runtime::state_changed!(self.invite_link, __ice_next) { self.invite_link = __ice_next; self.__ice_rev[133] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::CopyOnboardingInvite => (|| {
if (self.invite_link).is_empty() { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = "Invite copied".to_owned(); if ::ui_lang_runtime::state_changed!(self.toast, __ice_next) { self.toast = __ice_next; self.__ice_rev[117] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.toast_age, __ice_next) { self.toast_age = __ice_next; self.__ice_rev[118] += 1; } }
return ::crate::shell::clipboard::<__DucktapeMessage>(self.invite_link.to_owned());
})(),
__DucktapeMessage::EnterConsole => (|| {
if (self.mutation_phase != MutationPhase::Idle) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = crate::backend::network_label(self.onboarding_name.to_owned(), self.rpc.to_owned()); if ::ui_lang_runtime::state_changed!(self.network_name, __ice_next) { self.network_name = __ice_next; self.__ice_rev[124] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.password, __ice_next) { self.password = __ice_next; self.__ice_rev[6] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.hub_wallet_selected, __ice_next) { self.hub_wallet_selected = __ice_next; self.__ice_rev[129] += 1; } }
{ let __ice_next = MutationPhase::Onboarding; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
return { let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::load_wallets(self.rpc.to_owned()) }), |value| __DucktapeMessage::WalletsLoaded(value)); self.__ice_run_lane_23_generation = self.__ice_run_lane_23_generation.wrapping_add(1); let __generation = self.__ice_run_lane_23_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_23_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane23(__generation, ::std::boxed::Box::new(__message))) };
})(),
__DucktapeMessage::OnboardingFailed(cause) => (|| {
let _ = &cause;
{ let __ice_next = MutationPhase::Idle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = cause.message.to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::SwitchNetwork => (|| {
if (self.mutation_phase != MutationPhase::Idle) { return ::ducktape_view_guest::Task::none(); }
self.__ice_run_lane_10_generation = self.__ice_run_lane_10_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_10_handle.take() { __previous.abort(); }
self.__ice_run_lane_11_generation = self.__ice_run_lane_11_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_11_handle.take() { __previous.abort(); }
{ let __ice_next = (self.account_busy && (self.account_ceremony_phase).is_empty()); if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_phase, __ice_next) { self.account_ceremony_phase = __ice_next; self.__ice_rev[80] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_qr, __ice_next) { self.account_ceremony_qr = __ice_next; self.__ice_rev[81] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_detail, __ice_next) { self.account_ceremony_detail = __ice_next; self.__ice_rev[82] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_left, __ice_next) { self.account_ceremony_left = __ice_next; self.__ice_rev[83] += 1; } }
return { let (_, __task) = ::crate::shell::open(Self::__window_0()); __task.map(move |value| __DucktapeMessage::OnboardingReopened(value)) };
})(),
__DucktapeMessage::OnboardingReopened(id) => (|| {
let _ = &id;
{ let __ice_next = ::std::option::Option::Some(id); if ::ui_lang_runtime::state_changed!(self.onboarding_win, __ice_next) { self.onboarding_win = __ice_next; self.__ice_rev[121] += 1; } }
{ let __ice_next = HubStep::Networks; if ::ui_lang_runtime::state_changed!(self.hub_step, __ice_next) { self.hub_step = __ice_next; self.__ice_rev[125] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.password, __ice_next) { self.password = __ice_next; self.__ice_rev[6] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.hub_wallets, __ice_next) { self.hub_wallets = __ice_next; self.__ice_rev[128] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.hub_wallet_selected, __ice_next) { self.hub_wallet_selected = __ice_next; self.__ice_rev[129] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.signer_key, __ice_next) { self.signer_key = __ice_next; self.__ice_rev[17] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.live_agents, __ice_next) { self.live_agents = __ice_next; self.__ice_rev[39] += 1; } }
return ::ducktape_view_guest::Task::batch([
{ // __ICE_SOURCE 634 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6f6e626f617264696e672e696365
::crate::shell::close::<__DucktapeMessage>(({ crate::backend::window_target(self.console_win.clone()) }))
},
{ // __ICE_SOURCE 635 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6f6e626f617264696e672e696365
::crate::shell::close::<__DucktapeMessage>(({ crate::backend::window_target(self.huddle_win.clone()) }))
},
{ // __ICE_SOURCE 636 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6f6e626f617264696e672e696365
(::ducktape_view_guest::Task::perform(({ crate::backend::lock_signer() }), |value| value)).discard::<__DucktapeMessage>()
},
{ // __ICE_SOURCE 639 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6f6e626f617264696e672e696365
{ let __task = ::ducktape_view_guest::Task::perform(({ crate::backend::hub_state() }), |value| __DucktapeMessage::HubRefreshed(value)); self.__ice_run_lane_19_generation = self.__ice_run_lane_19_generation.wrapping_add(1); let __generation = self.__ice_run_lane_19_generation; let (__task, __handle) = __task.abortable(); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_19_handle.replace(__handle.abort_on_drop()) { __previous.abort(); } __task.map(move |__message| __DucktapeMessage::__RequestLane19(__generation, ::std::boxed::Box::new(__message))) }
},
]);
})(),
__DucktapeMessage::DismissAccountBanner => (|| {
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.account_banner_dismissed, __ice_next) { self.account_banner_dismissed = __ice_next; self.__ice_rev[79] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::OpenAccountWelcome => (|| {
if (self.mutation_phase != MutationPhase::Idle) { return ::ducktape_view_guest::Task::none(); }
self.__ice_run_lane_10_generation = self.__ice_run_lane_10_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_10_handle.take() { __previous.abort(); }
self.__ice_run_lane_11_generation = self.__ice_run_lane_11_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_11_handle.take() { __previous.abort(); }
{ let __ice_next = (self.account_busy && (self.account_ceremony_phase).is_empty()); if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_phase, __ice_next) { self.account_ceremony_phase = __ice_next; self.__ice_rev[80] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_qr, __ice_next) { self.account_ceremony_qr = __ice_next; self.__ice_rev[81] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_detail, __ice_next) { self.account_ceremony_detail = __ice_next; self.__ice_rev[82] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_left, __ice_next) { self.account_ceremony_left = __ice_next; self.__ice_rev[83] += 1; } }
{ let __ice_next = self.connected_rpc.to_owned(); if ::ui_lang_runtime::state_changed!(self.rpc, __ice_next) { self.rpc = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = self.network_chain_id.to_owned(); if ::ui_lang_runtime::state_changed!(self.hub_chain_id, __ice_next) { self.hub_chain_id = __ice_next; self.__ice_rev[136] += 1; } }
return { let (_, __task) = ::crate::shell::open(Self::__window_0()); __task.map(move |value| __DucktapeMessage::WelcomeReopened(value)) };
})(),
__DucktapeMessage::WelcomeReopened(id) => (|| {
let _ = &id;
{ let __ice_next = ::std::option::Option::Some(id); if ::ui_lang_runtime::state_changed!(self.onboarding_win, __ice_next) { self.onboarding_win = __ice_next; self.__ice_rev[121] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_phase, __ice_next) { self.ceremony_phase = __ice_next; self.__ice_rev[138] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_qr, __ice_next) { self.ceremony_qr = __ice_next; self.__ice_rev[139] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.ceremony_detail, __ice_next) { self.ceremony_detail = __ice_next; self.__ice_rev[140] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.onboarding_error, __ice_next) { self.onboarding_error = __ice_next; self.__ice_rev[131] += 1; self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = HubStep::Account; if ::ui_lang_runtime::state_changed!(self.hub_step, __ice_next) { self.hub_step = __ice_next; self.__ice_rev[125] += 1; } }
return ::ducktape_view_guest::Task::batch([
{ // __ICE_SOURCE 668 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6f6e626f617264696e672e696365
::crate::shell::close::<__DucktapeMessage>(({ crate::backend::window_target(self.console_win.clone()) }))
},
{ // __ICE_SOURCE 669 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f6f6e626f617264696e672e696365
::crate::shell::close::<__DucktapeMessage>(({ crate::backend::window_target(self.huddle_win.clone()) }))
},
]);
})(),
__DucktapeMessage::CallEvent(event) => (|| {
let _ = &event;
{ let __ice_next = crate::call::call_status_after(self.call_status.to_owned(), event.clone()); if ::ui_lang_runtime::state_changed!(self.call_status, __ice_next) { self.call_status = __ice_next; self.__ice_rev[147] += 1; } }
{ let __ice_next = crate::backend::keep_bool((event.kind == "connecting"), false, self.call_muted); if ::ui_lang_runtime::state_changed!(self.call_muted, __ice_next) { self.call_muted = __ice_next; self.__ice_rev[148] += 1; } }
{ let __ice_next = crate::backend::keep_bool((event.kind == "connecting"), false, self.call_camera); if ::ui_lang_runtime::state_changed!(self.call_camera, __ice_next) { self.call_camera = __ice_next; self.__ice_rev[150] += 1; } }
{ let __ice_next = crate::backend::keep_bool((event.kind == "connecting"), false, self.call_sharing); if ::ui_lang_runtime::state_changed!(self.call_sharing, __ice_next) { self.call_sharing = __ice_next; self.__ice_rev[151] += 1; } }
self.call_peers = crate::call::apply_call_peer(::std::mem::take(&mut self.call_peers), event.clone()); self.__ice_rev[149] += 1;
{ let __ice_next = crate::call::huddle_tile_rows(self.huddle_roster.clone(), self.call_peers.clone(), self.call_muted); if ::ui_lang_runtime::state_changed!(self.huddle_rows, __ice_next) { self.huddle_rows = __ice_next; self.__ice_rev[155] += 1; } }
{ let __ice_next = crate::call::call_video_live_after(self.call_peers.clone(), self.call_camera, self.call_sharing); if ::ui_lang_runtime::state_changed!(self.call_video_live, __ice_next) { self.call_video_live = __ice_next; self.__ice_rev[152] += 1; } }
{ let __ice_next = crate::call::huddle_stage_peer(self.call_peers.clone(), self.call_sharing); if ::ui_lang_runtime::state_changed!(self.huddle_stage, __ice_next) { self.huddle_stage = __ice_next; self.__ice_rev[153] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::ToggleCallMute => (|| {
if (!self.huddle_joined) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = ({ crate::call::call_set_muted((!self.call_muted)) }); if ::ui_lang_runtime::state_changed!(self.call_muted, __ice_next) { self.call_muted = __ice_next; self.__ice_rev[148] += 1; } }
{ let __ice_next = crate::call::huddle_tile_rows(self.huddle_roster.clone(), self.call_peers.clone(), self.call_muted); if ::ui_lang_runtime::state_changed!(self.huddle_rows, __ice_next) { self.huddle_rows = __ice_next; self.__ice_rev[155] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::ToggleCallCamera => (|| {
let source = ({ crate::video::call_use_camera((!self.call_camera)) });
{ let __ice_next = source.camera; if ::ui_lang_runtime::state_changed!(self.call_camera, __ice_next) { self.call_camera = __ice_next; self.__ice_rev[150] += 1; } }
{ let __ice_next = source.sharing; if ::ui_lang_runtime::state_changed!(self.call_sharing, __ice_next) { self.call_sharing = __ice_next; self.__ice_rev[151] += 1; } }
{ let __ice_next = crate::call::call_video_live_after(self.call_peers.clone(), self.call_camera, self.call_sharing); if ::ui_lang_runtime::state_changed!(self.call_video_live, __ice_next) { self.call_video_live = __ice_next; self.__ice_rev[152] += 1; } }
{ let __ice_next = crate::call::huddle_stage_peer(self.call_peers.clone(), self.call_sharing); if ::ui_lang_runtime::state_changed!(self.huddle_stage, __ice_next) { self.huddle_stage = __ice_next; self.__ice_rev[153] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::ToggleCallScreen => (|| {
let source = ({ crate::video::call_use_screen((!self.call_sharing)) });
{ let __ice_next = source.camera; if ::ui_lang_runtime::state_changed!(self.call_camera, __ice_next) { self.call_camera = __ice_next; self.__ice_rev[150] += 1; } }
{ let __ice_next = source.sharing; if ::ui_lang_runtime::state_changed!(self.call_sharing, __ice_next) { self.call_sharing = __ice_next; self.__ice_rev[151] += 1; } }
{ let __ice_next = crate::call::call_video_live_after(self.call_peers.clone(), self.call_camera, self.call_sharing); if ::ui_lang_runtime::state_changed!(self.call_video_live, __ice_next) { self.call_video_live = __ice_next; self.__ice_rev[152] += 1; } }
{ let __ice_next = crate::call::huddle_stage_peer(self.call_peers.clone(), self.call_sharing); if ::ui_lang_runtime::state_changed!(self.huddle_stage, __ice_next) { self.huddle_stage = __ice_next; self.__ice_rev[153] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::ShowHuddle => (|| {
let summon = crate::backend::huddle_summon(self.huddle_win.clone());
return match summon.clone() {
WindowSummon::Open => (|| {
return { let (_, __task) = ::crate::shell::open(Self::__window_2()); __task.map(move |value| __DucktapeMessage::HuddleOpened(value)) };
})(),
WindowSummon::Raise => (|| {
return ::crate::shell::raise::<__DucktapeMessage>(({ crate::backend::window_target(self.huddle_win.clone()) }));
})(),
};
})(),
__DucktapeMessage::HuddleOpened(id) => (|| {
let _ = &id;
{ let __ice_next = ::std::option::Option::Some(id); if ::ui_lang_runtime::state_changed!(self.huddle_win, __ice_next) { self.huddle_win = __ice_next; self.__ice_rev[123] += 1; } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::HuddleGoChannel => (|| {
if ((self.loading || (self.mutation_phase != MutationPhase::Idle)) || (self.huddle_channel).is_empty()) { return ::ducktape_view_guest::Task::none(); }
self.__ice_run_lane_10_generation = self.__ice_run_lane_10_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_10_handle.take() { __previous.abort(); }
self.__ice_run_lane_11_generation = self.__ice_run_lane_11_generation.wrapping_add(1); if let ::std::option::Option::Some(__previous) = self.__ice_run_lane_11_handle.take() { __previous.abort(); }
{ let __ice_next = (self.account_busy && (self.account_ceremony_phase).is_empty()); if ::ui_lang_runtime::state_changed!(self.account_busy, __ice_next) { self.account_busy = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_phase, __ice_next) { self.account_ceremony_phase = __ice_next; self.__ice_rev[80] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_qr, __ice_next) { self.account_ceremony_qr = __ice_next; self.__ice_rev[81] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_detail, __ice_next) { self.account_ceremony_detail = __ice_next; self.__ice_rev[82] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.account_ceremony_left, __ice_next) { self.account_ceremony_left = __ice_next; self.__ice_rev[83] += 1; } }
{ let __ice_next = ShellTab::Chat; if ::ui_lang_runtime::state_changed!(self.shell_tab, __ice_next) { self.shell_tab = __ice_next; self.__ice_rev[55] += 1; } }
return (::ducktape_view_guest::Task::done(self.huddle_channel.to_owned())).map(|value| __DucktapeMessage::ChooseChannel(value));
})(),
__DucktapeMessage::LeaveHuddleHere => (|| {
if (((self.loading || (self.mutation_phase != MutationPhase::Idle)) || (!self.huddle_joined)) || (self.huddle_channel).is_empty()) { return ::ducktape_view_guest::Task::none(); }
{ let __ice_next = (self.hydration_generation + 1); if ::ui_lang_runtime::state_changed!(self.hydration_generation, __ice_next) { self.hydration_generation = __ice_next; self.__ice_rev[15] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.hydration_retry_attempt, __ice_next) { self.hydration_retry_attempt = __ice_next; self.__ice_rev[18] += 1; } }
{ let __ice_next = MutationPhase::Huddle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.call_status, __ice_next) { self.call_status = __ice_next; self.__ice_rev[147] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.call_muted, __ice_next) { self.call_muted = __ice_next; self.__ice_rev[148] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.call_camera, __ice_next) { self.call_camera = __ice_next; self.__ice_rev[150] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.call_sharing, __ice_next) { self.call_sharing = __ice_next; self.__ice_rev[151] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.call_video_live, __ice_next) { self.call_video_live = __ice_next; self.__ice_rev[152] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.huddle_stage, __ice_next) { self.huddle_stage = __ice_next; self.__ice_rev[153] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.call_peers, __ice_next) { self.call_peers = __ice_next; self.__ice_rev[149] += 1; } }
{ let __ice_next = crate::call::huddle_tile_rows(self.huddle_roster.clone(), self.call_peers.clone(), self.call_muted); if ::ui_lang_runtime::state_changed!(self.huddle_rows, __ice_next) { self.huddle_rows = __ice_next; self.__ice_rev[155] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
return ::ducktape_view_guest::Task::batch([
{ // __ICE_SOURCE 147 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f687564646c652e696365
::crate::shell::close::<__DucktapeMessage>(({ crate::backend::window_target(self.huddle_win.clone()) }))
},
{ // __ICE_SOURCE 148 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6170702f7372632f75692f68616e646c6572732f687564646c652e696365
::ducktape_view_guest::Task::perform(({ crate::backend::leave_huddle(self.connected_rpc.to_owned(), self.password.to_owned(), self.huddle_channel.to_owned()) }), |result| match result { ::std::result::Result::Ok(value) => __DucktapeMessage::HuddleLeft(value), ::std::result::Result::Err(error) => __DucktapeMessage::MutationFailed(error) })
},
]);
})(),
__DucktapeMessage::HuddleLeft(_result) => (|| {
let _ = &_result;
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.huddle_joined, __ice_next) { self.huddle_joined = __ice_next; self.__ice_rev[142] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.huddle_roster, __ice_next) { self.huddle_roster = __ice_next; self.__ice_rev[154] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.huddle_rows, __ice_next) { self.huddle_rows = __ice_next; self.__ice_rev[155] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.huddle_channel, __ice_next) { self.huddle_channel = __ice_next; self.__ice_rev[143] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.huddle_channel_name, __ice_next) { self.huddle_channel_name = __ice_next; self.__ice_rev[144] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.huddle_joined_at, __ice_next) { self.huddle_joined_at = __ice_next; self.__ice_rev[145] += 1; } }
{ let __ice_next = MutationPhase::Idle; if ::ui_lang_runtime::state_changed!(self.mutation_phase, __ice_next) { self.mutation_phase = __ice_next; self.__ice_rev[19] += 1; self.__ice_derived.mutation_busy.take(); self.__ice_derived.hub_busy.take(); } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.error, __ice_next) { self.error = __ice_next; self.__ice_rev[20] += 1; self.__ice_derived.has_error.take(); } }
::ducktape_view_guest::Task::none()
})(),
__DucktapeMessage::__0C57616c6c6574526f77B7077(__scope, value) => { let __local = self.__ice_component_057616c6c6574526f77.entry(__scope).or_default(); { let __ice_next = value; if ::ui_lang_runtime::state_changed!(__local.pw, __ice_next) { __local.pw = __ice_next; __local.__ice_rev[0] += 1; } } ::ducktape_view_guest::Task::none() },
__DucktapeMessage::__0C50617373776f726453637265656eB7077(__scope, value) => { let __local = self.__ice_component_050617373776f726453637265656e.entry(__scope).or_default(); { let __ice_next = value; if ::ui_lang_runtime::state_changed!(__local.pw, __ice_next) { __local.pw = __ice_next; __local.__ice_rev[0] += 1; } } ::ducktape_view_guest::Task::none() },
__DucktapeMessage::__0C50617373776f726453637265656eB707732(__scope, value) => { let __local = self.__ice_component_050617373776f726453637265656e.entry(__scope).or_default(); { let __ice_next = value; if ::ui_lang_runtime::state_changed!(__local.pw2, __ice_next) { __local.pw2 = __ice_next; __local.__ice_rev[1] += 1; } } ::ducktape_view_guest::Task::none() },
__DucktapeMessage::__0C436f6e6669726d50687261736553637265656eB616e73776572(__scope, value) => { let __local = self.__ice_component_0436f6e6669726d50687261736553637265656e.entry(__scope).or_default(); { let __ice_next = value; if ::ui_lang_runtime::state_changed!(__local.answer, __ice_next) { __local.answer = __ice_next; __local.__ice_rev[0] += 1; } } ::ducktape_view_guest::Task::none() },
__DucktapeMessage::__0C526573746f726553637265656eB6e616d655f6472616674(__scope, value) => { let __local = self.__ice_component_0526573746f726553637265656e.entry(__scope).or_default(); { let __ice_next = value; if ::ui_lang_runtime::state_changed!(__local.name_draft, __ice_next) { __local.name_draft = __ice_next; __local.__ice_rev[0] += 1; } } ::ducktape_view_guest::Task::none() },
__DucktapeMessage::__0C526573746f726553637265656eB7077(__scope, value) => { let __local = self.__ice_component_0526573746f726553637265656e.entry(__scope).or_default(); { let __ice_next = value; if ::ui_lang_runtime::state_changed!(__local.pw, __ice_next) { __local.pw = __ice_next; __local.__ice_rev[1] += 1; } } ::ducktape_view_guest::Task::none() },
__DucktapeMessage::__0C4e6574776f726b7353637265656eB72656d6f7465(__scope, value) => { let __local = self.__ice_component_04e6574776f726b7353637265656e.entry(__scope).or_default(); { let __ice_next = value; if ::ui_lang_runtime::state_changed!(__local.remote, __ice_next) { __local.remote = __ice_next; __local.__ice_rev[0] += 1; } } ::ducktape_view_guest::Task::none() },
__DucktapeMessage::__SecretTyped(slot, text) => { if let Some(slot) = match slot.as_str() { "restore_words" => Some("restore_words"), "join_invite" => Some("join_invite"), _ => None } { self.__ice_secrets.set(slot, text); } ::ducktape_view_guest::Task::none() }
__DucktapeMessage::__BindWelcomeNameDraft(value) => { { let __ice_next = value; if ::ui_lang_runtime::state_changed!(self.welcome_name_draft, __ice_next) { self.welcome_name_draft = __ice_next; self.__ice_rev[137] += 1; } } ::ducktape_view_guest::Task::none() }
__DucktapeMessage::__BindChannelDraft(value) => { { let __ice_next = value; if ::ui_lang_runtime::state_changed!(self.channel_draft, __ice_next) { self.channel_draft = __ice_next; self.__ice_rev[43] += 1; } } ::ducktape_view_guest::Task::none() }
__DucktapeMessage::__BindPaletteDraft(value) => { { let __ice_next = value; if ::ui_lang_runtime::state_changed!(self.palette_draft, __ice_next) { self.palette_draft = __ice_next; self.__ice_rev[113] += 1; } } ::ducktape_view_guest::Task::none() }
__DucktapeMessage::__ExternNoop => ::ducktape_view_guest::Task::none(),
};
self.__tray_sync();
#[cfg(all(target_os = "macos", not(test)))]
{
let __accessibility = if ::ui_lang_runtime::accessibility_active() {
::ducktape_view_guest::Task::batch(self.__ice_accessibility_windows.attached().into_iter().map(|__window| ::ui_lang_runtime::snapshot_in::<__DucktapeMessage>("Ducktape", __window).map(move |__snapshot| __DucktapeMessage::__AccessibilityWindowSnapshot(__window, ::std::boxed::Box::new(__snapshot)))))
} else {
::ducktape_view_guest::Task::none()
};
return ::ducktape_view_guest::Task::batch([__task, __accessibility]);
}
#[cfg(not(all(target_os = "macos", not(test))))]
__task
}

}
}
