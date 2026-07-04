//! host-side LLM oracle worker for agent saga effects.
//!
//! the module state machine emits an opaque saga `WorkerRequest`. this crate is
//! the impure host-side counterpart: it recognizes agent `LlmRequest` specs,
//! calls the ChatGPT/Codex subscription Responses endpoint with the local Codex
//! OAuth token, and returns a saga `OracleResult` op.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use agent_interface::{
    AgentAction, AgentOutput, LlmRequest, MAX_ACTIONS_PER_RUN, MAX_REPLY_BLOCKS_BYTES,
    decode_llm_request, encode_output,
};
use chat_interface::{AuthorRef, Block, MessageView};
use reactor::Worker;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue, USER_AGENT};
use saga_interface::{SagaMsg, WorkerRequest, decode_worker_request, encode_msg};
use sdk::{Effect, Msg};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest as _, Sha256};

const RESPONSES_URL: &str = "https://chatgpt.com/backend-api/codex/responses";
const OAUTH_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const CODEX_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const CODEX_USER_AGENT: &str = "codex_cli_rs/0.142.5";
const ORIGINATOR: &str = "codex_cli_rs";
const DEFAULT_PROMPT: &str =
    "You are a Ducktape agent. Reply helpfully and return only the requested JSON output.";
const STRICT_OUTPUT_INSTRUCTION: &str = r#"Return ONLY a JSON object with this shape:
{"reply_blocks":[{"id":"<uuid>","kind":"Paragraph","text":"..."}],"actions":[]}
Allowed reply block kinds are Paragraph, Heading, and Code. Heading is rendered as a paragraph in Ducktape chat. Code may include an optional "lang". Actions are optional and must use only actions allowed by the agent registry. Do not include markdown fences around the JSON."#;

static MISSING_PROMPT_LOGGED: AtomicBool = AtomicBool::new(false);

/// ChatGPT/Codex OAuth auth store, preserving `~/.codex/auth.json` shape.
#[derive(Clone, Debug)]
pub struct AuthStore {
    path: PathBuf,
}

impl AuthStore {
    pub fn from_default_path() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            path: home.join(".codex").join("auth.json"),
        }
    }

    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    async fn valid_token(&self, client: &reqwest::Client) -> Result<TokenSet, String> {
        let token = self.load()?;
        if access_token_expires_soon_at(&token.access_token, now_millis()) {
            return self.refresh(client, &token).await;
        }
        Ok(token)
    }

    async fn refresh(
        &self,
        client: &reqwest::Client,
        current: &TokenSet,
    ) -> Result<TokenSet, String> {
        let resp = client
            .post(OAUTH_TOKEN_URL)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(refresh_form_body(&current.refresh_token))
            .send()
            .await
            .map_err(|e| format!("oauth refresh request failed: {e}"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("oauth refresh failed with {status}: {text}"));
        }
        let refreshed: RefreshResponse = resp
            .json()
            .await
            .map_err(|e| format!("oauth refresh response was not json: {e}"))?;
        let refresh_token = refreshed
            .refresh_token
            .unwrap_or_else(|| current.refresh_token.clone());
        let account_id = jwt_account_id(&refreshed.access_token)
            .or_else(|| current.account_id.clone())
            .ok_or_else(|| {
                "refreshed access token did not include a ChatGPT account id".to_string()
            })?;
        let token = TokenSet {
            access_token: refreshed.access_token,
            refresh_token,
            account_id: Some(account_id),
            id_token: refreshed.id_token,
            raw: current.raw.clone(),
        };
        self.persist(&token)?;
        Ok(token)
    }

    fn load(&self) -> Result<TokenSet, String> {
        let bytes = fs::read(&self.path)
            .map_err(|e| format!("read {} failed: {e}", self.path.display()))?;
        let raw: Value = serde_json::from_slice(&bytes)
            .map_err(|e| format!("{} is not json: {e}", self.path.display()))?;
        let tokens = raw
            .get("tokens")
            .and_then(Value::as_object)
            .ok_or_else(|| format!("{} has no tokens object", self.path.display()))?;
        let access_token = json_string(tokens, "access_token")?;
        let refresh_token = json_string(tokens, "refresh_token")?;
        let account_id = tokens
            .get("account_id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .or_else(|| jwt_account_id(&access_token));
        Ok(TokenSet {
            access_token,
            refresh_token,
            account_id,
            id_token: tokens
                .get("id_token")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            raw,
        })
    }

    fn persist(&self, token: &TokenSet) -> Result<(), String> {
        let mut raw = token.raw.clone();
        let root = raw
            .as_object_mut()
            .ok_or_else(|| "auth json root must be an object".to_string())?;
        let tokens_value = root
            .entry("tokens")
            .or_insert_with(|| Value::Object(Map::new()));
        let tokens = tokens_value
            .as_object_mut()
            .ok_or_else(|| "auth json tokens field must be an object".to_string())?;
        tokens.insert(
            "access_token".into(),
            Value::String(token.access_token.clone()),
        );
        tokens.insert(
            "refresh_token".into(),
            Value::String(token.refresh_token.clone()),
        );
        if let Some(account_id) = &token.account_id {
            tokens.insert("account_id".into(), Value::String(account_id.clone()));
        }
        if let Some(id_token) = &token.id_token {
            tokens.insert("id_token".into(), Value::String(id_token.clone()));
        }
        root.insert(
            "last_refresh".into(),
            Value::String(iso8601_millis(now_millis())),
        );

        write_auth_json(&self.path, &raw)
    }
}

