//! The governance module's view, run as wasm inside the app: every decision
//! the network is being asked to make, read through one `query.governance`
//! and refreshed whenever the app says the module's data moved.

pub mod host;

ui_lang::include_app!("src/ui/app.ice");

ui_lang_guest::export_app!(
    Approvals,
    __ApprovalsMessage,
    "Approvals",
    "Every decision this network is being asked to make.",
    ["query"]
);
