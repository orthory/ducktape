//! Pages: the block canvas collapses to marker + editor, with per-block chrome
//! moved behind hover, a `⋮` actions menu, and single add/parent affordances.

use super::harness::*;
use crate::screens::pages::{
    self, BlockKind, Message, PageBlock, PageDocument, PageMeta, PagesData, State,
};
use crate::theme;

fn block(id: &str, kind: BlockKind, text: &str) -> PageBlock {
    PageBlock {
        id: id.into(),
        kind,
        text: text.into(),
        depth: 0,
        checked: false,
        parent: "doc".into(),
        children: Vec::new(),
        marks: Vec::new(),
    }
}

fn ready_state(blocks: Vec<PageBlock>) -> State {
    let mut state = State::default();
    state.loaded(Ok(Some(PagesData {
        pages: vec![PageMeta {
            id: "doc".into(),
            title: "Notes".into(),
            parent: None,
        }],
        open_tabs: vec!["doc".into()],
        document: Some(PageDocument {
            id: "doc".into(),
            title: "Notes".into(),
            ancestry: vec![PageMeta {
                id: "doc".into(),
                title: "Notes".into(),
                parent: None,
            }],
            blocks,
            page_comments: 0,
            comment_threads: Vec::new(),
            presence: Vec::new(),
            self_key: None,
        }),
    })));
    state
}

fn light() -> theme::Palette {
    *theme::palette(theme::Mode::Light)
}

#[test]
fn block_row_has_no_permanent_toolbar() {
    // The ~17-control per-block toolbar is gone: move/paste/kind-cycle live on
    // the keyboard and the actions menu, not under every block (P-G1).
    let state = ready_state(vec![block("a", BlockKind::Paragraph, "hello")]);
    let mut ui = sim(pages::view(&state, light()));
    assert!(
        !has(&mut ui, Role::Button, "↑"),
        "move-up is keyboard-wired; no per-block button"
    );
    assert!(
        !has(&mut ui, Role::Button, "Paste"),
        "paste is keyboard-wired; no per-block button"
    );
    assert!(
        !has(&mut ui, Role::Button, "¶"),
        "the always-on kind-cycle button is gone (now Turn into, in the menu)"
    );
}

#[test]
fn hover_reveals_actions_menu_trigger() {
    let mut state = ready_state(vec![block("a", BlockKind::Paragraph, "hello")]);
    // Cursor over the row reveals the gutter grips.
    state.hovered_block = Some("a".into());
    let mut ui = sim(pages::view(&state, light()));
    assert!(
        has(&mut ui, Role::Button, "Block actions"),
        "hovering a block reveals its actions menu trigger"
    );
    ui.click(by::role(Role::Button, "Block actions"))
        .expect("menu trigger is clickable");
    assert!(emitted(ui, &Message::ToggleBlockMenu(0)));
}

#[test]
fn open_menu_carries_danger_delete() {
    let mut state = ready_state(vec![block("a", BlockKind::Paragraph, "hello")]);
    state.menu_open_block = Some("a".into());
    let mut ui = sim(pages::view(&state, light()));
    assert!(
        has(&mut ui, Role::Button, "Delete block"),
        "the actions menu holds the danger Delete (absorbing the old × button)"
    );
    ui.click(by::role(Role::Button, "Delete block"))
        .expect("delete is clickable");
    assert!(emitted(ui, &Message::RequestRemoveBlock(0)));
}

#[test]
fn single_add_affordance_replaces_the_palette() {
    // One "Add a block" affordance, not the always-on 8-button palette (P-G5).
    let state = ready_state(vec![block("a", BlockKind::Paragraph, "hello")]);
    let mut ui = sim(pages::view(&state, light()));
    assert!(has(&mut ui, Role::Button, "Add a block"));
    assert!(
        !has(&mut ui, Role::Button, "To-do"),
        "the permanent kind palette is gone"
    );
    assert!(
        !has(&mut ui, Role::Button, "Callout"),
        "the permanent kind palette is gone"
    );
    ui.click(by::role(Role::Button, "Add a block"))
        .expect("add affordance is clickable");
    assert!(emitted(ui, &Message::AddBlock(BlockKind::Paragraph)));
}

#[test]
fn move_page_parent_picker_is_gone() {
    // Re-parenting is a rail-tree gesture, never a per-document button wall (P-G6).
    let state = ready_state(vec![block("a", BlockKind::Paragraph, "hello")]);
    let mut ui = sim(pages::view(&state, light()));
    assert!(
        !has(&mut ui, Role::Button, "Top level"),
        "the MOVE PAGE parent picker is removed"
    );
}

#[test]
fn rail_header_new_and_refresh_are_ghost_buttons() {
    // The reported bug's fix: real, labelled New page / Refresh controls in a
    // boxless header (§0 / G7).
    let state = ready_state(vec![block("a", BlockKind::Paragraph, "hello")]);
    let mut ui = sim(pages::view(&state, light()));
    assert!(has(&mut ui, Role::Button, "New page"));
    assert!(has(&mut ui, Role::Button, "Refresh pages"));
    ui.click(by::role(Role::Button, "New page"))
        .expect("new page is clickable");
    assert!(emitted(ui, &Message::NewPage));
}

#[test]
fn delete_page_is_danger_styled_and_wired() {
    let state = ready_state(vec![block("a", BlockKind::Paragraph, "hello")]);
    let mut ui = sim(pages::view(&state, light()));
    assert!(has(&mut ui, Role::Button, "Delete page"));
    ui.click(by::role(Role::Button, "Delete page"))
        .expect("delete page is clickable");
    assert!(emitted(ui, &Message::RequestDeletePage));
}
