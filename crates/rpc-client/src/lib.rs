//! Bounded async client for Ducktape's public node RPC surface.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::time::Duration;

use futures::{SinkExt as _, StreamExt as _};
use reqwest::{Response, Url};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio_tungstenite::tungstenite::Message;

const MAX_JSON_BYTES: usize = 8 * 1024 * 1024;
const MAX_ERROR_BYTES: usize = 4 * 1024;
const TIMEOUT: Duration = Duration::from_secs(30);
const STREAM_IDLE_TIMEOUT: Duration = Duration::from_millis(7_500);

/// A node response or transport failure safe to show to a client user.
#[derive(Debug)]
pub struct Error(String);

impl Error {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<Error> for String {
    fn from(error: Error) -> Self {
        error.0
    }
}

/// Result returned by the RPC client.
pub type Result<T> = std::result::Result<T, Error>;

/// A live module event from the node's resumable `/v1/ws` stream.
#[derive(Clone, Debug, PartialEq)]
pub enum ModuleEvent {
    /// The requested topics are active. Cursors are suitable for reconnect.
    Ready { cursors: BTreeMap<String, String> },
    /// One committed operation applied to a module — the full feed row, so a
    /// follower folds the delta instead of refetching a snapshot.
    Changed {
        module: String,
        cursor: String,
        op: Box<StreamOp>,
    },
    /// Replay history was unavailable; hydrate a fresh snapshot at this cursor.
    Lagged { module: String, cursor: String },
    /// One topic will not deliver, and the node named which and why.
    ///
    /// NOT the connection's failure. The node refuses PER TOPIC and keeps
    /// serving the rest; collapsing that into a stream error took chat and
    /// pages down over a module this node never indexed, then reconnected
    /// forever to be refused again. Only a subscribe-time refusal arrives as
    /// this variant — a mid-session drop is transient (a scan error, a poisoned
    /// index) and still tears the stream down so the reconnect can recover it.
    Refused { module: String, code: String },
    /// The chain head, as the node's heartbeat reports it.
    ///
    /// The heartbeat rides EVERY block wake — nop fillers included, which feed
    /// no topic at all (`bin/noded/src/stream.rs`, the `block_rx` arm) — so this
    /// is the only event that moves a follower's head on a chain whose
    /// subscribed modules are quiet. Folding an op is not a substitute: an idle
    /// chain finalizes nop blocks forever and emits no ops, which used to freeze
    /// the console's head until someone typed.
    ///
    /// It carries the height and NOTHING ELSE on purpose. A read of `/v1/status`
    /// triggered by this event would be a poll wearing a consensus costume: an
    /// idle chain nop-fills once per `BLOCK_TIME` (`bin/node/src/constants.rs`),
    /// so "on every tip" is a 1 Hz timer with extra steps.
    Tip { height: u64 },
}

/// One applied-op feed row as the stream serves it: the block coordinates,
/// the dispatch origin, the op payload verbatim (`payload` when it is JSON,
/// `payload_hex` otherwise), and the module-assigned stamp the module
/// declared while applying it (chat's message seq, an edit's rev) — absent
/// when the op assigned nothing.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct StreamOp {
    pub height: u64,
    /// the block-wide dispatch index.
    pub seq: u32,
    /// the block's consensus time.
    pub time: u64,
    pub origin: StreamOrigin,
    #[serde(default)]
    pub payload: Option<serde_json::Value>,
    #[serde(default)]
    pub payload_hex: Option<String>,
    #[serde(default)]
    pub assigned: Option<serde_json::Value>,
    #[serde(default)]
    pub assigned_hex: Option<String>,
}

/// The dispatch origin of one applied op. External ids arrive rendered as
/// user handles.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct StreamOrigin {
    pub kind: StreamOriginKind,
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StreamOriginKind {
    External,
    Module,
    System,
}

/// One connected module-event session. Reconnect policy belongs to the caller.
pub type ModuleEventStream = futures::stream::BoxStream<'static, Result<ModuleEvent>>;

/// One log-ring line from the `logs` topic, with its resume cursor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogLine {
    pub cursor: String,
    pub line: String,
}

