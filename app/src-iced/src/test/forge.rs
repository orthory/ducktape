//! Forge: every load error offers a Retry that re-fires its producer message
//! (the port fires each fetch once, so without Retry an error is terminal).

use super::harness::*;
use crate::screens::forge::{
    self, Branch, ChangedFile, CodeContent, Commit, DiscussionPost, FilePage, ForgeItem,
    ItemDetail, ItemFilter, ItemKind, ItemState, Message, PullTab, Repository, RepositoryData,
    Resource, Review, ReviewComment, ReviewSide, ReviewVerdict, State, Tab, TreeEntry, TreeKind,
};
use crate::theme;
use iced::widget::text_editor;

fn repo() -> Repository {
    Repository {
        id: "core".into(),
        name: "core".into(),
        default_branch: "dev".into(),
        head: Some("0123456789abcdef".into()),
        browsable: true,
    }
}

#[test]
fn overview_error_offers_retry_that_reloads() {
    let state = State {
        repositories: Resource::Error("forge is unreachable".into()),
        ..State::default()
    };
    let p = theme::Mode::Light;
    let mut ui = sim(forge::view(&state, p));
    assert!(
        has(&mut ui, Role::Button, "Retry"),
        "an overview load error must offer a Retry"
    );
    ui.click(by::role(Role::Button, "Retry"))
        .expect("Retry is clickable");
    assert!(
        emitted(ui, &Message::Load),
        "Retry on the overview must re-fire the repositories load"
    );
}

#[test]
fn tree_error_offers_retry_that_reselects_the_repository() {
    let state = State {
        repositories: Resource::Ready(vec![repo()]),
        selected_repo: Some("core".into()),
        repository: Resource::Error("tree read failed".into()),
        tab: Tab::Code,
        ..State::default()
    };
    let mut ui = sim(forge::view(&state, theme::Mode::Dark));
    assert!(
        has(&mut ui, Role::Button, "Retry"),
        "a repository/tree load error must offer a Retry"
    );
    ui.click(by::role(Role::Button, "Retry"))
        .expect("Retry is clickable");
    assert!(
        emitted(ui, &Message::SelectRepository("core".into())),
        "Retry must re-select the current repository to reload its tree"
    );
}

#[test]
fn tree_error_retry_reselects_the_active_branch() {
    let state = State {
        repositories: Resource::Ready(vec![repo()]),
        selected_repo: Some("core".into()),
        selected_branch: Some("feature".into()),
        repository: Resource::Error("tree read failed".into()),
        tab: Tab::Code,
        ..State::default()
    };
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::Button, "Retry"))
        .expect("Retry is clickable");
    assert!(
        emitted(ui, &Message::SelectBranch("feature".into())),
        "with a branch checked out, Retry must reload that branch"
    );
}

#[test]
fn pull_request_form_lists_actual_branches() {
    // With two branches, the new-PR form must render branch pickers, not the
    // "push a branch" hint (M3).
    let mut state = State {
        repositories: Resource::Ready(vec![repo()]),
        selected_repo: Some("core".into()),
        repository: Resource::Ready(RepositoryData {
            branches: vec![
                Branch {
                    name: "dev".into(),
                    head: "a".repeat(40),
                },
                Branch {
                    name: "feature".into(),
                    head: "b".repeat(40),
                },
            ],
            tree: vec![],
            commits: vec![],
            commits_have_more: false,
            items: vec![],
            remote: false,
        }),
        tab: Tab::Pulls,
        ..State::default()
    };
    state.new_item_open = true;
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    assert!(
        has(&mut ui, Role::Button, "Cancel"),
        "the open new-PR form renders its Cancel control"
    );
    // The load-bearing M3 behaviour: with two branches the form shows the branch
    // pickers, NOT the "push a branch" hint. Assert the hint is absent (asserting
    // Cancel alone passed vacuously — every form has a Cancel).
    assert!(
        ui.find("No source branches yet — push a branch besides dev to open a pull request.")
            .is_err(),
        "with two branches the PR form must not show the push-a-branch hint"
    );
}

