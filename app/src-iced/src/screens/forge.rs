//! Native Forge repository browser and issue/pull-request surface.
//!
//! The module owns presentation state only. [`update`] emits typed effects for
//! the shell to execute and accepts their results through [`ServiceEvent`].

mod view;

pub use view::view;

use iced::widget::text_editor;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resource<T> {
    Loading,
    Empty,
    Error(String),
    Ready(T),
}

/// Read-only multi-line buffer backing the file viewer's selectable code pane.
///
/// iced's `text_editor` needs a persistent `Content` in state to keep its
/// selection across frames, but `Content` is neither `Clone`-cheap nor
/// `PartialEq`/`Eq`. This wrapper compares by rendered text — the same idiom
/// `screens/pages.rs::EditorState` uses — so `State` can keep its derives.
#[derive(Debug, Clone)]
pub struct CodeContent(pub text_editor::Content);

impl CodeContent {
    fn set(&mut self, text: &str) {
        self.0 = text_editor::Content::with_text(text);
    }
}

impl Default for CodeContent {
    fn default() -> Self {
        Self(text_editor::Content::new())
    }
}

impl PartialEq for CodeContent {
    fn eq(&self, other: &Self) -> bool {
        self.0.text() == other.0.text()
    }
}

impl Eq for CodeContent {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Code,
    Commits,
    Issues,
    Pulls,
}

