//! Stateful notification processing for live stream frames.
//!
//! The engine persists the unread count, but its per-topic cursors are strictly
//! in-memory. App startup always subscribes live from the tip; carrying cursors
//! across starts could replay historical events as a burst of notifications.

use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError},
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::Value;

use super::{
    decode,
    matchers::{self, Category, MatchState, MatcherCtx, Notification},
    state::{self, NotifyState, StoredNotification},
    NotifyConfig,
};

/// How many presented notifications the bell dropdown keeps.
pub const RECENT_CAP: usize = 50;

#[derive(Debug, Clone)]
pub enum Frame {
    Event {
        topic: String,
        cursor: String,
        op: Value,
    },
    Lagged {
        topic: String,
        cursor: String,
    },
}

/// Delivery seam for desktop notifications and unread badges.
///
/// Keeping OS integration behind this trait lets the entire engine be tested
/// with a capture sink and no desktop notification service.
pub trait Sink: Send {
    fn present(&self, n: &Notification);
    fn badge(&self, unread: u32);
    /// A presented notification for the in-app bell (live dropdown update).
    fn item(&self, _item: &StoredNotification) {}
}

pub struct Engine<S: Sink> {
    sink: S,
    match_state: MatchState,
    state_path: PathBuf,
    unread: u32,
    /// IN-MEMORY ONLY, never persisted. These cursors resume a live stream only
    /// during a transient reconnect in the current app session.
    cursors: BTreeMap<String, String>,
    /// Recent presented notifications, newest first, capped at [`RECENT_CAP`].
    /// Shared with [`super::NotifyHandles`] so the `notify_recent` command can
    /// read it without actor plumbing; the engine is the only writer.
    recent: Arc<Mutex<VecDeque<StoredNotification>>>,
}

impl<S: Sink> Engine<S> {
    pub fn new(
        sink: S,
        state_path: PathBuf,
        recent: Arc<Mutex<VecDeque<StoredNotification>>>,
    ) -> Self {
        let loaded = state::load(&state_path);
        sink.badge(loaded.unread);
        *recent.lock().unwrap_or_else(PoisonError::into_inner) =
            loaded.recent.into_iter().collect();

        Self {
            sink,
            match_state: MatchState::default(),
            state_path,
            unread: loaded.unread,
            cursors: BTreeMap::new(),
            recent,
        }
    }

    pub fn handle(
        &mut self,
        frame: Frame,
        config: &NotifyConfig,
        root_author: &dyn Fn(&str, u64) -> Option<String>,
    ) {
        match frame {
            Frame::Event { topic, cursor, op } => {
                self.cursors.insert(topic.clone(), cursor);

                let Some(op) = decode::decode_op_row(&op) else {
                    return;
                };
                let ctx = MatcherCtx {
                    self_user_key_hex: config.self_user_key_hex.as_deref(),
                    self_node_keys_hex: &config.self_node_keys_hex,
                    author_names: &config.author_names,
                    root_author,
                };
                let Some(notification) =
                    matchers::match_topic(&topic, &op, &ctx, &mut self.match_state)
                else {
                    return;
                };

                if !should_present(&notification, config) {
                    return;
                }

                self.sink.present(&notification);
                let stored = StoredNotification {
                    category: notification.category,
                    title: notification.title.clone(),
                    body: notification.body.clone(),
                    channel_id: notification.channel_id.clone(),
                    at: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map_or(0, |duration| duration.as_millis() as u64),
                };
                {
                    let mut recent =
                        self.recent.lock().unwrap_or_else(PoisonError::into_inner);
                    recent.push_front(stored.clone());
                    recent.truncate(RECENT_CAP);
                }
                self.sink.item(&stored);
                self.unread = self.unread.saturating_add(1);
                self.sink.badge(self.unread);
                // Unconditional: even on a saturated count the ring gained an item.
                self.persist();
            }
            Frame::Lagged { topic, cursor } => {
                self.cursors.insert(topic, cursor);
            }
        }
    }

