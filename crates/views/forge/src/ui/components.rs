use super::forge::action;
use super::*;
use crate::host;
use ducktape_view_guest::slots;

impl ForgeView {
    pub(super) fn code_screen(&self) -> wire::Node {
        let mut tree = Vec::new();
        match self.tree_phase.as_str() {
            "loading" => tree.push(native::text(
                "forge/tree-loading",
                "Loading repository tree…",
            )),
            "failed" => tree.push(native::text(
                "forge/tree-failed",
                "Could not load the tree. Open the repository again to retry.",
            )),
            "ready" => {
                if !self.tree_path.is_empty() {
                    tree.push(action(
                        "forge/tree-root",
                        "Back to the repository root",
                        Some(Message::ForgeOpenDir(String::new())),
                    ));
                }
                for entry in &self.tree_entries {
                    let route = if entry.kind == "dir" {
                        Message::ForgeOpenDir(entry.path.clone())
                    } else {
                        Message::ForgeOpenFile(entry.path.clone())
                    };
                    let mut button = action(
                        format!("forge/tree/{}", entry.path),
                        &entry.name,
                        Some(route),
                    );
                    if let wire::Node::Button {
                        description,
                        checked,
                        ..
                    } = &mut button
                    {
                        *description = Some(entry.path.clone());
                        *checked = Some(entry.path == self.file_path);
                    }
                    tree.push(button);
                }
                if self.tree_entries.is_empty() {
                    let empty = if !self.tree_born {
                        "This repository has no commits yet."
                    } else {
                        "This directory is empty."
                    };
                    tree.push(native::text("forge/tree-empty", empty));
                }
                if self.tree_truncated {
                    tree.push(native::text(
                        "forge/tree-omitted",
                        "Some directory entries are not shown.",
                    ));
                }
            }
            _ => {}
        }
        let pane = native::sized(
            native::container(
                "forge/tree-pane",
                native::scroll("forge/tree-scroll", native::column("forge/tree", tree)),
            ),
            Some(wire::Length::Fixed(self.tree_width as f32)),
            Some(wire::Length::Fill),
        );
        let resize = wire::Node::ResizeHandle {
            key: "forge/tree-resize".into(),
            on_press: None,
            on_release: None,
            on_drag: Some(slots::handler::<(f64, f64), Message>(Box::new(|(x, y)| {
                Some(Message::TreeResized(x, y))
            }))),
            cursor: Some(wire::mouse::Cursor::ResizingHorizontally),
            content: Box::new(native::sized(
                native::container("forge/tree-edge", native::text("forge/tree-grip", "⋮")),
                Some(wire::Length::Fixed(10.)),
                Some(wire::Length::Fill),
            )),
        };
        native::sized(
            native::row(
                "forge/code",
                [
                    pane,
                    resize,
                    native::scroll("forge/file-scroll", self.file_screen()),
                ],
            ),
            Some(wire::Length::Fill),
            Some(wire::Length::Fill),
        )
    }

    fn file_screen(&self) -> wire::Node {
        let path = host::forge_file_header(
            &self.opened_dir,
            &self.opened_rev,
            &self.file_path,
            &self.tree_rev,
        );
        let mut content = vec![
            native::text("forge/file-header", &path),
            native::text("forge/code-context", "Synced from the node · view only"),
        ];
        if self.tree_phase == "loading" {
            content.push(native::text(
                "forge/loading-tree",
                "Loading repository tree…",
            ));
        }
        if self.tree_phase == "failed" {
            content.push(native::text(
                "forge/unavailable-code",
                "Repository code is unavailable.",
            ));
        }
        if path.is_empty() && self.tree_phase == "ready" {
            let label = if !self.tree_born {
                "This repository has no commits yet."
            } else if self.tree_entries.is_empty() && !self.tree_truncated {
                "This commit has no files."
            } else {
                "Choose a file from the tree."
            };
            content.push(native::text("forge/choose-file", label));
        }
        if !path.is_empty() {
            match self.file_phase.as_str() {
                "loading" => content.push(native::text("forge/loading-file", "Loading file…")),
                "failed" => content.push(native::text("forge/file-failed", &self.file_note)),
                "ready" => {
                    if self.file_binary {
                        content.push(native::text(
                            "forge/binary",
                            "Binary file — no text preview.",
                        ));
                    } else if self.file_picture {
                        content.push(wire::Node::Surface {
                            key: "forge/file-picture".into(),
                            name: "picture".into(),
                            args: vec![
                                wire::SurfaceValue::Str("forge".into()),
                                wire::SurfaceValue::Str(self.file_path.clone()),
                            ],
                            on_event: None,
                        });
                        content.push(native::text(
                            "forge/picture-caption",
                            host::picture_caption(self.file_width, self.file_height),
                        ));
                    } else {
                        let markdown = host::markdown_path(&self.file_path);
                        content.push(wire::Node::Surface {
                            key: "forge/file-text".into(),
                            name: if markdown {
                                "forge_markdown"
                            } else {
                                "forge_code"
                            }
                            .into(),
                            args: vec![
                                wire::SurfaceValue::Str(self.file_text.clone()),
                                wire::SurfaceValue::Str(self.file_path.clone()),
                                wire::SurfaceValue::Bool(self.dark),
                            ],
                            on_event: markdown.then(|| {
                                slots::handler(Box::new(|value| match value {
                                    wire::SurfaceValue::Str(link) => {
                                        Some(Message::OpenMessageLink(link))
                                    }
                                    _ => None,
                                }))
                            }),
                        });
                    }
                    if self.file_truncated {
                        content.push(native::text(
                            "forge/file-truncated",
                            "This file is larger than the 64 KiB preview limit.",
                        ));
                    }
                    if !self.file_note.is_empty() {
                        content.push(native::text("forge/file-note", &self.file_note));
                    }
                }
                _ => {}
            }
        }
        native::column("forge/file", content)
    }

