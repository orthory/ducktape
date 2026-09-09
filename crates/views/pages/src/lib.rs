//! Pages owns document editing, Markdown presentation, selection, and undo history.
//! The desktop app supplies bounded source transfers and persists only accepted
//! canonical revisions; navigation and signed writes remain app intents.

#[path = "editor_indent.rs"]
pub mod indent;
#[path = "editor_inline.rs"]
pub mod inline;

pub mod document_ingress;
pub mod document_source;
pub mod editor;
pub mod editor_binding;
pub mod editor_menu;
pub mod editor_view;
pub mod host;
#[path = "editor_markdown.rs"]
pub mod markdown;
#[path = "editor_presentation.rs"]
pub mod presentation;

ui_lang::include_app!("src/ui/app.ice");

ui_lang_guest::export_app!(
    PagesView,
    "Pages",
    "The workspace's pages: the sidebar, the document header, the tab strip and the comments rail.",
    ["pages"]
);
