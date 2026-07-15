use iced::widget::{
    Button, Column, Space, button, column, container, row, scrollable, text, text_input,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Shadow, Vector};

use crate::icons::{self, Icon};
use crate::theme::{self, MONO, Palette, RADIUS_LG, RADIUS_MD, RADIUS_SM, SANS, SANS_SEMIBOLD};

use super::*;

const TREE_WIDTH: f32 = 258.0;
const BODY_PAD: f32 = 24.0;

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
