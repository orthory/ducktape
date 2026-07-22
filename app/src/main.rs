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
        let source = include_str!("ui/app.ice");
        let view = source.split_once("\nview\n").unwrap().1;
        assert!(!view.contains("sync_phase"));
        assert!(!source.contains("on refresh_now"));
        assert!(!view.contains("button \"Refresh\""));

        let refresh = source
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
            "message_draft",
            "message_edit_draft",
            "reply_draft",
            "page_draft",
            "active_page_title",
            "paragraph_draft",
        ];
        let overwrites_editable = refresh.lines().any(|line| {
            editable
                .iter()
                .any(|name| line.trim_start().starts_with(&format!("{name} =")))
        });
        assert!(!overwrites_editable);
        assert!(source.contains("run live_events(connected_rpc) when connected"));
        assert!(!source.contains("every 1s"));
        assert!(source.contains("run refresh(connected_rpc"));
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
                pages: Vec::new(),
                blocks: Vec::new(),
                active_page: String::new(),
                active_page_title: String::new(),
            },
        ));
        assert_eq!(app.status, "Live");
        assert_eq!(app.sync_phase, "idle");
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
        }));
        assert_eq!(app.message_draft, "second");
        assert_eq!(app.mutation_phase, "idle");
        assert!(!app.messages[0].pending);
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
        }));

        assert_eq!(app.message_draft, "retry me");
        assert!(app.messages.is_empty());
        assert_eq!(app.error, "rejected");
        assert_eq!(app.mutation_phase, "idle");
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

        let _ = app.__update(__DucktapeMessage::MutationFailed(backend::AppError {
            message: "rejected".into(),
        }));
        assert_eq!(app.reply_draft, "retry reply");
        assert!(app.thread_messages.is_empty());
        assert_eq!(app.mutation_phase, "idle");
    }
}
