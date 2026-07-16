use iced::widget::text::{LineHeight, Wrapping};
use iced::widget::{
    Column, Space, button, column, container, pick_list, row, scrollable, stack, text, text_editor,
    text_input,
};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Padding, Shadow, Vector};

use crate::icons::{self, Icon};
use crate::theme::{
    self, BODY, BODY_LG, CAPTION, HEADING, LABEL, MONO, Palette, RADIUS_LG, RADIUS_MD, RADIUS_SM,
    SANS, SANS_SEMIBOLD, TITLE,
};

use super::*;

const TREE_WIDTH: f32 = 258.0;
const BODY_PAD: f32 = 24.0;
/// Shared absolute line box for the file gutter and the code editor so their
/// rows stay aligned regardless of the gutter's smaller font size.
const CODE_LINE: f32 = 20.0;

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
        text("ducktape").font(SANS_SEMIBOLD).size(HEADING).color(p.ink),
        status_pill("ORG", p.purple, p),
        Space::new().width(Length::Fill),
        text(match &state.repositories {
            Resource::Ready(_) => format!("{count} repositories"),
            _ => "Local forge repositories".into(),
        })
        .font(MONO)
        .size(CAPTION)
        .color(p.muted_2),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    let mut body = column![
        header,
        text("Browse repositories backed by this node's local git forge.")
            .font(SANS)
            .size(BODY)
            .color(p.muted)
    ]
    .spacing(7)
    .padding([22, 26]);
    body = body.push(Space::new().height(11));
    match &state.repositories {
        Resource::Loading => body = body.push(center_note("Loading repositories...", None, p)),
        Resource::Empty => {
            body = body.push(center_note(
                "No local forge repositories yet",
                Some("This node did not report a browsable git repository."),
                p,
            ));
        }
        Resource::Error(error) => body = body.push(retry_banner(error, Message::Load, p)),
        Resource::Ready(repos) => {
            body = body.push(section_label("REPOSITORIES", p));
            // Responsive 2-up grid: chunk cards into wrapping rows so wide
            // windows fill horizontally instead of stacking one per line.
            for pair in repos.chunks(2) {
                let mut grid = row![].spacing(12);
                for repo in pair {
                    grid = grid.push(repo_card(repo, p));
                }
                if pair.len() == 1 {
                    grid = grid.push(Space::new().width(Length::Fill));
                }
                body = body.push(grid);
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
            .size(CAPTION)
            .color(p.muted),
        text(if repo.browsable {
            "browsable"
        } else {
            "no HEAD"
        })
        .font(MONO)
        .size(CAPTION)
        .color(p.muted),
        Space::new().width(Length::Fill),
        text(short_hash(repo.head.as_deref()))
            .font(MONO)
            .size(CAPTION)
            .color(p.muted_2),
    ]
    .spacing(14)
    .align_y(Alignment::Center);
    let card = button(
        column![
            row![
                icons::view(Icon::Forge, 14.0, p.muted),
                text(format!("ducktape/{}", repo.name))
                    .font(SANS_SEMIBOLD)
                    .size(TITLE)
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
        shadow: card_shadow(p),
        ..Default::default()
    })
    .on_press(Message::SelectRepository(id));
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::ListItem, repo.name.clone(), card);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    card.into()
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
    let remote = data.is_some_and(|data| data.remote);

    // B1: give the breadcrumb repo button real ink (its label used to inherit
    // `Color::BLACK` and vanish on the dark paper); hover flashes the accent.
    // The `⌄`/`›` glyph doubles as the P8 open-state chevron.
    let repo_open = state.repo_menu_open;
    let repo_button = button(
        row![
            text(repo.name.clone()).font(SANS_SEMIBOLD).size(TITLE),
            text(if repo_open { "⌄" } else { "›" })
                .font(MONO)
                .size(CAPTION),
        ]
        .spacing(5)
        .align_y(Alignment::Center),
    )
    .padding([2, 5])
    .style(move |_, status| button::Style {
        text_color: if matches!(status, button::Status::Hovered) {
            p.filled
        } else {
            p.ink
        },
        border: Border {
            radius: RADIUS_SM.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .on_press(Message::ToggleRepositoryMenu);
    #[cfg(all(feature = "agent", debug_assertions))]
    let repo_button =
        iced_agent_plugin::sem(iced_agent_plugin::Role::Button, repo.name.clone(), repo_button);

    // P3: the branch selector only belongs on the browsing tabs of a browsable
    // repo; elsewhere show a static default-branch pill.
    let browsing = matches!(state.tab, Tab::Code | Tab::Commits) && repo.browsable;
    let branch_open = state.branch_menu_open;
    let branch_control: Element<'a, Message> = if browsing {
        let branch_button = button(
            row![
                status_dot(p.green),
                text(branch.to_owned()).font(MONO).size(CAPTION),
                text(if branch_open { "⌄" } else { "›" })
                    .font(MONO)
                    .size(CAPTION)
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .padding([3, 8])
        .style(move |_, status| outlined_button(status, p))
        .on_press(Message::ToggleBranchMenu);
        #[cfg(all(feature = "agent", debug_assertions))]
        {
            iced_agent_plugin::sem(iced_agent_plugin::Role::Button, branch.to_owned(), branch_button)
        }
        #[cfg(not(all(feature = "agent", debug_assertions)))]
        {
            branch_button.into()
        }
    } else {
        status_pill(branch, p.muted_3, p)
    };

    let heading = row![
        icon_tile(Icon::Forge, 28.0, p),
        text_button("ducktape", Message::BackToRepositories, p),
        text("/").font(SANS).size(TITLE).color(p.icon_idle),
        repo_button,
        branch_control,
        status_pill(&short_hash(head), p.muted_3, p),
        Space::new().width(Length::Fill),
        status_pill(if remote { "REMOTE" } else { "DESKTOP" }, p.purple, p),
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
    let page = column![chrome, body]
        .width(Length::Fill)
        .height(Length::Fill);

    // P1: float the repo/branch menus over the page instead of pushing the body
    // down — iced 0.14 has no z-index, so `stack` is the overlay mechanism.
    if state.repo_menu_open {
        stack![page, overlay(repo_menu(state, p), 60.0, 110.0)].into()
    } else if state.branch_menu_open {
        stack![page, overlay(branch_menu(state, p), 60.0, 220.0)].into()
    } else {
        page.into()
    }
}

/// Position a floating menu roughly under its trigger in a stack's top layer.
fn overlay(menu: Element<'_, Message>, top: f32, left: f32) -> Element<'_, Message> {
    container(menu)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding {
            top,
            left,
            right: 0.0,
            bottom: 0.0,
        })
        .align_x(Alignment::Start)
        .align_y(Alignment::Start)
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
        Resource::Error(error) => retry_banner(error, reload_repository(state), p),
        Resource::Empty => center_note("Empty repository", None, p),
        Resource::Ready(data) => {
            let visible = visible_tree(&data.tree);
            let mut rows = column![
                row![
                    section_label("FILES", p),
                    Space::new().width(Length::Fill),
                    // P2: the header counts the visible rows, not every loaded entry.
                    text(visible.len()).font(MONO).size(CAPTION).color(p.muted_2)
                ]
                .padding(Padding {
                    top: 0.0,
                    right: 16.0,
                    bottom: 9.0,
                    left: 16.0,
                })
            ];
            for entry in visible {
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
            .size(BODY)
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
                .size(BODY)
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
            .size(BODY)
            .color(if state.selected_file.is_some() {
                p.ink_soft
            } else {
                p.muted_2
            }),
        Space::new().width(Length::Fill),
        text(latest).font(MONO).size(CAPTION).color(p.muted_2)
    ]
    .spacing(10)
    .padding([8, 16])
    .align_y(Alignment::Center);
    let content: Element<'a, Message> = match &state.file {
        Resource::Loading => center_note("Loading file...", None, p),
        Resource::Error(error) => container(retry_banner(
            error,
            state
                .selected_file
                .clone()
                .map_or(Message::CloseItem, Message::SelectFile),
            p,
        ))
        .padding([14, 16])
        .into(),
        Resource::Empty => center_note(
            state
                .selected_file
                .as_deref()
                .and_then(|path| path.rsplit('/').next())
                .unwrap_or("Select a file to read it here"),
            None,
            p,
        ),
        Resource::Ready(file) => {
            let mut code = column![code_pane(&state.file_content.0, p)];
            if file.has_more {
                code = code.push(
                    row![
                        Space::new().width(Length::Fill),
                        text(format!(
                            "{} / {} bytes",
                            file.loaded_bytes, file.total_bytes
                        ))
                        .font(MONO)
                        .size(CAPTION)
                        .color(p.muted_2),
                        secondary_button("Load more file", Some(Message::LoadMoreFile), p),
                        Space::new().width(Length::Fill)
                    ]
                    .spacing(10)
                    .padding([10, 16])
                    .align_y(Alignment::Center),
                );
            }
            code.height(Length::Fill).into()
        }
    };
    column![header, horizontal_divider(p), content]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// B2: a selectable, non-wrapping read-only code viewer with a synced line
/// gutter. The `text_editor` grows to content height (default `Shrink`) so a
/// single outer scrollable moves the gutter and the code together.
fn code_pane<'a>(content: &'a text_editor::Content, p: Palette) -> Element<'a, Message> {
    let count = content.line_count().max(1);
    let width = count.to_string().len();
    let numbers = (1..=count)
        .map(|line| format!("{line:>width$}"))
        .collect::<Vec<_>>()
        .join("\n");
    let gutter = container(
        text(numbers)
            .font(MONO)
            .size(CAPTION)
            .line_height(LineHeight::Absolute(CODE_LINE.into()))
            .color(p.icon_idle),
    )
    .padding(Padding {
        top: 12.0,
        right: 10.0,
        bottom: 12.0,
        left: 12.0,
    })
    .style(move |_| container::Style {
        background: Some(Background::Color(p.sidebar)),
        ..Default::default()
    });
    let editor = text_editor(content)
        .on_action(Message::FileAction)
        .font(MONO)
        .size(BODY)
        .line_height(LineHeight::Absolute(CODE_LINE.into()))
        .wrapping(Wrapping::None)
        .padding(Padding {
            top: 12.0,
            right: 14.0,
            bottom: 12.0,
            left: 14.0,
        })
        .style(move |_, _| text_editor::Style {
            background: Background::Color(p.paper),
            border: Border::default(),
            placeholder: p.muted_2,
            value: p.ink_soft,
            selection: theme::ACCENTS[0],
        });
    scrollable(row![gutter, editor])
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn commits_view<'a>(state: &'a State, repo: &'a Repository, p: Palette) -> Element<'a, Message> {
    let Some(data) = repository_data(state) else {
        return match &state.repository {
            Resource::Loading => center_note("Loading commits...", None, p),
            Resource::Error(error) => container(retry_banner(error, reload_repository(state), p))
                .padding([18.0, BODY_PAD])
                .into(),
            _ => center_note("No commits yet", None, p),
        };
    };
    let mut body = column![
        row![
            column![
                text("Commit history")
                    .font(SANS_SEMIBOLD)
                    .size(TITLE)
                    .color(p.ink),
                text("Read-only log from the local git repository.")
                    .font(SANS)
                    .size(LABEL)
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
            body = body.push(commit_row(commit, state, p));
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

/// M1: a commit row is a clickable toggle; the selected one grows a per-file
/// diff card below it.
fn commit_row<'a>(commit: &'a Commit, state: &'a State, p: Palette) -> Element<'a, Message> {
    let selected = state.selected_commit.as_deref() == Some(commit.id.as_str());
    let id = commit.id.clone();
    let header = button(
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
                    .size(BODY_LG)
                    .color(p.ink),
                text(format!(
                    "{} · {} · {}",
                    short_hash(Some(&commit.id)),
                    commit.author,
                    commit.time
                ))
                .font(MONO)
                .size(CAPTION)
                .color(p.muted_2)
            ]
            .spacing(4)
            .width(Length::Fill),
            text(if selected { "⌄" } else { "›" })
                .font(MONO)
                .size(BODY)
                .color(p.muted_2)
        ]
        .spacing(13)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([13, 6])
    .style(move |_, status| button::Style {
        background: (selected || matches!(status, button::Status::Hovered))
            .then_some(Background::Color(if selected { p.hover } else { p.sunken })),
        text_color: p.ink,
        border: Border {
            radius: RADIUS_SM.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .on_press(Message::ToggleCommit(id));
    if !selected {
        return header.into();
    }
    let diff: Element<'a, Message> = match &state.commit_diff {
        Resource::Loading => center_note("Loading diff...", None, p),
        Resource::Empty => center_note("No file changes in this commit", None, p),
        Resource::Error(error) => retry_banner(error, Message::ToggleCommit(commit.id.clone()), p),
        Resource::Ready(files) => {
            let mut list = column![].spacing(10);
            for file in files {
                list = list.push(diff_file_card(file, false, false, &[], p));
            }
            list.into()
        }
    };
    let detail = card(
        column![
            text(commit.summary.clone())
                .font(SANS)
                .size(BODY)
                .color(p.ink_soft),
            diff
        ]
        .spacing(10)
        .padding([12, 14]),
        p,
    );
    column![header, detail].spacing(6).into()
}

fn items_view<'a>(
    state: &'a State,
    repo: &'a Repository,
    kind: ItemKind,
    p: Palette,
) -> Element<'a, Message> {
    let Some(data) = repository_data(state) else {
        return match &state.repository {
            Resource::Error(error) => container(retry_banner(error, reload_repository(state), p))
                .padding([16, 24])
                .into(),
            _ => center_note("Loading items...", None, p),
        };
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
        .size(BODY_LG)
        .color(p.ink)
    ]
    .spacing(8);
    if kind == ItemKind::PullRequest {
        // M3: pick branches from what the repository actually has, filtering out
        // the other side so source ≠ target. When there is nothing to pick
        // (only the default branch exists), show the hint alone — no dead inputs.
        if data.branches.len() < 2 {
            form = form.push(
                text("No source branches yet — push a branch besides dev to open a pull request.")
                    .font(SANS)
                    .size(BODY)
                    .color(p.muted),
            );
        } else {
            let source = &state.new_item.source_branch;
            let target = &state.new_item.target_branch;
            let source_options: Vec<String> = data
                .branches
                .iter()
                .filter(|branch| &branch.name != target)
                .map(|branch| branch.name.clone())
                .collect();
            let target_options: Vec<String> = data
                .branches
                .iter()
                .filter(|branch| &branch.name != source)
                .map(|branch| branch.name.clone())
                .collect();
            let source_selected = (!source.is_empty()).then(|| source.clone());
            let target_selected = (!target.is_empty()).then(|| target.clone());
            form = form.push(
                row![
                    text("Merge").font(SANS_SEMIBOLD).size(LABEL).color(p.muted_3),
                    branch_picker(
                        source_options,
                        source_selected,
                        "Choose a source branch",
                        Message::SourceBranchChanged,
                    ),
                    text("into").font(SANS_SEMIBOLD).size(LABEL).color(p.muted_3),
                    branch_picker(
                        target_options,
                        target_selected,
                        "Choose a target branch",
                        Message::TargetBranchChanged,
                    ),
                ]
                .spacing(9)
                .align_y(Alignment::Center),
            );
        }
    }
    form = form
        .push(sem_input(
            "Title",
            &state.new_item.title,
            text_input("Title", &state.new_item.title)
                .font(SANS)
                .size(BODY)
                .on_input(Message::NewTitleChanged),
        ))
        .push(sem_input(
            "Description (markdown)",
            &state.new_item.body,
            text_input("Description (markdown)", &state.new_item.body)
                .font(SANS)
                .size(BODY)
                .on_input(Message::NewBodyChanged),
        ))
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

fn branch_picker(
    options: Vec<String>,
    selected: Option<String>,
    placeholder: &'static str,
    on_select: impl Fn(String) -> Message + 'static,
) -> Element<'static, Message> {
    pick_list(options, selected, on_select)
        .placeholder(placeholder)
        .font(MONO)
        .text_size(BODY)
        .width(Length::Fill)
        .into()
}

fn item_row(item: &ForgeItem, p: Palette) -> Element<'static, Message> {
    let number = item.number;
    let open = button(
        row![
            status_dot(match item.state {
                ItemState::Open => p.green,
                ItemState::Closed => p.red,
                ItemState::Merged => p.purple,
            }),
            column![
                text(item.title.clone())
                    .font(SANS_SEMIBOLD)
                    .size(BODY_LG)
                    .color(p.ink),
                text(format!(
                    "#{} opened by {} · {}",
                    item.number, item.author, item.updated
                ))
                .font(MONO)
                .size(CAPTION)
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
    .on_press(Message::OpenItem(number));
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::ListItem, item.title.clone(), open);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    open.into()
}

fn item_detail(state: &State, p: Palette) -> Element<'_, Message> {
    match &state.item_detail {
        Resource::Loading => center_note("Loading item...", None, p),
        Resource::Error(error) => column![
            text_button("‹ Back", Message::CloseItem, p),
            retry_banner(
                error,
                state
                    .selected_item
                    .map_or(Message::CloseItem, Message::OpenItem),
                p,
            )
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
                        sem_input(
                            "Title",
                            &state.edit_title,
                            text_input("Title", &state.edit_title)
                                .font(SANS)
                                .size(BODY)
                                .on_input(Message::EditTitleChanged),
                        ),
                        sem_input(
                            "Description (markdown)",
                            &state.edit_body,
                            text_input("Description (markdown)", &state.edit_body)
                                .font(SANS)
                                .size(BODY)
                                .on_input(Message::EditBodyChanged),
                        ),
                        row![
                            Space::new().width(Length::Fill),
                            secondary_button(
                                "Cancel",
                                (!state.busy).then_some(Message::CancelEditingItem),
                                p,
                            ),
                            primary_button(
                                if state.busy { "Saving..." } else { "Save changes" },
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
                                .size(HEADING)
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
                                .size(LABEL)
                                .color(p.muted_2)
                        ]
                        .spacing(9)
                        .align_y(Alignment::Center),
                    )
                    .push(item_body(detail, p));
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
                        body = body.push(merge_box(state, detail, p));
                    } else if item.kind == ItemKind::PullRequest {
                        body = body.push(merge_state_banner(item, p));
                    }
                    for comment in &detail.comments {
                        body = body.push(discussion_post(comment, p));
                    }
                    // M5: multi-line composer instead of a one-line input.
                    let draft = state.comment_content.0.text();
                    body = body.push(
                        column![
                            sem_comment(
                                &state.comment_content.0,
                                "Leave a comment",
                                Message::CommentAction,
                                p,
                            ),
                            row![
                                Space::new().width(Length::Fill),
                                primary_button(
                                    if state.busy { "Posting..." } else { "Comment" },
                                    (!state.busy && !draft.trim().is_empty())
                                        .then_some(Message::SubmitComment),
                                    p,
                                )
                            ]
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
                            body = body.push(commit_row(commit, state, p));
                        }
                    }
                }
                PullTab::Files => {
                    body = body.push(review_controls(state, detail, p));
                    // Submitted review summaries (verdict + note); their inline
                    // comments are anchored under the diff lines below.
                    for review in &detail.reviews {
                        body = body.push(review_card(review, p));
                    }
                    if let Some(error) = &detail.compare_error {
                        body = body.push(error_banner(error, p));
                    } else if detail.changed_files.is_empty() {
                        body = body.push(center_note("No changes", None, p));
                    } else {
                        for file in &detail.changed_files {
                            body = body.push(changed_file(file, state, detail, p));
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

/// M2: iced has no markdown renderer, so the body is shown raw. This is a
/// documented simplification, not silent parity — a `## Heading` reads
/// literally. When a renderer lands, swap this for a rendered view.
fn item_body<'a>(detail: &'a ItemDetail, p: Palette) -> Element<'a, Message> {
    card(
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
            .size(BODY)
            .color(p.ink_soft),
        )
        .padding([12, 15]),
        p,
    )
}

/// P7: the merge box shows source(head)→target(head) short hashes alongside the
/// approval tally and the merge action.
fn merge_box<'a>(state: &'a State, detail: &'a ItemDetail, p: Palette) -> Element<'a, Message> {
    let item = &detail.item;
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
    let source = item.source_branch.as_deref().unwrap_or("source");
    let target = item.target_branch.as_deref().unwrap_or("dev");
    let source_head = branch_head(state, source);
    let target_head = branch_head(state, target);
    card(
        row![
            column![
                section_label("MERGE", p),
                text(format!("{source} into {target}"))
                    .font(MONO)
                    .size(BODY)
                    .color(p.ink),
                text(format!(
                    "{} → {}",
                    short_hash(source_head.as_deref()),
                    short_hash(target_head.as_deref())
                ))
                .font(MONO)
                .size(CAPTION)
                .color(p.muted_2),
                text(format!(
                    "{approvals} approvals · {change_requests} change requests"
                ))
                .font(SANS)
                .size(CAPTION)
                .color(p.muted_2)
            ]
            .spacing(5),
            Space::new().width(Length::Fill),
            primary_button(
                if state.busy {
                    "Merging..."
                } else {
                    "Merge pull request"
                },
                (!state.busy).then_some(Message::MergePullRequest),
                p,
            )
        ]
        .padding([12, 14])
        .align_y(Alignment::Center),
        p,
    )
}

fn merge_state_banner(item: &ForgeItem, p: Palette) -> Element<'static, Message> {
    let (label, tone) = match item.state {
        ItemState::Merged => ("This pull request was merged.", p.purple),
        _ => ("This pull request is closed.", p.red),
    };
    container(text(label).font(SANS).size(BODY).color(p.ink_soft))
        .width(Length::Fill)
        .padding([9, 12])
        .style(move |_| tinted_box(tone, p))
        .into()
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
                .size(CAPTION)
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
            sem_input(
                "Leave a review summary",
                &state.review_body,
                text_input("Leave a review summary", &state.review_body)
                    .font(SANS)
                    .size(BODY)
                    .on_input(Message::ReviewBodyChanged),
            ),
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
            .size(CAPTION)
            .color(p.muted_2),
            row![
                text(if has_source_head {
                    "Review pins to the current source head."
                } else {
                    "Current source head is unavailable; refresh the repository before reviewing."
                })
                .font(SANS)
                .size(CAPTION)
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
) -> Element<'static, Message> {
    let btn = button(text(verdict.label()).font(SANS_SEMIBOLD).size(LABEL))
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
        .on_press(Message::ReviewVerdictChanged(verdict));
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Button, verdict.label(), btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn review_card(review: &Review, p: Palette) -> Element<'static, Message> {
    let tone = match review.verdict {
        ReviewVerdict::Approve => p.green,
        ReviewVerdict::RequestChanges => p.red,
        ReviewVerdict::Comment => p.blue,
    };
    let count = review.comments.len();
    let mut content = column![
        row![
            text(review.author.clone())
                .font(SANS_SEMIBOLD)
                .size(LABEL)
                .color(p.ink),
            status_pill(review.verdict.label(), tone, p),
            Space::new().width(Length::Fill),
            text(review.created_at.clone())
                .font(MONO)
                .size(CAPTION)
                .color(p.muted_2),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        text(review.body.clone())
            .font(SANS)
            .size(BODY)
            .color(p.ink_soft),
    ]
    .spacing(6)
    .padding([10, 12]);
    let footer = if count == 0 {
        format!("reviewed {}", short_hash(Some(&review.commit_oid)))
    } else {
        format!(
            "reviewed {} · {count} inline comment{}",
            short_hash(Some(&review.commit_oid)),
            if count == 1 { "" } else { "s" }
        )
    };
    content = content.push(text(footer).font(MONO).size(CAPTION).color(p.muted_2));
    card(content, p)
}

fn discussion_post(post: &DiscussionPost, p: Palette) -> Element<'static, Message> {
    card(
        column![
            row![
                text(post.author.clone())
                    .font(SANS_SEMIBOLD)
                    .size(LABEL)
                    .color(p.ink),
                Space::new().width(Length::Fill),
                text(post.time.clone())
                    .font(MONO)
                    .size(CAPTION)
                    .color(p.muted_2)
            ],
            text(post.body.clone())
                .font(SANS)
                .size(BODY)
                .color(p.ink_soft)
        ]
        .spacing(7)
        .padding([10, 12]),
        p,
    )
}

/// M4: a collapsible per-file review card — +/− counts in the header, a tinted
/// diff with a new-side gutter and click-to-stage, existing comments anchored
/// under their lines, and the manual line-number staging as a fallback.
fn changed_file<'a>(
    file: &'a ChangedFile,
    state: &'a State,
    detail: &'a ItemDetail,
    p: Palette,
) -> Element<'a, Message> {
    let collapsed = state.collapsed_files.iter().any(|path| path == &file.path);
    let existing: Vec<&ReviewComment> = detail
        .reviews
        .iter()
        .flat_map(|review| review.comments.iter())
        .filter(|comment| comment.path == file.path)
        .collect();
    let header = button(
        row![
            text(if collapsed { "›" } else { "⌄" })
                .font(MONO)
                .size(BODY)
                .color(p.muted_2),
            text(file.path.clone())
                .font(MONO)
                .size(BODY)
                .color(p.ink)
                .width(Length::Fill),
            text(format!("+{}", file.additions))
                .font(MONO)
                .size(CAPTION)
                .color(p.green),
            text(format!("−{}", file.deletions))
                .font(MONO)
                .size(CAPTION)
                .color(p.red)
        ]
        .spacing(9)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([8, 12])
    .style(move |_, status| button::Style {
        background: matches!(status, button::Status::Hovered)
            .then_some(Background::Color(p.sunken)),
        text_color: p.ink,
        ..Default::default()
    })
    .on_press(Message::ToggleChangedFile(file.path.clone()));

    let mut content = column![header];
    if !collapsed {
        content = content.push(horizontal_divider(p));
        content = content.push(diff_file_card(
            file,
            state.review_open,
            state.busy,
            &existing,
            p,
        ));

        // Any inline comments the reviewer has queued for this file.
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
                        .size(CAPTION)
                        .color(p.blue),
                    text(comment.body.clone())
                        .font(SANS)
                        .size(LABEL)
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
            content = content.push(line_comment_composer(state, p));
        } else if state.review_open {
            // Fallback trigger: open the composer for a manually typed line when
            // clicking an added line isn't enough (e.g. a delete-only file).
            content = content.push(
                row![
                    Space::new().width(Length::Fill),
                    secondary_button(
                        "Comment on a line",
                        (!state.busy).then_some(Message::StartReviewComment(file.path.clone())),
                        p,
                    )
                ]
                .padding([8, 12]),
            );
        }
    }
    card(content, p)
}

/// Render one file's diff: tinted lines with a new-side gutter. When staging is
/// open, added lines become buttons that queue a comment on that new line.
fn diff_file_card<'a>(
    file: &'a ChangedFile,
    stageable: bool,
    busy: bool,
    existing: &[&'a ReviewComment],
    p: Palette,
) -> Element<'a, Message> {
    let mut lines = column![];
    let mut new_line: u32 = 0;
    for raw in file.patch.lines() {
        let kind = classify_diff_line(raw);
        match kind {
            DiffLineKind::Hunk => {
                if let Some(start) = hunk_new_start(raw) {
                    new_line = start;
                }
                lines = lines.push(diff_line(raw, None, mix(p.paper, p.blue, 0.10), p.blue, p));
            }
            DiffLineKind::Meta => {
                lines = lines.push(diff_line(raw, None, p.paper, p.muted_2, p));
            }
            DiffLineKind::Add => {
                let line = new_line;
                new_line = new_line.saturating_add(1);
                let bg = mix(p.paper, p.green, 0.11);
                let has_comment = existing.iter().any(|comment| comment.line == line);
                let row = diff_line(raw, Some(line), bg, p.ink_soft, p);
                if stageable && !busy && !has_comment {
                    lines = lines.push(
                        button(row)
                            .padding(0)
                            .width(Length::Fill)
                            .style(|_, _| button::Style::default())
                            .on_press(Message::StartLineComment {
                                path: file.path.clone(),
                                line,
                            }),
                    );
                } else {
                    lines = lines.push(row);
                }
                // Anchor any existing review comment right under its line.
                for comment in existing.iter().filter(|comment| comment.line == line) {
                    lines = lines.push(anchored_comment(comment, p));
                }
            }
            DiffLineKind::Remove => {
                lines = lines.push(diff_line(raw, None, mix(p.paper, p.red, 0.11), p.red, p));
            }
            DiffLineKind::Context => {
                let line = new_line;
                new_line = new_line.saturating_add(1);
                lines = lines.push(diff_line(raw, Some(line), p.paper, p.ink_softer, p));
            }
        }
    }
    scrollable(lines)
        .direction(scrollable::Direction::Both {
            vertical: scrollable::Scrollbar::new(),
            horizontal: scrollable::Scrollbar::new(),
        })
        .width(Length::Fill)
        .into()
}

fn diff_line<'a>(
    raw: &'a str,
    gutter: Option<u32>,
    bg: Color,
    ink: Color,
    p: Palette,
) -> Element<'a, Message> {
    container(
        row![
            container(
                text(gutter.map(|line| line.to_string()).unwrap_or_default())
                    .font(MONO)
                    .size(CAPTION)
                    .color(p.icon_idle)
            )
            .width(46),
            text(raw.to_owned())
                .font(MONO)
                .size(BODY)
                .color(ink)
                .wrapping(Wrapping::None)
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .width(Length::Shrink)
    .padding([1, 8])
    .style(move |_| container::Style {
        background: Some(Background::Color(bg)),
        ..Default::default()
    })
    .into()
}

fn anchored_comment(comment: &ReviewComment, p: Palette) -> Element<'static, Message> {
    container(
        column![
            text(format!("{} · L{}", comment.path, comment.line))
                .font(MONO)
                .size(CAPTION)
                .color(p.blue),
            text(comment.body.clone())
                .font(SANS)
                .size(LABEL)
                .color(p.ink_soft)
        ]
        .spacing(3),
    )
    .width(Length::Fill)
    .padding([6, 12])
    .style(move |_| container::Style {
        background: Some(Background::Color(mix(p.paper, p.blue, 0.06))),
        ..Default::default()
    })
    .into()
}

fn line_comment_composer<'a>(state: &'a State, p: Palette) -> Element<'a, Message> {
    column![
        section_label("COMMENT ON NEW VERSION", p),
        row![
            sem_input(
                "Line",
                &state.review_comment_line,
                text_input("Line", &state.review_comment_line)
                    .font(MONO)
                    .size(BODY)
                    .width(72)
                    .on_input(Message::ReviewCommentLineChanged),
            ),
            sem_input(
                "Inline comment",
                &state.review_comment_body,
                text_input("Inline comment", &state.review_comment_body)
                    .font(SANS)
                    .size(BODY)
                    .on_input(Message::ReviewCommentBodyChanged)
                    .on_submit(Message::QueueReviewComment),
            ),
        ]
        .spacing(8),
        row![
            text("Click an added line above, or type a new-file line number.")
                .font(SANS)
                .size(CAPTION)
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
    .padding([10, 12])
    .into()
}

#[derive(Clone, Copy)]
enum DiffLineKind {
    Hunk,
    Meta,
    Add,
    Remove,
    Context,
}

fn classify_diff_line(raw: &str) -> DiffLineKind {
    if raw.starts_with("@@") {
        DiffLineKind::Hunk
    } else if raw.starts_with("+++")
        || raw.starts_with("---")
        || raw.starts_with("diff ")
        || raw.starts_with("index ")
        || raw.starts_with("new file")
        || raw.starts_with("deleted file")
        || raw.starts_with("old mode")
        || raw.starts_with("new mode")
        || raw.starts_with("similarity ")
        || raw.starts_with("rename ")
        || raw.starts_with("Binary ")
        || raw.starts_with('\\')
    {
        DiffLineKind::Meta
    } else if raw.starts_with('+') {
        DiffLineKind::Add
    } else if raw.starts_with('-') {
        DiffLineKind::Remove
    } else {
        DiffLineKind::Context
    }
}

/// Parse the new-file start line from a `@@ -a,b +c,d @@` hunk header.
fn hunk_new_start(raw: &str) -> Option<u32> {
    let plus = raw.split(" +").nth(1)?;
    let number = plus
        .split([',', ' '])
        .next()?
        .trim_start_matches('+');
    number.parse().ok()
}

fn repositories(state: &State) -> &[Repository] {
    match &state.repositories {
        Resource::Ready(repos) => repos,
        _ => &[],
    }
}

fn branch_head(state: &State, name: &str) -> Option<String> {
    repository_data(state)?
        .branches
        .iter()
        .find(|branch| branch.name == name)
        .map(|branch| branch.head.clone())
}

/// The message that re-loads the current repository ref — the Retry target for
/// tree/commit/item-list errors.
fn reload_repository(state: &State) -> Message {
    match &state.selected_branch {
        Some(branch) => Message::SelectBranch(branch.clone()),
        None => state
            .selected_repo
            .clone()
            .map_or(Message::Load, Message::SelectRepository),
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
        let item = button(
            row![
                status_dot(if repo.browsable { p.green } else { p.amber }),
                text(repo.name.clone())
                    .font(SANS_SEMIBOLD)
                    .size(BODY)
                    .width(Length::Fill),
                text(repo.default_branch.clone())
                    .font(MONO)
                    .size(CAPTION)
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
        .on_press(Message::SelectRepository(repo.id.clone()));
        #[cfg(all(feature = "agent", debug_assertions))]
        let item =
            iced_agent_plugin::sem(iced_agent_plugin::Role::ListItem, repo.name.clone(), item);
        menu = menu.push(item);
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
                .size(LABEL)
                .color(p.muted_2),
        );
    }
    for branch in branches {
        let item = button(
            row![
                status_dot(p.green),
                text(branch.name.clone())
                    .font(SANS_SEMIBOLD)
                    .size(BODY)
                    .width(Length::Fill),
                text(short_hash(Some(&branch.head)))
                    .font(MONO)
                    .size(CAPTION)
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
        .on_press(Message::SelectBranch(branch.name.clone()));
        #[cfg(all(feature = "agent", debug_assertions))]
        let item =
            iced_agent_plugin::sem(iced_agent_plugin::Role::ListItem, branch.name.clone(), item);
        menu = menu.push(item);
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
                color: with_alpha(p.shadow, 0.16),
                offset: Vector::new(0.0, 6.0),
                blur_radius: 18.0,
            },
            ..Default::default()
        })
        .into()
}

/// B4: the active tab is a 2px underline under the label, not a 4-side box
/// (iced `Border` widths are uniform).
fn tab_button(tab: Tab, active: Tab, badge: Option<usize>, p: Palette) -> Element<'static, Message> {
    let is_active = tab == active;
    let mut content = row![text(tab.label()).font(SANS_SEMIBOLD).size(TITLE)].spacing(7);
    if let Some(badge) = badge {
        content = content.push(
            container(text(badge).font(MONO).size(CAPTION).color(p.muted_2))
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
    let label = button(content.align_y(Alignment::Center))
        .padding([8, 0])
        .style(move |_, _| button::Style {
            text_color: if is_active { p.ink } else { p.muted_2 },
            ..Default::default()
        })
        .on_press(Message::SelectTab(tab));
    let underline = container(Space::new())
        .width(Length::Fill)
        .height(2)
        .style(move |_| container::Style {
            background: Some(Background::Color(if is_active {
                p.filled
            } else {
                Color::TRANSPARENT
            })),
            ..Default::default()
        });
    let stacked = column![label, underline].spacing(6).align_x(Alignment::Center);
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Tab, tab.label(), stacked);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    stacked.into()
}

fn filter_button(
    label: &'static str,
    count: usize,
    filter: ItemFilter,
    active: ItemFilter,
    p: Palette,
) -> Element<'static, Message> {
    let btn = button(text(format!("{label} {count}")).font(SANS_SEMIBOLD).size(LABEL))
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
        .on_press(Message::SetItemFilter(filter));
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Button, label, btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn pull_tab_button(
    label: &'static str,
    tab: PullTab,
    active: PullTab,
    p: Palette,
) -> Element<'static, Message> {
    let btn = button(text(label).font(SANS_SEMIBOLD).size(LABEL))
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
        .on_press(Message::SelectPullTab(tab));
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Tab, label, btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
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
    container(text(label.to_owned()).font(MONO).size(CAPTION).color(tone))
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
        .size(CAPTION)
        .color(p.muted_2)
        .into()
}

fn center_note<'a>(title: &str, detail: Option<&str>, p: Palette) -> Element<'a, Message> {
    let mut content = column![
        text(title.to_owned())
            .font(SANS_SEMIBOLD)
            .size(BODY)
            .color(p.muted_2)
    ]
    .spacing(5)
    .align_x(Alignment::Center);
    if let Some(detail) = detail {
        content = content.push(
            text(detail.to_owned())
                .font(SANS)
                .size(LABEL)
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

/// A tinted error box whose message is selectable so the user can copy it.
fn error_banner<'a>(error: &'a str, p: Palette) -> Element<'a, Message> {
    container(selectable(error, SANS, LABEL, p.red, p))
        .width(Length::Fill)
        .padding([7, 9])
        .style(move |_| tinted_box(p.red, p))
        .into()
}

/// B3: an error box that also offers a Retry re-firing the producer message —
/// forge fires each fetch once, so without this a transient error is terminal.
fn retry_banner<'a>(error: &'a str, on_retry: Message, p: Palette) -> Element<'a, Message> {
    container(
        column![
            selectable(error, SANS, LABEL, p.red, p),
            row![
                Space::new().width(Length::Fill),
                secondary_button("Retry", Some(on_retry), p)
            ]
        ]
        .spacing(8),
    )
    .width(Length::Fill)
    .padding([9, 11])
    .style(move |_| tinted_box(p.red, p))
    .into()
}

/// A read-only, selectable single line (the `workspace.rs::selectable_error`
/// idiom): a borderless `text_input` with no `on_input`.
fn selectable<'a>(
    value: &'a str,
    font: Font,
    size: f32,
    color: Color,
    p: Palette,
) -> Element<'a, Message> {
    let _ = p;
    text_input("", value)
        .font(font)
        .size(size)
        .padding(0)
        .style(move |_, _| text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            icon: color,
            placeholder: color,
            value: color,
            selection: theme::ACCENTS[0],
        })
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
            shadow: card_shadow(p),
            ..Default::default()
        })
        .into()
}