#[derive(Clone, Debug)]
struct TokenSet {
    access_token: String,
    refresh_token: String,
    account_id: Option<String>,
    id_token: Option<String>,
    raw: Value,
}

#[derive(Debug, Deserialize)]
struct RefreshResponse {
    access_token: String,
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    expires_in: Option<u64>,
}

/// Production LLM worker for agent `LlmRequest` saga effects.
pub struct LlmWorker {
    blobs: files::BlobHandle,
    auth: AuthStore,
    default_model: String,
    client: reqwest::Client,
}

impl LlmWorker {
    pub fn new(blobs: files::BlobHandle, auth: AuthStore, default_model: String) -> Self {
        Self {
            blobs,
            auth,
            default_model,
            client: reqwest::Client::new(),
        }
    }

    async fn answer(&self, llm: &LlmRequest) -> Result<Vec<u8>, String> {
        let prompt = self.prompt_text(llm)?;
        let request = build_responses_request(&self.default_model, &prompt, llm);
        let assistant_text = self.call_responses(&request).await?;
        let output = agent_output_from_text(&assistant_text, llm.job_id.is_some());
        Ok(encode_output(&output))
    }

    fn prompt_text(&self, llm: &LlmRequest) -> Result<String, String> {
        let prompt_hash: [u8; 32] = llm.prompt_hash.as_slice().try_into().map_err(|_| {
            format!(
                "prompt hash is {} bytes, expected 32",
                llm.prompt_hash.len()
            )
        })?;
        let Some(bytes) = self.blobs.get_chunk(&prompt_hash) else {
            if !MISSING_PROMPT_LOGGED.swap(true, Ordering::Relaxed) {
                eprintln!(
                    "[agent-oracle] prompt blob {} is missing; using generic fallback instructions",
                    hex(&prompt_hash)
                );
            }
            return Ok(DEFAULT_PROMPT.into());
        };
        let actual: [u8; 32] = Sha256::digest(&bytes).into();
        if actual != prompt_hash {
            return Err("prompt blob digest did not match prompt_hash".into());
        }
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    async fn call_responses(&self, request: &ResponsesRequest) -> Result<String, String> {
        let mut token = self.auth.valid_token(&self.client).await?;
        let mut resp = self.send_responses(request, &token).await?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            token = self.auth.refresh(&self.client, &token).await?;
            resp = self.send_responses(request, &token).await?;
        }
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("responses endpoint failed with {status}: {text}"));
        }
        let body = resp
            .text()
            .await
            .map_err(|e| format!("responses body read failed: {e}"))?;
        parse_sse_output_text(&body)
    }

    async fn send_responses(
        &self,
        request: &ResponsesRequest,
        token: &TokenSet,
    ) -> Result<reqwest::Response, String> {
        let account_id = token
            .account_id
            .as_deref()
            .ok_or_else(|| "missing ChatGPT account id".to_string())?;
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", token.access_token))
                .map_err(|e| format!("bad authorization header: {e}"))?,
        );
        headers.insert("chatgpt-account-id", header_value(account_id)?);
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(ACCEPT, HeaderValue::from_static("text/event-stream"));
        headers.insert(
            "openai-beta",
            HeaderValue::from_static("responses=experimental"),
        );
        headers.insert("originator", HeaderValue::from_static(ORIGINATOR));
        headers.insert(USER_AGENT, HeaderValue::from_static(CODEX_USER_AGENT));
        headers.insert("session_id", header_value(&session_id())?);

        self.client
            .post(RESPONSES_URL)
            .headers(headers)
            .json(request)
            .send()
            .await
            .map_err(|e| format!("responses request failed: {e}"))
    }
}

