type KeyRelease = ::ducktape_view_guest::wire::keyboard::KeyState;
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum AppTheme {
    App,
    AppDark,
}
#[derive(Clone, Copy)]
struct Palette {
    name: &'static str,
    colors: [::ducktape_view_guest::wire::Rgba; 128],
}
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
    pub(crate) document_error: String,
    pub(crate) document_dark: bool,
    pub(crate) document_commented: Vec<i64>,
    pub(crate) document_marks: Vec<crate::document_sync::CommentMark>,
    pub(crate) active_palette: AppTheme,
    pub(crate) connected: bool,
    pub(crate) chain: String,
    pub(crate) route_serial: i64,
    pub(crate) register_serial: i64,
    pub(crate) loading: bool,
    pub(crate) busy: bool,
    pub(crate) host_error: String,
    pub(crate) page_link: String,
    pub(crate) pages: Vec<crate::host::PageItem>,
    pub(crate) blocks: Vec<crate::document_sync::PageBlock>,
    pub(crate) pages_viewport_width: f64,
    pub(crate) pages_viewport_height: f64,
    pub(crate) pages_pane_width: f64,
    pub(crate) sidebar_width: f64,
    pub(crate) page_menu_open: bool,
    pub(crate) page_create_open: bool,
    pub(crate) active_page: String,
    pub(crate) active_page_title: String,
    pub(crate) active_page_parent: String,
    pub(crate) page_searching: bool,
    pub(crate) page_search_hits: Vec<crate::host::PageSearchHit>,
    pub(crate) page_search_query: String,
    pub(crate) page_search_serial: i64,
    pub(crate) page_delete_armed: bool,
    pub(crate) autosave: String,
    pub(crate) page_refusal: String,
    pub(crate) subpages: Vec<crate::host::Subpage>,
    pub(crate) orphaned_comment_drafts: Vec<String>,
    pub(crate) block_comments_open: bool,
    pub(crate) scope_target: String,
    pub(crate) scope_pinned: bool,
    pub(crate) thread_total: i64,
    pub(crate) comment_rows: Vec<crate::host::PageCommentThreadRow>,
    pub(crate) threads_loading: bool,
    pub(crate) commented_hits: Vec<String>,
    pub(crate) reply_thread: String,
    pub(crate) expanded_threads: Vec<String>,
    pub(crate) resolved_open: bool,
    pub(crate) page_draft: String,
    pub(crate) page_search_draft: String,
    pub(crate) block_comment_draft: String,
    pub(crate) reply_draft: String,
    pub(crate) pending_page: String,
    pub(crate) pending_comment: String,
    pub(crate) page_saved_text: String,
    pub(crate) buffer_page: String,
    pub(crate) page_inflight_text: String,
    pub(crate) sent: bool,
}
impl ::std::fmt::Debug for PagesView {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PagesView")
    }
}
#[derive(Clone)]
pub enum Message {
    SessionArrived(crate::host::SessionItem),
    CommentPointerMoved(f64, f64),
    ChoosePage(String),
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
    OpenPageSearchHit(String, String),
    UseOrphanedCommentDraft(String),
    DiscardOrphanedCommentDraft(String),
    ToggleBlockComments,
    CloseBlockComments,
    NarrowCommentScope(String),
    WidenCommentScope,
    ResolveThreadSubmit(String, bool),
    SelectReplyThread(String),
    ToggleThreadReplies(String),
    ToggleResolvedComments,
    PostThreadReply(String),
    PostBlockCommentSubmit,
    CopyToClipboard(String, String),
    DocumentPointerReleased(::ducktape_view_guest::wire::mouse::Button),
    DocumentKeyReleased(KeyRelease),
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
    PageDraftChanged(String),
    SearchDraftChanged(String),
    ReplyDraftChanged(String),
    CommentDraftChanged(String),
    DocumentUpdated(::ducktape_view_guest::EditorDocumentUpdate),
    DocumentTransaction(::ducktape_view_guest::EditorTransaction<Message>),
}
impl ::std::fmt::Debug for Message {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Message")
    }
}
#[allow(unused_parens)]
impl PagesView {
    fn palette(&self) -> Palette {
        match self.active_palette.clone() {
            AppTheme::App => Palette {
                name: "app",
                colors: [
                    ::ducktape_view_guest::wire::Rgba([
                        58.0 / 255.0,
                        56.0 / 255.0,
                        51.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        212.0 / 255.0,
                        210.0 / 255.0,
                        202.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        253.0 / 255.0,
                        253.0 / 255.0,
                        251.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        255.0 / 255.0,
                        255.0 / 255.0,
                        255.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        44.0 / 255.0,
                        43.0 / 255.0,
                        39.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        107.0 / 255.0,
                        105.0 / 255.0,
                        98.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        246.0 / 255.0,
                        245.0 / 255.0,
                        242.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        38.0 / 255.0,
                        37.0 / 255.0,
                        31.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        50.0 / 255.0,
                        47.0 / 255.0,
                        40.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        255.0 / 255.0,
                        255.0 / 255.0,
                        255.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        236.0 / 255.0,
                        235.0 / 255.0,
                        230.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        179.0 / 255.0,
                        177.0 / 255.0,
                        168.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        255.0 / 255.0,
                        255.0 / 255.0,
                        255.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        94.0 / 255.0,
                        92.0 / 255.0,
                        85.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        243.0 / 255.0,
                        242.0 / 255.0,
                        239.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        63.0 / 255.0,
                        62.0 / 255.0,
                        57.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        160.0 / 255.0,
                        90.0 / 255.0,
                        60.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        255.0 / 255.0,
                        255.0 / 255.0,
                        255.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        249.0 / 255.0,
                        241.0 / 255.0,
                        234.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        231.0 / 255.0,
                        210.0 / 255.0,
                        196.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        184.0 / 255.0,
                        84.0 / 255.0,
                        76.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        255.0 / 255.0,
                        255.0 / 255.0,
                        255.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        253.0 / 255.0,
                        244.0 / 255.0,
                        243.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        239.0 / 255.0,
                        214.0 / 255.0,
                        211.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        224.0 / 255.0,
                        101.0 / 255.0,
                        92.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        95.0 / 255.0,
                        158.0 / 255.0,
                        116.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        21.0 / 255.0,
                        20.0 / 255.0,
                        16.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        238.0 / 255.0,
                        245.0 / 255.0,
                        240.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        207.0 / 255.0,
                        227.0 / 255.0,
                        215.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        92.0 / 255.0,
                        180.0 / 255.0,
                        95.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        160.0 / 255.0,
                        123.0 / 255.0,
                        50.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        21.0 / 255.0,
                        20.0 / 255.0,
                        16.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        251.0 / 255.0,
                        244.0 / 255.0,
                        230.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        236.0 / 255.0,
                        220.0 / 255.0,
                        174.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        227.0 / 255.0,
                        180.0 / 255.0,
                        67.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        210.0 / 255.0,
                        208.0 / 255.0,
                        199.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        79.0 / 255.0,
                        77.0 / 255.0,
                        71.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        38.0 / 255.0,
                        37.0 / 255.0,
                        31.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        243.0 / 255.0,
                        241.0 / 255.0,
                        234.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        231.0 / 255.0,
                        230.0 / 255.0,
                        226.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        224.0 / 255.0,
                        223.0 / 255.0,
                        215.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        138.0 / 255.0,
                        137.0 / 255.0,
                        131.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        38.0 / 255.0,
                        37.0 / 255.0,
                        31.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        253.0 / 255.0,
                        252.0 / 255.0,
                        250.0 / 255.0,
                        0.501961,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        253.0 / 255.0,
                        252.0 / 255.0,
                        250.0 / 255.0,
                        0.619608,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        253.0 / 255.0,
                        252.0 / 255.0,
                        250.0 / 255.0,
                        0.858824,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        40.0 / 255.0,
                        38.0 / 255.0,
                        34.0 / 255.0,
                        0.129412,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        40.0 / 255.0,
                        38.0 / 255.0,
                        34.0 / 255.0,
                        0.219608,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        40.0 / 255.0,
                        38.0 / 255.0,
                        34.0 / 255.0,
                        0.301961,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        40.0 / 255.0,
                        38.0 / 255.0,
                        34.0 / 255.0,
                        0.219608,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        40.0 / 255.0,
                        38.0 / 255.0,
                        34.0 / 255.0,
                        0.101961,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        227.0 / 255.0,
                        225.0 / 255.0,
                        217.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        236.0 / 255.0,
                        234.0 / 255.0,
                        227.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        250.0 / 255.0,
                        250.0 / 255.0,
                        248.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        251.0 / 255.0,
                        251.0 / 255.0,
                        249.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        243.0 / 255.0,
                        242.0 / 255.0,
                        239.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        236.0 / 255.0,
                        235.0 / 255.0,
                        230.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        248.0 / 255.0,
                        247.0 / 255.0,
                        243.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        240.0 / 255.0,
                        239.0 / 255.0,
                        234.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        214.0 / 255.0,
                        212.0 / 255.0,
                        204.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        239.0 / 255.0,
                        238.0 / 255.0,
                        233.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        9.0 / 255.0,
                        11.0 / 255.0,
                        14.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        36.0 / 255.0,
                        42.0 / 255.0,
                        51.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        236.0 / 255.0,
                        233.0 / 255.0,
                        225.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        236.0 / 255.0,
                        214.0 / 255.0,
                        208.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        253.0 / 255.0,
                        246.0 / 255.0,
                        244.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        163.0 / 255.0,
                        82.0 / 255.0,
                        72.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        143.0 / 255.0,
                        70.0 / 255.0,
                        61.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        50.0 / 255.0,
                        47.0 / 255.0,
                        40.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        58.0 / 255.0,
                        57.0 / 255.0,
                        52.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        154.0 / 255.0,
                        152.0 / 255.0,
                        143.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        167.0 / 255.0,
                        165.0 / 255.0,
                        155.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        179.0 / 255.0,
                        177.0 / 255.0,
                        168.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        189.0 / 255.0,
                        187.0 / 255.0,
                        177.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        203.0 / 255.0,
                        201.0 / 255.0,
                        191.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        123.0 / 255.0,
                        167.0 / 255.0,
                        140.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        95.0 / 255.0,
                        122.0 / 255.0,
                        158.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        238.0 / 255.0,
                        242.0 / 255.0,
                        247.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        218.0 / 255.0,
                        226.0 / 255.0,
                        236.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        127.0 / 255.0,
                        154.0 / 255.0,
                        184.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        163.0 / 255.0,
                        82.0 / 255.0,
                        72.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        251.0 / 255.0,
                        236.0 / 255.0,
                        234.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        236.0 / 255.0,
                        207.0 / 255.0,
                        201.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        207.0 / 255.0,
                        106.0 / 255.0,
                        94.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        251.0 / 255.0,
                        248.0 / 255.0,
                        240.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        40.0 / 255.0,
                        38.0 / 255.0,
                        34.0 / 255.0,
                        0.341176,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        247.0 / 255.0,
                        246.0 / 255.0,
                        242.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        250.0 / 255.0,
                        249.0 / 255.0,
                        246.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        252.0 / 255.0,
                        251.0 / 255.0,
                        249.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        251.0 / 255.0,
                        250.0 / 255.0,
                        247.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        253.0 / 255.0,
                        248.0 / 255.0,
                        243.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        240.0 / 255.0,
                        236.0 / 255.0,
                        225.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        244.0 / 255.0,
                        231.0 / 255.0,
                        200.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        217.0 / 255.0,
                        216.0 / 255.0,
                        208.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        213.0 / 255.0,
                        211.0 / 255.0,
                        202.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        182.0 / 255.0,
                        180.0 / 255.0,
                        168.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        200.0 / 255.0,
                        198.0 / 255.0,
                        188.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        194.0 / 255.0,
                        192.0 / 255.0,
                        182.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        208.0 / 255.0,
                        206.0 / 255.0,
                        196.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        220.0 / 255.0,
                        219.0 / 255.0,
                        212.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        122.0 / 255.0,
                        120.0 / 255.0,
                        114.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        126.0 / 255.0,
                        158.0 / 255.0,
                        136.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        102.0 / 255.0,
                        100.0 / 255.0,
                        94.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        122.0 / 255.0,
                        111.0 / 255.0,
                        158.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        241.0 / 255.0,
                        237.0 / 255.0,
                        245.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        221.0 / 255.0,
                        210.0 / 255.0,
                        230.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        240.0 / 255.0,
                        245.0 / 255.0,
                        241.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        220.0 / 255.0,
                        235.0 / 255.0,
                        224.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        238.0 / 255.0,
                        246.0 / 255.0,
                        239.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        225.0 / 255.0,
                        239.0 / 255.0,
                        227.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        47.0 / 255.0,
                        107.0 / 255.0,
                        65.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        251.0 / 255.0,
                        238.0 / 255.0,
                        236.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        244.0 / 255.0,
                        221.0 / 255.0,
                        216.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        161.0 / 255.0,
                        67.0 / 255.0,
                        56.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        246.0 / 255.0,
                        243.0 / 255.0,
                        249.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        74.0 / 255.0,
                        72.0 / 255.0,
                        67.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        224.0 / 255.0,
                        145.0 / 255.0,
                        138.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        160.0 / 255.0,
                        138.0 / 255.0,
                        90.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        95.0 / 255.0,
                        138.0 / 255.0,
                        114.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        237.0 / 255.0,
                        244.0 / 255.0,
                        239.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        122.0 / 255.0,
                        111.0 / 255.0,
                        158.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        241.0 / 255.0,
                        239.0 / 255.0,
                        247.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        74.0 / 255.0,
                        72.0 / 255.0,
                        67.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        242.0 / 255.0,
                        241.0 / 255.0,
                        237.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        185.0 / 255.0,
                        113.0 / 255.0,
                        78.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        250.0 / 255.0,
                        240.0 / 255.0,
                        233.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        192.0 / 255.0,
                        138.0 / 255.0,
                        62.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        250.0 / 255.0,
                        243.0 / 255.0,
                        230.0 / 255.0,
                        1.000000,
                    ]),
                ],
            },
            AppTheme::AppDark => Palette {
                name: "app_dark",
                colors: [
                    ::ducktape_view_guest::wire::Rgba([
                        212.0 / 255.0,
                        210.0 / 255.0,
                        202.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        69.0 / 255.0,
                        68.0 / 255.0,
                        60.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        27.0 / 255.0,
                        26.0 / 255.0,
                        22.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        34.0 / 255.0,
                        33.0 / 255.0,
                        29.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        232.0 / 255.0,
                        230.0 / 255.0,
                        223.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        168.0 / 255.0,
                        166.0 / 255.0,
                        156.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        38.0 / 255.0,
                        37.0 / 255.0,
                        31.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        232.0 / 255.0,
                        230.0 / 255.0,
                        223.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        244.0 / 255.0,
                        242.0 / 255.0,
                        234.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        27.0 / 255.0,
                        26.0 / 255.0,
                        22.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        51.0 / 255.0,
                        50.0 / 255.0,
                        44.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        107.0 / 255.0,
                        106.0 / 255.0,
                        97.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        42.0 / 255.0,
                        41.0 / 255.0,
                        37.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        181.0 / 255.0,
                        179.0 / 255.0,
                        169.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        46.0 / 255.0,
                        45.0 / 255.0,
                        39.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        207.0 / 255.0,
                        205.0 / 255.0,
                        196.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        201.0 / 255.0,
                        138.0 / 255.0,
                        99.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        27.0 / 255.0,
                        26.0 / 255.0,
                        22.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        51.0 / 255.0,
                        38.0 / 255.0,
                        29.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        74.0 / 255.0,
                        56.0 / 255.0,
                        43.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        217.0 / 255.0,
                        123.0 / 255.0,
                        114.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        27.0 / 255.0,
                        26.0 / 255.0,
                        22.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        51.0 / 255.0,
                        33.0 / 255.0,
                        31.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        77.0 / 255.0,
                        47.0 / 255.0,
                        44.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        224.0 / 255.0,
                        101.0 / 255.0,
                        92.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        127.0 / 255.0,
                        184.0 / 255.0,
                        148.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        21.0 / 255.0,
                        20.0 / 255.0,
                        16.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        30.0 / 255.0,
                        42.0 / 255.0,
                        34.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        50.0 / 255.0,
                        71.0 / 255.0,
                        58.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        92.0 / 255.0,
                        180.0 / 255.0,
                        95.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        212.0 / 255.0,
                        169.0 / 255.0,
                        78.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        21.0 / 255.0,
                        20.0 / 255.0,
                        16.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        46.0 / 255.0,
                        39.0 / 255.0,
                        23.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        77.0 / 255.0,
                        63.0 / 255.0,
                        34.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        227.0 / 255.0,
                        180.0 / 255.0,
                        67.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        58.0 / 255.0,
                        57.0 / 255.0,
                        49.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        207.0 / 255.0,
                        205.0 / 255.0,
                        196.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        243.0 / 255.0,
                        241.0 / 255.0,
                        234.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        38.0 / 255.0,
                        37.0 / 255.0,
                        31.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        53.0 / 255.0,
                        52.0 / 255.0,
                        46.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        59.0 / 255.0,
                        58.0 / 255.0,
                        51.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        133.0 / 255.0,
                        131.0 / 255.0,
                        123.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        232.0 / 255.0,
                        230.0 / 255.0,
                        223.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        27.0 / 255.0,
                        26.0 / 255.0,
                        22.0 / 255.0,
                        0.501961,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        27.0 / 255.0,
                        26.0 / 255.0,
                        22.0 / 255.0,
                        0.619608,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        27.0 / 255.0,
                        26.0 / 255.0,
                        22.0 / 255.0,
                        0.858824,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        0.0 / 255.0,
                        0.0 / 255.0,
                        0.0 / 255.0,
                        0.250980,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        0.0 / 255.0,
                        0.0 / 255.0,
                        0.0 / 255.0,
                        0.349020,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        0.0 / 255.0,
                        0.0 / 255.0,
                        0.0 / 255.0,
                        0.450980,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        0.0 / 255.0,
                        0.0 / 255.0,
                        0.0 / 255.0,
                        0.349020,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        0.0 / 255.0,
                        0.0 / 255.0,
                        0.0 / 255.0,
                        0.149020,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        18.0 / 255.0,
                        17.0 / 255.0,
                        16.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        25.0 / 255.0,
                        24.0 / 255.0,
                        21.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        32.0 / 255.0,
                        31.0 / 255.0,
                        27.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        30.0 / 255.0,
                        29.0 / 255.0,
                        25.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        42.0 / 255.0,
                        41.0 / 255.0,
                        37.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        49.0 / 255.0,
                        48.0 / 255.0,
                        43.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        36.0 / 255.0,
                        35.0 / 255.0,
                        30.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        40.0 / 255.0,
                        39.0 / 255.0,
                        34.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        14.0 / 255.0,
                        13.0 / 255.0,
                        11.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        44.0 / 255.0,
                        43.0 / 255.0,
                        38.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        9.0 / 255.0,
                        11.0 / 255.0,
                        14.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        36.0 / 255.0,
                        42.0 / 255.0,
                        51.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        48.0 / 255.0,
                        47.0 / 255.0,
                        41.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        77.0 / 255.0,
                        47.0 / 255.0,
                        44.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        42.0 / 255.0,
                        29.0 / 255.0,
                        27.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        194.0 / 255.0,
                        90.0 / 255.0,
                        79.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        211.0 / 255.0,
                        104.0 / 255.0,
                        92.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        244.0 / 255.0,
                        242.0 / 255.0,
                        234.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        220.0 / 255.0,
                        218.0 / 255.0,
                        210.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        143.0 / 255.0,
                        141.0 / 255.0,
                        132.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        124.0 / 255.0,
                        122.0 / 255.0,
                        113.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        107.0 / 255.0,
                        106.0 / 255.0,
                        97.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        96.0 / 255.0,
                        95.0 / 255.0,
                        86.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        85.0 / 255.0,
                        84.0 / 255.0,
                        76.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        123.0 / 255.0,
                        167.0 / 255.0,
                        140.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        127.0 / 255.0,
                        154.0 / 255.0,
                        184.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        30.0 / 255.0,
                        37.0 / 255.0,
                        48.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        48.0 / 255.0,
                        62.0 / 255.0,
                        82.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        127.0 / 255.0,
                        154.0 / 255.0,
                        184.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        211.0 / 255.0,
                        104.0 / 255.0,
                        92.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        48.0 / 255.0,
                        31.0 / 255.0,
                        28.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        77.0 / 255.0,
                        47.0 / 255.0,
                        44.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        207.0 / 255.0,
                        106.0 / 255.0,
                        94.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        42.0 / 255.0,
                        37.0 / 255.0,
                        23.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        0.0 / 255.0,
                        0.0 / 255.0,
                        0.0 / 255.0,
                        0.501961,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        32.0 / 255.0,
                        31.0 / 255.0,
                        26.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        35.0 / 255.0,
                        34.0 / 255.0,
                        29.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        38.0 / 255.0,
                        37.0 / 255.0,
                        32.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        38.0 / 255.0,
                        36.0 / 255.0,
                        24.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        42.0 / 255.0,
                        34.0 / 255.0,
                        27.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        53.0 / 255.0,
                        50.0 / 255.0,
                        42.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        69.0 / 255.0,
                        58.0 / 255.0,
                        30.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        63.0 / 255.0,
                        62.0 / 255.0,
                        54.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        69.0 / 255.0,
                        68.0 / 255.0,
                        60.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        110.0 / 255.0,
                        109.0 / 255.0,
                        99.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        91.0 / 255.0,
                        90.0 / 255.0,
                        82.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        98.0 / 255.0,
                        97.0 / 255.0,
                        90.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        74.0 / 255.0,
                        73.0 / 255.0,
                        65.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        51.0 / 255.0,
                        50.0 / 255.0,
                        44.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        163.0 / 255.0,
                        161.0 / 255.0,
                        152.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        126.0 / 255.0,
                        158.0 / 255.0,
                        136.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        157.0 / 255.0,
                        155.0 / 255.0,
                        146.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        168.0 / 255.0,
                        154.0 / 255.0,
                        201.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        42.0 / 255.0,
                        38.0 / 255.0,
                        51.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        68.0 / 255.0,
                        60.0 / 255.0,
                        87.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        30.0 / 255.0,
                        42.0 / 255.0,
                        34.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        50.0 / 255.0,
                        71.0 / 255.0,
                        58.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        29.0 / 255.0,
                        42.0 / 255.0,
                        32.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        36.0 / 255.0,
                        53.0 / 255.0,
                        42.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        143.0 / 255.0,
                        201.0 / 255.0,
                        162.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        47.0 / 255.0,
                        31.0 / 255.0,
                        28.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        61.0 / 255.0,
                        39.0 / 255.0,
                        35.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        222.0 / 255.0,
                        139.0 / 255.0,
                        127.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        38.0 / 255.0,
                        35.0 / 255.0,
                        48.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        46.0 / 255.0,
                        45.0 / 255.0,
                        40.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        160.0 / 255.0,
                        92.0 / 255.0,
                        85.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        192.0 / 255.0,
                        168.0 / 255.0,
                        110.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        127.0 / 255.0,
                        184.0 / 255.0,
                        148.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        30.0 / 255.0,
                        42.0 / 255.0,
                        34.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        168.0 / 255.0,
                        154.0 / 255.0,
                        201.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        42.0 / 255.0,
                        38.0 / 255.0,
                        51.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        207.0 / 255.0,
                        205.0 / 255.0,
                        196.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        46.0 / 255.0,
                        45.0 / 255.0,
                        40.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        208.0 / 255.0,
                        144.0 / 255.0,
                        104.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        51.0 / 255.0,
                        38.0 / 255.0,
                        29.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        212.0 / 255.0,
                        169.0 / 255.0,
                        78.0 / 255.0,
                        1.000000,
                    ]),
                    ::ducktape_view_guest::wire::Rgba([
                        46.0 / 255.0,
                        39.0 / 255.0,
                        23.0 / 255.0,
                        1.000000,
                    ]),
                ],
            },
        }
    }
}
#[allow(unused_parens)]
impl PagesView {
    fn initial_state() -> Self {
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
            document_commented: Vec::new(),
            document_marks: Vec::new(),
            active_palette: AppTheme::App,
            connected: false,
            chain: "".to_owned(),
            route_serial: 0,
            register_serial: 0,
            loading: false,
            busy: false,
            host_error: "".to_owned(),
            page_link: "".to_owned(),
            pages: Vec::new(),
            blocks: Vec::new(),
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
            page_search_hits: Vec::new(),
            page_search_query: "".to_owned(),
            page_search_serial: 0,
            page_delete_armed: false,
            autosave: "idle".to_owned(),
            page_refusal: "".to_owned(),
            subpages: Vec::new(),
            orphaned_comment_drafts: Vec::new(),
            block_comments_open: false,
            scope_target: "".to_owned(),
            scope_pinned: false,
            thread_total: 0,
            comment_rows: Vec::new(),
            threads_loading: false,
            commented_hits: Vec::new(),
            reply_thread: "".to_owned(),
            expanded_threads: Vec::new(),
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
        }
    }
    pub(crate) fn boot() -> (Self, ducktape_view_guest::Task<Message>) {
        (Self::initial_state(), ducktape_view_guest::Task::none())
    }
    pub(crate) const PREFERRED_WINDOW_SIZE: &'static str = "none";
    pub(crate) const SNAPSHOT_SCHEMA: &'static str =
        "c2f5109c2b49baedbfd71e93d0db94a8e7ded190eefbfd51fd62505ca3524897";
    pub(crate) fn snapshot(&self) -> Result<Vec<u8>, String> {
        ::ducktape_view_guest::wire::Snapshot {
            schema: String::from(Self::SNAPSHOT_SCHEMA),
            state: ::ducktape_view_guest::wire::SnapshotValue::Record {
                name: String::from("PagesView"),
                fields: vec![
                    (String::from("pointer_y"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& self
                    .pointer_y))), (String::from("comment_anchor_y"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& self
                    .comment_anchor_y))), (String::from("comment_anchor_line"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .comment_anchor_line))), (String::from("comments_card_height"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& self
                    .comments_card_height))), (String::from("document_reserve"),
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("EditorReserve"), fields :
                    ::std::vec![(String::from("line"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (& self
                    .document_reserve).line))), (String::from("height"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (& self
                    .document_reserve).height)))] }), (String::from("document_focused"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .document_focused))), (String::from("focus_query"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .focus_query))), (String::from("document_paint"),
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("PreparedPresentation"), fields :
                    ::std::vec![(String::from("reference"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bytes((& (& self
                    .document_paint).reference).clone())), (String::from("data"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bytes((& (& self
                    .document_paint).data).clone())), (String::from("notice"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.document_paint).notice)))] }), (String::from("document"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bytes((& self.document)
                    .snapshot())), (String::from("document_history"),
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("HistoryState"), fields :
                    ::std::vec![(String::from("snapshot"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bytes((& (& self
                    .document_history).snapshot).clone()))] }),
                    (String::from("document_menu"),
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("MenuState"), fields :
                    ::std::vec![(String::from("snapshot"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bytes((& (& self
                    .document_menu).snapshot).clone()))] }),
                    (String::from("document_error"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.document_error))), (String::from("document_dark"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .document_dark))), (String::from("document_commented"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self
                    .document_commented).iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (item)))
                    .collect())), (String::from("document_marks"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self
                    .document_marks).iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("CommentMark"), fields :
                    ::std::vec![(String::from("line"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item).line))),
                    (String::from("count"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item).count)))]
                    }).collect())), (String::from("active_palette"), match & self
                    .active_palette { AppTheme::App =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("AppTheme"), fields : vec![(String::from("app"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    AppTheme::AppDark =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("AppTheme"), fields : vec![(String::from("app_dark"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] } }),
                    (String::from("connected"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .connected))), (String::from("chain"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.chain))), (String::from("route_serial"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .route_serial))), (String::from("register_serial"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .register_serial))), (String::from("loading"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .loading))), (String::from("busy"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self.busy))),
                    (String::from("host_error"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.host_error))), (String::from("page_link"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.page_link))), (String::from("pages"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self.pages)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("PageItem"), fields : ::std::vec![(String::from("id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).id))), (String::from("title"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).title))), (String::from("parent"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).parent))), (String::from("prefix"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).prefix))), (String::from("child_count"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .child_count)))] }).collect())), (String::from("blocks"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self.blocks)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("PageBlock"), fields : ::std::vec![(String::from("key"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item).key))),
                    (String::from("id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).id))), (String::from("parent"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).parent))), (String::from("kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).kind))), (String::from("text"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).text))), (String::from("checked"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .checked))), (String::from("prefix"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).prefix))), (String::from("child_count"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .child_count)))] }).collect())),
                    (String::from("pages_viewport_width"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& self
                    .pages_viewport_width))), (String::from("pages_viewport_height"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& self
                    .pages_viewport_height))), (String::from("pages_pane_width"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& self
                    .pages_pane_width))), (String::from("sidebar_width"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& self
                    .sidebar_width))), (String::from("page_menu_open"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .page_menu_open))), (String::from("page_create_open"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .page_create_open))), (String::from("active_page"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.active_page))), (String::from("active_page_title"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.active_page_title))), (String::from("active_page_parent"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.active_page_parent))), (String::from("page_searching"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .page_searching))), (String::from("page_search_hits"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self
                    .page_search_hits).iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("PageSearchHit"), fields :
                    ::std::vec![(String::from("page_id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).page_id))), (String::from("page_title"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).page_title))), (String::from("block_id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).block_id))), (String::from("kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).kind))), (String::from("text"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).text)))] }).collect())), (String::from("page_search_query"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.page_search_query))), (String::from("page_search_serial"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .page_search_serial))), (String::from("page_delete_armed"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .page_delete_armed))), (String::from("autosave"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.autosave))), (String::from("page_refusal"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.page_refusal))), (String::from("subpages"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self.subpages)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("Subpage"), fields : ::std::vec![(String::from("id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).id))), (String::from("title"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).title)))] }).collect())),
                    (String::from("orphaned_comment_drafts"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self
                    .orphaned_comment_drafts).iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(item)))
                    .collect())), (String::from("block_comments_open"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .block_comments_open))), (String::from("scope_target"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.scope_target))), (String::from("scope_pinned"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .scope_pinned))), (String::from("thread_total"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .thread_total))), (String::from("comment_rows"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self
                    .comment_rows).iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("PageCommentThreadRow"), fields :
                    ::std::vec![(String::from("thread"),
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("PageCommentThread"), fields :
                    ::std::vec![(String::from("id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& (item).thread).id))), (String::from("target"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& (item).thread).target))), (String::from("author"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& (item).thread).author))), (String::from("meta"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& (item).thread).meta))), (String::from("resolved"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (& (item)
                    .thread).resolved))), (String::from("comment_count"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (& (item)
                    .thread).comment_count))), (String::from("comments"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (& (item).thread)
                    .comments).iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("PageComment"), fields :
                    ::std::vec![(String::from("id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).id))), (String::from("ordinal"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .ordinal))), (String::from("author"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).author))), (String::from("meta"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).meta))), (String::from("text"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).text)))] }).collect()))] }), (String::from("anchor"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).anchor)))] }).collect())), (String::from("threads_loading"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .threads_loading))), (String::from("commented_hits"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self
                    .commented_hits).iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(item)))
                    .collect())), (String::from("reply_thread"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.reply_thread))), (String::from("expanded_threads"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self
                    .expanded_threads).iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(item)))
                    .collect())), (String::from("resolved_open"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .resolved_open))), (String::from("page_draft"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.page_draft))), (String::from("page_search_draft"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.page_search_draft))), (String::from("block_comment_draft"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.block_comment_draft))), (String::from("reply_draft"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.reply_draft))), (String::from("pending_page"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.pending_page))), (String::from("pending_comment"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.pending_comment))), (String::from("page_saved_text"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.page_saved_text))), (String::from("buffer_page"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.buffer_page))), (String::from("page_inflight_text"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.page_inflight_text))), (String::from("sent"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self.sent)))
                ],
            },
        }
            .encode()
    }
    pub(crate) fn restore(bytes: &[u8]) -> Result<Self, String> {
        let snapshot = ::ducktape_view_guest::wire::Snapshot::decode(bytes)?;
        if snapshot.schema != Self::SNAPSHOT_SCHEMA {
            return Err(String::from("snapshot schema mismatch"));
        }
        let value = snapshot.state;
        ((|| {
            let ::ducktape_view_guest::wire::SnapshotValue::Record {
                name: name,
                fields: fields,
            } = value else {
                return None;
            };
            if name != "PagesView" || fields.len() != 64 {
                return None;
            }
            let mut fields = fields.into_iter();
            let (name, value) = fields.next()?;
            if name != "pointer_y" {
                return None;
            }
            let pointer_y: f64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::F64(
                    item,
                ) if item.is_finite() => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "comment_anchor_y" {
                return None;
            }
            let comment_anchor_y: f64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::F64(
                    item,
                ) if item.is_finite() => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "comment_anchor_line" {
                return None;
            }
            let comment_anchor_line: i64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "comments_card_height" {
                return None;
            }
            let comments_card_height: f64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::F64(
                    item,
                ) if item.is_finite() => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "document_reserve" {
                return None;
            }
            let document_reserve: crate::editor_view::EditorReserve = ((|| {
                let ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name: name,
                    fields: fields,
                } = value else {
                    return None;
                };
                if name != "EditorReserve" || fields.len() != 2 {
                    return None;
                }
                let mut fields = fields.into_iter();
                let (name, field_0) = fields.next()?;
                if name != "line" {
                    return None;
                }
                let (name, field_1) = fields.next()?;
                if name != "height" {
                    return None;
                }
                Some(crate::editor_view::EditorReserve {
                    line: (match field_0 {
                        ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    height: (match field_1 {
                        ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                })
            })())?;
            let (name, value) = fields.next()?;
            if name != "document_focused" {
                return None;
            }
            let document_focused: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "focus_query" {
                return None;
            }
            let focus_query: i64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "document_paint" {
                return None;
            }
            let document_paint: crate::editor_view::PreparedPresentation = ((|| {
                let ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name: name,
                    fields: fields,
                } = value else {
                    return None;
                };
                if name != "PreparedPresentation" || fields.len() != 3 {
                    return None;
                }
                let mut fields = fields.into_iter();
                let (name, field_0) = fields.next()?;
                if name != "reference" {
                    return None;
                }
                let (name, field_1) = fields.next()?;
                if name != "data" {
                    return None;
                }
                let (name, field_2) = fields.next()?;
                if name != "notice" {
                    return None;
                }
                Some(crate::editor_view::PreparedPresentation {
                    reference: (match field_0 {
                        ::ducktape_view_guest::wire::SnapshotValue::Bytes(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    data: (match field_1 {
                        ::ducktape_view_guest::wire::SnapshotValue::Bytes(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                    notice: (match field_2 {
                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                })
            })())?;
            let (name, value) = fields.next()?;
            if name != "document" {
                return None;
            }
            let document: ::ducktape_view_guest::Editor = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bytes(bytes) => {
                    ::ducktape_view_guest::Editor::restore(&bytes)
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "document_history" {
                return None;
            }
            let document_history: crate::editor_binding::HistoryState = ((|| {
                let ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name: name,
                    fields: fields,
                } = value else {
                    return None;
                };
                if name != "HistoryState" || fields.len() != 1 {
                    return None;
                }
                let mut fields = fields.into_iter();
                let (name, field_0) = fields.next()?;
                if name != "snapshot" {
                    return None;
                }
                Some(crate::editor_binding::HistoryState {
                    snapshot: (match field_0 {
                        ::ducktape_view_guest::wire::SnapshotValue::Bytes(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                })
            })())?;
            let (name, value) = fields.next()?;
            if name != "document_menu" {
                return None;
            }
            let document_menu: crate::editor_binding::MenuState = ((|| {
                let ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name: name,
                    fields: fields,
                } = value else {
                    return None;
                };
                if name != "MenuState" || fields.len() != 1 {
                    return None;
                }
                let mut fields = fields.into_iter();
                let (name, field_0) = fields.next()?;
                if name != "snapshot" {
                    return None;
                }
                Some(crate::editor_binding::MenuState {
                    snapshot: (match field_0 {
                        ::ducktape_view_guest::wire::SnapshotValue::Bytes(item) => {
                            Some(item)
                        }
                        _ => None,
                    })?,
                })
            })())?;
            let (name, value) = fields.next()?;
            if name != "document_error" {
                return None;
            }
            let document_error: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "document_dark" {
                return None;
            }
            let document_dark: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "document_commented" {
                return None;
            }
            let document_commented: Vec<i64> = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| match item {
                            ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                                Some(item)
                            }
                            _ => None,
                        })
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "document_marks" {
                return None;
            }
            let document_marks: Vec<crate::document_sync::CommentMark> = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| (|| {
                            let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                name: name,
                                fields: fields,
                            } = item else {
                                return None;
                            };
                            if name != "CommentMark" || fields.len() != 2 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "line" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "count" {
                                return None;
                            }
                            Some(crate::document_sync::CommentMark {
                                line: (match field_0 {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                count: (match field_1 {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                            })
                        })())
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "active_palette" {
                return None;
            }
            let active_palette: AppTheme = ((|| {
                let ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name: name,
                    fields: fields,
                } = value else {
                    return None;
                };
                if name != "AppTheme" || fields.len() != 1 {
                    return None;
                }
                let (variant, payload) = fields.into_iter().next()?;
                match variant.as_str() {
                    "app" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(AppTheme::App)
                    }
                    "app_dark" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(AppTheme::AppDark)
                    }
                    _ => None,
                }
            })())?;
            let (name, value) = fields.next()?;
            if name != "connected" {
                return None;
            }
            let connected: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "chain" {
                return None;
            }
            let chain: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "route_serial" {
                return None;
            }
            let route_serial: i64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "register_serial" {
                return None;
            }
            let register_serial: i64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "loading" {
                return None;
            }
            let loading: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "busy" {
                return None;
            }
            let busy: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "host_error" {
                return None;
            }
            let host_error: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "page_link" {
                return None;
            }
            let page_link: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "pages" {
                return None;
            }
            let pages: Vec<crate::host::PageItem> = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| (|| {
                            let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                name: name,
                                fields: fields,
                            } = item else {
                                return None;
                            };
                            if name != "PageItem" || fields.len() != 5 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "id" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "title" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "parent" {
                                return None;
                            }
                            let (name, field_3) = fields.next()?;
                            if name != "prefix" {
                                return None;
                            }
                            let (name, field_4) = fields.next()?;
                            if name != "child_count" {
                                return None;
                            }
                            Some(crate::host::PageItem {
                                id: (match field_0 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                title: (match field_1 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                parent: (match field_2 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                prefix: (match field_3 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                child_count: (match field_4 {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                            })
                        })())
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "blocks" {
                return None;
            }
            let blocks: Vec<crate::document_sync::PageBlock> = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| (|| {
                            let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                name: name,
                                fields: fields,
                            } = item else {
                                return None;
                            };
                            if name != "PageBlock" || fields.len() != 8 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "key" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "id" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "parent" {
                                return None;
                            }
                            let (name, field_3) = fields.next()?;
                            if name != "kind" {
                                return None;
                            }
                            let (name, field_4) = fields.next()?;
                            if name != "text" {
                                return None;
                            }
                            let (name, field_5) = fields.next()?;
                            if name != "checked" {
                                return None;
                            }
                            let (name, field_6) = fields.next()?;
                            if name != "prefix" {
                                return None;
                            }
                            let (name, field_7) = fields.next()?;
                            if name != "child_count" {
                                return None;
                            }
                            Some(crate::document_sync::PageBlock {
                                key: (match field_0 {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                id: (match field_1 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                parent: (match field_2 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                kind: (match field_3 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                text: (match field_4 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                checked: (match field_5 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                prefix: (match field_6 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                child_count: (match field_7 {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                            })
                        })())
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "pages_viewport_width" {
                return None;
            }
            let pages_viewport_width: f64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::F64(
                    item,
                ) if item.is_finite() => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "pages_viewport_height" {
                return None;
            }
            let pages_viewport_height: f64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::F64(
                    item,
                ) if item.is_finite() => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "pages_pane_width" {
                return None;
            }
            let pages_pane_width: f64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::F64(
                    item,
                ) if item.is_finite() => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "sidebar_width" {
                return None;
            }
            let sidebar_width: f64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::F64(
                    item,
                ) if item.is_finite() => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "page_menu_open" {
                return None;
            }
            let page_menu_open: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "page_create_open" {
                return None;
            }
            let page_create_open: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "active_page" {
                return None;
            }
            let active_page: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "active_page_title" {
                return None;
            }
            let active_page_title: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "active_page_parent" {
                return None;
            }
            let active_page_parent: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "page_searching" {
                return None;
            }
            let page_searching: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "page_search_hits" {
                return None;
            }
            let page_search_hits: Vec<crate::host::PageSearchHit> = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| (|| {
                            let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                name: name,
                                fields: fields,
                            } = item else {
                                return None;
                            };
                            if name != "PageSearchHit" || fields.len() != 5 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "page_id" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "page_title" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "block_id" {
                                return None;
                            }
                            let (name, field_3) = fields.next()?;
                            if name != "kind" {
                                return None;
                            }
                            let (name, field_4) = fields.next()?;
                            if name != "text" {
                                return None;
                            }
                            Some(crate::host::PageSearchHit {
                                page_id: (match field_0 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                page_title: (match field_1 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                block_id: (match field_2 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                kind: (match field_3 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                text: (match field_4 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                            })
                        })())
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "page_search_query" {
                return None;
            }
            let page_search_query: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "page_search_serial" {
                return None;
            }
            let page_search_serial: i64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "page_delete_armed" {
                return None;
            }
            let page_delete_armed: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "autosave" {
                return None;
            }
            let autosave: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "page_refusal" {
                return None;
            }
            let page_refusal: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "subpages" {
                return None;
            }
            let subpages: Vec<crate::host::Subpage> = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| (|| {
                            let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                name: name,
                                fields: fields,
                            } = item else {
                                return None;
                            };
                            if name != "Subpage" || fields.len() != 2 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "id" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "title" {
                                return None;
                            }
                            Some(crate::host::Subpage {
                                id: (match field_0 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                title: (match field_1 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                            })
                        })())
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "orphaned_comment_drafts" {
                return None;
            }
            let orphaned_comment_drafts: Vec<String> = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| match item {
                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                Some(item)
                            }
                            _ => None,
                        })
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "block_comments_open" {
                return None;
            }
            let block_comments_open: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "scope_target" {
                return None;
            }
            let scope_target: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "scope_pinned" {
                return None;
            }
            let scope_pinned: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "thread_total" {
                return None;
            }
            let thread_total: i64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "comment_rows" {
                return None;
            }
            let comment_rows: Vec<crate::host::PageCommentThreadRow> = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| (|| {
                            let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                name: name,
                                fields: fields,
                            } = item else {
                                return None;
                            };
                            if name != "PageCommentThreadRow" || fields.len() != 2 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "thread" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "anchor" {
                                return None;
                            }
                            Some(crate::host::PageCommentThreadRow {
                                thread: ((|| {
                                    let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                        name: name,
                                        fields: fields,
                                    } = field_0 else {
                                        return None;
                                    };
                                    if name != "PageCommentThread" || fields.len() != 7 {
                                        return None;
                                    }
                                    let mut fields = fields.into_iter();
                                    let (name, field_0) = fields.next()?;
                                    if name != "id" {
                                        return None;
                                    }
                                    let (name, field_1) = fields.next()?;
                                    if name != "target" {
                                        return None;
                                    }
                                    let (name, field_2) = fields.next()?;
                                    if name != "author" {
                                        return None;
                                    }
                                    let (name, field_3) = fields.next()?;
                                    if name != "meta" {
                                        return None;
                                    }
                                    let (name, field_4) = fields.next()?;
                                    if name != "resolved" {
                                        return None;
                                    }
                                    let (name, field_5) = fields.next()?;
                                    if name != "comment_count" {
                                        return None;
                                    }
                                    let (name, field_6) = fields.next()?;
                                    if name != "comments" {
                                        return None;
                                    }
                                    Some(crate::host::PageCommentThread {
                                        id: (match field_0 {
                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                Some(item)
                                            }
                                            _ => None,
                                        })?,
                                        target: (match field_1 {
                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                Some(item)
                                            }
                                            _ => None,
                                        })?,
                                        author: (match field_2 {
                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                Some(item)
                                            }
                                            _ => None,
                                        })?,
                                        meta: (match field_3 {
                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                Some(item)
                                            }
                                            _ => None,
                                        })?,
                                        resolved: (match field_4 {
                                            ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => {
                                                Some(item)
                                            }
                                            _ => None,
                                        })?,
                                        comment_count: (match field_5 {
                                            ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                                                Some(item)
                                            }
                                            _ => None,
                                        })?,
                                        comments: (match field_6 {
                                            ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                                                items
                                                    .into_iter()
                                                    .map(|item| (|| {
                                                        let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                                            name: name,
                                                            fields: fields,
                                                        } = item else {
                                                            return None;
                                                        };
                                                        if name != "PageComment" || fields.len() != 5 {
                                                            return None;
                                                        }
                                                        let mut fields = fields.into_iter();
                                                        let (name, field_0) = fields.next()?;
                                                        if name != "id" {
                                                            return None;
                                                        }
                                                        let (name, field_1) = fields.next()?;
                                                        if name != "ordinal" {
                                                            return None;
                                                        }
                                                        let (name, field_2) = fields.next()?;
                                                        if name != "author" {
                                                            return None;
                                                        }
                                                        let (name, field_3) = fields.next()?;
                                                        if name != "meta" {
                                                            return None;
                                                        }
                                                        let (name, field_4) = fields.next()?;
                                                        if name != "text" {
                                                            return None;
                                                        }
                                                        Some(crate::host::PageComment {
                                                            id: (match field_0 {
                                                                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                    Some(item)
                                                                }
                                                                _ => None,
                                                            })?,
                                                            ordinal: (match field_1 {
                                                                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                                                                    Some(item)
                                                                }
                                                                _ => None,
                                                            })?,
                                                            author: (match field_2 {
                                                                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                    Some(item)
                                                                }
                                                                _ => None,
                                                            })?,
                                                            meta: (match field_3 {
                                                                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                    Some(item)
                                                                }
                                                                _ => None,
                                                            })?,
                                                            text: (match field_4 {
                                                                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                    Some(item)
                                                                }
                                                                _ => None,
                                                            })?,
                                                        })
                                                    })())
                                                    .collect::<Option<Vec<_>>>()
                                            }
                                            _ => None,
                                        })?,
                                    })
                                })())?,
                                anchor: (match field_1 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                            })
                        })())
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "threads_loading" {
                return None;
            }
            let threads_loading: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "commented_hits" {
                return None;
            }
            let commented_hits: Vec<String> = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| match item {
                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                Some(item)
                            }
                            _ => None,
                        })
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "reply_thread" {
                return None;
            }
            let reply_thread: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "expanded_threads" {
                return None;
            }
            let expanded_threads: Vec<String> = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| match item {
                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                Some(item)
                            }
                            _ => None,
                        })
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "resolved_open" {
                return None;
            }
            let resolved_open: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "page_draft" {
                return None;
            }
            let page_draft: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "page_search_draft" {
                return None;
            }
            let page_search_draft: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "block_comment_draft" {
                return None;
            }
            let block_comment_draft: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "reply_draft" {
                return None;
            }
            let reply_draft: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "pending_page" {
                return None;
            }
            let pending_page: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "pending_comment" {
                return None;
            }
            let pending_comment: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "page_saved_text" {
                return None;
            }
            let page_saved_text: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "buffer_page" {
                return None;
            }
            let buffer_page: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "page_inflight_text" {
                return None;
            }
            let page_inflight_text: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "sent" {
                return None;
            }
            let sent: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            Some(
                Self { pointer_y,
                    comment_anchor_y,
                    comment_anchor_line,
                    comments_card_height,
                    document_reserve,
                    document_focused,
                    focus_query,
                    document_paint,
                    document,
                    document_history,
                    document_menu,
                    document_error,
                    document_dark,
                    document_commented,
                    document_marks,
                    active_palette,
                    connected,
                    chain,
                    route_serial,
                    register_serial,
                    loading,
                    busy,
                    host_error,
                    page_link,
                    pages,
                    blocks,
                    pages_viewport_width,
                    pages_viewport_height,
                    pages_pane_width,
                    sidebar_width,
                    page_menu_open,
                    page_create_open,
                    active_page,
                    active_page_title,
                    active_page_parent,
                    page_searching,
                    page_search_hits,
                    page_search_query,
                    page_search_serial,
                    page_delete_armed,
                    autosave,
                    page_refusal,
                    subpages,
                    orphaned_comment_drafts,
                    block_comments_open,
                    scope_target,
                    scope_pinned,
                    thread_total,
                    comment_rows,
                    threads_loading,
                    commented_hits,
                    reply_thread,
                    expanded_threads,
                    resolved_open,
                    page_draft,
                    page_search_draft,
                    block_comment_draft,
                    reply_draft,
                    pending_page,
                    pending_comment,
                    page_saved_text,
                    buffer_page,
                    page_inflight_text,
                    sent,
                 },
            )
        })())
            .ok_or_else(|| String::from("snapshot state mismatch"))
    }
}
#[allow(unused_parens)]
impl PagesView {
    fn subscription(&self) -> ::ducktape_view_guest::Subscription<Message> {
        ::ducktape_view_guest::Subscription::batch([
            ::ducktape_view_guest::mouse::observe(
                ::ducktape_view_guest::Subscription::filter_events(|event| match event {
                    ::ducktape_view_guest::wire::Event::Mouse {
                        event: ::ducktape_view_guest::wire::mouse::Event::CursorMoved { x, y },
                        ..
                    } => Some(Message::CommentPointerMoved(*x as f64, *y as f64)),
                    _ => None,
                }),
            ),
            ::ducktape_view_guest::mouse::observe(
                ::ducktape_view_guest::Subscription::filter_events(|event| match event {
                    ::ducktape_view_guest::wire::Event::Mouse {
                        event: ::ducktape_view_guest::wire::mouse::Event::ButtonReleased(button),
                        ..
                    } => Some(Message::DocumentPointerReleased(*button)),
                    _ => None,
                }),
            ),
            ::ducktape_view_guest::Subscription::filter_events(|event| match event {
                ::ducktape_view_guest::wire::Event::Keyboard {
                    event: ::ducktape_view_guest::wire::keyboard::Event::Release(key),
                    ..
                } => Some(Message::DocumentKeyReleased(key.clone())),
                _ => None,
            }),
            ::ducktape_view_guest::events::observe(
                ::ducktape_view_guest::Subscription::filter_events(|event| match event {
                    ::ducktape_view_guest::wire::Event::Observation {
                        event:
                            ::ducktape_view_guest::wire::events::Event::Window(
                                ::ducktape_view_guest::wire::events::Window::Focused,
                            ),
                        ..
                    } => Some(Message::DocumentWindowFocused),
                    _ => None,
                }),
                ::ducktape_view_guest::wire::events::Interest {
                    focus: true,
                    ..Default::default()
                },
            ),
            ::ducktape_view_guest::events::observe(
                ::ducktape_view_guest::Subscription::filter_events(|event| match event {
                    ::ducktape_view_guest::wire::Event::Observation {
                        event:
                            ::ducktape_view_guest::wire::events::Event::Window(
                                ::ducktape_view_guest::wire::events::Window::Unfocused,
                            ),
                        ..
                    } => Some(Message::DocumentWindowUnfocused),
                    _ => None,
                }),
                ::ducktape_view_guest::wire::events::Interest {
                    focus: true,
                    ..Default::default()
                },
            ),
            crate::host::session().map(move |value| Message::SessionArrived(value)),
            if self.connected {
                ::ducktape_view_guest::Subscription::batch([crate::host::register(
                    self.active_page.to_owned(),
                    self.register_serial,
                )
                .map(move |value| Message::RegisterArrived(value))])
            } else {
                ::ducktape_view_guest::Subscription::none()
            },
            if (self.connected && (!(self.page_search_query).is_empty())) {
                ::ducktape_view_guest::Subscription::batch([crate::host::search(
                    self.page_search_query.to_owned(),
                    self.page_search_serial,
                )
                .map(move |value| Message::SearchArrived(value))])
            } else {
                ::ducktape_view_guest::Subscription::none()
            },
            crate::host::acts().map(move |value| Message::ActDone(value)),
            crate::host::saves().map(move |value| Message::SaveDone(value)),
            if ((((self.connected && (!self.loading)) && (!self.busy))
                && (!(self.active_page).is_empty()))
                && (self.active_page == self.buffer_page))
            {
                ::ducktape_view_guest::Subscription::batch([::ducktape_view_guest::every(
                    ::std::time::Duration::from_millis(900),
                )
                .map(move |value| Message::PageAutosaveTick)])
            } else {
                ::ducktape_view_guest::Subscription::none()
            },
        ])
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_state_preserves_document_and_drafts_through_snapshot() {
        let (mut app, _) = PagesView::boot();
        app.document = ducktape_view_guest::Editor::new("# 제목\n\nDocument 🙂");
        app.page_draft = "새 페이지".into();
        app.block_comment_draft = "Unsent comment".into();
        app.reply_draft = "Unsent reply".into();
        app.expanded_threads = vec!["thread-a".into()];
        app.pages_pane_width = 1060.;
        let snapshot = app.snapshot().unwrap();
        let restored = PagesView::restore(&snapshot).unwrap();
        assert_eq!(restored.snapshot().unwrap(), snapshot);
    }

    #[test]
    fn measured_pane_changes_the_guest_document_width() {
        let (mut app, _) = PagesView::boot();
        app.connected = true;
        app.active_page = "page".into();
        app.block_comments_open = true;
        let _ = app.update(Message::PagesPaneResized(1060., 700.));
        assert_eq!(app.pages_pane_width, 1060.);
        let mut constrained = false;
        app.view().for_each_mut(&mut |node| {
            if let ducktape_view_guest::wire::Node::Container { max_width: Some(width), .. } = node {
                constrained |= *width == 688.;
            }
        });
        assert!(constrained, "the guest must publish the measured squeeze width");
    }

    #[test]
    fn view_fits_default_stack() {
        ::std::thread::Builder::new()
            .stack_size(4 * 1024 * 1024)
            .spawn(|| {
                let (app, _) = PagesView::boot();
                let _ = app.view();
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
include!("app_update.rs");
include!("app_view.rs");
include!("icon.rs");
include!("kit.rs");
include!("pages.rs");
include!("rows.rs");
