//! Pages transaction round-trips against the embedded sim node.

use super::super::*;
use super::SimShell;
use crate::screens::user::Resource;
use iced_agent_plugin::Role;

#[test]
fn new_page_creates_a_document_round_trip() {
    let mut ui = SimShell::boot();
    ui.inject(Message::Navigate(Screen::Pages));
    assert!(
        matches!(ui.shell().user_screens.pages.data, Resource::Empty),
        "fresh sim has no pages"
    );

    ui.click(Role::Button, "New page");

    assert!(
        ui.shell().user_screens.pages.error.is_none(),
        "create failed: {:?}",
        ui.shell().user_screens.pages.error
    );
    // The committed page re-renders from node state — Ok chains a LoadPages
    // re-query, so `pages.data` is node data, not a local echo.
    let data = match &ui.shell().user_screens.pages.data {
        Resource::Ready(data) => data,
        other => panic!("pages list not loaded into the render model: {other:?}"),
    };
    let titles: Vec<String> = data.pages.iter().map(|p| p.title.clone()).collect();
    assert_eq!(
        titles,
        vec!["Untitled"],
        "the created page renders in the list"
    );
    let document = data
        .document
        .as_ref()
        .expect("the created page is the open document");
    assert_eq!(document.id, data.pages[0].id);
    assert_eq!(document.title, "Untitled");

    let listed = ui.node_query("pages", serde_json::json!("list_pages"));
    assert!(
        listed.to_string().contains("Untitled"),
        "page is committed node-side: {listed}"
    );
}