    pub fn mark_seen(&mut self) {
        self.unread = 0;
        self.sink.badge(0);
        self.persist();
    }

    /// Returns cursors for a transient in-session reconnect.
    ///
    /// They must not be used to resume history after an app restart.
    pub fn cursors(&self) -> &BTreeMap<String, String> {
        &self.cursors
    }

    /// Discards cursors after the node URL changes, since they belong to the old
    /// node's streams and are no longer valid.
    pub fn reset_cursors(&mut self) {
        self.cursors.clear();
    }

    fn persist(&self) {
        let recent = self
            .recent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .cloned()
            .collect();
        state::save(
            &self.state_path,
            &NotifyState {
                unread: self.unread,
                recent,
            },
        );
    }
}

fn should_present(notification: &Notification, config: &NotifyConfig) -> bool {
    config.prefs.enabled
        && category_enabled(notification.category, config)
        && !notification.channel_id.as_ref().is_some_and(|channel| {
            config
                .prefs
                .muted_channels
                .iter()
                .any(|muted| muted == channel)
        })
        && !(config.main_window_focused
            && notification
                .channel_id
                .as_ref()
                .is_some_and(|channel| config.focused_channel.as_ref() == Some(channel)))
}

fn category_enabled(category: Category, config: &NotifyConfig) -> bool {
    match category {
        Category::Mention => config.prefs.mentions,
        Category::Reply => config.prefs.replies,
        Category::Huddle => config.prefs.huddles,
        Category::Run => config.prefs.runs,
        Category::Forge => config.prefs.forge,
        Category::Governance => config.prefs.governance,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        fs,
        path::{Path, PathBuf},
        sync::{
            atomic::{AtomicU64, Ordering},
            Mutex,
        },
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::{json, Value};

    use super::*;
    use crate::notify::{
        matchers::{Category, Notification},
        state::{self, NotifyState},
        NotifyConfig, NotifyPrefs,
    };

    static PATH_COUNTER: AtomicU64 = AtomicU64::new(0);
    type DisablePreference = fn(&mut NotifyPrefs);

    #[derive(Default)]
    struct CaptureSink {
        presented: Mutex<Vec<Notification>>,
        badges: Mutex<Vec<u32>>,
    }

    impl Sink for CaptureSink {
        fn present(&self, notification: &Notification) {
            self.presented.lock().unwrap().push(notification.clone());
        }

        fn badge(&self, unread: u32) {
            self.badges.lock().unwrap().push(unread);
        }
    }

    struct TestStatePath {
        directory: PathBuf,
        state: PathBuf,
    }

    impl TestStatePath {
        fn new() -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_nanos());
            let suffix = PATH_COUNTER.fetch_add(1, Ordering::Relaxed);
            let directory = std::env::temp_dir().join(format!(
                "ducktape-notify-engine-{}-{nanos}-{suffix}",
                std::process::id()
            ));
            let state = directory.join("state.json");
            Self { directory, state }
        }

        fn path(&self) -> &Path {
            &self.state
        }
    }

    impl Drop for TestStatePath {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }

    fn config() -> NotifyConfig {
        NotifyConfig {
            self_user_key_hex: Some("1234".to_string()),
            self_node_keys_hex: vec!["aaaa".to_string(), "bbbb".to_string()],
            author_names: BTreeMap::from([("cccc".to_string(), "Casey".to_string())]),
            ..NotifyConfig::default()
        }
    }

    fn mention_op(channel: &str) -> Value {
        json!({
            "height": 42,
            "seq": 1,
            "time": 1_720_000_000_u64,
            "origin": { "kind": "external", "id": "cccc" },
            "payload": {
                "post_message": {
                    "channel_id": channel,
                    "message_id": "m1",
                    "blocks": [{
                        "paragraph": [{
                            "text": "hello",
                            "marks": [{ "mention": { "user": [18, 52] } }]
                        }]
                    }],
                    "thread": null,
                    "as_agent": null
                }
            }
        })
    }

    fn huddle_op(channel: &str) -> Value {
        json!({
            "height": 43,
            "seq": 1,
            "time": 1_720_000_001_u64,
            "origin": { "kind": "external", "id": "cccc" },
            "payload": {
                "join_huddle": { "channel_id": channel, "node": [204, 204] }
            }
        })
    }

    fn event(topic: &str, cursor: String, op: Value) -> Frame {
        Frame::Event {
            topic: topic.to_string(),
            cursor,
            op,
        }
    }

    fn no_root(_: &str, _: u64) -> Option<String> {
        None
    }

    fn engine(path: &TestStatePath) -> Engine<CaptureSink> {
        let engine = Engine::new(CaptureSink::default(), path.path().to_path_buf(), Arc::default());
        engine.sink.badges.lock().unwrap().clear();
        engine
    }

    fn presented(engine: &Engine<CaptureSink>) -> Vec<Notification> {
        engine.sink.presented.lock().unwrap().clone()
    }

    fn badges(engine: &Engine<CaptureSink>) -> Vec<u32> {
        engine.sink.badges.lock().unwrap().clone()
    }

    fn notification(category: Category) -> Notification {
        Notification {
            category,
            title: String::new(),
            body: String::new(),
            channel_id: None,
        }
    }

    #[test]
    fn event_presents_mention_increments_unread_advances_cursor_and_persists() {
        let path = TestStatePath::new();
        let mut engine = engine(&path);
        let cursor = "op/000000000000002a/0001".to_string();

        engine.handle(
            event("module:chat", cursor.clone(), mention_op("general")),
            &config(),
            &no_root,
        );

        let notifications = presented(&engine);
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].category, Category::Mention);
        assert_eq!(engine.unread, 1);
        assert_eq!(badges(&engine), vec![1]);
        assert_eq!(engine.cursors().get("module:chat"), Some(&cursor));
        let persisted = state::load(path.path());
        assert_eq!(persisted.unread, 1);
        // The presented notification rides along in the persisted ring.
        assert_eq!(persisted.recent.len(), 1);
        assert_eq!(persisted.recent[0].category, Category::Mention);
        assert_eq!(persisted.recent[0].channel_id.as_deref(), Some("general"));
    }

    #[test]
    fn disabled_notifications_drop_event_but_still_advance_cursor() {
        let path = TestStatePath::new();
        let mut engine = engine(&path);
        let cursor = "op/0000000000000001/0000".to_string();
        let mut config = config();
        config.prefs.enabled = false;

        engine.handle(
            event("module:chat", cursor.clone(), mention_op("general")),
            &config,
            &no_root,
        );

        assert!(presented(&engine).is_empty());
        assert_eq!(engine.unread, 0);
        assert!(badges(&engine).is_empty());
        assert_eq!(engine.cursors().get("module:chat"), Some(&cursor));
        assert!(!path.path().exists());
    }

    #[test]
    fn category_and_muted_channel_preferences_gate_notifications() {
        let category_path = TestStatePath::new();
        let mut category_engine = engine(&category_path);
        let mut category_config = config();
        category_config.prefs.mentions = false;

        category_engine.handle(
            event("module:chat", "op/0000000000000001/0000".to_string(), mention_op("general")),
            &category_config,
            &no_root,
        );
        category_engine.handle(
            event("module:chat", "op/0000000000000002/0000".to_string(), huddle_op("general")),
            &category_config,
            &no_root,
        );

        let notifications = presented(&category_engine);
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].category, Category::Huddle);
        assert_eq!(category_engine.unread, 1);

        let muted_path = TestStatePath::new();
        let mut muted_engine = engine(&muted_path);
        let mut muted_config = config();
        muted_config.prefs.muted_channels = vec!["general".to_string()];

        muted_engine.handle(
            event("module:chat", "op/0000000000000001/0000".to_string(), mention_op("general")),
            &muted_config,
            &no_root,
        );
        muted_engine.handle(
            event("module:chat", "op/0000000000000002/0000".to_string(), huddle_op("general")),
            &muted_config,
            &no_root,
        );

        assert!(presented(&muted_engine).is_empty());
        assert_eq!(muted_engine.unread, 0);
        assert!(badges(&muted_engine).is_empty());
        assert!(!muted_path.path().exists());
    }

    #[test]
    fn every_category_uses_its_matching_preference_toggle() {
        let cases: [(Category, DisablePreference); 6] = [
            (Category::Mention, |prefs| prefs.mentions = false),
            (Category::Reply, |prefs| prefs.replies = false),
            (Category::Huddle, |prefs| prefs.huddles = false),
            (Category::Run, |prefs| prefs.runs = false),
            (Category::Forge, |prefs| prefs.forge = false),
            (Category::Governance, |prefs| prefs.governance = false),
        ];

        for (disabled_category, disable) in cases {
            let mut config = config();
            config.prefs = NotifyPrefs {
                enabled: true,
                mentions: true,
                replies: true,
                huddles: true,
                runs: true,
                forge: true,
                governance: true,
                muted_channels: Vec::new(),
            };
            disable(&mut config.prefs);

            assert!(
                !should_present(&notification(disabled_category), &config),
                "{disabled_category:?} should use its matching preference toggle"
            );

            let enabled_category = if disabled_category == Category::Mention {
                Category::Reply
            } else {
                Category::Mention
            };
            assert!(
                should_present(&notification(enabled_category), &config),
                "disabling {disabled_category:?} should leave {enabled_category:?} enabled"
            );
        }
    }

    #[test]
    fn focused_channel_is_suppressed_only_while_main_window_is_focused() {
        let path = TestStatePath::new();
        let mut engine = engine(&path);
        let mut config = config();
        config.main_window_focused = true;
        config.focused_channel = Some("general".to_string());

        engine.handle(
            event("module:chat", "op/0000000000000001/0000".to_string(), mention_op("general")),
            &config,
            &no_root,
        );
        engine.handle(
            event("module:chat", "op/0000000000000002/0000".to_string(), mention_op("other")),
            &config,
            &no_root,
        );
        config.main_window_focused = false;
        engine.handle(
            event("module:chat", "op/0000000000000003/0000".to_string(), mention_op("general")),
            &config,
            &no_root,
        );

        let notifications = presented(&engine);
        assert_eq!(notifications.len(), 2);
        assert_eq!(notifications[0].channel_id.as_deref(), Some("other"));
        assert_eq!(notifications[1].channel_id.as_deref(), Some("general"));
        assert_eq!(engine.unread, 2);
        assert_eq!(badges(&engine), vec![1, 2]);
    }

    #[test]
    fn lagged_adopts_cursor_without_backfill_then_event_presents() {
        let path = TestStatePath::new();
        let mut engine = engine(&path);
        let lagged_cursor = "op/0000000000000050/ffff".to_string();

        engine.handle(
            Frame::Lagged {
                topic: "module:chat".to_string(),
                cursor: lagged_cursor.clone(),
            },
            &config(),
            &no_root,
        );

        assert_eq!(engine.cursors().get("module:chat"), Some(&lagged_cursor));
        assert!(presented(&engine).is_empty());
        assert_eq!(engine.unread, 0);
        assert!(badges(&engine).is_empty());
        assert!(!path.path().exists());

        let event_cursor = "op/0000000000000051/0000".to_string();
        engine.handle(
            event("module:chat", event_cursor.clone(), mention_op("general")),
            &config(),
            &no_root,
        );

        assert_eq!(presented(&engine).len(), 1);
        assert_eq!(engine.unread, 1);
        assert_eq!(engine.cursors().get("module:chat"), Some(&event_cursor));
    }

    #[test]
    fn mark_seen_persists_zero_and_new_restores_unread_badge() {
        let path = TestStatePath::new();
        let mut engine = Engine::new(CaptureSink::default(), path.path().to_path_buf(), Arc::default());
        assert_eq!(badges(&engine), vec![0]);

        engine.handle(
            event("module:chat", "op/0000000000000001/0000".to_string(), mention_op("general")),
            &config(),
            &no_root,
        );
        engine.mark_seen();

        assert_eq!(engine.unread, 0);
        assert_eq!(badges(&engine), vec![0, 1, 0]);
        assert_eq!(state::load(path.path()).unread, 0);

        let restored = Engine::new(CaptureSink::default(), path.path().to_path_buf(), Arc::default());
        assert_eq!(restored.unread, 0);
        assert_eq!(badges(&restored), vec![0]);
        assert!(presented(&restored).is_empty());
        assert!(restored.cursors().is_empty());

        let five_path = TestStatePath::new();
        state::save(five_path.path(), &NotifyState { unread: 5, recent: Vec::new() });
        let restored_five = Engine::new(CaptureSink::default(), five_path.path().to_path_buf(), Arc::default());
        assert_eq!(restored_five.unread, 5);
        assert_eq!(badges(&restored_five), vec![5]);
    }

    #[test]
    fn corrupt_state_loads_default_without_panicking() {
        let path = TestStatePath::new();
        fs::create_dir_all(&path.directory).unwrap();
        fs::write(path.path(), b"{ definitely not json").unwrap();

        assert_eq!(state::load(path.path()).unread, 0);
    }

    #[test]
    fn fresh_engine_does_not_replay_or_restore_cursors() {
        let path = TestStatePath::new();
        state::save(path.path(), &NotifyState { unread: 4, recent: Vec::new() });

        let mut engine = Engine::new(CaptureSink::default(), path.path().to_path_buf(), Arc::default());

        assert!(presented(&engine).is_empty());
        assert!(engine.cursors().is_empty());
        assert_eq!(engine.unread, 4);
        assert_eq!(badges(&engine), vec![4]);

        let cursor = "op/0000000000000063/0000".to_string();
        engine.handle(
            event("module:chat", cursor.clone(), mention_op("general")),
            &config(),
            &no_root,
        );
        assert_eq!(presented(&engine).len(), 1);
        assert_eq!(engine.cursors().get("module:chat"), Some(&cursor));

        engine.reset_cursors();
        assert!(engine.cursors().is_empty());
    }

    #[test]
    fn recent_ring_is_newest_first_capped_and_persisted() {
        let path = TestStatePath::new();
        let recent = Arc::new(Mutex::new(VecDeque::new()));
        let mut engine = Engine::new(
            CaptureSink::default(),
            path.path().to_path_buf(),
            recent.clone(),
        );
        let config = config();
        for i in 0..(RECENT_CAP + 5) {
            let mut op = mention_op("general");
            op["payload"]["post_message"]["message_id"] = json!(format!("m{i}"));
            op["payload"]["post_message"]["blocks"][0]["paragraph"][0]["text"] =
                json!(format!("msg {i}"));
            engine.handle(event("module:chat", format!("c{i}"), op), &config, &no_root);
        }

        {
            let ring = recent.lock().unwrap();
            assert_eq!(ring.len(), RECENT_CAP);
            // Newest first: the LAST fed frame sits at the front.
            assert!(
                ring[0].body.contains(&format!("msg {}", RECENT_CAP + 4)),
                "front should be the newest item, got body {:?}",
                ring[0].body
            );
            assert_eq!(ring[0].category, Category::Mention);
            assert_eq!(ring[0].channel_id.as_deref(), Some("general"));
            assert!(ring[0].at > 0);
        }

        // A fresh engine over the same state file reloads the ring.
        let reloaded = Arc::new(Mutex::new(VecDeque::new()));
        let _restored = Engine::new(
            CaptureSink::default(),
            path.path().to_path_buf(),
            reloaded.clone(),
        );
        let ring = reloaded.lock().unwrap();
        assert_eq!(ring.len(), RECENT_CAP);
        assert!(ring[0].body.contains(&format!("msg {}", RECENT_CAP + 4)));
    }
}
