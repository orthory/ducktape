//! Native notification decoding, matching, and unread state for the desktop shell.
//!
//! The notifier subscribes live from the tip whenever the app starts, so only the
//! unread count is persisted. Stream cursors stay in memory for reconnects within
//! the current session; persisting them could replay history as a burst of stale
//! desktop notifications on a later app start.

// Phase A defines and tests the notifier without calling it from the binary yet.
// DELETE this allow once the stream module wires the notifier into `setup()`.
#![allow(dead_code)]

pub mod decode;
pub mod engine;
pub mod huddle;
pub mod matchers;
pub mod present;
pub mod state;

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NotifyPrefs {
    pub enabled: bool,
    pub mentions: bool,
    pub replies: bool,
    pub huddles: bool,
    pub runs: bool,
    pub forge: bool,
    pub governance: bool,
    pub muted_channels: Vec<String>,
}

impl Default for NotifyPrefs {
    fn default() -> Self {
        Self {
            enabled: true,
            mentions: true,
            replies: true,
            huddles: true,
            runs: true,
            forge: true,
            governance: true,
            muted_channels: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NotifyConfig {
    pub node_url: Option<String>,
    pub self_user_key_hex: Option<String>,
    pub self_node_keys_hex: Vec<String>,
    pub focused_channel: Option<String>,
    pub main_window_focused: bool,
    pub author_names: std::collections::BTreeMap<String, String>,
    pub prefs: NotifyPrefs,
}