#[async_trait::async_trait(?Send)]
impl Worker for LlmWorker {
    async fn run(&self, effect: &Effect) -> Result<Option<Msg>, reactor::Error> {
        let request = match decode_worker_request(&effect.0) {
            Ok(request) => request,
            Err(_) => return Ok(None),
        };
        let llm = match decode_llm_request(&request.spec) {
            Ok(llm) => llm,
            Err(_) => return Ok(None),
        };
        let outcome = self.answer(&llm).await.map_err(clean_error);
        Ok(Some(oracle_result(&request, outcome)))
    }
}

fn oracle_result(request: &WorkerRequest, outcome: Result<Vec<u8>, String>) -> Msg {
    Msg {
        target: "saga".into(),
        payload: encode_msg(&SagaMsg::OracleResult {
            saga_id: request.saga_id.clone(),
            attempt: request.attempt,
            outcome,
        }),
    }
}

fn clean_error(error: String) -> String {
    const MAX: usize = 2048;
    if error.len() <= MAX {
        return error;
    }
    let mut keep = MAX;
    while keep > 0 && !error.is_char_boundary(keep) {
        keep -= 1;
    }
    let mut out = error;
    out.truncate(keep);
    out
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct ResponsesRequest {
    model: String,
    instructions: String,
    input: Vec<ResponseMessage>,
    stream: bool,
    store: bool,
    include: Vec<String>,
    reasoning: Reasoning,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct Reasoning {
    effort: &'static str,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct ResponseMessage {
    #[serde(rename = "type")]
    kind: &'static str,
    role: &'static str,
    content: Vec<ResponseContent>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct ResponseContent {
    #[serde(rename = "type")]
    kind: &'static str,
    text: String,
}

fn build_responses_request(
    default_model: &str,
    prompt: &str,
    llm: &LlmRequest,
) -> ResponsesRequest {
    let model = if llm.model_ref.trim().is_empty() {
        default_model.to_string()
    } else {
        llm.model_ref.clone()
    };
    ResponsesRequest {
        model,
        instructions: format!("{prompt}\n\n{STRICT_OUTPUT_INSTRUCTION}"),
        input: input_messages(llm),
        stream: true,
        store: false,
        include: Vec::new(),
        reasoning: Reasoning { effort: "low" },
    }
}

fn input_messages(llm: &LlmRequest) -> Vec<ResponseMessage> {
    if llm.job_id.is_some() {
        return vec![ResponseMessage::user(format!(
            "Agent job run {}\nJob id: {}\nContext hash: {}",
            llm.run_id,
            llm.job_id.as_deref().unwrap_or_default(),
            hex(&llm.context_hash)
        ))];
    }
    if llm.transcript.is_empty() {
        return vec![ResponseMessage::user(
            "No transcript was embedded for this run. Answer the user helpfully.".into(),
        )];
    }
    llm.transcript
        .iter()
        .map(|message| {
            let text = render_message(message);
            if is_this_agent(message, &llm.agent_id) {
                ResponseMessage::assistant(text)
            } else {
                ResponseMessage::user(text)
            }
        })
        .collect()
}

impl ResponseMessage {
    fn user(text: String) -> Self {
        Self {
            kind: "message",
            role: "user",
            content: vec![ResponseContent {
                kind: "input_text",
                text,
            }],
        }
    }

    fn assistant(text: String) -> Self {
        Self {
            kind: "message",
            role: "assistant",
            content: vec![ResponseContent {
                kind: "output_text",
                text,
            }],
        }
    }
}

fn is_this_agent(message: &MessageView, agent_id: &str) -> bool {
    matches!(
        &message.head.author,
        AuthorRef::Agent { module, agent_id: author_agent }
            if module == "agent" && author_agent == agent_id
    )
}

fn render_message(message: &MessageView) -> String {
    format!(
        "{} @{}: {}",
        render_author(&message.head.author),
        message.seq,
        render_blocks(&message.head.blocks)
    )
}

fn render_author(author: &AuthorRef) -> String {
    match author {
        AuthorRef::User(bytes) => format!("user:{}", hex(bytes)),
        AuthorRef::Agent { module, agent_id } => format!("agent:{module}/{agent_id}"),
        AuthorRef::Module(module) => format!("module:{module}"),
        AuthorRef::System => "system".into(),
    }
}

fn render_blocks(blocks: &[Block]) -> String {
    blocks
        .iter()
        .map(render_block)
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_block(block: &Block) -> String {
    match block {
        Block::Paragraph(spans) => spans.iter().map(|s| s.text.as_str()).collect(),
        Block::Code { lang, text } => match lang {
            Some(lang) if !lang.is_empty() => format!("```{lang}\n{text}\n```"),
            _ => format!("```\n{text}\n```"),
        },
        Block::Quote(spans) => {
            let text: String = spans.iter().map(|s| s.text.as_str()).collect();
            text.lines()
                .map(|line| format!("> {line}"))
                .collect::<Vec<_>>()
                .join("\n")
        }
        Block::Divider => "---".into(),
    }
}

fn agent_output_from_text(text: &str, job_run: bool) -> AgentOutput {
    let parsed = serde_json::from_str::<AgentOutput>(text)
        .or_else(|_| serde_json::from_str::<ModelOutput>(text).map(ModelOutput::into_agent_output));
    let fallback = || AgentOutput {
        reply_blocks: if job_run {
            Vec::new()
        } else {
            vec![Block::paragraph(non_empty_text(text))]
        },
        actions: Vec::new(),
    };
    normalize_output(parsed.unwrap_or_else(|_| fallback()), text, job_run)
}

#[derive(Debug, Deserialize)]
struct ModelOutput {
    #[serde(default)]
    reply_blocks: Vec<ModelBlock>,
    #[serde(default)]
    actions: Vec<AgentAction>,
}

impl ModelOutput {
    fn into_agent_output(self) -> AgentOutput {
        AgentOutput {
            reply_blocks: self
                .reply_blocks
                .into_iter()
                .filter_map(ModelBlock::into_block)
                .collect(),
            actions: self.actions,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ModelBlock {
    #[allow(dead_code)]
    #[serde(default)]
    id: Option<String>,
    kind: String,
    text: String,
    #[serde(default)]
    lang: Option<String>,
}

impl ModelBlock {
    fn into_block(self) -> Option<Block> {
        let text = self.text.trim().to_string();
        if text.is_empty() {
            return None;
        }
        match self.kind.as_str() {
            "Paragraph" | "Heading" => Some(Block::paragraph(text)),
            "Code" => Some(Block::Code {
                lang: self.lang.filter(|l| !l.is_empty()),
                text,
            }),
            _ => None,
        }
    }
}

fn normalize_output(mut output: AgentOutput, raw_text: &str, job_run: bool) -> AgentOutput {
    output.actions.truncate(MAX_ACTIONS_PER_RUN);
    if job_run {
        output.reply_blocks.clear();
    }
    if !job_run && output.reply_blocks.is_empty() {
        output
            .reply_blocks
            .push(Block::paragraph(non_empty_text(raw_text)));
    }
    if !output.reply_blocks.is_empty() {
        let bytes = serde_json::to_vec(&output.reply_blocks).expect("blocks serialize");
        if bytes.len() > MAX_REPLY_BLOCKS_BYTES {
            output.reply_blocks = vec![Block::paragraph(truncate_utf8(
                &non_empty_text(raw_text),
                MAX_REPLY_BLOCKS_BYTES / 4,
            ))];
        }
    }
    output
}

fn non_empty_text(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        "Done.".into()
    } else {
        trimmed.into()
    }
}

fn truncate_utf8(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut keep = max;
    while keep > 0 && !text.is_char_boundary(keep) {
        keep -= 1;
    }
    let mut out = text.to_string();
    out.truncate(keep);
    out
}

fn parse_sse_output_text(body: &str) -> Result<String, String> {
    let mut out = String::new();
    let mut completed: Option<String> = None;
    for line in body.lines() {
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        let value: Value =
            serde_json::from_str(data).map_err(|e| format!("bad responses SSE data json: {e}"))?;
        match value.get("type").and_then(Value::as_str) {
            Some("response.output_text.delta") => {
                if let Some(delta) = value.get("delta").and_then(Value::as_str) {
                    out.push_str(delta);
                }
            }
            Some("response.completed") => {
                completed = completed_text(&value);
            }
            _ => {}
        }
    }
    if out.is_empty() {
        out = completed.unwrap_or_default();
    }
    if out.is_empty() {
        return Err("responses stream completed without assistant text".into());
    }
    Ok(out)
}

fn completed_text(value: &Value) -> Option<String> {
    let output = value
        .pointer("/response/output")
        .or_else(|| value.get("output"))?
        .as_array()?;
    for item in output {
        if item.get("type").and_then(Value::as_str) != Some("message") {
            continue;
        }
        let Some(content) = item.get("content").and_then(Value::as_array) else {
            continue;
        };
        for part in content {
            if part.get("type").and_then(Value::as_str) == Some("output_text") {
                if let Some(text) = part.get("text").and_then(Value::as_str) {
                    return Some(text.to_string());
                }
            }
        }
    }
    None
}

fn access_token_expires_soon_at(token: &str, now_ms: u64) -> bool {
    let Some(exp_seconds) = jwt_exp(token) else {
        return true;
    };
    exp_seconds.saturating_mul(1000) <= now_ms.saturating_add(5 * 60 * 1000)
}

fn jwt_exp(token: &str) -> Option<u64> {
    jwt_payload(token)?.get("exp")?.as_u64()
}

fn jwt_account_id(token: &str) -> Option<String> {
    jwt_payload(token)?
        .get("https://api.openai.com/auth")?
        .get("chatgpt_account_id")?
        .as_str()
        .map(ToOwned::to_owned)
}

fn jwt_payload(token: &str) -> Option<Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = base64_url_decode(payload).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn base64_url_decode(input: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0u8;
    for byte in input.bytes().filter(|b| *b != b'=') {
        let value =
            base64_url_value(byte).ok_or_else(|| format!("invalid base64url byte: {byte}"))? as u32;
        buf = (buf << 6) | value;
        bits += 6;
        while bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            let mask = if bits == 0 { 0 } else { (1u32 << bits) - 1 };
            buf &= mask;
        }
    }
    Ok(out)
}

fn base64_url_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'-' => Some(62),
        b'_' => Some(63),
        _ => None,
    }
}

fn refresh_form_body(refresh_token: &str) -> String {
    format!(
        "grant_type=refresh_token&client_id={}&refresh_token={}",
        form_component(CODEX_CLIENT_ID),
        form_component(refresh_token)
    )
}

fn form_component(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn json_string(tokens: &Map<String, Value>, key: &str) -> Result<String, String> {
    tokens
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("tokens.{key} is missing"))
}

fn write_auth_json(path: &Path, raw: &Value) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(raw).expect("auth json serializes");
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|e| format!("open {} for write failed: {e}", path.display()))?;
    file.write_all(&bytes)
        .map_err(|e| format!("write {} failed: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("chmod 0600 {} failed: {e}", path.display()))?;
    }
    Ok(())
}

fn header_value(value: &str) -> Result<HeaderValue, String> {
    HeaderValue::from_str(value).map_err(|e| format!("bad header value: {e}"))
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_millis() as u64
}

fn iso8601_millis(ms: u64) -> String {
    let secs = (ms / 1000) as i64;
    let millis = ms % 1000;
    let days = secs.div_euclid(86_400);
    let day_seconds = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = day_seconds / 3600;
    let minute = (day_seconds % 3600) / 60;
    let second = day_seconds % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    (year as i32, m as u32, d as u32)
}

fn session_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    let mut bytes = (nanos ^ ((std::process::id() as u128) << 64)).to_be_bytes();
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chat_interface::{MessageHead, ReactionSummary, Span};
    use serde_json::json;

    #[test]
    fn builds_responses_body_from_transcript() {
        let llm = LlmRequest {
            run_id: "run-1".into(),
            agent_id: "bot".into(),
            model_ref: String::new(),
            prompt_hash: vec![0; 32],
            channel_id: "general".into(),
            anchor_seq: 2,
            job_id: None,
            context_hash: vec![1, 2, 3],
            transcript: vec![
                message(1, AuthorRef::User(vec![1, 2]), "hello"),
                message(
                    2,
                    AuthorRef::Agent {
                        module: "agent".into(),
                        agent_id: "bot".into(),
                    },
                    "hi",
                ),
            ],
        };
        let body = build_responses_request("gpt-5.1", "system prompt", &llm);
        assert_eq!(body.model, "gpt-5.1");
        assert!(body.instructions.contains("system prompt"));
        assert!(body.instructions.contains("Return ONLY a JSON object"));
        assert_eq!(body.stream, true);
        assert_eq!(body.store, false);
        assert_eq!(body.input.len(), 2);
        assert_eq!(body.input[0].role, "user");
        assert_eq!(body.input[0].content[0].kind, "input_text");
        assert_eq!(body.input[1].role, "assistant");
        assert_eq!(body.input[1].content[0].kind, "output_text");
    }

    #[test]
    fn parses_sse_deltas_and_completed_fallback() {
        let sse = concat!(
            "event: response.output_text.delta\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"hel\"}\n\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"lo\"}\n\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"output\":[{\"type\":\"message\",\"content\":[{\"type\":\"output_text\",\"text\":\"ignored\"}]}]}}\n\n",
        );
        assert_eq!(parse_sse_output_text(sse).unwrap(), "hello");

        let completed = "data: {\"type\":\"response.completed\",\"response\":{\"output\":[{\"type\":\"message\",\"content\":[{\"type\":\"output_text\",\"text\":\"final text\"}]}]}}\n\n";
        assert_eq!(parse_sse_output_text(completed).unwrap(), "final text");
    }

    #[test]
    fn jwt_expiry_and_account_claim_are_decoded() {
        let token = jwt_with_payload(json!({
            "exp": 2_000_000_000u64,
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "acct_123"
            }
        }));
        assert!(!access_token_expires_soon_at(&token, 1_000));
        assert!(access_token_expires_soon_at(
            &token,
            2_000_000_000_000 - 299_000
        ));
        assert_eq!(jwt_account_id(&token), Some("acct_123".into()));
    }

    #[test]
    fn refresh_form_is_urlencoded() {
        assert_eq!(
            refresh_form_body("rt space/+"),
            "grant_type=refresh_token&client_id=app_EMoamEEZ73f0CkXaXp7hrann&refresh_token=rt+space%2F%2B"
        );
    }

    #[test]
    fn auth_json_round_trips_and_preserves_shape() {
        let dir =
            std::env::temp_dir().join(format!("ducktape-agent-oracle-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("auth.json");
        let old_token = jwt_with_payload(json!({
            "exp": 1u64,
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "acct_old"
            }
        }));
        fs::write(
            &path,
            serde_json::to_vec(&json!({
                "OPENAI_API_KEY": null,
                "auth_mode": "chatgpt",
                "tokens": {
                    "access_token": old_token,
                    "refresh_token": "rt-old"
                },
                "extra": {"kept": true}
            }))
            .unwrap(),
        )
        .unwrap();

        let store = AuthStore::from_path(&path);
        let loaded = store.load().unwrap();
        assert_eq!(loaded.account_id, Some("acct_old".into()));
        let new_token = TokenSet {
            access_token: jwt_with_payload(json!({"exp": 2u64})),
            refresh_token: "rt-new".into(),
            account_id: Some("acct_new".into()),
            id_token: Some("id-new".into()),
            raw: loaded.raw,
        };
        store.persist(&new_token).unwrap();
        let after: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(after["OPENAI_API_KEY"], Value::Null);
        assert_eq!(after["auth_mode"], "chatgpt");
        assert_eq!(after["extra"]["kept"], true);
        assert_eq!(after["tokens"]["refresh_token"], "rt-new");
        assert_eq!(after["tokens"]["account_id"], "acct_new");
        assert!(after["last_refresh"].as_str().unwrap().ends_with('Z'));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn model_friendly_json_is_normalized_to_chat_blocks() {
        let text = r#"{"reply_blocks":[{"id":"b1","kind":"Paragraph","text":"hello"},{"kind":"Code","lang":"rust","text":"fn main() {}"}],"actions":[]}"#;
        let out = agent_output_from_text(text, false);
        assert_eq!(
            out.reply_blocks,
            vec![
                Block::paragraph("hello"),
                Block::Code {
                    lang: Some("rust".into()),
                    text: "fn main() {}".into()
                }
            ]
        );

        let wrapped = agent_output_from_text("not json", false);
        assert_eq!(wrapped.reply_blocks, vec![Block::paragraph("not json")]);
    }

    #[tokio::test]
    #[ignore]
    async fn live_responses_call_uses_local_codex_auth() {
        let blobs = files::BlobHandle::default();
        let prompt_hash = blobs.put_chunk(b"Reply with a tiny JSON AgentOutput.".to_vec());
        let worker = LlmWorker::new(blobs, AuthStore::from_default_path(), "gpt-5.1".to_string());
        let llm = LlmRequest {
            run_id: "live".into(),
            agent_id: "bot".into(),
            model_ref: String::new(),
            prompt_hash: prompt_hash.to_vec(),
            channel_id: "general".into(),
            anchor_seq: 1,
            job_id: None,
            context_hash: Vec::new(),
            transcript: vec![message(1, AuthorRef::User(b"human".to_vec()), "say hi")],
        };
        let _ = worker.answer(&llm).await.unwrap();
    }

    fn message(seq: u64, author: AuthorRef, text: &str) -> MessageView {
        MessageView {
            channel_id: "general".into(),
            seq,
            head: MessageHead {
                message_id: format!("m{seq}"),
                author,
                blocks: vec![Block::Paragraph(vec![Span::plain(text)])],
                created_at: seq,
                rev: 0,
                edited_at: None,
                base_rev: None,
                deleted: false,
                thread: None,
                reply_count: 0,
                last_reply_seq: None,
            },
            reactions: Vec::<ReactionSummary>::new(),
            channel_head_seq: seq,
        }
    }

    fn jwt_with_payload(payload: Value) -> String {
        format!(
            "e30.{}.sig",
            base64_url_encode(&serde_json::to_vec(&payload).unwrap())
        )
    }

    fn base64_url_encode(bytes: &[u8]) -> String {
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut out = String::new();
        let mut i = 0;
        while i < bytes.len() {
            let b0 = bytes[i];
            let b1 = *bytes.get(i + 1).unwrap_or(&0);
            let b2 = *bytes.get(i + 2).unwrap_or(&0);
            out.push(TABLE[(b0 >> 2) as usize] as char);
            out.push(TABLE[(((b0 & 0b11) << 4) | (b1 >> 4)) as usize] as char);
            if i + 1 < bytes.len() {
                out.push(TABLE[(((b1 & 0b1111) << 2) | (b2 >> 6)) as usize] as char);
            }
            if i + 2 < bytes.len() {
                out.push(TABLE[(b2 & 0b11_1111) as usize] as char);
            }
            i += 3;
        }
        out
    }
}
