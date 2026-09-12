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
pub(crate) enum Appearance {
    Light,
    Dark,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Act {
    Review,
    Merge,
}
#[allow(dead_code)]
pub struct ForgeView {
    pub(crate) active_palette: AppTheme,
    pub(crate) connected: bool,
    pub(crate) dark: bool,
    pub(crate) org: String,
    pub(crate) about: String,
    pub(crate) tier: String,
    pub(crate) network_chain_id: String,
    pub(crate) connected_rpc: String,
    pub(crate) connection_serial: i64,
    pub(crate) link_tick: i64,
    pub(crate) repos: Vec<crate::host::ForgeRepo>,
    pub(crate) list_phase: String,
    pub(crate) open_repo: String,
    pub(crate) repo_phase: String,
    pub(crate) branches: Vec<crate::host::ForgeBranch>,
    pub(crate) items: Vec<crate::host::ForgeItem>,
    pub(crate) tab: String,
    pub(crate) forge_item_number: i64,
    pub(crate) item_phase: String,
    pub(crate) forge_item_kind: String,
    pub(crate) forge_item_title: String,
    pub(crate) forge_item_state: String,
    pub(crate) forge_item_author: String,
    pub(crate) forge_item_branches: String,
    pub(crate) forge_item_body: String,
    pub(crate) forge_item_blocks: Vec<crate::host::ChatBlock>,
    pub(crate) forge_item_files_changed: i64,
    pub(crate) forge_item_additions: i64,
    pub(crate) forge_item_deletions: i64,
    pub(crate) diff_rows: Vec<crate::host::DiffLine>,
    pub(crate) forge_item_diff_truncated: bool,
    pub(crate) forge_item_merge_oid: String,
    pub(crate) forge_item_source_branch: String,
    pub(crate) forge_item_source_oid: String,
    pub(crate) forge_item_target_oid: String,
    pub(crate) forge_item_channel: String,
    pub(crate) forge_item_reviews: Vec<crate::host::ForgeReview>,
    pub(crate) forge_item_approvals: i64,
    pub(crate) forge_item_change_requests: i64,
    pub(crate) discussion: Vec<crate::host::ChatMessage>,
    pub(crate) discussion_clipped: bool,
    pub(crate) linked_note: Vec<crate::host::ChatMessage>,
    pub(crate) roster_set: bool,
    pub(crate) focus_seq: i64,
    pub(crate) landed_tick: i64,
    pub(crate) focus_number: i64,
    pub(crate) merge_conflicts: Vec<String>,
    pub(crate) merge_busy: bool,
    pub(crate) review_verdict: String,
    pub(crate) review_busy: bool,
    pub(crate) staged_comments: Vec<crate::host::ForgeDraftComment>,
    pub(crate) tree_pick: String,
    pub(crate) tree_path: String,
    pub(crate) tree_rev: String,
    pub(crate) tree_entries: Vec<crate::host::TreeEntry>,
    pub(crate) tree_born: bool,
    pub(crate) tree_truncated: bool,
    pub(crate) tree_phase: String,
    pub(crate) file_path: String,
    pub(crate) file_text: String,
    pub(crate) file_binary: bool,
    pub(crate) file_truncated: bool,
    pub(crate) file_picture: bool,
    pub(crate) file_width: i64,
    pub(crate) file_height: i64,
    pub(crate) file_note: String,
    pub(crate) file_phase: String,
    pub(crate) opened_dir: String,
    pub(crate) opened_rev: String,
    pub(crate) focus_path: String,
    pub(crate) focus_rev: String,
    pub(crate) review_draft: String,
    pub(crate) comment_draft: String,
    pub(crate) comment_path: String,
    pub(crate) comment_line: String,
    pub(crate) comment_side: String,
    pub(crate) host_error: String,
    pub(crate) sent: bool,
    pub(crate) viewport_width: f64,
    pub(crate) tree_width: f64,
    pub(crate) file_text_revision: u64,
}
impl ::std::fmt::Debug for ForgeView {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.write_str("ForgeView")
    }
}
#[derive(Clone)]
pub enum Message {
    SessionArrived(crate::host::SessionItem),
    ForgeLandLink(String),
    ReposArrived(crate::host::RepoListItem),
    RepoArrived(crate::host::RepoItem),
    ItemArrived(crate::host::ItemItem),
    DiscussionArrived(crate::host::DiscussionItem),
    TreeArrived(crate::host::TreeItem),
    BlobArrived(crate::host::BlobItem),
    ActDone(crate::host::ActItem),
    ForgeOpenRepo(String),
    ForgeCloseRepo,
    ForgePickBranch(String),
    ForgeOpenDir(String),
    ForgeOpenFile(String),
    ForgeOpenItem(i64),
    ForgeCloseItem,
    SelectForgeTab(String),
    ForgeReviewPick(String),
    ForgeReviewSubmit(String),
    ForgeMergeSubmit,
    ForgeCommentOpen(String, String, String),
    ForgeCommentCancel,
    ForgeCommentStage(String),
    ForgeCommentDrop(String),
    OpenMessageLink(String),
    CopyToClipboard(String, String),
    TreeResized(f64, f64),
    ViewportChanged(f64, f64),
    CommentDraftChanged(String),
    ReviewDraftChanged(String),
    Ignore,
}
impl ::std::fmt::Debug for Message {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.write_str("Message")
    }
}
#[allow(unused_parens)]
impl ForgeView {
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
                        1.0,
                        1.0,
                        1.0,
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
                        1.0,
                        1.0,
                        1.0,
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
                        1.0,
                        1.0,
                        1.0,
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
                        1.0,
                        1.0,
                        1.0,
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
                        1.0,
                        1.0,
                        1.0,
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
    fn title(&self) -> String {
        "Forge".to_owned()
    }
}
#[allow(unused_parens)]
impl ForgeView {
    fn state() -> Self {
        Self {
            active_palette: AppTheme::App,
            connected: false,
            dark: false,
            org: "".to_owned(),
            about: "".to_owned(),
            tier: "".to_owned(),
            network_chain_id: "".to_owned(),
            connected_rpc: "".to_owned(),
            connection_serial: 0,
            link_tick: 0,
            repos: Vec::new(),
            list_phase: "idle".to_owned(),
            open_repo: "".to_owned(),
            repo_phase: "idle".to_owned(),
            branches: Vec::new(),
            items: Vec::new(),
            tab: "code".to_owned(),
            forge_item_number: 0,
            item_phase: "idle".to_owned(),
            forge_item_kind: "".to_owned(),
            forge_item_title: "".to_owned(),
            forge_item_state: "".to_owned(),
            forge_item_author: "".to_owned(),
            forge_item_branches: "".to_owned(),
            forge_item_body: "".to_owned(),
            forge_item_blocks: Vec::new(),
            forge_item_files_changed: 0,
            forge_item_additions: 0,
            forge_item_deletions: 0,
            diff_rows: Vec::new(),
            forge_item_diff_truncated: false,
            forge_item_merge_oid: "".to_owned(),
            forge_item_source_branch: "".to_owned(),
            forge_item_source_oid: "".to_owned(),
            forge_item_target_oid: "".to_owned(),
            forge_item_channel: "".to_owned(),
            forge_item_reviews: Vec::new(),
            forge_item_approvals: 0,
            forge_item_change_requests: 0,
            discussion: Vec::new(),
            discussion_clipped: false,
            linked_note: Vec::new(),
            roster_set: false,
            focus_seq: 0,
            landed_tick: 0,
            focus_number: 0,
            merge_conflicts: Vec::new(),
            merge_busy: false,
            review_verdict: "comment".to_owned(),
            review_busy: false,
            staged_comments: Vec::new(),
            tree_pick: "".to_owned(),
            tree_path: "".to_owned(),
            tree_rev: "".to_owned(),
            tree_entries: Vec::new(),
            tree_born: false,
            tree_truncated: false,
            tree_phase: "loading".to_owned(),
            file_path: "".to_owned(),
            file_text: "".to_owned(),
            file_binary: false,
            file_truncated: false,
            file_picture: false,
            file_width: 0,
            file_height: 0,
            file_note: "".to_owned(),
            file_phase: "idle".to_owned(),
            opened_dir: "".to_owned(),
            opened_rev: "".to_owned(),
            focus_path: "".to_owned(),
            focus_rev: "".to_owned(),
            review_draft: "".to_owned(),
            comment_draft: "".to_owned(),
            comment_path: "".to_owned(),
            comment_line: "".to_owned(),
            comment_side: "".to_owned(),
            host_error: "".to_owned(),
            sent: false,
            viewport_width: 1280.0,
            tree_width: 258.0,
            file_text_revision: ::ducktape_view_guest::rev::seed(),
        }
    }
    pub(crate) fn boot() -> (Self, ::ducktape_view_guest::Task<Message>) {
        (Self::state(), ::ducktape_view_guest::Task::none())
    }
    pub(crate) const PREFERRED_WINDOW_SIZE: &'static str = "none";
    pub(crate) const SNAPSHOT_SCHEMA: &'static str =
        "6f5fe2a551e819c3aa46ee5398c1e2d53f84179b6f029050f43d932320cb977c";
    pub(crate) fn snapshot(&self) -> Result<Vec<u8>, String> {
        ::ducktape_view_guest::wire::Snapshot {
            schema: String::from(Self::SNAPSHOT_SCHEMA),
            state: ::ducktape_view_guest::wire::SnapshotValue::Record {
                name: String::from("ForgeView"),
                fields: vec![
                    (String::from("active_palette"), match & self.active_palette {
                    AppTheme::App => ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name : String::from("AppTheme"), fields : vec![(String::from("app"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    AppTheme::AppDark =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("AppTheme"), fields : vec![(String::from("app_dark"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] } }),
                    (String::from("connected"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .connected))), (String::from("dark"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self.dark))),
                    (String::from("org"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.org))), (String::from("about"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.about))), (String::from("tier"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.tier))), (String::from("network_chain_id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.network_chain_id))), (String::from("connected_rpc"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.connected_rpc))), (String::from("connection_serial"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .connection_serial))), (String::from("link_tick"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .link_tick))), (String::from("repos"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self.repos)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ForgeRepo"), fields :
                    ::std::vec![(String::from("name"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).name))), (String::from("head"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).head)))] }).collect())), (String::from("list_phase"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.list_phase))), (String::from("open_repo"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.open_repo))), (String::from("repo_phase"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.repo_phase))), (String::from("branches"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self.branches)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ForgeBranch"), fields :
                    ::std::vec![(String::from("name"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).name))), (String::from("head"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).head)))] }).collect())), (String::from("items"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self.items)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ForgeItem"), fields :
                    ::std::vec![(String::from("number"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .number))), (String::from("kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).kind))), (String::from("state"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).state))), (String::from("title"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).title))), (String::from("author"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).author))), (String::from("author_name"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).author_name)))] }).collect())), (String::from("tab"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.tab))), (String::from("forge_item_number"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .forge_item_number))), (String::from("item_phase"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.item_phase))), (String::from("forge_item_kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.forge_item_kind))), (String::from("forge_item_title"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.forge_item_title))), (String::from("forge_item_state"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.forge_item_state))), (String::from("forge_item_author"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.forge_item_author))), (String::from("forge_item_branches"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.forge_item_branches))), (String::from("forge_item_body"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.forge_item_body))), (String::from("forge_item_blocks"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self
                    .forge_item_blocks).iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatBlock"), fields :
                    ::std::vec![(String::from("kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).kind))), (String::from("text"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).text))), (String::from("lang"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).lang))), (String::from("rich"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item).rich))),
                    (String::from("spans"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (item).spans)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatSpan"), fields :
                    ::std::vec![(String::from("mention"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).mention))), (String::from("mention_link"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).mention_link))), (String::from("link_text"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).link_text))), (String::from("link"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).link))), (String::from("bold_italic"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).bold_italic))), (String::from("bold"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).bold))), (String::from("italic"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).italic))), (String::from("plain"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).plain)))] }).collect()))] }).collect())),
                    (String::from("forge_item_files_changed"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .forge_item_files_changed))), (String::from("forge_item_additions"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .forge_item_additions))), (String::from("forge_item_deletions"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .forge_item_deletions))), (String::from("diff_rows"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self.diff_rows)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("DiffLine"), fields : ::std::vec![(String::from("key"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item).key))),
                    (String::from("kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).kind))), (String::from("old_no"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).old_no))), (String::from("new_no"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).new_no))), (String::from("sign"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).sign))), (String::from("text"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).text))), (String::from("path"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).path))), (String::from("side"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).side)))] }).collect())),
                    (String::from("forge_item_diff_truncated"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .forge_item_diff_truncated))), (String::from("forge_item_merge_oid"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.forge_item_merge_oid))),
                    (String::from("forge_item_source_branch"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.forge_item_source_branch))),
                    (String::from("forge_item_source_oid"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.forge_item_source_oid))),
                    (String::from("forge_item_target_oid"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.forge_item_target_oid))), (String::from("forge_item_channel"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.forge_item_channel))), (String::from("forge_item_reviews"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self
                    .forge_item_reviews).iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ForgeReview"), fields :
                    ::std::vec![(String::from("author"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).author))), (String::from("author_name"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).author_name))), (String::from("verdict"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).verdict))), (String::from("body"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).body))), (String::from("blocks"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (item).blocks)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatBlock"), fields :
                    ::std::vec![(String::from("kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).kind))), (String::from("text"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).text))), (String::from("lang"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).lang))), (String::from("rich"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item).rich))),
                    (String::from("spans"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (item).spans)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatSpan"), fields :
                    ::std::vec![(String::from("mention"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).mention))), (String::from("mention_link"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).mention_link))), (String::from("link_text"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).link_text))), (String::from("link"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).link))), (String::from("bold_italic"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).bold_italic))), (String::from("bold"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).bold))), (String::from("italic"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).italic))), (String::from("plain"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).plain)))] }).collect()))] }).collect())),
                    (String::from("commit"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).commit))), (String::from("outdated"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item)
                    .outdated))), (String::from("created_at"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .created_at))), (String::from("comments"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (item).comments)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ForgeReviewComment"), fields :
                    ::std::vec![(String::from("anchor"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).anchor))), (String::from("body"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).body))), (String::from("blocks"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (item).blocks)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatBlock"), fields :
                    ::std::vec![(String::from("kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).kind))), (String::from("text"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).text))), (String::from("lang"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).lang))), (String::from("rich"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item).rich))),
                    (String::from("spans"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (item).spans)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatSpan"), fields :
                    ::std::vec![(String::from("mention"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).mention))), (String::from("mention_link"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).mention_link))), (String::from("link_text"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).link_text))), (String::from("link"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).link))), (String::from("bold_italic"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).bold_italic))), (String::from("bold"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).bold))), (String::from("italic"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).italic))), (String::from("plain"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).plain)))] }).collect()))] }).collect()))] }).collect()))] })
                    .collect())), (String::from("forge_item_approvals"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .forge_item_approvals))),
                    (String::from("forge_item_change_requests"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .forge_item_change_requests))), (String::from("discussion"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self.discussion)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatMessage"), fields :
                    ::std::vec![(String::from("seq"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item).seq))),
                    (String::from("author"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).author))), (String::from("meta"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).meta))), (String::from("blocks"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (item).blocks)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatBlock"), fields :
                    ::std::vec![(String::from("kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).kind))), (String::from("text"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).text))), (String::from("lang"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).lang))), (String::from("rich"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item).rich))),
                    (String::from("spans"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (item).spans)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatSpan"), fields :
                    ::std::vec![(String::from("mention"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).mention))), (String::from("mention_link"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).mention_link))), (String::from("link_text"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).link_text))), (String::from("link"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).link))), (String::from("bold_italic"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).bold_italic))), (String::from("bold"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).bold))), (String::from("italic"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).italic))), (String::from("plain"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).plain)))] }).collect()))] }).collect())),
                    (String::from("initial"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).initial))), (String::from("avatar_kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).avatar_kind))), (String::from("render_rev"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .render_rev)))] }).collect())), (String::from("discussion_clipped"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .discussion_clipped))), (String::from("linked_note"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self.linked_note)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatMessage"), fields :
                    ::std::vec![(String::from("seq"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item).seq))),
                    (String::from("author"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).author))), (String::from("meta"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).meta))), (String::from("blocks"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (item).blocks)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatBlock"), fields :
                    ::std::vec![(String::from("kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).kind))), (String::from("text"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).text))), (String::from("lang"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).lang))), (String::from("rich"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& (item).rich))),
                    (String::from("spans"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& (item).spans)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ChatSpan"), fields :
                    ::std::vec![(String::from("mention"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).mention))), (String::from("mention_link"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).mention_link))), (String::from("link_text"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).link_text))), (String::from("link"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).link))), (String::from("bold_italic"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).bold_italic))), (String::from("bold"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).bold))), (String::from("italic"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).italic))), (String::from("plain"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).plain)))] }).collect()))] }).collect())),
                    (String::from("initial"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).initial))), (String::from("avatar_kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).avatar_kind))), (String::from("render_rev"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .render_rev)))] }).collect())), (String::from("roster_set"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .roster_set))), (String::from("focus_seq"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .focus_seq))), (String::from("landed_tick"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .landed_tick))), (String::from("focus_number"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .focus_number))), (String::from("merge_conflicts"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self
                    .merge_conflicts).iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(item)))
                    .collect())), (String::from("merge_busy"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .merge_busy))), (String::from("review_verdict"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.review_verdict))), (String::from("review_busy"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .review_busy))), (String::from("staged_comments"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self
                    .staged_comments).iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("ForgeDraftComment"), fields :
                    ::std::vec![(String::from("anchor"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).anchor))), (String::from("path"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).path))), (String::from("line"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).line))), (String::from("side"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).side))), (String::from("body"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).body)))] }).collect())), (String::from("tree_pick"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.tree_pick))), (String::from("tree_path"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.tree_path))), (String::from("tree_rev"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.tree_rev))), (String::from("tree_entries"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self
                    .tree_entries).iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("TreeEntry"), fields :
                    ::std::vec![(String::from("name"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).name))), (String::from("path"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).path))), (String::from("kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).kind)))] }).collect())), (String::from("tree_born"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .tree_born))), (String::from("tree_truncated"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .tree_truncated))), (String::from("tree_phase"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.tree_phase))), (String::from("file_path"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.file_path))), (String::from("file_text"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.file_text))), (String::from("file_binary"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .file_binary))), (String::from("file_truncated"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .file_truncated))), (String::from("file_picture"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .file_picture))), (String::from("file_width"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .file_width))), (String::from("file_height"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .file_height))), (String::from("file_note"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.file_note))), (String::from("file_phase"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.file_phase))), (String::from("opened_dir"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.opened_dir))), (String::from("opened_rev"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.opened_rev))), (String::from("focus_path"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.focus_path))), (String::from("focus_rev"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.focus_rev))), (String::from("review_draft"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.review_draft))), (String::from("comment_draft"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.comment_draft))), (String::from("comment_path"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.comment_path))), (String::from("comment_line"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.comment_line))), (String::from("comment_side"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.comment_side))), (String::from("host_error"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.host_error))), (String::from("sent"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self.sent))),
                    (String::from("viewport_width"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& self
                    .viewport_width))), (String::from("tree_width"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& self
                    .tree_width)))
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
                name,
                fields,
            } = value else {
                return None;
            };
            if name != "ForgeView" || fields.len() != 80 {
                return None;
            }
            let mut fields = fields.into_iter();
            let (name, value) = fields.next()?;
            if name != "active_palette" {
                return None;
            }
            let active_palette: AppTheme = ((|| {
                let ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name,
                    fields,
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
            if name != "dark" {
                return None;
            }
            let dark: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "org" {
                return None;
            }
            let org: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "about" {
                return None;
            }
            let about: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "tier" {
                return None;
            }
            let tier: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "network_chain_id" {
                return None;
            }
            let network_chain_id: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "connected_rpc" {
                return None;
            }
            let connected_rpc: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "connection_serial" {
                return None;
            }
            let connection_serial: i64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "link_tick" {
                return None;
            }
            let link_tick: i64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "repos" {
                return None;
            }
            let repos: Vec<crate::host::ForgeRepo> = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| (|| {
                            let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                name,
                                fields,
                            } = item else {
                                return None;
                            };
                            if name != "ForgeRepo" || fields.len() != 2 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "name" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "head" {
                                return None;
                            }
                            Some(crate::host::ForgeRepo {
                                name: (match field_0 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                head: (match field_1 {
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
            if name != "list_phase" {
                return None;
            }
            let list_phase: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "open_repo" {
                return None;
            }
            let open_repo: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "repo_phase" {
                return None;
            }
            let repo_phase: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "branches" {
                return None;
            }
            let branches: Vec<crate::host::ForgeBranch> = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| (|| {
                            let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                name,
                                fields,
                            } = item else {
                                return None;
                            };
                            if name != "ForgeBranch" || fields.len() != 2 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "name" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "head" {
                                return None;
                            }
                            Some(crate::host::ForgeBranch {
                                name: (match field_0 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                head: (match field_1 {
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
            if name != "items" {
                return None;
            }
            let items: Vec<crate::host::ForgeItem> = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| (|| {
                            let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                name,
                                fields,
                            } = item else {
                                return None;
                            };
                            if name != "ForgeItem" || fields.len() != 6 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "number" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "kind" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "state" {
                                return None;
                            }
                            let (name, field_3) = fields.next()?;
                            if name != "title" {
                                return None;
                            }
                            let (name, field_4) = fields.next()?;
                            if name != "author" {
                                return None;
                            }
                            let (name, field_5) = fields.next()?;
                            if name != "author_name" {
                                return None;
                            }
                            Some(crate::host::ForgeItem {
                                number: (match field_0 {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                kind: (match field_1 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                state: (match field_2 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                title: (match field_3 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                author: (match field_4 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                author_name: (match field_5 {
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
            if name != "tab" {
                return None;
            }
            let tab: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "forge_item_number" {
                return None;
            }
            let forge_item_number: i64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "item_phase" {
                return None;
            }
            let item_phase: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "forge_item_kind" {
                return None;
            }
            let forge_item_kind: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "forge_item_title" {
                return None;
            }
            let forge_item_title: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "forge_item_state" {
                return None;
            }
            let forge_item_state: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "forge_item_author" {
                return None;
            }
            let forge_item_author: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "forge_item_branches" {
                return None;
            }
            let forge_item_branches: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "forge_item_body" {
                return None;
            }
            let forge_item_body: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "forge_item_blocks" {
                return None;
            }
            let forge_item_blocks: Vec<crate::host::ChatBlock> = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| (|| {
                            let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                name,
                                fields,
                            } = item else {
                                return None;
                            };
                            if name != "ChatBlock" || fields.len() != 5 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "kind" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "text" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "lang" {
                                return None;
                            }
                            let (name, field_3) = fields.next()?;
                            if name != "rich" {
                                return None;
                            }
                            let (name, field_4) = fields.next()?;
                            if name != "spans" {
                                return None;
                            }
                            Some(crate::host::ChatBlock {
                                kind: (match field_0 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                text: (match field_1 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                lang: (match field_2 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                rich: (match field_3 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                spans: (match field_4 {
                                    ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                                        items
                                            .into_iter()
                                            .map(|item| (|| {
                                                let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                                    name,
                                                    fields,
                                                } = item else {
                                                    return None;
                                                };
                                                if name != "ChatSpan" || fields.len() != 8 {
                                                    return None;
                                                }
                                                let mut fields = fields.into_iter();
                                                let (name, field_0) = fields.next()?;
                                                if name != "mention" {
                                                    return None;
                                                }
                                                let (name, field_1) = fields.next()?;
                                                if name != "mention_link" {
                                                    return None;
                                                }
                                                let (name, field_2) = fields.next()?;
                                                if name != "link_text" {
                                                    return None;
                                                }
                                                let (name, field_3) = fields.next()?;
                                                if name != "link" {
                                                    return None;
                                                }
                                                let (name, field_4) = fields.next()?;
                                                if name != "bold_italic" {
                                                    return None;
                                                }
                                                let (name, field_5) = fields.next()?;
                                                if name != "bold" {
                                                    return None;
                                                }
                                                let (name, field_6) = fields.next()?;
                                                if name != "italic" {
                                                    return None;
                                                }
                                                let (name, field_7) = fields.next()?;
                                                if name != "plain" {
                                                    return None;
                                                }
                                                Some(crate::host::ChatSpan {
                                                    mention: (match field_0 {
                                                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                            Some(item)
                                                        }
                                                        _ => None,
                                                    })?,
                                                    mention_link: (match field_1 {
                                                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                            Some(item)
                                                        }
                                                        _ => None,
                                                    })?,
                                                    link_text: (match field_2 {
                                                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                            Some(item)
                                                        }
                                                        _ => None,
                                                    })?,
                                                    link: (match field_3 {
                                                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                            Some(item)
                                                        }
                                                        _ => None,
                                                    })?,
                                                    bold_italic: (match field_4 {
                                                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                            Some(item)
                                                        }
                                                        _ => None,
                                                    })?,
                                                    bold: (match field_5 {
                                                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                            Some(item)
                                                        }
                                                        _ => None,
                                                    })?,
                                                    italic: (match field_6 {
                                                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                            Some(item)
                                                        }
                                                        _ => None,
                                                    })?,
                                                    plain: (match field_7 {
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
                        })())
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "forge_item_files_changed" {
                return None;
            }
            let forge_item_files_changed: i64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "forge_item_additions" {
                return None;
            }
            let forge_item_additions: i64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "forge_item_deletions" {
                return None;
            }
            let forge_item_deletions: i64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "diff_rows" {
                return None;
            }
            let diff_rows: Vec<crate::host::DiffLine> = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| (|| {
                            let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                name,
                                fields,
                            } = item else {
                                return None;
                            };
                            if name != "DiffLine" || fields.len() != 8 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "key" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "kind" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "old_no" {
                                return None;
                            }
                            let (name, field_3) = fields.next()?;
                            if name != "new_no" {
                                return None;
                            }
                            let (name, field_4) = fields.next()?;
                            if name != "sign" {
                                return None;
                            }
                            let (name, field_5) = fields.next()?;
                            if name != "text" {
                                return None;
                            }
                            let (name, field_6) = fields.next()?;
                            if name != "path" {
                                return None;
                            }
                            let (name, field_7) = fields.next()?;
                            if name != "side" {
                                return None;
                            }
                            Some(crate::host::DiffLine {
                                key: (match field_0 {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                kind: (match field_1 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                old_no: (match field_2 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                new_no: (match field_3 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                sign: (match field_4 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                text: (match field_5 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                path: (match field_6 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                side: (match field_7 {
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
            if name != "forge_item_diff_truncated" {
                return None;
            }
            let forge_item_diff_truncated: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "forge_item_merge_oid" {
                return None;
            }
            let forge_item_merge_oid: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "forge_item_source_branch" {
                return None;
            }
            let forge_item_source_branch: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "forge_item_source_oid" {
                return None;
            }
            let forge_item_source_oid: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "forge_item_target_oid" {
                return None;
            }
            let forge_item_target_oid: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "forge_item_channel" {
                return None;
            }
            let forge_item_channel: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "forge_item_reviews" {
                return None;
            }
            let forge_item_reviews: Vec<crate::host::ForgeReview> = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| (|| {
                            let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                name,
                                fields,
                            } = item else {
                                return None;
                            };
                            if name != "ForgeReview" || fields.len() != 9 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "author" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "author_name" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "verdict" {
                                return None;
                            }
                            let (name, field_3) = fields.next()?;
                            if name != "body" {
                                return None;
                            }
                            let (name, field_4) = fields.next()?;
                            if name != "blocks" {
                                return None;
                            }
                            let (name, field_5) = fields.next()?;
                            if name != "commit" {
                                return None;
                            }
                            let (name, field_6) = fields.next()?;
                            if name != "outdated" {
                                return None;
                            }
                            let (name, field_7) = fields.next()?;
                            if name != "created_at" {
                                return None;
                            }
                            let (name, field_8) = fields.next()?;
                            if name != "comments" {
                                return None;
                            }
                            Some(crate::host::ForgeReview {
                                author: (match field_0 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                author_name: (match field_1 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                verdict: (match field_2 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                body: (match field_3 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                blocks: (match field_4 {
                                    ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                                        items
                                            .into_iter()
                                            .map(|item| (|| {
                                                let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                                    name,
                                                    fields,
                                                } = item else {
                                                    return None;
                                                };
                                                if name != "ChatBlock" || fields.len() != 5 {
                                                    return None;
                                                }
                                                let mut fields = fields.into_iter();
                                                let (name, field_0) = fields.next()?;
                                                if name != "kind" {
                                                    return None;
                                                }
                                                let (name, field_1) = fields.next()?;
                                                if name != "text" {
                                                    return None;
                                                }
                                                let (name, field_2) = fields.next()?;
                                                if name != "lang" {
                                                    return None;
                                                }
                                                let (name, field_3) = fields.next()?;
                                                if name != "rich" {
                                                    return None;
                                                }
                                                let (name, field_4) = fields.next()?;
                                                if name != "spans" {
                                                    return None;
                                                }
                                                Some(crate::host::ChatBlock {
                                                    kind: (match field_0 {
                                                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                            Some(item)
                                                        }
                                                        _ => None,
                                                    })?,
                                                    text: (match field_1 {
                                                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                            Some(item)
                                                        }
                                                        _ => None,
                                                    })?,
                                                    lang: (match field_2 {
                                                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                            Some(item)
                                                        }
                                                        _ => None,
                                                    })?,
                                                    rich: (match field_3 {
                                                        ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => {
                                                            Some(item)
                                                        }
                                                        _ => None,
                                                    })?,
                                                    spans: (match field_4 {
                                                        ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                                                            items
                                                                .into_iter()
                                                                .map(|item| (|| {
                                                                    let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                                                        name,
                                                                        fields,
                                                                    } = item else {
                                                                        return None;
                                                                    };
                                                                    if name != "ChatSpan" || fields.len() != 8 {
                                                                        return None;
                                                                    }
                                                                    let mut fields = fields.into_iter();
                                                                    let (name, field_0) = fields.next()?;
                                                                    if name != "mention" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_1) = fields.next()?;
                                                                    if name != "mention_link" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_2) = fields.next()?;
                                                                    if name != "link_text" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_3) = fields.next()?;
                                                                    if name != "link" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_4) = fields.next()?;
                                                                    if name != "bold_italic" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_5) = fields.next()?;
                                                                    if name != "bold" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_6) = fields.next()?;
                                                                    if name != "italic" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_7) = fields.next()?;
                                                                    if name != "plain" {
                                                                        return None;
                                                                    }
                                                                    Some(crate::host::ChatSpan {
                                                                        mention: (match field_0 {
                                                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                Some(item)
                                                                            }
                                                                            _ => None,
                                                                        })?,
                                                                        mention_link: (match field_1 {
                                                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                Some(item)
                                                                            }
                                                                            _ => None,
                                                                        })?,
                                                                        link_text: (match field_2 {
                                                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                Some(item)
                                                                            }
                                                                            _ => None,
                                                                        })?,
                                                                        link: (match field_3 {
                                                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                Some(item)
                                                                            }
                                                                            _ => None,
                                                                        })?,
                                                                        bold_italic: (match field_4 {
                                                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                Some(item)
                                                                            }
                                                                            _ => None,
                                                                        })?,
                                                                        bold: (match field_5 {
                                                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                Some(item)
                                                                            }
                                                                            _ => None,
                                                                        })?,
                                                                        italic: (match field_6 {
                                                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                Some(item)
                                                                            }
                                                                            _ => None,
                                                                        })?,
                                                                        plain: (match field_7 {
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
                                            })())
                                            .collect::<Option<Vec<_>>>()
                                    }
                                    _ => None,
                                })?,
                                commit: (match field_5 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                outdated: (match field_6 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                created_at: (match field_7 {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                comments: (match field_8 {
                                    ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                                        items
                                            .into_iter()
                                            .map(|item| (|| {
                                                let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                                    name,
                                                    fields,
                                                } = item else {
                                                    return None;
                                                };
                                                if name != "ForgeReviewComment" || fields.len() != 3 {
                                                    return None;
                                                }
                                                let mut fields = fields.into_iter();
                                                let (name, field_0) = fields.next()?;
                                                if name != "anchor" {
                                                    return None;
                                                }
                                                let (name, field_1) = fields.next()?;
                                                if name != "body" {
                                                    return None;
                                                }
                                                let (name, field_2) = fields.next()?;
                                                if name != "blocks" {
                                                    return None;
                                                }
                                                Some(crate::host::ForgeReviewComment {
                                                    anchor: (match field_0 {
                                                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                            Some(item)
                                                        }
                                                        _ => None,
                                                    })?,
                                                    body: (match field_1 {
                                                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                            Some(item)
                                                        }
                                                        _ => None,
                                                    })?,
                                                    blocks: (match field_2 {
                                                        ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                                                            items
                                                                .into_iter()
                                                                .map(|item| (|| {
                                                                    let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                                                        name,
                                                                        fields,
                                                                    } = item else {
                                                                        return None;
                                                                    };
                                                                    if name != "ChatBlock" || fields.len() != 5 {
                                                                        return None;
                                                                    }
                                                                    let mut fields = fields.into_iter();
                                                                    let (name, field_0) = fields.next()?;
                                                                    if name != "kind" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_1) = fields.next()?;
                                                                    if name != "text" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_2) = fields.next()?;
                                                                    if name != "lang" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_3) = fields.next()?;
                                                                    if name != "rich" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_4) = fields.next()?;
                                                                    if name != "spans" {
                                                                        return None;
                                                                    }
                                                                    Some(crate::host::ChatBlock {
                                                                        kind: (match field_0 {
                                                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                Some(item)
                                                                            }
                                                                            _ => None,
                                                                        })?,
                                                                        text: (match field_1 {
                                                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                Some(item)
                                                                            }
                                                                            _ => None,
                                                                        })?,
                                                                        lang: (match field_2 {
                                                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                Some(item)
                                                                            }
                                                                            _ => None,
                                                                        })?,
                                                                        rich: (match field_3 {
                                                                            ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => {
                                                                                Some(item)
                                                                            }
                                                                            _ => None,
                                                                        })?,
                                                                        spans: (match field_4 {
                                                                            ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                                                                                items
                                                                                    .into_iter()
                                                                                    .map(|item| (|| {
                                                                                        let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                                                                            name,
                                                                                            fields,
                                                                                        } = item else {
                                                                                            return None;
                                                                                        };
                                                                                        if name != "ChatSpan" || fields.len() != 8 {
                                                                                            return None;
                                                                                        }
                                                                                        let mut fields = fields.into_iter();
                                                                                        let (name, field_0) = fields.next()?;
                                                                                        if name != "mention" {
                                                                                            return None;
                                                                                        }
                                                                                        let (name, field_1) = fields.next()?;
                                                                                        if name != "mention_link" {
                                                                                            return None;
                                                                                        }
                                                                                        let (name, field_2) = fields.next()?;
                                                                                        if name != "link_text" {
                                                                                            return None;
                                                                                        }
                                                                                        let (name, field_3) = fields.next()?;
                                                                                        if name != "link" {
                                                                                            return None;
                                                                                        }
                                                                                        let (name, field_4) = fields.next()?;
                                                                                        if name != "bold_italic" {
                                                                                            return None;
                                                                                        }
                                                                                        let (name, field_5) = fields.next()?;
                                                                                        if name != "bold" {
                                                                                            return None;
                                                                                        }
                                                                                        let (name, field_6) = fields.next()?;
                                                                                        if name != "italic" {
                                                                                            return None;
                                                                                        }
                                                                                        let (name, field_7) = fields.next()?;
                                                                                        if name != "plain" {
                                                                                            return None;
                                                                                        }
                                                                                        Some(crate::host::ChatSpan {
                                                                                            mention: (match field_0 {
                                                                                                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                                    Some(item)
                                                                                                }
                                                                                                _ => None,
                                                                                            })?,
                                                                                            mention_link: (match field_1 {
                                                                                                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                                    Some(item)
                                                                                                }
                                                                                                _ => None,
                                                                                            })?,
                                                                                            link_text: (match field_2 {
                                                                                                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                                    Some(item)
                                                                                                }
                                                                                                _ => None,
                                                                                            })?,
                                                                                            link: (match field_3 {
                                                                                                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                                    Some(item)
                                                                                                }
                                                                                                _ => None,
                                                                                            })?,
                                                                                            bold_italic: (match field_4 {
                                                                                                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                                    Some(item)
                                                                                                }
                                                                                                _ => None,
                                                                                            })?,
                                                                                            bold: (match field_5 {
                                                                                                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                                    Some(item)
                                                                                                }
                                                                                                _ => None,
                                                                                            })?,
                                                                                            italic: (match field_6 {
                                                                                                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                                    Some(item)
                                                                                                }
                                                                                                _ => None,
                                                                                            })?,
                                                                                            plain: (match field_7 {
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
                                                                })())
                                                                .collect::<Option<Vec<_>>>()
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
                        })())
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "forge_item_approvals" {
                return None;
            }
            let forge_item_approvals: i64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "forge_item_change_requests" {
                return None;
            }
            let forge_item_change_requests: i64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "discussion" {
                return None;
            }
            let discussion: Vec<crate::host::ChatMessage> = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| (|| {
                            let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                name,
                                fields,
                            } = item else {
                                return None;
                            };
                            if name != "ChatMessage" || fields.len() != 7 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "seq" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "author" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "meta" {
                                return None;
                            }
                            let (name, field_3) = fields.next()?;
                            if name != "blocks" {
                                return None;
                            }
                            let (name, field_4) = fields.next()?;
                            if name != "initial" {
                                return None;
                            }
                            let (name, field_5) = fields.next()?;
                            if name != "avatar_kind" {
                                return None;
                            }
                            let (name, field_6) = fields.next()?;
                            if name != "render_rev" {
                                return None;
                            }
                            Some(crate::host::ChatMessage {
                                seq: (match field_0 {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                author: (match field_1 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                meta: (match field_2 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                blocks: (match field_3 {
                                    ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                                        items
                                            .into_iter()
                                            .map(|item| (|| {
                                                let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                                    name,
                                                    fields,
                                                } = item else {
                                                    return None;
                                                };
                                                if name != "ChatBlock" || fields.len() != 5 {
                                                    return None;
                                                }
                                                let mut fields = fields.into_iter();
                                                let (name, field_0) = fields.next()?;
                                                if name != "kind" {
                                                    return None;
                                                }
                                                let (name, field_1) = fields.next()?;
                                                if name != "text" {
                                                    return None;
                                                }
                                                let (name, field_2) = fields.next()?;
                                                if name != "lang" {
                                                    return None;
                                                }
                                                let (name, field_3) = fields.next()?;
                                                if name != "rich" {
                                                    return None;
                                                }
                                                let (name, field_4) = fields.next()?;
                                                if name != "spans" {
                                                    return None;
                                                }
                                                Some(crate::host::ChatBlock {
                                                    kind: (match field_0 {
                                                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                            Some(item)
                                                        }
                                                        _ => None,
                                                    })?,
                                                    text: (match field_1 {
                                                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                            Some(item)
                                                        }
                                                        _ => None,
                                                    })?,
                                                    lang: (match field_2 {
                                                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                            Some(item)
                                                        }
                                                        _ => None,
                                                    })?,
                                                    rich: (match field_3 {
                                                        ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => {
                                                            Some(item)
                                                        }
                                                        _ => None,
                                                    })?,
                                                    spans: (match field_4 {
                                                        ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                                                            items
                                                                .into_iter()
                                                                .map(|item| (|| {
                                                                    let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                                                        name,
                                                                        fields,
                                                                    } = item else {
                                                                        return None;
                                                                    };
                                                                    if name != "ChatSpan" || fields.len() != 8 {
                                                                        return None;
                                                                    }
                                                                    let mut fields = fields.into_iter();
                                                                    let (name, field_0) = fields.next()?;
                                                                    if name != "mention" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_1) = fields.next()?;
                                                                    if name != "mention_link" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_2) = fields.next()?;
                                                                    if name != "link_text" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_3) = fields.next()?;
                                                                    if name != "link" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_4) = fields.next()?;
                                                                    if name != "bold_italic" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_5) = fields.next()?;
                                                                    if name != "bold" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_6) = fields.next()?;
                                                                    if name != "italic" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_7) = fields.next()?;
                                                                    if name != "plain" {
                                                                        return None;
                                                                    }
                                                                    Some(crate::host::ChatSpan {
                                                                        mention: (match field_0 {
                                                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                Some(item)
                                                                            }
                                                                            _ => None,
                                                                        })?,
                                                                        mention_link: (match field_1 {
                                                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                Some(item)
                                                                            }
                                                                            _ => None,
                                                                        })?,
                                                                        link_text: (match field_2 {
                                                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                Some(item)
                                                                            }
                                                                            _ => None,
                                                                        })?,
                                                                        link: (match field_3 {
                                                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                Some(item)
                                                                            }
                                                                            _ => None,
                                                                        })?,
                                                                        bold_italic: (match field_4 {
                                                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                Some(item)
                                                                            }
                                                                            _ => None,
                                                                        })?,
                                                                        bold: (match field_5 {
                                                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                Some(item)
                                                                            }
                                                                            _ => None,
                                                                        })?,
                                                                        italic: (match field_6 {
                                                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                Some(item)
                                                                            }
                                                                            _ => None,
                                                                        })?,
                                                                        plain: (match field_7 {
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
                                            })())
                                            .collect::<Option<Vec<_>>>()
                                    }
                                    _ => None,
                                })?,
                                initial: (match field_4 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                avatar_kind: (match field_5 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                render_rev: (match field_6 {
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
            if name != "discussion_clipped" {
                return None;
            }
            let discussion_clipped: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "linked_note" {
                return None;
            }
            let linked_note: Vec<crate::host::ChatMessage> = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| (|| {
                            let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                name,
                                fields,
                            } = item else {
                                return None;
                            };
                            if name != "ChatMessage" || fields.len() != 7 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "seq" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "author" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "meta" {
                                return None;
                            }
                            let (name, field_3) = fields.next()?;
                            if name != "blocks" {
                                return None;
                            }
                            let (name, field_4) = fields.next()?;
                            if name != "initial" {
                                return None;
                            }
                            let (name, field_5) = fields.next()?;
                            if name != "avatar_kind" {
                                return None;
                            }
                            let (name, field_6) = fields.next()?;
                            if name != "render_rev" {
                                return None;
                            }
                            Some(crate::host::ChatMessage {
                                seq: (match field_0 {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                author: (match field_1 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                meta: (match field_2 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                blocks: (match field_3 {
                                    ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                                        items
                                            .into_iter()
                                            .map(|item| (|| {
                                                let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                                    name,
                                                    fields,
                                                } = item else {
                                                    return None;
                                                };
                                                if name != "ChatBlock" || fields.len() != 5 {
                                                    return None;
                                                }
                                                let mut fields = fields.into_iter();
                                                let (name, field_0) = fields.next()?;
                                                if name != "kind" {
                                                    return None;
                                                }
                                                let (name, field_1) = fields.next()?;
                                                if name != "text" {
                                                    return None;
                                                }
                                                let (name, field_2) = fields.next()?;
                                                if name != "lang" {
                                                    return None;
                                                }
                                                let (name, field_3) = fields.next()?;
                                                if name != "rich" {
                                                    return None;
                                                }
                                                let (name, field_4) = fields.next()?;
                                                if name != "spans" {
                                                    return None;
                                                }
                                                Some(crate::host::ChatBlock {
                                                    kind: (match field_0 {
                                                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                            Some(item)
                                                        }
                                                        _ => None,
                                                    })?,
                                                    text: (match field_1 {
                                                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                            Some(item)
                                                        }
                                                        _ => None,
                                                    })?,
                                                    lang: (match field_2 {
                                                        ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                            Some(item)
                                                        }
                                                        _ => None,
                                                    })?,
                                                    rich: (match field_3 {
                                                        ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => {
                                                            Some(item)
                                                        }
                                                        _ => None,
                                                    })?,
                                                    spans: (match field_4 {
                                                        ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                                                            items
                                                                .into_iter()
                                                                .map(|item| (|| {
                                                                    let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                                                        name,
                                                                        fields,
                                                                    } = item else {
                                                                        return None;
                                                                    };
                                                                    if name != "ChatSpan" || fields.len() != 8 {
                                                                        return None;
                                                                    }
                                                                    let mut fields = fields.into_iter();
                                                                    let (name, field_0) = fields.next()?;
                                                                    if name != "mention" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_1) = fields.next()?;
                                                                    if name != "mention_link" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_2) = fields.next()?;
                                                                    if name != "link_text" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_3) = fields.next()?;
                                                                    if name != "link" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_4) = fields.next()?;
                                                                    if name != "bold_italic" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_5) = fields.next()?;
                                                                    if name != "bold" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_6) = fields.next()?;
                                                                    if name != "italic" {
                                                                        return None;
                                                                    }
                                                                    let (name, field_7) = fields.next()?;
                                                                    if name != "plain" {
                                                                        return None;
                                                                    }
                                                                    Some(crate::host::ChatSpan {
                                                                        mention: (match field_0 {
                                                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                Some(item)
                                                                            }
                                                                            _ => None,
                                                                        })?,
                                                                        mention_link: (match field_1 {
                                                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                Some(item)
                                                                            }
                                                                            _ => None,
                                                                        })?,
                                                                        link_text: (match field_2 {
                                                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                Some(item)
                                                                            }
                                                                            _ => None,
                                                                        })?,
                                                                        link: (match field_3 {
                                                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                Some(item)
                                                                            }
                                                                            _ => None,
                                                                        })?,
                                                                        bold_italic: (match field_4 {
                                                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                Some(item)
                                                                            }
                                                                            _ => None,
                                                                        })?,
                                                                        bold: (match field_5 {
                                                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                Some(item)
                                                                            }
                                                                            _ => None,
                                                                        })?,
                                                                        italic: (match field_6 {
                                                                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                                                                Some(item)
                                                                            }
                                                                            _ => None,
                                                                        })?,
                                                                        plain: (match field_7 {
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
                                            })())
                                            .collect::<Option<Vec<_>>>()
                                    }
                                    _ => None,
                                })?,
                                initial: (match field_4 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                avatar_kind: (match field_5 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                render_rev: (match field_6 {
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
            if name != "roster_set" {
                return None;
            }
            let roster_set: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "focus_seq" {
                return None;
            }
            let focus_seq: i64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "landed_tick" {
                return None;
            }
            let landed_tick: i64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "focus_number" {
                return None;
            }
            let focus_number: i64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "merge_conflicts" {
                return None;
            }
            let merge_conflicts: Vec<String> = (match value {
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
            if name != "merge_busy" {
                return None;
            }
            let merge_busy: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "review_verdict" {
                return None;
            }
            let review_verdict: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "review_busy" {
                return None;
            }
            let review_busy: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "staged_comments" {
                return None;
            }
            let staged_comments: Vec<crate::host::ForgeDraftComment> = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| (|| {
                            let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                name,
                                fields,
                            } = item else {
                                return None;
                            };
                            if name != "ForgeDraftComment" || fields.len() != 5 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "anchor" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "path" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "line" {
                                return None;
                            }
                            let (name, field_3) = fields.next()?;
                            if name != "side" {
                                return None;
                            }
                            let (name, field_4) = fields.next()?;
                            if name != "body" {
                                return None;
                            }
                            Some(crate::host::ForgeDraftComment {
                                anchor: (match field_0 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                path: (match field_1 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                line: (match field_2 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                side: (match field_3 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                body: (match field_4 {
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
            if name != "tree_pick" {
                return None;
            }
            let tree_pick: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "tree_path" {
                return None;
            }
            let tree_path: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "tree_rev" {
                return None;
            }
            let tree_rev: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "tree_entries" {
                return None;
            }
            let tree_entries: Vec<crate::host::TreeEntry> = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| (|| {
                            let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                name,
                                fields,
                            } = item else {
                                return None;
                            };
                            if name != "TreeEntry" || fields.len() != 3 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "name" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "path" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "kind" {
                                return None;
                            }
                            Some(crate::host::TreeEntry {
                                name: (match field_0 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                path: (match field_1 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                kind: (match field_2 {
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
            if name != "tree_born" {
                return None;
            }
            let tree_born: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "tree_truncated" {
                return None;
            }
            let tree_truncated: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "tree_phase" {
                return None;
            }
            let tree_phase: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "file_path" {
                return None;
            }
            let file_path: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "file_text" {
                return None;
            }
            let file_text: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "file_binary" {
                return None;
            }
            let file_binary: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "file_truncated" {
                return None;
            }
            let file_truncated: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "file_picture" {
                return None;
            }
            let file_picture: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "file_width" {
                return None;
            }
            let file_width: i64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "file_height" {
                return None;
            }
            let file_height: i64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "file_note" {
                return None;
            }
            let file_note: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "file_phase" {
                return None;
            }
            let file_phase: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "opened_dir" {
                return None;
            }
            let opened_dir: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "opened_rev" {
                return None;
            }
            let opened_rev: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "focus_path" {
                return None;
            }
            let focus_path: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "focus_rev" {
                return None;
            }
            let focus_rev: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "review_draft" {
                return None;
            }
            let review_draft: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "comment_draft" {
                return None;
            }
            let comment_draft: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "comment_path" {
                return None;
            }
            let comment_path: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "comment_line" {
                return None;
            }
            let comment_line: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "comment_side" {
                return None;
            }
            let comment_side: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
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
            if name != "sent" {
                return None;
            }
            let sent: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "viewport_width" {
                return None;
            }
            let viewport_width: f64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::F64(
                    item,
                ) if item.is_finite() => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "tree_width" {
                return None;
            }
            let tree_width: f64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::F64(
                    item,
                ) if item.is_finite() => Some(item),
                _ => None,
            })?;
            Some(Self {
                active_palette: active_palette,
                connected: connected,
                dark: dark,
                org: org,
                about: about,
                tier: tier,
                network_chain_id: network_chain_id,
                connected_rpc: connected_rpc,
                connection_serial: connection_serial,
                link_tick: link_tick,
                repos: repos,
                list_phase: list_phase,
                open_repo: open_repo,
                repo_phase: repo_phase,
                branches: branches,
                items: items,
                tab: tab,
                forge_item_number: forge_item_number,
                item_phase: item_phase,
                forge_item_kind: forge_item_kind,
                forge_item_title: forge_item_title,
                forge_item_state: forge_item_state,
                forge_item_author: forge_item_author,
                forge_item_branches: forge_item_branches,
                forge_item_body: forge_item_body,
                forge_item_blocks: forge_item_blocks,
                forge_item_files_changed: forge_item_files_changed,
                forge_item_additions: forge_item_additions,
                forge_item_deletions: forge_item_deletions,
                diff_rows: diff_rows,
                forge_item_diff_truncated: forge_item_diff_truncated,
                forge_item_merge_oid: forge_item_merge_oid,
                forge_item_source_branch: forge_item_source_branch,
                forge_item_source_oid: forge_item_source_oid,
                forge_item_target_oid: forge_item_target_oid,
                forge_item_channel: forge_item_channel,
                forge_item_reviews: forge_item_reviews,
                forge_item_approvals: forge_item_approvals,
                forge_item_change_requests: forge_item_change_requests,
                discussion: discussion,
                discussion_clipped: discussion_clipped,
                linked_note: linked_note,
                roster_set: roster_set,
                focus_seq: focus_seq,
                landed_tick: landed_tick,
                focus_number: focus_number,
                merge_conflicts: merge_conflicts,
                merge_busy: merge_busy,
                review_verdict: review_verdict,
                review_busy: review_busy,
                staged_comments: staged_comments,
                tree_pick: tree_pick,
                tree_path: tree_path,
                tree_rev: tree_rev,
                tree_entries: tree_entries,
                tree_born: tree_born,
                tree_truncated: tree_truncated,
                tree_phase: tree_phase,
                file_path: file_path,
                file_text: file_text,
                file_binary: file_binary,
                file_truncated: file_truncated,
                file_picture: file_picture,
                file_width: file_width,
                file_height: file_height,
                file_note: file_note,
                file_phase: file_phase,
                opened_dir: opened_dir,
                opened_rev: opened_rev,
                focus_path: focus_path,
                focus_rev: focus_rev,
                review_draft: review_draft,
                comment_draft: comment_draft,
                comment_path: comment_path,
                comment_line: comment_line,
                comment_side: comment_side,
                host_error: host_error,
                sent: sent,
                viewport_width: viewport_width,
                tree_width: tree_width,
                file_text_revision: ::ducktape_view_guest::rev::seed(),
            })
        })())
            .ok_or_else(|| String::from("snapshot state mismatch"))
    }
}
#[allow(unused_parens)]
impl ForgeView {
    pub(crate) fn subscription(&self) -> ::ducktape_view_guest::Subscription<Message> {
        ::ducktape_view_guest::Subscription::batch([
            crate::host::session().map(move |value| Message::SessionArrived(value)),
            if self.connected {
                ::ducktape_view_guest::Subscription::batch([crate::host::repos(
                    self.connection_serial,
                )
                .map(move |value| Message::ReposArrived(value))])
            } else {
                ::ducktape_view_guest::Subscription::none()
            },
            if self.connected {
                ::ducktape_view_guest::Subscription::batch([crate::host::repo(
                    self.connection_serial,
                    self.open_repo.to_owned(),
                )
                .map(move |value| Message::RepoArrived(value))])
            } else {
                ::ducktape_view_guest::Subscription::none()
            },
            if self.connected {
                ::ducktape_view_guest::Subscription::batch([crate::host::item(
                    self.connection_serial,
                    self.open_repo.to_owned(),
                    self.forge_item_number,
                )
                .map(move |value| Message::ItemArrived(value))])
            } else {
                ::ducktape_view_guest::Subscription::none()
            },
            if self.connected {
                ::ducktape_view_guest::Subscription::batch([crate::host::discussion(
                    self.connection_serial,
                    self.forge_item_channel.to_owned(),
                )
                .map(move |value| Message::DiscussionArrived(value))])
            } else {
                ::ducktape_view_guest::Subscription::none()
            },
            if self.connected {
                ::ducktape_view_guest::Subscription::batch([crate::host::tree(
                    self.connection_serial,
                    self.open_repo.to_owned(),
                    self.tree_rev.to_owned(),
                    self.tree_path.to_owned(),
                )
                .map(move |value| Message::TreeArrived(value))])
            } else {
                ::ducktape_view_guest::Subscription::none()
            },
            if self.connected {
                ::ducktape_view_guest::Subscription::batch([crate::host::blob(
                    self.connection_serial,
                    self.open_repo.to_owned(),
                    self.tree_rev.to_owned(),
                    self.file_path.to_owned(),
                    self.network_chain_id.to_owned(),
                )
                .map(move |value| Message::BlobArrived(value))])
            } else {
                ::ducktape_view_guest::Subscription::none()
            },
            crate::host::acts().map(move |value| Message::ActDone(value)),
        ])
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn view_fits_default_stack() {
        ::std::thread::Builder::new()
            .stack_size(4 * 1024 * 1024)
            .spawn(|| {
                let (app, _) = ForgeView::boot();
                let _ = app.view();
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
mod app_update;
mod app_view;
mod components;
mod forge;
mod icon;
mod kit;
