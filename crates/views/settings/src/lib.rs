//! Settings as a module-owned view: this device's preferences, the account
//! this key speaks for and the workspace's lifecycle, from the facts the
//! desktop app pushes. Every act — a theme, a rename, a minted ticket, the
//! signing seat, forgetting the network — leaves as an intent the app signs.

pub mod host;

ui_lang::include_app!("src/ui/app.ice");

ui_lang_guest::export_app!(
    SettingsView,
    "Settings",
    "This device's preferences, the account this key speaks for, and the workspace's lifecycle.",
    ["settings"]
);
