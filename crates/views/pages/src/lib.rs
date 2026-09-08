//! Pages as a module-owned view: the page sidebar, the document header, the
//! tab strip, the subpage list and the comments rail, from the facts the
//! desktop app pushes. The document itself is the app's editor, painted into
//! the slot this view leaves for it; every act — a pick, a search, a comment,
//! a delete — leaves as an intent the app signs.

pub mod host;

ui_lang::include_app!("src/ui/app.ice");

ui_lang_guest::export_app!(
    PagesView,
    "Pages",
    "The workspace's pages: the sidebar, the document header, the tab strip and the comments rail.",
    ["pages"]
);
