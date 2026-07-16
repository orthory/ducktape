//! Pages: the block canvas collapses to marker + editor, with per-block chrome
//! moved behind hover, a `⋮` actions menu, and single add/parent affordances.

use super::harness::*;
use crate::screens::pages::{
    self, BlockKind, Effect, Message, PageBlock, PageDocument, PageMeta, PagesData, State,
};
use crate::theme;
use crate::view_api::Resource;

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

fn meta(id: &str, title: &str, parent: Option<&str>) -> PageMeta {
    PageMeta {
        id: id.into(),
        title: title.into(),
        parent: parent.map(str::to_string),
    }
}

// A Ready state with only a rail (no open document), for filter/collapse tests.
fn rail_state(pages: Vec<PageMeta>) -> State {
    let mut state = State::default();
    state.loaded(Ok(Some(PagesData {
        pages,
        open_tabs: Vec::new(),
        document: None,
    })));
    state
}

fn tabs(state: &State) -> Vec<String> {
    match &state.data {
        Resource::Ready(data) => data.open_tabs.clone(),
        _ => Vec::new(),
    }
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

// A create must NOT open anything optimistically: the minted id stays pending
// until the commit succeeds, so a failed create leaves no phantom document.
#[test]
fn new_page_defers_open_until_commit() {
    let mut state = rail_state(vec![meta("doc", "Notes", None)]);
    let Some(Effect::CreatePage { id, parent: None }) =
        pages::update(&mut state, Message::NewPage)
    else {
        panic!("expected page create");
    };
    assert!(state.document().is_none(), "no optimistic document");
    assert!(tabs(&state).is_empty(), "no optimistic tab");
    assert_eq!(state.pending_create.as_deref(), Some(id.as_str()));
}

// The "cannot create any document" incident: a failed create lands in a state
// with NO open document, and the error used to render only inside an open
// document's body — the click looked like it did nothing.
#[test]
fn write_failure_is_visible_without_an_open_document() {
    let mut state = State::default();
    state.loaded(Ok(Some(PagesData {
        pages: vec![PageMeta {
            id: "doc".into(),
            title: "Notes".into(),
            parent: None,
        }],
        open_tabs: Vec::new(),
        document: None,
    })));
    state.error = Some("op rejected: Module(pages unavailable)".into());
    let mut ui = sim(pages::view(&state, light()));
    assert!(
        has(&mut ui, Role::Button, "Dismiss"),
        "a pages write failure must render even with no document open"
    );
    ui.click(by::role(Role::Button, "Dismiss"))
        .expect("dismiss is clickable");
    assert!(emitted(ui, &Message::DismissError));
}

// Same incident, fresh-workspace shape: the FIRST create failing leaves data
// Empty + error set; the empty state must not swallow it.
#[test]
fn write_failure_is_visible_on_the_empty_state() {
    let mut state = State::default();
    state.error = Some("op rejected: Module(pages unavailable)".into());
    let mut ui = sim(pages::view(&state, light()));
    assert!(
        has(&mut ui, Role::Button, "Dismiss"),
        "a first-create failure must render on the empty state"
    );
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

// --- render variants: every load arm reaches its own body ---

#[test]
fn loading_state_shows_a_loading_notice() {
    let state = State::default();
    let mut ui = sim(pages::view(&state, light()));
    assert!(ui.find("Loading pages…").is_ok());
}

#[test]
fn error_state_renders_the_load_error() {
    let mut state = State::default();
    state.loaded(Err("pages module offline".into()));
    let mut ui = sim(pages::view(&state, light()));
    assert!(ui.find("Couldn't load Pages").is_ok());
}

#[test]
fn empty_state_prompts_first_page() {
    let mut state = State::default();
    state.loaded(Ok(None));
    let mut ui = sim(pages::view(&state, light()));
    assert!(ui.find("No pages yet. Use + to start writing.").is_ok());
}

// --- rail interactions ---

#[test]
fn refresh_button_reloads() {
    let state = ready_state(vec![block("a", BlockKind::Paragraph, "hello")]);
    let mut ui = sim(pages::view(&state, light()));
    ui.click(by::role(Role::Button, "Refresh pages"))
        .expect("refresh is clickable");
    assert!(emitted(ui, &Message::Refresh));
}

#[test]
fn search_filters_and_reports_no_match() {
    let mut state = rail_state(vec![meta("a", "Release", None), meta("b", "Roadmap", None)]);
    state.query = "rel".into();
    {
        let mut ui = sim(pages::view(&state, light()));
        assert!(has(&mut ui, Role::ListItem, "Release"));
        assert!(
            !has(&mut ui, Role::ListItem, "Roadmap"),
            "the rail hides pages that don't match the search query"
        );
    }
    state.query = "zzz".into();
    let mut ui = sim(pages::view(&state, light()));
    assert!(ui.find("No pages match this search.").is_ok());
}

#[test]
fn collapsing_a_parent_hides_children_in_rail() {
    let mut state = rail_state(vec![
        meta("p", "Parent", None),
        meta("c", "Child", Some("p")),
    ]);
    {
        let mut ui = sim(pages::view(&state, light()));
        assert!(has(&mut ui, Role::ListItem, "Child"));
    }
    state.collapsed_pages = vec!["p".into()];
    let mut ui = sim(pages::view(&state, light()));
    assert!(has(&mut ui, Role::ListItem, "Parent"));
    assert!(
        !has(&mut ui, Role::ListItem, "Child"),
        "a collapsed parent's descendants drop out of the rail"
    );
}

// --- tab lifecycle ---

#[test]
fn open_page_appends_tab_and_loads() {
    let mut state = ready_state(vec![block("a", BlockKind::Paragraph, "hello")]);
    assert_eq!(
        pages::update(&mut state, Message::OpenPage("guide".into())),
        Some(Effect::LoadPage("guide".into()))
    );
    assert!(tabs(&state).contains(&"guide".to_string()));
}

#[test]
fn open_page_at_focuses_target_block() {
    let mut state = ready_state(vec![block("a", BlockKind::Paragraph, "hello")]);
    let effect = pages::update(
        &mut state,
        Message::OpenPageAt {
            page: "guide".into(),
            block: "b3".into(),
        },
    );
    assert_eq!(effect, Some(Effect::LoadPage("guide".into())));
    assert_eq!(state.focused_block.as_deref(), Some("b3"));
    assert!(tabs(&state).contains(&"guide".to_string()));
}

#[test]
fn close_tab_keeps_others_but_drops_the_active_document() {
    let mut state = ready_state(vec![block("a", BlockKind::Paragraph, "hello")]);
    if let Resource::Ready(data) = &mut state.data {
        data.open_tabs.push("guide".into());
    }
    pages::update(&mut state, Message::CloseTab("guide".into()));
    assert_eq!(tabs(&state), vec!["doc".to_string()]);
    assert!(
        state.document().is_some(),
        "closing an inactive tab leaves the open document alone"
    );
    pages::update(&mut state, Message::CloseTab("doc".into()));
    assert!(tabs(&state).is_empty());
    assert!(
        state.document().is_none(),
        "closing the active tab drops its open document"
    );
}

// --- title commit ---

#[test]
fn title_edit_and_commit_renames_page() {
    let mut state = ready_state(vec![block("a", BlockKind::Paragraph, "hello")]);
    pages::update(&mut state, Message::TitleChanged("Renamed".into()));
    assert_eq!(state.document().unwrap().title, "Renamed");
    assert_eq!(
        pages::update(&mut state, Message::CommitTitle),
        Some(Effect::RenamePage {
            page: "doc".into(),
            title: "Renamed".into(),
        })
    );
}

// --- block enter / backspace transitions (caret defaults to end-of-text with
// no live selection, so these exercise the deterministic branches) ---

#[test]
fn enter_at_end_of_bullet_adds_a_bullet() {
    let mut state = ready_state(vec![block("a", BlockKind::Bulleted, "milk")]);
    let Some(Effect::SplitBlock { left, right, .. }) =
        pages::update(&mut state, Message::BlockEnter(0))
    else {
        panic!("expected a split");
    };
    assert_eq!(left.text, "milk");
    assert_eq!(right.text, "");
    assert_eq!(
        right.kind,
        BlockKind::Bulleted,
        "the new block continues the list kind"
    );
}

#[test]
fn enter_on_empty_bullet_exits_to_paragraph() {
    let mut state = ready_state(vec![block("a", BlockKind::Bulleted, "")]);
    assert_eq!(
        pages::update(&mut state, Message::BlockEnter(0)),
        Some(Effect::SetBlockKind {
            block: "a".into(),
            kind: BlockKind::Paragraph,
        })
    );
}

#[test]
fn backspace_empty_block_removes_it() {
    let mut state = ready_state(vec![block("a", BlockKind::Paragraph, "")]);
    assert_eq!(
        pages::update(&mut state, Message::BlockBackspace(0)),
        Some(Effect::RemoveBlock("a".into()))
    );
}

// --- slash command: menu render → application ---

#[test]
fn slash_menu_offers_matching_kind() {
    let mut state = ready_state(vec![block("a", BlockKind::Paragraph, "/h1")]);
    state.slash_for = Some(0);
    let mut ui = sim(pages::view(&state, light()));
    assert!(has(&mut ui, Role::Button, "H1"));
    ui.click(by::role(Role::Button, "H1"))
        .expect("slash option is clickable");
    assert!(emitted(ui, &Message::ApplySlash(0, BlockKind::Heading1)));
}

#[test]
fn apply_slash_sets_kind_and_clears_text() {
    let mut state = ready_state(vec![block("a", BlockKind::Paragraph, "/h1")]);
    state.slash_for = Some(0);
    let effect = pages::update(&mut state, Message::ApplySlash(0, BlockKind::Heading1));
    assert_eq!(
        effect,
        Some(Effect::ApplySlash {
            block: "a".into(),
            kind: BlockKind::Heading1,
            text: String::new(),
        })
    );
    let doc_block = &state.document().unwrap().blocks[0];
    assert_eq!(doc_block.kind, BlockKind::Heading1);
    assert_eq!(doc_block.text, "");
    assert_eq!(state.slash_for, None);
}

// --- paste drop: the 60-block cap and its notice ---

#[test]
fn paste_over_limit_reports_dropped_lines() {
    let mut state = ready_state(vec![block("a", BlockKind::Paragraph, "hello")]);
    let text = (0..62)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let Some(Effect::PasteBlocks { blocks, .. }) =
        pages::update(&mut state, Message::PasteBlocks(0, text))
    else {
        panic!("expected a paste");
    };
    assert_eq!(blocks.len(), 60, "only the first 60 lines are pasted");
    assert_eq!(state.paste_dropped, 2);
}

#[test]
fn paste_drop_notice_renders() {
    let mut state = ready_state(vec![block("a", BlockKind::Paragraph, "hello")]);
    state.paste_dropped = 5;
    let mut ui = sim(pages::view(&state, light()));
    assert!(
        ui.find("5 pasted lines were dropped at the 60-block safety limit.")
            .is_ok()
    );
}

// --- delete page: confirm/cancel gate ---

#[test]
fn delete_page_confirm_and_cancel() {
    let mut state = ready_state(vec![block("a", BlockKind::Paragraph, "hello")]);
    pages::update(&mut state, Message::RequestDeletePage);
    assert!(state.pending_page_delete);
    pages::update(&mut state, Message::CancelDeletePage);
    assert!(!state.pending_page_delete);

    pages::update(&mut state, Message::RequestDeletePage);
    assert_eq!(
        pages::update(&mut state, Message::ConfirmDeletePage),
        Some(Effect::DeletePage("doc".into()))
    );
    assert!(!state.pending_page_delete);
}

#[test]
fn delete_page_confirmation_is_wired() {
    let mut state = ready_state(vec![block("a", BlockKind::Paragraph, "hello")]);
    state.pending_page_delete = true;
    let mut ui = sim(pages::view(&state, light()));
    assert!(has(&mut ui, Role::Button, "Cancel"));
    assert!(has(&mut ui, Role::Button, "Delete"));
    ui.click(by::role(Role::Button, "Delete"))
        .expect("confirm delete is clickable");
    assert!(emitted(ui, &Message::ConfirmDeletePage));
}
