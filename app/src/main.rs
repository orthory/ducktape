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
        assert!(source.contains("run block_tip(connected_rpc)"));
        assert!(source.contains("run refresh(connected_rpc"));
    }

    #[test]
    fn stale_refresh_is_ignored_after_user_action() {
        let (mut app, _) = Ducktape::__boot();
        app.status = "current".into();
        app.sync_phase = "refreshing".into();
        app.loading = false;

        let _ = app.__update(__DucktapeMessage::ChooseChannel("next".into()));
        assert_eq!(app.sync_phase, "idle");
        app.loading = false;

        let _ = app.__update(__DucktapeMessage::WorkspaceRefreshed(
            backend::WorkspaceData {
                rpc: "http://stale".into(),
                status: "stale".into(),
                height: 99,
                channels: Vec::new(),
                messages: Vec::new(),
                active_channel: String::new(),
                pages: Vec::new(),
                blocks: Vec::new(),
                active_page: String::new(),
                active_page_title: String::new(),
            },
        ));
        assert_eq!(app.status, "current");

        app.sync_phase = "polling".into();
        let _ = app.__update(__DucktapeMessage::PollFailed(backend::AppError {
            message: "offline".into(),
        }));
        assert_eq!(app.sync_phase, "idle");
    }
}
