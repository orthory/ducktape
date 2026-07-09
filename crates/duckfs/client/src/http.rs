//! the shipping transport: a blocking-reqwest [`NodeApi`] over the noded http
//! surface.
//!
//! this is the one implementation phase 3 ships (phase-4 FUSE will add a
//! colocated-odb one behind the same trait). every method is a single
//! request/response against the duckfs routes in `bin/noded/src/files_http.rs`:
//! reads are GETs with the params as the query string, staging POSTs the raw
//! chunk bytes, and a commit POSTs the snake_case `CommitBody` and reads back the
//! CAMELCASE `BlockSummary`. a module rejection arrives as a 400 `{"error":
//! <msg>}` and passes through VERBATIM as [`ApiError::Rejected`] — the conflict
//! taxonomy keys on the exact string, so it is never reworded. a 404 is
//! [`ApiError::NotFound`] (a `stat` treats it as "nothing there"); anything else
//! that is not a clean 2xx is [`ApiError::Transport`].

use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use duckfs_core::{Change, DiffEntry, DigestHex, EntryInfo, RefsInfo, SnapshotInfo};
use serde::Deserialize;

use crate::api::{ApiError, CommitReceipt, NodeApi};

/// a node addressed over http. holds a blocking client and the base url; each
/// call builds one request off it.
pub struct HttpNode {
    client: reqwest::blocking::Client,
    base: String,
}

impl HttpNode {
    /// address the node at `base_url` (e.g. `http://127.0.0.1:8844`). the client
    /// takes no proxy (localhost daemon, never a corporate proxy) and a short
    /// connect timeout so a dead node fails fast instead of hanging a verb.
    pub fn new(base_url: impl Into<String>) -> Self {
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .connect_timeout(Duration::from_secs(10))
            // a commit rides real consensus; give it room without hanging forever.
            .timeout(Duration::from_secs(120))
            .build()
            .expect("build a blocking reqwest client");
        HttpNode {
            client,
            base: base_url.into().trim_end_matches('/').to_string(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    /// send a request and normalize the outcome: a clean 2xx yields the response;
    /// a 404 is [`ApiError::NotFound`]; any other non-2xx is decoded as the
    /// `{"error": msg}` envelope into [`ApiError::Rejected`] (verbatim), falling
    /// back to the raw body when it is not that shape.
    fn run(
        &self,
        req: reqwest::blocking::RequestBuilder,
    ) -> Result<reqwest::blocking::Response, ApiError> {
        let resp = req.send().map_err(|e| ApiError::Transport(e.to_string()))?;
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        if status.as_u16() == 404 {
            return Err(ApiError::NotFound);
        }
        Err(ApiError::Rejected(error_message(resp)))
    }

    /// GET `path` with `params` and decode the json reply into `T`.
    fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
    ) -> Result<T, ApiError> {
        let resp = self.run(self.client.get(self.url(path)).query(params))?;
        resp.json().map_err(|e| ApiError::Transport(e.to_string()))
    }
}

/// the `{"error": msg}` envelope the daemon wraps a module rejection in. the msg
/// is the module string untouched.
#[derive(Deserialize)]
struct ErrorBody {
    error: String,
}

/// pull the verbatim module message out of a non-2xx response body, falling back
/// to the raw text if it is not the `{"error": ...}` envelope.
fn error_message(resp: reqwest::blocking::Response) -> String {
    let text = resp.text().unwrap_or_default();
    match serde_json::from_str::<ErrorBody>(&text) {
        Ok(body) => body.error,
        Err(_) => text,
    }
}

/// the camelCase `BlockSummary` a stage/commit/pin POST answers with. only the
/// height matters to the engine (it resolves the snapshot id by height); the app
/// hash is carried for parity with the wire type but unused here.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BlockSummaryWire {
    height: u64,
    #[allow(dead_code)]
    app_hash: String,
}

/// `{entries, next}` — the paged shape `ls`/`find` share.
#[derive(Deserialize)]
struct PageWire {
    entries: Vec<EntryInfo>,
    #[serde(default)]
    next: Option<String>,
}

/// `{b64, eof}` — a `read` reply.
#[derive(Deserialize)]
struct ReadWire {
    b64: String,
    eof: bool,
}

/// `{snapshots}` — a `history` reply.
#[derive(Deserialize)]
struct HistoryWire {
    snapshots: Vec<SnapshotInfo>,
}

/// `{entries}` — a `diff` reply.
#[derive(Deserialize)]
struct DiffWire {
    entries: Vec<DiffEntry>,
}

/// `{present}` — a `has-chunks` reply.
#[derive(Deserialize)]
struct HasChunksWire {
    present: Vec<bool>,
}

/// `{digest}` — a `stage` reply.
#[derive(Deserialize)]
struct StageWire {
    digest: DigestHex,
}

impl NodeApi for HttpNode {
    fn refs(&self) -> Result<RefsInfo, ApiError> {
        self.get_json("/v1/files/refs", &[])
    }

