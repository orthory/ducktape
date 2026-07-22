ui_lang::include_app!("src/ui/app.ice");

mod backend;

fn main() -> iced::Result {
    Ducktape::run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_selection_stays_in_the_component_scope() {
        let (mut app, _) = Ducktape::__boot();
        let _ = app.__update(__DucktapeMessage::__WorkspaceTabsHandleSelectTab(
            "workspace-tabs".into(),
            "pages".into(),
        ));

        assert_eq!(
            app.__ice_component_workspacetabs["workspace-tabs"].tab,
            "pages"
        );
    }

    #[test]
    fn background_refresh_preserves_editing_state() {
        let root = include_str!("ui/app.ice");
        let view = include_str!("ui/view.ice");
        let lifecycle = include_str!("ui/handlers/lifecycle.ice");
        assert!(!view.contains("sync_phase"));
        assert!(root.contains("use \"view.ice\""));
        assert!(!lifecycle.contains("on refresh_now"));
        assert!(!view.contains("button \"Refresh\""));

        let refresh = lifecycle
            .split_once("on workspace_refreshed(next)\n")
            .unwrap()
            .1
            .split_once("\non ")
            .unwrap()
            .0;
        let editable = [
            "rpc",
            "password",
            "channel_draft",
            "channel_name_draft",
            "member_key_draft",
            "message_draft",
            "message_edit_draft",
            "reply_draft",
            "chat_search_draft",
            "page_draft",
            "block_draft",
            "page_search_draft",
        ];
        let overwrites_editable = refresh.lines().any(|line| {
            editable
                .iter()
                .any(|name| line.trim_start().starts_with(&format!("{name} =")))
        });
        assert!(!overwrites_editable);
        assert!(lifecycle.contains("run live_events(connected_rpc) when connected"));
        assert!(!lifecycle.contains("every 1s"));
        assert!(lifecycle.contains("run refresh(connected_rpc"));
        assert!(lifecycle.contains("active_page_title = next.active_page_title"));
        assert!(lifecycle.contains("block_edit_draft = refreshed_block_draft("));
    }

    #[test]
    fn stale_refresh_is_ignored_after_user_action() {
        let (mut app, _) = Ducktape::__boot();
        app.status = "current".into();
        app.sync_phase = "refreshing".into();
        app.hydration_generation = 3;
        app.loading = false;

        let _ = app.__update(__DucktapeMessage::ChooseChannel("next".into()));
        assert_eq!(app.sync_phase, "idle");
        assert_eq!(app.hydration_generation, 4);

        let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
            kind: "changed".into(),
            status: "Live".into(),
            height: 99,
        }));
        assert!(app.live_dirty);

        let _ = app.__update(__DucktapeMessage::WorkspaceRefreshed(
            backend::WorkspaceData {
                generation: 3,
                rpc: "http://stale".into(),
                status: "stale".into(),
                height: 99,
                channels: Vec::new(),
                messages: Vec::new(),
                active_channel: String::new(),
                active_channel_name: String::new(),
                active_channel_archived: false,
                active_channel_members_only: false,
                active_channel_huddle_count: 0,
                channel_members: Vec::new(),
                pages: Vec::new(),
                blocks: Vec::new(),
                active_page: String::new(),
                active_page_title: String::new(),
                active_page_parent: String::new(),
            },
        ));
        assert_eq!(app.status, "Live");
        assert_eq!(app.sync_phase, "idle");

        app.sync_phase = "refreshing".into();
        let _ = app.__update(__DucktapeMessage::WorkspaceRefreshed(
            backend::WorkspaceData {
                generation: 4,
                rpc: "http://current".into(),
                status: "fresh".into(),
                height: 100,
                channels: Vec::new(),
                messages: Vec::new(),
                active_channel: String::new(),
                active_channel_name: String::new(),
                active_channel_archived: false,
                active_channel_members_only: false,
                active_channel_huddle_count: 0,
                channel_members: Vec::new(),
                pages: Vec::new(),
                blocks: Vec::new(),
                active_page: "page".into(),
                active_page_title: "Remote title".into(),
                active_page_parent: String::new(),
            },
        ));
        assert_eq!(app.active_page_title, "Remote title");
    }

    #[test]
    fn optimistic_send_never_erases_the_next_draft() {
        let (mut app, _) = Ducktape::__boot();
        app.connected = true;
        app.loading = false;
        app.active_channel = "general".into();
        app.message_draft = "first".into();

        let _ = app.__update(__DucktapeMessage::SendMessageSubmit);
        assert_eq!(app.mutation_phase, "message");
        assert!(app.message_draft.is_empty());
        assert_eq!(app.messages.len(), 1);
        assert!(app.messages[0].pending);

        app.message_draft = "second".into();
        let _ = app.__update(__DucktapeMessage::ChatMutated(backend::ChatData {
            channels: Vec::new(),
            messages: vec![backend::ChatMessage {
                id: "message-1".into(),
                seq: 1,
                author: "you".into(),
                meta: "#1".into(),
                body: "first".into(),
                pending: false,
                rev: 0,
                edited: false,
                deleted: false,
                reply_count: 0,
                thread_seq: 0,
                reactions: Vec::new(),
            }],
            active_channel: "general".into(),
            active_channel_name: "general".into(),
            active_channel_archived: false,
            active_channel_members_only: false,
            active_channel_huddle_count: 0,
            channel_members: Vec::new(),
            selected_message_seq: 0,
            selected_message_rev: 0,
            selected_message_body: String::new(),
            active_thread_seq: 0,
            thread_target_seq: 0,
            thread_messages: Vec::new(),
            thread_next_reply_offset: 0,
            thread_has_more: false,
        }));
        assert_eq!(app.message_draft, "second");
        assert_eq!(app.mutation_phase, "idle");
        assert!(!app.messages[0].pending);
    }

    #[test]
    fn message_actions_require_explicit_intent() {
        let (mut app, _) = Ducktape::__boot();
        app.mutation_phase = "idle".into();

        let _ = app.__update(__DucktapeMessage::SelectMessage(7, "hello".into(), 2));
        assert_eq!(app.message_action, "toolbar");
        let _ = app.__update(__DucktapeMessage::BeginMessageEdit);
        assert_eq!(app.message_action, "editing");
        let _ = app.__update(__DucktapeMessage::CancelMessageAction);
        assert_eq!(app.message_action, "toolbar");
        let _ = app.__update(__DucktapeMessage::ManageReactions);
        assert_eq!(app.message_action, "reactions");
        let _ = app.__update(__DucktapeMessage::CancelMessageAction);
        let _ = app.__update(__DucktapeMessage::ArmMessageDelete);
        assert_eq!(app.message_action, "delete");
    }

    #[test]
    fn selecting_another_message_invalidates_the_pending_thread() {
        let (mut app, _) = Ducktape::__boot();
        app.mutation_phase = "idle".into();
        app.selected_message_seq = 1;
        app.thread_generation = 4;
        app.thread_loading = true;
        app.active_thread_seq = 1;
        app.thread_messages = backend::optimistic_message(Vec::new(), "old thread".into());
        app.reply_draft = "old reply".into();

        let _ = app.__update(__DucktapeMessage::SelectMessage(2, "next".into(), 0));
        assert_eq!(app.thread_generation, 5);
        assert!(!app.thread_loading);
        assert_eq!(app.active_thread_seq, 0);
        assert!(app.thread_messages.is_empty());
        assert!(app.reply_draft.is_empty());

        let _ = app.__update(__DucktapeMessage::ThreadLoaded(backend::ThreadLoadData {
            generation: 4,
            root_seq: 1,
            target_seq: 0,
            messages: Vec::new(),
            next_reply_offset: 0,
            has_more: false,
        }));
        assert_eq!(app.active_thread_seq, 0);
    }

    #[test]
    fn thread_pages_and_new_replies_preserve_loaded_messages() {
        let message = |seq: i64, thread_seq: i64, body: &str| backend::ChatMessage {
            id: format!("message-{seq}"),
            seq,
            author: "user".into(),
            meta: format!("#{seq}"),
            body: body.into(),
            pending: false,
            rev: 0,
            edited: false,
            deleted: false,
            reply_count: 0,
            thread_seq,
            reactions: Vec::new(),
        };
        let (mut app, _) = Ducktape::__boot();
        app.active_thread_seq = 1;
        app.thread_generation = 7;
        app.thread_loading = true;
        app.thread_messages = vec![message(1, 0, "root"), message(2, 1, "first")];

        let _ = app.__update(__DucktapeMessage::ThreadPageLoaded(
            backend::ThreadPageData {
                generation: 7,
                messages: vec![message(3, 1, "second")],
                next_reply_offset: 2,
                has_more: false,
            },
        ));
        assert_eq!(app.thread_messages.len(), 3);
        assert_eq!(app.thread_messages[1].body, "first");
        assert_eq!(app.thread_next_reply_offset, 2);

        app.thread_messages = backend::optimistic_message(app.thread_messages, "third".into());
        app.pending_reply = "third".into();
        app.mutation_phase = "reply".into();
        let _ = app.__update(__DucktapeMessage::ThreadMutated(message(4, 1, "third")));
        assert_eq!(app.thread_messages.len(), 4);
        assert_eq!(app.thread_messages[1].body, "first");
        assert_eq!(app.thread_messages[3].body, "third");
        assert!(!app.thread_messages.iter().any(|message| message.pending));
        assert_eq!(app.thread_next_reply_offset, 3);

        let (mut partial, _) = Ducktape::__boot();
        partial.active_thread_seq = 1;
        partial.thread_next_reply_offset = 256;
        partial.thread_has_more = true;
        partial.thread_messages =
            backend::optimistic_message(vec![message(1, 0, "root")], "new tail".into());
        partial.mutation_phase = "reply".into();
        let _ = partial.__update(__DucktapeMessage::ThreadMutated(message(
            300, 1, "new tail",
        )));
        assert_eq!(partial.thread_next_reply_offset, 256);

        partial.thread_generation = 8;
        partial.thread_loading = true;
        let _ = partial.__update(__DucktapeMessage::ThreadPageLoaded(
            backend::ThreadPageData {
                generation: 8,
                messages: vec![message(257, 1, "unseen"), message(300, 1, "new tail")],
                next_reply_offset: 258,
                has_more: false,
            },
        ));
        assert_eq!(partial.thread_next_reply_offset, 258);
        assert_eq!(partial.thread_messages.len(), 3);
        assert_eq!(partial.thread_messages[1].body, "unseen");
        assert_eq!(partial.thread_messages[2].body, "new tail");
    }

    #[test]
    fn changing_endpoint_clears_remote_bound_interaction_state() {
        let (mut app, _) = Ducktape::__boot();
        app.loading = false;
        app.connected_rpc = "http://node-a".into();
        app.rpc = "http://node-b".into();
        app.password = "node-a-password".into();
        app.selected_message_seq = 1;
        app.message_action = "editing".into();
        app.message_edit_draft = "node a edit".into();
        app.active_thread_seq = 1;
        app.reply_draft = "node a reply".into();
        app.selected_block_id = "same-id".into();
        app.block_edit_draft = "node a block".into();
        app.message_draft = "node a message".into();
        app.page_search_draft = "node a search".into();

        let _ = app.__update(__DucktapeMessage::Reconnect);

        assert_eq!(app.connected_rpc, "http://node-b");
        assert!(app.password.is_empty());
        assert_eq!(app.selected_message_seq, 0);
        assert_eq!(app.message_action, "toolbar");
        assert!(app.message_edit_draft.is_empty());
        assert_eq!(app.active_thread_seq, 0);
        assert!(app.reply_draft.is_empty());
        assert!(app.selected_block_id.is_empty());
        assert!(app.block_edit_draft.is_empty());
        assert!(app.message_draft.is_empty());
        assert!(app.page_search_draft.is_empty());

        let _ = app.__update(__DucktapeMessage::Failed(backend::AppError {
            message: "offline".into(),
            committed: false,
        }));
        assert_eq!(app.connected_rpc, "http://node-b");
    }

    #[test]
    fn same_endpoint_reconnect_preserves_unsent_drafts() {
        let (mut app, _) = Ducktape::__boot();
        app.loading = false;
        app.connected_rpc = "http://node-a".into();
        app.rpc = "http://node-a/".into();
        app.message_draft = "next message".into();
        app.failed_message_draft = "unsent message".into();

        let _ = app.__update(__DucktapeMessage::Reconnect);

        assert_eq!(app.rpc, "http://node-a");
        assert_eq!(app.connected_rpc, "http://node-a");
        assert_eq!(app.message_draft, "next message");
        assert_eq!(app.failed_message_draft, "unsent message");
    }

    #[test]
    fn page_search_load_selects_the_canonical_block() {
        let (mut app, _) = Ducktape::__boot();
        let _ = app.__update(__DucktapeMessage::PagesUpdated(backend::PagesData {
            pages: Vec::new(),
            blocks: vec![backend::PageBlock {
                id: "block-1".into(),
                parent: "page-1".into(),
                kind: "Todo".into(),
                text: "Canonical text".into(),
                pending: false,
                checked: true,
                prefix: String::new(),
                child_count: 0,
                mark_count: 0,
            }],
            active_page: "page-1".into(),
            active_page_title: "Page".into(),
            active_page_parent: String::new(),
            selected_block_id: "block-1".into(),
            selected_block_kind: "Todo".into(),
            selected_block_text: "Canonical text".into(),
            selected_block_checked: true,
            page_title_selected: false,
        }));

        assert_eq!(app.selected_block_id, "block-1");
        assert_eq!(app.selected_block_kind, "Todo");
        assert_eq!(app.block_edit_draft, "Canonical text");
        assert!(app.selected_block_checked);
    }

    #[test]
    fn page_blocks_can_be_created_but_not_converted_to_subpages() {
        let (app, _) = Ducktape::__boot();
        assert!(app.block_kinds.iter().any(|kind| kind == "Page"));
        assert!(!app.editable_block_kinds.iter().any(|kind| kind == "Page"));
    }

    #[test]
    fn page_navigation_ignores_the_previous_block_autosave_callback() {
        let (mut app, _) = Ducktape::__boot();
        app.loading = false;
        app.active_page = "old-page".into();
        app.block_autosave_generation = 4;

        let _ = app.__update(__DucktapeMessage::ChoosePage("next-page".into()));
        let navigation_generation = app.hydration_generation;
        assert_eq!(app.block_autosave_generation, 5);

        let _ = app.__update(__DucktapeMessage::BlockTextSaved(backend::AutosaveResult {
            generation: 4,
            written: true,
        }));
        assert_eq!(app.hydration_generation, navigation_generation);
        assert_eq!(app.sync_phase, "idle");
        assert!(app.loading);
    }

    #[test]
    fn block_edits_invalidate_an_older_workspace_refresh() {
        let (mut app, _) = Ducktape::__boot();
        app.loading = false;
        app.connected_rpc = "http://node".into();
        app.selected_block_id = "block-1".into();
        app.selected_block_kind = "Text".into();
        app.block_edit_draft = "old".into();
        app.hydration_generation = 3;
        app.sync_phase = "refreshing".into();

        let _ = app.__update(__DucktapeMessage::BlockTextChanged("new".into()));
        assert_eq!(app.hydration_generation, 4);
        assert_eq!(app.sync_phase, "idle");

        let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
            kind: "changed".into(),
            status: "Live".into(),
            height: 1,
        }));
        assert_eq!(app.hydration_generation, 5);
        assert_eq!(app.sync_phase, "refreshing");
        let _ = app.__update(__DucktapeMessage::BlockTextSaved(backend::AutosaveResult {
            generation: app.block_autosave_generation,
            written: true,
        }));
        assert_eq!(app.hydration_generation, 6);

        let _ = app.__update(__DucktapeMessage::WorkspaceRefreshed(
            backend::WorkspaceData {
                generation: 5,
                rpc: "http://node".into(),
                status: "stale".into(),
                height: 1,
                channels: Vec::new(),
                messages: Vec::new(),
                active_channel: String::new(),
                active_channel_name: String::new(),
                active_channel_archived: false,
                active_channel_members_only: false,
                active_channel_huddle_count: 0,
                channel_members: Vec::new(),
                pages: Vec::new(),
                blocks: vec![backend::PageBlock {
                    id: "block-1".into(),
                    parent: "page".into(),
                    kind: "Text".into(),
                    text: "old".into(),
                    pending: false,
                    checked: false,
                    prefix: String::new(),
                    child_count: 0,
                    mark_count: 0,
                }],
                active_page: "page".into(),
                active_page_title: "Page".into(),
                active_page_parent: String::new(),
            },
        ));
        assert_eq!(app.block_edit_draft, "new");
    }

    #[test]
    fn failed_optimistic_send_rolls_back_and_restores_the_draft() {
        let (mut app, _) = Ducktape::__boot();
        app.connected = true;
        app.loading = false;
        app.active_channel = "general".into();
        app.message_draft = "retry me".into();

        let _ = app.__update(__DucktapeMessage::SendMessageSubmit);
        let _ = app.__update(__DucktapeMessage::MutationFailed(backend::AppError {
            message: "rejected".into(),
            committed: false,
        }));

        assert_eq!(app.message_draft, "retry me");
        assert!(app.messages.is_empty());
        assert_eq!(app.error, "rejected");
        assert_eq!(app.mutation_phase, "idle");
    }

    #[test]
    fn failed_send_preserves_the_next_and_unsent_drafts() {
        let (mut app, _) = Ducktape::__boot();
        app.connected = true;
        app.loading = false;
        app.active_channel = "general".into();
        app.message_draft = "first".into();

        let _ = app.__update(__DucktapeMessage::SendMessageSubmit);
        app.message_draft = "second".into();
        let _ = app.__update(__DucktapeMessage::MutationFailed(backend::AppError {
            message: "rejected".into(),
            committed: false,
        }));

        assert_eq!(app.message_draft, "second");
        assert_eq!(app.failed_message_draft, "first");
        app.message_draft.clear();
        let _ = app.__update(__DucktapeMessage::RestoreFailedMessage);
        assert_eq!(app.message_draft, "first");
        assert!(app.failed_message_draft.is_empty());
    }

    #[test]
    fn committed_mutation_keeps_optimistic_state_until_refresh() {
        let (mut app, _) = Ducktape::__boot();
        app.connected = true;
        app.loading = false;
        app.connected_rpc = "http://node".into();
        app.active_channel = "general".into();
        app.message_draft = "committed once".into();

        let _ = app.__update(__DucktapeMessage::SendMessageSubmit);
        let _ = app.__update(__DucktapeMessage::MutationFailed(backend::AppError {
            message: "read failed after commit".into(),
            committed: true,
        }));

        assert!(app.message_draft.is_empty());
        assert_eq!(app.messages.len(), 1);
        assert!(app.messages[0].pending);
        assert_eq!(app.mutation_phase, "recovering");
        assert_eq!(app.sync_phase, "refreshing");
        assert_eq!(app.block_autosave_generation, 1);

        app.message_draft = "must wait".into();
        let _ = app.__update(__DucktapeMessage::SendMessageSubmit);
        assert_eq!(app.messages.len(), 1);
        assert_eq!(app.mutation_phase, "recovering");
    }

    #[test]
    fn committed_message_change_cannot_be_submitted_twice() {
        let (mut app, _) = Ducktape::__boot();
        app.connected_rpc = "http://node".into();
        app.active_channel = "general".into();
        app.selected_message_seq = 7;
        app.selected_message_rev = 2;
        app.message_action = "editing".into();
        app.message_edit_draft = "committed edit".into();
        app.mutation_phase = "message-edit".into();

        let _ = app.__update(__DucktapeMessage::MutationFailed(backend::AppError {
            message: "read failed after commit".into(),
            committed: true,
        }));

        assert_eq!(app.selected_message_seq, 0);
        assert_eq!(app.selected_message_rev, 0);
        assert_eq!(app.message_action, "toolbar");
        assert!(app.message_edit_draft.is_empty());
        assert_eq!(app.mutation_phase, "recovering");
    }

    #[test]
    fn failed_thread_reply_rolls_back_and_restores_the_draft() {
        let (mut app, _) = Ducktape::__boot();
        app.connected = true;
        app.loading = false;
        app.active_channel = "general".into();
        app.active_thread_seq = 1;
        app.reply_draft = "retry reply".into();

        let _ = app.__update(__DucktapeMessage::SendReplySubmit);
        assert_eq!(app.mutation_phase, "reply");
        assert!(app.reply_draft.is_empty());
        assert!(app.thread_messages[0].pending);

        let _ = app.__update(__DucktapeMessage::ReplyMutationFailed(backend::AppError {
            message: "rejected".into(),
            committed: false,
        }));
        assert_eq!(app.reply_draft, "retry reply");
        assert!(app.thread_messages.is_empty());
        assert_eq!(app.mutation_phase, "recovering");
        assert!(app.thread_loading);
    }

    #[test]
    fn committed_thread_reply_survives_a_failed_recovery_read() {
        let (mut app, _) = Ducktape::__boot();
        app.thread_generation = 4;
        app.thread_loading = true;
        app.mutation_phase = "recovering".into();
        app.thread_messages = backend::optimistic_message(Vec::new(), "committed reply".into());

        let _ = app.__update(__DucktapeMessage::ThreadFailed(backend::HydrationError {
            generation: 4,
            message: "read failed after commit".into(),
        }));

        assert!(!app.thread_loading);
        assert_eq!(app.thread_messages.len(), 1);
        assert!(app.thread_messages[0].pending);
    }

    #[test]
    fn failed_block_insert_rolls_back_and_restores_the_draft() {
        let (mut app, _) = Ducktape::__boot();
        app.connected = true;
        app.loading = false;
        app.active_page = "welcome".into();
        app.new_block_kind = "Heading 2".into();
        app.block_draft = "retry heading".into();

        let _ = app.__update(__DucktapeMessage::AddBlockSubmit);
        assert_eq!(app.mutation_phase, "block");
        assert!(app.block_draft.is_empty());
        assert_eq!(app.blocks[0].kind, "Heading 2");
        assert!(app.blocks[0].pending);

        let _ = app.__update(__DucktapeMessage::MutationFailed(backend::AppError {
            message: "rejected".into(),
            committed: false,
        }));
        assert_eq!(app.block_draft, "retry heading");
        assert!(app.blocks.is_empty());
        assert_eq!(app.mutation_phase, "idle");
    }
}
