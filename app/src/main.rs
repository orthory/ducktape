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
        let toggles_input = source
            .lines()
            .filter(|line| line.trim_start().starts_with("input "))
            .any(|line| line.contains("sync_phase"));
        assert!(!toggles_input);

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
}
