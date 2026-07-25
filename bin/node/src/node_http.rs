//! The operator CLI's thin HTTP client for a node's `/v1` surface: one
//! `submit` and one `query` primitive, shared by every `user`/`agent` verb so
//! the `{target, payload}` / `{target, query}` shapes and the receipt/error
//! handling live in exactly one place instead of being re-inlined per verb.
//!
//! Both hit the frameless lanes: `/v1/submit` stamps the NODE's key as the op
//! origin (valid when the node is bound to the account the op mutates), and
//! `/v1/query` reads committed module state.

/// Submit one module op over `/v1/submit` `{target, payload}` and return the
/// commit height from the receipt. A non-2xx status carries the node's
/// rejection string.
pub(crate) fn submit(
    base: &str,
    target: &str,
    payload: &serde_json::Value,
) -> Result<u64, Box<dyn std::error::Error>> {
    let body = post(base, "/v1/submit", &serde_json::json!({ "target": target, "payload": payload }))?;
    serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v["height"].as_u64())
        .ok_or_else(|| format!("unexpected submit receipt: {body}").into())
}

/// Read committed module state over `/v1/query` `{target, query}` and return
/// the module's reply as JSON for the caller to deserialize.
pub(crate) fn query(
    base: &str,
    target: &str,
    query: serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let body = post(base, "/v1/query", &serde_json::json!({ "target": target, "query": query }))?;
    Ok(serde_json::from_str(&body)?)
}

/// Why a node-local read did not produce an answer.
///
/// The distinction is the whole point: "the node is not running" is an
/// ordinary state a read verb must render calmly, while "the node answered
/// something unexpected" must be surfaced. Collapsing both into one error is
/// how a 404 or a changed body shape comes to look like "nothing is there".
pub(crate) enum ReadFailure {
    /// nothing is listening on the node's HTTP surface.
    Unreachable,
    /// the node was reached but the exchange failed (status or body).
    Rejected(String),
}

impl std::fmt::Display for ReadFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadFailure::Unreachable => write!(f, "the node is not running"),
            ReadFailure::Rejected(detail) => write!(f, "{detail}"),
        }
    }
}

/// Read one node-local JSON surface over GET (the `/v1` read routes that are
/// not module queries, e.g. the volatile service catalog).
pub(crate) fn get_json(base: &str, path: &str) -> Result<serde_json::Value, ReadFailure> {
    let resp = reqwest::blocking::Client::new()
        .get(format!("{base}{path}"))
        .send()
        .map_err(|error| match error.is_connect() {
            // reqwest's Display does not mention the refusal — the cause is in
            // the source chain — so ask the error what it IS rather than
            // grepping how it prints.
            true => ReadFailure::Unreachable,
            false => ReadFailure::Rejected(format!("GET {path}: {error}")),
        })?;
    let status = resp.status();
    let text = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(ReadFailure::Rejected(format!(
            "{path} rejected ({status}): {text}"
        )));
    }
    serde_json::from_str(&text)
        .map_err(|error| ReadFailure::Rejected(format!("{path} returned undecodable JSON: {error}")))
}

/// POST one node-local JSON surface and return the decoded reply (the `/v1`
/// routes that are not module submits, e.g. service signaling).
pub(crate) fn post_json(
    base: &str,
    path: &str,
    body: &serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_str(&post(base, path, body)?)?)
}

/// One blocking POST of a JSON body, returning the response text or the node's
/// rejection string on a non-success status.
fn post(
    base: &str,
    path: &str,
    body: &serde_json::Value,
) -> Result<String, Box<dyn std::error::Error>> {
    let resp = reqwest::blocking::Client::new()
        .post(format!("{base}{path}"))
        .json(body)
        .send()
        .map_err(|e| format!("POST {base}{path}: {e}"))?;
    let status = resp.status();
    let text = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(format!("{path} rejected ({status}): {text}").into());
    }
    Ok(text)
}
