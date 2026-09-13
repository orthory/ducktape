impl PagesView {
    fn pages(&self) -> Node {
        let sidebar = kit::sized(
            kit::container(format!("{PAGE_KEY}/page-list"), self.sidebar()),
            Some(Length::Fixed(self.sidebar_width as f32)),
            Some(Length::Fill),
        );
        let divider = Node::ResizeHandle {
            key: format!("{PAGE_KEY}/sidebar-divider"),
            on_press: None,
            on_release: None,
            on_drag: Some(slots::handler(Box::new(|(x, y): (f64, f64)| {
                Some(Message::SidebarResized(x, y))
            }))),
            cursor: Some(wire::mouse::Cursor::ResizingHorizontally),
            content: Box::new(Node::Space {
                width: Some(Length::Fixed(4.)),
                height: Some(Length::Fill),
            }),
        };
        let mut body = Vec::new();
        if !self.host_error.is_empty() {
            body.push(kit::text("pages/host-error", &self.host_error));
        }
        if self.connected && !self.active_page.is_empty() {
            body.push(self.document_toolbar());
        }
        body.push(measured(
            "PagesView/root/pages/pane-measure",
            self.document_pane(),
            Message::PagesPaneResized,
        ));
        let mut main = fill(kit::column("pages/main", body));
        if self.connected && self.page_delete_armed {
            main = Node::Stack {
                padding: None,
                background: None,
                border: None,
                clip: false,
                key: "pages/delete-stack".into(),
                width: Some(Length::Fill),
                height: Some(Length::Fill),
                under: 1,
                children: vec![main, self.delete_dialog()],
            };
        }
        fill(kit::spaced(
            kit::row(PAGE_KEY, [sidebar, divider, main]),
            0.,
        ))
    }

    fn sidebar(&self) -> Node {
        if !self.connected {
            return fill(kit::column(
                "pages/sidebar",
                [kit::heading("pages/sidebar/title", "Pages")],
            ));
        }
        let mut rows = vec![kit::padded(
            kit::row(
                "pages/sidebar/header",
                [
                    kit::heading("pages/sidebar/title", "Pages"),
                    kit::text("pages/sidebar/count", self.pages.len().to_string()),
                    action(
                        "pages/sidebar/new",
                        if self.page_create_open {
                            "Cancel"
                        } else {
                            "New page"
                        },
                        Message::TogglePageCreate,
                        !self.unavailable(),
                        ButtonPreset::Secondary,
                    ),
                ],
            ),
            wire::Edges::all(8.),
        )];
        if self.page_create_open {
            rows.push(input(
                format!("{PAGE_KEY}/new-page"),
                "New page",
                &self.page_draft,
                Message::PageDraftChanged,
                Some(Message::CreatePageSubmit),
                self.unavailable(),
            ));
            rows.push(action(
                "pages/sidebar/create",
                "Create page",
                Message::CreatePageSubmit,
                !self.unavailable() && !self.page_draft.trim().is_empty(),
                ButtonPreset::Primary,
            ));
        }
        rows.push(kit::scroll(
            "pages/sidebar/scroll",
            kit::column(
                "pages/sidebar/list",
                self.pages.iter().map(|page| self.page_button(page)),
            ),
        ));
        fill(kit::column("pages/sidebar", rows))
    }

    fn document_toolbar(&self) -> Node {
        let title = if self.active_page_title.is_empty() {
            "Untitled"
        } else {
            &self.active_page_title
        };
        let mut controls = vec![kit::text("pages/toolbar/title", title)];
        if !self.active_page_parent.is_empty() {
            controls.push(kit::text("pages/toolbar/parent", &self.active_page_parent));
        }
        controls.push(kit::sized(
            input(
                format!("{PAGE_KEY}/page-search"),
                "Search pages…",
                &self.page_search_draft,
                Message::SearchDraftChanged,
                Some(Message::SearchPagesSubmit),
                !self.host_error.is_empty() || self.page_searching,
            ),
            Some(Length::Fixed(190.)),
            None,
        ));
        let searching = !self.page_search_draft.is_empty() || !self.page_search_query.is_empty();
        if searching {
            controls.push(action(
                "pages/toolbar/clear-search",
                "Clear search",
                Message::ClearPageSearch,
                true,
                ButtonPreset::Text,
            ));
        }
        controls.push(named(
            kit::button_child(
                "pages/toolbar/comments",
                kit::row(
                    "pages/toolbar/comments-label",
                    [
                        kit::text("pages/toolbar/comments-text", "Comments"),
                        kit::text(
                            "pages/toolbar/comments-count",
                            self.thread_total.to_string(),
                        ),
                    ],
                ),
                (!self.unavailable()).then(|| slots::message(Message::ToggleBlockComments)),
                ButtonPreset::Secondary,
            ),
            "Comments",
        ));
        controls.push(action(
            "pages/toolbar/link",
            "Copy page link",
            Message::CopyToClipboard(self.page_link.clone(), "Page link".into()),
            !self.page_link.is_empty(),
            ButtonPreset::Text,
        ));
        controls.push(action(
            "pages/toolbar/menu",
            "Page actions",
            Message::TogglePageMenu,
            !self.unavailable(),
            ButtonPreset::Text,
        ));
        let status = match self.autosave.as_str() {
            "saving" => "Saving…",
            "error" => "Not saved",
            _ => "Saved",
        };
        controls.push(kit::text("pages/toolbar/save-status", status));
        kit::padded(kit::row("pages/toolbar", controls), wire::Edges::all(8.))
    }

    fn document_pane(&self) -> Node {
        let surface = if !self.connected {
            fill(empty_state(
                "pages/disconnected",
                "Not connected",
                "Connect to a network to read and edit pages.",
            ))
        } else if self.active_page.is_empty() {
            if self.loading {
                fill(empty_state(
                    "pages/loading",
                    "Loading pages…",
                    "Your documents will appear here.",
                ))
            } else {
                fill(empty_state(
                    "pages/selection",
                    "Choose a page",
                    "Select a page or create a new one.",
                ))
            }
        } else {
            self.document_surface()
        };
        let mut children = vec![surface];
        let search_ready = self.connected
            && crate::host::search_answer_stands(
                &self.page_search_query,
                &self.page_search_draft,
                self.page_searching,
            );
        if search_ready {
            children.push(self.search_panel());
        }
        if self.connected && !self.active_page.is_empty() && self.block_comments_open {
            children.push(self.comments_layer());
        }
        if self.connected && self.page_menu_open {
            let menu = kit::padded(
                kit::container(
                    "pages/menu/card",
                    action(
                        "pages/menu/delete",
                        "Delete page",
                        Message::ArmPageDelete,
                        !self.unavailable(),
                        ButtonPreset::Danger,
                    ),
                ),
                wire::Edges::all(8.),
            );
            children.push(overlay(
                "pages/menu",
                kit::sized(menu, Some(Length::Fixed(200.)), None),
                Message::ClosePageMenu,
                wire::AlignX::Right,
                wire::AlignY::Top,
            ));
        }
        Node::Stack {
            padding: None,
            background: None,
            border: None,
            clip: false,
            key: "pages/document/stack".into(),
            width: Some(Length::Fill),
            height: Some(Length::Fill),
            under: 1,
            children,
        }
    }

    fn document_surface(&self) -> Node {
        let mut content = Vec::new();
        if !self.page_refusal.is_empty() {
            content.push(kit::text("pages/document/refusal", &self.page_refusal));
        }
        for (index, draft) in self.orphaned_comment_drafts.iter().enumerate() {
            let key = format!("pages/recovered/{index}");
            content.push(kit::column(
                &key,
                [
                    kit::text(format!("{key}/text"), draft),
                    kit::row(
                        format!("{key}/actions"),
                        [
                            action(
                                format!("{key}/restore"),
                                "Use recovered draft",
                                Message::UseOrphanedCommentDraft(draft.clone()),
                                !self.unavailable(),
                                ButtonPreset::Secondary,
                            ),
                            action(
                                format!("{key}/discard"),
                                "Discard",
                                Message::DiscardOrphanedCommentDraft(draft.clone()),
                                true,
                                ButtonPreset::Text,
                            ),
                        ],
                    ),
                ],
            ));
        }
        let notice = crate::editor_view::presentation_notice(&self.document, &self.document_paint);
        if !notice.is_empty() {
            content.push(kit::text("pages/document/presentation-notice", notice));
        }
        if !self.document_error.is_empty() {
            content.push(kit::text("pages/document/error", &self.document_error));
        }
        content.push(self.document_editor());
        for page in &self.subpages {
            content.push(action(
                format!("pages/subpage/{}", page.id),
                &page.title,
                Message::ChoosePage(page.id.clone()),
                !self.unavailable(),
                ButtonPreset::Text,
            ));
        }
        let mut surface = fill(kit::padded(
            kit::container(
                "pages/document/surface",
                fill(kit::column("pages/document/content", content)),
            ),
            wire::Edges {
                top: 26.,
                right: 40.,
                bottom: 18.,
                left: 22.,
            },
        ));
        if let Node::Container { max_width, .. } = &mut surface {
            *max_width = Some(crate::host::document_width(
                self.pages_pane_width,
                self.block_comments_open,
            ) as f32);
        }
        surface
    }

    fn search_panel(&self) -> Node {
        let results = if self.page_search_hits.is_empty() {
            kit::text("pages/search/empty", "No matching pages")
        } else {
            kit::scroll(
                "pages/search/scroll",
                kit::column(
                    "pages/search/results",
                    self.page_search_hits
                        .iter()
                        .map(|hit| self.search_result(hit)),
                ),
            )
        };
        let contents = fill(kit::column(
            "pages/search/content",
            [
                kit::row(
                    "pages/search/header",
                    [
                        kit::heading(
                            "pages/search/query",
                            format!("Search · {}", self.page_search_query),
                        ),
                        action(
                            "pages/search/clear",
                            "Clear search",
                            Message::ClearPageSearch,
                            true,
                            ButtonPreset::Secondary,
                        ),
                    ],
                ),
                results,
            ],
        ));
        let panel = kit::sized(
            kit::padded(
                kit::container("pages/search/panel", contents),
                wire::Edges::all(16.),
            ),
            Some(Length::Fixed(
                (self.pages_pane_width as f32 - 48.).clamp(240., 720.),
            )),
            Some(Length::Fixed(400.)),
        );
        overlay(
            "pages/search",
            panel,
            Message::ClearPageSearch,
            wire::AlignX::Center,
            wire::AlignY::Top,
        )
    }

    fn document_editor(&self) -> Node {
        let (document, on_document) = self
            .document
            .document("app:document".into(), Message::DocumentUpdated);
        let presentation =
            crate::editor_view::paint(self.document.state_view(), &self.document_paint);
        presentation
            .validate(self.document.state_view().text)
            .expect("invalid editor presentation");
        let editable = self.connected
            && !self.loading
            && self.host_error.is_empty()
            && !self.active_page.is_empty()
            && self.active_page == self.buffer_page;
        Node::Editor {
            key: format!("{PAGE_KEY}/document"),
            document,
            on_document,
            editable,
            placeholder: "Write with Markdown…".into(),
            width: None,
            height: None,
            min_height: None,
            max_height: None,
            options: Box::new(wire::EditorOptions {
                binding: Some(Box::new(
                    crate::editor_binding::keys(
                        self.document_history.clone(),
                        self.document_menu.clone(),
                    )
                    .register(Message::DocumentCommitted, Message::DocumentTransaction),
                )),
                presentation: Some(Box::new(presentation)),
                size: Some(14.),
                line_height: Some(wire::LineHeight::Relative(1.65)),
                wrapping: Some(wire::Wrapping::Word),
                ..Default::default()
            }),
        }
    }

    fn comments_layer(&self) -> Node {
        use wire::FloatOp::{Add, Geometry, Multiply, Number, Subtract};
        Node::Float {
            key: "pages/comments/anchor".into(),
            x: wire::FloatExpression {
                ops: vec![
                    Geometry(4),
                    Geometry(6),
                    Add,
                    Geometry(0),
                    Subtract,
                    Geometry(2),
                    Subtract,
                    Number(crate::host::comments_right_anchor(self.pages_pane_width)),
                    Multiply,
                    Number(crate::host::comments_left_inset(self.pages_pane_width)),
                    Add,
                ],
            },
            y: wire::FloatExpression {
                ops: vec![Number(crate::host::comment_card_offset(
                    self.pages_pane_width,
                    self.comment_anchor_y,
                    self.pages_viewport_height,
                ))],
            },
            scale: 1.,
            shadow: Default::default(),
            radius: None,
            content: Box::new(measured(
                "pages/comments/size",
                self.comments_card(),
                Message::CommentsCardMeasured,
            )),
        }
    }

    fn comments_card(&self) -> Node {
        let disabled = self.unavailable() || self.threads_loading;
        let mut header = vec![kit::heading(
            "pages/comments/title",
            crate::host::comment_scope_label(
                &self.blocks,
                &self.scope_target,
                &self.active_page,
                self.thread_total,
            ),
        )];
        if !self.scope_pinned && !self.scope_target.is_empty() {
            header.push(action(
                "pages/comments/widen",
                "All comments on this page",
                Message::WidenCommentScope,
                true,
                ButtonPreset::Text,
            ));
        }
        header.push(action(
            "pages/comments/close",
            "Close",
            Message::CloseBlockComments,
            true,
            ButtonPreset::Text,
        ));
        let mut threads = Vec::new();
        if self.threads_loading {
            threads.push(kit::text("pages/comments/loading", "Loading comments…"));
        }
        let groups = crate::host::scope_groups(
            self.comment_rows.clone(),
            &self.scope_target,
            &self.active_page,
        );
        let resolved = crate::host::scope_resolved(self.comment_rows.clone(), &self.scope_target);
        if groups.is_empty() && resolved.is_empty() {
            threads.push(kit::text(
                "pages/comments/empty",
                crate::host::empty_scope_label(&self.scope_target),
            ));
        }
        for group in groups {
            let block_anchor = self.scope_target.is_empty()
                && group.target != self.active_page
                && !group.anchor.is_empty();
            if block_anchor {
                threads.push(named(
                    action(
                        format!("pages/comments/scope/{}", group.target),
                        &group.anchor,
                        Message::NarrowCommentScope(group.target.clone()),
                        !disabled,
                        ButtonPreset::Text,
                    ),
                    "Comments on this block",
                ));
            }
            threads.extend(
                group
                    .threads
                    .iter()
                    .map(|thread| self.comment_thread(thread)),
            );
        }
        if !resolved.is_empty() {
            threads.push(named(
                action(
                    "pages/comments/resolved",
                    crate::host::resolved_label(&resolved),
                    Message::ToggleResolvedComments,
                    true,
                    ButtonPreset::Text,
                ),
                "Resolved threads",
            ));
            if self.resolved_open {
                threads.extend(resolved.iter().map(|row| self.comment_thread(&row.thread)));
            }
        }
        let body = fill(kit::column(
            "pages/comments/content",
            [
                kit::row("pages/comments/header", header),
                kit::scroll(
                    "pages/comments/scroll",
                    kit::column("pages/comments/threads", threads),
                ),
                kit::text(
                    "pages/comments/hint",
                    crate::host::compose_hint_of(
                        &self.blocks,
                        &self.scope_target,
                        &self.active_page,
                    ),
                ),
                input(
                    format!("{PAGE_KEY}/page-comment({})", self.active_page),
                    "Start a thread…",
                    &self.block_comment_draft,
                    Message::CommentDraftChanged,
                    Some(Message::PostBlockCommentSubmit),
                    disabled,
                ),
                action(
                    "pages/comments/submit",
                    "Post",
                    Message::PostBlockCommentSubmit,
                    !disabled && !self.block_comment_draft.trim().is_empty(),
                    ButtonPreset::Primary,
                ),
            ],
        ));
        let limit =
            crate::host::comment_card_height(self.comment_anchor_y, self.pages_viewport_height)
                as f32;
        let mut card = kit::sized(
            kit::padded(
                kit::container("PagesView/root/pages/comments-card", body),
                wire::Edges::all(12.),
            ),
            Some(Length::Fixed(
                crate::host::comments_card_width(self.pages_pane_width) as f32,
            )),
            Some(Length::Fixed(limit)),
        );
        if let Node::Container { clip, .. } = &mut card {
            *clip = true;
        }
        card
    }
}
