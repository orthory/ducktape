//! The Approvals screen as a module-owned view: every decision the network
//! is being asked to make, and the ones it has settled, rendered from a
//! wasm component the desktop app loads from a file.
//!
//! The view is a pure function of what the host pushes in (`governance.props`,
//! one item per change) and speaks back only in intents (`governance.vote`,
//! `governance.execute`). Loading, credentials, signing and the write itself
//! stay in the desktop app, which is what keeps this component free of every
//! network and key concern: a guest that sees no key cannot leak one.

pub mod host;

ui_lang::include_app!("src/ui/app.ice");

ui_lang_guest::export_app!(
    GovernanceView,
    "Approvals",
    "Every decision this network is being asked to make, and the ones it has settled.",
    ["governance"]
);