    fn stat(&self, path: &str, snapshot: Option<&str>) -> Result<Option<EntryInfo>, ApiError> {
        let mut params = vec![("path", path.to_string())];
        push_opt(&mut params, "snapshot", snapshot);
        // a 404 means "no entry at that path" — Ok(None), not a failure.
        match self.run(self.client.get(self.url("/v1/files/stat")).query(&params)) {
            Ok(resp) => resp
                .json::<EntryInfo>()
                .map(Some)
                .map_err(|e| ApiError::Transport(e.to_string())),
            Err(ApiError::NotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn ls(
        &self,
        path: &str,
        snapshot: Option<&str>,
        after: Option<&str>,
        limit: u64,
    ) -> Result<(Vec<EntryInfo>, Option<String>), ApiError> {
        let mut params = vec![("path", path.to_string()), ("limit", limit.to_string())];
        push_opt(&mut params, "snapshot", snapshot);
        push_opt(&mut params, "after", after);
        let page: PageWire = self.get_json("/v1/files/ls", &params)?;
        Ok((page.entries, page.next))
    }

    fn find(
        &self,
        prefix: &str,
        snapshot: Option<&str>,
        after: Option<&str>,
        limit: u64,
    ) -> Result<(Vec<EntryInfo>, Option<String>), ApiError> {
        let mut params = vec![("prefix", prefix.to_string()), ("limit", limit.to_string())];
        push_opt(&mut params, "snapshot", snapshot);
        push_opt(&mut params, "after", after);
        let page: PageWire = self.get_json("/v1/files/find", &params)?;
        Ok((page.entries, page.next))
    }

    fn read(
        &self,
        path: &str,
        snapshot: Option<&str>,
        offset: u64,
        len: u64,
    ) -> Result<(Vec<u8>, bool), ApiError> {
        let mut params = vec![
            ("path", path.to_string()),
            ("offset", offset.to_string()),
            ("len", len.to_string()),
        ];
        push_opt(&mut params, "snapshot", snapshot);
        let read: ReadWire = self.get_json("/v1/files/read", &params)?;
        let bytes = STANDARD
            .decode(read.b64.as_bytes())
            .map_err(|e| ApiError::Transport(e.to_string()))?;
        Ok((bytes, read.eof))
    }

    fn history(&self, limit: u64) -> Result<Vec<SnapshotInfo>, ApiError> {
        let params = vec![("limit", limit.to_string())];
        let hist: HistoryWire = self.get_json("/v1/files/history", &params)?;
        Ok(hist.snapshots)
    }

    fn diff(&self, from: &str, to: &str, prefix: &str) -> Result<Vec<DiffEntry>, ApiError> {
        let params = vec![
            ("from", from.to_string()),
            ("to", to.to_string()),
            ("prefix", prefix.to_string()),
        ];
        let diff: DiffWire = self.get_json("/v1/files/diff", &params)?;
        Ok(diff.entries)
    }

    fn has_chunks(&self, ids: &[String]) -> Result<Vec<bool>, ApiError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        // the route takes a comma-separated `ids` param; reqwest percent-encodes
        // the joined string and axum decodes it back before the split.
        let params = vec![("ids", ids.join(","))];
        let hc: HasChunksWire = self.get_json("/v1/files/has-chunks", &params)?;
        Ok(hc.present)
    }

    fn stage_chunk(&self, bytes: &[u8]) -> Result<DigestHex, ApiError> {
        let resp = self.run(
            self.client
                .post(self.url("/v1/files/stage"))
                .header("content-type", "application/octet-stream")
                .body(bytes.to_vec()),
        )?;
        let staged: StageWire = resp
            .json()
            .map_err(|e| ApiError::Transport(e.to_string()))?;
        Ok(staged.digest)
    }

    fn commit(
        &self,
        base: Option<&str>,
        message: &str,
        changes: Vec<Change>,
    ) -> Result<CommitReceipt, ApiError> {
        // the snake_case CommitBody the module wire speaks (base omitted/null is
        // the empty tree — a first commit).
        let body = CommitBodyOut {
            base_snapshot: base,
            message,
            changes,
        };
        let resp = self.run(self.client.post(self.url("/v1/files/commit")).json(&body))?;
        let block: BlockSummaryWire = resp
            .json()
            .map_err(|e| ApiError::Transport(e.to_string()))?;
        Ok(CommitReceipt {
            height: block.height,
        })
    }

    fn pin(&self, snapshot: &str, name: &str) -> Result<(), ApiError> {
        let body = serde_json::json!({ "snapshot": snapshot, "name": name });
        self.run(self.client.post(self.url("/v1/files/pin")).json(&body))?;
        Ok(())
    }
}

/// the POST /v1/files/commit body — snake_case, matching `CommitBody` in
/// `files_http.rs` (borrowed so no needless clones of the changes).
#[derive(serde::Serialize)]
struct CommitBodyOut<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    base_snapshot: Option<&'a str>,
    message: &'a str,
    changes: Vec<Change>,
}

/// push an optional string param only when present (an absent param reads as the
/// route's default; a `snapshot=` empty string would be a different, wrong ask).
fn push_opt(params: &mut Vec<(&'static str, String)>, key: &'static str, value: Option<&str>) {
    if let Some(v) = value {
        params.push((key, v.to_string()));
    }
}