/// The stable portion of `GET /v1/status` needed by public clients.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct Status {
    pub height: u64,
    pub public_key: String,
}

/// An HTTP(S) origin serving Ducktape's `/v1` endpoints.
#[derive(Clone)]
pub struct Client {
    origin: String,
    base: Url,
    http: reqwest::Client,
}

#[derive(Serialize)]
struct QueryRequest<'a, Q> {
    target: &'a str,
    query: &'a Q,
}

#[derive(Serialize)]
struct SubscribeRequest {
    op: &'static str,
    topics: Vec<String>,
    resume: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum StreamFrame {
    Subscribed {
        topics: BTreeMap<String, String>,
    },
    Event {
        topic: String,
        cursor: String,
        op: Box<StreamOp>,
    },
    Lagged {
        topic: String,
        cursor: String,
    },
    Heartbeat {
        height: u64,
    },
    Error {
        topic: String,
        /// the node's stable snake_case refusal token (`unknown_module`,
        /// `topic_not_admitted`, `unavailable`). It has been on the wire all
        /// along and was thrown away here, leaving only a sentence to branch on.
        #[serde(default)]
        code: String,
        detail: String,
    },
}

impl Client {
    /// Build a client from an HTTP(S) origin without credentials, path, or query.
    pub fn new(origin: &str) -> Result<Self> {
        let mut base = Url::parse(origin).map_err(|_| Error::new("RPC endpoint is not a URL"))?;
        let invalid_origin = !matches!(base.scheme(), "http" | "https")
            || base.host_str().is_none()
            || !base.username().is_empty()
            || base.password().is_some()
            || base.query().is_some()
            || base.fragment().is_some()
            || !matches!(base.path(), "" | "/");
        if invalid_origin {
            return Err(Error::new(
                "RPC endpoint must be an http(s) origin without credentials or a path",
            ));
        }
        base.set_path("/");
        let origin = base.as_str().trim_end_matches('/').to_string();
        let http = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .build()
            .map_err(|error| Error::new(format!("could not initialize RPC client: {error}")))?;
        Ok(Self { origin, base, http })
    }

    /// Canonical origin without a trailing slash.
    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// Read the node's current committed height.
    pub async fn status(&self) -> Result<Status> {
        let response = self
            .http
            .get(self.url("v1/status")?)
            .send()
            .await
            .map_err(|error| Error::new(format!("RPC status failed: {error}")))?;
        decode_json(response).await
    }

    /// The whole `GET /v1/status` document, verbatim. [`Status`] is the stable
    /// two-field contract every client can rely on; the operational projection
    /// beside it (`root_hash`, `operations.consensus`, `operations.storage`) is
    /// node-owned and evolves with the daemon, so it is served as JSON rather
    /// than frozen into a type here.
    pub async fn status_json(&self) -> Result<serde_json::Value> {
        let response = self
            .http
            .get(self.url("v1/status")?)
            .send()
            .await
            .map_err(|error| Error::new(format!("RPC status failed: {error}")))?;
        decode_json(response).await
    }

    /// Submit one typed module query and decode its typed reply.
    pub async fn query<Q: Serialize, R: DeserializeOwned>(
        &self,
        target: &str,
        query: &Q,
    ) -> Result<R> {
        let response = self
            .http
            .post(self.url("v1/query")?)
            .json(&QueryRequest { target, query })
            .send()
            .await
            .map_err(|error| Error::new(format!("{target} query failed: {error}")))?;
        decode_json(response).await
    }

    /// Submit one typed index-tier view request and decode its typed reply —
    /// the read model behind every human-facing list, page, and search.
    pub async fn view<Q: Serialize, R: DeserializeOwned>(
        &self,
        module: &str,
        query: &Q,
    ) -> Result<R> {
        let response = self
            .http
            .post(self.url(&format!("v1/index/{module}/view"))?)
            .json(query)
            .send()
            .await
            .map_err(|error| Error::new(format!("{module} view failed: {error}")))?;
        decode_json(response).await
    }

