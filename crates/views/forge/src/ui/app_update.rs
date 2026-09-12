#[allow(warnings, clippy::all)]
mod __ice_group_app_update {
use super::*;
impl super::ForgeView {
#[allow(clippy::assign_op_pattern)]
pub(super) fn __update(&mut self, message: __ForgeViewMessage) -> ::iced::Task<__ForgeViewMessage> {
match message {
__ForgeViewMessage::SessionArrived(item) => (|| {

let _ = &item;
{ let __ice_next = item.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[76] += 1; } }
if (!(item.error).is_empty()) { return ::iced::Task::none(); }
let next = item.next.clone();
{ let __ice_next = crate::host::connection_serial_after(self.connected, next.connected, self.connection_serial); if ::ui_lang_runtime::state_changed!(self.connection_serial, __ice_next) { self.connection_serial = __ice_next; self.__ice_rev[8] += 1; } }
{ let __ice_next = next.connected; if ::ui_lang_runtime::state_changed!(self.connected, __ice_next) { self.connected = __ice_next; self.__ice_rev[1] += 1; } }
{ let __ice_next = next.dark; if ::ui_lang_runtime::state_changed!(self.dark, __ice_next) { self.dark = __ice_next; self.__ice_rev[2] += 1; } }
{ let __ice_next = next.org.to_owned(); if ::ui_lang_runtime::state_changed!(self.org, __ice_next) { self.org = __ice_next; self.__ice_rev[3] += 1; } }
{ let __ice_next = next.about.to_owned(); if ::ui_lang_runtime::state_changed!(self.about, __ice_next) { self.about = __ice_next; self.__ice_rev[4] += 1; } }
{ let __ice_next = next.tier.to_owned(); if ::ui_lang_runtime::state_changed!(self.tier, __ice_next) { self.tier = __ice_next; self.__ice_rev[5] += 1; } }
{ let __ice_next = next.network_chain_id.to_owned(); if ::ui_lang_runtime::state_changed!(self.network_chain_id, __ice_next) { self.network_chain_id = __ice_next; self.__ice_rev[6] += 1; } }
{ let __ice_next = next.connected_rpc.to_owned(); if ::ui_lang_runtime::state_changed!(self.connected_rpc, __ice_next) { self.connected_rpc = __ice_next; self.__ice_rev[7] += 1; } }
let routed = (next.link_tick != self.link_tick);
{ let __ice_next = next.link_tick; if ::ui_lang_runtime::state_changed!(self.link_tick, __ice_next) { self.link_tick = __ice_next; self.__ice_rev[9] += 1; } }
return match crate::host::appearance_of(next.dark) {
Appearance::Light => (|| {
{ let __ice_next = AppTheme::App; if ::ui_lang_runtime::state_changed!(self.active_palette, __ice_next) { self.active_palette = __ice_next; self.__ice_rev[0] += 1; } }
return (::iced::Task::done(crate::host::routed_link(routed, ::std::convert::AsRef::as_ref(&(next.link))))).map(|value| __ForgeViewMessage::ForgeLandLink(value));
})(),
Appearance::Dark => (|| {
{ let __ice_next = AppTheme::AppDark; if ::ui_lang_runtime::state_changed!(self.active_palette, __ice_next) { self.active_palette = __ice_next; self.__ice_rev[0] += 1; } }
return (::iced::Task::done(crate::host::routed_link(routed, ::std::convert::AsRef::as_ref(&(next.link))))).map(|value| __ForgeViewMessage::ForgeLandLink(value));
})(),
};
})(),
__ForgeViewMessage::ForgeLandLink(url) => (|| {

let _ = &url;
if (url).is_empty() { return ::iced::Task::none(); }
let link = crate::host::forge_link(::std::convert::AsRef::as_ref(&(url)));
if (link.repo).is_empty() { return ::iced::Task::none(); }
{ let __ice_next = link.number; if ::ui_lang_runtime::state_changed!(self.focus_number, __ice_next) { self.focus_number = __ice_next; self.__ice_rev[45] += 1; } }
{ let __ice_next = link.seq; if ::ui_lang_runtime::state_changed!(self.focus_seq, __ice_next) { self.focus_seq = __ice_next; self.__ice_rev[43] += 1; } }
{ let __ice_next = link.path.to_owned(); if ::ui_lang_runtime::state_changed!(self.focus_path, __ice_next) { self.focus_path = __ice_next; self.__ice_rev[69] += 1; } }
{ let __ice_next = link.rev.to_owned(); if ::ui_lang_runtime::state_changed!(self.focus_rev, __ice_next) { self.focus_rev = __ice_next; self.__ice_rev[70] += 1; } }
return (::iced::Task::done(link.repo.to_owned())).map(|value| __ForgeViewMessage::ForgeOpenRepo(value));
})(),
__ForgeViewMessage::ReposArrived(next) => (|| {

let _ = &next;
{ let __ice_next = next.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[76] += 1; } }
if (!(next.error).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = next.repos.clone(); if ::ui_lang_runtime::state_changed!(self.repos, __ice_next) { self.repos = __ice_next; self.__ice_rev[10] += 1; } }
{ let __ice_next = "ready".to_owned(); if ::ui_lang_runtime::state_changed!(self.list_phase, __ice_next) { self.list_phase = __ice_next; self.__ice_rev[11] += 1; } }
::iced::Task::none()
})(),
__ForgeViewMessage::RepoArrived(next) => (|| {

let _ = &next;
if (next.repo != self.open_repo) { return ::iced::Task::none(); }
{ let __ice_next = next.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[76] += 1; } }
if (!(next.error).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = next.branches.clone(); if ::ui_lang_runtime::state_changed!(self.branches, __ice_next) { self.branches = __ice_next; self.__ice_rev[14] += 1; } }
{ let __ice_next = next.items.clone(); if ::ui_lang_runtime::state_changed!(self.items, __ice_next) { self.items = __ice_next; self.__ice_rev[15] += 1; } }
{ let __ice_next = "ready".to_owned(); if ::ui_lang_runtime::state_changed!(self.repo_phase, __ice_next) { self.repo_phase = __ice_next; self.__ice_rev[13] += 1; } }
let parked = ((self.focus_number > 0) || (!(self.focus_path).is_empty()));
if (!parked) { return ::iced::Task::none(); }
{ let __ice_next = crate::host::keep_focus(self.focus_rev.to_owned(), ::std::convert::AsRef::as_ref(&(self.tree_rev))); if ::ui_lang_runtime::state_changed!(self.tree_rev, __ice_next) { self.tree_rev = __ice_next; self.__ice_rev[53] += 1; } }
{ let __ice_next = crate::host::keep_focus(crate::host::forge_parent(::std::convert::AsRef::as_ref(&(self.focus_path))), ::std::convert::AsRef::as_ref(&(self.tree_path))); if ::ui_lang_runtime::state_changed!(self.tree_path, __ice_next) { self.tree_path = __ice_next; self.__ice_rev[52] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.tree_entries, __ice_next) { self.tree_entries = __ice_next; self.__ice_rev[54] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.tree_truncated, __ice_next) { self.tree_truncated = __ice_next; self.__ice_rev[56] += 1; } }
{ let __ice_next = "loading".to_owned(); if ::ui_lang_runtime::state_changed!(self.tree_phase, __ice_next) { self.tree_phase = __ice_next; self.__ice_rev[57] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.focus_rev, __ice_next) { self.focus_rev = __ice_next; self.__ice_rev[70] += 1; } }
if (self.focus_number <= 0) { return ::iced::Task::none(); }
let number = self.focus_number;
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.focus_number, __ice_next) { self.focus_number = __ice_next; self.__ice_rev[45] += 1; } }
return (::iced::Task::done(number)).map(|value| __ForgeViewMessage::ForgeOpenItem(value));
})(),
__ForgeViewMessage::ItemArrived(next) => (|| {

let _ = &next;
if ((next.repo != self.open_repo) || (next.number != self.forge_item_number)) { return ::iced::Task::none(); }
{ let __ice_next = next.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[76] += 1; } }
if (!(next.error).is_empty()) { return ::iced::Task::none(); }
let branch_moved = crate::host::forge_branch_moved(::std::convert::AsRef::as_ref(&(next.source_oid)), ::std::convert::AsRef::as_ref(&(self.forge_item_source_oid)));
{ let __ice_next = crate::host::staged_comment_drop_note((branch_moved && (!(self.staged_comments).is_empty()))); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[76] += 1; } }
self.staged_comments = crate::host::keep_staged(branch_moved, ::std::mem::take(&mut self.staged_comments)); self.__ice_rev[50] += 1;
{ let __ice_next = crate::host::keep_draft(branch_moved, ::std::convert::AsRef::as_ref(&(self.comment_draft))); if ::ui_lang_runtime::state_changed!(self.comment_draft, __ice_next) { self.comment_draft = __ice_next; self.__ice_rev[72] += 1; } }
{ let __ice_next = crate::host::keep_draft(branch_moved, ::std::convert::AsRef::as_ref(&(self.comment_path))); if ::ui_lang_runtime::state_changed!(self.comment_path, __ice_next) { self.comment_path = __ice_next; self.__ice_rev[73] += 1; } }
{ let __ice_next = crate::host::keep_draft(branch_moved, ::std::convert::AsRef::as_ref(&(self.comment_line))); if ::ui_lang_runtime::state_changed!(self.comment_line, __ice_next) { self.comment_line = __ice_next; self.__ice_rev[74] += 1; } }
{ let __ice_next = crate::host::keep_draft(branch_moved, ::std::convert::AsRef::as_ref(&(self.comment_side))); if ::ui_lang_runtime::state_changed!(self.comment_side, __ice_next) { self.comment_side = __ice_next; self.__ice_rev[75] += 1; } }
{ let __ice_next = "ready".to_owned(); if ::ui_lang_runtime::state_changed!(self.item_phase, __ice_next) { self.item_phase = __ice_next; self.__ice_rev[18] += 1; } }
{ let __ice_next = next.kind.to_owned(); if ::ui_lang_runtime::state_changed!(self.forge_item_kind, __ice_next) { self.forge_item_kind = __ice_next; self.__ice_rev[19] += 1; } }
{ let __ice_next = crate::host::kind_tab(::std::convert::AsRef::as_ref(&(next.kind))); if ::ui_lang_runtime::state_changed!(self.tab, __ice_next) { self.tab = __ice_next; self.__ice_rev[16] += 1; } }
{ let __ice_next = next.title.to_owned(); if ::ui_lang_runtime::state_changed!(self.forge_item_title, __ice_next) { self.forge_item_title = __ice_next; self.__ice_rev[20] += 1; } }
{ let __ice_next = next.state.to_owned(); if ::ui_lang_runtime::state_changed!(self.forge_item_state, __ice_next) { self.forge_item_state = __ice_next; self.__ice_rev[21] += 1; } }
{ let __ice_next = next.author.to_owned(); if ::ui_lang_runtime::state_changed!(self.forge_item_author, __ice_next) { self.forge_item_author = __ice_next; self.__ice_rev[22] += 1; } }
{ let __ice_next = next.branches.to_owned(); if ::ui_lang_runtime::state_changed!(self.forge_item_branches, __ice_next) { self.forge_item_branches = __ice_next; self.__ice_rev[23] += 1; } }
{ let __ice_next = next.body.to_owned(); if ::ui_lang_runtime::state_changed!(self.forge_item_body, __ice_next) { self.forge_item_body = __ice_next; self.__ice_rev[24] += 1; } }
{ let __ice_next = next.blocks.clone(); if ::ui_lang_runtime::state_changed!(self.forge_item_blocks, __ice_next) { self.forge_item_blocks = __ice_next; self.__ice_rev[25] += 1; } }
{ let __ice_next = next.channel_id.to_owned(); if ::ui_lang_runtime::state_changed!(self.forge_item_channel, __ice_next) { self.forge_item_channel = __ice_next; self.__ice_rev[35] += 1; } }
{ let __ice_next = next.source_branch.to_owned(); if ::ui_lang_runtime::state_changed!(self.forge_item_source_branch, __ice_next) { self.forge_item_source_branch = __ice_next; self.__ice_rev[32] += 1; } }
{ let __ice_next = next.source_oid.to_owned(); if ::ui_lang_runtime::state_changed!(self.forge_item_source_oid, __ice_next) { self.forge_item_source_oid = __ice_next; self.__ice_rev[33] += 1; } }
{ let __ice_next = next.target_oid.to_owned(); if ::ui_lang_runtime::state_changed!(self.forge_item_target_oid, __ice_next) { self.forge_item_target_oid = __ice_next; self.__ice_rev[34] += 1; } }
{ let __ice_next = next.merge_oid.to_owned(); if ::ui_lang_runtime::state_changed!(self.forge_item_merge_oid, __ice_next) { self.forge_item_merge_oid = __ice_next; self.__ice_rev[31] += 1; } }
{ let __ice_next = next.diff_rows.clone(); if ::ui_lang_runtime::state_changed!(self.diff_rows, __ice_next) { self.diff_rows = __ice_next; self.__ice_rev[29] += 1; } }
{ let __ice_next = next.diff_truncated; if ::ui_lang_runtime::state_changed!(self.forge_item_diff_truncated, __ice_next) { self.forge_item_diff_truncated = __ice_next; self.__ice_rev[30] += 1; } }
{ let __ice_next = next.files_changed; if ::ui_lang_runtime::state_changed!(self.forge_item_files_changed, __ice_next) { self.forge_item_files_changed = __ice_next; self.__ice_rev[26] += 1; } }
{ let __ice_next = next.additions; if ::ui_lang_runtime::state_changed!(self.forge_item_additions, __ice_next) { self.forge_item_additions = __ice_next; self.__ice_rev[27] += 1; } }
{ let __ice_next = next.deletions; if ::ui_lang_runtime::state_changed!(self.forge_item_deletions, __ice_next) { self.forge_item_deletions = __ice_next; self.__ice_rev[28] += 1; } }
{ let __ice_next = next.reviews.clone(); if ::ui_lang_runtime::state_changed!(self.forge_item_reviews, __ice_next) { self.forge_item_reviews = __ice_next; self.__ice_rev[36] += 1; } }
{ let __ice_next = next.approvals; if ::ui_lang_runtime::state_changed!(self.forge_item_approvals, __ice_next) { self.forge_item_approvals = __ice_next; self.__ice_rev[37] += 1; } }
{ let __ice_next = next.change_requests; if ::ui_lang_runtime::state_changed!(self.forge_item_change_requests, __ice_next) { self.forge_item_change_requests = __ice_next; self.__ice_rev[38] += 1; } }
::iced::Task::none()
})(),
__ForgeViewMessage::DiscussionArrived(next) => (|| {

let _ = &next;
if (next.channel_id != self.forge_item_channel) { return ::iced::Task::none(); }
{ let __ice_next = next.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[76] += 1; } }
if (!(next.error).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = next.messages.clone(); if ::ui_lang_runtime::state_changed!(self.discussion, __ice_next) { self.discussion = __ice_next; self.__ice_rev[39] += 1; } }
{ let __ice_next = next.clipped; if ::ui_lang_runtime::state_changed!(self.discussion_clipped, __ice_next) { self.discussion_clipped = __ice_next; self.__ice_rev[40] += 1; } }
{ let __ice_next = ({ crate::host::seat_roster(::std::convert::AsRef::as_ref(&(crate::host::composer_scope(::std::convert::AsRef::as_ref(&(self.connected_rpc)), ::std::convert::AsRef::as_ref(&(self.forge_item_channel))))), ::std::convert::AsRef::as_ref(&(next.members))) }); if ::ui_lang_runtime::state_changed!(self.roster_set, __ice_next) { self.roster_set = __ice_next; self.__ice_rev[42] += 1; } }
if (self.focus_seq == 0) { return ::iced::Task::none(); }
let landed = self.focus_seq;
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.focus_seq, __ice_next) { self.focus_seq = __ice_next; self.__ice_rev[43] += 1; } }
{ let __ice_next = crate::host::note_at_seq(::std::convert::AsRef::as_ref(&(next.messages)), landed); if ::ui_lang_runtime::state_changed!(self.linked_note, __ice_next) { self.linked_note = __ice_next; self.__ice_rev[41] += 1; } }
{ let __ice_next = (self.landed_tick + 1); if ::ui_lang_runtime::state_changed!(self.landed_tick, __ice_next) { self.landed_tick = __ice_next; self.__ice_rev[44] += 1; } }
return ::ui_lang_guest::widget::perform::<__ForgeViewMessage>(::ui_lang_guest::wire::WidgetCommand::ScrollToKey { target: ::std::string::String::from("ForgeView/forge/item-detail"), key: ::ui_lang_guest::wire::ListKey::from(landed).virtual_key() });
})(),
__ForgeViewMessage::TreeArrived(next) => (|| {

let _ = &next;
if ((next.repo != self.open_repo) || (next.path != self.tree_path)) { return ::iced::Task::none(); }
if ((!(self.tree_rev).is_empty()) && (next.rev != self.tree_rev)) { return ::iced::Task::none(); }
{ let __ice_next = next.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[76] += 1; } }
{ let __ice_next = crate::host::phase_of(::std::convert::AsRef::as_ref(&(next.error))); if ::ui_lang_runtime::state_changed!(self.tree_phase, __ice_next) { self.tree_phase = __ice_next; self.__ice_rev[57] += 1; } }
if (!(next.error).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = next.rev.to_owned(); if ::ui_lang_runtime::state_changed!(self.tree_rev, __ice_next) { self.tree_rev = __ice_next; self.__ice_rev[53] += 1; } }
{ let __ice_next = next.born; if ::ui_lang_runtime::state_changed!(self.tree_born, __ice_next) { self.tree_born = __ice_next; self.__ice_rev[55] += 1; } }
{ let __ice_next = next.entries.clone(); if ::ui_lang_runtime::state_changed!(self.tree_entries, __ice_next) { self.tree_entries = __ice_next; self.__ice_rev[54] += 1; } }
{ let __ice_next = next.truncated; if ::ui_lang_runtime::state_changed!(self.tree_truncated, __ice_next) { self.tree_truncated = __ice_next; self.__ice_rev[56] += 1; } }
if (self.focus_path).is_empty() { return ::iced::Task::none(); }
let path = self.focus_path.to_owned();
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.focus_path, __ice_next) { self.focus_path = __ice_next; self.__ice_rev[69] += 1; } }
return (::iced::Task::done(path.to_owned())).map(|value| __ForgeViewMessage::ForgeOpenFile(value));
})(),
__ForgeViewMessage::BlobArrived(next) => (|| {

let _ = &next;
if ((next.repo != self.open_repo) || (next.path != self.file_path)) { return ::iced::Task::none(); }
{ let __ice_next = next.note.to_owned(); if ::ui_lang_runtime::state_changed!(self.file_note, __ice_next) { self.file_note = __ice_next; self.__ice_rev[65] += 1; } }
{ let __ice_next = crate::host::phase_of(::std::convert::AsRef::as_ref(&(next.error))); if ::ui_lang_runtime::state_changed!(self.file_phase, __ice_next) { self.file_phase = __ice_next; self.__ice_rev[66] += 1; } }
if (!(next.error).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = next.text.to_owned(); if ::ui_lang_runtime::state_changed!(self.file_text, __ice_next) { self.file_text = __ice_next; self.__ice_rev[59] += 1; } }
{ let __ice_next = next.binary; if ::ui_lang_runtime::state_changed!(self.file_binary, __ice_next) { self.file_binary = __ice_next; self.__ice_rev[60] += 1; } }
{ let __ice_next = next.truncated; if ::ui_lang_runtime::state_changed!(self.file_truncated, __ice_next) { self.file_truncated = __ice_next; self.__ice_rev[61] += 1; } }
{ let __ice_next = next.picture; if ::ui_lang_runtime::state_changed!(self.file_picture, __ice_next) { self.file_picture = __ice_next; self.__ice_rev[62] += 1; } }
{ let __ice_next = next.width; if ::ui_lang_runtime::state_changed!(self.file_width, __ice_next) { self.file_width = __ice_next; self.__ice_rev[63] += 1; } }
{ let __ice_next = next.height; if ::ui_lang_runtime::state_changed!(self.file_height, __ice_next) { self.file_height = __ice_next; self.__ice_rev[64] += 1; } }
::iced::Task::none()
})(),
__ForgeViewMessage::ActDone(next) => (|| {

let _ = &next;
{ let __ice_next = next.error.to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[76] += 1; } }
return match crate::host::act_of(::std::convert::AsRef::as_ref(&(next.kind))) {
Act::Review => (|| {
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.review_busy, __ice_next) { self.review_busy = __ice_next; self.__ice_rev[49] += 1; } }
if (!(next.error).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = "comment".to_owned(); if ::ui_lang_runtime::state_changed!(self.review_verdict, __ice_next) { self.review_verdict = __ice_next; self.__ice_rev[48] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.staged_comments, __ice_next) { self.staged_comments = __ice_next; self.__ice_rev[50] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.review_draft, __ice_next) { self.review_draft = __ice_next; self.__ice_rev[71] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.comment_draft, __ice_next) { self.comment_draft = __ice_next; self.__ice_rev[72] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.comment_path, __ice_next) { self.comment_path = __ice_next; self.__ice_rev[73] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.comment_line, __ice_next) { self.comment_line = __ice_next; self.__ice_rev[74] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.comment_side, __ice_next) { self.comment_side = __ice_next; self.__ice_rev[75] += 1; } }
::iced::Task::none()
})(),
Act::Merge => (|| {
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.merge_busy, __ice_next) { self.merge_busy = __ice_next; self.__ice_rev[47] += 1; } }
{ let __ice_next = next.conflicts.clone(); if ::ui_lang_runtime::state_changed!(self.merge_conflicts, __ice_next) { self.merge_conflicts = __ice_next; self.__ice_rev[46] += 1; } }
::iced::Task::none()
})(),
};
})(),
__ForgeViewMessage::ForgeOpenRepo(name) => (|| {

let _ = &name;
if (!self.connected) { return ::iced::Task::none(); }
{ let __ice_next = name.to_owned(); if ::ui_lang_runtime::state_changed!(self.open_repo, __ice_next) { self.open_repo = __ice_next; self.__ice_rev[12] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[76] += 1; } }
{ let __ice_next = "loading".to_owned(); if ::ui_lang_runtime::state_changed!(self.repo_phase, __ice_next) { self.repo_phase = __ice_next; self.__ice_rev[13] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.branches, __ice_next) { self.branches = __ice_next; self.__ice_rev[14] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.items, __ice_next) { self.items = __ice_next; self.__ice_rev[15] += 1; } }
{ let __ice_next = "code".to_owned(); if ::ui_lang_runtime::state_changed!(self.tab, __ice_next) { self.tab = __ice_next; self.__ice_rev[16] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.tree_pick, __ice_next) { self.tree_pick = __ice_next; self.__ice_rev[51] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.forge_item_number, __ice_next) { self.forge_item_number = __ice_next; self.__ice_rev[17] += 1; } }
{ let __ice_next = "idle".to_owned(); if ::ui_lang_runtime::state_changed!(self.item_phase, __ice_next) { self.item_phase = __ice_next; self.__ice_rev[18] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.forge_item_channel, __ice_next) { self.forge_item_channel = __ice_next; self.__ice_rev[35] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.linked_note, __ice_next) { self.linked_note = __ice_next; self.__ice_rev[41] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.diff_rows, __ice_next) { self.diff_rows = __ice_next; self.__ice_rev[29] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.discussion, __ice_next) { self.discussion = __ice_next; self.__ice_rev[39] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.discussion_clipped, __ice_next) { self.discussion_clipped = __ice_next; self.__ice_rev[40] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.merge_conflicts, __ice_next) { self.merge_conflicts = __ice_next; self.__ice_rev[46] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.staged_comments, __ice_next) { self.staged_comments = __ice_next; self.__ice_rev[50] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.tree_path, __ice_next) { self.tree_path = __ice_next; self.__ice_rev[52] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.tree_rev, __ice_next) { self.tree_rev = __ice_next; self.__ice_rev[53] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.tree_entries, __ice_next) { self.tree_entries = __ice_next; self.__ice_rev[54] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.tree_born, __ice_next) { self.tree_born = __ice_next; self.__ice_rev[55] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.tree_truncated, __ice_next) { self.tree_truncated = __ice_next; self.__ice_rev[56] += 1; } }
{ let __ice_next = "loading".to_owned(); if ::ui_lang_runtime::state_changed!(self.tree_phase, __ice_next) { self.tree_phase = __ice_next; self.__ice_rev[57] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.file_path, __ice_next) { self.file_path = __ice_next; self.__ice_rev[58] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.file_text, __ice_next) { self.file_text = __ice_next; self.__ice_rev[59] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.file_note, __ice_next) { self.file_note = __ice_next; self.__ice_rev[65] += 1; } }
{ let __ice_next = "idle".to_owned(); if ::ui_lang_runtime::state_changed!(self.file_phase, __ice_next) { self.file_phase = __ice_next; self.__ice_rev[66] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.opened_dir, __ice_next) { self.opened_dir = __ice_next; self.__ice_rev[67] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.opened_rev, __ice_next) { self.opened_rev = __ice_next; self.__ice_rev[68] += 1; } }
::iced::Task::none()
})(),
__ForgeViewMessage::ForgeCloseRepo => (|| {

{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.open_repo, __ice_next) { self.open_repo = __ice_next; self.__ice_rev[12] += 1; } }
{ let __ice_next = "idle".to_owned(); if ::ui_lang_runtime::state_changed!(self.repo_phase, __ice_next) { self.repo_phase = __ice_next; self.__ice_rev[13] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.branches, __ice_next) { self.branches = __ice_next; self.__ice_rev[14] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.items, __ice_next) { self.items = __ice_next; self.__ice_rev[15] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.tree_pick, __ice_next) { self.tree_pick = __ice_next; self.__ice_rev[51] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.forge_item_number, __ice_next) { self.forge_item_number = __ice_next; self.__ice_rev[17] += 1; } }
{ let __ice_next = "idle".to_owned(); if ::ui_lang_runtime::state_changed!(self.item_phase, __ice_next) { self.item_phase = __ice_next; self.__ice_rev[18] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.forge_item_channel, __ice_next) { self.forge_item_channel = __ice_next; self.__ice_rev[35] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.linked_note, __ice_next) { self.linked_note = __ice_next; self.__ice_rev[41] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.focus_seq, __ice_next) { self.focus_seq = __ice_next; self.__ice_rev[43] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.diff_rows, __ice_next) { self.diff_rows = __ice_next; self.__ice_rev[29] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.discussion, __ice_next) { self.discussion = __ice_next; self.__ice_rev[39] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.discussion_clipped, __ice_next) { self.discussion_clipped = __ice_next; self.__ice_rev[40] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.merge_conflicts, __ice_next) { self.merge_conflicts = __ice_next; self.__ice_rev[46] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.staged_comments, __ice_next) { self.staged_comments = __ice_next; self.__ice_rev[50] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.tree_path, __ice_next) { self.tree_path = __ice_next; self.__ice_rev[52] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.tree_rev, __ice_next) { self.tree_rev = __ice_next; self.__ice_rev[53] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.tree_entries, __ice_next) { self.tree_entries = __ice_next; self.__ice_rev[54] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.tree_born, __ice_next) { self.tree_born = __ice_next; self.__ice_rev[55] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.tree_truncated, __ice_next) { self.tree_truncated = __ice_next; self.__ice_rev[56] += 1; } }
{ let __ice_next = "loading".to_owned(); if ::ui_lang_runtime::state_changed!(self.tree_phase, __ice_next) { self.tree_phase = __ice_next; self.__ice_rev[57] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.file_path, __ice_next) { self.file_path = __ice_next; self.__ice_rev[58] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.file_text, __ice_next) { self.file_text = __ice_next; self.__ice_rev[59] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.file_note, __ice_next) { self.file_note = __ice_next; self.__ice_rev[65] += 1; } }
{ let __ice_next = "idle".to_owned(); if ::ui_lang_runtime::state_changed!(self.file_phase, __ice_next) { self.file_phase = __ice_next; self.__ice_rev[66] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.opened_dir, __ice_next) { self.opened_dir = __ice_next; self.__ice_rev[67] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.opened_rev, __ice_next) { self.opened_rev = __ice_next; self.__ice_rev[68] += 1; } }
::iced::Task::none()
})(),
__ForgeViewMessage::ForgePickBranch(name) => (|| {

let _ = &name;
if ((!self.connected) || (self.open_repo).is_empty()) { return ::iced::Task::none(); }
let head = crate::host::forge_branch_head(::std::convert::AsRef::as_ref(&(self.branches)), ::std::convert::AsRef::as_ref(&(name)));
if (head).is_empty() { return ::iced::Task::none(); }
{ let __ice_next = name.to_owned(); if ::ui_lang_runtime::state_changed!(self.tree_pick, __ice_next) { self.tree_pick = __ice_next; self.__ice_rev[51] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.tree_path, __ice_next) { self.tree_path = __ice_next; self.__ice_rev[52] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.tree_entries, __ice_next) { self.tree_entries = __ice_next; self.__ice_rev[54] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.tree_truncated, __ice_next) { self.tree_truncated = __ice_next; self.__ice_rev[56] += 1; } }
{ let __ice_next = "loading".to_owned(); if ::ui_lang_runtime::state_changed!(self.tree_phase, __ice_next) { self.tree_phase = __ice_next; self.__ice_rev[57] += 1; } }
{ let __ice_next = head.to_owned(); if ::ui_lang_runtime::state_changed!(self.tree_rev, __ice_next) { self.tree_rev = __ice_next; self.__ice_rev[53] += 1; } }
::iced::Task::none()
})(),
__ForgeViewMessage::ForgeOpenDir(path) => (|| {

let _ = &path;
if ((!self.connected) || (self.open_repo).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = path.to_owned(); if ::ui_lang_runtime::state_changed!(self.tree_path, __ice_next) { self.tree_path = __ice_next; self.__ice_rev[52] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.tree_entries, __ice_next) { self.tree_entries = __ice_next; self.__ice_rev[54] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.tree_truncated, __ice_next) { self.tree_truncated = __ice_next; self.__ice_rev[56] += 1; } }
{ let __ice_next = "loading".to_owned(); if ::ui_lang_runtime::state_changed!(self.tree_phase, __ice_next) { self.tree_phase = __ice_next; self.__ice_rev[57] += 1; } }
::iced::Task::none()
})(),
__ForgeViewMessage::ForgeOpenFile(path) => (|| {

let _ = &path;
if ((!self.connected) || (self.open_repo).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = self.tree_path.to_owned(); if ::ui_lang_runtime::state_changed!(self.opened_dir, __ice_next) { self.opened_dir = __ice_next; self.__ice_rev[67] += 1; } }
{ let __ice_next = self.tree_rev.to_owned(); if ::ui_lang_runtime::state_changed!(self.opened_rev, __ice_next) { self.opened_rev = __ice_next; self.__ice_rev[68] += 1; } }
{ let __ice_next = path.to_owned(); if ::ui_lang_runtime::state_changed!(self.file_path, __ice_next) { self.file_path = __ice_next; self.__ice_rev[58] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.file_text, __ice_next) { self.file_text = __ice_next; self.__ice_rev[59] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.file_binary, __ice_next) { self.file_binary = __ice_next; self.__ice_rev[60] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.file_truncated, __ice_next) { self.file_truncated = __ice_next; self.__ice_rev[61] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.file_picture, __ice_next) { self.file_picture = __ice_next; self.__ice_rev[62] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.file_note, __ice_next) { self.file_note = __ice_next; self.__ice_rev[65] += 1; } }
{ let __ice_next = "loading".to_owned(); if ::ui_lang_runtime::state_changed!(self.file_phase, __ice_next) { self.file_phase = __ice_next; self.__ice_rev[66] += 1; } }
::iced::Task::none()
})(),
__ForgeViewMessage::ForgeOpenItem(number) => (|| {

let _ = &number;
if ((!self.connected) || (self.open_repo).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = number; if ::ui_lang_runtime::state_changed!(self.forge_item_number, __ice_next) { self.forge_item_number = __ice_next; self.__ice_rev[17] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.linked_note, __ice_next) { self.linked_note = __ice_next; self.__ice_rev[41] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.host_error, __ice_next) { self.host_error = __ice_next; self.__ice_rev[76] += 1; } }
{ let __ice_next = "loading".to_owned(); if ::ui_lang_runtime::state_changed!(self.item_phase, __ice_next) { self.item_phase = __ice_next; self.__ice_rev[18] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.forge_item_channel, __ice_next) { self.forge_item_channel = __ice_next; self.__ice_rev[35] += 1; } }
{ let __ice_next = "comment".to_owned(); if ::ui_lang_runtime::state_changed!(self.review_verdict, __ice_next) { self.review_verdict = __ice_next; self.__ice_rev[48] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.staged_comments, __ice_next) { self.staged_comments = __ice_next; self.__ice_rev[50] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.review_draft, __ice_next) { self.review_draft = __ice_next; self.__ice_rev[71] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.comment_draft, __ice_next) { self.comment_draft = __ice_next; self.__ice_rev[72] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.comment_path, __ice_next) { self.comment_path = __ice_next; self.__ice_rev[73] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.comment_line, __ice_next) { self.comment_line = __ice_next; self.__ice_rev[74] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.comment_side, __ice_next) { self.comment_side = __ice_next; self.__ice_rev[75] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.merge_conflicts, __ice_next) { self.merge_conflicts = __ice_next; self.__ice_rev[46] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.diff_rows, __ice_next) { self.diff_rows = __ice_next; self.__ice_rev[29] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.discussion, __ice_next) { self.discussion = __ice_next; self.__ice_rev[39] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.discussion_clipped, __ice_next) { self.discussion_clipped = __ice_next; self.__ice_rev[40] += 1; } }
::iced::Task::none()
})(),
__ForgeViewMessage::ForgeCloseItem => (|| {

{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.forge_item_number, __ice_next) { self.forge_item_number = __ice_next; self.__ice_rev[17] += 1; } }
{ let __ice_next = "idle".to_owned(); if ::ui_lang_runtime::state_changed!(self.item_phase, __ice_next) { self.item_phase = __ice_next; self.__ice_rev[18] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.forge_item_channel, __ice_next) { self.forge_item_channel = __ice_next; self.__ice_rev[35] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.linked_note, __ice_next) { self.linked_note = __ice_next; self.__ice_rev[41] += 1; } }
{ let __ice_next = 0; if ::ui_lang_runtime::state_changed!(self.focus_seq, __ice_next) { self.focus_seq = __ice_next; self.__ice_rev[43] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.diff_rows, __ice_next) { self.diff_rows = __ice_next; self.__ice_rev[29] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.discussion, __ice_next) { self.discussion = __ice_next; self.__ice_rev[39] += 1; } }
{ let __ice_next = false; if ::ui_lang_runtime::state_changed!(self.discussion_clipped, __ice_next) { self.discussion_clipped = __ice_next; self.__ice_rev[40] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.merge_conflicts, __ice_next) { self.merge_conflicts = __ice_next; self.__ice_rev[46] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.staged_comments, __ice_next) { self.staged_comments = __ice_next; self.__ice_rev[50] += 1; } }
::iced::Task::none()
})(),
__ForgeViewMessage::SelectForgeTab(next) => (|| {

let _ = &next;
{ let __ice_next = next.to_owned(); if ::ui_lang_runtime::state_changed!(self.tab, __ice_next) { self.tab = __ice_next; self.__ice_rev[16] += 1; } }
if (self.forge_item_number <= 0) { return ::iced::Task::none(); }
return (::iced::Task::done(true)).map(|value| { let _ = &value; __ForgeViewMessage::ForgeCloseItem });
})(),
__ForgeViewMessage::ForgeReviewPick(verdict) => (|| {

let _ = &verdict;
{ let __ice_next = verdict.to_owned(); if ::ui_lang_runtime::state_changed!(self.review_verdict, __ice_next) { self.review_verdict = __ice_next; self.__ice_rev[48] += 1; } }
::iced::Task::none()
})(),
__ForgeViewMessage::ForgeReviewSubmit(body) => (|| {

let _ = &body;
if ((self.review_busy || (!self.connected)) || (self.forge_item_source_oid).is_empty()) { return ::iced::Task::none(); }
if ((body).is_empty() && (self.staged_comments).is_empty()) { return ::iced::Task::none(); }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.review_busy, __ice_next) { self.review_busy = __ice_next; self.__ice_rev[49] += 1; } }
{ let __ice_next = ({ crate::host::review_submit(self.open_repo.to_owned(), self.forge_item_number, self.review_verdict.to_owned(), body.to_owned(), self.forge_item_source_oid.to_owned(), self.staged_comments.clone()) }); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[77] += 1; } }
::iced::Task::none()
})(),
__ForgeViewMessage::ForgeMergeSubmit => (|| {

if ((((!self.connected) || self.merge_busy) || (self.open_repo).is_empty()) || (self.forge_item_number <= 0)) { return ::iced::Task::none(); }
{ let __ice_next = true; if ::ui_lang_runtime::state_changed!(self.merge_busy, __ice_next) { self.merge_busy = __ice_next; self.__ice_rev[47] += 1; } }
{ let __ice_next = ::std::vec::Vec::new(); if ::ui_lang_runtime::state_changed!(self.merge_conflicts, __ice_next) { self.merge_conflicts = __ice_next; self.__ice_rev[46] += 1; } }
{ let __ice_next = ({ crate::host::merge(self.open_repo.to_owned(), self.forge_item_number, self.forge_item_source_branch.to_owned(), self.forge_item_source_oid.to_owned(), self.forge_item_target_oid.to_owned()) }); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[77] += 1; } }
::iced::Task::none()
})(),
__ForgeViewMessage::ForgeCommentOpen(path, line, side) => (|| {

let _ = &path;
let _ = &line;
let _ = &side;
if (path).is_empty() { return ::iced::Task::none(); }
{ let __ice_next = path.to_owned(); if ::ui_lang_runtime::state_changed!(self.comment_path, __ice_next) { self.comment_path = __ice_next; self.__ice_rev[73] += 1; } }
{ let __ice_next = line.to_owned(); if ::ui_lang_runtime::state_changed!(self.comment_line, __ice_next) { self.comment_line = __ice_next; self.__ice_rev[74] += 1; } }
{ let __ice_next = side.to_owned(); if ::ui_lang_runtime::state_changed!(self.comment_side, __ice_next) { self.comment_side = __ice_next; self.__ice_rev[75] += 1; } }
::iced::Task::none()
})(),
__ForgeViewMessage::ForgeCommentCancel => (|| {

{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.comment_path, __ice_next) { self.comment_path = __ice_next; self.__ice_rev[73] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.comment_line, __ice_next) { self.comment_line = __ice_next; self.__ice_rev[74] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.comment_side, __ice_next) { self.comment_side = __ice_next; self.__ice_rev[75] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.comment_draft, __ice_next) { self.comment_draft = __ice_next; self.__ice_rev[72] += 1; } }
::iced::Task::none()
})(),
__ForgeViewMessage::ForgeCommentStage(body) => (|| {

let _ = &body;
if (((self.comment_path).is_empty() || (body).is_empty()) || crate::host::forge_comment_cap_reached(::std::convert::AsRef::as_ref(&(self.staged_comments)))) { return ::iced::Task::none(); }
self.staged_comments = crate::host::stage_forge_comment(::std::mem::take(&mut self.staged_comments), self.comment_path.to_owned(), self.comment_line.to_owned(), self.comment_side.to_owned(), body.to_owned()); self.__ice_rev[50] += 1;
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.comment_path, __ice_next) { self.comment_path = __ice_next; self.__ice_rev[73] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.comment_line, __ice_next) { self.comment_line = __ice_next; self.__ice_rev[74] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.comment_side, __ice_next) { self.comment_side = __ice_next; self.__ice_rev[75] += 1; } }
{ let __ice_next = "".to_owned(); if ::ui_lang_runtime::state_changed!(self.comment_draft, __ice_next) { self.comment_draft = __ice_next; self.__ice_rev[72] += 1; } }
::iced::Task::none()
})(),
__ForgeViewMessage::ForgeCommentDrop(anchor) => (|| {

let _ = &anchor;
self.staged_comments = crate::host::drop_forge_comment(::std::mem::take(&mut self.staged_comments), ::std::convert::AsRef::as_ref(&(anchor))); self.__ice_rev[50] += 1;
::iced::Task::none()
})(),
__ForgeViewMessage::OpenMessageLink(url) => (|| {

let _ = &url;
{ let __ice_next = ({ crate::host::open_link(::std::convert::AsRef::as_ref(&(url))) }); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[77] += 1; } }
::iced::Task::none()
})(),
__ForgeViewMessage::CopyToClipboard(text, label) => (|| {

let _ = &text;
let _ = &label;
{ let __ice_next = ({ crate::host::copy(::std::convert::AsRef::as_ref(&(text)), ::std::convert::AsRef::as_ref(&(label))) }); if ::ui_lang_runtime::state_changed!(self.sent, __ice_next) { self.sent = __ice_next; self.__ice_rev[77] += 1; } }
::iced::Task::none()
})(),
__ForgeViewMessage::TreeResized(dx, _dy) => (|| {

let _ = &dx;
let _ = &_dy;
{ let __ice_next = crate::host::tree_width_after_delta(self.tree_width, dx, self.viewport_width); if ::ui_lang_runtime::state_changed!(self.tree_width, __ice_next) { self.tree_width = __ice_next; self.__ice_rev[79] += 1; } }
::iced::Task::none()
})(),
__ForgeViewMessage::ViewportChanged(width, _height) => (|| {

let _ = &width;
let _ = &_height;
{ let __ice_next = width; if ::ui_lang_runtime::state_changed!(self.viewport_width, __ice_next) { self.viewport_width = __ice_next; self.__ice_rev[78] += 1; } }
{ let __ice_next = crate::host::tree_width_after_delta(self.tree_width, 0.0, width); if ::ui_lang_runtime::state_changed!(self.tree_width, __ice_next) { self.tree_width = __ice_next; self.__ice_rev[79] += 1; } }
::iced::Task::none()
})(),
__ForgeViewMessage::__BindCommentDraft(value) => { { let __ice_next = value; if ::ui_lang_runtime::state_changed!(self.comment_draft, __ice_next) { self.comment_draft = __ice_next; self.__ice_rev[72] += 1; } } ::iced::Task::none() }
__ForgeViewMessage::__BindReviewDraft(value) => { { let __ice_next = value; if ::ui_lang_runtime::state_changed!(self.review_draft, __ice_next) { self.review_draft = __ice_next; self.__ice_rev[71] += 1; } } ::iced::Task::none() }
__ForgeViewMessage::__ExternNoop => ::iced::Task::none(),
}
}

}
}
