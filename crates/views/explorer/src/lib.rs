//! The block explorer as a module-owned view on the kernel contract.
//!
//! The kernel pushes session facts only (`explorer.props`: connected, dark,
//! and the live head and sync line the app already holds). The block window
//! is this view's own — `rpc.blocks`, re-read on every `rpc.live` hit for the
//! `block` plane — and so is the workspace search, which fans out over
//! `rpc.query` and `rpc.view`. A clipboard copy is the one act that leaves as
//! an intent: the OS door is the kernel's. The endpoint, the key and the
//! password never cross.

pub mod host;

ui_lang::include_app!("src/ui/app.ice");

ui_lang_guest::export_app!(
    ExplorerView,
    "Explorer",
    "The ledger this network wrote: blocks, their operations, and a search over the workspace.",
    ["explorer"]
);
