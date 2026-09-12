macro_rules! __ice_generated_items_506167657356696577 { ($($item:item)*) => { $(#[allow(warnings, clippy::all)] $item)* }; }
__ice_generated_items_506167657356696577! {
type __IceElement<'a, Message, Theme = ()> = <(&'a (), Message, Theme) as ::ducktape_view_guest::wire::Erase>::Node;
pub(crate) type __IceMessage = __PagesViewMessage;
type __IceKeyRelease = ::ducktape_view_guest::wire::keyboard::KeyState;
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum AppTheme {
App,
AppDark,
}
#[derive(Clone, Copy)]
struct __IcePalette { name: &'static str, colors: [::ducktape_view_guest::wire::Rgba; 128] }
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum CommentsMode {
Beside,
Squeeze,
Inline,
}
#[allow(dead_code)]
pub struct PagesView {
pub(crate) pointer_y: f64,
pub(crate) comment_anchor_y: f64,
pub(crate) comment_anchor_line: i64,
pub(crate) comments_card_height: f64,
pub(crate) document_reserve: crate::editor_view::EditorReserve,
pub(crate) document_focused: bool,
pub(crate) focus_query: i64,
pub(crate) document_paint: crate::editor_view::PreparedPresentation,
pub(crate) document: ::ducktape_view_guest::Editor,
pub(crate) document_history: crate::editor_binding::HistoryState,
pub(crate) document_menu: crate::editor_binding::MenuState,
pub(crate) document_error: ::std::string::String,
pub(crate) document_dark: bool,
pub(crate) document_commented: ::std::vec::Vec<i64>,
pub(crate) document_marks: ::std::vec::Vec<crate::document_sync::CommentMark>,
pub(crate) active_palette: AppTheme,
pub(crate) connected: bool,
pub(crate) chain: ::std::string::String,
pub(crate) route_serial: i64,
pub(crate) register_serial: i64,
pub(crate) loading: bool,
pub(crate) busy: bool,
pub(crate) host_error: ::std::string::String,
pub(crate) page_link: ::std::string::String,
pub(crate) pages: ::std::vec::Vec<crate::host::PageItem>,
pub(crate) blocks: ::std::vec::Vec<crate::document_sync::PageBlock>,
pub(crate) pages_viewport_width: f64,
pub(crate) pages_viewport_height: f64,
pub(crate) pages_pane_width: f64,
pub(crate) sidebar_width: f64,
pub(crate) page_menu_open: bool,
pub(crate) page_create_open: bool,
pub(crate) active_page: ::std::string::String,
pub(crate) active_page_title: ::std::string::String,
pub(crate) active_page_parent: ::std::string::String,
pub(crate) page_searching: bool,
pub(crate) page_search_hits: ::std::vec::Vec<crate::host::PageSearchHit>,
pub(crate) page_search_query: ::std::string::String,
pub(crate) page_search_serial: i64,
pub(crate) page_delete_armed: bool,
pub(crate) autosave: ::std::string::String,
pub(crate) page_refusal: ::std::string::String,
pub(crate) subpages: ::std::vec::Vec<crate::host::Subpage>,
pub(crate) orphaned_comment_drafts: ::std::vec::Vec<::std::string::String>,
pub(crate) block_comments_open: bool,
pub(crate) scope_target: ::std::string::String,
pub(crate) scope_pinned: bool,
pub(crate) thread_total: i64,
pub(crate) comment_rows: ::std::vec::Vec<crate::host::PageCommentThreadRow>,
pub(crate) threads_loading: bool,
pub(crate) commented_hits: ::std::vec::Vec<::std::string::String>,
pub(crate) reply_thread: ::std::string::String,
pub(crate) expanded_threads: ::std::vec::Vec<::std::string::String>,
pub(crate) resolved_open: bool,
pub(crate) page_draft: ::std::string::String,
pub(crate) page_search_draft: ::std::string::String,
pub(crate) block_comment_draft: ::std::string::String,
pub(crate) reply_draft: ::std::string::String,
pub(crate) pending_page: ::std::string::String,
pub(crate) pending_comment: ::std::string::String,
pub(crate) page_saved_text: ::std::string::String,
pub(crate) buffer_page: ::std::string::String,
pub(crate) page_inflight_text: ::std::string::String,
pub(crate) sent: bool,
pub(crate) __ice_rev: [u64; 64],
}
impl ::std::fmt::Debug for PagesView { fn fmt(&self, __formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result { __formatter.write_str("PagesView") } }
#[derive(Clone)]
pub(crate) enum __PagesViewMessage {
SessionArrived(crate::host::SessionItem),
CommentPointerMoved(f64, f64),
ChoosePage(::std::string::String),
RegisterArrived(crate::host::RegisterItem),
SearchArrived(crate::host::SearchItem),
ActDone(crate::host::ActItem),
SaveDone(crate::host::SaveItem),
PageAutosaveTick,
TogglePageCreate,
CreatePageSubmit,
ArmPageDelete,
DisarmPageDelete,
DeletePageSubmit,
SearchPagesSubmit,
ClearPageSearch,
OpenPageSearchHit(::std::string::String, ::std::string::String),
UseOrphanedCommentDraft(::std::string::String),
DiscardOrphanedCommentDraft(::std::string::String),
ToggleBlockComments,
CloseBlockComments,
NarrowCommentScope(::std::string::String),
WidenCommentScope,
ResolveThreadSubmit(::std::string::String, bool),
SelectReplyThread(::std::string::String),
ToggleThreadReplies(::std::string::String),
ToggleResolvedComments,
PostThreadReply(::std::string::String),
PostBlockCommentSubmit,
CopyToClipboard(::std::string::String, ::std::string::String),
DocumentPointerReleased(::ducktape_view_guest::wire::mouse::Button),
DocumentKeyReleased(__IceKeyRelease),
DocumentWindowFocused,
DocumentWindowUnfocused,
DocumentFocusChecked(i64, bool),
SidebarResized(f64, f64),
PagesViewportChanged(f64, f64),
PagesPaneResized(f64, f64),
CommentsCardMeasured(f64, f64),
TogglePageMenu,
ClosePageMenu,
DocumentCommitted(crate::editor_binding::EditorUpdate),
__BindPageDraft(::std::string::String),
__BindPageSearchDraft(::std::string::String),
__BindReplyDraft(::std::string::String),
__BindBlockCommentDraft(::std::string::String),
__EditDocument(::ducktape_view_guest::EditorDocumentUpdate),
__0T646f63756d656e74(::ducktape_view_guest::EditorTransaction<__PagesViewMessage>),
__ExternNoop,
}
impl ::std::fmt::Debug for __PagesViewMessage { fn fmt(&self, __formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result { __formatter.write_str("__PagesViewMessage") } }
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_PageItem(_value: &crate::host::PageItem) {
let _: &::std::string::String = &_value.id;
let _: &::std::string::String = &_value.title;
let _: &::std::string::String = &_value.parent;
let _: &::std::string::String = &_value.prefix;
let _: &i64 = &_value.child_count;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_Subpage(_value: &crate::host::Subpage) {
let _: &::std::string::String = &_value.id;
let _: &::std::string::String = &_value.title;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_PageSearchHit(_value: &crate::host::PageSearchHit) {
let _: &::std::string::String = &_value.page_id;
let _: &::std::string::String = &_value.page_title;
let _: &::std::string::String = &_value.block_id;
let _: &::std::string::String = &_value.kind;
let _: &::std::string::String = &_value.text;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_PageComment(_value: &crate::host::PageComment) {
let _: &::std::string::String = &_value.id;
let _: &i64 = &_value.ordinal;
let _: &::std::string::String = &_value.author;
let _: &::std::string::String = &_value.meta;
let _: &::std::string::String = &_value.text;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_PageCommentThread(_value: &crate::host::PageCommentThread) {
let _: &::std::string::String = &_value.id;
let _: &::std::string::String = &_value.target;
let _: &::std::string::String = &_value.author;
let _: &::std::string::String = &_value.meta;
let _: &bool = &_value.resolved;
let _: &i64 = &_value.comment_count;
let _: &::std::vec::Vec<crate::host::PageComment> = &_value.comments;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_PageCommentThreadRow(_value: &crate::host::PageCommentThreadRow) {
let _: &crate::host::PageCommentThread = &_value.thread;
let _: &::std::string::String = &_value.anchor;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_PageCommentGroup(_value: &crate::host::PageCommentGroup) {
let _: &::std::string::String = &_value.target;
let _: &::std::string::String = &_value.anchor;
let _: &::std::vec::Vec<crate::host::PageCommentThread> = &_value.threads;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_Session(_value: &crate::host::Session) {
let _: &bool = &_value.connected;
let _: &bool = &_value.dark;
let _: &::std::string::String = &_value.chain;
let _: &::std::string::String = &_value.route_page;
let _: &i64 = &_value.route_serial;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_SessionItem(_value: &crate::host::SessionItem) {
let _: &crate::host::Session = &_value.next;
let _: &::std::string::String = &_value.error;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_RegisterItem(_value: &crate::host::RegisterItem) {
let _: &::std::vec::Vec<crate::host::PageItem> = &_value.pages;
let _: &::std::string::String = &_value.active_page;
let _: &::std::string::String = &_value.active_page_title;
let _: &::std::string::String = &_value.active_page_parent;
let _: &::std::vec::Vec<crate::document_sync::PageBlock> = &_value.blocks;
let _: &::std::vec::Vec<crate::host::Subpage> = &_value.subpages;
let _: &::std::string::String = &_value.document;
let _: &::std::vec::Vec<crate::host::PageCommentThreadRow> = &_value.comment_rows;
let _: &i64 = &_value.thread_total;
let _: &::std::vec::Vec<::std::string::String> = &_value.commented_hits;
let _: &::std::string::String = &_value.error;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_SearchItem(_value: &crate::host::SearchItem) {
let _: &::std::string::String = &_value.query;
let _: &::std::vec::Vec<crate::host::PageSearchHit> = &_value.hits;
let _: &::std::string::String = &_value.error;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_ActItem(_value: &crate::host::ActItem) {
let _: &::std::string::String = &_value.page;
let _: &::std::string::String = &_value.error;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_SaveItem(_value: &crate::host::SaveItem) {
let _: &bool = &_value.written;
let _: &::std::string::String = &_value.refusal;
let _: &::std::string::String = &_value.document;
let _: &::std::string::String = &_value.error;
}
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
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_PageBlock(_value: &crate::document_sync::PageBlock) {
let _: &i64 = &_value.key;
let _: &::std::string::String = &_value.id;
let _: &::std::string::String = &_value.parent;
let _: &::std::string::String = &_value.kind;
let _: &::std::string::String = &_value.text;
let _: &bool = &_value.checked;
let _: &::std::string::String = &_value.prefix;
let _: &i64 = &_value.child_count;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_CommentMark(_value: &crate::document_sync::CommentMark) {
let _: &i64 = &_value.line;
let _: &i64 = &_value.count;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_PreparedPresentation(_value: &crate::editor_view::PreparedPresentation) {
let _: &::std::vec::Vec<u8> = &_value.reference;
let _: &::std::vec::Vec<u8> = &_value.data;
let _: &::std::string::String = &_value.notice;
}
#[allow(dead_code, non_snake_case)] fn __ui_lang_check_EditorReserve(_value: &crate::editor_view::EditorReserve) {
let _: &i64 = &_value.line;
let _: &i64 = &_value.height;
}
#[allow(dead_code)] fn __ui_lang_check_subscription_session() { let _: ::ducktape_view_guest::Subscription<crate::host::SessionItem> = crate::host::session(); }
#[allow(dead_code)] fn __ui_lang_check_subscription_register(arg0: ::std::string::String, arg1: i64) { let _: ::ducktape_view_guest::Subscription<crate::host::RegisterItem> = crate::host::register(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_subscription_search(arg0: ::std::string::String, arg1: i64) { let _: ::ducktape_view_guest::Subscription<crate::host::SearchItem> = crate::host::search(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_subscription_acts() { let _: ::ducktape_view_guest::Subscription<crate::host::ActItem> = crate::host::acts(); }
#[allow(dead_code)] fn __ui_lang_check_subscription_saves() { let _: ::ducktape_view_guest::Subscription<crate::host::SaveItem> = crate::host::saves(); }
#[allow(dead_code)] fn __ui_lang_check_pure_connection_serial_after(arg0: bool, arg1: bool, arg2: i64) { let _: i64 = crate::host::connection_serial_after(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_route_arrived(arg0: i64, arg1: i64) { let _: bool = crate::host::route_arrived(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_sync_create<'a>(arg0: &'a str) { let _: bool = crate::host::create(arg0); }
#[allow(dead_code)] fn __ui_lang_check_sync_delete<'a>(arg0: &'a str) { let _: bool = crate::host::delete(arg0); }
#[allow(dead_code)] fn __ui_lang_check_sync_post<'a>(arg0: &'a str, arg1: &'a str, arg2: &'a str) { let _: bool = crate::host::post(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_sync_resolve<'a>(arg0: &'a str, arg1: bool) { let _: bool = crate::host::resolve(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_sync_save<'a>(arg0: &'a str, arg1: &'a str, arg2: &'a str) { let _: bool = crate::host::save(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_sync_copy<'a>(arg0: &'a str, arg1: &'a str) { let _: bool = crate::host::copy(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_sync_open_link<'a>(arg0: &'a str) { let _: bool = crate::host::open_link(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_icon<'a>(arg0: &'a str) { let _: ::std::vec::Vec<u8> = crate::host::icon(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_count_label(arg0: i64) { let _: ::std::string::String = crate::host::count_label(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_keep_str<'a>(arg0: bool, arg1: &'a str, arg2: &'a str) { let _: ::std::string::String = crate::host::keep_str(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_keep_i64(arg0: bool, arg1: i64, arg2: i64) { let _: i64 = crate::host::keep_i64(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_sidebar_width_after_delta(arg0: f64, arg1: f64, arg2: f64) { let _: f64 = crate::host::sidebar_width_after_delta(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_search_answer_stands<'a>(arg0: &'a str, arg1: &'a str, arg2: bool) { let _: bool = crate::host::search_answer_stands(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_initials_of<'a>(arg0: &'a str) { let _: ::std::string::String = crate::host::initials_of(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_page_address<'a>(arg0: &'a str, arg1: &'a str) { let _: ::std::string::String = crate::host::page_address(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_page_display_title<'a>(arg0: &'a [crate::host::PageItem], arg1: &'a str, arg2: &'a str) { let _: ::std::string::String = crate::host::page_display_title(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_compose_hint_of<'a>(arg0: &'a [crate::document_sync::PageBlock], arg1: &'a str, arg2: &'a str) { let _: ::std::string::String = crate::host::compose_hint_of(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_comment_marks<'a>(arg0: &'a [crate::document_sync::PageBlock], arg1: &'a [::std::string::String]) { let _: ::std::vec::Vec<crate::document_sync::CommentMark> = crate::host::comment_marks(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_commented_lines<'a>(arg0: &'a [crate::document_sync::PageBlock], arg1: &'a [::std::string::String]) { let _: ::std::vec::Vec<i64> = crate::host::commented_lines(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_block_at_line<'a>(arg0: &'a [crate::document_sync::PageBlock], arg1: i64) { let _: ::std::string::String = crate::host::block_at_line(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_document_text<'a>(arg0: &'a ::ducktape_view_guest::Editor) { let _: ::std::string::String = crate::host::document_text(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_document_editor<'a>(arg0: &'a str) { let _: ::ducktape_view_guest::Editor = crate::host::document_editor(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_install_decision<'a>(arg0: &'a str, arg1: &'a str, arg2: &'a str, arg3: &'a str, arg4: &'a str) { let _: bool = crate::host::install_decision(arg0, arg1, arg2, arg3, arg4); }
#[allow(dead_code)] fn __ui_lang_check_pure_has_unclosed_fence<'a>(arg0: &'a str) { let _: bool = crate::host::has_unclosed_fence(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_saved_baseline<'a>(arg0: bool, arg1: &'a str, arg2: &'a str) { let _: ::std::string::String = crate::host::saved_baseline(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_baseline_at_submitted_title<'a>(arg0: &'a str, arg1: &'a str) { let _: ::std::string::String = crate::host::baseline_at_submitted_title(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_remember_draft<'a>(arg0: &'a [::std::string::String], arg1: &'a str) { let _: ::std::vec::Vec<::std::string::String> = crate::host::remember_draft(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_forget_draft<'a>(arg0: &'a [::std::string::String], arg1: &'a str) { let _: ::std::vec::Vec<::std::string::String> = crate::host::forget_draft(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_navigation_link(arg0: ::std::vec::Vec<u8>) { let _: ::std::string::String = crate::host::navigation_link(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_navigation_comment_line(arg0: ::std::vec::Vec<u8>) { let _: i64 = crate::host::navigation_comment_line(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_comment_card_offset(arg0: f64, arg1: f64, arg2: f64) { let _: f64 = crate::host::comment_card_offset(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_comment_card_height(arg0: f64, arg1: f64) { let _: f64 = crate::host::comment_card_height(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_comments_mode(arg0: f64) { let _: CommentsMode = crate::host::comments_mode(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_document_width(arg0: f64, arg1: bool) { let _: f64 = crate::host::document_width(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_comments_card_width(arg0: f64) { let _: f64 = crate::host::comments_card_width(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_comments_right_anchor(arg0: f64) { let _: f64 = crate::host::comments_right_anchor(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_comments_left_inset(arg0: f64) { let _: f64 = crate::host::comments_left_inset(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_comments_reserve(arg0: f64, arg1: bool, arg2: i64, arg3: f64) { let _: crate::editor_view::EditorReserve = crate::host::comments_reserve(arg0, arg1, arg2, arg3); }
#[allow(dead_code)] fn __ui_lang_check_pure_measured_card_height(arg0: f64, arg1: f64) { let _: f64 = crate::host::measured_card_height(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_comment_scope_label<'a>(arg0: &'a [crate::document_sync::PageBlock], arg1: &'a str, arg2: &'a str, arg3: i64) { let _: ::std::string::String = crate::host::comment_scope_label(arg0, arg1, arg2, arg3); }
#[allow(dead_code)] fn __ui_lang_check_pure_comment_post_target<'a>(arg0: &'a [crate::host::PageCommentThreadRow], arg1: &'a str, arg2: &'a str) { let _: ::std::string::String = crate::host::comment_post_target(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_scope_groups<'a>(arg0: ::std::vec::Vec<crate::host::PageCommentThreadRow>, arg1: &'a str, arg2: &'a str) { let _: ::std::vec::Vec<crate::host::PageCommentGroup> = crate::host::scope_groups(arg0, arg1, arg2); }
#[allow(dead_code)] fn __ui_lang_check_pure_scope_resolved<'a>(arg0: ::std::vec::Vec<crate::host::PageCommentThreadRow>, arg1: &'a str) { let _: ::std::vec::Vec<crate::host::PageCommentThreadRow> = crate::host::scope_resolved(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_kept_ids(arg0: bool, arg1: ::std::vec::Vec<::std::string::String>) { let _: ::std::vec::Vec<::std::string::String> = crate::host::kept_ids(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_resolved_label<'a>(arg0: &'a [crate::host::PageCommentThreadRow]) { let _: ::std::string::String = crate::host::resolved_label(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_empty_scope_label<'a>(arg0: &'a str) { let _: ::std::string::String = crate::host::empty_scope_label(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_opener_text<'a>(arg0: &'a crate::host::PageCommentThread) { let _: ::std::string::String = crate::host::opener_text(arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_thread_replies<'a>(arg0: &'a crate::host::PageCommentThread, arg1: bool) { let _: ::std::vec::Vec<crate::host::PageComment> = crate::host::thread_replies(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_reply_toggle_label<'a>(arg0: &'a crate::host::PageCommentThread, arg1: bool) { let _: ::std::string::String = crate::host::reply_toggle_label(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_reply_thread_after_press<'a>(arg0: &'a str, arg1: &'a str) { let _: ::std::string::String = crate::host::reply_thread_after_press(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_expanded<'a>(arg0: &'a [::std::string::String], arg1: &'a str) { let _: bool = crate::host::expanded(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_toggled<'a>(arg0: ::std::vec::Vec<::std::string::String>, arg1: &'a str) { let _: ::std::vec::Vec<::std::string::String> = crate::host::toggled(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_initial_history() { let _: crate::editor_binding::HistoryState = crate::editor_binding::initial_history(); }
#[allow(dead_code)] fn __ui_lang_check_pure_initial_menu() { let _: crate::editor_binding::MenuState = crate::editor_binding::initial_menu(); }
#[allow(dead_code)] fn __ui_lang_check_editor_binding_keys() { let _: fn(crate::editor_binding::HistoryState, crate::editor_binding::MenuState) -> ::ducktape_view_guest::EditorBinding<crate::editor_binding::EditorUpdate> = crate::editor_binding::keys; }
#[allow(dead_code)] fn __ui_lang_check_pure_presentation_notice<'a>(arg0: &'a ::ducktape_view_guest::Editor, arg1: &'a crate::editor_view::PreparedPresentation) { let _: ::std::string::String = crate::editor_view::presentation_notice(arg0, arg1); }
#[allow(dead_code)] fn __ui_lang_check_pure_empty_presentation() { let _: crate::editor_view::PreparedPresentation = crate::editor_view::empty_presentation(); }
#[allow(dead_code)] fn __ui_lang_check_pure_no_reserve() { let _: crate::editor_view::EditorReserve = crate::editor_view::no_reserve(); }
#[allow(dead_code)] fn __ui_lang_check_editor_highlighter_paint(arg0: crate::editor_view::PreparedPresentation) { let __editor = ::ducktape_view_guest::Editor::default(); let _: ::ducktape_view_guest::wire::editor_presentation::EditorPresentation = crate::editor_view::paint(__editor.state_view(), arg0); }
#[allow(dead_code)] fn __ui_lang_check_pure_document_presentation<'a>(arg0: &'a ::ducktape_view_guest::Editor, arg1: crate::editor_binding::MenuState, arg2: bool, arg3: ::std::vec::Vec<i64>, arg4: ::std::vec::Vec<crate::document_sync::CommentMark>, arg5: bool, arg6: crate::editor_view::EditorReserve) { let _: crate::editor_view::PreparedPresentation = crate::editor_view::document_presentation(arg0, arg1, arg2, arg3, arg4, arg5, arg6); }
}
__ice_generated_items_506167657356696577! {
#[allow(unused_parens)]
impl PagesView {
#[must_use]
}
}
__ice_generated_items_506167657356696577! {
#[allow(unused_parens)]
impl PagesView {
fn __palette(&self) -> __IcePalette {
match self.active_palette.clone() {
AppTheme::App => __IcePalette { name: "app", colors: [::ducktape_view_guest::wire::Rgba::from_rgba8(58, 56, 51, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(212, 210, 202, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(253, 253, 251, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(255, 255, 255, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(44, 43, 39, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(107, 105, 98, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(246, 245, 242, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(38, 37, 31, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(50, 47, 40, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(255, 255, 255, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(236, 235, 230, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(179, 177, 168, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(255, 255, 255, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(94, 92, 85, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(243, 242, 239, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(63, 62, 57, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(160, 90, 60, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(255, 255, 255, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(249, 241, 234, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(231, 210, 196, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(184, 84, 76, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(255, 255, 255, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(253, 244, 243, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(239, 214, 211, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(224, 101, 92, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(95, 158, 116, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(21, 20, 16, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(238, 245, 240, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(207, 227, 215, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(92, 180, 95, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(160, 123, 50, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(21, 20, 16, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(251, 244, 230, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(236, 220, 174, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(227, 180, 67, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(210, 208, 199, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(79, 77, 71, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(38, 37, 31, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(243, 241, 234, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(231, 230, 226, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(224, 223, 215, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(138, 137, 131, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(38, 37, 31, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(253, 252, 250, 0.501961), ::ducktape_view_guest::wire::Rgba::from_rgba8(253, 252, 250, 0.619608), ::ducktape_view_guest::wire::Rgba::from_rgba8(253, 252, 250, 0.858824), ::ducktape_view_guest::wire::Rgba::from_rgba8(40, 38, 34, 0.129412), ::ducktape_view_guest::wire::Rgba::from_rgba8(40, 38, 34, 0.219608), ::ducktape_view_guest::wire::Rgba::from_rgba8(40, 38, 34, 0.301961), ::ducktape_view_guest::wire::Rgba::from_rgba8(40, 38, 34, 0.219608), ::ducktape_view_guest::wire::Rgba::from_rgba8(40, 38, 34, 0.101961), ::ducktape_view_guest::wire::Rgba::from_rgba8(227, 225, 217, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(236, 234, 227, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(250, 250, 248, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(251, 251, 249, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(243, 242, 239, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(236, 235, 230, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(248, 247, 243, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(240, 239, 234, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(214, 212, 204, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(239, 238, 233, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(9, 11, 14, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(36, 42, 51, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(236, 233, 225, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(236, 214, 208, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(253, 246, 244, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(163, 82, 72, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(143, 70, 61, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(50, 47, 40, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(58, 57, 52, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(154, 152, 143, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(167, 165, 155, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(179, 177, 168, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(189, 187, 177, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(203, 201, 191, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(123, 167, 140, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(95, 122, 158, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(238, 242, 247, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(218, 226, 236, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(127, 154, 184, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(163, 82, 72, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(251, 236, 234, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(236, 207, 201, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(207, 106, 94, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(251, 248, 240, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(40, 38, 34, 0.341176), ::ducktape_view_guest::wire::Rgba::from_rgba8(247, 246, 242, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(250, 249, 246, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(252, 251, 249, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(251, 250, 247, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(253, 248, 243, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(240, 236, 225, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(244, 231, 200, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(217, 216, 208, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(213, 211, 202, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(182, 180, 168, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(200, 198, 188, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(194, 192, 182, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(208, 206, 196, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(220, 219, 212, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(122, 120, 114, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(126, 158, 136, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(102, 100, 94, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(122, 111, 158, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(241, 237, 245, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(221, 210, 230, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(240, 245, 241, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(220, 235, 224, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(238, 246, 239, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(225, 239, 227, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(47, 107, 65, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(251, 238, 236, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(244, 221, 216, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(161, 67, 56, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(246, 243, 249, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(74, 72, 67, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(224, 145, 138, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(160, 138, 90, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(95, 138, 114, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(237, 244, 239, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(122, 111, 158, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(241, 239, 247, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(74, 72, 67, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(242, 241, 237, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(185, 113, 78, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(250, 240, 233, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(192, 138, 62, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(250, 243, 230, 1.000000)] },
AppTheme::AppDark => __IcePalette { name: "app_dark", colors: [::ducktape_view_guest::wire::Rgba::from_rgba8(212, 210, 202, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(69, 68, 60, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(27, 26, 22, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(34, 33, 29, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(232, 230, 223, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(168, 166, 156, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(38, 37, 31, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(232, 230, 223, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(244, 242, 234, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(27, 26, 22, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(51, 50, 44, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(107, 106, 97, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(42, 41, 37, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(181, 179, 169, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(46, 45, 39, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(207, 205, 196, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(201, 138, 99, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(27, 26, 22, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(51, 38, 29, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(74, 56, 43, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(217, 123, 114, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(27, 26, 22, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(51, 33, 31, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(77, 47, 44, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(224, 101, 92, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(127, 184, 148, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(21, 20, 16, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(30, 42, 34, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(50, 71, 58, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(92, 180, 95, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(212, 169, 78, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(21, 20, 16, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(46, 39, 23, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(77, 63, 34, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(227, 180, 67, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(58, 57, 49, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(207, 205, 196, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(243, 241, 234, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(38, 37, 31, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(53, 52, 46, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(59, 58, 51, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(133, 131, 123, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(232, 230, 223, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(27, 26, 22, 0.501961), ::ducktape_view_guest::wire::Rgba::from_rgba8(27, 26, 22, 0.619608), ::ducktape_view_guest::wire::Rgba::from_rgba8(27, 26, 22, 0.858824), ::ducktape_view_guest::wire::Rgba::from_rgba8(0, 0, 0, 0.250980), ::ducktape_view_guest::wire::Rgba::from_rgba8(0, 0, 0, 0.349020), ::ducktape_view_guest::wire::Rgba::from_rgba8(0, 0, 0, 0.450980), ::ducktape_view_guest::wire::Rgba::from_rgba8(0, 0, 0, 0.349020), ::ducktape_view_guest::wire::Rgba::from_rgba8(0, 0, 0, 0.149020), ::ducktape_view_guest::wire::Rgba::from_rgba8(18, 17, 16, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(25, 24, 21, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(32, 31, 27, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(30, 29, 25, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(42, 41, 37, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(49, 48, 43, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(36, 35, 30, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(40, 39, 34, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(14, 13, 11, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(44, 43, 38, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(9, 11, 14, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(36, 42, 51, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(48, 47, 41, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(77, 47, 44, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(42, 29, 27, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(194, 90, 79, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(211, 104, 92, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(244, 242, 234, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(220, 218, 210, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(143, 141, 132, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(124, 122, 113, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(107, 106, 97, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(96, 95, 86, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(85, 84, 76, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(123, 167, 140, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(127, 154, 184, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(30, 37, 48, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(48, 62, 82, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(127, 154, 184, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(211, 104, 92, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(48, 31, 28, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(77, 47, 44, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(207, 106, 94, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(42, 37, 23, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(0, 0, 0, 0.501961), ::ducktape_view_guest::wire::Rgba::from_rgba8(32, 31, 26, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(35, 34, 29, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(38, 37, 32, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(38, 36, 24, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(42, 34, 27, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(53, 50, 42, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(69, 58, 30, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(63, 62, 54, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(69, 68, 60, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(110, 109, 99, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(91, 90, 82, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(98, 97, 90, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(74, 73, 65, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(51, 50, 44, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(163, 161, 152, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(126, 158, 136, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(157, 155, 146, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(168, 154, 201, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(42, 38, 51, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(68, 60, 87, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(30, 42, 34, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(50, 71, 58, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(29, 42, 32, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(36, 53, 42, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(143, 201, 162, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(47, 31, 28, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(61, 39, 35, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(222, 139, 127, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(38, 35, 48, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(46, 45, 40, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(160, 92, 85, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(192, 168, 110, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(127, 184, 148, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(30, 42, 34, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(168, 154, 201, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(42, 38, 51, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(207, 205, 196, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(46, 45, 40, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(208, 144, 104, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(51, 38, 29, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(212, 169, 78, 1.000000), ::ducktape_view_guest::wire::Rgba::from_rgba8(46, 39, 23, 1.000000)] },
}
}
}
}
__ice_generated_items_506167657356696577! {
#[allow(unused_parens)]
impl PagesView {
fn __state() -> Self {
Self {
pointer_y: 0.0,
comment_anchor_y: (-1.0),
comment_anchor_line: 0,
comments_card_height: 0.0,
document_reserve: crate::editor_view::no_reserve(),
document_focused: false,
focus_query: 0,
document_paint: crate::editor_view::empty_presentation(),
document: ::ducktape_view_guest::Editor::new("".to_owned()),
document_history: crate::editor_binding::initial_history(),
document_menu: crate::editor_binding::initial_menu(),
document_error: "".to_owned(),
document_dark: false,
document_commented: ::std::vec::Vec::new(),
document_marks: ::std::vec::Vec::new(),
active_palette: AppTheme::App,
connected: false,
chain: "".to_owned(),
route_serial: 0,
register_serial: 0,
loading: false,
busy: false,
host_error: "".to_owned(),
page_link: "".to_owned(),
pages: ::std::vec::Vec::new(),
blocks: ::std::vec::Vec::new(),
pages_viewport_width: 1280.0,
pages_viewport_height: 700.0,
pages_pane_width: 1280.0,
sidebar_width: 230.0,
page_menu_open: false,
page_create_open: false,
active_page: "".to_owned(),
active_page_title: "".to_owned(),
active_page_parent: "".to_owned(),
page_searching: false,
page_search_hits: ::std::vec::Vec::new(),
page_search_query: "".to_owned(),
page_search_serial: 0,
page_delete_armed: false,
autosave: "idle".to_owned(),
page_refusal: "".to_owned(),
subpages: ::std::vec::Vec::new(),
orphaned_comment_drafts: ::std::vec::Vec::new(),
block_comments_open: false,
scope_target: "".to_owned(),
scope_pinned: false,
thread_total: 0,
comment_rows: ::std::vec::Vec::new(),
threads_loading: false,
commented_hits: ::std::vec::Vec::new(),
reply_thread: "".to_owned(),
expanded_threads: ::std::vec::Vec::new(),
resolved_open: false,
page_draft: "".to_owned(),
page_search_draft: "".to_owned(),
block_comment_draft: "".to_owned(),
reply_draft: "".to_owned(),
pending_page: "".to_owned(),
pending_comment: "".to_owned(),
page_saved_text: "".to_owned(),
buffer_page: "".to_owned(),
page_inflight_text: "".to_owned(),
sent: false,
__ice_rev: [::ducktape_view_guest::rev::seed(); 64],
}
}
fn __boot_task(&mut self) -> ::ducktape_view_guest::Task<__PagesViewMessage> {
let task = (|| {
::ducktape_view_guest::Task::none()
})();
task
}
pub(crate) fn __boot() -> (Self, ::ducktape_view_guest::Task<__PagesViewMessage>) {
let mut state = Self::__state();
let task = state.__boot_task();
(state, task)
}
pub(crate) const __PREFERRED_WINDOW_SIZE: &'static str = "none";
#[allow(clippy::too_many_arguments)] fn __restore_state(pointer_y: f64, comment_anchor_y: f64, comment_anchor_line: i64, comments_card_height: f64, document_reserve: crate::editor_view::EditorReserve, document_focused: bool, focus_query: i64, document_paint: crate::editor_view::PreparedPresentation, document: ::ducktape_view_guest::Editor, document_history: crate::editor_binding::HistoryState, document_menu: crate::editor_binding::MenuState, document_error: ::std::string::String, document_dark: bool, document_commented: ::std::vec::Vec<i64>, document_marks: ::std::vec::Vec<crate::document_sync::CommentMark>, active_palette: AppTheme, connected: bool, chain: ::std::string::String, route_serial: i64, register_serial: i64, loading: bool, busy: bool, host_error: ::std::string::String, page_link: ::std::string::String, pages: ::std::vec::Vec<crate::host::PageItem>, blocks: ::std::vec::Vec<crate::document_sync::PageBlock>, pages_viewport_width: f64, pages_viewport_height: f64, pages_pane_width: f64, sidebar_width: f64, page_menu_open: bool, page_create_open: bool, active_page: ::std::string::String, active_page_title: ::std::string::String, active_page_parent: ::std::string::String, page_searching: bool, page_search_hits: ::std::vec::Vec<crate::host::PageSearchHit>, page_search_query: ::std::string::String, page_search_serial: i64, page_delete_armed: bool, autosave: ::std::string::String, page_refusal: ::std::string::String, subpages: ::std::vec::Vec<crate::host::Subpage>, orphaned_comment_drafts: ::std::vec::Vec<::std::string::String>, block_comments_open: bool, scope_target: ::std::string::String, scope_pinned: bool, thread_total: i64, comment_rows: ::std::vec::Vec<crate::host::PageCommentThreadRow>, threads_loading: bool, commented_hits: ::std::vec::Vec<::std::string::String>, reply_thread: ::std::string::String, expanded_threads: ::std::vec::Vec<::std::string::String>, resolved_open: bool, page_draft: ::std::string::String, page_search_draft: ::std::string::String, block_comment_draft: ::std::string::String, reply_draft: ::std::string::String, pending_page: ::std::string::String, pending_comment: ::std::string::String, page_saved_text: ::std::string::String, buffer_page: ::std::string::String, page_inflight_text: ::std::string::String, sent: bool) -> Self {
Self {
pointer_y: pointer_y,
comment_anchor_y: comment_anchor_y,
comment_anchor_line: comment_anchor_line,
comments_card_height: comments_card_height,
document_reserve: document_reserve,
document_focused: document_focused,
focus_query: focus_query,
document_paint: document_paint,
document: document,
document_history: document_history,
document_menu: document_menu,
document_error: document_error,
document_dark: document_dark,
document_commented: document_commented,
document_marks: document_marks,
active_palette: active_palette,
connected: connected,
chain: chain,
route_serial: route_serial,
register_serial: register_serial,
loading: loading,
busy: busy,
host_error: host_error,
page_link: page_link,
pages: pages,
blocks: blocks,
pages_viewport_width: pages_viewport_width,
pages_viewport_height: pages_viewport_height,
pages_pane_width: pages_pane_width,
sidebar_width: sidebar_width,
page_menu_open: page_menu_open,
page_create_open: page_create_open,
active_page: active_page,
active_page_title: active_page_title,
active_page_parent: active_page_parent,
page_searching: page_searching,
page_search_hits: page_search_hits,
page_search_query: page_search_query,
page_search_serial: page_search_serial,
page_delete_armed: page_delete_armed,
autosave: autosave,
page_refusal: page_refusal,
subpages: subpages,
orphaned_comment_drafts: orphaned_comment_drafts,
block_comments_open: block_comments_open,
scope_target: scope_target,
scope_pinned: scope_pinned,
thread_total: thread_total,
comment_rows: comment_rows,
threads_loading: threads_loading,
commented_hits: commented_hits,
reply_thread: reply_thread,
expanded_threads: expanded_threads,
resolved_open: resolved_open,
page_draft: page_draft,
page_search_draft: page_search_draft,
block_comment_draft: block_comment_draft,
reply_draft: reply_draft,
pending_page: pending_page,
pending_comment: pending_comment,
page_saved_text: page_saved_text,
buffer_page: buffer_page,
page_inflight_text: page_inflight_text,
sent: sent,
__ice_rev: [::ducktape_view_guest::rev::seed(); 64],
}
}
pub(crate) const __SNAPSHOT_SCHEMA: &'static str = "c2f5109c2b49baedbfd71e93d0db94a8e7ded190eefbfd51fd62505ca3524897";
pub(crate) fn __snapshot(&self) -> ::std::result::Result<::std::vec::Vec<u8>, ::std::string::String> { ::ducktape_view_guest::wire::Snapshot {schema: ::std::string::String::from(Self::__SNAPSHOT_SCHEMA), state: ::ducktape_view_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("PagesView"), fields: vec![(::std::string::String::from("pointer_y"), ::ducktape_view_guest::wire::SnapshotValue::F64(*(&self.pointer_y))), (::std::string::String::from("comment_anchor_y"), ::ducktape_view_guest::wire::SnapshotValue::F64(*(&self.comment_anchor_y))), (::std::string::String::from("comment_anchor_line"), ::ducktape_view_guest::wire::SnapshotValue::I64(*(&self.comment_anchor_line))), (::std::string::String::from("comments_card_height"), ::ducktape_view_guest::wire::SnapshotValue::F64(*(&self.comments_card_height))), (::std::string::String::from("document_reserve"), ::ducktape_view_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("EditorReserve"), fields: ::std::vec![(::std::string::String::from("line"), ::ducktape_view_guest::wire::SnapshotValue::I64(*(&(&self.document_reserve).line))), (::std::string::String::from("height"), ::ducktape_view_guest::wire::SnapshotValue::I64(*(&(&self.document_reserve).height)))] }), (::std::string::String::from("document_focused"), ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.document_focused))), (::std::string::String::from("focus_query"), ::ducktape_view_guest::wire::SnapshotValue::I64(*(&self.focus_query))), (::std::string::String::from("document_paint"), ::ducktape_view_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("PreparedPresentation"), fields: ::std::vec![(::std::string::String::from("reference"), ::ducktape_view_guest::wire::SnapshotValue::Bytes((&(&self.document_paint).reference).clone())), (::std::string::String::from("data"), ::ducktape_view_guest::wire::SnapshotValue::Bytes((&(&self.document_paint).data).clone())), (::std::string::String::from("notice"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&self.document_paint).notice)))] }), (::std::string::String::from("document"), ::ducktape_view_guest::wire::SnapshotValue::Bytes((&self.document).snapshot())), (::std::string::String::from("document_history"), ::ducktape_view_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("HistoryState"), fields: ::std::vec![(::std::string::String::from("snapshot"), ::ducktape_view_guest::wire::SnapshotValue::Bytes((&(&self.document_history).snapshot).clone()))] }), (::std::string::String::from("document_menu"), ::ducktape_view_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("MenuState"), fields: ::std::vec![(::std::string::String::from("snapshot"), ::ducktape_view_guest::wire::SnapshotValue::Bytes((&(&self.document_menu).snapshot).clone()))] }), (::std::string::String::from("document_error"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.document_error))), (::std::string::String::from("document_dark"), ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.document_dark))), (::std::string::String::from("document_commented"), ::ducktape_view_guest::wire::SnapshotValue::List((&self.document_commented).iter().map(|__item| ::ducktape_view_guest::wire::SnapshotValue::I64(*(__item))).collect())), (::std::string::String::from("document_marks"), ::ducktape_view_guest::wire::SnapshotValue::List((&self.document_marks).iter().map(|__item| ::ducktape_view_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("CommentMark"), fields: ::std::vec![(::std::string::String::from("line"), ::ducktape_view_guest::wire::SnapshotValue::I64(*(&(__item).line))), (::std::string::String::from("count"), ::ducktape_view_guest::wire::SnapshotValue::I64(*(&(__item).count)))] }).collect())), (::std::string::String::from("active_palette"), match &self.active_palette { AppTheme::App => ::ducktape_view_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("AppTheme"), fields: vec![(::std::string::String::from("app"), ::ducktape_view_guest::wire::SnapshotValue::Unit)] }, AppTheme::AppDark => ::ducktape_view_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("AppTheme"), fields: vec![(::std::string::String::from("app_dark"), ::ducktape_view_guest::wire::SnapshotValue::Unit)] } }), (::std::string::String::from("connected"), ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.connected))), (::std::string::String::from("chain"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.chain))), (::std::string::String::from("route_serial"), ::ducktape_view_guest::wire::SnapshotValue::I64(*(&self.route_serial))), (::std::string::String::from("register_serial"), ::ducktape_view_guest::wire::SnapshotValue::I64(*(&self.register_serial))), (::std::string::String::from("loading"), ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.loading))), (::std::string::String::from("busy"), ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.busy))), (::std::string::String::from("host_error"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.host_error))), (::std::string::String::from("page_link"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.page_link))), (::std::string::String::from("pages"), ::ducktape_view_guest::wire::SnapshotValue::List((&self.pages).iter().map(|__item| ::ducktape_view_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("PageItem"), fields: ::std::vec![(::std::string::String::from("id"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).id))), (::std::string::String::from("title"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).title))), (::std::string::String::from("parent"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).parent))), (::std::string::String::from("prefix"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).prefix))), (::std::string::String::from("child_count"), ::ducktape_view_guest::wire::SnapshotValue::I64(*(&(__item).child_count)))] }).collect())), (::std::string::String::from("blocks"), ::ducktape_view_guest::wire::SnapshotValue::List((&self.blocks).iter().map(|__item| ::ducktape_view_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("PageBlock"), fields: ::std::vec![(::std::string::String::from("key"), ::ducktape_view_guest::wire::SnapshotValue::I64(*(&(__item).key))), (::std::string::String::from("id"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).id))), (::std::string::String::from("parent"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).parent))), (::std::string::String::from("kind"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).kind))), (::std::string::String::from("text"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).text))), (::std::string::String::from("checked"), ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&(__item).checked))), (::std::string::String::from("prefix"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).prefix))), (::std::string::String::from("child_count"), ::ducktape_view_guest::wire::SnapshotValue::I64(*(&(__item).child_count)))] }).collect())), (::std::string::String::from("pages_viewport_width"), ::ducktape_view_guest::wire::SnapshotValue::F64(*(&self.pages_viewport_width))), (::std::string::String::from("pages_viewport_height"), ::ducktape_view_guest::wire::SnapshotValue::F64(*(&self.pages_viewport_height))), (::std::string::String::from("pages_pane_width"), ::ducktape_view_guest::wire::SnapshotValue::F64(*(&self.pages_pane_width))), (::std::string::String::from("sidebar_width"), ::ducktape_view_guest::wire::SnapshotValue::F64(*(&self.sidebar_width))), (::std::string::String::from("page_menu_open"), ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.page_menu_open))), (::std::string::String::from("page_create_open"), ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.page_create_open))), (::std::string::String::from("active_page"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.active_page))), (::std::string::String::from("active_page_title"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.active_page_title))), (::std::string::String::from("active_page_parent"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.active_page_parent))), (::std::string::String::from("page_searching"), ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.page_searching))), (::std::string::String::from("page_search_hits"), ::ducktape_view_guest::wire::SnapshotValue::List((&self.page_search_hits).iter().map(|__item| ::ducktape_view_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("PageSearchHit"), fields: ::std::vec![(::std::string::String::from("page_id"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).page_id))), (::std::string::String::from("page_title"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).page_title))), (::std::string::String::from("block_id"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).block_id))), (::std::string::String::from("kind"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).kind))), (::std::string::String::from("text"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).text)))] }).collect())), (::std::string::String::from("page_search_query"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.page_search_query))), (::std::string::String::from("page_search_serial"), ::ducktape_view_guest::wire::SnapshotValue::I64(*(&self.page_search_serial))), (::std::string::String::from("page_delete_armed"), ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.page_delete_armed))), (::std::string::String::from("autosave"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.autosave))), (::std::string::String::from("page_refusal"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.page_refusal))), (::std::string::String::from("subpages"), ::ducktape_view_guest::wire::SnapshotValue::List((&self.subpages).iter().map(|__item| ::ducktape_view_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("Subpage"), fields: ::std::vec![(::std::string::String::from("id"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).id))), (::std::string::String::from("title"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).title)))] }).collect())), (::std::string::String::from("orphaned_comment_drafts"), ::ducktape_view_guest::wire::SnapshotValue::List((&self.orphaned_comment_drafts).iter().map(|__item| ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(__item))).collect())), (::std::string::String::from("block_comments_open"), ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.block_comments_open))), (::std::string::String::from("scope_target"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.scope_target))), (::std::string::String::from("scope_pinned"), ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.scope_pinned))), (::std::string::String::from("thread_total"), ::ducktape_view_guest::wire::SnapshotValue::I64(*(&self.thread_total))), (::std::string::String::from("comment_rows"), ::ducktape_view_guest::wire::SnapshotValue::List((&self.comment_rows).iter().map(|__item| ::ducktape_view_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("PageCommentThreadRow"), fields: ::std::vec![(::std::string::String::from("thread"), ::ducktape_view_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("PageCommentThread"), fields: ::std::vec![(::std::string::String::from("id"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&(__item).thread).id))), (::std::string::String::from("target"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&(__item).thread).target))), (::std::string::String::from("author"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&(__item).thread).author))), (::std::string::String::from("meta"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(&(__item).thread).meta))), (::std::string::String::from("resolved"), ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&(&(__item).thread).resolved))), (::std::string::String::from("comment_count"), ::ducktape_view_guest::wire::SnapshotValue::I64(*(&(&(__item).thread).comment_count))), (::std::string::String::from("comments"), ::ducktape_view_guest::wire::SnapshotValue::List((&(&(__item).thread).comments).iter().map(|__item| ::ducktape_view_guest::wire::SnapshotValue::Record { name: ::std::string::String::from("PageComment"), fields: ::std::vec![(::std::string::String::from("id"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).id))), (::std::string::String::from("ordinal"), ::ducktape_view_guest::wire::SnapshotValue::I64(*(&(__item).ordinal))), (::std::string::String::from("author"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).author))), (::std::string::String::from("meta"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).meta))), (::std::string::String::from("text"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).text)))] }).collect()))] }), (::std::string::String::from("anchor"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&(__item).anchor)))] }).collect())), (::std::string::String::from("threads_loading"), ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.threads_loading))), (::std::string::String::from("commented_hits"), ::ducktape_view_guest::wire::SnapshotValue::List((&self.commented_hits).iter().map(|__item| ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(__item))).collect())), (::std::string::String::from("reply_thread"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.reply_thread))), (::std::string::String::from("expanded_threads"), ::ducktape_view_guest::wire::SnapshotValue::List((&self.expanded_threads).iter().map(|__item| ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(__item))).collect())), (::std::string::String::from("resolved_open"), ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.resolved_open))), (::std::string::String::from("page_draft"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.page_draft))), (::std::string::String::from("page_search_draft"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.page_search_draft))), (::std::string::String::from("block_comment_draft"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.block_comment_draft))), (::std::string::String::from("reply_draft"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.reply_draft))), (::std::string::String::from("pending_page"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.pending_page))), (::std::string::String::from("pending_comment"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.pending_comment))), (::std::string::String::from("page_saved_text"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.page_saved_text))), (::std::string::String::from("buffer_page"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.buffer_page))), (::std::string::String::from("page_inflight_text"), ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&self.page_inflight_text))), (::std::string::String::from("sent"), ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.sent)))] }}.encode() }
pub(crate) fn __restore(__bytes: &[u8]) -> ::std::result::Result<Self, ::std::string::String> { let __snapshot = ::ducktape_view_guest::wire::Snapshot::decode(__bytes)?; if __snapshot.schema != Self::__SNAPSHOT_SCHEMA { return ::std::result::Result::Err(::std::string::String::from("snapshot schema mismatch")); } let __value = __snapshot.state; ((|| { let ::ducktape_view_guest::wire::SnapshotValue::Record {name: __name, fields: __fields} = __value else { return ::std::option::Option::None; }; if __name != "PagesView" || __fields.len() != 64 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __value) = __fields.next()?; if __name != "pointer_y" { return ::std::option::Option::None; } let pointer_y: f64 = (match __value { ::ducktape_view_guest::wire::SnapshotValue::F64(__item) if __item.is_finite() => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "comment_anchor_y" { return ::std::option::Option::None; } let comment_anchor_y: f64 = (match __value { ::ducktape_view_guest::wire::SnapshotValue::F64(__item) if __item.is_finite() => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "comment_anchor_line" { return ::std::option::Option::None; } let comment_anchor_line: i64 = (match __value { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "comments_card_height" { return ::std::option::Option::None; } let comments_card_height: f64 = (match __value { ::ducktape_view_guest::wire::SnapshotValue::F64(__item) if __item.is_finite() => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "document_reserve" { return ::std::option::Option::None; } let document_reserve: crate::editor_view::EditorReserve = ((|| { let ::ducktape_view_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __value else { return ::std::option::Option::None; }; if __name != "EditorReserve" || __fields.len() != 2 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "line" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "height" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::editor_view::EditorReserve { line: (match __field_0 { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, height: (match __field_1 { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })())?; let (__name, __value) = __fields.next()?; if __name != "document_focused" { return ::std::option::Option::None; } let document_focused: bool = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "focus_query" { return ::std::option::Option::None; } let focus_query: i64 = (match __value { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "document_paint" { return ::std::option::Option::None; } let document_paint: crate::editor_view::PreparedPresentation = ((|| { let ::ducktape_view_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __value else { return ::std::option::Option::None; }; if __name != "PreparedPresentation" || __fields.len() != 3 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "reference" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "data" { return ::std::option::Option::None; } let (__name, __field_2) = __fields.next()?; if __name != "notice" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::editor_view::PreparedPresentation { reference: (match __field_0 { ::ducktape_view_guest::wire::SnapshotValue::Bytes(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, data: (match __field_1 { ::ducktape_view_guest::wire::SnapshotValue::Bytes(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, notice: (match __field_2 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })())?; let (__name, __value) = __fields.next()?; if __name != "document" { return ::std::option::Option::None; } let document: ::ducktape_view_guest::Editor = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Bytes(bytes) => ::ducktape_view_guest::Editor::restore(&bytes), _ => None })?; let (__name, __value) = __fields.next()?; if __name != "document_history" { return ::std::option::Option::None; } let document_history: crate::editor_binding::HistoryState = ((|| { let ::ducktape_view_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __value else { return ::std::option::Option::None; }; if __name != "HistoryState" || __fields.len() != 1 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "snapshot" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::editor_binding::HistoryState { snapshot: (match __field_0 { ::ducktape_view_guest::wire::SnapshotValue::Bytes(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })())?; let (__name, __value) = __fields.next()?; if __name != "document_menu" { return ::std::option::Option::None; } let document_menu: crate::editor_binding::MenuState = ((|| { let ::ducktape_view_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __value else { return ::std::option::Option::None; }; if __name != "MenuState" || __fields.len() != 1 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "snapshot" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::editor_binding::MenuState { snapshot: (match __field_0 { ::ducktape_view_guest::wire::SnapshotValue::Bytes(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })())?; let (__name, __value) = __fields.next()?; if __name != "document_error" { return ::std::option::Option::None; } let document_error: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "document_dark" { return ::std::option::Option::None; } let document_dark: bool = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "document_commented" { return ::std::option::Option::None; } let document_commented: ::std::vec::Vec<i64> = (match __value { ::ducktape_view_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| match __item { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None }).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "document_marks" { return ::std::option::Option::None; } let document_marks: ::std::vec::Vec<crate::document_sync::CommentMark> = (match __value { ::ducktape_view_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| (|| { let ::ducktape_view_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __item else { return ::std::option::Option::None; }; if __name != "CommentMark" || __fields.len() != 2 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "line" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "count" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::document_sync::CommentMark { line: (match __field_0 { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, count: (match __field_1 { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })()).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "active_palette" { return ::std::option::Option::None; } let active_palette: AppTheme = ((|| { let ::ducktape_view_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __value else { return ::std::option::Option::None; }; if __name != "AppTheme" || __fields.len() != 1 { return ::std::option::Option::None; } let (__variant, __payload) = __fields.into_iter().next()?; match __variant.as_str() { "app" => matches!(__payload, ::ducktape_view_guest::wire::SnapshotValue::Unit).then_some(AppTheme::App), "app_dark" => matches!(__payload, ::ducktape_view_guest::wire::SnapshotValue::Unit).then_some(AppTheme::AppDark), _ => ::std::option::Option::None } })())?; let (__name, __value) = __fields.next()?; if __name != "connected" { return ::std::option::Option::None; } let connected: bool = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "chain" { return ::std::option::Option::None; } let chain: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "route_serial" { return ::std::option::Option::None; } let route_serial: i64 = (match __value { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "register_serial" { return ::std::option::Option::None; } let register_serial: i64 = (match __value { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "loading" { return ::std::option::Option::None; } let loading: bool = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "busy" { return ::std::option::Option::None; } let busy: bool = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "host_error" { return ::std::option::Option::None; } let host_error: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "page_link" { return ::std::option::Option::None; } let page_link: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "pages" { return ::std::option::Option::None; } let pages: ::std::vec::Vec<crate::host::PageItem> = (match __value { ::ducktape_view_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| (|| { let ::ducktape_view_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __item else { return ::std::option::Option::None; }; if __name != "PageItem" || __fields.len() != 5 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "id" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "title" { return ::std::option::Option::None; } let (__name, __field_2) = __fields.next()?; if __name != "parent" { return ::std::option::Option::None; } let (__name, __field_3) = __fields.next()?; if __name != "prefix" { return ::std::option::Option::None; } let (__name, __field_4) = __fields.next()?; if __name != "child_count" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::PageItem { id: (match __field_0 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, title: (match __field_1 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, parent: (match __field_2 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, prefix: (match __field_3 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, child_count: (match __field_4 { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })()).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "blocks" { return ::std::option::Option::None; } let blocks: ::std::vec::Vec<crate::document_sync::PageBlock> = (match __value { ::ducktape_view_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| (|| { let ::ducktape_view_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __item else { return ::std::option::Option::None; }; if __name != "PageBlock" || __fields.len() != 8 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "key" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "id" { return ::std::option::Option::None; } let (__name, __field_2) = __fields.next()?; if __name != "parent" { return ::std::option::Option::None; } let (__name, __field_3) = __fields.next()?; if __name != "kind" { return ::std::option::Option::None; } let (__name, __field_4) = __fields.next()?; if __name != "text" { return ::std::option::Option::None; } let (__name, __field_5) = __fields.next()?; if __name != "checked" { return ::std::option::Option::None; } let (__name, __field_6) = __fields.next()?; if __name != "prefix" { return ::std::option::Option::None; } let (__name, __field_7) = __fields.next()?; if __name != "child_count" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::document_sync::PageBlock { key: (match __field_0 { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, id: (match __field_1 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, parent: (match __field_2 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, kind: (match __field_3 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, text: (match __field_4 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, checked: (match __field_5 { ::ducktape_view_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, prefix: (match __field_6 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, child_count: (match __field_7 { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })()).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "pages_viewport_width" { return ::std::option::Option::None; } let pages_viewport_width: f64 = (match __value { ::ducktape_view_guest::wire::SnapshotValue::F64(__item) if __item.is_finite() => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "pages_viewport_height" { return ::std::option::Option::None; } let pages_viewport_height: f64 = (match __value { ::ducktape_view_guest::wire::SnapshotValue::F64(__item) if __item.is_finite() => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "pages_pane_width" { return ::std::option::Option::None; } let pages_pane_width: f64 = (match __value { ::ducktape_view_guest::wire::SnapshotValue::F64(__item) if __item.is_finite() => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "sidebar_width" { return ::std::option::Option::None; } let sidebar_width: f64 = (match __value { ::ducktape_view_guest::wire::SnapshotValue::F64(__item) if __item.is_finite() => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "page_menu_open" { return ::std::option::Option::None; } let page_menu_open: bool = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "page_create_open" { return ::std::option::Option::None; } let page_create_open: bool = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "active_page" { return ::std::option::Option::None; } let active_page: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "active_page_title" { return ::std::option::Option::None; } let active_page_title: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "active_page_parent" { return ::std::option::Option::None; } let active_page_parent: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "page_searching" { return ::std::option::Option::None; } let page_searching: bool = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "page_search_hits" { return ::std::option::Option::None; } let page_search_hits: ::std::vec::Vec<crate::host::PageSearchHit> = (match __value { ::ducktape_view_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| (|| { let ::ducktape_view_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __item else { return ::std::option::Option::None; }; if __name != "PageSearchHit" || __fields.len() != 5 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "page_id" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "page_title" { return ::std::option::Option::None; } let (__name, __field_2) = __fields.next()?; if __name != "block_id" { return ::std::option::Option::None; } let (__name, __field_3) = __fields.next()?; if __name != "kind" { return ::std::option::Option::None; } let (__name, __field_4) = __fields.next()?; if __name != "text" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::PageSearchHit { page_id: (match __field_0 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, page_title: (match __field_1 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, block_id: (match __field_2 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, kind: (match __field_3 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, text: (match __field_4 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })()).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "page_search_query" { return ::std::option::Option::None; } let page_search_query: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "page_search_serial" { return ::std::option::Option::None; } let page_search_serial: i64 = (match __value { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "page_delete_armed" { return ::std::option::Option::None; } let page_delete_armed: bool = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "autosave" { return ::std::option::Option::None; } let autosave: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "page_refusal" { return ::std::option::Option::None; } let page_refusal: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "subpages" { return ::std::option::Option::None; } let subpages: ::std::vec::Vec<crate::host::Subpage> = (match __value { ::ducktape_view_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| (|| { let ::ducktape_view_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __item else { return ::std::option::Option::None; }; if __name != "Subpage" || __fields.len() != 2 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "id" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "title" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::Subpage { id: (match __field_0 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, title: (match __field_1 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })()).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "orphaned_comment_drafts" { return ::std::option::Option::None; } let orphaned_comment_drafts: ::std::vec::Vec<::std::string::String> = (match __value { ::ducktape_view_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| match __item { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None }).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "block_comments_open" { return ::std::option::Option::None; } let block_comments_open: bool = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "scope_target" { return ::std::option::Option::None; } let scope_target: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "scope_pinned" { return ::std::option::Option::None; } let scope_pinned: bool = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "thread_total" { return ::std::option::Option::None; } let thread_total: i64 = (match __value { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "comment_rows" { return ::std::option::Option::None; } let comment_rows: ::std::vec::Vec<crate::host::PageCommentThreadRow> = (match __value { ::ducktape_view_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| (|| { let ::ducktape_view_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __item else { return ::std::option::Option::None; }; if __name != "PageCommentThreadRow" || __fields.len() != 2 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "thread" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "anchor" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::PageCommentThreadRow { thread: ((|| { let ::ducktape_view_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __field_0 else { return ::std::option::Option::None; }; if __name != "PageCommentThread" || __fields.len() != 7 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "id" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "target" { return ::std::option::Option::None; } let (__name, __field_2) = __fields.next()?; if __name != "author" { return ::std::option::Option::None; } let (__name, __field_3) = __fields.next()?; if __name != "meta" { return ::std::option::Option::None; } let (__name, __field_4) = __fields.next()?; if __name != "resolved" { return ::std::option::Option::None; } let (__name, __field_5) = __fields.next()?; if __name != "comment_count" { return ::std::option::Option::None; } let (__name, __field_6) = __fields.next()?; if __name != "comments" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::PageCommentThread { id: (match __field_0 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, target: (match __field_1 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, author: (match __field_2 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, meta: (match __field_3 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, resolved: (match __field_4 { ::ducktape_view_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, comment_count: (match __field_5 { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, comments: (match __field_6 { ::ducktape_view_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| (|| { let ::ducktape_view_guest::wire::SnapshotValue::Record { name: __name, fields: __fields } = __item else { return ::std::option::Option::None; }; if __name != "PageComment" || __fields.len() != 5 { return ::std::option::Option::None; } let mut __fields = __fields.into_iter(); let (__name, __field_0) = __fields.next()?; if __name != "id" { return ::std::option::Option::None; } let (__name, __field_1) = __fields.next()?; if __name != "ordinal" { return ::std::option::Option::None; } let (__name, __field_2) = __fields.next()?; if __name != "author" { return ::std::option::Option::None; } let (__name, __field_3) = __fields.next()?; if __name != "meta" { return ::std::option::Option::None; } let (__name, __field_4) = __fields.next()?; if __name != "text" { return ::std::option::Option::None; } ::std::option::Option::Some(crate::host::PageComment { id: (match __field_0 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, ordinal: (match __field_1 { ::ducktape_view_guest::wire::SnapshotValue::I64(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, author: (match __field_2 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, meta: (match __field_3 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?, text: (match __field_4 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })()).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })? }) })())?, anchor: (match __field_1 { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })? }) })()).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "threads_loading" { return ::std::option::Option::None; } let threads_loading: bool = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "commented_hits" { return ::std::option::Option::None; } let commented_hits: ::std::vec::Vec<::std::string::String> = (match __value { ::ducktape_view_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| match __item { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None }).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "reply_thread" { return ::std::option::Option::None; } let reply_thread: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "expanded_threads" { return ::std::option::Option::None; } let expanded_threads: ::std::vec::Vec<::std::string::String> = (match __value { ::ducktape_view_guest::wire::SnapshotValue::List(__items) => __items.into_iter().map(|__item| match __item { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None }).collect::<::std::option::Option<::std::vec::Vec<_>>>(), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "resolved_open" { return ::std::option::Option::None; } let resolved_open: bool = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "page_draft" { return ::std::option::Option::None; } let page_draft: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "page_search_draft" { return ::std::option::Option::None; } let page_search_draft: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "block_comment_draft" { return ::std::option::Option::None; } let block_comment_draft: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "reply_draft" { return ::std::option::Option::None; } let reply_draft: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "pending_page" { return ::std::option::Option::None; } let pending_page: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "pending_comment" { return ::std::option::Option::None; } let pending_comment: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "page_saved_text" { return ::std::option::Option::None; } let page_saved_text: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "buffer_page" { return ::std::option::Option::None; } let buffer_page: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "page_inflight_text" { return ::std::option::Option::None; } let page_inflight_text: ::std::string::String = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Str(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; let (__name, __value) = __fields.next()?; if __name != "sent" { return ::std::option::Option::None; } let sent: bool = (match __value { ::ducktape_view_guest::wire::SnapshotValue::Bool(__item) => ::std::option::Option::Some(__item), _ => ::std::option::Option::None })?; ::std::option::Option::Some(Self::__restore_state(pointer_y, comment_anchor_y, comment_anchor_line, comments_card_height, document_reserve, document_focused, focus_query, document_paint, document, document_history, document_menu, document_error, document_dark, document_commented, document_marks, active_palette, connected, chain, route_serial, register_serial, loading, busy, host_error, page_link, pages, blocks, pages_viewport_width, pages_viewport_height, pages_pane_width, sidebar_width, page_menu_open, page_create_open, active_page, active_page_title, active_page_parent, page_searching, page_search_hits, page_search_query, page_search_serial, page_delete_armed, autosave, page_refusal, subpages, orphaned_comment_drafts, block_comments_open, scope_target, scope_pinned, thread_total, comment_rows, threads_loading, commented_hits, reply_thread, expanded_threads, resolved_open, page_draft, page_search_draft, block_comment_draft, reply_draft, pending_page, pending_comment, page_saved_text, buffer_page, page_inflight_text, sent)) })()).ok_or_else(|| ::std::string::String::from("snapshot state mismatch")) }
}
}
__ice_generated_items_506167657356696577! {
#[allow(unused_parens)]
impl PagesView {
}
}
__ice_generated_items_506167657356696577! {
#[allow(unused_parens)]
impl PagesView {
fn __subscription(&self) -> ::ducktape_view_guest::Subscription<__PagesViewMessage> {
::ducktape_view_guest::Subscription::batch([
::ducktape_view_guest::mouse::observe(::ducktape_view_guest::Subscription::filter_events(|event| match event { ::ducktape_view_guest::wire::Event::Mouse { event: ::ducktape_view_guest::wire::mouse::Event::CursorMoved { x, y }, .. } => Some(__PagesViewMessage::CommentPointerMoved(*x as f64, *y as f64)), _ => None })), 
::ducktape_view_guest::mouse::observe(::ducktape_view_guest::Subscription::filter_events(|event| match event { ::ducktape_view_guest::wire::Event::Mouse { event: ::ducktape_view_guest::wire::mouse::Event::ButtonReleased(button), .. } => Some(__PagesViewMessage::DocumentPointerReleased(*button)), _ => None })), 
::ducktape_view_guest::Subscription::filter_events(|event| match event { ::ducktape_view_guest::wire::Event::Keyboard { event: ::ducktape_view_guest::wire::keyboard::Event::Release(key), .. } => Some(__PagesViewMessage::DocumentKeyReleased(key.clone())), _ => None }),
::ducktape_view_guest::events::observe(::ducktape_view_guest::Subscription::filter_events(|event| match event { ::ducktape_view_guest::wire::Event::Observation { event: ::ducktape_view_guest::wire::events::Event::Window(::ducktape_view_guest::wire::events::Window::Focused), .. } => Some(__PagesViewMessage::DocumentWindowFocused), _ => None }), ::ducktape_view_guest::wire::events::Interest { focus: true, ..Default::default() }),
::ducktape_view_guest::events::observe(::ducktape_view_guest::Subscription::filter_events(|event| match event { ::ducktape_view_guest::wire::Event::Observation { event: ::ducktape_view_guest::wire::events::Event::Window(::ducktape_view_guest::wire::events::Window::Unfocused), .. } => Some(__PagesViewMessage::DocumentWindowUnfocused), _ => None }), ::ducktape_view_guest::wire::events::Interest { focus: true, ..Default::default() }),
crate::host::session().map(move |__value| __PagesViewMessage::SessionArrived(__value)),
if self.connected { ::ducktape_view_guest::Subscription::batch([crate::host::register(self.active_page.to_owned(), self.register_serial).map(move |__value| __PagesViewMessage::RegisterArrived(__value)),
]) } else { ::ducktape_view_guest::Subscription::none() },
if (self.connected && (!(self.page_search_query).is_empty())) { ::ducktape_view_guest::Subscription::batch([crate::host::search(self.page_search_query.to_owned(), self.page_search_serial).map(move |__value| __PagesViewMessage::SearchArrived(__value)),
]) } else { ::ducktape_view_guest::Subscription::none() },
crate::host::acts().map(move |__value| __PagesViewMessage::ActDone(__value)),
crate::host::saves().map(move |__value| __PagesViewMessage::SaveDone(__value)),
if ((((self.connected && (!self.loading)) && (!self.busy)) && (!(self.active_page).is_empty())) && (self.active_page == self.buffer_page)) { ::ducktape_view_guest::Subscription::batch([::ducktape_view_guest::every(::std::time::Duration::from_millis(900)).map(move |__value| __PagesViewMessage::PageAutosaveTick),
]) } else { ::ducktape_view_guest::Subscription::none() },
])
}
}
}
__ice_generated_items_506167657356696577! {
#[allow(unused_parens)]
impl PagesView {
}
#[cfg(test)] mod __ice_tests { use super::*;
#[test]
fn __ice_view_fits_default_stack() {
::std::thread::Builder::new().stack_size(4 * 1024 * 1024).spawn(|| {
let (__app, _) = PagesView::__boot();
let _ = __app.__view();
}).unwrap().join().unwrap();
}
}
}
include!("app_update.rs");
include!("app_view.rs");
include!("icon.rs");
include!("kit.rs");
include!("pages.rs");
include!("rows.rs");
