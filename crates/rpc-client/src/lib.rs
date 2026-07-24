//! Bounded async client for Ducktape's public node RPC surface.

use std::fmt;
use std::time::Duration;

use futures::StreamExt as _;
use reqwest::{Response, Url};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

const MAX_JSON_BYTES: usize = 8 * 1024 * 1024;
const MAX_ERROR_BYTES: usize = 4 * 1024;
const TIMEOUT: Duration = Duration::from_secs(30);

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

/// The stable portion of `GET /v1/status` needed by public clients.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct Status {
    pub height: u64,
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

    fn url(&self, path: &str) -> Result<Url> {
        self.base
            .join(path)
            .map_err(|_| Error::new("could not build RPC URL"))
    }
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
}
