//! Files: render variants, the write affordances, and navigation emissions.

use std::collections::BTreeMap;

use super::harness::*;
use crate::screens::file_browser::{self, FileEntry, FileKind, FileListing, Message, State};
use crate::theme;
use crate::view_api::Resource;

fn live_listing() -> FileListing {
    FileListing {
        path: "/shared".into(),
        entries: Vec::new(),
        preview: None,
        read_only: false,
        refreshing: false,
        head: Some("head".into()),
        snapshot: None,
        history: Vec::new(),
        diff: Vec::new(),
    }
}

fn entry(name: &str, kind: FileKind) -> FileEntry {
    FileEntry {
        path: format!("/shared/{name}"),
        name: name.into(),
        kind,
        size: 42,
        executable: false,
        object: String::new(),
        meta: BTreeMap::new(),
    }
}

fn ready(listing: FileListing) -> State {
    State {
        data: Resource::Ready(listing),
        ..State::default()
    }
}

fn ui_of(state: &State) -> iced_test::Simulator<'_, Message> {
    sim(file_browser::view(state, *theme::palette(theme::Mode::Light)))
}

#[test]
fn fresh_shared_root_offers_the_first_folder() {
    // A brand-new chain answers "path not found" for /shared. The adapter
    // synthesizes an empty writeable listing instead of a dead error screen, so
    // the network can create its very first directory. The string is the
    // WRAPPED form the service layer actually delivers, parens and all —
    // matching the raw module string here once masked a live failure.
    let mut state = State::default();
    file_browser::loaded(&mut state, Err("Module(files: path not found)".into()));
    assert!(
        matches!(&state.data, Resource::Ready(listing) if !listing.read_only),
        "a fresh /shared must be a writeable Ready listing"
    );

    let p = *theme::palette(theme::Mode::Light);
    let mut ui = sim(file_browser::view(&state, p));
    assert!(
        has(&mut ui, Role::Button, "New folder"),
        "the fresh-chain files tab must offer New folder"
    );
    ui.click(by::role(Role::Button, "New folder"))
        .expect("New folder is enabled on a writeable root");
    assert!(
        emitted(ui, &Message::ToggleNewFolder),
        "New folder must be live, not a disabled decoy"
    );
}

#[test]
fn ready_header_offers_refresh() {
    let state = State {
        data: Resource::Ready(live_listing()),
        ..State::default()
    };
    let p = *theme::palette(theme::Mode::Light);
    let mut ui = sim(file_browser::view(&state, p));
    assert!(
        has(&mut ui, Role::Button, "Refresh"),
        "the header must expose a manual Refresh"
    );
    ui.click(by::role(Role::Button, "Refresh"))
        .expect("Refresh is clickable when files are loaded");
    assert!(emitted(ui, &Message::Refresh));
}

// The passive render variants each show their own center state, so the tab is
// never a blank card while loading or empty.
#[test]
fn passive_states_show_their_center_copy() {
    for (data, needle) in [
        (Resource::Loading, "Loading files…"),
        (Resource::Empty, "Empty directory"),
        // A missing node is a calm "enter a network", not the red error card.
        (
            Resource::Error("please enter a network".into()),
            "No node connected",
        ),
    ] {
        let state = State {
            data,
            ..State::default()
        };
        let mut ui = ui_of(&state);
        assert!(ui.find(needle).is_ok(), "expected {needle:?} to render");
        assert!(
            !has(&mut ui, Role::Button, "Retry"),
            "a passive state must not offer Retry"
        );
    }
}

// A genuine read failure is the error card with a copyable detail and a live
// Retry — the surfacing path whose absence hid the pages regression.
#[test]
fn read_failure_surfaces_a_retry() {
    let state = State {
        data: Resource::Error("Module(files: read timed out)".into()),
        ..State::default()
    };
    let mut ui = ui_of(&state);
    assert!(ui.find("Could not read folder").is_ok());
    assert!(
        ui.find("Module(files: read timed out)").is_ok(),
        "the failure detail is shown and selectable for copy"
    );
    ui.click(by::role(Role::Button, "Retry"))
        .expect("the error card offers a live Retry");
    assert!(emitted(ui, &Message::Refresh));
}