    /// Read the recent block rows (`GET /v1/blocks`), oldest-first — the
    /// explorer surface. Rows are the node's own JSON projection, and they are
    /// NOT all op-carrying: a node following from a checkpoint records its
    /// ascension boundary as a row with an empty `hash` and no `ops`. Filter
    /// for what you present (the app's `explorer_window` does).
    pub async fn blocks(&self, limit: usize) -> Result<Vec<serde_json::Value>> {
        let mut url = self.url("v1/blocks")?;
        url.set_query(Some(&format!("limit={limit}")));
        let response = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|error| Error::new(format!("RPC blocks failed: {error}")))?;
        #[derive(Deserialize)]
        struct Blocks {
            blocks: Vec<serde_json::Value>,
        }
        let reply: Blocks = decode_json(response).await?;
        Ok(reply.blocks)
    }

    /// One GET against a `/v1/files/*` read lane, query-string params, JSON
    /// reply verbatim — the files browser's transport.
    pub async fn files_get(
        &self,
        lane: &str,
        params: &[(&str, &str)],
    ) -> Result<serde_json::Value> {
        let mut url = self.url(&format!("v1/files/{lane}"))?;
        {
            let mut pairs = url.query_pairs_mut();
            for (key, value) in params {
                pairs.append_pair(key, value);
            }
        }
        let response = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|error| Error::new(format!("RPC files {lane} failed: {error}")))?;
        decode_json(response).await
    }

    /// One POST against a `/v1/files/*` write lane with a JSON body — the
    /// files browser's mutation transport (the node encodes + submits the
    /// corresponding `FilesMsg`).
    pub async fn files_post(
        &self,
        lane: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let response = self
            .http
            .post(self.url(&format!("v1/files/{lane}"))?)
            .json(body)
            .send()
            .await
            .map_err(|error| Error::new(format!("RPC files {lane} failed: {error}")))?;
        decode_json(response).await
    }

    /// Stage one duckfs chunk (`POST /v1/files/stage`, raw bytes ≤ 1 MiB) —
    /// returns the staged chunk's digest.
    pub async fn files_stage(&self, bytes: Vec<u8>) -> Result<String> {
        let response = self
            .http
            .post(self.url("v1/files/stage")?)
            .header("content-type", "application/octet-stream")
            .body(bytes)
            .send()
            .await
            .map_err(|error| Error::new(format!("RPC files stage failed: {error}")))?;
        #[derive(Deserialize)]
        struct Staged {
            digest: String,
        }
        let reply: Staged = decode_json(response).await?;
        Ok(reply.digest)
    }

    /// Land raw bytes in the node-local BLOB store (`POST /v1/files/blob`) —
    /// the op-receipt lane forge fetches `PushRefs`/`MergePr` packfiles from by
    /// digest. A distinct plane from [`Self::files_stage`]'s duckfs chunk lane:
    /// a pack staged there would never be found by a `pack_digest` lookup.
    pub async fn put_blob(&self, bytes: Vec<u8>) -> Result<String> {
        let response = self
            .http
            .post(self.url("v1/files/blob")?)
            .header("content-type", "application/octet-stream")
            .body(bytes)
            .send()
            .await
            .map_err(|error| Error::new(format!("RPC blob put failed: {error}")))?;
        #[derive(Deserialize)]
        struct Stored {
            digest: String,
        }
        let reply: Stored = decode_json(response).await?;
        Ok(reply.digest)
    }

    /// Read the peers standing (`GET /v1/peers`), the node's own JSON view.
    pub async fn peers(&self) -> Result<serde_json::Value> {
        let response = self
            .http
            .get(self.url("v1/peers")?)
            .send()
            .await
            .map_err(|error| Error::new(format!("RPC peers failed: {error}")))?;
        decode_json(response).await
    }

    /// Subscribe the node's log ring (`logs` topic): each item is one log
    /// line with its resume cursor. Reconnect policy belongs to the caller.
    pub async fn log_events(
        &self,
        resume: Option<String>,
    ) -> Result<futures::stream::BoxStream<'static, Result<LogLine>>> {
        let mut cursors = BTreeMap::new();
        if let Some(cursor) = resume {
            cursors.insert("logs".to_string(), cursor);
        }
        let subscribe = serde_json::to_string(&SubscribeRequest {
            op: "subscribe",
            topics: vec!["logs".to_string()],
            resume: cursors,
        })
        .map_err(|error| Error::new(format!("could not encode log subscription: {error}")))?;
        let url = self.stream_url()?;
        let (mut socket, _) = tokio::time::timeout(TIMEOUT, tokio_tungstenite::connect_async(&url))
            .await
            .map_err(|_| Error::new("RPC stream connection timed out"))?
            .map_err(|error| Error::new(format!("RPC stream connection failed: {error}")))?;
        tokio::time::timeout(TIMEOUT, socket.send(Message::Text(subscribe)))
            .await
            .map_err(|_| Error::new("RPC stream subscription timed out"))?
            .map_err(|error| Error::new(format!("RPC stream subscription failed: {error}")))?;
        let stream = futures::stream::unfold(Some(socket), move |socket| async move {
            let mut socket = socket?;
            loop {
                let message = match tokio::time::timeout(STREAM_IDLE_TIMEOUT, socket.next()).await {
                    Ok(Some(message)) => message,
                    Ok(None) => return Some((Err(Error::new("RPC stream closed")), None)),
                    Err(_) => {
                        return Some((Err(Error::new("RPC stream heartbeat timed out")), None));
                    }
                };
                let Ok(Message::Text(text)) = message else {
                    if matches!(message, Ok(Message::Close(_)) | Err(_)) {
                        return Some((Err(Error::new("RPC stream closed")), None));
                    }
                    continue;
                };
                #[derive(Deserialize)]
                #[serde(tag = "type", rename_all = "snake_case")]
                enum LogFrame {
                    Subscribed {},
                    Tail {
                        cursor: String,
                        item: serde_json::Value,
                    },
                    Heartbeat,
                    #[serde(other)]
                    Other,
                }
                match serde_json::from_str::<LogFrame>(&text) {
                    Ok(LogFrame::Tail { cursor, item }) => {
                        let line = item["line"].as_str().unwrap_or_default().to_string();
                        if line.is_empty() {
                            continue;
                        }
                        return Some((Ok(LogLine { cursor, line }), Some(socket)));
                    }
                    Ok(_) => continue,
                    Err(_) => continue,
                }
            }
        })
        .boxed();
        Ok(stream)
    }

    /// Submit an already-signed operation frame.
    pub async fn submit_frame(&self, frame: Vec<u8>) -> Result<()> {
        let response = self
            .http
            .post(self.url("v1/submit/frame")?)
            .header("content-type", "application/octet-stream")
            .body(frame)
            .send()
            .await
            .map_err(|error| Error::new(format!("transaction submission failed: {error}")))?;
        if response.status().is_success() {
            return Ok(());
        }
        Err(response_error(response).await)
    }

    /// Connect to the node stream and subscribe to committed module changes.
    pub async fn module_events(
        &self,
        modules: Vec<String>,
        resume: BTreeMap<String, String>,
    ) -> Result<ModuleEventStream> {
        if modules.is_empty() {
            return Err(Error::new("at least one module stream is required"));
        }
        let topics = modules
            .into_iter()
            .map(|module| format!("module:{module}"))
            .collect::<Vec<_>>();
        let expected = topics.iter().cloned().collect::<BTreeSet<_>>();
        let subscribe = serde_json::to_string(&SubscribeRequest {
            op: "subscribe",
            topics,
            resume,
        })
        .map_err(|error| Error::new(format!("could not encode stream subscription: {error}")))?;
        let url = self.stream_url()?;
        let (mut socket, _) = tokio::time::timeout(TIMEOUT, tokio_tungstenite::connect_async(&url))
            .await
            .map_err(|_| Error::new("RPC stream connection timed out"))?
            .map_err(|error| Error::new(format!("RPC stream connection failed: {error}")))?;
        tokio::time::timeout(TIMEOUT, socket.send(Message::Text(subscribe)))
            .await
            .map_err(|_| Error::new("RPC stream subscription timed out"))?
            .map_err(|error| Error::new(format!("RPC stream subscription failed: {error}")))?;
        Ok(module_event_stream(socket, expected, STREAM_IDLE_TIMEOUT))
    }

    fn url(&self, path: &str) -> Result<Url> {
        self.base
            .join(path)
            .map_err(|_| Error::new("could not build RPC URL"))
    }

    fn stream_url(&self) -> Result<String> {
        let mut url = self.url("v1/ws")?;
        let scheme = if url.scheme() == "https" { "wss" } else { "ws" };
        url.set_scheme(scheme)
            .map_err(|_| Error::new("could not build RPC stream URL"))?;
        Ok(url.to_string())
    }
}

