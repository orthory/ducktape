//! Files as a module-owned view: the duckfs browser — one directory at a
//! time, its preview, its history and the write bar — drawn from the facts
//! the desktop app pushes. Every navigation and every write leaves as an
//! intent the app signs; the pictures, the highlighted reader and the
//! Markdown document are the app's own surfaces, painted into the slots the
//! view leaves for them.

pub mod host;

ui_lang::include_app!("src/ui/app.ice");

ui_lang_guest::export_app!(
    FilesView,
    "Files",
    "The duckfs browser: one directory at a time, its preview, its history and its writes.",
    ["files"]
);
