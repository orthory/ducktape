//! The WS client of the node's typed multiplexed `/v1/ws` stream (PR #306).
//!
//! Connects, subscribes the notifier topics, maps wire frames onto the
//! engine's [`Frame`], and drives [`Engine::handle`]. App start and node-url
//! changes subscribe live-from-tip (no resume — the no-replay guarantee);
//! only a transient in-session reconnect resumes from the engine's in-memory
//! cursors. Inbound wire data never panics the loop.

use std::collections::BTreeMap;
use std::sync::{Arc, PoisonError};
use std::time::Duration;

use futures_util::{SinkExt as _, StreamExt as _};
use serde_json::Value;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::Notify;
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::Message;

use super::engine::{Engine, Frame, Sink};

/// The notifier's fixed topic set — subscribed once per connection.
pub const TOPICS: [&str; 5] = [
    "module:chat",
    "module:pages",
    "module:runs",
    "module:forge",
    "module:governance",
];

/// Watchdog default until the first heartbeat announces the real interval.
const DEFAULT_HEARTBEAT_MS: u64 = 3_000;
const BACKOFF_FLOOR: Duration = Duration::from_secs(1);
const BACKOFF_CAP: Duration = Duration::from_secs(30);

/// Handle to the spawned stream task. Dropping it does NOT stop the loop;
/// call [`StreamHandle::shutdown`].
pub struct StreamHandle {
    shutdown: Arc<Notify>,
}

impl StreamHandle {
    /// Ask the loop to exit; it returns at its next select point.
    /// (`notify_one` stores a permit, so a wake between select points is
    /// never lost.)
    pub fn shutdown(&self) {
        self.shutdown.notify_one();
    }
}

/// Thin wrapper: spawns [`run_loop`] on tauri's async runtime.
pub fn spawn<S: Sink + 'static>(
    shared: Arc<super::Shared>,
    engine: Engine<S>,
    cmds: UnboundedReceiver<super::Cmd>,
) -> StreamHandle {
    let shutdown = Arc::new(Notify::new());
    // The JoinHandle is dropped deliberately: the task detaches on drop, and
    // shutdown rides the Notify, not the handle.
    tauri::async_runtime::spawn(run_loop(shared, engine, cmds, shutdown.clone()));
    StreamHandle { shutdown }
}

/// The actual loop — factored so tests drive it directly under #[tokio::test].
///
/// Never panics: connection and serde failures back off (1s doubling to 30s,
/// reset by any successful inbound message) and retry.
pub async fn run_loop<S: Sink>(
    shared: Arc<super::Shared>,
    mut engine: Engine<S>,
    mut cmds: UnboundedReceiver<super::Cmd>,
    shutdown: Arc<Notify>,
) {
    // The url the CURRENT cursors belong to — reset when it changes.
    let mut connected_url: Option<String> = None;
    let mut backoff = BACKOFF_FLOOR;
    // A closed cmds channel disables its select arm instead of busy-waking.
    let mut cmds_open = true;

    loop {
        // ---- park until the config names a node url ----
        let url = loop {
            if let Some(url) = config_url(&shared) {
                break url;
            }
            tokio::select! {
                _ = shared.changed.notified() => {}
                cmd = cmds.recv(), if cmds_open => service_cmd(&mut engine, &mut cmds_open, cmd),
                _ = shutdown.notified() => return,
            }
        };

        // ---- a different node's streams: this session's cursors are dead ----
        if connected_url.as_deref() != Some(url.as_str()) {
            if connected_url.is_some() {
                engine.reset_cursors();
            }
            connected_url = Some(url.clone());
        }

        match connection(
            &shared,
            &mut engine,
            &mut cmds,
            &mut cmds_open,
            &shutdown,
            &url,
            &mut backoff,
        )
        .await
        {
            ConnEnd::Shutdown => return,
            // a node_url change is a deliberate user action: reconnect
            // immediately, and re-floor the backoff — a failing streak against
            // the OLD node must not penalise the first dial of the new one.
            ConnEnd::Reconfigured => backoff = BACKOFF_FLOOR,
            ConnEnd::Dropped => {
                if let ParkEnd::Shutdown = backoff_park(
                    &shared,
                    &mut engine,
                    &mut cmds,
                    &mut cmds_open,
                    &shutdown,
                    &url,
                    backoff,
                )
                .await
                {
                    return;
                }
                backoff = (backoff * 2).min(BACKOFF_CAP);
            }
        }
    }
}

