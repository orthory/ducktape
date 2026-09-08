//! The facts the host pushes, the readings folded off them, and the writes
//! that leave as intents — one per act the settings screen offers.

use iced::futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use ui_lang_guest::host;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct HostError {
    pub message: String,
}

/// One key association as the account card lists it: the scheme token the
/// CLI prints, the hex key and the label ("" when none).
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct AccountKeyRow {
    pub scheme: String,
    pub pubkey: String,
    pub label: String,
}

/// The screen's facts, as the app holds them. The signing seat crosses as
/// `unlocked` only: the password never leaves the app. `drafts_cleared`
/// moves once per committed op and `drafts_scope` names which drafts it
/// consumed (`name`, `keys`, `label`, `account`).
#[derive(Clone, Debug, Default, Hash, PartialEq, Serialize, Deserialize)]
pub struct SettingsProps {
    pub dark: bool,
    pub connected: bool,
    pub loading: bool,
    pub status: String,
    pub busy: bool,
    pub recovering: bool,
    pub appearance: String,
    pub desktop_notifications: bool,
    pub unlocked: bool,
    pub account_name: String,
    pub network_name: String,
    pub connected_rpc: String,
    pub account_ceremony_phase: String,
    pub account_ceremony_qr: String,
    pub account_ceremony_detail: String,
    pub account_ceremony_left: String,
    pub settings_key_state: String,
    pub settings_key_path: String,
    pub settings_open_tabs: i64,
    pub tier: String,
    pub admin: bool,
    pub members_line: String,
    pub members_answered: bool,
    pub account_number: String,
    pub account_renaming: bool,
    pub account_exists: bool,
    pub account_keys: i64,
    pub account_key_rows: Vec<AccountKeyRow>,
    pub account_busy: bool,
    pub account_ticket: String,
    pub drafts_cleared: i64,
    pub drafts_scope: String,
}

/// The facts now, and again on every change the host sees.
pub fn props() -> impl Stream<Item = Result<SettingsProps, HostError>> + Send + 'static {
    host::subscribe("settings.props", &[]).map(|answer| {
        let bytes = answer.map_err(|message| HostError { message })?;
        serde_json::from_slice(&bytes).map_err(|error| HostError {
            message: error.to_string(),
        })
    })
}

/// `settings.tab` — open another rail tab (`members`, `node`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tab {
    pub tab: String,
}

/// `settings.unlock` — verify the key password and keep the signing seat.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unlock {
    pub password: String,
}

/// `settings.rename` and `settings.create` — an account name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Name {
    pub name: String,
}

/// `settings.key_add` — mint a ticket for another device's key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyAdd {
    pub pubkey: String,
    pub label: String,
}

/// `settings.join` — join the account a ticket was minted for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Join {
    pub ticket: String,
}

/// `settings.key_remove` — drop one key association.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyRemove {
    pub pubkey: String,
}

/// `settings.passkey`, `settings.passkey_desktop`, `settings.wallet` — the
/// label the new key is admitted under.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Label {
    pub label: String,
}

/// `settings.copy` — the host puts `text` on the clipboard and toasts `label`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Copy {
    pub text: String,
    pub label: String,
}

/// `settings.notifications` — desktop banners on or off.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notifications {
    pub enabled: bool,
}

pub fn open_tab(tab: &str) -> bool {
    notify("settings.tab", &Tab { tab: tab.into() })
}

pub fn reconnect_network() -> bool {
    notify("settings.reconnect", &())
}

pub fn switch_workspace() -> bool {
    notify("settings.switch_network", &())
}

pub fn unlock(password: &str) -> bool {
    notify(
        "settings.unlock",
        &Unlock {
            password: password.into(),
        },
    )
}

pub fn lock() -> bool {
    notify("settings.lock", &())
}

pub fn rename_account(name: &str) -> bool {
    notify("settings.rename", &Name { name: name.into() })
}

pub fn create_account(name: &str) -> bool {
    notify("settings.create", &Name { name: name.into() })
}

pub fn mint_ticket(pubkey: &str, label: &str) -> bool {
    notify(
        "settings.key_add",
        &KeyAdd {
            pubkey: pubkey.into(),
            label: label.into(),
        },
    )
}

pub fn join_account(ticket: &str) -> bool {
    notify(
        "settings.join",
        &Join {
            ticket: ticket.into(),
        },
    )
}

pub fn remove_key(pubkey: &str) -> bool {
    notify(
        "settings.key_remove",
        &KeyRemove {
            pubkey: pubkey.into(),
        },
    )
}

pub fn add_passkey(label: &str) -> bool {
    notify(
        "settings.passkey",
        &Label {
            label: label.into(),
        },
    )
}

pub fn add_passkey_here(label: &str) -> bool {
    notify(
        "settings.passkey_desktop",
        &Label {
            label: label.into(),
        },
    )
}

pub fn cancel_ceremony() -> bool {
    notify("settings.ceremony_cancel", &())
}

pub fn link_wallet(label: &str) -> bool {
    notify(
        "settings.wallet",
        &Label {
            label: label.into(),
        },
    )
}

pub fn login() -> bool {
    notify("settings.login", &())
}

pub fn copy(text: &str, label: &str) -> bool {
    notify(
        "settings.copy",
        &Copy {
            text: text.into(),
            label: label.into(),
        },
    )
}

pub fn clear_tabs() -> bool {
    notify("settings.clear_tabs", &())
}

pub fn forget_network() -> bool {
    notify("settings.forget", &())
}

pub fn set_light() -> bool {
    notify("settings.light", &())
}

pub fn set_dark() -> bool {
    notify("settings.dark", &())
}

pub fn set_notifications(enabled: bool) -> bool {
    notify("settings.notifications", &Notifications { enabled })
}

fn notify<T: Serialize>(operation: &str, payload: &T) -> bool {
    let bytes = serde_json::to_vec(payload).expect("an intent encodes");
    host::notify(operation, &bytes);
    true
}

// The desktop app's own readings (app/src/backend), repeated here because the
// view is its own crate: the wire carries the words, not the functions.

pub fn connection_degraded(status: &str) -> bool {
    status == "Offline"
        || status == "Sync delayed"
        || status == "Reconnecting…"
        || status == "Live · resyncing"
}

pub fn initial_of(name: &str) -> String {
    name.trim()
        .chars()
        .next()
        .map(|first| first.to_uppercase().to_string())
        .unwrap_or_default()
}

/// Did the op the app reported consume this draft? `account` — an op that
/// re-reads the account — takes every draft the card offers.
pub fn drafts_cleared_by(scope: &str, draft: &str) -> bool {
    scope == draft || scope == "account"
}

/// The draft as it stands, or nothing once an op consumed it.
pub fn keep_draft(consumed: bool, draft: &str) -> String {
    if consumed { String::new() } else { draft.into() }
}
