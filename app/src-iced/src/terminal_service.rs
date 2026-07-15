//! Trusted lifecycle and bidirectional wire adapter for native terminals.
//!
//! A worker owns one node-created session and the one websocket that both
//! subscribes to its output and sends its input. The UI sees bounded events;
//! terminal capabilities and node URLs never cross into `screens::terminal`.

use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::time::Duration;

use base64::Engine as _;
use futures_util::{SinkExt as _, StreamExt as _};
use reqwest::{Client, Response, Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;

use crate::transport::NodeClient;

const AGENT: &str = "codex";
const COMMAND_QUEUE: usize = 64;
const EVENT_QUEUE: usize = 256;
const MAX_INPUT_BYTES: usize = 64 * 1024;
const MAX_OUTPUT_BYTES: usize = 256 * 1024;
const MAX_JSON_BYTES: usize = 384 * 1024;
const MAX_ID_BYTES: usize = 128;
const MAX_COLS: u16 = 500;
const MAX_ROWS: u16 = 300;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
const WRITE_TIMEOUT: Duration = Duration::from_millis(500);
const STREAM_WATCHDOG: Duration = Duration::from_millis(7_500);
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Connected { generation: u64 },
    Reconnecting { generation: u64, detail: String },
    Output { generation: u64, bytes: Vec<u8> },
    Failed { generation: u64, detail: String },
}

#[derive(Debug)]
enum Command {
    Input(Vec<u8>),
    Resize { cols: u16, rows: u16 },
}

pub struct Handle {
    generation: u64,
    commands: mpsc::Sender<Command>,
    stop: watch::Sender<bool>,
    events: Receiver<Event>,
    worker: Option<JoinHandle<()>>,
}