/// A data-rich state exercising every new surface (selectable code pane, tinted
/// diff with a new-side gutter, expandable commit diff, collapsible changed-file
/// review cards, merge box) so a broken widget tree fails the build in CI rather
/// than only in the live app that needs a git-backed node.
fn rich_pull_detail() -> ItemDetail {
    ItemDetail {
        item: ForgeItem {
            number: 7,
            kind: ItemKind::PullRequest,
            state: ItemState::Open,
            title: "Wire the selectable code pane".into(),
            author: "you".into(),
            updated: "2m ago".into(),
            source_branch: Some("feature".into()),
            target_branch: Some("dev".into()),
        },
        body: "## Steps\n- render\n- select".into(),
        can_edit: true,
        comments: vec![DiscussionPost {
            author: "reviewer".into(),
            body: "Looks close.".into(),
            time: "1m ago".into(),
        }],
        commits: vec![Commit {
            id: "c".repeat(40),
            summary: "Add pane".into(),
            author: "you".into(),
            time: "1m ago".into(),
        }],
        changed_files: vec![ChangedFile {
            path: "src/lib.rs".into(),
            additions: 2,
            deletions: 1,
            patch: "@@ -1,3 +1,4 @@\n context\n-old line\n+new line\n+another\n".into(),
        }],
        compare_error: None,
        reviews: vec![Review {
            author: "reviewer".into(),
            verdict: ReviewVerdict::RequestChanges,
            body: "One concern.".into(),
            commit_oid: "d".repeat(40),
            comments: vec![ReviewComment {
                path: "src/lib.rs".into(),
                line: 2,
                side: ReviewSide::New,
                body: "Guard this.".into(),
            }],
            created_at: "1m ago".into(),
        }],
    }
}

fn rich_state() -> State {
    let mut state = State {
        repositories: Resource::Ready(vec![repo()]),
        selected_repo: Some("core".into()),
        repository: Resource::Ready(RepositoryData {
            branches: vec![
                Branch {
                    name: "dev".into(),
                    head: "a".repeat(40),
                },
                Branch {
                    name: "feature".into(),
                    head: "b".repeat(40),
                },
            ],
            tree: vec![
                TreeEntry {
                    path: "src".into(),
                    name: "src".into(),
                    kind: TreeKind::Directory,
                    depth: 0,
                    open: true,
                },
                TreeEntry {
                    path: "src/lib.rs".into(),
                    name: "lib.rs".into(),
                    kind: TreeKind::File,
                    depth: 1,
                    open: false,
                },
            ],
            commits: vec![Commit {
                id: "c".repeat(40),
                summary: "Add pane".into(),
                author: "you".into(),
                time: "1m ago".into(),
            }],
            commits_have_more: true,
            items: vec![ForgeItem {
                number: 7,
                kind: ItemKind::PullRequest,
                state: ItemState::Open,
                title: "Wire the selectable code pane".into(),
                author: "you".into(),
                updated: "2m ago".into(),
                source_branch: Some("feature".into()),
                target_branch: Some("dev".into()),
            }],
            remote: true,
        }),
        selected_file: Some("src/lib.rs".into()),
        file: Resource::Ready(forge::FilePage {
            path: "src/lib.rs".into(),
            text: "fn main() {}\nlet x = 1;\n".into(),
            loaded_bytes: 24,
            total_bytes: 48,
            has_more: true,
        }),
        file_content: CodeContent(text_editor::Content::with_text("fn main() {}\nlet x = 1;\n")),
        selected_commit: Some("c".repeat(40)),
        commit_diff: Resource::Ready(vec![ChangedFile {
            path: "src/lib.rs".into(),
            additions: 2,
            deletions: 1,
            patch: "@@ -1,3 +1,4 @@\n ctx\n-old\n+new\n+more\n".into(),
        }]),
        ..State::default()
    };
    state.item_detail = Resource::Ready(rich_pull_detail());
    state.review_open = true;
    state.review_comment_file = Some("src/lib.rs".into());
    state.review_comment_line = "4".into();
    state.review_comments = vec![ReviewComment {
        path: "src/lib.rs".into(),
        line: 4,
        side: ReviewSide::New,
        body: "Queued note.".into(),
    }];
    state
}

#[test]
fn every_forge_surface_builds_in_both_themes() {
    for mode in [theme::Mode::Light, theme::Mode::Dark] {
        for tab in [Tab::Code, Tab::Commits, Tab::Issues, Tab::Pulls] {
            let state = State {
                tab,
                ..rich_state()
            };
            let _ = forge::view(&state, mode);
        }
        // Item detail across every pull sub-tab.
        for pull_tab in [PullTab::Conversation, PullTab::Commits, PullTab::Files] {
            let mut state = rich_state();
            state.selected_item = Some(7);
            state.pull_tab = pull_tab;
            let _ = forge::view(&state, mode);
        }
    }
}

