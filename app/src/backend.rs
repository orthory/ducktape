use std::fmt::Write as _;
use std::io::Read as _;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chat::{AuthorRef, ChatMsg, ChatQuery, ChatReply, PostPolicy};
use futures::StreamExt as _;
use pages::{BlockKind, NewBlock, PageMsg, PageQuery, PageReply};
use reqwest::{Client, Response, Url};
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::io::AsyncWriteExt as _;
use zeroize::{Zeroize as _, Zeroizing};

const DEFAULT_RPC: &str = "http://127.0.0.1:8844";
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const MAX_ERROR_BYTES: usize = 4 * 1024;
const MAX_SIGNED_PAYLOAD_BYTES: usize = 23 * 1024;
const MAX_KEY_FILE_BYTES: u64 = 64 * 1024;
const MAX_FRAME_HEX_BYTES: usize = 3 * 1024 * 1024;
const ENCRYPTED_KEY_PREFIX: &str = "ducktape-user-key-v1:";
const RPC_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ChatChannel {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ChatMessage {
    pub author: String,
    pub meta: String,
    pub body: String,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ChatData {
    pub channels: Vec<ChatChannel>,
    pub messages: Vec<ChatMessage>,
    pub active_channel: String,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct PageItem {
    pub id: String,
    pub title: String,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct PageBlock {
    pub id: String,
    pub kind: String,
    pub text: String,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct PagesData {
    pub pages: Vec<PageItem>,
    pub blocks: Vec<PageBlock>,
    pub active_page: String,
    pub active_page_title: String,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct WorkspaceData {
    pub rpc: String,
    pub status: String,
    pub channels: Vec<ChatChannel>,
    pub messages: Vec<ChatMessage>,
    pub active_channel: String,
    pub pages: Vec<PageItem>,
    pub blocks: Vec<PageBlock>,
    pub active_page: String,
    pub active_page_title: String,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct AppError {
    pub message: String,
}

#[derive(Clone)]
struct Rpc {
    origin: String,
    base: Url,
    client: Client,
}

#[derive(Serialize)]
struct QueryRequest<'a, Q> {
    target: &'a str,
    query: &'a Q,
}

#[derive(serde::Deserialize)]
struct NodeStatus {
    height: u64,
}

impl Rpc {
    fn new(input: &str) -> Result<Self, String> {
        let configured = if input.trim().is_empty() {
            std::env::var("DUCKTAPE_NODE").unwrap_or_else(|_| DEFAULT_RPC.to_string())
        } else {
            input.trim().to_string()
        };
        let mut base = Url::parse(&configured).map_err(|_| "RPC endpoint is not a URL")?;
        let invalid_origin = !matches!(base.scheme(), "http" | "https")
            || base.host_str().is_none()
            || !base.username().is_empty()
            || base.password().is_some()
            || base.query().is_some()
            || base.fragment().is_some()
            || !matches!(base.path(), "" | "/");
        if invalid_origin {
            return Err(
                "RPC endpoint must be an http(s) origin without credentials or a path".into(),
            );
        }
        base.set_path("/");
        let origin = base.as_str().trim_end_matches('/').to_string();
        let client = Client::builder()
            .timeout(RPC_TIMEOUT)
            .build()
            .map_err(|error| format!("could not initialize RPC client: {error}"))?;
        Ok(Self {
            origin,
            base,
            client,
        })
    }

    fn url(&self, path: &str) -> Result<Url, String> {
        self.base
            .join(path)
            .map_err(|_| "could not build RPC URL".to_string())
    }

    async fn status(&self) -> Result<NodeStatus, String> {
        let response = self
            .client
            .get(self.url("v1/status")?)
            .send()
            .await
            .map_err(|error| format!("RPC status failed: {error}"))?;
        decode_json(response).await
    }

    async fn query<Q: Serialize, R: DeserializeOwned>(
        &self,
        target: &str,
        query: &Q,
    ) -> Result<R, String> {
        let response = self
            .client
            .post(self.url("v1/query")?)
            .json(&QueryRequest { target, query })
            .send()
            .await
            .map_err(|error| format!("{target} query failed: {error}"))?;
        decode_json(response).await
    }

    async fn submit_frame(&self, frame: Vec<u8>) -> Result<(), String> {
        let response = self
            .client
            .post(self.url("v1/submit/frame")?)
            .header("content-type", "application/octet-stream")
            .body(frame)
            .send()
            .await
            .map_err(|error| format!("transaction submission failed: {error}"))?;
        if response.status().is_success() {
            return Ok(());
        }
        Err(response_error(response).await)
    }
}

pub async fn connect(rpc: String) -> Result<WorkspaceData, AppError> {
    async {
        let rpc = Rpc::new(&rpc)?;
        let status = rpc.status().await?;
        let chat = load_chat_data(&rpc, None).await?;
        let pages = load_pages_data(&rpc, None).await?;
        Ok(WorkspaceData {
            rpc: rpc.origin,
            status: format!("Connected · block {}", status.height),
            channels: chat.channels,
            messages: chat.messages,
            active_channel: chat.active_channel,
            pages: pages.pages,
            blocks: pages.blocks,
            active_page: pages.active_page,
            active_page_title: pages.active_page_title,
        })
    }
    .await
    .map_err(app_error)
}

pub async fn load_chat(rpc: String, channel_id: String) -> Result<ChatData, AppError> {
    async {
        let rpc = Rpc::new(&rpc)?;
        load_chat_data(&rpc, Some(&channel_id)).await
    }
    .await
    .map_err(app_error)
}

pub async fn create_channel(
    rpc: String,
    password: String,
    name: String,
) -> Result<ChatData, AppError> {
    async {
        let name = bounded_text(name, "channel name", 128)?;
        let channel_id = fresh_id("channel");
        let rpc = Rpc::new(&rpc)?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::CreateChannel {
                channel_id: channel_id.clone(),
                name,
                post_policy: PostPolicy::Open,
            }),
            password,
        )
        .await?;
        load_chat_data(&rpc, Some(&channel_id)).await
    }
    .await
    .map_err(app_error)
}

pub async fn send_message(
    rpc: String,
    password: String,
    channel_id: String,
    body: String,
) -> Result<ChatData, AppError> {
    async {
        if channel_id.is_empty() {
            return Err("choose a channel first".to_string());
        }
        let body = bounded_text(body, "message", 16 * 1024)?;
        let rpc = Rpc::new(&rpc)?;
        signed_write(
            &rpc,
            "chat",
            chat::encode_msg(&ChatMsg::PostMessage {
                channel_id: channel_id.clone(),
                message_id: fresh_id("message"),
                blocks: vec![chat::Block::paragraph(body)],
                thread: None,
                as_agent: None,
            }),
            password,
        )
        .await?;
        load_chat_data(&rpc, Some(&channel_id)).await
    }
    .await
    .map_err(app_error)
}

pub async fn load_page(rpc: String, page_id: String) -> Result<PagesData, AppError> {
    async {
        let rpc = Rpc::new(&rpc)?;
        load_pages_data(&rpc, Some(&page_id)).await
    }
    .await
    .map_err(app_error)
}

pub async fn create_page(
    rpc: String,
    password: String,
    title: String,
) -> Result<PagesData, AppError> {
    async {
        let title = bounded_text(title, "page title", 512)?;
        let page_id = fresh_id("page");
        let rpc = Rpc::new(&rpc)?;
        signed_write(
            &rpc,
            "pages",
            pages::encode_msg(&PageMsg::CreatePage {
                page_id: page_id.clone(),
                title,
                parent: None,
            }),
            password,
        )
        .await?;
        load_pages_data(&rpc, Some(&page_id)).await
    }
    .await
    .map_err(app_error)
}

pub async fn rename_page(
    rpc: String,
    password: String,
    page_id: String,
    title: String,
) -> Result<PagesData, AppError> {
    async {
        if page_id.is_empty() {
            return Err("choose a page first".to_string());
        }
        let title = bounded_text(title, "page title", 512)?;
        let rpc = Rpc::new(&rpc)?;
        signed_write(
            &rpc,
            "pages",
            pages::encode_msg(&PageMsg::UpdateText {
                block_id: page_id.clone(),
                text: title,
                marks: None,
            }),
            password,
        )
        .await?;
        load_pages_data(&rpc, Some(&page_id)).await
    }
    .await
    .map_err(app_error)
}

pub async fn add_paragraph(
    rpc: String,
    password: String,
    page_id: String,
    text: String,
) -> Result<PagesData, AppError> {
    async {
        if page_id.is_empty() {
            return Err("choose a page first".to_string());
        }
        let text = bounded_text(text, "paragraph", 64 * 1024)?;
        let rpc = Rpc::new(&rpc)?;
        let reply: PageReply = rpc
            .query(
                "pages",
                &PageQuery::GetPage {
                    page_id: page_id.clone(),
                },
            )
            .await?;
        let blocks = match reply {
            PageReply::Page(Some(blocks)) => blocks,
            _ => return Err("page was not found".into()),
        };
        let root = blocks
            .first()
            .filter(|block| block.kind == BlockKind::Page)
            .ok_or_else(|| "page has no root block".to_string())?;
        signed_write(
            &rpc,
            "pages",
            pages::encode_msg(&PageMsg::InsertBlock {
                parent: page_id.clone(),
                after: root.children.last().cloned(),
                block: NewBlock {
                    id: fresh_id("block"),
                    kind: BlockKind::Paragraph,
                    text,
                    marks: Vec::new(),
                },
            }),
            password,
        )
        .await?;
        load_pages_data(&rpc, Some(&page_id)).await
    }
    .await
    .map_err(app_error)
}

async fn load_chat_data(rpc: &Rpc, requested: Option<&str>) -> Result<ChatData, String> {
    let reply: ChatReply = rpc.query("chat", &ChatQuery::Channels).await?;
    let wire_channels = match reply {
        ChatReply::Channels(channels) => channels,
        _ => return Err("node returned an invalid channel list".into()),
    };
    let channels = wire_channels
        .iter()
        .filter(|channel| !channel.archived && !channel.id.contains(':'))
        .map(|channel| ChatChannel {
            id: channel.id.clone(),
            name: channel.name.clone(),
        })
        .collect::<Vec<_>>();
    let active_channel = requested
        .filter(|id| channels.iter().any(|channel| channel.id == *id))
        .map(str::to_string)
        .or_else(|| channels.first().map(|channel| channel.id.clone()))
        .unwrap_or_default();
    let messages = if active_channel.is_empty() {
        Vec::new()
    } else {
        load_messages(rpc, &active_channel).await?
    };
    Ok(ChatData {
        channels,
        messages,
        active_channel,
    })
}

async fn load_messages(rpc: &Rpc, channel_id: &str) -> Result<Vec<ChatMessage>, String> {
    let reply: ChatReply = rpc
        .query(
            "chat",
            &ChatQuery::MessagesLatest {
                channel_id: channel_id.to_string(),
                limit: 128,
            },
        )
        .await?;
    let messages = match reply {
        ChatReply::Messages(messages) => messages,
        _ => return Err("node returned an invalid message list".into()),
    };
    Ok(messages
        .into_iter()
        .filter(|message| message.head.thread.is_none())
        .map(|message| ChatMessage {
            author: author_name(&message.head.author),
            meta: format!("#{}", message.seq),
            body: if message.head.deleted {
                "Message deleted".into()
            } else {
                message_body(&message.head.blocks)
            },
        })
        .collect())
}

async fn load_pages_data(rpc: &Rpc, requested: Option<&str>) -> Result<PagesData, String> {
    let reply: PageReply = rpc.query("pages", &PageQuery::ListPages).await?;
    let wire_pages = match reply {
        PageReply::PageList(pages) => pages,
        _ => return Err("node returned an invalid page list".into()),
    };
    let pages = wire_pages
        .into_iter()
        .map(|page| PageItem {
            id: page.id,
            title: page.title,
        })
        .collect::<Vec<_>>();
    let active_page = requested
        .filter(|id| pages.iter().any(|page| page.id == *id))
        .map(str::to_string)
        .or_else(|| pages.first().map(|page| page.id.clone()))
        .unwrap_or_default();
    if active_page.is_empty() {
        return Ok(PagesData {
            pages,
            blocks: Vec::new(),
            active_page,
            active_page_title: String::new(),
        });
    }
    let reply: PageReply = rpc
        .query(
            "pages",
            &PageQuery::GetPage {
                page_id: active_page.clone(),
            },
        )
        .await?;
    let wire_blocks = match reply {
        PageReply::Page(Some(blocks)) => blocks,
        _ => return Err("page was not found".into()),
    };
    let active_page_title = wire_blocks
        .first()
        .map(|block| block.text.clone())
        .unwrap_or_default();
    let blocks = wire_blocks
        .into_iter()
        .skip(1)
        .map(|block| PageBlock {
            id: block.id,
            kind: block_kind_name(block.kind).into(),
            text: block.text,
        })
        .collect();
    Ok(PagesData {
        pages,
        blocks,
        active_page,
        active_page_title,
    })
}

async fn signed_write(
    rpc: &Rpc,
    target: &str,
    payload: Vec<u8>,
    password: String,
) -> Result<(), String> {
    if payload.is_empty() || payload.len() > MAX_SIGNED_PAYLOAD_BYTES {
        return Err(format!(
            "{target} transaction exceeds the signed payload limit"
        ));
    }
    let frame = sign_frame(target, &payload, password).await?;
    rpc.submit_frame(frame).await
}

async fn sign_frame(target: &str, payload: &[u8], mut password: String) -> Result<Vec<u8>, String> {
    let key = user_key_path()?;
    let encrypted = key_is_encrypted(&key)?;
    let payload_hex = hex_encode(payload);
    let input = signing_input(encrypted, &password, &payload_hex);
    password.zeroize();
    let input = Zeroizing::new(input?);
    let mut command = tokio::process::Command::new(ducktape_binary());
    command
        .arg("user")
        .arg("sign-frame")
        .arg("--key")
        .arg(&key)
        .arg("--target")
        .arg(target)
        .arg("--seq")
        .arg(next_sequence().to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(|error| {
        format!("could not start the ducktape signer ({error}); build node-bin or set DUCKTAPE_BIN")
    })?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "ducktape signer stdin is unavailable".to_string())?;
    stdin
        .write_all(&input)
        .await
        .map_err(|error| format!("could not send payload to signer: {error}"))?;
    drop(stdin);
    let output = tokio::time::timeout(RPC_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| "ducktape signer timed out".to_string())?
        .map_err(|error| format!("ducktape signer failed: {error}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "ducktape signer refused the transaction: {}",
            bounded_detail(&detail)
        ));
    }
    let stdout = std::str::from_utf8(&output.stdout)
        .map_err(|_| "ducktape signer returned non-UTF-8 output".to_string())?;
    let frame_hex = stdout
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| "ducktape signer returned no frame".to_string())?;
    hex_decode(frame_hex.trim())
}

fn signing_input(encrypted: bool, password: &str, payload_hex: &str) -> Result<Vec<u8>, String> {
    let invalid_password = password.len() > 16 * 1024
        || password
            .as_bytes()
            .iter()
            .any(|byte| matches!(byte, 0 | b'\n' | b'\r'));
    if invalid_password {
        return Err("key password is too long or contains a line delimiter".into());
    }
    if encrypted && password.is_empty() {
        return Err("the local user key is locked; enter its password".into());
    }
    let mut input = Vec::with_capacity(password.len() + payload_hex.len() + 2);
    if encrypted {
        input.extend_from_slice(password.as_bytes());
        input.push(b'\n');
    }
    input.extend_from_slice(payload_hex.as_bytes());
    input.push(b'\n');
    Ok(input)
}

fn user_key_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("DUCKTAPE_USER_KEY") {
        return Ok(path.into());
    }
    if let Some(root) = std::env::var_os("DUCKTAPE_HOME") {
        return Ok(PathBuf::from(root).join("user.key"));
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".ducktape/user.key"))
        .ok_or_else(|| "cannot locate local user.key; set DUCKTAPE_USER_KEY".to_string())
}