impl Handle {
    pub fn start(client: NodeClient, generation: u64) -> Self {
        let (commands, command_rx) = mpsc::channel(COMMAND_QUEUE);
        let (events_tx, events) = sync_channel(EVENT_QUEUE);
        let (stop, stop_rx) = watch::channel(false);
        let worker = tokio::spawn(run(
            client.origin(),
            generation,
            command_rx,
            events_tx,
            stop_rx,
        ));
        Self {
            generation,
            commands,
            stop,
            events,
            worker: Some(worker),
        }
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub fn send_input(&self, bytes: Vec<u8>) -> bool {
        !bytes.is_empty()
            && bytes.len() <= MAX_INPUT_BYTES
            && self.commands.try_send(Command::Input(bytes)).is_ok()
    }

    pub fn resize(&self, cols: u16, rows: u16) -> bool {
        cols > 0
            && rows > 0
            && cols <= MAX_COLS
            && rows <= MAX_ROWS
            && self
                .commands
                .try_send(Command::Resize { cols, rows })
                .is_ok()
    }

    pub fn take_events(&self) -> Vec<Event> {
        let mut events = Vec::new();
        while let Ok(event) = self.events.try_recv() {
            events.push(event);
        }
        events
    }

    pub fn request_stop(&self) {
        let _ = self.stop.send(true);
    }

    pub fn is_stopped(&self) -> bool {
        self.worker.as_ref().is_none_or(JoinHandle::is_finished)
    }

    pub async fn shutdown(mut self) {
        self.request_stop();
        if let Some(worker) = self.worker.take() {
            let _ = tokio::time::timeout(Duration::from_secs(3), worker).await;
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
        // Dropping a JoinHandle detaches it. The stop watch remains alive in
        // the worker long enough to perform the one idempotent HTTP close.
        self.worker.take();
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatedSession {
    session_id: String,
    topic: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
enum ClientFrame<'a> {
    Subscribe {
        topics: [&'a str; 1],
        resume: std::collections::BTreeMap<&'a str, &'a str>,
    },
    TermInput {
        session: &'a str,
        data: String,
    },
    TermResize {
        session: &'a str,
        cols: u16,
        rows: u16,
    },
}

#[derive(Debug, PartialEq, Eq)]
enum WireEvent {
    Subscribed,
    Output { cursor: String, bytes: Vec<u8> },
    Lagged,
    Refused(String),
    Ignore,
}

enum ConnectionEnd {
    Stopped,
    Reconnect(String),
    Fatal(String),
}

async fn run(
    origin: String,
    generation: u64,
    mut commands: mpsc::Receiver<Command>,
    events: SyncSender<Event>,
    mut stop: watch::Receiver<bool>,
) {
    let http = Client::new();
    let mut session = match create_session(&http, &origin, &mut stop).await {
        Ok(Some(session)) => Some(session),
        Ok(None) => return,
        Err(detail) => {
            let _ = send_event(&events, &mut stop, Event::Failed { generation, detail }).await;
            return;
        }
    };
    let mut cursor = String::new();
    let mut size = None;
    let mut attempts = 0u32;
    let mut fatal = None;

    while !*stop.borrow() {
        let current = session
            .as_ref()
            .expect("session exists until the close path");
        match connect_once(
            &origin,
            current,
            generation,
            &mut cursor,
            &mut size,
            &mut attempts,
            &mut commands,
            &events,
            &mut stop,
        )
        .await
        {
            ConnectionEnd::Stopped => break,
            ConnectionEnd::Fatal(detail) => {
                fatal = Some(detail);
                break;
            }
            ConnectionEnd::Reconnect(detail) => {
                if *stop.borrow() {
                    break;
                }
                attempts = attempts.saturating_add(1);
                if !send_event(
                    &events,
                    &mut stop,
                    Event::Reconnecting { generation, detail },
                )
                .await
                {
                    break;
                }
                drain_disconnected_commands(&mut commands, &mut size);
                let backoff = Duration::from_secs(2u64.pow(attempts.min(4)));
                tokio::select! {
                    _ = tokio::time::sleep(backoff) => {}
                    changed = stop.changed() => {
                        if changed.is_err() || *stop.borrow() {
                            break;
                        }
                    }
                }
            }
        }
    }

    // Ownership is consumed here, so every created session has one and only
    // one close attempt regardless of which exit path reached teardown.
    if let Some(session) = session.take() {
        close_session(&http, &origin, &session.session_id).await;
    }
    if let Some(detail) = fatal {
        let _ = send_event(&events, &mut stop, Event::Failed { generation, detail }).await;
    }
}

#[allow(clippy::too_many_arguments)]
async fn connect_once(
    origin: &str,
    session: &CreatedSession,
    generation: u64,
    cursor: &mut String,
    size: &mut Option<(u16, u16)>,
    attempts: &mut u32,
    commands: &mut mpsc::Receiver<Command>,
    events: &SyncSender<Event>,
    stop: &mut watch::Receiver<bool>,
) -> ConnectionEnd {
    let websocket = match websocket_url(origin) {
        Ok(url) => url,
        Err(detail) => return ConnectionEnd::Fatal(detail),
    };
    let socket = tokio::select! {
        result = tokio::time::timeout(CONNECT_TIMEOUT, tokio_tungstenite::connect_async(websocket.as_str())) => {
            match result {
                Ok(Ok((socket, _))) => socket,
                Ok(Err(_)) => return ConnectionEnd::Reconnect("could not reach the terminal stream".into()),
                Err(_) => return ConnectionEnd::Reconnect("terminal stream connection timed out".into()),
            }
        }
        changed = stop.changed() => {
            let _ = changed;
            return ConnectionEnd::Stopped;
        }
    };
    let (mut sink, mut source) = socket.split();
    let mut resume = std::collections::BTreeMap::new();
    if !cursor.is_empty() {
        resume.insert(session.topic.as_str(), cursor.as_str());
    }
    let subscribe = ClientFrame::Subscribe {
        topics: [session.topic.as_str()],
        resume,
    };
    if send_frame(&mut sink, &subscribe, stop).await.is_err() {
        return ConnectionEnd::Reconnect("could not subscribe to the terminal stream".into());
    }

    let mut subscribed = false;
    let watchdog = tokio::time::sleep(STREAM_WATCHDOG);
    tokio::pin!(watchdog);
    loop {
        tokio::select! {
            () = &mut watchdog => {
                return ConnectionEnd::Reconnect("terminal stream heartbeat timed out".into());
            }
            changed = stop.changed() => {
                let _ = changed;
                return ConnectionEnd::Stopped;
            }
            command = commands.recv(), if subscribed => {
                let Some(command) = command else {
                    return ConnectionEnd::Stopped;
                };
                let frame = match command {
                    Command::Input(bytes) => ClientFrame::TermInput {
                        session: &session.session_id,
                        data: base64::engine::general_purpose::STANDARD.encode(bytes),
                    },
                    Command::Resize { cols, rows } => {
                        *size = Some((cols, rows));
                        ClientFrame::TermResize { session: &session.session_id, cols, rows }
                    }
                };
                if send_frame(&mut sink, &frame, stop).await.is_err() {
                    return ConnectionEnd::Reconnect("terminal stream write failed".into());
                }
            }
            message = source.next() => {
                let Some(message) = message else {
                    return ConnectionEnd::Reconnect("terminal stream closed".into());
                };
                let message = match message {
                    Ok(message) => message,
                    Err(_) => return ConnectionEnd::Reconnect("terminal stream failed".into()),
                };
                watchdog.as_mut().reset(tokio::time::Instant::now() + STREAM_WATCHDOG);
                let Message::Text(text) = message else {
                    if matches!(message, Message::Close(_)) {
                        return ConnectionEnd::Reconnect("terminal stream closed".into());
                    }
                    continue;
                };
                if text.len() > MAX_JSON_BYTES {
                    return ConnectionEnd::Fatal("terminal stream sent an oversized frame".into());
                }
                match parse_wire_event(&text, &session.topic) {
                    Ok(WireEvent::Subscribed) if !subscribed => {
                        // Commands can arrive while DNS/connect/subscribe is
                        // pending. Discard their Input before declaring Live;
                        // only the newest geometry is safe to carry forward.
                        drain_disconnected_commands(commands, size);
                        *attempts = 0;
                        subscribed = true;
                        if let Some((cols, rows)) = *size {
                            let resize = ClientFrame::TermResize {
                                session: &session.session_id,
                                cols,
                                rows,
                            };
                            if send_frame(&mut sink, &resize, stop).await.is_err() {
                                return ConnectionEnd::Reconnect("terminal resize failed".into());
                            }
                        }
                        if !send_event(events, stop, Event::Connected { generation }).await {
                            return ConnectionEnd::Stopped;
                        }
                    }
                    Ok(WireEvent::Output { cursor: next, bytes }) if subscribed => {
                        if !send_event(events, stop, Event::Output { generation, bytes }).await {
                            return ConnectionEnd::Stopped;
                        }
                        *cursor = next;
                    }
                    Ok(WireEvent::Lagged) => return ConnectionEnd::Fatal(
                        "Terminal output history expired; restart the session.".into(),
                    ),
                    Ok(WireEvent::Refused(detail)) => return ConnectionEnd::Fatal(detail),
                    Ok(WireEvent::Subscribed | WireEvent::Output { .. } | WireEvent::Ignore) => {}
                    Err(detail) => return ConnectionEnd::Fatal(detail),
                }
            }
        }
    }
}

async fn send_frame<S, T>(
    sink: &mut S,
    frame: &T,
    stop: &mut watch::Receiver<bool>,
) -> Result<(), ()>
where
    S: futures_util::Sink<Message> + Unpin,
    T: Serialize,
{
    let text = serde_json::to_string(frame).map_err(|_| ())?;
    tokio::select! {
        result = tokio::time::timeout(WRITE_TIMEOUT, sink.send(Message::Text(text))) => {
            result.map_err(|_| ())?.map_err(|_| ())
        }
        changed = stop.changed() => {
            let _ = changed;
            Err(())
        }
    }
}

async fn send_event(
    sender: &SyncSender<Event>,
    stop: &mut watch::Receiver<bool>,
    mut event: Event,
) -> bool {
    loop {
        match sender.try_send(event) {
            Ok(()) => return true,
            Err(TrySendError::Disconnected(_)) => return false,
            Err(TrySendError::Full(pending)) => event = pending,
        }
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(5)) => {}
            changed = stop.changed() => {
                if changed.is_err() || *stop.borrow() {
                    return false;
                }
            }
        }
    }
}

fn drain_disconnected_commands(
    commands: &mut mpsc::Receiver<Command>,
    size: &mut Option<(u16, u16)>,
) {
    while let Ok(command) = commands.try_recv() {
        if let Command::Resize { cols, rows } = command {
            *size = Some((cols, rows));
        }
        // Input is deliberately discarded while disconnected. Replaying old
        // keystrokes into a freshly redrawn TUI is more dangerous than loss.
    }
}

fn parse_wire_event(text: &str, topic: &str) -> Result<WireEvent, String> {
    let value: Value = serde_json::from_str(text)
        .map_err(|_| "terminal stream sent an invalid JSON frame".to_string())?;
    let Some(object) = value.as_object() else {
        return Err("terminal stream sent an invalid frame".into());
    };
    match object.get("type").and_then(Value::as_str) {
        Some("subscribed") => {
            let accepted = object
                .get("topics")
                .and_then(Value::as_object)
                .is_some_and(|topics| topics.contains_key(topic));
            if accepted {
                Ok(WireEvent::Subscribed)
            } else {
                Err("the node did not accept the terminal subscription".into())
            }
        }
        Some("event") if object.get("topic").and_then(Value::as_str) == Some(topic) => {
            let cursor = object
                .get("cursor")
                .and_then(Value::as_str)
                .filter(|cursor| {
                    !cursor.is_empty()
                        && cursor.len() <= 32
                        && cursor.bytes().all(|byte| byte.is_ascii_digit())
                })
                .ok_or_else(|| "terminal stream sent an invalid cursor".to_string())?;
            let item = object
                .get("item")
                .and_then(Value::as_str)
                .ok_or_else(|| "terminal stream sent an invalid output chunk".to_string())?;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(item)
                .map_err(|_| "terminal stream sent invalid output encoding".to_string())?;
            if bytes.len() > MAX_OUTPUT_BYTES {
                return Err("terminal stream sent an oversized output chunk".into());
            }
            Ok(WireEvent::Output {
                cursor: cursor.to_owned(),
                bytes,
            })
        }
        Some("lagged") if object.get("topic").and_then(Value::as_str) == Some(topic) => {
            Ok(WireEvent::Lagged)
        }
        Some("error") if object.get("topic").and_then(Value::as_str) == Some(topic) => {
            let detail = object
                .get("detail")
                .and_then(Value::as_str)
                .unwrap_or("the node refused the terminal stream");
            Ok(WireEvent::Refused(bounded_detail(detail)))
        }
        _ => Ok(WireEvent::Ignore),
    }
}

async fn create_session(
    http: &Client,
    origin: &str,
    stop: &mut watch::Receiver<bool>,
) -> Result<Option<CreatedSession>, String> {
    let url = endpoint(origin, "v1/term/sessions")?;
    let request = http
        .post(url)
        .timeout(HTTP_TIMEOUT)
        .json(&serde_json::json!({ "agent": AGENT }));
    let response = tokio::select! {
        result = request.send() => result.map_err(|_| "could not create a terminal session".to_string())?,
        changed = stop.changed() => {
            let _ = changed;
            return Ok(None);
        }
    };
    let status = response.status();
    let bytes = read_bounded(response, MAX_JSON_BYTES).await?;
    if !status.is_success() {
        return Err(http_error(status.as_u16(), &bytes));
    }
    let created: CreatedSession = serde_json::from_slice(&bytes)
        .map_err(|_| "the node returned an invalid terminal session".to_string())?;
    if !safe_id(&created.session_id)
        || created.topic != format!("term:{}", created.session_id)
        || created.topic.len() > MAX_ID_BYTES + 5
    {
        return Err("the node returned an unsafe terminal session identifier".into());
    }
    Ok(Some(created))
}

async fn close_session(http: &Client, origin: &str, session_id: &str) {
    let Ok(url) = endpoint(origin, &format!("v1/term/sessions/{session_id}/close")) else {
        return;
    };
    let result = http.post(url).timeout(Duration::from_secs(2)).send().await;
    if !result
        .as_ref()
        .is_ok_and(|response| response.status().is_success())
    {
        tracing::warn!(
            target: "ducktape::term",
            reason = "close_failed",
            "terminal session close did not complete cleanly"
        );
    }
}

async fn read_bounded(mut response: Response, limit: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "could not read the terminal response".to_string())?
    {
        if bytes.len().saturating_add(chunk.len()) > limit {
            return Err("the terminal response exceeded its size limit".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn http_error(status: u16, bytes: &[u8]) -> String {
    let detail = serde_json::from_slice::<Value>(bytes)
        .ok()
        .and_then(|value| value.get("error")?.as_str().map(str::to_owned))
        .unwrap_or_else(|| format!("node replied {status}"));
    bounded_detail(&detail)
}

fn bounded_detail(detail: &str) -> String {
    detail
        .replace(['\r', '\n'], " ")
        .chars()
        .take(300)
        .collect::<String>()
}

fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_BYTES
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn endpoint(origin: &str, path: &str) -> Result<Url, String> {
    Url::parse(origin)
        .and_then(|base| base.join(path))
        .map_err(|_| "invalid terminal endpoint".to_string())
}

fn websocket_url(origin: &str) -> Result<Url, String> {
    let mut url = endpoint(origin, "v1/ws")?;
    let scheme = match url.scheme() {
        "http" => "ws",
        "https" => "wss",
        _ => return Err("invalid terminal endpoint scheme".into()),
    };
    url.set_scheme(scheme)
        .map_err(|_| "invalid terminal stream endpoint".to_string())?;
    Ok(url)
}

#[cfg(test)]
mod tests {
    use std::pin::Pin;
    use std::task::{Context, Poll};

    use super::*;

    struct PendingSink;

    impl futures_util::Sink<Message> for PendingSink {
        type Error = ();

        fn poll_ready(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Pending
        }

        fn start_send(self: Pin<&mut Self>, _item: Message) -> Result<(), Self::Error> {
            unreachable!("a permanently pending sink never accepts a frame")
        }

        fn poll_flush(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Pending
        }

        fn poll_close(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn stop_interrupts_a_pending_websocket_write() {
        let (stop_tx, mut stop) = watch::channel(false);
        let mut sink = PendingSink;
        let frame = serde_json::json!({ "op": "subscribe" });
        let write = send_frame(&mut sink, &frame, &mut stop);
        let trigger = async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            stop_tx.send(true).unwrap();
        };
        let started = tokio::time::Instant::now();
        let (result, ()) = tokio::join!(write, trigger);

        assert!(result.is_err());
        assert!(started.elapsed() < WRITE_TIMEOUT);
    }

    #[test]
    fn output_frame_is_bounded_decoded_and_cursor_preserving() {
        let topic = "term:s1";
        let event = parse_wire_event(
            r#"{"type":"event","topic":"term:s1","cursor":"42","item":"7Jik66as"}"#,
            topic,
        )
        .unwrap();
        assert_eq!(
            event,
            WireEvent::Output {
                cursor: "42".into(),
                bytes: "오리".as_bytes().to_vec(),
            }
        );
        assert!(
            parse_wire_event(r#"{"type":"event","topic":"term:s1","item":"eA=="}"#, topic,)
                .is_err()
        );
    }

    #[test]
    fn lagged_terminal_output_is_fatal_not_replayable() {
        assert_eq!(
            parse_wire_event(
                r#"{"type":"lagged","topic":"term:s1","cursor":"9"}"#,
                "term:s1",
            )
            .unwrap(),
            WireEvent::Lagged
        );
    }

    #[test]
    fn session_ids_and_geometry_are_closed_and_bounded() {
        assert!(safe_id("019b-session_1"));
        assert!(!safe_id("../session"));
        assert!(!safe_id(&"x".repeat(MAX_ID_BYTES + 1)));
        assert!(
            websocket_url("http://127.0.0.1:9000")
                .unwrap()
                .as_str()
                .starts_with("ws://")
        );
    }

    #[test]
    fn disconnected_input_is_dropped_but_latest_resize_is_retained() {
        let (sender, mut receiver) = mpsc::channel(3);
        sender.try_send(Command::Input(b"stale".to_vec())).unwrap();
        sender
            .try_send(Command::Resize { cols: 80, rows: 24 })
            .unwrap();
        sender
            .try_send(Command::Resize {
                cols: 100,
                rows: 32,
            })
            .unwrap();
        let mut size = None;
        drain_disconnected_commands(&mut receiver, &mut size);
        assert_eq!(size, Some((100, 32)));
        assert!(matches!(
            receiver.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }
}