    pub(super) fn diff_screen(&self) -> wire::Node {
        let mut content = vec![
            native::heading("forge/diff-title", "Changes"),
            native::text(
                "forge/diff-count",
                host::forge_stats(
                    self.forge_item_files_changed,
                    self.forge_item_additions,
                    self.forge_item_deletions,
                ),
            ),
        ];
        let rows = self
            .diff_rows
            .iter()
            .map(|line| {
                let key = format!("forge/diff/{}", line.key);
                match line.kind.as_str() {
                    "file" => native::heading(key, &line.text),
                    "hunk" => native::text(key, &line.text),
                    _ => {
                        let line_number = if line.side == "old" {
                            &line.old_no
                        } else {
                            &line.new_no
                        };
                        let mut cells = vec![
                            native::text(format!("{key}/old"), &line.old_no),
                            native::text(format!("{key}/new"), &line.new_no),
                            native::text(format!("{key}/sign"), &line.sign),
                        ];
                        let text = native::text_options(
                            native::text(format!("{key}/text"), &line.text),
                            wire::TextOptions {
                                wrapping: Some(wire::Wrapping::None),
                                font: Some(wire::NamedFont {
                                    family: wire::FontFamily::Monospace,
                                    weight: wire::Weight::Normal,
                                    stretch: wire::FontStretch::Normal,
                                    style: wire::FontStyle::Normal,
                                }),
                                ..Default::default()
                            },
                        );
                        cells.push(text);
                        if !line.path.is_empty() {
                            cells.push(action(
                                format!("{key}/comment"),
                                "Comment on this line",
                                Some(Message::ForgeCommentOpen(
                                    line.path.clone(),
                                    line_number.clone(),
                                    line.side.clone(),
                                )),
                            ));
                        }
                        native::sized(
                            native::row(key, cells),
                            None,
                            Some(wire::Length::Fixed(24.)),
                        )
                    }
                }
            })
            .collect();
        content.push(wire::Node::KeyedColumn {
            key: "forge/diff-lines".into(),
            keys: Some(
                self.diff_rows
                    .iter()
                    .map(|line| wire::ListKey::from(line.key))
                    .collect(),
            ),
            children: rows,
            background: None,
            border: None,
            spacing: None,
            padding: None,
            width: Some(wire::Length::Fill),
            height: None,
            max_width: None,
            align: None,
            virtual_row: Some(24.),
        });
        if self.forge_item_diff_truncated {
            content.push(native::text(
                "forge/diff-truncated",
                "This diff is truncated; open the repository locally to see the rest.",
            ));
        }
        native::column("forge/diff", content)
    }

    pub(super) fn merge_screen(&self) -> wire::Node {
        let mut content = vec![native::heading("forge/merge-title", "Merge")];
        match self.forge_item_state.as_str() {
            "merged" => content.push(native::text(
                "forge/merged",
                host::forge_merge_note(&self.forge_item_merge_oid, &self.forge_item_branches),
            )),
            "closed" => content.push(native::text("forge/closed", "This pull request is closed.")),
            "open" => {
                if !self.merge_conflicts.is_empty() {
                    content.push(native::text(
                        "forge/conflict-title",
                        "Merge conflicts — resolve on the branch and push again:",
                    ));
                    for path in &self.merge_conflicts {
                        content.push(native::text(format!("forge/conflict/{path}"), path));
                    }
                }
                if self.forge_item_change_requests > 0 {
                    content.push(native::text(
                        "forge/changes-requested",
                        format!(
                            "{} reviewers requested changes — merge not recommended",
                            self.forge_item_change_requests
                        ),
                    ));
                }
                let available =
                    self.connected && !self.merge_busy && !self.forge_item_source_oid.is_empty();
                content.push(action(
                    "forge/merge",
                    if self.merge_busy {
                        "Merging…"
                    } else {
                        "Merge pull request"
                    },
                    available.then_some(Message::ForgeMergeSubmit),
                ));
            }
            _ => {}
        }
        native::column("forge/merge-box", content)
    }