enum ConnEnd {
    /// Connect/subscribe failed, socket errored or closed, or the liveness
    /// watchdog expired — back off, then reconnect (same url ⇒ resume).
    Dropped,
    /// node_url changed (or became None) — reconnect without backoff.
    Reconfigured,
    Shutdown,
}

/// One connection: dial, subscribe, then pump frames until something ends it.
async fn connection<S: Sink>(
    shared: &super::Shared,
    engine: &mut Engine<S>,
    cmds: &mut UnboundedReceiver<super::Cmd>,
    cmds_open: &mut bool,
    shutdown: &Notify,
    url: &str,
    backoff: &mut Duration,
) -> ConnEnd {
    let endpoint = ws_url(url);
    let mut ws = tokio::select! {
        connected = tokio_tungstenite::connect_async(endpoint.clone()) => match connected {
            Ok((ws, _)) => ws,
            Err(err) => {
                eprintln!("notify stream: connect {endpoint} failed: {err}");
                return ConnEnd::Dropped;
            }
        },
        _ = shutdown.notified() => return ConnEnd::Shutdown,
    };

    let resume = engine.cursors();
    let subscribe = subscribe_text(&TOPICS, (!resume.is_empty()).then_some(resume));
    if let Err(err) = ws.send(Message::Text(subscribe)).await {
        eprintln!("notify stream: subscribe on {endpoint} failed: {err}");
        return ConnEnd::Dropped;
    }

    let http_base = url.trim_end_matches('/').to_string();
    let root_author =
        |channel: &str, root: u64| super::http::root_author(&http_base, channel, root);

    let mut interval = Duration::from_millis(DEFAULT_HEARTBEAT_MS);
    let mut deadline = Instant::now() + watchdog_timeout(interval);

    loop {
        tokio::select! {
            inbound = ws.next() => {
                let Some(Ok(frame)) = inbound else { return ConnEnd::Dropped };
                // ANY inbound message re-arms the watchdog and resets the
                // backoff: an old node sending legacy block frames reads as
                // connected-but-dormant, never as a reconnect storm.
                *backoff = BACKOFF_FLOOR;
                deadline = Instant::now() + watchdog_timeout(interval);
                let Message::Text(text) = frame else { continue };
                match map_frame(&text) {
                    Mapped::Event { topic, cursor, op } => {
                        // snapshot per frame; never hold the lock across handle.
                        let config = snapshot_config(shared);
                        engine.handle(Frame::Event { topic, cursor, op }, &config, &root_author);
                    }
                    Mapped::Lagged { topic, cursor } => {
                        let config = snapshot_config(shared);
                        engine.handle(Frame::Lagged { topic, cursor }, &config, &root_author);
                    }
                    Mapped::Heartbeat { interval_ms } => {
                        interval = Duration::from_millis(interval_ms.max(1));
                        deadline = Instant::now() + watchdog_timeout(interval);
                    }
                    Mapped::ErrorFrame { topic, detail } => {
                        // per-topic refusal; the socket stays open.
                        eprintln!("notify stream: server refused {topic}: {detail}");
                    }
                    Mapped::Ignored => {}
                }
            }
            cmd = cmds.recv(), if *cmds_open => service_cmd(engine, cmds_open, cmd),
            _ = shared.changed.notified() => {
                // config is re-read per frame anyway; only a url change matters.
                if config_url(shared).as_deref() != Some(url) {
                    return ConnEnd::Reconfigured;
                }
            }
            _ = tokio::time::sleep_until(deadline) => {
                eprintln!("notify stream: liveness watchdog expired on {endpoint}; reconnecting");
                return ConnEnd::Dropped;
            }
            _ = shutdown.notified() => return ConnEnd::Shutdown,
        }
    }
}

