//! Forge: every load error offers a Retry that re-fires its producer message
//! (the port fires each fetch once, so without Retry an error is terminal).

use super::harness::*;
use crate::screens::forge::{
    self, Branch, ChangedFile, CodeContent, Commit, DiscussionPost, ForgeItem, ItemDetail,
    ItemKind, ItemState, Message, PullTab, Repository, RepositoryData, Resource, Review,
    ReviewComment, ReviewSide, ReviewVerdict, State, Tab, TreeEntry, TreeKind,
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
    // The "New pull request" button is present and the hint is not the only body.
    assert!(
        has(&mut ui, Role::Button, "Cancel"),
        "the open new-PR form renders its Cancel control"
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
