//! Native notification decode and matching helpers for the desktop shell.

pub mod decode;
pub mod engine;
pub mod matchers;
#[allow(dead_code)] // Constructed when the notification worker is wired into app startup.
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
