//! The block explorer as a module-owned view: the ledger the desktop app
//! pushes — blocks, their ops, the head — and one workspace search the app
//! runs on the view's behalf. A refresh, a search, its clearing and a
//! clipboard copy leave as intents; the answer comes back as props.

pub mod host;

ui_lang::include_app!("src/ui/app.ice");

ui_lang_guest::export_app!(
    ExplorerView,
    "Explorer",
    "The ledger this network wrote: blocks, their operations, and a search over the workspace.",
    ["explorer"]
);
