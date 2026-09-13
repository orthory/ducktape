use super::*;
use crate::host;
use ducktape_view_guest::slots;

pub(super) fn action(key: impl Into<String>, label: &str, message: Option<Message>) -> wire::Node {
    native::button(
        key,
        label,
        message.map(slots::message),
        wire::ButtonPreset::Secondary,
    )
}

fn picker(
    key: &str,
    options: Vec<String>,
    selected: &str,
    placeholder: Option<String>,
    route: fn(String) -> Message,
) -> wire::Node {
    let selected = options
        .iter()
        .position(|value| value == selected)
        .map(|index| index as u32);
    let choices = options.clone();
    wire::Node::PickList {
        key: key.into(),
        options,
        selected,
        placeholder,
        on_select: slots::handler(Box::new(move |index: u32| {
            choices.get(index as usize).cloned().map(route)
        })),
        width: None,
        style: Default::default(),
        settings: Default::default(),
    }
}

impl ForgeView {
    pub(super) fn forge_screen(&self) -> wire::Node {
        if !self.connected {
            return self.disconnected("forge/disconnected".into());
        }
        let mut content = Vec::new();
        if !self.host_error.is_empty() {
            content.push(self.unavailable("forge/error".into()));
        }
        if self.open_repo.is_empty() {
            content.push(native::heading("forge/organization", &self.org));
            content.push(native::text("forge/about", &self.about));
            content.push(native::text("forge/tier", &self.tier));
            content.push(native::text(
                "forge/repo-count",
                host::plural(self.repos.len() as i64, "repository", "repositories"),
            ));
            if self.repos.is_empty() {
                let label = match self.list_phase.as_str() {
                    "loading" => "Loading repositories…",
                    "failed" => "Could not load repositories. Reconnect to retry.",
                    "ready" => {
                        "No repositories yet — push a git repository to this network to create one."
                    }
                    _ => "",
                };
                content.push(native::text("forge/list-status", label));
                if self.list_phase == "ready" {
                    let command = host::forge_push_command(&self.connected_rpc);
                    content.push(native::text("forge/push-command", &command));
                    content.push(action(
                        "forge/copy-push",
                        "Copy command",
                        Some(Message::CopyToClipboard(command, "Command copied".into())),
                    ));
                }
            }
            for repo in &self.repos {
                content.push(native::column(
                    format!("forge/repo/{}", repo.name),
                    [
                        action(
                            format!("forge/open/{}", repo.name),
                            &repo.name,
                            Some(Message::ForgeOpenRepo(repo.name.clone())),
                        ),
                        native::text(format!("forge/head/{}", repo.name), &repo.head),
                    ],
                ));
            }
            return native::scroll("forge/repositories", native::column("forge/list", content));
        }
        let mut header = vec![
            action(
                "forge/all-repos",
                "All repos",
                Some(Message::ForgeCloseRepo),
            ),
            picker(
                "ForgeView/forge/repo-pick",
                host::repo_names(&self.repos),
                &self.open_repo,
                None,
                Message::ForgeOpenRepo,
            ),
        ];
        if !self.branches.is_empty() {
            header.push(picker(
                "ForgeView/forge/branch-pick",
                host::branch_names(&self.branches),
                &host::forge_tree_branch(&self.branches, &self.tree_pick, &self.tree_rev),
                Some(host::commit_label(&self.tree_rev)),
                Message::ForgePickBranch,
            ));
        }
        content.push(native::row("forge/navigation", header));
        let tabs = [
            ("code", "Code"),
            ("pulls", "Pull requests"),
            ("issues", "Issues"),
        ];
        content.push(native::row(
            "forge/tabs",
            tabs.into_iter().map(|(tab, label)| {
                let mut button = action(
                    format!("forge/tab/{tab}"),
                    label,
                    (self.tab != tab || self.forge_item_number > 0)
                        .then(|| Message::SelectForgeTab(tab.into())),
                );
                if let wire::Node::Button { checked, label, .. } = &mut button {
                    *checked = Some(self.tab == tab);
                    *label = Some(
                        match tab {
                            "code" => "Browse the code",
                            "pulls" => "Show pull requests",
                            _ => "Show issues",
                        }
                        .into(),
                    );
                }
                let kind = match tab {
                    "pulls" => "pr",
                    "issues" => "issue",
                    _ => "",
                };
                if kind.is_empty() {
                    return button;
                }
                native::row(
                    format!("forge/tab-count/{tab}"),
                    [
                        button,
                        native::text(
                            format!("forge/count/{tab}"),
                            host::forge_open_count(&self.items, kind).to_string(),
                        ),
                    ],
                )
            }),
        ));
        let body = if self.forge_item_number > 0 {
            self.item_screen()
        } else {
            match self.tab.as_str() {
                "code" => self.code_screen(),
                "issues" => self.tracker_screen("issues"),
                _ => self.tracker_screen("pulls"),
            }
        };
        content.push(body);
        native::sized(
            native::column("forge/root", content),
            Some(wire::Length::Fill),
            Some(wire::Length::Fill),
        )
    }