fn key_is_encrypted(path: &std::path::Path) -> Result<bool, String> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("cannot read local user key at {}: {error}", path.display()))?;
    if metadata.len() > MAX_KEY_FILE_BYTES {
        return Err("local user key file is unexpectedly large".into());
    }
    let mut file = std::fs::File::open(path)
        .map_err(|error| format!("cannot read local user key at {}: {error}", path.display()))?;
    let mut prefix = [0; ENCRYPTED_KEY_PREFIX.len()];
    let read = file
        .read(&mut prefix)
        .map_err(|error| format!("cannot read local user key at {}: {error}", path.display()))?;
    let encrypted = read == prefix.len() && prefix == ENCRYPTED_KEY_PREFIX.as_bytes();
    prefix.zeroize();
    Ok(encrypted)
}

fn ducktape_binary() -> PathBuf {
    if let Some(path) = std::env::var_os("DUCKTAPE_BIN") {
        return path.into();
    }
    if let Ok(current) = std::env::current_exe()
        && let Some(sibling) = current.parent().map(|parent| parent.join("ducktape"))
        && sibling.is_file()
    {
        return sibling;
    }
    PathBuf::from("ducktape")
}

async fn decode_json<T: DeserializeOwned>(response: Response) -> Result<T, String> {
    let status = response.status();
    let bytes = read_bounded(response, MAX_RESPONSE_BYTES).await?;
    if !status.is_success() {
        return Err(format!(
            "RPC returned {status}: {}",
            bounded_detail(&String::from_utf8_lossy(&bytes))
        ));
    }
    serde_json::from_slice(&bytes).map_err(|error| format!("RPC returned invalid JSON: {error}"))
}

