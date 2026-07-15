//! Native Forge repository browser and issue/pull-request surface.
//!
//! The module owns presentation state only. [`update`] emits typed effects for
//! the shell to execute and accepts their results through [`ServiceEvent`].

use iced::widget::{
    Button, Column, Space, button, column, container, row, scrollable, text, text_input,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Shadow, Vector};

use crate::icons::{self, Icon};
use crate::theme::{self, MONO, Palette, RADIUS_LG, RADIUS_MD, RADIUS_SM, SANS, SANS_SEMIBOLD};

const TREE_WIDTH: f32 = 258.0;
const BODY_PAD: f32 = 24.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resource<T> {
    Loading,
    Empty,
    Error(String),
    Ready(T),
}

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
    pub item_filter: ItemFilter,
    pub new_item_open: bool,
    pub new_item: NewItemDraft,
    pub selected_item: Option<u64>,
    pub item_detail: Resource<ItemDetail>,
    pub pull_tab: PullTab,
    pub comment_draft: String,
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
            item_filter: ItemFilter::Open,
            new_item_open: false,
            new_item: NewItemDraft::default(),
            selected_item: None,
            item_detail: Resource::Empty,
            pull_tab: PullTab::Conversation,
            comment_draft: String::new(),
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

#[derive(Debug, Clone, PartialEq, Eq)]
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
    LoadMoreFile,
    LoadMoreCommits,
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
    CommentChanged(String),
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
            None
        }
        Message::ToggleDirectory(path) => {
            let id = state.selected_repo.clone()?;
            if let Resource::Ready(data) = &mut state.repository {
                if let Some(entry) = data.tree.iter_mut().find(|entry| entry.path == path) {
                    entry.open = !entry.open;
                    if !entry.open {
                        return None;
                    }
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
            clear_review_draft(state);
            Some(Command::LoadItem {
                repository_id: id,
                number,
            })
        }
        Message::CloseItem => {
            state.selected_item = None;
            state.item_detail = Resource::Empty;
            state.comment_draft.clear();
            state.editing_item = false;
            state.review_open = false;
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
            let Some(path) = state.review_comment_file.clone() else {
                return None;
            };
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
        Message::CommentChanged(value) => {
            state.comment_draft = value;
            None
        }
        Message::SubmitComment => {
            let id = state.selected_repo.clone()?;
            let number = state.selected_item?;
            let body = state.comment_draft.trim().to_owned();
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
                    data.tree.sort_by(|a, b| a.path.cmp(&b.path));
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
                } else {
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
        ServiceEvent::WriteFinished(result) => {
            state.busy = false;
            match result {
                Ok(()) => {
                    state.new_item_open = false;
                    state.new_item.title.clear();
                    state.new_item.body.clear();
                    state.comment_draft.clear();
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

fn is_child_of(candidate: &str, parent: &str) -> bool {
    candidate
        .strip_prefix(parent)
        .is_some_and(|suffix| suffix.starts_with('/'))
}

pub fn view(state: &State, mode: theme::Mode) -> Element<'_, Message> {
    let palette = *theme::palette(mode);
    let Some(repo_id) = &state.selected_repo else {
        return overview(state, palette);
    };
    let repo = repositories(state).iter().find(|repo| &repo.id == repo_id);
    match repo {
        Some(repo) => listing(state, repo, palette),
        None => center_note(
            if matches!(state.repositories, Resource::Loading) {
                "Loading repository..."
            } else {
                "Repository not found"
            },
            None,
            palette,
        ),
    }
}

fn overview(state: &State, p: Palette) -> Element<'_, Message> {
    let count = repositories(state).len();
    let header = row![
        icon_tile(Icon::Forge, 30.0, p),
        text("ducktape").font(SANS_SEMIBOLD).size(18).color(p.ink),
        status_pill("ORG", p.purple, p),
        Space::new().width(Length::Fill),
        text(match &state.repositories {
            Resource::Ready(_) => format!("{count} repositories"),
            _ => "local forge repositories".into(),
        })
        .font(MONO)
        .size(10.5)
        .color(p.muted_2),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    let mut body = column![
        header,
        text("Browse repositories backed by this node's local git forge.")
            .font(SANS)
            .size(12.5)
            .color(p.muted)
    ]
    .spacing(7)
    .padding([22, 26]);
    body = body.push(Space::new().height(11));
    match &state.repositories {
        Resource::Loading => body = body.push(center_note("Loading repositories...", None, p)),
        Resource::Empty => {
            body = body.push(center_note(
                "No local forge repositories",
                Some("This node did not report a browsable git repository."),
                p,
            ));
        }
        Resource::Error(error) => body = body.push(error_banner(error, p)),
        Resource::Ready(repos) => {
            body = body.push(section_label("REPOSITORIES", p));
            for repo in repos {
                body = body.push(repo_card(repo, p));
            }
        }
    }
    container(scrollable(body))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn repo_card(repo: &Repository, p: Palette) -> Element<'static, Message> {
    let id = repo.id.clone();
    let meta = row![
        status_dot(if repo.browsable { p.green } else { p.amber }),
        text(repo.default_branch.clone())
            .font(MONO)
            .size(10.5)
            .color(p.muted),
        text(if repo.browsable {
            "browsable"
        } else {
            "no HEAD"
        })
        .font(MONO)
        .size(10.5)
        .color(p.muted),
        Space::new().width(Length::Fill),
        text(short_hash(repo.head.as_deref()))
            .font(MONO)
            .size(10.5)
            .color(p.muted_2),
    ]
    .spacing(14)
    .align_y(Alignment::Center);
    button(
        column![
            row![
                icons::view(Icon::Forge, 14.0, p.muted),
                text(format!("ducktape/{}", repo.name))
                    .font(SANS_SEMIBOLD)
                    .size(14.5)
                    .color(p.ink)
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            meta
        ]
        .spacing(12),
    )
    .width(Length::Fill)
    .padding([15, 17])
    .style(move |_, status| button::Style {
        background: Some(Background::Color(
            if matches!(status, button::Status::Hovered) {
                p.sunken
            } else {
                p.paper
            },
        )),
        text_color: p.ink,
        border: Border {
            color: if matches!(status, button::Status::Hovered) {
                p.border_strong
            } else {
                p.border
            },
            width: 1.0,
            radius: RADIUS_LG.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba8(0, 0, 0, 0.07),
            offset: Vector::new(0.0, 1.0),
            blur_radius: 3.0,
        },
        ..Default::default()
    })
    .on_press(Message::SelectRepository(id))
    .into()
}

fn listing<'a>(state: &'a State, repo: &'a Repository, p: Palette) -> Element<'a, Message> {
    let data = match &state.repository {
        Resource::Ready(data) => Some(data),
        _ => None,
    };
    let head = state
        .selected_branch
        .as_ref()
        .and_then(|branch| data?.branches.iter().find(|item| &item.name == branch))
        .map(|branch| branch.head.as_str())
        .or(repo.head.as_deref());
    let branch = state
        .selected_branch
        .as_deref()
        .unwrap_or(&repo.default_branch);
    let open_issues = data.map_or(0, |data| {
        data.items
            .iter()
            .filter(|item| item.kind == ItemKind::Issue && item.state == ItemState::Open)
            .count()
    });
    let open_pulls = data.map_or(0, |data| {
        data.items
            .iter()
            .filter(|item| item.kind == ItemKind::PullRequest && item.state == ItemState::Open)
            .count()
    });
    let commits = data.map_or(0, |data| data.commits.len());

    let repo_button = button(
        row![
            text(repo.name.clone()).font(SANS_SEMIBOLD).size(15),
            icons::view(Icon::ChevronRight, 11.0, p.ink)
        ]
        .spacing(5)
        .align_y(Alignment::Center),
    )
    .padding(0)
    .style(|_, _| button::Style::default())
    .on_press(Message::ToggleRepositoryMenu);
    let branch_button = button(
        row![
            status_dot(p.green),
            text(branch.to_owned()).font(MONO).size(10),
            icons::view(Icon::ChevronRight, 10.0, p.ink)
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding([3, 8])
    .style(move |_, status| outlined_button(status, p))
    .on_press(Message::ToggleBranchMenu);
    let heading = row![
        icon_tile(Icon::Forge, 28.0, p),
        text_button("ducktape", Message::BackToRepositories, p),
        text("/").font(SANS).size(15).color(p.icon_idle),
        repo_button,
        branch_button,
        status_pill(&short_hash(head), p.muted_3, p),
        Space::new().width(Length::Fill),
        status_pill("DESKTOP", p.purple, p),
    ]
    .spacing(9)
    .align_y(Alignment::Center);

    let tabs = row![
        tab_button(Tab::Code, state.tab, None, p),
        tab_button(Tab::Commits, state.tab, Some(commits), p),
        tab_button(Tab::Issues, state.tab, Some(open_issues), p),
        tab_button(Tab::Pulls, state.tab, Some(open_pulls), p),
    ]
    .spacing(22)
    .align_y(Alignment::End);
    let chrome = column![heading, tabs].spacing(13).padding(Padding {
        top: 16.0,
        right: 24.0,
        bottom: 0.0,
        left: 24.0,
    });

    let body = if state.selected_item.is_some() {
        item_detail(state, p)
    } else {
        match state.tab {
            Tab::Code => code_view(state, repo, p),
            Tab::Commits => commits_view(state, repo, p),
            Tab::Issues => items_view(state, repo, ItemKind::Issue, p),
            Tab::Pulls => items_view(state, repo, ItemKind::PullRequest, p),
        }
    };
    let mut page = column![chrome];
    if state.repo_menu_open {
        page = page.push(repo_menu(state, p));
    }
    if state.branch_menu_open {
        page = page.push(branch_menu(state, p));
    }
    page.push(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn code_view<'a>(state: &'a State, repo: &'a Repository, p: Palette) -> Element<'a, Message> {
    if !repo.browsable {
        return center_note(
            "No committed tree",
            Some("This local forge repository has no HEAD yet, so there is no code to browse."),
            p,
        );
    }
    let tree: Element<'a, Message> = match &state.repository {
        Resource::Loading => center_note("Loading repository...", None, p),
        Resource::Error(error) => error_banner(error, p),
        Resource::Empty => center_note("Empty repository", None, p),
        Resource::Ready(data) => {
            let mut rows = column![
                row![
                    section_label("FILES", p),
                    Space::new().width(Length::Fill),
                    text(data.tree.len()).font(MONO).size(10).color(p.muted_2)
                ]
                .padding(Padding {
                    top: 0.0,
                    right: 16.0,
                    bottom: 9.0,
                    left: 16.0,
                })
            ];
            for entry in visible_tree(&data.tree) {
                rows = rows.push(tree_row(entry, state.selected_file.as_deref(), p));
            }
            scrollable(rows).into()
        }
    };
    let tree = container(tree)
        .width(TREE_WIDTH)
        .height(Length::Fill)
        .padding([11, 0])
        .style(move |_| container::Style {
            background: Some(Background::Color(p.sidebar)),
            border: Border {
                color: p.border_soft,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        });
    row![tree, file_viewer(state, repo, p)]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn visible_tree(entries: &[TreeEntry]) -> Vec<&TreeEntry> {
    entries
        .iter()
        .filter(|entry| {
            let mut cursor = entry.path.as_str();
            while let Some((parent, _)) = cursor.rsplit_once('/') {
                if !entries
                    .iter()
                    .any(|candidate| candidate.path == parent && candidate.open)
                {
                    return false;
                }
                cursor = parent;
            }
            true
        })
        .collect()
}

fn tree_row(entry: &TreeEntry, selected: Option<&str>, p: Palette) -> Element<'static, Message> {
    let is_selected = selected == Some(entry.path.as_str());
    let message = if entry.kind == TreeKind::Directory {
        Message::ToggleDirectory(entry.path.clone())
    } else {
        Message::SelectFile(entry.path.clone())
    };
    let lead: Element<'static, Message> = if entry.kind == TreeKind::Directory {
        text(if entry.open { "⌄" } else { "›" })
            .font(MONO)
            .size(12)
            .color(p.muted_2)
            .into()
    } else {
        Space::new().width(11).into()
    };
    button(
        row![
            Space::new().width((entry.depth * 15) as f32),
            lead,
            icons::view(
                if entry.kind == TreeKind::Directory {
                    Icon::Modules
                } else {
                    Icon::Files
                },
                13.0,
                if entry.kind == TreeKind::Directory {
                    p.filled
                } else {
                    p.muted
                }
            ),
            text(entry.name.clone())
                .font(if entry.kind == TreeKind::Directory {
                    SANS_SEMIBOLD
                } else {
                    MONO
                })
                .size(if entry.kind == TreeKind::Directory {
                    12.5
                } else {
                    12.0
                })
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([5, 13])
    .style(move |_, status| button::Style {
        background: (is_selected || matches!(status, button::Status::Hovered)).then_some(
            Background::Color(if is_selected { p.hover } else { p.sunken }),
        ),
        text_color: if is_selected { p.ink } else { p.ink_softer },
        ..Default::default()
    })
    .on_press(message)
    .into()
}

fn file_viewer<'a>(state: &'a State, repo: &'a Repository, p: Palette) -> Element<'a, Message> {
    let title = state.selected_file.as_ref().map_or_else(
        || "Select a file".into(),
        |path| format!("{}/{path}", repo.name),
    );
    let latest = repository_data(state)
        .and_then(|data| data.commits.first())
        .map(|commit| format!("{} · {} · {}", commit.summary, commit.author, commit.time))
        .unwrap_or_default();
    let header = row![
        text(title)
            .font(MONO)
            .size(12)
            .color(if state.selected_file.is_some() {
                p.ink_soft
            } else {
                p.muted_2
            }),
        Space::new().width(Length::Fill),
        text(latest).font(MONO).size(10).color(p.muted_2)
    ]
    .spacing(10)
    .padding([8, 16])
    .align_y(Alignment::Center);
    let content: Element<'a, Message> = match &state.file {
        Resource::Loading => center_note("Loading file...", None, p),
        Resource::Error(error) => error_banner(error, p),
        Resource::Empty => center_note(
            state
                .selected_file
                .as_deref()
                .and_then(|path| path.rsplit('/').next())
                .unwrap_or("Select a file"),
            None,
            p,
        ),
        Resource::Ready(file) => {
            let mut code = column![
                container(
                    text(file.text.clone())
                        .font(MONO)
                        .size(12)
                        .color(p.ink_soft)
                )
                .padding([14, 18])
            ];
            if file.has_more {
                code = code.push(
                    row![
                        Space::new().width(Length::Fill),
                        text(format!(
                            "{} / {} bytes",
                            file.loaded_bytes, file.total_bytes
                        ))
                        .font(MONO)
                        .size(10.5)
                        .color(p.muted_2),
                        secondary_button("Load more file", Some(Message::LoadMoreFile), p),
                        Space::new().width(Length::Fill)
                    ]
                    .spacing(10)
                    .padding([10, 16])
                    .align_y(Alignment::Center),
                );
            }
            scrollable(code).into()
        }
    };
    column![header, horizontal_divider(p), content]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn commits_view<'a>(state: &'a State, repo: &'a Repository, p: Palette) -> Element<'a, Message> {
    let Some(data) = repository_data(state) else {
        return match &state.repository {
            Resource::Loading => center_note("Loading commits...", None, p),
            Resource::Error(error) => error_banner(error, p),
            _ => center_note("No commits yet", None, p),
        };
    };
    let mut body = column![
        row![
            column![
                text("Commit history")
                    .font(SANS_SEMIBOLD)
                    .size(15)
                    .color(p.ink),
                text("Read-only log from the local git repository.")
                    .font(SANS)
                    .size(11.5)
                    .color(p.muted)
            ]
            .spacing(4),
            Space::new().width(Length::Fill),
            status_pill(&format!("{} COMMITS", data.commits.len()), p.blue, p)
        ]
        .align_y(Alignment::Center)
    ]
    .spacing(0)
    .padding([18.0, BODY_PAD]);
    if data.commits.is_empty() {
        body = body.push(center_note(
            if repo.browsable {
                "No commits yet"
            } else {
                "No committed tree"
            },
            (!repo.browsable).then_some("This node has no local forge HEAD to browse."),
            p,
        ));
    } else {
        for commit in &data.commits {
            body = body.push(commit_row(commit, p));
        }
        if data.commits_have_more {
            body = body.push(
                row![
                    Space::new().width(Length::Fill),
                    secondary_button("Load more commits", Some(Message::LoadMoreCommits), p),
                    Space::new().width(Length::Fill)
                ]
                .padding([12, 0]),
            );
        }
    }
    scrollable(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn commit_row(commit: &Commit, p: Palette) -> Element<'static, Message> {
    row![
        container(icons::view(Icon::Forge, 13.0, p.blue))
            .width(24)
            .height(24)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(move |_| tinted_box(p.blue, p)),
        column![
            text(commit.summary.clone())
                .font(SANS_SEMIBOLD)
                .size(14)
                .color(p.ink),
            text(format!(
                "{} · {} · {}",
                short_hash(Some(&commit.id)),
                commit.author,
                commit.time
            ))
            .font(MONO)
            .size(11)
            .color(p.muted_2)
        ]
        .spacing(4)
    ]
    .spacing(13)
    .padding([13, 0])
    .into()
}

fn items_view<'a>(
    state: &'a State,
    repo: &'a Repository,
    kind: ItemKind,
    p: Palette,
) -> Element<'a, Message> {
    let Some(data) = repository_data(state) else {
        return center_note("Loading items...", None, p);
    };
    let all: Vec<&ForgeItem> = data.items.iter().filter(|item| item.kind == kind).collect();
    let open_count = all
        .iter()
        .filter(|item| item.state == ItemState::Open)
        .count();
    let closed_count = all.len() - open_count;
    let visible: Vec<&ForgeItem> = all
        .iter()
        .copied()
        .filter(|item| match state.item_filter {
            ItemFilter::Open => item.state == ItemState::Open,
            ItemFilter::Closed => item.state != ItemState::Open,
        })
        .collect();
    let mut body = column![
        row![
            filter_button("Open", open_count, ItemFilter::Open, state.item_filter, p),
            filter_button(
                "Closed",
                closed_count,
                ItemFilter::Closed,
                state.item_filter,
                p
            ),
            Space::new().width(Length::Fill),
            primary_button(
                if kind == ItemKind::Issue {
                    "New issue"
                } else {
                    "New pull request"
                },
                Some(Message::ToggleNewItem),
                p,
            )
        ]
        .spacing(8)
        .align_y(Alignment::Center)
    ]
    .spacing(12)
    .padding(Padding {
        top: 16.0,
        right: 24.0,
        bottom: 24.0,
        left: 24.0,
    });
    if state.new_item_open {
        body = body.push(new_item_form(state, kind, data, p));
    }
    if let Some(error) = &state.error {
        body = body.push(error_banner(error, p));
    }
    let mut list = column![];
    if visible.is_empty() {
        let (title, detail) = if all.is_empty() {
            if kind == ItemKind::Issue {
                (
                    "No issues yet",
                    format!(
                        "Open the first issue to start tracking work in {}.",
                        repo.name
                    ),
                )
            } else {
                (
                    "No pull requests yet",
                    format!(
                        "Push a branch and open a pull request to propose changes to {}.",
                        repo.name
                    ),
                )
            }
        } else {
            (
                if state.item_filter == ItemFilter::Open {
                    "No open items"
                } else {
                    "No closed items"
                },
                String::new(),
            )
        };
        list = list.push(center_note(
            title,
            (!detail.is_empty()).then_some(detail.as_str()),
            p,
        ));
    } else {
        for item in visible {
            list = list.push(item_row(item, p));
        }
    }
    body = body.push(card(list, p));
    scrollable(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn new_item_form<'a>(
    state: &'a State,
    kind: ItemKind,
    data: &'a RepositoryData,
    p: Palette,
) -> Element<'a, Message> {
    let mut form = column![
        text(if kind == ItemKind::Issue {
            "New issue"
        } else {
            "New pull request"
        })
        .font(SANS_SEMIBOLD)
        .size(13)
        .color(p.ink)
    ]
    .spacing(8);
    if kind == ItemKind::PullRequest {
        form = form.push(
            row![
                text("Merge").font(SANS_SEMIBOLD).size(11).color(p.muted_3),
                text_input("Source branch", &state.new_item.source_branch)
                    .font(MONO)
                    .size(12)
                    .on_input(Message::SourceBranchChanged),
                text("into").font(SANS_SEMIBOLD).size(11).color(p.muted_3),
                text_input("Target", &state.new_item.target_branch)
                    .font(MONO)
                    .size(12)
                    .on_input(Message::TargetBranchChanged)
            ]
            .spacing(9)
            .align_y(Alignment::Center),
        );
        if data.branches.len() < 2 {
            form = form.push(
                text("No source branches — push a branch besides dev to open a pull request.")
                    .font(SANS)
                    .size(12)
                    .color(p.muted),
            );
        }
    }
    form = form
        .push(
            text_input("Title", &state.new_item.title)
                .font(SANS)
                .size(12.5)
                .on_input(Message::NewTitleChanged),
        )
        .push(
            text_input("Description (markdown)", &state.new_item.body)
                .font(SANS)
                .size(12.5)
                .on_input(Message::NewBodyChanged),
        )
        .push(
            row![
                Space::new().width(Length::Fill),
                secondary_button("Cancel", Some(Message::ToggleNewItem), p),
                primary_button(
                    if state.busy {
                        "Opening..."
                    } else if kind == ItemKind::Issue {
                        "Open issue"
                    } else {
                        "Open pull request"
                    },
                    (!state.busy && !state.new_item.title.trim().is_empty())
                        .then_some(Message::SubmitNewItem),
                    p,
                )
            ]
            .spacing(8),
        );
    container(form)
        .max_width(720)
        .padding([13, 15])
        .style(move |_| container::Style {
            background: Some(Background::Color(p.sidebar)),
            border: Border {
                color: p.border_strong,
                width: 1.0,
                radius: RADIUS_MD.into(),
            },
            ..Default::default()
        })
        .into()
}

fn item_row(item: &ForgeItem, p: Palette) -> Element<'static, Message> {
    let number = item.number;
    button(
        row![
            status_dot(match item.state {
                ItemState::Open => p.green,
                ItemState::Closed => p.red,
                ItemState::Merged => p.purple,
            }),
            column![
                text(item.title.clone())
                    .font(SANS_SEMIBOLD)
                    .size(13.5)
                    .color(p.ink),
                text(format!(
                    "#{} opened by {} · {}",
                    item.number, item.author, item.updated
                ))
                .font(MONO)
                .size(10.5)
                .color(p.muted_2)
            ]
            .spacing(4)
            .width(Length::Fill),
        ]
        .spacing(11)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([12, 14])
    .style(move |_, status| button::Style {
        background: matches!(status, button::Status::Hovered)
            .then_some(Background::Color(p.sunken)),
        text_color: p.ink,
        border: Border {
            color: p.border_soft,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    })
    .on_press(Message::OpenItem(number))
    .into()
}

fn item_detail(state: &State, p: Palette) -> Element<'_, Message> {
    match &state.item_detail {
        Resource::Loading => center_note("Loading item...", None, p),
        Resource::Error(error) => column![
            text_button("‹ Back", Message::CloseItem, p),
            error_banner(error, p)
        ]
        .spacing(10)
        .padding([16, 24])
        .into(),
        Resource::Empty => column![
            text_button("‹ Back", Message::CloseItem, p),
            center_note("Item not found", None, p)
        ]
        .spacing(10)
        .padding([16, 24])
        .into(),
        Resource::Ready(detail) => {
            let item = &detail.item;
            let mut body = column![text_button(
                if item.kind == ItemKind::Issue {
                    "‹ Issues"
                } else {
                    "‹ Pull requests"
                },
                Message::CloseItem,
                p,
            )]
            .spacing(10)
            .padding(Padding {
                top: 16.0,
                right: 24.0,
                bottom: 30.0,
                left: 24.0,
            });
            if state.editing_item {
                body = body.push(card(
                    column![
                        text_input("Title", &state.edit_title)
                            .font(SANS)
                            .size(13)
                            .on_input(Message::EditTitleChanged),
                        text_input("Description (markdown)", &state.edit_body)
                            .font(SANS)
                            .size(12.5)
                            .on_input(Message::EditBodyChanged),
                        row![
                            Space::new().width(Length::Fill),
                            secondary_button(
                                "Cancel",
                                (!state.busy).then_some(Message::CancelEditingItem),
                                p,
                            ),
                            primary_button(
                                if state.busy { "Saving..." } else { "Save" },
                                (!state.busy && !state.edit_title.trim().is_empty())
                                    .then_some(Message::SaveItemEdit),
                                p,
                            )
                        ]
                        .spacing(8)
                    ]
                    .spacing(9)
                    .padding([12, 14]),
                    p,
                ));
            } else {
                body = body
                    .push(
                        row![
                            text(format!("{}  #{}", item.title, item.number))
                                .font(SANS_SEMIBOLD)
                                .size(19)
                                .color(p.ink)
                                .width(Length::Fill),
                            secondary_button(
                                "Edit",
                                (detail.can_edit && !state.busy)
                                    .then_some(Message::StartEditingItem),
                                p,
                            ),
                            secondary_button(
                                if item.state == ItemState::Open {
                                    if item.kind == ItemKind::Issue {
                                        "Close issue"
                                    } else {
                                        "Close"
                                    }
                                } else {
                                    "Reopen"
                                },
                                (item.state != ItemState::Merged && !state.busy)
                                    .then_some(Message::ToggleItemState),
                                p,
                            )
                        ]
                        .spacing(10)
                        .align_y(Alignment::Start),
                    )
                    .push(
                        row![
                            status_pill(
                                item.state.label(),
                                match item.state {
                                    ItemState::Open => p.green,
                                    ItemState::Closed => p.red,
                                    ItemState::Merged => p.purple,
                                },
                                p
                            ),
                            text(format!("{} opened this · {}", item.author, item.updated))
                                .font(SANS)
                                .size(11.5)
                                .color(p.muted_2)
                        ]
                        .spacing(9)
                        .align_y(Alignment::Center),
                    )
                    .push(card(
                        container(
                            text(if detail.body.trim().is_empty() {
                                "No description provided."
                            } else {
                                &detail.body
                            })
                            .font(if detail.body.len() > 200_000 {
                                MONO
                            } else {
                                SANS
                            })
                            .size(12)
                            .color(p.ink_soft),
                        )
                        .padding([12, 15]),
                        p,
                    ));
            }
            if item.kind == ItemKind::PullRequest {
                body = body.push(
                    row![
                        pull_tab_button("Conversation", PullTab::Conversation, state.pull_tab, p),
                        pull_tab_button("Commits", PullTab::Commits, state.pull_tab, p),
                        pull_tab_button("Files changed", PullTab::Files, state.pull_tab, p)
                    ]
                    .spacing(6),
                );
            }
            match state.pull_tab {
                PullTab::Conversation => {
                    if item.kind == ItemKind::PullRequest && item.state == ItemState::Open {
                        let approvals = detail
                            .reviews
                            .iter()
                            .filter(|review| review.verdict == ReviewVerdict::Approve)
                            .count();
                        let change_requests = detail
                            .reviews
                            .iter()
                            .filter(|review| review.verdict == ReviewVerdict::RequestChanges)
                            .count();
                        body = body.push(card(
                            row![
                                column![
                                    section_label("MERGE", p),
                                    text(format!(
                                        "{} into {}",
                                        item.source_branch.as_deref().unwrap_or("source"),
                                        item.target_branch.as_deref().unwrap_or("dev")
                                    ))
                                    .font(MONO)
                                    .size(11.5)
                                    .color(p.ink),
                                    text(format!(
                                        "{approvals} approvals · {change_requests} change requests"
                                    ))
                                    .font(SANS)
                                    .size(10.5)
                                    .color(p.muted_2)
                                ]
                                .spacing(5),
                                Space::new().width(Length::Fill),
                                primary_button(
                                    "Merge pull request",
                                    (!state.busy).then_some(Message::MergePullRequest),
                                    p,
                                )
                            ]
                            .padding([12, 14])
                            .align_y(Alignment::Center),
                            p,
                        ));
                    }
                    for comment in &detail.comments {
                        body = body.push(discussion_post(comment, p));
                    }
                    body = body.push(
                        row![
                            text_input("Leave a comment", &state.comment_draft)
                                .font(SANS)
                                .size(12.5)
                                .on_input(Message::CommentChanged),
                            primary_button(
                                if state.busy { "Posting..." } else { "Comment" },
                                (!state.busy && !state.comment_draft.trim().is_empty())
                                    .then_some(Message::SubmitComment),
                                p,
                            )
                        ]
                        .spacing(8),
                    );
                }
                PullTab::Commits => {
                    if let Some(error) = &detail.compare_error {
                        body = body.push(error_banner(error, p));
                    } else if detail.commits.is_empty() {
                        body = body.push(center_note("No commits", None, p));
                    } else {
                        for commit in &detail.commits {
                            body = body.push(commit_row(commit, p));
                        }
                    }
                }
                PullTab::Files => {
                    body = body.push(review_controls(state, detail, p));
                    for review in &detail.reviews {
                        body = body.push(review_card(review, p));
                    }
                    if let Some(error) = &detail.compare_error {
                        body = body.push(error_banner(error, p));
                    } else if detail.changed_files.is_empty() {
                        body = body.push(center_note("No changes", None, p));
                    } else {
                        for file in &detail.changed_files {
                            body = body.push(changed_file(file, state, p));
                        }
                    }
                }
            }
            if let Some(error) = &state.error {
                body = body.push(error_banner(error, p));
            }
            scrollable(body)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        }
    }
}

fn review_controls<'a>(
    state: &'a State,
    detail: &'a ItemDetail,
    p: Palette,
) -> Element<'a, Message> {
    if !state.review_open {
        let mut bar = row![
            text(format!("{} reviews", detail.reviews.len()))
                .font(MONO)
                .size(10.5)
                .color(p.muted_2),
            Space::new().width(Length::Fill)
        ];
        if detail.item.state == ItemState::Open {
            bar = bar.push(primary_button(
                "Review changes",
                (!state.busy).then_some(Message::ToggleReview),
                p,
            ));
        }
        return bar.align_y(Alignment::Center).into();
    }

    let has_source_head = detail
        .item
        .source_branch
        .as_deref()
        .and_then(|source| {
            repository_data(state)?
                .branches
                .iter()
                .find(|branch| branch.name == source)
        })
        .is_some();
    card(
        column![
            section_label("SUBMIT REVIEW", p),
            row![
                review_verdict_button(ReviewVerdict::Approve, state.review_verdict, p),
                review_verdict_button(ReviewVerdict::RequestChanges, state.review_verdict, p,),
                review_verdict_button(ReviewVerdict::Comment, state.review_verdict, p),
            ]
            .spacing(7),
            text_input("Leave a review summary", &state.review_body)
                .font(SANS)
                .size(12.5)
                .on_input(Message::ReviewBodyChanged),
            text(format!(
                "{} new-side inline comment{} queued",
                state.review_comments.len(),
                if state.review_comments.len() == 1 {
                    ""
                } else {
                    "s"
                }
            ))
            .font(MONO)
            .size(9.5)
            .color(p.muted_2),
            row![
                text(if has_source_head {
                    "Review pins to the current source head."
                } else {
                    "Current source head is unavailable; refresh the repository before reviewing."
                })
                .font(SANS)
                .size(10.5)
                .color(if has_source_head { p.muted_2 } else { p.amber }),
                Space::new().width(Length::Fill),
                secondary_button("Cancel", (!state.busy).then_some(Message::ToggleReview), p,),
                primary_button(
                    if state.busy {
                        "Submitting..."
                    } else {
                        "Submit review"
                    },
                    (!state.busy && has_source_head && !state.review_body.trim().is_empty())
                        .then_some(Message::SubmitReview),
                    p,
                )
            ]
            .spacing(8)
            .align_y(Alignment::Center)
        ]
        .spacing(9)
        .padding([12, 14]),
        p,
    )
}

fn review_verdict_button(
    verdict: ReviewVerdict,
    active: ReviewVerdict,
    p: Palette,
) -> Button<'static, Message> {
    button(text(verdict.label()).font(SANS_SEMIBOLD).size(10.5))
        .padding([5, 9])
        .style(move |_, _| button::Style {
            background: (verdict == active).then_some(Background::Color(p.filled)),
            text_color: if verdict == active {
                p.on_filled
            } else {
                p.muted_2
            },
            border: Border {
                color: p.border,
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            ..Default::default()
        })
        .on_press(Message::ReviewVerdictChanged(verdict))
}

fn review_card(review: &Review, p: Palette) -> Element<'static, Message> {
    let tone = match review.verdict {
        ReviewVerdict::Approve => p.green,
        ReviewVerdict::RequestChanges => p.red,
        ReviewVerdict::Comment => p.blue,
    };
    let mut content = column![
        row![
            text(review.author.clone())
                .font(SANS_SEMIBOLD)
                .size(11.5)
                .color(p.ink),
            status_pill(review.verdict.label(), tone, p),
            Space::new().width(Length::Fill),
            text(review.created_at.clone())
                .font(MONO)
                .size(10)
                .color(p.muted_2),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        text(review.body.clone())
            .font(SANS)
            .size(12)
            .color(p.ink_soft),
        text(format!("reviewed {}", short_hash(Some(&review.commit_oid))))
            .font(MONO)
            .size(9.5)
            .color(p.muted_2),
    ]
    .spacing(6)
    .padding([10, 12]);
    for comment in &review.comments {
        content = content.push(
            column![
                text(format!(
                    "{}:{} ({})",
                    comment.path,
                    comment.line,
                    match comment.side {
                        ReviewSide::Old => "old",
                        ReviewSide::New => "new",
                    }
                ))
                .font(MONO)
                .size(9.5)
                .color(p.muted_2),
                text(comment.body.clone())
                    .font(SANS)
                    .size(11.5)
                    .color(p.ink_soft)
            ]
            .spacing(3)
            .padding([4, 8]),
        );
    }
    card(content, p)
}

fn discussion_post(post: &DiscussionPost, p: Palette) -> Element<'static, Message> {
    card(
        column![
            row![
                text(post.author.clone())
                    .font(SANS_SEMIBOLD)
                    .size(11.5)
                    .color(p.ink),
                Space::new().width(Length::Fill),
                text(post.time.clone()).font(MONO).size(10).color(p.muted_2)
            ],
            text(post.body.clone())
                .font(SANS)
                .size(12)
                .color(p.ink_soft)
        ]
        .spacing(7)
        .padding([10, 12]),
        p,
    )
}

fn changed_file<'a>(file: &'a ChangedFile, state: &'a State, p: Palette) -> Element<'a, Message> {
    let mut content = column![
        row![
            text(file.path.clone())
                .font(MONO)
                .size(12)
                .color(p.ink)
                .width(Length::Fill),
            text(format!("+{}", file.additions))
                .font(MONO)
                .size(11)
                .color(p.green),
            text(format!("−{}", file.deletions))
                .font(MONO)
                .size(11)
                .color(p.red)
        ]
        .spacing(9)
        .padding([8, 12]),
        horizontal_divider(p),
        container(
            text(file.patch.clone())
                .font(MONO)
                .size(11.5)
                .color(p.ink_softer)
        )
        .padding([10, 14])
    ];
    for (index, comment) in state
        .review_comments
        .iter()
        .enumerate()
        .filter(|(_, comment)| comment.path == file.path)
    {
        content = content.push(
            row![
                text(format!("NEW L{}", comment.line))
                    .font(MONO)
                    .size(9.5)
                    .color(p.blue),
                text(comment.body.clone())
                    .font(SANS)
                    .size(11.5)
                    .color(p.ink_soft)
                    .width(Length::Fill),
                secondary_button(
                    "Remove",
                    (!state.busy).then_some(Message::RemoveReviewComment(index)),
                    p,
                )
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .padding([7, 12]),
        );
    }
    if state.review_open && state.review_comment_file.as_deref() == Some(file.path.as_str()) {
        content = content.push(
            column![
                section_label("COMMENT ON NEW VERSION", p),
                row![
                    text_input("Line", &state.review_comment_line)
                        .font(MONO)
                        .size(11.5)
                        .width(72)
                        .on_input(Message::ReviewCommentLineChanged),
                    text_input("Inline comment", &state.review_comment_body)
                        .font(SANS)
                        .size(11.5)
                        .on_input(Message::ReviewCommentBodyChanged)
                        .on_submit(Message::QueueReviewComment),
                ]
                .spacing(8),
                row![
                    text("Line numbers refer to the new file shown by the diff.")
                        .font(SANS)
                        .size(9.5)
                        .color(p.muted_2),
                    Space::new().width(Length::Fill),
                    secondary_button(
                        "Cancel",
                        (!state.busy).then_some(Message::CancelReviewComment),
                        p,
                    ),
                    primary_button(
                        "Queue comment",
                        (!state.busy
                            && !state.review_comment_line.is_empty()
                            && !state.review_comment_body.trim().is_empty())
                        .then_some(Message::QueueReviewComment),
                        p,
                    )
                ]
                .spacing(8)
                .align_y(Alignment::Center)
            ]
            .spacing(8)
            .padding([10, 12]),
        );
    } else if state.review_open {
        content = content.push(
            row![
                Space::new().width(Length::Fill),
                secondary_button(
                    "Comment on new line",
                    (!state.busy).then_some(Message::StartReviewComment(file.path.clone())),
                    p,
                )
            ]
            .padding([8, 12]),
        );
    }
    card(content, p)
}

fn repositories(state: &State) -> &[Repository] {
    match &state.repositories {
        Resource::Ready(repos) => repos,
        _ => &[],
    }
}

fn repository_data(state: &State) -> Option<&RepositoryData> {
    match &state.repository {
        Resource::Ready(data) => Some(data),
        _ => None,
    }
}

fn repo_menu(state: &State, p: Palette) -> Element<'_, Message> {
    let mut menu = column![section_label(
        &format!("REPOSITORIES - {}", repositories(state).len()),
        p,
    )]
    .spacing(3)
    .padding(6);
    for repo in repositories(state) {
        menu = menu.push(
            button(
                row![
                    status_dot(if repo.browsable { p.green } else { p.amber }),
                    text(repo.name.clone())
                        .font(SANS_SEMIBOLD)
                        .size(12.5)
                        .width(Length::Fill),
                    text(repo.default_branch.clone())
                        .font(MONO)
                        .size(10)
                        .color(p.muted_2)
                ]
                .spacing(9)
                .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .padding([8, 9])
            .style(move |_, status| button::Style {
                background: matches!(status, button::Status::Hovered)
                    .then_some(Background::Color(p.panel)),
                text_color: p.ink,
                border: Border {
                    radius: RADIUS_SM.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .on_press(Message::SelectRepository(repo.id.clone())),
        );
    }
    floating_menu(menu, p)
}

fn branch_menu(state: &State, p: Palette) -> Element<'_, Message> {
    let branches = repository_data(state).map_or(&[][..], |data| data.branches.as_slice());
    let mut menu = column![section_label(&format!("BRANCHES - {}", branches.len()), p)]
        .spacing(3)
        .padding(6);
    if branches.is_empty() {
        menu = menu.push(
            text("No local branches")
                .font(SANS)
                .size(11)
                .color(p.muted_2),
        );
    }
    for branch in branches {
        menu = menu.push(
            button(
                row![
                    status_dot(p.green),
                    text(branch.name.clone())
                        .font(SANS_SEMIBOLD)
                        .size(12.5)
                        .width(Length::Fill),
                    text(short_hash(Some(&branch.head)))
                        .font(MONO)
                        .size(10)
                        .color(p.muted_2)
                ]
                .spacing(9)
                .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .padding([8, 9])
            .style(move |_, status| button::Style {
                background: matches!(status, button::Status::Hovered)
                    .then_some(Background::Color(p.panel)),
                text_color: p.ink,
                border: Border {
                    radius: RADIUS_SM.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .on_press(Message::SelectBranch(branch.name.clone())),
        );
    }
    floating_menu(menu, p)
}

fn floating_menu<'a>(content: Column<'a, Message>, p: Palette) -> Element<'a, Message> {
    container(content)
        .width(260)
        .padding(0)
        .style(move |_| container::Style {
            background: Some(Background::Color(p.paper)),
            border: Border {
                color: p.border_strong,
                width: 1.0,
                radius: RADIUS_LG.into(),
            },
            shadow: Shadow {
                color: Color::from_rgba8(0, 0, 0, 0.14),
                offset: Vector::new(0.0, 6.0),
                blur_radius: 18.0,
            },
            ..Default::default()
        })
        .into()
}

fn tab_button(tab: Tab, active: Tab, badge: Option<usize>, p: Palette) -> Button<'static, Message> {
    let mut content = row![text(tab.label()).font(SANS_SEMIBOLD).size(13)].spacing(7);
    if let Some(badge) = badge {
        content = content.push(
            container(text(badge).font(MONO).size(10).color(p.muted_2))
                .padding([1, 7])
                .style(move |_| container::Style {
                    background: Some(Background::Color(p.panel)),
                    border: Border {
                        radius: 9.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
        );
    }
    button(content.align_y(Alignment::Center))
        .padding([10, 0])
        .style(move |_, _| button::Style {
            text_color: if tab == active { p.ink } else { p.muted_2 },
            border: Border {
                color: if tab == active {
                    p.filled
                } else {
                    Color::TRANSPARENT
                },
                width: 2.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .on_press(Message::SelectTab(tab))
}

fn filter_button(
    label: &'static str,
    count: usize,
    filter: ItemFilter,
    active: ItemFilter,
    p: Palette,
) -> Button<'static, Message> {
    button(
        text(format!("{label} {count}"))
            .font(SANS_SEMIBOLD)
            .size(11.5),
    )
    .padding([6, 10])
    .style(move |_, _| button::Style {
        background: (filter == active).then_some(Background::Color(p.filled)),
        text_color: if filter == active {
            p.on_filled
        } else {
            p.muted_2
        },
        border: Border {
            color: p.border,
            width: 1.0,
            radius: RADIUS_SM.into(),
        },
        ..Default::default()
    })
    .on_press(Message::SetItemFilter(filter))
}

fn pull_tab_button(
    label: &'static str,
    tab: PullTab,
    active: PullTab,
    p: Palette,
) -> Button<'static, Message> {
    button(text(label).font(SANS_SEMIBOLD).size(11))
        .padding([5, 10])
        .style(move |_, _| button::Style {
            background: (tab == active).then_some(Background::Color(p.filled)),
            text_color: if tab == active {
                p.on_filled
            } else {
                p.muted_2
            },
            border: Border {
                color: p.border,
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            ..Default::default()
        })
        .on_press(Message::SelectPullTab(tab))
}

fn icon_tile(icon: Icon, size: f32, p: Palette) -> Element<'static, Message> {
    container(icons::view(icon, size * 0.53, p.on_filled))
        .width(size)
        .height(size)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(move |_| container::Style {
            background: Some(Background::Color(p.filled)),
            border: Border {
                radius: RADIUS_SM.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

fn status_dot(color: Color) -> Element<'static, Message> {
    container(Space::new())
        .width(8)
        .height(8)
        .style(move |_| container::Style {
            background: Some(Background::Color(color)),
            border: Border {
                radius: 99.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

fn status_pill(label: &str, tone: Color, p: Palette) -> Element<'static, Message> {
    container(text(label.to_owned()).font(MONO).size(9).color(tone))
        .padding([3, 8])
        .style(move |_| tinted_box(tone, p))
        .into()
}

fn tinted_box(tone: Color, p: Palette) -> container::Style {
    container::Style {
        background: Some(Background::Color(mix(p.paper, tone, 0.09))),
        border: Border {
            color: mix(p.paper, tone, 0.25),
            width: 1.0,
            radius: RADIUS_SM.into(),
        },
        ..Default::default()
    }
}

fn section_label(label: &str, p: Palette) -> Element<'static, Message> {
    text(label.to_owned())
        .font(MONO)
        .size(9)
        .color(p.muted_2)
        .into()
}

fn center_note<'a>(title: &str, detail: Option<&str>, p: Palette) -> Element<'a, Message> {
    let mut content = column![
        text(title.to_owned())
            .font(SANS_SEMIBOLD)
            .size(12.5)
            .color(p.muted_2)
    ]
    .spacing(5)
    .align_x(Alignment::Center);
    if let Some(detail) = detail {
        content = content.push(
            text(detail.to_owned())
                .font(SANS)
                .size(11.5)
                .color(p.muted_2),
        );
    }
    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(24)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
}

fn error_banner<'a>(error: &str, p: Palette) -> Element<'a, Message> {
    container(text(error.to_owned()).font(SANS).size(11).color(p.red))
        .width(Length::Fill)
        .padding([7, 9])
        .style(move |_| tinted_box(p.red, p))
        .into()
}

fn card<'a>(content: impl Into<Element<'a, Message>>, p: Palette) -> Element<'a, Message> {
    container(content)
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(p.paper)),
            border: Border {
                color: p.border,
                width: 1.0,
                radius: RADIUS_LG.into(),
            },
            shadow: Shadow {
                color: Color::from_rgba8(0, 0, 0, 0.06),
                offset: Vector::new(0.0, 1.0),
                blur_radius: 3.0,
            },
            ..Default::default()
        })
        .into()
}

fn text_button(label: &'static str, message: Message, p: Palette) -> Button<'static, Message> {
    button(text(label).font(SANS_SEMIBOLD).size(15))
        .padding(0)
        .style(move |_, status| button::Style {
            text_color: if matches!(status, button::Status::Hovered) {
                p.ink
            } else {
                p.muted
            },
            ..Default::default()
        })
        .on_press(message)
}

fn secondary_button(
    label: &'static str,
    message: Option<Message>,
    p: Palette,
) -> Button<'static, Message> {
    button(text(label).font(SANS_SEMIBOLD).size(11))
        .padding([6, 10])
        .style(move |_, status| outlined_button(status, p))
        .on_press_maybe(message)
}

fn primary_button(
    label: &'static str,
    message: Option<Message>,
    p: Palette,
) -> Button<'static, Message> {
    let enabled = message.is_some();
    button(text(label).font(SANS_SEMIBOLD).size(11))
        .padding([6, 10])
        .style(move |_, _| button::Style {
            background: Some(Background::Color(if enabled { p.filled } else { p.chip })),
            text_color: if enabled { p.on_filled } else { p.muted_2 },
            border: Border {
                radius: RADIUS_SM.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .on_press_maybe(message)
}

fn outlined_button(status: button::Status, p: Palette) -> button::Style {
    button::Style {
        background: Some(Background::Color(
            if matches!(status, button::Status::Hovered) {
                p.sunken
            } else {
                p.paper
            },
        )),
        text_color: p.ink,
        border: Border {
            color: p.border,
            width: 1.0,
            radius: RADIUS_SM.into(),
        },
        ..Default::default()
    }
}

fn horizontal_divider(p: Palette) -> Element<'static, Message> {
    container(Space::new())
        .height(1)
        .style(move |_| container::Style {
            background: Some(Background::Color(p.border_soft)),
            ..Default::default()
        })
        .into()
}

fn short_hash(value: Option<&str>) -> String {
    value.map_or_else(
        || "unborn".into(),
        |value| {
            if value.chars().count() <= 10 {
                value.into()
            } else {
                format!("{}...", value.chars().take(10).collect::<String>())
            }
        },
    )
}

fn mix(base: Color, tint: Color, amount: f32) -> Color {
    Color {
        r: base.r + (tint.r - base.r) * amount,
        g: base.g + (tint.g - base.g) * amount,
        b: base.b + (tint.b - base.b) * amount,
        a: 1.0,
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
        let mut state = State::default();
        state.repositories = Resource::Ready(vec![repo()]);
        state.selected_file = Some("old.rs".into());
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
        let _ = view(&state, theme::Mode::Light);
        let state = State {
            repositories: Resource::Ready(vec![repo()]),
            ..State::default()
        };
        let _ = view(&state, theme::Mode::Dark);
    }

    #[test]
    fn issue_submit_trims_title_and_keeps_markdown_body() {
        let mut state = State::default();
        state.selected_repo = Some("core".into());
        state.tab = Tab::Issues;
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
        let mut state = State::default();
        state.repository = Resource::Ready(RepositoryData {
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
        });
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
        let mut state = State::default();
        state.file = Resource::Ready(FilePage {
            path: "README.md".into(),
            text: "first".into(),
            loaded_bytes: 5,
            total_bytes: 11,
            has_more: true,
        });
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
}