enum ParkEnd {
    Elapsed,
    Shutdown,
}

/// Wait out the reconnect backoff while still servicing commands, config
/// changes, and shutdown. A node_url change cuts the wait short — dialing a
/// NEW node is a user action, not a retry against the failing one.
async fn backoff_park<S: Sink>(
    shared: &super::Shared,
    engine: &mut Engine<S>,
    cmds: &mut UnboundedReceiver<super::Cmd>,
    cmds_open: &mut bool,
    shutdown: &Notify,
    url: &str,
    delay: Duration,
) -> ParkEnd {
    let deadline = Instant::now() + delay;
    loop {
        tokio::select! {
            _ = tokio::time::sleep_until(deadline) => return ParkEnd::Elapsed,
            _ = shared.changed.notified() => {
                if config_url(shared).as_deref() != Some(url) {
                    return ParkEnd::Elapsed;
                }
            }
            cmd = cmds.recv(), if *cmds_open => service_cmd(engine, cmds_open, cmd),
            _ = shutdown.notified() => return ParkEnd::Shutdown,
        }
    }
}

fn service_cmd<S: Sink>(engine: &mut Engine<S>, cmds_open: &mut bool, cmd: Option<super::Cmd>) {
    match cmd {
        Some(super::Cmd::MarkSeen) => engine.mark_seen(),
        None => *cmds_open = false,
    }
}

/// One inbound text frame mapped to the loop's action. Unknown types (a
/// legacy node's `block` frames, `tail`, `subscribed`) and malformed JSON map
/// to [`Mapped::Ignored`] — never a panic, never a reconnect.
#[derive(Debug, PartialEq)]
pub(crate) enum Mapped {
    Event {
        topic: String,
        cursor: String,
        op: Value,
    },
    Lagged {
        topic: String,
        cursor: String,
    },
    Heartbeat {
        interval_ms: u64,
    },
    ErrorFrame {
        topic: String,
        detail: String,
    },
    Ignored,
}

pub(crate) fn map_frame(text: &str) -> Mapped {
    let Ok(frame) = serde_json::from_str::<Value>(text) else {
        return Mapped::Ignored;
    };
    match frame.get("type").and_then(Value::as_str) {
        Some("event") => {
            let Some((topic, cursor)) = topic_cursor(&frame) else {
                return Mapped::Ignored;
            };
            let Some(op) = frame.get("op") else {
                return Mapped::Ignored;
            };
            Mapped::Event {
                topic,
                cursor,
                op: op.clone(),
            }
        }
        Some("lagged") => match topic_cursor(&frame) {
            Some((topic, cursor)) => Mapped::Lagged { topic, cursor },
            None => Mapped::Ignored,
        },
        Some("heartbeat") => Mapped::Heartbeat {
            interval_ms: frame
                .get("intervalMs")
                .and_then(Value::as_u64)
                .unwrap_or(DEFAULT_HEARTBEAT_MS),
        },
        Some("error") => Mapped::ErrorFrame {
            topic: frame
                .get("topic")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            detail: frame
                .get("detail")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        },
        _ => Mapped::Ignored,
    }
}

fn topic_cursor(frame: &Value) -> Option<(String, String)> {
    let topic = frame.get("topic")?.as_str()?.to_string();
    let cursor = frame.get("cursor")?.as_str()?.to_string();
    Some((topic, cursor))
}

/// The one client→server frame we send. An absent resume entry means
/// live-from-tip server-side; we OMIT the `resume` key entirely when there is
/// nothing to resume (the server's `#[serde(default)]` parses omission and
/// `{}` identically — omission is the deliberate choice here).
pub(crate) fn subscribe_text(topics: &[&str], resume: Option<&BTreeMap<String, String>>) -> String {
    let mut frame = serde_json::Map::new();
    frame.insert("op".into(), "subscribe".into());
    frame.insert(
        "topics".into(),
        topics.iter().copied().map(Value::from).collect(),
    );
    if let Some(resume) = resume.filter(|map| !map.is_empty()) {
        frame.insert(
            "resume".into(),
            resume
                .iter()
                .map(|(topic, cursor)| (topic.clone(), Value::from(cursor.as_str())))
                .collect(),
        );
    }
    Value::Object(frame).to_string()
}