fn module_event_stream<S>(
    socket: S,
    expected: BTreeSet<String>,
    idle_timeout: Duration,
) -> ModuleEventStream
where
    S: futures::Stream<Item = std::result::Result<Message, tokio_tungstenite::tungstenite::Error>>
        + Send
        + Unpin
        + 'static,
{
    // `subscribed` is what separates a topic that never started from one that
    // stopped: the node refuses at subscribe time and reports later trouble the
    // same way, and only the clock tells them apart.
    futures::stream::unfold(Some((socket, expected, false)), move |state| async move {
        let (mut socket, expected, mut subscribed) = state?;
        loop {
            let message = match tokio::time::timeout(idle_timeout, socket.next()).await {
                Ok(Some(message)) => message,
                Ok(None) => return Some((Err(Error::new("RPC stream closed")), None)),
                Err(_) => {
                    return Some((Err(Error::new("RPC stream heartbeat timed out")), None));
                }
            };
            if let Some(event) = decode_stream_message(message, &expected, subscribed) {
                subscribed = subscribed || matches!(event, Ok(ModuleEvent::Ready { .. }));
                return Some((event, Some((socket, expected, subscribed))));
            }
        }
    })
    .boxed()
}

fn decode_stream_message(
    message: std::result::Result<Message, tokio_tungstenite::tungstenite::Error>,
    expected: &BTreeSet<String>,
    subscribed: bool,
) -> Option<Result<ModuleEvent>> {
    let message = match message {
        Ok(message) => message,
        Err(error) => return Some(Err(Error::new(format!("RPC stream failed: {error}")))),
    };
    let Message::Text(text) = message else {
        return match message {
            Message::Close(_) => Some(Err(Error::new("RPC stream closed"))),
            Message::Binary(_) => Some(Err(Error::new("RPC stream returned binary data"))),
            _ => None,
        };
    };
    if text.len() > MAX_JSON_BYTES {
        return Some(Err(Error::new("RPC stream frame exceeds the client limit")));
    }
    let frame = match serde_json::from_str::<StreamFrame>(&text) {
        Ok(frame) => frame,
        Err(error) => {
            return Some(Err(Error::new(format!(
                "RPC stream returned invalid JSON: {error}"
            ))));
        }
    };
    match frame {
        StreamFrame::Subscribed { topics } => {
            // ADMITTING NONE IS THE CONNECTION FAILING; admitting some is one
            // plane degrading. Anything refused already arrived as its own
            // `Error` frame ahead of this one, so the caller has been told
            // which and why by the time this lands.
            if topics.is_empty() {
                return Some(Err(Error::new("RPC stream admitted no requested topic")));
            }
            Some(Ok(ModuleEvent::Ready { cursors: topics }))
        }
        StreamFrame::Event { topic, cursor, op } => Some(
            module_name(&topic, expected).map(|module| ModuleEvent::Changed { module, cursor, op }),
        ),
        StreamFrame::Lagged { topic, cursor } => {
            Some(module_name(&topic, expected).map(|module| ModuleEvent::Lagged { module, cursor }))
        }
        // BEFORE `subscribed`, this topic never started and the others did:
        // degrade that plane and keep the connection. AFTER, it is a scan error
        // or a poisoned index — transient, and tearing down is what lets the
        // reconnect recover it. Same frame, two meanings, told apart by when.
        StreamFrame::Error {
            topic,
            code,
            detail,
        } => match subscribed {
            false => Some(
                module_name(&topic, expected).map(|module| ModuleEvent::Refused { module, code }),
            ),
            true => Some(Err(Error::new(format!(
                "RPC stream topic {topic} failed: {detail}"
            )))),
        },
        StreamFrame::Heartbeat { height } => Some(Ok(ModuleEvent::Tip { height })),
    }
}

