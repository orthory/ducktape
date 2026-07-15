//! Typed client for the daemon HTTP/WebSocket surface.
//!
//! The desktop shell manages a local node process, then talks to it over the
//! same bounded wire API as every other Ducktape client. No UI code reaches
//! into node internals.

use std::collections::BTreeMap;
use std::fmt;
use std::time::Duration;

use base64::Engine as _;
use futures_util::{SinkExt as _, StreamExt as _};
use reqwest::{Client, Response, StatusCode, Url};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

pub use crate::view_api::SubmitReceipt;

const STATUS_TIMEOUT: Duration = Duration::from_secs(6);
const CALL_TIMEOUT: Duration = Duration::from_secs(30);
const STREAM_WATCHDOG: Duration = Duration::from_millis(7_500);
const MAX_ERROR_BYTES: usize = 300;
const MAX_ERROR_WIRE_BYTES: usize = 4 * 1024;
const MAX_JSON_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const MAX_BLOB_DOWNLOAD_BYTES: usize = 4 * 1024 * 1024;
const MAX_TOPICS: usize = 64;
const MAX_FILE_PATH_BYTES: usize = 4 * 1024;
const MAX_FILE_LIST_ENTRIES: usize = 4_096;
const MAX_FILE_PREVIEW_BYTES: usize = 1024 * 1024;
const MAX_METRICS_BYTES: usize = 4 * 1024 * 1024;
const MAX_GATEWAY_PROXY_BODY_BYTES: usize = 8 * 1024 * 1024;
const MAX_GATEWAY_PROXY_WIRE_BYTES: usize = 2 * 1024 * 1024;
const MAX_QUERY_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Refused,
    Timeout,
    Http,
    BadBody,
    InvalidUrl,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeError {
    pub kind: ErrorKind,
    pub status: Option<u16>,
    detail: String,
}

impl NodeError {
    fn new(kind: ErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            status: None,
            detail: detail.into(),
        }
    }

    fn http(status: StatusCode, detail: String) -> Self {
        Self {
            kind: ErrorKind::Http,
            status: Some(status.as_u16()),
            detail,
        }
    }
}

impl fmt::Display for NodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}