/// `http://h:p` → `ws://h:p/v1/ws` (https → wss); tolerates a trailing slash.
pub(crate) fn ws_url(http_base: &str) -> String {
    let base = http_base.trim_end_matches('/');
    if let Some(rest) = base.strip_prefix("https://") {
        format!("wss://{rest}/v1/ws")
    } else if let Some(rest) = base.strip_prefix("http://") {
        format!("ws://{rest}/v1/ws")
    } else if base.starts_with("ws://") || base.starts_with("wss://") {
        format!("{base}/v1/ws")
    } else {
        format!("ws://{base}/v1/ws")
    }
}

fn snapshot_config(shared: &super::Shared) -> super::NotifyConfig {
    shared
        .config
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

fn config_url(shared: &super::Shared) -> Option<String> {
    shared
        .config
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .node_url
        .clone()
}

fn watchdog_timeout(interval: Duration) -> Duration {
    interval * 5 / 2
}

#[cfg(test)]
mod tests {
    use std::io::{Read as _, Write as _};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Mutex as StdMutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::json;
    use tokio_tungstenite::WebSocketStream;

    use super::*;
    use crate::notify::matchers::{Category, Notification};
    use crate::notify::{Cmd, NotifyConfig, Shared};

    // ---- pure mapper ----

    fn op_row() -> Value {
        json!({
            "height": 6,
            "seq": 0,
            "time": 1_720_000_000_u64,
            "origin": { "kind": "external", "id": "cccc" },
            "payload": {
                "post_message": {
                    "channel_id": "general",
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

    #[test]
    fn map_frame_event_carries_the_verbatim_op_row() {
        let text = json!({
            "type": "event",
            "topic": "module:chat",
            "cursor": "op/0000000000000006/0000",
            "op": op_row()
        })
        .to_string();

        match map_frame(&text) {
            Mapped::Event { topic, cursor, op } => {
                assert_eq!(topic, "module:chat");
                assert_eq!(cursor, "op/0000000000000006/0000");
                assert_eq!(op, op_row());
                // the op is exactly what decode::decode_op_row parses.
                let row = crate::notify::decode::decode_op_row(&op).expect("op row decodes");
                assert_eq!(row.origin.id.as_deref(), Some("cccc"));
            }
            other => panic!("expected Event, got {other:?}"),
        }
    }

    #[test]
    fn map_frame_lagged_heartbeat_and_error() {
        assert_eq!(
            map_frame(r#"{"type":"lagged","topic":"module:chat","cursor":"op/00000000000000ff/ffff"}"#),
            Mapped::Lagged {
                topic: "module:chat".into(),
                cursor: "op/00000000000000ff/ffff".into(),
            }
        );
        assert_eq!(
            map_frame(
                r#"{"type":"heartbeat","height":12,"appHash":"ab","timeMs":123,"intervalMs":5000}"#
            ),
            Mapped::Heartbeat { interval_ms: 5000 }
        );
        assert_eq!(
            map_frame(
                r#"{"type":"error","topic":"module:chat","code":"unavailable","detail":"no index store configured"}"#
            ),
            Mapped::ErrorFrame {
                topic: "module:chat".into(),
                detail: "no index store configured".into(),
            }
        );
    }

    #[test]
    fn map_frame_ignores_legacy_unknown_and_malformed_frames() {
        // a legacy node's block frames read as connected-but-dormant.
        assert_eq!(map_frame(r#"{"type":"block","height":3}"#), Mapped::Ignored);
        // feature streams we do not subscribe to.
        assert_eq!(
            map_frame(r#"{"type":"tail","topic":"logs","cursor":"1","item":{"line":"x"}}"#),
            Mapped::Ignored
        );
        assert_eq!(
            map_frame(r#"{"type":"subscribed","topics":{"module:chat":"op/0000000000000000/ffff"}}"#),
            Mapped::Ignored
        );
        // malformed inputs must never panic.
        assert_eq!(map_frame("{nope"), Mapped::Ignored);
        assert_eq!(map_frame(""), Mapped::Ignored);
        assert_eq!(map_frame(r#"{"no":"type"}"#), Mapped::Ignored);
        assert_eq!(map_frame(r#"{"type":"event","topic":"module:chat"}"#), Mapped::Ignored);
        assert_eq!(map_frame(r#"{"type":"event","topic":7,"cursor":"c","op":{}}"#), Mapped::Ignored);
    }

    // ---- subscribe frame ----

    #[test]
    fn subscribe_text_app_start_omits_resume() {
        let frame: Value =
            serde_json::from_str(&subscribe_text(&TOPICS, None)).expect("subscribe json");
        assert_eq!(frame["op"], "subscribe");
        assert_eq!(
            frame["topics"],
            json!([
                "module:chat",
                "module:pages",
                "module:runs",
                "module:forge",
                "module:governance"
            ])
        );
        assert!(
            frame.get("resume").is_none(),
            "app start subscribes live-from-tip with NO resume key: {frame}"
        );

        // an empty map is the same deliberate omission.
        let empty = BTreeMap::new();
        let frame: Value =
            serde_json::from_str(&subscribe_text(&TOPICS, Some(&empty))).expect("subscribe json");
        assert!(frame.get("resume").is_none(), "empty resume is omitted: {frame}");
    }

    #[test]
    fn subscribe_text_reconnect_carries_the_stored_cursors() {
        let cursors = BTreeMap::from([
            ("module:chat".to_string(), "op/0000000000000006/0000".to_string()),
            ("module:runs".to_string(), "op/0000000000000004/0001".to_string()),
        ]);
        let frame: Value =
            serde_json::from_str(&subscribe_text(&TOPICS, Some(&cursors))).expect("subscribe json");
        assert_eq!(frame["op"], "subscribe");
        assert_eq!(
            frame["resume"],
            json!({
                "module:chat": "op/0000000000000006/0000",
                "module:runs": "op/0000000000000004/0001"
            })
        );
    }

    // ---- url derivation ----

    #[test]
    fn ws_url_derives_the_stream_endpoint() {
        assert_eq!(ws_url("http://127.0.0.1:8844"), "ws://127.0.0.1:8844/v1/ws");
        assert_eq!(ws_url("http://127.0.0.1:8844/"), "ws://127.0.0.1:8844/v1/ws");
        assert_eq!(ws_url("https://node.example:8844"), "wss://node.example:8844/v1/ws");
        assert_eq!(ws_url("127.0.0.1:8844"), "ws://127.0.0.1:8844/v1/ws");
    }

    // ---- the loop against a real in-process ws server ----

    static PATH_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestState {
        dir: PathBuf,
    }

    impl TestState {
        fn new() -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_nanos());
            let suffix = PATH_COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "ducktape-notify-stream-{}-{nanos}-{suffix}",
                std::process::id()
            ));
            Self { dir }
        }

        fn path(&self) -> PathBuf {
            self.dir.join("state.json")
        }
    }

    impl Drop for TestState {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    /// Sink whose captures outlive the engine moved into the loop.
    #[derive(Clone, Default)]
    struct CaptureSink {
        presented: Arc<StdMutex<Vec<Notification>>>,
    }

    impl Sink for CaptureSink {
        fn present(&self, notification: &Notification) {
            self.presented.lock().unwrap().push(notification.clone());
        }

        fn badge(&self, _unread: u32) {}
    }

    async fn read_client_json(ws: &mut WebSocketStream<tokio::net::TcpStream>) -> Value {
        loop {
            let msg = ws.next().await.expect("client open").expect("client frame");
            if let Message::Text(text) = msg {
                return serde_json::from_str(&text).expect("client frame is json");
            }
        }
    }

    async fn send_server_json(ws: &mut WebSocketStream<tokio::net::TcpStream>, frame: &Value) {
        ws.send(Message::Text(frame.to_string()))
            .await
            .expect("server send");
    }

    fn shared_for(url: String) -> Arc<Shared> {
        Arc::new(Shared {
            config: StdMutex::new(NotifyConfig {
                node_url: Some(url),
                self_user_key_hex: Some("1234".to_string()),
                ..NotifyConfig::default()
            }),
            changed: Notify::new(),
        })
    }

    #[tokio::test]
    async fn run_loop_presents_mention_and_resumes_from_the_advanced_cursor() {
        tokio::time::timeout(Duration::from_secs(30), async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind");
            let addr = listener.local_addr().expect("addr");
            let (subs_tx, mut subs_rx) = tokio::sync::mpsc::unbounded_channel::<Value>();

            // scripted server: conn 1 delivers a mention then drops; conn 2
            // just reports the resubscribe and holds the socket open.
            let server = tokio::spawn(async move {
                let (tcp, _) = listener.accept().await.expect("accept 1");
                let mut ws = tokio_tungstenite::accept_async(tcp).await.expect("ws 1");
                let sub = read_client_json(&mut ws).await;
                subs_tx.send(sub).expect("report subscribe 1");
                send_server_json(
                    &mut ws,
                    &json!({"type":"subscribed","topics":{"module:chat":"op/0000000000000005/ffff"}}),
                )
                .await;
                send_server_json(
                    &mut ws,
                    &json!({"type":"heartbeat","height":5,"appHash":"aa","timeMs":1,"intervalMs":3000}),
                )
                .await;
                send_server_json(
                    &mut ws,
                    &json!({
                        "type": "event",
                        "topic": "module:chat",
                        "cursor": "op/0000000000000006/0000",
                        "op": op_row()
                    }),
                )
                .await;
                drop(ws); // transient disconnect → same-url reconnect must resume

                let (tcp, _) = listener.accept().await.expect("accept 2");
                let mut ws = tokio_tungstenite::accept_async(tcp).await.expect("ws 2");
                let sub = read_client_json(&mut ws).await;
                subs_tx.send(sub).expect("report subscribe 2");
                // hold the connection open until the client shuts down.
                while matches!(ws.next().await, Some(Ok(_))) {}
            });

            let sink = CaptureSink::default();
            let presented = sink.presented.clone();
            let state = TestState::new();
            let engine = Engine::new(sink, state.path(), Arc::default());
            let shared = shared_for(format!("http://{addr}/"));
            let (_cmds_tx, cmds_rx) = tokio::sync::mpsc::unbounded_channel::<Cmd>();
            let shutdown = Arc::new(Notify::new());
            let loop_task = tokio::spawn(run_loop(shared, engine, cmds_rx, shutdown.clone()));

            // app start: live-from-tip, no resume.
            let sub1 = subs_rx.recv().await.expect("first subscribe");
            assert_eq!(sub1["op"], "subscribe");
            assert_eq!(sub1["topics"], json!(TOPICS));
            assert!(
                sub1.get("resume").is_none(),
                "app start must subscribe with no resume: {sub1}"
            );

            // the reconnect's resume proves the engine cursor advanced.
            let sub2 = subs_rx.recv().await.expect("second subscribe");
            assert_eq!(sub2["op"], "subscribe");
            assert_eq!(sub2["topics"], json!(TOPICS));
            assert_eq!(
                sub2["resume"],
                json!({"module:chat": "op/0000000000000006/0000"}),
                "same-url reconnect resumes from the advanced cursor"
            );

            // exactly one Mention crossed the sink (present ran before the
            // reconnect, so no polling is needed here).
            let notifications = presented.lock().unwrap().clone();
            assert_eq!(notifications.len(), 1, "exactly one notification");
            assert_eq!(notifications[0].category, Category::Mention);
            assert_eq!(notifications[0].channel_id.as_deref(), Some("general"));

            shutdown.notify_one();
            loop_task.await.expect("run_loop returns");
            server.await.expect("server script");
        })
        .await
        .expect("test within deadline");
    }

    // ---- live e2e hook ----

    /// Minimal blocking POST for the live hook — raw std-TCP http/1.1, the
    /// same "any plain http client is a full citizen" idiom as the daemon's
    /// own e2e suite.
    fn http_post(base: &str, path: &str, body: &Value) -> (u16, Value) {
        let authority = base
            .strip_prefix("http://")
            .unwrap_or(base)
            .trim_end_matches('/');
        let mut stream = std::net::TcpStream::connect(authority).expect("daemon reachable");
        stream
            .set_read_timeout(Some(Duration::from_secs(15)))
            .expect("read timeout");
        let bytes = serde_json::to_vec(body).expect("body serializes");
        let head = format!(
            "POST {path} HTTP/1.1\r\nhost: {authority}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            bytes.len()
        );
        stream.write_all(head.as_bytes()).expect("write head");
        stream.write_all(&bytes).expect("write body");
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).expect("read reply");
        let text = String::from_utf8_lossy(&raw);
        let code = text
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let payload = text
            .split("\r\n\r\n")
            .nth(1)
            .and_then(|b| serde_json::from_str(b.trim()).ok())
            .unwrap_or(Value::Null);
        (code, payload)
    }

    /// Live hook against a REAL daemon speaking the typed stream protocol:
    /// `DUCKTAPE_STREAM_E2E_URL=http://127.0.0.1:PORT \
    ///    cargo test -p ducktape-desktop -- --ignored live_stream_e2e`
    #[tokio::test]
    #[ignore = "needs DUCKTAPE_STREAM_E2E_URL pointing at a live daemon"]
    async fn live_stream_e2e() {
        let Ok(base) = std::env::var("DUCKTAPE_STREAM_E2E_URL") else {
            eprintln!("live_stream_e2e: DUCKTAPE_STREAM_E2E_URL unset; skipping");
            return;
        };
        let base = base.trim_end_matches('/').to_string();
        let channel = format!(
            "notify-e2e-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_millis())
        );

        let (code, reply) = http_post(
            &base,
            "/v1/submit",
            &json!({
                "target": "chat",
                "payload": { "create_channel": {
                    "channel_id": channel, "name": "Notify E2E", "post_policy": "open"
                }},
                "origin": "e2e",
            }),
        );
        assert_eq!(code, 200, "create_channel failed: {reply}");

        let sink = CaptureSink::default();
        let presented = sink.presented.clone();
        let state = TestState::new();
        let engine = Engine::new(sink, state.path(), Arc::default());
        let shared = shared_for(base.clone());
        let (_cmds_tx, cmds_rx) = tokio::sync::mpsc::unbounded_channel::<Cmd>();
        let shutdown = Arc::new(Notify::new());
        let loop_task = tokio::spawn(run_loop(shared, engine, cmds_rx, shutdown.clone()));

        // live-from-tip: a post landing BEFORE the subscribe would never be
        // delivered, so give the connection a moment to settle.
        tokio::time::sleep(Duration::from_secs(3)).await;

        // mention my user key ("1234" → bytes [18, 52]) from a DIFFERENT origin.
        let (code, reply) = http_post(
            &base,
            "/v1/submit",
            &json!({
                "target": "chat",
                "payload": { "post_message": {
                    "channel_id": channel,
                    "message_id": format!("{channel}-m1"),
                    "blocks": [{ "paragraph": [{
                        "text": "hey you",
                        "marks": [{ "mention": { "user": [18, 52] } }]
                    }] }],
                    "thread": null,
                    "as_agent": null,
                }},
                "origin": "someone-else",
            }),
        );
        assert_eq!(code, 200, "post_message failed: {reply}");

        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if !presented.lock().unwrap().is_empty() {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "no mention notification within 30s"
            );
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        let notifications = presented.lock().unwrap().clone();
        assert_eq!(notifications.len(), 1, "exactly one notification");
        assert_eq!(notifications[0].category, Category::Mention);
        assert_eq!(notifications[0].channel_id.as_deref(), Some(channel.as_str()));

        shutdown.notify_one();
        loop_task.await.expect("run_loop returns");
    }
}
