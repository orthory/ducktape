//! Native notification decoding, matching, and unread state for the desktop shell.
//!
//! The notifier subscribes live from the tip whenever the app starts, so only the
//! unread count and the bell dropdown's recent ring are persisted. Stream cursors
//! stay in memory for reconnects within the current session; persisting them could
//! replay history as a burst of stale desktop notifications on a later app start.

pub mod decode;
pub mod http;
pub mod engine;
pub mod huddle;
pub mod matchers;
pub mod present;
pub mod stream;
pub mod state;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};

use tauri::Manager as _;

use state::StoredNotification;

/// Webview-pushed runtime config, shared with the stream task.
pub struct Shared {
    pub config: std::sync::Mutex<NotifyConfig>,
    /// Notified on any config replacement (the loop re-reads; a node_url change
    /// reconnects). MUST be signalled with permit-storing `notify_one`, never
    /// `notify_waiters`: the stream loop registers a fresh `notified()` future
    /// on every select iteration, so a permitless wake fired between
    /// registrations (e.g. while the loop is inside `engine.handle`) is lost —
    /// and against a healthy old node the watchdog never rescues it, leaving
    /// the notifier attached to a stale node forever. `notify_one` stores the
    /// permit; coalesced wakes are fine because the loop re-reads the whole
    /// config on every wake.
    pub changed: tokio::sync::Notify,
}

/// Handles held in tauri managed state so command handlers (and app exit)
/// can reach the stream task.
pub struct NotifyHandles {
    pub shared: Arc<Shared>,
    pub cmds: tokio::sync::mpsc::UnboundedSender<Cmd>,
    /// The spawned stream task's handle. The task is detach-on-drop, so this
    /// is held for the graceful [`stream::StreamHandle::shutdown`] at app exit
    /// rather than for keep-alive.
    pub stream: stream::StreamHandle,
    /// The engine's recent ring (newest first) read by [`notify_recent`];
    /// the engine is the only writer.
    pub recent: Arc<Mutex<VecDeque<StoredNotification>>>,
}

/// Build the engine over the real [`present::AppSink`], spawn the stream loop,
/// and register the native focus backstop. Call from setup() AFTER `tray::init`
/// (both hook window events on the same `"main"` window; handlers stack).
pub fn init<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<()> {
    let state_path = app
        .path()
        .app_data_dir()?
        .join("notify")
        .join("state.json");
    let recent = Arc::new(Mutex::new(VecDeque::new()));
    let engine = engine::Engine::new(present::AppSink(app.clone()), state_path, recent.clone());

    let (cmds, cmds_rx) = tokio::sync::mpsc::unbounded_channel();
    let shared = Arc::new(Shared {
        config: Mutex::new(NotifyConfig::default()),
        changed: tokio::sync::Notify::new(),
    });
    let stream = stream::spawn(shared.clone(), engine, cmds_rx);

    // Native focus backstop: the webview pushes focus through notify_configure
    // too, but a hidden or wedged webview can miss focus/blur. The OS window
    // event is the floor; last writer wins between the two sources. This only
    // floors `main_window_focused` (focus-suppression of the viewed channel) —
    // seen-marking belongs to the bell dropdown (`notify_mark_seen` from the
    // webview), so focusing the window no longer clears the badge.
    if let Some(main) = app.get_webview_window("main") {
        let focus_shared = shared.clone();
        main.on_window_event(move |event| {
            if let tauri::WindowEvent::Focused(focused) = event {
                focus_shared
                    .config
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .main_window_focused = *focused;
                focus_shared.changed.notify_one();
            }
        });
    }

    app.manage(NotifyHandles {
        shared,
        cmds,
        stream,
        recent,
    });
    Ok(())
}

/// Replace the shared config and wake the stream loop — the whole body of
/// [`notify_configure`], factored so tests can drive it without a tauri State.
/// Permit-storing `notify_one` (see [`Shared::changed`]): a wake that lands
/// while the loop is between `notified()` registrations must not be lost.
pub(crate) fn apply_config(shared: &Shared, config: NotifyConfig) {
    *shared
        .config
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = config;
    shared.changed.notify_one();
}

#[tauri::command]
pub fn notify_configure(
    state: tauri::State<'_, NotifyHandles>,
    config: NotifyConfig,
) -> Result<(), String> {
    apply_config(&state.shared, config);
    Ok(())
}

#[tauri::command]
pub fn notify_mark_seen(state: tauri::State<'_, NotifyHandles>) -> Result<(), String> {
    // A closed channel means the stream task is gone (app teardown).
    let _ = state.cmds.send(Cmd::MarkSeen);
    Ok(())
}

/// Recent presented notifications, newest first, for the in-app bell dropdown.
#[tauri::command]
pub fn notify_recent(
    state: tauri::State<'_, NotifyHandles>,
) -> Result<Vec<StoredNotification>, String> {
    Ok(state
        .recent
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .cloned()
        .collect())
}

/// Commands crossing from tauri command handlers into the stream task.
pub enum Cmd {
    MarkSeen,
}

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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use super::{apply_config, NotifyConfig, NotifyPrefs, Shared};

    #[tokio::test]
    async fn apply_config_replaces_the_config_and_wakes_a_parked_waiter() {
        let shared = Arc::new(Shared {
            config: Mutex::new(NotifyConfig::default()),
            changed: tokio::sync::Notify::new(),
        });

        // An already-parked waiter observes the wake.
        let notified = shared.changed.notified();
        tokio::pin!(notified);
        assert!(
            !notified.as_mut().enable(),
            "no permit may exist before apply_config runs"
        );

        apply_config(
            &shared,
            NotifyConfig {
                node_url: Some("http://127.0.0.1:8844".to_string()),
                prefs: NotifyPrefs {
                    mentions: false,
                    ..NotifyPrefs::default()
                },
                ..NotifyConfig::default()
            },
        );

        tokio::time::timeout(Duration::from_secs(5), notified)
            .await
            .expect("apply_config wakes the parked waiter");

        let seen = shared.config.lock().expect("config lock");
        assert_eq!(seen.node_url.as_deref(), Some("http://127.0.0.1:8844"));
        assert!(!seen.prefs.mentions, "the whole config was replaced");
    }

    /// The lost-wake regression (review finding on the stream seam): the loop
    /// registers a fresh `notified()` per select iteration, so a config change
    /// landing BETWEEN registrations must leave a stored permit behind. With
    /// `notify_waiters` this test hangs; `notify_one` stores the permit and the
    /// late registration returns immediately.
    #[tokio::test]
    async fn apply_config_wake_survives_until_the_next_waiter_registers() {
        let shared = Arc::new(Shared {
            config: Mutex::new(NotifyConfig::default()),
            changed: tokio::sync::Notify::new(),
        });

        // Nobody is parked when the config changes — exactly the window the
        // stream loop spends inside engine.handle.
        apply_config(
            &shared,
            NotifyConfig {
                node_url: Some("http://127.0.0.1:9000".to_string()),
                ..NotifyConfig::default()
            },
        );

        // The waiter registers only afterwards, as the loop's next select
        // iteration does. The stored permit must wake it immediately.
        tokio::time::timeout(Duration::from_secs(5), shared.changed.notified())
            .await
            .expect("a wake fired between registrations must not be lost");
    }
}