async fn response_error(response: Response) -> String {
    let status = response.status();
    match read_bounded(response, MAX_ERROR_BYTES).await {
        Ok(bytes) => format!(
            "transaction was rejected ({status}): {}",
            bounded_detail(&String::from_utf8_lossy(&bytes))
        ),
        Err(error) => format!("transaction was rejected ({status}): {error}"),
    }
}

async fn read_bounded(response: Response, limit: usize) -> Result<Vec<u8>, String> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err("RPC response exceeds the desktop limit".into());
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| format!("could not read RPC response: {error}"))?;
        if bytes.len().saturating_add(chunk.len()) > limit {
            return Err("RPC response exceeds the desktop limit".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn bounded_text(value: String, field: &str, limit: usize) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.len() > limit || value.chars().any(|character| character == '\0') {
        return Err(format!("{field} must be between 1 and {limit} bytes"));
    }
    Ok(value.to_string())
}

fn app_error(message: String) -> AppError {
    AppError { message }
}

fn message_body(blocks: &[chat::Block]) -> String {
    blocks
        .iter()
        .map(|block| match block {
            chat::Block::Paragraph(spans) => span_text(spans),
            chat::Block::Code { lang, text } => match lang {
                Some(lang) => format!("{lang}\n{text}"),
                None => text.clone(),
            },
            chat::Block::Quote(spans) => format!("“{}”", span_text(spans)),
            chat::Block::Divider => "────────".into(),
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn span_text(spans: &[chat::Span]) -> String {
    spans.iter().map(|span| span.text.as_str()).collect()
}

fn author_name(author: &AuthorRef) -> String {
    match author {
        AuthorRef::User(key) => format!("user {}", short_hex(key)),
        AuthorRef::Agent { agent_id, .. } => format!("@{agent_id}"),
        AuthorRef::Module(module) => module.clone(),
        AuthorRef::System => "system".into(),
    }
}

fn short_hex(bytes: &[u8]) -> String {
    let mut output = String::new();
    for byte in bytes.iter().take(4) {
        let _ = write!(output, "{byte:02x}");
    }
    if bytes.len() > 4 {
        output.push('…');
    }
    output
}

const fn block_kind_name(kind: BlockKind) -> &'static str {
    match kind {
        BlockKind::Page => "Page",
        BlockKind::Paragraph => "Text",
        BlockKind::Heading1 => "Heading 1",
        BlockKind::Heading2 => "Heading 2",
        BlockKind::Heading3 => "Heading 3",
        BlockKind::Bulleted => "Bullet",
        BlockKind::Numbered => "Number",
        BlockKind::Todo => "Todo",
        BlockKind::Toggle => "Toggle",
        BlockKind::Quote => "Quote",
        BlockKind::Code => "Code",
        BlockKind::Callout => "Callout",
        BlockKind::Divider => "Divider",
    }
}

fn bounded_detail(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        return "no detail".into();
    }
    value.chars().take(300).collect()
}

fn next_sequence() -> u64 {
    static SEQUENCE: OnceLock<AtomicU64> = OnceLock::new();
    SEQUENCE
        .get_or_init(|| AtomicU64::new(epoch_nanos() as u64))
        .fetch_add(1, Ordering::Relaxed)
}

fn fresh_id(prefix: &str) -> String {
    format!("{prefix}-{}-{}", epoch_nanos(), next_sequence())
}

fn epoch_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn hex_decode(value: &str) -> Result<Vec<u8>, String> {
    let valid = !value.is_empty()
        && value.len() <= MAX_FRAME_HEX_BYTES
        && value.len().is_multiple_of(2)
        && value.bytes().all(|byte| byte.is_ascii_hexdigit());
    if !valid {
        return Err("ducktape signer returned an invalid frame".into());
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).expect("validated ASCII hex");
            u8::from_str_radix(pair, 16).map_err(|_| "ducktape signer returned invalid hex".into())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use commonware_cryptography::{Signer as _, ed25519};

    use super::*;

    #[test]
    fn signer_stdin_only_includes_a_password_for_encrypted_keys() {
        assert_eq!(signing_input(false, "ignored", "00").unwrap(), b"00\n");
        assert_eq!(
            signing_input(true, "secret", "00").unwrap(),
            b"secret\n00\n"
        );
        assert!(signing_input(true, "", "00").is_err());
        assert!(signing_input(true, "bad\nsecret", "00").is_err());

        let directory = tempfile::tempdir().unwrap();
        let key = directory.path().join("user.key");
        std::fs::write(&key, format!("{ENCRYPTED_KEY_PREFIX}ciphertext")).unwrap();
        assert!(key_is_encrypted(&key).unwrap());
        std::fs::write(&key, "plaintext-key").unwrap();
        assert!(!key_is_encrypted(&key).unwrap());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn chat_and_pages_round_trip_over_signed_frames() {
        let storage = tempfile::tempdir().unwrap();
        let sim = simnode::boot(
            storage.path(),
            "127.0.0.1:0".parse().unwrap(),
            simnode::SimOpts {
                auto: true,
                ..Default::default()
            },
        )
        .unwrap();
        let rpc = Rpc::new(&format!("http://{}", sim.addr())).unwrap();
        let signer = ed25519::PrivateKey::from_seed(7);

        submit_test(
            &rpc,
            &signer,
            1,
            "chat",
            chat::encode_msg(&ChatMsg::CreateChannel {
                channel_id: "general".into(),
                name: "General".into(),
                post_policy: PostPolicy::Open,
            }),
        )
        .await;
        submit_test(
            &rpc,
            &signer,
            2,
            "chat",
            chat::encode_msg(&ChatMsg::PostMessage {
                channel_id: "general".into(),
                message_id: "hello-1".into(),
                blocks: vec![chat::Block::paragraph("hello from the app")],
                thread: None,
                as_agent: None,
            }),
        )
        .await;
        submit_test(
            &rpc,
            &signer,
            3,
            "pages",
            pages::encode_msg(&PageMsg::CreatePage {
                page_id: "welcome".into(),
                title: "Welcome".into(),
                parent: None,
            }),
        )
        .await;
        submit_test(
            &rpc,
            &signer,
            4,
            "pages",
            pages::encode_msg(&PageMsg::InsertBlock {
                parent: "welcome".into(),
                after: None,
                block: NewBlock {
                    id: "intro".into(),
                    kind: BlockKind::Paragraph,
                    text: "A signed page block".into(),
                    marks: Vec::new(),
                },
            }),
        )
        .await;

        let chat = load_chat_data(&rpc, Some("general")).await.unwrap();
        assert_eq!(chat.channels[0].name, "General");
        assert_eq!(chat.messages[0].body, "hello from the app");
        let pages = load_pages_data(&rpc, Some("welcome")).await.unwrap();
        assert_eq!(pages.active_page_title, "Welcome");
        assert_eq!(pages.blocks[0].text, "A signed page block");
        sim.shutdown();
    }

    async fn submit_test(
        rpc: &Rpc,
        signer: &ed25519::PrivateKey,
        sequence: u64,
        target: &str,
        payload: Vec<u8>,
    ) {
        let frame = node::encode_frame(
            signer,
            sequence,
            &sdk::Msg {
                target: target.into(),
                payload,
            },
            None,
        );
        rpc.submit_frame(frame).await.unwrap();
    }
}
