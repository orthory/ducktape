//! Pages as a module-owned view: the page sidebar, the document header, the
//! tab strip, the subpage list and the comments rail, from the facts the
//! desktop app pushes. The document itself is the app's editor, painted into
//! the slot this view leaves for it; every act — a pick, a search, a comment,
//! a delete — leaves as an intent the app signs.

#[path = "editor_indent.rs"]
pub mod indent;
#[path = "editor_inline.rs"]
pub mod inline;

pub mod document_source;
pub mod document_ingress;
pub mod editor_binding;
#[path = "editor_markdown.rs"]
pub mod markdown;
#[path = "editor_presentation.rs"]
pub mod presentation;
pub mod editor;
pub mod editor_menu;
pub mod host;

ui_lang::include_app!("src/ui/app.ice");

ui_lang_guest::export_app!(
    PagesView,
    "Pages",
    "The workspace's pages: the sidebar, the document header, the tab strip and the comments rail.",
    ["pages"]
);