/// Mode-aware card shadow (kills the hardcoded black/white shadow — `p.shadow`
/// is graphite in light mode, near-black in dark).
fn card_shadow(p: Palette) -> Shadow {
    Shadow {
        color: with_alpha(p.shadow, 0.06),
        offset: Vector::new(0.0, 1.0),
        blur_radius: 3.0,
    }
}

fn with_alpha(color: Color, alpha: f32) -> Color {
    Color { a: alpha, ..color }
}

/// Dev-only text-input tagging: wraps `input` in a `TextInput` semantic node
/// carrying `value`. Compiled out entirely unless the agent bridge is built.
#[cfg(all(feature = "agent", debug_assertions))]
fn sem_input<'a>(
    name: &'static str,
    value: &str,
    input: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    iced_agent_plugin::Sem::new(iced_agent_plugin::Role::TextInput, name, input)
        .value(value.to_string())
        .into()
}
#[cfg(not(all(feature = "agent", debug_assertions)))]
fn sem_input<'a>(
    _name: &'static str,
    _value: &str,
    input: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    input.into()
}

/// M5: a multi-line, editable comment composer backed by a `text_editor`.
fn sem_comment<'a>(
    content: &'a text_editor::Content,
    name: &'static str,
    on_action: impl Fn(text_editor::Action) -> Message + 'a,
    p: Palette,
) -> Element<'a, Message> {
    let editor = text_editor(content)
        .placeholder(name)
        .on_action(on_action)
        .font(SANS)
        .size(BODY)
        .padding([8, 10])
        .min_height(64.0)
        .style(move |_, _| text_editor::Style {
            background: Background::Color(p.paper),
            border: Border {
                color: p.border,
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            placeholder: p.muted_2,
            value: p.ink,
            selection: theme::ACCENTS[0],
        });
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::Sem::new(iced_agent_plugin::Role::TextInput, name, editor)
        .value(content.text())
        .into();
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    editor.into()
}