    fn tracker_screen(&self, tab: &str) -> wire::Node {
        let items = host::filter_forge_items(&self.items, tab);
        let mut content = Vec::new();
        match self.repo_phase.as_str() {
            "loading" => content.push(self.loading_tracker("forge/tracker-loading".into())),
            "failed" => content.push(self.tracker_unavailable("forge/tracker-failed".into())),
            "ready" => {
                if items.is_empty() {
                    content.push(if tab == "issues" {
                        self.empty_issues("forge/no-issues".into())
                    } else {
                        self.empty_pulls("forge/no-pulls".into())
                    });
                }
                for item in items {
                    let key = format!("forge/item/{}", item.number);
                    content.push(native::column(
                        &key,
                        [
                            action(
                                format!("{key}/open"),
                                &item.title,
                                Some(Message::ForgeOpenItem(item.number)),
                            ),
                            native::text(format!("{key}/number"), format!("#{}", item.number)),
                            native::text(format!("{key}/state"), &item.state),
                            native::text(format!("{key}/author"), &item.author_name),
                        ],
                    ));
                }
            }
            _ => {}
        }
        native::scroll(
            "forge/tracker",
            native::column("forge/tracker-content", content),
        )
    }

    fn item_screen(&self) -> wire::Node {
        let mut content = vec![action(
            "forge/back",
            "Back to tracker",
            Some(Message::ForgeCloseItem),
        )];
        match self.item_phase.as_str() {
            "loading" => content.push(self.loading_item("forge/item-loading".into())),
            "failed" => content.push(self.item_unavailable("forge/item-failed".into())),
            "ready" => {
                content.extend([
                    native::heading("forge/item-title", &self.forge_item_title),
                    native::text("forge/item-number", format!("#{}", self.forge_item_number)),
                    native::text("forge/item-state", &self.forge_item_state),
                    native::text("forge/item-author", &self.forge_item_author),
                    native::text("forge/item-branches", &self.forge_item_branches),
                    action(
                        "forge/copy-item",
                        "Copy link",
                        Some(Message::CopyToClipboard(
                            host::duck_forge_item_link(
                                &self.open_repo,
                                self.forge_item_number,
                                &self.network_chain_id,
                            ),
                            "Link copied".into(),
                        )),
                    ),
                ]);
                if !self.forge_item_body.is_empty() {
                    content
                        .push(self.item_body("forge/item-body".into(), Message::OpenMessageLink));
                }
                if !self.diff_rows.is_empty() {
                    content.push(self.diff_screen());
                }
                if self.forge_item_kind == "pr" {
                    content.push(self.merge_screen());
                    content.push(self.review_screen());
                }
                content.push(native::heading("forge/discussion-title", "Discussion"));
                for note in &self.linked_note {
                    content.push(native::heading(
                        format!("forge/linked/{}/title", note.seq),
                        "Linked note",
                    ));
                    content.push(self.note(format!("forge/linked/{}", note.seq), note));
                }
                if self.discussion.is_empty() && !self.discussion_clipped {
                    content.push(native::text("forge/no-discussion", "No discussion yet."));
                }
                if self.discussion_clipped {
                    content.push(native::text(
                        "forge/discussion-clipped",
                        "Older comments are not shown.",
                    ));
                }
                for note in &self.discussion {
                    content.push(self.note(format!("forge/note/{}", note.seq), note));
                }
                content.push(wire::Node::Surface {
                    key: "forge/note-composer".into(),
                    name: "forge_composer".into(),
                    args: vec![
                        wire::SurfaceValue::Str(host::composer_scope(
                            &self.connected_rpc,
                            &self.forge_item_channel,
                        )),
                        wire::SurfaceValue::Str("note".into()),
                        wire::SurfaceValue::Bool(true),
                        wire::SurfaceValue::Str("Write a note…".into()),
                        wire::SurfaceValue::Bool(
                            !self.connected
                                || self.forge_item_channel.is_empty()
                                || self.item_phase != "ready",
                        ),
                        wire::SurfaceValue::Bool(false),
                        wire::SurfaceValue::Str("The note wasn’t sent".into()),
                    ],
                    on_event: None,
                });
            }
            _ => {}
        }
        native::scroll(
            "forge/item-scroll",
            native::column("forge/item-content", content),
        )
    }

    fn note(&self, key: String, note: &host::ChatMessage) -> wire::Node {
        native::column(
            &key,
            [
                native::text(
                    format!("{key}/avatar"),
                    if note.avatar_kind == "human" {
                        note.initial.clone()
                    } else {
                        format!("AI · {}", note.initial)
                    },
                ),
                native::heading(format!("{key}/author"), &note.author),
                native::text(format!("{key}/meta"), &note.meta),
                self.rich_body(
                    format!("{key}/body"),
                    Message::OpenMessageLink,
                    note.blocks.clone(),
                ),
            ],
        )
    }
}