impl Tab {
    const fn label(self) -> &'static str {
        match self {
            Self::Code => "Code",
            Self::Commits => "Commits",
            Self::Issues => "Issues",
            Self::Pulls => "Pull requests",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemFilter {
    Open,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Issue,
    PullRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemState {
    Open,
    Closed,
    Merged,
}

impl ItemState {
    const fn label(self) -> &'static str {
        match self {
            Self::Open => "OPEN",
            Self::Closed => "CLOSED",
            Self::Merged => "MERGED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repository {
    pub id: String,
    pub name: String,
    pub default_branch: String,
    pub head: Option<String>,
    pub browsable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Branch {
    pub name: String,
    pub head: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeKind {
    Directory,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    pub path: String,
    pub name: String,
    pub kind: TreeKind,
    pub depth: usize,
    pub open: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub id: String,
    pub summary: String,
    pub author: String,
    pub time: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePage {
    pub path: String,
    pub text: String,
    pub loaded_bytes: u64,
    pub total_bytes: u64,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeItem {
    pub number: u64,
    pub kind: ItemKind,
    pub state: ItemState,
    pub title: String,
    pub author: String,
    pub updated: String,
    pub source_branch: Option<String>,
    pub target_branch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscussionPost {
    pub author: String,
    pub body: String,
    pub time: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewVerdict {
    Approve,
    RequestChanges,
    Comment,
}

impl ReviewVerdict {
    const fn label(self) -> &'static str {
        match self {
            Self::Approve => "Approve",
            Self::RequestChanges => "Request changes",
            Self::Comment => "Comment",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewSide {
    Old,
    New,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewComment {
    pub path: String,
    pub line: u32,
    pub side: ReviewSide,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Review {
    pub author: String,
    pub verdict: ReviewVerdict,
    pub body: String,
    pub commit_oid: String,
    pub comments: Vec<ReviewComment>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemDetail {
    pub item: ForgeItem,
    pub body: String,
    pub can_edit: bool,
    pub comments: Vec<DiscussionPost>,
    pub commits: Vec<Commit>,
    pub changed_files: Vec<ChangedFile>,
    pub compare_error: Option<String>,
    pub reviews: Vec<Review>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    pub path: String,
    pub additions: u64,
    pub deletions: u64,
    pub patch: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryData {
    pub branches: Vec<Branch>,
    pub tree: Vec<TreeEntry>,
    pub commits: Vec<Commit>,
    pub commits_have_more: bool,
    pub items: Vec<ForgeItem>,
    /// True when this repository is browsed over the network mirror rather than
    /// the node's own on-disk forge (drives the REMOTE/DESKTOP pill and copy).
    pub remote: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullTab {
    Conversation,
    Commits,
    Files,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewItemDraft {
    pub title: String,
    pub body: String,
    pub source_branch: String,
    pub target_branch: String,
}

impl Default for NewItemDraft {
    fn default() -> Self {
        Self {
            title: String::new(),
            body: String::new(),
            source_branch: String::new(),
            target_branch: "dev".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub repositories: Resource<Vec<Repository>>,
    pub selected_repo: Option<String>,
    pub repository: Resource<RepositoryData>,
    pub tab: Tab,
    pub selected_branch: Option<String>,
    pub repo_menu_open: bool,
    pub branch_menu_open: bool,
    pub selected_file: Option<String>,
    pub file: Resource<FilePage>,
    /// Selectable, read-only buffer mirroring the loaded file text.
    pub file_content: CodeContent,
    /// Commit whose per-file diff is expanded in a commit log, if any.
    pub selected_commit: Option<String>,
    /// Loaded parent→commit diff for `selected_commit`.
    pub commit_diff: Resource<Vec<ChangedFile>>,
    /// Changed-file paths the reviewer has collapsed in the Files-changed tab.
    pub collapsed_files: Vec<String>,
    pub item_filter: ItemFilter,
    pub new_item_open: bool,
    pub new_item: NewItemDraft,
    pub selected_item: Option<u64>,
    pub item_detail: Resource<ItemDetail>,
    pub pull_tab: PullTab,
    /// Multi-line composer buffer for a new discussion comment.
    pub comment_content: CodeContent,
    pub editing_item: bool,
    pub edit_title: String,
    pub edit_body: String,
    pub review_open: bool,
    pub review_verdict: ReviewVerdict,
    pub review_body: String,
    pub review_comment_file: Option<String>,
    pub review_comment_line: String,
    pub review_comment_body: String,
    pub review_comments: Vec<ReviewComment>,
    pub busy: bool,
    pub error: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            repositories: Resource::Loading,
            selected_repo: None,
            repository: Resource::Empty,
            tab: Tab::Code,
            selected_branch: None,
            repo_menu_open: false,
            branch_menu_open: false,
            selected_file: None,
            file: Resource::Empty,
            file_content: CodeContent::default(),
            selected_commit: None,
            commit_diff: Resource::Empty,
            collapsed_files: Vec::new(),
            item_filter: ItemFilter::Open,
            new_item_open: false,
            new_item: NewItemDraft::default(),
            selected_item: None,
            item_detail: Resource::Empty,
            pull_tab: PullTab::Conversation,
            comment_content: CodeContent::default(),
            editing_item: false,
            edit_title: String::new(),
            edit_body: String::new(),
            review_open: false,
            review_verdict: ReviewVerdict::Comment,
            review_body: String::new(),
            review_comment_file: None,
            review_comment_line: String::new(),
            review_comment_body: String::new(),
            review_comments: Vec::new(),
            busy: false,
            error: None,
        }
    }
}

// `text_editor::Action` (carried by the read-only file pane and the comment
// composer) is `PartialEq` but not `Eq`, so `Message` cannot derive `Eq`.
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum Message {
    Load,
    SelectRepository(String),
    BackToRepositories,
    ToggleRepositoryMenu,
    ToggleBranchMenu,
    SelectBranch(String),
    SelectTab(Tab),
    ToggleDirectory(String),
    SelectFile(String),
    /// Read-only selection/scroll action on the file viewer; edits are dropped.
    FileAction(text_editor::Action),
    LoadMoreFile,
    LoadMoreCommits,
    /// Expand/collapse a commit's per-file diff in a commit log.
    ToggleCommit(String),
    /// Expand/collapse one changed file in the Files-changed tab.
    ToggleChangedFile(String),
    /// Stage a pending inline review comment on a clicked new-side line.
    StartLineComment { path: String, line: u32 },
    SetItemFilter(ItemFilter),
    ToggleNewItem,
    NewTitleChanged(String),
    NewBodyChanged(String),
    SourceBranchChanged(String),
    TargetBranchChanged(String),
    SubmitNewItem,
    OpenItem(u64),
    CloseItem,
    SelectPullTab(PullTab),
    StartEditingItem,
    CancelEditingItem,
    EditTitleChanged(String),
    EditBodyChanged(String),
    SaveItemEdit,
    ToggleItemState,
    MergePullRequest,
    ToggleReview,
    ReviewVerdictChanged(ReviewVerdict),
    ReviewBodyChanged(String),
    StartReviewComment(String),
    ReviewCommentLineChanged(String),
    ReviewCommentBodyChanged(String),
    QueueReviewComment,
    CancelReviewComment,
    RemoveReviewComment(usize),
    SubmitReview,
    /// Editable action on the multi-line comment composer.
    CommentAction(text_editor::Action),
    SubmitComment,
    Service(ServiceEvent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    LoadRepositories,
    LoadRepository {
        repository_id: String,
        reference: Option<String>,
    },
    LoadDirectory {
        repository_id: String,
        path: String,
        reference: Option<String>,
    },
    LoadFile {
        repository_id: String,
        path: String,
        reference: Option<String>,
        offset: u64,
    },
    LoadMoreCommits {
        repository_id: String,
        reference: Option<String>,
        after: String,
    },
    LoadCommitDiff {
        repository_id: String,
        commit_id: String,
    },
    OpenIssue {
        repository_id: String,
        title: String,
        body: String,
    },
    OpenPullRequest {
        repository_id: String,
        title: String,
        body: String,
        source_branch: String,
        target_branch: String,
    },
    LoadItem {
        repository_id: String,
        number: u64,
    },
    EditItem {
        repository_id: String,
        number: u64,
        title: Option<String>,
        body: Option<String>,
    },
    SetItemState {
        repository_id: String,
        number: u64,
        open: bool,
    },
    MergePullRequest {
        repository_id: String,
        number: u64,
    },
    SubmitReview {
        repository_id: String,
        number: u64,
        verdict: ReviewVerdict,
        body: String,
        commit_oid: String,
        comments: Vec<ReviewComment>,
    },
    AddComment {
        repository_id: String,
        number: u64,
        body: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceEvent {
    RepositoriesLoaded(Result<Vec<Repository>, String>),
    RepositoryLoaded(Result<RepositoryData, String>),
    DirectoryLoaded {
        path: String,
        result: Result<Vec<TreeEntry>, String>,
    },
    FileLoaded(Result<FilePage, String>),
    MoreCommitsLoaded(Result<(Vec<Commit>, bool), String>),
    CommitDiffLoaded {
        commit_id: String,
        result: Result<Vec<ChangedFile>, String>,
    },
    WriteFinished(Result<(), String>),
    ItemLoaded(Result<Option<ItemDetail>, String>),
}

pub fn update(state: &mut State, message: Message) -> Option<Command> {
    match message {
        Message::Load => {
            *state = State::default();
            Some(Command::LoadRepositories)
        }
        Message::SelectRepository(id) => {
            state.selected_repo = Some(id.clone());
            state.selected_branch = None;
            state.repository = Resource::Loading;
            state.selected_file = None;
            state.file = Resource::Empty;
            state.tab = Tab::Code;
            state.repo_menu_open = false;
            state.selected_item = None;
            Some(Command::LoadRepository {
                repository_id: id,
                reference: None,
            })
        }
        Message::BackToRepositories => {
            state.selected_repo = None;
            state.repo_menu_open = false;
            state.selected_item = None;
            None
        }
        Message::ToggleRepositoryMenu => {
            state.repo_menu_open = !state.repo_menu_open;
            state.branch_menu_open = false;
            None
        }
        Message::ToggleBranchMenu => {
            state.branch_menu_open = !state.branch_menu_open;
            state.repo_menu_open = false;
            None
        }
        Message::SelectBranch(branch) => {
            let id = state.selected_repo.clone()?;
            state.selected_branch = Some(branch.clone());
            state.branch_menu_open = false;
            state.repository = Resource::Loading;
            state.selected_file = None;
            state.file = Resource::Empty;
            Some(Command::LoadRepository {
                repository_id: id,
                reference: Some(branch),
            })
        }
        Message::SelectTab(tab) => {
            state.tab = tab;
            state.selected_item = None;
            state.new_item_open = false;
            state.selected_commit = None;
            state.commit_diff = Resource::Empty;
            None
        }
        Message::ToggleDirectory(path) => {
            let id = state.selected_repo.clone()?;
            if let Resource::Ready(data) = &mut state.repository
                && let Some(entry) = data.tree.iter_mut().find(|entry| entry.path == path)
            {
                entry.open = !entry.open;
                if !entry.open {
                    return None;
                }
            }
            Some(Command::LoadDirectory {
                repository_id: id,
                path,
                reference: state.selected_branch.clone(),
            })
        }
        Message::SelectFile(path) => {
            let id = state.selected_repo.clone()?;
            state.selected_file = Some(path.clone());
            state.file = Resource::Loading;
            Some(Command::LoadFile {
                repository_id: id,
                path,
                reference: state.selected_branch.clone(),
                offset: 0,
            })
        }
        Message::LoadMoreFile => {
            let id = state.selected_repo.clone()?;
            let Resource::Ready(file) = &state.file else {
                return None;
            };
            if !file.has_more {
                return None;
            }
            Some(Command::LoadFile {
                repository_id: id,
                path: file.path.clone(),
                reference: state.selected_branch.clone(),
                offset: file.loaded_bytes,
            })
        }
        Message::FileAction(action) => {
            // Read-only pane: apply selection and scroll, drop every edit.
            if !action.is_edit() {
                state.file_content.0.perform(action);
            }
            None
        }
        Message::LoadMoreCommits => {
            let id = state.selected_repo.clone()?;
            let Resource::Ready(data) = &state.repository else {
                return None;
            };
            if !data.commits_have_more {
                return None;
            }
            Some(Command::LoadMoreCommits {
                repository_id: id,
                reference: state.selected_branch.clone(),
                after: data.commits.last()?.id.clone(),
            })
        }
        Message::ToggleCommit(commit_id) => {
            let repository_id = state.selected_repo.clone()?;
            let same = state.selected_commit.as_deref() == Some(commit_id.as_str());
            // Re-clicking (or retrying) a failed diff reloads it; re-clicking a
            // loaded one collapses it.
            if same && !matches!(state.commit_diff, Resource::Error(_)) {
                state.selected_commit = None;
                state.commit_diff = Resource::Empty;
                return None;
            }
            state.selected_commit = Some(commit_id.clone());
            state.commit_diff = Resource::Loading;
            Some(Command::LoadCommitDiff {
                repository_id,
                commit_id,
            })
        }
        Message::ToggleChangedFile(path) => {
            if let Some(index) = state.collapsed_files.iter().position(|item| item == &path) {
                state.collapsed_files.remove(index);
            } else {
                state.collapsed_files.push(path);
            }
            None
        }
        Message::StartLineComment { path, line } => {
            if !state.review_open || state.busy {
                return None;
            }
            state.review_comment_file = Some(path);
            state.review_comment_line = line.to_string();
            state.review_comment_body.clear();
            state.error = None;
            None
        }
        Message::SetItemFilter(filter) => {
            state.item_filter = filter;
            None
        }
        Message::ToggleNewItem => {
            state.new_item_open = !state.new_item_open;
            state.error = None;
            None
        }
        Message::NewTitleChanged(value) => {
            state.new_item.title = value;
            None
        }
        Message::NewBodyChanged(value) => {
            state.new_item.body = value;
            None
        }
        Message::SourceBranchChanged(value) => {
            state.new_item.source_branch = value;
            None
        }
        Message::TargetBranchChanged(value) => {
            state.new_item.target_branch = value;
            None
        }
        Message::SubmitNewItem => {
            let id = state.selected_repo.clone()?;
            let title = state.new_item.title.trim().to_owned();
            if title.is_empty() || state.busy {
                return None;
            }
            state.busy = true;
            state.error = None;
            match state.tab {
                Tab::Issues => Some(Command::OpenIssue {
                    repository_id: id,
                    title,
                    body: state.new_item.body.clone(),
                }),
                Tab::Pulls
                    if !state.new_item.source_branch.is_empty()
                        && !state.new_item.target_branch.is_empty() =>
                {
                    Some(Command::OpenPullRequest {
                        repository_id: id,
                        title,
                        body: state.new_item.body.clone(),
                        source_branch: state.new_item.source_branch.clone(),
                        target_branch: state.new_item.target_branch.clone(),
                    })
                }
                _ => {
                    state.busy = false;
                    None
                }
            }
        }
        Message::OpenItem(number) => {
            let id = state.selected_repo.clone()?;
            state.selected_item = Some(number);
            state.item_detail = Resource::Loading;
            state.pull_tab = PullTab::Conversation;
            state.editing_item = false;
            state.review_open = false;
            state.selected_commit = None;
            state.commit_diff = Resource::Empty;
            state.collapsed_files.clear();
            clear_review_draft(state);
            Some(Command::LoadItem {
                repository_id: id,
                number,
            })
        }
        Message::CloseItem => {
            state.selected_item = None;
            state.item_detail = Resource::Empty;
            state.comment_content.set("");
            state.editing_item = false;
            state.review_open = false;
            state.selected_commit = None;
            state.commit_diff = Resource::Empty;
            state.collapsed_files.clear();
            clear_review_draft(state);
            None
        }
        Message::SelectPullTab(tab) => {
            state.pull_tab = tab;
            None
        }
        Message::StartEditingItem => {
            let Resource::Ready(detail) = &state.item_detail else {
                return None;
            };
            if !detail.can_edit || state.busy {
                return None;
            }
            state.edit_title = detail.item.title.clone();
            state.edit_body = detail.body.clone();
            state.editing_item = true;
            state.error = None;
            None
        }
        Message::CancelEditingItem => {
            state.editing_item = false;
            state.error = None;
            None
        }
        Message::EditTitleChanged(value) => {
            state.edit_title = value;
            None
        }
        Message::EditBodyChanged(value) => {
            state.edit_body = value;
            None
        }
        Message::SaveItemEdit => {
            let repository_id = state.selected_repo.clone()?;
            let Resource::Ready(detail) = &state.item_detail else {
                return None;
            };
            let title = state.edit_title.trim();
            if !state.editing_item || !detail.can_edit || title.is_empty() || state.busy {
                return None;
            }
            let title = (title != detail.item.title).then(|| title.to_owned());
            let body = (state.edit_body != detail.body).then(|| state.edit_body.clone());
            if title.is_none() && body.is_none() {
                state.editing_item = false;
                return None;
            }
            state.busy = true;
            state.error = None;
            Some(Command::EditItem {
                repository_id,
                number: detail.item.number,
                title,
                body,
            })
        }
        Message::ToggleItemState => {
            let id = state.selected_repo.clone()?;
            let Resource::Ready(detail) = &state.item_detail else {
                return None;
            };
            if detail.item.state == ItemState::Merged || state.busy {
                return None;
            }
            state.busy = true;
            Some(Command::SetItemState {
                repository_id: id,
                number: detail.item.number,
                open: detail.item.state != ItemState::Open,
            })
        }
        Message::MergePullRequest => {
            let id = state.selected_repo.clone()?;
            let Resource::Ready(detail) = &state.item_detail else {
                return None;
            };
            if detail.item.kind != ItemKind::PullRequest
                || detail.item.state != ItemState::Open
                || state.busy
            {
                return None;
            }
            state.busy = true;
            Some(Command::MergePullRequest {
                repository_id: id,
                number: detail.item.number,
            })
        }
        Message::ToggleReview => {
            let Resource::Ready(detail) = &state.item_detail else {
                return None;
            };
            if detail.item.kind != ItemKind::PullRequest || state.busy {
                return None;
            }
            state.review_open = !state.review_open;
            if !state.review_open {
                clear_review_draft(state);
            }
            state.error = None;
            None
        }
        Message::ReviewVerdictChanged(verdict) => {
            state.review_verdict = verdict;
            None
        }
        Message::ReviewBodyChanged(value) => {
            state.review_body = value;
            None
        }
        Message::StartReviewComment(path) => {
            if !state.review_open || state.busy {
                return None;
            }
            state.review_comment_file = Some(path);
            state.review_comment_line.clear();
            state.review_comment_body.clear();
            state.error = None;
            None
        }
        Message::ReviewCommentLineChanged(value) => {
            if value.is_empty() || value.chars().all(|character| character.is_ascii_digit()) {
                state.review_comment_line = value;
            }
            None
        }
        Message::ReviewCommentBodyChanged(value) => {
            state.review_comment_body = value;
            None
        }
        Message::QueueReviewComment => {
            let path = state.review_comment_file.clone()?;
            let Ok(line) = state.review_comment_line.parse::<u32>() else {
                state.error = Some("Enter a positive new-file line number.".into());
                return None;
            };
            let body = state.review_comment_body.trim().to_owned();
            if line == 0 || body.is_empty() || body.len() > 16 * 1024 {
                state.error =
                    Some("Inline comments need a positive line and at most 16 KiB of text.".into());
                return None;
            }
            if state.review_comments.len() >= 64 {
                state.error = Some("A review can contain at most 64 inline comments.".into());
                return None;
            }
            state.review_comments.push(ReviewComment {
                path,
                line,
                side: ReviewSide::New,
                body,
            });
            state.review_comment_file = None;
            state.review_comment_line.clear();
            state.review_comment_body.clear();
            state.error = None;
            None
        }
        Message::CancelReviewComment => {
            state.review_comment_file = None;
            state.review_comment_line.clear();
            state.review_comment_body.clear();
            state.error = None;
            None
        }
        Message::RemoveReviewComment(index) => {
            if index < state.review_comments.len() && !state.busy {
                state.review_comments.remove(index);
            }
            None
        }
        Message::SubmitReview => {
            let repository_id = state.selected_repo.clone()?;
            let Resource::Ready(detail) = &state.item_detail else {
                return None;
            };
            let source = detail.item.source_branch.as_deref()?;
            let commit_oid = repository_data(state)?
                .branches
                .iter()
                .find(|branch| branch.name == source)?
                .head
                .clone();
            let body = state.review_body.trim().to_owned();
            if !state.review_open
                || detail.item.kind != ItemKind::PullRequest
                || body.is_empty()
                || state.busy
            {
                return None;
            }
            state.busy = true;
            state.error = None;
            Some(Command::SubmitReview {
                repository_id,
                number: detail.item.number,
                verdict: state.review_verdict,
                body,
                commit_oid,
                comments: state.review_comments.clone(),
            })
        }
        Message::CommentAction(action) => {
            state.comment_content.0.perform(action);
            None
        }
        Message::SubmitComment => {
            let id = state.selected_repo.clone()?;
            let number = state.selected_item?;
            let body = state.comment_content.0.text().trim().to_owned();
            if body.is_empty() || state.busy {
                return None;
            }
            state.busy = true;
            Some(Command::AddComment {
                repository_id: id,
                number,
                body,
            })
        }
        Message::Service(event) => service(state, event),
    }
}

fn service(state: &mut State, event: ServiceEvent) -> Option<Command> {
    match event {
        ServiceEvent::RepositoriesLoaded(result) => {
            state.repositories = result.map_or_else(Resource::Error, |repos| {
                if repos.is_empty() {
                    Resource::Empty
                } else {
                    Resource::Ready(repos)
                }
            });
        }
        ServiceEvent::RepositoryLoaded(result) => {
            state.repository = result.map_or_else(Resource::Error, Resource::Ready);
        }
        ServiceEvent::DirectoryLoaded { path, result } => match result {
            Ok(mut entries) => {
                if let Resource::Ready(data) = &mut state.repository {
                    data.tree.retain(|entry| !is_child_of(&entry.path, &path));
                    data.tree.append(&mut entries);
                    // P9: keep the dirs-before-files grouping the root load uses;
                    // a plain path sort intermixes a lazily-loaded subdir's dirs
                    // and files.
                    data.tree.sort_by_cached_key(tree_order_key);
                }
            }
            Err(error) => state.error = Some(error),
        },
        ServiceEvent::FileLoaded(result) => match result {
            Ok(page) => {
                if let Resource::Ready(current) = &mut state.file
                    && current.path == page.path
                    && page.loaded_bytes > current.loaded_bytes
                {
                    current.text.push_str(&page.text);
                    current.loaded_bytes = page.loaded_bytes;
                    current.total_bytes = page.total_bytes;
                    current.has_more = page.has_more;
                    state.file_content.set(&current.text);
                } else {
                    state.file_content.set(&page.text);
                    state.file = Resource::Ready(page);
                }
            }
            Err(error) => state.file = Resource::Error(error),
        },
        ServiceEvent::MoreCommitsLoaded(result) => match result {
            Ok((mut commits, has_more)) => {
                if let Resource::Ready(data) = &mut state.repository {
                    data.commits.append(&mut commits);
                    data.commits_have_more = has_more;
                }
            }
            Err(error) => state.error = Some(error),
        },
        ServiceEvent::CommitDiffLoaded { commit_id, result } => {
            // Ignore a stale reply after the reviewer collapsed or switched rows.
            if state.selected_commit.as_deref() == Some(commit_id.as_str()) {
                state.commit_diff = match result {
                    Ok(files) if files.is_empty() => Resource::Empty,
                    Ok(files) => Resource::Ready(files),
                    Err(error) => Resource::Error(error),
                };
            }
        }
        ServiceEvent::WriteFinished(result) => {
            state.busy = false;
            match result {
                Ok(()) => {
                    state.new_item_open = false;
                    state.new_item.title.clear();
                    state.new_item.body.clear();
                    state.comment_content.set("");
                    state.editing_item = false;
                    state.review_open = false;
                    state.review_body.clear();
                    clear_review_draft(state);
                    if let (Some(repository_id), Some(number)) =
                        (state.selected_repo.clone(), state.selected_item)
                    {
                        state.item_detail = Resource::Loading;
                        return Some(Command::LoadItem {
                            repository_id,
                            number,
                        });
                    }
                    if let Some(repository_id) = state.selected_repo.clone() {
                        state.repository = Resource::Loading;
                        return Some(Command::LoadRepository {
                            repository_id,
                            reference: state.selected_branch.clone(),
                        });
                    }
                }
                Err(error) => state.error = Some(error),
            }
        }
        ServiceEvent::ItemLoaded(result) => {
            state.item_detail = match result {
                Ok(Some(detail)) => Resource::Ready(detail),
                Ok(None) => Resource::Empty,
                Err(error) => Resource::Error(error),
            };
        }
    }
    None
}

fn clear_review_draft(state: &mut State) {
    state.review_comment_file = None;
    state.review_comment_line.clear();
    state.review_comment_body.clear();
    state.review_comments.clear();
}

/// Sort key that yields a DFS pre-order with siblings grouped dirs-first then
/// by name: each path component becomes `(0 for a directory, 1 for a file, name)`
/// — intermediate components are always directories, and the last takes the
/// entry's own kind. Ancestors sort before descendants (prefix keys sort first).
fn tree_order_key(entry: &TreeEntry) -> Vec<(u8, String)> {
    let components: Vec<&str> = entry.path.split('/').collect();
    let last = components.len().saturating_sub(1);
    components
        .iter()
        .enumerate()
        .map(|(index, component)| {
            let is_dir = index != last || entry.kind == TreeKind::Directory;
            (u8::from(!is_dir), (*component).to_owned())
        })
        .collect()
}

fn is_child_of(candidate: &str, parent: &str) -> bool {
    candidate
        .strip_prefix(parent)
        .is_some_and(|suffix| suffix.starts_with('/'))
}

fn repository_data(state: &State) -> Option<&RepositoryData> {
    match &state.repository {
        Resource::Ready(data) => Some(data),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> Repository {
        Repository {
            id: "core".into(),
            name: "core".into(),
            default_branch: "dev".into(),
            head: Some("0123456789abcdef".into()),
            browsable: true,
        }
    }

    fn pull_detail() -> ItemDetail {
        ItemDetail {
            item: ForgeItem {
                number: 7,
                kind: ItemKind::PullRequest,
                state: ItemState::Open,
                title: "Original".into(),
                author: "you".into(),
                updated: "now".into(),
                source_branch: Some("feature".into()),
                target_branch: Some("dev".into()),
            },
            body: "body".into(),
            can_edit: true,
            comments: vec![],
            commits: vec![],
            changed_files: vec![],
            compare_error: None,
            reviews: vec![],
        }
    }

    #[test]
    fn repository_selection_resets_browser_and_loads_content() {
        let mut state = State {
            repositories: Resource::Ready(vec![repo()]),
            selected_file: Some("old.rs".into()),
            ..State::default()
        };
        assert_eq!(
            update(&mut state, Message::SelectRepository("core".into())),
            Some(Command::LoadRepository {
                repository_id: "core".into(),
                reference: None,
            })
        );
        assert_eq!(state.file, Resource::Empty);
        assert_eq!(state.tab, Tab::Code);
    }

    #[test]
    fn native_overview_constructs_in_both_themes() {
        let state = State::default();
        let _ = view(&state, crate::theme::Mode::Light);
        let state = State {
            repositories: Resource::Ready(vec![repo()]),
            ..State::default()
        };
        let _ = view(&state, crate::theme::Mode::Dark);
    }

    #[test]
    fn issue_submit_trims_title_and_keeps_markdown_body() {
        let mut state = State {
            selected_repo: Some("core".into()),
            tab: Tab::Issues,
            ..State::default()
        };
        state.new_item.title = "  broken build  ".into();
        state.new_item.body = "**details**".into();
        assert_eq!(
            update(&mut state, Message::SubmitNewItem),
            Some(Command::OpenIssue {
                repository_id: "core".into(),
                title: "broken build".into(),
                body: "**details**".into(),
            })
        );
        assert!(state.busy);
    }

    #[test]
    fn directory_results_replace_only_existing_descendants() {
        let mut state = State {
            repository: Resource::Ready(RepositoryData {
                branches: vec![],
                tree: vec![
                    TreeEntry {
                        path: "src".into(),
                        name: "src".into(),
                        kind: TreeKind::Directory,
                        depth: 0,
                        open: true,
                    },
                    TreeEntry {
                        path: "src/old.rs".into(),
                        name: "old.rs".into(),
                        kind: TreeKind::File,
                        depth: 1,
                        open: false,
                    },
                    TreeEntry {
                        path: "README.md".into(),
                        name: "README.md".into(),
                        kind: TreeKind::File,
                        depth: 0,
                        open: false,
                    },
                ],
                commits: vec![],
                commits_have_more: false,
                items: vec![],
                remote: false,
            }),
            ..State::default()
        };
        update(
            &mut state,
            Message::Service(ServiceEvent::DirectoryLoaded {
                path: "src".into(),
                result: Ok(vec![TreeEntry {
                    path: "src/new.rs".into(),
                    name: "new.rs".into(),
                    kind: TreeKind::File,
                    depth: 1,
                    open: false,
                }]),
            }),
        );
        let Resource::Ready(data) = state.repository else {
            panic!("ready")
        };
        assert!(data.tree.iter().any(|entry| entry.path == "README.md"));
        assert!(data.tree.iter().any(|entry| entry.path == "src/new.rs"));
        assert!(!data.tree.iter().any(|entry| entry.path == "src/old.rs"));
    }

    #[test]
    fn file_paging_appends_without_losing_existing_text() {
        let mut state = State {
            file: Resource::Ready(FilePage {
                path: "README.md".into(),
                text: "first".into(),
                loaded_bytes: 5,
                total_bytes: 11,
                has_more: true,
            }),
            ..State::default()
        };
        update(
            &mut state,
            Message::Service(ServiceEvent::FileLoaded(Ok(FilePage {
                path: "README.md".into(),
                text: " second".into(),
                loaded_bytes: 11,
                total_bytes: 11,
                has_more: false,
            }))),
        );
        assert!(
            matches!(state.file, Resource::Ready(FilePage { text, .. }) if text == "first second")
        );
    }

    #[test]
    fn author_edit_sends_only_changed_fields() {
        let mut state = State {
            selected_repo: Some("core".into()),
            item_detail: Resource::Ready(pull_detail()),
            ..State::default()
        };
        assert_eq!(update(&mut state, Message::StartEditingItem), None);
        assert!(state.editing_item);
        update(&mut state, Message::EditTitleChanged("  Retitled  ".into()));
        assert_eq!(
            update(&mut state, Message::SaveItemEdit),
            Some(Command::EditItem {
                repository_id: "core".into(),
                number: 7,
                title: Some("Retitled".into()),
                body: None,
            })
        );
    }

    #[test]
    fn review_pins_to_the_loaded_source_head() {
        let mut state = State {
            selected_repo: Some("core".into()),
            repository: Resource::Ready(RepositoryData {
                branches: vec![Branch {
                    name: "feature".into(),
                    head: "a".repeat(40),
                }],
                tree: vec![],
                commits: vec![],
                commits_have_more: false,
                items: vec![],
                remote: false,
            }),
            item_detail: Resource::Ready(pull_detail()),
            ..State::default()
        };
        update(&mut state, Message::ToggleReview);
        update(
            &mut state,
            Message::ReviewVerdictChanged(ReviewVerdict::Approve),
        );
        update(&mut state, Message::ReviewBodyChanged("Looks good".into()));
        assert_eq!(
            update(&mut state, Message::SubmitReview),
            Some(Command::SubmitReview {
                repository_id: "core".into(),
                number: 7,
                verdict: ReviewVerdict::Approve,
                body: "Looks good".into(),
                commit_oid: "a".repeat(40),
                comments: vec![],
            })
        );
    }

    #[test]
    fn review_queues_new_side_inline_comments() {
        let mut state = State {
            selected_repo: Some("core".into()),
            repository: Resource::Ready(RepositoryData {
                branches: vec![Branch {
                    name: "feature".into(),
                    head: "b".repeat(40),
                }],
                tree: vec![],
                commits: vec![],
                commits_have_more: false,
                items: vec![],
                remote: false,
            }),
            item_detail: Resource::Ready(pull_detail()),
            ..State::default()
        };
        update(&mut state, Message::ToggleReview);
        update(&mut state, Message::StartReviewComment("src/lib.rs".into()));
        update(&mut state, Message::ReviewCommentLineChanged("17".into()));
        update(
            &mut state,
            Message::ReviewCommentBodyChanged("Handle this error".into()),
        );
        update(&mut state, Message::QueueReviewComment);
        update(&mut state, Message::ReviewBodyChanged("One concern".into()));
        assert_eq!(
            update(&mut state, Message::SubmitReview),
            Some(Command::SubmitReview {
                repository_id: "core".into(),
                number: 7,
                verdict: ReviewVerdict::Comment,
                body: "One concern".into(),
                commit_oid: "b".repeat(40),
                comments: vec![ReviewComment {
                    path: "src/lib.rs".into(),
                    line: 17,
                    side: ReviewSide::New,
                    body: "Handle this error".into(),
                }],
            })
        );
    }

    #[test]
    fn commit_toggle_loads_diff_then_collapses() {
        let mut state = State {
            selected_repo: Some("core".into()),
            ..State::default()
        };
        assert_eq!(
            update(&mut state, Message::ToggleCommit("abc123".into())),
            Some(Command::LoadCommitDiff {
                repository_id: "core".into(),
                commit_id: "abc123".into(),
            })
        );
        assert_eq!(state.selected_commit.as_deref(), Some("abc123"));
        assert!(matches!(state.commit_diff, Resource::Loading));
        // Toggling the same commit collapses it and issues no reload.
        assert_eq!(update(&mut state, Message::ToggleCommit("abc123".into())), None);
        assert_eq!(state.selected_commit, None);
        assert!(matches!(state.commit_diff, Resource::Empty));
    }

    #[test]
    fn commit_toggle_retries_a_failed_diff_instead_of_collapsing() {
        let mut state = State {
            selected_repo: Some("core".into()),
            selected_commit: Some("abc123".into()),
            commit_diff: Resource::Error("diff failed".into()),
            ..State::default()
        };
        assert_eq!(
            update(&mut state, Message::ToggleCommit("abc123".into())),
            Some(Command::LoadCommitDiff {
                repository_id: "core".into(),
                commit_id: "abc123".into(),
            })
        );
        assert!(matches!(state.commit_diff, Resource::Loading));
    }

    #[test]
    fn file_action_ignores_edits_but_the_pane_stays_read_only() {
        let mut state = State::default();
        state.file_content.set("fn main() {}");
        update(
            &mut state,
            Message::FileAction(text_editor::Action::Edit(text_editor::Edit::Insert('X'))),
        );
        assert!(
            !state.file_content.0.text().contains('X'),
            "the read-only file pane must drop every edit action"
        );
        assert!(state.file_content.0.text().starts_with("fn main() {}"));
    }

    #[test]
    fn clicking_an_added_line_stages_a_comment_there() {
        let mut state = State {
            review_open: true,
            ..State::default()
        };
        update(
            &mut state,
            Message::StartLineComment {
                path: "src/lib.rs".into(),
                line: 42,
            },
        );
        assert_eq!(state.review_comment_file.as_deref(), Some("src/lib.rs"));
        assert_eq!(state.review_comment_line, "42");
    }

    #[test]
    fn line_staging_is_inert_until_a_review_is_open() {
        let mut state = State::default();
        update(
            &mut state,
            Message::StartLineComment {
                path: "src/lib.rs".into(),
                line: 42,
            },
        );
        assert_eq!(state.review_comment_file, None);
    }
}