fn text_button(label: &'static str, message: Message, p: Palette) -> Element<'static, Message> {
    let btn = button(text(label).font(SANS_SEMIBOLD).size(TITLE))
        .padding([2, 3])
        .style(move |_, status| button::Style {
            text_color: if matches!(status, button::Status::Hovered) {
                p.ink
            } else {
                p.muted
            },
            ..Default::default()
        })
        .on_press(message);
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Button, label, btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn secondary_button(
    label: &'static str,
    message: Option<Message>,
    p: Palette,
) -> Element<'static, Message> {
    let enabled = message.is_some();
    let btn = button(text(label).font(SANS_SEMIBOLD).size(LABEL))
        .padding([6, 10])
        .style(move |_, status| {
            if enabled {
                outlined_button(status, p)
            } else {
                // Disabled controls must look disabled: dim ink, no border.
                button::Style {
                    background: Some(Background::Color(p.sunken)),
                    text_color: p.muted_2,
                    border: Border {
                        radius: RADIUS_SM.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            }
        })
        .on_press_maybe(message);
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::Sem::new(iced_agent_plugin::Role::Button, label, btn)
        .disabled(!enabled)
        .into();
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn primary_button(
    label: &'static str,
    message: Option<Message>,
    p: Palette,
) -> Element<'static, Message> {
    let enabled = message.is_some();
    let btn = button(text(label).font(SANS_SEMIBOLD).size(LABEL))
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
        .on_press_maybe(message);
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::Sem::new(iced_agent_plugin::Role::Button, label, btn)
        .disabled(!enabled)
        .into();
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
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
        .width(Length::Fill)
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