impl std::error::Error for NodeError {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModuleStatus {
    pub id: String,
    pub root: String,
    #[serde(default)]
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NodeStatus {
    pub version: String,
    pub app_hash: String,
    pub height: u64,
    #[serde(default)]
    pub modules: Vec<ModuleStatus>,
    #[serde(default)]
    pub public_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileEntry {
    pub path: String,
    pub kind: String,
    pub size: u64,
    pub exec: bool,
    pub object: String,
    #[serde(default)]
    pub meta: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileSnapshot {
    pub id: String,
    pub parent: Option<String>,
    pub root_tree: String,
    pub author: String,
    pub height: u64,
    pub consensus_time: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileRefs {
    pub head: Option<String>,
    #[serde(default)]
    pub pins: BTreeMap<String, String>,
    pub window_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricsSample {
    pub time_ms: u64,
    pub text: String,
}

#[derive(Debug, Deserialize)]
struct FilePage {
    #[serde(default)]
    entries: Vec<FileEntry>,
    next: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FileRange {
    b64: String,
    eof: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BlockRecord {
    pub height: u64,
    pub hash: String,
    pub commit_hash: String,
    #[serde(default)]
    pub ops: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ServerFrame {
    Subscribed {
        topics: BTreeMap<String, Option<String>>,
    },
    Event {
        topic: String,
        cursor: String,
        op: Value,
    },
    Tail {
        topic: String,
        cursor: String,
        item: Value,
    },
    Lagged {
        topic: String,
        cursor: String,
    },
    Heartbeat {
        height: u64,
        app_hash: String,
        time_ms: u64,
        interval_ms: u64,
    },
    Error {
        topic: String,
        code: String,
        detail: String,
    },
}

impl ServerFrame {
    fn cursor(&self) -> Option<(&str, &str)> {
        match self {
            Self::Event { topic, cursor, .. }
            | Self::Tail { topic, cursor, .. }
            | Self::Lagged { topic, cursor } => Some((topic, cursor)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum StreamEvent {
    Connected,
    Frame(ServerFrame),
    Disconnected(String),
}

#[derive(Debug, Serialize)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
enum ClientFrame<'a> {
    Subscribe {
        topics: &'a [String],
        resume: &'a BTreeMap<String, String>,
    },
}

#[derive(Debug, Clone)]
pub struct NodeClient {
    base: Url,
    http: Client,
}

impl NodeClient {
    pub fn local(port: u16) -> Result<Self, NodeError> {
        Self::new(&format!("http://127.0.0.1:{port}"))
    }

    pub fn new(base: &str) -> Result<Self, NodeError> {
        let mut base = Url::parse(base)
            .map_err(|_| NodeError::new(ErrorKind::InvalidUrl, "invalid node address"))?;
        if !matches!(base.scheme(), "http" | "https")
            || base.host_str().is_none()
            || !base.username().is_empty()
            || base.password().is_some()
            || base.query().is_some()
            || base.fragment().is_some()
        {
            return Err(NodeError::new(
                ErrorKind::InvalidUrl,
                "node address must be an http(s) origin without credentials",
            ));
        }
        base.set_path("/");
        let http = Client::builder()
            .build()
            .map_err(|_| NodeError::new(ErrorKind::Refused, "could not initialize node client"))?;
        Ok(Self { base, http })
    }

    /// Canonical HTTP(S) origin used by the remote Forge mirror.
    pub(crate) fn origin(&self) -> String {
        self.base.as_str().trim_end_matches('/').to_owned()
    }

    pub async fn status(&self) -> Result<NodeStatus, NodeError> {
        let status: NodeStatus = self.get_json("v1/status", STATUS_TIMEOUT).await?;
        if status.version.is_empty() || status.app_hash.is_empty() {
            return Err(NodeError::new(
                ErrorKind::BadBody,
                "the process answering this port is not a Ducktape node",
            ));
        }
        Ok(status)
    }

    pub async fn submit(
        &self,
        target: &str,
        payload: Value,
        origin: Option<&str>,
    ) -> Result<SubmitReceipt, NodeError> {
        self.post_json(
            "v1/submit",
            &serde_json::json!({ "target": target, "payload": payload, "origin": origin }),
        )
        .await
    }

    pub async fn submit_frame(&self, frame: Vec<u8>) -> Result<SubmitReceipt, NodeError> {
        let response = self
            .send(
                self.http
                    .post(self.url("v1/submit/frame")?)
                    .header("content-type", "application/octet-stream")
                    .body(frame),
                CALL_TIMEOUT,
            )
            .await?;
        self.decode(response).await
    }

    pub async fn query(&self, target: &str, query: Value) -> Result<Value, NodeError> {
        self.post_json(
            "v1/query",
            &serde_json::json!({ "target": target, "query": query }),
        )
        .await
    }

    pub(crate) async fn query_bounded(
        &self,
        target: &str,
        query: Value,
        max_bytes: usize,
    ) -> Result<Value, NodeError> {
        let response = self
            .send(
                self.http
                    .post(self.url("v1/query")?)
                    .json(&serde_json::json!({
                        "target": target,
                        "query": query
                    })),
                CALL_TIMEOUT,
            )
            .await?;
        let response = checked(response).await?;
        let bytes = read_bounded(
            response,
            max_bytes.min(MAX_QUERY_RESPONSE_BYTES),
            "module query response",
        )
        .await?;
        serde_json::from_slice(&bytes)
            .map_err(|_| NodeError::new(ErrorKind::BadBody, "node returned an invalid response"))
    }

    pub async fn view(&self, module: &str, request: Value) -> Result<Value, NodeError> {
        let module = safe_segment(module)?;
        self.post_json(&format!("v1/index/{module}/view"), &request)
            .await
    }

    pub async fn blocks(&self, limit: Option<usize>) -> Result<Vec<BlockRecord>, NodeError> {
        #[derive(Deserialize)]
        struct Reply {
            #[serde(default)]
            blocks: Vec<BlockRecord>,
        }
        let mut url = self.url("v1/blocks")?;
        if let Some(limit) = limit {
            url.query_pairs_mut()
                .append_pair("limit", &limit.min(4_096).to_string());
        }
        let response = self.send(self.http.get(url), STATUS_TIMEOUT).await?;
        Ok(self.decode::<Reply>(response).await?.blocks)
    }

    pub async fn put_blob(&self, bytes: Vec<u8>) -> Result<String, NodeError> {
        #[derive(Deserialize)]
        struct Reply {
            digest: String,
        }
        let response = self
            .send(
                self.http
                    .post(self.url("v1/files/blob")?)
                    .header("content-type", "application/octet-stream")
                    .body(bytes),
                CALL_TIMEOUT,
            )
            .await?;
        Ok(self.decode::<Reply>(response).await?.digest)
    }

    /// Read the node's bounded Prometheus/OpenMetrics exposition.
    pub async fn metrics_text(&self) -> Result<String, NodeError> {
        let response = self
            .send(self.http.get(self.url("metrics")?), STATUS_TIMEOUT)
            .await?;
        let bytes = read_bounded(
            checked(response).await?,
            MAX_METRICS_BYTES,
            "metrics exposition",
        )
        .await?;
        String::from_utf8(bytes)
            .map_err(|_| NodeError::new(ErrorKind::BadBody, "metrics exposition is not UTF-8"))
    }

    /// Decode one `metrics` WebSocket tail item. The same parser is used for
    /// endpoint and stream samples so their trust boundary stays identical.
    pub fn metrics_tail(item: &Value) -> Result<MetricsSample, NodeError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Wire {
            time_ms: u64,
            text: String,
        }
        let wire: Wire = serde_json::from_value(item.clone())
            .map_err(|_| NodeError::new(ErrorKind::BadBody, "metrics stream item is invalid"))?;
        if wire.text.len() > MAX_METRICS_BYTES {
            return Err(NodeError::new(
                ErrorKind::BadBody,
                "metrics stream item exceeds the desktop safety limit",
            ));
        }
        Ok(MetricsSample {
            time_ms: wire.time_ms,
            text: wire.text,
        })
    }

    /// Exercise one finalized route through the authenticated gateway proxy.
    /// Callers build the credential-free request head from the signed record.
    pub async fn gateway_proxy_status(&self, head: Value) -> Result<u16, NodeError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct ResponseHead {
            status: u16,
            #[serde(default)]
            headers: Vec<Value>,
            #[serde(default)]
            body_len: u64,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Reply {
            head: ResponseHead,
            body_b64: String,
        }
        let response = self
            .send(
                self.http
                    .post(self.url("v1/gateway/proxy")?)
                    .json(&serde_json::json!({ "head": head, "bodyB64": "" })),
                CALL_TIMEOUT,
            )
            .await?;
        let bytes = read_bounded(
            checked(response).await?,
            MAX_GATEWAY_PROXY_WIRE_BYTES,
            "gateway proxy response",
        )
        .await?;
        let reply: Reply = serde_json::from_slice(&bytes).map_err(|_| {
            NodeError::new(ErrorKind::BadBody, "gateway proxy returned invalid JSON")
        })?;
        if reply.head.status < 100
            || reply.head.status > 599
            || reply.head.headers.len() > 256
            || reply.head.body_len > MAX_GATEWAY_PROXY_BODY_BYTES as u64
            || reply.body_b64.len() > MAX_GATEWAY_PROXY_BODY_BYTES.saturating_mul(4).div_ceil(3) + 4
        {
            return Err(NodeError::new(
                ErrorKind::BadBody,
                "gateway proxy returned an invalid response head",
            ));
        }
        Ok(reply.head.status)
    }

    /// Stage one DuckFS chunk and return the node-verified SHA-256 digest.
    pub async fn files_stage(&self, bytes: Vec<u8>) -> Result<String, NodeError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Reply {
            digest: String,
        }
        if bytes.len() > 1024 * 1024 {
            return Err(NodeError::new(
                ErrorKind::BadBody,
                "DuckFS stage exceeds the chunk limit",
            ));
        }
        let response = self
            .send(
                self.http
                    .post(self.url("v1/files/stage")?)
                    .header("content-type", "application/octet-stream")
                    .body(bytes),
                CALL_TIMEOUT,
            )
            .await?;
        let reply: Reply = self.decode(response).await?;
        safe_digest(&reply.digest)?;
        Ok(reply.digest)
    }

    pub async fn get_blob(&self, digest: &str) -> Result<Vec<u8>, NodeError> {
        let digest = safe_digest(digest)?;
        let response = self
            .send(
                self.http.get(self.url(&format!("v1/files/blob/{digest}"))?),
                CALL_TIMEOUT,
            )
            .await?;
        let response = checked(response).await?;
        read_bounded(response, MAX_BLOB_DOWNLOAD_BYTES, "blob response").await
    }

    pub async fn files_ls(
        &self,
        path: &str,
        snapshot: Option<&str>,
    ) -> Result<Vec<FileEntry>, NodeError> {
        let path = safe_file_path(path)?;
        if let Some(snapshot) = snapshot {
            safe_digest(snapshot)?;
        }
        let mut after: Option<String> = None;
        let mut entries = Vec::new();
        loop {
            let mut url = self.url("v1/files/ls")?;
            {
                let mut query = url.query_pairs_mut();
                query.append_pair("path", path);
                query.append_pair("limit", "256");
                if let Some(snapshot) = snapshot {
                    query.append_pair("snapshot", snapshot);
                }
                if let Some(after) = &after {
                    query.append_pair("after", after);
                }
            }
            let response = self.send(self.http.get(url), CALL_TIMEOUT).await?;
            let page: FilePage = self.decode(response).await?;
            if entries.len().saturating_add(page.entries.len()) > MAX_FILE_LIST_ENTRIES {
                return Err(NodeError::new(
                    ErrorKind::BadBody,
                    "directory listing exceeds the desktop safety limit",
                ));
            }
            entries.extend(page.entries);
            match page.next {
                Some(next) if after.as_ref() != Some(&next) && !next.is_empty() => {
                    after = Some(next)
                }
                Some(_) => {
                    return Err(NodeError::new(
                        ErrorKind::BadBody,
                        "directory listing returned a stalled cursor",
                    ));
                }
                None => return Ok(entries),
            }
        }
    }

    pub async fn files_stat(
        &self,
        path: &str,
        snapshot: Option<&str>,
    ) -> Result<Option<FileEntry>, NodeError> {
        let path = safe_file_path(path)?;
        if let Some(snapshot) = snapshot {
            safe_digest(snapshot)?;
        }
        let mut url = self.url("v1/files/stat")?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("path", path);
            if let Some(snapshot) = snapshot {
                query.append_pair("snapshot", snapshot);
            }
        }
        let response = self.send(self.http.get(url), CALL_TIMEOUT).await?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        self.decode(response).await.map(Some)
    }

    pub async fn files_preview(
        &self,
        path: &str,
        snapshot: Option<&str>,
    ) -> Result<(Vec<u8>, bool), NodeError> {
        let path = safe_file_path(path)?;
        if let Some(snapshot) = snapshot {
            safe_digest(snapshot)?;
        }
        let mut url = self.url("v1/files/read")?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("path", path);
            query.append_pair("offset", "0");
            query.append_pair("len", &MAX_FILE_PREVIEW_BYTES.to_string());
            if let Some(snapshot) = snapshot {
                query.append_pair("snapshot", snapshot);
            }
        }
        let response = self.send(self.http.get(url), CALL_TIMEOUT).await?;
        let range: FileRange = self.decode(response).await?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(range.b64)
            .map_err(|_| NodeError::new(ErrorKind::BadBody, "file body is not valid base64"))?;
        if bytes.len() > MAX_FILE_PREVIEW_BYTES {
            return Err(NodeError::new(
                ErrorKind::BadBody,
                "file preview exceeds the desktop safety limit",
            ));
        }
        Ok((bytes, range.eof))
    }

    /// Read one exact DuckFS file from a pinned snapshot, page by page.
    pub async fn files_read_exact(
        &self,
        path: &str,
        snapshot: &str,
        size: u64,
    ) -> Result<Vec<u8>, NodeError> {
        let path = safe_file_path(path)?;
        safe_digest(snapshot)?;
        if size > 64 * 1024 * 1024 {
            return Err(NodeError::new(
                ErrorKind::BadBody,
                "DuckFS file exceeds the gateway file limit",
            ));
        }
        if size == 0 {
            return Ok(Vec::new());
        }
        let mut bytes = Vec::with_capacity(size as usize);
        let mut eof = false;
        while !eof && bytes.len() < size as usize {
            let mut url = self.url("v1/files/read")?;
            {
                let mut query = url.query_pairs_mut();
                query.append_pair("path", path);
                query.append_pair("snapshot", snapshot);
                query.append_pair("offset", &bytes.len().to_string());
                query.append_pair("len", &MAX_FILE_PREVIEW_BYTES.to_string());
            }
            let response = self.send(self.http.get(url), CALL_TIMEOUT).await?;
            let range: FileRange = self.decode(response).await?;
            let page = base64::engine::general_purpose::STANDARD
                .decode(range.b64)
                .map_err(|_| NodeError::new(ErrorKind::BadBody, "file body is not valid base64"))?;
            if page.is_empty() && !range.eof {
                return Err(NodeError::new(ErrorKind::BadBody, "DuckFS read stalled"));
            }
            if bytes.len().saturating_add(page.len()) > size as usize {
                return Err(NodeError::new(
                    ErrorKind::BadBody,
                    "DuckFS file grew while reading",
                ));
            }
            bytes.extend(page);
            eof = range.eof;
        }
        if !eof || bytes.len() != size as usize {
            return Err(NodeError::new(
                ErrorKind::BadBody,
                "DuckFS file changed while reading",
            ));
        }
        Ok(bytes)
    }

    pub async fn files_history(&self, limit: usize) -> Result<Vec<FileSnapshot>, NodeError> {
        #[derive(Deserialize)]
        struct Reply {
            #[serde(default)]
            snapshots: Vec<FileSnapshot>,
        }
        let mut url = self.url("v1/files/history")?;
        url.query_pairs_mut()
            .append_pair("limit", &limit.clamp(1, 256).to_string());
        let response = self.send(self.http.get(url), CALL_TIMEOUT).await?;
        Ok(self.decode::<Reply>(response).await?.snapshots)
    }

    pub async fn files_refs(&self) -> Result<FileRefs, NodeError> {
        let value = self
            .query("files", serde_json::json!({ "refs": {} }))
            .await?;
        serde_json::from_value(
            value
                .get("refs")
                .cloned()
                .ok_or_else(|| NodeError::new(ErrorKind::BadBody, "files refs reply is missing"))?,
        )
        .map_err(|_| NodeError::new(ErrorKind::BadBody, "files refs reply is invalid"))
    }

    pub async fn files_commit(&self, body: Value) -> Result<SubmitReceipt, NodeError> {
        self.post_json("v1/files/commit", &body).await
    }

    pub async fn gateway_browser_base(&self) -> Result<String, NodeError> {
        #[derive(Deserialize)]
        struct Reply {
            base: String,
        }
        let reply: Reply = self.get_json("v1/gateway/browser", STATUS_TIMEOUT).await?;
        let url = Url::parse(&reply.base).map_err(|_| {
            NodeError::new(
                ErrorKind::BadBody,
                "gateway returned an invalid browser base",
            )
        })?;
        if url.scheme() != "http"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || !url.host_str().is_some_and(|host| {
                host.parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
            })
            || url.port().is_none_or(|port| port == 0)
        {
            return Err(NodeError::new(
                ErrorKind::BadBody,
                "gateway browser base is not a loopback HTTP origin",
            ));
        }
        Ok(reply.base.trim_end_matches('/').to_string())
    }

    /// Spawn a reconnecting subscription. Dropping the receiver cancels it.
    pub fn subscribe(
        &self,
        topics: Vec<String>,
        resume: BTreeMap<String, String>,
    ) -> Result<mpsc::Receiver<StreamEvent>, NodeError> {
        let mut topics = topics;
        topics.sort();
        topics.dedup();
        if topics.is_empty()
            || topics.len() > MAX_TOPICS
            || topics.iter().any(|topic| !safe_topic(topic))
        {
            return Err(NodeError::new(
                ErrorKind::BadBody,
                "stream topics are empty, unsafe, or exceed the connection limit",
            ));
        }
        let (sender, receiver) = mpsc::channel(256);
        let client = self.clone();
        tokio::spawn(async move {
            let mut resume = resume;
            let mut attempts = 0u32;
            loop {
                attempts = attempts.saturating_add(1);
                if attempts == 1 || attempts % 10 == 0 {
                    tracing::debug!(
                        target: "ducktape::shell",
                        attempts,
                        event = "node_stream_connecting",
                        "connecting to node stream"
                    );
                }
                let mut connected = false;
                let result = client
                    .stream_once(&topics, &mut resume, &sender, &mut connected)
                    .await;
                if sender.is_closed() {
                    break;
                }
                // A session that reached Connected was healthy; a later drop
                // should reconnect quickly, not inherit the escalated backoff
                // from earlier failures (which would pin it at the 16s cap).
                if connected {
                    attempts = 0;
                }
                let reason = result
                    .err()
                    .map(|error| error.to_string())
                    .unwrap_or_else(|| "node stream closed".into());
                if sender
                    .send(StreamEvent::Disconnected(reason))
                    .await
                    .is_err()
                {
                    break;
                }
                let backoff = Duration::from_secs(2u64.pow(attempts.min(4)));
                tokio::select! {
                    () = sender.closed() => break,
                    () = tokio::time::sleep(backoff) => {}
                }
            }
        });
        Ok(receiver)
    }

    async fn stream_once(
        &self,
        topics: &[String],
        resume: &mut BTreeMap<String, String>,
        sender: &mpsc::Sender<StreamEvent>,
        connected: &mut bool,
    ) -> Result<(), NodeError> {
        let url = self.websocket_url()?;
        let (socket, _) = tokio_tungstenite::connect_async(url.as_str())
            .await
            .map_err(|_| NodeError::new(ErrorKind::Refused, "could not reach the node stream"))?;
        let (mut sink, mut source) = socket.split();
        let subscribe = serde_json::to_string(&ClientFrame::Subscribe { topics, resume })
            .map_err(|_| NodeError::new(ErrorKind::BadBody, "could not encode subscription"))?;
        sink.send(Message::Text(subscribe.into()))
            .await
            .map_err(|_| {
                NodeError::new(ErrorKind::Refused, "could not subscribe to node stream")
            })?;
        sender
            .send(StreamEvent::Connected)
            .await
            .map_err(|_| NodeError::new(ErrorKind::Refused, "stream receiver closed"))?;
        *connected = true;

        let mut watchdog = STREAM_WATCHDOG;
        loop {
            let next = tokio::select! {
                () = sender.closed() => return Ok(()),
                next = tokio::time::timeout(watchdog, source.next()) => next,
            }
            .map_err(|_| NodeError::new(ErrorKind::Timeout, "node stream heartbeat timed out"))?;
            let message = next
                .ok_or_else(|| NodeError::new(ErrorKind::Refused, "node stream closed"))?
                .map_err(|_| NodeError::new(ErrorKind::Refused, "node stream failed"))?;
            let Message::Text(text) = message else {
                continue;
            };
            let frame: ServerFrame = serde_json::from_str(&text).map_err(|_| {
                NodeError::new(ErrorKind::BadBody, "node stream sent an invalid frame")
            })?;
            if let Some((topic, cursor)) = frame.cursor() {
                resume.insert(topic.to_owned(), cursor.to_owned());
            }
            if let ServerFrame::Heartbeat { interval_ms, .. } = &frame {
                watchdog = Duration::from_millis(interval_ms.saturating_mul(5).div_ceil(2))
                    .max(STREAM_WATCHDOG);
            }
            if sender.send(StreamEvent::Frame(frame)).await.is_err() {
                return Ok(());
            }
        }
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        timeout: Duration,
    ) -> Result<T, NodeError> {
        let response = self.send(self.http.get(self.url(path)?), timeout).await?;
        self.decode(response).await
    }

    async fn post_json<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, NodeError> {
        let response = self
            .send(self.http.post(self.url(path)?).json(body), CALL_TIMEOUT)
            .await?;
        self.decode(response).await
    }

    async fn send(
        &self,
        request: reqwest::RequestBuilder,
        timeout: Duration,
    ) -> Result<Response, NodeError> {
        request
            .timeout(timeout)
            .send()
            .await
            .map_err(classify_reqwest)
    }

    async fn decode<T: DeserializeOwned>(&self, response: Response) -> Result<T, NodeError> {
        let response = checked(response).await?;
        let bytes = read_bounded(response, MAX_JSON_RESPONSE_BYTES, "JSON response").await?;
        serde_json::from_slice(&bytes)
            .map_err(|_| NodeError::new(ErrorKind::BadBody, "node returned an invalid response"))
    }

    fn url(&self, path: &str) -> Result<Url, NodeError> {
        self.base
            .join(path)
            .map_err(|_| NodeError::new(ErrorKind::InvalidUrl, "invalid node endpoint"))
    }

    pub(crate) fn cache_key(&self) -> String {
        self.base.as_str().to_owned()
    }

    fn websocket_url(&self) -> Result<Url, NodeError> {
        let mut url = self.url("v1/ws")?;
        url.set_scheme(if self.base.scheme() == "https" {
            "wss"
        } else {
            "ws"
        })
        .map_err(|_| NodeError::new(ErrorKind::InvalidUrl, "invalid node stream endpoint"))?;
        Ok(url)
    }
}

fn classify_reqwest(error: reqwest::Error) -> NodeError {
    if error.is_timeout() {
        NodeError::new(
            ErrorKind::Timeout,
            "the node did not answer before the deadline",
        )
    } else {
        NodeError::new(ErrorKind::Refused, "could not reach the node")
    }
}

async fn checked(response: Response) -> Result<Response, NodeError> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let bytes = read_bounded(response, MAX_ERROR_WIRE_BYTES, "error response")
        .await
        .unwrap_or_default();
    let text = String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_ERROR_BYTES)]);
    let detail = serde_json::from_slice::<Value>(&bytes)
        .ok()
        .and_then(|body| body.get("error")?.as_str().map(str::to_owned))
        .unwrap_or_else(|| text.into_owned());
    let detail = sanitize_error_detail(&detail);
    Err(NodeError::http(
        status,
        if detail.is_empty() {
            format!("node replied {status}")
        } else {
            detail
        },
    ))
}

fn sanitize_error_detail(detail: &str) -> String {
    detail
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .take(MAX_ERROR_BYTES)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

async fn read_bounded(
    mut response: Response,
    max: usize,
    label: &'static str,
) -> Result<Vec<u8>, NodeError> {
    if response
        .content_length()
        .is_some_and(|length| length > max as u64)
    {
        return Err(NodeError::new(
            ErrorKind::BadBody,
            format!("{label} exceeds the desktop safety limit"),
        ));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(classify_reqwest)? {
        if bytes.len().saturating_add(chunk.len()) > max {
            return Err(NodeError::new(
                ErrorKind::BadBody,
                format!("{label} exceeds the desktop safety limit"),
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn safe_segment(value: &str) -> Result<&str, NodeError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(NodeError::new(ErrorKind::BadBody, "unsafe module id"));
    }
    Ok(value)
}

fn safe_digest(value: &str) -> Result<&str, NodeError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(NodeError::new(ErrorKind::BadBody, "invalid blob digest"));
    }
    Ok(value)
}

fn safe_topic(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 192
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'))
}

fn safe_file_path(value: &str) -> Result<&str, NodeError> {
    if !value.starts_with('/')
        || value.len() > MAX_FILE_PATH_BYTES
        || value
            .bytes()
            .any(|byte| byte == 0 || byte == b'\r' || byte == b'\n')
        || value.split('/').any(|part| matches!(part, "." | ".."))
    {
        return Err(NodeError::new(ErrorKind::BadBody, "unsafe file path"));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_origin_is_normalized_and_credentials_are_rejected() {
        let client = NodeClient::new("http://127.0.0.1:2022/surprise").unwrap();
        assert_eq!(
            client.url("v1/status").unwrap().as_str(),
            "http://127.0.0.1:2022/v1/status"
        );
        assert!(NodeClient::new("http://secret@example.test").is_err());
        assert!(NodeClient::new("file:///tmp/socket").is_err());
    }

    #[test]
    fn wire_segments_are_bounded() {
        assert!(safe_segment("chat").is_ok());
        assert!(safe_segment("../chat").is_err());
        assert!(safe_digest(&"a".repeat(64)).is_ok());
        assert!(safe_digest("not-a-digest").is_err());
        assert!(safe_topic("module:chat"));
        assert!(!safe_topic("module/chat"));
    }

    #[test]
    fn file_paths_are_absolute_and_cannot_escape() {
        assert_eq!(
            safe_file_path("/shared/plan.md").unwrap(),
            "/shared/plan.md"
        );
        assert!(safe_file_path("shared/plan.md").is_err());
        assert!(safe_file_path("/shared/../secret").is_err());
        assert!(safe_file_path("/shared\nsecret").is_err());
    }

    #[tokio::test]
    async fn exact_read_accepts_an_empty_file_without_a_range_request() {
        let client = NodeClient::new("http://127.0.0.1:1").unwrap();
        let bytes = client
            .files_read_exact("/shared/empty.html", &"a".repeat(64), 0)
            .await
            .unwrap();
        assert!(bytes.is_empty());
    }

    #[test]
    fn metrics_tail_uses_the_bounded_endpoint_sample_shape() {
        let sample = NodeClient::metrics_tail(&serde_json::json!({
            "timeMs": 42,
            "text": "ducktape_block_height 7\n"
        }))
        .unwrap();
        assert_eq!(sample.time_ms, 42);
        assert_eq!(sample.text, "ducktape_block_height 7\n");
        assert!(
            NodeClient::metrics_tail(&serde_json::json!({
                "timeMs": 42,
                "text": "x".repeat(MAX_METRICS_BYTES + 1)
            }))
            .is_err()
        );
    }

    #[test]
    fn error_details_are_single_line_and_bounded() {
        let detail = sanitize_error_detail(&format!("first\r\nsecond\0{}", "x".repeat(500)));
        assert!(!detail.contains(['\r', '\n', '\0']));
        assert!(detail.chars().count() <= MAX_ERROR_BYTES);
        assert!(detail.starts_with("first second"));
    }
}