    pub(super) fn review_screen(&self) -> wire::Node {
        let mut content = vec![native::heading("forge/reviews-title", "Reviews")];
        if self.forge_item_reviews.is_empty() {
            content.push(native::text("forge/no-reviews", "No reviews yet."));
        }
        for (index, review) in self.forge_item_reviews.iter().enumerate() {
            let key = format!("forge/review/{index}");
            let mut details = vec![
                native::heading(format!("{key}/author"), &review.author_name),
                native::text(
                    format!("{key}/verdict"),
                    host::verdict_label(&review.verdict),
                ),
                native::text(format!("{key}/commit"), &review.commit),
                self.finality(format!("{key}/finality"), review.created_at),
                self.rich_body(
                    format!("{key}/body"),
                    Message::OpenMessageLink,
                    review.blocks.clone(),
                ),
            ];
            if review.outdated {
                details.push(native::text(format!("{key}/outdated"), "outdated"));
            }
            for (index, comment) in review.comments.iter().enumerate() {
                details.push(native::text(
                    format!("{key}/comment/{index}/anchor"),
                    &comment.anchor,
                ));
                details.push(self.rich_body(
                    format!("{key}/comment/{index}/body"),
                    Message::OpenMessageLink,
                    comment.blocks.clone(),
                ));
            }
            content.push(native::column(key, details));
        }
        let available = self.connected && !self.review_busy;
        content.push(native::row(
            "forge/verdicts",
            [
                ("comment", "Comment"),
                ("approve", "Approve"),
                ("request_changes", "Request changes"),
            ]
            .into_iter()
            .map(|(value, label)| {
                let mut button = action(
                    format!("forge/verdict/{value}"),
                    label,
                    available.then(|| Message::ForgeReviewPick(value.into())),
                );
                if let wire::Node::Button { checked, .. } = &mut button {
                    *checked = Some(self.review_verdict == value);
                }
                button
            }),
        ));
        let target =
            host::forge_comment_target(&self.comment_path, &self.comment_line, &self.comment_side);
        let comment_capacity = !host::forge_comment_cap_reached(&self.staged_comments);
        if !target.is_empty() {
            content.push(native::row(
                "forge/comment-target",
                [
                    native::text("forge/comment-anchor", target),
                    action(
                        "forge/cancel-comment",
                        "Cancel",
                        Some(Message::ForgeCommentCancel),
                    ),
                ],
            ));
            let submit = available && comment_capacity && !self.comment_draft.is_empty();
            content.push(self.review_input(
                "forge/comment-body",
                "Comment on this line…",
                &self.comment_draft,
                Message::CommentDraftChanged,
                submit.then(|| Message::ForgeCommentStage(self.comment_draft.clone())),
            ));
            content.push(action(
                "forge/add-comment",
                "Add comment",
                submit.then(|| Message::ForgeCommentStage(self.comment_draft.clone())),
            ));
        }
        if !comment_capacity {
            content.push(native::text(
                "forge/comment-limit",
                "Comment limit reached for one review — submit this review, then start another.",
            ));
        }
        for (index, comment) in self.staged_comments.iter().enumerate() {
            let key = format!("forge/staged/{index}");
            content.push(native::column(
                &key,
                [
                    native::text(format!("{key}/anchor"), &comment.anchor),
                    native::text(format!("{key}/status"), "not sent yet"),
                    native::text(format!("{key}/body"), &comment.body),
                    action(
                        format!("{key}/remove"),
                        "Remove staged comment",
                        Some(Message::ForgeCommentDrop(comment.anchor.clone())),
                    ),
                ],
            ));
        }
        let submit = available
            && !self.forge_item_source_oid.is_empty()
            && (!self.review_draft.is_empty() || !self.staged_comments.is_empty());
        content.push(self.review_input(
            "forge/review-body",
            "Leave a review…",
            &self.review_draft,
            Message::ReviewDraftChanged,
            submit.then(|| Message::ForgeReviewSubmit(self.review_draft.clone())),
        ));
        content.push(action(
            "forge/submit-review",
            "Submit review",
            submit.then(|| Message::ForgeReviewSubmit(self.review_draft.clone())),
        ));
        native::column("forge/reviews", content)
    }

    fn review_input(
        &self,
        key: &str,
        hint: &str,
        value: &str,
        route: fn(String) -> Message,
        submit: Option<Message>,
    ) -> wire::Node {
        let mut input = native::input(
            key,
            hint,
            value,
            slots::handler(Box::new(move |value| Some(route(value)))),
            submit.map(slots::message),
        );
        if let wire::Node::Input { options, .. } = &mut input {
            options.disabled = self.review_busy || !self.connected;
        }
        input
    }
}
