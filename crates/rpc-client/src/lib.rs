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
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModuleEvent {
    /// The requested topics are active. Cursors are suitable for reconnect.
    Ready { cursors: BTreeMap<String, String> },
    /// One committed operation changed a module.
    Changed {
        module: String,
        cursor: String,
        height: u64,
    },
    /// Replay history was unavailable; hydrate a fresh snapshot at this cursor.
    Lagged { module: String, cursor: String },
}

/// One connected module-event session. Reconnect policy belongs to the caller.
pub type ModuleEventStream = futures::stream::BoxStream<'static, Result<ModuleEvent>>;

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
        topics: BTreeMap<String, Option<String>>,
    },
    Event {
        topic: String,
        cursor: String,
        op: StreamOp,
    },
    Lagged {
        topic: String,
        cursor: String,
    },
    Heartbeat,
    Error {
        topic: String,
        detail: String,
    },
}

#[derive(Deserialize)]
struct StreamOp {
    height: u64,
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

    /// Query one module's derived materialized view, such as Chat/Pages search.
    pub async fn view<Q: Serialize, R: DeserializeOwned>(
        &self,
        module: &str,
        query: &Q,
    ) -> Result<R> {
        let valid_module = !module.is_empty()
            && module
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
        if !valid_module {
            return Err(Error::new("view module id is invalid"));
        }
        let response = self
            .http
            .post(self.url(&format!("v1/index/{module}/view"))?)
            .json(query)
            .send()
            .await
            .map_err(|error| Error::new(format!("{module} view query failed: {error}")))?;
        decode_json(response).await
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
    futures::stream::unfold(Some((socket, expected)), move |state| async move {
        let (mut socket, expected) = state?;
        loop {
            let message = match tokio::time::timeout(idle_timeout, socket.next()).await {
                Ok(Some(message)) => message,
                Ok(None) => return Some((Err(Error::new("RPC stream closed")), None)),
                Err(_) => {
                    return Some((Err(Error::new("RPC stream heartbeat timed out")), None));
                }
            };
            if let Some(event) = decode_stream_message(message, &expected) {
                return Some((event, Some((socket, expected))));
            }
        }
    })
    .boxed()
}

fn decode_stream_message(
    message: std::result::Result<Message, tokio_tungstenite::tungstenite::Error>,
    expected: &BTreeSet<String>,
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
            let complete = expected
                .iter()
                .all(|topic| topics.get(topic).is_some_and(|cursor| cursor.is_some()));
            if !complete {
                return Some(Err(Error::new(
                    "RPC stream refused a required module topic",
                )));
            }
            let cursors = topics
                .into_iter()
                .filter_map(|(topic, cursor)| cursor.map(|cursor| (topic, cursor)))
                .collect();
            Some(Ok(ModuleEvent::Ready { cursors }))
        }
        StreamFrame::Event { topic, cursor, op } => Some(module_name(&topic, expected).map(
            |module| ModuleEvent::Changed {
                module,
                cursor,
                height: op.height,
            },
        )),
        StreamFrame::Lagged { topic, cursor } => {
            Some(module_name(&topic, expected).map(|module| ModuleEvent::Lagged { module, cursor }))
        }
        StreamFrame::Error { topic, detail } => Some(Err(Error::new(format!(
            "RPC stream topic {topic} failed: {detail}"
        )))),
        StreamFrame::Heartbeat => None,
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

    #[test]
    fn decodes_only_subscribed_module_topics() {
        let expected = BTreeSet::from(["module:chat".to_string(), "module:pages".to_string()]);
        let subscribed =
            r#"{"type":"subscribed","topics":{"module:chat":"c1","module:pages":"p1"}}"#;
        let ready = decode_stream_message(Ok(Message::Text(subscribed.into())), &expected)
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
            decode_stream_message(Ok(Message::Text(unexpected.into())), &expected)
                .unwrap()
                .is_err()
        );

        let unknown = r#"{"type":"retired_frame"}"#;
        assert!(
            decode_stream_message(Ok(Message::Text(unknown.into())), &expected)
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
