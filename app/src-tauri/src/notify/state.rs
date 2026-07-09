//! Persistence for the notification unread count.
//!
//! `state.json` deliberately contains only `unread`. The connection subscribes
//! live from the tip on every app start, so cursors belong to the in-memory
//! [`super::engine::Engine`] and must never be written here. Persisting a cursor
//! would make a later start replay history as a burst of desktop notifications.

use std::{fs, path::Path};

/// The complete persisted notification state: only the unread badge count.
///
/// Stream cursors are intentionally excluded because they are valid only for a
/// reconnect during the current app session.
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct NotifyState {
    pub unread: u32,
}

pub fn load(path: &Path) -> NotifyState {
    fs::read(path)
        .ok()
        .and_then(|contents| serde_json::from_slice(&contents).ok())
        .unwrap_or_default()
}

pub fn save(path: &Path, state: &NotifyState) {
    let Ok(contents) = serde_json::to_vec(state) else {
        return;
    };

    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, contents);
}
