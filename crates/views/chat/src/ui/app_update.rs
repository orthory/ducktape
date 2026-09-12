#[allow(warnings, clippy::all)]
mod __ice_group_app_update {
use super::*;
impl super::ChatView {
#[allow(clippy::assign_op_pattern)]
pub(super) fn __update(&mut self, message: __ChatViewMessage) -> ::iced::Task<__ChatViewMessage> {
match message {
__ChatViewMessage::SidebarResized(dx, _dy) => (|| {

let _ = &dx;
let _ = &_dy;
{ let __ice_next = crate::host::sidebar_width_after_delta(self.sidebar_width, dx, self.chat_viewport_width); if ::ui_lang_runtime::state_changed!(self.sidebar_width, __ice_next) { self.sidebar_width = __ice_next; self.__ice_rev[76] += 1; } }
{ let __ice_next = crate::host::details_width_after_delta(self.details_width, 0.0, self.chat_viewport_width, self.sidebar_width); if ::ui_lang_runtime::state_changed!(self.details_width, __ice_next) { self.details_width = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = crate::host::thread_width_after_delta(self.thread_width, 0.0, self.chat_viewport_width, self.sidebar_width); if ::ui_lang_runtime::state_changed!(self.thread_width, __ice_next) { self.thread_width = __ice_next; self.__ice_rev[78] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::DetailsResized(dx, _dy) => (|| {

let _ = &dx;
let _ = &_dy;
{ let __ice_next = crate::host::details_width_after_delta(self.details_width, (-dx), self.chat_viewport_width, self.sidebar_width); if ::ui_lang_runtime::state_changed!(self.details_width, __ice_next) { self.details_width = __ice_next; self.__ice_rev[77] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::ThreadResized(dx, _dy) => (|| {

let _ = &dx;
let _ = &_dy;
{ let __ice_next = crate::host::thread_width_after_delta(self.thread_width, (-dx), self.chat_viewport_width, self.sidebar_width); if ::ui_lang_runtime::state_changed!(self.thread_width, __ice_next) { self.thread_width = __ice_next; self.__ice_rev[78] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::ChatViewportChanged(width, _height) => (|| {

let _ = &width;
let _ = &_height;
{ let __ice_next = width; if ::ui_lang_runtime::state_changed!(self.chat_viewport_width, __ice_next) { self.chat_viewport_width = __ice_next; self.__ice_rev[75] += 1; } }
{ let __ice_next = crate::host::sidebar_width_after_delta(self.sidebar_width, 0.0, width); if ::ui_lang_runtime::state_changed!(self.sidebar_width, __ice_next) { self.sidebar_width = __ice_next; self.__ice_rev[76] += 1; } }
{ let __ice_next = crate::host::details_width_after_delta(self.details_width, 0.0, width, self.sidebar_width); if ::ui_lang_runtime::state_changed!(self.details_width, __ice_next) { self.details_width = __ice_next; self.__ice_rev[77] += 1; } }
{ let __ice_next = crate::host::thread_width_after_delta(self.thread_width, 0.0, width, self.sidebar_width); if ::ui_lang_runtime::state_changed!(self.thread_width, __ice_next) { self.thread_width = __ice_next; self.__ice_rev[78] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::SessionArrived(item) => (|| {

let _ = &item;
{ let __ice_next = item.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[84] += 1; } }
if (!(item.error).is_empty()) { return ::iced::Task::none(); }
let next = item.next.clone();
let sent_now = (next.sent_serial != self.sent_serial);
let chord_now = (next.copy_chord_serial != self.copy_chord_serial);
let moved_room = ((next.active_channel != self.active_channel) || (next.land_seq != self.land_seq));
{ let __ice_next = next.sent_serial; if ::ui_lang_runtime::state_changed!(self.sent_serial, __ice_next) { self.sent_serial = __ice_next; self.__ice_rev[25] += 1; } }
{ let __ice_next = next.copy_chord_serial; if ::ui_lang_runtime::state_changed!(self.copy_chord_serial, __ice_next) { self.copy_chord_serial = __ice_next; self.__ice_rev[24] += 1; } }
{ let __ice_next = crate::host::connection_serial_after(self.connected, next.connected, self.connection_serial); if ::ui_lang_runtime::state_changed!(self.connection_serial, __ice_next) { self.connection_serial = __ice_next; self.__ice_rev[31] += 1; } }
{ let __ice_next = next.connected; if ::ui_lang_runtime::state_changed!(self.connected, __ice_next) { self.connected = __ice_next; self.__ice_rev[6] += 1; } }
{ let __ice_next = next.endpoint.to_owned(); if ::ui_lang_runtime::state_changed!(self.endpoint, __ice_next) { self.endpoint = __ice_next; self.__ice_rev[1] += 1; } }
{ let __ice_next = next.network_name.to_owned(); if ::ui_lang_runtime::state_changed!(self.network_name, __ice_next) { self.network_name = __ice_next; self.__ice_rev[2] += 1; } }
{ let __ice_next = next.network_chain_id.to_owned(); if ::ui_lang_runtime::state_changed!(self.network_chain_id, __ice_next) { self.network_chain_id = __ice_next; self.__ice_rev[3] += 1; } }
{ let __ice_next = next.status.to_owned(); if ::ui_lang_runtime::state_changed!(self.status, __ice_next) { self.status = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = next.block_height; if ::ui_lang_runtime::state_changed!(self.block_height, __ice_next) { self.block_height = __ice_next; self.__ice_rev[5] += 1; } }
{ let __ice_next = next.me.to_owned(); if ::ui_lang_runtime::state_changed!(self.me, __ice_next) { self.me = __ice_next; self.__ice_rev[27] += 1; } }
{ let __ice_next = next.me_key.to_owned(); if ::ui_lang_runtime::state_changed!(self.me_key, __ice_next) { self.me_key = __ice_next; self.__ice_rev[28] += 1; } }
{ let __ice_next = ({ crate::host::seat_reader(::std::convert::AsRef::as_ref(&(next.me)), ::std::convert::AsRef::as_ref(&(next.me_key))) }); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[85] += 1; } }
{ let __ice_next = next.names_serial; if ::ui_lang_runtime::state_changed!(self.names_serial, __ice_next) { self.names_serial = __ice_next; self.__ice_rev[29] += 1; } }
{ let __ice_next = next.rooms.clone(); if ::ui_lang_runtime::state_changed!(self.rooms, __ice_next) { self.rooms = __ice_next; self.__ice_rev[9] += 1; } }
{ let __ice_next = next.dm_rows.clone(); if ::ui_lang_runtime::state_changed!(self.dm_rows, __ice_next) { self.dm_rows = __ice_next; self.__ice_rev[10] += 1; } }
{ let __ice_next = next.channel_create_open; if ::ui_lang_runtime::state_changed!(self.channel_create_open, __ice_next) { self.channel_create_open = __ice_next; self.__ice_rev[11] += 1; } }
{ let __ice_next = next.active_channel.to_owned(); if ::ui_lang_runtime::state_changed!(self.active_channel, __ice_next) { self.active_channel = __ice_next; self.__ice_rev[12] += 1; } }
{ let __ice_next = next.active_dm_peer.to_owned(); if ::ui_lang_runtime::state_changed!(self.active_dm_peer, __ice_next) { self.active_dm_peer = __ice_next; self.__ice_rev[13] += 1; } }
{ let __ice_next = next.active_dm.clone(); if ::ui_lang_runtime::state_changed!(self.active_dm, __ice_next) { self.active_dm = __ice_next; self.__ice_rev[14] += 1; } }
{ let __ice_next = next.land_seq; if ::ui_lang_runtime::state_changed!(self.land_seq, __ice_next) { self.land_seq = __ice_next; self.__ice_rev[30] += 1; } }
{ let __ice_next = next.unread_boundary; if ::ui_lang_runtime::state_changed!(self.unread_boundary, __ice_next) { self.unread_boundary = __ice_next; self.__ice_rev[21] += 1; } }
{ let __ice_next = next.loading; if ::ui_lang_runtime::state_changed!(self.session_loading, __ice_next) { self.session_loading = __ice_next; self.__ice_rev[7] += 1; } }
{ let __ice_next = next.busy; if ::ui_lang_runtime::state_changed!(self.session_busy, __ice_next) { self.session_busy = __ice_next; self.__ice_rev[8] += 1; } }
{ let __ice_next = self.session_busy; if ::ui_lang_runtime::state_changed!(self.busy, __ice_next) { self.busy = __ice_next; self.__ice_rev[45] += 1; } }
{ let __ice_next = next.huddle_joined; if ::ui_lang_runtime::state_changed!(self.huddle_joined, __ice_next) { self.huddle_joined = __ice_next; self.__ice_rev[15] += 1; } }
{ let __ice_next = next.huddle_channel.to_owned(); if ::ui_lang_runtime::state_changed!(self.huddle_channel, __ice_next) { self.huddle_channel = __ice_next; self.__ice_rev[16] += 1; } }
{ let __ice_next = next.huddle_channel_name.to_owned(); if ::ui_lang_runtime::state_changed!(self.huddle_channel_name, __ice_next) { self.huddle_channel_name = __ice_next; self.__ice_rev[17] += 1; } }
{ let __ice_next = next.huddle_joined_at; if ::ui_lang_runtime::state_changed!(self.huddle_joined_at, __ice_next) { self.huddle_joined_at = __ice_next; self.__ice_rev[18] += 1; } }
{ let __ice_next = next.huddle_now; if ::ui_lang_runtime::state_changed!(self.huddle_now, __ice_next) { self.huddle_now = __ice_next; self.__ice_rev[19] += 1; } }
{ let __ice_next = next.call_muted; if ::ui_lang_runtime::state_changed!(self.call_muted, __ice_next) { self.call_muted = __ice_next; self.__ice_rev[20] += 1; } }
{ let __ice_next = next.shift_held; if ::ui_lang_runtime::state_changed!(self.shift_held, __ice_next) { self.shift_held = __ice_next; self.__ice_rev[23] += 1; } }
{ let __ice_next = next.pending_sends.clone(); if ::ui_lang_runtime::state_changed!(self.pending_sends, __ice_next) { self.pending_sends = __ice_next; self.__ice_rev[26] += 1; } }
{ let __ice_next = next.live_agents.clone(); if ::ui_lang_runtime::state_changed!(self.live_agents, __ice_next) { self.live_agents = __ice_next; self.__ice_rev[22] += 1; } }
{ let __ice_next = (self.session_loading || ((!(self.active_channel).is_empty()) && (self.room_channel != self.active_channel))); if ::ui_lang_runtime::state_changed!(self.loading, __ice_next) { self.loading = __ice_next; self.__ice_rev[44] += 1; } }
return ::iced::Task::batch([
{ // __ICE_SOURCE 357 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6372617465732f76696577732f636861742f7372632f75692f6170702e696365
(::iced::Task::done(moved_room)).map(|value| __ChatViewMessage::SessionSettled(value))
},
{ // __ICE_SOURCE 362 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6372617465732f76696577732f636861742f7372632f75692f6170702e696365
(::iced::Task::done(sent_now)).map(|value| __ChatViewMessage::SnapStream(value))
},
{ // __ICE_SOURCE 365 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6372617465732f76696577732f636861742f7372632f75692f6170702e696365
(::iced::Task::done(chord_now)).map(|value| __ChatViewMessage::CopyChord(value))
},
{ // __ICE_SOURCE 368 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6372617465732f76696577732f636861742f7372632f75692f6170702e696365
(::iced::Task::done(next.dark)).map(|value| __ChatViewMessage::ToneChanged(value))
},
]);
})(),
__ChatViewMessage::ToneChanged(dark) => (|| {

let _ = &dark;
return match crate::host::tone_of(dark) {
Tone::Light => (|| {
{ let __ice_next = AppTheme::App; if ::ui_lang_runtime::state_changed!(self.active_palette, __ice_next) { self.active_palette = __ice_next; self.__ice_rev[0] += 1; } }
::iced::Task::none()
})(),
Tone::Dark => (|| {
{ let __ice_next = AppTheme::AppDark; if ::ui_lang_runtime::state_changed!(self.active_palette, __ice_next) { self.active_palette = __ice_next; self.__ice_rev[0] += 1; } }
::iced::Task::none()
})(),
};
})(),
__ChatViewMessage::SessionSettled(moved_room) => (|| {

let _ = &moved_room;
return match crate::host::room_move(moved_room) {
RoomMove::Stayed => (|| {
{ let __ice_next = crate::host::with_pending(::std::convert::AsRef::as_ref(&(self.room_messages)), ::std::convert::AsRef::as_ref(&(self.pending_sends)), 0, ::std::convert::AsRef::as_ref(&(self.me))); if ::ui_lang_runtime::state_changed!(self.messages, __ice_next) { self.messages = __ice_next; self.__ice_rev[37] += 1; } }
{ let __ice_next = crate::host::first_unread_seq(::std::convert::AsRef::as_ref(&(self.messages)), self.unread_boundary); if ::ui_lang_runtime::state_changed!(self.unread_marker_seq, __ice_next) { self.unread_marker_seq = __ice_next; self.__ice_rev[64] += 1; } }
{ let __ice_next = crate::host::timeline_of(::std::convert::AsRef::as_ref(&(self.messages)), ::std::convert::AsRef::as_ref(&(self.live_agents))); if ::ui_lang_runtime::state_changed!(self.timeline, __ice_next) { self.timeline = __ice_next; self.__ice_rev[46] += 1; } }
{ let __ice_next = crate::host::room_key((self.connection_serial + self.room_serial), self.names_serial, ::std::convert::AsRef::as_ref(&(self.active_channel)), self.land_seq, self.history_pages); if ::ui_lang_runtime::state_changed!(self.room_key, __ice_next) { self.room_key = __ice_next; self.__ice_rev[34] += 1; } }
{ let __ice_next = crate::host::thread_key((self.connection_serial + self.room_serial), self.names_serial, ::std::convert::AsRef::as_ref(&(self.active_channel)), self.active_thread_seq, self.thread_target_seq, self.thread_pages); if ::ui_lang_runtime::state_changed!(self.thread_key, __ice_next) { self.thread_key = __ice_next; self.__ice_rev[48] += 1; } }
::iced::Task::none()
})(),
RoomMove::Moved => (|| {
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.history_pages, __ice_next) { self.history_pages = __ice_next; self.__ice_rev[33] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.selected_message_seq, __ice_next) { self.selected_message_seq = __ice_next; self.__ice_rev[65] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.selected_message_rev, __ice_next) { self.selected_message_rev = __ice_next; self.__ice_rev[66] += 1; } }
{ let __ice_next = MessageAction::Toolbar; if ::ui_lang_runtime::state_changed!(self.message_action, __ice_next) { self.message_action = __ice_next; self.__ice_rev[67] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.message_edit_draft, __ice_next) { self.message_edit_draft = __ice_next; self.__ice_rev[80] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.active_thread_seq, __ice_next) { self.active_thread_seq = __ice_next; self.__ice_rev[49] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.thread_target_seq, __ice_next) { self.thread_target_seq = __ice_next; self.__ice_rev[50] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.thread_pages, __ice_next) { self.thread_pages = __ice_next; self.__ice_rev[47] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.thread_messages, __ice_next) { self.thread_messages = __ice_next; self.__ice_rev[53] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.thread_has_more, __ice_next) { self.thread_has_more = __ice_next; self.__ice_rev[54] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.thread_next_reply_seq, __ice_next) { self.thread_next_reply_seq = __ice_next; self.__ice_rev[55] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.thread_loading, __ice_next) { self.thread_loading = __ice_next; self.__ice_rev[56] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.thread_selected_seq, __ice_next) { self.thread_selected_seq = __ice_next; self.__ice_rev[69] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.thread_selected_rev, __ice_next) { self.thread_selected_rev = __ice_next; self.__ice_rev[70] += 1; } }
{ let __ice_next = MessageAction::Toolbar; if ::ui_lang_runtime::state_changed!(self.thread_message_action, __ice_next) { self.thread_message_action = __ice_next; self.__ice_rev[71] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.thread_edit_draft, __ice_next) { self.thread_edit_draft = __ice_next; self.__ice_rev[83] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.copy_anchor_seq, __ice_next) { self.copy_anchor_seq = __ice_next; self.__ice_rev[72] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.copy_head_seq, __ice_next) { self.copy_head_seq = __ice_next; self.__ice_rev[73] += 1; } }
{ let __ice_next = CopySurface::Nowhere; if ::ui_lang_runtime::state_changed!(self.copy_surface, __ice_next) { self.copy_surface = __ice_next; self.__ice_rev[74] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.channel_settings_open, __ice_next) { self.channel_settings_open = __ice_next; self.__ice_rev[68] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.at_live_tail, __ice_next) { self.at_live_tail = __ice_next; self.__ice_rev[62] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.room_messages, __ice_next) { self.room_messages = __ice_next; self.__ice_rev[36] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.messages, __ice_next) { self.messages = __ice_next; self.__ice_rev[37] += 1; } }
{ let __ice_next = crate::host::timeline_of(::std::convert::AsRef::as_ref(&(::std::vec::Vec::new())), ::std::convert::AsRef::as_ref(&(::std::vec::Vec::new()))); if ::ui_lang_runtime::state_changed!(self.timeline, __ice_next) { self.timeline = __ice_next; self.__ice_rev[46] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.unread_marker_seq, __ice_next) { self.unread_marker_seq = __ice_next; self.__ice_rev[64] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.channel_members, __ice_next) { self.channel_members = __ice_next; self.__ice_rev[38] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.post_refusal, __ice_next) { self.post_refusal = __ice_next; self.__ice_rev[42] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.has_older_history, __ice_next) { self.has_older_history = __ice_next; self.__ice_rev[43] += 1; } }
{ let __ice_next = crate::host::room_key((self.connection_serial + self.room_serial), self.names_serial, ::std::convert::AsRef::as_ref(&(self.active_channel)), self.land_seq, 0); if ::ui_lang_runtime::state_changed!(self.room_key, __ice_next) { self.room_key = __ice_next; self.__ice_rev[34] += 1; } }
{ let __ice_next = crate::host::thread_key((self.connection_serial + self.room_serial), self.names_serial, ::std::convert::AsRef::as_ref(&(self.active_channel)), 0, 0, 0); if ::ui_lang_runtime::state_changed!(self.thread_key, __ice_next) { self.thread_key = __ice_next; self.__ice_rev[48] += 1; } }
::iced::Task::none()
})(),
};
})(),
__ChatViewMessage::SnapStream(moved) => (|| {

let _ = &moved;
if (!moved) { return ::iced::Task::none(); }
return ::ui_lang_guest::widget::perform::<__ChatViewMessage>(::ui_lang_guest::wire::WidgetCommand::Snap { target: ::std::string::String::from("ChatView/chat/message-stream"), x: (0.0) as f32, y: (0.0) as f32 });
})(),
__ChatViewMessage::RevealStream(target_key) => (|| {

let _ = &target_key;
if (target_key <= 0) { return ::iced::Task::none(); }
return ::ui_lang_guest::widget::perform::<__ChatViewMessage>(::ui_lang_guest::wire::WidgetCommand::ScrollToKey { target: ::std::string::String::from("ChatView/chat/message-stream"), key: ::ui_lang_guest::wire::ListKey::from(target_key).virtual_key() });
})(),
__ChatViewMessage::RevealThread(target_key) => (|| {

let _ = &target_key;
if (target_key <= 0) { return ::iced::Task::none(); }
return ::ui_lang_guest::widget::perform::<__ChatViewMessage>(::ui_lang_guest::wire::WidgetCommand::ScrollToKey { target: ::std::string::String::from("ChatView/chat/thread-pane/thread-stream"), key: ::ui_lang_guest::wire::ListKey::from(target_key).virtual_key() });
})(),
__ChatViewMessage::RoomArrived(item) => (|| {

let _ = &item;
{ let __ice_next = item.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[84] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.history_loading, __ice_next) { self.history_loading = __ice_next; self.__ice_rev[63] += 1; } }
if (item.channel != self.active_channel) { return ::iced::Task::none(); }
{ let __ice_next = item.channel.to_owned(); if ::ui_lang_runtime::state_changed!(self.room_channel, __ice_next) { self.room_channel = __ice_next; self.__ice_rev[35] += 1; } }
{ let __ice_next = self.session_loading; if ::ui_lang_runtime::state_changed!(self.loading, __ice_next) { self.loading = __ice_next; self.__ice_rev[44] += 1; } }
if (!(item.error).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = item.name.to_owned(); if ::ui_lang_runtime::state_changed!(self.active_channel_name, __ice_next) { self.active_channel_name = __ice_next; self.__ice_rev[39] += 1; } }
{ let __ice_next = item.archived; if ::ui_lang_runtime::state_changed!(self.active_channel_archived, __ice_next) { self.active_channel_archived = __ice_next; self.__ice_rev[40] += 1; } }
{ let __ice_next = item.members_only; if ::ui_lang_runtime::state_changed!(self.active_channel_members_only, __ice_next) { self.active_channel_members_only = __ice_next; self.__ice_rev[41] += 1; } }
{ let __ice_next = item.members.clone(); if ::ui_lang_runtime::state_changed!(self.channel_members, __ice_next) { self.channel_members = __ice_next; self.__ice_rev[38] += 1; } }
{ let __ice_next = crate::host::post_gate(item.archived, item.members_only, ::std::convert::AsRef::as_ref(&(item.members)), ::std::convert::AsRef::as_ref(&(self.me))); if ::ui_lang_runtime::state_changed!(self.post_refusal, __ice_next) { self.post_refusal = __ice_next; self.__ice_rev[42] += 1; } }
{ let __ice_next = item.messages.clone(); if ::ui_lang_runtime::state_changed!(self.room_messages, __ice_next) { self.room_messages = __ice_next; self.__ice_rev[36] += 1; } }
{ let __ice_next = crate::host::with_pending(::std::convert::AsRef::as_ref(&(self.room_messages)), ::std::convert::AsRef::as_ref(&(self.pending_sends)), 0, ::std::convert::AsRef::as_ref(&(self.me))); if ::ui_lang_runtime::state_changed!(self.messages, __ice_next) { self.messages = __ice_next; self.__ice_rev[37] += 1; } }
{ let __ice_next = crate::host::first_unread_seq(::std::convert::AsRef::as_ref(&(self.messages)), self.unread_boundary); if ::ui_lang_runtime::state_changed!(self.unread_marker_seq, __ice_next) { self.unread_marker_seq = __ice_next; self.__ice_rev[64] += 1; } }
{ let __ice_next = crate::host::timeline_of(::std::convert::AsRef::as_ref(&(self.messages)), ::std::convert::AsRef::as_ref(&(self.live_agents))); if ::ui_lang_runtime::state_changed!(self.timeline, __ice_next) { self.timeline = __ice_next; self.__ice_rev[46] += 1; } }
{ let __ice_next = item.has_older; if ::ui_lang_runtime::state_changed!(self.has_older_history, __ice_next) { self.has_older_history = __ice_next; self.__ice_rev[43] += 1; } }
{ let __ice_next = ((self.land_seq > 0) || (self.history_pages > 0)); if ::ui_lang_runtime::state_changed!(self.history_view, __ice_next) { self.history_view = __ice_next; self.__ice_rev[61] += 1; } }
{ let __ice_next = crate::host::message_target_key(::std::convert::AsRef::as_ref(&(self.messages)), self.land_seq, (self.land_seq > 0)); if ::ui_lang_runtime::state_changed!(self.stream_reveal_key, __ice_next) { self.stream_reveal_key = __ice_next; self.__ice_rev[52] += 1; } }
return match crate::host::landing_thread(item.thread_root) {
LandingThread::Absent => (|| {
{ let __ice_next = crate::host::thread_key((self.connection_serial + self.room_serial), self.names_serial, ::std::convert::AsRef::as_ref(&(self.active_channel)), self.active_thread_seq, self.thread_target_seq, self.thread_pages); if ::ui_lang_runtime::state_changed!(self.thread_key, __ice_next) { self.thread_key = __ice_next; self.__ice_rev[48] += 1; } }
return (::iced::Task::done(self.stream_reveal_key)).map(|value| __ChatViewMessage::RevealStream(value));
})(),
LandingThread::Seated => (|| {
{ let __ice_next = item.thread_root; if ::ui_lang_runtime::state_changed!(self.active_thread_seq, __ice_next) { self.active_thread_seq = __ice_next; self.__ice_rev[49] += 1; } }
{ let __ice_next = self.land_seq; if ::ui_lang_runtime::state_changed!(self.thread_target_seq, __ice_next) { self.thread_target_seq = __ice_next; self.__ice_rev[50] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.thread_pages, __ice_next) { self.thread_pages = __ice_next; self.__ice_rev[47] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.thread_loading, __ice_next) { self.thread_loading = __ice_next; self.__ice_rev[56] += 1; } }
{ let __ice_next = crate::host::thread_key((self.connection_serial + self.room_serial), self.names_serial, ::std::convert::AsRef::as_ref(&(self.active_channel)), item.thread_root, self.land_seq, 0); if ::ui_lang_runtime::state_changed!(self.thread_key, __ice_next) { self.thread_key = __ice_next; self.__ice_rev[48] += 1; } }
return (::iced::Task::done(self.stream_reveal_key)).map(|value| __ChatViewMessage::RevealStream(value));
})(),
};
})(),
__ChatViewMessage::ThreadArrived(item) => (|| {

let _ = &item;
{ let __ice_next = item.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[84] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.thread_loading, __ice_next) { self.thread_loading = __ice_next; self.__ice_rev[56] += 1; } }
if (item.root_seq != self.active_thread_seq) { return ::iced::Task::none(); }
if (!(item.error).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = crate::host::with_pending(::std::convert::AsRef::as_ref(&(item.messages)), ::std::convert::AsRef::as_ref(&(self.pending_sends)), self.active_thread_seq, ::std::convert::AsRef::as_ref(&(self.me))); if ::ui_lang_runtime::state_changed!(self.thread_messages, __ice_next) { self.thread_messages = __ice_next; self.__ice_rev[53] += 1; } }
{ let __ice_next = item.target_seq; if ::ui_lang_runtime::state_changed!(self.thread_target_seq, __ice_next) { self.thread_target_seq = __ice_next; self.__ice_rev[50] += 1; } }
{ let __ice_next = item.has_more; if ::ui_lang_runtime::state_changed!(self.thread_has_more, __ice_next) { self.thread_has_more = __ice_next; self.__ice_rev[54] += 1; } }
{ let __ice_next = item.next_reply_seq; if ::ui_lang_runtime::state_changed!(self.thread_next_reply_seq, __ice_next) { self.thread_next_reply_seq = __ice_next; self.__ice_rev[55] += 1; } }
{ let __ice_next = crate::host::message_target_key(::std::convert::AsRef::as_ref(&(self.thread_messages)), item.target_seq, (item.target_seq > 0)); if ::ui_lang_runtime::state_changed!(self.thread_reveal_key, __ice_next) { self.thread_reveal_key = __ice_next; self.__ice_rev[51] += 1; } }
return (::iced::Task::done(self.thread_reveal_key)).map(|value| __ChatViewMessage::RevealThread(value));
})(),
__ChatViewMessage::SearchArrived(item) => (|| {

let _ = &item;
{ let __ice_next = item.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[84] += 1; } }
if ((item.query).is_empty() || (item.query != self.search_query)) { return ::iced::Task::none(); }
{ let __ice_next = item.hits.clone(); if ::ui_lang_runtime::state_changed!(self.search_hits, __ice_next) { self.search_hits = __ice_next; self.__ice_rev[60] += 1; } }
return match crate::host::search_outcome((item.error).is_empty()) {
SearchOutcome::Answered => (|| {
{ let __ice_next = SearchPhase::Done; if ::ui_lang_runtime::state_changed!(self.search_phase, __ice_next) { self.search_phase = __ice_next; self.__ice_rev[58] += 1; } }
::iced::Task::none()
})(),
SearchOutcome::Refused => (|| {
{ let __ice_next = SearchPhase::Idle; if ::ui_lang_runtime::state_changed!(self.search_phase, __ice_next) { self.search_phase = __ice_next; self.__ice_rev[58] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.search_query, __ice_next) { self.search_query = __ice_next; self.__ice_rev[59] += 1; } }
::iced::Task::none()
})(),
};
})(),
__ChatViewMessage::ActDone(item) => (|| {

let _ = &item;
{ let __ice_next = self.session_busy; if ::ui_lang_runtime::state_changed!(self.busy, __ice_next) { self.busy = __ice_next; self.__ice_rev[45] += 1; } }
{ let __ice_next = item.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[84] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.selected_message_seq, __ice_next) { self.selected_message_seq = __ice_next; self.__ice_rev[65] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.selected_message_rev, __ice_next) { self.selected_message_rev = __ice_next; self.__ice_rev[66] += 1; } }
{ let __ice_next = MessageAction::Toolbar; if ::ui_lang_runtime::state_changed!(self.message_action, __ice_next) { self.message_action = __ice_next; self.__ice_rev[67] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.message_edit_draft, __ice_next) { self.message_edit_draft = __ice_next; self.__ice_rev[80] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.thread_selected_seq, __ice_next) { self.thread_selected_seq = __ice_next; self.__ice_rev[69] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.thread_selected_rev, __ice_next) { self.thread_selected_rev = __ice_next; self.__ice_rev[70] += 1; } }
{ let __ice_next = MessageAction::Toolbar; if ::ui_lang_runtime::state_changed!(self.thread_message_action, __ice_next) { self.thread_message_action = __ice_next; self.__ice_rev[71] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.thread_edit_draft, __ice_next) { self.thread_edit_draft = __ice_next; self.__ice_rev[83] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.member_key_draft, __ice_next) { self.member_key_draft = __ice_next; self.__ice_rev[82] += 1; } }
{ let __ice_next = (self.room_serial + 1); if ::ui_lang_runtime::state_changed!(self.room_serial, __ice_next) { self.room_serial = __ice_next; self.__ice_rev[32] += 1; } }
{ let __ice_next = crate::host::room_key((self.connection_serial + self.room_serial), self.names_serial, ::std::convert::AsRef::as_ref(&(self.active_channel)), self.land_seq, self.history_pages); if ::ui_lang_runtime::state_changed!(self.room_key, __ice_next) { self.room_key = __ice_next; self.__ice_rev[34] += 1; } }
{ let __ice_next = crate::host::thread_key((self.connection_serial + self.room_serial), self.names_serial, ::std::convert::AsRef::as_ref(&(self.active_channel)), self.active_thread_seq, self.thread_target_seq, self.thread_pages); if ::ui_lang_runtime::state_changed!(self.thread_key, __ice_next) { self.thread_key = __ice_next; self.__ice_rev[48] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::SearchChatSubmit => (|| {

if ((self.search_draft).trim().to_owned()).is_empty() { return ::iced::Task::none(); }
{ let __ice_next = SearchPhase::Searching; if ::ui_lang_runtime::state_changed!(self.search_phase, __ice_next) { self.search_phase = __ice_next; self.__ice_rev[58] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.search_hits, __ice_next) { self.search_hits = __ice_next; self.__ice_rev[60] += 1; } }
{ let __ice_next = (self.search_draft).trim().to_owned(); if ::ui_lang_runtime::state_changed!(self.search_query, __ice_next) { self.search_query = __ice_next; self.__ice_rev[59] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[84] += 1; } }
{ let __ice_next = crate::host::search_key(self.connection_serial, self.names_serial, ::std::convert::AsRef::as_ref(&(self.search_query))); if ::ui_lang_runtime::state_changed!(self.search_key, __ice_next) { self.search_key = __ice_next; self.__ice_rev[57] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::ClearChatSearch => (|| {

{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.search_draft, __ice_next) { self.search_draft = __ice_next; self.__ice_rev[79] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.search_query, __ice_next) { self.search_query = __ice_next; self.__ice_rev[59] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.search_hits, __ice_next) { self.search_hits = __ice_next; self.__ice_rev[60] += 1; } }
{ let __ice_next = SearchPhase::Idle; if ::ui_lang_runtime::state_changed!(self.search_phase, __ice_next) { self.search_phase = __ice_next; self.__ice_rev[58] += 1; } }
{ let __ice_next = crate::host::search_key(self.connection_serial, self.names_serial, ::std::convert::AsRef::as_ref(&(""))); if ::ui_lang_runtime::state_changed!(self.search_key, __ice_next) { self.search_key = __ice_next; self.__ice_rev[57] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::OpenChatSearchHit(channel_id, _root_seq, target_seq) => (|| {

let _ = &channel_id;
let _ = &_root_seq;
let _ = &target_seq;
{ let __ice_next = SearchPhase::Idle; if ::ui_lang_runtime::state_changed!(self.search_phase, __ice_next) { self.search_phase = __ice_next; self.__ice_rev[58] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.search_hits, __ice_next) { self.search_hits = __ice_next; self.__ice_rev[60] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.search_query, __ice_next) { self.search_query = __ice_next; self.__ice_rev[59] += 1; } }
{ let __ice_next = crate::host::send_open_hit(::std::convert::AsRef::as_ref(&(channel_id)), target_seq); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[85] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::ToggleChannelCreate => (|| {

{ let __ice_next = crate::host::send_toggle_create(); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[85] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::ChooseChannel(id) => (|| {

let _ = &id;
{ let __ice_next = crate::host::send_choose_channel(::std::convert::AsRef::as_ref(&(id))); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[85] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::ChooseDm(peer_key) => (|| {

let _ = &peer_key;
{ let __ice_next = crate::host::send_choose_dm(::std::convert::AsRef::as_ref(&(peer_key))); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[85] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::ToggleChannelSettings => (|| {

if (self.active_channel).is_empty() { return ::iced::Task::none(); }
{ let __ice_next = self.active_channel_name.to_owned(); if ::ui_lang_runtime::state_changed!(self.channel_name_draft, __ice_next) { self.channel_name_draft = __ice_next; self.__ice_rev[81] += 1; } }
{ let __ice_next = (!self.channel_settings_open); if ::ui_lang_runtime::state_changed!(self.channel_settings_open, __ice_next) { self.channel_settings_open = __ice_next; self.__ice_rev[68] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::ShowHuddle => (|| {

{ let __ice_next = crate::host::send_show_huddle(); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[85] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::LeaveHuddleHere => (|| {

{ let __ice_next = crate::host::send_leave_huddle(); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[85] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::JoinHuddleSubmit => (|| {

{ let __ice_next = crate::host::send_join_huddle(); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[85] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::OpenMessageLink(url) => (|| {

let _ = &url;
{ let __ice_next = crate::host::send_open_link(::std::convert::AsRef::as_ref(&(url))); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[85] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::CopyToClipboard(text, label) => (|| {

let _ = &text;
let _ = &label;
{ let __ice_next = crate::host::send_copy(::std::convert::AsRef::as_ref(&(text)), ::std::convert::AsRef::as_ref(&(label))); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[85] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::CopyMessageLink(link) => (|| {

let _ = &link;
{ let __ice_next = MessageAction::Toolbar; if ::ui_lang_runtime::state_changed!(self.message_action, __ice_next) { self.message_action = __ice_next; self.__ice_rev[67] += 1; } }
{ let __ice_next = MessageAction::Toolbar; if ::ui_lang_runtime::state_changed!(self.thread_message_action, __ice_next) { self.thread_message_action = __ice_next; self.__ice_rev[71] += 1; } }
if (link).is_empty() { return ::iced::Task::none(); }
{ let __ice_next = crate::host::send_copy_link(::std::convert::AsRef::as_ref(&(link))); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[85] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::CancelRun(run_id) => (|| {

let _ = &run_id;
{ let __ice_next = crate::host::send_cancel_run(::std::convert::AsRef::as_ref(&(run_id))); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[85] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::OpenRun(dispatch_id) => (|| {

let _ = &dispatch_id;
{ let __ice_next = crate::host::send_open_run(::std::convert::AsRef::as_ref(&(dispatch_id))); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[85] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::ChatScrolled(absolute_x, absolute_y, relative_x, relative_y) => (|| {

let _ = &absolute_x;
let _ = &absolute_y;
let _ = &relative_x;
let _ = &relative_y;
{ let __ice_next = crate::host::near_scroll_tail(relative_y); if ::ui_lang_runtime::state_changed!(self.at_live_tail, __ice_next) { self.at_live_tail = __ice_next; self.__ice_rev[62] += 1; } }
{ let __ice_next = crate::host::send_scrolled(absolute_x, absolute_y, relative_x, relative_y); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[85] += 1; } }
if ((((((!crate::host::near_scroll_top(relative_y)) || self.history_loading) || self.loading) || self.busy) || (self.active_channel).is_empty()) || (!self.has_older_history)) { return ::iced::Task::none(); }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.history_loading, __ice_next) { self.history_loading = __ice_next; self.__ice_rev[63] += 1; } }
{ let __ice_next = (self.history_pages + 1); if ::ui_lang_runtime::state_changed!(self.history_pages, __ice_next) { self.history_pages = __ice_next; self.__ice_rev[33] += 1; } }
{ let __ice_next = crate::host::room_key((self.connection_serial + self.room_serial), self.names_serial, ::std::convert::AsRef::as_ref(&(self.active_channel)), self.land_seq, self.history_pages); if ::ui_lang_runtime::state_changed!(self.room_key, __ice_next) { self.room_key = __ice_next; self.__ice_rev[34] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::LoadMoreHistory => (|| {

if (((((self.history_loading || self.loading) || self.busy) || (self.active_channel).is_empty()) || (self.messages).is_empty()) || (!self.has_older_history)) { return ::iced::Task::none(); }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.history_loading, __ice_next) { self.history_loading = __ice_next; self.__ice_rev[63] += 1; } }
{ let __ice_next = (self.history_pages + 1); if ::ui_lang_runtime::state_changed!(self.history_pages, __ice_next) { self.history_pages = __ice_next; self.__ice_rev[33] += 1; } }
{ let __ice_next = crate::host::room_key((self.connection_serial + self.room_serial), self.names_serial, ::std::convert::AsRef::as_ref(&(self.active_channel)), self.land_seq, self.history_pages); if ::ui_lang_runtime::state_changed!(self.room_key, __ice_next) { self.room_key = __ice_next; self.__ice_rev[34] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::OpenMessageActions(seq, body, rev) => (|| {

let _ = &seq;
let _ = &body;
let _ = &rev;
if (seq <= 0) { return ::iced::Task::none(); }
{ let __ice_next = seq; if ::ui_lang_runtime::state_changed!(self.selected_message_seq, __ice_next) { self.selected_message_seq = __ice_next; self.__ice_rev[65] += 1; } }
{ let __ice_next = rev; if ::ui_lang_runtime::state_changed!(self.selected_message_rev, __ice_next) { self.selected_message_rev = __ice_next; self.__ice_rev[66] += 1; } }
{ let __ice_next = MessageAction::More; if ::ui_lang_runtime::state_changed!(self.message_action, __ice_next) { self.message_action = __ice_next; self.__ice_rev[67] += 1; } }
{ let __ice_next = body.to_owned(); if ::ui_lang_runtime::state_changed!(self.message_edit_draft, __ice_next) { self.message_edit_draft = __ice_next; self.__ice_rev[80] += 1; } }
return ::iced::Task::none().chain({ // __ICE_SOURCE 635 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6372617465732f76696577732f636861742f7372632f75692f6170702e696365
::ui_lang_guest::widget::perform::<__ChatViewMessage>(::ui_lang_guest::wire::WidgetCommand::Focus { target: ::std::string::String::from("ChatView/chat/message-action-focus") })
}).chain({ // __ICE_SOURCE 636 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6372617465732f76696577732f636861742f7372632f75692f6170702e696365
::ui_lang_guest::widget::perform::<__ChatViewMessage>(::ui_lang_guest::wire::WidgetCommand::FocusNext)
});
})(),
__ChatViewMessage::OpenMessageReactions(seq, body, rev) => (|| {

let _ = &seq;
let _ = &body;
let _ = &rev;
if (seq <= 0) { return ::iced::Task::none(); }
{ let __ice_next = crate::host::reaction_refusal(self.active_channel_archived, ::std::convert::AsRef::as_ref(&(self.host_error))); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[84] += 1; } }
if self.active_channel_archived { return ::iced::Task::none(); }
{ let __ice_next = seq; if ::ui_lang_runtime::state_changed!(self.selected_message_seq, __ice_next) { self.selected_message_seq = __ice_next; self.__ice_rev[65] += 1; } }
{ let __ice_next = rev; if ::ui_lang_runtime::state_changed!(self.selected_message_rev, __ice_next) { self.selected_message_rev = __ice_next; self.__ice_rev[66] += 1; } }
{ let __ice_next = MessageAction::Reactions; if ::ui_lang_runtime::state_changed!(self.message_action, __ice_next) { self.message_action = __ice_next; self.__ice_rev[67] += 1; } }
{ let __ice_next = body.to_owned(); if ::ui_lang_runtime::state_changed!(self.message_edit_draft, __ice_next) { self.message_edit_draft = __ice_next; self.__ice_rev[80] += 1; } }
return ::iced::Task::none().chain({ // __ICE_SOURCE 651 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6372617465732f76696577732f636861742f7372632f75692f6170702e696365
::ui_lang_guest::widget::perform::<__ChatViewMessage>(::ui_lang_guest::wire::WidgetCommand::Focus { target: ::std::string::String::from("ChatView/chat/message-reaction-focus") })
}).chain({ // __ICE_SOURCE 652 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6372617465732f76696577732f636861742f7372632f75692f6170702e696365
::ui_lang_guest::widget::perform::<__ChatViewMessage>(::ui_lang_guest::wire::WidgetCommand::FocusNext)
});
})(),
__ChatViewMessage::BeginMessageEdit(seq, body, rev) => (|| {

let _ = &seq;
let _ = &body;
let _ = &rev;
if (seq <= 0) { return ::iced::Task::none(); }
let seed = crate::host::edit_body_of(::std::convert::AsRef::as_ref(&(self.messages)), seq, rev);
if (seed).is_empty() { return ::iced::Task::none(); }
{ let __ice_next = crate::host::send_begin_edit(::std::convert::AsRef::as_ref(&(crate::host::edit_scope(::std::convert::AsRef::as_ref(&(self.endpoint)), ::std::convert::AsRef::as_ref(&(self.active_channel)), seq))), ::std::convert::AsRef::as_ref(&(seed)), seq, rev); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[85] += 1; } }
{ let __ice_next = seq; if ::ui_lang_runtime::state_changed!(self.selected_message_seq, __ice_next) { self.selected_message_seq = __ice_next; self.__ice_rev[65] += 1; } }
{ let __ice_next = rev; if ::ui_lang_runtime::state_changed!(self.selected_message_rev, __ice_next) { self.selected_message_rev = __ice_next; self.__ice_rev[66] += 1; } }
{ let __ice_next = MessageAction::Editing; if ::ui_lang_runtime::state_changed!(self.message_action, __ice_next) { self.message_action = __ice_next; self.__ice_rev[67] += 1; } }
{ let __ice_next = body.to_owned(); if ::ui_lang_runtime::state_changed!(self.message_edit_draft, __ice_next) { self.message_edit_draft = __ice_next; self.__ice_rev[80] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::ArmMessageDelete(seq, body, rev) => (|| {

let _ = &seq;
let _ = &body;
let _ = &rev;
if (seq <= 0) { return ::iced::Task::none(); }
{ let __ice_next = seq; if ::ui_lang_runtime::state_changed!(self.selected_message_seq, __ice_next) { self.selected_message_seq = __ice_next; self.__ice_rev[65] += 1; } }
{ let __ice_next = rev; if ::ui_lang_runtime::state_changed!(self.selected_message_rev, __ice_next) { self.selected_message_rev = __ice_next; self.__ice_rev[66] += 1; } }
{ let __ice_next = MessageAction::Delete; if ::ui_lang_runtime::state_changed!(self.message_action, __ice_next) { self.message_action = __ice_next; self.__ice_rev[67] += 1; } }
{ let __ice_next = body.to_owned(); if ::ui_lang_runtime::state_changed!(self.message_edit_draft, __ice_next) { self.message_edit_draft = __ice_next; self.__ice_rev[80] += 1; } }
return ::iced::Task::none().chain({ // __ICE_SOURCE 675 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6372617465732f76696577732f636861742f7372632f75692f6170702e696365
::ui_lang_guest::widget::perform::<__ChatViewMessage>(::ui_lang_guest::wire::WidgetCommand::Focus { target: ::std::string::String::from("ChatView/chat/message-delete-focus") })
}).chain({ // __ICE_SOURCE 676 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6372617465732f76696577732f636861742f7372632f75692f6170702e696365
::ui_lang_guest::widget::perform::<__ChatViewMessage>(::ui_lang_guest::wire::WidgetCommand::FocusNext)
});
})(),
__ChatViewMessage::ClearMessageSelection => (|| {

{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.selected_message_seq, __ice_next) { self.selected_message_seq = __ice_next; self.__ice_rev[65] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.selected_message_rev, __ice_next) { self.selected_message_rev = __ice_next; self.__ice_rev[66] += 1; } }
{ let __ice_next = MessageAction::Toolbar; if ::ui_lang_runtime::state_changed!(self.message_action, __ice_next) { self.message_action = __ice_next; self.__ice_rev[67] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.message_edit_draft, __ice_next) { self.message_edit_draft = __ice_next; self.__ice_rev[80] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::OpenThreadMessageActions(seq, body, rev) => (|| {

let _ = &seq;
let _ = &body;
let _ = &rev;
if (seq <= 0) { return ::iced::Task::none(); }
{ let __ice_next = seq; if ::ui_lang_runtime::state_changed!(self.thread_selected_seq, __ice_next) { self.thread_selected_seq = __ice_next; self.__ice_rev[69] += 1; } }
{ let __ice_next = rev; if ::ui_lang_runtime::state_changed!(self.thread_selected_rev, __ice_next) { self.thread_selected_rev = __ice_next; self.__ice_rev[70] += 1; } }
{ let __ice_next = MessageAction::More; if ::ui_lang_runtime::state_changed!(self.thread_message_action, __ice_next) { self.thread_message_action = __ice_next; self.__ice_rev[71] += 1; } }
{ let __ice_next = body.to_owned(); if ::ui_lang_runtime::state_changed!(self.thread_edit_draft, __ice_next) { self.thread_edit_draft = __ice_next; self.__ice_rev[83] += 1; } }
return ::iced::Task::none().chain({ // __ICE_SOURCE 691 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6372617465732f76696577732f636861742f7372632f75692f6170702e696365
::ui_lang_guest::widget::perform::<__ChatViewMessage>(::ui_lang_guest::wire::WidgetCommand::Focus { target: ::std::string::String::from("ChatView/chat/thread-pane/thread-action-focus") })
}).chain({ // __ICE_SOURCE 692 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6372617465732f76696577732f636861742f7372632f75692f6170702e696365
::ui_lang_guest::widget::perform::<__ChatViewMessage>(::ui_lang_guest::wire::WidgetCommand::FocusNext)
});
})(),
__ChatViewMessage::OpenThreadMessageReactions(seq, body, rev) => (|| {

let _ = &seq;
let _ = &body;
let _ = &rev;
if (seq <= 0) { return ::iced::Task::none(); }
{ let __ice_next = crate::host::reaction_refusal(self.active_channel_archived, ::std::convert::AsRef::as_ref(&(self.host_error))); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[84] += 1; } }
if self.active_channel_archived { return ::iced::Task::none(); }
{ let __ice_next = seq; if ::ui_lang_runtime::state_changed!(self.thread_selected_seq, __ice_next) { self.thread_selected_seq = __ice_next; self.__ice_rev[69] += 1; } }
{ let __ice_next = rev; if ::ui_lang_runtime::state_changed!(self.thread_selected_rev, __ice_next) { self.thread_selected_rev = __ice_next; self.__ice_rev[70] += 1; } }
{ let __ice_next = MessageAction::Reactions; if ::ui_lang_runtime::state_changed!(self.thread_message_action, __ice_next) { self.thread_message_action = __ice_next; self.__ice_rev[71] += 1; } }
{ let __ice_next = body.to_owned(); if ::ui_lang_runtime::state_changed!(self.thread_edit_draft, __ice_next) { self.thread_edit_draft = __ice_next; self.__ice_rev[83] += 1; } }
return ::iced::Task::none().chain({ // __ICE_SOURCE 703 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6372617465732f76696577732f636861742f7372632f75692f6170702e696365
::ui_lang_guest::widget::perform::<__ChatViewMessage>(::ui_lang_guest::wire::WidgetCommand::Focus { target: ::std::string::String::from("ChatView/chat/thread-pane/thread-reaction-focus") })
}).chain({ // __ICE_SOURCE 704 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6372617465732f76696577732f636861742f7372632f75692f6170702e696365
::ui_lang_guest::widget::perform::<__ChatViewMessage>(::ui_lang_guest::wire::WidgetCommand::FocusNext)
});
})(),
__ChatViewMessage::BeginThreadMessageEdit(seq, body, rev) => (|| {

let _ = &seq;
let _ = &body;
let _ = &rev;
if (seq <= 0) { return ::iced::Task::none(); }
let seed = crate::host::edit_body_of(::std::convert::AsRef::as_ref(&(self.thread_messages)), seq, rev);
if (seed).is_empty() { return ::iced::Task::none(); }
{ let __ice_next = crate::host::send_begin_edit(::std::convert::AsRef::as_ref(&(crate::host::edit_scope(::std::convert::AsRef::as_ref(&(self.endpoint)), ::std::convert::AsRef::as_ref(&(self.active_channel)), seq))), ::std::convert::AsRef::as_ref(&(seed)), seq, rev); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[85] += 1; } }
{ let __ice_next = seq; if ::ui_lang_runtime::state_changed!(self.thread_selected_seq, __ice_next) { self.thread_selected_seq = __ice_next; self.__ice_rev[69] += 1; } }
{ let __ice_next = rev; if ::ui_lang_runtime::state_changed!(self.thread_selected_rev, __ice_next) { self.thread_selected_rev = __ice_next; self.__ice_rev[70] += 1; } }
{ let __ice_next = MessageAction::Editing; if ::ui_lang_runtime::state_changed!(self.thread_message_action, __ice_next) { self.thread_message_action = __ice_next; self.__ice_rev[71] += 1; } }
{ let __ice_next = body.to_owned(); if ::ui_lang_runtime::state_changed!(self.thread_edit_draft, __ice_next) { self.thread_edit_draft = __ice_next; self.__ice_rev[83] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::ArmThreadMessageDelete(seq, body, rev) => (|| {

let _ = &seq;
let _ = &body;
let _ = &rev;
if (seq <= 0) { return ::iced::Task::none(); }
{ let __ice_next = seq; if ::ui_lang_runtime::state_changed!(self.thread_selected_seq, __ice_next) { self.thread_selected_seq = __ice_next; self.__ice_rev[69] += 1; } }
{ let __ice_next = rev; if ::ui_lang_runtime::state_changed!(self.thread_selected_rev, __ice_next) { self.thread_selected_rev = __ice_next; self.__ice_rev[70] += 1; } }
{ let __ice_next = MessageAction::Delete; if ::ui_lang_runtime::state_changed!(self.thread_message_action, __ice_next) { self.thread_message_action = __ice_next; self.__ice_rev[71] += 1; } }
{ let __ice_next = body.to_owned(); if ::ui_lang_runtime::state_changed!(self.thread_edit_draft, __ice_next) { self.thread_edit_draft = __ice_next; self.__ice_rev[83] += 1; } }
return ::iced::Task::none().chain({ // __ICE_SOURCE 723 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6372617465732f76696577732f636861742f7372632f75692f6170702e696365
::ui_lang_guest::widget::perform::<__ChatViewMessage>(::ui_lang_guest::wire::WidgetCommand::Focus { target: ::std::string::String::from("ChatView/chat/thread-pane/thread-delete-focus") })
}).chain({ // __ICE_SOURCE 724 1 2f686f6d652f656464792f6465762f6475636b746170652f6475636b746170652f2e636f6465782f776f726b74726565732f677075692d6b69742d6d6967726174696f6e2f6372617465732f76696577732f636861742f7372632f75692f6170702e696365
::ui_lang_guest::widget::perform::<__ChatViewMessage>(::ui_lang_guest::wire::WidgetCommand::FocusNext)
});
})(),
__ChatViewMessage::ClearThreadMessageSelection => (|| {

{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.thread_selected_seq, __ice_next) { self.thread_selected_seq = __ice_next; self.__ice_rev[69] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.thread_selected_rev, __ice_next) { self.thread_selected_rev = __ice_next; self.__ice_rev[70] += 1; } }
{ let __ice_next = MessageAction::Toolbar; if ::ui_lang_runtime::state_changed!(self.thread_message_action, __ice_next) { self.thread_message_action = __ice_next; self.__ice_rev[71] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.thread_edit_draft, __ice_next) { self.thread_edit_draft = __ice_next; self.__ice_rev[83] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::OpenThreadFor(seq) => (|| {

let _ = &seq;
if ((seq <= 0) || (self.active_channel).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.channel_settings_open, __ice_next) { self.channel_settings_open = __ice_next; self.__ice_rev[68] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.selected_message_seq, __ice_next) { self.selected_message_seq = __ice_next; self.__ice_rev[65] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.selected_message_rev, __ice_next) { self.selected_message_rev = __ice_next; self.__ice_rev[66] += 1; } }
{ let __ice_next = MessageAction::Toolbar; if ::ui_lang_runtime::state_changed!(self.message_action, __ice_next) { self.message_action = __ice_next; self.__ice_rev[67] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.message_edit_draft, __ice_next) { self.message_edit_draft = __ice_next; self.__ice_rev[80] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.thread_selected_seq, __ice_next) { self.thread_selected_seq = __ice_next; self.__ice_rev[69] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.thread_selected_rev, __ice_next) { self.thread_selected_rev = __ice_next; self.__ice_rev[70] += 1; } }
{ let __ice_next = MessageAction::Toolbar; if ::ui_lang_runtime::state_changed!(self.thread_message_action, __ice_next) { self.thread_message_action = __ice_next; self.__ice_rev[71] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.thread_edit_draft, __ice_next) { self.thread_edit_draft = __ice_next; self.__ice_rev[83] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.copy_anchor_seq, __ice_next) { self.copy_anchor_seq = __ice_next; self.__ice_rev[72] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.copy_head_seq, __ice_next) { self.copy_head_seq = __ice_next; self.__ice_rev[73] += 1; } }
{ let __ice_next = CopySurface::Nowhere; if ::ui_lang_runtime::state_changed!(self.copy_surface, __ice_next) { self.copy_surface = __ice_next; self.__ice_rev[74] += 1; } }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.thread_loading, __ice_next) { self.thread_loading = __ice_next; self.__ice_rev[56] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.thread_messages, __ice_next) { self.thread_messages = __ice_next; self.__ice_rev[53] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.thread_has_more, __ice_next) { self.thread_has_more = __ice_next; self.__ice_rev[54] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.thread_next_reply_seq, __ice_next) { self.thread_next_reply_seq = __ice_next; self.__ice_rev[55] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.thread_pages, __ice_next) { self.thread_pages = __ice_next; self.__ice_rev[47] += 1; } }
{ let __ice_next = seq; if ::ui_lang_runtime::state_changed!(self.active_thread_seq, __ice_next) { self.active_thread_seq = __ice_next; self.__ice_rev[49] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.thread_target_seq, __ice_next) { self.thread_target_seq = __ice_next; self.__ice_rev[50] += 1; } }
{ let __ice_next = crate::host::thread_key((self.connection_serial + self.room_serial), self.names_serial, ::std::convert::AsRef::as_ref(&(self.active_channel)), seq, 0, 0); if ::ui_lang_runtime::state_changed!(self.thread_key, __ice_next) { self.thread_key = __ice_next; self.__ice_rev[48] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::CloseThread => (|| {

{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.active_thread_seq, __ice_next) { self.active_thread_seq = __ice_next; self.__ice_rev[49] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.thread_target_seq, __ice_next) { self.thread_target_seq = __ice_next; self.__ice_rev[50] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.thread_messages, __ice_next) { self.thread_messages = __ice_next; self.__ice_rev[53] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.thread_pages, __ice_next) { self.thread_pages = __ice_next; self.__ice_rev[47] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.thread_has_more, __ice_next) { self.thread_has_more = __ice_next; self.__ice_rev[54] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.thread_next_reply_seq, __ice_next) { self.thread_next_reply_seq = __ice_next; self.__ice_rev[55] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.thread_loading, __ice_next) { self.thread_loading = __ice_next; self.__ice_rev[56] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.thread_selected_seq, __ice_next) { self.thread_selected_seq = __ice_next; self.__ice_rev[69] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.thread_selected_rev, __ice_next) { self.thread_selected_rev = __ice_next; self.__ice_rev[70] += 1; } }
{ let __ice_next = MessageAction::Toolbar; if ::ui_lang_runtime::state_changed!(self.thread_message_action, __ice_next) { self.thread_message_action = __ice_next; self.__ice_rev[71] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.thread_edit_draft, __ice_next) { self.thread_edit_draft = __ice_next; self.__ice_rev[83] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.copy_anchor_seq, __ice_next) { self.copy_anchor_seq = __ice_next; self.__ice_rev[72] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.copy_head_seq, __ice_next) { self.copy_head_seq = __ice_next; self.__ice_rev[73] += 1; } }
{ let __ice_next = CopySurface::Nowhere; if ::ui_lang_runtime::state_changed!(self.copy_surface, __ice_next) { self.copy_surface = __ice_next; self.__ice_rev[74] += 1; } }
{ let __ice_next = crate::host::thread_key((self.connection_serial + self.room_serial), self.names_serial, ::std::convert::AsRef::as_ref(&(self.active_channel)), 0, 0, 0); if ::ui_lang_runtime::state_changed!(self.thread_key, __ice_next) { self.thread_key = __ice_next; self.__ice_rev[48] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::LoadMoreThread => (|| {

if (((self.thread_loading || self.busy) || (self.active_thread_seq <= 0)) || (!self.thread_has_more)) { return ::iced::Task::none(); }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.thread_loading, __ice_next) { self.thread_loading = __ice_next; self.__ice_rev[56] += 1; } }
{ let __ice_next = (self.thread_pages + 1); if ::ui_lang_runtime::state_changed!(self.thread_pages, __ice_next) { self.thread_pages = __ice_next; self.__ice_rev[47] += 1; } }
{ let __ice_next = crate::host::thread_key((self.connection_serial + self.room_serial), self.names_serial, ::std::convert::AsRef::as_ref(&(self.active_channel)), self.active_thread_seq, self.thread_target_seq, self.thread_pages); if ::ui_lang_runtime::state_changed!(self.thread_key, __ice_next) { self.thread_key = __ice_next; self.__ice_rev[48] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::AddReactionSubmit(emoji) => (|| {

let _ = &emoji;
if ((self.active_channel).is_empty() || (self.selected_message_seq <= 0)) { return ::iced::Task::none(); }
{ let __ice_next = crate::host::reaction_refusal(self.active_channel_archived, ::std::convert::AsRef::as_ref(&(self.host_error))); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[84] += 1; } }
if self.active_channel_archived { return ::iced::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[84] += 1; } }
{ let __ice_next = crate::host::reaction_applied(::std::convert::AsRef::as_ref(&(self.room_messages)), self.selected_message_seq, ::std::convert::AsRef::as_ref(&(emoji)), true); if ::ui_lang_runtime::state_changed!(self.room_messages, __ice_next) { self.room_messages = __ice_next; self.__ice_rev[36] += 1; } }
{ let __ice_next = crate::host::with_pending(::std::convert::AsRef::as_ref(&(self.room_messages)), ::std::convert::AsRef::as_ref(&(self.pending_sends)), 0, ::std::convert::AsRef::as_ref(&(self.me))); if ::ui_lang_runtime::state_changed!(self.messages, __ice_next) { self.messages = __ice_next; self.__ice_rev[37] += 1; } }
{ let __ice_next = crate::host::timeline_of(::std::convert::AsRef::as_ref(&(self.messages)), ::std::convert::AsRef::as_ref(&(self.live_agents))); if ::ui_lang_runtime::state_changed!(self.timeline, __ice_next) { self.timeline = __ice_next; self.__ice_rev[46] += 1; } }
{ let __ice_next = crate::host::reaction_applied(::std::convert::AsRef::as_ref(&(self.thread_messages)), self.selected_message_seq, ::std::convert::AsRef::as_ref(&(emoji)), true); if ::ui_lang_runtime::state_changed!(self.thread_messages, __ice_next) { self.thread_messages = __ice_next; self.__ice_rev[53] += 1; } }
{ let __ice_next = ({ crate::host::write_reaction(::std::convert::AsRef::as_ref(&(self.active_channel)), self.selected_message_seq, ::std::convert::AsRef::as_ref(&(emoji)), true) }); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[85] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::AddReactionAt(seq, emoji) => (|| {

let _ = &seq;
let _ = &emoji;
if ((self.active_channel).is_empty() || (seq <= 0)) { return ::iced::Task::none(); }
{ let __ice_next = crate::host::reaction_refusal(self.active_channel_archived, ::std::convert::AsRef::as_ref(&(self.host_error))); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[84] += 1; } }
if self.active_channel_archived { return ::iced::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[84] += 1; } }
{ let __ice_next = crate::host::reaction_applied(::std::convert::AsRef::as_ref(&(self.room_messages)), seq, ::std::convert::AsRef::as_ref(&(emoji)), true); if ::ui_lang_runtime::state_changed!(self.room_messages, __ice_next) { self.room_messages = __ice_next; self.__ice_rev[36] += 1; } }
{ let __ice_next = crate::host::with_pending(::std::convert::AsRef::as_ref(&(self.room_messages)), ::std::convert::AsRef::as_ref(&(self.pending_sends)), 0, ::std::convert::AsRef::as_ref(&(self.me))); if ::ui_lang_runtime::state_changed!(self.messages, __ice_next) { self.messages = __ice_next; self.__ice_rev[37] += 1; } }
{ let __ice_next = crate::host::timeline_of(::std::convert::AsRef::as_ref(&(self.messages)), ::std::convert::AsRef::as_ref(&(self.live_agents))); if ::ui_lang_runtime::state_changed!(self.timeline, __ice_next) { self.timeline = __ice_next; self.__ice_rev[46] += 1; } }
{ let __ice_next = crate::host::reaction_applied(::std::convert::AsRef::as_ref(&(self.thread_messages)), seq, ::std::convert::AsRef::as_ref(&(emoji)), true); if ::ui_lang_runtime::state_changed!(self.thread_messages, __ice_next) { self.thread_messages = __ice_next; self.__ice_rev[53] += 1; } }
{ let __ice_next = ({ crate::host::write_reaction(::std::convert::AsRef::as_ref(&(self.active_channel)), seq, ::std::convert::AsRef::as_ref(&(emoji)), true) }); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[85] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::RemoveReactionAt(seq, emoji) => (|| {

let _ = &seq;
let _ = &emoji;
if ((self.active_channel).is_empty() || (seq <= 0)) { return ::iced::Task::none(); }
{ let __ice_next = crate::host::reaction_refusal(self.active_channel_archived, ::std::convert::AsRef::as_ref(&(self.host_error))); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[84] += 1; } }
if self.active_channel_archived { return ::iced::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[84] += 1; } }
{ let __ice_next = crate::host::reaction_applied(::std::convert::AsRef::as_ref(&(self.room_messages)), seq, ::std::convert::AsRef::as_ref(&(emoji)), false); if ::ui_lang_runtime::state_changed!(self.room_messages, __ice_next) { self.room_messages = __ice_next; self.__ice_rev[36] += 1; } }
{ let __ice_next = crate::host::with_pending(::std::convert::AsRef::as_ref(&(self.room_messages)), ::std::convert::AsRef::as_ref(&(self.pending_sends)), 0, ::std::convert::AsRef::as_ref(&(self.me))); if ::ui_lang_runtime::state_changed!(self.messages, __ice_next) { self.messages = __ice_next; self.__ice_rev[37] += 1; } }
{ let __ice_next = crate::host::timeline_of(::std::convert::AsRef::as_ref(&(self.messages)), ::std::convert::AsRef::as_ref(&(self.live_agents))); if ::ui_lang_runtime::state_changed!(self.timeline, __ice_next) { self.timeline = __ice_next; self.__ice_rev[46] += 1; } }
{ let __ice_next = crate::host::reaction_applied(::std::convert::AsRef::as_ref(&(self.thread_messages)), seq, ::std::convert::AsRef::as_ref(&(emoji)), false); if ::ui_lang_runtime::state_changed!(self.thread_messages, __ice_next) { self.thread_messages = __ice_next; self.__ice_rev[53] += 1; } }
{ let __ice_next = ({ crate::host::write_reaction(::std::convert::AsRef::as_ref(&(self.active_channel)), seq, ::std::convert::AsRef::as_ref(&(emoji)), false) }); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[85] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::DeleteMessageSubmit => (|| {

if (((self.busy || (self.active_channel).is_empty()) || (self.selected_message_seq <= 0)) || (self.message_action != MessageAction::Delete)) { return ::iced::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[84] += 1; } }
{ let __ice_next = ({ crate::host::write_delete(::std::convert::AsRef::as_ref(&(self.active_channel)), self.selected_message_seq) }); if ::ui_lang_runtime::state_changed!(self.busy, __ice_next) { self.busy = __ice_next; self.__ice_rev[45] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::DeleteThreadMessageSubmit => (|| {

if (((self.busy || (self.active_channel).is_empty()) || (self.thread_selected_seq <= 0)) || (self.thread_message_action != MessageAction::Delete)) { return ::iced::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[84] += 1; } }
{ let __ice_next = ({ crate::host::write_delete(::std::convert::AsRef::as_ref(&(self.active_channel)), self.thread_selected_seq) }); if ::ui_lang_runtime::state_changed!(self.busy, __ice_next) { self.busy = __ice_next; self.__ice_rev[45] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::RenameChannelSubmit => (|| {

if ((self.busy || (self.active_channel).is_empty()) || ((self.channel_name_draft).trim().to_owned()).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[84] += 1; } }
{ let __ice_next = ({ crate::host::write_rename(::std::convert::AsRef::as_ref(&(self.active_channel)), ::std::convert::AsRef::as_ref(&((self.channel_name_draft).trim().to_owned()))) }); if ::ui_lang_runtime::state_changed!(self.busy, __ice_next) { self.busy = __ice_next; self.__ice_rev[45] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::ArchiveChannelSubmit => (|| {

if ((self.busy || (self.active_channel).is_empty()) || self.active_channel_archived) { return ::iced::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[84] += 1; } }
{ let __ice_next = ({ crate::host::write_archived(::std::convert::AsRef::as_ref(&(self.active_channel)), true) }); if ::ui_lang_runtime::state_changed!(self.busy, __ice_next) { self.busy = __ice_next; self.__ice_rev[45] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::UnarchiveChannelSubmit => (|| {

if ((self.busy || (self.active_channel).is_empty()) || (!self.active_channel_archived)) { return ::iced::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[84] += 1; } }
{ let __ice_next = ({ crate::host::write_archived(::std::convert::AsRef::as_ref(&(self.active_channel)), false) }); if ::ui_lang_runtime::state_changed!(self.busy, __ice_next) { self.busy = __ice_next; self.__ice_rev[45] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::AddChannelMemberSubmit => (|| {

if ((self.busy || (self.active_channel).is_empty()) || ((self.member_key_draft).trim().to_owned()).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[84] += 1; } }
{ let __ice_next = ({ crate::host::write_membership(::std::convert::AsRef::as_ref(&(self.active_channel)), ::std::convert::AsRef::as_ref(&((self.member_key_draft).trim().to_owned())), true) }); if ::ui_lang_runtime::state_changed!(self.busy, __ice_next) { self.busy = __ice_next; self.__ice_rev[45] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::RemoveChannelMemberSubmit(key) => (|| {

let _ = &key;
if ((self.busy || (self.active_channel).is_empty()) || (key).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[84] += 1; } }
{ let __ice_next = ({ crate::host::write_membership(::std::convert::AsRef::as_ref(&(self.active_channel)), ::std::convert::AsRef::as_ref(&(key)), false) }); if ::ui_lang_runtime::state_changed!(self.busy, __ice_next) { self.busy = __ice_next; self.__ice_rev[45] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::PressMessage(seq, surface) => (|| {

let _ = &seq;
let _ = &surface;
if (!self.shift_held) { return ::iced::Task::none(); }
let range = crate::host::copy_range_after_press(self.copy_anchor_seq, self.copy_surface.clone(), seq, surface.clone());
{ let __ice_next = range.anchor; if ::ui_lang_runtime::state_changed!(self.copy_anchor_seq, __ice_next) { self.copy_anchor_seq = __ice_next; self.__ice_rev[72] += 1; } }
{ let __ice_next = range.head; if ::ui_lang_runtime::state_changed!(self.copy_head_seq, __ice_next) { self.copy_head_seq = __ice_next; self.__ice_rev[73] += 1; } }
{ let __ice_next = crate::host::copy_surface_of(::std::convert::AsRef::as_ref(&(range.surface))); if ::ui_lang_runtime::state_changed!(self.copy_surface, __ice_next) { self.copy_surface = __ice_next; self.__ice_rev[74] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::ClearCopyRange => (|| {

{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.copy_anchor_seq, __ice_next) { self.copy_anchor_seq = __ice_next; self.__ice_rev[72] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.copy_head_seq, __ice_next) { self.copy_head_seq = __ice_next; self.__ice_rev[73] += 1; } }
{ let __ice_next = CopySurface::Nowhere; if ::ui_lang_runtime::state_changed!(self.copy_surface, __ice_next) { self.copy_surface = __ice_next; self.__ice_rev[74] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::CopySelectedMessages => (|| {

let rows = crate::host::copy_range_rows(::std::convert::AsRef::as_ref(&(self.messages)), ::std::convert::AsRef::as_ref(&(self.thread_messages)), self.copy_surface.clone());
let count = crate::host::copy_range_count(::std::convert::AsRef::as_ref(&(rows)), self.copy_anchor_seq, self.copy_head_seq);
if (count == 0) { return ::iced::Task::none(); }
{ let __ice_next = crate::host::send_copy(::std::convert::AsRef::as_ref(&(crate::host::copy_range_text(::std::convert::AsRef::as_ref(&(rows)), self.copy_anchor_seq, self.copy_head_seq))), ::std::convert::AsRef::as_ref(&(crate::host::copy_range_label(count)))); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[85] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::CopyChord(fired) => (|| {

let _ = &fired;
if (!fired) { return ::iced::Task::none(); }
let rows = crate::host::copy_range_rows(::std::convert::AsRef::as_ref(&(self.messages)), ::std::convert::AsRef::as_ref(&(self.thread_messages)), self.copy_surface.clone());
let count = crate::host::copy_range_count(::std::convert::AsRef::as_ref(&(rows)), self.copy_anchor_seq, self.copy_head_seq);
if (count == 0) { return ::iced::Task::none(); }
{ let __ice_next = crate::host::send_copy(::std::convert::AsRef::as_ref(&(crate::host::copy_range_text(::std::convert::AsRef::as_ref(&(rows)), self.copy_anchor_seq, self.copy_head_seq))), ::std::convert::AsRef::as_ref(&(crate::host::copy_range_label(count)))); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[85] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::__0C4368617453637265656eH636861745f706f696e7465725f70726573736564(__scope, _x, y) => (|| {

let _ = &_x;
let _ = &y;
::ui_lang_guest::invalidate_component("ChatScreen", &(__scope.clone())); let __local = self.__ice_component_04368617453637265656e.entry(__scope.clone()).or_insert_with(|| __IceChatScreenState {message_action_focus: self.__ice_component_04368617453637265656e_initial.message_action_focus.clone(),chat_pointer_y: self.__ice_component_04368617453637265656e_initial.chat_pointer_y.clone(),chat_height: self.__ice_component_04368617453637265656e_initial.chat_height.clone(),thread_pointer_y: self.__ice_component_04368617453637265656e_initial.thread_pointer_y.clone(),thread_height: self.__ice_component_04368617453637265656e_initial.thread_height.clone(),__ice_rev: [::ui_lang_runtime::rev::seed(); 5],});
{ let __ice_next = y; if ::ui_lang_runtime::state_changed!(__local.chat_pointer_y, __ice_next) { __local.chat_pointer_y = __ice_next; __local.__ice_rev[1] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::__0C4368617453637265656eH636861745f726573697a6564(__scope, _width, height) => (|| {

let _ = &_width;
let _ = &height;
::ui_lang_guest::invalidate_component("ChatScreen", &(__scope.clone())); let __local = self.__ice_component_04368617453637265656e.entry(__scope.clone()).or_insert_with(|| __IceChatScreenState {message_action_focus: self.__ice_component_04368617453637265656e_initial.message_action_focus.clone(),chat_pointer_y: self.__ice_component_04368617453637265656e_initial.chat_pointer_y.clone(),chat_height: self.__ice_component_04368617453637265656e_initial.chat_height.clone(),thread_pointer_y: self.__ice_component_04368617453637265656e_initial.thread_pointer_y.clone(),thread_height: self.__ice_component_04368617453637265656e_initial.thread_height.clone(),__ice_rev: [::ui_lang_runtime::rev::seed(); 5],});
{ let __ice_next = height; if ::ui_lang_runtime::state_changed!(__local.chat_height, __ice_next) { __local.chat_height = __ice_next; __local.__ice_rev[2] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::__0C4368617453637265656eH7468726561645f706f696e7465725f70726573736564(__scope, _x, y) => (|| {

let _ = &_x;
let _ = &y;
::ui_lang_guest::invalidate_component("ChatScreen", &(__scope.clone())); let __local = self.__ice_component_04368617453637265656e.entry(__scope.clone()).or_insert_with(|| __IceChatScreenState {message_action_focus: self.__ice_component_04368617453637265656e_initial.message_action_focus.clone(),chat_pointer_y: self.__ice_component_04368617453637265656e_initial.chat_pointer_y.clone(),chat_height: self.__ice_component_04368617453637265656e_initial.chat_height.clone(),thread_pointer_y: self.__ice_component_04368617453637265656e_initial.thread_pointer_y.clone(),thread_height: self.__ice_component_04368617453637265656e_initial.thread_height.clone(),__ice_rev: [::ui_lang_runtime::rev::seed(); 5],});
{ let __ice_next = y; if ::ui_lang_runtime::state_changed!(__local.thread_pointer_y, __ice_next) { __local.thread_pointer_y = __ice_next; __local.__ice_rev[3] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::__0C4368617453637265656eH7468726561645f726573697a6564(__scope, _width, height) => (|| {

let _ = &_width;
let _ = &height;
::ui_lang_guest::invalidate_component("ChatScreen", &(__scope.clone())); let __local = self.__ice_component_04368617453637265656e.entry(__scope.clone()).or_insert_with(|| __IceChatScreenState {message_action_focus: self.__ice_component_04368617453637265656e_initial.message_action_focus.clone(),chat_pointer_y: self.__ice_component_04368617453637265656e_initial.chat_pointer_y.clone(),chat_height: self.__ice_component_04368617453637265656e_initial.chat_height.clone(),thread_pointer_y: self.__ice_component_04368617453637265656e_initial.thread_pointer_y.clone(),thread_height: self.__ice_component_04368617453637265656e_initial.thread_height.clone(),__ice_rev: [::ui_lang_runtime::rev::seed(); 5],});
{ let __ice_next = height; if ::ui_lang_runtime::state_changed!(__local.thread_height, __ice_next) { __local.thread_height = __ice_next; __local.__ice_rev[4] += 1; } }
::iced::Task::none()
})(),
__ChatViewMessage::__0C4368617453637265656eB6d6573736167655f616374696f6e5f666f637573(__scope, value) => { ::ui_lang_guest::invalidate_component("ChatScreen", &(__scope)); let __local = self.__ice_component_04368617453637265656e.entry(__scope).or_insert_with(|| __IceChatScreenState {message_action_focus: self.__ice_component_04368617453637265656e_initial.message_action_focus.clone(),chat_pointer_y: self.__ice_component_04368617453637265656e_initial.chat_pointer_y.clone(),chat_height: self.__ice_component_04368617453637265656e_initial.chat_height.clone(),thread_pointer_y: self.__ice_component_04368617453637265656e_initial.thread_pointer_y.clone(),thread_height: self.__ice_component_04368617453637265656e_initial.thread_height.clone(),__ice_rev: [::ui_lang_runtime::rev::seed(); 5],}); { let __ice_next = value; if ::ui_lang_runtime::state_changed!(__local.message_action_focus, __ice_next) { __local.message_action_focus = __ice_next; __local.__ice_rev[0] += 1; } } ::iced::Task::none() },
__ChatViewMessage::__BindSearchDraft(value) => { { let __ice_next = value; if ::ui_lang_runtime::state_changed!(self.search_draft, __ice_next) { self.search_draft = __ice_next; self.__ice_rev[79] += 1; } } ::iced::Task::none() }
__ChatViewMessage::__BindChannelNameDraft(value) => { { let __ice_next = value; if ::ui_lang_runtime::state_changed!(self.channel_name_draft, __ice_next) { self.channel_name_draft = __ice_next; self.__ice_rev[81] += 1; } } ::iced::Task::none() }
__ChatViewMessage::__BindMemberKeyDraft(value) => { { let __ice_next = value; if ::ui_lang_runtime::state_changed!(self.member_key_draft, __ice_next) { self.member_key_draft = __ice_next; self.__ice_rev[82] += 1; } } ::iced::Task::none() }
__ChatViewMessage::__ExternNoop => ::iced::Task::none(),
}
}

}
}
