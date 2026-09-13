use super::*;
use ducktape_view_guest::slots;

fn action(key: String, label: &str, message: Message, disabled: bool) -> wire::Node {
    native::button(
        key,
        label,
        (!disabled).then(|| slots::message(message)),
        wire::ButtonPreset::Secondary,
    )
}
fn field(
    key: String,
    label: &str,
    value: &str,
    route: fn(String) -> Message,
    submit: Option<Message>,
    disabled: bool,
) -> wire::Node {
    let mut node = native::input(
        key,
        label,
        value,
        slots::handler::<String, Message>(Box::new(move |value| Some(route(value)))),
        submit.map(slots::message),
    );
    if let wire::Node::Input { options, .. } = &mut node {
        options.label = label.into();
        options.disabled = disabled;
    }
    node
}
fn divider(key: String, route: fn(f64, f64) -> Message) -> wire::Node {
    wire::Node::ResizeHandle {
        key,
        on_press: None,
        on_release: None,
        on_drag: Some(slots::handler::<(f64, f64), Message>(Box::new(
            move |(x, y)| Some(route(x, y)),
        ))),
        cursor: Some(wire::mouse::Cursor::ResizingHorizontally),
        content: Box::new(wire::Node::Space {
            width: Some(wire::Length::Fixed(10.)),
            height: Some(wire::Length::Fill),
        }),
    }
}
impl ChatView {
    pub(super) fn chat_screen(&self, key: String) -> wire::Node {
        if !self.connected {
            return self.disconnected(format!("{key}/disconnected"));
        }
        let mut panes = vec![
            self.sidebar(&key),
            divider(format!("{key}/sidebar-resize"), Message::SidebarResized),
            self.room(&key),
        ];
        if self.channel_settings_open && !self.active_channel.is_empty() {
            panes.push(divider(
                format!("{key}/details-resize"),
                Message::DetailsResized,
            ));
            panes.push(self.channel_details(format!("{key}/details-pane")));
        }
        if self.active_thread_seq > 0 && !self.active_channel.is_empty() {
            panes.push(divider(
                format!("{key}/thread-resize"),
                Message::ThreadResized,
            ));
            panes.push(self.thread(format!("{key}/thread-pane")));
        }
        native::sized(
            native::row(key, panes),
            Some(wire::Length::Fill),
            Some(wire::Length::Fill),
        )
    }
    fn sidebar(&self, key: &str) -> wire::Node {
        let mut search = field(
            format!("{key}/channel-sidebar/chat-search"),
            "Search messages",
            &self.search_draft,
            Message::SearchDraftChanged,
            Some(Message::SearchChatSubmit),
            false,
        );
        if let wire::Node::Input { placeholder, .. } = &mut search {
            *placeholder = "Search…".into();
        }
        let mut children = vec![
            native::heading(format!("{key}/network"), &self.network_name),
            native::text(
                format!("{key}/status"),
                format!(
                    "{} · {}",
                    self.status,
                    crate::host::height_label(self.block_height)
                ),
            ),
            search,
        ];
        let search_active =
            self.search_phase != SearchPhase::Idle || !self.search_draft.trim().is_empty();
        if search_active {
            children.push(action(
                format!("{key}/clear-search"),
                "Clear message search",
                Message::ClearChatSearch,
                false,
            ));
        }
        children.push(native::row(
            format!("{key}/channels-header"),
            [
                native::heading(format!("{key}/channels-label"), "Channels"),
                action(
                    format!("{key}/new-channel"),
                    if self.channel_create_open {
                        "Close new channel"
                    } else {
                        "New channel"
                    },
                    Message::ToggleChannelCreate,
                    self.loading || self.busy,
                ),
            ],
        ));
        let mut rooms = Vec::new();
        for room in &self.rooms {
            rooms.push(self.channel_button(
                format!("{key}/channel/{}", room.channel.id),
                Message::ChooseChannel,
                room.channel.clone(),
                room.channel.id == self.active_channel,
                room.unread,
            ));
        }
        if !self.dm_rows.is_empty() {
            rooms.push(native::heading(
                format!("{key}/dm-heading"),
                "Direct messages",
            ));
        }
        for row in &self.dm_rows {
            rooms.push(self.direct_message(
                format!("{key}/dm/{}", row.peer.key),
                Message::ChooseDm,
                row.peer.clone(),
                row.peer.key == self.active_dm_peer,
                row.unread,
            ));
        }
        children.push(native::scroll(
            format!("{key}/rooms"),
            native::column(format!("{key}/room-list"), rooms),
        ));
        native::sized(
            native::container(
                format!("{key}/channel-sidebar"),
                native::column(format!("{key}/sidebar-content"), children),
            ),
            Some(wire::Length::Fixed(self.sidebar_width as f32)),
            Some(wire::Length::Fill),
        )
    }
    fn room(&self, key: &str) -> wire::Node {
        let mut header = Vec::new();
        if self.active_dm.name.is_empty() {
            header.push(native::sized(
                native::heading(format!("{key}/room-name"), &self.active_channel_name),
                Some(wire::Length::Fill),
                None,
            ));
        } else {
            header.push(native::sized(
                self.direct_message_header(format!("{key}/dm-header")),
                Some(wire::Length::Fill),
                None,
            ));
        }
        if self.active_channel_archived {
            header.push(self.archived_badge(format!("{key}/archived")));
        }
        if self.active_channel_members_only {
            header.push(self.private_badge(format!("{key}/private")));
        }
        if self.huddle_joined {
            header.push(self.huddle_controls(
                format!("{key}/huddle"),
                || Message::LeaveHuddleHere,
                || Message::ShowHuddle,
            ));
        } else if !self.active_channel.is_empty() {
            header.push(self.start_huddle(format!("{key}/huddle"), || Message::JoinHuddleSubmit));
        }
        header.push(action(
            format!("{key}/details"),
            "Channel details",
            Message::ToggleChannelSettings,
            self.active_channel.is_empty(),
        ));
        let mut children = vec![native::row(format!("{key}/header"), header)];
        if !self.host_error.is_empty() {
            children.push(native::text(format!("{key}/error"), &self.host_error));
        }
        let query_matches =
            !self.search_query.is_empty() && self.search_draft.trim() == self.search_query;
        let search_stands = query_matches
            && (self.search_phase == SearchPhase::Searching
                || crate::host::search_answer_stands(
                    &self.search_query,
                    &self.search_draft,
                    false,
                ));
        if search_stands {
            children.push(self.search_results(format!("{key}/search-results")));
        } else {
            if self.loading && self.messages.is_empty() {
                children.push(self.loading_messages(format!("{key}/loading")));
            }
            if !self.loading && self.messages.is_empty() {
                children.push(self.empty_messages(format!("{key}/empty")));
            }
            if self.has_older_history {
                children.push(action(
                    format!("{key}/older"),
                    if self.history_loading {
                        "Loading older messages…"
                    } else {
                        "Load older messages"
                    },
                    Message::LoadMoreHistory,
                    self.loading || self.history_loading || self.busy,
                ));
            }
            children.push(self.message_list(
                format!("{key}/message-stream"),
                &self.messages,
                CopySurface::Timeline,
            ));
            if self.copy_surface == CopySurface::Timeline {
                children.push(self.selection_bar(format!("{key}/copy-range"), &self.messages));
            }
            if !self.messages.is_empty() && (self.history_view || !self.at_live_tail) {
                children.push(action(
                    format!("{key}/latest"),
                    "↓  Jump to latest",
                    Message::ChooseChannel(self.active_channel.clone()),
                    false,
                ));
            }
            if self.selected_message_seq > 0 {
                children.push(self.message_menu(key, false));
            }
        }
        if !self.post_refusal.is_empty() {
            children.push(self.composer_gate(format!("{key}/refusal")));
        }
        children.push(wire::Node::Surface {
            key: format!("{key}/composer"),
            name: "chat_composer".into(),
            args: vec![
                wire::SurfaceValue::Str(crate::host::composer_scope(
                    &self.endpoint,
                    &self.active_channel,
                )),
                wire::SurfaceValue::Str("message".into()),
                wire::SurfaceValue::Bool(false),
                wire::SurfaceValue::Str("Message the channel…".into()),
                wire::SurfaceValue::Bool(
                    self.loading
                        || !self.connected
                        || self.active_channel.is_empty()
                        || !self.post_refusal.is_empty(),
                ),
                wire::SurfaceValue::Bool(self.busy),
                wire::SurfaceValue::Str("An earlier message wasn’t sent".into()),
            ],
            on_event: None,
        });
        native::sized(
            native::column(format!("{key}/room"), children),
            Some(wire::Length::Fill),
            Some(wire::Length::Fill),
        )
    }
    fn search_results(&self, key: String) -> wire::Node {
        let children = match self.search_phase {
            SearchPhase::Searching => vec![self.loading_messages(format!("{key}/loading"))],
            SearchPhase::Done if self.search_hits.is_empty() => {
                vec![native::text(format!("{key}/empty"), "No messages match")]
            }
            SearchPhase::Done => self
                .search_hits
                .iter()
                .map(|hit| {
                    self.search_result(
                        format!("{key}/{}/{}", hit.channel_id, hit.seq),
                        Message::OpenChatSearchHit,
                        hit.clone(),
                    )
                })
                .collect(),
            SearchPhase::Idle => Vec::new(),
        };
        native::scroll(key.clone(), native::column(format!("{key}/rows"), children))
    }
    fn message_list(
        &self,
        key: String,
        messages: &[crate::host::ChatMessage],
        surface: CopySurface,
    ) -> wire::Node {
        let thread = surface == CopySurface::Thread;
        let selected = if thread {
            self.thread_selected_seq
        } else {
            self.selected_message_seq
        };
        let mut keys = Vec::new();
        let mut rows = Vec::new();
        for message in messages {
            let scope = format!("{key}/message/{}", message.view_key);
            let ranged = crate::host::seq_in_copy_range(
                message.seq,
                self.copy_anchor_seq,
                self.copy_head_seq,
                self.copy_surface,
                surface,
            );
            let target =
                selected == message.seq || (thread && self.thread_target_seq == message.seq);
            let plate = crate::host::message_plate(message.deleted, target, ranged);
            let mut children = Vec::new();
            if !thread && self.unread_boundary > 0 && message.seq == self.unread_marker_seq {
                children.push(native::text(format!("{scope}/unread"), "New messages"));
            }
            children.push(self.message_card(message, surface, plate));
            for live in &self.live_agents {
                if crate::host::run_in_thread(live, message.seq) {
                    let run_key = format!("{scope}/run/{}", live.agent);
                    let content = if thread {
                        self.live_run_card(
                            run_key,
                            Message::CancelRun,
                            Message::OpenRun,
                            live.clone(),
                        )
                    } else {
                        action(
                            run_key,
                            &crate::host::live_thread_label(&live.agent),
                            Message::OpenThreadFor(message.seq),
                            false,
                        )
                    };
                    children.push(content);
                }
            }
            let actions = if thread {
                [
                    Message::OpenThreadMessageReactions(
                        message.seq,
                        message.body.clone(),
                        message.rev,
                    ),
                    Message::OpenThreadMessageActions(
                        message.seq,
                        message.body.clone(),
                        message.rev,
                    ),
                ]
            } else {
                [
                    Message::OpenMessageReactions(message.seq, message.body.clone(), message.rev),
                    Message::OpenMessageActions(message.seq, message.body.clone(), message.rev),
                ]
            };
            let [reaction, more] = actions;
            if !message.pending && !message.deleted {
                if !thread && message.reply_count == 0 {
                    children.push(action(
                        format!("{scope}/thread"),
                        "Open thread",
                        Message::OpenThreadFor(message.seq),
                        false,
                    ));
                }
                children.push(action(
                    format!("{scope}/thumbs-up"),
                    "React with 👍",
                    Message::AddReactionAt(message.seq, "👍".into()),
                    self.active_channel_archived,
                ));
                children.push(native::row(
                    format!("{scope}/actions"),
                    [
                        action(
                            format!("{scope}/react"),
                            "Manage reactions",
                            reaction,
                            self.active_channel_archived,
                        ),
                        action(
                            format!("{scope}/more"),
                            "More message actions",
                            more.clone(),
                            false,
                        ),
                    ],
                ));
                let content = native::column(format!("{scope}/content"), children);
                rows.push(wire::Node::MouseArea {
                    key: scope,
                    on_press: None,
                    on_release: None,
                    on_double_click: None,
                    on_right_press: Some(slots::message(more)),
                    on_right_release: None,
                    on_middle_press: None,
                    on_middle_release: None,
                    on_enter: None,
                    on_exit: None,
                    on_move: None,
                    on_press_at: None,
                    on_scroll: None,
                    content: Box::new(content),
                });
            } else {
                rows.push(native::column(scope, children));
            }
            keys.push(wire::ListKey::from(message.view_key));
        }
        let list = wire::Node::KeyedColumn {
            key: format!("{key}/rows"),
            keys: Some(keys),
            children: rows,
            background: None,
            border: None,
            spacing: None,
            padding: None,
            width: Some(wire::Length::Fill),
            height: None,
            max_width: None,
            align: None,
            virtual_row: Some(44.0f32),
        };
        let mut scroll = native::scroll(key, list);
        if let wire::Node::Scroll {
            virtual_rows,
            anchor_y,
            on_scroll,
            ..
        } = &mut scroll
        {
            *virtual_rows = true;
            *anchor_y = wire::ScrollAnchor::End;
            if !thread {
                *on_scroll = Some(slots::handler::<(f32, f32, f32, f32), Message>(Box::new(
                    |(x, y, rx, ry)| {
                        Some(Message::ChatScrolled(
                            x.into(),
                            y.into(),
                            rx.into(),
                            ry.into(),
                        ))
                    },
                )));
            }
        }
        scroll
    }
    fn selection_bar(&self, key: String, messages: &[crate::host::ChatMessage]) -> wire::Node {
        native::row(
            key.clone(),
            [
                native::text(
                    format!("{key}/count"),
                    crate::host::copy_range_label(crate::host::copy_range_count(
                        messages,
                        self.copy_anchor_seq,
                        self.copy_head_seq,
                    )),
                ),
                action(
                    format!("{key}/clear"),
                    "Clear",
                    Message::ClearCopyRange,
                    false,
                ),
                action(
                    format!("{key}/copy-range"),
                    "Copy",
                    Message::CopySelectedMessages,
                    false,
                ),
            ],
        )
    }
    fn thread(&self, key: String) -> wire::Node {
        let mut children = vec![native::row(
            format!("{key}/header"),
            [
                native::heading(format!("{key}/title"), "Thread"),
                action(
                    format!("{key}/close"),
                    "Close thread",
                    Message::CloseThread,
                    false,
                ),
            ],
        )];
        if self.thread_loading && self.thread_messages.is_empty() {
            children.push(self.loading_messages(format!("{key}/loading")));
        }
        if self.thread_has_more {
            children.push(action(
                format!("{key}/older"),
                "Load more replies",
                Message::LoadMoreThread,
                self.thread_loading || self.busy,
            ));
        }
        children.push(self.message_list(
            format!("{key}/thread-stream"),
            &self.thread_messages,
            CopySurface::Thread,
        ));
        if self.copy_surface == CopySurface::Thread {
            children.push(self.selection_bar(format!("{key}/copy-range"), &self.thread_messages));
        }
        if self.thread_selected_seq > 0 {
            children.push(self.message_menu(&key, true));
        }
        children.push(wire::Node::Surface {
            key: format!("{key}/reply_composer"),
            name: "chat_composer".into(),
            args: vec![
                wire::SurfaceValue::Str(crate::host::thread_scope(
                    &self.endpoint,
                    &self.active_channel,
                    self.active_thread_seq,
                )),
                wire::SurfaceValue::Str("reply".into()),
                wire::SurfaceValue::Bool(true),
                wire::SurfaceValue::Str("Reply…".into()),
                wire::SurfaceValue::Bool(
                    self.thread_loading || !self.connected || !self.post_refusal.is_empty(),
                ),
                wire::SurfaceValue::Bool(false),
                wire::SurfaceValue::Str("Unsent reply".into()),
            ],
            on_event: None,
        });
        native::sized(
            native::column(key, children),
            Some(wire::Length::Fixed(self.thread_width as f32)),
            Some(wire::Length::Fill),
        )
    }
    fn channel_details(&self, key: String) -> wire::Node {
        let mut children = vec![
            native::row(
                format!("{key}/header"),
                [
                    native::heading(format!("{key}/title"), "Channel details"),
                    action(
                        format!("{key}/close"),
                        "Close channel details",
                        Message::ToggleChannelSettings,
                        false,
                    ),
                ],
            ),
            native::text(format!("{key}/name"), &self.active_channel_name),
            self.name_label(format!("{key}/name-label")),
            field(
                format!("{key}/name-input"),
                "Channel name",
                &self.channel_name_draft,
                Message::ChannelNameDraftChanged,
                Some(Message::RenameChannelSubmit),
                self.busy,
            ),
            action(
                format!("{key}/rename"),
                "Rename",
                Message::RenameChannelSubmit,
                self.busy || self.channel_name_draft.trim().is_empty(),
            ),
            action(
                format!("{key}/link"),
                "Copy channel link",
                Message::CopyToClipboard(
                    crate::host::duck_channel_link(
                        self.active_channel.clone(),
                        self.network_chain_id.clone(),
                    ),
                    "Channel link copied".into(),
                ),
                false,
            ),
            self.members_label(format!("{key}/members-label")),
            field(
                format!("{key}/member-input"),
                "Member account or public key",
                &self.member_key_draft,
                Message::MemberKeyDraftChanged,
                Some(Message::AddChannelMemberSubmit),
                self.busy,
            ),
            action(
                format!("{key}/add-member"),
                "Add",
                Message::AddChannelMemberSubmit,
                self.busy || self.member_key_draft.trim().is_empty(),
            ),
        ];
        if self.active_channel_archived {
            children.push(self.archived_badge(format!("{key}/archived")));
        }
        if self.active_channel_members_only {
            children.push(self.private_badge(format!("{key}/private")));
        }
        if self.channel_members.is_empty() {
            children.push(native::text(format!("{key}/no-members"), "No members added. An Open channel needs none — membership only gates posting in a members-only channel."));
        }
        for member in &self.channel_members {
            children.push(self.member_row(
                format!("{key}/member/{}", member.key),
                Message::RemoveChannelMemberSubmit,
                member.clone(),
            ));
        }
        let (label, message) = if self.active_channel_archived {
            ("Unarchive channel", Message::UnarchiveChannelSubmit)
        } else {
            ("Archive channel", Message::ArchiveChannelSubmit)
        };
        children.push(action(format!("{key}/archive"), label, message, self.busy));
        native::sized(
            native::container(
                key.clone(),
                native::scroll(
                    format!("{key}/scroll"),
                    native::column(format!("{key}/content"), children),
                ),
            ),
            Some(wire::Length::Fixed(self.details_width as f32)),
            Some(wire::Length::Fill),
        )
    }
    fn message_menu(&self, key: &str, thread: bool) -> wire::Node {
        let (seq, rev, body, mode, close) = if thread {
            (
                self.thread_selected_seq,
                self.thread_selected_rev,
                &self.thread_edit_draft,
                self.thread_message_action,
                Message::ClearThreadMessageSelection,
            )
        } else {
            (
                self.selected_message_seq,
                self.selected_message_rev,
                &self.message_edit_draft,
                self.message_action,
                Message::ClearMessageSelection,
            )
        };
        let prefix = if thread { "thread-" } else { "message-" };
        let focus = match mode {
            MessageAction::Reactions => "reaction-focus",
            MessageAction::Delete => "delete-focus",
            _ => "action-focus",
        };
        let mut children = Vec::new();
        match mode {
            MessageAction::Toolbar | MessageAction::More => {
                let reaction = if thread {
                    Message::OpenThreadMessageReactions(seq, body.clone(), rev)
                } else {
                    Message::OpenMessageReactions(seq, body.clone(), rev)
                };
                let edit = if thread {
                    Message::BeginThreadMessageEdit(seq, body.clone(), rev)
                } else {
                    Message::BeginMessageEdit(seq, body.clone(), rev)
                };
                let delete = if thread {
                    Message::ArmThreadMessageDelete(seq, body.clone(), rev)
                } else {
                    Message::ArmMessageDelete(seq, body.clone(), rev)
                };
                children.extend([
                    action(
                        format!("{key}/{prefix}add-reaction"),
                        "Add reaction",
                        reaction,
                        self.active_channel_archived,
                    ),
                    action(
                        format!("{key}/{prefix}copy-link"),
                        "Copy message link",
                        Message::CopyMessageLink(crate::host::duck_channel_message_link(
                            self.active_channel.clone(),
                            seq,
                            self.network_chain_id.clone(),
                        )),
                        false,
                    ),
                    action(
                        format!("{key}/{prefix}edit"),
                        "Edit message",
                        edit,
                        self.active_channel_archived,
                    ),
                    action(
                        format!("{key}/{prefix}delete"),
                        "Delete message",
                        delete,
                        self.active_channel_archived,
                    ),
                ]);
            }
            MessageAction::Reactions => {
                let mut choices = Vec::new();
                for emoji in crate::host::reaction_palette() {
                    let mut button = action(
                        format!("{key}/{prefix}reaction/{emoji}"),
                        &emoji,
                        Message::AddReactionAt(seq, emoji.clone()),
                        self.active_channel_archived,
                    );
                    if let wire::Node::Button {
                        label, description, ..
                    } = &mut button
                    {
                        *label = Some("Add reaction".into());
                        *description = Some(emoji);
                    }
                    choices.push(button);
                }
                children.push(wire::Node::Grid {
                    key: format!("{key}/{prefix}reaction-grid"),
                    columns: Some(8),
                    fluid: None,
                    spacing: Some(4.),
                    padding: None,
                    width: Some(wire::Length::Fill),
                    height: None,
                    aspect: None,
                    background: None,
                    border: None,
                    children: choices,
                });
            }
            MessageAction::Editing => {
                children.push(wire::Node::Surface {
                    key: format!("{key}/{prefix}edit-composer"),
                    name: "chat_composer".into(),
                    args: vec![
                        wire::SurfaceValue::Str(crate::host::edit_scope(
                            &self.endpoint,
                            &self.active_channel,
                            seq,
                        )),
                        wire::SurfaceValue::Str(if thread { "thread_edit" } else { "edit" }.into()),
                        wire::SurfaceValue::Bool(true),
                        wire::SurfaceValue::Str("Edit message".into()),
                        wire::SurfaceValue::Bool(self.busy),
                        wire::SurfaceValue::Bool(false),
                        wire::SurfaceValue::Str("Could not save changes".into()),
                    ],
                    on_event: None,
                });
            }
            MessageAction::Delete => {
                children.push(native::text(
                    format!("{key}/{prefix}confirm"),
                    "Delete this message?",
                ));
                children.push(action(
                    format!("{key}/{prefix}confirm-delete"),
                    "Delete",
                    if thread {
                        Message::DeleteThreadMessageSubmit
                    } else {
                        Message::DeleteMessageSubmit
                    },
                    self.busy,
                ));
            }
        }
        children.push(action(
            format!("{key}/{prefix}close"),
            if mode == MessageAction::Editing {
                "Cancel message edit"
            } else {
                "Cancel"
            },
            close,
            self.busy && mode == MessageAction::Editing,
        ));
        native::column(format!("{key}/{prefix}{focus}"), children)
    }
}
