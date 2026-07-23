ui_lang::include_app!("src/ui/app.ice");

mod backend;

fn main() -> iced::Result {
    Ducktape::run()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(seq: i64, body: &str, deleted: bool) -> backend::ChatMessage {
        backend::ChatMessage {
            id: format!("message-{seq}"),
            seq,
            author: "user".into(),
            meta: format!("#{seq}"),
            body: body.into(),
            blocks: backend::paragraph_blocks(body),
            pending: false,
            rev: 2,
            edited: false,
            deleted,
            reply_count: 0,
            thread_seq: 0,
            show_author: true,
            initial: "U".into(),
            avatar_r: 0.4,
            avatar_g: 0.4,
            avatar_b: 0.9,
            reactions: Vec::new(),
        }
    }

    fn compose(text: &str) -> iced::widget::text_editor::Content {
        iced::widget::text_editor::Content::with_text(text)
    }

    fn composer(app: &Ducktape) -> String {
        app.message_editor.text().trim().to_string()
    }

    fn reply_composer(app: &Ducktape) -> String {
        app.reply_editor.text().trim().to_string()
    }

    fn workspace(
        generation: i64,
        active_channel: &str,
        messages: Vec<backend::ChatMessage>,
        active_page: &str,
        blocks: Vec<backend::PageBlock>,
    ) -> backend::WorkspaceData {
        backend::WorkspaceData {
            generation,
            rpc: "http://node".into(),
            status: "Live".into(),
            height: 1,
            channels: Vec::new(),
            messages,
            active_channel: active_channel.into(),
            active_channel_name: active_channel.into(),
            active_channel_archived: false,
            active_channel_members_only: false,
            active_channel_huddle_count: 0,
            channel_members: Vec::new(),
            pages: Vec::new(),
            blocks,
            active_page: active_page.into(),
            active_page_title: active_page.into(),
            active_page_parent: String::new(),
        }
    }

    fn chat_data(active_channel: &str, messages: Vec<backend::ChatMessage>) -> backend::ChatData {
        backend::ChatData {
            channels: Vec::new(),
            messages,
            active_channel: active_channel.into(),
            active_channel_name: active_channel.into(),
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
        }
    }

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

        let _ = app.__update(__DucktapeMessage::__WorkspaceTabsHandleToggleConnection(
            "workspace-tabs".into(),
        ));
        assert!(app.__ice_component_workspacetabs["workspace-tabs"].connection_open);
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
        for scoped in [
            "channel_name_draft",
            "member_key_draft",
            "message_draft",
            "reply_draft",
        ] {
            assert!(refresh.contains(&format!(
                "{scoped} = retain_for_endpoint({scoped}, active_channel, next.active_channel)"
            )));
        }
        assert!(refresh.contains("selected_message_seq = refreshed_required_message_seq("));
        assert!(refresh.contains("failed_message_draft = remember_failed_draft("));
        assert!(lifecycle.contains("run live_events(connected_rpc) when connected"));
        assert!(!lifecycle.contains("every 1s"));
        assert!(lifecycle.contains("run refresh(connected_rpc"));
        assert!(lifecycle.contains("run refresh_live_thread(connected_rpc"));
        assert!(lifecycle.contains("parallel\n    run refresh_live_thread("));
        assert!(lifecycle.contains("active_page_title = next.active_page_title"));
        assert!(lifecycle.contains("block_edit_draft = refreshed_block_draft("));
        assert!(lifecycle.contains(
            "block_comment_draft = retain_selected_string(block_comment_draft, selected_block_id)"
        ));
        let live_comment_callbacks = lifecycle
            .split_once("on live_block_comments_refreshed(next)\n")
            .unwrap()
            .1
            .split_once("\non refresh_failed(cause)")
            .unwrap()
            .0;
        assert!(!live_comment_callbacks.contains("run refresh("));
    }

    #[test]
    fn context_destroying_page_handlers_recover_drafts() {
        let pages = include_str!("ui/handlers/pages.ice");
        for name in [
            "open_page_search_hit(page_id, block_id)",
            "choose_page(id)",
            "select_block(key, id, kind, text, checked, open_actions)",
            "clear_block_selection",
            "pages_mutated(next)",
        ] {
            let rest = pages.split_once(&format!("on {name}")).unwrap().1;
            let body = rest.split_once("\non ").map_or(rest, |(body, _)| body);
            assert!(body.contains("remember_orphaned_block_drafts("), "{name}");
            assert!(body.contains("remember_orphaned_comment_drafts("), "{name}");
        }
        let close_comments = pages
            .split_once("on close_block_comments\n")
            .unwrap()
            .1
            .split_once("\non ")
            .unwrap()
            .0;
        assert!(close_comments.contains("remember_orphaned_comment_drafts("));
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
    fn a_new_live_event_replaces_an_inflight_workspace_refresh() {
        let (mut app, _) = Ducktape::__boot();
        app.loading = false;
        app.hydration_generation = 10;

        let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
            kind: "changed".into(),
            status: "first".into(),
            height: 1,
        }));
        assert_eq!(app.hydration_generation, 11);
        assert_eq!(app.sync_phase, "refreshing");
        assert!(!app.live_dirty);

        let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
            kind: "changed".into(),
            status: "second".into(),
            height: 2,
        }));
        assert_eq!(app.hydration_generation, 12);
        assert_eq!(app.status, "second");
        assert!(!app.live_dirty);

        let _ = app.__update(__DucktapeMessage::WorkspaceRefreshed(
            backend::WorkspaceData {
                generation: 11,
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
                blocks: Vec::new(),
                active_page: String::new(),
                active_page_title: String::new(),
                active_page_parent: String::new(),
            },
        ));
        assert_eq!(app.status, "second");
        assert_eq!(app.sync_phase, "refreshing");

        let _ = app.__update(__DucktapeMessage::WorkspaceRefreshed(
            backend::WorkspaceData {
                generation: 12,
                rpc: "http://node".into(),
                status: "fresh".into(),
                height: 2,
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
        assert_eq!(app.status, "fresh");
        assert_eq!(app.sync_phase, "idle");
    }

    #[test]
    fn workspace_refresh_cannot_retarget_drafts_to_fallback_contexts() {
        let (mut app, _) = Ducktape::__boot();
        app.connected_rpc = "http://node".into();
        app.sync_phase = "refreshing".into();
        app.hydration_generation = 7;
        app.active_channel = "deleted-channel".into();
        app.message_draft = "channel draft".into();
        app.selected_message_seq = 7;
        app.selected_message_rev = 2;
        app.hovered_message_seq = 7;
        app.message_action = "editing".into();
        app.message_edit_draft = "message edit".into();
        app.channel_settings_open = true;
        app.channel_name_draft = "channel rename".into();
        app.member_key_draft = "member".into();
        app.active_thread_seq = 7;
        app.thread_generation = 4;
        app.thread_target_seq = 9;
        app.thread_messages = vec![message(7, "old thread", false)];
        app.thread_next_reply_offset = 4;
        app.thread_has_more = true;
        app.thread_loading = true;
        app.reply_draft = "thread reply".into();
        app.pending_reply = "pending thread reply".into();
        app.active_page = "deleted-page".into();
        app.block_insert_open = true;
        app.block_insert_after_id = "deleted-block".into();
        app.block_draft = "new block draft".into();

        let _ = app.__update(__DucktapeMessage::WorkspaceRefreshed(workspace(
            7,
            "fallback-channel",
            vec![message(7, "same sequence, other channel", false)],
            "fallback-page",
            Vec::new(),
        )));

        assert_eq!(app.active_channel, "fallback-channel");
        assert_eq!(app.selected_message_seq, 0);
        assert_eq!(app.selected_message_rev, 0);
        assert_eq!(app.hovered_message_seq, 0);
        assert_eq!(app.message_action, "toolbar");
        assert!(app.message_edit_draft.is_empty());
        assert!(!app.channel_settings_open);
        assert!(app.channel_name_draft.is_empty());
        assert!(app.member_key_draft.is_empty());
        assert_eq!(app.active_thread_seq, 0);
        assert_eq!(app.thread_generation, 5);
        assert_eq!(app.thread_target_seq, 0);
        assert!(app.thread_messages.is_empty());
        assert_eq!(app.thread_next_reply_offset, 0);
        assert!(!app.thread_has_more);
        assert!(!app.thread_loading);
        assert!(app.reply_draft.is_empty());
        assert!(app.pending_reply.is_empty());
        assert!(app.message_draft.is_empty());
        assert_eq!(app.failed_message_draft, "channel draft\nmessage edit");
        assert_eq!(app.failed_reply_draft, "thread reply");
        assert_eq!(app.active_page, "fallback-page");
        assert!(!app.block_insert_open);
        assert!(app.block_insert_after_id.is_empty());
        assert_eq!(app.block_draft, "new block draft");

        let _ = app.__update(__DucktapeMessage::ThreadLoaded(backend::ThreadLoadData {
            generation: 4,
            root_seq: 7,
            target_seq: 7,
            messages: vec![message(7, "stale fallback collision", false)],
            next_reply_offset: 1,
            has_more: true,
        }));
        assert_eq!(app.active_thread_seq, 0);
        assert!(app.thread_messages.is_empty());
    }

    #[test]
    fn unrelated_chat_mutation_preserves_open_editors_and_thread_state() {
        let (mut app, _) = Ducktape::__boot();
        app.active_channel = "general".into();
        app.selected_message_seq = 7;
        app.selected_message_rev = 2;
        app.message_action = "editing".into();
        app.message_edit_draft = "edit in progress".into();
        app.active_thread_seq = 9;
        app.thread_target_seq = 10;
        app.thread_messages = vec![message(9, "thread root", false)];
        app.thread_next_reply_offset = 3;
        app.thread_has_more = true;
        app.reply_editor = compose("reply in progress");
        app.message_editor = compose("next message");
        app.mutation_phase = "message".into();
        app.pending_message = "sent message".into();

        let _ = app.__update(__DucktapeMessage::ChatMutated(backend::ChatData {
            channels: Vec::new(),
            messages: vec![
                message(7, "message being edited", false),
                message(9, "thread root", false),
            ],
            active_channel: "general".into(),
            active_channel_name: "General".into(),
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

        assert_eq!(app.selected_message_seq, 7);
        assert_eq!(app.message_action, "editing");
        assert_eq!(app.message_edit_draft, "edit in progress");
        assert_eq!(app.active_thread_seq, 9);
        assert_eq!(app.thread_target_seq, 10);
        assert_eq!(app.thread_messages.len(), 1);
        assert_eq!(app.thread_next_reply_offset, 3);
        assert!(app.thread_has_more);
        assert_eq!(reply_composer(&app), "reply in progress");
        assert_eq!(composer(&app), "next message");
        assert_eq!(app.mutation_phase, "idle");
    }

    #[test]
    fn tombstoned_thread_root_closes_every_thread_bound_state() {
        let (mut app, _) = Ducktape::__boot();
        app.connected_rpc = "http://node".into();
        app.sync_phase = "refreshing".into();
        app.hydration_generation = 3;
        app.active_channel = "general".into();
        app.active_thread_seq = 9;
        app.thread_generation = 11;
        app.thread_target_seq = 10;
        app.thread_messages = vec![message(9, "thread root", false)];
        app.thread_next_reply_offset = 4;
        app.thread_has_more = true;
        app.thread_loading = true;
        app.reply_draft = "unsent reply".into();
        app.pending_reply = "pending reply".into();

        let _ = app.__update(__DucktapeMessage::WorkspaceRefreshed(workspace(
            3,
            "general",
            vec![message(9, "thread root", true)],
            "",
            Vec::new(),
        )));

        assert_eq!(app.active_thread_seq, 0);
        assert_eq!(app.thread_generation, 12);
        assert_eq!(app.thread_target_seq, 0);
        assert!(app.thread_messages.is_empty());
        assert_eq!(app.thread_next_reply_offset, 0);
        assert!(!app.thread_has_more);
        assert!(!app.thread_loading);
        assert!(app.reply_draft.is_empty());
        assert!(app.pending_reply.is_empty());
        assert_eq!(app.failed_reply_draft, "unsent reply");
    }

    #[test]
    fn unrelated_hydration_keeps_an_initial_thread_load_alive() {
        let (mut refresh, _) = Ducktape::__boot();
        refresh.connected_rpc = "http://node".into();
        refresh.active_channel = "general".into();
        refresh.loading = false;
        refresh.mutation_phase = "idle".into();
        refresh.thread_generation = 6;
        let _ = refresh.__update(__DucktapeMessage::OpenThreadFor(7));
        assert_eq!(refresh.active_thread_seq, 7);
        assert_eq!(refresh.thread_generation, 7);
        assert!(refresh.thread_loading);
        refresh.sync_phase = "refreshing".into();
        refresh.hydration_generation = 5;

        let _ = refresh.__update(__DucktapeMessage::WorkspaceRefreshed(workspace(
            5,
            "general",
            vec![message(7, "root", false)],
            "",
            Vec::new(),
        )));
        assert_eq!(refresh.active_thread_seq, 7);
        assert_eq!(refresh.thread_generation, 7);
        assert!(refresh.thread_loading);

        refresh.sync_phase = "refreshing".into();
        refresh.hydration_generation = 6;
        let _ = refresh.__update(__DucktapeMessage::WorkspaceRefreshed(workspace(
            6,
            "general",
            vec![message(7, "root", true)],
            "",
            Vec::new(),
        )));
        assert_eq!(refresh.active_thread_seq, 0);
        assert_eq!(refresh.thread_generation, 8);
        assert!(!refresh.thread_loading);
        let _ = refresh.__update(__DucktapeMessage::ThreadLoaded(backend::ThreadLoadData {
            generation: 7,
            root_seq: 7,
            target_seq: 7,
            messages: vec![message(7, "stale deleted root", false)],
            next_reply_offset: 1,
            has_more: true,
        }));
        assert_eq!(refresh.active_thread_seq, 0);
        assert!(refresh.thread_messages.is_empty());

        let (mut mutation, _) = Ducktape::__boot();
        mutation.active_channel = "general".into();
        mutation.loading = false;
        mutation.mutation_phase = "idle".into();
        mutation.thread_generation = 8;
        let _ = mutation.__update(__DucktapeMessage::OpenThreadFor(7));
        mutation.mutation_phase = "message".into();
        let _ = mutation.__update(__DucktapeMessage::ChatMutated(chat_data(
            "general",
            vec![message(7, "root", false)],
        )));
        assert_eq!(mutation.active_thread_seq, 7);
        assert_eq!(mutation.thread_generation, 9);
        assert!(mutation.thread_loading);

        mutation.mutation_phase = "channel".into();
        let _ = mutation.__update(__DucktapeMessage::ChatMutated(chat_data(
            "fallback",
            vec![message(7, "same sequence, other channel", false)],
        )));
        assert_eq!(mutation.thread_generation, 10);
        assert!(!mutation.thread_loading);

        let _ = mutation.__update(__DucktapeMessage::ThreadLoaded(backend::ThreadLoadData {
            generation: 9,
            root_seq: 7,
            target_seq: 7,
            messages: vec![message(7, "stale load", false)],
            next_reply_offset: 1,
            has_more: true,
        }));
        assert_eq!(mutation.active_thread_seq, 0);
        assert!(mutation.thread_messages.is_empty());
    }

    #[test]
    fn pages_mutation_keeps_the_canonical_selected_block() {
        let (mut app, _) = Ducktape::__boot();
        app.mutation_phase = "block-kind".into();
        app.selected_block_id = "block-1".into();
        app.selected_block_kind = "Text".into();
        app.block_edit_draft = "local".into();
        app.block_actions_open = true;

        let block = backend::PageBlock {
            key: 1,
            id: "block-1".into(),
            parent: "page-1".into(),
            kind: "Todo".into(),
            text: "canonical".into(),
            pending: false,
            checked: true,
            prefix: String::new(),
            child_count: 0,
            mark_count: 0,
        };
        let _ = app.__update(__DucktapeMessage::PagesMutated(backend::PagesData {
            pages: Vec::new(),
            blocks: vec![block],
            active_page: "page-1".into(),
            active_page_title: "Page".into(),
            active_page_parent: String::new(),
            selected_block_id: "block-1".into(),
            selected_block_kind: "Todo".into(),
            selected_block_text: "canonical".into(),
            selected_block_checked: true,
            page_title_selected: false,
        }));

        assert_eq!(app.selected_block_id, "block-1");
        assert_eq!(app.selected_block_kind, "Todo");
        assert_eq!(app.block_edit_draft, "canonical");
        assert!(app.selected_block_checked);
        assert!(!app.block_actions_open);
        assert_eq!(app.mutation_phase, "idle");
    }

    #[test]
    fn ready_events_and_stale_searches_do_not_rehydrate_navigation() {
        let (mut chat, _) = Ducktape::__boot();
        chat.loading = false;
        chat.chat_search_generation = 4;
        chat.chat_searching = true;
        let _ = chat.__update(__DucktapeMessage::ChooseChannel("next".into()));
        assert_eq!(chat.chat_search_generation, 5);
        assert!(!chat.chat_searching);
        let _ = chat.__update(__DucktapeMessage::ChatSearchLoaded(
            backend::ChatSearchData {
                generation: 4,
                hits: vec![backend::ChatSearchHit {
                    channel_id: "old".into(),
                    seq: 1,
                    root_seq: 1,
                    author: "user".into(),
                    text: "stale".into(),
                    meta: "#1".into(),
                }],
            },
        ));
        assert!(chat.chat_search_hits.is_empty());

        let (mut pages, _) = Ducktape::__boot();
        pages.loading = false;
        pages.page_search_generation = 8;
        pages.page_searching = true;
        let _ = pages.__update(__DucktapeMessage::ChoosePage("next".into()));
        assert_eq!(pages.page_search_generation, 9);
        assert!(!pages.page_searching);
        let _ = pages.__update(__DucktapeMessage::PageSearchLoaded(
            backend::PageSearchData {
                generation: 8,
                hits: vec![backend::PageSearchHit {
                    page_id: "old".into(),
                    block_id: "old-block".into(),
                    kind: "Text".into(),
                    text: "stale".into(),
                }],
            },
        ));
        assert!(pages.page_search_hits.is_empty());

        let (mut live, _) = Ducktape::__boot();
        live.loading = false;
        live.hydration_generation = 2;
        let _ = live.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
            kind: "ready".into(),
            status: "Live".into(),
            height: -1,
        }));
        assert_eq!(live.hydration_generation, 2);
        assert_eq!(live.sync_phase, "idle");
        assert!(!live.live_dirty);
    }

    #[test]
    fn optimistic_sends_are_independent_and_never_erase_the_next_draft() {
        let (mut app, _) = Ducktape::__boot();
        app.connected = true;
        app.loading = false;
        app.active_channel = "general".into();
        app.message_editor = compose("first");

        let _ = app.__update(__DucktapeMessage::SendMessageSubmit);
        let first_id = app.messages[0].id.clone();
        assert_eq!(app.mutation_phase, "idle");
        assert!(app.message_draft.is_empty());
        assert!(composer(&app).is_empty());
        assert_eq!(app.messages.len(), 1);
        assert!(app.messages[0].pending);

        app.message_editor = compose("second");
        let _ = app.__update(__DucktapeMessage::SendMessageSubmit);
        let second_id = app.messages[1].id.clone();
        assert_ne!(first_id, second_id);
        assert_eq!(app.messages.len(), 2);
        assert!(app.messages.iter().all(|message| message.pending));

        app.message_editor = compose("third");
        let _ = app.__update(__DucktapeMessage::MessageSent(backend::ChatSendResult {
            operation_id: first_id.clone(),
            channel_id: "general".into(),
            data: backend::ChatData {
                channels: Vec::new(),
                messages: vec![backend::ChatMessage {
                    id: first_id,
                    seq: 1,
                    author: "you".into(),
                    meta: "#1".into(),
                    body: "first".into(),
                    blocks: backend::paragraph_blocks("first"),
                    pending: false,
                    rev: 0,
                    edited: false,
                    deleted: false,
                    reply_count: 0,
                    thread_seq: 0,
                    show_author: true,
                    initial: "U".into(),
                    avatar_r: 0.4,
                    avatar_g: 0.4,
                    avatar_b: 0.9,
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
            },
        }));
        assert_eq!(composer(&app), "third");
        assert_eq!(app.mutation_phase, "idle");
        assert!(!app.messages[0].pending);
        assert_eq!(app.messages[1].id, second_id);
        assert!(app.messages[1].pending);

        let view = include_str!("ui/view.ice");
        assert!(view.contains("stack #message(message.id) width=fill"));
        assert!(!view.contains("#message(message.seq)"));
    }

    #[test]
    fn message_actions_require_explicit_intent() {
        let (mut app, _) = Ducktape::__boot();
        app.mutation_phase = "idle".into();

        let _ = app.__update(__DucktapeMessage::MessageEntered(7));
        assert_eq!(app.hovered_message_seq, 7);
        app.chat_pointer_y = 450.0;
        app.chat_height = 500.0;
        let _ = app.__update(__DucktapeMessage::OpenMessageActions(7, "hello".into(), 2));
        assert_eq!(app.message_menu_y, 260.0);
        let _ = app.__update(__DucktapeMessage::MessageExited(7));
        assert_eq!(app.hovered_message_seq, 0);
        assert_eq!(app.selected_message_seq, 7);
        assert_eq!(app.message_action, "more");
        let _ = app.__update(__DucktapeMessage::BeginMessageEdit(7, "hello".into(), 2));
        assert_eq!(app.message_action, "editing");
        let _ = app.__update(__DucktapeMessage::CancelMessageAction);
        assert_eq!(app.message_action, "toolbar");
        let _ = app.__update(__DucktapeMessage::OpenMessageReactions(
            7,
            "hello".into(),
            2,
        ));
        assert_eq!(app.message_action, "reactions");
        let _ = app.__update(__DucktapeMessage::CancelMessageAction);
        let _ = app.__update(__DucktapeMessage::ArmMessageDelete(7, "hello".into(), 2));
        assert_eq!(app.message_action, "delete");
        let _ = app.__update(__DucktapeMessage::OpenMessageActionsAccessibly(
            7,
            "hello".into(),
            2,
        ));
        assert_eq!(app.message_action, "more");
        assert_eq!(app.message_menu_y, 0.0);
    }

    #[test]
    fn message_action_toolbar_stays_compact_and_accessible() {
        let components = include_str!("ui/components/chat.ice");
        let toolbar = components
            .split_once("component MessageCard")
            .unwrap()
            .1
            .split_once("component ThreadMessageCard")
            .unwrap()
            .0;
        assert!(toolbar.contains("mouse enter=message_entered(message.seq)"));
        assert!(toolbar.contains("if !message.deleted && !message.pending && hovered"));
        assert!(toolbar.contains("if !message.deleted && !message.pending && !hovered"));
        assert!(toolbar.contains("-> open_message_actions_accessibly("));
        assert!(!toolbar.contains("hovered || selected"));
        assert_eq!(toolbar.matches("width=26.0 height=26.0").count(), 4);
        assert!(components.contains(
            "text message.author size=14.0 wrapping=none font=display @text-fg\n          text message.meta size=11.0 wrapping=none @text-muted\n          space width=fill"
        ));
        // Slack-style grouping: the colored avatar + author header only renders
        // for a run's first message; continuations keep the body aligned via a
        // gutter that matches the avatar's width.
        assert!(components.contains("if message.show_author\n      container width=36.0 height=36.0"));
        assert!(components.contains("if !message.show_author\n      space width=36.0"));
        // The per-author avatar tint rides in through a container-style extern
        // because `bg=` only takes static colors.
        assert!(components.contains(
            "style=avatar_style(message.avatar_r, message.avatar_g, message.avatar_b)"
        ));
        // Rich bodies render structured blocks, not one flattened string.
        assert!(components.contains("for block in message.blocks"));
        assert!(components.contains("if block.kind == \"code\""));
        assert!(components.contains("flex width=fill wrap"));
        assert!(toolbar.contains("bg=popover"));
        for label in ["Open thread", "Manage reactions", "More message actions"] {
            assert!(toolbar.contains(&format!("label=\"{label}\"")));
        }
        assert!(
            components.contains(
                "button label=\"Open thread\" padding=4.0 -> open_thread_for(message.seq)"
            )
        );

        let view = include_str!("ui/view.ice");
        let chat = view
            .split_once("    chat:")
            .unwrap()
            .1
            .split_once("    pages:")
            .unwrap()
            .0;
        assert!(
            chat.contains(
                "overlay when=(selected_message_seq > 0 && message_action != \"toolbar\")"
            )
        );
        assert!(chat.contains("dismiss=clear_message_selection backdrop=transparent"));
        assert!(chat.contains("mouse move=chat_pointer_moved"));
        assert!(chat.contains("sensor show=chat_resized resize=chat_resized"));
        assert!(chat.contains("float x=0.0 y=message_menu_y"));
        assert!(chat.contains(
            "stack width=fill height=fill\n                mouse move=chat_pointer_moved"
        ));
        let overlay_content = chat
            .split_once("                  content\n")
            .unwrap()
            .1
            .split_once("                  layer\n")
            .unwrap()
            .0;
        assert!(overlay_content.contains("space width=fill height=fill"));
        assert!(!overlay_content.contains("message_action =="));
        let more = view
            .split_once("message_action == \"more\"")
            .unwrap()
            .1
            .split_once("message_action == \"reactions\"")
            .unwrap()
            .0;
        assert!(more.contains("button \"React\""));
        assert!(more.contains("button \"Open thread\""));
        assert!(more.contains("button \"Edit\""));
        assert!(more.contains("button \"Delete\""));
        assert!(more.contains("button \"Close\""));

        let handlers = include_str!("ui/handlers/chat.ice");
        for focus in [
            "#workspace-tabs/message-action-focus",
            "#workspace-tabs/message-reaction-focus",
            "#workspace-tabs/message-delete-focus",
        ] {
            assert!(handlers.contains(focus));
        }
        for focus in [
            "#message-action-focus",
            "#message-reaction-focus",
            "#message-delete-focus",
        ] {
            assert!(chat.contains(&format!("input \"\" {focus}")));
        }
        assert_eq!(handlers.matches("task widget focus-next").count(), 7);
        assert!(!include_str!("ui/backend.ice").contains("task focus_next()"));
        let activate = handlers
            .split_once("on begin_message_edit(seq, body, rev)\n")
            .unwrap()
            .1
            .split_once("\non ")
            .unwrap()
            .0;
        assert!(activate.contains("task widget focus #workspace-tabs/message-edit"));
    }

    #[test]
    fn thread_messages_mirror_the_main_action_system() {
        let components = include_str!("ui/components/chat.ice");
        let card = components
            .split_once("component ThreadMessageCard")
            .unwrap()
            .1;
        assert!(card.contains(
            "mouse enter=thread_message_entered(message.seq) exit=thread_message_exited(message.seq)"
        ));
        assert!(card.contains(
            "-> open_thread_message_reactions(message.seq, message.body, message.rev)"
        ));
        assert!(card.contains(
            "-> open_thread_message_actions(message.seq, message.body, message.rev)"
        ));
        // No open-thread action from inside a thread you are already reading.
        assert!(!card.contains("open_thread_for"));

        let view = include_str!("ui/view.ice");
        let thread = view
            .split_once("if active_thread_seq > 0 && !channel_settings_open")
            .unwrap()
            .1
            .split_once("    pages:")
            .unwrap()
            .0;
        // A SECOND overlay, keyed on thread-scoped state, independent of the main one.
        assert!(thread.contains(
            "overlay when=(thread_selected_seq > 0 && thread_message_action != \"toolbar\")"
        ));
        assert!(thread.contains("dismiss=clear_thread_message_selection backdrop=transparent"));
        assert!(thread.contains("float x=0.0 y=thread_menu_y"));
        assert!(thread.contains("mouse move=thread_pointer_moved"));
        assert!(thread.contains("sensor show=thread_resized resize=thread_resized"));
        // The picker reuses the seq-targeted reaction mutations against the thread selection.
        assert!(thread.contains("-> add_reaction_at(thread_selected_seq, \"👍\")"));
        assert!(thread.contains("-> remove_reaction_at(thread_selected_seq, reaction.emoji)"));
        // More-menu omits Open thread (already inside the thread).
        let more = thread
            .split_once("thread_message_action == \"more\"")
            .unwrap()
            .1
            .split_once("thread_message_action == \"reactions\"")
            .unwrap()
            .0;
        for label in ["\"React\"", "\"Edit\"", "\"Delete\"", "\"Close\""] {
            assert!(more.contains(&format!("button {label}")), "{label}");
        }
        assert!(!more.contains("Open thread"));

        let handlers = include_str!("ui/handlers/chat.ice");
        for name in [
            "on open_thread_message_actions(seq, body, rev)",
            "on open_thread_message_reactions(seq, body, rev)",
            "on begin_thread_message_edit(seq, body, rev)",
            "on arm_thread_message_delete(seq, body, rev)",
            "on clear_thread_message_selection",
            "on edit_thread_message_submit",
            "on delete_thread_message_submit",
        ] {
            assert!(handlers.contains(name), "{name}");
        }
        // Thread edit/delete target the thread selection, never the main one.
        let edit = handlers
            .split_once("on edit_thread_message_submit\n")
            .unwrap()
            .1
            .split_once("\non ")
            .unwrap()
            .0;
        assert!(edit.contains(
            "edit_message(connected_rpc, password, active_channel, thread_selected_seq, thread_selected_rev, trim(thread_edit_draft))"
        ));
        let delete = handlers
            .split_once("on delete_thread_message_submit\n")
            .unwrap()
            .1
            .split_once("\non ")
            .unwrap()
            .0;
        assert!(delete.contains(
            "delete_message(connected_rpc, password, active_channel, thread_selected_seq)"
        ));
    }

    #[test]
    fn thread_action_state_is_independent_of_the_main_message_menu() {
        let (mut app, _) = Ducktape::__boot();
        app.mutation_phase = "idle".into();
        app.active_channel = "general".into();
        app.active_thread_seq = 1;

        // Opening a thread action must not touch the main message menu.
        app.thread_pointer_y = 400.0;
        app.thread_height = 500.0;
        let _ = app.__update(__DucktapeMessage::OpenThreadMessageActions(2, "reply".into(), 3));
        assert_eq!(app.thread_selected_seq, 2);
        assert_eq!(app.thread_message_action, "more");
        assert_eq!(app.thread_menu_y, 210.0);
        assert_eq!(app.selected_message_seq, 0);
        assert_eq!(app.message_action, "toolbar");

        // And a main message action must not touch the thread menu.
        let _ = app.__update(__DucktapeMessage::OpenMessageActions(5, "root".into(), 1));
        assert_eq!(app.selected_message_seq, 5);
        assert_eq!(app.message_action, "more");
        assert_eq!(app.thread_selected_seq, 2);
        assert_eq!(app.thread_message_action, "more");

        let _ = app.__update(__DucktapeMessage::ClearThreadMessageSelection);
        assert_eq!(app.thread_selected_seq, 0);
        assert_eq!(app.thread_message_action, "toolbar");
        assert_eq!(app.selected_message_seq, 5);
        assert_eq!(app.message_action, "more");
    }

    #[test]
    fn opening_another_thread_invalidates_the_pending_thread() {
        let (mut app, _) = Ducktape::__boot();
        app.mutation_phase = "idle".into();
        app.active_channel = "general".into();
        app.selected_message_seq = 1;
        app.thread_generation = 4;
        app.thread_loading = true;
        app.active_thread_seq = 1;
        app.thread_messages =
            backend::optimistic_message(Vec::new(), "old thread".into(), "pending-old".into());
        app.reply_editor = compose("old reply");

        let _ = app.__update(__DucktapeMessage::OpenThreadFor(2));
        assert_eq!(app.thread_generation, 5);
        assert!(app.thread_loading);
        assert_eq!(app.active_thread_seq, 2);
        assert!(app.thread_messages.is_empty());
        assert!(reply_composer(&app).is_empty());

        let _ = app.__update(__DucktapeMessage::ThreadLoaded(backend::ThreadLoadData {
            generation: 4,
            root_seq: 1,
            target_seq: 0,
            messages: Vec::new(),
            next_reply_offset: 0,
            has_more: false,
        }));
        assert_eq!(app.active_thread_seq, 2);
    }

    #[test]
    fn thread_pagination_preserves_multiple_pending_replies() {
        let message = |seq: i64, thread_seq: i64, body: &str| backend::ChatMessage {
            id: format!("message-{seq}"),
            seq,
            author: "user".into(),
            meta: format!("#{seq}"),
            body: body.into(),
            blocks: backend::paragraph_blocks(body),
            pending: false,
            rev: 0,
            edited: false,
            deleted: false,
            reply_count: 0,
            thread_seq,
            show_author: true,
            initial: "U".into(),
            avatar_r: 0.4,
            avatar_g: 0.4,
            avatar_b: 0.9,
            reactions: Vec::new(),
        };
        let (mut app, _) = Ducktape::__boot();
        app.active_thread_seq = 1;
        app.thread_generation = 7;
        app.thread_loading = true;
        app.thread_messages = backend::optimistic_message(
            backend::optimistic_message(
                vec![message(1, 0, "root"), message(2, 1, "first")],
                "pending first".into(),
                "pending-first".into(),
            ),
            "pending second".into(),
            "pending-second".into(),
        );

        let _ = app.__update(__DucktapeMessage::ThreadPageLoaded(
            backend::ThreadPageData {
                generation: 7,
                messages: vec![message(3, 1, "second")],
                next_reply_offset: 2,
                has_more: false,
            },
        ));
        assert_eq!(app.thread_messages.len(), 5);
        assert_eq!(app.thread_messages[1].body, "first");
        assert_eq!(app.thread_next_reply_offset, 2);
        assert!(
            app.thread_messages
                .iter()
                .any(|message| { message.pending && message.id == "pending-first" })
        );
        assert!(
            app.thread_messages
                .iter()
                .any(|message| { message.pending && message.id == "pending-second" })
        );
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
        app.reply_editor = compose("node a reply");
        app.selected_block_id = "same-id".into();
        app.block_edit_draft = "node a block".into();
        app.block_comments_open = true;
        app.block_comments_target = "same-id".into();
        app.block_comment_draft = "node a comment".into();
        app.message_editor = compose("node a message");
        app.page_search_draft = "node a search".into();

        let _ = app.__update(__DucktapeMessage::Reconnect);

        assert_eq!(app.connected_rpc, "http://node-b");
        assert!(app.password.is_empty());
        assert_eq!(app.selected_message_seq, 0);
        assert_eq!(app.message_action, "toolbar");
        assert!(app.message_edit_draft.is_empty());
        assert_eq!(app.active_thread_seq, 0);
        assert!(reply_composer(&app).is_empty());
        assert!(app.selected_block_id.is_empty());
        assert!(app.block_edit_draft.is_empty());
        assert!(!app.block_comments_open);
        assert!(app.block_comments_target.is_empty());
        assert!(app.block_comment_draft.is_empty());
        assert!(app.message_draft.is_empty());
        assert!(composer(&app).is_empty());
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
        app.message_editor = compose("next message");
        app.failed_message_draft = "unsent message".into();

        let _ = app.__update(__DucktapeMessage::Reconnect);

        assert_eq!(app.rpc, "http://node-a");
        assert_eq!(app.connected_rpc, "http://node-a");
        assert_eq!(composer(&app), "next message");
        assert_eq!(app.failed_message_draft, "unsent message");
    }

    #[test]
    fn page_search_load_selects_the_canonical_block() {
        let (mut app, _) = Ducktape::__boot();
        let _ = app.__update(__DucktapeMessage::PagesUpdated(backend::PagesData {
            pages: Vec::new(),
            blocks: vec![backend::PageBlock {
                key: 0,
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

        let view = include_str!("ui/view.ice");
        assert!(view.contains("if !block.pending && block.kind == \"Page\""));
        assert!(view.contains(
            "button label=block.kind description=block.text width=fill padding=5.0 -> choose_page(block.id)"
        ));
        assert!(view.contains(
            "if !block.pending && block.kind != \"Page\" && block.id == selected_block_id"
        ));
    }

    #[test]
    fn page_title_and_block_actions_use_native_focus_and_overlay_paths() {
        let components = include_str!("ui/components/pages.ice");
        let handlers = include_str!("ui/handlers/pages.ice");
        let view = include_str!("ui/view.ice");

        assert!(components.contains("task widget focus #title-input"));
        assert!(!components.contains("defer_focus"));
        assert!(!handlers.contains("focus_page_title"));
        assert!(!include_str!("ui/backend.ice").contains("defer_focus"));
        assert!(view.contains("mouse move=pages_pointer_moved"));
        assert!(view.contains("overlay when=(connected && !empty(active_page)"));
        assert!(view.contains("dismiss=close_block_actions backdrop=transparent"));
        assert!(view.contains("float x=(block_menu_x + 10.0)"));
        assert!(!view.contains("pin x=(block_menu_x"));
        assert!(view.contains("BlockActionsMenu block_id=selected_block_id"));
        assert!(view.matches("button \"Insert divider\"").count() == 2);

        assert!(components.contains("text block.prefix size=14.0"));
        assert!(components.contains(
            "component InlineBlockInsert(kind:str, kinds:[str], disabled:bool, prefix:str)"
        ));
        assert!(view.contains("prefix=block.prefix #block-insert-row"));
        assert!(view.contains(
            "container width=fill padding-left=56.0\n                    PageTitleEditor"
        ));
    }

    #[test]
    fn macos_shell_uses_layered_system_materials() {
        let ui = concat!(
            include_str!("ui/app.ice"),
            include_str!("ui/backend.ice"),
            include_str!("ui/state.ice"),
            include_str!("ui/theme.ice"),
            include_str!("ui/view.ice"),
            include_str!("ui/components/shell.ice"),
            include_str!("ui/components/chat.ice"),
            include_str!("ui/components/pages.ice"),
            include_str!("ui/handlers/lifecycle.ice"),
            include_str!("ui/handlers/chat.ice"),
            include_str!("ui/handlers/pages.ice"),
        );
        for gradient in ["linear(", "radial(", "conic("] {
            assert!(!ui.contains(gradient), "{gradient}");
        }

        let app = include_str!("ui/app.ice");
        assert!(app.contains("transparent true"));
        assert!(app.contains("titlebar-transparent true"));
        assert!(app.contains("fullsize-content-view true"));
        assert!(app.contains("font \"../../assets/InterVariable.ttf\""));

        let theme = include_str!("ui/theme.ice");
        assert!(theme.contains("font ui family=\"Inter\" weight=normal"));
        for material in [
            "bg #0b0b10ec",
            "surface #17171fe8",
            "popover #20202be8",
            "sidebar #101016ef",
            "elevated #262631e8",
        ] {
            assert!(theme.contains(material), "{material}");
        }
        // The redesign introduces an indigo accent for life (avatars, selection,
        // primary actions) while staying a layered dark system material set.
        for accent in ["primary #6f6cf6", "primaryhi #8a88ff"] {
            assert!(theme.contains(accent), "{accent}");
        }

        let shell = include_str!("ui/components/shell.ice");
        assert!(shell.contains(
            "container width=sidebar_width height=fill padding=12.0 padding-top=38.0 bg=sidebar"
        ));
        // the rail width is drag-resizable via the divider resize-handle.
        assert!(shell.contains("resize-handle drag=sidebar_dragged cursor=resize-horizontal"));

        let view = include_str!("ui/view.ice");
        assert!(view.contains("container width=fill padding=6.0 bg=transparent border=white/11"));
        assert!(view.contains("if active_thread_seq > 0 && !channel_settings_open"));
        assert!(view.contains("container width=fill padding=5.0 bg=transparent border=white/12"));
    }

    #[test]
    fn compact_controls_share_a_single_geometry_and_type_scale() {
        let view = include_str!("ui/view.ice");
        assert!(view.contains("padding=6.2 text-size=13.0 line-height=1.2"));
        assert!(view.contains(
            "min-height=44.0 max-height=150.0 size=14.0 line-height=1.3 padding=6.6 wrapping=word"
        ));
        assert!(view.contains("button \"Send\" disabled="));
        assert!(view.contains("height=30.0 padding=7.0 -> send_message_submit"));
        assert!(
            view.matches("container width=fill height=fill align-x=center align-y=center")
                .count()
                >= 10
        );
        for line in view
            .lines()
            .filter(|line| line.trim_start().starts_with("input "))
        {
            assert!(!line.contains(" height="), "{line}");
        }

        let components = concat!(
            include_str!("ui/components/shell.ice"),
            include_str!("ui/components/chat.ice"),
            include_str!("ui/components/pages.ice"),
        );
        assert!(components.contains("row width=fill height=fill spacing=9.0 align=center"));
        assert!(components.contains(
            "button label=\"Insert block below\" disabled=disabled width=28.0 height=28.0 padding=0.0"
        ));
        for line in view.lines().chain(components.lines()).filter(|line| {
            [
                "button \"+\" label",
                "button \"×\" label",
                "button \"…\" label",
            ]
            .iter()
            .any(|needle| line.contains(needle))
        }) {
            assert!(line.contains("width="), "{line}");
            assert!(line.contains("height="), "{line}");
        }
        for obsolete_size in ["size=10.0", "size=12.0", "text-size=12.0"] {
            assert!(!view.contains(obsolete_size), "{obsolete_size}");
            assert!(!components.contains(obsolete_size), "{obsolete_size}");
        }
    }

    #[test]
    fn composer_enter_sends_and_shift_enter_inserts_a_newline() {
        use iced::keyboard::key::{Named, NativeCode, Physical};
        use iced::keyboard::{Key, Modifiers};
        use iced::widget::text_editor::{Binding, KeyPress, Status};

        fn press(key: Key, modifiers: Modifiers) -> KeyPress {
            KeyPress {
                key: key.clone(),
                modified_key: key,
                physical_key: Physical::Unidentified(NativeCode::Unidentified),
                modifiers,
                text: None,
                status: Status::Focused { is_hovered: false },
            }
        }

        let enter = Key::Named(Named::Enter);
        // Plain Enter raises the custom send command routed to send_message_submit.
        assert_eq!(
            backend::composer_keys(press(enter.clone(), Modifiers::empty())),
            Some(Binding::Custom(backend::ComposerCmd)),
        );
        // Shift+Enter keeps iced's native newline insertion — never a send.
        assert_eq!(
            backend::composer_keys(press(enter, Modifiers::SHIFT)),
            Some(Binding::Enter),
        );
        // Any other key passes through to its native binding, not a send.
        let passthrough =
            backend::composer_keys(press(Key::Named(Named::ArrowLeft), Modifiers::empty()));
        assert!(!matches!(passthrough, Some(Binding::Custom(_))));
    }

    #[test]
    fn block_comments_use_a_floating_side_panel() {
        let view = include_str!("ui/view.ice");
        let pages = view.split_once("    pages:").unwrap().1;
        assert!(pages.contains("block_comments_open"));
        assert!(
            pages
                .contains("overlay when=(connected && !empty(active_page) && block_comments_open)")
        );
        assert!(pages.contains("dismiss=close_block_comments backdrop=transparent"));
        assert!(pages.contains("align-x=end align-y=start"));
        assert!(pages.contains("width=300.0 height=380.0"));
        assert!(pages.contains("bg=popover"));
        assert!(pages.contains("#block-comment(scope_key(connected_rpc, selected_block_id))"));
        assert!(!pages.contains("button \"Save\""));
        assert!(!pages.contains("Saving"));

        let components = include_str!("ui/components/pages.ice");
        assert!(components.contains("label=\"Comments\""));
        assert!(components.contains("-> open_block_comments"));

        let handlers = include_str!("ui/handlers/pages.ice");
        assert!(handlers.contains("on post_block_comment_submit"));
        assert!(handlers.contains(
            "run post_block_comment(connected_rpc, password, block_comments_target, active_block_comment_thread"
        ));
    }

    #[test]
    fn selecting_another_block_discards_a_stale_comment_page() {
        let (mut app, _) = Ducktape::__boot();
        app.mutation_phase = "idle".into();
        let _ = app.__update(__DucktapeMessage::SelectBlock(
            0,
            "block-a".into(),
            "Text".into(),
            "A".into(),
            false,
            false,
        ));
        let _ = app.__update(__DucktapeMessage::OpenBlockComments);
        let stale_generation = app.block_comments_generation;

        let _ = app.__update(__DucktapeMessage::SelectBlock(
            1,
            "block-b".into(),
            "Text".into(),
            "B".into(),
            false,
            false,
        ));
        let _ = app.__update(__DucktapeMessage::BlockThreadsLoaded(
            backend::BlockThreadListData {
                generation: stale_generation,
                target: "block-a".into(),
                from: 0,
                threads: vec![backend::PageCommentThread {
                    id: "thread-a".into(),
                    author: "Alice".into(),
                    meta: "1".into(),
                    resolved: false,
                    comment_count: 1,
                }],
                total: 1,
                next_from: 0,
                has_more: false,
            },
        ));

        assert_eq!(app.selected_block_id, "block-b");
        assert!(!app.block_comments_open);
        assert!(app.block_comment_threads.is_empty());
    }

    #[test]
    fn comment_pages_merge_by_identity_and_ordinal() {
        let thread = |id: &str, count: i64| backend::PageCommentThread {
            id: id.into(),
            author: "user".into(),
            meta: count.to_string(),
            resolved: false,
            comment_count: count,
        };
        let comment = |ordinal, text: &str| backend::PageComment {
            id: format!("comment-{ordinal}"),
            ordinal,
            author: "user".into(),
            meta: format!("#{ordinal}"),
            text: text.into(),
        };

        let threads = backend::append_page_comment_threads(
            vec![thread("b", 1), thread("a", 1)],
            vec![thread("b", 2), thread("c", 1)],
        );
        assert_eq!(
            threads
                .iter()
                .map(|thread| thread.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
        assert_eq!(threads[1].comment_count, 2);

        let comments = backend::append_page_comments(
            vec![comment(1, "first"), comment(3, "old")],
            vec![comment(2, "second"), comment(3, "new")],
        );
        assert_eq!(
            comments
                .iter()
                .map(|comment| comment.ordinal)
                .collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert_eq!(comments[2].text, "new");
    }

    #[test]
    fn live_comment_refresh_updates_threads_without_touching_the_draft() {
        let (mut app, _) = Ducktape::__boot();
        app.connected = true;
        app.loading = false;
        app.mutation_phase = "idle".into();
        app.sync_phase = "idle".into();
        app.selected_block_id = "block-1".into();
        app.block_comments_open = true;
        app.block_comments_target = "block-1".into();
        app.block_comment_draft = "draft stays".into();
        app.block_comment_threads_has_more = true;
        app.active_block_comment_thread = "deleted-thread".into();
        app.block_thread_comments = vec![backend::PageComment {
            id: "stale-comment".into(),
            ordinal: 1,
            author: "user".into(),
            meta: "#1".into(),
            text: "stale".into(),
        }];

        let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
            kind: "changed".into(),
            status: "Live".into(),
            height: 8,
        }));
        let workspace_generation = app.hydration_generation;
        let stale_generation = app.block_comments_generation;
        assert_eq!(app.sync_phase, "refreshing");
        let _ = app.__update(__DucktapeMessage::LoadMoreBlockThreads);
        assert_ne!(app.block_comments_generation, stale_generation);

        let _ = app.__update(__DucktapeMessage::LiveBlockCommentsRefreshed(
            backend::BlockCommentsRefreshData {
                generation: stale_generation,
                target: "block-1".into(),
                threads: Vec::new(),
                total: 0,
                threads_next_from: 0,
                threads_has_more: false,
                thread_id: String::new(),
                comments: Vec::new(),
                comments_next_from: 0,
                comments_has_more: false,
            },
        ));
        assert_eq!(app.sync_phase, "refreshing");

        let _ = app.__update(__DucktapeMessage::WorkspaceRefreshed(
            backend::WorkspaceData {
                generation: workspace_generation,
                rpc: "http://node".into(),
                status: "Live".into(),
                height: 8,
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
                    key: 0,
                    id: "block-1".into(),
                    parent: "page".into(),
                    kind: "Text".into(),
                    text: "block".into(),
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
        assert_eq!(app.sync_phase, "idle");
        let generation = app.block_comments_generation;

        let _ = app.__update(__DucktapeMessage::LiveBlockCommentsRefreshed(
            backend::BlockCommentsRefreshData {
                generation,
                target: "block-1".into(),
                threads: vec![backend::PageCommentThread {
                    id: "thread-1".into(),
                    author: "user".into(),
                    meta: "1".into(),
                    resolved: false,
                    comment_count: 1,
                }],
                total: 3,
                threads_next_from: 0,
                threads_has_more: false,
                thread_id: String::new(),
                comments: Vec::new(),
                comments_next_from: 0,
                comments_has_more: false,
            },
        ));

        assert_eq!(app.block_comment_thread_total, 3);
        assert_eq!(app.block_comment_draft, "draft stays");
        assert!(app.active_block_comment_thread.is_empty());
        assert!(app.block_thread_comments.is_empty());
        assert!(!app.block_comment_threads_loading);
    }

    #[test]
    fn a_live_event_waits_for_thread_pagination_then_refreshes() {
        let (mut app, _) = Ducktape::__boot();
        app.loading = false;
        app.active_channel = "general".into();
        app.active_thread_seq = 7;
        app.thread_generation = 4;
        app.thread_loading = true;
        app.hydration_generation = 9;

        let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
            kind: "changed".into(),
            status: "Live".into(),
            height: 10,
        }));
        assert!(app.live_dirty);
        assert_eq!(app.hydration_generation, 9);

        let _ = app.__update(__DucktapeMessage::ThreadPageLoaded(
            backend::ThreadPageData {
                generation: 4,
                messages: Vec::new(),
                next_reply_offset: 0,
                has_more: false,
            },
        ));
        assert!(!app.live_dirty);
        assert!(!app.thread_loading);
        assert_eq!(app.hydration_generation, 10);
        assert_eq!(app.sync_phase, "refreshing");
    }

    #[test]
    fn block_comment_recovery_always_unlocks_mutations() {
        let (mut failed, _) = Ducktape::__boot();
        failed.block_comments_open = true;
        failed.block_comments_generation = 7;
        failed.block_comment_threads_loading = true;
        failed.mutation_phase = "recovering".into();
        let _ = failed.__update(__DucktapeMessage::BlockThreadsRecoveryFailed(
            backend::HydrationError {
                generation: 7,
                message: "recovery read failed".into(),
            },
        ));
        assert_eq!(failed.mutation_phase, "idle");
        assert!(!failed.block_comment_threads_loading);

        let (mut recovered, _) = Ducktape::__boot();
        recovered.block_comments_open = true;
        recovered.block_comments_target = "block-1".into();
        recovered.selected_block_id = "block-1".into();
        recovered.block_comments_generation = 8;
        recovered.block_comment_threads_loading = true;
        recovered.mutation_phase = "recovering".into();
        recovered.error = "write result was uncertain".into();
        let _ = recovered.__update(__DucktapeMessage::BlockThreadsRecovered(
            backend::BlockThreadListData {
                generation: 8,
                target: "block-1".into(),
                from: 0,
                threads: Vec::new(),
                total: 0,
                next_from: 0,
                has_more: false,
            },
        ));
        assert_eq!(recovered.mutation_phase, "idle");
        assert!(recovered.error.is_empty());
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
                    key: 0,
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
    fn remote_block_deletion_recovers_local_drafts_and_closes_the_editor() {
        let (mut app, _) = Ducktape::__boot();
        app.loading = false;
        app.connected_rpc = "http://node".into();
        app.hydration_generation = 4;
        app.sync_phase = "refreshing".into();
        app.selected_block_id = "deleted".into();
        app.selected_block_kind = "Text".into();
        app.block_edit_draft = "unfinished block".into();
        app.block_autosave_status = "error".into();
        app.block_autosave_generation = 8;
        app.block_comments_open = true;
        app.block_comments_target = "deleted".into();
        app.block_comment_draft = "unfinished comment".into();
        app.active_page = "page".into();
        app.page_delete_armed = true;
        app.page_title_selected = true;

        let _ = app.__update(__DucktapeMessage::WorkspaceRefreshed(
            backend::WorkspaceData {
                generation: 4,
                rpc: "http://node".into(),
                status: "Live".into(),
                height: 9,
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
                active_page_title: "Page".into(),
                active_page_parent: String::new(),
            },
        ));

        assert_eq!(app.orphaned_block_drafts, ["unfinished block"]);
        assert_eq!(app.orphaned_comment_drafts, ["unfinished comment"]);
        assert!(app.selected_block_id.is_empty());
        assert!(app.block_edit_draft.is_empty());
        assert!(app.block_comment_draft.is_empty());
        assert!(!app.block_comments_open);
        assert_eq!(app.block_autosave_generation, 9);
        assert!(app.page_delete_armed);
        assert!(app.page_title_selected);

        let _ = app.__update(__DucktapeMessage::UseOrphanedBlockDraft(
            "unfinished block".into(),
        ));
        assert_eq!(app.block_draft, "unfinished block");
        assert!(app.orphaned_block_drafts.is_empty());
    }

    #[test]
    fn live_thread_refresh_preserves_the_reply_draft_and_rejects_stale_results() {
        let (mut app, _) = Ducktape::__boot();
        app.active_channel = "general".into();
        app.active_thread_seq = 7;
        app.thread_target_seq = 9;
        app.live_thread_generation = 3;
        app.reply_editor = compose("typing");
        app.thread_messages = backend::optimistic_message(
            backend::optimistic_message(Vec::new(), "pending first".into(), "pending-first".into()),
            "pending second".into(),
            "pending-second".into(),
        );

        let _ = app.__update(__DucktapeMessage::LiveThreadRefreshed(
            backend::LiveThreadData {
                generation: 3,
                channel_id: "other".into(),
                root_seq: 7,
                target_seq: 0,
                messages: Vec::new(),
                next_reply_offset: 99,
                has_more: true,
            },
        ));
        assert_eq!(app.thread_next_reply_offset, 0);

        let _ = app.__update(__DucktapeMessage::LiveThreadRefreshed(
            backend::LiveThreadData {
                generation: 3,
                channel_id: "general".into(),
                root_seq: 7,
                target_seq: 0,
                messages: Vec::new(),
                next_reply_offset: 5,
                has_more: true,
            },
        ));
        assert_eq!(reply_composer(&app), "typing");
        assert_eq!(app.thread_target_seq, 0);
        assert_eq!(app.thread_next_reply_offset, 5);
        assert!(app.thread_has_more);
        assert!(
            app.thread_messages
                .iter()
                .any(|message| { message.pending && message.id == "pending-first" })
        );
        assert!(
            app.thread_messages
                .iter()
                .any(|message| { message.pending && message.id == "pending-second" })
        );

        let _ = app.__update(__DucktapeMessage::CloseThread);
        let _ = app.__update(__DucktapeMessage::LiveThreadRefreshed(
            backend::LiveThreadData {
                generation: 3,
                channel_id: "general".into(),
                root_seq: 7,
                target_seq: 9,
                messages: Vec::new(),
                next_reply_offset: 99,
                has_more: true,
            },
        ));
        assert_eq!(app.thread_next_reply_offset, 0);
        assert!(!app.thread_has_more);
    }

    #[test]
    fn reconnect_recovers_active_drafts_for_the_same_endpoint() {
        let (mut app, _) = Ducktape::__boot();
        app.loading = false;
        app.rpc = "http://node".into();
        app.connected_rpc = "http://node".into();
        app.selected_block_id = "block".into();
        app.block_edit_draft = "unfinished block".into();
        app.block_autosave_status = "saving".into();
        app.block_comment_draft = "unfinished comment".into();

        let _ = app.__update(__DucktapeMessage::Reconnect);

        assert_eq!(app.orphaned_block_drafts, ["unfinished block"]);
        assert_eq!(app.orphaned_comment_drafts, ["unfinished comment"]);
    }

    #[test]
    fn page_and_block_context_changes_recover_local_drafts() {
        let (mut app, _) = Ducktape::__boot();
        app.loading = false;
        app.connected_rpc = "http://draft-context-test".into();
        app.selected_block_id = "block-a".into();
        app.selected_block_kind = "Text".into();
        app.block_edit_draft = "failed edit".into();
        app.block_autosave_status = "error".into();
        app.block_comment_draft = "comment a".into();

        let _ = app.__update(__DucktapeMessage::SelectBlock(
            0,
            "block-b".into(),
            "Text".into(),
            "canonical b".into(),
            false,
            false,
        ));
        assert_eq!(app.orphaned_block_drafts, ["failed edit"]);
        assert_eq!(app.orphaned_comment_drafts, ["comment a"]);
        assert_eq!(app.block_edit_draft, "canonical b");

        app.block_comment_draft = "comment b".into();
        let _ = app.__update(__DucktapeMessage::CloseBlockComments);
        assert_eq!(app.orphaned_comment_drafts, ["comment a", "comment b"]);

        app.block_edit_draft = "saving b".into();
        app.block_autosave_status = "saving".into();
        let _ = app.__update(__DucktapeMessage::ChoosePage("next".into()));
        assert_eq!(app.orphaned_block_drafts, ["failed edit", "saving b"]);
        assert!(app.block_edit_draft.is_empty());
        assert!(app.selected_block_id.is_empty());
    }

    #[test]
    fn failed_optimistic_send_rolls_back_and_restores_the_draft() {
        let (mut app, _) = Ducktape::__boot();
        app.connected = true;
        app.loading = false;
        app.active_channel = "general".into();
        app.message_editor = compose("retry me");

        let _ = app.__update(__DucktapeMessage::SendMessageSubmit);
        let operation_id = app.messages[0].id.clone();
        let _ = app.__update(__DucktapeMessage::MessageSendFailed(
            backend::OptimisticMutationError {
                message: "rejected".into(),
                committed: false,
                operation_id,
                scope_id: "general".into(),
                body: "retry me".into(),
            },
        ));

        assert_eq!(composer(&app), "retry me");
        assert_eq!(app.message_draft, "retry me");
        assert!(app.failed_message_draft.is_empty());
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
        app.message_editor = compose("first");

        let _ = app.__update(__DucktapeMessage::SendMessageSubmit);
        let operation_id = app.messages[0].id.clone();
        app.message_editor = compose("second");
        let _ = app.__update(__DucktapeMessage::MessageSendFailed(
            backend::OptimisticMutationError {
                message: "rejected".into(),
                committed: false,
                operation_id,
                scope_id: "general".into(),
                body: "first".into(),
            },
        ));

        assert_eq!(composer(&app), "second");
        assert_eq!(app.failed_message_draft, "first");
        app.message_editor = compose("");
        let _ = app.__update(__DucktapeMessage::RestoreFailedMessage);
        assert_eq!(composer(&app), "first");
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
        app.message_editor = compose("committed once");

        let _ = app.__update(__DucktapeMessage::SendMessageSubmit);
        let operation_id = app.messages[0].id.clone();
        let _ = app.__update(__DucktapeMessage::MessageSendFailed(
            backend::OptimisticMutationError {
                message: "read failed after commit".into(),
                committed: true,
                operation_id,
                scope_id: "general".into(),
                body: "committed once".into(),
            },
        ));

        assert!(app.message_draft.is_empty());
        assert_eq!(app.messages.len(), 1);
        assert!(app.messages[0].pending);
        assert_eq!(app.mutation_phase, "idle");
        assert_eq!(app.sync_phase, "refreshing");

        app.message_editor = compose("still available");
        let _ = app.__update(__DucktapeMessage::SendMessageSubmit);
        assert_eq!(app.messages.len(), 2);
        assert_eq!(app.mutation_phase, "idle");
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
    fn optimistic_thread_replies_settle_independently_out_of_order() {
        let (mut app, _) = Ducktape::__boot();
        app.connected = true;
        app.loading = false;
        app.active_channel = "general".into();
        app.active_thread_seq = 1;
        app.reply_editor = compose("first");

        let _ = app.__update(__DucktapeMessage::SendReplySubmit);
        let first_id = app.thread_messages[0].id.clone();
        assert_eq!(app.mutation_phase, "idle");
        assert!(reply_composer(&app).is_empty());
        assert!(app.thread_messages[0].pending);

        app.reply_editor = compose("second");
        let _ = app.__update(__DucktapeMessage::SendReplySubmit);
        let second_id = app.thread_messages[1].id.clone();
        assert_ne!(first_id, second_id);
        assert_eq!(app.thread_messages.len(), 2);
        assert!(app.thread_messages.iter().all(|message| message.pending));

        let mut second = message(3, "second", false);
        second.id = second_id.clone();
        second.thread_seq = 1;
        let _ = app.__update(__DucktapeMessage::ThreadReplySent(second));
        assert_eq!(app.thread_messages.len(), 2);
        assert!(
            app.thread_messages
                .iter()
                .any(|message| message.id == first_id && message.pending)
        );
        assert!(
            app.thread_messages
                .iter()
                .any(|message| message.body == "second" && !message.pending)
        );

        let mut first = message(2, "first", false);
        first.id = first_id.clone();
        first.thread_seq = 1;
        let _ = app.__update(__DucktapeMessage::ThreadReplySent(first));
        assert_eq!(app.thread_messages.len(), 2);
        assert!(app.thread_messages.iter().all(|message| !message.pending));
        assert_eq!(app.thread_messages[0].body, "first");
        assert_eq!(app.thread_messages[1].body, "second");
    }

    #[test]
    fn failed_thread_reply_rolls_back_only_itself_and_preserves_the_newer_draft() {
        let (mut app, _) = Ducktape::__boot();
        app.connected = true;
        app.loading = false;
        app.active_channel = "general".into();
        app.active_thread_seq = 1;
        app.reply_editor = compose("first");

        let _ = app.__update(__DucktapeMessage::SendReplySubmit);
        let first_id = app.thread_messages[0].id.clone();
        app.reply_editor = compose("second");
        let _ = app.__update(__DucktapeMessage::SendReplySubmit);
        let second_id = app.thread_messages[1].id.clone();
        app.reply_editor = compose("newer draft");

        let _ = app.__update(__DucktapeMessage::ThreadReplySendFailed(
            backend::OptimisticMutationError {
                message: "rejected".into(),
                committed: false,
                operation_id: first_id,
                scope_id: "general".into(),
                body: "first".into(),
            },
        ));
        assert_eq!(reply_composer(&app), "newer draft");
        assert_eq!(app.failed_reply_draft, "first");
        assert_eq!(app.thread_messages.len(), 1);
        assert_eq!(app.thread_messages[0].id, second_id);
        assert!(app.thread_messages[0].pending);
        assert_eq!(app.mutation_phase, "idle");
        assert!(!app.thread_loading);

        let _ = app.__update(__DucktapeMessage::RestoreFailedReply);
        assert_eq!(reply_composer(&app), "newer draft");
        assert_eq!(app.failed_reply_draft, "first");
        app.reply_editor = compose("");
        let _ = app.__update(__DucktapeMessage::RestoreFailedReply);
        assert_eq!(reply_composer(&app), "first");
        assert!(app.failed_reply_draft.is_empty());
    }

    #[test]
    fn committed_thread_reply_refreshes_without_blocking_the_composer() {
        let (mut app, _) = Ducktape::__boot();
        app.connected = true;
        app.loading = false;
        app.connected_rpc = "http://node".into();
        app.active_channel = "general".into();
        app.active_thread_seq = 1;
        app.reply_editor = compose("committed");

        let _ = app.__update(__DucktapeMessage::SendReplySubmit);
        let operation_id = app.thread_messages[0].id.clone();
        let _ = app.__update(__DucktapeMessage::ThreadReplySendFailed(
            backend::OptimisticMutationError {
                message: "read failed after commit".into(),
                committed: true,
                operation_id,
                scope_id: "general".into(),
                body: "committed".into(),
            },
        ));
        assert_eq!(app.thread_messages.len(), 1);
        assert!(app.thread_messages[0].pending);
        assert!(reply_composer(&app).is_empty());
        assert!(app.failed_reply_draft.is_empty());
        assert_eq!(app.mutation_phase, "idle");
        assert!(!app.thread_loading);
        assert_eq!(app.sync_phase, "refreshing");

        app.reply_editor = compose("still available");
        let _ = app.__update(__DucktapeMessage::SendReplySubmit);
        assert_eq!(app.thread_messages.len(), 2);
        assert!(app.thread_messages.iter().all(|message| message.pending));
        assert!(reply_composer(&app).is_empty());
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
        let operation_id = app.blocks[0].id.clone();
        assert_eq!(app.mutation_phase, "idle");
        assert!(app.block_draft.is_empty());
        assert_eq!(app.blocks[0].kind, "Heading 2");
        assert!(app.blocks[0].pending);

        let _ = app.__update(__DucktapeMessage::BlockAddFailed(
            backend::OptimisticMutationError {
                message: "rejected".into(),
                committed: false,
                operation_id,
                scope_id: "welcome".into(),
                body: "retry heading".into(),
            },
        ));
        assert_eq!(app.block_draft, "retry heading");
        assert!(app.orphaned_block_drafts.is_empty());
        assert!(app.blocks.is_empty());
        assert_eq!(app.mutation_phase, "idle");
    }
}