// ---------------------------------------------------------------------------
// View-layer coverage: render variants (loading/empty/error/ready) and the
// interaction→Message wiring the module's own reducer tests can't see. The
// reducer transitions live in `screens/forge.rs::tests`; these drive the real
// widget tree through the simulator instead.
// ---------------------------------------------------------------------------

fn repo_data(items: Vec<ForgeItem>) -> RepositoryData {
    RepositoryData {
        branches: vec![
            Branch {
                name: "dev".into(),
                head: "a".repeat(40),
            },
            Branch {
                name: "feature".into(),
                head: "b".repeat(40),
            },
        ],
        tree: vec![],
        commits: vec![],
        commits_have_more: false,
        items,
        remote: false,
    }
}

/// A repo selected and loaded, sitting on `tab`. The default browser landing.
fn browsing(tab: Tab, repository: Resource<RepositoryData>) -> State {
    State {
        repositories: Resource::Ready(vec![repo()]),
        selected_repo: Some("core".into()),
        repository,
        tab,
        ..State::default()
    }
}

fn item(number: u64, kind: ItemKind, state: ItemState, title: &str) -> ForgeItem {
    ForgeItem {
        number,
        kind,
        state,
        title: title.into(),
        author: "you".into(),
        updated: "now".into(),
        source_branch: (kind == ItemKind::PullRequest).then(|| "feature".into()),
        target_branch: (kind == ItemKind::PullRequest).then(|| "dev".into()),
    }
}

/// A loaded, open pull request in its detail view.
fn open_pr() -> State {
    let mut state = browsing(Tab::Pulls, Resource::Ready(repo_data(vec![])));
    state.selected_item = Some(7);
    state.item_detail = Resource::Ready(rich_pull_detail());
    state
}

// --- Overview (repository list) --------------------------------------------

#[test]
fn overview_loading_shows_a_note() {
    let state = State::default();
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    assert!(ui.find("Loading repositories...").is_ok());
}

#[test]
fn overview_empty_shows_the_no_repos_note() {
    let state = State {
        repositories: Resource::Empty,
        ..State::default()
    };
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    assert!(ui.find("No local forge repositories yet").is_ok());
}

#[test]
fn overview_repo_card_selects_the_repository() {
    let state = State {
        repositories: Resource::Ready(vec![repo()]),
        ..State::default()
    };
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::ListItem, "core"))
        .expect("a repo card is clickable");
    assert!(emitted(ui, &Message::SelectRepository("core".into())));
}

#[test]
fn selecting_a_missing_repo_shows_not_found() {
    let state = State {
        repositories: Resource::Ready(vec![repo()]),
        selected_repo: Some("ghost".into()),
        ..State::default()
    };
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    assert!(ui.find("Repository not found").is_ok());
}

// --- Code tab / file viewer -------------------------------------------------

fn code_state(file: Resource<FilePage>) -> State {
    let mut data = repo_data(vec![]);
    data.tree = vec![TreeEntry {
        path: "README.md".into(),
        name: "README.md".into(),
        kind: TreeKind::File,
        depth: 0,
        open: false,
    }];
    let mut state = browsing(Tab::Code, Resource::Ready(data));
    state.selected_file = Some("src/lib.rs".into());
    state.file = file;
    state
}

#[test]
fn code_tab_loading_shows_a_note() {
    let state = browsing(Tab::Code, Resource::Loading);
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    assert!(ui.find("Loading repository...").is_ok());
}

#[test]
fn non_browsable_repo_has_no_tree() {
    let empty = Repository {
        id: "empty".into(),
        name: "empty".into(),
        default_branch: "main".into(),
        head: None,
        browsable: false,
    };
    let state = State {
        repositories: Resource::Ready(vec![empty]),
        selected_repo: Some("empty".into()),
        tab: Tab::Code,
        ..State::default()
    };
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    assert!(ui.find("No committed tree").is_ok());
}

#[test]
fn code_tree_lists_files() {
    // `file` is Empty so the file pane can't be the source of the name — the
    // README row can only come from the Ready tree.
    let state = code_state(Resource::Empty);
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    assert!(ui.find("README.md").is_ok(), "the Ready tree renders file rows");
}

#[test]
fn file_error_retry_reloads_that_file() {
    let state = code_state(Resource::Error("read failed".into()));
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::Button, "Retry"))
        .expect("Retry is clickable");
    assert!(
        emitted(ui, &Message::SelectFile("src/lib.rs".into())),
        "a file load error must Retry by re-selecting that same file"
    );
}

