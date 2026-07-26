//! `NodeLink` — a service daemon's handle on the node it serves.
//!
//! The compute plane runs OUT OF PROCESS now (`ducktape service run compute`),
//! so everything it used to reach through the in-process [`crate::NodeHandle`]
//! actor lane travels the node's own `/v1` surface instead. This is the one
//! place that mapping lives:
//!
//! | actor command | `/v1` route |
//! |---|---|
//! | `NodeCommand::Submit` | `POST /v1/submit` — the node re-signs with ITS key |
//! | `NodeCommand::SubmitFrame` | `POST /v1/submit/frame` — verbatim, signer verified |
//! | `NodeCommand::Query` | `POST /v1/query` — committed module state |
//! | `ActorNodeApi` (duckfs engine) | [`duckfs_client::http::HttpNode`] |
//!
//! The `origin` field of `/v1/submit` is deliberately not sent: `bin/node`
//! discards it and frames the op with the node key, which is exactly the
//! identity a saga lease is held under. That equivalence is the whole reason a
//! daemon needs no keypair of its own.
//!
//! Two things stay host-local paths rather than `/v1` calls, because they are
//! host resources and not node state: the node-private podman socket, and the
//! forge module's materialized bare repos (`<storage>/forge-repo`), which the
//! worktree lane clones from directly. A daemon that drives this host's podman
//! is already on this host.

use std::path::{Path, PathBuf};

use duckfs_client::http::HttpNode;

/// How long one lane call may take by DEFAULT. A submit rides real consensus,
/// so it gets the same generous ceiling `HttpNode` gives a commit; a lane that
/// hangs forever would wedge a run with no diagnosis. A link that only reads
/// committed state on someone's interactive critical path says so with
/// [`NodeLink::with_timeout`] instead of inheriting this.
const CALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

fn http_client(timeout: std::time::Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .timeout(timeout)
        .build()
        .expect("build a reqwest client")
}

/// The node a daemon serves, addressed over its local `/v1` surface.
#[derive(Clone)]
pub struct NodeLink {
    base: String,
    /// the node's forge repo base (`<storage>/forge-repo`) — a host path, not a
    /// route. `None` on a node whose storage dir is unknown to the daemon.
    forge_repo: Option<PathBuf>,
    client: reqwest::Client,
}

impl NodeLink {
    /// address the node at `base` (e.g. `http://127.0.0.1:8844`).
    pub fn new(base: impl Into<String>) -> Self {
        Self {
            base: base.into().trim_end_matches('/').to_string(),
            forge_repo: None,
            client: http_client(CALL_TIMEOUT),
        }
    }

    /// Bound this link's calls at `timeout` rather than the consensus-sized
    /// default — for a daemon whose link carries only committed READS on a
    /// caller's critical path (the airlock grant gate: a borrower's session is
    /// blocked on it). Fail-closed in seconds beats a two-minute hang while a
    /// node is wedged or restarting.
    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.client = http_client(timeout);
        self
    }

    /// point the forge worktree lane at the node's materialized bare repos.
    pub fn with_forge_repo(mut self, base: impl Into<PathBuf>) -> Self {
        self.forge_repo = Some(base.into());
        self
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    pub fn forge_repo(&self) -> Option<&Path> {
        self.forge_repo.as_deref()
    }

    /// the duckfs checkout/commit engine's transport.
    ///
    /// MUST be called from a blocking context (`spawn_blocking`), never from an
    /// async one: the engine is sync `std::fs`, and `reqwest`'s blocking client
    /// owns a runtime whose DROP panics inside an async context. Building it
    /// where it is used keeps both halves on the blocking side.
    pub fn files(&self) -> HttpNode {
        HttpNode::new(self.base.clone())
    }

    /// submit one module op. `payload` is the module's own serde_json wire
    /// bytes, which `/v1/submit` takes as a json value and re-serializes — the
    /// module decodes the same value either way.
    pub async fn submit(&self, target: &str, payload: &[u8]) -> Result<u64, String> {
        let payload: serde_json::Value = serde_json::from_slice(payload)
            .map_err(|error| format!("op payload is not json: {error}"))?;
        let body = serde_json::json!({ "target": target, "payload": payload });
        let text = self.post_json("/v1/submit", &body).await?;
        serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|value| value["height"].as_u64())
            .ok_or_else(|| format!("unexpected submit receipt: {text}"))
    }

    /// submit an ALREADY-SIGNED frame verbatim. The node verifies the signature
    /// and the frame's own signer becomes the op's origin, so this is the only
    /// lane that can carry an identity other than the node's.
    pub async fn submit_frame(&self, frame: Vec<u8>) -> Result<(), String> {
        let response = self
            .client
            .post(format!("{}/v1/submit/frame", self.base))
            .header("content-type", "application/octet-stream")
            .body(frame)
            .send()
            .await
            .map_err(|error| format!("POST /v1/submit/frame: {error}"))?;
        Self::body_of(response).await.map(|_| ())
    }

    /// read committed module state. `req` is the module's encoded `*Query`; the
    /// reply is its encoded `*Reply`, so callers decode with the module's own
    /// codec exactly as they did on the actor lane.
    pub async fn query(&self, target: &str, req: &[u8]) -> Result<Vec<u8>, String> {
        let query: serde_json::Value = serde_json::from_slice(req)
            .map_err(|error| format!("query is not json: {error}"))?;
        let body = serde_json::json!({ "target": target, "query": query });
        self.post_json("/v1/query", &body)
            .await
            .map(String::into_bytes)
    }

    async fn post_json(&self, path: &str, body: &serde_json::Value) -> Result<String, String> {
        let response = self
            .client
            .post(format!("{}{path}", self.base))
            .json(body)
            .send()
            .await
            .map_err(|error| format!("POST {path}: {error}"))?;
        Self::body_of(response).await
    }

    /// the node's rejection string rides through VERBATIM — the duckfs conflict
    /// taxonomy and the saga's refusal messages both key on the exact text.
    async fn body_of(response: reqwest::Response) -> Result<String, String> {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            let detail = serde_json::from_str::<serde_json::Value>(&text)
                .ok()
                .and_then(|value| value["error"].as_str().map(str::to_string))
                .unwrap_or(text);
            return Err(detail);
        }
        Ok(text)
    }
}