// A read-only snapshot badges itself and the write affordances are inert decoys
// (not merely reducer-guarded): the disabled button drops its on_press, while
// Refresh — read-safe — stays live.
#[test]
fn snapshot_listing_disables_writes_but_not_refresh() {
    let listing = FileListing {
        read_only: true,
        snapshot: Some("snap-7".into()),
        ..live_listing()
    };
    let state = ready(listing);

    let mut ui = ui_of(&state);
    assert!(ui.find("snapshot").is_ok(), "read-only shows the snapshot chip");
    ui.click(by::role(Role::Button, "New folder"))
        .expect("the button is present");
    assert!(
        !emitted(ui, &Message::ToggleNewFolder),
        "New folder must be inert while browsing a snapshot"
    );

    let mut ui = ui_of(&state);
    ui.click(by::role(Role::Button, "Refresh"))
        .expect("Refresh stays live on a snapshot");
    assert!(emitted(ui, &Message::Refresh));
}

#[test]
fn history_toggle_emits() {
    let state = ready(live_listing());
    let mut ui = ui_of(&state);
    ui.click(by::role(Role::Button, "History"))
        .expect("History is a header action");
    assert!(emitted(ui, &Message::ToggleHistory));
}

// The mkdir form: the name field is addressable and the enabled Create folder
// button fires the create.
#[test]
fn new_folder_form_creates() {
    let state = State {
        show_new_folder: true,
        new_folder_name: "designs".into(),
        ..ready(live_listing())
    };
    let mut ui = ui_of(&state);
    assert!(has(&mut ui, Role::TextInput, "Folder name"));
    ui.click(by::role(Role::Button, "Create folder"))
        .expect("a non-empty draft enables Create folder");
    assert!(emitted(ui, &Message::CreateFolder));
}

// A directory row is the primary navigation affordance; opening it loads that
// child directory.
#[test]
fn directory_row_opens_the_child() {
    let state = ready(FileListing {
        entries: vec![entry("child", FileKind::Directory)],
        ..live_listing()
    });
    let mut ui = ui_of(&state);
    ui.click(by::role(Role::ListItem, "child"))
        .expect("a directory row opens");
    assert!(emitted(
        ui,
        &Message::OpenEntry("/shared/child".into(), FileKind::Directory)
    ));
}

#[test]
fn file_row_offers_download() {
    let state = ready(FileListing {
        entries: vec![entry("logo.svg", FileKind::File)],
        ..live_listing()
    });
    let mut ui = ui_of(&state);
    ui.click(by::role(Role::Button, "Download"))
        .expect("a file row exposes Download");
    assert!(emitted(ui, &Message::Download("/shared/logo.svg".into(), 42)));
}

// The pending-delete confirm strip renders the subtree warning and wires both
// the destructive confirm and the cancel escape. Kept on an empty listing so
// the only Delete/Cancel in the tree are the confirm strip's.
#[test]
fn delete_confirm_strip_confirms_and_cancels() {
    let state = State {
        pending_delete: Some(entry("notes", FileKind::Directory)),
        ..ready(live_listing())
    };
    let mut ui = ui_of(&state);
    assert!(
        ui.find("Delete notes? This removes the whole subtree.").is_ok(),
        "the confirm names the target and warns about the subtree"
    );
    ui.click(by::role(Role::Button, "Delete"))
        .expect("the confirm strip offers Delete");
    assert!(emitted(ui, &Message::ConfirmDelete));

    let mut ui = ui_of(&state);
    ui.click(by::role(Role::Button, "Cancel"))
        .expect("the confirm strip offers Cancel");
    assert!(emitted(ui, &Message::CancelDelete));
}

// The breadcrumb renders parent segments as live crumbs (plain buttons, so
// addressed by their text) that navigate up; the current segment is inert text.
#[test]
fn breadcrumb_parent_navigates() {
    let state = ready(FileListing {
        path: "/team/design".into(),
        ..live_listing()
    });
    let mut ui = ui_of(&state);
    ui.click("team")
        .expect("the parent crumb is a live link");
    assert!(emitted(
        ui,
        &Message::OpenEntry("/team".into(), FileKind::Directory)
    ));
}

// A non-fatal error over a Ready listing surfaces as an inline banner (the tab
// keeps rendering its columns); it auto-clears on the next update, so there is
// no dismiss button by design.
#[test]
fn ready_error_surfaces_inline() {
    let state = State {
        error: Some("Switch to Live head before uploading dropped files.".into()),
        ..ready(live_listing())
    };
    let mut ui = ui_of(&state);
    assert!(
        ui.find("Switch to Live head before uploading dropped files.")
            .is_ok(),
        "the inline banner shows the reason over the still-rendered listing"
    );
}