#[test]
fn paged_file_offers_load_more() {
    let state = code_state(Resource::Ready(FilePage {
        path: "src/lib.rs".into(),
        text: "fn main() {}\n".into(),
        loaded_bytes: 13,
        total_bytes: 40,
        has_more: true,
    }));
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    assert!(
        has(&mut ui, Role::Button, "Load more file"),
        "a partially-loaded file must offer a Load more control"
    );
}

// --- Commits tab ------------------------------------------------------------

#[test]
fn commits_error_retry_reloads_the_repository() {
    let state = browsing(Tab::Commits, Resource::Error("log read failed".into()));
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::Button, "Retry"))
        .expect("Retry is clickable");
    assert!(emitted(ui, &Message::SelectRepository("core".into())));
}

#[test]
fn commits_load_more_pages_the_log() {
    let mut data = repo_data(vec![]);
    data.commits = vec![Commit {
        id: "c".repeat(40),
        summary: "seed".into(),
        author: "you".into(),
        time: "now".into(),
    }];
    data.commits_have_more = true;
    let state = browsing(Tab::Commits, Resource::Ready(data));
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::Button, "Load more commits"))
        .expect("Load more is clickable");
    assert!(emitted(ui, &Message::LoadMoreCommits));
}

// --- Chrome: tabs + repo/branch menus --------------------------------------

#[test]
fn selecting_a_tab_switches_it() {
    let state = browsing(Tab::Code, Resource::Ready(repo_data(vec![])));
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::Tab, "Commits"))
        .expect("a tab is clickable");
    assert!(emitted(ui, &Message::SelectTab(Tab::Commits)));
}

#[test]
fn repo_breadcrumb_toggles_the_menu() {
    let state = browsing(Tab::Code, Resource::Ready(repo_data(vec![])));
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::Button, "core"))
        .expect("the repo breadcrumb is clickable");
    assert!(emitted(ui, &Message::ToggleRepositoryMenu));
}

#[test]
fn repo_menu_selects_a_repository() {
    let mut state = browsing(Tab::Code, Resource::Ready(repo_data(vec![])));
    state.repo_menu_open = true;
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::ListItem, "core"))
        .expect("a repo menu item is clickable");
    assert!(emitted(ui, &Message::SelectRepository("core".into())));
}

#[test]
fn branch_menu_checks_out_a_branch() {
    let mut state = browsing(Tab::Code, Resource::Ready(repo_data(vec![])));
    state.branch_menu_open = true;
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::ListItem, "feature"))
        .expect("a branch menu item is clickable");
    assert!(emitted(ui, &Message::SelectBranch("feature".into())));
}

// --- Issues / Pull-request lists -------------------------------------------

#[test]
fn items_error_retry_reloads_the_repository() {
    let state = browsing(Tab::Issues, Resource::Error("items read failed".into()));
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::Button, "Retry"))
        .expect("Retry is clickable");
    assert!(emitted(ui, &Message::SelectRepository("core".into())));
}

#[test]
fn empty_issues_shows_the_first_issue_prompt() {
    let state = browsing(Tab::Issues, Resource::Ready(repo_data(vec![])));
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    assert!(ui.find("No issues yet").is_ok());
}

#[test]
fn new_issue_button_opens_the_form() {
    let state = browsing(Tab::Issues, Resource::Ready(repo_data(vec![])));
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::Button, "New issue"))
        .expect("New issue is clickable");
    assert!(emitted(ui, &Message::ToggleNewItem));
}

#[test]
fn issue_row_opens_the_item() {
    let state = browsing(
        Tab::Issues,
        Resource::Ready(repo_data(vec![item(
            3,
            ItemKind::Issue,
            ItemState::Open,
            "Broken build",
        )])),
    );
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::ListItem, "Broken build"))
        .expect("an issue row is clickable");
    assert!(emitted(ui, &Message::OpenItem(3)));
}

#[test]
fn item_filter_switches_to_closed() {
    let items = vec![
        item(3, ItemKind::Issue, ItemState::Open, "open one"),
        item(4, ItemKind::Issue, ItemState::Closed, "closed one"),
    ];
    let state = browsing(Tab::Issues, Resource::Ready(repo_data(items)));
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::Button, "Closed"))
        .expect("the Closed filter is clickable");
    assert!(emitted(ui, &Message::SetItemFilter(ItemFilter::Closed)));
}

