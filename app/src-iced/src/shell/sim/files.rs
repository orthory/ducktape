//! Files transaction round-trips against the embedded sim node.

use super::super::*;
use super::SimShell;
use crate::screens::file_browser;
use crate::screens::user::Resource;
use iced_agent_plugin::Role;

fn create_folder(ui: &mut SimShell, name: &str) {
    ui.click(Role::Button, "New folder");
    ui.inject(Message::UserScreen(user_screens::Message::Files(
        file_browser::Message::NewFolderNameChanged(name.into()),
    )));
    ui.click(Role::Button, "Create folder");
    assert!(
        ui.shell().user_screens.files.error.is_none(),
        "create {name:?} failed: {:?}",
        ui.shell().user_screens.files.error
    );
}

fn node_ls(ui: &SimShell, path: &str) -> serde_json::Value {
    ui.node_query(
        "files",
        serde_json::json!({
            "ls": {"path": path, "snapshot": null, "after": null, "limit": 256}
        }),
    )
}

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

#[test]
fn nested_folder() {
    let mut ui = SimShell::boot();
    ui.inject(Message::Navigate(Screen::Files));
    assert!(
        ui.shell().user_screens.files.error.is_none(),
        "files load failed: {:?}",
        ui.shell().user_screens.files.error
    );

    create_folder(&mut ui, "parent");
    ui.click(Role::ListItem, "parent");
    assert!(
        ui.shell().user_screens.files.error.is_none(),
        "open parent failed: {:?}",
        ui.shell().user_screens.files.error
    );
    create_folder(&mut ui, "child");

    let listing = match &ui.shell().user_screens.files.data {
        Resource::Ready(listing) => listing,
        other => panic!("nested listing not loaded into the render model: {other:?}"),
    };
    assert_eq!(listing.path, "/shared/parent");
    assert!(listing.entries.iter().any(|entry| entry.name == "child"));
    assert!(ui.has(Role::ListItem, "child"), "committed child renders");

    let node = node_ls(&ui, "/shared/parent");
    assert!(
        node["ls"]["entries"].as_array().is_some_and(|entries| {
            entries
                .iter()
                .any(|entry| entry["path"] == "/shared/parent/child")
        }),
        "child is committed node-side: {node}"
    );
}

#[test]
fn delete_folder() {
    let mut ui = SimShell::boot();
    ui.inject(Message::Navigate(Screen::Files));
    assert!(
        ui.shell().user_screens.files.error.is_none(),
        "files load failed: {:?}",
        ui.shell().user_screens.files.error
    );
    create_folder(&mut ui, "trash");

    ui.click(Role::Button, "Delete");
    assert_eq!(
        ui.shell()
            .user_screens
            .files
            .pending_delete
            .as_ref()
            .map(|entry| entry.path.as_str()),
        Some("/shared/trash")
    );
    ui.click(Role::Button, "Delete");
    assert!(
        ui.shell().user_screens.files.error.is_none(),
        "delete failed: {:?}",
        ui.shell().user_screens.files.error
    );

    let listing = match &ui.shell().user_screens.files.data {
        Resource::Ready(listing) => listing,
        other => panic!("listing not reloaded after delete: {other:?}"),
    };
    assert!(listing.entries.iter().all(|entry| entry.name != "trash"));
    assert!(
        !ui.has(Role::ListItem, "trash"),
        "deleted folder leaves the listing"
    );

    let node = node_ls(&ui, "/shared");
    assert!(
        node["ls"]["entries"].as_array().is_some_and(|entries| {
            entries.iter().all(|entry| entry["path"] != "/shared/trash")
        }),
        "deleted folder is absent node-side: {node}"
    );
}

// No rename scenario: file_browser::Message exposes no rename operation.
