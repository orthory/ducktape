//! Files transaction round-trips against the embedded sim node.

use super::super::*;
use super::SimShell;
use crate::screens::file_browser;
use crate::screens::user::Resource;
use iced_agent_plugin::Role;

#[test]
fn create_folder_commits_and_relists() {
    let mut ui = SimShell::boot();
    ui.inject(Message::Navigate(Screen::Files));
    assert!(
        ui.shell().user_screens.files.error.is_none(),
        "files load failed: {:?}",
        ui.shell().user_screens.files.error
    );

    // Open the new-folder strip and submit: buttons are real clicks, the name
    // field is a bare input driven through update() (the chat exemplar's rule).
    ui.click(Role::Button, "New folder");
    ui.inject(Message::UserScreen(user_screens::Message::Files(
        file_browser::Message::NewFolderNameChanged("qa-folder".into()),
    )));
    ui.click(Role::Button, "Create folder");

    assert!(
        ui.shell().user_screens.files.error.is_none(),
        "create folder failed: {:?}",
        ui.shell().user_screens.files.error
    );
    // The committed folder re-renders from node state — Ok chains a re-list.
    assert!(
        ui.has(Role::ListItem, "qa-folder"),
        "committed folder renders in the listing"
    );
    let listing = match &ui.shell().user_screens.files.data {
        Resource::Ready(listing) => listing,
        other => panic!("listing not loaded into the render model: {other:?}"),
    };
    assert!(
        listing.entries.iter().any(|entry| entry.name == "qa-folder"),
        "render model carries the committed folder"
    );
}
