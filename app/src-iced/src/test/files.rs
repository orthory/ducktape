//! Files: the fresh-chain write path and the refresh affordance.

use super::harness::*;
use crate::screens::file_browser::{self, FileListing, Message, State};
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

#[test]
fn fresh_shared_root_offers_the_first_folder() {
    // A brand-new chain answers "path not found" for /shared. The adapter
    // synthesizes an empty writeable listing instead of a dead error screen, so
    // the network can create its very first directory.
    let mut state = State::default();
    file_browser::loaded(&mut state, Err("files: path not found".into()));
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
