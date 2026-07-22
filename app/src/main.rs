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
}
