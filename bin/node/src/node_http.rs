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

/// Read one node-local JSON surface over GET (the `/v1` read routes that are
/// not module queries, e.g. the volatile service catalog).
pub(crate) fn get_json(
    base: &str,
    path: &str,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let resp = reqwest::blocking::Client::new()
        .get(format!("{base}{path}"))
        .send()
        .map_err(|e| format!("GET {base}{path}: {e}"))?;
    let status = resp.status();
    let text = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(format!("{path} rejected ({status}): {text}").into());
    }
    Ok(serde_json::from_str(&text)?)
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
