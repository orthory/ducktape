//! Persistence for the notification unread count and the recent ring.
//!
//! `state.json` carries the unread badge count and the bell dropdown's recent
//! notifications — both survive a restart so the dropdown matches the badge.
//! The connection subscribes live from the tip on every app start, so cursors
//! belong to the in-memory [`super::engine::Engine`] and must never be written
//! here. Persisting a cursor would make a later start replay history as a
//! burst of desktop notifications.

use std::{fs, path::Path};

use super::matchers::Category;

/// One presented notification kept for the in-app bell dropdown.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredNotification {
    pub category: Category,
    pub title: String,
    pub body: String,
    pub channel_id: Option<String>,
    pub message_id: Option<String>,
    /// Epoch milliseconds at present time.
    pub at: u64,
}

/// The complete persisted notification state.
///
/// Stream cursors are intentionally excluded because they are valid only for a
/// reconnect during the current app session.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct NotifyState {
    pub unread: u32,
    /// Recent presented notifications, newest first, capped by the engine.
    pub recent: Vec<StoredNotification>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_round_trips_and_old_format_still_loads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");

        let state = NotifyState {
            unread: 2,
            recent: vec![StoredNotification {
                category: Category::Mention,
                title: "t".into(),
                body: "b".into(),
                channel_id: Some("general".into()),
                message_id: Some("m1".into()),
                at: 1,
            }],
        };
        save(&path, &state);
        assert_eq!(load(&path), state);

        // A pre-ring state.json (unread alone) loads with an empty list.
        std::fs::write(&path, br#"{"unread":3}"#).unwrap();
        let old = load(&path);
        assert_eq!(old.unread, 3);
        assert!(old.recent.is_empty());

        // Rings written before message anchors existed remain readable.
        std::fs::write(
            &path,
            br#"{"unread":1,"recent":[{"category":"mention","title":"t","body":"b","channelId":"general","at":1}]}"#,
        )
        .unwrap();
        let old_ring = load(&path);
        assert_eq!(old_ring.recent[0].message_id, None);
    }
}