#[test]
fn single_branch_pr_form_shows_the_push_hint() {
    let mut data = repo_data(vec![]);
    data.branches = vec![Branch {
        name: "dev".into(),
        head: "a".repeat(40),
    }];
    let mut state = browsing(Tab::Pulls, Resource::Ready(data));
    state.new_item_open = true;
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    assert!(
        ui.find("No source branches yet — push a branch besides dev to open a pull request.")
            .is_ok(),
        "with only the default branch, the PR form shows the push-a-branch hint"
    );
}

// --- Item detail ------------------------------------------------------------

#[test]
fn item_detail_loading_shows_a_note() {
    let mut state = browsing(Tab::Pulls, Resource::Ready(repo_data(vec![])));
    state.selected_item = Some(7);
    state.item_detail = Resource::Loading;
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    assert!(ui.find("Loading item...").is_ok());
}

#[test]
fn item_detail_error_retry_reloads_the_item() {
    let mut state = browsing(Tab::Pulls, Resource::Ready(repo_data(vec![])));
    state.selected_item = Some(7);
    state.item_detail = Resource::Error("item read failed".into());
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::Button, "Retry"))
        .expect("Retry is clickable");
    assert!(
        emitted(ui, &Message::OpenItem(7)),
        "an item load error must Retry by re-opening that item"
    );
}

#[test]
fn missing_item_shows_not_found() {
    let mut state = browsing(Tab::Pulls, Resource::Ready(repo_data(vec![])));
    state.selected_item = Some(7);
    state.item_detail = Resource::Empty;
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    assert!(ui.find("Item not found").is_ok());
}

#[test]
fn open_pr_back_button_closes_the_item() {
    let state = open_pr();
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::Button, "‹ Pull requests"))
        .expect("the back button is clickable");
    assert!(emitted(ui, &Message::CloseItem));
}

#[test]
fn open_pr_edit_starts_editing() {
    let state = open_pr();
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::Button, "Edit"))
        .expect("Edit is clickable");
    assert!(emitted(ui, &Message::StartEditingItem));
}

#[test]
fn open_pr_close_toggles_state() {
    let state = open_pr();
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::Button, "Close"))
        .expect("Close is clickable");
    assert!(emitted(ui, &Message::ToggleItemState));
}

#[test]
fn open_pr_merge_box_merges() {
    let state = open_pr();
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::Button, "Merge pull request"))
        .expect("the merge action is clickable");
    assert!(emitted(ui, &Message::MergePullRequest));
}

#[test]
fn pr_pull_tabs_switch_to_files() {
    let state = open_pr();
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::Tab, "Files changed"))
        .expect("a pull sub-tab is clickable");
    assert!(emitted(ui, &Message::SelectPullTab(PullTab::Files)));
}

#[test]
fn files_tab_offers_review_changes() {
    let mut state = open_pr();
    state.pull_tab = PullTab::Files;
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    ui.click(by::role(Role::Button, "Review changes"))
        .expect("Review changes is clickable");
    assert!(emitted(ui, &Message::ToggleReview));
}

#[test]
fn open_review_shows_verdict_buttons() {
    let mut state = open_pr();
    state.pull_tab = PullTab::Files;
    state.review_open = true;
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    assert!(has(&mut ui, Role::Button, "Approve"));
    assert!(has(&mut ui, Role::Button, "Request changes"));
    assert!(
        has(&mut ui, Role::Button, "Comment"),
        "an open review renders all three verdict buttons"
    );
}

#[test]
fn merged_pr_shows_the_merged_banner() {
    let mut detail = rich_pull_detail();
    detail.item.state = ItemState::Merged;
    let mut state = browsing(Tab::Pulls, Resource::Ready(repo_data(vec![])));
    state.selected_item = Some(7);
    state.item_detail = Resource::Ready(detail);
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    assert!(ui.find("This pull request was merged.").is_ok());
}

// --- Error surfacing (the class of gap that hid the pages bug) --------------

#[test]
fn item_detail_surfaces_a_write_error() {
    let mut state = open_pr();
    state.error = Some("op rejected: Module(\"merge conflict\")".into());
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    assert!(
        ui.find("op rejected: Module(\"merge conflict\")").is_ok(),
        "a failed write must surface its error in the item view"
    );
}

#[test]
fn items_view_surfaces_a_write_error() {
    let mut state = browsing(Tab::Issues, Resource::Ready(repo_data(vec![])));
    state.error = Some("op rejected: Module(\"forge closed\")".into());
    let mut ui = sim(forge::view(&state, theme::Mode::Light));
    assert!(
        ui.find("op rejected: Module(\"forge closed\")").is_ok(),
        "a failed issue/PR write must surface its error in the list view"
    );
}