fn module_name(topic: &str, expected: &BTreeSet<String>) -> Result<String> {
    if !expected.contains(topic) {
        return Err(Error::new("RPC stream returned an unexpected topic"));
    }
    topic
        .strip_prefix("module:")
        .map(str::to_string)
        .ok_or_else(|| Error::new("RPC stream returned an invalid module topic"))
}

async fn decode_json<T: DeserializeOwned>(response: Response) -> Result<T> {
    let status = response.status();
    let bytes = read_bounded(response, MAX_JSON_BYTES).await?;
    if !status.is_success() {
        return Err(Error::new(format!(
            "RPC returned {status}: {}",
            bounded_detail(&String::from_utf8_lossy(&bytes))
        )));
    }
    serde_json::from_slice(&bytes)
        .map_err(|error| Error::new(format!("RPC returned invalid JSON: {error}")))
}

async fn response_error(response: Response) -> Error {
    let status = response.status();
    match read_bounded(response, MAX_ERROR_BYTES).await {
        Ok(bytes) => Error::new(format!(
            "transaction was rejected ({status}): {}",
            bounded_detail(&String::from_utf8_lossy(&bytes))
        )),
        Err(error) => Error::new(format!("transaction was rejected ({status}): {error}")),
    }
}

async fn read_bounded(response: Response, limit: usize) -> Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(Error::new("RPC response exceeds the client limit"));
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk =
            chunk.map_err(|error| Error::new(format!("could not read RPC response: {error}")))?;
        if bytes.len().saturating_add(chunk.len()) > limit {
            return Err(Error::new("RPC response exceeds the client limit"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn bounded_detail(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        return "no detail".into();
    }
    value.chars().take(300).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_bare_http_origins() {
        let client = Client::new("http://127.0.0.1:8844/").unwrap();
        assert_eq!(client.origin(), "http://127.0.0.1:8844");

        for invalid in [
            "not-a-url",
            "ftp://127.0.0.1",
            "http://user@127.0.0.1",
            "http://127.0.0.1/path",
            "http://127.0.0.1?query",
            "http://127.0.0.1#fragment",
        ] {
            assert!(Client::new(invalid).is_err(), "accepted {invalid}");
        }
    }

    #[tokio::test]
    async fn rejects_view_path_injection_before_transport() {
        let client = Client::new("http://127.0.0.1:1").unwrap();
        assert!(
            client
                .view::<_, serde_json::Value>("../status", &())
                .await
                .is_err()
        );
    }

    /// THE HEARTBEAT CARRIES THE HEAD, AND IT USED TO BE DROPPED HERE.
    ///
    /// `StreamFrame::Heartbeat` was a UNIT variant, so `height` never survived
    /// deserialization and the frame decoded to nothing. A follower's head then
    /// moved only when one of its subscribed modules committed an op — and an
    /// idle chain nop-fills forever without feeding any topic, so the console
    /// sat on a frozen block number until someone typed.
    ///
    /// The frame below is the node's own, verbatim: `bin/noded/tests/daemon_e2e.rs`
    /// pins that it serves `type`/`height`/`interval_ms`, so this test fails if
    /// either side moves. `root_hash`, `time_ms` and `interval_ms` are not
    /// decoded — the tip's whole contract is the height.
    #[test]
    fn a_heartbeat_decodes_to_the_head_it_carries() {
        let expected = BTreeSet::from(["module:chat".to_string()]);
        let heartbeat = r#"{"type":"heartbeat","height":41,"root_hash":"ab","time_ms":1752000000000,"interval_ms":3000}"#;
        let tip = decode_stream_message(Ok(Message::Text(heartbeat.into())), &expected, true)
            .expect("a heartbeat is an event, not a skipped frame")
            .expect("a heartbeat is not an error");
        assert_eq!(tip, ModuleEvent::Tip { height: 41 });
    }

    /// ONE REFUSED TOPIC MUST NOT TAKE THE OTHERS DOWN.
    ///
    /// The node answers PER TOPIC — a refusal is its own `Error` frame ahead of
    /// `Subscribed`, and the admitted topics still carry cursors. This required
    /// EVERY requested topic to come back or it errored the whole stream, and
    /// the app's reconnect loop then re-asked forever and was refused again —
    /// so subscribing to one module a node does not index took chat and pages
    /// dark for the life of the process.
    #[test]
    fn a_refused_topic_degrades_its_plane_and_spares_the_rest() {
        let expected = BTreeSet::from(["module:chat".to_string(), "module:valset".to_string()]);

        // subscribe time: the node names the topic and the reason.
        let refusal = r#"{"type":"error","topic":"module:valset","code":"unknown_module","detail":"this node indexes no such module"}"#;
        let event = decode_stream_message(Ok(Message::Text(refusal.into())), &expected, false)
            .expect("a refusal is an event")
            .expect("a refusal is not the connection failing");
        assert_eq!(
            event,
            ModuleEvent::Refused {
                module: "valset".into(),
                code: "unknown_module".into(),
            },
            "the TYPED code, not the sentence — a reason is greppable or it is prose"
        );

        // and the rest of the subscription still starts.
        let subscribed = r#"{"type":"subscribed","topics":{"module:chat":"c1"}}"#;
        let ready = decode_stream_message(Ok(Message::Text(subscribed.into())), &expected, false)
            .unwrap()
            .expect("a partial subscription is not a failure");
        assert_eq!(
            ready,
            ModuleEvent::Ready {
                cursors: BTreeMap::from([("module:chat".into(), "c1".into())]),
            }
        );

        // admitting NOTHING is the connection failing, not a plane degrading.
        let none = r#"{"type":"subscribed","topics":{}}"#;
        assert!(
            decode_stream_message(Ok(Message::Text(none.into())), &expected, false)
                .unwrap()
                .is_err()
        );

        // the SAME frame after `subscribed` is a live topic dropping out — a
        // scan error or a poisoned index. Transient, and the reconnect is what
        // recovers it, so it must still tear the stream down.
        assert!(
            decode_stream_message(Ok(Message::Text(refusal.into())), &expected, true)
                .unwrap()
                .is_err(),
            "a mid-session drop must not be mistaken for a permanent refusal"
        );
    }

    #[test]
    fn decodes_only_subscribed_module_topics() {
        let expected = BTreeSet::from(["module:chat".to_string(), "module:pages".to_string()]);
        let subscribed =
            r#"{"type":"subscribed","topics":{"module:chat":"c1","module:pages":"p1"}}"#;
        let ready = decode_stream_message(Ok(Message::Text(subscribed.into())), &expected, false)
            .unwrap()
            .unwrap();
        assert_eq!(
            ready,
            ModuleEvent::Ready {
                cursors: BTreeMap::from([
                    ("module:chat".into(), "c1".into()),
                    ("module:pages".into(), "p1".into()),
                ]),
            }
        );

        let unexpected =
            r#"{"type":"event","topic":"module:files","cursor":"f1","op":{"height":7}}"#;
        assert!(
            decode_stream_message(Ok(Message::Text(unexpected.into())), &expected, true)
                .unwrap()
                .is_err()
        );

        let unknown = r#"{"type":"retired_frame"}"#;
        assert!(
            decode_stream_message(Ok(Message::Text(unknown.into())), &expected, true)
                .unwrap()
                .is_err()
        );
    }

    #[test]
    fn status_requires_the_current_shape() {
        assert!(
            serde_json::from_str::<Status>(r#"{"height":7}"#).is_err(),
            "a status without public_key must not receive a removed default"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn silent_module_stream_times_out() {
        let socket = futures::stream::pending::<
            std::result::Result<Message, tokio_tungstenite::tungstenite::Error>,
        >();
        let expected = BTreeSet::from(["module:chat".to_string()]);
        let mut stream = module_event_stream(socket, expected, STREAM_IDLE_TIMEOUT);
        let waiting = tokio::spawn(async move { stream.next().await.unwrap() });
        tokio::task::yield_now().await;
        tokio::time::advance(STREAM_IDLE_TIMEOUT).await;

        let error = waiting.await.unwrap().unwrap_err();
        assert_eq!(error.to_string(), "RPC stream heartbeat timed out");
    }
}
